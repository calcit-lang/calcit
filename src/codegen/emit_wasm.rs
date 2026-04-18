//! Minimal WASM codegen for Calcit — generates binary `.wasm` via `wasm-encoder`.
//!
//! Supports a small subset of Calcit for demonstration purposes:
//! - `defn` with fixed-arity arguments (all f64)
//! - `let` bindings
//! - `if` conditionals
//! - Arithmetic: `&+`, `&-`, `&*`, `&/`, `&number:rem`
//! - Comparisons: `&<`, `&>`, `&=`
//! - `recur` (tail recursion via WASM loop)
//! - Number literals, Bool literals, Nil (→ 0.0)
//! - Tag values (compiled to f64 integer constants)
//! - Record creation (`&%{}`) and field access (`&record:nth`, `&record:get`)
//! - Tuple creation (`::`) and field access (`&tuple:nth`)
//!
//! All values are represented as f64 (matching Calcit's single numeric type).
//! Booleans: true → 1.0, false/nil → 0.0.
//! Tags: mapped to positive f64 integers at compile time.
//! Record/Tuple pointers: i32 offsets into linear memory, converted to/from f64.
//! Output is a `.wasm` binary that can be loaded by Node.js, Deno, or any WASM runtime.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use wasm_encoder::{
  CodeSection, ConstExpr, ExportKind, ExportSection, Function, FunctionSection, GlobalSection, GlobalType, Ieee64, Instruction,
  MemorySection, MemoryType, Module, TypeSection, ValType,
};

use crate::builtins::syntax::get_raw_args_fn;
use crate::calcit::{
  Calcit, CalcitArgLabel, CalcitFnArgs, CalcitImport, CalcitLocal, CalcitProc, CalcitStruct, CalcitSyntax, MethodKind,
};
use crate::program;

/// Base offset — reserve first 16 bytes for bookkeeping.
/// The actual heap start will be shifted when string literals occupy the
/// initial segment (see `build_string_pool`).
const HEAP_BASE: i32 = 16;
/// Global index for the heap pointer (bump allocator).
const HEAP_PTR_GLOBAL: u32 = 0;
/// Magic marker written at `raw_base` of every heap allocation. Used by
/// `type-of` to distinguish real pointers from raw f64 numbers that happen to
/// fall inside the heap address range. Value chosen to be unlikely to appear
/// as the low 32 bits of a typical integer f64 value.
const HEAP_MAGIC: i32 = 0xCA1C_17A9u32 as i32;

/// Convert f64 to wasm-encoder's Ieee64 representation.
fn f64_const(v: f64) -> Instruction<'static> {
  Instruction::F64Const(Ieee64::from(v))
}

/// MemArg for f64 load/store (8-byte aligned, memory 0).
fn mem_arg_f64(offset: u64) -> wasm_encoder::MemArg {
  wasm_encoder::MemArg {
    offset,
    align: 3, // log2(8) = 3
    memory_index: 0,
  }
}

/// MemArg for i32 load/store (4-byte aligned, memory 0).
fn mem_arg_i32(offset: u64) -> wasm_encoder::MemArg {
  wasm_encoder::MemArg {
    offset,
    align: 2, // log2(4) = 2
    memory_index: 0,
  }
}

/// Emit a WASM binary module from the compiled program.
/// Processes functions from all namespaces in the program.
pub fn emit_wasm(init_ns: &str, emit_path: &str) -> Result<(), String> {
  let program_data = program::clone_compiled_program_snapshot()?;

  // First pass: extract all function signatures from all namespaces
  let mut fn_defs: Vec<(String, String, CalcitFnArgs, Vec<Calcit>)> = Vec::new(); // (ns, def_name, args, body)

  // Collect init_ns first, then other namespaces (ordering for export clarity)
  let mut ns_order: Vec<&str> = Vec::new();
  if program_data.contains_key(init_ns) {
    ns_order.push(init_ns);
  }
  for ns in program_data.keys() {
    if ns.as_ref() != init_ns {
      ns_order.push(ns);
    }
  }

  for &ns in &ns_order {
    let Some(file_info) = program_data.get(ns) else {
      continue;
    };
    for (def_name, compiled) in &file_info.defs {
      if compiled.kind != program::CompiledDefKind::Fn {
        continue;
      }
      match extract_fn_parts(&compiled.preprocessed_code) {
        Ok((args, body)) => {
          fn_defs.push((ns.to_string(), def_name.to_string(), args, body));
        }
        Err(e) => {
          eprintln!("[wasm] skipping {ns}/{def_name}: {e}");
        }
      }
    }
  }

  if fn_defs.is_empty() {
    return Err(format!("namespace not found or no functions: {init_ns}"));
  }

  // Build fn_index: host imports first, then user functions offset by num_imports
  let num_imports = HOST_IMPORTS.len() as u32;
  let (mut compiled_fns, runtime_fn_index) = build_runtime_fns(num_imports);
  let runtime_fn_count = compiled_fns.len() as u32;
  let mut export_name_counts: HashMap<String, usize> = HashMap::new();
  for (_, name, _, _) in &fn_defs {
    *export_name_counts.entry(name.clone()).or_insert(0) += 1;
  }
  let mut fn_index: HashMap<String, u32> = HashMap::new();
  let mut fn_arity: HashMap<String, u32> = HashMap::new();
  // Track functions with rest args: value is the fixed-arity count (params before `&`).
  // WASM arity for such functions is `fixed_arity + 1` (the rest list pointer).
  let mut fn_has_rest: HashMap<String, u32> = HashMap::new();
  for (i, (ns, name, args, _)) in fn_defs.iter().enumerate() {
    let idx = num_imports + runtime_fn_count + i as u32;
    let qualified = format!("{ns}/{name}");
    fn_index.insert(qualified.clone(), idx);
    // Also insert bare name — last writer wins; bare name is fallback
    fn_index.insert(name.clone(), idx);
    // Compute WASM arity (only Idx labels, not markers)
    let (arity, rest_fixed) = compute_fn_arity(args);
    fn_arity.insert(qualified.clone(), arity);
    fn_arity.insert(name.clone(), arity);
    if let Some(fixed) = rest_fixed {
      fn_has_rest.insert(qualified, fixed);
      fn_has_rest.insert(name.clone(), fixed);
    }
  }

  // Collect all tags referenced in function bodies
  let tag_index = collect_all_tags_from(&fn_defs);
  let record_field_tags = collect_record_field_tags_from_program(&program_data, &tag_index);

  // Build string literal pool: assigns each unique string a memory offset.
  let (string_pool, string_data_segment, heap_start) = build_string_pool(&fn_defs, &tag_index);

  let env = WasmCompileEnv {
    fn_index,
    fn_arity,
    fn_has_rest,
    runtime_fn_index,
    tag_index,
    record_field_tags,
    string_pool,
  };

  // Second pass: compile. If a function fails, we still reserve its slot
  // with a trivial body so indices remain stable.
  for (ns, def_name, args, body) in &fn_defs {
    let export_name = if export_name_counts.get(def_name).copied().unwrap_or(0) > 1 {
      format!("{ns}/{def_name}")
    } else {
      def_name.clone()
    };
    match compile_fn(def_name, &export_name, args, body, &env) {
      Ok(func) => compiled_fns.push(func),
      Err(e) => {
        eprintln!("[wasm] skipping {ns}/{def_name}: {e}");
        let (arity, _) = compute_fn_arity(args);
        compiled_fns.push(CompiledFn {
          export_name: Some(export_name),
          params: vec![ValType::F64; arity as usize],
          results: vec![ValType::F64],
          locals: vec![],
          instructions: vec![f64_const(0.0)],
        });
      }
    }
  }

  if compiled_fns.is_empty() {
    return Err("no functions could be compiled to WASM".into());
  }

  // Build module using wasm-encoder
  let wasm_bytes = build_wasm_module(&compiled_fns, heap_start, &string_data_segment)?;

  // Write output
  let out_path = Path::new(emit_path);
  if !out_path.exists() {
    fs::create_dir_all(out_path).map_err(|e| format!("failed to create dir: {e}"))?;
  }
  let wasm_file = out_path.join("program.wasm");
  fs::write(&wasm_file, &wasm_bytes).map_err(|e| format!("failed to write WASM: {e}"))?;
  println!("wrote WASM to: {}", wasm_file.display());

  Ok(())
}

/// Intermediate representation of a compiled function before encoding.
struct CompiledFn {
  export_name: Option<String>,
  params: Vec<ValType>,
  results: Vec<ValType>,
  /// All local variables (including temporaries), indexed by declaration order
  locals: Vec<ValType>,
  /// Instruction sequence for the function body
  instructions: Vec<Instruction<'static>>,
}

#[derive(Clone)]
struct WasmCompileEnv {
  fn_index: HashMap<String, u32>,
  fn_arity: HashMap<String, u32>,
  fn_has_rest: HashMap<String, u32>,
  runtime_fn_index: HashMap<String, u32>,
  tag_index: HashMap<String, u32>,
  record_field_tags: HashMap<u32, Vec<u32>>,
  string_pool: HashMap<String, u32>,
}

/// Build a binary WASM module from compiled functions.
/// Host-imported function specification.
struct HostImport {
  module: &'static str,
  name: &'static str,
  arity: usize,
}

/// List of host-imported functions.
/// These are provided by the JS environment and indexed before user functions.
const HOST_IMPORTS: &[HostImport] = &[
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

/// Build a binary WASM module from compiled functions.
/// Host imports occupy the first function indices (0..HOST_IMPORTS.len()),
/// then user functions follow at indices HOST_IMPORTS.len()..
fn build_wasm_module(fns: &[CompiledFn], heap_start: i32, string_data: &[u8]) -> Result<Vec<u8>, String> {
  let mut module = Module::new();
  let num_imports = HOST_IMPORTS.len() as u32;

  // Type section: host imports first, then user functions
  let mut types = TypeSection::new();
  for imp in HOST_IMPORTS {
    let params: Vec<ValType> = vec![ValType::F64; imp.arity];
    types.ty().function(params, vec![ValType::F64]);
  }
  for f in fns {
    types.ty().function(f.params.clone(), f.results.clone());
  }
  module.section(&types);

  // Import section: host functions
  let mut imports = wasm_encoder::ImportSection::new();
  for (i, imp) in HOST_IMPORTS.iter().enumerate() {
    imports.import(imp.module, imp.name, wasm_encoder::EntityType::Function(i as u32));
  }
  module.section(&imports);

  // Function section: map each user function to its type (offset by num_imports)
  let mut functions = FunctionSection::new();
  for (i, _) in fns.iter().enumerate() {
    functions.function(num_imports + i as u32);
  }
  module.section(&functions);

  // Memory section: 1 page (64KB) for linear memory (records, tuples)
  let mut memories = MemorySection::new();
  memories.memory(MemoryType {
    minimum: 1,
    maximum: None,
    memory64: false,
    shared: false,
    page_size_log2: None,
  });
  module.section(&memories);

  // Global section: heap pointer for bump allocator
  let mut globals = GlobalSection::new();
  globals.global(
    GlobalType {
      val_type: ValType::I32,
      mutable: true,
      shared: false,
    },
    &ConstExpr::i32_const(heap_start),
  );
  module.section(&globals);

  // Export section: user functions (indices offset by num_imports)
  let mut exports = ExportSection::new();
  exports.export("memory", ExportKind::Memory, 0);
  for (i, f) in fns.iter().enumerate() {
    if let Some(export_name) = &f.export_name {
      exports.export(export_name, ExportKind::Func, num_imports + i as u32);
    }
  }
  module.section(&exports);

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

fn build_runtime_fns(base_index: u32) -> (Vec<CompiledFn>, HashMap<String, u32>) {
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
  fns.push(build_rt_map_make());

  let map_from_flat_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_from_flat"), map_from_flat_idx);
  fns.push(build_rt_map_from_flat(hash_idx, map_root_assoc_idx, map_make_idx));

  let map_root_from_flat_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_root_from_flat"), map_root_from_flat_idx);
  fns.push(build_rt_map_root_from_flat(hash_idx, map_root_assoc_idx));

  let map_linearize_idx = base_index + fns.len() as u32;
  fn_index.insert(String::from("__rt_map_linearize"), map_linearize_idx);
  fns.push(build_rt_map_linearize(map_root_write_pairs_idx));

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

fn rt_emit_alloc_const(builder: &mut RuntimeFnBuilder, byte_size: i32, dst_local: u32) {
  let raw = builder.alloc_i32();
  builder.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  builder.emit(Instruction::LocalTee(raw));
  builder.emit(Instruction::I32Const(HEAP_MAGIC));
  builder.emit(Instruction::I32Store(mem_arg_i32(0)));
  builder.emit(Instruction::LocalGet(raw));
  builder.emit(Instruction::I32Const(4));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::I32Const(0));
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

fn rt_emit_alloc_dynamic(builder: &mut RuntimeFnBuilder, size_local: u32, dst_local: u32) {
  let raw = builder.alloc_i32();
  builder.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  builder.emit(Instruction::LocalTee(raw));
  builder.emit(Instruction::I32Const(HEAP_MAGIC));
  builder.emit(Instruction::I32Store(mem_arg_i32(0)));
  builder.emit(Instruction::LocalGet(raw));
  builder.emit(Instruction::I32Const(4));
  builder.emit(Instruction::I32Add);
  builder.emit(Instruction::I32Const(0));
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
  rt_emit_alloc_const(builder, RT_MAP_TABLE_BYTES, dst_local);
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
  rt_emit_alloc_dynamic(builder, size, dst_local);
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

fn build_rt_map_make() -> CompiledFn {
  let mut b = RuntimeFnBuilder::new(2);
  let dst = b.alloc_i32();
  rt_emit_alloc_const(&mut b, 16, dst);
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
  rt_emit_alloc_const(&mut b, RT_MAP_TABLE_BYTES, new_root);
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
  rt_emit_alloc_const(&mut b, RT_MAP_TABLE_BYTES, new_root);
  b.emit(Instruction::I32Const(RT_MAP_TABLE_SLOTS));
  b.emit(Instruction::LocalSet(slots));
  rt_emit_copy_slots(&mut b, copy_fn_idx, new_root, 0, slots);
  rt_emit_alloc_const(&mut b, RT_MAP_TABLE_BYTES, new_table1);
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

  rt_emit_alloc_const(&mut b, RT_MAP_TABLE_BYTES, new_root);
  b.emit(Instruction::I32Const(RT_MAP_TABLE_SLOTS));
  b.emit(Instruction::LocalSet(slots));
  rt_emit_copy_slots(&mut b, copy_fn_idx, new_root, 0, slots);
  rt_emit_alloc_const(&mut b, RT_MAP_TABLE_BYTES, new_table1);
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

fn build_rt_map_linearize(root_write_pairs_idx: u32) -> CompiledFn {
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
  rt_emit_alloc_dynamic(&mut b, size, dst);
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
  rt_emit_alloc_dynamic(&mut b, size, dst);
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

/// Extract function name, args, and body from preprocessed `(defn name (args...) body...)` form.
fn extract_fn_parts(code: &Calcit) -> Result<(CalcitFnArgs, Vec<Calcit>), String> {
  let Calcit::List(items) = code else {
    return Err(format!("expected preprocessed defn list, got: {code}"));
  };
  match (items.first(), items.get(1), items.get(2)) {
    (Some(Calcit::Syntax(CalcitSyntax::Defn, _)), Some(Calcit::Symbol { .. }), Some(Calcit::List(args))) => {
      let raw_args = get_raw_args_fn(args)?;
      Ok((raw_args, items.drop_left().drop_left().drop_left().to_vec()))
    }
    _ => Err(format!("expected preprocessed defn form, got: {code}")),
  }
}

/// Context for WASM code generation within a single function.
struct WasmGenCtx {
  /// Map from local variable name to WASM local index
  locals: HashMap<String, u32>,
  /// Local declarations to add (beyond parameters)
  extra_locals: Vec<ValType>,
  /// Next local index (starts after parameters)
  next_local: u32,
  /// Whether this function uses recur (needs loop wrapping)
  uses_recur: bool,
  /// Argument local indices in order (for recur)
  arg_indices: Vec<u32>,
  /// Collected instructions
  instructions: Vec<Instruction<'static>>,
  /// Function name → index map for cross-function calls
  fn_index: HashMap<String, u32>,
  /// Function name → WASM arity (number of f64 params, excluding markers)
  fn_arity: HashMap<String, u32>,
  /// Function name → fixed-arity (params before `&`) for functions with rest args.
  /// WASM arity for these is `fixed_arity + 1` (the rest list pointer).
  fn_has_rest: HashMap<String, u32>,
  /// Internal runtime helper name → function index.
  runtime_fn_index: HashMap<String, u32>,
  /// Tag name → integer ID map (compile-time constant, shared across all functions)
  tag_index: HashMap<String, u32>,
  /// Record struct tag id → field tag ids in index order.
  record_field_tags: HashMap<u32, Vec<u32>>,
  /// Current block nesting depth relative to the recur loop
  /// (0 = directly inside the loop, 1 = inside one if/block, etc.)
  block_depth: u32,
  /// String literal pool: string content → logical pointer (f64).
  /// Strings are pre-allocated in a data segment before the heap.
  string_pool: HashMap<String, u32>,
}

impl WasmGenCtx {
  fn new(num_params: u32, env: WasmCompileEnv) -> Self {
    WasmGenCtx {
      locals: HashMap::new(),
      extra_locals: Vec::new(),
      next_local: num_params,
      uses_recur: false,
      arg_indices: Vec::new(),
      instructions: Vec::new(),
      fn_index: env.fn_index,
      fn_arity: env.fn_arity,
      fn_has_rest: env.fn_has_rest,
      runtime_fn_index: env.runtime_fn_index,
      tag_index: env.tag_index,
      record_field_tags: env.record_field_tags,
      block_depth: 0,
      string_pool: env.string_pool,
    }
  }

  /// Allocate an anonymous f64 local variable.
  fn alloc_local(&mut self) -> u32 {
    self.alloc_local_typed(ValType::F64)
  }

  /// Allocate an anonymous local variable of the given type.
  fn alloc_local_typed(&mut self, vt: ValType) -> u32 {
    let idx = self.next_local;
    self.next_local += 1;
    self.extra_locals.push(vt);
    idx
  }

  fn declare_local(&mut self, name: &str) -> u32 {
    let idx = self.alloc_local();
    self.locals.insert(name.to_owned(), idx);
    idx
  }

  fn emit(&mut self, instr: Instruction<'static>) {
    self.instructions.push(instr);
  }
}

/// Compute WASM arity for a function signature.
///
/// Returns `(wasm_arity, rest_fixed)`:
/// - `wasm_arity`: total number of f64 params the function takes
/// - `rest_fixed`: if the function has a rest arg, the number of fixed params
///   before `&` (`wasm_arity - 1`); otherwise `None`.
///
/// Optional marks (`?`) are transparent: callers pad nil for omitted optional args.
/// Rest args (`&`) are represented as a single f64 list-pointer param.
fn compute_fn_arity(args: &CalcitFnArgs) -> (u32, Option<u32>) {
  match args {
    CalcitFnArgs::Args(v) => (v.len() as u32, None),
    CalcitFnArgs::MarkedArgs(labels) => {
      let mut fixed: u32 = 0;
      let mut rest_param_count: u32 = 0;
      let mut rest_seen = false;
      for label in labels {
        match label {
          CalcitArgLabel::Idx(_) => {
            if rest_seen {
              rest_param_count += 1;
            } else {
              fixed += 1;
            }
          }
          CalcitArgLabel::OptionalMark => {}
          CalcitArgLabel::RestMark => {
            rest_seen = true;
          }
        }
      }
      if rest_seen && rest_param_count > 0 {
        (fixed + 1, Some(fixed))
      } else {
        (fixed, None)
      }
    }
  }
}

fn compile_fn(
  _name: &str,
  export_name: &str,
  args: &CalcitFnArgs,
  body: &[Calcit],
  env: &WasmCompileEnv,
) -> Result<CompiledFn, String> {
  let mut param_names = Vec::new();
  match args {
    CalcitFnArgs::Args(idxs) => {
      for idx in idxs {
        param_names.push(CalcitLocal::read_name(*idx));
      }
    }
    CalcitFnArgs::MarkedArgs(labels) => {
      // Track `&` marker: the next Idx after RestMark is the rest-args list param.
      // We still add it as a regular f64 param (holding the list pointer),
      // so no special handling is needed beyond accepting the marker.
      let mut seen_rest = false;
      for label in labels {
        match label {
          CalcitArgLabel::Idx(idx) => {
            param_names.push(CalcitLocal::read_name(*idx));
            if seen_rest {
              // Only one rest param allowed — ignore any extras defensively.
              seen_rest = false;
            }
          }
          CalcitArgLabel::OptionalMark => {
            // Optional marker — not a parameter slot, just skip it.
            // The caller always passes all args (nil for omitted optional ones).
          }
          CalcitArgLabel::RestMark => {
            seen_rest = true;
          }
        }
      }
    }
  }

  let arity = param_names.len();
  let mut ctx = WasmGenCtx::new(arity as u32, env.clone());

  // Register parameter locals
  for (i, pname) in param_names.iter().enumerate() {
    ctx.locals.insert(pname.clone(), i as u32);
    ctx.arg_indices.push(i as u32);
  }

  // Check if body uses recur
  ctx.uses_recur = body.iter().any(check_uses_recur);

  if ctx.uses_recur {
    // loop $recur (result f64) ... end
    ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Result(ValType::F64)));
    emit_body(&mut ctx, body)?;
    ctx.emit(Instruction::End); // end loop
  } else {
    emit_body(&mut ctx, body)?;
  }

  Ok(CompiledFn {
    export_name: Some(export_name.to_owned()),
    params: vec![ValType::F64; arity],
    results: vec![ValType::F64],
    locals: ctx.extra_locals,
    instructions: ctx.instructions,
  })
}

fn check_uses_recur(expr: &Calcit) -> bool {
  match expr {
    Calcit::Proc(CalcitProc::Recur) => true,
    Calcit::List(xs) => {
      // Don't recurse into nested defn
      if let Some(Calcit::Syntax(CalcitSyntax::Defn, _)) = xs.first() {
        return false;
      }
      xs.iter().any(check_uses_recur)
    }
    _ => false,
  }
}

/// Emit instructions for a sequence of expressions (last is the return value).
fn emit_body(ctx: &mut WasmGenCtx, exprs: &[Calcit]) -> Result<(), String> {
  if exprs.is_empty() {
    ctx.emit(f64_const(0.0));
    return Ok(());
  }
  for (i, expr) in exprs.iter().enumerate() {
    emit_expr(ctx, expr)?;
    if i < exprs.len() - 1 {
      ctx.emit(Instruction::Drop);
    }
  }
  Ok(())
}

/// Emit instructions for a single Calcit expression.
fn emit_expr(ctx: &mut WasmGenCtx, expr: &Calcit) -> Result<(), String> {
  match expr {
    Calcit::Number(n) => {
      ctx.emit(f64_const(*n));
    }
    Calcit::Bool(true) => {
      ctx.emit(f64_const(1.0));
    }
    Calcit::Bool(false) | Calcit::Nil => {
      ctx.emit(f64_const(0.0));
    }
    Calcit::Tag(t) => {
      let tag_str = t.to_string();
      let id = *ctx
        .tag_index
        .get(&tag_str)
        .ok_or_else(|| format!("unknown tag in WASM codegen: {tag_str}"))?;
      ctx.emit(f64_const(id as f64));
    }
    Calcit::Local(local) => {
      let name = &*local.sym;
      let idx = *ctx.locals.get(name).ok_or_else(|| format!("undefined local variable: {name}"))?;
      ctx.emit(Instruction::LocalGet(idx));
    }
    Calcit::List(xs) if !xs.is_empty() => {
      emit_call_expr(ctx, xs)?;
    }
    // `do` appears as a bare (non-call) expression when used as a body sequencer in defn.
    // It's a no-op — just emit nil so it can be dropped by emit_body.
    Calcit::Import(import) if import.def.as_ref() == "do" => {
      ctx.emit(f64_const(0.0));
    }
    Calcit::Str(s) => {
      let ptr = ctx
        .string_pool
        .get(s.as_ref())
        .ok_or_else(|| format!("string literal not found in pool: {s}"))?;
      ctx.emit(f64_const(*ptr as f64));
    }
    Calcit::Record(_) => return Err("Record literals not supported in WASM codegen (use constructor)".into()),
    Calcit::Tuple(_) => return Err("Tuple literals not supported in WASM codegen (use constructor)".into()),
    _ => return Err(format!("unsupported WASM expression: {expr}")),
  }
  Ok(())
}

/// Emit instructions for a call expression.
fn emit_call_expr(ctx: &mut WasmGenCtx, xs: &crate::calcit::CalcitList) -> Result<(), String> {
  let head = &xs[0];
  let args_list: Vec<Calcit> = xs.drop_left().to_vec();

  match head {
    Calcit::Syntax(syn, _) => match syn {
      CalcitSyntax::If => emit_if(ctx, &args_list),
      CalcitSyntax::CoreLet => emit_let(ctx, &args_list),
      CalcitSyntax::Match => emit_match(ctx, &args_list),
      CalcitSyntax::HintFn => {
        // hint-fn is metadata-only; emit nothing (0.0 placeholder)
        ctx.emit(f64_const(0.0));
        Ok(())
      }
      CalcitSyntax::AssertType => {
        // assert-type checks type at preprocess time; at runtime just evaluate the expression
        if args_list.is_empty() {
          return Err("assert-type expects at least 1 arg".into());
        }
        emit_expr(ctx, &args_list[0])
      }
      CalcitSyntax::Defn => Err("nested defn not supported in WASM".into()),
      _ => Err(format!("unsupported syntax in WASM: {syn}")),
    },
    Calcit::Proc(proc) => emit_proc_call(ctx, proc, &args_list),
    Calcit::Method(name, kind) => match kind {
      MethodKind::Invoke(_) => emit_method_invoke(ctx, name.as_ref(), &args_list),
      _ => Err(format!("unsupported method in WASM: .{name}")),
    },
    Calcit::Import(import) => {
      // `do` is a sequencing form in calcit.core — emit all args, return last
      if import.def.as_ref() == "do" {
        return emit_body(ctx, &args_list);
      }
      // Try qualified "ns/def" first, then bare "def" as fallback
      let qualified = format!("{}/{}", import.ns, import.def);
      let fn_idx = ctx
        .fn_index
        .get(&qualified)
        .or_else(|| ctx.fn_index.get(import.def.as_ref()))
        .ok_or_else(|| format!("unknown function: {qualified}"))?;
      let fn_idx = *fn_idx;
      let target_arity = ctx
        .fn_arity
        .get(&qualified)
        .or_else(|| ctx.fn_arity.get(import.def.as_ref()))
        .copied()
        .unwrap_or(args_list.len() as u32);
      let rest_fixed = ctx
        .fn_has_rest
        .get(&qualified)
        .or_else(|| ctx.fn_has_rest.get(import.def.as_ref()))
        .copied();
      emit_call_args(ctx, &args_list, target_arity, rest_fixed)?;
      ctx.emit(Instruction::Call(fn_idx));
      Ok(())
    }
    Calcit::Symbol { sym, .. } => {
      let name = sym.as_ref();
      // IO functions: call host log_value for each arg, return nil
      if matches!(name, "println" | "eprintln" | "echo") {
        let log_idx = HOST_IMPORTS
          .iter()
          .position(|imp| imp.module == "io" && imp.name == "log_value")
          .expect("log_value host import") as u32;
        for arg in &args_list {
          emit_expr(ctx, arg)?;
          ctx.emit(Instruction::Call(log_idx));
          ctx.emit(Instruction::Drop); // drop log_value's return
        }
        ctx.emit(f64_const(0.0)); // nil
        return Ok(());
      }
      let fn_idx = *ctx.fn_index.get(name).ok_or_else(|| format!("unknown function: {sym}"))?;
      let target_arity = ctx.fn_arity.get(name).copied().unwrap_or(args_list.len() as u32);
      let rest_fixed = ctx.fn_has_rest.get(name).copied();
      emit_call_args(ctx, &args_list, target_arity, rest_fixed)?;
      ctx.emit(Instruction::Call(fn_idx));
      Ok(())
    }
    Calcit::Registered(name) => {
      // Registered procs (eprintln, println, echo, etc.)
      let name = name.as_ref();
      if matches!(name, "println" | "eprintln" | "echo") {
        let log_idx = HOST_IMPORTS
          .iter()
          .position(|imp| imp.module == "io" && imp.name == "log_value")
          .expect("log_value host import") as u32;
        for arg in &args_list {
          emit_expr(ctx, arg)?;
          ctx.emit(Instruction::Call(log_idx));
          ctx.emit(Instruction::Drop);
        }
        ctx.emit(f64_const(0.0)); // nil
        return Ok(());
      }
      Err(format!("unsupported registered proc in WASM: {name}"))
    }
    _ => Err(format!("unsupported call head in WASM: {head}")),
  }
}

fn emit_method_invoke(ctx: &mut WasmGenCtx, name: &str, args: &[Calcit]) -> Result<(), String> {
  if args.is_empty() {
    return Err(format!("method .{name} expects at least 1 operand"));
  }

  let receiver = ctx.alloc_local();
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(receiver));

  let extra_locals: Vec<u32> = args[1..]
    .iter()
    .map(|arg| {
      let local = ctx.alloc_local();
      emit_expr(ctx, arg)?;
      ctx.emit(Instruction::LocalSet(local));
      Ok(local)
    })
    .collect::<Result<_, String>>()?;

  match name {
    "empty?" => emit_method_empty(ctx, receiver),
    "count" => emit_method_count(ctx, receiver),
    "first" => {
      if !extra_locals.is_empty() {
        return Err("method .first expects 0 arguments".into());
      }
      emit_method_first(ctx, receiver)
    }
    "rest" => {
      if !extra_locals.is_empty() {
        return Err("method .rest expects 0 arguments".into());
      }
      emit_method_rest(ctx, receiver)
    }
    "nth" => {
      if extra_locals.len() != 1 {
        return Err("method .nth expects 1 argument".into());
      }
      emit_method_nth(ctx, receiver, extra_locals[0])
    }
    "get" => {
      if extra_locals.len() != 1 {
        return Err("method .get expects 1 argument".into());
      }
      emit_method_get(ctx, receiver, extra_locals[0])
    }
    _ => Err(format!("unsupported invoke method in WASM: .{name}")),
  }
}

fn emit_type_of_local(ctx: &mut WasmGenCtx, value_local: u32) {
  let number_tag = get_type_tag(ctx, "number");
  let is_valid_ptr = ctx.alloc_local_typed(ValType::I32);
  let raw_base = ctx.alloc_local_typed(ValType::I32);

  ctx.emit(Instruction::LocalGet(value_local));
  ctx.emit(Instruction::LocalGet(value_local));
  ctx.emit(Instruction::F64Trunc);
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::LocalGet(value_local));
  ctx.emit(f64_const((HEAP_BASE + 8) as f64));
  ctx.emit(Instruction::F64Ge);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalGet(value_local));
  ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Lt);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalSet(is_valid_ptr));

  ctx.emit(Instruction::LocalGet(value_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(raw_base));

  ctx.emit(Instruction::LocalGet(is_valid_ptr));
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  ctx.emit(Instruction::LocalGet(raw_base));
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::I32Const(HEAP_MAGIC));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  ctx.emit(Instruction::LocalGet(raw_base));
  ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::Else);
  ctx.emit(f64_const(number_tag));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::Else);
  ctx.emit(f64_const(number_tag));
  ctx.emit(Instruction::End);
}

fn emit_heap_count_from_local(ctx: &mut WasmGenCtx, receiver_local: u32) {
  ctx.emit(Instruction::LocalGet(receiver_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
}

fn emit_list_nth_from_local(ctx: &mut WasmGenCtx, receiver_local: u32, index_local: u32) {
  ctx.emit(Instruction::LocalGet(index_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::LocalGet(receiver_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
}

fn emit_list_first_from_local(ctx: &mut WasmGenCtx, receiver_local: u32) {
  ctx.emit(Instruction::LocalGet(receiver_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
}

fn emit_list_rest_from_local(ctx: &mut WasmGenCtx, receiver_local: u32) {
  let src = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(receiver_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(src));

  let old_count = emit_load_count_i32(ctx, src);
  let new_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(old_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(new_count));

  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(new_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));

  let dst = emit_alloc_with_count(ctx, new_count, total_slots, "list");
  let dst_base = emit_addr_offset(ctx, dst, 8);
  let src_base = emit_addr_offset(ctx, src, 16);
  emit_copy_f64_loop(ctx, dst_base, src_base, new_count);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
}

fn emit_tuple_nth_from_local(ctx: &mut WasmGenCtx, receiver_local: u32, index_local: u32) {
  ctx.emit(Instruction::LocalGet(index_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::LocalGet(receiver_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
}

fn emit_map_get_from_local(ctx: &mut WasmGenCtx, receiver_local: u32, key_local: u32) {
  let fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_map_get_value")
    .expect("runtime helper __rt_map_get_value must exist");
  ctx.emit(Instruction::LocalGet(receiver_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalGet(key_local));
  ctx.emit(Instruction::Call(fn_idx));
}

fn emit_method_empty(ctx: &mut WasmGenCtx, receiver_local: u32) -> Result<(), String> {
  let nil_tag = get_type_tag(ctx, "nil");
  let list_tag = get_type_tag(ctx, "list");
  let map_tag = get_type_tag(ctx, "map");
  let set_tag = get_type_tag(ctx, "set");
  let string_tag = get_type_tag(ctx, "string");
  let tuple_tag = get_type_tag(ctx, "tuple");
  let record_tag = get_type_tag(ctx, "record");
  let type_local = ctx.alloc_local();
  emit_type_of_local(ctx, receiver_local);
  ctx.emit(Instruction::LocalSet(type_local));

  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(nil_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::Else);

  for tag in [list_tag, map_tag, set_tag, string_tag, tuple_tag, record_tag] {
    ctx.emit(Instruction::LocalGet(type_local));
    ctx.emit(f64_const(tag));
    ctx.emit(Instruction::F64Eq);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
    ctx.emit(f64_const(1.0));
    ctx.emit(f64_const(0.0));
    emit_heap_count_from_local(ctx, receiver_local);
    ctx.emit(f64_const(0.0));
    ctx.emit(Instruction::F64Eq);
    ctx.emit(Instruction::Select);
    ctx.emit(Instruction::Else);
  }

  ctx.emit(f64_const(0.0));
  for _ in 0..6 {
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_method_count(ctx: &mut WasmGenCtx, receiver_local: u32) -> Result<(), String> {
  let nil_tag = get_type_tag(ctx, "nil");
  let list_tag = get_type_tag(ctx, "list");
  let map_tag = get_type_tag(ctx, "map");
  let set_tag = get_type_tag(ctx, "set");
  let string_tag = get_type_tag(ctx, "string");
  let tuple_tag = get_type_tag(ctx, "tuple");
  let record_tag = get_type_tag(ctx, "record");
  let type_local = ctx.alloc_local();
  emit_type_of_local(ctx, receiver_local);
  ctx.emit(Instruction::LocalSet(type_local));

  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(nil_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::Else);

  for tag in [list_tag, map_tag, set_tag, string_tag, tuple_tag, record_tag] {
    ctx.emit(Instruction::LocalGet(type_local));
    ctx.emit(f64_const(tag));
    ctx.emit(Instruction::F64Eq);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
    emit_heap_count_from_local(ctx, receiver_local);
    ctx.emit(Instruction::Else);
  }

  ctx.emit(f64_const(0.0));
  for _ in 0..6 {
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_method_nth(ctx: &mut WasmGenCtx, receiver_local: u32, index_local: u32) -> Result<(), String> {
  let list_tag = get_type_tag(ctx, "list");
  let tuple_tag = get_type_tag(ctx, "tuple");
  let type_local = ctx.alloc_local();
  emit_type_of_local(ctx, receiver_local);
  ctx.emit(Instruction::LocalSet(type_local));

  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(list_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_list_nth_from_local(ctx, receiver_local, index_local);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(tuple_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_tuple_nth_from_local(ctx, receiver_local, index_local);
  ctx.emit(Instruction::Else);
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_method_first(ctx: &mut WasmGenCtx, receiver_local: u32) -> Result<(), String> {
  let list_tag = get_type_tag(ctx, "list");
  let tuple_tag = get_type_tag(ctx, "tuple");
  let type_local = ctx.alloc_local();
  emit_type_of_local(ctx, receiver_local);
  ctx.emit(Instruction::LocalSet(type_local));

  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(list_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_list_first_from_local(ctx, receiver_local);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(tuple_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  let zero = ctx.alloc_local();
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalSet(zero));
  emit_tuple_nth_from_local(ctx, receiver_local, zero);
  ctx.emit(Instruction::Else);
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_method_rest(ctx: &mut WasmGenCtx, receiver_local: u32) -> Result<(), String> {
  let list_tag = get_type_tag(ctx, "list");
  let type_local = ctx.alloc_local();
  emit_type_of_local(ctx, receiver_local);
  ctx.emit(Instruction::LocalSet(type_local));

  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(list_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_list_rest_from_local(ctx, receiver_local);
  ctx.emit(Instruction::Else);
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_method_get(ctx: &mut WasmGenCtx, receiver_local: u32, key_local: u32) -> Result<(), String> {
  let map_tag = get_type_tag(ctx, "map");
  let list_tag = get_type_tag(ctx, "list");
  let tuple_tag = get_type_tag(ctx, "tuple");
  let type_local = ctx.alloc_local();
  emit_type_of_local(ctx, receiver_local);
  ctx.emit(Instruction::LocalSet(type_local));

  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(map_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_map_get_from_local(ctx, receiver_local, key_local);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(list_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_list_nth_from_local(ctx, receiver_local, key_local);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(type_local));
  ctx.emit(f64_const(tuple_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_tuple_nth_from_local(ctx, receiver_local, key_local);
  ctx.emit(Instruction::Else);
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

/// Emit argument evaluation for a direct function call.
///
/// Handles three cases:
/// - Fixed-arity (no rest): evaluates each arg, pads nil for missing optional args.
/// - Rest args (callee has `&`): evaluates the first `fixed` args as-is, then
///   packs the remaining args into a list and passes that as the last f64 param.
fn emit_call_args(ctx: &mut WasmGenCtx, args_list: &[Calcit], target_arity: u32, rest_fixed: Option<u32>) -> Result<(), String> {
  match rest_fixed {
    Some(fixed) => {
      let fixed = fixed as usize;
      if args_list.len() < fixed {
        return Err(format!("rest-args call expected at least {} args, got {}", fixed, args_list.len()));
      }
      // Emit fixed args directly
      for arg in args_list.iter().take(fixed) {
        emit_expr(ctx, arg)?;
      }
      // Pack the rest into a list
      emit_list_new(ctx, &args_list[fixed..])?;
    }
    None => {
      for arg in args_list {
        emit_expr(ctx, arg)?;
      }
      // Pad nil for missing optional args
      for _ in args_list.len()..(target_arity as usize) {
        ctx.emit(f64_const(0.0));
      }
    }
  }
  Ok(())
}

/// Emit instructions for builtin proc calls.
fn emit_proc_call(ctx: &mut WasmGenCtx, proc: &CalcitProc, args: &[Calcit]) -> Result<(), String> {
  match proc {
    // Arithmetic
    CalcitProc::NativeAdd => emit_binary(ctx, Instruction::F64Add, args),
    CalcitProc::NativeMinus => emit_binary(ctx, Instruction::F64Sub, args),
    CalcitProc::NativeMultiply => emit_binary(ctx, Instruction::F64Mul, args),
    CalcitProc::NativeDivide => emit_binary(ctx, Instruction::F64Div, args),
    CalcitProc::NativeNumberRem => {
      // a - trunc(a/b) * b
      if args.len() != 2 {
        return Err("rem expects 2 args".into());
      }
      emit_expr(ctx, &args[0])?; // a
      emit_expr(ctx, &args[0])?; // a (again)
      emit_expr(ctx, &args[1])?; // b
      ctx.emit(Instruction::F64Div);
      ctx.emit(Instruction::F64Trunc);
      emit_expr(ctx, &args[1])?; // b (again)
      ctx.emit(Instruction::F64Mul);
      ctx.emit(Instruction::F64Sub);
      Ok(())
    }

    // Comparisons — produce f64 (1.0 or 0.0)
    CalcitProc::NativeLessThan => emit_cmp(ctx, Instruction::F64Lt, args),
    CalcitProc::NativeGreaterThan => emit_cmp(ctx, Instruction::F64Gt, args),
    CalcitProc::NativeEquals | CalcitProc::Identical => emit_cmp(ctx, Instruction::F64Eq, args),
    CalcitProc::NativeCompare => {
      // &compare a b → -1.0 if a<b, 0.0 if a==b, 1.0 if a>b
      if args.len() != 2 {
        return Err(format!("&compare expects 2 args, got {}", args.len()));
      }
      let a = ctx.alloc_local();
      let b = ctx.alloc_local();
      emit_expr(ctx, &args[0])?;
      ctx.emit(Instruction::LocalSet(a));
      emit_expr(ctx, &args[1])?;
      ctx.emit(Instruction::LocalSet(b));
      // if a < b then -1 else (if a > b then 1 else 0)
      ctx.emit(Instruction::LocalGet(a));
      ctx.emit(Instruction::LocalGet(b));
      ctx.emit(Instruction::F64Lt);
      ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
      ctx.emit(f64_const(-1.0));
      ctx.emit(Instruction::Else);
      ctx.emit(Instruction::LocalGet(a));
      ctx.emit(Instruction::LocalGet(b));
      ctx.emit(Instruction::F64Gt);
      ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
      ctx.emit(f64_const(1.0));
      ctx.emit(Instruction::Else);
      ctx.emit(f64_const(0.0));
      ctx.emit(Instruction::End);
      ctx.emit(Instruction::End);
      Ok(())
    }
    CalcitProc::Not => {
      if args.len() != 1 {
        return Err("not expects 1 arg".into());
      }
      // not: 0.0 → 1.0, else → 0.0
      ctx.emit(f64_const(1.0)); // true result
      ctx.emit(f64_const(0.0)); // false result
      emit_expr(ctx, &args[0])?;
      ctx.emit(f64_const(0.0));
      ctx.emit(Instruction::F64Eq); // i32 condition
      ctx.emit(Instruction::Select);
      Ok(())
    }

    // Math (unary)
    CalcitProc::Floor => emit_unary(ctx, Instruction::F64Floor, args),
    CalcitProc::Ceil => emit_unary(ctx, Instruction::F64Ceil, args),
    CalcitProc::Round => emit_unary(ctx, Instruction::F64Nearest, args),
    CalcitProc::Sqrt => emit_unary(ctx, Instruction::F64Sqrt, args),
    CalcitProc::Sin => emit_host_call(ctx, "sin", args),
    CalcitProc::Cos => emit_host_call(ctx, "cos", args),
    CalcitProc::Pow => emit_host_call(ctx, "pow", args),

    // type-of: reads the heap type header or returns :number for non-pointers.
    CalcitProc::TypeOf => emit_type_of(ctx, args),

    // type predicates
    CalcitProc::ListQuestion => emit_type_predicate(ctx, "list", args),
    CalcitProc::TagQuestion => emit_type_predicate(ctx, "tag", args),
    CalcitProc::SymbolQuestion => emit_type_predicate(ctx, "symbol", args),
    CalcitProc::NilQuestion => emit_type_predicate(ctx, "nil", args),
    CalcitProc::StringQuestion => emit_type_predicate(ctx, "string", args),
    CalcitProc::MapQuestion => emit_type_predicate(ctx, "map", args),
    CalcitProc::NumberQuestion => emit_type_predicate(ctx, "number", args),
    CalcitProc::BoolQuestion => emit_type_predicate(ctx, "bool", args),
    CalcitProc::SetQuestion => emit_type_predicate(ctx, "set", args),
    CalcitProc::TupleQuestion => emit_type_predicate(ctx, "tuple", args),
    CalcitProc::RecordQuestion => emit_type_predicate(ctx, "record", args),
    CalcitProc::FnQuestion => emit_type_predicate(ctx, "fn", args),

    // Recur
    CalcitProc::Recur => {
      if args.len() != ctx.arg_indices.len() {
        return Err(format!(
          "recur arity mismatch: expected {}, got {}",
          ctx.arg_indices.len(),
          args.len()
        ));
      }
      // Evaluate all args into temp locals first
      let mut temps = Vec::new();
      for arg in args {
        let tmp = ctx.alloc_local();
        emit_expr(ctx, arg)?;
        ctx.emit(Instruction::LocalSet(tmp));
        temps.push(tmp);
      }
      // Copy temps back to arg locals
      for (i, &tmp) in temps.iter().enumerate() {
        ctx.emit(Instruction::LocalGet(tmp));
        ctx.emit(Instruction::LocalSet(ctx.arg_indices[i]));
      }
      ctx.emit(Instruction::Br(ctx.block_depth)); // br to the recur loop
      // After unconditional br, mark as unreachable for the type checker
      ctx.emit(Instruction::Unreachable);
      Ok(())
    }

    // Record operations
    CalcitProc::NativeRecord => emit_record_new(ctx, args),
    CalcitProc::NativeRecordNth => emit_record_nth(ctx, args),
    CalcitProc::NativeRecordGet => emit_record_get(ctx, args),
    CalcitProc::NativeRecordCount => emit_record_count(ctx, args),
    CalcitProc::NativeRecordFieldTag => emit_record_field_tag(ctx, args),
    CalcitProc::NativeRecordAssoc | CalcitProc::NativeRecordAssocAt | CalcitProc::NativeRecordWith => {
      Err("Record mutation (assoc/with) not yet supported in WASM codegen".into())
    }
    CalcitProc::NativeRecordFromMap
    | CalcitProc::NativeRecordToMap
    | CalcitProc::NativeRecordExtendAs
    | CalcitProc::NativeRecordPartial
    | CalcitProc::NativeRecordContains
    | CalcitProc::NativeRecordStruct
    | CalcitProc::NativeRecordGetName
    | CalcitProc::NativeRecordImpls
    | CalcitProc::NativeRecordWithAt
    | CalcitProc::NativeLooseRecord => Err(format!("Record operation {proc} not yet supported in WASM codegen")),
    CalcitProc::NativeRecordMatches => emit_record_matches(ctx, args),

    // Tuple operations
    CalcitProc::NativeTuple => emit_tuple_new(ctx, args),
    CalcitProc::NativeTupleNth => emit_tuple_nth(ctx, args),
    CalcitProc::NativeTupleCount => emit_tuple_count(ctx, args),
    CalcitProc::NativeTupleValidateEnum => {
      // Runtime enum validation — no-op in WASM, just evaluate args and discard
      for arg in args {
        emit_expr(ctx, arg)?;
        ctx.emit(Instruction::Drop);
      }
      ctx.emit(f64_const(0.0)); // nil
      Ok(())
    }
    CalcitProc::NativeEnumTupleNew
    | CalcitProc::NativeTupleImpls
    | CalcitProc::NativeTupleParams
    | CalcitProc::NativeTupleEnum
    | CalcitProc::NativeTupleImplTraits
    | CalcitProc::NativeTupleEnumHasVariant
    | CalcitProc::NativeTupleEnumVariantArity => Err(format!("Tuple operation {proc} not yet supported in WASM codegen")),
    CalcitProc::NativeTupleAssoc => emit_tuple_assoc(ctx, args),

    // Bitwise operations — convert to i32, operate, convert back to f64
    CalcitProc::BitShl => emit_bitwise_binary(ctx, Instruction::I32Shl, args),
    CalcitProc::BitShr => emit_bitwise_binary(ctx, Instruction::I32ShrS, args),
    CalcitProc::BitAnd => emit_bitwise_binary(ctx, Instruction::I32And, args),
    CalcitProc::BitOr => emit_bitwise_binary(ctx, Instruction::I32Or, args),
    CalcitProc::BitXor => emit_bitwise_binary(ctx, Instruction::I32Xor, args),
    CalcitProc::BitNot => {
      if args.len() != 1 {
        return Err("bit-not expects 1 arg".into());
      }
      emit_expr(ctx, &args[0])?;
      ctx.emit(Instruction::I32TruncF64S);
      ctx.emit(Instruction::I32Const(-1)); // all bits set
      ctx.emit(Instruction::I32Xor);
      ctx.emit(Instruction::F64ConvertI32S);
      Ok(())
    }

    // Raise — terminates execution
    CalcitProc::Raise => {
      // `raise` aborts the program; emit WASM unreachable trap.
      // Any preceding args are evaluated for side effects but discarded.
      for arg in args {
        emit_expr(ctx, arg)?;
        ctx.emit(Instruction::Drop);
      }
      ctx.emit(Instruction::Unreachable);
      Ok(())
    }

    // ------- List operations -------
    CalcitProc::List => emit_list_new(ctx, args),
    CalcitProc::Append => emit_list_append(ctx, args),
    CalcitProc::Prepend => emit_list_prepend(ctx, args),
    CalcitProc::Butlast => emit_list_butlast(ctx, args),
    CalcitProc::NativeListCount => emit_ds_count(ctx, args),
    CalcitProc::NativeListNth => emit_list_nth(ctx, args),
    CalcitProc::NativeListFirst => emit_list_first(ctx, args),
    CalcitProc::NativeListRest => emit_list_rest(ctx, args),
    CalcitProc::NativeListEmpty => emit_ds_empty(ctx, args),
    CalcitProc::NativeListSlice => emit_list_slice(ctx, args),
    CalcitProc::NativeListReverse => emit_list_reverse(ctx, args),
    CalcitProc::NativeListConcat => emit_list_concat(ctx, args),
    CalcitProc::NativeListAssoc => emit_list_assoc(ctx, args),
    CalcitProc::NativeListDissoc => emit_list_dissoc(ctx, args),
    CalcitProc::NativeListContains => emit_list_contains(ctx, args),
    CalcitProc::NativeListIncludes => emit_list_includes(ctx, args),
    CalcitProc::NativeListQ => emit_list_q(ctx, args),

    // ------- BufList operations -------
    CalcitProc::NativeBufListNew => emit_buf_list_new(ctx, args),
    CalcitProc::NativeBufListPush => emit_buf_list_push(ctx, args),
    CalcitProc::NativeBufListConcat => emit_buf_list_concat(ctx, args),
    CalcitProc::NativeBufListToList => emit_buf_list_to_list(ctx, args),
    CalcitProc::NativeBufListCount => emit_buf_list_count(ctx, args),

    // ------- Map operations -------
    CalcitProc::NativeMap => emit_map_new(ctx, args),
    CalcitProc::NativeMapGet => emit_map_get_op(ctx, args),
    CalcitProc::NativeMapAssoc => emit_map_assoc(ctx, args),
    CalcitProc::NativeMapDissoc => emit_map_dissoc(ctx, args),
    CalcitProc::NativeMapCount => emit_ds_count(ctx, args),
    CalcitProc::NativeMapEmpty => emit_ds_empty(ctx, args),
    CalcitProc::NativeMapContains => emit_map_contains(ctx, args),
    CalcitProc::NativeMapIncludes => emit_map_includes(ctx, args),
    CalcitProc::ToPairs => emit_map_to_pairs(ctx, args),
    CalcitProc::NativeMapToList => emit_map_to_list(ctx, args),

    // ------- Set operations -------
    CalcitProc::Set => emit_set_new(ctx, args),
    CalcitProc::NativeInclude => emit_set_include(ctx, args),
    CalcitProc::NativeExclude => emit_set_exclude(ctx, args),
    CalcitProc::NativeSetCount => emit_ds_count(ctx, args),
    CalcitProc::NativeSetEmpty => emit_ds_empty(ctx, args),
    CalcitProc::NativeSetIncludes => emit_set_includes(ctx, args),
    CalcitProc::NativeDifference => emit_set_difference(ctx, args),
    CalcitProc::NativeUnion => emit_set_union(ctx, args),
    CalcitProc::NativeMerge => emit_map_merge(ctx, args),
    CalcitProc::NativeMapDiffNew => emit_map_diff_new(ctx, args),
    CalcitProc::NativeMapDiffKeys => emit_map_diff_keys(ctx, args),
    CalcitProc::NativeMapCommonKeys => emit_map_common_keys(ctx, args),
    CalcitProc::Range => emit_range(ctx, args),
    CalcitProc::NativeHash => emit_hash_proc(ctx, args),

    // List / String / Other — not yet supported
    _ => Err(format!("unsupported proc in WASM: {proc}")),
  }
}

fn emit_unary(ctx: &mut WasmGenCtx, instr: Instruction<'static>, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err(format!("{instr:?} expects 1 arg, got {}", args.len()));
  }
  emit_expr(ctx, &args[0])?;
  ctx.emit(instr);
  Ok(())
}

/// Emit `type-of v`. All heap objects carry a type tag at `raw_base` (ptr - 8),
/// so for values that look like heap pointers we read that tag; otherwise we
/// fall back to `:number`.
///
/// Pointer detection heuristic (enough for core-library usage patterns):
/// - value must be a finite integer (== trunc(v))
/// - value must be within `[HEAP_BASE + 8, current_heap_ptr)` (logical ptrs begin
///   8 bytes after the raw base)
/// - the i32 offset at `(ptr - 8)` must contain a registered type tag id
///
/// Values failing any check are reported as `:number`. This is a deliberate
/// simplification — distinguishing bool/nil/tag/number without NaN-boxing is
/// not supported and is unlikely to be needed by the subset of core functions
/// currently compiled.
fn emit_type_of(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err(format!("type-of expects 1 arg, got {}", args.len()));
  }
  let number_tag = get_type_tag(ctx, "number");
  let v_local = ctx.alloc_local_typed(ValType::F64);
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(v_local));

  let is_valid_ptr = ctx.alloc_local_typed(ValType::I32);
  let raw_base = ctx.alloc_local_typed(ValType::I32);

  // 1) integer-valued: v == trunc(v)
  ctx.emit(Instruction::LocalGet(v_local));
  ctx.emit(Instruction::LocalGet(v_local));
  ctx.emit(Instruction::F64Trunc);
  ctx.emit(Instruction::F64Eq);
  // 2) v >= (HEAP_BASE + 8) as f64  (lowest possible logical pointer)
  ctx.emit(Instruction::LocalGet(v_local));
  ctx.emit(f64_const((HEAP_BASE + 8) as f64));
  ctx.emit(Instruction::F64Ge);
  ctx.emit(Instruction::I32And);
  // 3) v < current heap_ptr
  ctx.emit(Instruction::LocalGet(v_local));
  ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Lt);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalSet(is_valid_ptr));

  // raw_base = trunc(v) - 8
  ctx.emit(Instruction::LocalGet(v_local));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(raw_base));

  // Short-circuit: only load memory when range is valid.
  ctx.emit(Instruction::LocalGet(is_valid_ptr));
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  // Check magic at raw_base+0 == HEAP_MAGIC.
  ctx.emit(Instruction::LocalGet(raw_base));
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::I32Const(HEAP_MAGIC));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  // Load tag id (i32) at raw_base+4 and convert to f64.
  ctx.emit(Instruction::LocalGet(raw_base));
  ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::Else);
  ctx.emit(f64_const(number_tag));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::Else);
  ctx.emit(f64_const(number_tag));
  ctx.emit(Instruction::End);
  Ok(())
}

/// Emit a type predicate like `list?`. Compares `type-of v` with the given
/// type tag and pushes 1.0 (true) or 0.0 (false).
fn emit_type_predicate(ctx: &mut WasmGenCtx, type_name: &str, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err(format!("{}? expects 1 arg, got {}", type_name, args.len()));
  }
  let expected_tag = get_type_tag(ctx, type_name);
  // Emit type-of, which leaves a tag f64 on the stack
  emit_type_of(ctx, args)?;
  // Compare with expected tag
  ctx.emit(f64_const(expected_tag));
  ctx.emit(Instruction::F64Eq);
  // Convert i32 boolean to f64
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// Emit a call to a host-imported function by name.
fn emit_host_call(ctx: &mut WasmGenCtx, name: &str, args: &[Calcit]) -> Result<(), String> {
  let import_idx = HOST_IMPORTS
    .iter()
    .position(|imp| imp.name == name)
    .ok_or_else(|| format!("unknown host import: {name}"))?;
  let expected_arity = HOST_IMPORTS[import_idx].arity;
  if args.len() != expected_arity {
    return Err(format!("{name} expects {expected_arity} args, got {}", args.len()));
  }
  for arg in args {
    emit_expr(ctx, arg)?;
  }
  ctx.emit(Instruction::Call(import_idx as u32));
  Ok(())
}

fn emit_binary(ctx: &mut WasmGenCtx, instr: Instruction<'static>, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err(format!("{instr:?} expects 2 args, got {}", args.len()));
  }
  emit_expr(ctx, &args[0])?;
  emit_expr(ctx, &args[1])?;
  ctx.emit(instr);
  Ok(())
}

fn emit_cmp(ctx: &mut WasmGenCtx, instr: Instruction<'static>, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err(format!("{instr:?} expects 2 args, got {}", args.len()));
  }
  // select (f64.const 1) (f64.const 0) (cmp a b)
  ctx.emit(f64_const(1.0));
  ctx.emit(f64_const(0.0));
  emit_expr(ctx, &args[0])?;
  emit_expr(ctx, &args[1])?;
  ctx.emit(instr);
  ctx.emit(Instruction::Select);
  Ok(())
}

/// Emit a binary bitwise operation: convert both args to i32, apply op, convert back to f64.
fn emit_bitwise_binary(ctx: &mut WasmGenCtx, instr: Instruction<'static>, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err(format!("{instr:?} expects 2 args, got {}", args.len()));
  }
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64S);
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64S);
  ctx.emit(instr);
  ctx.emit(Instruction::F64ConvertI32S);
  Ok(())
}

/// Emit WASM for `if` expression.
fn emit_if(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() < 2 || args.len() > 3 {
    return Err(format!("if expects 2-3 args, got {}", args.len()));
  }
  // condition → i32
  emit_expr(ctx, &args[0])?;
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Ne); // nonzero is truthy → i32

  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  ctx.block_depth += 1;
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::Else);
  if args.len() == 3 {
    emit_expr(ctx, &args[2])?;
  } else {
    ctx.emit(f64_const(0.0));
  }
  ctx.block_depth -= 1;
  ctx.emit(Instruction::End);
  Ok(())
}

/// Emit WASM for `let` expression.
fn emit_let(ctx: &mut WasmGenCtx, body: &[Calcit]) -> Result<(), String> {
  if body.is_empty() {
    ctx.emit(f64_const(0.0));
    return Ok(());
  }

  let pair = &body[0];
  let rest = &body[1..];

  match pair {
    Calcit::Nil => emit_body(ctx, rest),
    Calcit::List(xs) if xs.is_empty() => emit_body(ctx, rest),
    Calcit::List(xs) if xs.len() == 2 => {
      let var_name = match &xs[0] {
        Calcit::Local(CalcitLocal { sym, .. }) => sym.to_string(),
        Calcit::Symbol { sym, .. } => sym.to_string(),
        other => return Err(format!("let binding expected symbol, got: {other}")),
      };

      emit_expr(ctx, &xs[1])?;
      let idx = ctx.declare_local(&var_name);
      ctx.emit(Instruction::LocalSet(idx));

      // Flatten nested lets
      if rest.len() == 1 {
        if let Calcit::List(inner) = &rest[0] {
          if let Some(Calcit::Syntax(CalcitSyntax::CoreLet, _)) = inner.first() {
            let inner_body: Vec<Calcit> = inner.drop_left().to_vec();
            return emit_let(ctx, &inner_body);
          }
        }
      }

      emit_body(ctx, rest)
    }
    _ => Err(format!("unsupported let binding form: {pair}")),
  }
}

/// Emit WASM for `match` expression (pattern matching on enum tuples).
///
/// Preprocessed form: [value_expr, (pattern body), (pattern body), ...]
/// Each pattern is either `_` (wildcard) or `(:tag binding1 binding2 ...)`.
/// The value must be a tuple — we read its tag_id at offset 0 and compare.
///
/// Compilation strategy: nested if/else chain comparing the tag_id.
///   evaluate value → store pointer in temp local
///   load tag_id from pointer
///   if tag == :variant1_id then { bind payloads; body1 }
///   else if tag == :variant2_id then { bind payloads; body2 }
///   else { wildcard_body or 0.0 }
fn emit_match(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.is_empty() {
    return Err("match requires a value and branches".into());
  }

  // Evaluate the value expression (a tuple) and store its f64 pointer
  emit_expr(ctx, &args[0])?;
  let ptr_f64 = ctx.alloc_local();
  ctx.emit(Instruction::LocalSet(ptr_f64));

  // Convert to i32 for memory access and load the tag_id (f64 at offset 8, after count)
  let tag_local = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(ptr_f64));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalSet(tag_local));

  // Collect branches: separate tag branches and wildcard
  let branches = &args[1..];
  let mut tag_branches: Vec<(&Calcit, &Calcit)> = Vec::new(); // (pattern, body)
  let mut wildcard_body: Option<&Calcit> = None;

  for branch in branches {
    let Calcit::List(pair) = branch else {
      return Err(format!("match branch expected a pair, got: {branch}"));
    };
    if pair.len() != 2 {
      return Err(format!("match branch expected 2 elements, got {}", pair.len()));
    }
    let pattern = &pair[0];
    let body = &pair[1];

    match pattern {
      // Wildcard
      Calcit::Symbol { sym, .. } | Calcit::Local(CalcitLocal { sym, .. }) if sym.as_ref() == "_" => {
        wildcard_body = Some(body);
      }
      // Tag pattern: (:tag binding1 binding2 ...)
      Calcit::List(_) => {
        tag_branches.push((pattern, body));
      }
      other => return Err(format!("unsupported match pattern: {other}")),
    }
  }

  // Generate nested if/else chain
  let num_tag_branches = tag_branches.len();
  if num_tag_branches == 0 {
    // Only wildcard
    if let Some(body) = wildcard_body {
      emit_expr(ctx, body)?;
    } else {
      ctx.emit(f64_const(0.0));
    }
    return Ok(());
  }

  // For each tag branch we emit:
  //   if (tag_local == variant_tag_id) then { bind payloads; body }
  //   else { next branch or wildcard }
  for (i, (pattern, body)) in tag_branches.iter().enumerate() {
    let Calcit::List(pat_xs) = pattern else {
      return Err(format!("match pattern expected list, got: {pattern}"));
    };
    let tag_str = match &pat_xs[0] {
      Calcit::Tag(t) => t.to_string(),
      other => return Err(format!("match pattern expected tag, got: {other}")),
    };
    let tag_id = *ctx
      .tag_index
      .get(&tag_str)
      .ok_or_else(|| format!("unknown tag in match pattern: {tag_str}"))?;

    // Compare: tag_local == tag_id
    ctx.emit(Instruction::LocalGet(tag_local));
    ctx.emit(f64_const(tag_id as f64));
    ctx.emit(Instruction::F64Eq);

    ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
    ctx.block_depth += 1;

    // Bind payload variables from tuple fields
    let binding_count = pat_xs.len() - 1;
    for bind_idx in 0..binding_count {
      let binding = &pat_xs[1 + bind_idx];
      let bind_name = match binding {
        Calcit::Local(CalcitLocal { sym, .. }) => sym.to_string(),
        Calcit::Symbol { sym, .. } => sym.to_string(),
        other => return Err(format!("match binding expected symbol, got: {other}")),
      };
      // Payload at offset (2 + bind_idx) * 8 from tuple pointer (skip count + tag)
      let offset = ((2 + bind_idx) * 8) as u64;
      ctx.emit(Instruction::LocalGet(ptr_f64));
      ctx.emit(Instruction::I32TruncF64U);
      ctx.emit(Instruction::F64Load(mem_arg_f64(offset)));
      let idx = ctx.declare_local(&bind_name);
      ctx.emit(Instruction::LocalSet(idx));
    }

    // Emit body
    emit_expr(ctx, body)?;

    ctx.emit(Instruction::Else);
    // If this is the last tag branch, emit wildcard or default
    if i == num_tag_branches - 1 {
      if let Some(wb) = wildcard_body {
        emit_expr(ctx, wb)?;
      } else {
        ctx.emit(f64_const(0.0));
      }
    }
    // Otherwise the next iteration will emit the next if/else inside this else block
  }

  // Close all the if/else blocks (one End per branch)
  for _ in 0..num_tag_branches {
    ctx.block_depth -= 1;
    ctx.emit(Instruction::End);
  }

  Ok(())
}

// ---------------------------------------------------------------------------
// Tag collection
// ---------------------------------------------------------------------------

/// Collect all Tag values from function bodies (multi-namespace format) and build tag→id map.
/// Tag IDs start at 1 (0 is unused/reserved).
/// Builtin type tags always registered in tag_index, so `type-of` can return them
/// and heap objects can carry them in their header slot.
const BUILTIN_TYPE_TAGS: &[&str] = &[
  "buf-list", "list", "map", "set", "tuple", "record", "number", "bool", "nil", "tag", "fn", "string", "symbol",
];

fn collect_all_tags_from(fn_defs: &[(String, String, CalcitFnArgs, Vec<Calcit>)]) -> HashMap<String, u32> {
  let mut tags: Vec<String> = Vec::new();
  // Always include builtin type tags — used by `type-of` and heap headers.
  for t in BUILTIN_TYPE_TAGS {
    tags.push((*t).to_string());
  }
  for (_, _, _, body) in fn_defs {
    for expr in body {
      collect_tags_from_expr(expr, &mut tags);
    }
  }
  tags.sort();
  tags.dedup();
  tags.into_iter().enumerate().map(|(i, t)| (t, (i + 1) as u32)).collect()
}

fn collect_tags_from_expr(expr: &Calcit, tags: &mut Vec<String>) {
  match expr {
    Calcit::Tag(t) => {
      tags.push(t.to_string());
    }
    Calcit::List(xs) => {
      for x in xs.iter() {
        collect_tags_from_expr(x, tags);
      }
    }
    Calcit::Struct(s) => {
      tags.push(s.name.to_string());
      for f in s.fields.iter() {
        tags.push(f.to_string());
      }
    }
    // When struct refs are imports, resolve them to collect their tags
    Calcit::Import(CalcitImport { ns, def, .. }) => {
      if let Ok(struct_def) = resolve_struct_ref(expr) {
        tags.push(struct_def.name.to_string());
        for f in struct_def.fields.iter() {
          tags.push(f.to_string());
        }
      }
      // Also try to collect tags from the expression in case it's used as a value
      let _ = (ns, def); // suppress unused warnings
    }
    _ => {}
  }
}

// ---------------------------------------------------------------------------
// Record operations
// ---------------------------------------------------------------------------

/// Build a string literal pool for WASM linear memory.
///
/// Scans all function bodies for `Calcit::Str` literals, deduplicates them,
/// and lays them out in memory starting at `HEAP_BASE`.
///
/// Each string is stored as:
///   `[magic:i32][type_tag("string"):i32][byte_len:f64][utf8_bytes... padded to 8]`
///
/// Returns:
///   - `string_pool`: maps string content → logical pointer (offset of byte_len field)
///   - `data_segment`: raw bytes for the WASM data section
///   - `heap_start`: the new heap start offset (after all string data)
fn build_string_pool(
  fn_defs: &[(String, String, CalcitFnArgs, Vec<Calcit>)],
  tag_index: &HashMap<String, u32>,
) -> (HashMap<String, u32>, Vec<u8>, i32) {
  let mut strings: Vec<String> = Vec::new();
  for (_, _, _, body) in fn_defs {
    for expr in body {
      collect_strings_from_expr(expr, &mut strings);
    }
  }
  strings.sort();
  strings.dedup();

  if strings.is_empty() {
    return (HashMap::new(), Vec::new(), HEAP_BASE);
  }

  let string_tag_id = *tag_index.get("string").expect("string type tag must exist") as i32;
  let mut pool: HashMap<String, u32> = HashMap::new();
  let mut data: Vec<u8> = Vec::new();
  let mut offset = HEAP_BASE as u32; // current write position in linear memory

  for s in &strings {
    let byte_len = s.len() as u32;
    // Write header: magic (i32) + type_tag (i32)
    data.extend_from_slice(&(HEAP_MAGIC as u32).to_le_bytes());
    data.extend_from_slice(&(string_tag_id as u32).to_le_bytes());
    // Logical pointer = offset + 8 (after header)
    let logical_ptr = offset + 8;
    pool.insert(s.clone(), logical_ptr);
    // Write byte_len as f64
    data.extend_from_slice(&(byte_len as f64).to_le_bytes());
    // Write UTF-8 bytes
    data.extend_from_slice(s.as_bytes());
    // Pad to 8-byte alignment
    let padded_len = (byte_len + 7) & !7;
    for _ in byte_len..padded_len {
      data.push(0);
    }
    // Advance offset: 8 (header) + 8 (byte_len f64) + padded_len
    offset += 8 + 8 + padded_len;
  }

  let heap_start = offset as i32;
  (pool, data, heap_start)
}

fn collect_record_field_tags_from_program(
  program_data: &program::CompiledProgram,
  tag_index: &HashMap<String, u32>,
) -> HashMap<u32, Vec<u32>> {
  let mut result = HashMap::new();

  for (_, file_info) in program_data {
    for (_, compiled) in &file_info.defs {
      let struct_def =
        try_parse_defrecord_form(&compiled.preprocessed_code).or_else(|| try_parse_defrecord_form(&compiled.codegen_form));
      let Some(struct_def) = struct_def else {
        continue;
      };

      let Some(struct_tag_id) = tag_index.get(struct_def.name.ref_str()) else {
        continue;
      };

      let field_tag_ids = struct_def
        .fields
        .iter()
        .filter_map(|field| tag_index.get(field.ref_str()).copied())
        .collect::<Vec<_>>();
      result.insert(*struct_tag_id, field_tag_ids);
    }
  }

  result
}

fn collect_strings_from_expr(expr: &Calcit, strings: &mut Vec<String>) {
  match expr {
    Calcit::Str(s) => {
      strings.push(s.to_string());
    }
    Calcit::List(xs) => {
      for x in xs.iter() {
        collect_strings_from_expr(x, strings);
      }
    }
    _ => {}
  }
}

// ---------------------------------------------------------------------------
// Record operations (continued)
// ---------------------------------------------------------------------------

/// Emit `&%{} struct_ref :field1 val1 :field2 val2 ...` — allocate a Record in linear memory.
///
/// The preprocessed form is: [NativeRecord, struct_ref, :tag1, val1, :tag2, val2, ...]
/// where struct_ref is either Calcit::Struct or Calcit::Import pointing to the struct def.
///
/// Memory layout: [count: f64] [struct_tag_id: f64] [field_0: f64] [field_1: f64] ...
/// All fields are in the order defined by CalcitStruct.fields (alphabetical).
/// Returns the pointer as f64.
fn emit_record_new(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.is_empty() {
    return Err("&%{} requires at least struct_ref argument".into());
  }
  // First arg is the struct definition — resolve it
  let struct_def = resolve_struct_ref(&args[0])?;

  let field_count = struct_def.fields.len();

  // Remaining args are interleaved: :tag1, val1, :tag2, val2, ...
  let field_args = &args[1..];
  if field_args.len() != field_count * 2 {
    return Err(format!(
      "&%{{}}: expected {} tag-value pairs ({} args), got {}",
      field_count,
      field_count * 2,
      field_args.len()
    ));
  }

  // Get struct tag ID
  let struct_tag_id = *ctx
    .tag_index
    .get(&struct_def.name.to_string())
    .ok_or_else(|| format!("unknown struct tag: {}", struct_def.name))?;

  // Layout: [count:f64][struct_tag:f64][field0:f64][field1:f64]...
  // Total bytes: (2 + field_count) * 8
  let total_size = ((2 + field_count) * 8) as i32;

  // Allocate: save i32 pointer to a temporary local
  let ptr_local = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, total_size, ptr_local, "record");

  // Store field count at offset 0
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(f64_const(field_count as f64));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  // Store struct tag at offset 8
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(f64_const(struct_tag_id as f64));
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));

  // Store each field value at offset (2 + i) * 8
  // field_args layout: [:tag0, val0, :tag1, val1, ...]
  for i in 0..field_count {
    let value_expr = &field_args[i * 2 + 1]; // skip the tag, take the value
    ctx.emit(Instruction::LocalGet(ptr_local));
    emit_expr(ctx, value_expr)?;
    ctx.emit(Instruction::F64Store(mem_arg_f64(((2 + i) * 8) as u64)));
  }

  // Return pointer as f64
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// Resolve a struct reference (either inline Calcit::Struct or Calcit::Import) to a CalcitStruct.
fn resolve_struct_ref(node: &Calcit) -> Result<CalcitStruct, String> {
  match node {
    Calcit::Struct(s) => Ok(s.clone()),
    Calcit::Import(CalcitImport { ns, def, .. }) => {
      // Try runtime first
      if let Some(Calcit::Struct(s)) = program::lookup_runtime_ready(ns, def) {
        return Ok(s);
      }
      // Try compiled def
      if let Some(compiled) = program::lookup_compiled_def(ns, def) {
        if let Calcit::Struct(s) = &compiled.codegen_form {
          return Ok(s.clone());
        }
        if let Calcit::Struct(s) = &compiled.preprocessed_code {
          return Ok(s.clone());
        }
        // Try to extract struct from defrecord form: (defrecord Name :field1 :field2 ...)
        if let Some(struct_def) = try_parse_defrecord_form(&compiled.codegen_form) {
          return Ok(struct_def);
        }
        if let Some(struct_def) = try_parse_defrecord_form(&compiled.preprocessed_code) {
          return Ok(struct_def);
        }
        return Err(format!("&%{{}}: compiled def {ns}/{def} is not a struct"));
      }
      // Try source code
      if let Some(source) = program::lookup_def_code(ns, def) {
        if let Some(struct_def) = try_parse_defrecord_form(&source) {
          return Ok(struct_def);
        }
      }
      Err(format!("&%{{}}: cannot resolve struct reference {ns}/{def}"))
    }
    other => Err(format!("&%{{}}: expected struct reference, got: {other}")),
  }
}

/// Try to extract a CalcitStruct from a `(defrecord Name :field1 :field2 ...)` form.
fn try_parse_defrecord_form(code: &Calcit) -> Option<CalcitStruct> {
  let Calcit::List(xs) = code else { return None };
  if xs.len() < 2 {
    return None;
  }
  // Check head is defrecord (Symbol)
  let is_defrecord = match &xs[0] {
    Calcit::Symbol { sym, .. } => sym.as_ref() == "defrecord" || sym.as_ref().ends_with("/defrecord"),
    _ => false,
  };
  if !is_defrecord {
    return None;
  }
  // Extract name
  let name = match &xs[1] {
    Calcit::Tag(t) => t.clone(),
    Calcit::Symbol { sym, .. } => {
      // ns/def format — extract just the def part
      let name_str = sym.as_ref().rsplit('/').next().unwrap_or(sym.as_ref());
      cirru_edn::EdnTag::from(name_str)
    }
    Calcit::Import(CalcitImport { def, .. }) => cirru_edn::EdnTag::from(def.as_ref()),
    _ => return None,
  };
  // Extract fields (remaining args that are Tags)
  let mut fields: Vec<cirru_edn::EdnTag> = Vec::new();
  for item in xs.iter().skip(2) {
    if let Calcit::Tag(t) = item {
      fields.push(t.clone());
    }
  }
  fields.sort();
  Some(CalcitStruct {
    name,
    fields: std::sync::Arc::new(fields),
    field_types: std::sync::Arc::new(vec![]),
    generics: std::sync::Arc::new(vec![]),
    impls: vec![],
  })
}

/// Emit `&record:nth record idx_literal tag_literal` — O(1) field access by index.
///
/// `idx` must be a compile-time Number constant.
fn emit_record_nth(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  // args: [record_expr, idx_expr, tag_expr]
  if args.len() < 2 {
    return Err("&record:nth requires at least 2 args (record, index)".into());
  }
  // Layout: [count:f64][struct_tag:f64][field0:f64]...
  // Field at byte offset (2 + idx) * 8 from the record pointer
  match &args[1] {
    Calcit::Number(n) => {
      // Static index — compile-time constant offset
      let idx = *n as usize;
      let offset = ((2 + idx) * 8) as u64;
      emit_expr(ctx, &args[0])?;
      ctx.emit(Instruction::I32TruncF64U);
      ctx.emit(Instruction::F64Load(mem_arg_f64(offset)));
    }
    _ => {
      // Dynamic index — compute offset at runtime: (2 + idx) * 8
      emit_expr(ctx, &args[0])?;
      ctx.emit(Instruction::I32TruncF64U);
      let ptr_local = ctx.alloc_local_typed(ValType::I32);
      ctx.emit(Instruction::LocalSet(ptr_local));
      // Compute byte offset: (2 + idx) * 8
      emit_expr(ctx, &args[1])?;
      ctx.emit(Instruction::I32TruncF64U);
      ctx.emit(Instruction::I32Const(2));
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::I32Const(8));
      ctx.emit(Instruction::I32Mul);
      // Add base pointer
      ctx.emit(Instruction::LocalGet(ptr_local));
      ctx.emit(Instruction::I32Add);
      // Load f64 at computed offset
      ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    }
  }
  Ok(())
}

/// Emit `&record:get record :field_tag` — field access by tag name.
///
/// Since fields are sorted alphabetically (matching CalcitStruct), we need the
/// struct type info to map tag to index. For now, this is only supported when
/// the tag is a compile-time constant and we can infer the struct type.
fn emit_record_get(_ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&record:get requires 2 args (record, tag)".into());
  }
  // For now, fall back to runtime error — we need struct type info to map field name to index.
  // This operation is typically rewritten to &record:nth during preprocessing.
  Err("&record:get not yet supported in WASM (use &record:nth via preprocessing optimization)".into())
}

/// Emit `&record:count record` — returns the number of fields.
/// Layout: [count:f64][struct_tag:f64][fields...]
/// Count is at offset 0 from the record pointer.
fn emit_record_count(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.is_empty() {
    return Err("&record:count requires 1 arg (record)".into());
  }
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  Ok(())
}

fn emit_record_field_tag(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&record:field-tag requires 2 args (record, index)".into());
  }

  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  let ptr_local = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalSet(ptr_local));

  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  let idx_local = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalSet(idx_local));

  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  let struct_tag_local = ctx.alloc_local();
  ctx.emit(Instruction::LocalSet(struct_tag_local));

  let mut struct_entries = ctx
    .record_field_tags
    .iter()
    .map(|(tag, fields)| (*tag, fields.clone()))
    .collect::<Vec<_>>();
  struct_entries.sort_by_key(|(tag, _)| *tag);

  if struct_entries.is_empty() {
    ctx.emit(f64_const(0.0));
    return Ok(());
  }

  for (struct_tag_id, field_tag_ids) in &struct_entries {
    ctx.emit(Instruction::LocalGet(struct_tag_local));
    ctx.emit(f64_const(*struct_tag_id as f64));
    ctx.emit(Instruction::F64Eq);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));

    if field_tag_ids.is_empty() {
      ctx.emit(f64_const(0.0));
    } else {
      for (field_idx, field_tag_id) in field_tag_ids.iter().enumerate() {
        ctx.emit(Instruction::LocalGet(idx_local));
        ctx.emit(Instruction::I32Const(field_idx as i32));
        ctx.emit(Instruction::I32Eq);
        ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
        ctx.emit(f64_const(*field_tag_id as f64));
        ctx.emit(Instruction::Else);
      }

      ctx.emit(f64_const(0.0));
      for _ in 0..field_tag_ids.len() {
        ctx.emit(Instruction::End);
      }
    }

    ctx.emit(Instruction::Else);
  }

  ctx.emit(f64_const(0.0));
  for _ in 0..struct_entries.len() {
    ctx.emit(Instruction::End);
  }
  Ok(())
}

/// Emit `&record:matches? a b` — check if two records have the same struct type.
///
/// Record layout: [count: f64] [struct_tag: f64] [field0: f64] ...
/// Compares the struct_tag (offset 0) of both records.
fn emit_record_matches(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&record:matches? expects 2 args".into());
  }
  // Load struct_tag of first record (at offset 8, after count)
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  // Load struct_tag of second record (at offset 8, after count)
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  // Compare and return 1.0 or 0.0
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  ctx.block_depth += 1;
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::Else);
  ctx.emit(f64_const(0.0));
  ctx.block_depth -= 1;
  ctx.emit(Instruction::End);
  Ok(())
}

// ---------------------------------------------------------------------------
// Tuple operations
// ---------------------------------------------------------------------------

/// Emit `:: tag val0 val1 ...` — allocate a Tuple in linear memory.
///
/// Memory layout: [count: f64] [tag_id: f64] [payload_0: f64] [payload_1: f64] ...
/// count = number of payloads (excludes the tag itself).
/// Returns the pointer as f64.
fn emit_tuple_new(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.is_empty() {
    return Err(":: requires at least a tag argument".into());
  }

  // First arg is the tag
  let tag_id = match &args[0] {
    Calcit::Tag(t) => {
      let tag_str = t.to_string();
      *ctx
        .tag_index
        .get(&tag_str)
        .ok_or_else(|| format!("unknown tag in tuple constructor: {tag_str}"))?
    }
    other => return Err(format!("::: expected tag as first arg, got: {other}")),
  };

  let payload = &args[1..];
  // Layout: count + tag + payloads
  let total_size = ((2 + payload.len()) * 8) as i32;

  let ptr_local = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, total_size, ptr_local, "tuple");

  // Store count at offset 0
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(f64_const(payload.len() as f64));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  // Store tag at offset 8
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(f64_const(tag_id as f64));
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));

  // Store payload fields starting at offset 16
  for (i, val) in payload.iter().enumerate() {
    ctx.emit(Instruction::LocalGet(ptr_local));
    emit_expr(ctx, val)?;
    ctx.emit(Instruction::F64Store(mem_arg_f64(((2 + i) * 8) as u64)));
  }

  // Return pointer as f64
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// Emit `&tuple:nth tuple idx` — O(1) payload access by index.
///
/// Tuple layout: [count:f64][tag:f64][payload0:f64]...
/// idx 0 returns tag, idx 1+ returns payloads.
/// Offset = (1 + idx) * 8  (skip count slot).
fn emit_tuple_nth(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&tuple:nth requires 2 args (tuple, index)".into());
  }
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let idx_local = ctx.alloc_local_typed(ValType::I32);
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(idx_local));

  ctx.emit(Instruction::LocalGet(idx_local));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  Ok(())
}

/// Emit `&tuple:count tuple` — payload count (excludes tag).
///
/// Tuple layout: [count:f64][tag:f64][payload0:f64]...
/// Count is at offset 0.
fn emit_tuple_count(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("&tuple:count expects 1 arg".into());
  }
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  Ok(())
}

fn emit_tuple_assoc(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 3 {
    return Err("&tuple:assoc expects 3 args".into());
  }
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);
  let idx = ctx.alloc_local_typed(ValType::I32);
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(idx));
  let val = ctx.alloc_local();
  emit_expr(ctx, &args[2])?;
  ctx.emit(Instruction::LocalSet(val));

  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));

  let size = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(total_slots));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::LocalSet(size));

  let dst = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc_dynamic(ctx, size, dst, "tuple");

  let dst_base = emit_addr_offset(ctx, dst, 0);
  let src_base = emit_addr_offset(ctx, src, 0);
  emit_copy_f64_loop(ctx, dst_base, src_base, total_slots);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(val));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

// ---------------------------------------------------------------------------
// Memory helpers
// ---------------------------------------------------------------------------

/// Emit inline bump-allocator: allocate `byte_size` bytes and store the i32
/// base pointer into `ptr_local`.
///
/// Look up the tag ID for a builtin type tag (e.g. "list", "map").
/// Panics if the tag is missing — builtin type tags are always pre-registered.
fn get_type_tag(ctx: &WasmGenCtx, name: &str) -> f64 {
  *ctx
    .tag_index
    .get(name)
    .unwrap_or_else(|| panic!("builtin type tag not registered: {name}")) as f64
}

fn emit_hash_mix(ctx: &mut WasmGenCtx) {
  ctx.emit(Instruction::I32Const(0x9e37_79b9u32 as i32));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Rotl);
}

fn emit_hash_expr_i32(ctx: &mut WasmGenCtx, expr: &Calcit) -> Result<(), String> {
  match expr {
    Calcit::Nil => {
      ctx.emit(Instruction::I32Const(0x1357_2468u32 as i32));
      Ok(())
    }
    Calcit::Bool(true) => {
      ctx.emit(Instruction::I32Const(0x4210_abceu32 as i32));
      Ok(())
    }
    Calcit::Bool(false) => {
      ctx.emit(Instruction::I32Const(0x24ce_1357u32 as i32));
      Ok(())
    }
    Calcit::Number(_) => {
      emit_expr(ctx, expr)?;
      ctx.emit(Instruction::I64ReinterpretF64);
      ctx.emit(Instruction::I64Const(32));
      ctx.emit(Instruction::I64ShrU);
      ctx.emit(Instruction::I32WrapI64);
      emit_hash_mix(ctx);
      Ok(())
    }
    Calcit::Tag(_) => {
      emit_expr(ctx, expr)?;
      ctx.emit(Instruction::I32TruncF64U);
      emit_hash_mix(ctx);
      Ok(())
    }
    Calcit::Str(_) | Calcit::Local(_) | Calcit::List(_) | Calcit::Import(_) | Calcit::Registered(_) => {
      emit_expr(ctx, expr)?;
      ctx.emit(Instruction::I32TruncF64U);
      emit_hash_mix(ctx);
      Ok(())
    }
    _ => Err(format!("unsupported WASM hash expression: {expr}")),
  }
}

fn emit_hash_proc(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("&hash expects 1 arg".into());
  }
  emit_hash_expr_i32(ctx, &args[0])?;
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// Bump-allocator emitter with heap type header.
///
/// Allocates `byte_size + 8` bytes: first 8 bytes store the type tag (f64),
/// the remaining `byte_size` bytes are the logical payload. Returns the LOGICAL
/// pointer (= raw_base + 8) in `ptr_local`, so all existing offset math works
/// unchanged. `type-of` can recover the type by loading at `(ptr - 8)`.
///
/// WAT sketch:
/// ```wasm
/// global.get $heap_ptr
/// f64.const <type_tag>
/// f64.store offset=0
/// global.get $heap_ptr
/// i32.const 8
/// i32.add
/// local.tee $ptr_local
/// i32.const <byte_size>
/// i32.add
/// global.set $heap_ptr
/// ```
fn emit_bump_alloc(ctx: &mut WasmGenCtx, byte_size: i32, ptr_local: u32, type_tag: &str) {
  let tag_val = get_type_tag(ctx, type_tag) as i32;
  // Write magic at raw_base+0.
  ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  ctx.emit(Instruction::I32Const(HEAP_MAGIC));
  ctx.emit(Instruction::I32Store(mem_arg_i32(0)));
  // Write tag id at raw_base+4.
  ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  ctx.emit(Instruction::I32Const(tag_val));
  ctx.emit(Instruction::I32Store(mem_arg_i32(4)));
  // Compute logical ptr = base + 8 and save.
  ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalTee(ptr_local));
  // Bump by byte_size, yielding new heap_ptr = old_base + 8 + byte_size.
  ctx.emit(Instruction::I32Const(byte_size));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
}

/// Bump-allocator with dynamic size (i32 local) and heap type header.
fn emit_bump_alloc_dynamic(ctx: &mut WasmGenCtx, size_local: u32, ptr_local: u32, type_tag: &str) {
  let tag_val = get_type_tag(ctx, type_tag) as i32;
  ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  ctx.emit(Instruction::I32Const(HEAP_MAGIC));
  ctx.emit(Instruction::I32Store(mem_arg_i32(0)));
  ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  ctx.emit(Instruction::I32Const(tag_val));
  ctx.emit(Instruction::I32Store(mem_arg_i32(4)));
  ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalTee(ptr_local));
  ctx.emit(Instruction::LocalGet(size_local));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
}

// ===========================================================================
// Shared data-structure helpers
// ===========================================================================

/// Evaluate expression → i32 pointer, saved in a new local.
fn emit_ptr_to_i32(ctx: &mut WasmGenCtx, expr: &Calcit) -> Result<u32, String> {
  let local = ctx.alloc_local_typed(ValType::I32);
  emit_expr(ctx, expr)?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(local));
  Ok(local)
}

/// Load the count (first f64 slot) from an i32 pointer, store as i32.
fn emit_load_count_i32(ctx: &mut WasmGenCtx, ptr_i32: u32) -> u32 {
  let count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_i32));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(count));
  count
}

/// Compute `base_local + byte_offset` and store in a new i32 local.
fn emit_addr_offset(ctx: &mut WasmGenCtx, base: u32, byte_offset: i32) -> u32 {
  let local = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(base));
  ctx.emit(Instruction::I32Const(byte_offset));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(local));
  local
}

/// Copy `n` f64 slots from `src_base` to `dst_base` (both i32 locals).
fn emit_copy_f64_loop(ctx: &mut WasmGenCtx, dst_base: u32, src_base: u32, n: u32) {
  let fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_copy_f64_slots")
    .expect("runtime helper __rt_copy_f64_slots must exist");
  ctx.emit(Instruction::LocalGet(dst_base));
  ctx.emit(Instruction::LocalGet(src_base));
  ctx.emit(Instruction::LocalGet(n));
  ctx.emit(Instruction::Call(fn_idx));
}

fn emit_runtime_lookup_i32_f64_to_i32(ctx: &mut WasmGenCtx, helper: &str, ptr_local: u32, target_local: u32) -> u32 {
  let result = ctx.alloc_local_typed(ValType::I32);
  let fn_idx = *ctx
    .runtime_fn_index
    .get(helper)
    .unwrap_or_else(|| panic!("runtime helper missing: {helper}"));
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::LocalGet(target_local));
  ctx.emit(Instruction::Call(fn_idx));
  ctx.emit(Instruction::LocalSet(result));
  result
}

fn emit_runtime_lookup_i32_to_i32(ctx: &mut WasmGenCtx, helper: &str, ptr_local: u32) -> u32 {
  let result = ctx.alloc_local_typed(ValType::I32);
  let fn_idx = *ctx
    .runtime_fn_index
    .get(helper)
    .unwrap_or_else(|| panic!("runtime helper missing: {helper}"));
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::Call(fn_idx));
  ctx.emit(Instruction::LocalSet(result));
  result
}

/// Generic count accessor — works for list, map, set (count is always the first f64).
fn emit_ds_count(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("count expects 1 arg".into());
  }
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  Ok(())
}

/// Generic empty? check — `count == 0`.
fn emit_ds_empty(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("empty? expects 1 arg".into());
  }
  ctx.emit(f64_const(1.0));
  ctx.emit(f64_const(0.0));
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::Select);
  Ok(())
}

/// Allocate a new data-structure with a count header and return its i32 pointer.
/// `count_i32` is the count local; `slot_count_expr` computes the total f64 slots
/// (including the count header). Returns the allocated i32 pointer local.
fn emit_alloc_with_count(ctx: &mut WasmGenCtx, count_i32: u32, total_slots_i32: u32, type_tag: &str) -> u32 {
  let size = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(total_slots_i32));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::LocalSet(size));

  let ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc_dynamic(ctx, size, ptr, type_tag);

  // Store count
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::LocalGet(count_i32));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ptr
}

fn emit_alloc_map_with_root(ctx: &mut WasmGenCtx, count_i32: u32, root_i32: u32) -> u32 {
  let ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, 16, ptr, "map");

  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::LocalGet(count_i32));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::LocalGet(root_i32));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));

  ptr
}

// ===========================================================================
// List operations — layout: [count:f64] [elem0:f64] [elem1:f64] ...
// ===========================================================================

/// `[] elem0 elem1 ...` — create a list with static arity.
fn emit_list_new(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  let count = args.len();
  let total_bytes = ((1 + count) * 8) as i32;
  let ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, total_bytes, ptr, "list");

  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(f64_const(count as f64));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  for (i, arg) in args.iter().enumerate() {
    ctx.emit(Instruction::LocalGet(ptr));
    emit_expr(ctx, arg)?;
    ctx.emit(Instruction::F64Store(mem_arg_f64(((1 + i) * 8) as u64)));
  }

  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&list:nth list idx` — element at dynamic index.
fn emit_list_nth(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&list:nth expects 2 args".into());
  }
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  // offset = (1 + idx) * 8
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  Ok(())
}

/// `&list:first list` — first element.
fn emit_list_first(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("&list:first expects 1 arg".into());
  }
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  Ok(())
}

/// `&list:rest list` — new list without the first element.
fn emit_list_rest(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("&list:rest expects 1 arg".into());
  }
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let old_count = emit_load_count_i32(ctx, src);

  let new_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(old_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(new_count));

  // total_slots = 1 + new_count
  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(new_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));

  let dst = emit_alloc_with_count(ctx, new_count, total_slots, "list");

  // Copy elements: dst[8..] ← src[16..]
  let dst_base = emit_addr_offset(ctx, dst, 8);
  let src_base = emit_addr_offset(ctx, src, 16);
  emit_copy_f64_loop(ctx, dst_base, src_base, new_count);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `append list elem` — new list with element added at end.
fn emit_list_append(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("append expects 2 args".into());
  }
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let old_count = emit_load_count_i32(ctx, src);
  // Evaluate element into a local BEFORE allocation
  let elem = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(elem));

  let new_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(old_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(new_count));

  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(new_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));

  let dst = emit_alloc_with_count(ctx, new_count, total_slots, "list");

  // Copy old elements: dst[8..] ← src[8..]
  let dst_base = emit_addr_offset(ctx, dst, 8);
  let src_base = emit_addr_offset(ctx, src, 8);
  emit_copy_f64_loop(ctx, dst_base, src_base, old_count);

  // Store new element at dst[8 + old_count * 8]
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(old_count));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(elem));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `prepend list elem` — new list with element at front.
fn emit_list_prepend(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("prepend expects 2 args".into());
  }
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let old_count = emit_load_count_i32(ctx, src);
  let elem = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(elem));

  let new_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(old_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(new_count));

  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(new_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));

  let dst = emit_alloc_with_count(ctx, new_count, total_slots, "list");

  // Store element at dst[8]
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(elem));
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));

  // Copy old elements: dst[16..] ← src[8..]
  let dst_base = emit_addr_offset(ctx, dst, 16);
  let src_base = emit_addr_offset(ctx, src, 8);
  emit_copy_f64_loop(ctx, dst_base, src_base, old_count);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `butlast list` — new list without the last element.
fn emit_list_butlast(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("butlast expects 1 arg".into());
  }
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let old_count = emit_load_count_i32(ctx, src);

  let new_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(old_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(new_count));

  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(new_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));

  let dst = emit_alloc_with_count(ctx, new_count, total_slots, "list");

  let dst_base = emit_addr_offset(ctx, dst, 8);
  let src_base = emit_addr_offset(ctx, src, 8);
  emit_copy_f64_loop(ctx, dst_base, src_base, new_count);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&list:slice list start` or `&list:slice list start end`.
fn emit_list_slice(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() < 2 || args.len() > 3 {
    return Err("&list:slice expects 2-3 args".into());
  }
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);

  let start = ctx.alloc_local_typed(ValType::I32);
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(start));

  let end = ctx.alloc_local_typed(ValType::I32);
  if args.len() == 3 {
    emit_expr(ctx, &args[2])?;
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(end));
  } else {
    ctx.emit(Instruction::LocalGet(count));
    ctx.emit(Instruction::LocalSet(end));
  }

  let new_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(end));
  ctx.emit(Instruction::LocalGet(start));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(new_count));

  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(new_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));

  let dst = emit_alloc_with_count(ctx, new_count, total_slots, "list");

  // src_base = src + 8 + start*8
  let src_base = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(start));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(src_base));

  let dst_base = emit_addr_offset(ctx, dst, 8);
  emit_copy_f64_loop(ctx, dst_base, src_base, new_count);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&list:reverse list` — new list in reverse order.
fn emit_list_reverse(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("&list:reverse expects 1 arg".into());
  }
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);

  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));

  let dst = emit_alloc_with_count(ctx, count, total_slots, "list");

  // Loop: dst[8 + i*8] = src[8 + (count-1-i)*8]
  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(i));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));

  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  // dst addr = dst + 8 + i*8
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);

  // src addr = src + 8 + (count-1-i)*8
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));

  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(i));
  ctx.emit(Instruction::Br(0));

  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&list:concat a b` — concatenate two lists.
fn emit_list_concat(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&list:concat expects 2 args".into());
  }
  let src_a = emit_ptr_to_i32(ctx, &args[0])?;
  let count_a = emit_load_count_i32(ctx, src_a);
  let src_b = emit_ptr_to_i32(ctx, &args[1])?;
  let count_b = emit_load_count_i32(ctx, src_b);

  let new_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count_a));
  ctx.emit(Instruction::LocalGet(count_b));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(new_count));

  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(new_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));

  let dst = emit_alloc_with_count(ctx, new_count, total_slots, "list");

  // Copy a: dst[8..] ← src_a[8..]
  let dst_base_a = emit_addr_offset(ctx, dst, 8);
  let src_base_a = emit_addr_offset(ctx, src_a, 8);
  emit_copy_f64_loop(ctx, dst_base_a, src_base_a, count_a);

  // Copy b: dst[8 + count_a*8 ..] ← src_b[8..]
  let dst_base_b = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(count_a));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(dst_base_b));
  let src_base_b = emit_addr_offset(ctx, src_b, 8);
  emit_copy_f64_loop(ctx, dst_base_b, src_base_b, count_b);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&list:assoc list idx value` — new list with element replaced at index.
fn emit_list_assoc(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 3 {
    return Err("&list:assoc expects 3 args".into());
  }
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);
  let idx = ctx.alloc_local_typed(ValType::I32);
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(idx));
  let val = ctx.alloc_local();
  emit_expr(ctx, &args[2])?;
  ctx.emit(Instruction::LocalSet(val));

  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));

  let dst = emit_alloc_with_count(ctx, count, total_slots, "list");

  // Copy all elements
  let dst_base = emit_addr_offset(ctx, dst, 8);
  let src_base = emit_addr_offset(ctx, src, 8);
  emit_copy_f64_loop(ctx, dst_base, src_base, count);

  // Overwrite at idx: dst[8 + idx*8]
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(val));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&list:dissoc list idx` — new list without element at index.
fn emit_list_dissoc(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&list:dissoc expects 2 args".into());
  }
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);
  let idx = ctx.alloc_local_typed(ValType::I32);
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(idx));

  let new_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(new_count));

  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(new_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));

  let dst = emit_alloc_with_count(ctx, new_count, total_slots, "list");

  // Copy [0..idx): src_base = src+8, dst_base = dst+8, n = idx
  let before_n = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::LocalSet(before_n));
  let dst_b1 = emit_addr_offset(ctx, dst, 8);
  let src_b1 = emit_addr_offset(ctx, src, 8);
  emit_copy_f64_loop(ctx, dst_b1, src_b1, before_n);

  // Copy [idx+1..count): n = count - idx - 1
  let after_n = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(after_n));

  // dst_base2 = dst + 8 + idx*8
  let dst_b2 = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(dst_b2));

  // src_base2 = src + 8 + (idx+1)*8
  let src_b2 = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(idx));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(src_b2));

  emit_copy_f64_loop(ctx, dst_b2, src_b2, after_n);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `list? x` — true (1.0) when x is a list value.
/// Implemented as: (type-of x) == list-tag
fn emit_list_q(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err(format!("list? expects 1 arg, got {}", args.len()));
  }
  let list_tag = get_type_tag(ctx, "list");
  ctx.emit(f64_const(1.0));
  ctx.emit(f64_const(0.0));
  emit_type_of(ctx, args)?;
  ctx.emit(f64_const(list_tag));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::Select);
  Ok(())
}

/// `&list:contains? list idx` — true if 0 ≤ idx < count.
fn emit_list_contains(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&list:contains? expects 2 args".into());
  }
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, ptr);
  ctx.emit(f64_const(1.0));
  ctx.emit(f64_const(0.0));
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32LtU);
  ctx.emit(Instruction::Select);
  Ok(())
}

/// `&list:includes? list value` — linear scan for matching f64 value.
fn emit_list_includes(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&list:includes? expects 2 args".into());
  }
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, ptr);
  let target = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(target));

  let result = ctx.alloc_local();
  ctx.emit(f64_const(0.0)); // default: false
  ctx.emit(Instruction::LocalSet(result));

  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(i));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));

  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  // Load elem at ptr + 8 + i*8
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(target));
  ctx.emit(Instruction::F64Eq);

  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::Br(2)); // break outer block
  ctx.emit(Instruction::End); // end if

  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(i));
  ctx.emit(Instruction::Br(0));

  ctx.emit(Instruction::End); // end loop
  ctx.emit(Instruction::End); // end block

  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

// ===========================================================================
// BufList operations — layout: [capacity:f64] [count:f64] [elem0:f64] ...
// Mutable append-only list. `push` and `concat` mutate in-place.
// ===========================================================================

const BUF_LIST_INITIAL_CAPACITY: i32 = 8;

/// `(&buf-list:new)` — create empty BufList with initial capacity
fn emit_buf_list_new(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if !args.is_empty() {
    return Err("&buf-list:new expects 0 args".into());
  }
  // Allocate: 2 header slots (capacity, count) + initial_capacity data slots
  let total_slots = 2 + BUF_LIST_INITIAL_CAPACITY;
  let byte_size = total_slots * 8;
  let ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, byte_size, ptr, "buf-list");
  // Store capacity
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(f64_const(BUF_LIST_INITIAL_CAPACITY as f64));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  // Store count = 0
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
  // Return as f64
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `(&buf-list:push buf item)` — mutates buf, returns buf.
/// If count == capacity, grow to 2x capacity.
fn emit_buf_list_push(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&buf-list:push expects 2 args".into());
  }
  let buf_ptr = emit_ptr_to_i32(ctx, &args[0])?;

  // Load count
  let count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(count));

  // Load capacity
  let capacity = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(capacity));

  // if count >= capacity, grow
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::LocalGet(capacity));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  {
    // New capacity = old * 2
    let new_cap = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(capacity));
    ctx.emit(Instruction::I32Const(2));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::LocalSet(new_cap));

    // Allocate new buffer: (2 + new_cap) * 8
    let new_size = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(new_cap));
    ctx.emit(Instruction::I32Const(2));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::LocalSet(new_size));

    let new_ptr = ctx.alloc_local_typed(ValType::I32);
    emit_bump_alloc_dynamic(ctx, new_size, new_ptr, "buf-list");

    // Store new capacity
    ctx.emit(Instruction::LocalGet(new_ptr));
    ctx.emit(Instruction::LocalGet(new_cap));
    ctx.emit(Instruction::F64ConvertI32U);
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

    // Store count (unchanged)
    ctx.emit(Instruction::LocalGet(new_ptr));
    ctx.emit(Instruction::LocalGet(count));
    ctx.emit(Instruction::F64ConvertI32U);
    ctx.emit(Instruction::F64Store(mem_arg_f64(8)));

    // Copy old data: new_ptr+16 ← buf_ptr+16, count elements
    let dst_base = emit_addr_offset(ctx, new_ptr, 16);
    let src_base = emit_addr_offset(ctx, buf_ptr, 16);
    emit_copy_f64_loop(ctx, dst_base, src_base, count);

    // Update buf_ptr to new_ptr
    ctx.emit(Instruction::LocalGet(new_ptr));
    ctx.emit(Instruction::LocalSet(buf_ptr));
  }
  ctx.emit(Instruction::End); // end if

  // Store the new element at buf_ptr + 16 + count * 8
  let elem_addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(elem_addr));

  ctx.emit(Instruction::LocalGet(elem_addr));
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  // Increment count
  let new_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(new_count));
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.emit(Instruction::LocalGet(new_count));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));

  // Return buf_ptr as f64
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `(&buf-list:concat buf list)` — append all list elements to buf
fn emit_buf_list_concat(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&buf-list:concat expects 2 args".into());
  }
  let buf_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let list_ptr = emit_ptr_to_i32(ctx, &args[1])?;

  let list_count = emit_load_count_i32(ctx, list_ptr);

  // Loop: for i in 0..list_count, push list[i] to buf
  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(i));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty)); // break target
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty)); // continue target

  // if i >= list_count, break
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::LocalGet(list_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1)); // break

  // Load buf's count and capacity
  let b_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(b_count));

  let b_cap = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(b_cap));

  // if count >= capacity, grow
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::LocalGet(b_cap));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  {
    let new_cap = ctx.alloc_local_typed(ValType::I32);
    // new_cap = max(cap * 2, count + list_count)
    ctx.emit(Instruction::LocalGet(b_cap));
    ctx.emit(Instruction::I32Const(2));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::LocalSet(new_cap));

    // Ensure new_cap >= b_count + list_count
    let needed = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(b_count));
    ctx.emit(Instruction::LocalGet(list_count));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(needed));

    ctx.emit(Instruction::LocalGet(new_cap));
    ctx.emit(Instruction::LocalGet(needed));
    ctx.emit(Instruction::I32LtU);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
    ctx.emit(Instruction::LocalGet(needed));
    ctx.emit(Instruction::LocalSet(new_cap));
    ctx.emit(Instruction::End);

    let new_size = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(new_cap));
    ctx.emit(Instruction::I32Const(2));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::LocalSet(new_size));

    let new_ptr = ctx.alloc_local_typed(ValType::I32);
    emit_bump_alloc_dynamic(ctx, new_size, new_ptr, "buf-list");

    ctx.emit(Instruction::LocalGet(new_ptr));
    ctx.emit(Instruction::LocalGet(new_cap));
    ctx.emit(Instruction::F64ConvertI32U);
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

    ctx.emit(Instruction::LocalGet(new_ptr));
    ctx.emit(Instruction::LocalGet(b_count));
    ctx.emit(Instruction::F64ConvertI32U);
    ctx.emit(Instruction::F64Store(mem_arg_f64(8)));

    let dst_base = emit_addr_offset(ctx, new_ptr, 16);
    let src_base = emit_addr_offset(ctx, buf_ptr, 16);
    emit_copy_f64_loop(ctx, dst_base, src_base, b_count);

    ctx.emit(Instruction::LocalGet(new_ptr));
    ctx.emit(Instruction::LocalSet(buf_ptr));
  }
  ctx.emit(Instruction::End); // end if (grow)

  // Reload count after possible grow
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(b_count));

  // Store list[i] at buf_ptr + 16 + b_count * 8
  let elem_addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(elem_addr));

  // Load list[i]: list_ptr + 8 + i * 8
  let list_elem_addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(list_ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(list_elem_addr));

  ctx.emit(Instruction::LocalGet(elem_addr));
  ctx.emit(Instruction::LocalGet(list_elem_addr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  // Increment buf count
  let new_b_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(new_b_count));
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.emit(Instruction::LocalGet(new_b_count));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));

  // i++
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(i));

  ctx.emit(Instruction::Br(0)); // continue loop
  ctx.emit(Instruction::End); // end loop
  ctx.emit(Instruction::End); // end block

  // Return buf_ptr
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `(&buf-list:to-list buf)` — freeze buf into an immutable list
fn emit_buf_list_to_list(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("&buf-list:to-list expects 1 arg".into());
  }
  let buf_ptr = emit_ptr_to_i32(ctx, &args[0])?;

  // Load count
  let count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(count));

  // Allocate a new list: (1 + count) slots
  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));

  let dst = emit_alloc_with_count(ctx, count, total_slots, "list");

  // Copy data: dst+8 ← buf_ptr+16, count elements
  let dst_base = emit_addr_offset(ctx, dst, 8);
  let src_base = emit_addr_offset(ctx, buf_ptr, 16);
  emit_copy_f64_loop(ctx, dst_base, src_base, count);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `(&buf-list:count buf)` — return count as f64
fn emit_buf_list_count(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("&buf-list:count expects 1 arg".into());
  }
  let buf_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  ctx.emit(Instruction::LocalGet(buf_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8))); // count is at offset 8
  Ok(())
}

// ===========================================================================
// Map operations — outer layout: [count:f64] [root_ptr:f64]
// Root layout: [count:f64] [k0:f64] [v0:f64] [k1:f64] [v1:f64] ...
// ===========================================================================

/// `&{} key0 val0 key1 val1 ...` — create a map (key-value pairs).
fn emit_map_new(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() % 2 != 0 {
    return Err("&{} expects even number of args (key-value pairs)".into());
  }
  let count = args.len() / 2;
  let count_local = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(count as i32));
  ctx.emit(Instruction::LocalSet(count_local));

  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count_local));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));
  let root = emit_alloc_with_count(ctx, count_local, total_slots, "map");

  for i in 0..count {
    // key at offset 8 + i*16
    ctx.emit(Instruction::LocalGet(root));
    emit_expr(ctx, &args[i * 2])?;
    ctx.emit(Instruction::F64Store(mem_arg_f64((8 + i * 16) as u64)));
    // value at offset 16 + i*16
    ctx.emit(Instruction::LocalGet(root));
    emit_expr(ctx, &args[i * 2 + 1])?;
    ctx.emit(Instruction::F64Store(mem_arg_f64((16 + i * 16) as u64)));
  }

  let fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_map_root_from_flat")
    .expect("runtime helper __rt_map_root_from_flat must exist");
  ctx.emit(Instruction::LocalGet(root));
  ctx.emit(Instruction::Call(fn_idx));
  let hashed_root = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalSet(hashed_root));
  let ptr = emit_alloc_map_with_root(ctx, count_local, hashed_root);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&map:get map key` — linear scan for key; returns value or nil.
fn emit_map_get_op(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&map:get expects 2 args".into());
  }
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let target = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(target));
  let fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_map_get_value")
    .expect("runtime helper __rt_map_get_value must exist");
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::LocalGet(target));
  ctx.emit(Instruction::Call(fn_idx));
  Ok(())
}

/// `&map:contains? map key` — scan for key, return bool.
fn emit_map_contains(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&map:contains? expects 2 args".into());
  }
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let target = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(target));
  let found_idx = emit_runtime_lookup_i32_f64_to_i32(ctx, "__rt_map_contains_key", ptr, target);
  ctx.emit(f64_const(1.0));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalGet(found_idx));
  ctx.emit(Instruction::Select);
  Ok(())
}

/// `&map:includes? map value` — scan values for match.
fn emit_map_includes(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&map:includes? expects 2 args".into());
  }
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let target = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(target));
  let found_idx = emit_runtime_lookup_i32_f64_to_i32(ctx, "__rt_map_contains_value", ptr, target);
  ctx.emit(f64_const(1.0));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalGet(found_idx));
  ctx.emit(Instruction::Select);
  Ok(())
}

/// `&map:assoc map key value` — new map with key-value added or updated.
fn emit_map_assoc(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 3 {
    return Err("&map:assoc expects 3 args".into());
  }
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let key = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(key));
  let val = ctx.alloc_local();
  emit_expr(ctx, &args[2])?;
  ctx.emit(Instruction::LocalSet(val));
  let fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_map_assoc")
    .expect("runtime helper __rt_map_assoc must exist");
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::LocalGet(key));
  ctx.emit(Instruction::LocalGet(val));
  ctx.emit(Instruction::Call(fn_idx));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&map:dissoc map key` — new map without key (or same if key absent).
fn emit_map_dissoc(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&map:dissoc expects 2 args".into());
  }
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let target = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(target));
  let fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_map_dissoc")
    .expect("runtime helper __rt_map_dissoc must exist");
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::LocalGet(target));
  ctx.emit(Instruction::Call(fn_idx));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `to-pairs map` — convert map to list of 2-element lists.
fn emit_map_to_pairs(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("to-pairs expects 1 arg".into());
  }
  emit_map_to_pair_list(ctx, args, "set")
}

/// `&map:to-list map` — convert map to list of `[key, value]` pairs.
fn emit_map_to_list(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("&map:to-list expects 1 arg".into());
  }
  emit_map_to_pair_list(ctx, args, "list")
}

/// Shared implementation: convert a map to a list/set of `[key, value]` pairs.
fn emit_map_to_pair_list(ctx: &mut WasmGenCtx, args: &[Calcit], outer_tag: &str) -> Result<(), String> {
  let map_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, map_ptr);
  let flat_ptr = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", map_ptr);

  // Outer list: [count, pair_ptr_0, pair_ptr_1, ...]
  let outer_ts = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(outer_ts));
  let outer = emit_alloc_with_count(ctx, count, outer_ts, outer_tag);

  // Loop: for each entry, create a 2-element list [key, value]
  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(i));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));

  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  // Allocate pair: [2, key, value] = 3 f64 slots = 24 bytes
  let pair = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, 24, pair, "list");
  ctx.emit(Instruction::LocalGet(pair));
  ctx.emit(f64_const(2.0));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  // Load key from map: map_ptr + 8 + i*16
  ctx.emit(Instruction::LocalGet(pair));
  ctx.emit(Instruction::LocalGet(flat_ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));

  // Load value from map: map_ptr + 16 + i*16
  ctx.emit(Instruction::LocalGet(pair));
  ctx.emit(Instruction::LocalGet(flat_ptr));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::F64Store(mem_arg_f64(16)));

  // Store pair ptr in outer list: outer[8 + i*8]
  ctx.emit(Instruction::LocalGet(outer));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(pair));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(i));
  ctx.emit(Instruction::Br(0));

  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(outer));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

// ===========================================================================
// Set operations — layout: [count:f64] [elem0:f64] [elem1:f64] ...
// ===========================================================================

/// `#{} elem0 elem1 ...` — create a set.
fn emit_set_new(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  // Same layout as list; deduplicate is the caller's responsibility
  let count = args.len();
  let total_bytes = ((1 + count) * 8) as i32;
  let ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, total_bytes, ptr, "set");

  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(f64_const(count as f64));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  for (i, arg) in args.iter().enumerate() {
    ctx.emit(Instruction::LocalGet(ptr));
    emit_expr(ctx, arg)?;
    ctx.emit(Instruction::F64Store(mem_arg_f64(((1 + i) * 8) as u64)));
  }

  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&set:includes? set value` — linear scan.
fn emit_set_includes(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&set:includes? expects 2 args".into());
  }
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let target = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(target));
  let found_idx = emit_runtime_lookup_i32_f64_to_i32(ctx, "__rt_set_find_elem", ptr, target);
  ctx.emit(f64_const(1.0));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalGet(found_idx));
  ctx.emit(Instruction::I32Const(-1));
  ctx.emit(Instruction::I32Ne);
  ctx.emit(Instruction::Select);
  Ok(())
}

/// `&include set value` — new set with element added (if not present).
fn emit_set_include(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&include expects 2 args".into());
  }
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);
  let elem = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(elem));

  let found_idx = emit_runtime_lookup_i32_f64_to_i32(ctx, "__rt_set_find_elem", src, elem);

  let dst = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::LocalSet(dst)); // default: return same

  ctx.emit(Instruction::LocalGet(found_idx));
  ctx.emit(Instruction::I32Const(-1));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  {
    // Not found: append like list
    let nc = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(count));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(nc));
    let ts = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(nc));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(ts));
    let d = emit_alloc_with_count(ctx, nc, ts, "set");
    ctx.emit(Instruction::LocalGet(d));
    ctx.emit(Instruction::LocalSet(dst));

    let db = emit_addr_offset(ctx, d, 8);
    let sb = emit_addr_offset(ctx, src, 8);
    emit_copy_f64_loop(ctx, db, sb, count);

    // Append element
    ctx.emit(Instruction::LocalGet(d));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(count));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(elem));
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  }
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&exclude set value` — new set without the element.
fn emit_set_exclude(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&exclude expects 2 args".into());
  }
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);
  let target = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(target));

  let found_idx = emit_runtime_lookup_i32_f64_to_i32(ctx, "__rt_set_find_elem", src, target);

  // If not found, return original
  let dst = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::LocalSet(dst));

  ctx.emit(Instruction::LocalGet(found_idx));
  ctx.emit(Instruction::I32Const(-1));
  ctx.emit(Instruction::I32Ne);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  {
    let nc = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(count));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Sub);
    ctx.emit(Instruction::LocalSet(nc));
    let ts = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(nc));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(ts));
    let d = emit_alloc_with_count(ctx, nc, ts, "set");
    ctx.emit(Instruction::LocalGet(d));
    ctx.emit(Instruction::LocalSet(dst));

    // Copy before found_idx
    let before_n = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(found_idx));
    ctx.emit(Instruction::LocalSet(before_n));
    let db1 = emit_addr_offset(ctx, d, 8);
    let sb1 = emit_addr_offset(ctx, src, 8);
    emit_copy_f64_loop(ctx, db1, sb1, before_n);

    // Copy after found_idx
    let after_n = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(count));
    ctx.emit(Instruction::LocalGet(found_idx));
    ctx.emit(Instruction::I32Sub);
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Sub);
    ctx.emit(Instruction::LocalSet(after_n));

    let db2 = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(d));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(found_idx));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(db2));

    let sb2 = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(src));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(found_idx));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(sb2));

    emit_copy_f64_loop(ctx, db2, sb2, after_n);
  }
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&difference a b` — set of elements in `a` not in `b`.
fn emit_set_difference(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&difference expects 2 args".into());
  }
  let a = emit_ptr_to_i32(ctx, &args[0])?;
  let a_count = emit_load_count_i32(ctx, a);
  let b = emit_ptr_to_i32(ctx, &args[1])?;

  // Over-allocate: max possible result is a_count elements
  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));
  let dst = emit_alloc_with_count(ctx, a_count, total_slots, "set");

  let write_idx = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(write_idx));

  let ai = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(ai));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  // Load a[ai]
  let elem = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(a));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(elem));

  let found_idx = emit_runtime_lookup_i32_f64_to_i32(ctx, "__rt_set_find_elem", b, elem);

  // If not found in b, copy elem to result
  ctx.emit(Instruction::LocalGet(found_idx));
  ctx.emit(Instruction::I32Const(-1));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(elem));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(write_idx));
  ctx.emit(Instruction::End);

  // ai++
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(ai));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // Patch actual count
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&union a b` — set of all elements in `a` and `b` (no duplicates).
fn emit_set_union(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&union expects 2 args".into());
  }
  let a = emit_ptr_to_i32(ctx, &args[0])?;
  let a_count = emit_load_count_i32(ctx, a);
  let b = emit_ptr_to_i32(ctx, &args[1])?;
  let b_count = emit_load_count_i32(ctx, b);

  // Over-allocate: max possible is a_count + b_count
  let max_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(max_count));

  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(max_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));
  let dst = emit_alloc_with_count(ctx, max_count, total_slots, "set");

  // Copy all of a into result
  let db = emit_addr_offset(ctx, dst, 8);
  let sb = emit_addr_offset(ctx, a, 8);
  emit_copy_f64_loop(ctx, db, sb, a_count);

  let write_idx = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::LocalSet(write_idx));

  let bi = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(bi));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  // Load b[bi]
  let elem = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(b));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(elem));

  let found_idx = emit_runtime_lookup_i32_f64_to_i32(ctx, "__rt_set_find_elem", a, elem);

  // If not found in a, append to result
  ctx.emit(Instruction::LocalGet(found_idx));
  ctx.emit(Instruction::I32Const(-1));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(elem));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(write_idx));
  ctx.emit(Instruction::End);

  // bi++
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(bi));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // Patch actual count
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&merge a b` — merge two maps; keys in `b` override keys in `a`.
fn emit_map_merge(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&merge expects 2 args".into());
  }
  let a = emit_ptr_to_i32(ctx, &args[0])?;
  let a_count = emit_load_count_i32(ctx, a);
  let a_flat = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", a);
  let b = emit_ptr_to_i32(ctx, &args[1])?;
  let b_count = emit_load_count_i32(ctx, b);
  let b_flat = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", b);

  // Over-allocate: max count is a_count + b_count
  let max_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(max_count));

  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(max_count));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));
  let dst_root = emit_alloc_with_count(ctx, max_count, total_slots, "map");

  // Copy all of a's kv pairs into result
  let copy_n = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::LocalSet(copy_n));
  let db = emit_addr_offset(ctx, dst_root, 8);
  let sb = emit_addr_offset(ctx, a_flat, 8);
  emit_copy_f64_loop(ctx, db, sb, copy_n);

  // write_count tracks actual number of entries
  let write_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::LocalSet(write_count));

  // For each entry in b, find matching key in dst or append
  let bi = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(bi));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  // Load b key and value
  let bk = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(b_flat));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  let bkv_addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalTee(bkv_addr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(bk));
  let bv = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(bkv_addr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalSet(bv));

  // Scan dst for matching key
  let found_idx = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(-1));
  ctx.emit(Instruction::LocalSet(found_idx));
  let di = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(di));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(di));
  ctx.emit(Instruction::LocalGet(write_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(dst_root));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(di));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(bk));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(di));
  ctx.emit(Instruction::LocalSet(found_idx));
  ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(di));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(di));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(found_idx));
  ctx.emit(Instruction::I32Const(-1));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  {
    // Not found: append at write_count position
    let addr = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(dst_root));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(write_count));
    ctx.emit(Instruction::I32Const(16));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(addr));
    ctx.emit(Instruction::LocalGet(addr));
    ctx.emit(Instruction::LocalGet(bk));
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
    ctx.emit(Instruction::LocalGet(addr));
    ctx.emit(Instruction::LocalGet(bv));
    ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
    ctx.emit(Instruction::LocalGet(write_count));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(write_count));
  }
  ctx.emit(Instruction::Else);
  {
    // Found: overwrite value at found_idx
    ctx.emit(Instruction::LocalGet(dst_root));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(found_idx));
    ctx.emit(Instruction::I32Const(16));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(bv));
    ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
  }
  ctx.emit(Instruction::End);

  // bi++
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(bi));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // Patch actual count
  ctx.emit(Instruction::LocalGet(dst_root));
  ctx.emit(Instruction::LocalGet(write_count));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  let dst = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_from_flat", dst_root);
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&map:diff-new a b` — map of entries in `b` whose keys are NOT in `a`.
fn emit_map_diff_new(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&map:diff-new expects 2 args".into());
  }
  let a = emit_ptr_to_i32(ctx, &args[0])?;
  let a_count = emit_load_count_i32(ctx, a);
  let a_flat = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", a);
  let b = emit_ptr_to_i32(ctx, &args[1])?;
  let b_count = emit_load_count_i32(ctx, b);
  let b_flat = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", b);

  // Over-allocate: max is b_count entries
  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));
  let dst_root = emit_alloc_with_count(ctx, b_count, total_slots, "map");

  let write_idx = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(write_idx));

  let bi = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(bi));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  // Load b[bi] key
  let bk = ctx.alloc_local();
  let bkv_addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(b_flat));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalTee(bkv_addr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(bk));

  // Scan a for key
  let found = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(found));
  let ai = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(ai));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(a_flat));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(bk));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(found));
  ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(ai));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // If NOT found in a, copy b[bi] kv to result
  ctx.emit(Instruction::LocalGet(found));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  {
    let addr = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(dst_root));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(write_idx));
    ctx.emit(Instruction::I32Const(16));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(addr));
    ctx.emit(Instruction::LocalGet(addr));
    ctx.emit(Instruction::LocalGet(bk));
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
    ctx.emit(Instruction::LocalGet(addr));
    ctx.emit(Instruction::LocalGet(bkv_addr));
    ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
    ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
    ctx.emit(Instruction::LocalGet(write_idx));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(write_idx));
  }
  ctx.emit(Instruction::End);

  // bi++
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(bi));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // Patch actual count
  ctx.emit(Instruction::LocalGet(dst_root));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  let dst = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_from_flat", dst_root);
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&map:diff-keys a b` — set of keys in `a` that are NOT in `b`.
fn emit_map_diff_keys(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&map:diff-keys expects 2 args".into());
  }
  let a = emit_ptr_to_i32(ctx, &args[0])?;
  let a_count = emit_load_count_i32(ctx, a);
  let a_flat = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", a);
  let b = emit_ptr_to_i32(ctx, &args[1])?;
  let b_count = emit_load_count_i32(ctx, b);
  let b_flat = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", b);

  // Result is a set: over-allocate with a_count
  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));
  let dst = emit_alloc_with_count(ctx, a_count, total_slots, "set");

  let write_idx = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(write_idx));

  let ai = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(ai));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  // Load a[ai] key (maps: stride 16, key at offset 0 from entry)
  let ak = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(a_flat));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(ak));

  // Scan b for key
  let found = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(found));
  let bi = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(bi));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(b_flat));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(ak));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(found));
  ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(bi));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // If NOT found in b, add key to result set
  ctx.emit(Instruction::LocalGet(found));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(ak));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(write_idx));
  ctx.emit(Instruction::End);

  // ai++
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(ai));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // Patch actual count
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&map:common-keys a b` — set of keys present in both `a` and `b`.
fn emit_map_common_keys(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&map:common-keys expects 2 args".into());
  }
  let a = emit_ptr_to_i32(ctx, &args[0])?;
  let a_count = emit_load_count_i32(ctx, a);
  let a_flat = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", a);
  let b = emit_ptr_to_i32(ctx, &args[1])?;
  let b_count = emit_load_count_i32(ctx, b);
  let b_flat = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", b);

  // Result is a set: over-allocate with a_count
  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));
  let dst = emit_alloc_with_count(ctx, a_count, total_slots, "set");

  let write_idx = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(write_idx));

  let ai = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(ai));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  // Load a[ai] key
  let ak = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(a_flat));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(ak));

  // Scan b for key
  let found = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(found));
  let bi = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(bi));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(b_flat));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(ak));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(found));
  ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(bi));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // If found in b, add key to result set
  ctx.emit(Instruction::LocalGet(found));
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(ak));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(write_idx));
  ctx.emit(Instruction::End);

  // ai++
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(ai));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // Patch actual count
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `range n` or `range a b` — create a list of numbers [0..n) or [a..b).
fn emit_range(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.is_empty() || args.len() > 2 {
    return Err("range expects 1 or 2 args".into());
  }

  let start = ctx.alloc_local();
  let end = ctx.alloc_local();

  if args.len() == 1 {
    ctx.emit(f64_const(0.0));
    ctx.emit(Instruction::LocalSet(start));
    emit_expr(ctx, &args[0])?;
    ctx.emit(Instruction::LocalSet(end));
  } else {
    emit_expr(ctx, &args[0])?;
    ctx.emit(Instruction::LocalSet(start));
    emit_expr(ctx, &args[1])?;
    ctx.emit(Instruction::LocalSet(end));
  }

  // count = max(0, trunc(end) - trunc(start))
  let count = ctx.alloc_local_typed(ValType::I32);
  let raw_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(end));
  ctx.emit(Instruction::I32TruncF64S);
  ctx.emit(Instruction::LocalGet(start));
  ctx.emit(Instruction::I32TruncF64S);
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(raw_count));
  // clamp to 0 if negative: select(val_true, val_false, cond)
  ctx.emit(Instruction::LocalGet(raw_count)); // val if true (raw > 0)
  ctx.emit(Instruction::I32Const(0)); // val if false
  ctx.emit(Instruction::LocalGet(raw_count));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::I32GtS); // cond: raw_count > 0
  ctx.emit(Instruction::Select);
  ctx.emit(Instruction::LocalSet(count));

  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));
  let dst = emit_alloc_with_count(ctx, count, total_slots, "list");

  // Fill: dst[8 + i*8] = start + i
  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(i));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(start));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Add);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(i));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}
