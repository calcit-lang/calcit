use super::*;

/// Resolve a compile-time Calcit callee expression to a WASM function index.
/// Supports Fn literals (with def_ref), Imports, and Symbols.
/// Validate the actual caller's arity before selecting a non-rest static call.
fn resolve_callee_fn_idx(ctx: &WasmGenCtx, callee: &Calcit, expected_arity: u32) -> Result<u32, String> {
  if (matches!(callee, Calcit::Local(_)) || matches!(callee, Calcit::Symbol { sym, .. } if ctx.locals.contains_key(sym.as_ref())))
    && let Some(definition) = resolve_known_function_value(ctx, callee)
  {
    return resolve_callee_fn_idx(ctx, &Calcit::Import(definition), expected_arity);
  }
  let (qualified, short) = match callee {
    Calcit::Fn { info, .. } => {
      let def_ref = info
        .def_ref
        .as_ref()
        .ok_or_else(|| format!("fn literal without def_ref in foldl callee: {}/{}", info.def_ns, info.name))?;
      (format!("{}/{}", def_ref.def_ns, def_ref.def_name), def_ref.def_name.to_string())
    }
    Calcit::Import(import) => (format!("{}/{}", import.ns, import.def), import.def.to_string()),
    Calcit::Symbol { sym, .. } if ctx.locals.contains_key(sym.as_ref()) => {
      return Err(format!("HOF callee has no known local target: {sym}"));
    }
    Calcit::Symbol { sym, .. } => (sym.to_string(), sym.to_string()),
    _ => return Err(format!("HOF callee must be a static fn/import/symbol in WASM, got: {callee}")),
  };
  let fn_idx = ctx
    .fn_index
    .get(&qualified)
    .or_else(|| ctx.fn_index.get(&short))
    .copied()
    .ok_or_else(|| format!("unknown HOF callee: {qualified}"))?;
  // Rest calls pack their arguments or trap when fixed arguments are missing.
  // Ordinary functions must match exactly, including Imports and known locals.
  if !ctx.fn_has_rest.contains_key(&qualified) && !ctx.fn_has_rest.contains_key(&short) {
    let arity = ctx
      .fn_arity
      .get(&qualified)
      .or_else(|| ctx.fn_arity.get(&short))
      .ok_or_else(|| format!("unknown HOF callee arity: {qualified}"))?;
    if *arity != expected_arity {
      return Err(format!(
        "HOF callee must accept {expected_arity} arguments, but {qualified} has arity {arity}"
      ));
    }
  }
  Ok(fn_idx)
}

/// The fixed parameter count of a static HOF callee declared with a `&` rest
/// parameter, looked up the same way as `resolve_callee_fn_idx`.
fn resolve_callee_rest_fixed(ctx: &WasmGenCtx, callee: &Calcit) -> Option<u32> {
  if (matches!(callee, Calcit::Local(_)) || matches!(callee, Calcit::Symbol { sym, .. } if ctx.locals.contains_key(sym.as_ref())))
    && let Some(definition) = resolve_known_function_value(ctx, callee)
  {
    return resolve_callee_rest_fixed(ctx, &Calcit::Import(definition));
  }
  let (qualified, short) = match callee {
    Calcit::Fn { info, .. } => {
      let def_ref = info.def_ref.as_ref()?;
      (format!("{}/{}", def_ref.def_ns, def_ref.def_name), def_ref.def_name.to_string())
    }
    Calcit::Import(import) => (format!("{}/{}", import.ns, import.def), import.def.to_string()),
    Calcit::Symbol { sym, .. } => (sym.to_string(), sym.to_string()),
    _ => return None,
  };
  ctx.fn_has_rest.get(&qualified).or_else(|| ctx.fn_has_rest.get(&short)).copied()
}

/// Calls a static HOF callee with argument locals. For a `&` rest callee the
/// arguments after its fixed parameters are packed into the rest list, and a
/// call without every fixed argument traps like the native arity error (#1964).
fn emit_static_callee_call(ctx: &mut WasmGenCtx, fn_idx: u32, rest_fixed: Option<u32>, args: &[u32]) {
  let fixed = match rest_fixed {
    None => args.len(),
    Some(fixed) if fixed as usize <= args.len() => fixed as usize,
    Some(_) => {
      ctx.emit(Instruction::Unreachable);
      return;
    }
  };
  let rest = rest_fixed.map(|_| emit_list_from_locals(ctx, &args[fixed..]));
  for arg in args[..fixed].iter().chain(rest.as_ref()) {
    ctx.emit(Instruction::LocalGet(*arg));
  }
  ctx.emit(Instruction::Call(fn_idx));
}

/// Try to extract (param_names, body_exprs) from an inline lambda form.
///
/// Handles both `Calcit::List(defn name args body...)` forms (anonymous lambdas
/// appearing in preprocessed code) and `Calcit::Fn { info }` values that don't
/// have a resolvable `def_ref`.
pub(super) fn try_extract_inline_lambda(callee: &Calcit) -> Option<(Vec<String>, Vec<Calcit>)> {
  // Handle Calcit::Fn values — anonymous lambdas created by `fn` macro after preprocessing.
  if let Calcit::Fn { info, .. } = callee {
    let params: Vec<String> = match info.args.as_ref() {
      CalcitFnArgs::MarkedArgs(labels) => labels
        .iter()
        .filter_map(|label| {
          if let CalcitArgLabel::Idx(idx) = label {
            Some(CalcitLocal::read_name(*idx))
          } else {
            None
          }
        })
        .collect(),
      CalcitFnArgs::Args(idxs) => idxs.iter().map(|&idx| CalcitLocal::read_name(idx)).collect(),
    };
    let body = info.body.clone();
    if body.is_empty() {
      return None;
    }
    return Some((params, body));
  }

  let Calcit::List(items) = callee else {
    return None;
  };
  // Pattern: ((&syntax defn) name (args...) body...)
  match (items.first(), items.get(1), items.get(2)) {
    (Some(Calcit::Syntax(CalcitSyntax::Defn, _)), _, Some(Calcit::List(args))) => {
      let params: Vec<String> = args
        .iter()
        .filter_map(|a| match a {
          Calcit::Local(CalcitLocal { sym, .. }) | Calcit::Symbol { sym, .. } if !matches!(sym.as_ref(), "&" | "?") => {
            Some(sym.as_ref().to_owned())
          }
          _ => None,
        })
        .collect();
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
pub(super) fn emit_foldl(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(3, args, "foldl")?;

  // Evaluate collection → i32 pointer
  let list_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  // Load count (number of elements)
  let count = emit_load_count_i32(ctx, list_ptr);

  // Evaluate init → acc local
  let acc = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(acc));

  // Loop index
  let i = ctx.alloc_i32(0);

  // Temporary element local
  let elem = ctx.alloc_local();

  // Resolve callee: static fn, inline lambda, or native proc
  let fn_call_kind = resolve_callee_kind(ctx, &args[2], acc, elem).map_err(|e| {
    // Provide a more specific error message for the inline lambda path
    if let Some((params, _)) = try_extract_inline_lambda(&args[2])
      && params.len() < 2
    {
      return format!("foldl inline lambda must have at least 2 params, got {}", params.len());
    }
    e
  })?;

  // Block + Loop pattern
  ctx.begin_block();
  ctx.begin_loop();

  // if i >= count: break out of block
  ctx.loop_exit_if_ge(i, count);

  // elem = list_ptr[(1+i)*8] — skip the count slot at offset 0
  emit_list_load_elem(ctx, list_ptr, i);
  ctx.emit(Instruction::LocalSet(elem));

  // acc = fn(acc, elem)
  emit_foldl_step(ctx, &fn_call_kind, acc, elem)?;
  ctx.emit(Instruction::LocalSet(acc));

  // i += 1; continue loop
  ctx.i32_inc(i);
  ctx.br_loop();

  ctx.end_block_loop();

  // Push acc as result
  ctx.emit(Instruction::LocalGet(acc));
  Ok(())
}

/// Helper enum for foldl call strategy.
pub(super) enum FoldlCallKind {
  /// A static function index and, for a `&` rest callee, its fixed arity.
  Static(u32, Option<u32>),
  Inline(Arc<InlineClosure>),
  /// A native builtin proc (e.g. `&+`, `&merge`).
  Proc(CalcitProc),
  /// A local variable holding a function table index (f64) — uses call_indirect.
  /// Value is the WASM local index of the variable.
  Dynamic(u32),
}

/// Emit a call to a native proc using two pre-allocated locals as arguments.
/// Registers the locals under synthetic names so `emit_proc_call` can resolve them.
pub(super) fn emit_foldl_proc_call(
  ctx: &mut WasmGenCtx,
  proc: &CalcitProc,
  acc: u32,
  elem: u32,
  types: [Option<Arc<CalcitTypeAnnotation>>; 2],
) -> Result<(), String> {
  use crate::wasm::host::calcit::{CalcitLocal, CalcitSymbolInfo, DYNAMIC_TYPE};
  use std::sync::Arc;

  let sym_a: Arc<str> = Arc::from("__foldl_acc__");
  let sym_b: Arc<str> = Arc::from("__foldl_elem__");
  let dummy_info = Arc::new(CalcitSymbolInfo {
    at_ns: Arc::from("wasm"),
    at_def: Arc::from("__foldl"),
  });
  let prev_a = ctx.locals.insert(sym_a.as_ref().to_owned(), acc);
  let prev_b = ctx.locals.insert(sym_b.as_ref().to_owned(), elem);
  let prev_type_a = ctx.local_types.remove(sym_a.as_ref());
  let prev_type_b = ctx.local_types.remove(sym_b.as_ref());
  let [type_a, type_b] = types;
  let expr_a = crate::wasm::host::calcit::Calcit::Local(CalcitLocal {
    idx: 0,
    sym: sym_a.clone(),
    info: dummy_info.clone(),
    location: None,
    type_info: type_a.unwrap_or_else(|| DYNAMIC_TYPE.clone()),
  });
  let expr_b = crate::wasm::host::calcit::Calcit::Local(CalcitLocal {
    idx: 0,
    sym: sym_b.clone(),
    info: dummy_info,
    location: None,
    type_info: type_b.unwrap_or_else(|| DYNAMIC_TYPE.clone()),
  });
  let result = emit_proc_call(ctx, proc, &[expr_a, expr_b]);
  if let Some(annotation) = prev_type_a {
    ctx.local_types.insert(sym_a.to_string(), annotation);
  }
  if let Some(annotation) = prev_type_b {
    ctx.local_types.insert(sym_b.to_string(), annotation);
  }
  match prev_a {
    Some(v) => {
      ctx.locals.insert(sym_a.as_ref().to_owned(), v);
    }
    None => {
      ctx.locals.remove(sym_a.as_ref());
    }
  }
  match prev_b {
    Some(v) => {
      ctx.locals.insert(sym_b.as_ref().to_owned(), v);
    }
    None => {
      ctx.locals.remove(sym_b.as_ref());
    }
  }
  result
}

/// Resolve a callee to a `FoldlCallKind`, also accepting native procs.
pub(super) fn resolve_callee_kind(ctx: &WasmGenCtx, callee: &Calcit, _acc: u32, _elem: u32) -> Result<FoldlCallKind, String> {
  resolve_hof_callee(ctx, callee, 2)
}

/// Emit the body of a single foldl iteration step:
/// `acc = call_kind(acc, elem)`. Leaves the result on the stack (caller must LocalSet acc).
pub(super) fn emit_foldl_step(ctx: &mut WasmGenCtx, fn_call_kind: &FoldlCallKind, acc: u32, elem: u32) -> Result<(), String> {
  match fn_call_kind {
    FoldlCallKind::Static(fn_idx, rest_fixed) => {
      emit_static_callee_call(ctx, *fn_idx, *rest_fixed, &[acc, elem]);
      Ok(())
    }
    FoldlCallKind::Inline(closure) => emit_inline_closure_body(ctx, closure, &[acc, elem]),
    FoldlCallKind::Proc(proc) => emit_foldl_proc_call(ctx, proc, acc, elem, [None, None]),
    // Dynamic dispatch via call_indirect: (acc_f64, elem_f64) → f64. Canonical type 2.
    FoldlCallKind::Dynamic(fn_local_idx) => {
      ctx.emit(Instruction::LocalGet(acc));
      ctx.emit(Instruction::LocalGet(elem));
      ctx.emit(Instruction::LocalGet(*fn_local_idx));
      ctx.emit(Instruction::I32TruncF64S);
      ctx.emit(call_indirect(2));
      Ok(())
    }
  }
}

/// `&list:foldl-shortcut xs acc default fn` — like foldl but fn returns `:: bool new_acc`.
/// If bool is true, return new_acc immediately (short-circuit). Otherwise continue.
/// After exhausting xs, return default.
/// fn must be statically resolvable or an inline lambda.
pub(super) fn emit_foldl_shortcut(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(4, args, "&list:foldl-shortcut")?;

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

  let i = ctx.alloc_i32(0);

  let elem = ctx.alloc_local();
  let enum_ptr = ctx.alloc_local_typed(ValType::I32);

  // Resolve callee (after allocating acc/elem so Proc variant has the right indices)
  let fn_call_kind = resolve_callee_kind(ctx, &args[3], acc, elem).map_err(|e| format!("&list:foldl-shortcut: {e}"))?;

  // Outer block for early exit; inner loop for iteration
  ctx.begin_block();
  ctx.begin_loop();

  // if i >= count: exit block (return default)
  ctx.loop_exit_if_ge(i, count);

  // elem = list_ptr[(1+i)*8]
  emit_list_load_elem(ctx, list_ptr, i);
  ctx.emit(Instruction::LocalSet(elem));

  // enum_ptr = trunc(fn(acc, elem))
  emit_foldl_step(ctx, &fn_call_kind, acc, elem)?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(enum_ptr));

  // tag = F64Load(enum_ptr + 8) — bool flag (1.0 = true → early exit)
  ctx.emit(Instruction::LocalGet(enum_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::F64Eq); // → i32

  ctx.begin_block_if();
  // Early exit: result = payload at enum_ptr + 16
  ctx.emit(Instruction::LocalGet(enum_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(16)));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::Br(2)); // break outer block (If=0, Loop=1, Block=2)
  ctx.emit(Instruction::End); // end if

  // else: acc = payload, continue
  ctx.emit(Instruction::LocalGet(enum_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(16)));
  ctx.emit(Instruction::LocalSet(acc));

  // i += 1; continue loop
  ctx.i32_inc(i);
  ctx.br_loop();

  ctx.end_block_loop();

  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

/// `foldr-shortcut xs acc default fn` — like `&list:foldl-shortcut` but iterates right-to-left.
pub(super) fn emit_foldr_shortcut(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(4, args, "foldr-shortcut")?;

  // Resolve callee
  let fn_call_kind = resolve_callee_kind(ctx, &args[3], 0, 0) // temp indices; real ones allocated below
    .map_err(|e| format!("foldr-shortcut: {e}"))?;

  let list_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, list_ptr);

  let acc = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(acc));

  let result = ctx.alloc_local();
  emit_expr(ctx, &args[2])?;
  ctx.emit(Instruction::LocalSet(result));

  // i = count - 1 (signed i32, start from last element)
  let i = ctx.i32_offset(count, -1);

  let elem = ctx.alloc_local();
  let enum_ptr = ctx.alloc_local_typed(ValType::I32);

  ctx.begin_block();
  ctx.begin_loop();

  // if i < 0: exit block (return default)
  ctx.loop_exit_if_neg(i);

  // elem = list_ptr[(1+i)*8]
  emit_list_load_elem(ctx, list_ptr, i);
  ctx.emit(Instruction::LocalSet(elem));

  // enum_ptr = trunc(fn(acc, elem))
  emit_foldl_step(ctx, &fn_call_kind, acc, elem)?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(enum_ptr));

  // tag = F64Load(enum_ptr + 8) — bool flag
  ctx.emit(Instruction::LocalGet(enum_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::F64Eq);

  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(enum_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(16)));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::Br(2)); // break outer block
  ctx.emit(Instruction::End); // end if

  // else: acc = payload, i--
  ctx.emit(Instruction::LocalGet(enum_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(16)));
  ctx.emit(Instruction::LocalSet(acc));

  ctx.i32_dec(i);
  ctx.br_loop();

  ctx.end_block_loop();

  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

/// `foldl-compare xs acc f` — return true if `f(acc, elem)` holds for every consecutive pair.
/// Equivalent to: check f(acc, xs[0]), then f(xs[0], xs[1]), ..., return true if all pass.
/// Used to implement multi-arg comparison operators like `<`, `<=`, `=`, `>`, `>=`.
/// `f` can be a static fn, inline lambda, or native proc.
pub(super) fn emit_foldl_compare(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(3, args, "foldl-compare")?;
  let element_type = infer_wasm_static_type(ctx, &args[0]).and_then(|annotation| match annotation.as_ref() {
    CalcitTypeAnnotation::List(element) => Some(element.clone()),
    _ => None,
  });
  let initial_type = infer_wasm_static_type(ctx, &args[1]);
  // After the first iteration, the accumulator is the previous list element.
  // Keep only a common type proved by both sources, not the callee's expected type.
  let accumulator_type = initial_type.as_ref().zip(element_type.as_ref()).and_then(|(initial, element)| {
    if initial.is_proven_for(element) {
      Some(element.clone())
    } else if element.is_proven_for(initial) {
      Some(initial.clone())
    } else {
      None
    }
  });

  // Evaluate xs → list pointer
  let list_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, list_ptr);

  // Evaluate initial acc
  let acc = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(acc));

  let i = ctx.alloc_i32(0);

  let elem = ctx.alloc_local();
  let result = ctx.alloc_local();
  ctx.emit(f64_const(1.0)); // default: true
  ctx.emit(Instruction::LocalSet(result));

  // Resolve callee after allocating acc/elem
  let fn_call_kind = resolve_callee_kind(ctx, &args[2], acc, elem).map_err(|e| format!("foldl-compare: {e}"))?;

  // Outer block (break here with false or after exhausting list)
  ctx.begin_block();
  ctx.begin_loop();

  // if i >= count: break (result = true, already set)
  ctx.loop_exit_if_ge(i, count);

  // elem = list_ptr[(1+i)*8]
  emit_list_load_elem(ctx, list_ptr, i);
  ctx.emit(Instruction::LocalSet(elem));

  // condition = f(acc, elem) → 1.0 (true) or 0.0 (false)
  match &fn_call_kind {
    FoldlCallKind::Proc(proc) => emit_foldl_proc_call(ctx, proc, acc, elem, [accumulator_type, element_type])?,
    _ => emit_foldl_step(ctx, &fn_call_kind, acc, elem)?,
  }
  // if condition == 0.0: result = false, break
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Eq); // i32: 1 if condition was 0.0 (false)
  ctx.begin_block_if();
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::Br(2)); // break outer block
  ctx.emit(Instruction::End);

  // acc = elem (for next iteration)
  ctx.emit(Instruction::LocalGet(elem));
  ctx.emit(Instruction::LocalSet(acc));

  // i += 1; continue
  ctx.i32_inc(i);
  ctx.br_loop();

  ctx.end_block_loop();

  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

// ===========================================================================
// Unary HOF helpers: map, filter, each, any?, every?, find, find-index
// ===========================================================================

/// Emit a call with the element evidence from the actual source collection.
fn emit_unary_step(ctx: &mut WasmGenCtx, kind: &FoldlCallKind, arg: u32, source_collection: Option<&Calcit>) -> Result<(), String> {
  use crate::wasm::host::calcit::{CalcitLocal, CalcitSymbolInfo, DYNAMIC_TYPE};
  use std::sync::Arc;
  match kind {
    FoldlCallKind::Static(fn_idx, rest_fixed) => {
      emit_static_callee_call(ctx, *fn_idx, *rest_fixed, &[arg]);
      Ok(())
    }
    FoldlCallKind::Inline(closure) => emit_inline_closure_body(ctx, closure, &[arg]),
    FoldlCallKind::Proc(proc) => {
      let annotation = source_collection
        .and_then(|source| infer_wasm_static_type(ctx, source))
        .and_then(|annotation| match annotation.as_ref() {
          CalcitTypeAnnotation::List(element) | CalcitTypeAnnotation::Set(element) => Some(element.clone()),
          _ => None,
        });
      let sym: Arc<str> = Arc::from("__hof_arg__");
      let dummy_info = Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("wasm"),
        at_def: Arc::from("__hof"),
      });
      let prev = ctx.locals.insert(sym.as_ref().to_owned(), arg);
      let prev_type = ctx.local_types.remove(sym.as_ref());
      let expr = crate::wasm::host::calcit::Calcit::Local(CalcitLocal {
        idx: 0,
        sym: sym.clone(),
        info: dummy_info,
        location: None,
        type_info: annotation.unwrap_or_else(|| DYNAMIC_TYPE.clone()),
      });
      let result = emit_proc_call(ctx, proc, &[expr]);
      if let Some(annotation) = prev_type {
        ctx.local_types.insert(sym.to_string(), annotation);
      }
      match prev {
        Some(v) => {
          ctx.locals.insert(sym.as_ref().to_owned(), v);
        }
        None => {
          ctx.locals.remove(sym.as_ref());
        }
      }
      result
    }
    // Dynamic dispatch via call_indirect: local holds f64 table slot index.
    // Canonical type 1 = (f64) → f64.
    FoldlCallKind::Dynamic(fn_local_idx) => {
      ctx.emit(Instruction::LocalGet(arg));
      ctx.emit(Instruction::LocalGet(*fn_local_idx));
      ctx.emit(Instruction::I32TruncF64S);
      ctx.emit(call_indirect(1));
      Ok(())
    }
  }
}

/// Emit a call to the HOF callee with two arguments (index, elem) for map-indexed,
/// matching the native `(f idx elem)` callback order.
fn emit_binary_step_ie(ctx: &mut WasmGenCtx, kind: &FoldlCallKind, idx: u32, elem: u32) -> Result<(), String> {
  match kind {
    FoldlCallKind::Static(fn_idx, rest_fixed) => {
      let idx_f64 = ctx.alloc_local();
      ctx.ptr_to_f64(idx);
      ctx.emit(Instruction::LocalSet(idx_f64));
      emit_static_callee_call(ctx, *fn_idx, *rest_fixed, &[idx_f64, elem]);
      Ok(())
    }
    FoldlCallKind::Inline(closure) => {
      if !closure.accepts(2) {
        return Err("map-indexed inline lambda must accept 2 arguments".into());
      }
      let idx_f64 = ctx.alloc_local();
      ctx.ptr_to_f64(idx);
      ctx.emit(Instruction::LocalSet(idx_f64));
      emit_inline_closure_body(ctx, closure, &[idx_f64, elem])
    }
    FoldlCallKind::Proc(_) => Err("map-indexed proc callee not supported".into()),
    // Dynamic dispatch: (idx_as_f64, elem_f64) → f64. Canonical type 2.
    FoldlCallKind::Dynamic(fn_local_idx) => {
      ctx.ptr_to_f64(idx);
      ctx.emit(Instruction::LocalGet(elem));
      ctx.emit(Instruction::LocalGet(*fn_local_idx));
      ctx.emit(Instruction::I32TruncF64S);
      ctx.emit(call_indirect(2));
      Ok(())
    }
  }
}

/// Resolve a HOF callee for the argument count supplied by its caller.
fn resolve_hof_callee(ctx: &WasmGenCtx, callee: &Calcit, expected_arity: u32) -> Result<FoldlCallKind, String> {
  if let Some(closure) = resolve_inline_closure(ctx, callee) {
    return Ok(FoldlCallKind::Inline(closure));
  }
  if let Ok(fn_idx) = resolve_callee_fn_idx(ctx, callee, expected_arity) {
    return Ok(FoldlCallKind::Static(fn_idx, resolve_callee_rest_fixed(ctx, callee)));
  }
  if let Calcit::Proc(proc) = callee {
    return Ok(FoldlCallKind::Proc(*proc));
  }
  // Local variable holding a function value (dyn dispatch via call_indirect)
  if let Some(name) = match callee {
    Calcit::Local(local) => Some(local.sym.as_ref()),
    Calcit::Symbol { sym, .. } if ctx.locals.contains_key(sym.as_ref()) => Some(sym.as_ref()),
    _ => None,
  } {
    let local_idx = ctx
      .locals
      .get(name)
      .copied()
      .ok_or_else(|| format!("HOF callee local not found: {name}"))?;
    return Ok(FoldlCallKind::Dynamic(local_idx));
  }
  Err(format!("HOF callee must be a static fn/import/symbol in WASM, got: {callee}"))
}

/// Resolve a single-arg HOF callee (map, filter, each, any?, etc.).
fn resolve_unary_callee(ctx: &WasmGenCtx, callee: &Calcit) -> Result<FoldlCallKind, String> {
  resolve_hof_callee(ctx, callee, 1)
}

/// `&map:fold-kv map init f` — fold map entries in iteration order, passing
/// the accumulator, key, and value to `f`.
pub(super) fn emit_map_fold_kv(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(3, args, "&map:fold-kv")?;

  let map_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let linearize_idx = *ctx
    .runtime_fn_index
    .get("__rt_map_linearize")
    .ok_or("runtime __rt_map_linearize missing")?;
  ctx.emit(Instruction::LocalGet(map_ptr));
  ctx.emit(Instruction::Call(linearize_idx));
  let flat_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalSet(flat_ptr));

  let count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(flat_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(count));

  let acc = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(acc));

  let kind = resolve_hof_callee(ctx, &args[2], 3).map_err(|error| format!("&map:fold-kv: {error}"))?;
  if let FoldlCallKind::Inline(closure) = &kind
    && !closure.accepts(3)
  {
    return Err("&map:fold-kv inline lambda must accept 3 arguments".into());
  }

  let i = ctx.alloc_i32(0);
  let key = ctx.alloc_local();
  let value = ctx.alloc_local();
  ctx.begin_block();
  ctx.begin_loop();
  ctx.loop_exit_if_ge(i, count);

  // Entries are stored as [key, value] pairs in the linearized map buffer.
  ctx.emit(Instruction::LocalGet(flat_ptr));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(key));

  ctx.emit(Instruction::LocalGet(flat_ptr));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(value));

  match &kind {
    FoldlCallKind::Static(fn_idx, rest_fixed) => emit_static_callee_call(ctx, *fn_idx, *rest_fixed, &[acc, key, value]),
    FoldlCallKind::Inline(closure) => emit_inline_closure_body(ctx, closure, &[acc, key, value])?,
    FoldlCallKind::Proc(proc) => emit_map_fold_kv_proc_call(ctx, proc, acc, key, value)?,
    FoldlCallKind::Dynamic(fn_idx) => {
      ctx.emit(Instruction::LocalGet(acc));
      ctx.emit(Instruction::LocalGet(key));
      ctx.emit(Instruction::LocalGet(value));
      ctx.emit(Instruction::LocalGet(*fn_idx));
      ctx.emit(Instruction::I32TruncF64S);
      ctx.emit(call_indirect(3));
    }
  }
  ctx.emit(Instruction::LocalSet(acc));

  ctx.i32_inc(i);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(acc));
  Ok(())
}

fn emit_map_fold_kv_proc_call(ctx: &mut WasmGenCtx, proc: &CalcitProc, acc: u32, key: u32, value: u32) -> Result<(), String> {
  use crate::wasm::host::calcit::{CalcitLocal, CalcitSymbolInfo, DYNAMIC_TYPE};
  use std::sync::Arc;

  let info = Arc::new(CalcitSymbolInfo {
    at_ns: Arc::from("wasm"),
    at_def: Arc::from("__map_fold_kv"),
  });
  let mut expressions = Vec::with_capacity(3);
  let mut previous = Vec::with_capacity(3);
  for (name, index) in [("acc", acc), ("key", key), ("value", value)] {
    let symbol: Arc<str> = Arc::from(format!("__map_fold_kv_{name}__"));
    previous.push((symbol.to_string(), ctx.locals.insert(symbol.to_string(), index)));
    expressions.push(Calcit::Local(CalcitLocal {
      idx: 0,
      sym: symbol,
      info: info.clone(),
      location: None,
      type_info: DYNAMIC_TYPE.clone(),
    }));
  }
  let result = emit_proc_call(ctx, proc, &expressions);
  for (symbol, previous) in previous {
    if let Some(previous) = previous {
      ctx.locals.insert(symbol, previous);
    } else {
      ctx.locals.remove(&symbol);
    }
  }
  result
}

/// `map xs f` — apply f to every element, returning new list of same length.
/// Copy the type tag (i32 at raw_base+4 = ptr-4) from `src` to `dst`.
/// Used by map/filter to preserve set→set or list→list semantics.
fn emit_copy_type_tag(ctx: &mut WasmGenCtx, src_ptr: u32, dst_ptr: u32) {
  // Read tag from src_ptr - 4
  ctx.emit(Instruction::LocalGet(src_ptr));
  ctx.emit(Instruction::I32Const(4));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  // Write tag to dst_ptr - 4
  let tag_tmp = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalSet(tag_tmp));
  ctx.emit(Instruction::LocalGet(dst_ptr));
  ctx.emit(Instruction::I32Const(4));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalGet(tag_tmp));
  ctx.emit(Instruction::I32Store(mem_arg_i32(0)));
}

pub(super) fn emit_map(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "map")?;
  let src_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src_ptr);
  let dst_ptr = emit_alloc_list(ctx, count);
  // Preserve source type tag (set→set, list→list)
  emit_copy_type_tag(ctx, src_ptr, dst_ptr);
  let kind = resolve_unary_callee(ctx, &args[1]).map_err(|e| format!("map: {e}"))?;
  let (i, elem) = emit_list_iter_begin(ctx, src_ptr, count);
  // write f(elem) into dst[i]: push address of dst[i] then store result
  ctx.emit(Instruction::LocalGet(dst_ptr));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  emit_unary_step(ctx, &kind, elem, Some(&args[0]))?;
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  emit_list_iter_end(ctx, i);
  ctx.ptr_to_f64(dst_ptr);
  Ok(())
}

/// `map-indexed xs f` — apply f(idx, elem) to every element, returning new list.
pub(super) fn emit_map_indexed(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "map-indexed")?;
  let src_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src_ptr);
  let dst_ptr = emit_alloc_list(ctx, count);
  let kind = resolve_hof_callee(ctx, &args[1], 2).map_err(|e| format!("map-indexed: {e}"))?;
  let (i, elem) = emit_list_iter_begin(ctx, src_ptr, count);
  // write f(i, elem) into dst[i]
  ctx.emit(Instruction::LocalGet(dst_ptr));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  emit_binary_step_ie(ctx, &kind, i, elem)?;
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  emit_list_iter_end(ctx, i);
  ctx.ptr_to_f64(dst_ptr);
  Ok(())
}

/// `each xs f` — apply f to every element for side effects; returns 0.0 (nil).
pub(super) fn emit_each(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "each")?;
  let src_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src_ptr);
  let kind = resolve_unary_callee(ctx, &args[1]).map_err(|e| format!("each: {e}"))?;
  let (i, elem) = emit_list_iter_begin(ctx, src_ptr, count);
  emit_unary_step(ctx, &kind, elem, Some(&args[0]))?;
  ctx.emit(Instruction::Drop);
  emit_list_iter_end(ctx, i);
  ctx.emit(f64_const(0.0));
  Ok(())
}

/// `filter xs f` / `&list:filter xs f` — return list of elements where f returns truthy.
pub(super) fn emit_filter(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "filter")?;
  let src_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src_ptr);
  let kind = resolve_unary_callee(ctx, &args[1]).map_err(|e| format!("filter: {e}"))?;
  let dst_ptr = emit_alloc_list(ctx, count);
  emit_copy_type_tag(ctx, src_ptr, dst_ptr);
  let result_count = ctx.alloc_i32(0);
  let (i, elem) = emit_list_iter_begin(ctx, src_ptr, count);
  emit_unary_step(ctx, &kind, elem, Some(&args[0]))?;
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Ne);
  ctx.begin_block_if();
  emit_list_store_elem(ctx, dst_ptr, result_count, elem);
  ctx.i32_inc(result_count);
  ctx.emit(Instruction::End);
  emit_list_iter_end(ctx, i);
  // update count header to actual result_count
  ctx.store_i32_as_f64(dst_ptr, result_count, 0);
  ctx.ptr_to_f64(dst_ptr);
  Ok(())
}

/// `any? xs f` — return 1.0 if any element satisfies f, else 0.0.
pub(super) fn emit_any(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "any?")?;
  let src_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src_ptr);
  let kind = resolve_unary_callee(ctx, &args[1]).map_err(|e| format!("any?: {e}"))?;
  let result = ctx.alloc_local();
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalSet(result));
  let (i, elem) = emit_list_iter_begin(ctx, src_ptr, count);
  emit_unary_step(ctx, &kind, elem, Some(&args[0]))?;
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Ne);
  ctx.begin_block_if();
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::Br(2)); // exit the block+loop
  ctx.emit(Instruction::End);
  emit_list_iter_end(ctx, i);
  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

/// `every? xs f` — return 1.0 if every element satisfies f, else 0.0.
pub(super) fn emit_every(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "every?")?;
  let src_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src_ptr);
  let kind = resolve_unary_callee(ctx, &args[1]).map_err(|e| format!("every?: {e}"))?;
  let result = ctx.alloc_local();
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::LocalSet(result));
  let (i, elem) = emit_list_iter_begin(ctx, src_ptr, count);
  emit_unary_step(ctx, &kind, elem, Some(&args[0]))?;
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Eq);
  ctx.begin_block_if();
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::Br(2)); // exit the block+loop
  ctx.emit(Instruction::End);
  emit_list_iter_end(ctx, i);
  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

/// `find xs f` — return `Option` containing the first matching element.
pub(super) fn emit_find(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "find")?;
  let src_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src_ptr);
  let kind = resolve_unary_callee(ctx, &args[1]).map_err(|e| format!("find: {e}"))?;
  let result = ctx.alloc_local();
  let found = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(found));
  let (i, elem) = emit_list_iter_begin(ctx, src_ptr, count);
  emit_unary_step(ctx, &kind, elem, Some(&args[0]))?;
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Ne);
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(elem));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(found));
  ctx.emit(Instruction::Br(2)); // exit the block+loop
  ctx.emit(Instruction::End);
  emit_list_iter_end(ctx, i);
  ctx.emit(Instruction::LocalGet(found));
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_option_enum(ctx, Some(result))?;
  ctx.emit(Instruction::Else);
  emit_option_enum(ctx, None)?;
  ctx.emit(Instruction::End);
  Ok(())
}

/// `mapcat xs f` — apply f to every element, then flatten results one level.
pub(super) fn emit_mapcat(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "mapcat")?;
  // Emit (map xs f) → leaves f64 list pointer on stack
  emit_map(ctx, args)?;
  // Store into a local, then flatten
  let mapped_local = ctx.alloc_local();
  ctx.emit(Instruction::LocalSet(mapped_local));
  emit_list_flatten_f64_local(ctx, mapped_local)
}

/// `filter-not xs f` — return elements where f(elem) is falsy.
pub(super) fn emit_filter_not(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "filter-not")?;
  let src_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src_ptr);
  let kind = resolve_unary_callee(ctx, &args[1]).map_err(|e| format!("filter-not: {e}"))?;
  let dst_ptr = emit_alloc_list(ctx, count);
  emit_copy_type_tag(ctx, src_ptr, dst_ptr);
  let result_count = ctx.alloc_i32(0);
  let (i, elem) = emit_list_iter_begin(ctx, src_ptr, count);
  emit_unary_step(ctx, &kind, elem, Some(&args[0]))?;
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Eq); // keep when f returns falsy (== 0.0)
  ctx.begin_block_if();
  emit_list_store_elem(ctx, dst_ptr, result_count, elem);
  ctx.i32_inc(result_count);
  ctx.emit(Instruction::End);
  emit_list_iter_end(ctx, i);
  // update count header to actual result_count
  ctx.store_i32_as_f64(dst_ptr, result_count, 0);
  ctx.ptr_to_f64(dst_ptr);
  Ok(())
}

/// `update x k f` — rewrite the slot `k` of a List, Map or Enum with `f(old)`.
/// Like the core definition, a missing list index or map key returns `x`
/// unchanged without calling `f`; the receiver kind is dispatched at runtime.
pub(super) fn emit_update(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(3, args, "update")?;
  let receiver = ctx.alloc_local();
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::LocalSet(receiver));

  let key_local = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(key_local));

  let kind = resolve_unary_callee(ctx, &args[2]).map_err(|e| format!("update: {e}"))?;

  super::methods::emit_update_has_slot(ctx, receiver, key_local);
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Ne);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  // new_val = f(get(x, k)); result = assoc(x, k, new_val)
  let cur_val = ctx.alloc_local();
  super::methods::emit_method_get(ctx, receiver, key_local)?;
  ctx.emit(Instruction::LocalSet(cur_val));
  let new_val = ctx.alloc_local();
  emit_unary_step(ctx, &kind, cur_val, None)?;
  ctx.emit(Instruction::LocalSet(new_val));
  super::methods::emit_method_assoc(ctx, receiver, key_local, new_val)?;
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(receiver));
  ctx.emit(Instruction::End);
  Ok(())
}

/// `find-index xs f` — return `Option` containing the first matching index.
pub(super) fn emit_find_index(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "find-index")?;
  let src_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src_ptr);
  let kind = resolve_unary_callee(ctx, &args[1]).map_err(|e| format!("find-index: {e}"))?;
  let result = ctx.alloc_local();
  let found = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(found));
  let (i, elem) = emit_list_iter_begin(ctx, src_ptr, count);
  emit_unary_step(ctx, &kind, elem, Some(&args[0]))?;
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Ne);
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(found));
  ctx.emit(Instruction::Br(2)); // exit the block+loop
  ctx.emit(Instruction::End);
  emit_list_iter_end(ctx, i);
  ctx.emit(Instruction::LocalGet(found));
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_option_enum(ctx, Some(result))?;
  ctx.emit(Instruction::Else);
  emit_option_enum(ctx, None)?;
  ctx.emit(Instruction::End);
  Ok(())
}

/// `map-kv xs f` — apply binary `f(k, v) → [k', v']` to every entry of a map, returning a new map.
/// Iterates via `__rt_map_linearize` which returns a flat `[n_pairs, k0, v0, k1, v1, ...]` buffer.
pub(super) fn emit_map_kv(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  emit_map_kv_impl(ctx, args, false)
}

/// `filter-map-kv xs f` — apply binary `f(k, v) → MapEntryDecision<k', v'>`,
/// associating `:keep` payloads and omitting `:drop` results.
pub(super) fn emit_filter_map_kv(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  emit_map_kv_impl(ctx, args, true)
}

fn emit_map_kv_impl(ctx: &mut WasmGenCtx, args: &[Calcit], uses_decision: bool) -> Result<(), String> {
  let op_name = if uses_decision { "filter-map-kv" } else { "map-kv" };
  expect_arity(2, args, op_name)?;

  // Resolve binary callee: f(k, v) → [k', v']
  let kind = if let Some(closure) = resolve_inline_closure(ctx, &args[1]) {
    if !closure.accepts(2) {
      return Err(format!("{op_name}: inline lambda must accept 2 arguments"));
    }
    FoldlCallKind::Inline(closure)
  } else {
    return Err(format!("{op_name}: callee must be an inline lambda in WASM"));
  };

  // Evaluate map source → i32 ptr
  emit_expr(ctx, &args[0])?;
  let map_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(map_ptr));

  // __rt_map_linearize(map_ptr) → flat [n_pairs, k0, v0, ...] buffer (i32 ptr)
  let linearize_idx = *ctx
    .runtime_fn_index
    .get("__rt_map_linearize")
    .ok_or("runtime __rt_map_linearize missing")?;
  ctx.emit(Instruction::LocalGet(map_ptr));
  ctx.emit(Instruction::Call(linearize_idx));
  let flat_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalSet(flat_ptr));

  // n_pairs = flat_ptr[0] (f64 → i32)
  let n_pairs = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(flat_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(n_pairs));

  // Start accumulator as an empty map
  emit_map_new(ctx, &[])?;
  let acc = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(acc));

  let i = ctx.alloc_i32(0);
  let key = ctx.alloc_local();
  let val = ctx.alloc_local();
  let result_ptr = ctx.alloc_local_typed(ValType::I32);
  let new_key = ctx.alloc_local();
  let new_val = ctx.alloc_local();

  ctx.begin_block();
  ctx.begin_loop();
  ctx.loop_exit_if_ge(i, n_pairs);

  // key = flat_ptr[8 + i*16]
  ctx.emit(Instruction::LocalGet(flat_ptr));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(key));

  // val = flat_ptr[16 + i*16]
  ctx.emit(Instruction::LocalGet(flat_ptr));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(val));

  // result = f(key, val) — inline the lambda
  match &kind {
    FoldlCallKind::Inline(closure) => emit_inline_closure_body(ctx, closure, &[key, val])?,
    _ => unreachable!(),
  }

  // `map-kv` returns a [k', v'] pair list, while `filter-map-kv` returns a
  // MapEntryDecision enum. Both are heap values whose pointer is on the stack.
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(result_ptr));

  let assoc_idx = *ctx.runtime_fn_index.get("__rt_map_assoc").expect("__rt_map_assoc");
  if uses_decision {
    let keep_tag = *ctx.tag_index.get("keep").ok_or("filter-map-kv: :keep tag missing")?;

    // Named enum layout is [payload_count, tag, payload_0, payload_1, ...].
    // Only :keep carries the replacement key and value; :drop leaves acc intact.
    ctx.emit(Instruction::LocalGet(result_ptr));
    ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
    ctx.emit(f64_const(keep_tag as f64));
    ctx.emit(Instruction::F64Eq);
    ctx.begin_block_if();

    ctx.emit(Instruction::LocalGet(result_ptr));
    ctx.emit(Instruction::F64Load(mem_arg_f64(16)));
    ctx.emit(Instruction::LocalSet(new_key));
    ctx.emit(Instruction::LocalGet(result_ptr));
    ctx.emit(Instruction::F64Load(mem_arg_f64(24)));
    ctx.emit(Instruction::LocalSet(new_val));

    ctx.emit(Instruction::LocalGet(acc));
    ctx.emit(Instruction::LocalGet(new_key));
    ctx.emit(Instruction::LocalGet(new_val));
    ctx.emit(Instruction::Call(assoc_idx));
    ctx.emit(Instruction::LocalSet(acc));
    ctx.emit(Instruction::End);
  } else {
    // Pair-list layout is [count, key, value].
    ctx.emit(Instruction::LocalGet(result_ptr));
    ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
    ctx.emit(Instruction::LocalSet(new_key));
    ctx.emit(Instruction::LocalGet(result_ptr));
    ctx.emit(Instruction::F64Load(mem_arg_f64(16)));
    ctx.emit(Instruction::LocalSet(new_val));

    ctx.emit(Instruction::LocalGet(acc));
    ctx.emit(Instruction::LocalGet(new_key));
    ctx.emit(Instruction::LocalGet(new_val));
    ctx.emit(Instruction::Call(assoc_idx));
    ctx.emit(Instruction::LocalSet(acc));
  }

  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(i));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.ptr_to_f64(acc);
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::wasm::host::calcit::CalcitSymbolInfo;

  // Deliberately mismatched callback metadata cannot pass a strict Calcit test.
  // Check the backend boundary directly: never emit an invalid static signature.
  #[test]
  fn known_hof_alias_checks_the_callers_arity() {
    let mut ctx = WasmGenCtx::new(
      0,
      WasmCompileEnv {
        function_value_dependencies: Default::default(),
        fn_index: HashMap::new(),
        fn_arity: HashMap::new(),
        fn_has_rest: HashMap::new(),
        runtime_fn_index: HashMap::new(),
        tag_index: HashMap::new(),
        struct_layouts: Default::default(),
        string_pool: HashMap::new(),
        atom_globals: HashMap::new(),
        value_imports: HashMap::new(),
        static_fn_defs: HashMap::new(),
        fn_table_index: HashMap::new(),
        host_imports: HashMap::new(),
        target: WasmTarget::Core,
        boundary: WasmBoundary::Native,
      },
    );
    for (name, arity) in [("unary", 1), ("binary", 2), ("ternary", 3)] {
      let target = format!("tests.hof/{name}");
      ctx.fn_index.insert(target.clone(), arity);
      ctx.fn_arity.insert(target, arity);
      let slot = ctx.alloc_local();
      ctx.locals.insert(name.to_string(), slot);
      let definition = CalcitImport {
        ns: Arc::from("tests.hof"),
        def: Arc::from(name),
        info: Arc::new(ImportInfo::SameFile { at_def: Arc::from("test") }),
        def_id: None,
      };
      ctx.local_function_values.insert(slot, definition.clone());
      let alias = Calcit::Local(CalcitLocal {
        idx: 0,
        sym: Arc::from(name),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.hof"),
          at_def: Arc::from("test"),
        }),
        location: None,
        type_info: crate::wasm::host::calcit::DYNAMIC_TYPE.clone(),
      });
      let symbol = Calcit::Symbol {
        sym: Arc::from(name),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.hof"),
          at_def: Arc::from("test"),
        }),
        location: None,
      };
      for supplied in [1, 2] {
        for callee in [&alias, &symbol] {
          let kind = resolve_hof_callee(&ctx, callee, supplied).expect("local call strategy");
          if arity == supplied {
            assert!(matches!(kind, FoldlCallKind::Static(index, None) if index == arity));
          } else {
            assert!(matches!(kind, FoldlCallKind::Dynamic(index) if index == slot));
            assert!(resolve_hof_callee(&ctx, &Calcit::Import(definition.clone()), supplied).is_err());
          }
        }
      }
      assert_eq!(
        matches!(resolve_unary_callee(&ctx, &alias).unwrap(), FoldlCallKind::Static(_, _)),
        arity == 1
      );
      assert_eq!(
        matches!(resolve_callee_kind(&ctx, &alias, 0, 0).unwrap(), FoldlCallKind::Static(_, _)),
        arity == 2
      );
      // An unknown local shadows the same-named global, even with a matching arity.
      ctx.fn_index.insert(name.to_string(), arity);
      ctx.fn_arity.insert(name.to_string(), arity);
      ctx.local_function_values.remove(&slot);
      for supplied in [1, 2] {
        assert!(matches!(resolve_hof_callee(&ctx, &symbol, supplied).unwrap(), FoldlCallKind::Dynamic(index) if index == slot));
      }
      assert!(matches!(resolve_callee_kind(&ctx, &symbol, 0, 0).unwrap(), FoldlCallKind::Dynamic(index) if index == slot));
      ctx.local_function_values.insert(slot, definition);
      // Rest functions still use the shared pack-or-trap path at either call site.
      ctx.fn_has_rest.insert(format!("tests.hof/{name}"), arity - 1);
      for supplied in [1, 2] {
        assert!(
          matches!(resolve_hof_callee(&ctx, &alias, supplied).unwrap(), FoldlCallKind::Static(_, Some(fixed)) if fixed == arity - 1)
        );
      }
    }
  }
}
