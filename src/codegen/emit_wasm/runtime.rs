use super::*;

pub(super) struct HostImport {
  pub(super) module: &'static str,
  pub(super) name: &'static str,
  pub(super) arity: usize,
}

/// List of host-imported functions.
/// These are provided by the JS environment and indexed before user functions.
pub(super) const HOST_IMPORTS: &[HostImport] = &[
  HostImport {
    module: "math",
    name: "pow",
    arity: 2,
  },
  HostImport {
    module: "math",
    name: "sin",
    arity: 1,
  },
  HostImport {
    module: "math",
    name: "cos",
    arity: 1,
  },
  // IO: log a single value (f64) — host reads memory to decode type
  HostImport {
    module: "io",
    name: "log_value",
    arity: 1,
  },
];

/// Maximum arity covered by canonical call_indirect type entries.
/// Types 0..MAX_CANONICAL_ARITY are reserved: type N = (f64 × N) → f64.
pub(super) const MAX_CANONICAL_ARITY: u32 = 8;

/// Build a binary WASM module from compiled functions.
/// Host imports occupy the first function indices (0..HOST_IMPORTS.len()),
/// then user functions follow at indices HOST_IMPORTS.len()..
///
/// `runtime_fn_count`: how many leading entries in `fns` are runtime helper
/// functions (not user-defined calcit functions). Only the user calcit fns
/// (fns[runtime_fn_count..]) are registered in the funcref table.
pub(super) fn build_wasm_module(
  fns: &[CompiledFn],
  heap_start: i32,
  string_data: &[u8],
  atom_initial_values: &[f64],
  lazy_count: u32,
  runtime_fn_count: u32,
) -> Result<Vec<u8>, String> {
  let mut module = Module::new();
  let num_imports = HOST_IMPORTS.len() as u32;
  let user_fn_count = fns.len() as u32 - runtime_fn_count;

  // Type section:
  //   0..MAX_CANONICAL_ARITY  — canonical HOF callback types: (f64×N) → f64
  //   MAX_CANONICAL_ARITY + 0..HOST_IMPORTS.len() — host import types
  //   MAX_CANONICAL_ARITY + HOST_IMPORTS.len() + 0..fns.len() — user fn types
  let mut types = TypeSection::new();
  for arity in 0..MAX_CANONICAL_ARITY {
    types.ty().function(vec![ValType::F64; arity as usize], vec![ValType::F64]);
  }
  for imp in HOST_IMPORTS {
    let params: Vec<ValType> = vec![ValType::F64; imp.arity];
    types.ty().function(params, vec![ValType::F64]);
  }
  for f in fns {
    types.ty().function(f.params.clone(), f.results.clone());
  }
  module.section(&types);

  // Import section: host functions (type indices shifted by MAX_CANONICAL_ARITY)
  let mut imports = wasm_encoder::ImportSection::new();
  for (i, imp) in HOST_IMPORTS.iter().enumerate() {
    imports.import(
      imp.module,
      imp.name,
      wasm_encoder::EntityType::Function(MAX_CANONICAL_ARITY + i as u32),
    );
  }
  module.section(&imports);

  // Function section: map each user function to its type
  let mut functions = FunctionSection::new();
  for (i, _) in fns.iter().enumerate() {
    functions.function(MAX_CANONICAL_ARITY + num_imports + i as u32);
  }
  module.section(&functions);

  // Table section: one funcref table holding all user calcit functions.
  // Table slot i → calcit fn at fn_defs[i] (function index: num_imports + runtime_fn_count + i).
  let mut tables = TableSection::new();
  tables.table(TableType {
    element_type: RefType::FUNCREF,
    minimum: user_fn_count as u64,
    maximum: Some(user_fn_count as u64),
    table64: false,
    shared: false,
  });
  module.section(&tables);

  // Memory section: 100 pages (6.4MB) for linear memory (records, tuples)
  let mut memories = MemorySection::new();
  memories.memory(MemoryType {
    minimum: 100,
    maximum: None,
    memory64: false,
    shared: false,
    page_size_log2: None,
  });
  module.section(&memories);

  // Global section: heap pointer, atom globals, lazy val globals, lazy flag globals
  let mut globals = GlobalSection::new();
  globals.global(
    GlobalType {
      val_type: ValType::I32,
      mutable: true,
      shared: false,
    },
    &ConstExpr::i32_const(heap_start),
  );
  for &init_val in atom_initial_values {
    globals.global(
      GlobalType {
        val_type: ValType::F64,
        mutable: true,
        shared: false,
      },
      &ConstExpr::f64_const(init_val.into()),
    );
  }
  // Lazy value globals (f64, init 0.0)
  for _ in 0..lazy_count {
    globals.global(
      GlobalType {
        val_type: ValType::F64,
        mutable: true,
        shared: false,
      },
      &ConstExpr::f64_const(wasm_encoder::Ieee64::from(0.0f64)),
    );
  }
  // Lazy flag globals (i32, init 0)
  for _ in 0..lazy_count {
    globals.global(
      GlobalType {
        val_type: ValType::I32,
        mutable: true,
        shared: false,
      },
      &ConstExpr::i32_const(0),
    );
  }
  module.section(&globals);

  // Export section: memory, heap pointer global, and named functions
  let mut exports = ExportSection::new();
  exports.export("memory", ExportKind::Memory, 0);
  exports.export("__heap_ptr", ExportKind::Global, HEAP_PTR_GLOBAL);
  for (i, f) in fns.iter().enumerate() {
    if let Some(export_name) = &f.export_name {
      exports.export(export_name, ExportKind::Func, num_imports + i as u32);
    }
  }
  module.section(&exports);

  // Element section: populate the funcref table with user calcit function indices.
  // table slot i → function index (num_imports + runtime_fn_count + i).
  if user_fn_count > 0 {
    let fn_indices: Vec<u32> = (0..user_fn_count).map(|i| num_imports + runtime_fn_count + i).collect();
    let mut elements = ElementSection::new();
    elements.active(Some(0), &ConstExpr::i32_const(0), Elements::Functions(fn_indices.as_slice().into()));
    module.section(&elements);
  }

  // Code section
  let mut codes = CodeSection::new();
  for f in fns {
    let locals: Vec<(u32, ValType)> = if f.locals.is_empty() {
      vec![]
    } else {
      // Group consecutive identical types
      let mut groups = Vec::new();
      let mut count = 1u32;
      let mut prev = f.locals[0];
      for &t in &f.locals[1..] {
        if t == prev {
          count += 1;
        } else {
          groups.push((count, prev));
          prev = t;
          count = 1;
        }
      }
      groups.push((count, prev));
      groups
    };

    let mut func = Function::new(locals);
    for instr in &f.instructions {
      func.instruction(instr);
    }
    func.instruction(&Instruction::End);
    codes.function(&func);
  }
  module.section(&codes);

  // Data section: string literals pre-allocated before the heap
  if !string_data.is_empty() {
    let mut data = wasm_encoder::DataSection::new();
    data.active(0, &ConstExpr::i32_const(HEAP_BASE), string_data.iter().copied());
    module.section(&data);
  }

  Ok(module.finish())
}

pub(super) fn build_runtime_fns(
  base_index: u32,
  map_tag: i32,
  list_tag: i32,
  string_tag: i32,
) -> (Vec<CompiledFn>, HashMap<String, u32>) {
  let mut fn_index = HashMap::new();
  let mut fns = Vec::new();

  let copy_name = String::from("__rt_copy_f64_slots");
  fn_index.insert(copy_name.clone(), base_index + fns.len() as u32);

  let i_local = 3u32;
  let copy_instructions = vec![
    Instruction::I32Const(0),
    Instruction::LocalSet(i_local),
    Instruction::Block(wasm_encoder::BlockType::Empty),
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(i_local),
    Instruction::LocalGet(2),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    Instruction::LocalGet(0),
    Instruction::LocalGet(i_local),
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::LocalGet(1),
    Instruction::LocalGet(i_local),
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::F64Store(mem_arg_f64(0)),
    Instruction::LocalGet(i_local),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(i_local),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
  ];
  fns.push(CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32, ValType::I32],
    results: vec![],
    locals: vec![ValType::I32],
    instructions: copy_instructions,
  });

  let map_flat_pairs_name = String::from("__rt_map_flat_pairs");
  fn_index.insert(map_flat_pairs_name, base_index + fns.len() as u32);
  let map_flat_pairs_instructions = vec![
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(8)),
    Instruction::I32TruncF64U,
  ];
  fns.push(CompiledFn {
    export_name: None,
    params: vec![ValType::I32],
    results: vec![ValType::I32],
    locals: vec![],
    instructions: map_flat_pairs_instructions,
  });

  let map_find_key_name = String::from("__rt_map_find_key");
  fn_index.insert(map_find_key_name, base_index + fns.len() as u32);
  let map_find_key_instructions = vec![
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(8)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(5),
    Instruction::LocalGet(5),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(2),
    Instruction::I32Const(-1),
    Instruction::LocalSet(4),
    Instruction::I32Const(0),
    Instruction::LocalSet(3),
    Instruction::Block(wasm_encoder::BlockType::Empty),
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(3),
    Instruction::LocalGet(2),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    Instruction::LocalGet(5),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(3),
    Instruction::I32Const(16),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::LocalGet(1),
    Instruction::F64Eq,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(3),
    Instruction::LocalSet(4),
    Instruction::Br(2),
    Instruction::End,
    Instruction::LocalGet(3),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(3),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(4),
  ];
  fns.push(CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::F64],
    results: vec![ValType::I32],
    locals: vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32],
    instructions: map_find_key_instructions,
  });

  let map_find_value_name = String::from("__rt_map_find_value");
  fn_index.insert(map_find_value_name, base_index + fns.len() as u32);
  let map_find_value_instructions = vec![
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(8)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(5),
    Instruction::LocalGet(5),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(2),
    Instruction::I32Const(-1),
    Instruction::LocalSet(4),
    Instruction::I32Const(0),
    Instruction::LocalSet(3),
    Instruction::Block(wasm_encoder::BlockType::Empty),
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(3),
    Instruction::LocalGet(2),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    Instruction::LocalGet(5),
    Instruction::I32Const(16),
    Instruction::I32Add,
    Instruction::LocalGet(3),
    Instruction::I32Const(16),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::LocalGet(1),
    Instruction::F64Eq,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(3),
    Instruction::LocalSet(4),
    Instruction::Br(2),
    Instruction::End,
    Instruction::LocalGet(3),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(3),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(4),
  ];
  fns.push(CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::F64],
    results: vec![ValType::I32],
    locals: vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32],
    instructions: map_find_value_instructions,
  });

  let set_find_name = String::from("__rt_set_find_elem");
  fn_index.insert(set_find_name, base_index + fns.len() as u32);
  let set_find_instructions = vec![
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(2),
    Instruction::I32Const(-1),
    Instruction::LocalSet(4),
    Instruction::I32Const(0),
    Instruction::LocalSet(3),
    Instruction::Block(wasm_encoder::BlockType::Empty),
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(3),
    Instruction::LocalGet(2),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(3),
    Instruction::I32Const(8),
    Instruction::I32Mul,
    Instruction::I32Add,
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::LocalGet(1),
    Instruction::F64Eq,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(3),
    Instruction::LocalSet(4),
    Instruction::Br(2),
    Instruction::End,
    Instruction::LocalGet(3),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(3),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    Instruction::LocalGet(4),
  ];
  fns.push(CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::F64],
    results: vec![ValType::I32],
    locals: vec![ValType::I32, ValType::I32, ValType::I32],
    instructions: set_find_instructions,
  });

  let hash_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_hash_f64"), hash_idx);
  fns.push(build_rt_hash_f64_semantic(string_tag));

  // String comparison (needed by generic_eq below)
  let str_compare_early_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_str_compare_early"), str_compare_early_idx);
  fns.push(build_rt_str_compare());

  // Semantic equality: for string pointers compare content; otherwise use F64Eq.
  let generic_eq_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_generic_eq"), generic_eq_idx);
  fns.push(build_rt_generic_eq(str_compare_early_idx, string_tag));

  let map_root_assoc_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_root_assoc"), map_root_assoc_idx);
  fns.push(build_rt_map_root_assoc(
    *fn_index.get("__rt_copy_f64_slots").expect("copy helper"),
    generic_eq_idx,
  ));

  let map_root_lookup_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_root_lookup"), map_root_lookup_idx);
  fns.push(build_rt_map_root_lookup(generic_eq_idx));

  let map_root_contains_value_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_root_contains_value"), map_root_contains_value_idx);
  fns.push(build_rt_map_root_contains_value());

  let map_root_write_pairs_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_root_write_pairs"), map_root_write_pairs_idx);
  fns.push(build_rt_map_root_write_pairs());

  let map_make_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_make"), map_make_idx);
  fns.push(build_rt_map_make(map_tag));

  let map_from_flat_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_from_flat"), map_from_flat_idx);
  fns.push(build_rt_map_from_flat(hash_idx, map_root_assoc_idx, map_make_idx));

  let map_root_from_flat_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_root_from_flat"), map_root_from_flat_idx);
  fns.push(build_rt_map_root_from_flat(hash_idx, map_root_assoc_idx));

  let map_linearize_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_linearize"), map_linearize_idx);
  fns.push(build_rt_map_linearize(map_root_write_pairs_idx, list_tag));

  let map_assoc_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_assoc"), map_assoc_idx);
  fns.push(build_rt_map_assoc(hash_idx, map_root_assoc_idx, map_make_idx));

  let map_get_value_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_get_value"), map_get_value_idx);
  fns.push(build_rt_map_get_value(hash_idx, map_root_lookup_idx));

  let map_contains_key_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_contains_key"), map_contains_key_idx);
  fns.push(build_rt_map_contains_key(hash_idx, map_root_lookup_idx));

  let map_contains_value_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_contains_value"), map_contains_value_idx);
  fns.push(build_rt_map_contains_value(map_root_contains_value_idx));

  let map_dissoc_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_dissoc"), map_dissoc_idx);
  fns.push(build_rt_map_dissoc(
    *fn_index.get("__rt_copy_f64_slots").expect("copy helper"),
    map_linearize_idx,
    map_from_flat_idx,
  ));

  // String comparison helper: __rt_str_compare(ptr_a: i32, ptr_b: i32) → f64
  // Returns -1.0 / 0.0 / 1.0 for lexicographic byte order.
  let str_compare_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_str_compare"), str_compare_idx);
  fns.push(build_rt_str_compare());

  // Generic compare: __rt_generic_compare(a: f64, b: f64) → f64
  // If both values look like string pointers (heap address with string tag), use __rt_str_compare.
  // Otherwise, do numeric comparison: -1.0, 0.0, 1.0.
  let generic_compare_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_generic_compare"), generic_compare_idx);
  fns.push(build_rt_generic_compare(str_compare_idx, string_tag));

  // Substring search helper: __rt_str_find_index(h_ptr: i32, n_ptr: i32) → f64
  // Returns byte offset of first occurrence, or -1.0 if not found.
  let str_find_index_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_str_find_index"), str_find_index_idx);
  fns.push(build_rt_str_find_index());

  // Prefix check: __rt_str_starts_with(s_ptr: i32, p_ptr: i32) → f64
  // Returns 1.0 if s starts with p, else 0.0.
  let str_starts_with_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_str_starts_with"), str_starts_with_idx);
  fns.push(build_rt_str_starts_with());

  // Suffix check: __rt_str_ends_with(s_ptr: i32, suf_ptr: i32) → f64
  // Returns 1.0 if s ends with suf, else 0.0.
  let str_ends_with_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_str_ends_with"), str_ends_with_idx);
  fns.push(build_rt_str_ends_with());

  // Number-to-string: __rt_f64_to_str(value: f64) → i32 (string logical ptr)
  let f64_to_str_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_f64_to_str"), f64_to_str_idx);
  fns.push(build_rt_f64_to_str(string_tag));

  (fns, fn_index)
}

const RT_MAP_TABLE_KIND: f64 = 0.0;
const RT_MAP_BUCKET_KIND: f64 = 1.0;
const RT_MAP_TABLE_CHILDREN: i32 = 32;
const RT_MAP_TABLE_SLOTS: i32 = 33; // kind + 32 child pointers
const RT_MAP_TABLE_BYTES: i32 = RT_MAP_TABLE_SLOTS * 8;

struct RuntimeFnBuilder {
  locals: Vec<ValType>,
  next_local: u32,
  instructions: Vec<Instruction<'static>>,
}

impl RuntimeFnBuilder {
  fn new(param_count: u32) -> Self {
    Self {
      locals: Vec::new(),
      next_local: param_count,
      instructions: Vec::new(),
    }
  }

  fn alloc_i32(&mut self) -> u32 {
    let idx = self.next_local;
    self.next_local += 1;
    self.locals.push(ValType::I32);
    idx
  }

  fn alloc_f64(&mut self) -> u32 {
    let idx = self.next_local;
    self.next_local += 1;
    self.locals.push(ValType::F64);
    idx
  }

  fn emit(&mut self, instr: Instruction<'static>) {
    self.instructions.push(instr);
  }

  fn finish(self, params: Vec<ValType>, results: Vec<ValType>) -> CompiledFn {
    CompiledFn {
      export_name: None,
      params,
      results,
      locals: self.locals,
      instructions: self.instructions,
    }
  }
}

fn rt_emit_alloc_const(builder: &mut RuntimeFnBuilder, byte_size: i32, dst_local: u32, type_tag: i32) {
  let raw = builder.alloc_i32();
  builder.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  builder.emit(Instruction::LocalTee(raw));
  builder.emit(Instruction::I32Const(HEAP_MAGIC));
  builder.emit(Instruction::I32Store(mem_arg_i32(0)));
  builder.emit(Instruction::LocalGet(raw));
  builder.emit(Instruction::I32Const(4));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::I32Const(type_tag));
  builder.emit(Instruction::I32Store(mem_arg_i32(0)));
  builder.emit(Instruction::LocalGet(raw));
  builder.emit(Instruction::I32Const(8));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::LocalSet(dst_local));
  builder.emit(Instruction::LocalGet(raw));
  builder.emit(Instruction::I32Const(byte_size + 8));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
}

fn rt_emit_alloc_dynamic(builder: &mut RuntimeFnBuilder, size_local: u32, dst_local: u32, type_tag: i32) {
  let raw = builder.alloc_i32();
  builder.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  builder.emit(Instruction::LocalTee(raw));
  builder.emit(Instruction::I32Const(HEAP_MAGIC));
  builder.emit(Instruction::I32Store(mem_arg_i32(0)));
  builder.emit(Instruction::LocalGet(raw));
  builder.emit(Instruction::I32Const(4));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::I32Const(type_tag));
  builder.emit(Instruction::I32Store(mem_arg_i32(0)));
  builder.emit(Instruction::LocalGet(raw));
  builder.emit(Instruction::I32Const(8));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::LocalSet(dst_local));
  builder.emit(Instruction::LocalGet(raw));
  builder.emit(Instruction::I32Const(8));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::LocalGet(size_local));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
}

fn rt_emit_table_child_addr(builder: &mut RuntimeFnBuilder, table_local: u32, child_idx_local: u32, dst_local: u32) {
  builder.emit(Instruction::LocalGet(table_local));
  builder.emit(Instruction::I32Const(8));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::LocalGet(child_idx_local));
  builder.emit(Instruction::I32Const(8));
  builder.emit(Instruction::I32Mul);
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::LocalSet(dst_local));
}

fn rt_emit_load_table_child(builder: &mut RuntimeFnBuilder, table_local: u32, child_idx_local: u32, dst_local: u32) {
  builder.emit(Instruction::LocalGet(table_local));
  builder.emit(Instruction::I32Const(8));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::LocalGet(child_idx_local));
  builder.emit(Instruction::I32Const(8));
  builder.emit(Instruction::I32Mul);
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::F64Load(mem_arg_f64(0)));
  builder.emit(Instruction::I32TruncF64U);
  builder.emit(Instruction::LocalSet(dst_local));
}

fn rt_emit_alloc_empty_table(builder: &mut RuntimeFnBuilder, dst_local: u32) {
  rt_emit_alloc_const(builder, RT_MAP_TABLE_BYTES, dst_local, 0);
  builder.emit(Instruction::LocalGet(dst_local));
  builder.emit(f64_const(RT_MAP_TABLE_KIND));
  builder.emit(Instruction::F64Store(mem_arg_f64(0)));

  let i = builder.alloc_i32();
  let addr = builder.alloc_i32();
  builder.emit(Instruction::I32Const(0));
  builder.emit(Instruction::LocalSet(i));
  builder.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  builder.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  builder.emit(Instruction::LocalGet(i));
  builder.emit(Instruction::I32Const(RT_MAP_TABLE_CHILDREN));
  builder.emit(Instruction::I32GeU);
  builder.emit(Instruction::BrIf(1));
  rt_emit_table_child_addr(builder, dst_local, i, addr);
  builder.emit(Instruction::LocalGet(addr));
  builder.emit(f64_const(0.0));
  builder.emit(Instruction::F64Store(mem_arg_f64(0)));
  builder.emit(Instruction::LocalGet(i));
  builder.emit(Instruction::I32Const(1));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::LocalSet(i));
  builder.emit(Instruction::Br(0));
  builder.emit(Instruction::End);
  builder.emit(Instruction::End);
}

fn rt_emit_alloc_bucket(builder: &mut RuntimeFnBuilder, count_local: u32, dst_local: u32) {
  let slots = builder.alloc_i32();
  let size = builder.alloc_i32();
  builder.emit(Instruction::LocalGet(count_local));
  builder.emit(Instruction::I32Const(2));
  builder.emit(Instruction::I32Mul);
  builder.emit(Instruction::I32Const(2));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::LocalSet(slots));
  builder.emit(Instruction::LocalGet(slots));
  builder.emit(Instruction::I32Const(8));
  builder.emit(Instruction::I32Mul);
  builder.emit(Instruction::LocalSet(size));
  rt_emit_alloc_dynamic(builder, size, dst_local, 0);
  builder.emit(Instruction::LocalGet(dst_local));
  builder.emit(f64_const(RT_MAP_BUCKET_KIND));
  builder.emit(Instruction::F64Store(mem_arg_f64(0)));
  builder.emit(Instruction::LocalGet(dst_local));
  builder.emit(Instruction::LocalGet(count_local));
  builder.emit(Instruction::F64ConvertI32U);
  builder.emit(Instruction::F64Store(mem_arg_f64(8)));
}

fn rt_emit_copy_slots(builder: &mut RuntimeFnBuilder, copy_fn_idx: u32, dst_local: u32, src_local: u32, count_local: u32) {
  builder.emit(Instruction::LocalGet(dst_local));
  builder.emit(Instruction::LocalGet(src_local));
  builder.emit(Instruction::LocalGet(count_local));
  builder.emit(Instruction::Call(copy_fn_idx));
}

#[allow(dead_code)]
fn build_rt_hash_f64() -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I64ReinterpretF64);
  b.emit(Instruction::I64Const(32));
  b.emit(Instruction::I64ShrU);
  b.emit(Instruction::I32WrapI64);
  b.emit(Instruction::I32Const(0x9e37_79b9u32 as i32));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Rotl);
  b.finish(vec![ValType::F64], vec![ValType::I32])
}

fn build_rt_map_make(map_tag: i32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2);
  let dst = b.alloc_i32();
  rt_emit_alloc_const(&mut b, 16, dst, map_tag);
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(8)));
  b.emit(Instruction::LocalGet(dst));
  b.finish(vec![ValType::I32, ValType::I32], vec![ValType::I32])
}

fn build_rt_map_root_assoc(copy_fn_idx: u32, generic_eq_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(4); // root, key, value, hash
  let idx0 = b.alloc_i32();
  let idx1 = b.alloc_i32();
  let table1 = b.alloc_i32();
  let bucket = b.alloc_i32();
  let new_root = b.alloc_i32();
  let new_table1 = b.alloc_i32();
  let new_bucket = b.alloc_i32();
  let bucket_count = b.alloc_i32();
  let found_idx = b.alloc_i32();
  let i = b.alloc_i32();
  let slots = b.alloc_i32();
  let addr = b.alloc_i32();
  let added = b.alloc_i32();
  let key_addr = b.alloc_i32();

  b.emit(Instruction::LocalGet(3));
  b.emit(Instruction::I32Const(31));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(idx0));
  b.emit(Instruction::LocalGet(3));
  b.emit(Instruction::I32Const(5));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::I32Const(31));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(idx1));

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  rt_emit_alloc_empty_table(&mut b, new_root);
  rt_emit_alloc_empty_table(&mut b, new_table1);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(bucket_count));
  rt_emit_alloc_bucket(&mut b, bucket_count, new_bucket);
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Store(mem_arg_f64(16)));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::F64Store(mem_arg_f64(24)));
  rt_emit_table_child_addr(&mut b, new_table1, idx1, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  rt_emit_table_child_addr(&mut b, new_root, idx0, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_table1));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::Else);

  rt_emit_load_table_child(&mut b, 0, idx0, table1);
  b.emit(Instruction::LocalGet(table1));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  rt_emit_alloc_const(&mut b, RT_MAP_TABLE_BYTES, new_root, 0);
  b.emit(Instruction::I32Const(RT_MAP_TABLE_SLOTS));
  b.emit(Instruction::LocalSet(slots));
  rt_emit_copy_slots(&mut b, copy_fn_idx, new_root, 0, slots);
  rt_emit_alloc_empty_table(&mut b, new_table1);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(bucket_count));
  rt_emit_alloc_bucket(&mut b, bucket_count, new_bucket);
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Store(mem_arg_f64(16)));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::F64Store(mem_arg_f64(24)));
  rt_emit_table_child_addr(&mut b, new_table1, idx1, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  rt_emit_table_child_addr(&mut b, new_root, idx0, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_table1));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::Else);

  rt_emit_load_table_child(&mut b, table1, idx1, bucket);
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  rt_emit_alloc_const(&mut b, RT_MAP_TABLE_BYTES, new_root, 0);
  b.emit(Instruction::I32Const(RT_MAP_TABLE_SLOTS));
  b.emit(Instruction::LocalSet(slots));
  rt_emit_copy_slots(&mut b, copy_fn_idx, new_root, 0, slots);
  rt_emit_alloc_const(&mut b, RT_MAP_TABLE_BYTES, new_table1, 0);
  b.emit(Instruction::I32Const(RT_MAP_TABLE_SLOTS));
  b.emit(Instruction::LocalSet(slots));
  rt_emit_copy_slots(&mut b, copy_fn_idx, new_table1, table1, slots);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(bucket_count));
  rt_emit_alloc_bucket(&mut b, bucket_count, new_bucket);
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Store(mem_arg_f64(16)));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::F64Store(mem_arg_f64(24)));
  rt_emit_table_child_addr(&mut b, new_table1, idx1, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  rt_emit_table_child_addr(&mut b, new_root, idx0, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_table1));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::Else);

  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(bucket_count));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::LocalSet(found_idx));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(bucket_count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalTee(key_addr));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(generic_eq_idx));
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalSet(found_idx));
  b.emit(Instruction::Br(2));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(found_idx));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(bucket_count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(slots));
  rt_emit_alloc_bucket(&mut b, slots, new_bucket);
  b.emit(Instruction::LocalGet(bucket_count));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(slots));
  rt_emit_copy_slots(&mut b, copy_fn_idx, new_bucket, bucket, slots);
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::LocalGet(bucket_count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(8)));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(bucket_count));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalTee(addr));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::F64Store(mem_arg_f64(8)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::Else);
  rt_emit_alloc_bucket(&mut b, bucket_count, new_bucket);
  b.emit(Instruction::LocalGet(bucket_count));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(slots));
  rt_emit_copy_slots(&mut b, copy_fn_idx, new_bucket, bucket, slots);
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::I32Const(24));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(found_idx));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::End);

  rt_emit_alloc_const(&mut b, RT_MAP_TABLE_BYTES, new_root, 0);
  b.emit(Instruction::I32Const(RT_MAP_TABLE_SLOTS));
  b.emit(Instruction::LocalSet(slots));
  rt_emit_copy_slots(&mut b, copy_fn_idx, new_root, 0, slots);
  rt_emit_alloc_const(&mut b, RT_MAP_TABLE_BYTES, new_table1, 0);
  b.emit(Instruction::I32Const(RT_MAP_TABLE_SLOTS));
  b.emit(Instruction::LocalSet(slots));
  rt_emit_copy_slots(&mut b, copy_fn_idx, new_table1, table1, slots);
  rt_emit_table_child_addr(&mut b, new_table1, idx1, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_bucket));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  rt_emit_table_child_addr(&mut b, new_root, idx0, addr);
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(new_table1));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(new_root));
  b.emit(Instruction::LocalGet(added));

  b.finish(
    vec![ValType::I32, ValType::F64, ValType::F64, ValType::I32],
    vec![ValType::I32, ValType::I32],
  )
}

fn build_rt_map_root_lookup(generic_eq_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(3); // root, key, hash
  let idx0 = b.alloc_i32();
  let idx1 = b.alloc_i32();
  let table1 = b.alloc_i32();
  let bucket = b.alloc_i32();
  let count = b.alloc_i32();
  let i = b.alloc_i32();
  let found = b.alloc_i32();
  let value = b.alloc_f64();

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(found));
  b.emit(f64_const(0.0));
  b.emit(Instruction::LocalSet(value));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32Const(31));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(idx0));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32Const(5));
  b.emit(Instruction::I32ShrU);
  b.emit(Instruction::I32Const(31));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(idx1));
  rt_emit_load_table_child(&mut b, 0, idx0, table1);
  b.emit(Instruction::LocalGet(table1));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  rt_emit_load_table_child(&mut b, table1, idx1, bucket);
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(generic_eq_idx));
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Const(24));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(value));
  b.emit(Instruction::Br(2));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(found));
  b.emit(Instruction::LocalGet(value));
  b.finish(vec![ValType::I32, ValType::F64, ValType::I32], vec![ValType::I32, ValType::F64])
}

fn build_rt_map_root_contains_value() -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2); // root, target
  let i0 = b.alloc_i32();
  let i1 = b.alloc_i32();
  let table1 = b.alloc_i32();
  let bucket = b.alloc_i32();
  let count = b.alloc_i32();
  let bi = b.alloc_i32();
  let found = b.alloc_i32();

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i0));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i0));
  b.emit(Instruction::I32Const(RT_MAP_TABLE_CHILDREN));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  rt_emit_load_table_child(&mut b, 0, i0, table1);
  b.emit(Instruction::LocalGet(table1));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i1));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i1));
  b.emit(Instruction::I32Const(RT_MAP_TABLE_CHILDREN));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  rt_emit_load_table_child(&mut b, table1, i1, bucket);
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(bi));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(bi));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Const(24));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(bi));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::Br(6));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(bi));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(bi));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i1));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i1));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i0));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i0));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(found));
  b.finish(vec![ValType::I32, ValType::F64], vec![ValType::I32])
}

fn build_rt_map_root_write_pairs() -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(3); // root, dst_base, write_idx
  let i0 = b.alloc_i32();
  let i1 = b.alloc_i32();
  let table1 = b.alloc_i32();
  let bucket = b.alloc_i32();
  let count = b.alloc_i32();
  let bi = b.alloc_i32();
  let out = b.alloc_i32();
  let addr = b.alloc_i32();
  let tmp = b.alloc_f64();

  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::LocalSet(out));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i0));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i0));
  b.emit(Instruction::I32Const(RT_MAP_TABLE_CHILDREN));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  rt_emit_load_table_child(&mut b, 0, i0, table1);
  b.emit(Instruction::LocalGet(table1));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i1));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i1));
  b.emit(Instruction::I32Const(RT_MAP_TABLE_CHILDREN));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  rt_emit_load_table_child(&mut b, table1, i1, bucket);
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(bi));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(bi));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));

  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(addr));

  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(bi));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(tmp));
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(tmp));
  b.emit(Instruction::F64Store(mem_arg_f64(0)));

  b.emit(Instruction::LocalGet(bucket));
  b.emit(Instruction::I32Const(24));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(bi));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(tmp));
  b.emit(Instruction::LocalGet(addr));
  b.emit(Instruction::LocalGet(tmp));
  b.emit(Instruction::F64Store(mem_arg_f64(8)));

  b.emit(Instruction::LocalGet(out));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out));
  b.emit(Instruction::LocalGet(bi));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(bi));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i1));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i1));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i0));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i0));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(out));
  b.finish(vec![ValType::I32, ValType::I32, ValType::I32], vec![ValType::I32])
}

fn build_rt_map_from_flat(hash_idx: u32, root_assoc_idx: u32, map_make_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let count = b.alloc_i32();
  let i = b.alloc_i32();
  let root = b.alloc_i32();
  let actual = b.alloc_i32();
  let key = b.alloc_f64();
  let val = b.alloc_f64();
  let hash = b.alloc_i32();
  let added = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(actual));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(key));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(val));
  b.emit(Instruction::LocalGet(key));
  b.emit(Instruction::Call(hash_idx));
  b.emit(Instruction::LocalSet(hash));
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(key));
  b.emit(Instruction::LocalGet(val));
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::Call(root_assoc_idx));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::LocalGet(actual));
  b.emit(Instruction::LocalGet(added));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(actual));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(actual));
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::Call(map_make_idx));
  b.finish(vec![ValType::I32], vec![ValType::I32])
}

fn build_rt_map_root_from_flat(hash_idx: u32, root_assoc_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let count = b.alloc_i32();
  let i = b.alloc_i32();
  let root = b.alloc_i32();
  let key = b.alloc_f64();
  let val = b.alloc_f64();
  let hash = b.alloc_i32();
  let added = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(key));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(val));
  b.emit(Instruction::LocalGet(key));
  b.emit(Instruction::Call(hash_idx));
  b.emit(Instruction::LocalSet(hash));
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(key));
  b.emit(Instruction::LocalGet(val));
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::Call(root_assoc_idx));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(root));
  b.finish(vec![ValType::I32], vec![ValType::I32])
}

fn build_rt_map_linearize(root_write_pairs_idx: u32, list_tag: i32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  let count = b.alloc_i32();
  let root = b.alloc_i32();
  let slots = b.alloc_i32();
  let size = b.alloc_i32();
  let dst = b.alloc_i32();
  let base = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(slots));
  b.emit(Instruction::LocalGet(slots));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::LocalSet(size));
  rt_emit_alloc_dynamic(&mut b, size, dst, list_tag);
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(base));
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(base));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Call(root_write_pairs_idx));
  b.emit(Instruction::Drop);
  b.emit(Instruction::LocalGet(dst));
  b.finish(vec![ValType::I32], vec![ValType::I32])
}

fn build_rt_map_assoc(hash_idx: u32, root_assoc_idx: u32, map_make_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(3);
  let count = b.alloc_i32();
  let root = b.alloc_i32();
  let hash = b.alloc_i32();
  let added = b.alloc_i32();
  let new_count = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(hash_idx));
  b.emit(Instruction::LocalSet(hash));
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::Call(root_assoc_idx));
  b.emit(Instruction::LocalSet(added));
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::LocalGet(added));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(new_count));
  b.emit(Instruction::LocalGet(new_count));
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::Call(map_make_idx));
  b.finish(vec![ValType::I32, ValType::F64, ValType::F64], vec![ValType::I32])
}

fn build_rt_map_get_value(hash_idx: u32, root_lookup_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2);
  let root = b.alloc_i32();
  let hash = b.alloc_i32();
  let found = b.alloc_i32();
  let value = b.alloc_f64();
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(hash_idx));
  b.emit(Instruction::LocalSet(hash));
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::Call(root_lookup_idx));
  b.emit(Instruction::LocalSet(value));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::LocalGet(value));
  b.finish(vec![ValType::I32, ValType::F64], vec![ValType::F64])
}

fn build_rt_map_contains_key(hash_idx: u32, root_lookup_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2);
  let root = b.alloc_i32();
  let hash = b.alloc_i32();
  let found = b.alloc_i32();
  let value = b.alloc_f64();
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(hash_idx));
  b.emit(Instruction::LocalSet(hash));
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(hash));
  b.emit(Instruction::Call(root_lookup_idx));
  b.emit(Instruction::LocalSet(value));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::LocalGet(found));
  b.finish(vec![ValType::I32, ValType::F64], vec![ValType::I32])
}

fn build_rt_map_contains_value(root_contains_value_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2);
  let root = b.alloc_i32();
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(root));
  b.emit(Instruction::LocalGet(root));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(root_contains_value_idx));
  b.finish(vec![ValType::I32, ValType::F64], vec![ValType::I32])
}

fn build_rt_map_dissoc(copy_fn_idx: u32, map_linearize_idx: u32, map_from_flat_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2);
  let count = b.alloc_i32();
  let flat = b.alloc_i32();
  let found = b.alloc_i32();
  let found_idx = b.alloc_i32();
  let i = b.alloc_i32();
  let new_count = b.alloc_i32();
  let slots = b.alloc_i32();
  let size = b.alloc_i32();
  let dst = b.alloc_i32();
  let before_slots = b.alloc_i32();
  let after_slots = b.alloc_i32();
  let dst_base = b.alloc_i32();
  let src_base = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::Call(map_linearize_idx));
  b.emit(Instruction::LocalSet(flat));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::LocalSet(found_idx));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(flat));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(found));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalSet(found_idx));
  b.emit(Instruction::Br(2));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(found));
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(new_count));
  b.emit(Instruction::LocalGet(new_count));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(slots));
  b.emit(Instruction::LocalGet(slots));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::LocalSet(size));
  rt_emit_alloc_dynamic(&mut b, size, dst, 0);
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::LocalGet(new_count));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(found_idx));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(before_slots));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(dst_base));
  b.emit(Instruction::LocalGet(flat));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(src_base));
  rt_emit_copy_slots(&mut b, copy_fn_idx, dst_base, src_base, before_slots);
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::LocalGet(found_idx));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::LocalSet(after_slots));
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(found_idx));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(dst_base));
  b.emit(Instruction::LocalGet(flat));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(found_idx));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(src_base));
  rt_emit_copy_slots(&mut b, copy_fn_idx, dst_base, src_base, after_slots);
  b.emit(Instruction::LocalGet(dst));
  b.emit(Instruction::Call(map_from_flat_idx));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.finish(vec![ValType::I32, ValType::F64], vec![ValType::I32])
}

/// `__rt_str_compare(ptr_a: i32, ptr_b: i32) → f64`
///
/// Lexicographic byte comparison. Returns -1.0, 0.0, or 1.0.
/// Strings are UTF-8 in linear memory at logical_ptr+8, length at logical_ptr+0 (f64).
fn build_rt_str_compare() -> CompiledFn {
  // params: 0 = ptr_a (i32), 1 = ptr_b (i32)
  // locals: 2 = len_a, 3 = len_b, 4 = min_len, 5 = i, 6 = byte_a, 7 = byte_b
  let instructions = vec![
    // len_a = i32.trunc(f64.load ptr_a+0)
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(2),
    // len_b = i32.trunc(f64.load ptr_b+0)
    Instruction::LocalGet(1),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(3),
    // min_len = min(len_a, len_b)
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::I32LtU,
    Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)),
    Instruction::LocalGet(2),
    Instruction::Else,
    Instruction::LocalGet(3),
    Instruction::End,
    Instruction::LocalSet(4),
    // i = 0
    Instruction::I32Const(0),
    Instruction::LocalSet(5),
    // block $outer (result f64) — used for early return via Br(2)
    Instruction::Block(wasm_encoder::BlockType::Result(ValType::F64)),
    // block $break_loop — exit loop via BrIf(1) or Br(1)
    Instruction::Block(wasm_encoder::BlockType::Empty),
    // loop $loop
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    // if i >= min_len: exit loop
    Instruction::LocalGet(5),
    Instruction::LocalGet(4),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    // byte_a = i32.load8u(ptr_a + 8 + i)
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(5),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(6),
    // byte_b = i32.load8u(ptr_b + 8 + i)
    Instruction::LocalGet(1),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(5),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(7),
    // if byte_a < byte_b: push -1.0, br $outer (depth 3 from inside if)
    Instruction::LocalGet(6),
    Instruction::LocalGet(7),
    Instruction::I32LtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(-1.0f64)),
    Instruction::Br(3),
    Instruction::End,
    // if byte_a > byte_b: push 1.0, br $outer
    Instruction::LocalGet(6),
    Instruction::LocalGet(7),
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(1.0f64)),
    Instruction::Br(3),
    Instruction::End,
    // i += 1; continue loop
    Instruction::LocalGet(5),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(5),
    Instruction::Br(0),
    Instruction::End, // end loop
    Instruction::End, // end $break_loop
    // After loop: compare lengths to determine result
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::I32LtU,
    Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)),
    Instruction::F64Const(Ieee64::from(-1.0f64)),
    Instruction::Else,
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)),
    Instruction::F64Const(Ieee64::from(1.0f64)),
    Instruction::Else,
    Instruction::F64Const(Ieee64::from(0.0f64)),
    Instruction::End,
    Instruction::End,
    Instruction::End, // end $outer
  ];

  CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // len_a
      ValType::I32, // len_b
      ValType::I32, // min_len
      ValType::I32, // i
      ValType::I32, // byte_a
      ValType::I32, // byte_b
    ],
    instructions,
  }
}

/// `__rt_str_find_index(h_ptr: i32, n_ptr: i32) → f64`
///
/// Naive byte-level substring search. Returns the byte offset of the first
/// occurrence of the needle string in the haystack, or -1.0 if not found.
/// An empty needle always returns 0.0.
fn build_rt_str_find_index() -> CompiledFn {
  // params: 0=h_ptr (i32), 1=n_ptr (i32)
  // locals: 2=h_len(i32), 3=n_len(i32), 4=h_base(i32), 5=n_base(i32),
  //         6=i(i32), 7=j(i32), 8=limit(i32), 9=byte_h(i32), 10=byte_n(i32)
  let instructions = vec![
    // h_len = i32(f64.load h_ptr+0)
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(2),
    // n_len = i32(f64.load n_ptr+0)
    Instruction::LocalGet(1),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(3),
    // h_base = h_ptr + 8
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(4),
    // n_base = n_ptr + 8
    Instruction::LocalGet(1),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(5),
    // Block $outer (result f64) — used for early-exit via Br
    Instruction::Block(wasm_encoder::BlockType::Result(ValType::F64)),
    // if n_len == 0: push 0.0, br $outer
    Instruction::LocalGet(3),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(0.0f64)),
    Instruction::Br(1), // 0=If, 1=$outer
    Instruction::End,
    // if n_len > h_len: push -1.0, br $outer
    Instruction::LocalGet(3),
    Instruction::LocalGet(2),
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(-1.0f64)),
    Instruction::Br(1), // 0=If, 1=$outer
    Instruction::End,
    // limit = h_len - n_len  (last valid start position)
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::I32Sub,
    Instruction::LocalSet(8),
    // i = 0
    Instruction::I32Const(0),
    Instruction::LocalSet(6),
    // Block $exit_outer (empty) — outer loop exit
    Instruction::Block(wasm_encoder::BlockType::Empty),
    // Loop $outer_loop
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    // if i > limit: br $exit_outer
    Instruction::LocalGet(6),
    Instruction::LocalGet(8),
    Instruction::I32GtU,
    Instruction::BrIf(1), // 0=$outer_loop(continue), 1=$exit_outer(break)
    // j = 0
    Instruction::I32Const(0),
    Instruction::LocalSet(7),
    // Block $mismatch (empty) — inner loop exit on mismatch
    Instruction::Block(wasm_encoder::BlockType::Empty),
    // Loop $inner_loop
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    // if j >= n_len: found at i → push f64(i), br $outer
    Instruction::LocalGet(7),
    Instruction::LocalGet(3),
    Instruction::I32GeU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(6),
    Instruction::F64ConvertI32U,
    // depths inside If: 0=If,1=$inner_loop,2=$mismatch,3=$outer_loop,4=$exit_outer,5=$outer
    Instruction::Br(5),
    Instruction::End,
    // byte_h = h_base[i + j]
    Instruction::LocalGet(4),
    Instruction::LocalGet(6),
    Instruction::I32Add,
    Instruction::LocalGet(7),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(9),
    // byte_n = n_base[j]
    Instruction::LocalGet(5),
    Instruction::LocalGet(7),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(10),
    // if byte_h != byte_n: br $mismatch
    Instruction::LocalGet(9),
    Instruction::LocalGet(10),
    Instruction::I32Ne,
    Instruction::BrIf(1), // 0=$inner_loop, 1=$mismatch
    // j++
    Instruction::LocalGet(7),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(7),
    Instruction::Br(0), // continue $inner_loop
    Instruction::End,   // end $inner_loop
    Instruction::End,   // end $mismatch
    // i++
    Instruction::LocalGet(6),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(6),
    Instruction::Br(0), // continue $outer_loop
    Instruction::End,   // end $outer_loop
    Instruction::End,   // end $exit_outer
    // Not found: push -1.0 (result of $outer)
    Instruction::F64Const(Ieee64::from(-1.0f64)),
    Instruction::End, // end $outer (result f64)
  ];

  CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // h_len (2)
      ValType::I32, // n_len (3)
      ValType::I32, // h_base (4)
      ValType::I32, // n_base (5)
      ValType::I32, // i (6)
      ValType::I32, // j (7)
      ValType::I32, // limit (8)
      ValType::I32, // byte_h (9)
      ValType::I32, // byte_n (10)
    ],
    instructions,
  }
}

/// `__rt_str_starts_with(s_ptr: i32, p_ptr: i32) → f64`
///
/// Returns 1.0 if the string at s_ptr starts with the prefix at p_ptr, else 0.0.
/// An empty prefix always returns 1.0.
fn build_rt_str_starts_with() -> CompiledFn {
  // params: 0=s_ptr(i32), 1=p_ptr(i32)
  // locals: 2=s_len(i32), 3=p_len(i32), 4=s_base(i32), 5=p_base(i32),
  //         6=i(i32), 7=byte_s(i32), 8=byte_p(i32)
  let instructions = vec![
    // s_len = i32(f64.load s_ptr+0)
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(2),
    // p_len = i32(f64.load p_ptr+0)
    Instruction::LocalGet(1),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(3),
    // s_base = s_ptr + 8
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(4),
    // p_base = p_ptr + 8
    Instruction::LocalGet(1),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(5),
    // Block $outer (result f64) — early-exit via Br
    Instruction::Block(wasm_encoder::BlockType::Result(ValType::F64)),
    // if p_len == 0: push 1.0, br $outer (empty prefix always matches)
    Instruction::LocalGet(3),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(1.0f64)),
    Instruction::Br(1), // 0=If, 1=$outer
    Instruction::End,
    // if p_len > s_len: push 0.0, br $outer
    Instruction::LocalGet(3),
    Instruction::LocalGet(2),
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(0.0f64)),
    Instruction::Br(1), // 0=If, 1=$outer
    Instruction::End,
    // i = 0
    Instruction::I32Const(0),
    Instruction::LocalSet(6),
    // Block $fail (empty) — break here on first mismatch
    Instruction::Block(wasm_encoder::BlockType::Empty),
    // Loop $loop
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    // if i >= p_len: all bytes matched → push 1.0, br $outer
    Instruction::LocalGet(6),
    Instruction::LocalGet(3),
    Instruction::I32GeU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(1.0f64)),
    // depths: 0=If, 1=$loop, 2=$fail, 3=$outer
    Instruction::Br(3),
    Instruction::End,
    // byte_s = s_base[i]
    Instruction::LocalGet(4),
    Instruction::LocalGet(6),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(7),
    // byte_p = p_base[i]
    Instruction::LocalGet(5),
    Instruction::LocalGet(6),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(8),
    // if byte_s != byte_p: br $fail
    Instruction::LocalGet(7),
    Instruction::LocalGet(8),
    Instruction::I32Ne,
    Instruction::BrIf(1), // 0=$loop, 1=$fail
    // i++
    Instruction::LocalGet(6),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(6),
    Instruction::Br(0), // continue $loop
    Instruction::End,   // end $loop
    Instruction::End,   // end $fail
    // Mismatch: push 0.0 (result of $outer)
    Instruction::F64Const(Ieee64::from(0.0f64)),
    Instruction::End, // end $outer
  ];

  CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // s_len (2)
      ValType::I32, // p_len (3)
      ValType::I32, // s_base (4)
      ValType::I32, // p_base (5)
      ValType::I32, // i (6)
      ValType::I32, // byte_s (7)
      ValType::I32, // byte_p (8)
    ],
    instructions,
  }
}

/// `__rt_str_ends_with(s_ptr: i32, suf_ptr: i32) → f64`
///
/// Returns 1.0 if the string at s_ptr ends with the suffix at suf_ptr, else 0.0.
/// An empty suffix always returns 1.0.
fn build_rt_str_ends_with() -> CompiledFn {
  // params: 0=s_ptr(i32), 1=suf_ptr(i32)
  // locals: 2=s_len(i32), 3=suf_len(i32), 4=s_base(i32), 5=suf_base(i32),
  //         6=offset(i32), 7=i(i32), 8=byte_s(i32), 9=byte_suf(i32)
  let instructions = vec![
    // s_len = i32(f64.load s_ptr+0)
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(2),
    // suf_len = i32(f64.load suf_ptr+0)
    Instruction::LocalGet(1),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(3),
    // s_base = s_ptr + 8
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(4),
    // suf_base = suf_ptr + 8
    Instruction::LocalGet(1),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(5),
    // Block $outer (result f64)
    Instruction::Block(wasm_encoder::BlockType::Result(ValType::F64)),
    // if suf_len == 0: push 1.0, br $outer
    Instruction::LocalGet(3),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(1.0f64)),
    Instruction::Br(1),
    Instruction::End,
    // if suf_len > s_len: push 0.0, br $outer
    Instruction::LocalGet(3),
    Instruction::LocalGet(2),
    Instruction::I32GtU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(0.0f64)),
    Instruction::Br(1),
    Instruction::End,
    // offset = s_len - suf_len  (start byte position in s for comparison)
    Instruction::LocalGet(2),
    Instruction::LocalGet(3),
    Instruction::I32Sub,
    Instruction::LocalSet(6),
    // i = 0
    Instruction::I32Const(0),
    Instruction::LocalSet(7),
    // Block $fail (empty)
    Instruction::Block(wasm_encoder::BlockType::Empty),
    // Loop $loop
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    // if i >= suf_len: push 1.0, br $outer
    Instruction::LocalGet(7),
    Instruction::LocalGet(3),
    Instruction::I32GeU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::F64Const(Ieee64::from(1.0f64)),
    // depths: 0=If, 1=$loop, 2=$fail, 3=$outer
    Instruction::Br(3),
    Instruction::End,
    // byte_s = s_base[offset + i]
    Instruction::LocalGet(4),
    Instruction::LocalGet(6),
    Instruction::I32Add,
    Instruction::LocalGet(7),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(8),
    // byte_suf = suf_base[i]
    Instruction::LocalGet(5),
    Instruction::LocalGet(7),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(9),
    // if byte_s != byte_suf: br $fail
    Instruction::LocalGet(8),
    Instruction::LocalGet(9),
    Instruction::I32Ne,
    Instruction::BrIf(1),
    // i++
    Instruction::LocalGet(7),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(7),
    Instruction::Br(0), // continue $loop
    Instruction::End,   // end $loop
    Instruction::End,   // end $fail
    // Mismatch: push 0.0
    Instruction::F64Const(Ieee64::from(0.0f64)),
    Instruction::End, // end $outer
  ];

  CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // s_len (2)
      ValType::I32, // suf_len (3)
      ValType::I32, // s_base (4)
      ValType::I32, // suf_base (5)
      ValType::I32, // offset (6)
      ValType::I32, // i (7)
      ValType::I32, // byte_s (8)
      ValType::I32, // byte_suf (9)
    ],
    instructions,
  }
}

/// `__rt_f64_to_str(value: f64) → i32`
/// Converts a number to a heap-allocated string.
/// Handles integers (positive, negative, zero). Non-integers get "number".
fn build_rt_f64_to_str(string_tag: i32) -> CompiledFn {
  // param 0: value (f64)
  // locals: 1=raw_i64(i64), 2=neg(i32), 3=abs_i64(i64), 4=ndigits(i32),
  //          5=tmp_i64(i64), 6=payload(i32), 7=str_ptr(i32), 8=content(i32),
  //          9=pos(i32), 10=digit(i32), 11=raw_base(i32)
  use wasm_encoder::Ieee64;
  let mut b = vec![Instruction::LocalGet(0)];

  // Check if value is an integer: floor(value) == value (and not NaN)
  // Use: value - floor(value) == 0.0
  b.push(Instruction::F64Floor);
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Eq);
  b.push(Instruction::If(wasm_encoder::BlockType::Result(wasm_encoder::ValType::F64)));
  // --- integer branch ---

  // neg = value < 0.0 ? 1 : 0
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.push(Instruction::F64Lt);
  b.push(Instruction::LocalSet(2)); // neg

  // raw_i64 = i64.trunc_f64_s(value)
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I64TruncF64S);
  b.push(Instruction::LocalSet(1)); // raw_i64

  // abs_i64 = neg ? -raw_i64 : raw_i64
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::I64Const(0));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I64Sub);
  b.push(Instruction::LocalSet(3));
  b.push(Instruction::Else);
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::LocalSet(3));
  b.push(Instruction::End);

  // Count digits: ndigits = 0; tmp = abs_i64; do { ndigits++; tmp /= 10; } while tmp != 0
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(4)); // ndigits = 0
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalSet(5)); // tmp = abs_i64

  b.push(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.push(Instruction::Loop(wasm_encoder::BlockType::Empty));
  // ndigits++
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(4));
  // tmp /= 10
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Const(10));
  b.push(Instruction::I64DivU);
  b.push(Instruction::LocalSet(5));
  // if tmp == 0: break
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Eqz);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // total_len = ndigits + (neg ? 1 : 0)
  // payload = 8 (byte_len f64) + total_len rounded up to 8 bytes
  // Compute total_len
  b.push(Instruction::LocalGet(4)); // ndigits
  b.push(Instruction::LocalGet(2)); // neg flag (0 or 1)
  b.push(Instruction::I32Add); // total_len

  // payload = total_len (round up to 8): (total_len + 7) & ~7
  b.push(Instruction::I32Const(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-8i32));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add); // payload = 8 + padded_total_len
  b.push(Instruction::LocalSet(6)); // payload

  // Allocate string: raw_base = HEAP_PTR_GLOBAL
  b.push(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.push(Instruction::LocalTee(11)); // raw_base
  // Write HEAP_MAGIC at raw_base
  b.push(Instruction::I32Const(HEAP_MAGIC));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  // Write string_tag at raw_base+4
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(string_tag));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  // str_ptr = raw_base + 8
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(7)); // str_ptr (logical)
  // content = str_ptr + 8 (after byte_len slot)
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(8)); // content base
  // Advance heap_ptr by payload
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Add);
  b.push(Instruction::GlobalSet(HEAP_PTR_GLOBAL));

  // Write byte_len = total_len (ndigits + neg) at str_ptr+0
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::LocalGet(4)); // ndigits
  b.push(Instruction::LocalGet(2)); // neg
  b.push(Instruction::I32Add); // total_len
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::F64Store(mem_arg_f64(0)));

  // Write '-' at content[0] if neg
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::LocalGet(8)); // content addr
  b.push(Instruction::I32Const(b'-' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::End);

  // Write digits right-to-left, pos starts at (neg + ndigits - 1)
  // pos = neg + ndigits - 1
  b.push(Instruction::LocalGet(2)); // neg
  b.push(Instruction::LocalGet(4)); // ndigits
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(9)); // pos

  // tmp = abs_i64 again
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalSet(5));

  // Loop: write digits from least significant to most significant (right to left)
  b.push(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.push(Instruction::Loop(wasm_encoder::BlockType::Empty));
  // digit = tmp % 10
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Const(10));
  b.push(Instruction::I64RemU);
  b.push(Instruction::I32WrapI64);
  b.push(Instruction::LocalSet(10)); // digit
  // content[pos] = '0' + digit
  b.push(Instruction::LocalGet(8)); // content base
  b.push(Instruction::LocalGet(9)); // pos
  b.push(Instruction::I32Add); // content + pos
  b.push(Instruction::LocalGet(10)); // digit
  b.push(Instruction::I32Const(b'0' as i32));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  // pos--
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(9));
  // tmp /= 10
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Const(10));
  b.push(Instruction::I64DivU);
  b.push(Instruction::LocalSet(5));
  // if tmp == 0: break
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Eqz);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // return str_ptr as f64 — leave on stack
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::Else); // end integer branch / start non-integer branch

  // --- non-integer branch: allocate string "number" ---
  // "number" = 6 bytes: 110 117 109 98 101 114 (0x6e,0x75,0x6d,0x62,0x65,0x72)
  // payload = 8 (byte_len) + 8 (padded 6 bytes to 8)
  b.push(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.push(Instruction::LocalTee(11));
  b.push(Instruction::I32Const(HEAP_MAGIC));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(string_tag));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(7)); // str_ptr
  // Advance heap_ptr by 16 (8 byte_len + 8 padded content)
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32Const(24)); // 8 header + 8 byte_len + 8 padded bytes
  b.push(Instruction::I32Add);
  b.push(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
  // byte_len = 6
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::F64Const(Ieee64::from(6.0f64)));
  b.push(Instruction::F64Store(mem_arg_f64(0)));
  // content base = str_ptr + 8
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(8));
  // write "number" bytes
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::I32Const(b'n' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::I32Const(b'u' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(1)));
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::I32Const(b'm' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(2)));
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::I32Const(b'b' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(3)));
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::I32Const(b'e' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(4)));
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::I32Const(b'r' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(5)));
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::F64ConvertI32U);

  b.push(Instruction::End); // end if/else for integer check

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I64, // raw_i64 (1)
      ValType::I32, // neg (2)
      ValType::I64, // abs_i64 (3)
      ValType::I32, // ndigits (4)
      ValType::I64, // tmp_i64 (5)
      ValType::I32, // payload (6)
      ValType::I32, // str_ptr (7)
      ValType::I32, // content (8)
      ValType::I32, // pos (9)
      ValType::I32, // digit (10)
      ValType::I32, // raw_base (11)
    ],
    instructions: b,
  }
}

/// `__rt_generic_compare(a: f64, b: f64) → f64`
///
/// Universal comparison: dispatches to string comparison if both args are string heap pointers,
/// otherwise does numeric comparison. Returns -1.0, 0.0, or 1.0.
#[allow(clippy::vec_init_then_push)]
fn build_rt_generic_compare(str_compare_fn_idx: u32, string_tag: i32) -> CompiledFn {
  use wasm_encoder::Ieee64;
  // params: 0 = a (f64), 1 = b (f64)
  // locals: 2 = a_i32 (i32), 3 = raw_base (i32), 4 = is_str (i32)
  let heap_min = (HEAP_BASE as f64) + 8.0;
  let mut ins = vec![];

  // --- Check if `a` is a string heap pointer ---
  // is_str = (a >= heap_min) && (floor(a) == a) && (magic ok) && (tag == string_tag)
  // Step 1: heap range check
  ins.push(Instruction::LocalGet(0));
  ins.push(Instruction::F64Const(Ieee64::from(heap_min)));
  ins.push(Instruction::F64Ge);
  // Step 2: integer-valued check (floor(a) == a)
  ins.push(Instruction::LocalGet(0));
  ins.push(Instruction::F64Floor);
  ins.push(Instruction::LocalGet(0));
  ins.push(Instruction::F64Eq);
  ins.push(Instruction::I32And);
  ins.push(Instruction::LocalSet(4)); // is_str = preliminary check

  // If preliminary check passed, verify magic and type tag
  ins.push(Instruction::LocalGet(4));
  ins.push(Instruction::If(wasm_encoder::BlockType::Empty));
  {
    // a_i32 = (i32) a
    ins.push(Instruction::LocalGet(0));
    ins.push(Instruction::I32TruncF64U);
    ins.push(Instruction::LocalSet(2));
    // raw_base = a_i32 - 8
    ins.push(Instruction::LocalGet(2));
    ins.push(Instruction::I32Const(8));
    ins.push(Instruction::I32Sub);
    ins.push(Instruction::LocalSet(3));
    // check HEAP_MAGIC at raw_base+0
    ins.push(Instruction::LocalGet(3));
    ins.push(Instruction::I32Load(mem_arg_i32(0)));
    ins.push(Instruction::I32Const(HEAP_MAGIC));
    ins.push(Instruction::I32Eq);
    // check string_tag at raw_base+4
    ins.push(Instruction::LocalGet(3));
    ins.push(Instruction::I32Load(mem_arg_i32(4)));
    ins.push(Instruction::I32Const(string_tag));
    ins.push(Instruction::I32Eq);
    ins.push(Instruction::I32And);
    ins.push(Instruction::LocalSet(4));
  }
  ins.push(Instruction::End);

  // --- Dispatch based on is_str ---
  ins.push(Instruction::Block(wasm_encoder::BlockType::Result(ValType::F64)));
  ins.push(Instruction::Block(wasm_encoder::BlockType::Empty));
  ins.push(Instruction::LocalGet(4));
  ins.push(Instruction::I32Eqz);
  ins.push(Instruction::BrIf(0)); // jump to numeric compare if not string

  // String path: call __rt_str_compare(i32(a), i32(b))
  ins.push(Instruction::LocalGet(0));
  ins.push(Instruction::I32TruncF64U);
  ins.push(Instruction::LocalGet(1));
  ins.push(Instruction::I32TruncF64U);
  ins.push(Instruction::Call(str_compare_fn_idx));
  ins.push(Instruction::Br(1)); // jump to end of outer block
  ins.push(Instruction::End); // end inner block (numeric path)

  // Numeric path: a < b → -1.0, a > b → 1.0, else 0.0
  ins.push(Instruction::LocalGet(0));
  ins.push(Instruction::LocalGet(1));
  ins.push(Instruction::F64Lt);
  ins.push(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  ins.push(Instruction::F64Const(Ieee64::from(-1.0f64)));
  ins.push(Instruction::Else);
  ins.push(Instruction::LocalGet(0));
  ins.push(Instruction::LocalGet(1));
  ins.push(Instruction::F64Gt);
  ins.push(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  ins.push(Instruction::F64Const(Ieee64::from(1.0f64)));
  ins.push(Instruction::Else);
  ins.push(Instruction::F64Const(Ieee64::from(0.0f64)));
  ins.push(Instruction::End);
  ins.push(Instruction::End);
  ins.push(Instruction::End); // end outer block

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64, ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // a_i32 (2)
      ValType::I32, // raw_base (3)
      ValType::I32, // is_str (4)
    ],
    instructions: ins,
  }
}

/// `__rt_generic_eq(a: f64, b: f64) → i32`
///
/// Semantic equality: if both args are string heap pointers, compare byte content.
/// Otherwise use `f64 ==` (identity/numeric equality).
#[allow(clippy::vec_init_then_push)]
fn build_rt_generic_eq(str_compare_fn_idx: u32, string_tag: i32) -> CompiledFn {
  use wasm_encoder::Ieee64;
  let mut ins = Vec::<Instruction>::new();
  // locals: 0=a, 1=b (params)
  // locals: 2=a_i32, 3=b_i32, 4=raw_a, 5=raw_b, 6=is_str_a, 7=is_str_b

  // Check if a looks like a string heap pointer
  ins.push(Instruction::LocalGet(0));
  ins.push(Instruction::F64Const(Ieee64::from(HEAP_BASE as f64)));
  ins.push(Instruction::F64Ge);
  ins.push(Instruction::If(wasm_encoder::BlockType::Empty));
  // a_i32 = i32(a)
  ins.push(Instruction::LocalGet(0));
  ins.push(Instruction::I32TruncF64U);
  ins.push(Instruction::LocalSet(2));
  // raw_a = a_i32 - 8
  ins.push(Instruction::LocalGet(2));
  ins.push(Instruction::I32Const(8));
  ins.push(Instruction::I32Sub);
  ins.push(Instruction::LocalSet(4));
  // check magic at raw_a+0
  ins.push(Instruction::LocalGet(4));
  ins.push(Instruction::I32Load(mem_arg_i32(0)));
  ins.push(Instruction::I32Const(HEAP_MAGIC));
  ins.push(Instruction::I32Eq);
  // check string_tag at raw_a+4
  ins.push(Instruction::LocalGet(4));
  ins.push(Instruction::I32Load(mem_arg_i32(4)));
  ins.push(Instruction::I32Const(string_tag));
  ins.push(Instruction::I32Eq);
  ins.push(Instruction::I32And);
  ins.push(Instruction::LocalSet(6)); // is_str_a
  ins.push(Instruction::End); // end if

  // Check if b is also a string heap pointer
  ins.push(Instruction::LocalGet(1));
  ins.push(Instruction::F64Const(Ieee64::from(HEAP_BASE as f64)));
  ins.push(Instruction::F64Ge);
  ins.push(Instruction::If(wasm_encoder::BlockType::Empty));
  ins.push(Instruction::LocalGet(1));
  ins.push(Instruction::I32TruncF64U);
  ins.push(Instruction::LocalSet(3));
  ins.push(Instruction::LocalGet(3));
  ins.push(Instruction::I32Const(8));
  ins.push(Instruction::I32Sub);
  ins.push(Instruction::LocalSet(5));
  ins.push(Instruction::LocalGet(5));
  ins.push(Instruction::I32Load(mem_arg_i32(0)));
  ins.push(Instruction::I32Const(HEAP_MAGIC));
  ins.push(Instruction::I32Eq);
  ins.push(Instruction::LocalGet(5));
  ins.push(Instruction::I32Load(mem_arg_i32(4)));
  ins.push(Instruction::I32Const(string_tag));
  ins.push(Instruction::I32Eq);
  ins.push(Instruction::I32And);
  ins.push(Instruction::LocalSet(7)); // is_str_b
  ins.push(Instruction::End); // end if

  // If both are strings: compare content, return compare(a,b)==0
  ins.push(Instruction::LocalGet(6));
  ins.push(Instruction::LocalGet(7));
  ins.push(Instruction::I32And);
  ins.push(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  ins.push(Instruction::LocalGet(2)); // a_i32
  ins.push(Instruction::LocalGet(3)); // b_i32
  ins.push(Instruction::Call(str_compare_fn_idx));
  ins.push(Instruction::F64Const(Ieee64::from(0.0f64)));
  ins.push(Instruction::F64Eq);
  ins.push(Instruction::Else);
  // Otherwise: f64 equality
  ins.push(Instruction::LocalGet(0));
  ins.push(Instruction::LocalGet(1));
  ins.push(Instruction::F64Eq);
  ins.push(Instruction::End);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64, ValType::F64],
    results: vec![ValType::I32],
    locals: vec![
      ValType::I32, // a_i32 (2)
      ValType::I32, // b_i32 (3)
      ValType::I32, // raw_a (4)
      ValType::I32, // raw_b (5)
      ValType::I32, // is_str_a (6)
      ValType::I32, // is_str_b (7)
    ],
    instructions: ins,
  }
}

/// `__rt_hash_f64_semantic(v: f64) → i32`
///
/// For string heap pointers: compute a djb2-style content hash over the bytes.
/// For all other values: use bit-mixing of f64 bits.
#[allow(clippy::vec_init_then_push)]
fn build_rt_hash_f64_semantic(string_tag: i32) -> CompiledFn {
  use wasm_encoder::Ieee64;
  let mut ins = Vec::<Instruction>::new();
  // locals: 0=v (param), 1=v_i32, 2=raw, 3=is_str, 4=len, 5=i, 6=hash, 7=byte_val

  // Check if v is a string heap pointer
  ins.push(Instruction::LocalGet(0));
  ins.push(Instruction::F64Const(Ieee64::from(HEAP_BASE as f64)));
  ins.push(Instruction::F64Ge);
  ins.push(Instruction::If(wasm_encoder::BlockType::Empty));
  ins.push(Instruction::LocalGet(0));
  ins.push(Instruction::I32TruncF64U);
  ins.push(Instruction::LocalSet(1));
  ins.push(Instruction::LocalGet(1));
  ins.push(Instruction::I32Const(8));
  ins.push(Instruction::I32Sub);
  ins.push(Instruction::LocalSet(2));
  ins.push(Instruction::LocalGet(2));
  ins.push(Instruction::I32Load(mem_arg_i32(0)));
  ins.push(Instruction::I32Const(HEAP_MAGIC));
  ins.push(Instruction::I32Eq);
  ins.push(Instruction::LocalGet(2));
  ins.push(Instruction::I32Load(mem_arg_i32(4)));
  ins.push(Instruction::I32Const(string_tag));
  ins.push(Instruction::I32Eq);
  ins.push(Instruction::I32And);
  ins.push(Instruction::LocalSet(3)); // is_str
  ins.push(Instruction::End); // end if

  // is_str block
  ins.push(Instruction::LocalGet(3));
  ins.push(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  // content hash: djb2 over bytes
  // len = i32(f64.load(v_i32+0))
  ins.push(Instruction::LocalGet(1));
  ins.push(Instruction::F64Load(mem_arg_f64(0)));
  ins.push(Instruction::I32TruncF64U);
  ins.push(Instruction::LocalSet(4));
  // hash = 5381
  ins.push(Instruction::I32Const(5381));
  ins.push(Instruction::LocalSet(6));
  // i = 0
  ins.push(Instruction::I32Const(0));
  ins.push(Instruction::LocalSet(5));
  ins.push(Instruction::Block(wasm_encoder::BlockType::Empty));
  ins.push(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ins.push(Instruction::LocalGet(5));
  ins.push(Instruction::LocalGet(4));
  ins.push(Instruction::I32GeU);
  ins.push(Instruction::BrIf(1));
  // byte = load8u(v_i32 + 8 + i)
  ins.push(Instruction::LocalGet(1));
  ins.push(Instruction::I32Const(8));
  ins.push(Instruction::I32Add);
  ins.push(Instruction::LocalGet(5));
  ins.push(Instruction::I32Add);
  ins.push(Instruction::I32Load8U(mem_arg_byte(0)));
  ins.push(Instruction::LocalSet(7));
  // hash = hash * 33 ^ byte
  ins.push(Instruction::LocalGet(6));
  ins.push(Instruction::I32Const(33));
  ins.push(Instruction::I32Mul);
  ins.push(Instruction::LocalGet(7));
  ins.push(Instruction::I32Xor);
  ins.push(Instruction::LocalSet(6));
  // i++
  ins.push(Instruction::LocalGet(5));
  ins.push(Instruction::I32Const(1));
  ins.push(Instruction::I32Add);
  ins.push(Instruction::LocalSet(5));
  ins.push(Instruction::Br(0));
  ins.push(Instruction::End); // end loop
  ins.push(Instruction::End); // end block
  ins.push(Instruction::LocalGet(6));
  ins.push(Instruction::Else);
  // non-string: bit-mix of f64
  ins.push(Instruction::LocalGet(0));
  ins.push(Instruction::I64ReinterpretF64);
  ins.push(Instruction::I64Const(32));
  ins.push(Instruction::I64ShrU);
  ins.push(Instruction::I32WrapI64);
  ins.push(Instruction::I32Const(0x9e37_79b9u32 as i32));
  ins.push(Instruction::I32Mul);
  ins.push(Instruction::I32Const(16));
  ins.push(Instruction::I32Rotl);
  ins.push(Instruction::End); // end is_str if

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64],
    results: vec![ValType::I32],
    locals: vec![
      ValType::I32, // v_i32 (1)
      ValType::I32, // raw (2)
      ValType::I32, // is_str (3)
      ValType::I32, // len (4)
      ValType::I32, // i (5)
      ValType::I32, // hash (6)
      ValType::I32, // byte_val (7)
    ],
    instructions: ins,
  }
}
