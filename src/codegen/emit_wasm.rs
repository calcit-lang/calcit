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

#[path = "emit_wasm/methods.rs"]
mod methods;
#[path = "emit_wasm/records.rs"]
mod records;
#[path = "emit_wasm/runtime.rs"]
mod runtime;

use methods::{emit_call_args, emit_method_invoke};
use records::{
  emit_record_count, emit_record_field_tag, emit_record_get, emit_record_get_name, emit_record_matches, emit_record_new,
  emit_record_nth, emit_record_struct, emit_record_to_map, emit_tuple_assoc, emit_tuple_count, emit_tuple_new, emit_tuple_nth,
  resolve_struct_ref, try_parse_defrecord_form,
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
  for (i, (ns, name, args, _)) in fn_defs.iter().enumerate() {
    let idx = num_imports + runtime_fn_count + i as u32;
    let qualified = format!("{ns}/{name}");
    fn_index.insert(qualified.clone(), idx);
    fn_index.insert(name.clone(), idx);
    let (arity, rest_fixed) = compute_fn_arity(args);
    fn_arity.insert(qualified.clone(), arity);
    fn_arity.insert(name.clone(), arity);
    if let Some(fixed) = rest_fixed {
      fn_has_rest.insert(qualified, fixed);
      fn_has_rest.insert(name.clone(), fixed);
    }
  }

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
      if let Ok(struct_def) = resolve_struct_ref(expr) {
        let tag_str = struct_def.name.to_string();
        let id = *ctx
          .tag_index
          .get(&tag_str)
          .ok_or_else(|| format!("unknown struct tag in WASM codegen: {tag_str}"))?;
        ctx.emit(f64_const(id as f64));
      } else {
        return Err(format!("unsupported WASM expression: {}/{}", import.ns, import.def));
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
      // High-level set wrappers (union/difference/include) use reduce+lambda which can't be
      // compiled to WASM directly. Intercept and inline native 2+ arg set operations.
      if import.ns.as_ref() == "calcit.core" {
        match import.def.as_ref() {
          "union" => return emit_set_op_variadic(ctx, &args_list, SetOpKind::Union),
          "difference" => return emit_set_op_variadic(ctx, &args_list, SetOpKind::Difference),
          "include" => return emit_set_op_variadic(ctx, &args_list, SetOpKind::Include),
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
    _ => Err(format!("unsupported call head in WASM: {head}")),
  }
}

fn emit_call_spread_args(ctx: &mut WasmGenCtx, call_args: &[Calcit], target_arity: u32, rest_fixed: Option<u32>) -> Result<(), String> {
  let Some(fixed) = rest_fixed else {
    return Err("&call-spread in WASM currently requires the target function to accept rest args".into());
  };

  let fixed = fixed as usize;
  let spread_arg = if call_args.len() == fixed + 1 {
    &call_args[fixed]
  } else if call_args.len() == fixed + 2 && matches!(call_args[fixed], Calcit::Syntax(CalcitSyntax::ArgSpread, _)) {
    &call_args[fixed + 1]
  } else {
    return Err(format!(
      "&call-spread in WASM expects {} fixed args plus `& spread-list`, got {} args",
      fixed,
      call_args.len()
    ));
  };

  for arg in call_args.iter().take(fixed) {
    emit_expr(ctx, arg)?;
  }
  emit_expr(ctx, spread_arg)?;

  let emitted_args = fixed + 1;
  for _ in emitted_args..(target_arity as usize) {
    ctx.emit(f64_const(0.0));
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
    CalcitProc::NativeRecordStruct => emit_record_struct(ctx, args),
    CalcitProc::NativeRecordGetName => emit_record_get_name(ctx, args),
    CalcitProc::NativeRecordToMap => emit_record_to_map(ctx, args),
    CalcitProc::NativeRecordAssoc | CalcitProc::NativeRecordAssocAt | CalcitProc::NativeRecordWith => {
      Err("Record mutation (assoc/with) not yet supported in WASM codegen".into())
    }
    CalcitProc::NativeRecordFromMap
    | CalcitProc::NativeRecordExtendAs
    | CalcitProc::NativeRecordPartial
    | CalcitProc::NativeRecordContains
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
    CalcitProc::NativeSetToList => emit_set_to_list(ctx, args),
    CalcitProc::NativeDifference => emit_set_difference(ctx, args),
    CalcitProc::NativeUnion => emit_set_union(ctx, args),
    CalcitProc::NativeSetDestruct => emit_set_destruct(ctx, args),
    CalcitProc::NativeMerge => emit_map_merge(ctx, args),
    CalcitProc::NativeMergeNonNil => emit_map_merge_non_nil(ctx, args),
    CalcitProc::NativeMapDiffNew => emit_map_diff_new(ctx, args),
    CalcitProc::NativeMapDiffKeys => emit_map_diff_keys(ctx, args),
    CalcitProc::NativeMapCommonKeys => emit_map_common_keys(ctx, args),
    CalcitProc::NativeMapDestruct => emit_map_destruct(ctx, args),
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

    // --- List higher-order and utility operations ---
    CalcitProc::NativeListDistinct => emit_list_distinct(ctx, args),

    // Higher-order list operations
    CalcitProc::Foldl => emit_foldl(ctx, args),
    CalcitProc::FoldlShortcut => emit_foldl_shortcut(ctx, args),
    CalcitProc::FoldrShortcut => emit_foldr_shortcut(ctx, args),

    // Format (stub — only used in raise/error paths)
    CalcitProc::FormatToLisp => emit_format_to_lisp(ctx, args),
    // get-env — returns nil in WASM (env vars not available)
    CalcitProc::GetEnv => {
      for arg in args {
        emit_expr(ctx, arg)?;
        ctx.emit(Instruction::Drop);
      }
      ctx.emit(f64_const(0.0));
      Ok(())
    }

    // Not yet supported
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
  ctx.emit(Instruction::Br(0));

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


/// Tag for which native set binary operation to apply in `emit_set_op_variadic`.
enum SetOpKind {
  Union,
  Difference,
  Include,
}

/// High-level `union`/`difference`/`include(base & xs)` — fold the extra args using the
/// corresponding native 2-arg set operation.  Handles 1…N extra args.
fn emit_set_op_variadic(ctx: &mut WasmGenCtx, args: &[Calcit], kind: SetOpKind) -> Result<(), String> {
  if args.is_empty() {
    return Err("set op variadic requires at least 1 arg (base)".into());
  }
  if args.len() == 1 {
    // No extra args — return the base set unchanged.
    return emit_expr(ctx, &args[0]);
  }
  // For exactly 2 args: direct call to the existing 2-arg implementation.
  if args.len() == 2 {
    return match kind {
      SetOpKind::Union => emit_set_union(ctx, args),
      SetOpKind::Difference => emit_set_difference(ctx, args),
      SetOpKind::Include => emit_set_include(ctx, args),
    };
  }
  // N-arg case: chain operations.
  // acc = op(args[0], args[1]); for each remaining: acc = op(acc, args[i])
  let acc = ctx.alloc_local();
  // First iteration uses original Calcit args
  {
    let first_two = [args[0].clone(), args[1].clone()];
    match kind {
      SetOpKind::Union => emit_set_union(ctx, &first_two)?,
      SetOpKind::Difference => emit_set_difference(ctx, &first_two)?,
      SetOpKind::Include => emit_set_include(ctx, &first_two)?,
    }
    ctx.emit(Instruction::LocalSet(acc));
  }
  for extra in &args[2..] {
    let extra_val = ctx.alloc_local();
    emit_expr(ctx, extra)?;
    ctx.emit(Instruction::LocalSet(extra_val));

    // Convert acc f64 → i32 ptr
    let acc_ptr = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(acc));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(acc_ptr));

    // Apply operation using raw ptr helpers
    let new_dst = match kind {
      SetOpKind::Union => {
        let extra_ptr = ctx.alloc_local_typed(ValType::I32);
        ctx.emit(Instruction::LocalGet(extra_val));
        ctx.emit(Instruction::I32TruncF64U);
        ctx.emit(Instruction::LocalSet(extra_ptr));
        emit_set_union_from_ptrs(ctx, acc_ptr, extra_ptr)?
      }
      SetOpKind::Difference => {
        let extra_ptr = ctx.alloc_local_typed(ValType::I32);
        ctx.emit(Instruction::LocalGet(extra_val));
        ctx.emit(Instruction::I32TruncF64U);
        ctx.emit(Instruction::LocalSet(extra_ptr));
        emit_set_difference_from_ptrs(ctx, acc_ptr, extra_ptr)?
      }
      SetOpKind::Include => emit_set_include_from_ptrs(ctx, acc_ptr, extra_val)?,
    };
    ctx.emit(Instruction::LocalGet(new_dst));
    ctx.emit(Instruction::LocalSet(acc));
  }
  ctx.emit(Instruction::LocalGet(acc));
  Ok(())
}
/// `&union a b` from pre-loaded i32 pointers. Returns f64 local with result pointer.
fn emit_set_union_from_ptrs(ctx: &mut WasmGenCtx, a: u32, b: u32) -> Result<u32, String> {
  let a_count = emit_load_count_i32(ctx, a);
  let b_count = emit_load_count_i32(ctx, b);

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

  let be = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(b));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(be));

  let found = emit_runtime_lookup_i32_f64_to_i32(ctx, "__rt_set_find_elem", dst, be);
  ctx.emit(Instruction::LocalGet(found));
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
  ctx.emit(Instruction::LocalGet(be));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(write_idx));
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(bi));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  let result = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(result));
  Ok(result)
}

/// `&difference a b` from pre-loaded i32 pointers. Returns f64 local with result pointer.
fn emit_set_difference_from_ptrs(ctx: &mut WasmGenCtx, a: u32, b: u32) -> Result<u32, String> {
  let a_count = emit_load_count_i32(ctx, a);

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

  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(ai));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  let result = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(result));
  Ok(result)
}

/// `&include set elem` from pre-loaded i32 set pointer + f64 elem local. Returns f64 result local.
fn emit_set_include_from_ptrs(ctx: &mut WasmGenCtx, src: u32, elem: u32) -> Result<u32, String> {
  let count = emit_load_count_i32(ctx, src);
  let found_idx = emit_runtime_lookup_i32_f64_to_i32(ctx, "__rt_set_find_elem", src, elem);

  let dst = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::LocalSet(dst));

  ctx.emit(Instruction::LocalGet(found_idx));
  ctx.emit(Instruction::I32Const(-1));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  {
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

  let result = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(result));
  Ok(result)
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

/// `&set:to-list set` — copy set payload into a list.
fn emit_set_to_list(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("&set:to-list expects 1 arg".into());
  }
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);
  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));

  let dst = emit_alloc_with_count(ctx, count, total_slots, "list");
  let dst_base = emit_addr_offset(ctx, dst, 8);
  let src_base = emit_addr_offset(ctx, src, 8);
  emit_copy_f64_loop(ctx, dst_base, src_base, count);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
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

  // Over-allocate: max is a_count + b_count
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
  let be = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(b));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(be));

  // Scan dst for matching element
  let found = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(found));
  let di = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(di));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(di));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(di));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(be));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(found));
  ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(di));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(di));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(found));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  {
    // Not found: append at write_idx position
    let addr = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(dst));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(write_idx));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(addr));
    ctx.emit(Instruction::LocalGet(addr));
    ctx.emit(Instruction::LocalGet(be));
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
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
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&merge a b` — map merge where keys from `b` override keys from `a`.
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

  // Over-allocate: at most a_count + b_count entries.
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

  let write_idx = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(write_idx));

  // Copy all entries from a into dst flat buffer.
  let ai = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(ai));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  let ak = ctx.alloc_local();
  let av = ctx.alloc_local();
  let a_addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(a_flat));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalTee(a_addr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(ak));
  ctx.emit(Instruction::LocalGet(a_addr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalSet(av));

  let out_addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(dst_root));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(out_addr));
  ctx.emit(Instruction::LocalGet(out_addr));
  ctx.emit(Instruction::LocalGet(ak));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(out_addr));
  ctx.emit(Instruction::LocalGet(av));
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(write_idx));

  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(ai));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // Merge entries from b: override existing key, otherwise append.
  let bi = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(bi));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  let bk = ctx.alloc_local();
  let bv = ctx.alloc_local();
  let b_addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(b_flat));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalTee(b_addr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(bk));
  ctx.emit(Instruction::LocalGet(b_addr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalSet(bv));

  let found = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(found));
  let di = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(di));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(di));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  let d_addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(dst_root));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(di));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalTee(d_addr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(bk));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  // Override value for existing key.
  ctx.emit(Instruction::LocalGet(d_addr));
  ctx.emit(Instruction::LocalGet(bv));
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(found));
  ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(di));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(di));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // If key not found, append pair.
  ctx.emit(Instruction::LocalGet(found));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  {
    let out_addr = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(dst_root));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(write_idx));
    ctx.emit(Instruction::I32Const(16));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(out_addr));
    ctx.emit(Instruction::LocalGet(out_addr));
    ctx.emit(Instruction::LocalGet(bk));
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
    ctx.emit(Instruction::LocalGet(out_addr));
    ctx.emit(Instruction::LocalGet(bv));
    ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
    ctx.emit(Instruction::LocalGet(write_idx));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(write_idx));
  }
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(bi));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // Patch actual count and convert flat map back to runtime map.
  ctx.emit(Instruction::LocalGet(dst_root));
  ctx.emit(Instruction::LocalGet(write_idx));
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

  // Over-allocate: max is a_count entries
  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));
  let dst_root = emit_alloc_with_count(ctx, a_count, total_slots, "map");

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

  // Load a[ai] key and val
  let ak = ctx.alloc_local();
  let av = ctx.alloc_local();
  let akv_addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(a_flat));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalTee(akv_addr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(ak));
  ctx.emit(Instruction::LocalGet(akv_addr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalSet(av));

  // Scan b for key and value match
  let found_eq = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(found_eq));
  let bi = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(bi));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

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
  ctx.emit(Instruction::LocalGet(ak));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));

  // Key found in b — mark found regardless of value (diff-new only excludes keys present in b)
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(found_eq));

  // Break b loop since we found the key
  ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(bi));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // If NOT found_eq, copy a[ai] kv to result
  ctx.emit(Instruction::LocalGet(found_eq));
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
    ctx.emit(Instruction::LocalGet(ak));
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
    ctx.emit(Instruction::LocalGet(addr));
    ctx.emit(Instruction::LocalGet(av));
    ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
    ctx.emit(Instruction::LocalGet(write_idx));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(write_idx));
  }
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

// ===========================================================================
// String operations — layout: [magic:i32][type_tag:i32][byte_len:f64][utf8_bytes... padded]
// logical_ptr points to byte_len field; bytes start at logical_ptr+8.
// All byte counts are UTF-8 byte lengths (matching Rust str::len() semantics).
// ===========================================================================

/// Allocate a new heap string of `len_i32` (i32 local) bytes.
/// Returns `(ptr_local, content_base_local)`:
/// - `ptr_local`: i32 local = logical pointer (where byte_len f64 lives)
/// - `content_base_local`: i32 local = ptr + 8 (start of UTF-8 content)
/// Also stores byte_len as f64 into ptr+0.
fn emit_str_alloc(ctx: &mut WasmGenCtx, len_i32: u32) -> (u32, u32) {
  // padded_len = (len + 7) & -8  (round up to 8-byte boundary)
  let padded = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(len_i32));
  ctx.emit(Instruction::I32Const(7));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(-8i32)); // ~7 in two's complement
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalSet(padded));

  // payload = 8 (byte_len f64) + padded_len
  let payload = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::LocalGet(padded));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(payload));

  let ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc_dynamic(ctx, payload, ptr, "string");

  // Store byte_len as f64 at ptr+0
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::LocalGet(len_i32));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  let content_base = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(content_base));

  (ptr, content_base)
}

/// `count` on a string — returns byte length as f64.
fn emit_str_count(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("&str:count expects 1 arg".into());
  }
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0))); // byte_len at logical_ptr+0
  Ok(())
}

/// `str-empty?` — returns 1.0 if byte_len == 0, else 0.0.
fn emit_str_empty(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("str-empty? expects 1 arg".into());
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

/// `&str:concat a b` — new string with a's bytes followed by b's bytes.
fn emit_str_concat(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&str:concat expects 2 args".into());
  }
  let ptr_a = emit_ptr_to_i32(ctx, &args[0])?;
  let ptr_b = emit_ptr_to_i32(ctx, &args[1])?;

  let len_a = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_a));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(len_a));

  let len_b = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_b));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(len_b));

  let len_c = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(len_a));
  ctx.emit(Instruction::LocalGet(len_b));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(len_c));

  let (ptr_c, dst_c) = emit_str_alloc(ctx, len_c);

  // Copy a's bytes: memory.copy(dst_c, ptr_a+8, len_a)
  let src_a = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_a));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(src_a));

  ctx.emit(Instruction::LocalGet(dst_c));
  ctx.emit(Instruction::LocalGet(src_a));
  ctx.emit(Instruction::LocalGet(len_a));
  ctx.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });

  // Copy b's bytes: memory.copy(dst_c + len_a, ptr_b+8, len_b)
  let dst_b = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(dst_c));
  ctx.emit(Instruction::LocalGet(len_a));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(dst_b));

  let src_b = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_b));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(src_b));

  ctx.emit(Instruction::LocalGet(dst_b));
  ctx.emit(Instruction::LocalGet(src_b));
  ctx.emit(Instruction::LocalGet(len_b));
  ctx.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });

  ctx.emit(Instruction::LocalGet(ptr_c));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&str:nth str idx` — byte value at index `idx` as f64 (UTF-8 byte, not char).
fn emit_str_nth(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&str:nth expects 2 args".into());
  }
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  // addr = ptr + 8 + idx
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&str:first str` — first byte value as f64.
fn emit_str_first(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("&str:first expects 1 arg".into());
  }
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::I32Load8U(mem_arg_byte(8))); // offset 8 = first byte after byte_len
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&str:rest str` — new string without the first byte.
fn emit_str_rest(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("&str:rest expects 1 arg".into());
  }
  let ptr_a = emit_ptr_to_i32(ctx, &args[0])?;

  let old_len = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_a));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(old_len));

  let new_len = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(old_len));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(new_len));

  let (ptr_b, dst_b) = emit_str_alloc(ctx, new_len);

  // src = ptr_a + 8 + 1 (skip byte_len header + first byte)
  let src = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_a));
  ctx.emit(Instruction::I32Const(9));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(src));

  ctx.emit(Instruction::LocalGet(dst_b));
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::LocalGet(new_len));
  ctx.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });

  ctx.emit(Instruction::LocalGet(ptr_b));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&str:slice str start end` — new string from bytes [start, end) (byte indices).
fn emit_str_slice(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 3 && args.len() != 2 {
    return Err("&str:slice expects 2 or 3 args (str, start[, end])".into());
  }
  let ptr_a = emit_ptr_to_i32(ctx, &args[0])?;

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
    // end defaults to the byte length of the string
    ctx.emit(Instruction::LocalGet(ptr_a));
    ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(end));
  }

  let new_len = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(end));
  ctx.emit(Instruction::LocalGet(start));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(new_len));

  let (ptr_b, dst_b) = emit_str_alloc(ctx, new_len);

  // src = ptr_a + 8 + start
  let src = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr_a));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(start));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(src));

  ctx.emit(Instruction::LocalGet(dst_b));
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::LocalGet(new_len));
  ctx.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });

  ctx.emit(Instruction::LocalGet(ptr_b));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&str:compare a b` — lexicographic byte comparison; returns -1.0 / 0.0 / 1.0.
fn emit_str_compare(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&str:compare expects 2 args".into());
  }
  let ptr_a = emit_ptr_to_i32(ctx, &args[0])?;
  let ptr_b = emit_ptr_to_i32(ctx, &args[1])?;
  let fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_str_compare")
    .ok_or_else(|| "runtime helper __rt_str_compare not found".to_string())?;
  ctx.emit(Instruction::LocalGet(ptr_a));
  ctx.emit(Instruction::LocalGet(ptr_b));
  ctx.emit(Instruction::Call(fn_idx));
  Ok(())
}

/// `&str:contains? str idx` — 1.0 if the byte index is within string length, else 0.0.
fn emit_str_contains(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&str:contains? expects 2 args (str, idx)".into());
  }
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  // byte_len as i32
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  // idx as i32
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  // byte_len > idx → 1 if in bounds, 0 otherwise
  ctx.emit(Instruction::I32GtU);
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&str:find-index haystack needle` — byte offset of first occurrence, or -1.0.
fn emit_str_find_index(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&str:find-index expects 2 args (haystack, needle)".into());
  }
  let ptr_h = emit_ptr_to_i32(ctx, &args[0])?;
  let ptr_n = emit_ptr_to_i32(ctx, &args[1])?;
  let fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_str_find_index")
    .ok_or_else(|| "runtime helper __rt_str_find_index not found".to_string())?;
  ctx.emit(Instruction::LocalGet(ptr_h));
  ctx.emit(Instruction::LocalGet(ptr_n));
  ctx.emit(Instruction::Call(fn_idx));
  Ok(())
}

/// `&str:includes? haystack needle` — 1.0 if needle appears in haystack, else 0.0.
fn emit_str_includes(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&str:includes? expects 2 args (haystack, needle)".into());
  }
  let ptr_h = emit_ptr_to_i32(ctx, &args[0])?;
  let ptr_n = emit_ptr_to_i32(ctx, &args[1])?;
  let fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_str_find_index")
    .ok_or_else(|| "runtime helper __rt_str_find_index not found".to_string())?;
  ctx.emit(Instruction::LocalGet(ptr_h));
  ctx.emit(Instruction::LocalGet(ptr_n));
  ctx.emit(Instruction::Call(fn_idx));
  // result >= 0.0 → 1 or 0, convert to f64
  ctx.emit(Instruction::F64Const(Ieee64::from(0.0f64)));
  ctx.emit(Instruction::F64Ge);
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `starts-with? s prefix` — returns 1.0 if s starts with prefix, else 0.0.
fn emit_str_starts_with(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("starts-with? expects 2 args (string, prefix)".into());
  }
  let ptr_s = emit_ptr_to_i32(ctx, &args[0])?;
  let ptr_p = emit_ptr_to_i32(ctx, &args[1])?;
  let fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_str_starts_with")
    .ok_or_else(|| "runtime helper __rt_str_starts_with not found".to_string())?;
  ctx.emit(Instruction::LocalGet(ptr_s));
  ctx.emit(Instruction::LocalGet(ptr_p));
  ctx.emit(Instruction::Call(fn_idx));
  Ok(())
}

/// `ends-with? s suffix` — returns 1.0 if s ends with suffix, else 0.0.
fn emit_str_ends_with(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("ends-with? expects 2 args (string, suffix)".into());
  }
  let ptr_s = emit_ptr_to_i32(ctx, &args[0])?;
  let ptr_suf = emit_ptr_to_i32(ctx, &args[1])?;
  let fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_str_ends_with")
    .ok_or_else(|| "runtime helper __rt_str_ends_with not found".to_string())?;
  ctx.emit(Instruction::LocalGet(ptr_s));
  ctx.emit(Instruction::LocalGet(ptr_suf));
  ctx.emit(Instruction::Call(fn_idx));
  Ok(())
}

/// `turn-string v` / `&str v` — convert any value to its string representation.
/// Strings are returned as-is. nil/false → "". Numbers → decimal string.
fn emit_turn_string(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("turn-string expects 1 arg".into());
  }

  let f64_to_str_idx = *ctx
    .runtime_fn_index
    .get("__rt_f64_to_str")
    .ok_or_else(|| "runtime helper __rt_f64_to_str not found".to_string())?;

  let string_type_tag = *ctx.tag_index.get("string").ok_or("string tag missing")? as i32;

  let v = ctx.alloc_local(); // f64 -- the argument value
  let result = ctx.alloc_local(); // f64 -- the output string ptr (0.0 = not yet set)
  let v_i32 = ctx.alloc_local_typed(ValType::I32); // v truncated to i32
  let raw_base = ctx.alloc_local_typed(ValType::I32); // raw_base = v_i32 - 8

  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(v));

  // --- Check if v is already a heap string ---
  // Use f64 comparison first (avoids unsafe truncation for small/negative values)
  let heap_min = (HEAP_BASE as f64) + 8.0;
  ctx.emit(Instruction::LocalGet(v));
  ctx.emit(f64_const(heap_min));
  ctx.emit(Instruction::F64Ge);
  // AND floor(v) == v (integer-valued pointer)
  ctx.emit(Instruction::LocalGet(v));
  ctx.emit(Instruction::F64Floor);
  ctx.emit(Instruction::LocalGet(v));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  {
    ctx.emit(Instruction::LocalGet(v));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(v_i32));
    ctx.emit(Instruction::LocalGet(v_i32));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Sub);
    ctx.emit(Instruction::LocalSet(raw_base));
    // Check HEAP_MAGIC AND type_tag == string_tag
    ctx.emit(Instruction::LocalGet(raw_base));
    ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
    ctx.emit(Instruction::I32Const(HEAP_MAGIC));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::LocalGet(raw_base));
    ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
    ctx.emit(Instruction::I32Const(string_type_tag));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::I32And);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
    ctx.emit(Instruction::LocalGet(v));
    ctx.emit(Instruction::LocalSet(result));
    ctx.emit(Instruction::End); // inner if
  }
  ctx.emit(Instruction::End); // outer if

  // If result is still 0.0, convert value to string
  ctx.emit(Instruction::LocalGet(result));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  {
    // nil/false (0.0) -> allocate empty string ""
    ctx.emit(Instruction::LocalGet(v));
    ctx.emit(f64_const(0.0));
    ctx.emit(Instruction::F64Eq);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
    {
      let raw = ctx.alloc_local_typed(ValType::I32);
      ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
      ctx.emit(Instruction::LocalTee(raw));
      ctx.emit(Instruction::I32Const(HEAP_MAGIC));
      ctx.emit(Instruction::I32Store(mem_arg_i32(0)));
      ctx.emit(Instruction::LocalGet(raw));
      ctx.emit(Instruction::I32Const(string_type_tag));
      ctx.emit(Instruction::I32Store(mem_arg_i32(4)));
      // write byte_len = 0.0 at raw+8
      ctx.emit(Instruction::LocalGet(raw));
      ctx.emit(Instruction::I32Const(8));
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::F64Const(wasm_encoder::Ieee64::from(0.0f64)));
      ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
      // advance heap_ptr by 16 (header=8 + byte_len_slot=8)
      ctx.emit(Instruction::LocalGet(raw));
      ctx.emit(Instruction::I32Const(16));
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
      // result = (raw + 8) as f64
      ctx.emit(Instruction::LocalGet(raw));
      ctx.emit(Instruction::I32Const(8));
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::F64ConvertI32U);
      ctx.emit(Instruction::LocalSet(result));
    }
    ctx.emit(Instruction::Else);
    // Non-nil non-string -> call __rt_f64_to_str
    ctx.emit(Instruction::LocalGet(v));
    ctx.emit(Instruction::Call(f64_to_str_idx));
    ctx.emit(Instruction::LocalSet(result));
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

fn emit_format_to_lisp(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  // Evaluate args for side effects, then return nil
  for arg in args {
    emit_expr(ctx, arg)?;
    ctx.emit(Instruction::Drop);
  }
  ctx.emit(f64_const(0.0));
  Ok(())
}

/// `&list:distinct xs` — return new list with duplicate elements removed (O(n²)).
/// Two elements are considered equal when their f64 bit patterns are identical.
fn emit_list_distinct(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("&list:distinct expects 1 arg".into());
  }
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let n = emit_load_count_i32(ctx, src);

  // Allocate output with same max capacity as input (over-alloc; count updated at end)
  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(n));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));
  let dst = emit_alloc_with_count(ctx, n, total_slots, "list");

  let write_idx = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(write_idx));

  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(i));

  // Outer loop: iterate src elements
  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::LocalGet(n));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  // elem = src[(1 + i) * 8]
  let elem = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(elem));

  // Inner loop: scan dst[0..write_idx) for elem
  let j = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(j));
  let found = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(found));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(j));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  // existing = dst[(1 + j) * 8]
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(j));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(elem));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(found));
  ctx.emit(Instruction::Br(2)); // exit inner block (ends inner loop)
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(j));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(j));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End); // end inner loop
  ctx.emit(Instruction::End); // end inner block

  // If not found: dst[1 + write_idx] = elem; write_idx++
  ctx.emit(Instruction::LocalGet(found));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  {
    let write_addr = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(dst));
    ctx.emit(Instruction::LocalGet(write_idx));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(write_addr));
    ctx.emit(Instruction::LocalGet(write_addr));
    ctx.emit(Instruction::LocalGet(elem));
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
    ctx.emit(Instruction::LocalGet(write_idx));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(write_idx));
  }
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(i));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End); // end outer loop
  ctx.emit(Instruction::End); // end outer block

  // Update count field to actual write_idx
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&map:destruct map` — returns `[key val rest-map]` for the first entry, or nil if empty.
fn emit_map_destruct(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("&map:destruct expects 1 arg".into());
  }
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);

  let result = ctx.alloc_local();

  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  // empty map → nil
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::Else);
  {
    // Linearize to get flat [k0,v0,k1,v1,...] starting at flat+8
    let flat = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", src);

    let key0 = ctx.alloc_local();
    let val0 = ctx.alloc_local();

    // key0 = flat[8 + 0*16] (first key in linearized array)
    ctx.emit(Instruction::LocalGet(flat));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    ctx.emit(Instruction::LocalSet(key0));

    // val0 = flat[8 + 0*16 + 8]
    ctx.emit(Instruction::LocalGet(flat));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
    ctx.emit(Instruction::LocalSet(val0));

    // rest-map = __rt_map_dissoc(src, key0)
    let dissoc_fn = *ctx
      .runtime_fn_index
      .get("__rt_map_dissoc")
      .expect("__rt_map_dissoc must exist");
    let rest_map = ctx.alloc_local();
    ctx.emit(Instruction::LocalGet(src));
    ctx.emit(Instruction::LocalGet(key0));
    ctx.emit(Instruction::Call(dissoc_fn));
    ctx.emit(Instruction::F64ConvertI32U);
    ctx.emit(Instruction::LocalSet(rest_map));

    // Allocate 3-element list [key0, val0, rest-map]
    let list_ptr = ctx.alloc_local_typed(ValType::I32);
    emit_bump_alloc(ctx, 32, list_ptr, "list"); // 4 * 8 bytes
    ctx.emit(Instruction::LocalGet(list_ptr));
    ctx.emit(f64_const(3.0));
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
    ctx.emit(Instruction::LocalGet(list_ptr));
    ctx.emit(Instruction::LocalGet(key0));
    ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
    ctx.emit(Instruction::LocalGet(list_ptr));
    ctx.emit(Instruction::LocalGet(val0));
    ctx.emit(Instruction::F64Store(mem_arg_f64(16)));
    ctx.emit(Instruction::LocalGet(list_ptr));
    ctx.emit(Instruction::LocalGet(rest_map));
    ctx.emit(Instruction::F64Store(mem_arg_f64(24)));

    ctx.emit(Instruction::LocalGet(list_ptr));
    ctx.emit(Instruction::F64ConvertI32U);
    ctx.emit(Instruction::LocalSet(result));
  }
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

/// `&set:destruct set` — returns `[elem rest-set]` for the first entry, or nil if empty.
fn emit_set_destruct(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("&set:destruct expects 1 arg".into());
  }
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);

  let result = ctx.alloc_local();

  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  // empty set → nil
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::Else);
  {
    // First element is at src + 8
    let elem0 = ctx.alloc_local();
    ctx.emit(Instruction::LocalGet(src));
    ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
    ctx.emit(Instruction::LocalSet(elem0));

    // rest-set = exclude(src, elem0)
    let found_idx = emit_runtime_lookup_i32_f64_to_i32(ctx, "__rt_set_find_elem", src, elem0);
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
    let rest_set_ptr = emit_alloc_with_count(ctx, nc, ts, "set");

    // Copy elements before found_idx
    let db1 = emit_addr_offset(ctx, rest_set_ptr, 8);
    let sb1 = emit_addr_offset(ctx, src, 8);
    emit_copy_f64_loop(ctx, db1, sb1, found_idx);

    // Copy elements after found_idx
    let after_n = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(nc));
    ctx.emit(Instruction::LocalGet(found_idx));
    ctx.emit(Instruction::I32Sub);
    ctx.emit(Instruction::LocalSet(after_n));
    let db2 = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(rest_set_ptr));
    ctx.emit(Instruction::LocalGet(found_idx));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(db2));
    let sb2 = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(src));
    ctx.emit(Instruction::LocalGet(found_idx));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(sb2));
    emit_copy_f64_loop(ctx, db2, sb2, after_n);

    let rest_set_f64 = ctx.alloc_local();
    ctx.emit(Instruction::LocalGet(rest_set_ptr));
    ctx.emit(Instruction::F64ConvertI32U);
    ctx.emit(Instruction::LocalSet(rest_set_f64));

    // Allocate 2-element list [elem0, rest-set]
    let list_ptr = ctx.alloc_local_typed(ValType::I32);
    emit_bump_alloc(ctx, 24, list_ptr, "list"); // 3 * 8 bytes
    ctx.emit(Instruction::LocalGet(list_ptr));
    ctx.emit(f64_const(2.0));
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
    ctx.emit(Instruction::LocalGet(list_ptr));
    ctx.emit(Instruction::LocalGet(elem0));
    ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
    ctx.emit(Instruction::LocalGet(list_ptr));
    ctx.emit(Instruction::LocalGet(rest_set_f64));
    ctx.emit(Instruction::F64Store(mem_arg_f64(16)));

    ctx.emit(Instruction::LocalGet(list_ptr));
    ctx.emit(Instruction::F64ConvertI32U);
    ctx.emit(Instruction::LocalSet(result));
  }
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

/// `&merge-non-nil a b` — map merge where nil values from `b` are skipped.
fn emit_map_merge_non_nil(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&merge-non-nil expects 2 args".into());
  }
  let a = emit_ptr_to_i32(ctx, &args[0])?;
  let a_count = emit_load_count_i32(ctx, a);
  let a_flat = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", a);
  let b = emit_ptr_to_i32(ctx, &args[1])?;
  let b_count = emit_load_count_i32(ctx, b);
  let b_flat = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", b);

  // Over-allocate: at most a_count + b_count entries
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

  // Start by copying all non-nil-valued entries from b into a temporary HAMT via __rt_map_assoc
  // Strategy: build result starting from an empty map, then:
  //   1. Insert all a entries
  //   2. Insert all b entries (skip if value is nil=0.0)
  // Use __rt_map_make to build from flat linearised arrays.
  // Simpler approach: re-use the same logic as emit_map_merge but skip nil values from b.

  let write_idx = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(write_idx));

  // Copy all a entries
  let ai = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(ai));
  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  {
    let ak = ctx.alloc_local();
    let av = ctx.alloc_local();
    let a_addr = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(a_flat));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(ai));
    ctx.emit(Instruction::I32Const(16));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalTee(a_addr));
    ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    ctx.emit(Instruction::LocalSet(ak));
    ctx.emit(Instruction::LocalGet(a_addr));
    ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
    ctx.emit(Instruction::LocalSet(av));

    let d_addr = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(dst_root));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(write_idx));
    ctx.emit(Instruction::I32Const(16));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalTee(d_addr));
    ctx.emit(Instruction::LocalGet(ak));
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
    ctx.emit(Instruction::LocalGet(d_addr));
    ctx.emit(Instruction::LocalGet(av));
    ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
    ctx.emit(Instruction::LocalGet(write_idx));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(write_idx));
  }
  ctx.emit(Instruction::LocalGet(ai));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(ai));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // Update existing a entries with b, skip nil b values; add new b entries
  let bi = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(bi));
  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  {
    let bk = ctx.alloc_local();
    let bv = ctx.alloc_local();
    let b_addr = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(b_flat));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(bi));
    ctx.emit(Instruction::I32Const(16));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalTee(b_addr));
    ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
    ctx.emit(Instruction::LocalSet(bk));
    ctx.emit(Instruction::LocalGet(b_addr));
    ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
    ctx.emit(Instruction::LocalSet(bv));

    // Skip if bv is nil (0.0)
    ctx.emit(Instruction::LocalGet(bv));
    ctx.emit(f64_const(0.0));
    ctx.emit(Instruction::F64Ne);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
    {
      // Scan existing dst for bk; update if found, else insert
      let found_di = ctx.alloc_local_typed(ValType::I32);
      ctx.emit(Instruction::I32Const(-1i32 as u32 as i32));
      ctx.emit(Instruction::LocalSet(found_di));
      let di = ctx.alloc_local_typed(ValType::I32);
      ctx.emit(Instruction::I32Const(0));
      ctx.emit(Instruction::LocalSet(di));
      ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
      ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
      ctx.emit(Instruction::LocalGet(di));
      ctx.emit(Instruction::LocalGet(write_idx));
      ctx.emit(Instruction::I32GeU);
      ctx.emit(Instruction::BrIf(1));
      {
        let dk_addr = ctx.alloc_local_typed(ValType::I32);
        ctx.emit(Instruction::LocalGet(dst_root));
        ctx.emit(Instruction::I32Const(8));
        ctx.emit(Instruction::I32Add);
        ctx.emit(Instruction::LocalGet(di));
        ctx.emit(Instruction::I32Const(16));
        ctx.emit(Instruction::I32Mul);
        ctx.emit(Instruction::I32Add);
        ctx.emit(Instruction::LocalTee(dk_addr));
        ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
        ctx.emit(Instruction::LocalGet(bk));
        ctx.emit(Instruction::F64Eq);
        ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
        ctx.emit(Instruction::LocalGet(di));
        ctx.emit(Instruction::LocalSet(found_di));
        ctx.emit(Instruction::Br(2)); // exit scan loop
        ctx.emit(Instruction::End);
      }
      ctx.emit(Instruction::LocalGet(di));
      ctx.emit(Instruction::I32Const(1));
      ctx.emit(Instruction::I32Add);
      ctx.emit(Instruction::LocalSet(di));
      ctx.emit(Instruction::Br(0));
      ctx.emit(Instruction::End);
      ctx.emit(Instruction::End);

      // found_di >= 0: update value; else: append
      ctx.emit(Instruction::LocalGet(found_di));
      ctx.emit(Instruction::I32Const(0));
      ctx.emit(Instruction::I32GeS);
      ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
      {
        // Update dst[found_di].value = bv
        let upd_addr = ctx.alloc_local_typed(ValType::I32);
        ctx.emit(Instruction::LocalGet(dst_root));
        ctx.emit(Instruction::I32Const(8));
        ctx.emit(Instruction::I32Add);
        ctx.emit(Instruction::LocalGet(found_di));
        ctx.emit(Instruction::I32Const(16));
        ctx.emit(Instruction::I32Mul);
        ctx.emit(Instruction::I32Add);
        ctx.emit(Instruction::LocalTee(upd_addr));
        ctx.emit(Instruction::LocalGet(bv));
        ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
      }
      ctx.emit(Instruction::Else);
      {
        // Append new key-value pair
        let new_addr = ctx.alloc_local_typed(ValType::I32);
        ctx.emit(Instruction::LocalGet(dst_root));
        ctx.emit(Instruction::I32Const(8));
        ctx.emit(Instruction::I32Add);
        ctx.emit(Instruction::LocalGet(write_idx));
        ctx.emit(Instruction::I32Const(16));
        ctx.emit(Instruction::I32Mul);
        ctx.emit(Instruction::I32Add);
        ctx.emit(Instruction::LocalTee(new_addr));
        ctx.emit(Instruction::LocalGet(bk));
        ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
        ctx.emit(Instruction::LocalGet(new_addr));
        ctx.emit(Instruction::LocalGet(bv));
        ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
        ctx.emit(Instruction::LocalGet(write_idx));
        ctx.emit(Instruction::I32Const(1));
        ctx.emit(Instruction::I32Add);
        ctx.emit(Instruction::LocalSet(write_idx));
      }
      ctx.emit(Instruction::End);
    }
    ctx.emit(Instruction::End); // end if bv != nil
  }
  ctx.emit(Instruction::LocalGet(bi));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(bi));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // Build the result map via __rt_map_from_flat from the flat dst_root buffer
  ctx.emit(Instruction::LocalGet(dst_root));
  ctx.emit(Instruction::LocalGet(write_idx));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  let dst = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_from_flat", dst_root);
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// Resolve a compile-time Calcit callee expression to a WASM function index.
/// Supports Fn literals (with def_ref), Imports, and Symbols.
/// The callee must have arity 2 (acc, elem) — as required by foldl/foldl-shortcut/foldr-shortcut.
fn resolve_callee_fn_idx(ctx: &WasmGenCtx, callee: &Calcit) -> Result<u32, String> {
  match callee {
    Calcit::Fn { info, .. } => {
      let def_ref = info.def_ref.as_ref().ok_or_else(|| {
        format!("fn literal without def_ref in foldl callee: {}/{}", info.def_ns, info.name)
      })?;
      let qualified = format!("{}/{}", def_ref.def_ns, def_ref.def_name);
      let fn_idx = ctx
        .fn_index
        .get(&qualified)
        .or_else(|| ctx.fn_index.get(def_ref.def_name.as_ref()))
        .copied()
        .ok_or_else(|| format!("unknown fn in foldl callee: {qualified}"))?;
      // Callee must have arity 2 (acc, elem). If the def_ref points to a function with
      // different arity (e.g. the outer containing function), reject it — calling with 2
      // args would produce an invalid WASM module.
      if let Some(arity) = ctx
        .fn_arity
        .get(&qualified)
        .or_else(|| ctx.fn_arity.get(def_ref.def_name.as_ref()))
        .copied()
      {
        if arity != 2 {
          return Err(format!(
            "foldl callee must be a 2-arg fn (acc, elem), but {qualified} has arity {arity}"
          ));
        }
      }
      Ok(fn_idx)
    }
    Calcit::Import(import) => {
      let qualified = format!("{}/{}", import.ns, import.def);
      ctx
        .fn_index
        .get(&qualified)
        .or_else(|| ctx.fn_index.get(import.def.as_ref()))
        .copied()
        .ok_or_else(|| format!("unknown import in foldl callee: {qualified}"))
    }
    Calcit::Symbol { sym, .. } => ctx
      .fn_index
      .get(sym.as_ref())
      .copied()
      .ok_or_else(|| format!("unknown symbol in foldl callee: {sym}")),
    _ => Err(format!("foldl callee must be a static fn/import/symbol in WASM, got: {callee}")),
  }
}

/// Try to extract (param_names, body_exprs) from an inline lambda form.
///
/// Handles both `Calcit::List(defn name args body...)` forms (anonymous lambdas
/// appearing in preprocessed code) and `Calcit::Fn { info }` values that don't
/// have a resolvable `def_ref`.
fn try_extract_inline_lambda(callee: &Calcit) -> Option<(Vec<String>, Vec<Calcit>)> {
  let Calcit::List(items) = callee else {
    return None;
  };
  // Pattern: ((&syntax defn) name (args...) body...)
  match (items.first(), items.get(1), items.get(2)) {
    (Some(Calcit::Syntax(CalcitSyntax::Defn, _)), _, Some(Calcit::List(args))) => {
      let params: Vec<String> = args
        .iter()
        .filter_map(|a| match a {
          Calcit::Local(CalcitLocal { sym, .. }) => Some(sym.as_ref().to_owned()),
          Calcit::Symbol { sym, .. } => Some(sym.as_ref().to_owned()),
          _ => None,
        })
        .collect();
      if params.len() < 1 {
        return None;
      }
      let body: Vec<Calcit> = items.drop_left().drop_left().drop_left().to_vec();
      if body.is_empty() {
        return None;
      }
      Some((params, body))
    }
    _ => None,
  }
}

/// `foldl xs init fn` — iterate xs left-to-right, calling fn(acc, elem) each step.
/// Only supports list collections. fn must be statically resolvable or an inline lambda.
fn emit_foldl(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 3 {
    return Err(format!("foldl expects 3 args, got {}", args.len()));
  }

  // Evaluate collection → i32 pointer
  let list_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  // Load count (number of elements)
  let count = emit_load_count_i32(ctx, list_ptr);

  // Evaluate init → acc local
  let acc = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(acc));

  // Loop index
  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(i));

  // Temporary element local
  let elem = ctx.alloc_local();

  // Resolve callee: static fn or inline lambda
  let fn_call_kind = if let Ok(fn_idx) = resolve_callee_fn_idx(ctx, &args[2]) {
    FoldlCallKind::Static(fn_idx)
  } else if let Some((params, body)) = try_extract_inline_lambda(&args[2]) {
    if params.len() < 2 {
      return Err(format!("foldl inline lambda must have at least 2 params, got {}", params.len()));
    }
    FoldlCallKind::Inline(params, body)
  } else {
    return Err(format!("foldl callee must be a static fn/import/symbol in WASM, got: {}", args[2]));
  };

  // Block + Loop pattern
  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));

  // if i >= count: break out of block
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  // elem = list_ptr[(1+i)*8] — skip the count slot at offset 0
  ctx.emit(Instruction::LocalGet(list_ptr));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(elem));

  // acc = fn(acc, elem)
  match fn_call_kind {
    FoldlCallKind::Static(fn_idx) => {
      ctx.emit(Instruction::LocalGet(acc));
      ctx.emit(Instruction::LocalGet(elem));
      ctx.emit(Instruction::Call(fn_idx));
    }
    FoldlCallKind::Inline(ref params, ref body) => {
      let old0 = ctx.locals.insert(params[0].clone(), acc);
      let old1 = ctx.locals.insert(params[1].clone(), elem);
      emit_body(ctx, body)?;
      // Restore previous local mappings
      match old0 {
        Some(v) => { ctx.locals.insert(params[0].clone(), v); }
        None => { ctx.locals.remove(&params[0]); }
      }
      match old1 {
        Some(v) => { ctx.locals.insert(params[1].clone(), v); }
        None => { ctx.locals.remove(&params[1]); }
      }
    }
  }
  ctx.emit(Instruction::LocalSet(acc));

  // i += 1; continue loop
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(i));
  ctx.emit(Instruction::Br(0));

  ctx.emit(Instruction::End); // end loop
  ctx.emit(Instruction::End); // end block

  // Push acc as result
  ctx.emit(Instruction::LocalGet(acc));
  Ok(())
}

/// Helper enum for foldl call strategy.
enum FoldlCallKind {
  Static(u32),
  Inline(Vec<String>, Vec<Calcit>),
}

/// `foldl-shortcut xs acc default fn` — like foldl but fn returns `:: bool new_acc`.
/// If bool is true, return new_acc immediately (short-circuit). Otherwise continue.
/// After exhausting xs, return default.
/// fn must be statically resolvable or an inline lambda.
fn emit_foldl_shortcut(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 4 {
    return Err(format!("foldl-shortcut expects 4 args, got {}", args.len()));
  }

  // Resolve callee
  let fn_call_kind = if let Ok(fn_idx) = resolve_callee_fn_idx(ctx, &args[3]) {
    FoldlCallKind::Static(fn_idx)
  } else if let Some((params, body)) = try_extract_inline_lambda(&args[3]) {
    if params.len() < 2 {
      return Err(format!("foldl-shortcut inline lambda must have ≥2 params"));
    }
    FoldlCallKind::Inline(params, body)
  } else {
    return Err(format!("foldl callee must be a static fn/import/symbol in WASM, got: {}", args[3]));
  };

  // Evaluate collection → i32 pointer
  let list_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, list_ptr);

  // Evaluate init acc → local
  let acc = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(acc));

  // Evaluate default → result local (will be overwritten on early exit)
  let result = ctx.alloc_local();
  emit_expr(ctx, &args[2])?;
  ctx.emit(Instruction::LocalSet(result));

  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(i));

  let elem = ctx.alloc_local();
  let tuple_ptr = ctx.alloc_local_typed(ValType::I32);

  // Outer block for early exit; inner loop for iteration
  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));

  // if i >= count: exit block (return default)
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  // elem = list_ptr[(1+i)*8]
  ctx.emit(Instruction::LocalGet(list_ptr));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(elem));

  // tuple_ptr = trunc(fn(acc, elem))
  match fn_call_kind {
    FoldlCallKind::Static(fn_idx) => {
      ctx.emit(Instruction::LocalGet(acc));
      ctx.emit(Instruction::LocalGet(elem));
      ctx.emit(Instruction::Call(fn_idx));
    }
    FoldlCallKind::Inline(ref params, ref body) => {
      let old0 = ctx.locals.insert(params[0].clone(), acc);
      let old1 = ctx.locals.insert(params[1].clone(), elem);
      emit_body(ctx, body)?;
      match old0 {
        Some(v) => { ctx.locals.insert(params[0].clone(), v); }
        None => { ctx.locals.remove(&params[0]); }
      }
      match old1 {
        Some(v) => { ctx.locals.insert(params[1].clone(), v); }
        None => { ctx.locals.remove(&params[1]); }
      }
    }
  }
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(tuple_ptr));

  // tag = F64Load(tuple_ptr + 8) — bool flag (1.0 = true → early exit)
  ctx.emit(Instruction::LocalGet(tuple_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::F64Eq); // → i32

  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  // Early exit: result = payload at tuple_ptr + 16
  ctx.emit(Instruction::LocalGet(tuple_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(16)));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::Br(2)); // break outer block (If=0, Loop=1, Block=2)
  ctx.emit(Instruction::End); // end if

  // else: acc = payload, continue
  ctx.emit(Instruction::LocalGet(tuple_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(16)));
  ctx.emit(Instruction::LocalSet(acc));

  // i += 1; continue loop
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

/// `foldr-shortcut xs acc default fn` — like foldl-shortcut but iterates right-to-left.
fn emit_foldr_shortcut(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 4 {
    return Err(format!("foldr-shortcut expects 4 args, got {}", args.len()));
  }

  // Resolve callee
  let fn_call_kind = if let Ok(fn_idx) = resolve_callee_fn_idx(ctx, &args[3]) {
    FoldlCallKind::Static(fn_idx)
  } else if let Some((params, body)) = try_extract_inline_lambda(&args[3]) {
    if params.len() < 2 {
      return Err(format!("foldr-shortcut inline lambda must have ≥2 params"));
    }
    FoldlCallKind::Inline(params, body)
  } else {
    return Err(format!("foldl callee must be a static fn/import/symbol in WASM, got: {}", args[3]));
  };

  let list_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, list_ptr);

  let acc = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(acc));

  let result = ctx.alloc_local();
  emit_expr(ctx, &args[2])?;
  ctx.emit(Instruction::LocalSet(result));

  // i = count - 1 (signed i32, start from last element)
  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(i));

  let elem = ctx.alloc_local();
  let tuple_ptr = ctx.alloc_local_typed(ValType::I32);

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));

  // if i < 0: exit block (return default)
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::I32LtS);
  ctx.emit(Instruction::BrIf(1));

  // elem = list_ptr[(1+i)*8]
  ctx.emit(Instruction::LocalGet(list_ptr));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(elem));

  // tuple_ptr = trunc(fn(acc, elem))
  match fn_call_kind {
    FoldlCallKind::Static(fn_idx) => {
      ctx.emit(Instruction::LocalGet(acc));
      ctx.emit(Instruction::LocalGet(elem));
      ctx.emit(Instruction::Call(fn_idx));
    }
    FoldlCallKind::Inline(ref params, ref body) => {
      let old0 = ctx.locals.insert(params[0].clone(), acc);
      let old1 = ctx.locals.insert(params[1].clone(), elem);
      emit_body(ctx, body)?;
      match old0 {
        Some(v) => { ctx.locals.insert(params[0].clone(), v); }
        None => { ctx.locals.remove(&params[0]); }
      }
      match old1 {
        Some(v) => { ctx.locals.insert(params[1].clone(), v); }
        None => { ctx.locals.remove(&params[1]); }
      }
    }
  }
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(tuple_ptr));

  // tag = F64Load(tuple_ptr + 8) — bool flag
  ctx.emit(Instruction::LocalGet(tuple_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::F64Eq);

  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(tuple_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(16)));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::Br(2)); // break outer block
  ctx.emit(Instruction::End); // end if

  // else: acc = payload, i--
  ctx.emit(Instruction::LocalGet(tuple_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(16)));
  ctx.emit(Instruction::LocalSet(acc));

  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(i));
  ctx.emit(Instruction::Br(0));

  ctx.emit(Instruction::End); // end loop
  ctx.emit(Instruction::End); // end block

  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

/// `&str:pad-left str target-size pattern` — pads str on the left.
fn emit_str_pad_left(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 3 {
    return Err("&str:pad-left expects 3 args (str, size, pattern)".into());
  }
  let ptr_s = emit_ptr_to_i32(ctx, &args[0])?;
  let ptr_p = emit_ptr_to_i32(ctx, &args[2])?;
  let fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_str_pad_left")
    .ok_or_else(|| "runtime helper __rt_str_pad_left not found".to_string())?;
  ctx.emit(Instruction::LocalGet(ptr_s));
  // target_size as i32
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalGet(ptr_p));
  ctx.emit(Instruction::Call(fn_idx));
  Ok(())
}

/// `&str:pad-right str target-size pattern` — pads str on the right.
fn emit_str_pad_right(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 3 {
    return Err("&str:pad-right expects 3 args (str, size, pattern)".into());
  }
  let ptr_s = emit_ptr_to_i32(ctx, &args[0])?;
  let ptr_p = emit_ptr_to_i32(ctx, &args[2])?;
  let fn_idx = *ctx
    .runtime_fn_index
    .get("__rt_str_pad_right")
    .ok_or_else(|| "runtime helper __rt_str_pad_right not found".to_string())?;
  ctx.emit(Instruction::LocalGet(ptr_s));
  // target_size as i32
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalGet(ptr_p));
  ctx.emit(Instruction::Call(fn_idx));
  Ok(())
}

// ===========================================================================
// __str_new — exported FFI helper for JS → WASM string passing.
//
// JS protocol:
//   1. Read heap top: `const top = inst.exports.__heap_ptr.value`
//   2. Write UTF-8 bytes at `top + 16` (after the 8-byte header and 8-byte byte_len)
//   3. Call `inst.exports.__str_new(top + 16, byteLen)` → returns f64 logical pointer
//
// The zero-copy path: when src_ptr == logical_ptr + 8, memory.copy is a no-op.
// JS may also write to any other scratch address and pass that as src_ptr.
// ===========================================================================

/// Build the `__str_new(src_ptr: i32, byte_len: i32) → f64` runtime function.
/// Copies `byte_len` bytes from `src_ptr` into a new heap-allocated tagged string.
fn build_str_new_fn(str_tag_id: i32) -> CompiledFn {
  // params: 0 = src_ptr (i32), 1 = byte_len (i32)
  // locals: 2 = padded (i32), 3 = payload (i32), 4 = ptr (i32)
  let instructions = vec![
    // padded = (byte_len + 7) & -8
    Instruction::LocalGet(1),
    Instruction::I32Const(7),
    Instruction::I32Add,
    Instruction::I32Const(-8i32),
    Instruction::I32And,
    Instruction::LocalSet(2), // padded
    // payload = 8 + padded
    Instruction::I32Const(8),
    Instruction::LocalGet(2),
    Instruction::I32Add,
    Instruction::LocalSet(3), // payload
    // Write HEAP_MAGIC at heap_ptr (raw_base + 0)
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Store(mem_arg_i32(0)),
    // Write str_tag_id at heap_ptr + 4
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(str_tag_id),
    Instruction::I32Store(mem_arg_i32(4)),
    // ptr = heap_ptr + 8; advance heap_ptr += payload
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalTee(4), // ptr = logical_ptr
    Instruction::LocalGet(3), // payload
    Instruction::I32Add,
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    // Store byte_len as f64 at ptr+0
    Instruction::LocalGet(4),
    Instruction::LocalGet(1), // byte_len
    Instruction::F64ConvertI32U,
    Instruction::F64Store(mem_arg_f64(0)),
    // memory.copy(dst = ptr+8, src = src_ptr, n = byte_len)
    Instruction::LocalGet(4),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(0), // src_ptr
    Instruction::LocalGet(1), // byte_len
    Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 },
    // return logical_ptr as f64
    Instruction::LocalGet(4),
    Instruction::F64ConvertI32U,
  ];

  CompiledFn {
    export_name: Some("__str_new".to_string()),
    params: vec![ValType::I32, ValType::I32],
    results: vec![ValType::F64],
    locals: vec![ValType::I32, ValType::I32, ValType::I32], // padded, payload, ptr
    instructions,
  }
}

/// Build `__rt_str_pad_left(str_ptr: i32, target_size: i32, pattern_ptr: i32) → f64`.
/// Pads `str` on the left with repeating `pattern` bytes until `target_size` total bytes.
/// If `str` is already >= `target_size`, returns the original pointer unchanged.
fn build_str_pad_left_fn(str_tag_id: i32) -> CompiledFn {
  // params: 0=str_ptr(i32), 1=target_size(i32), 2=pattern_ptr(i32)
  // locals: 3=str_len, 4=pad_size, 5=pat_len, 6=padded, 7=payload,
  //         8=new_ptr, 9=dst_base, 10=i, 11=j, 12=byte_val  (all i32)
  let instructions = vec![
    // str_len = i32(f64.load str_ptr+0)
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(3),
    // Block $outer (result f64) — for early return
    Instruction::Block(wasm_encoder::BlockType::Result(ValType::F64)),
    // if str_len >= target_size: return f64(str_ptr)
    Instruction::LocalGet(3),
    Instruction::LocalGet(1),
    Instruction::I32GeU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::F64ConvertI32U,
    Instruction::Br(1), // 0=If, 1=$outer
    Instruction::End,
    // pad_size = target_size - str_len
    Instruction::LocalGet(1),
    Instruction::LocalGet(3),
    Instruction::I32Sub,
    Instruction::LocalSet(4),
    // pat_len = i32(f64.load pattern_ptr+0)
    Instruction::LocalGet(2),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(5),
    // if pat_len == 0: return original (guard)
    Instruction::LocalGet(5),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::F64ConvertI32U,
    Instruction::Br(1),
    Instruction::End,
    // padded = (target_size + 7) & -8
    Instruction::LocalGet(1),
    Instruction::I32Const(7),
    Instruction::I32Add,
    Instruction::I32Const(-8i32),
    Instruction::I32And,
    Instruction::LocalSet(6),
    // payload = 8 + padded
    Instruction::I32Const(8),
    Instruction::LocalGet(6),
    Instruction::I32Add,
    Instruction::LocalSet(7),
    // Write HEAP_MAGIC at heap_ptr
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Store(mem_arg_i32(0)),
    // Write str_tag_id at heap_ptr+4
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(str_tag_id),
    Instruction::I32Store(mem_arg_i32(4)),
    // new_ptr = heap_ptr + 8; advance heap by 8 + payload
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(8), // new_ptr
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(8),
    Instruction::LocalGet(7),
    Instruction::I32Add,
    Instruction::I32Add,
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    // Write byte_len (target_size as f64) at new_ptr+0
    Instruction::LocalGet(8),
    Instruction::LocalGet(1),
    Instruction::F64ConvertI32U,
    Instruction::F64Store(mem_arg_f64(0)),
    // dst_base = new_ptr + 8
    Instruction::LocalGet(8),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(9),
    // i = 0, j = 0
    Instruction::I32Const(0),
    Instruction::LocalSet(10),
    Instruction::I32Const(0),
    Instruction::LocalSet(11),
    // Fill pad_size bytes with pattern (cycling)
    Instruction::Block(wasm_encoder::BlockType::Empty), // $pad_break
    Instruction::Loop(wasm_encoder::BlockType::Empty),  // $pad_loop
    // if i >= pad_size: break
    Instruction::LocalGet(10),
    Instruction::LocalGet(4),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    // if j >= pat_len: j = 0
    Instruction::LocalGet(11),
    Instruction::LocalGet(5),
    Instruction::I32GeU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::LocalSet(11),
    Instruction::End,
    // byte_val = (pattern_ptr+8)[j]
    Instruction::LocalGet(2),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(11),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(12),
    // dst_base[i] = byte_val
    Instruction::LocalGet(9),
    Instruction::LocalGet(10),
    Instruction::I32Add,
    Instruction::LocalGet(12),
    Instruction::I32Store8(mem_arg_byte(0)),
    // i++, j++
    Instruction::LocalGet(10),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(10),
    Instruction::LocalGet(11),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(11),
    Instruction::Br(0), // continue $pad_loop
    Instruction::End,   // $pad_loop
    Instruction::End,   // $pad_break
    // Copy original string bytes after the pad region
    // memory.copy(dst = dst_base + pad_size, src = str_ptr+8, n = str_len)
    Instruction::LocalGet(9),
    Instruction::LocalGet(4),
    Instruction::I32Add, // dst = dst_base + pad_size
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Add, // src = str_ptr + 8
    Instruction::LocalGet(3), // str_len
    Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 },
    // return f64(new_ptr)
    Instruction::LocalGet(8),
    Instruction::F64ConvertI32U,
    Instruction::End, // end $outer
  ];
  CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32, ValType::I32],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // str_len (3)
      ValType::I32, // pad_size (4)
      ValType::I32, // pat_len (5)
      ValType::I32, // padded (6)
      ValType::I32, // payload (7)
      ValType::I32, // new_ptr (8)
      ValType::I32, // dst_base (9)
      ValType::I32, // i (10)
      ValType::I32, // j (11)
      ValType::I32, // byte_val (12)
    ],
    instructions,
  }
}

/// Build `__rt_str_pad_right(str_ptr: i32, target_size: i32, pattern_ptr: i32) → f64`.
/// Pads `str` on the right with repeating `pattern` bytes until `target_size` total bytes.
fn build_str_pad_right_fn(str_tag_id: i32) -> CompiledFn {
  // params: 0=str_ptr(i32), 1=target_size(i32), 2=pattern_ptr(i32)
  // Same locals layout as pad_left.
  let instructions = vec![
    // str_len = i32(f64.load str_ptr+0)
    Instruction::LocalGet(0),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(3),
    // Block $outer (result f64)
    Instruction::Block(wasm_encoder::BlockType::Result(ValType::F64)),
    // if str_len >= target_size: return f64(str_ptr)
    Instruction::LocalGet(3),
    Instruction::LocalGet(1),
    Instruction::I32GeU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::F64ConvertI32U,
    Instruction::Br(1),
    Instruction::End,
    // pad_size = target_size - str_len
    Instruction::LocalGet(1),
    Instruction::LocalGet(3),
    Instruction::I32Sub,
    Instruction::LocalSet(4),
    // pat_len = i32(f64.load pattern_ptr+0)
    Instruction::LocalGet(2),
    Instruction::F64Load(mem_arg_f64(0)),
    Instruction::I32TruncF64U,
    Instruction::LocalSet(5),
    // if pat_len == 0: return original (guard)
    Instruction::LocalGet(5),
    Instruction::I32Eqz,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(0),
    Instruction::F64ConvertI32U,
    Instruction::Br(1),
    Instruction::End,
    // padded = (target_size + 7) & -8
    Instruction::LocalGet(1),
    Instruction::I32Const(7),
    Instruction::I32Add,
    Instruction::I32Const(-8i32),
    Instruction::I32And,
    Instruction::LocalSet(6),
    // payload = 8 + padded
    Instruction::I32Const(8),
    Instruction::LocalGet(6),
    Instruction::I32Add,
    Instruction::LocalSet(7),
    // Write HEAP_MAGIC at heap_ptr
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(HEAP_MAGIC),
    Instruction::I32Store(mem_arg_i32(0)),
    // Write str_tag_id at heap_ptr+4
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(str_tag_id),
    Instruction::I32Store(mem_arg_i32(4)),
    // new_ptr = heap_ptr + 8; advance heap
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(8), // new_ptr
    Instruction::GlobalGet(HEAP_PTR_GLOBAL),
    Instruction::I32Const(8),
    Instruction::LocalGet(7),
    Instruction::I32Add,
    Instruction::I32Add,
    Instruction::GlobalSet(HEAP_PTR_GLOBAL),
    // Write byte_len (target_size as f64) at new_ptr+0
    Instruction::LocalGet(8),
    Instruction::LocalGet(1),
    Instruction::F64ConvertI32U,
    Instruction::F64Store(mem_arg_f64(0)),
    // dst_base = new_ptr + 8
    Instruction::LocalGet(8),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalSet(9),
    // Copy original string to the start of dst_base
    Instruction::LocalGet(9), // dst = dst_base
    Instruction::LocalGet(0),
    Instruction::I32Const(8),
    Instruction::I32Add, // src = str_ptr + 8
    Instruction::LocalGet(3), // n = str_len
    Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 },
    // i = 0, j = 0
    Instruction::I32Const(0),
    Instruction::LocalSet(10),
    Instruction::I32Const(0),
    Instruction::LocalSet(11),
    // Fill pad_size bytes with pattern after the original
    Instruction::Block(wasm_encoder::BlockType::Empty),
    Instruction::Loop(wasm_encoder::BlockType::Empty),
    Instruction::LocalGet(10),
    Instruction::LocalGet(4),
    Instruction::I32GeU,
    Instruction::BrIf(1),
    // if j >= pat_len: j = 0
    Instruction::LocalGet(11),
    Instruction::LocalGet(5),
    Instruction::I32GeU,
    Instruction::If(wasm_encoder::BlockType::Empty),
    Instruction::I32Const(0),
    Instruction::LocalSet(11),
    Instruction::End,
    // byte_val = (pattern_ptr+8)[j]
    Instruction::LocalGet(2),
    Instruction::I32Const(8),
    Instruction::I32Add,
    Instruction::LocalGet(11),
    Instruction::I32Add,
    Instruction::I32Load8U(mem_arg_byte(0)),
    Instruction::LocalSet(12),
    // dst_base[str_len + i] = byte_val
    Instruction::LocalGet(9),
    Instruction::LocalGet(3), // str_len
    Instruction::I32Add,
    Instruction::LocalGet(10), // i
    Instruction::I32Add,
    Instruction::LocalGet(12),
    Instruction::I32Store8(mem_arg_byte(0)),
    // i++, j++
    Instruction::LocalGet(10),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(10),
    Instruction::LocalGet(11),
    Instruction::I32Const(1),
    Instruction::I32Add,
    Instruction::LocalSet(11),
    Instruction::Br(0),
    Instruction::End,
    Instruction::End,
    // return f64(new_ptr)
    Instruction::LocalGet(8),
    Instruction::F64ConvertI32U,
    Instruction::End, // end $outer
  ];
  CompiledFn {
    export_name: None,
    params: vec![ValType::I32, ValType::I32, ValType::I32],
    results: vec![ValType::F64],
    locals: vec![
      ValType::I32, // str_len (3)
      ValType::I32, // pad_size (4)
      ValType::I32, // pat_len (5)
      ValType::I32, // padded (6)
      ValType::I32, // payload (7)
      ValType::I32, // new_ptr (8)
      ValType::I32, // dst_base (9)
      ValType::I32, // i (10)
      ValType::I32, // j (11)
      ValType::I32, // byte_val (12)
    ],
    instructions,
  }
}
