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
use crate::calcit::{Calcit, CalcitArgLabel, CalcitFnArgs, CalcitImport, CalcitLocal, CalcitProc, CalcitStruct, CalcitSyntax};
use crate::program;

/// Initial heap offset — reserve first 16 bytes.
const HEAP_START: i32 = 16;
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
  let mut fn_index: HashMap<String, u32> = HashMap::new();
  let mut fn_arity: HashMap<String, u32> = HashMap::new();
  // Track functions with rest args: value is the fixed-arity count (params before `&`).
  // WASM arity for such functions is `fixed_arity + 1` (the rest list pointer).
  let mut fn_has_rest: HashMap<String, u32> = HashMap::new();
  for (i, (ns, name, args, _)) in fn_defs.iter().enumerate() {
    let idx = num_imports + i as u32;
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

  // Second pass: compile. If a function fails, we still reserve its slot
  // with a trivial body so indices remain stable.
  let mut compiled_fns: Vec<CompiledFn> = Vec::new();
  for (ns, def_name, args, body) in &fn_defs {
    match compile_fn(def_name, args, body, &fn_index, &fn_arity, &fn_has_rest, &tag_index) {
      Ok(func) => compiled_fns.push(func),
      Err(e) => {
        eprintln!("[wasm] skipping {ns}/{def_name}: {e}");
        let (arity, _) = compute_fn_arity(args);
        compiled_fns.push(CompiledFn {
          name: def_name.clone(),
          arity: arity as usize,
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
  let wasm_bytes = build_wasm_module(&compiled_fns)?;

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
  name: String,
  arity: usize,
  /// All local variables (including temporaries), indexed by declaration order
  locals: Vec<ValType>,
  /// Instruction sequence for the function body
  instructions: Vec<Instruction<'static>>,
}

/// Build a binary WASM module from compiled functions.
/// Host-imported function specification.
struct HostImport {
  module: &'static str,
  name: &'static str,
  arity: usize,
}

/// List of host-imported math functions.
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
];

/// Build a binary WASM module from compiled functions.
/// Host imports occupy the first function indices (0..HOST_IMPORTS.len()),
/// then user functions follow at indices HOST_IMPORTS.len()..
fn build_wasm_module(fns: &[CompiledFn]) -> Result<Vec<u8>, String> {
  let mut module = Module::new();
  let num_imports = HOST_IMPORTS.len() as u32;

  // Type section: host imports first, then user functions
  let mut types = TypeSection::new();
  for imp in HOST_IMPORTS {
    let params: Vec<ValType> = vec![ValType::F64; imp.arity];
    types.ty().function(params, vec![ValType::F64]);
  }
  for f in fns {
    let params: Vec<ValType> = vec![ValType::F64; f.arity];
    types.ty().function(params, vec![ValType::F64]);
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
    &ConstExpr::i32_const(HEAP_START),
  );
  module.section(&globals);

  // Export section: user functions (indices offset by num_imports)
  let mut exports = ExportSection::new();
  exports.export("memory", ExportKind::Memory, 0);
  for (i, f) in fns.iter().enumerate() {
    exports.export(&f.name, ExportKind::Func, num_imports + i as u32);
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

  Ok(module.finish())
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
  /// Tag name → integer ID map (compile-time constant, shared across all functions)
  tag_index: HashMap<String, u32>,
  /// Current block nesting depth relative to the recur loop
  /// (0 = directly inside the loop, 1 = inside one if/block, etc.)
  block_depth: u32,
}

impl WasmGenCtx {
  fn new(
    num_params: u32,
    fn_index: HashMap<String, u32>,
    fn_arity: HashMap<String, u32>,
    fn_has_rest: HashMap<String, u32>,
    tag_index: HashMap<String, u32>,
  ) -> Self {
    WasmGenCtx {
      locals: HashMap::new(),
      extra_locals: Vec::new(),
      next_local: num_params,
      uses_recur: false,
      arg_indices: Vec::new(),
      instructions: Vec::new(),
      fn_index,
      fn_arity,
      fn_has_rest,
      tag_index,
      block_depth: 0,
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
  name: &str,
  args: &CalcitFnArgs,
  body: &[Calcit],
  fn_index: &HashMap<String, u32>,
  fn_arity: &HashMap<String, u32>,
  fn_has_rest: &HashMap<String, u32>,
  tag_index: &HashMap<String, u32>,
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
  let mut ctx = WasmGenCtx::new(
    arity as u32,
    fn_index.clone(),
    fn_arity.clone(),
    fn_has_rest.clone(),
    tag_index.clone(),
  );

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
    name: name.to_owned(),
    arity,
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
    Calcit::Str(_) => return Err("String values not yet supported in WASM codegen".into()),
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
    Calcit::Import(import) => {
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
      let fn_idx = *ctx.fn_index.get(sym.as_ref()).ok_or_else(|| format!("unknown function: {sym}"))?;
      let target_arity = ctx.fn_arity.get(sym.as_ref()).copied().unwrap_or(args_list.len() as u32);
      let rest_fixed = ctx.fn_has_rest.get(sym.as_ref()).copied();
      emit_call_args(ctx, &args_list, target_arity, rest_fixed)?;
      ctx.emit(Instruction::Call(fn_idx));
      Ok(())
    }
    _ => Err(format!("unsupported call head in WASM: {head}")),
  }
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
    CalcitProc::NativeRecordCount => emit_record_count(args),
    CalcitProc::NativeRecordAssoc | CalcitProc::NativeRecordAssocAt | CalcitProc::NativeRecordWith => {
      Err("Record mutation (assoc/with) not yet supported in WASM codegen".into())
    }
    CalcitProc::NativeRecordFromMap
    | CalcitProc::NativeRecordToMap
    | CalcitProc::NativeRecordExtendAs
    | CalcitProc::NativeRecordPartial
    | CalcitProc::NativeRecordMatches
    | CalcitProc::NativeRecordContains
    | CalcitProc::NativeRecordStruct
    | CalcitProc::NativeRecordGetName
    | CalcitProc::NativeRecordImpls
    | CalcitProc::NativeRecordWithAt
    | CalcitProc::NativeLooseRecord => Err(format!("Record operation {proc} not yet supported in WASM codegen")),

    // Tuple operations
    CalcitProc::NativeTuple => emit_tuple_new(ctx, args),
    CalcitProc::NativeTupleNth => emit_tuple_nth(ctx, args),
    CalcitProc::NativeTupleCount => emit_tuple_count(args),
    CalcitProc::NativeEnumTupleNew
    | CalcitProc::NativeTupleAssoc
    | CalcitProc::NativeTupleImpls
    | CalcitProc::NativeTupleParams
    | CalcitProc::NativeTupleEnum
    | CalcitProc::NativeTupleImplTraits
    | CalcitProc::NativeTupleEnumHasVariant
    | CalcitProc::NativeTupleEnumVariantArity
    | CalcitProc::NativeTupleValidateEnum => Err(format!("Tuple operation {proc} not yet supported in WASM codegen")),

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

    // ------- Set operations -------
    CalcitProc::Set => emit_set_new(ctx, args),
    CalcitProc::NativeInclude => emit_set_include(ctx, args),
    CalcitProc::NativeExclude => emit_set_exclude(ctx, args),
    CalcitProc::NativeSetCount => emit_ds_count(ctx, args),
    CalcitProc::NativeSetEmpty => emit_ds_empty(ctx, args),
    CalcitProc::NativeSetIncludes => emit_set_includes(ctx, args),

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
/// - value must be within `[HEAP_START + 8, current_heap_ptr)` (logical ptrs begin
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
  // 2) v >= (HEAP_START + 8) as f64
  ctx.emit(Instruction::LocalGet(v_local));
  ctx.emit(f64_const((HEAP_START + 8) as f64));
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

  // Convert to i32 for memory access and load the tag_id (f64 at offset 0)
  let tag_local = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(ptr_f64));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
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
      // Payload at offset (1 + bind_idx) * 8 from tuple pointer
      let offset = ((1 + bind_idx) * 8) as u64;
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
  "list", "map", "set", "tuple", "record", "number", "bool", "nil", "tag", "fn", "string",
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

/// Emit `&%{} struct_ref :field1 val1 :field2 val2 ...` — allocate a Record in linear memory.
///
/// The preprocessed form is: [NativeRecord, struct_ref, :tag1, val1, :tag2, val2, ...]
/// where struct_ref is either Calcit::Struct or Calcit::Import pointing to the struct def.
///
/// Memory layout: [struct_tag_id: f64] [field_0: f64] [field_1: f64] ...
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

  // Total bytes: (1 + field_count) * 8
  let total_size = ((1 + field_count) * 8) as i32;

  // Allocate: save i32 pointer to a temporary local
  let ptr_local = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, total_size, ptr_local, "record");

  // Store struct tag at offset 0
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(f64_const(struct_tag_id as f64));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  // Store each field value at offset (1 + i) * 8
  // field_args layout: [:tag0, val0, :tag1, val1, ...]
  for i in 0..field_count {
    let value_expr = &field_args[i * 2 + 1]; // skip the tag, take the value
    ctx.emit(Instruction::LocalGet(ptr_local));
    emit_expr(ctx, value_expr)?;
    ctx.emit(Instruction::F64Store(mem_arg_f64(((1 + i) * 8) as u64)));
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
  let idx = match &args[1] {
    Calcit::Number(n) => *n as usize,
    other => return Err(format!("&record:nth index must be a number literal, got: {other}")),
  };
  // Field is at byte offset (1 + idx) * 8 from the record pointer
  let offset = ((1 + idx) * 8) as u64;

  // Evaluate record expression → f64 pointer
  emit_expr(ctx, &args[0])?;
  // Convert f64 pointer to i32
  ctx.emit(Instruction::I32TruncF64U);
  // Load f64 value at the field offset
  ctx.emit(Instruction::F64Load(mem_arg_f64(offset)));
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
/// Since this is known at compile time via struct definition, it could be optimized.
/// For now, return an error indicating the caller should use the preprocessed form.
fn emit_record_count(args: &[Calcit]) -> Result<(), String> {
  let _ = args;
  Err("&record:count not yet supported in WASM codegen".into())
}

// ---------------------------------------------------------------------------
// Tuple operations
// ---------------------------------------------------------------------------

/// Emit `:: tag val0 val1 ...` — allocate a Tuple in linear memory.
///
/// Memory layout: [tag_id: f64] [payload_0: f64] [payload_1: f64] ...
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
  let total_size = ((1 + payload.len()) * 8) as i32;

  let ptr_local = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, total_size, ptr_local, "tuple");

  // Store tag at offset 0
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(f64_const(tag_id as f64));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  // Store payload fields
  for (i, val) in payload.iter().enumerate() {
    ctx.emit(Instruction::LocalGet(ptr_local));
    emit_expr(ctx, val)?;
    ctx.emit(Instruction::F64Store(mem_arg_f64(((1 + i) * 8) as u64)));
  }

  // Return pointer as f64
  ctx.emit(Instruction::LocalGet(ptr_local));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// Emit `&tuple:nth tuple idx` — O(1) payload access by index.
fn emit_tuple_nth(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&tuple:nth requires 2 args (tuple, index)".into());
  }
  let idx = match &args[1] {
    Calcit::Number(n) => *n as usize,
    other => return Err(format!("&tuple:nth index must be a number literal, got: {other}")),
  };
  // Payload field is at offset (1 + idx) * 8
  let offset = ((1 + idx) * 8) as u64;

  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::F64Load(mem_arg_f64(offset)));
  Ok(())
}

/// Emit `&tuple:count tuple` — not yet supported.
fn emit_tuple_count(args: &[Calcit]) -> Result<(), String> {
  let _ = args;
  Err("&tuple:count not yet supported in WASM codegen".into())
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
  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(i));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));

  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::LocalGet(n));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1)); // exit block

  // dst_base + i*8
  ctx.emit(Instruction::LocalGet(dst_base));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  // load src_base + i*8
  ctx.emit(Instruction::LocalGet(src_base));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  // i++
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(i));
  ctx.emit(Instruction::Br(0)); // continue loop

  ctx.emit(Instruction::End); // end loop
  ctx.emit(Instruction::End); // end block
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
// Map operations — layout: [count:f64] [k0:f64] [v0:f64] [k1:f64] [v1:f64] ...
// ===========================================================================

/// `&{} key0 val0 key1 val1 ...` — create a map (key-value pairs).
fn emit_map_new(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() % 2 != 0 {
    return Err("&{} expects even number of args (key-value pairs)".into());
  }
  let count = args.len() / 2;
  let total_bytes = ((1 + count * 2) * 8) as i32;
  let ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, total_bytes, ptr, "map");

  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(f64_const(count as f64));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  for i in 0..count {
    // key at offset 8 + i*16
    ctx.emit(Instruction::LocalGet(ptr));
    emit_expr(ctx, &args[i * 2])?;
    ctx.emit(Instruction::F64Store(mem_arg_f64((8 + i * 16) as u64)));
    // value at offset 16 + i*16
    ctx.emit(Instruction::LocalGet(ptr));
    emit_expr(ctx, &args[i * 2 + 1])?;
    ctx.emit(Instruction::F64Store(mem_arg_f64((16 + i * 16) as u64)));
  }

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
  let count = emit_load_count_i32(ctx, ptr);
  let target = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(target));

  let result = ctx.alloc_local();
  ctx.emit(f64_const(0.0));
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

  // Key at ptr + 8 + i*16
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(target));
  ctx.emit(Instruction::F64Eq);

  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  // Value at ptr + 16 + i*16
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::Br(2)); // break outer block
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(i));
  ctx.emit(Instruction::Br(0));

  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

/// `&map:contains? map key` — scan for key, return bool.
fn emit_map_contains(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&map:contains? expects 2 args".into());
  }
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, ptr);
  let target = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(target));

  let result = ctx.alloc_local();
  ctx.emit(f64_const(0.0));
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

  // Key at ptr + 8 + i*16
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(target));
  ctx.emit(Instruction::F64Eq);

  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(i));
  ctx.emit(Instruction::Br(0));

  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

/// `&map:includes? map value` — scan values for match.
fn emit_map_includes(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&map:includes? expects 2 args".into());
  }
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, ptr);
  let target = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(target));

  let result = ctx.alloc_local();
  ctx.emit(f64_const(0.0));
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

  // Value at ptr + 16 + i*16
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(target));
  ctx.emit(Instruction::F64Eq);

  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(i));
  ctx.emit(Instruction::Br(0));

  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

/// `&map:assoc map key value` — new map with key-value added or updated.
fn emit_map_assoc(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 3 {
    return Err("&map:assoc expects 3 args".into());
  }
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);
  let key = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(key));
  let val = ctx.alloc_local();
  emit_expr(ctx, &args[2])?;
  ctx.emit(Instruction::LocalSet(val));

  // Scan for existing key → found_idx (-1 if not found)
  let found_idx = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(-1));
  ctx.emit(Instruction::LocalSet(found_idx));

  let si = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(si));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(si));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(si));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(key));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(si));
  ctx.emit(Instruction::LocalSet(found_idx));
  ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(si));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(si));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // Branch: found or not-found
  let dst = ctx.alloc_local_typed(ValType::I32);

  ctx.emit(Instruction::LocalGet(found_idx));
  ctx.emit(Instruction::I32Const(-1));
  ctx.emit(Instruction::I32Eq);

  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  {
    // Not found: new map with count+1 entries
    let nc = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(count));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(nc));
    let ts = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(nc));
    ctx.emit(Instruction::I32Const(2));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(ts));
    let d = emit_alloc_with_count(ctx, nc, ts, "map");
    ctx.emit(Instruction::LocalGet(d));
    ctx.emit(Instruction::LocalSet(dst));

    // Copy existing entries (count * 2 f64 values)
    let copy_n = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(count));
    ctx.emit(Instruction::I32Const(2));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::LocalSet(copy_n));
    let db = emit_addr_offset(ctx, d, 8);
    let sb = emit_addr_offset(ctx, src, 8);
    emit_copy_f64_loop(ctx, db, sb, copy_n);

    // Append new key-value at end: offset 8 + count*16
    ctx.emit(Instruction::LocalGet(d));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(count));
    ctx.emit(Instruction::I32Const(16));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    let kaddr = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalTee(kaddr));
    ctx.emit(Instruction::LocalGet(key));
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
    ctx.emit(Instruction::LocalGet(kaddr));
    ctx.emit(Instruction::LocalGet(val));
    ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
  }
  ctx.emit(Instruction::Else);
  {
    // Found: new map with same count, update value at found_idx
    let ts = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(count));
    ctx.emit(Instruction::I32Const(2));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(ts));
    let d = emit_alloc_with_count(ctx, count, ts, "map");
    ctx.emit(Instruction::LocalGet(d));
    ctx.emit(Instruction::LocalSet(dst));

    let copy_n = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(count));
    ctx.emit(Instruction::I32Const(2));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::LocalSet(copy_n));
    let db = emit_addr_offset(ctx, d, 8);
    let sb = emit_addr_offset(ctx, src, 8);
    emit_copy_f64_loop(ctx, db, sb, copy_n);

    // Overwrite value at found_idx: offset 16 + found_idx*16
    ctx.emit(Instruction::LocalGet(d));
    ctx.emit(Instruction::I32Const(16));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(found_idx));
    ctx.emit(Instruction::I32Const(16));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(val));
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  }
  ctx.emit(Instruction::End); // end if

  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `&map:dissoc map key` — new map without key (or same if key absent).
fn emit_map_dissoc(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 2 {
    return Err("&map:dissoc expects 2 args".into());
  }
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);
  let target = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(target));

  // Scan for key → found_idx
  let found_idx = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(-1));
  ctx.emit(Instruction::LocalSet(found_idx));

  let si = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(si));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(si));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(si));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(target));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(si));
  ctx.emit(Instruction::LocalSet(found_idx));
  ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(si));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(si));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // If not found, return original map pointer
  let dst = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::LocalSet(dst)); // default: return original

  ctx.emit(Instruction::LocalGet(found_idx));
  ctx.emit(Instruction::I32Const(-1));
  ctx.emit(Instruction::I32Ne);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  {
    // Found: create new map with count-1
    let nc = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(count));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Sub);
    ctx.emit(Instruction::LocalSet(nc));
    let ts = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(nc));
    ctx.emit(Instruction::I32Const(2));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(ts));
    let d = emit_alloc_with_count(ctx, nc, ts, "map");
    ctx.emit(Instruction::LocalGet(d));
    ctx.emit(Instruction::LocalSet(dst));

    // Copy entries before found_idx (found_idx * 2 f64 values)
    let before_n = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(found_idx));
    ctx.emit(Instruction::I32Const(2));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::LocalSet(before_n));
    let db1 = emit_addr_offset(ctx, d, 8);
    let sb1 = emit_addr_offset(ctx, src, 8);
    emit_copy_f64_loop(ctx, db1, sb1, before_n);

    // Copy entries after found_idx: (count - found_idx - 1) * 2
    let after_n = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(count));
    ctx.emit(Instruction::LocalGet(found_idx));
    ctx.emit(Instruction::I32Sub);
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Sub);
    ctx.emit(Instruction::I32Const(2));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::LocalSet(after_n));

    // dst_base2 = d + 8 + found_idx*16
    let db2 = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(d));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(found_idx));
    ctx.emit(Instruction::I32Const(16));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(db2));

    // src_base2 = src + 8 + (found_idx+1)*16
    let sb2 = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(src));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(found_idx));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::I32Const(16));
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

/// `to-pairs map` — convert map to list of 2-element lists.
fn emit_map_to_pairs(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err("to-pairs expects 1 arg".into());
  }
  let map_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, map_ptr);

  // Outer list: [count, pair_ptr_0, pair_ptr_1, ...]
  let outer_ts = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(outer_ts));
  let outer = emit_alloc_with_count(ctx, count, outer_ts, "list");

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
  ctx.emit(Instruction::LocalGet(map_ptr));
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
  ctx.emit(Instruction::LocalGet(map_ptr));
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
  // Same scan pattern as list_includes (same layout)
  emit_list_includes(ctx, args)
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

  // Check if already present
  let found = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(found));

  let si = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(si));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(si));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(si));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(elem));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(found));
  ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(si));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(si));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  let dst = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::LocalSet(dst)); // default: return same

  ctx.emit(Instruction::LocalGet(found));
  ctx.emit(Instruction::I32Eqz);
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

  // Scan for value → found_idx
  let found_idx = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(-1));
  ctx.emit(Instruction::LocalSet(found_idx));
  let si = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(si));
  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(si));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(si));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(target));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(si));
  ctx.emit(Instruction::LocalSet(found_idx));
  ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(si));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(si));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

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
