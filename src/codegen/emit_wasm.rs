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

/// Emit a WASM binary module from the compiled program.
/// Only processes functions from the namespace that contains the init entry.
pub fn emit_wasm(init_ns: &str, emit_path: &str) -> Result<(), String> {
  let program_data = program::clone_compiled_program_snapshot()?;

  let mut compiled_fns: Vec<CompiledFn> = Vec::new();

  if let Some(file_info) = program_data.get(init_ns) {
    // First pass: extract all function signatures
    let mut fn_defs: Vec<(String, CalcitFnArgs, Vec<Calcit>)> = Vec::new();
    for (def_name, compiled) in &file_info.defs {
      if compiled.kind != program::CompiledDefKind::Fn {
        continue;
      }
      match extract_fn_parts(&compiled.preprocessed_code) {
        Ok((args, body)) => {
          fn_defs.push((def_name.to_string(), args, body));
        }
        Err(e) => {
          eprintln!("[wasm] skipping {init_ns}/{def_name}: {e}");
        }
      }
    }

    // Build provisional index map (all functions)
    let fn_index: HashMap<String, u32> = fn_defs
      .iter()
      .enumerate()
      .map(|(i, (name, _, _))| (name.clone(), i as u32))
      .collect();

    // Collect all tags referenced in function bodies
    let tag_index = collect_all_tags(&fn_defs);

    // Second pass: compile. If a function fails, we still reserve its slot
    // with a trivial body so indices remain stable.
    for (def_name, args, body) in &fn_defs {
      match compile_fn(def_name, args, body, &fn_index, &tag_index) {
        Ok(func) => compiled_fns.push(func),
        Err(e) => {
          eprintln!("[wasm] skipping {init_ns}/{def_name}: {e}");
          // Insert a stub function to maintain index stability
          let arity = match args {
            CalcitFnArgs::Args(v) => v.len(),
            CalcitFnArgs::MarkedArgs(v) => v.len(),
          };
          compiled_fns.push(CompiledFn {
            name: def_name.clone(),
            arity,
            locals: vec![],
            instructions: vec![f64_const(0.0)],
          });
        }
      }
    }
  } else {
    return Err(format!("namespace not found: {init_ns}"));
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
fn build_wasm_module(fns: &[CompiledFn]) -> Result<Vec<u8>, String> {
  let mut module = Module::new();

  // Type section: each function gets its own type (all f64 params/result)
  let mut types = TypeSection::new();
  for f in fns {
    let params: Vec<ValType> = vec![ValType::F64; f.arity];
    types.ty().function(params, vec![ValType::F64]);
  }
  module.section(&types);

  // Function section: map each function to its type
  let mut functions = FunctionSection::new();
  for (i, _) in fns.iter().enumerate() {
    functions.function(i as u32);
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

  // Export section
  let mut exports = ExportSection::new();
  exports.export("memory", ExportKind::Memory, 0);
  for (i, f) in fns.iter().enumerate() {
    exports.export(&f.name, ExportKind::Func, i as u32);
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
  /// Tag name → integer ID map (compile-time constant, shared across all functions)
  tag_index: HashMap<String, u32>,
  /// Current block nesting depth relative to the recur loop
  /// (0 = directly inside the loop, 1 = inside one if/block, etc.)
  block_depth: u32,
}

impl WasmGenCtx {
  fn new(num_params: u32, fn_index: HashMap<String, u32>, tag_index: HashMap<String, u32>) -> Self {
    WasmGenCtx {
      locals: HashMap::new(),
      extra_locals: Vec::new(),
      next_local: num_params,
      uses_recur: false,
      arg_indices: Vec::new(),
      instructions: Vec::new(),
      fn_index,
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

fn compile_fn(
  name: &str,
  args: &CalcitFnArgs,
  body: &[Calcit],
  fn_index: &HashMap<String, u32>,
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
      for label in labels {
        match label {
          CalcitArgLabel::Idx(idx) => {
            param_names.push(CalcitLocal::read_name(*idx));
          }
          CalcitArgLabel::OptionalMark | CalcitArgLabel::RestMark => {
            return Err("optional/rest args not supported in WASM codegen".into());
          }
        }
      }
    }
  }

  let arity = param_names.len();
  let mut ctx = WasmGenCtx::new(arity as u32, fn_index.clone(), tag_index.clone());

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
      CalcitSyntax::Defn => Err("nested defn not supported in WASM".into()),
      _ => Err(format!("unsupported syntax in WASM: {syn}")),
    },
    Calcit::Proc(proc) => emit_proc_call(ctx, proc, &args_list),
    Calcit::Import(import) => {
      let fn_idx = *ctx
        .fn_index
        .get(import.def.as_ref())
        .ok_or_else(|| format!("unknown function: {}", import.def))?;
      for arg in &args_list {
        emit_expr(ctx, arg)?;
      }
      ctx.emit(Instruction::Call(fn_idx));
      Ok(())
    }
    Calcit::Symbol { sym, .. } => {
      let fn_idx = *ctx.fn_index.get(sym.as_ref()).ok_or_else(|| format!("unknown function: {sym}"))?;
      for arg in &args_list {
        emit_expr(ctx, arg)?;
      }
      ctx.emit(Instruction::Call(fn_idx));
      Ok(())
    }
    _ => Err(format!("unsupported call head in WASM: {head}")),
  }
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
    CalcitProc::Sin | CalcitProc::Cos => Err(format!("trigonometric function {proc} not available in WASM (no f64.sin/cos)")),
    CalcitProc::Pow => Err("pow not yet supported in WASM codegen (no f64.pow instruction)".into()),

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

/// Collect all Tag values from function bodies and build a compile-time tag→id map.
/// Tag IDs start at 1 (0 is unused/reserved).
fn collect_all_tags(fn_defs: &[(String, CalcitFnArgs, Vec<Calcit>)]) -> HashMap<String, u32> {
  let mut tags: Vec<String> = Vec::new();
  for (_, _, body) in fn_defs {
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
  emit_bump_alloc(ctx, total_size, ptr_local);

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
  emit_bump_alloc(ctx, total_size, ptr_local);

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
/// ```wasm
/// global.get $heap_ptr        ;; [i32:old_ptr]
/// local.tee $ptr_local        ;; [i32:old_ptr] (saved)
/// i32.const <byte_size>       ;; [i32:old_ptr, i32:size]
/// i32.add                     ;; [i32:new_ptr]
/// global.set $heap_ptr        ;; [] (bumped)
/// ```
fn emit_bump_alloc(ctx: &mut WasmGenCtx, byte_size: i32, ptr_local: u32) {
  ctx.emit(Instruction::GlobalGet(HEAP_PTR_GLOBAL));
  ctx.emit(Instruction::LocalTee(ptr_local));
  ctx.emit(Instruction::I32Const(byte_size));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::GlobalSet(HEAP_PTR_GLOBAL));
}
