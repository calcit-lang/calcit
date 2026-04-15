//! Minimal WASM codegen for Calcit — generates WAT (WebAssembly Text format).
//!
//! Supports a small subset of Calcit for demonstration purposes:
//! - `defn` with fixed-arity arguments (all f64)
//! - `let` bindings
//! - `if` conditionals
//! - Arithmetic: `&+`, `&-`, `&*`, `&/`, `&number:rem`
//! - Comparisons: `&<`, `&>`, `&=`
//! - `recur` (tail recursion via WASM loop)
//! - Number literals, Bool literals, Nil (→ 0.0)
//!
//! All values are represented as f64 (matching Calcit's single numeric type).
//! Booleans: true → 1.0, false/nil → 0.0.
//! Output is a `.wat` file that can be compiled with `wat2wasm` (from wabt)
//! or run directly with `wasmtime`.

use std::collections::HashMap;
use std::fmt::Write;
use std::fs;
use std::path::Path;

use crate::builtins::syntax::get_raw_args_fn;
use crate::calcit::{Calcit, CalcitArgLabel, CalcitFnArgs, CalcitLocal, CalcitProc, CalcitSyntax};
use crate::program;

/// Emit a WAT module from the compiled program.
/// Only processes functions from the namespace that contains the init entry.
pub fn emit_wasm(init_ns: &str, emit_path: &str) -> Result<(), String> {
  let program_data = program::clone_compiled_program_snapshot()?;

  let mut functions: Vec<WasmFunc> = Vec::new();

  if let Some(file_info) = program_data.get(init_ns) {
    for (def_name, compiled) in &file_info.defs {
      if compiled.kind != program::CompiledDefKind::Fn {
        continue;
      }
      match extract_fn_parts(&compiled.preprocessed_code) {
        Ok((args, body)) => match gen_wasm_func(def_name, &args, &body) {
          Ok(func) => functions.push(func),
          Err(e) => {
            eprintln!("[wasm] skipping {init_ns}/{def_name}: {e}");
          }
        },
        Err(e) => {
          eprintln!("[wasm] skipping {init_ns}/{def_name}: {e}");
        }
      }
    }
  } else {
    return Err(format!("namespace not found: {init_ns}"));
  }

  if functions.is_empty() {
    return Err("no functions could be compiled to WASM".into());
  }

  // Build module
  let mut wat = String::from("(module\n");
  for func in &functions {
    wat.push_str(&func.wat);
    wat.push('\n');
  }
  // Export all compiled functions
  for func in &functions {
    writeln!(wat, "  (export \"{}\" (func ${}))", func.name, func.name).expect("write");
  }
  wat.push_str(")\n");

  // Write output
  let out_path = Path::new(emit_path);
  if !out_path.exists() {
    fs::create_dir_all(out_path).map_err(|e| format!("failed to create dir: {e}"))?;
  }
  let wat_file = out_path.join("program.wat");
  fs::write(&wat_file, &wat).map_err(|e| format!("failed to write WAT: {e}"))?;
  println!("wrote WAT to: {}", wat_file.display());

  Ok(())
}

struct WasmFunc {
  name: String,
  wat: String,
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
  /// Map from local variable name to WASM local index name
  locals: HashMap<String, String>,
  /// Local declarations to emit at function start
  local_decls: Vec<String>,
  /// Counter for generating unique local names
  local_counter: usize,
  /// Whether this function uses recur (needs loop wrapping)
  uses_recur: bool,
  /// Argument names in order (for recur)
  arg_names: Vec<String>,
}

impl WasmGenCtx {
  fn new() -> Self {
    WasmGenCtx {
      locals: HashMap::new(),
      local_decls: Vec::new(),
      local_counter: 0,
      uses_recur: false,
      arg_names: Vec::new(),
    }
  }

  fn fresh_local(&mut self, hint: &str) -> String {
    let name = format!("$_{}_{}", hint, self.local_counter);
    self.local_counter += 1;
    name
  }

  fn declare_local(&mut self, name: &str) -> String {
    let wasm_name = self.fresh_local(name);
    self.local_decls.push(format!("(local {wasm_name} f64)"));
    self.locals.insert(name.to_owned(), wasm_name.clone());
    wasm_name
  }
}

fn gen_wasm_func(name: &str, args: &CalcitFnArgs, body: &[Calcit]) -> Result<WasmFunc, String> {
  let mut ctx = WasmGenCtx::new();

  // Process arguments — only simple fixed-arity args supported
  let mut params = Vec::new();
  match args {
    CalcitFnArgs::Args(idxs) => {
      for idx in idxs {
        let sym = CalcitLocal::read_name(*idx);
        let wasm_name = format!("${sym}");
        ctx.locals.insert(sym.clone(), wasm_name.clone());
        ctx.arg_names.push(wasm_name.clone());
        params.push(format!("(param {wasm_name} f64)"));
      }
    }
    CalcitFnArgs::MarkedArgs(labels) => {
      for label in labels {
        match label {
          CalcitArgLabel::Idx(idx) => {
            let sym = CalcitLocal::read_name(*idx);
            let wasm_name = format!("${sym}");
            ctx.locals.insert(sym.clone(), wasm_name.clone());
            ctx.arg_names.push(wasm_name.clone());
            params.push(format!("(param {wasm_name} f64)"));
          }
          CalcitArgLabel::OptionalMark | CalcitArgLabel::RestMark => {
            return Err("optional/rest args not supported in WASM codegen".into());
          }
        }
      }
    }
  }

  // Check if body uses recur
  ctx.uses_recur = body.iter().any(check_uses_recur);

  // Generate body
  let body_code = gen_body(&mut ctx, body)?;

  let params_str = params.join(" ");
  let locals_str = ctx.local_decls.join("\n    ");

  let func_body = if ctx.uses_recur {
    // Wrap in (loop $recur ...)
    format!("    {locals_str}\n    (loop $recur (result f64)\n      {body_code}\n    )",)
  } else {
    format!("    {locals_str}\n    {body_code}")
  };

  let wat = format!("  (func ${name} {params_str} (result f64)\n{func_body}\n  )",);

  Ok(WasmFunc {
    name: name.to_owned(),
    wat,
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

/// Generate WASM code for a sequence of expressions (last one is the return value).
fn gen_body(ctx: &mut WasmGenCtx, exprs: &[Calcit]) -> Result<String, String> {
  if exprs.is_empty() {
    return Ok("(f64.const 0)".into());
  }
  let mut parts = Vec::new();
  for (i, expr) in exprs.iter().enumerate() {
    if i == exprs.len() - 1 {
      // Last expression is the return value
      parts.push(gen_expr(ctx, expr)?);
    } else {
      // Non-last expressions: evaluate and drop result
      let code = gen_expr(ctx, expr)?;
      parts.push(format!("(drop {code})"));
    }
  }
  Ok(parts.join("\n      "))
}

/// Generate WASM expression code for a single Calcit node.
fn gen_expr(ctx: &mut WasmGenCtx, expr: &Calcit) -> Result<String, String> {
  match expr {
    Calcit::Number(n) => Ok(format!("(f64.const {n})")),
    Calcit::Bool(true) => Ok("(f64.const 1)".into()),
    Calcit::Bool(false) | Calcit::Nil => Ok("(f64.const 0)".into()),

    Calcit::Local(local) => {
      let name = &*local.sym;
      match ctx.locals.get(name) {
        Some(wasm_name) => Ok(format!("(local.get {wasm_name})")),
        None => Err(format!("undefined local variable: {name}")),
      }
    }

    Calcit::List(xs) if !xs.is_empty() => gen_call_expr(ctx, xs),

    _ => Err(format!("unsupported WASM expression: {expr}")),
  }
}

/// Generate WASM code for a call expression (list with head + args).
fn gen_call_expr(ctx: &mut WasmGenCtx, xs: &crate::calcit::CalcitList) -> Result<String, String> {
  let head = &xs[0];
  let args_list: Vec<Calcit> = xs.drop_left().to_vec();
  let args_slice = &args_list;

  match head {
    // Syntax forms
    Calcit::Syntax(syn, _) => match syn {
      CalcitSyntax::If => gen_if(ctx, &args_list),
      CalcitSyntax::CoreLet => gen_let(ctx, &args_list),
      CalcitSyntax::Defn => Err("nested defn not supported in WASM".into()),
      _ => Err(format!("unsupported syntax in WASM: {syn}")),
    },

    // Builtin procs
    Calcit::Proc(proc) => gen_proc_call(ctx, proc, args_slice),

    // Function calls (imports or local defs)
    Calcit::Import(import) => {
      let mut arg_codes = Vec::new();
      for arg in args_slice {
        arg_codes.push(gen_expr(ctx, arg)?);
      }
      Ok(format!("(call ${} {})", import.def, arg_codes.join(" ")))
    }

    // Symbol-based calls (for self-recursion or unresolved)
    Calcit::Symbol { sym, .. } => {
      let mut arg_codes = Vec::new();
      for arg in args_slice {
        arg_codes.push(gen_expr(ctx, arg)?);
      }
      Ok(format!("(call ${sym} {})", arg_codes.join(" ")))
    }

    _ => Err(format!("unsupported call head in WASM: {head}")),
  }
}

/// Generate WASM for builtin proc calls.
fn gen_proc_call(ctx: &mut WasmGenCtx, proc: &CalcitProc, args: &[Calcit]) -> Result<String, String> {
  match proc {
    // Arithmetic
    CalcitProc::NativeAdd => binary_op(ctx, "f64.add", args),
    CalcitProc::NativeMinus => binary_op(ctx, "f64.sub", args),
    CalcitProc::NativeMultiply => binary_op(ctx, "f64.mul", args),
    CalcitProc::NativeDivide => binary_op(ctx, "f64.div", args),
    CalcitProc::NativeNumberRem => {
      // WASM doesn't have f64.rem, so we use: a - trunc(a/b) * b
      if args.len() != 2 {
        return Err("rem expects 2 args".into());
      }
      let a = gen_expr(ctx, &args[0])?;
      let b = gen_expr(ctx, &args[1])?;
      Ok(format!("(f64.sub {a} (f64.mul (f64.trunc (f64.div {a} {b})) {b}))"))
    }

    // Comparisons — produce f64 (1.0 for true, 0.0 for false)
    CalcitProc::NativeLessThan => cmp_op(ctx, "f64.lt", args),
    CalcitProc::NativeGreaterThan => cmp_op(ctx, "f64.gt", args),
    CalcitProc::NativeEquals => cmp_op(ctx, "f64.eq", args),
    CalcitProc::Not => {
      if args.len() != 1 {
        return Err("not expects 1 arg".into());
      }
      let a = gen_expr(ctx, &args[0])?;
      // not: 0.0 → 1.0, anything else → 0.0
      Ok(format!("(select (f64.const 1) (f64.const 0) (f64.eq {a} (f64.const 0)))"))
    }

    // Math functions (unary)
    CalcitProc::Floor => unary_op(ctx, "f64.floor", args),
    CalcitProc::Ceil => unary_op(ctx, "f64.ceil", args),
    CalcitProc::Round => unary_op(ctx, "f64.nearest", args),
    CalcitProc::Sqrt => unary_op(ctx, "f64.sqrt", args),
    CalcitProc::Sin | CalcitProc::Cos => {
      // WASM has no built-in sin/cos; reject for now
      Err(format!("trigonometric function {proc} not available in WASM (no f64.sin/cos)"))
    }
    CalcitProc::Pow => {
      // a^b: no direct WASM op; we import Math.pow at module level or reject
      // For now, compile as repeated multiply for small integer exponents,
      // or reject for general case.
      Err("pow not yet supported in WASM codegen (no f64.pow instruction)".into())
    }
    CalcitProc::Identical => cmp_op(ctx, "f64.eq", args),

    // Recur — tail call via br to loop
    CalcitProc::Recur => {
      if args.len() != ctx.arg_names.len() {
        return Err(format!(
          "recur arity mismatch: expected {}, got {}",
          ctx.arg_names.len(),
          args.len()
        ));
      }
      let mut code = String::new();
      // Evaluate all args first (into temp locals to avoid order issues)
      let mut temps = Vec::new();
      for arg in args.iter() {
        let arg_code = gen_expr(ctx, arg)?;
        let tmp = ctx.fresh_local("recur_tmp");
        ctx.local_decls.push(format!("(local {tmp} f64)"));
        writeln!(code, "(local.set {tmp} {arg_code})").expect("write");
        temps.push(tmp);
      }
      // Assign temps back to params
      for (i, tmp) in temps.iter().enumerate() {
        writeln!(code, "(local.set {} (local.get {tmp}))", ctx.arg_names[i]).expect("write");
      }
      code.push_str("(br $recur)");
      // br $recur branches unconditionally; code after is unreachable.
      // Wrap in a block to satisfy the type checker:
      Ok(format!("(block (result f64)\n{code}\n(f64.const 0)\n)"))
    }

    _ => Err(format!("unsupported proc in WASM: {proc}")),
  }
}

fn unary_op(ctx: &mut WasmGenCtx, op: &str, args: &[Calcit]) -> Result<String, String> {
  if args.len() != 1 {
    return Err(format!("{op} expects 1 arg, got {}", args.len()));
  }
  let a = gen_expr(ctx, &args[0])?;
  Ok(format!("({op} {a})"))
}

fn binary_op(ctx: &mut WasmGenCtx, op: &str, args: &[Calcit]) -> Result<String, String> {
  if args.len() != 2 {
    return Err(format!("{op} expects 2 args, got {}", args.len()));
  }
  let a = gen_expr(ctx, &args[0])?;
  let b = gen_expr(ctx, &args[1])?;
  Ok(format!("({op} {a} {b})"))
}

fn cmp_op(ctx: &mut WasmGenCtx, op: &str, args: &[Calcit]) -> Result<String, String> {
  if args.len() != 2 {
    return Err(format!("{op} expects 2 args, got {}", args.len()));
  }
  let a = gen_expr(ctx, &args[0])?;
  let b = gen_expr(ctx, &args[1])?;
  // Comparison produces i32; convert to f64 via select
  Ok(format!("(select (f64.const 1) (f64.const 0) ({op} {a} {b}))"))
}

/// Generate WASM for `if` expression.
fn gen_if(ctx: &mut WasmGenCtx, args_list: &[Calcit]) -> Result<String, String> {
  if args_list.len() < 2 || args_list.len() > 3 {
    return Err(format!("if expects 2-3 args, got {}", args_list.len()));
  }
  let cond = gen_expr(ctx, &args_list[0])?;
  let then_branch = gen_expr(ctx, &args_list[1])?;
  let else_branch = if args_list.len() == 3 {
    gen_expr(ctx, &args_list[2])?
  } else {
    "(f64.const 0)".into()
  };

  // Convert f64 condition to i32: nonzero is truthy
  // (f64.ne cond (f64.const 0))
  Ok(format!(
    "(if (result f64) (f64.ne {cond} (f64.const 0))\n        (then {then_branch})\n        (else {else_branch})\n      )"
  ))
}

/// Generate WASM for `let` expression.
fn gen_let(ctx: &mut WasmGenCtx, body: &[Calcit]) -> Result<String, String> {
  if body.is_empty() {
    return Ok("(f64.const 0)".into());
  }

  let pair = &body[0];
  let rest = &body[1..];

  match pair {
    Calcit::Nil => {
      // No binding, just evaluate body
      gen_body(ctx, rest)
    }
    Calcit::List(xs) if xs.is_empty() => {
      // No binding, just evaluate body
      gen_body(ctx, rest)
    }
    Calcit::List(xs) if xs.len() == 2 => {
      let var_name = match &xs[0] {
        Calcit::Local(CalcitLocal { sym, .. }) => sym.to_string(),
        Calcit::Symbol { sym, .. } => sym.to_string(),
        other => return Err(format!("let binding expected symbol, got: {other}")),
      };
      let value_code = gen_expr(ctx, &xs[1])?;

      // Declare local and set value
      let wasm_name = ctx.declare_local(&var_name);
      let mut code = format!("(local.set {wasm_name} {value_code})\n");

      // Check if rest is a single nested let (optimization: flatten)
      if rest.len() == 1 {
        if let Calcit::List(inner) = &rest[0] {
          if let Some(Calcit::Syntax(CalcitSyntax::CoreLet, _)) = inner.first() {
            let inner_body: Vec<Calcit> = inner.drop_left().to_vec();
            let inner_code = gen_let(ctx, &inner_body)?;
            code.push_str(&inner_code);
            return Ok(code);
          }
        }
      }

      let body_code = gen_body(ctx, rest)?;
      code.push_str(&body_code);
      Ok(code)
    }
    _ => Err(format!("unsupported let binding form: {pair}")),
  }
}
