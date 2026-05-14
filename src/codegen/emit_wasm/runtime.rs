use super::*;
use wasm_encoder::{BlockType, Ieee64};

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
  // IO: log a string directly (ptr to heap string) — more efficient than log_value for strings
  HostImport {
    module: "io",
    name: "log_str",
    arity: 1,
  },
  // IO: read file contents as string (ptr to path in heap)
  HostImport {
    module: "io",
    name: "read_file_str",
    arity: 1,
  },
  // IO: check if file exists (ptr to path in heap) — returns 1.0 if exists, 0.0 otherwise
  HostImport {
    module: "io",
    name: "file_exists",
    arity: 1,
  },
  // IO: parse JSON string (ptr to JSON string in heap) — returns parsed value or nil on error
  HostImport {
    module: "io",
    name: "parse_json",
    arity: 1,
  },
  // IO: get current time in milliseconds
  HostImport {
    module: "io",
    name: "current_time",
    arity: 0,
  },
  // IO: get environment variable (ptr to key in heap) — returns value string or nil
  HostImport {
    module: "io",
    name: "get_env",
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

  // Global section: heap pointer for bump allocator, then atom globals
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
  fns.push(build_rt_hash_f64());

  // __rt_hash_list_or_set(ptr: i32) -> i32
  // XOR-based content hash over all elements (order-independent for sets, order-dependent for lists).
  // Both sets and lists have identical memory layout, so one function suffices.
  let hash_list_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_hash_list_or_set"), hash_list_idx);
  fns.push(build_rt_hash_list_or_set(hash_idx));

  let map_root_assoc_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_root_assoc"), map_root_assoc_idx);
  fns.push(build_rt_map_root_assoc(*fn_index.get("__rt_copy_f64_slots").expect("copy helper")));

  let map_root_lookup_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_root_lookup"), map_root_lookup_idx);
  fns.push(build_rt_map_root_lookup());

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

  // Radix display: __rt_display_by(value: f64, radix: f64) → f64 (string logical ptr as f64)
  let display_by_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_display_by"), display_by_idx);
  fns.push(build_rt_display_by(string_tag));

  // Trim whitespace: __rt_trim_ws(s: f64) → f64 (trimmed string ptr)
  let trim_ws_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_trim_ws"), trim_ws_idx);
  fns.push(build_rt_trim_ws(string_tag));

  // Trim char: __rt_trim_char(s: f64, c: f64) → f64 (trimmed string ptr)
  let trim_char_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_trim_char"), trim_char_idx);
  fns.push(build_rt_trim_char(string_tag));

  // blank?: __rt_blank(s: f64) → f64 (1.0 if blank, 0.0 otherwise)
  let blank_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_blank"), blank_idx);
  fns.push(build_rt_blank());

  // parse-float: __rt_parse_float(s: f64) → f64 (parsed number)
  let parse_float_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_parse_float"), parse_float_idx);
  fns.push(build_rt_parse_float());

  // char-from-code: __rt_char_from_code(cp: f64) → f64 (single-char string ptr)
  let char_from_code_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_char_from_code"), char_from_code_idx);
  fns.push(build_rt_char_from_code(string_tag));

  // str-replace: __rt_str_replace(s:f64, pat:f64, rep:f64) → f64 (new string ptr)
  let str_replace_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_str_replace"), str_replace_idx);
  fns.push(build_rt_str_replace(string_tag));

  // str-escape: __rt_str_escape(s:f64) → f64 (new string ptr with escaped chars)
  let str_escape_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_str_escape"), str_escape_idx);
  fns.push(build_rt_str_escape(string_tag));

  // map-equal: __rt_map_equal(a: i32, b: i32) → i32 (1 if equal, 0 otherwise)
  let map_equal_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_equal"), map_equal_idx);
  fns.push(build_rt_map_equal(map_linearize_idx, map_get_value_idx));

  // str-find-from: __rt_str_find_from(h_ptr: i32, h_start: i32, pat_ptr: i32) → i32
  // Returns byte offset of first occurrence at or after h_start, or -1.
  let str_find_from_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_str_find_from"), str_find_from_idx);
  fns.push(build_rt_str_find_from());

  // utf8-char-len: __rt_utf8_char_len(b: i32) → i32
  // Returns byte width of a UTF-8 character given its first byte.
  let utf8_char_len_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_utf8_char_len"), utf8_char_len_idx);
  fns.push(build_rt_utf8_char_len());

  // str-split: __rt_str_split(s_ptr: i32, pat_ptr: i32) → i32 (list ptr)
  let str_split_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_str_split"), str_split_idx);
  fns.push(build_rt_str_split(string_tag, list_tag, str_find_from_idx, utf8_char_len_idx));

  // value-equal: __rt_value_equal(a: f64, b: f64) → i32 (1=equal, 0=not)
  // Deep equality for all heap types (strings, lists). For other types: f64 equality.
  let value_equal_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_value_equal"), value_equal_idx);
  fns.push(build_rt_value_equal(string_tag, list_tag, str_compare_idx, value_equal_idx));

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

fn build_rt_hash_list_or_set(hash_f64_idx: u32) -> CompiledFn {
  // params: ptr: i32
  // Returns i32 hash via XOR of each element's hash — order-independent.
  // Layout: [count: f64 at ptr][elem0: f64 at ptr+8] ...
  let mut b = RuntimeFnBuilder::new(1); // param: ptr (i32)
  let count = b.alloc_i32();
  let i = b.alloc_i32();
  let acc = b.alloc_i32();
  let elem_addr = b.alloc_i32();
  // count = i32(f64.load[ptr])
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count));
  // i = 0; acc = 0
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(acc));
  // loop
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  // if i >= count break
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1)); // break outer block
  // elem_addr = ptr + 8 + i*8
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(3)); // * 8 = << 3
  b.emit(Instruction::I32Shl);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(elem_addr));
  // acc ^= hash_f64(f64.load[elem_addr])
  b.emit(Instruction::LocalGet(acc));
  b.emit(Instruction::LocalGet(elem_addr));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::Call(hash_f64_idx));
  b.emit(Instruction::I32Xor);
  b.emit(Instruction::LocalSet(acc));
  // i += 1
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0)); // continue loop
  b.emit(Instruction::End); // end loop
  b.emit(Instruction::End); // end block
  // return acc
  b.emit(Instruction::LocalGet(acc));
  b.finish(vec![ValType::I32], vec![ValType::I32])
}

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

fn build_rt_map_root_assoc(copy_fn_idx: u32) -> CompiledFn {
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
  b.emit(Instruction::F64Eq);
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

fn build_rt_map_root_lookup() -> CompiledFn {
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
  b.emit(Instruction::F64Eq);
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

/// `__rt_display_by(value: f64, radix: f64) -> f64`
///
/// Converts `value` (integer f64) to a string in the given radix.
/// Prefixes: radix 2 → "0b", radix 8 → "0o", radix 16 → "0x", else no prefix.
/// Negative values get a "-" prefix.
#[allow(clippy::vec_init_then_push)]
fn build_rt_display_by(string_tag: i32) -> CompiledFn {
  // params: 0=value(f64), 1=radix(f64)
  // locals (allocated manually to match flat vec):
  //   2=radix_i64(i64), 3=is_neg(i32), 4=abs_i64(i64), 5=tmp_i64(i64),
  //   6=ndigits(i32), 7=prefix_len(i32), 8=total_len(i32), 9=padded(i32),
  //   10=str_ptr(i32), 11=content(i32), 12=digit_pos(i32), 13=digit(i32),
  //   14=raw_base(i32), 15=ch(i32), 16=payload(i32)
  let mut b: Vec<Instruction> = Vec::new();

  // radix_i64 = i64.trunc_f64_s(radix)
  b.push(Instruction::LocalGet(1)); // radix f64
  b.push(Instruction::I64TruncF64S);
  b.push(Instruction::LocalSet(2)); // radix_i64

  // is_neg = value < 0.0 ? 1 : 0
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.push(Instruction::F64Lt);
  b.push(Instruction::LocalSet(3)); // is_neg

  // abs_i64 = trunc(value) in i64, then abs
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I64TruncF64S);
  b.push(Instruction::LocalSet(5)); // tmp = signed i64(value)
  // abs: if is_neg, negate
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::I64Const(0));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Sub);
  b.push(Instruction::LocalSet(4)); // abs_i64
  b.push(Instruction::Else);
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::End);

  // Count ndigits: loop { ndigits++; tmp /= radix; } while tmp != 0
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(6)); // ndigits = 0
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalSet(5)); // tmp = abs_i64

  b.push(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.push(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(6)); // ndigits++
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(2)); // radix_i64
  b.push(Instruction::I64DivU);
  b.push(Instruction::LocalSet(5)); // tmp /= radix
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Eqz);
  b.push(Instruction::BrIf(1)); // break if tmp == 0
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // prefix_len: 2 if radix ∈ {2,8,16}, else 0
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(7)); // prefix_len = 0
  // if radix == 2 || radix == 8 || radix == 16
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Const(Ieee64::from(2.0f64)));
  b.push(Instruction::F64Eq);
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Const(Ieee64::from(8.0f64)));
  b.push(Instruction::F64Eq);
  b.push(Instruction::I32Or);
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Const(Ieee64::from(16.0f64)));
  b.push(Instruction::F64Eq);
  b.push(Instruction::I32Or);
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::I32Const(2));
  b.push(Instruction::LocalSet(7)); // prefix_len = 2
  b.push(Instruction::End);

  // total_len = is_neg + prefix_len + ndigits
  b.push(Instruction::LocalGet(3)); // is_neg
  b.push(Instruction::LocalGet(7)); // prefix_len
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(6)); // ndigits
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(8)); // total_len

  // padded = (total_len + 7) & ~7
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::I32Const(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-8i32));
  b.push(Instruction::I32And);
  b.push(Instruction::LocalSet(9)); // padded

  // payload = 8 (byte_len f64) + padded
  b.push(Instruction::I32Const(8));
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(16)); // payload

  // Allocate string: raw_base = HEAP_PTR_GLOBAL
  b.push(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.push(Instruction::LocalTee(14)); // raw_base
  b.push(Instruction::I32Const(HEAP_MAGIC));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(string_tag));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  // str_ptr = raw_base + 8
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(10)); // str_ptr (logical)
  // content = str_ptr + 8
  b.push(Instruction::LocalGet(10));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(11)); // content
  // Advance heap_ptr: raw_base + 8 + payload
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::GlobalSet(HEAP_PTR_GLOBAL));

  // Write byte_len at str_ptr+0
  b.push(Instruction::LocalGet(10));
  b.push(Instruction::LocalGet(8)); // total_len
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::F64Store(mem_arg_f64(0)));

  // Write '-' if is_neg (content[0] = '-')
  b.push(Instruction::LocalGet(3)); // is_neg
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32Const(b'-' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::End);

  // Write prefix bytes: current byte offset = is_neg (0 or 1)
  // pos = is_neg  (i32 local 12 reused as write-cursor for prefix/digit region)
  b.push(Instruction::LocalGet(3)); // is_neg as starting byte offset
  b.push(Instruction::LocalSet(12)); // digit_pos = is_neg (cursor)

  // if radix == 2: content[cursor]='0', content[cursor+1]='b'; cursor+=2
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Const(Ieee64::from(2.0f64)));
  b.push(Instruction::F64Eq);
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(b'0' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(b'b' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(2));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(12));
  b.push(Instruction::End);

  // if radix == 8: prefix "0o"
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Const(Ieee64::from(8.0f64)));
  b.push(Instruction::F64Eq);
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(b'0' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(b'o' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(2));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(12));
  b.push(Instruction::End);

  // if radix == 16: prefix "0x"
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Const(Ieee64::from(16.0f64)));
  b.push(Instruction::F64Eq);
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(b'0' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(b'x' as i32));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(2));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(12));
  b.push(Instruction::End);

  // Now write digits right-to-left starting at content[cursor + ndigits - 1]
  // digit_pos = cursor + ndigits - 1
  b.push(Instruction::LocalGet(12)); // cursor (after prefix)
  b.push(Instruction::LocalGet(6)); // ndigits
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(12)); // digit_pos (rightmost digit byte offset)

  // tmp = abs_i64
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalSet(5));

  // Loop: write digits right to left
  b.push(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.push(Instruction::Loop(wasm_encoder::BlockType::Empty));
  // digit = i32(tmp % radix_i64)
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(2)); // radix_i64
  b.push(Instruction::I64RemU);
  b.push(Instruction::I32WrapI64);
  b.push(Instruction::LocalSet(13)); // digit
  // ch = digit < 10 ? '0' + digit : 'a' + digit - 10
  b.push(Instruction::LocalGet(13));
  b.push(Instruction::I32Const(10));
  b.push(Instruction::I32LtU);
  b.push(Instruction::If(wasm_encoder::BlockType::Empty));
  b.push(Instruction::LocalGet(13));
  b.push(Instruction::I32Const(b'0' as i32));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(15)); // ch = '0' + digit
  b.push(Instruction::Else);
  b.push(Instruction::LocalGet(13));
  b.push(Instruction::I32Const(b'a' as i32 - 10));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(15)); // ch = 'a' + digit - 10
  b.push(Instruction::End);
  // content[digit_pos] = ch
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  // digit_pos--
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(12));
  // tmp /= radix
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I64DivU);
  b.push(Instruction::LocalSet(5));
  // break if tmp == 0
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I64Eqz);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // return str_ptr as f64
  b.push(Instruction::LocalGet(10));
  b.push(Instruction::F64ConvertI32U);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64, ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I64, // radix_i64 (2)
      ValType::I32, // is_neg (3)
      ValType::I64, // abs_i64 (4)
      ValType::I64, // tmp_i64 (5)
      ValType::I32, // ndigits (6)
      ValType::I32, // prefix_len (7)
      ValType::I32, // total_len (8)
      ValType::I32, // padded (9)
      ValType::I32, // str_ptr (10)
      ValType::I32, // content (11)
      ValType::I32, // digit_pos / cursor (12)
      ValType::I32, // digit (13)
      ValType::I32, // raw_base (14)
      ValType::I32, // ch (15)
      ValType::I32, // payload (16)
    ],
    instructions: b,
  }
}

// ---------------------------------------------------------------------------
// Helper: emit "is ASCII whitespace" check for local `loc` → leaves i32 on stack.
// Whitespace: 0x09 (tab), 0x0A (LF), 0x0D (CR), 0x20 (space).
fn emit_is_ws_check(b: &mut Vec<Instruction<'static>>, loc: u32) {
  b.push(Instruction::LocalGet(loc));
  b.push(Instruction::I32Const(0x09));
  b.push(Instruction::I32Eq);
  b.push(Instruction::LocalGet(loc));
  b.push(Instruction::I32Const(0x0A));
  b.push(Instruction::I32Eq);
  b.push(Instruction::I32Or);
  b.push(Instruction::LocalGet(loc));
  b.push(Instruction::I32Const(0x0D));
  b.push(Instruction::I32Eq);
  b.push(Instruction::I32Or);
  b.push(Instruction::LocalGet(loc));
  b.push(Instruction::I32Const(0x20));
  b.push(Instruction::I32Eq);
  b.push(Instruction::I32Or);
}

/// `__rt_trim_ws(s: f64) → f64`
///
/// Strips leading and trailing ASCII whitespace from a heap string.
/// Returns a new heap-allocated string.
#[allow(clippy::vec_init_then_push)]
fn build_rt_trim_ws(str_tag: i32) -> CompiledFn {
  // params: 0=s(f64)
  // locals: 1=ptr(i32), 2=byte_len(i32), 3=content(i32), 4=start(i32),
  //         5=end(i32), 6=new_len(i32), 7=raw_base(i32), 8=new_ptr(i32),
  //         9=padded(i32), 10=b(i32)
  let mut b: Vec<Instruction> = Vec::new();

  // ptr = i32(s)
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(1));

  // byte_len = i32(f64load(ptr+0))
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Load(mem_arg_f64(0)));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(2));

  // content = ptr + 8
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(3));

  // start = 0
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(4));

  // Forward loop: while start < byte_len && is_ws(content[start]): start++
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32GeU);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalSet(10));
  emit_is_ws_check(&mut b, 10);
  b.push(Instruction::I32Eqz);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // end = byte_len
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::LocalSet(5));

  // Backward loop: while end > start && is_ws(content[end-1]): end--
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32LeU);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-1i32));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalSet(10));
  emit_is_ws_check(&mut b, 10);
  b.push(Instruction::I32Eqz);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(5));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // new_len = end - start
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(6));

  // padded = (new_len + 7) & -8
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Const(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-8i32));
  b.push(Instruction::I32And);
  b.push(Instruction::LocalSet(9));

  // Allocate heap string
  b.push(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.push(Instruction::LocalTee(7)); // raw_base
  b.push(Instruction::I32Const(HEAP_MAGIC));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(str_tag));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  // new_ptr = raw_base + 8
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(8));
  // byte_len at new_ptr+0
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::F64Store(mem_arg_f64(0)));
  // advance heap: raw_base + 16 + padded
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Const(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::I32Add);
  b.push(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
  // memory.copy(new_ptr+8, content+start, new_len)
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
  // return f64(new_ptr)
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::F64ConvertI32U);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // ptr (1)
      ValType::I32, // byte_len (2)
      ValType::I32, // content (3)
      ValType::I32, // start (4)
      ValType::I32, // end (5)
      ValType::I32, // new_len (6)
      ValType::I32, // raw_base (7)
      ValType::I32, // new_ptr (8)
      ValType::I32, // padded (9)
      ValType::I32, // b (10)
    ],
    instructions: b,
  }
}

/// `__rt_trim_char(s: f64, c: f64) → f64`
///
/// Strips the first byte of the `c` string from both ends of `s`.
#[allow(clippy::vec_init_then_push)]
fn build_rt_trim_char(str_tag: i32) -> CompiledFn {
  // params: 0=s(f64), 1=c(f64) (char string ptr)
  // locals: 2=ptr(i32), 3=byte_len(i32), 4=content(i32), 5=start(i32),
  //         6=end(i32), 7=new_len(i32), 8=raw_base(i32), 9=new_ptr(i32),
  //         10=padded(i32), 11=ch(i32), 12=b(i32)
  let mut b: Vec<Instruction> = Vec::new();

  // ch = first byte of char string c
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalSet(11));

  // ptr = i32(s)
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(2));

  // byte_len = i32(f64load(ptr))
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::F64Load(mem_arg_f64(0)));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(3));

  // content = ptr + 8
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(4));

  // start = 0
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(5));

  // Forward loop: while start < byte_len && content[start] == ch
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32GeU);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalSet(12));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32Ne);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(5));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // end = byte_len
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalSet(6));

  // Backward loop: while end > start && content[end-1] == ch
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32LeU);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-1i32));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalSet(12));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::I32Ne);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(6));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // new_len = end - start
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(7));

  // padded = (new_len + 7) & -8
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Const(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-8i32));
  b.push(Instruction::I32And);
  b.push(Instruction::LocalSet(10));

  // Allocate heap string
  b.push(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.push(Instruction::LocalTee(8));
  b.push(Instruction::I32Const(HEAP_MAGIC));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(str_tag));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(9));
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::F64Store(mem_arg_f64(0)));
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::I32Const(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(10));
  b.push(Instruction::I32Add);
  b.push(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::F64ConvertI32U);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64, ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // ptr (2)
      ValType::I32, // byte_len (3)
      ValType::I32, // content (4)
      ValType::I32, // start (5)
      ValType::I32, // end (6)
      ValType::I32, // new_len (7)
      ValType::I32, // raw_base (8)
      ValType::I32, // new_ptr (9)
      ValType::I32, // padded (10)
      ValType::I32, // ch (11)
      ValType::I32, // b (12)
    ],
    instructions: b,
  }
}

/// `__rt_blank(s: f64) → f64`
///
/// Returns 1.0 if the string contains only ASCII whitespace (or is empty), 0.0 otherwise.
#[allow(clippy::vec_init_then_push)]
fn build_rt_blank() -> CompiledFn {
  // params: 0=s(f64)
  // locals: 1=ptr(i32), 2=byte_len(i32), 3=content(i32), 4=i(i32), 5=b(i32)
  let mut b: Vec<Instruction> = Vec::new();

  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(1));

  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Load(mem_arg_f64(0)));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(2));

  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(3));

  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(4));

  // Loop: if any byte is non-whitespace, return 0.0
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  // if i >= byte_len: break (all whitespace)
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32GeU);
  b.push(Instruction::BrIf(1));
  // b = load8(content + i)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalSet(5));
  // if NOT is_ws: return 0.0
  emit_is_ws_check(&mut b, 5);
  b.push(Instruction::I32Eqz);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.push(Instruction::Return);
  b.push(Instruction::End);
  // i++
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  b.push(Instruction::F64Const(Ieee64::from(1.0f64)));

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // ptr (1)
      ValType::I32, // byte_len (2)
      ValType::I32, // content (3)
      ValType::I32, // i (4)
      ValType::I32, // b (5)
    ],
    instructions: b,
  }
}

/// `__rt_parse_float(s: f64) → f64`
///
/// Parses a decimal string to f64. Handles optional sign, integer digits,
/// optional fractional digits. No scientific notation.
#[allow(clippy::vec_init_then_push)]
fn build_rt_parse_float() -> CompiledFn {
  // params: 0=s(f64)
  // locals: 1=ptr(i32), 2=byte_len(i32), 3=content(i32), 4=i(i32),
  //         5=is_neg(i32), 6=b(i32), 7=result(f64), 8=frac_mult(f64)
  let mut b: Vec<Instruction> = Vec::new();

  // ptr = i32(s)
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(1));
  // byte_len = i32(f64load(ptr))
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::F64Load(mem_arg_f64(0)));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(2));
  // content = ptr + 8
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(3));
  // i = 0
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(4));

  // Skip whitespace
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32GeU);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalSet(6));
  emit_is_ws_check(&mut b, 6);
  b.push(Instruction::I32Eqz);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // Optional sign
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(5)); // is_neg = 0

  // if i < byte_len && content[i] == '-': is_neg=1, i++
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32LtU);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalSet(6));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Const(b'-' as i32));
  b.push(Instruction::I32Eq);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::LocalSet(5));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::Else);
  // elif '+': i++
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Const(b'+' as i32));
  b.push(Instruction::I32Eq);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::End);
  b.push(Instruction::End);
  b.push(Instruction::End);

  // result = 0.0
  b.push(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.push(Instruction::LocalSet(7));

  // Integer digit loop
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32GeU);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalSet(6));
  // if b < '0': break
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Const(b'0' as i32));
  b.push(Instruction::I32LtU);
  b.push(Instruction::BrIf(1));
  // if b > '9': break
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Const(b'9' as i32));
  b.push(Instruction::I32GtU);
  b.push(Instruction::BrIf(1));
  // result = result * 10.0 + f64(b - '0')
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::F64Const(Ieee64::from(10.0f64)));
  b.push(Instruction::F64Mul);
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Const(b'0' as i32));
  b.push(Instruction::I32Sub);
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::F64Add);
  b.push(Instruction::LocalSet(7));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // If next char is '.': parse fractional part
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32LtU);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::I32Const(b'.' as i32));
  b.push(Instruction::I32Eq);
  b.push(Instruction::If(BlockType::Empty));
  // i++
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(4));
  // frac_mult = 0.1
  b.push(Instruction::F64Const(Ieee64::from(0.1f64)));
  b.push(Instruction::LocalSet(8));
  // Fractional loop
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32GeU);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalSet(6));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Const(b'0' as i32));
  b.push(Instruction::I32LtU);
  b.push(Instruction::BrIf(1));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Const(b'9' as i32));
  b.push(Instruction::I32GtU);
  b.push(Instruction::BrIf(1));
  // result = result + f64(b - '0') * frac_mult
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Const(b'0' as i32));
  b.push(Instruction::I32Sub);
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::F64Mul);
  b.push(Instruction::F64Add);
  b.push(Instruction::LocalSet(7));
  // frac_mult *= 0.1
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::F64Const(Ieee64::from(0.1f64)));
  b.push(Instruction::F64Mul);
  b.push(Instruction::LocalSet(8));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);
  b.push(Instruction::End); // end if '.'
  b.push(Instruction::End); // end if i < byte_len

  // Apply sign
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::F64Sub);
  b.push(Instruction::LocalSet(7));
  b.push(Instruction::End);

  b.push(Instruction::LocalGet(7));

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // ptr (1)
      ValType::I32, // byte_len (2)
      ValType::I32, // content (3)
      ValType::I32, // i (4)
      ValType::I32, // is_neg (5)
      ValType::I32, // b (6)
      ValType::F64, // result (7)
      ValType::F64, // frac_mult (8)
    ],
    instructions: b,
  }
}

/// `__rt_char_from_code(cp: f64) → f64`
///
/// Encodes a Unicode codepoint (u32) as a UTF-8 string and allocates it on the heap.
/// Returns the logical string pointer as f64.
#[allow(clippy::vec_init_then_push)]
fn build_rt_char_from_code(str_tag: i32) -> CompiledFn {
  // params: 0=cp(f64)
  // locals: 1=code(i32), 2=raw_base(i32), 3=new_ptr(i32), 4=byte_len(i32)
  let mut b: Vec<Instruction> = Vec::new();

  // code = i32(cp)
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(1));

  // Determine byte_len based on codepoint range
  // Default: 1 byte (ASCII)
  b.push(Instruction::I32Const(1));
  b.push(Instruction::LocalSet(4));

  // if code >= 0x80: byte_len = 2
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(0x80));
  b.push(Instruction::I32GeU);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::I32Const(2));
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::End);

  // if code >= 0x800: byte_len = 3
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(0x800));
  b.push(Instruction::I32GeU);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::I32Const(3));
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::End);

  // if code >= 0x10000: byte_len = 4
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(0x10000));
  b.push(Instruction::I32GeU);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::End);

  // Allocate: raw_base = HEAP_PTR; write magic+tag; new_ptr = raw_base+8
  b.push(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.push(Instruction::LocalTee(2)); // raw_base
  b.push(Instruction::I32Const(HEAP_MAGIC));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(str_tag));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  // new_ptr = raw_base + 8
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(3));
  // store byte_len as f64 at new_ptr+0
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::F64Store(mem_arg_f64(0)));
  // advance heap: raw_base + 16 + ((byte_len+7)&-8)
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32Const(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-8i32));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Add);
  b.push(Instruction::GlobalSet(HEAP_PTR_GLOBAL));

  // Write UTF-8 bytes at new_ptr+8 based on byte_len
  // content_base = new_ptr + 8
  // byte_len == 1: store byte code (0..7F)
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Eq);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Store8(mem_arg_byte(8)));
  b.push(Instruction::End);

  // byte_len == 2: 110xxxxx 10xxxxxx
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(2));
  b.push(Instruction::I32Eq);
  b.push(Instruction::If(BlockType::Empty));
  // b0 = 0xC0 | (code >> 6)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0xC0));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(6));
  b.push(Instruction::I32ShrU);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(8)));
  // b1 = 0x80 | (code & 0x3F)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0x80));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(0x3F));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(9)));
  b.push(Instruction::End);

  // byte_len == 3: 1110xxxx 10xxxxxx 10xxxxxx
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(3));
  b.push(Instruction::I32Eq);
  b.push(Instruction::If(BlockType::Empty));
  // b0 = 0xE0 | (code >> 12)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0xE0));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(12));
  b.push(Instruction::I32ShrU);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(8)));
  // b1 = 0x80 | ((code >> 6) & 0x3F)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0x80));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(6));
  b.push(Instruction::I32ShrU);
  b.push(Instruction::I32Const(0x3F));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(9)));
  // b2 = 0x80 | (code & 0x3F)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0x80));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(0x3F));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(10)));
  b.push(Instruction::End);

  // byte_len == 4: 11110xxx 10xxxxxx 10xxxxxx 10xxxxxx
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::I32Eq);
  b.push(Instruction::If(BlockType::Empty));
  // b0 = 0xF0 | (code >> 18)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0xF0));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(18));
  b.push(Instruction::I32ShrU);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(8)));
  // b1 = 0x80 | ((code >> 12) & 0x3F)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0x80));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(12));
  b.push(Instruction::I32ShrU);
  b.push(Instruction::I32Const(0x3F));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(9)));
  // b2 = 0x80 | ((code >> 6) & 0x3F)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0x80));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(6));
  b.push(Instruction::I32ShrU);
  b.push(Instruction::I32Const(0x3F));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(10)));
  // b3 = 0x80 | (code & 0x3F)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(0x80));
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32Const(0x3F));
  b.push(Instruction::I32And);
  b.push(Instruction::I32Or);
  b.push(Instruction::I32Store8(mem_arg_byte(11)));
  b.push(Instruction::End);

  // return f64(new_ptr)
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::F64ConvertI32U);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // code (1)
      ValType::I32, // raw_base (2)
      ValType::I32, // new_ptr (3)
      ValType::I32, // byte_len (4)
    ],
    instructions: b,
  }
}

/// `__rt_str_replace(s: f64, pat: f64, rep: f64) → f64`
///
/// Replaces ALL non-overlapping occurrences of `pat` in `s` with `rep`.
/// Writes output directly to heap — no separate scratch buffer needed.
///
/// Layout of a string logical ptr P: [byte_len:f64][utf8_bytes...]
#[allow(clippy::vec_init_then_push)]
fn build_rt_str_replace(str_tag: i32) -> CompiledFn {
  // params: 0=s(f64), 1=pat(f64), 2=rep(f64)
  // locals: 3=s_ptr(i32), 4=s_len(i32), 5=s_cont(i32)
  //         6=pat_ptr(i32), 7=pat_len(i32), 8=pat_cont(i32)
  //         9=rep_ptr(i32), 10=rep_len(i32), 11=rep_cont(i32)
  //         12=raw_base(i32), 13=content_base(i32)
  //         14=out_pos(i32), 15=si(i32)
  //         16=pi(i32), 17=matched(i32)
  //         18=new_ptr(i32), 19=padded(i32)
  //         20=max_out(i32), 21=tmp(i32)
  let mut b: Vec<Instruction> = Vec::new();

  // unpack s
  b.push(Instruction::LocalGet(0));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(3));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::F64Load(mem_arg_f64(0)));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(4));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(5));

  // unpack pat
  b.push(Instruction::LocalGet(1));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(6));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::F64Load(mem_arg_f64(0)));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(7));
  b.push(Instruction::LocalGet(6));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(8));

  // if pat_len == 0: return s unchanged
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Eqz);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::LocalGet(3));
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::Return);
  b.push(Instruction::End);

  // unpack rep
  b.push(Instruction::LocalGet(2));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(9));
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::F64Load(mem_arg_f64(0)));
  b.push(Instruction::I32TruncF64U);
  b.push(Instruction::LocalSet(10));
  b.push(Instruction::LocalGet(9));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(11));

  // max_out = s_len + (s_len + 1) * rep_len (worst case: all chars replaced)
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(10));
  b.push(Instruction::I32Mul);
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(20));

  // raw_base = HEAP_PTR (save before allocating)
  b.push(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  b.push(Instruction::LocalSet(12));

  // content_base = raw_base + 16 (after 8-byte header + 8-byte byte_len)
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(13));

  // padded = (max_out + 7) & -8
  b.push(Instruction::LocalGet(20));
  b.push(Instruction::I32Const(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-8i32));
  b.push(Instruction::I32And);
  b.push(Instruction::LocalSet(19));

  // Pessimistic bump: HEAP_PTR = raw_base + 16 + padded
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(19));
  b.push(Instruction::I32Add);
  b.push(Instruction::GlobalSet(HEAP_PTR_GLOBAL));

  // out_pos = 0, si = 0
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(14));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(15));

  // Main scan loop: while si + pat_len <= s_len
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  // break if si + pat_len > s_len
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::I32GtU);
  b.push(Instruction::BrIf(1));

  // Try to match: pi = 0, matched = 1
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(16));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::LocalSet(17));

  // Inner compare loop
  b.push(Instruction::Block(BlockType::Empty));
  b.push(Instruction::Loop(BlockType::Empty));
  // if pi >= pat_len: done (matched)
  b.push(Instruction::LocalGet(16));
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32GeU);
  b.push(Instruction::BrIf(1));
  // if s_cont[si+pi] != pat_cont[pi]: mismatch
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(8));
  b.push(Instruction::LocalGet(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::I32Ne);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::LocalSet(17)); // matched = 0
  b.push(Instruction::Br(2)); // break out of inner block
  b.push(Instruction::End);
  // pi++; continue
  b.push(Instruction::LocalGet(16));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(16));
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // if matched: copy rep → content_base+out_pos, si += pat_len, out_pos += rep_len
  b.push(Instruction::LocalGet(17));
  b.push(Instruction::If(BlockType::Empty));
  // memory.copy(content_base + out_pos, rep_cont, rep_len)
  b.push(Instruction::LocalGet(13));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(11));
  b.push(Instruction::LocalGet(10));
  b.push(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::LocalGet(10));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(14)); // out_pos += rep_len
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::LocalGet(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(15)); // si += pat_len
  b.push(Instruction::Else);
  // not matched: copy 1 byte; si++; out_pos++
  b.push(Instruction::LocalGet(13));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Load8U(mem_arg_byte(0)));
  b.push(Instruction::I32Store8(mem_arg_byte(0)));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(14));
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::I32Const(1));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(15));
  b.push(Instruction::End);
  b.push(Instruction::Br(0));
  b.push(Instruction::End);
  b.push(Instruction::End);

  // Copy tail: s_cont[si..s_len] → content_base + out_pos
  b.push(Instruction::LocalGet(4));
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::I32Sub);
  b.push(Instruction::LocalSet(21)); // tmp = s_len - si
  b.push(Instruction::LocalGet(21));
  b.push(Instruction::I32Const(0));
  b.push(Instruction::I32GtU);
  b.push(Instruction::If(BlockType::Empty));
  b.push(Instruction::LocalGet(13));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(5));
  b.push(Instruction::LocalGet(15));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(21));
  b.push(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::LocalGet(21));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(14));
  b.push(Instruction::End);

  // Write header: magic, str_tag, byte_len at raw_base
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(HEAP_MAGIC));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(4));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(str_tag));
  b.push(Instruction::I32Store(mem_arg_i32(0)));
  // new_ptr = raw_base + 8
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(8));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalSet(18));
  // byte_len = out_pos
  b.push(Instruction::LocalGet(18));
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::F64ConvertI32U);
  b.push(Instruction::F64Store(mem_arg_f64(0)));

  // Correct HEAP_PTR: raw_base + 16 + ((out_pos + 7) & -8)
  b.push(Instruction::LocalGet(14));
  b.push(Instruction::I32Const(7));
  b.push(Instruction::I32Add);
  b.push(Instruction::I32Const(-8i32));
  b.push(Instruction::I32And);
  b.push(Instruction::LocalSet(19));
  b.push(Instruction::LocalGet(12));
  b.push(Instruction::I32Const(16));
  b.push(Instruction::I32Add);
  b.push(Instruction::LocalGet(19));
  b.push(Instruction::I32Add);
  b.push(Instruction::GlobalSet(HEAP_PTR_GLOBAL));

  // return f64(new_ptr)
  b.push(Instruction::LocalGet(18));
  b.push(Instruction::F64ConvertI32U);

  CompiledFn {
    export_name: None,
    params: vec![ValType::F64, ValType::F64, ValType::F64],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // s_ptr (3)
      ValType::I32, // s_len (4)
      ValType::I32, // s_cont (5)
      ValType::I32, // pat_ptr (6)
      ValType::I32, // pat_len (7)
      ValType::I32, // pat_cont (8)
      ValType::I32, // rep_ptr (9)
      ValType::I32, // rep_len (10)
      ValType::I32, // rep_cont (11)
      ValType::I32, // raw_base (12)
      ValType::I32, // content_base (13)
      ValType::I32, // out_pos (14)
      ValType::I32, // si (15)
      ValType::I32, // pi (16)
      ValType::I32, // matched (17)
      ValType::I32, // new_ptr (18)
      ValType::I32, // padded (19)
      ValType::I32, // max_out (20)
      ValType::I32, // tmp (21)
    ],
    instructions: b,
  }
}

/// `__rt_str_escape(s: f64) → f64`
///
/// Escapes special characters in a string and wraps in double quotes.
/// Currently a stub that returns the input string unchanged.
#[allow(clippy::vec_init_then_push)]
fn build_rt_str_escape(_str_tag: i32) -> CompiledFn {
  let mut b: Vec<Instruction> = Vec::new();
  b.push(Instruction::LocalGet(0));
  CompiledFn {
    export_name: None,
    params: vec![ValType::F64],
    results: vec![ValType::F64],
    locals: vec![],
    instructions: b,
  }
}

/// `__rt_map_equal(a: i32, b: i32) → i32`
/// Returns 1 if maps a and b have the same key-value pairs (shallow value equality), else 0.
fn build_rt_map_equal(map_linearize_idx: u32, map_get_value_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2); // a=0, b=1
  let count_a = b.alloc_i32();
  let count_b = b.alloc_i32();
  let flat = b.alloc_i32();
  let i = b.alloc_i32();
  let key = b.alloc_f64();
  let val_a = b.alloc_f64();
  let val_b = b.alloc_f64();
  let all_eq = b.alloc_i32();

  // count_a = a[0] as i32
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count_a));

  // count_b = b[0] as i32
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(count_b));

  // if count_a != count_b → return 0
  b.emit(Instruction::LocalGet(count_a));
  b.emit(Instruction::LocalGet(count_b));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Else);

  // if count_a == 0 → return 1
  b.emit(Instruction::LocalGet(count_a));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Else);

  // linearize a → flat
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::Call(map_linearize_idx));
  b.emit(Instruction::LocalSet(flat));

  // all_eq = 1, i = 0
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(all_eq));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));

  // loop over pairs
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  // if i >= count_a, exit
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(count_a));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));

  // key = flat + 8 + i*16
  b.emit(Instruction::LocalGet(flat));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::LocalSet(key));

  // val_a = flat + 8 + i*16 + 8
  b.emit(Instruction::LocalGet(flat));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::LocalSet(val_a));

  // val_b = __rt_map_get_value(b_ptr, key)
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(key));
  b.emit(Instruction::Call(map_get_value_idx));
  b.emit(Instruction::LocalSet(val_b));

  // if val_a != val_b → all_eq = 0, break
  b.emit(Instruction::LocalGet(val_a));
  b.emit(Instruction::LocalGet(val_b));
  b.emit(Instruction::F64Eq);
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(all_eq));
  b.emit(Instruction::Br(2)); // break out of block
  b.emit(Instruction::End);

  // i++
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End); // loop
  b.emit(Instruction::End); // block

  b.emit(Instruction::LocalGet(all_eq));
  b.emit(Instruction::End); // end count_a == 0 else
  b.emit(Instruction::End); // end count_a != count_b if

  b.finish(vec![ValType::I32, ValType::I32], vec![ValType::I32])
}

/// `__rt_str_find_from(h_ptr: i32, h_start: i32, pat_ptr: i32) -> i32`
/// Searches for pat in h starting at byte offset h_start.
/// Returns the byte offset of the first match, or -1 if not found.
fn build_rt_str_find_from() -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(3); // h_ptr=0, h_start=1, pat_ptr=2
  let h_len = b.alloc_i32();
  let p_len = b.alloc_i32();
  let h_base = b.alloc_i32();
  let p_base = b.alloc_i32();
  let i = b.alloc_i32();
  let j = b.alloc_i32();
  let limit = b.alloc_i32();
  let bh = b.alloc_i32();
  let bp = b.alloc_i32();

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(h_len));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(p_len));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(h_base));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(p_base));

  // Block (result i32) for early-exit
  b.emit(Instruction::Block(wasm_encoder::BlockType::Result(ValType::I32)));

  // if p_len == 0: return h_start
  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Br(1));
  b.emit(Instruction::End);

  // if h_start + p_len > h_len: return -1
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(h_len));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::Br(1));
  b.emit(Instruction::End);

  // limit = h_len - p_len
  b.emit(Instruction::LocalGet(h_len));
  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(limit));

  // i = h_start
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalSet(i));

  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty)); // $exit
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty)); // $outer
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(limit));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::BrIf(1)); // $exit
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(j));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty)); // $mismatch
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty)); // $inner
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  // nesting: 0=If, 1=$inner, 2=$mismatch, 3=$outer, 4=$exit, 5=Block(i32)
  b.emit(Instruction::Br(5));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(h_base));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(bh));
  b.emit(Instruction::LocalGet(p_base));
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalSet(bp));
  b.emit(Instruction::LocalGet(bh));
  b.emit(Instruction::LocalGet(bp));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::BrIf(1)); // $mismatch
  b.emit(Instruction::LocalGet(j));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(j));
  b.emit(Instruction::Br(0)); // $inner
  b.emit(Instruction::End); // end $inner
  b.emit(Instruction::End); // end $mismatch
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0)); // $outer
  b.emit(Instruction::End); // end $outer
  b.emit(Instruction::End); // end $exit
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::End); // end Block(i32)

  b.finish(vec![ValType::I32, ValType::I32, ValType::I32], vec![ValType::I32])
}

/// `__rt_utf8_char_len(b: i32) -> i32`
/// Given the first byte of a UTF-8 sequence, returns its byte width (1-4).
fn build_rt_utf8_char_len() -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(1);
  b.emit(Instruction::Block(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(0x80));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Br(1));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(0xE0));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::Br(1));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(0xF0));
  b.emit(Instruction::I32LtU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::I32Const(3));
  b.emit(Instruction::Br(1));
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::End);
  b.finish(vec![ValType::I32], vec![ValType::I32])
}

/// `__rt_str_split(s_ptr: i32, pat_ptr: i32) -> i32` (list logical ptr)
/// Splits string s by pat. Empty pieces are dropped. Empty pat = char-split.
fn build_rt_str_split(string_tag: i32, list_tag: i32, find_from_idx: u32, utf8_char_len_idx: u32) -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2); // s_ptr=0, pat_ptr=1
  let s_len = b.alloc_i32();
  let p_len = b.alloc_i32();
  let s_cont = b.alloc_i32();
  let count = b.alloc_i32();
  let i = b.alloc_i32();
  let prev = b.alloc_i32();
  let list_ptr = b.alloc_i32();
  let li = b.alloc_i32();
  let idx = b.alloc_i32();
  let piece_len = b.alloc_i32();
  let str_ptr = b.alloc_i32();
  let str_cont = b.alloc_i32();
  let char_len = b.alloc_i32();
  let padded = b.alloc_i32();
  let size = b.alloc_i32();
  let ki = b.alloc_i32();
  let list_size = b.alloc_i32();

  // load s_len, p_len, s_cont
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(s_len));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(p_len));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(s_cont));

  // --- PASS 1: COUNT ---
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  // empty-pat: count UTF-8 chars
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(s_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(s_cont));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::Call(utf8_char_len_idx));
  b.emit(Instruction::LocalSet(char_len));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(char_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  // non-empty pat: count delimited pieces
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(prev));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(prev));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(find_from_idx));
  b.emit(Instruction::LocalSet(idx));
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  // last segment
  b.emit(Instruction::LocalGet(s_len));
  b.emit(Instruction::LocalGet(prev));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(piece_len));
  b.emit(Instruction::LocalGet(piece_len));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::End);
  b.emit(Instruction::Br(2)); // break loop
  b.emit(Instruction::End);
  // mid segment
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::LocalGet(prev));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(piece_len));
  b.emit(Instruction::LocalGet(piece_len));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(count));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(prev));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::End); // if p_len == 0

  // --- ALLOCATE LIST ---
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(list_size));
  rt_emit_alloc_dynamic(&mut b, list_size, list_ptr, list_tag);
  b.emit(Instruction::LocalGet(list_ptr));
  b.emit(Instruction::LocalGet(count));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));

  // --- PASS 2: FILL ---
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(li));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(prev));

  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  // empty-pat: emit each UTF-8 char as a string
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(s_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(s_cont));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::Call(utf8_char_len_idx));
  b.emit(Instruction::LocalSet(char_len));
  // padded = (char_len + 7) & -8
  b.emit(Instruction::LocalGet(char_len));
  b.emit(Instruction::I32Const(7));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(-8));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(padded));
  b.emit(Instruction::LocalGet(padded));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(size));
  rt_emit_alloc_dynamic(&mut b, size, str_ptr, string_tag);
  b.emit(Instruction::LocalGet(str_ptr));
  b.emit(Instruction::LocalGet(char_len));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(str_ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(str_cont));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(ki));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::LocalGet(char_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(str_cont));
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(s_cont));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(ki));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  // store str_ptr in list
  b.emit(Instruction::LocalGet(list_ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(li));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(str_ptr));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(li));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(li));
  b.emit(Instruction::LocalGet(i));
  b.emit(Instruction::LocalGet(char_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(i));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  // non-empty pat: emit each delimited piece
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(prev));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(find_from_idx));
  b.emit(Instruction::LocalSet(idx));
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(s_len));
  b.emit(Instruction::LocalGet(prev));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(piece_len));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::LocalGet(prev));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(piece_len));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(piece_len));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(piece_len));
  b.emit(Instruction::I32Const(7));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(-8));
  b.emit(Instruction::I32And);
  b.emit(Instruction::LocalSet(padded));
  b.emit(Instruction::LocalGet(padded));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(size));
  rt_emit_alloc_dynamic(&mut b, size, str_ptr, string_tag);
  b.emit(Instruction::LocalGet(str_ptr));
  b.emit(Instruction::LocalGet(piece_len));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(str_ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(str_cont));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(ki));
  b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::LocalGet(piece_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(str_cont));
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(s_cont));
  b.emit(Instruction::LocalGet(prev));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Store8(mem_arg_byte(0)));
  b.emit(Instruction::LocalGet(ki));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(ki));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(list_ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(li));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(str_ptr));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(li));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(li));
  b.emit(Instruction::End); // if piece_len > 0
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::I32Const(-1));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::BrIf(1)); // exit
  b.emit(Instruction::LocalGet(idx));
  b.emit(Instruction::LocalGet(p_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(prev));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);

  b.emit(Instruction::End);
  b.emit(Instruction::End); // if p_len == 0

  b.emit(Instruction::LocalGet(list_ptr));
  b.finish(vec![ValType::I32, ValType::I32], vec![ValType::I32])
}

/// `__rt_value_equal(a: f64, b: f64) → i32`
///
/// Deep structural equality:
/// - Numbers/booleans/nil: f64 equality.
/// - Strings: byte-by-byte comparison via __rt_str_compare.
/// - Lists: element-wise recursive comparison.
/// - Other heap objects: pointer (f64) equality.
fn build_rt_value_equal(string_tag: i32, list_tag: i32, str_compare_idx: u32, self_idx: u32) -> CompiledFn {
  // params: 0 = a (f64), 1 = b (f64)
  let mut b = RuntimeFnBuilder::new(2);
  let result = b.alloc_i32(); // 0=not-equal, 1=equal
  let ptr_a = b.alloc_i32();
  let ptr_b = b.alloc_i32();
  let tag_a = b.alloc_i32();
  let tag_b = b.alloc_i32();
  let cnt = b.alloc_i32();
  let i = b.alloc_i32();
  let elem_a = b.alloc_f64();
  let elem_b = b.alloc_f64();
  let heap_min = (HEAP_BASE + 8) as f64;

  // Fast path: exact f64 equality
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Else);

  // Check both are valid heap pointers
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::F64Const(Ieee64::from(heap_min)));
  b.emit(Instruction::F64Ge);
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::F64Const(Ieee64::from(heap_min)));
  b.emit(Instruction::F64Ge);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));

  // Convert to i32 pointers
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(ptr_a));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(ptr_b));

  // Read type tags (ptr - 4 = raw_base + 4)
  b.emit(Instruction::LocalGet(ptr_a));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(tag_a));
  b.emit(Instruction::LocalGet(ptr_b));
  b.emit(Instruction::I32Const(4));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(tag_b));

  // If different types → not equal
  b.emit(Instruction::LocalGet(tag_a));
  b.emit(Instruction::LocalGet(tag_b));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Else);

  // Same type: dispatch on tag

  // String comparison
  b.emit(Instruction::LocalGet(tag_a));
  b.emit(Instruction::I32Const(string_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  // str_compare returns f64: 0.0 = equal
  b.emit(Instruction::LocalGet(ptr_a));
  b.emit(Instruction::LocalGet(ptr_b));
  b.emit(Instruction::Call(str_compare_idx));
  b.emit(Instruction::F64Const(Ieee64::from(0.0f64)));
  b.emit(Instruction::F64Eq);
  b.emit(Instruction::Else);

  // List deep comparison
  b.emit(Instruction::LocalGet(tag_a));
  b.emit(Instruction::I32Const(list_tag));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  {
    // count_a == count_b?
    b.emit(Instruction::LocalGet(ptr_a));
    b.emit(Instruction::F64Load(mem_arg_f64(0)));
    b.emit(Instruction::I32TruncF64U);
    b.emit(Instruction::LocalSet(cnt));
    b.emit(Instruction::LocalGet(ptr_b));
    b.emit(Instruction::F64Load(mem_arg_f64(0)));
    b.emit(Instruction::I32TruncF64U);
    b.emit(Instruction::LocalGet(cnt));
    b.emit(Instruction::I32Ne);
    b.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::Else);
    // Compare each element recursively
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::LocalSet(result));
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::LocalSet(i));
    // block (break-out-on-mismatch)
    b.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
    b.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
    // exit when i >= cnt
    b.emit(Instruction::LocalGet(i));
    b.emit(Instruction::LocalGet(cnt));
    b.emit(Instruction::I32GeU);
    b.emit(Instruction::BrIf(1)); // break out of block
    // elem_a = ptr_a[8 + i*8]
    b.emit(Instruction::LocalGet(ptr_a));
    b.emit(Instruction::I32Const(8));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalGet(i));
    b.emit(Instruction::I32Const(8));
    b.emit(Instruction::I32Mul);
    b.emit(Instruction::I32Add);
    b.emit(Instruction::F64Load(mem_arg_f64(0)));
    b.emit(Instruction::LocalSet(elem_a));
    // elem_b = ptr_b[8 + i*8]
    b.emit(Instruction::LocalGet(ptr_b));
    b.emit(Instruction::I32Const(8));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalGet(i));
    b.emit(Instruction::I32Const(8));
    b.emit(Instruction::I32Mul);
    b.emit(Instruction::I32Add);
    b.emit(Instruction::F64Load(mem_arg_f64(0)));
    b.emit(Instruction::LocalSet(elem_b));
    // if !value_equal(elem_a, elem_b) → result=0, break
    b.emit(Instruction::LocalGet(elem_a));
    b.emit(Instruction::LocalGet(elem_b));
    b.emit(Instruction::Call(self_idx));
    b.emit(Instruction::I32Eqz);
    b.emit(Instruction::If(wasm_encoder::BlockType::Empty));
    b.emit(Instruction::I32Const(0));
    b.emit(Instruction::LocalSet(result));
    b.emit(Instruction::Br(2)); // break out of block
    b.emit(Instruction::End);
    // i++
    b.emit(Instruction::LocalGet(i));
    b.emit(Instruction::I32Const(1));
    b.emit(Instruction::I32Add);
    b.emit(Instruction::LocalSet(i));
    b.emit(Instruction::Br(0)); // continue loop
    b.emit(Instruction::End); // loop
    b.emit(Instruction::End); // block
    b.emit(Instruction::LocalGet(result));
    b.emit(Instruction::End); // count_ne if
  }

  b.emit(Instruction::Else);
  // Other heap types: not equal (different pointers already checked above)
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::End); // list if

  b.emit(Instruction::End); // string if
  b.emit(Instruction::End); // tag_ne if
  b.emit(Instruction::Else);
  // Not both heap pointers → not equal
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::End); // both_heap if

  b.emit(Instruction::End); // fast-path if

  b.finish(vec![ValType::F64, ValType::F64], vec![ValType::I32])
}
