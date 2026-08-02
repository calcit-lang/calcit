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
  CodeSection, ConstExpr, ElementSection, Elements, ExportKind, ExportSection, Function, FunctionSection, GlobalSection, GlobalType,
  Ieee64, Instruction, MemorySection, MemoryType, Module, RefType, TableSection, TableType, TypeSection, ValType,
};

use crate::builtins::syntax::get_raw_args_fn;
use crate::calcit::{
  Calcit, CalcitArgLabel, CalcitFnArgs, CalcitImport, CalcitLocal, CalcitProc, CalcitStruct, CalcitSyntax, MethodKind,
};
use crate::program;

#[path = "emit_wasm/methods.rs"]
mod methods;
#[path = "emit_wasm/records.rs"]
mod records;
#[path = "emit_wasm/runtime.rs"]
mod runtime;

use methods::{emit_call_args, emit_method_invoke};
use records::{
  emit_enum_tuple_new, emit_record_contains, emit_record_count, emit_record_field_tag, emit_record_get, emit_record_get_name,
  emit_record_matches, emit_record_new, emit_record_nth, emit_record_struct, emit_record_to_map, emit_tuple_assoc, emit_tuple_count,
  emit_tuple_new, emit_tuple_nth, resolve_struct_ref, try_parse_defrecord_form,
};
use runtime::{HOST_IMPORTS, build_runtime_fns, build_wasm_module};

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

/// MemArg for single-byte load/store (1-byte aligned, memory 0).
fn mem_arg_byte(offset: u64) -> wasm_encoder::MemArg {
  wasm_encoder::MemArg {
    offset,
    align: 0, // 2^0 = 1 byte
    memory_index: 0,
  }
}

/// Emit a WASM binary module from the compiled program.
/// Processes functions from all namespaces in the program.
#[path = "emit_wasm/heap.rs"]
mod heap;
#[path = "emit_wasm/hof.rs"]
mod hof;
#[path = "emit_wasm/lists.rs"]
mod lists;
#[path = "emit_wasm/maps.rs"]
mod maps;
#[path = "emit_wasm/sets.rs"]
mod sets;
#[path = "emit_wasm/strings.rs"]
mod strings;

#[allow(unused_imports)]
pub(super) use heap::*; // makes heap fns available to sibling submodules via `use super::*`
use hof::*;
use lists::*;
use maps::*;
use sets::*;
use strings::*;

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

  // Collect tags early — needed to embed the string type tag in the __str_new helper.
  let tag_index = collect_all_tags_from(&fn_defs);
  println!("TAG INDEX: {tag_index:?}");

  let (mut compiled_fns, mut runtime_fn_index) = build_runtime_fns(
    num_imports,
    *tag_index.get("map").expect("map tag must exist") as i32,
    *tag_index.get("list").expect("list tag must exist") as i32,
    *tag_index.get("string").expect("string tag must exist") as i32,
  );

  // Emit __str_new(src_ptr: i32, byte_len: i32) → f64 now that we know the string tag id.
  // This is a runtime helper exported for JS FFI: copies bytes into a tagged heap string.
  let str_tag_id = *tag_index.get("string").expect("string tag must exist") as i32;
  let str_new_idx = num_imports + compiled_fns.len() as u32;
  runtime_fn_index.insert("__str_new".to_string(), str_new_idx);
  compiled_fns.push(build_str_new_fn(str_tag_id));

  // Pad helpers (need str_tag_id for heap allocation).
  let str_pad_left_idx = num_imports + compiled_fns.len() as u32;
  runtime_fn_index.insert("__rt_str_pad_left".to_string(), str_pad_left_idx);
  compiled_fns.push(build_str_pad_left_fn(str_tag_id));

  let str_pad_right_idx = num_imports + compiled_fns.len() as u32;
  runtime_fn_index.insert("__rt_str_pad_right".to_string(), str_pad_right_idx);
  compiled_fns.push(build_str_pad_right_fn(str_tag_id));

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
  // fn_table_index: user calcit fn (index i in fn_defs) → table slot i.
  // Table slots are 0-based within user calcit functions only (not runtime helpers).
  let mut fn_table_index: HashMap<String, u32> = HashMap::new();
  for (i, (ns, name, args, _)) in fn_defs.iter().enumerate() {
    let idx = num_imports + runtime_fn_count + i as u32;
    let qualified = format!("{ns}/{name}");
    fn_index.insert(qualified.clone(), idx);
    fn_index.insert(name.clone(), idx);
    let (arity, rest_fixed) = compute_fn_arity(args);
    fn_arity.insert(qualified.clone(), arity);
    fn_arity.insert(name.clone(), arity);
    if let Some(fixed) = rest_fixed {
      fn_has_rest.insert(qualified.clone(), fixed);
      fn_has_rest.insert(name.clone(), fixed);
    }
    // Table slot = position in fn_defs (0-based)
    fn_table_index.insert(qualified, i as u32);
    fn_table_index.insert(name.clone(), i as u32);
  }

  let record_field_tags = collect_record_field_tags_from_program(&program_data, &tag_index);

  // Build string literal pool: assigns each unique string a memory offset.
  let (string_pool, string_data_segment, heap_start) = build_string_pool(&fn_defs, &tag_index);

  // Scan for defatom definitions — each gets a mutable WASM global (f64).
  let mut atom_initial_values: Vec<f64> = Vec::new();
  let mut atom_globals: HashMap<String, u32> = HashMap::new();
  // Collect top-level value defs so imported constants can be emitted as expressions.
  let mut value_imports: HashMap<String, Calcit> = HashMap::new();
  for &ns in &ns_order {
    let Some(file_info) = program_data.get(ns) else {
      continue;
    };
    for (def_name, compiled) in &file_info.defs {
      let qualified = format!("{ns}/{def_name}");
      if matches!(compiled.kind, program::CompiledDefKind::Value | program::CompiledDefKind::LazyValue) {
        value_imports.insert(qualified.clone(), compiled.preprocessed_code.to_owned());
      }
      if let crate::calcit::Calcit::List(xs) = &compiled.preprocessed_code
        && matches!(
          xs.first(),
          Some(crate::calcit::Calcit::Syntax(crate::calcit::CalcitSyntax::Defatom, _))
        )
      {
        let global_idx = atom_initial_values.len() as u32;
        atom_globals.insert(qualified, global_idx);
        // Determine initial value from 3rd node (index 2)
        let init_val = match xs.get(2) {
          Some(crate::calcit::Calcit::Bool(true)) => 1.0,
          Some(crate::calcit::Calcit::Number(n)) => *n,
          _ => 0.0, // false / nil / complex init → default 0.0
        };
        atom_initial_values.push(init_val);
      }
    }
  }

  let env = WasmCompileEnv {
    fn_index,
    fn_arity,
    fn_has_rest,
    runtime_fn_index,
    tag_index,
    record_field_tags,
    string_pool,
    atom_globals,
    value_imports,
    fn_table_index,
  };

  // Second pass: compile. If a function fails, we still reserve its slot
  // with a trivial body so indices remain stable.
  for (ns, def_name, args, body) in &fn_defs {
    let export_name = if export_name_counts.get(def_name).copied().unwrap_or(0) > 1 {
      format!("{ns}/{def_name}")
    } else {
      def_name.clone()
    };
    // Try custom implementation (avoids skip for known-good WASM rewrites).
    let result = try_custom_def_impl(ns, def_name, &export_name, args, &env)
      .unwrap_or_else(|| compile_fn(def_name, &export_name, args, body, &env));
    match result {
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
  let wasm_bytes = build_wasm_module(
    &compiled_fns,
    heap_start,
    &string_data_segment,
    &atom_initial_values,
    str_tag_id,
    runtime_fn_count,
  )?;

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
  /// qualified atom name ("ns/def") → WASM global index
  atom_globals: HashMap<String, u32>,
  /// qualified value name ("ns/def") → preprocessed expression for inlining.
  value_imports: HashMap<String, Calcit>,
  /// qualified function name → funcref table slot index (0-based, for call_indirect)
  fn_table_index: HashMap<String, u32>,
}

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
  /// qualified atom name ("ns/def") → WASM global index
  atom_globals: HashMap<String, u32>,
  /// qualified value name ("ns/def") → preprocessed expression for inlining.
  value_imports: HashMap<String, Calcit>,
  /// qualified function name → funcref table slot index (for call_indirect)
  fn_table_index: HashMap<String, u32>,
  /// inline lambda locals: local name → (params, body)
  /// When a let-binding holds a `fn`/`defn` form, it's stored here for inlining at call sites.
  lambda_locals: HashMap<String, (Vec<String>, Vec<Calcit>)>,
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
      atom_globals: env.atom_globals,
      value_imports: env.value_imports,
      fn_table_index: env.fn_table_index,
      lambda_locals: HashMap::new(),
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

  /// Emit a `Call` to a runtime helper by name.
  /// Caller must push the arguments onto the stack first.
  /// Panics if the name is not registered.
  pub(super) fn call_rt(&mut self, name: &str) {
    let fn_idx = *self
      .runtime_fn_index
      .get(name)
      .unwrap_or_else(|| panic!("runtime helper missing: {name}"));
    self.emit(Instruction::Call(fn_idx));
  }

  /// Evaluate all `args`, dropping each result, then push nil (0.0).
  /// Used for operations that are intentionally no-ops in WASM.
  pub(super) fn stub_proc(&mut self, args: &[Calcit]) -> Result<(), String> {
    for arg in args {
      emit_expr(self, arg)?;
      self.emit(Instruction::Drop);
    }
    self.emit(f64_const(0.0));
    Ok(())
  }

  /// Silently ignore all args and return nil. Use for type-system procs whose
  /// arguments may contain tags/values not representable in WASM (e.g. `:&core-number-methods`).
  /// NOTE: This should only be used for initialization/setup code that runs before user code.
  pub(super) fn silent_nil(&mut self) -> Result<(), String> {
    self.emit(f64_const(0.0));
    Ok(())
  }

  // -----------------------------------------------------------------------
  // Integer arithmetic helpers
  // -----------------------------------------------------------------------

  /// `local += 1` — 4-instruction pattern compressed to one call.
  pub(super) fn i32_inc(&mut self, local: u32) {
    self.emit(Instruction::LocalGet(local));
    self.emit(Instruction::I32Const(1));
    self.emit(Instruction::I32Add);
    self.emit(Instruction::LocalSet(local));
  }

  /// `local -= 1`.
  pub(super) fn i32_dec(&mut self, local: u32) {
    self.emit(Instruction::LocalGet(local));
    self.emit(Instruction::I32Const(1));
    self.emit(Instruction::I32Sub);
    self.emit(Instruction::LocalSet(local));
  }

  /// `local = a + b` — store sum of two i32 locals into `dst`.
  #[allow(dead_code)]
  pub(super) fn i32_add_into(&mut self, a: u32, b: u32, dst: u32) {
    self.emit(Instruction::LocalGet(a));
    self.emit(Instruction::LocalGet(b));
    self.emit(Instruction::I32Add);
    self.emit(Instruction::LocalSet(dst));
  }

  /// `local = a - b`.
  #[allow(dead_code)]
  pub(super) fn i32_sub_into(&mut self, a: u32, b: u32, dst: u32) {
    self.emit(Instruction::LocalGet(a));
    self.emit(Instruction::LocalGet(b));
    self.emit(Instruction::I32Sub);
    self.emit(Instruction::LocalSet(dst));
  }

  // -----------------------------------------------------------------------
  // Control-flow structure helpers (Block / Loop / If)
  // -----------------------------------------------------------------------

  /// Begin a `block` (used as the outer breakable container around a loop).
  pub(super) fn begin_block(&mut self) {
    self.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  }

  /// Begin a `loop` (the inner repeated body).
  pub(super) fn begin_loop(&mut self) {
    self.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  }

  /// Begin an `if` with no result value (void block).
  pub(super) fn begin_block_if(&mut self) {
    self.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  }

  /// Emit `end` for a single block/loop/if.
  #[allow(dead_code)]
  pub(super) fn end_one(&mut self) {
    self.emit(Instruction::End);
  }

  /// Emit two `end`s — closes a loop then its surrounding block.
  pub(super) fn end_block_loop(&mut self) {
    self.emit(Instruction::End); // end loop
    self.emit(Instruction::End); // end block
  }

  /// `br 0` — jump back to the top of the enclosing loop.
  pub(super) fn br_loop(&mut self) {
    self.emit(Instruction::Br(0));
  }

  /// `br_if 1` — exit the outer block (break out of the block+loop pair) if i32 on stack is non-zero.
  pub(super) fn br_if_exit(&mut self) {
    self.emit(Instruction::BrIf(1));
  }

  /// Emit the "i >= count → break" guard common to all forward-indexed loops.
  /// Expects both locals to be i32. Emits: `LocalGet(i); LocalGet(count); I32GeU; BrIf(1)`.
  pub(super) fn loop_exit_if_ge(&mut self, i: u32, count: u32) {
    self.emit(Instruction::LocalGet(i));
    self.emit(Instruction::LocalGet(count));
    self.emit(Instruction::I32GeU);
    self.br_if_exit();
  }

  /// Emit the "i < 0 → break" guard used by reverse loops.
  /// Expects `i` to be an i32. Emits: `LocalGet(i); I32Const(0); I32LtS; BrIf(1)`.
  pub(super) fn loop_exit_if_neg(&mut self, i: u32) {
    self.emit(Instruction::LocalGet(i));
    self.emit(Instruction::I32Const(0));
    self.emit(Instruction::I32LtS);
    self.br_if_exit();
  }

  /// Push an i32 local onto the WASM stack as f64.
  /// Common return pattern: `LocalGet(ptr) + F64ConvertI32U`.
  pub(super) fn ptr_to_f64(&mut self, local: u32) {
    self.emit(Instruction::LocalGet(local));
    self.emit(Instruction::F64ConvertI32U);
  }

  /// Store an i32 local as f64 into slot at `byte_offset` relative to `ptr`.
  /// Pattern: `LocalGet(ptr) + LocalGet(val) + F64ConvertI32U + F64Store(offset)`.
  pub(super) fn store_i32_as_f64(&mut self, ptr: u32, val: u32, byte_offset: u64) {
    self.emit(Instruction::LocalGet(ptr));
    self.emit(Instruction::LocalGet(val));
    self.emit(Instruction::F64ConvertI32U);
    self.emit(Instruction::F64Store(mem_arg_f64(byte_offset)));
  }

  /// Allocate a new i32 local and initialise it to a constant.
  /// Common pattern: `let x = alloc_local_typed(I32); I32Const(val); LocalSet(x)`.
  pub(super) fn alloc_i32(&mut self, val: i32) -> u32 {
    let local = self.alloc_local_typed(ValType::I32);
    self.emit(Instruction::I32Const(val));
    self.emit(Instruction::LocalSet(local));
    local
  }

  /// Allocate a new i32 local = `src + delta` (negative delta uses I32Sub).
  /// Replaces: `alloc_local_typed(I32) + LocalGet(src) + I32Const(|delta|) + I32Add/Sub + LocalSet`.
  pub(super) fn i32_offset(&mut self, src: u32, delta: i32) -> u32 {
    let local = self.alloc_local_typed(ValType::I32);
    self.emit(Instruction::LocalGet(src));
    self.emit(Instruction::I32Const(delta.unsigned_abs() as i32));
    if delta >= 0 {
      self.emit(Instruction::I32Add);
    } else {
      self.emit(Instruction::I32Sub);
    }
    self.emit(Instruction::LocalSet(local));
    local
  }
}

/// Check that `args` has exactly `n` elements and return an error if not.
#[inline]
fn expect_arity(n: usize, args: &[Calcit], proc_name: &str) -> Result<(), String> {
  if args.len() != n {
    return Err(format!("{proc_name} expects {n} arg(s), got {}", args.len()));
  }
  Ok(())
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

fn try_custom_def_impl(
  ns: &str,
  def_name: &str,
  export_name: &str,
  _args: &CalcitFnArgs,
  env: &WasmCompileEnv,
) -> Option<Result<CompiledFn, String>> {
  if ns != "calcit.core" {
    return None;
  }
  // Helpers for building a 2-param custom function body.
  type BodyFn = fn(&mut WasmGenCtx) -> Result<(), String>;
  let arity: u32 = 2;
  let build = |body_fn: BodyFn, en: &str, env: &WasmCompileEnv| -> Result<CompiledFn, String> {
    let mut ctx = WasmGenCtx::new(arity, env.clone());
    // Register param locals 0 and 1
    ctx.locals.insert("__p0__".into(), 0);
    ctx.locals.insert("__p1__".into(), 1);
    ctx.arg_indices.push(0);
    ctx.arg_indices.push(1);
    body_fn(&mut ctx)?;
    Ok(CompiledFn {
      export_name: Some(en.to_owned()),
      params: vec![ValType::F64; arity as usize],
      results: vec![ValType::F64],
      locals: ctx.extra_locals,
      instructions: ctx.instructions,
    })
  };
  match def_name {
    "repeat" => Some(build(|ctx| emit_repeat_from_locals(ctx, 0, 1), export_name, env)),
    "interleave" => Some(build(|ctx| emit_interleave_from_locals(ctx, 0, 1), export_name, env)),
    "zipmap" => Some(build(|ctx| emit_zipmap_from_locals(ctx, 0, 1), export_name, env)),
    "join" => Some(build(|ctx| emit_join_from_locals(ctx, 0, 1), export_name, env)),
    "join-str" => Some(build(|ctx| emit_join_str_from_locals(ctx, 0, 1), export_name, env)),
    _ => None,
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

/// Emit an inline IIFE with TCO: `((defn name (params...) body...) init_args...)`.
///
/// - Allocates new f64 locals for each param, initializes from `init_args`.
/// - If the body uses `recur`, wraps with `loop...end` and sets up `ctx.arg_indices`
///   so that `recur` correctly jumps back to the loop start.
/// - Outer params of the same name are shadowed and restored after.
fn emit_inline_iife(ctx: &mut WasmGenCtx, params: &[String], body: &[Calcit], init_args: &[Calcit]) -> Result<(), String> {
  // Evaluate all init args and store in fresh locals (before modifying ctx.locals).
  let mut param_locals: Vec<u32> = Vec::new();
  for (i, _param_name) in params.iter().enumerate() {
    let tmp = ctx.alloc_local();
    if i < init_args.len() {
      emit_expr(ctx, &init_args[i])?;
    } else {
      ctx.emit(f64_const(0.0)); // nil for missing args
    }
    ctx.emit(Instruction::LocalSet(tmp));
    param_locals.push(tmp);
  }

  // Shadow outer locals with IIFE params (save old bindings for restoration).
  let mut saved: Vec<(String, Option<u32>)> = Vec::new();
  for (i, name) in params.iter().enumerate() {
    saved.push((name.clone(), ctx.locals.get(name).copied()));
    ctx.locals.insert(name.clone(), param_locals[i]);
  }

  // Emit body — with a loop wrapper if TCO recur is needed.
  let uses_recur = body.iter().any(check_uses_recur);
  if uses_recur {
    let old_arg_indices = std::mem::replace(&mut ctx.arg_indices, param_locals);
    let old_block_depth = ctx.block_depth;
    ctx.block_depth = 0;

    ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Result(ValType::F64)));
    emit_body(ctx, body)?;
    ctx.emit(Instruction::End); // end loop

    ctx.block_depth = old_block_depth;
    ctx.arg_indices = old_arg_indices;
  } else {
    emit_body(ctx, body)?;
  }

  // Restore shadowed locals.
  for (name, old) in saved {
    match old {
      Some(idx) => {
        ctx.locals.insert(name, idx);
      }
      None => {
        ctx.locals.remove(&name);
      }
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
    Calcit::List(xs) if xs.is_empty() => {
      emit_list_new(ctx, &[])?;
    }
    Calcit::Tag(t) => {
      let tag_str = t.to_string();
      let id = *ctx
        .tag_index
        .get(&tag_str)
        .ok_or_else(|| format!("unknown tag in WASM codegen: {tag_str}"))?;
      ctx.emit(f64_const(id as f64));
    }
    Calcit::Struct(s) => {
      let tag_str = s.name.to_string();
      let id = *ctx
        .tag_index
        .get(&tag_str)
        .ok_or_else(|| format!("unknown struct tag in WASM codegen: {tag_str}"))?;
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
    Calcit::Import(import) => {
      // Check if this is a reference to a known WASM global (defatom)
      let qualified = format!("{}/{}", import.ns, import.def);
      if let Some(&global_idx) = ctx.atom_globals.get(&qualified) {
        ctx.emit(Instruction::GlobalGet(global_idx));
      } else if import.def.as_ref() == "{}" {
        // `{}` used as a bare expression — evaluates to an empty map.
        emit_map_new(ctx, &[])?;
      } else if import.def.as_ref() == "[]" {
        // `[]` used as a bare expression — evaluates to an empty list.
        emit_list_new(ctx, &[])?;
      } else if let Ok(struct_def) = resolve_struct_ref(expr) {
        let tag_str = struct_def.name.to_string();
        let id = *ctx
          .tag_index
          .get(&tag_str)
          .ok_or_else(|| format!("unknown struct tag in WASM codegen: {tag_str}"))?;
        ctx.emit(f64_const(id as f64));
      } else if let Some(&slot) = ctx
        .fn_table_index
        .get(&qualified)
        .or_else(|| ctx.fn_table_index.get(import.def.as_ref()))
      {
        // Function reference used as a value — encode as f64 table slot index.
        ctx.emit(f64_const(slot as f64));
      } else if let Some(value_expr) = ctx.value_imports.get(&qualified).cloned() {
        // Imported top-level def value (e.g. a string constant). Inline its expression.
        emit_expr(ctx, &value_expr)?;
      } else {
        // defvar or otherwise non-function import — emit nil (0.0) as a placeholder.
        // This covers `def`-defined values that aren't representable as f64.
        // Such references appear mostly in initializer/registration paths that are
        // no-ops in WASM (e.g., &init-builtin-impls!).
        ctx.emit(f64_const(0.0));
      }
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
    // Function value (Fn with def_ref) — encode as f64 table slot index for call_indirect.
    Calcit::Fn { info, .. } => {
      if let Some(def_ref) = &info.def_ref {
        let qualified = format!("{}/{}", def_ref.def_ns, def_ref.def_name);
        let slot = ctx
          .fn_table_index
          .get(&qualified)
          .or_else(|| ctx.fn_table_index.get(def_ref.def_name.as_ref()))
          .copied()
          .ok_or_else(|| format!("fn value not in table: {qualified}"))?;
        ctx.emit(f64_const(slot as f64));
      } else {
        // Anonymous inline closure (no def_ref) — captured upvalues not representable.
        eprintln!("[wasm] anonymous closure (no def_ref): emitting nil placeholder");
        ctx.emit(f64_const(0.0));
      }
    }
    // `[]` used as a bare expression (not in call position) evaluates to an empty list.
    // This arises from `cond` branches like `true []` where `[]` is the return value.
    Calcit::Proc(CalcitProc::List) => {
      emit_list_new(ctx, &[])?;
    }
    // `{}` used as a bare expression — evaluates to an empty map.
    Calcit::Proc(CalcitProc::NativeMap) => {
      emit_map_new(ctx, &[])?;
    }
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
      CalcitSyntax::CallSpread => emit_call_spread(ctx, &args_list),
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
      CalcitSyntax::UnsafeCoerce => {
        if args_list.is_empty() {
          return Err("unsafe-coerce expects at least 1 arg".into());
        }
        emit_expr(ctx, &args_list[0])
      }
      CalcitSyntax::Defn => {
        // A `fn`/`defn` form in value position creates a closure capturing outer variables.
        // Closures with captured upvalues can't be represented as static WASM function
        // indices in the current codegen. Emit nil as a placeholder so the outer function
        // still compiles; callers that invoke the returned "closure" will receive nil.
        eprintln!("[wasm] closure-as-value: emitting nil placeholder for nested fn/defn");
        ctx.emit(f64_const(0.0));
        Ok(())
      }
      CalcitSyntax::Quote | CalcitSyntax::Quasiquote => {
        // Quote creates a runtime value (quoted symbol/expression).
        // In WASM, emit nil as a placeholder — quote values appear mainly in
        // error-reporting paths (e.g., the assert macro formats the failing
        // expression via `format-to-lisp (quote ~xs)`) and don't affect the
        // normal execution path.
        ctx.emit(f64_const(0.0));
        Ok(())
      }
      CalcitSyntax::Reset => {
        // reset! atom new-value — set atom global and return new value
        if args_list.len() != 2 {
          return Err(format!("reset! expects 2 args, got {}", args_list.len()));
        }
        let qualified = match &args_list[0] {
          Calcit::Import(import) => format!("{}/{}", import.ns, import.def),
          _ => return Err(format!("reset! first arg must be an atom import, got: {}", args_list[0])),
        };
        let global_idx = *ctx
          .atom_globals
          .get(&qualified)
          .ok_or_else(|| format!("unknown atom in reset!: {qualified}"))?;
        emit_expr(ctx, &args_list[1])?;
        // Store new value in global, and leave it on stack as return value
        let tmp = ctx.alloc_local();
        ctx.emit(Instruction::LocalTee(tmp));
        ctx.emit(Instruction::GlobalSet(global_idx));
        ctx.emit(Instruction::LocalGet(tmp));
        Ok(())
      }
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
      // High-level set wrappers (union/difference/include) use reduce+lambda which can't be
      // compiled to WASM directly. Intercept and inline native 2+ arg set operations.
      if import.ns.as_ref() == "calcit.core" {
        match import.def.as_ref() {
          "union" => return emit_set_op_variadic(ctx, &args_list, SetOpKind::Union),
          "difference" => return emit_set_op_variadic(ctx, &args_list, SetOpKind::Difference),
          "include" => return emit_set_op_variadic(ctx, &args_list, SetOpKind::Include),
          // `reduce xs x0 f` → inline as foldl so proc/import callees work.
          "reduce" if args_list.len() == 3 => return emit_foldl(ctx, &args_list),
          // `foldl-compare xs acc f` → inline comparison loop.
          "foldl-compare" if args_list.len() == 3 => return emit_foldl_compare(ctx, &args_list),
          // Unary HOF intercepts — callee 'f' would be unresolvable inside core defs.
          "map" | "&list:map" if args_list.len() == 2 => return emit_map(ctx, &args_list),
          "map-indexed" | "&list:map-indexed" if args_list.len() == 2 => return emit_map_indexed(ctx, &args_list),
          "each" if args_list.len() == 2 => return emit_each(ctx, &args_list),
          "filter" | "&list:filter" | "&set:filter" if args_list.len() == 2 => return emit_filter(ctx, &args_list),
          "any?" if args_list.len() == 2 => return emit_any(ctx, &args_list),
          "every?" if args_list.len() == 2 => return emit_every(ctx, &args_list),
          "find" if args_list.len() == 2 => return emit_find(ctx, &args_list),
          "find-index" if args_list.len() == 2 => return emit_find_index(ctx, &args_list),
          // `concat` takes variadic lists — intercept to emit variadic list concat.
          "concat" => return emit_list_concat(ctx, &args_list),
          // `deref` — in WASM, all atoms are globals; just evaluate the atom expression.
          "deref" if args_list.len() == 1 => return emit_expr(ctx, &args_list[0]),
          // `str` / `str-spaced` — variadic string concat; core defs use (&syntax &) which is unsupported.
          "str" if !args_list.is_empty() => return emit_str_variadic(ctx, &args_list),
          "str-spaced" if !args_list.is_empty() => return emit_str_spaced(ctx, &args_list),
          // `foldl'` is an inline variant of `foldl` with same arg order (xs acc f).
          "foldl'" if args_list.len() == 3 => return emit_foldl(ctx, &args_list),
          // `filter-not` — keep elements where f(elem) is falsy.
          "filter-not" if args_list.len() == 2 => return emit_filter_not(ctx, &args_list),
          // `slice` — delegated to list slice (list-only, 2-3 args).
          "slice" if args_list.len() >= 2 && args_list.len() <= 3 => return emit_list_slice(ctx, &args_list),
          // `dissoc` — delegated to map dissoc for 2-arg calls.
          "dissoc" if args_list.len() == 2 => return emit_map_dissoc(ctx, &args_list),
          // `conj` — append one or more elements to a list.
          "conj" if args_list.len() >= 2 => return emit_conj(ctx, &args_list),
          // `update` — map update: new map with key set to f(old value).
          "update" if args_list.len() == 3 => return emit_update(ctx, &args_list),
          // `mapcat xs f` — map then flatten one level.
          "mapcat" if args_list.len() == 2 => return emit_mapcat(ctx, &args_list),
          // Simple loop implementations for HOF-based definitions.
          "repeat" if args_list.len() == 2 => return emit_repeat(ctx, &args_list),
          "interleave" if args_list.len() == 2 => return emit_interleave(ctx, &args_list),
          "zipmap" if args_list.len() == 2 => return emit_zipmap(ctx, &args_list),
          "join" if args_list.len() == 2 => return emit_join(ctx, &args_list),
          "join-str" if args_list.len() == 2 => return emit_join_str(ctx, &args_list),
          // `let` — multi-binding form: (let ((name val)...) body...).
          // The preprocessor normally expands this to nested `&let` forms, but intercept here
          // as a fallback for cases where the macro expansion hasn't occurred.
          "let" if !args_list.is_empty() => return emit_let_multi(ctx, &args_list),
          // `map-kv xs f` — apply a binary function to each map entry, returning a new map.
          "map-kv" if args_list.len() == 2 => return emit_map_kv(ctx, &args_list),
          // `'` — list literal constructor (calcit.core/'), same as ([] a b c).
          "'" => return emit_list_new(ctx, &args_list),
          _ => {}
        }
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
      // HOF interceptors — Symbol-head calls appear when preprocessor doesn't resolve
      // intra-namespace references to Import nodes (e.g. calcit.core internal calls).
      match name {
        "map" if args_list.len() == 2 => return emit_map(ctx, &args_list),
        "filter" | "&list:filter" | "&set:filter" if args_list.len() == 2 => return emit_filter(ctx, &args_list),
        "filter-not" if args_list.len() == 2 => return emit_filter_not(ctx, &args_list),
        "each" if args_list.len() == 2 => return emit_each(ctx, &args_list),
        "any?" if args_list.len() == 2 => return emit_any(ctx, &args_list),
        "every?" if args_list.len() == 2 => return emit_every(ctx, &args_list),
        "find" if args_list.len() == 2 => return emit_find(ctx, &args_list),
        "find-index" if args_list.len() == 2 => return emit_find_index(ctx, &args_list),
        "map-indexed" if args_list.len() == 2 => return emit_map_indexed(ctx, &args_list),
        "mapcat" if args_list.len() == 2 => return emit_mapcat(ctx, &args_list),
        "reduce" if args_list.len() == 3 => return emit_foldl(ctx, &args_list),
        "foldl'" if args_list.len() == 3 => return emit_foldl(ctx, &args_list),
        "update" if args_list.len() == 3 => return emit_update(ctx, &args_list),
        _ => {}
      }
      // Check if this symbol refers to an inline lambda captured in this scope.
      if let Some((params, body)) = ctx.lambda_locals.get(name).cloned()
        && params.len() == args_list.len()
      {
        for (param, arg) in params.iter().zip(args_list.iter()) {
          let arg_lambda = match arg {
            Calcit::Local(a) => ctx.lambda_locals.get(a.sym.as_ref()).cloned(),
            Calcit::Symbol { sym: s, .. } => ctx.lambda_locals.get(s.as_ref()).cloned(),
            _ => None,
          };
          if let Some(captured) = arg_lambda {
            ctx.lambda_locals.insert(param.clone(), captured);
          } else {
            emit_expr(ctx, arg)?;
            let idx = ctx.declare_local(param);
            ctx.emit(Instruction::LocalSet(idx));
          }
        }
        return emit_body(ctx, &body);
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
    Calcit::Fn { info, .. } => {
      let def_ref = info.def_ref.as_ref().ok_or_else(|| {
        format!(
          "function literal without def reference is not supported in WASM: {}/{}",
          info.def_ns, info.name
        )
      })?;
      // Apply HOF interceptors for calcit.core functions referenced as Fn values
      // (same logic as the Import arm, but using def_ref for the name lookup).
      if def_ref.def_ns.as_ref() == "calcit.core" {
        match def_ref.def_name.as_ref() {
          "map" if args_list.len() == 2 => return emit_map(ctx, &args_list),
          "filter" | "&list:filter" | "&set:filter" if args_list.len() == 2 => return emit_filter(ctx, &args_list),
          "filter-not" if args_list.len() == 2 => return emit_filter_not(ctx, &args_list),
          "each" if args_list.len() == 2 => return emit_each(ctx, &args_list),
          "any?" if args_list.len() == 2 => return emit_any(ctx, &args_list),
          "every?" if args_list.len() == 2 => return emit_every(ctx, &args_list),
          "find" if args_list.len() == 2 => return emit_find(ctx, &args_list),
          "find-index" if args_list.len() == 2 => return emit_find_index(ctx, &args_list),
          "map-indexed" if args_list.len() == 2 => return emit_map_indexed(ctx, &args_list),
          "mapcat" if args_list.len() == 2 => return emit_mapcat(ctx, &args_list),
          "reduce" if args_list.len() == 3 => return emit_foldl(ctx, &args_list),
          "foldl'" if args_list.len() == 3 => return emit_foldl(ctx, &args_list),
          "update" if args_list.len() == 3 => return emit_update(ctx, &args_list),
          _ => {}
        }
      }
      let qualified = format!("{}/{}", def_ref.def_ns, def_ref.def_name);
      let fn_idx = ctx
        .fn_index
        .get(&qualified)
        .or_else(|| ctx.fn_index.get(def_ref.def_name.as_ref()))
        .copied()
        .ok_or_else(|| format!("unknown function literal target in WASM: {qualified}"))?;
      let target_arity = ctx
        .fn_arity
        .get(&qualified)
        .or_else(|| ctx.fn_arity.get(def_ref.def_name.as_ref()))
        .copied()
        .unwrap_or(args_list.len() as u32);
      let rest_fixed = ctx
        .fn_has_rest
        .get(&qualified)
        .or_else(|| ctx.fn_has_rest.get(def_ref.def_name.as_ref()))
        .copied();
      emit_call_args(ctx, &args_list, target_arity, rest_fixed)?;
      ctx.emit(Instruction::Call(fn_idx));
      Ok(())
    }
    // Inline IIFE: call head is `(defn name (params...) body...)`
    // Generated by `apply-args` and similar macros for TCO-friendly recursive loops.
    Calcit::List(iife_items) if matches!(iife_items.first(), Some(Calcit::Syntax(CalcitSyntax::Defn, _))) => {
      let params = match iife_items.get(2) {
        Some(Calcit::List(param_list)) => param_list
          .iter()
          .filter_map(|p| match p {
            Calcit::Local(CalcitLocal { sym, .. }) => Some(sym.as_ref().to_owned()),
            Calcit::Symbol { sym, .. } => Some(sym.as_ref().to_owned()),
            _ => None,
          })
          .collect::<Vec<_>>(),
        other => return Err(format!("IIFE defn: params list expected, got: {other:?}")),
      };
      let body: Vec<Calcit> = iife_items.iter().skip(3).cloned().collect();
      emit_inline_iife(ctx, &params, &body, &args_list)
    }
    // Dynamic call via a local variable holding a function table index (f64).
    // Implements Rust-style `dyn Fn` dispatch via WASM call_indirect.
    Calcit::Local(local) => {
      let local_name = local.sym.as_ref().to_string();
      // If this local is an inline lambda, inline the call directly.
      if let Some((params, body)) = ctx.lambda_locals.get(&local_name).cloned()
        && params.len() == args_list.len()
      {
        // Bind each arg to its param local then emit the body.
        // If an arg is itself a lambda_local (a thunk/lambda), propagate rather than evaluate.
        for (param, arg) in params.iter().zip(args_list.iter()) {
          let arg_lambda = match arg {
            Calcit::Local(a) => ctx.lambda_locals.get(a.sym.as_ref()).cloned(),
            Calcit::Symbol { sym, .. } => ctx.lambda_locals.get(sym.as_ref()).cloned(),
            _ => None,
          };
          if let Some(captured) = arg_lambda {
            ctx.lambda_locals.insert(param.clone(), captured);
          } else {
            emit_expr(ctx, arg)?;
            let idx = ctx.declare_local(param);
            ctx.emit(Instruction::LocalSet(idx));
          }
        }
        return emit_body(ctx, &body);
      }
      let local_idx = *ctx
        .locals
        .get(&local_name)
        .ok_or_else(|| format!("undefined local used as function: {}", local.sym))?;
      // Emit all arguments
      for arg in &args_list {
        emit_expr(ctx, arg)?;
      }
      // Load function table index (f64) and truncate to i32 for call_indirect
      ctx.emit(Instruction::LocalGet(local_idx));
      ctx.emit(Instruction::I32TruncF64S);
      // Canonical type index = number of args (type N = (f64×N) → f64)
      ctx.emit(Instruction::CallIndirect {
        type_index: args_list.len() as u32,
        table_index: 0,
      });
      Ok(())
    }
    _ => Err(format!("unsupported call head in WASM: {head}")),
  }
}

fn emit_call_spread(ctx: &mut WasmGenCtx, args_list: &[Calcit]) -> Result<(), String> {
  if args_list.is_empty() {
    return Err("&call-spread expects at least a callee".into());
  }

  let head = &args_list[0];
  let call_args = &args_list[1..];

  match head {
    Calcit::Import(import) => {
      let qualified = format!("{}/{}", import.ns, import.def);
      let fn_idx = ctx
        .fn_index
        .get(&qualified)
        .or_else(|| ctx.fn_index.get(import.def.as_ref()))
        .copied()
        .ok_or_else(|| format!("unknown function: {qualified}"))?;
      let target_arity = ctx
        .fn_arity
        .get(&qualified)
        .or_else(|| ctx.fn_arity.get(import.def.as_ref()))
        .copied()
        .unwrap_or(call_args.len() as u32);
      let rest_fixed = ctx
        .fn_has_rest
        .get(&qualified)
        .or_else(|| ctx.fn_has_rest.get(import.def.as_ref()))
        .copied();
      emit_call_spread_args(ctx, call_args, target_arity, rest_fixed)?;
      ctx.emit(Instruction::Call(fn_idx));
      Ok(())
    }
    Calcit::Symbol { sym, .. } => {
      let name = sym.as_ref();
      let fn_idx = *ctx.fn_index.get(name).ok_or_else(|| format!("unknown function: {sym}"))?;
      let target_arity = ctx.fn_arity.get(name).copied().unwrap_or(call_args.len() as u32);
      let rest_fixed = ctx.fn_has_rest.get(name).copied();
      emit_call_spread_args(ctx, call_args, target_arity, rest_fixed)?;
      ctx.emit(Instruction::Call(fn_idx));
      Ok(())
    }
    Calcit::Fn { info, .. } => {
      let def_ref = info.def_ref.as_ref().ok_or_else(|| {
        format!(
          "function literal without def reference is not supported in WASM: {}/{}",
          info.def_ns, info.name
        )
      })?;
      let qualified = format!("{}/{}", def_ref.def_ns, def_ref.def_name);
      let fn_idx = ctx
        .fn_index
        .get(&qualified)
        .or_else(|| ctx.fn_index.get(def_ref.def_name.as_ref()))
        .copied()
        .ok_or_else(|| format!("unknown function literal target in WASM: {qualified}"))?;
      let target_arity = ctx
        .fn_arity
        .get(&qualified)
        .or_else(|| ctx.fn_arity.get(def_ref.def_name.as_ref()))
        .copied()
        .unwrap_or(call_args.len() as u32);
      let rest_fixed = ctx
        .fn_has_rest
        .get(&qualified)
        .or_else(|| ctx.fn_has_rest.get(def_ref.def_name.as_ref()))
        .copied();
      emit_call_spread_args(ctx, call_args, target_arity, rest_fixed)?;
      ctx.emit(Instruction::Call(fn_idx));
      Ok(())
    }
    Calcit::Proc(proc) => {
      if matches!(proc, CalcitProc::Recur | CalcitProc::NativeListDissoc | CalcitProc::NativeMapDissoc) {
        // These procs handle ArgSpread markers themselves — pass call_args raw.
        emit_proc_call(ctx, proc, call_args)
      } else {
        // A proc used as the spread callee — just emit a regular proc call with spread args
        emit_call_spread_args_as_regular(ctx, proc, call_args)
      }
    }
    // Method spread call: e.g. `(.dissoc x & args)` — emit unreachable trap.
    // In practice reached only for record/tuple methods not yet implemented in WASM.
    // List/map cases are handled via their native procs before this point.
    Calcit::Method(_name, MethodKind::Invoke(_)) => {
      ctx.emit(Instruction::Unreachable);
      Ok(())
    }
    // Dynamic spread call via a local holding a function value — emit unreachable.
    Calcit::Local(_) => {
      ctx.emit(Instruction::Unreachable);
      Ok(())
    }
    _ => Err(format!("unsupported call head in WASM: {head}")),
  }
}

/// For proc callees in spread calls: collect args (handling `& spread-list`) and emit proc.
fn emit_call_spread_args_as_regular(ctx: &mut WasmGenCtx, proc: &CalcitProc, call_args: &[Calcit]) -> Result<(), String> {
  // Gather concrete args, expanding `& spread-list` if present
  let mut real_args: Vec<Calcit> = vec![];
  let mut i = 0;
  while i < call_args.len() {
    if matches!(call_args[i], Calcit::Syntax(CalcitSyntax::ArgSpread, _)) {
      i += 1; // skip the & marker, next is the spread list
    } else {
      real_args.push(call_args[i].clone());
    }
    i += 1;
  }
  emit_proc_call(ctx, proc, &real_args)
}

fn emit_call_spread_args(ctx: &mut WasmGenCtx, call_args: &[Calcit], target_arity: u32, rest_fixed: Option<u32>) -> Result<(), String> {
  let Some(fixed) = rest_fixed else {
    return Err("&call-spread in WASM currently requires the target function to accept rest args".into());
  };
  let fixed = fixed as usize;

  // Locate the ArgSpread marker (if any) to split explicit args from the spread list.
  let spread_pos = call_args
    .iter()
    .position(|a| matches!(a, Calcit::Syntax(CalcitSyntax::ArgSpread, _)));

  let (explicit_args, spread_list_opt): (&[Calcit], Option<&Calcit>) = if let Some(pos) = spread_pos {
    (&call_args[..pos], call_args.get(pos + 1))
  } else if call_args.len() == fixed + 1 {
    // Old-style: no ArgSpread marker; last arg is the rest list.
    (&call_args[..fixed], Some(&call_args[fixed]))
  } else {
    return Err(format!(
      "&call-spread in WASM expects {} fixed args plus `& spread-list`, got {} args",
      fixed,
      call_args.len()
    ));
  };

  let Some(spread_list_expr) = spread_list_opt else {
    return Err("&call-spread missing spread list expression".into());
  };

  let n_explicit = explicit_args.len();
  let n_from_spread = fixed.saturating_sub(n_explicit);

  // Emit explicit fixed args.
  for arg in explicit_args {
    emit_expr(ctx, arg)?;
  }

  if n_from_spread == 0 {
    // All fixed params already emitted; spread_list_expr IS the rest arg.
    emit_expr(ctx, spread_list_expr)?;
  } else {
    // Need to extract `n_from_spread` fixed params from the spread list,
    // then emit the remainder as the rest arg.
    emit_expr(ctx, spread_list_expr)?;
    let spread_f64 = ctx.alloc_local();
    ctx.emit(Instruction::LocalSet(spread_f64));
    let spread_i32 = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(spread_f64));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(spread_i32));

    // Extract each required fixed param: list[i] = spread_i32[(1+i)*8]
    for i in 0..n_from_spread {
      ctx.emit(Instruction::LocalGet(spread_i32));
      ctx.emit(Instruction::I32Const(((1 + i) * 8) as i32));
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    }

    // Emit the rest arg = list_slice(spread, n_from_spread).
    emit_list_slice_from_i32_local(ctx, spread_i32, n_from_spread)?;
  }

  let emitted_args = fixed + 1;
  for _ in emitted_args..(target_arity as usize) {
    ctx.emit(f64_const(0.0));
  }

  Ok(())
}

/// Emit a new list containing `src_i32[from_idx..]`.
/// `src_i32` is a WASM local holding the I32 pointer to the source list.
/// Leaves an f64 pointer to the new slice list on the stack.
fn emit_list_slice_from_i32_local(ctx: &mut WasmGenCtx, src_i32: u32, from_idx: usize) -> Result<(), String> {
  // old_count = load count field (f64 → i32)
  let old_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(src_i32));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(old_count));

  let new_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(old_count));
  ctx.emit(Instruction::I32Const(from_idx as i32));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(new_count));

  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(new_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));

  let dst = emit_alloc_with_count(ctx, new_count, total_slots, "list");

  // src_base = src_i32 + 8 + from_idx*8
  let src_base = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(src_i32));
  ctx.emit(Instruction::I32Const((8 + from_idx * 8) as i32));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(src_base));

  let dst_base = emit_addr_offset(ctx, dst, 8);
  emit_copy_f64_loop(ctx, dst_base, src_base, new_count);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
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
    CalcitProc::NativeEquals => emit_equals(ctx, args),
    CalcitProc::Identical => emit_cmp(ctx, Instruction::F64Eq, args),
    CalcitProc::NativeCompare => {
      // &compare a b → -1.0 if a<b, 0.0 if a==b, 1.0 if a>b
      expect_arity(2, args, "&compare")?;
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
      expect_arity(1, args, "not")?;
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

    // round?: x == floor(x)
    CalcitProc::IsRound => {
      expect_arity(1, args, "round?")?;
      let v = ctx.alloc_local();
      emit_expr(ctx, &args[0])?;
      ctx.emit(Instruction::LocalSet(v));
      ctx.emit(Instruction::LocalGet(v));
      ctx.emit(Instruction::F64Floor);
      ctx.emit(Instruction::LocalGet(v));
      ctx.emit(Instruction::F64Eq);
      ctx.emit(Instruction::F64ConvertI32U);
      Ok(())
    }

    // &number:fract: x - floor(x)
    CalcitProc::NativeNumberFract => {
      expect_arity(1, args, "&number:fract")?;
      let v = ctx.alloc_local();
      emit_expr(ctx, &args[0])?;
      ctx.emit(Instruction::LocalSet(v));
      ctx.emit(Instruction::LocalGet(v));
      ctx.emit(Instruction::LocalGet(v));
      ctx.emit(Instruction::F64Floor);
      ctx.emit(Instruction::F64Sub);
      Ok(())
    }
    CalcitProc::Sin => emit_host_call(ctx, "sin", args),
    CalcitProc::Cos => emit_host_call(ctx, "cos", args),
    CalcitProc::Pow => emit_host_call(ctx, "pow", args),

    // type-of: reads the heap type header or returns :number for non-pointers.
    CalcitProc::TypeOf => emit_type_of(ctx, args),

    // type predicates
    CalcitProc::ListQuestion => emit_type_predicate(ctx, "list", args),
    CalcitProc::TagQuestion => emit_type_predicate(ctx, "tag", args),
    CalcitProc::SymbolQuestion => emit_type_predicate(ctx, "symbol", args),
    CalcitProc::NilQuestion => {
      expect_arity(1, args, "nil?")?;
      // Current WASM backend represents nil as 0.0.
      ctx.emit(f64_const(1.0));
      ctx.emit(f64_const(0.0));
      emit_expr(ctx, &args[0])?;
      ctx.emit(f64_const(0.0));
      ctx.emit(Instruction::F64Eq);
      ctx.emit(Instruction::Select);
      Ok(())
    }
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
      // Check for spread: args may be [a1, ..., ak, ArgSpread, rest_list]
      let spread_pos = args.iter().position(|a| matches!(a, Calcit::Syntax(CalcitSyntax::ArgSpread, _)));

      if let Some(pos) = spread_pos {
        // Spread recur: unpack rest list into remaining param slots.
        let explicit = &args[..pos];
        let rest_expr = args.get(pos + 1).ok_or("recur spread: missing list after &")?;
        let total = ctx.arg_indices.len();
        let n_explicit = explicit.len();
        if n_explicit >= total {
          return Err(format!("recur spread: too many explicit args ({n_explicit} >= {total})"));
        }
        let n_remaining = total - n_explicit; // includes rest-list slot

        // Evaluate spread list into a temp local.
        emit_expr(ctx, rest_expr)?;
        let spread_f64 = ctx.alloc_local();
        ctx.emit(Instruction::LocalSet(spread_f64));
        let spread_i32 = ctx.alloc_local_typed(ValType::I32);
        ctx.emit(Instruction::LocalGet(spread_f64));
        ctx.emit(Instruction::I32TruncF64U);
        ctx.emit(Instruction::LocalSet(spread_i32));

        // Evaluate explicit prefix args into temps.
        let mut temps = Vec::new();
        for arg in explicit {
          let tmp = ctx.alloc_local();
          emit_expr(ctx, arg)?;
          ctx.emit(Instruction::LocalSet(tmp));
          temps.push(tmp);
        }

        // Extract (n_remaining - 1) individual elements from spread list for the remaining fixed slots.
        for i in 0..(n_remaining - 1) {
          let tmp = ctx.alloc_local();
          ctx.emit(Instruction::LocalGet(spread_i32));
          ctx.emit(Instruction::I32Const(((1 + i) * 8) as i32));
          ctx.emit(Instruction::I32Add);
          ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
          ctx.emit(Instruction::LocalSet(tmp));
          temps.push(tmp);
        }

        // Last slot: list_slice(spread, n_remaining-1) — the new rest list.
        let rest_slice_local = ctx.alloc_local();
        emit_list_slice_from_i32_local(ctx, spread_i32, n_remaining - 1)?;
        ctx.emit(Instruction::LocalSet(rest_slice_local));
        temps.push(rest_slice_local);

        if temps.len() != total {
          return Err(format!("recur spread: computed {} temps but need {}", temps.len(), total));
        }

        // Copy temps into arg locals.
        for (i, &tmp) in temps.iter().enumerate() {
          ctx.emit(Instruction::LocalGet(tmp));
          ctx.emit(Instruction::LocalSet(ctx.arg_indices[i]));
        }
        ctx.emit(Instruction::Br(ctx.block_depth));
        ctx.emit(Instruction::Unreachable);
        Ok(())
      } else {
        // Non-spread recur.
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
    }

    // Record operations
    CalcitProc::NativeRecord => emit_record_new(ctx, args),
    CalcitProc::NativeRecordNth => emit_record_nth(ctx, args),
    CalcitProc::NativeRecordGet => emit_record_get(ctx, args),
    CalcitProc::NativeRecordCount => emit_record_count(ctx, args),
    CalcitProc::NativeRecordFieldTag => emit_record_field_tag(ctx, args),
    CalcitProc::NativeRecordStruct => emit_record_struct(ctx, args),
    CalcitProc::NativeRecordGetName => emit_record_get_name(ctx, args),
    CalcitProc::NativeRecordToMap => emit_record_to_map(ctx, args),
    CalcitProc::NativeRecordAssoc | CalcitProc::NativeRecordAssocAt | CalcitProc::NativeRecordWith => {
      Err(format!("{proc} not yet supported in WASM codegen"))
    }
    CalcitProc::NativeRecordFromMap
    | CalcitProc::NativeRecordExtendAs
    | CalcitProc::NativeRecordPartial
    | CalcitProc::NativeRecordImpls
    | CalcitProc::NativeRecordWithAt
    | CalcitProc::NativeLooseRecord => Err(format!("Record operation {proc} not yet supported in WASM codegen")),
    CalcitProc::NativeRecordContains => emit_record_contains(ctx, args),
    CalcitProc::NativeRecordMatches => emit_record_matches(ctx, args),

    // Tuple operations
    CalcitProc::NativeTuple => emit_tuple_new(ctx, args),
    CalcitProc::NativeTupleNth => emit_tuple_nth(ctx, args),
    CalcitProc::NativeTupleCount => emit_tuple_count(ctx, args),
    CalcitProc::NativeTupleValidateEnum => ctx.stub_proc(args), // no-op in WASM
    // %:: enum variant constructor: (enum_class tag payload...) — ignore enum_class
    CalcitProc::NativeEnumTupleNew => emit_enum_tuple_new(ctx, args),
    CalcitProc::NativeTupleImpls
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
      expect_arity(1, args, "bit-not")?;
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
    CalcitProc::NativeListAppend => emit_list_append(ctx, args),
    CalcitProc::NativeListPrepend => emit_list_prepend(ctx, args),
    CalcitProc::NativeListButlast => emit_list_butlast(ctx, args),
    CalcitProc::NativeListLast => emit_list_last(ctx, args),
    CalcitProc::NativeListSort => {
      // Sort is not yet implemented in WASM — pass list through as stub
      if args.is_empty() {
        ctx.emit(f64_const(0.0));
      } else {
        emit_expr(ctx, &args[0])?;
      }
      Ok(())
    }
    CalcitProc::NativeListRange => emit_range(ctx, args),
    CalcitProc::NativeListFoldl => emit_foldl(ctx, args),
    CalcitProc::NativeListFoldlShortcut => emit_foldl_shortcut(ctx, args),
    CalcitProc::NativeListCount => emit_ds_count(ctx, args),
    CalcitProc::NativeListNth => emit_list_nth(ctx, args),
    CalcitProc::NativeListFirst => emit_list_first(ctx, args),
    CalcitProc::NativeListRest => emit_list_rest(ctx, args),
    CalcitProc::NativeListEmpty => emit_ds_empty(ctx, args),
    CalcitProc::NativeListSlice => emit_list_slice(ctx, args),
    CalcitProc::NativeListReverse => emit_list_reverse(ctx, args),
    CalcitProc::NativeListConcat => emit_list_concat(ctx, args),
    CalcitProc::NativeListAssoc => emit_list_assoc(ctx, args),
    CalcitProc::NativeListAssocBefore => emit_list_assoc_before(ctx, args),
    CalcitProc::NativeListAssocAfter => emit_list_assoc_after(ctx, args),
    CalcitProc::NativeListDissoc => emit_list_dissoc(ctx, args),
    CalcitProc::NativeListToSet => emit_list_to_set(ctx, args),
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
    CalcitProc::NativeSetToList => emit_set_to_list(ctx, args),
    CalcitProc::NativeDifference => emit_set_difference(ctx, args),
    CalcitProc::NativeUnion => emit_set_union(ctx, args),
    CalcitProc::NativeSetIntersection => emit_set_intersection(ctx, args),
    CalcitProc::NativeSetDestruct => emit_set_destruct(ctx, args),
    CalcitProc::NativeMerge => emit_map_merge(ctx, args),
    CalcitProc::NativeMergeNonNil => emit_map_merge_non_nil(ctx, args),
    CalcitProc::NativeMapDiffNew => emit_map_diff_new(ctx, args),
    CalcitProc::NativeMapDiffKeys => emit_map_diff_keys(ctx, args),
    CalcitProc::NativeMapCommonKeys => emit_map_common_keys(ctx, args),
    CalcitProc::NativeMapDiffTriple => maps::emit_map_diff_triple(ctx, args),
    CalcitProc::NativeMapDestruct => emit_map_destruct(ctx, args),
    CalcitProc::NativeMapKeys => emit_map_keys(ctx, args),
    CalcitProc::NativeMapVals => emit_map_vals(ctx, args),
    CalcitProc::Range => emit_range(ctx, args),
    CalcitProc::NativeHash => emit_hash_proc(ctx, args),

    // ------- String operations -------
    CalcitProc::NativeStrCount => emit_str_count(ctx, args),
    CalcitProc::NativeStrEmpty => emit_str_empty(ctx, args),
    CalcitProc::NativeStrConcat => emit_str_concat(ctx, args),
    CalcitProc::NativeStrNth => emit_str_nth(ctx, args),
    CalcitProc::NativeStrFirst => emit_str_first(ctx, args),
    CalcitProc::NativeStrRest => emit_str_rest(ctx, args),
    CalcitProc::NativeStrSlice => emit_str_slice(ctx, args),
    CalcitProc::NativeStrCompare => emit_str_compare(ctx, args),
    CalcitProc::NativeStrContains => emit_str_contains(ctx, args),
    CalcitProc::NativeStrIncludes => emit_str_includes(ctx, args),
    CalcitProc::NativeStrFindIndex => emit_str_find_index(ctx, args),
    CalcitProc::NativeStrPadLeft => emit_str_pad_left(ctx, args),
    CalcitProc::NativeStrPadRight => emit_str_pad_right(ctx, args),
    CalcitProc::StartsWith => emit_str_starts_with(ctx, args),
    CalcitProc::EndsWith => emit_str_ends_with(ctx, args),
    CalcitProc::TurnString | CalcitProc::NativeStr => emit_turn_string(ctx, args),
    CalcitProc::Trim => emit_trim(ctx, args),
    CalcitProc::IsBlank => emit_blank(ctx, args),
    CalcitProc::GetCharCode => emit_get_char_code(ctx, args),
    CalcitProc::ParseFloat => emit_parse_float(ctx, args),
    CalcitProc::CharFromCode => emit_char_from_code(ctx, args),
    CalcitProc::NativeStrReplace => emit_str_replace(ctx, args),
    CalcitProc::NativeStrEscape => emit_str_escape(ctx, args),
    CalcitProc::Split => emit_split(ctx, args),
    CalcitProc::SplitLines => emit_split_lines(ctx, args),

    // --- List higher-order and utility operations ---
    CalcitProc::NativeListDistinct => emit_list_distinct(ctx, args),

    // Higher-order list operations
    CalcitProc::Foldl => emit_foldl(ctx, args),
    CalcitProc::FoldlShortcut => emit_foldl_shortcut(ctx, args),
    CalcitProc::FoldrShortcut => emit_foldr_shortcut(ctx, args),

    // Format (stub — only used in raise/error paths)
    CalcitProc::FormatToLisp => emit_format_to_lisp(ctx, args),
    // to-lispy-string — stub, only used in raise/error message paths
    CalcitProc::PrStr => ctx.stub_proc(args),
    // get-env — returns nil in WASM (env vars not available)
    CalcitProc::GetEnv => ctx.stub_proc(args),

    // @atom deref: just emit the argument (which should already be a GlobalGet)
    CalcitProc::AtomDeref => {
      expect_arity(1, args, "&atom:deref")?;
      emit_expr(ctx, &args[0])
    }

    // quit! — trap (abort) the WASM instance
    CalcitProc::Quit => {
      ctx.emit(Instruction::Unreachable);
      ctx.emit(f64_const(0.0)); // unreachable, but keeps type stack valid
      Ok(())
    }

    // &get-calcit-backend — runtime-env query, not meaningful in WASM; return nil.
    CalcitProc::NativeGetCalcitBackend => {
      ctx.emit(f64_const(0.0));
      Ok(())
    }

    // turn-tag — converts a string to a tag at runtime; in WASM, return the string as-is.
    CalcitProc::TurnTag => {
      expect_arity(1, args, "turn-tag")?;
      emit_expr(ctx, &args[0])
    }

    // Runtime trait tables are intentionally not implemented in the internal
    // WASM validation backend. Preprocessing must eliminate these operations.
    CalcitProc::NativeStructImplTraits | CalcitProc::NativeEnumImplTraits => {
      Err("runtime trait registration must be eliminated before WASM codegen".into())
    }

    // register-calcit-builtin-impls — builtin impl registration; not meaningful in WASM; return nil.
    CalcitProc::RegisterCalcitBuiltinImpls => {
      eprintln!("[wasm warning] RegisterCalcitBuiltinImpls is ignored in WASM (builtin impls already registered)");
      ctx.silent_nil()
    }

    // &impl::new — only valid as compile-time metadata for WASM.
    CalcitProc::NativeImplNew => Err("runtime trait impl construction must be eliminated before WASM codegen".into()),

    // &assert-traits — a residual assertion would require a runtime trait table.
    CalcitProc::NativeAssertTraits => Err(
      "runtime trait assertions are not supported by the internal WASM backend; make the receiver type statically resolvable".into(),
    ),

    // &get-os — host OS info; not available in WASM; return nil.
    CalcitProc::NativeGetOs => ctx.stub_proc(args),

    // definition metadata — not available in WASM; return nil.
    CalcitProc::NativeGetDefDoc | CalcitProc::NativeGetDefSchema => ctx.stub_proc(args),

    // &number:display-by — radix string formatting.
    CalcitProc::NativeNumberDisplayBy => {
      if args.len() != 2 {
        return Err("&number:display-by expects 2 args".into());
      }
      emit_expr(ctx, &args[0])?; // value f64
      emit_expr(ctx, &args[1])?; // radix f64
      ctx.call_rt("__rt_display_by");
      Ok(())
    }

    // &number:format — formatting; stub in WASM.
    CalcitProc::NativeNumberFormat => ctx.stub_proc(args),

    // sort — native sort with optional comparator; not yet supported in WASM; return first arg.
    CalcitProc::Sort => {
      if args.is_empty() {
        ctx.emit(f64_const(0.0));
      } else {
        // Skip any non-constant args (comparator lambda). Just pass the source list through
        // as a trivial (unsorted) stub so the function compiles.
        emit_expr(ctx, &args[0])?;
      }
      Ok(())
    }

    // Not yet supported
    _ => Err(format!("unsupported proc in WASM: {proc}")),
  }
}

fn emit_unary(ctx: &mut WasmGenCtx, instr: Instruction<'static>, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    // keep explicit for clarity in math ops
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

/// Emit `=` with string content equality support.
///
/// Semantics:
/// - Fast path: `a == b` on raw f64 representation.
/// - If not equal and both look like heap strings, compare by `__rt_str_compare`.
/// - Otherwise false.
fn emit_equals(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err(format!("= expects 2 args, got {}", args.len()));
  }

  let a = ctx.alloc_local();
  let b = ctx.alloc_local();

  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(a));
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(b));

  let result = emit_equals_core(ctx, a, b)?;
  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

/// Core structural equality check given two pre-evaluated f64 locals.
/// Returns the f64 result local (0.0 = not equal, 1.0 = equal).
/// Uses structural set comparison (inline loop with shallow element equality).
#[allow(private_interfaces)]
pub(super) fn emit_equals_core(ctx: &mut WasmGenCtx, a: u32, b: u32) -> Result<u32, String> {
  emit_equals_core_impl(ctx, a, b, true)
}

/// Shallow version of emit_equals_core: for set comparison, uses __rt_set_find_elem
/// (pointer equality only). Call this from within emit_set_find_structural to
/// avoid infinite mutual recursion.
#[allow(private_interfaces)]
pub(super) fn emit_equals_core_shallow(ctx: &mut WasmGenCtx, a: u32, b: u32) -> Result<u32, String> {
  emit_equals_core_impl(ctx, a, b, false)
}

/// Internal implementation. When `structural_sets` is true, the set comparison
/// uses an inline structural search (calling `emit_equals_core_impl` with
/// `structural_sets=false` for elements, preventing infinite recursion).
/// When false, uses __rt_set_find_elem (F64Eq for elements).
fn emit_equals_core_impl(ctx: &mut WasmGenCtx, a: u32, b: u32, structural_sets: bool) -> Result<u32, String> {
  let result = ctx.alloc_local(); // 0.0 = not equal, 1.0 = equal

  let string_tag = *ctx.tag_index.get("string").ok_or("string tag not found")? as i32;
  let list_tag = *ctx.tag_index.get("list").ok_or("list tag not found")? as i32;
  let set_tag = *ctx.tag_index.get("set").ok_or("set tag not found")? as i32;
  let map_tag = *ctx.tag_index.get("map").ok_or("map tag not found")? as i32;
  let rt_str_compare = *ctx
    .runtime_fn_index
    .get("__rt_str_compare")
    .ok_or("runtime helper __rt_str_compare not found")?;
  let rt_set_find_elem = *ctx
    .runtime_fn_index
    .get("__rt_set_find_elem")
    .ok_or("runtime helper __rt_set_find_elem not found")?;
  let rt_map_equal = *ctx
    .runtime_fn_index
    .get("__rt_map_equal")
    .ok_or("runtime helper __rt_map_equal not found")?;

  // Default: not equal
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalSet(result));

  // --- Fast path: exact f64 equality ---
  ctx.emit(Instruction::LocalGet(a));
  ctx.emit(Instruction::LocalGet(b));
  ctx.emit(Instruction::F64Eq);
  ctx.begin_block_if();
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::End);

  // Skip heap comparison if already equal
  ctx.emit(Instruction::LocalGet(result));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Eq);
  ctx.begin_block_if(); // outer: only proceed if result is still 0.0

  // --- Check both values are heap pointers ---
  // A valid heap ptr must be >= HEAP_BASE+8 AND < memory_size (in bytes).
  // We use memory.size * 65536 as the upper bound to prevent OOB on large f64
  // values (e.g., from &hash) that pass the lower-bound check but exceed memory.
  let ptr_a = ctx.alloc_local_typed(ValType::I32);
  let ptr_b = ctx.alloc_local_typed(ValType::I32);
  let both_heap = ctx.alloc_local_typed(ValType::I32);
  let mem_size_f64 = ctx.alloc_local();

  // mem_size_f64 = f64(memory.size * 65536)
  ctx.emit(Instruction::MemorySize(0));
  ctx.emit(Instruction::I32Const(16)); // 2^16 = 65536
  ctx.emit(Instruction::I32Shl);
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(mem_size_f64));

  ctx.emit(Instruction::LocalGet(a));
  ctx.emit(f64_const((HEAP_BASE + 8) as f64));
  ctx.emit(Instruction::F64Ge);
  ctx.emit(Instruction::LocalGet(a));
  ctx.emit(Instruction::LocalGet(mem_size_f64));
  ctx.emit(Instruction::F64Lt);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalGet(b));
  ctx.emit(f64_const((HEAP_BASE + 8) as f64));
  ctx.emit(Instruction::F64Ge);
  ctx.emit(Instruction::LocalGet(b));
  ctx.emit(Instruction::LocalGet(mem_size_f64));
  ctx.emit(Instruction::F64Lt);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalSet(both_heap));

  ctx.emit(Instruction::LocalGet(both_heap));
  ctx.begin_block_if(); // only proceed if both are heap ptrs

  ctx.emit(Instruction::LocalGet(a));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(ptr_a));
  ctx.emit(Instruction::LocalGet(b));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(ptr_b));

  // Read type tags
  let tag_a = ctx.alloc_local_typed(ValType::I32);
  let tag_b = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_a));
  ctx.emit(Instruction::I32Const(4));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::LocalSet(tag_a));
  ctx.emit(Instruction::LocalGet(ptr_b));
  ctx.emit(Instruction::I32Const(4));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::LocalSet(tag_b));

  // Only compare if same type
  ctx.emit(Instruction::LocalGet(tag_a));
  ctx.emit(Instruction::LocalGet(tag_b));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();

  // --- String comparison ---
  ctx.emit(Instruction::LocalGet(tag_a));
  ctx.emit(Instruction::I32Const(string_tag));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(ptr_a));
  ctx.emit(Instruction::LocalGet(ptr_b));
  ctx.emit(Instruction::Call(rt_str_compare));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Eq); // str_compare returns f64; 0.0 = equal
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::End); // end string if

  // Skip if already resolved
  ctx.emit(Instruction::LocalGet(result));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::LocalGet(tag_a));
  ctx.emit(Instruction::I32Const(string_tag));
  ctx.emit(Instruction::I32Ne);
  ctx.emit(Instruction::I32And);
  ctx.begin_block_if();

  // --- List comparison: same count + each element F64Eq ---
  ctx.emit(Instruction::LocalGet(tag_a));
  ctx.emit(Instruction::I32Const(list_tag));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  {
    let cnt_a = ctx.alloc_local_typed(ValType::I32);
    let cnt_b = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(ptr_a));
    ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(cnt_a));
    ctx.emit(Instruction::LocalGet(ptr_b));
    ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(cnt_b));

    // If counts match, compare element-by-element
    ctx.emit(Instruction::LocalGet(cnt_a));
    ctx.emit(Instruction::LocalGet(cnt_b));
    ctx.emit(Instruction::I32Eq);
    ctx.begin_block_if();
    {
      let all_eq = ctx.alloc_i32(1); // assume equal
      let li = ctx.alloc_i32(0);
      ctx.begin_block();
      ctx.begin_loop();
      ctx.loop_exit_if_ge(li, cnt_a);
      // elem_a = ptr_a[(1+li)*8], elem_b = ptr_b[(1+li)*8]
      let offset_a = ctx.alloc_local_typed(ValType::I32);
      let offset_b = ctx.alloc_local_typed(ValType::I32);
      ctx.emit(Instruction::LocalGet(ptr_a));
      ctx.emit(Instruction::I32Const(8));
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::LocalGet(li));
      ctx.emit(Instruction::I32Const(8));
      ctx.emit(Instruction::I32Mul);
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::LocalSet(offset_a));
      ctx.emit(Instruction::LocalGet(ptr_b));
      ctx.emit(Instruction::I32Const(8));
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::LocalGet(li));
      ctx.emit(Instruction::I32Const(8));
      ctx.emit(Instruction::I32Mul);
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::LocalSet(offset_b));
      // Deep element comparison: f64 fast-path, then inline string comparison.
      let elem_a = ctx.alloc_local(); // f64
      let elem_b = ctx.alloc_local(); // f64
      let elems_eq = ctx.alloc_i32(0); // i32: 0=not-equal, 1=equal
      let elem_a_i32 = ctx.alloc_local_typed(ValType::I32);
      let elem_b_i32 = ctx.alloc_local_typed(ValType::I32);
      let elem_tag_a = ctx.alloc_local_typed(ValType::I32);
      let elem_tag_b = ctx.alloc_local_typed(ValType::I32);
      ctx.emit(Instruction::LocalGet(offset_a));
      ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
      ctx.emit(Instruction::LocalSet(elem_a));
      ctx.emit(Instruction::LocalGet(offset_b));
      ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
      ctx.emit(Instruction::LocalSet(elem_b));
      // Fast path: pointer equality
      ctx.emit(Instruction::LocalGet(elem_a));
      ctx.emit(Instruction::LocalGet(elem_b));
      ctx.emit(Instruction::F64Eq);
      ctx.begin_block_if();
      ctx.emit(Instruction::I32Const(1));
      ctx.emit(Instruction::LocalSet(elems_eq));
      ctx.emit(Instruction::Else);
      // Check if both are valid heap pointers
      let heap_min_elem = (HEAP_BASE + 8) as f64;
      ctx.emit(Instruction::LocalGet(elem_a));
      ctx.emit(f64_const(heap_min_elem));
      ctx.emit(Instruction::F64Ge);
      ctx.emit(Instruction::LocalGet(elem_b));
      ctx.emit(f64_const(heap_min_elem));
      ctx.emit(Instruction::F64Ge);
      ctx.emit(Instruction::I32And);
      ctx.begin_block_if();
      ctx.emit(Instruction::LocalGet(elem_a));
      ctx.emit(Instruction::I32TruncF64U);
      ctx.emit(Instruction::LocalSet(elem_a_i32));
      ctx.emit(Instruction::LocalGet(elem_b));
      ctx.emit(Instruction::I32TruncF64U);
      ctx.emit(Instruction::LocalSet(elem_b_i32));
      // Read type tags
      ctx.emit(Instruction::LocalGet(elem_a_i32));
      ctx.emit(Instruction::I32Const(4));
      ctx.emit(Instruction::I32Sub);
      ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
      ctx.emit(Instruction::LocalSet(elem_tag_a));
      ctx.emit(Instruction::LocalGet(elem_b_i32));
      ctx.emit(Instruction::I32Const(4));
      ctx.emit(Instruction::I32Sub);
      ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
      ctx.emit(Instruction::LocalSet(elem_tag_b));
      // Both strings? Call str_compare
      ctx.emit(Instruction::LocalGet(elem_tag_a));
      ctx.emit(Instruction::I32Const(string_tag));
      ctx.emit(Instruction::I32Eq);
      ctx.emit(Instruction::LocalGet(elem_tag_b));
      ctx.emit(Instruction::I32Const(string_tag));
      ctx.emit(Instruction::I32Eq);
      ctx.emit(Instruction::I32And);
      ctx.begin_block_if();
      ctx.emit(Instruction::LocalGet(elem_a_i32));
      ctx.emit(Instruction::LocalGet(elem_b_i32));
      ctx.emit(Instruction::Call(rt_str_compare));
      ctx.emit(f64_const(0.0));
      ctx.emit(Instruction::F64Eq); // 1 if equal
      ctx.emit(Instruction::LocalSet(elems_eq));
      ctx.emit(Instruction::End); // string if
      ctx.emit(Instruction::End); // both-heap if
      ctx.emit(Instruction::End); // fast-path if
      // if NOT elems_eq → all_eq = 0, break
      ctx.emit(Instruction::LocalGet(elems_eq));
      ctx.emit(Instruction::I32Eqz);
      ctx.begin_block_if();
      // not equal → mark all_eq = 0 and break
      ctx.emit(Instruction::I32Const(0));
      ctx.emit(Instruction::LocalSet(all_eq));
      ctx.emit(Instruction::Br(2)); // break out of block
      ctx.emit(Instruction::End);
      ctx.i32_inc(li);
      ctx.emit(Instruction::Br(0));
      ctx.emit(Instruction::End); // loop
      ctx.emit(Instruction::End); // block
      // result = all_eq
      ctx.emit(Instruction::LocalGet(all_eq));
      ctx.emit(Instruction::F64ConvertI32U);
      ctx.emit(Instruction::LocalSet(result));
    }
    ctx.emit(Instruction::End); // count eq if
  }
  ctx.emit(Instruction::End); // list if

  // --- Set comparison: same count + every elem of A is in B ---
  ctx.emit(Instruction::LocalGet(tag_a));
  ctx.emit(Instruction::I32Const(set_tag));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  {
    let cnt_a = ctx.alloc_local_typed(ValType::I32);
    let cnt_b = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(ptr_a));
    ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(cnt_a));
    ctx.emit(Instruction::LocalGet(ptr_b));
    ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(cnt_b));

    ctx.emit(Instruction::LocalGet(cnt_a));
    ctx.emit(Instruction::LocalGet(cnt_b));
    ctx.emit(Instruction::I32Eq);
    ctx.begin_block_if();
    {
      let all_eq = ctx.alloc_i32(1);
      let si = ctx.alloc_i32(0);
      ctx.begin_block();
      ctx.begin_loop();
      ctx.loop_exit_if_ge(si, cnt_a);
      // load elem from set A
      let offset_a = ctx.alloc_local_typed(ValType::I32);
      ctx.emit(Instruction::LocalGet(ptr_a));
      ctx.emit(Instruction::I32Const(8));
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::LocalGet(si));
      ctx.emit(Instruction::I32Const(8));
      ctx.emit(Instruction::I32Mul);
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::LocalSet(offset_a));
      ctx.emit(Instruction::LocalGet(offset_a));
      ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
      let elem_a = ctx.alloc_local();
      ctx.emit(Instruction::LocalSet(elem_a));
      // check elem_a in set B
      let elem_not_found = if structural_sets {
        // Inline structural search: iterate B and compare with shallow equality
        let bj = ctx.alloc_i32(0);
        let found_b = ctx.alloc_i32(-1);
        ctx.begin_block();
        ctx.begin_loop();
        ctx.loop_exit_if_ge(bj, cnt_b);
        let b_off = ctx.alloc_local_typed(ValType::I32);
        ctx.emit(Instruction::LocalGet(ptr_b));
        ctx.emit(Instruction::I32Const(8));
        ctx.emit(Instruction::I32Add);
        ctx.emit(Instruction::LocalGet(bj));
        ctx.emit(Instruction::I32Const(8));
        ctx.emit(Instruction::I32Mul);
        ctx.emit(Instruction::I32Add);
        ctx.emit(Instruction::LocalSet(b_off));
        let b_elem = ctx.alloc_local();
        ctx.emit(Instruction::LocalGet(b_off));
        ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
        ctx.emit(Instruction::LocalSet(b_elem));
        let eq_r = emit_equals_core_impl(ctx, elem_a, b_elem, false)?;
        ctx.emit(Instruction::LocalGet(eq_r));
        ctx.emit(f64_const(0.0));
        ctx.emit(Instruction::F64Ne);
        ctx.begin_block_if();
        ctx.emit(Instruction::LocalGet(bj));
        ctx.emit(Instruction::LocalSet(found_b));
        ctx.emit(Instruction::Br(2));
        ctx.emit(Instruction::End);
        ctx.i32_inc(bj);
        ctx.emit(Instruction::Br(0));
        ctx.emit(Instruction::End); // loop
        ctx.emit(Instruction::End); // block
        // found_b is -1 if not found, or index if found
        ctx.emit(Instruction::LocalGet(found_b));
        ctx.emit(Instruction::I32Const(-1));
        ctx.emit(Instruction::I32Eq); // 1 if not found, 0 if found
        let not_found_flag = ctx.alloc_local_typed(ValType::I32);
        ctx.emit(Instruction::LocalSet(not_found_flag));
        not_found_flag
      } else {
        // Shallow: use __rt_set_find_elem (F64Eq for elements)
        ctx.emit(Instruction::LocalGet(ptr_b));
        ctx.emit(Instruction::LocalGet(elem_a));
        ctx.emit(Instruction::Call(rt_set_find_elem));
        ctx.emit(Instruction::I32Const(-1));
        ctx.emit(Instruction::I32Eq); // 1 if not found, 0 if found
        let not_found_flag = ctx.alloc_local_typed(ValType::I32);
        ctx.emit(Instruction::LocalSet(not_found_flag));
        not_found_flag
      };
      ctx.emit(Instruction::LocalGet(elem_not_found));
      ctx.begin_block_if();
      ctx.emit(Instruction::I32Const(0));
      ctx.emit(Instruction::LocalSet(all_eq));
      ctx.emit(Instruction::Br(2)); // break
      ctx.emit(Instruction::End);
      ctx.i32_inc(si);
      ctx.emit(Instruction::Br(0));
      ctx.emit(Instruction::End); // loop
      ctx.emit(Instruction::End); // block
      ctx.emit(Instruction::LocalGet(all_eq));
      ctx.emit(Instruction::F64ConvertI32U);
      ctx.emit(Instruction::LocalSet(result));
    }
    ctx.emit(Instruction::End); // count eq if
  }
  ctx.emit(Instruction::End); // set if

  // --- Map comparison: use __rt_map_equal runtime helper ---
  ctx.emit(Instruction::LocalGet(result));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::LocalGet(tag_a));
  ctx.emit(Instruction::I32Const(map_tag));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::I32And);
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(ptr_a));
  ctx.emit(Instruction::LocalGet(ptr_b));
  ctx.emit(Instruction::Call(rt_map_equal));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::End); // end map if

  ctx.emit(Instruction::End); // "not yet resolved" if
  ctx.emit(Instruction::End); // "same type tag" if
  ctx.emit(Instruction::End); // "both heap" if
  ctx.emit(Instruction::End); // "result is still 0.0" outer if

  Ok(result)
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

/// Handle `calcit.core/let` in call position: `(let ((name val)...) body...)`.
/// First arg is a list of binding pairs; remaining args are the body expressions.
fn emit_let_multi(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.is_empty() {
    ctx.emit(f64_const(0.0));
    return Ok(());
  }
  let Calcit::List(pairs_list) = &args[0] else {
    return Err(format!("let expects a list of binding pairs, got: {}", args[0]));
  };
  let body = &args[1..];
  emit_let_pairs(ctx, &pairs_list.to_vec(), body)
}

fn emit_let_pairs(ctx: &mut WasmGenCtx, pairs: &[Calcit], body: &[Calcit]) -> Result<(), String> {
  if pairs.is_empty() {
    return emit_body(ctx, body);
  }
  let pair = &pairs[0];
  let Calcit::List(xs) = pair else {
    return Err(format!("let binding expects a pair, got: {pair}"));
  };
  if xs.len() != 2 {
    return Err(format!("let binding pair must have 2 elements, got {}", xs.len()));
  }
  let var_name = match &xs[0] {
    Calcit::Local(CalcitLocal { sym, .. }) => sym.to_string(),
    Calcit::Symbol { sym, .. } => sym.to_string(),
    other => return Err(format!("let binding expected symbol, got: {other}")),
  };
  emit_expr(ctx, &xs[1])?;
  let idx = ctx.declare_local(&var_name);
  ctx.emit(Instruction::LocalSet(idx));
  emit_let_pairs(ctx, &pairs[1..], body)
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

      // Check if the binding value is an inline lambda.
      // If so, store it for inlining at call sites instead of emitting as runtime value.
      if let Some((params, body)) = try_extract_inline_lambda(&xs[1]) {
        ctx.lambda_locals.insert(var_name.clone(), (params, body));
        // Allocate a local slot (unused at runtime) so shadowing cleanup works.
        ctx.declare_local(&var_name);
        // Flatten nested lets
        if rest.len() == 1
          && let Calcit::List(inner) = &rest[0]
          && let Some(Calcit::Syntax(CalcitSyntax::CoreLet, _)) = inner.first()
        {
          let inner_body: Vec<Calcit> = inner.drop_left().to_vec();
          return emit_let(ctx, &inner_body);
        }
        return emit_body(ctx, rest);
      }

      emit_expr(ctx, &xs[1])?;
      let idx = ctx.declare_local(&var_name);
      ctx.emit(Instruction::LocalSet(idx));

      // Flatten nested lets
      if rest.len() == 1
        && let Calcit::List(inner) = &rest[0]
        && let Some(Calcit::Syntax(CalcitSyntax::CoreLet, _)) = inner.first()
      {
        let inner_body: Vec<Calcit> = inner.drop_left().to_vec();
        return emit_let(ctx, &inner_body);
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
    data.extend(std::iter::repeat_n(0u8, (padded_len - byte_len) as usize));
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

  for file_info in program_data.values() {
    for compiled in file_info.defs.values() {
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

/// If `expr` is a literal tuple constructor `(NativeTuple :tag val0 val1...)` with only
/// literal args (Tag, Str, Number, Bool, Nil), return its lispy string representation.
/// Used both to pre-intern the string and to emit it as a constant in `emit_turn_string`.
pub(crate) fn try_format_tuple_literal(expr: &Calcit) -> Option<String> {
  if let Calcit::List(list) = expr
    && !list.is_empty()
    && let Calcit::Proc(p) = &list[0]
    && *p == CalcitProc::NativeTuple
  {
    let mut s = String::from("(:: ");
    for (i, item) in list.iter().skip(1).enumerate() {
      if i > 0 {
        s.push(' ');
      }
      match item {
        Calcit::Tag(_) | Calcit::Str(_) | Calcit::Number(_) | Calcit::Bool(_) | Calcit::Nil => {
          use std::fmt::Write;
          write!(s, "{item}").ok()?;
        }
        _ => return None,
      }
    }
    s.push(')');
    return Some(s);
  }
  None
}

fn collect_strings_from_expr(expr: &Calcit, strings: &mut Vec<String>) {
  match expr {
    Calcit::Str(s) => {
      strings.push(s.to_string());
    }
    // Intern tag names as strings so that emit_ptr_to_i32 can convert tags to string ptrs
    // (used when literal tags are passed to string procs like starts-with?, ends-with?, etc.)
    Calcit::Tag(t) => {
      strings.push(t.to_string());
    }
    Calcit::List(xs) => {
      // Pre-intern strings produced by `(format-to-lisp (quote X))` at compile time.
      // The assert= macro expands to this pattern for the error message.
      if xs.len() == 2
        && let Calcit::Proc(p) = &xs[0]
        && matches!(p.as_ref(), "format-to-lisp")
        && let Calcit::List(inner) = &xs[1]
        && inner.len() >= 2
        && let Calcit::Syntax(CalcitSyntax::Quote, _) = &inner[0]
      {
        let s = crate::calcit::format_to_lisp(&inner[1]);
        strings.push(s);
      }
      // Pre-intern lispy strings for literal tuple constructors (used by `str`/`turn-string`).
      if let Some(tuple_str) = try_format_tuple_literal(expr) {
        strings.push(tuple_str);
      }
      for x in xs.iter() {
        collect_strings_from_expr(x, strings);
      }
    }
    _ => {}
  }
}
