use super::*;

/// Resolve a compile-time Calcit callee expression to a WASM function index.
/// Supports Fn literals (with def_ref), Imports, and Symbols.
/// The callee must have arity 2 (acc, elem) — as required by foldl/foldl-shortcut/foldr-shortcut.
pub(super) fn resolve_callee_fn_idx(ctx: &WasmGenCtx, callee: &Calcit) -> Result<u32, String> {
  match callee {
    Calcit::Fn { info, .. } => {
      let def_ref = info
        .def_ref
        .as_ref()
        .ok_or_else(|| format!("fn literal without def_ref in foldl callee: {}/{}", info.def_ns, info.name))?;
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
          Calcit::Local(CalcitLocal { sym, .. }) => Some(sym.as_ref().to_owned()),
          Calcit::Symbol { sym, .. } => Some(sym.as_ref().to_owned()),
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
  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(i));

  // Temporary element local
  let elem = ctx.alloc_local();

  // Resolve callee: static fn, inline lambda, or native proc
  let fn_call_kind = resolve_callee_kind(ctx, &args[2], acc, elem).map_err(|e| {
    // Provide a more specific error message for the inline lambda path
    if let Some((params, _)) = try_extract_inline_lambda(&args[2]) {
      if params.len() < 2 {
        return format!("foldl inline lambda must have at least 2 params, got {}", params.len());
      }
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
  Static(u32),
  Inline(Vec<String>, Vec<Calcit>),
  /// A native builtin proc (e.g. `&+`, `&merge`).
  Proc(CalcitProc),
  /// A local variable holding a function table index (f64) — uses call_indirect.
  /// Value is the WASM local index of the variable.
  Dynamic(u32),
}

/// Emit a call to a native proc using two pre-allocated locals as arguments.
/// Registers the locals under synthetic names so `emit_proc_call` can resolve them.
pub(super) fn emit_foldl_proc_call(ctx: &mut WasmGenCtx, proc: &CalcitProc, acc: u32, elem: u32) -> Result<(), String> {
  use crate::calcit::{CalcitLocal, CalcitSymbolInfo, DYNAMIC_TYPE};
  use std::sync::Arc;

  let sym_a: Arc<str> = Arc::from("__foldl_acc__");
  let sym_b: Arc<str> = Arc::from("__foldl_elem__");
  let dummy_info = Arc::new(CalcitSymbolInfo {
    at_ns: Arc::from("wasm"),
    at_def: Arc::from("__foldl"),
  });
  let prev_a = ctx.locals.insert(sym_a.as_ref().to_owned(), acc);
  let prev_b = ctx.locals.insert(sym_b.as_ref().to_owned(), elem);
  let expr_a = crate::calcit::Calcit::Local(CalcitLocal {
    idx: 0,
    sym: sym_a.clone(),
    info: dummy_info.clone(),
    location: None,
    type_info: DYNAMIC_TYPE.clone(),
  });
  let expr_b = crate::calcit::Calcit::Local(CalcitLocal {
    idx: 0,
    sym: sym_b.clone(),
    info: dummy_info,
    location: None,
    type_info: DYNAMIC_TYPE.clone(),
  });
  let result = emit_proc_call(ctx, proc, &[expr_a, expr_b]);
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
  if let Ok(fn_idx) = resolve_callee_fn_idx(ctx, callee) {
    return Ok(FoldlCallKind::Static(fn_idx));
  }
  if let Some((params, body)) = try_extract_inline_lambda(callee) {
    return Ok(FoldlCallKind::Inline(params, body));
  }
  if let Calcit::Proc(proc) = callee {
    return Ok(FoldlCallKind::Proc(*proc));
  }
  // Local variable holding a function value (dyn dispatch via call_indirect)
  if let Calcit::Local(local) = callee {
    let local_idx = ctx
      .locals
      .get(&*local.sym)
      .copied()
      .ok_or_else(|| format!("foldl callee local not found: {}", local.sym))?;
    return Ok(FoldlCallKind::Dynamic(local_idx));
  }
  Err(format!("foldl callee must be a static fn/import/symbol in WASM, got: {callee}"))
}

/// Emit the body of a single foldl iteration step:
/// `acc = call_kind(acc, elem)`. Leaves the result on the stack (caller must LocalSet acc).
pub(super) fn emit_foldl_step(ctx: &mut WasmGenCtx, fn_call_kind: &FoldlCallKind, acc: u32, elem: u32) -> Result<(), String> {
  match fn_call_kind {
    FoldlCallKind::Static(fn_idx) => {
      ctx.emit(Instruction::LocalGet(acc));
      ctx.emit(Instruction::LocalGet(elem));
      ctx.emit(Instruction::Call(*fn_idx));
      Ok(())
    }
    FoldlCallKind::Inline(params, body) => {
      let old0 = ctx.locals.insert(params[0].clone(), acc);
      let old1 = ctx.locals.insert(params[1].clone(), elem);
      emit_body(ctx, body)?;
      match old0 {
        Some(v) => {
          ctx.locals.insert(params[0].clone(), v);
        }
        None => {
          ctx.locals.remove(&params[0]);
        }
      }
      match old1 {
        Some(v) => {
          ctx.locals.insert(params[1].clone(), v);
        }
        None => {
          ctx.locals.remove(&params[1]);
        }
      }
      Ok(())
    }
    FoldlCallKind::Proc(proc) => emit_foldl_proc_call(ctx, proc, acc, elem),
    // Dynamic dispatch via call_indirect: (acc_f64, elem_f64) → f64. Canonical type 2.
    FoldlCallKind::Dynamic(fn_local_idx) => {
      ctx.emit(Instruction::LocalGet(acc));
      ctx.emit(Instruction::LocalGet(elem));
      ctx.emit(Instruction::LocalGet(*fn_local_idx));
      ctx.emit(Instruction::I32TruncF64S);
      ctx.emit(Instruction::CallIndirect { type_index: 2, table_index: 0 });
      Ok(())
    }
  }
}

/// `foldl-shortcut xs acc default fn` — like foldl but fn returns `:: bool new_acc`.
/// If bool is true, return new_acc immediately (short-circuit). Otherwise continue.
/// After exhausting xs, return default.
/// fn must be statically resolvable or an inline lambda.
pub(super) fn emit_foldl_shortcut(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(4, args, "foldl-shortcut")?;

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

  // Resolve callee (after allocating acc/elem so Proc variant has the right indices)
  let fn_call_kind = resolve_callee_kind(ctx, &args[3], acc, elem)
    .map_err(|e| format!("foldl-shortcut: {e}"))?;

  // Outer block for early exit; inner loop for iteration
  ctx.begin_block();
  ctx.begin_loop();

  // if i >= count: exit block (return default)
  ctx.loop_exit_if_ge(i, count);

  // elem = list_ptr[(1+i)*8]
  emit_list_load_elem(ctx, list_ptr, i);
  ctx.emit(Instruction::LocalSet(elem));

  // tuple_ptr = trunc(fn(acc, elem))
  emit_foldl_step(ctx, &fn_call_kind, acc, elem)?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(tuple_ptr));

  // tag = F64Load(tuple_ptr + 8) — bool flag (1.0 = true → early exit)
  ctx.emit(Instruction::LocalGet(tuple_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::F64Eq); // → i32

  ctx.begin_block_if();
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
  ctx.i32_inc(i);
  ctx.br_loop();

  ctx.end_block_loop();

  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

/// `foldr-shortcut xs acc default fn` — like foldl-shortcut but iterates right-to-left.
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
  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(i));

  let elem = ctx.alloc_local();
  let tuple_ptr = ctx.alloc_local_typed(ValType::I32);

  ctx.begin_block();
  ctx.begin_loop();

  // if i < 0: exit block (return default)
  ctx.loop_exit_if_neg(i);

  // elem = list_ptr[(1+i)*8]
  emit_list_load_elem(ctx, list_ptr, i);
  ctx.emit(Instruction::LocalSet(elem));

  // tuple_ptr = trunc(fn(acc, elem))
  emit_foldl_step(ctx, &fn_call_kind, acc, elem)?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(tuple_ptr));

  // tag = F64Load(tuple_ptr + 8) — bool flag
  ctx.emit(Instruction::LocalGet(tuple_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::F64Eq);

  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(tuple_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(16)));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::Br(2)); // break outer block
  ctx.emit(Instruction::End); // end if

  // else: acc = payload, i--
  ctx.emit(Instruction::LocalGet(tuple_ptr));
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

  // Evaluate xs → list pointer
  let list_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, list_ptr);

  // Evaluate initial acc
  let acc = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(acc));

  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(i));

  let elem = ctx.alloc_local();
  let result = ctx.alloc_local();
  ctx.emit(f64_const(1.0)); // default: true
  ctx.emit(Instruction::LocalSet(result));

  // Resolve callee after allocating acc/elem
  let fn_call_kind = resolve_callee_kind(ctx, &args[2], acc, elem)
    .map_err(|e| format!("foldl-compare: {e}"))?;

  // Outer block (break here with false or after exhausting list)
  ctx.begin_block();
  ctx.begin_loop();

  // if i >= count: break (result = true, already set)
  ctx.loop_exit_if_ge(i, count);

  // elem = list_ptr[(1+i)*8]
  emit_list_load_elem(ctx, list_ptr, i);
  ctx.emit(Instruction::LocalSet(elem));

  // condition = f(acc, elem) → 1.0 (true) or 0.0 (false)
  emit_foldl_step(ctx, &fn_call_kind, acc, elem)?;
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

/// Emit a call to the HOF callee with a single argument (elem local).
fn emit_unary_step(ctx: &mut WasmGenCtx, kind: &FoldlCallKind, arg: u32) -> Result<(), String> {
  use crate::calcit::{CalcitLocal, CalcitSymbolInfo, DYNAMIC_TYPE};
  use std::sync::Arc;
  match kind {
    FoldlCallKind::Static(fn_idx) => {
      ctx.emit(Instruction::LocalGet(arg));
      ctx.emit(Instruction::Call(*fn_idx));
      Ok(())
    }
    FoldlCallKind::Inline(params, body) => {
      let param = params.first().ok_or("inline lambda has no params")?;
      let old = ctx.locals.insert(param.clone(), arg);
      emit_body(ctx, body)?;
      match old {
        Some(v) => { ctx.locals.insert(param.clone(), v); }
        None => { ctx.locals.remove(param); }
      }
      Ok(())
    }
    FoldlCallKind::Proc(proc) => {
      let sym: Arc<str> = Arc::from("__hof_arg__");
      let dummy_info = Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("wasm"),
        at_def: Arc::from("__hof"),
      });
      let prev = ctx.locals.insert(sym.as_ref().to_owned(), arg);
      let expr = crate::calcit::Calcit::Local(CalcitLocal {
        idx: 0,
        sym: sym.clone(),
        info: dummy_info,
        location: None,
        type_info: DYNAMIC_TYPE.clone(),
      });
      let result = emit_proc_call(ctx, proc, &[expr]);
      match prev {
        Some(v) => { ctx.locals.insert(sym.as_ref().to_owned(), v); }
        None => { ctx.locals.remove(sym.as_ref()); }
      }
      result
    }
    // Dynamic dispatch via call_indirect: local holds f64 table slot index.
    // Canonical type 1 = (f64) → f64.
    FoldlCallKind::Dynamic(fn_local_idx) => {
      ctx.emit(Instruction::LocalGet(arg));
      ctx.emit(Instruction::LocalGet(*fn_local_idx));
      ctx.emit(Instruction::I32TruncF64S);
      ctx.emit(Instruction::CallIndirect { type_index: 1, table_index: 0 });
      Ok(())
    }
  }
}

/// Emit a call to the HOF callee with two arguments (elem, index) for map-indexed.
fn emit_binary_step_ei(ctx: &mut WasmGenCtx, kind: &FoldlCallKind, elem: u32, idx: u32) -> Result<(), String> {
  match kind {
    FoldlCallKind::Static(fn_idx) => {
      ctx.emit(Instruction::LocalGet(elem));
      ctx.ptr_to_f64(idx);
      ctx.emit(Instruction::Call(*fn_idx));
      Ok(())
    }
    FoldlCallKind::Inline(params, body) => {
      if params.len() < 2 {
        return Err("map-indexed inline lambda needs at least 2 params".into());
      }
      let old0 = ctx.locals.insert(params[0].clone(), elem);
      let old1 = ctx.locals.insert(params[1].clone(), idx);
      emit_body(ctx, body)?;
      match old0 { Some(v) => { ctx.locals.insert(params[0].clone(), v); } None => { ctx.locals.remove(&params[0]); } }
      match old1 { Some(v) => { ctx.locals.insert(params[1].clone(), v); } None => { ctx.locals.remove(&params[1]); } }
      Ok(())
    }
    FoldlCallKind::Proc(_) => Err("map-indexed proc callee not supported".into()),
    // Dynamic dispatch: (elem_f64, idx_as_f64) → f64. Canonical type 2.
    FoldlCallKind::Dynamic(fn_local_idx) => {
      ctx.emit(Instruction::LocalGet(elem));
      ctx.ptr_to_f64(idx);
      ctx.emit(Instruction::LocalGet(*fn_local_idx));
      ctx.emit(Instruction::I32TruncF64S);
      ctx.emit(Instruction::CallIndirect { type_index: 2, table_index: 0 });
      Ok(())
    }
  }
}

/// Resolve a single-arg HOF callee (map, filter, each, any?, etc.).
fn resolve_unary_callee(ctx: &WasmGenCtx, callee: &Calcit) -> Result<FoldlCallKind, String> {
  if let Ok(fn_idx) = resolve_callee_fn_idx(ctx, callee) {
    return Ok(FoldlCallKind::Static(fn_idx));
  }
  if let Some((params, body)) = try_extract_inline_lambda(callee) {
    return Ok(FoldlCallKind::Inline(params, body));
  }
  if let Calcit::Proc(proc) = callee {
    return Ok(FoldlCallKind::Proc(*proc));
  }
  // Local variable holding a function value (dyn dispatch via call_indirect)
  if let Calcit::Local(local) = callee {
    let local_idx = ctx
      .locals
      .get(&*local.sym)
      .copied()
      .ok_or_else(|| format!("HOF callee local not found: {}", local.sym))?;
    return Ok(FoldlCallKind::Dynamic(local_idx));
  }
  Err(format!("HOF callee must be a static fn/import/symbol in WASM, got: {callee}"))
}

/// `map xs f` — apply f to every element, returning new list of same length.
pub(super) fn emit_map(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "map")?;
  let src_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src_ptr);
  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::LocalSet(total_slots));
  let dst_ptr = emit_alloc_with_count(ctx, count, total_slots, "list");
  let kind = resolve_unary_callee(ctx, &args[1]).map_err(|e| format!("map: {e}"))?;
  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0)); ctx.emit(Instruction::LocalSet(i));
  let elem = ctx.alloc_local();
  ctx.begin_block();
  ctx.begin_loop();
  ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::LocalGet(count)); ctx.emit(Instruction::I32GeU); ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(src_ptr)); ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::I32Const(8)); ctx.emit(Instruction::I32Mul); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::F64Load(mem_arg_f64(0))); ctx.emit(Instruction::LocalSet(elem));
  ctx.emit(Instruction::LocalGet(dst_ptr)); ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::I32Const(8)); ctx.emit(Instruction::I32Mul); ctx.emit(Instruction::I32Add);
  emit_unary_step(ctx, &kind, elem)?;
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::LocalSet(i)); ctx.br_loop();
  ctx.end_block_loop();
  ctx.emit(Instruction::LocalGet(dst_ptr)); ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `map-indexed xs f` — apply f(elem, idx) to every element, returning new list.
pub(super) fn emit_map_indexed(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "map-indexed")?;
  let src_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src_ptr);
  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::LocalSet(total_slots));
  let dst_ptr = emit_alloc_with_count(ctx, count, total_slots, "list");
  let kind = resolve_unary_callee(ctx, &args[1]).map_err(|e| format!("map-indexed: {e}"))?;
  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0)); ctx.emit(Instruction::LocalSet(i));
  let elem = ctx.alloc_local();
  ctx.begin_block();
  ctx.begin_loop();
  ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::LocalGet(count)); ctx.emit(Instruction::I32GeU); ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(src_ptr)); ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::I32Const(8)); ctx.emit(Instruction::I32Mul); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::F64Load(mem_arg_f64(0))); ctx.emit(Instruction::LocalSet(elem));
  ctx.emit(Instruction::LocalGet(dst_ptr)); ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::I32Const(8)); ctx.emit(Instruction::I32Mul); ctx.emit(Instruction::I32Add);
  emit_binary_step_ei(ctx, &kind, elem, i)?;
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::LocalSet(i)); ctx.br_loop();
  ctx.end_block_loop();
  ctx.emit(Instruction::LocalGet(dst_ptr)); ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `each xs f` — apply f to every element for side effects; returns 0.0 (nil).
pub(super) fn emit_each(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "each")?;
  let src_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src_ptr);
  let kind = resolve_unary_callee(ctx, &args[1]).map_err(|e| format!("each: {e}"))?;
  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0)); ctx.emit(Instruction::LocalSet(i));
  let elem = ctx.alloc_local();
  ctx.begin_block();
  ctx.begin_loop();
  ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::LocalGet(count)); ctx.emit(Instruction::I32GeU); ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(src_ptr)); ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::I32Const(8)); ctx.emit(Instruction::I32Mul); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::F64Load(mem_arg_f64(0))); ctx.emit(Instruction::LocalSet(elem));
  emit_unary_step(ctx, &kind, elem)?;
  ctx.emit(Instruction::Drop);
  ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::LocalSet(i)); ctx.br_loop();
  ctx.end_block_loop();
  ctx.emit(f64_const(0.0));
  Ok(())
}

/// `filter xs f` / `&list:filter xs f` — return list of elements where f returns truthy.
pub(super) fn emit_filter(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "filter")?;
  let src_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src_ptr);
  let kind = resolve_unary_callee(ctx, &args[1]).map_err(|e| format!("filter: {e}"))?;
  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::LocalSet(total_slots));
  let dst_ptr = emit_alloc_with_count(ctx, count, total_slots, "list");
  let result_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0)); ctx.emit(Instruction::LocalSet(result_count));
  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0)); ctx.emit(Instruction::LocalSet(i));
  let elem = ctx.alloc_local();
  ctx.begin_block();
  ctx.begin_loop();
  ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::LocalGet(count)); ctx.emit(Instruction::I32GeU); ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(src_ptr)); ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::I32Const(8)); ctx.emit(Instruction::I32Mul); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::F64Load(mem_arg_f64(0))); ctx.emit(Instruction::LocalSet(elem));
  emit_unary_step(ctx, &kind, elem)?;
  ctx.emit(f64_const(0.0)); ctx.emit(Instruction::F64Ne);
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(dst_ptr)); ctx.emit(Instruction::LocalGet(result_count)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::I32Const(8)); ctx.emit(Instruction::I32Mul); ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(elem)); ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(result_count)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::LocalSet(result_count));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::LocalSet(i)); ctx.br_loop();
  ctx.end_block_loop();
  ctx.emit(Instruction::LocalGet(dst_ptr)); ctx.emit(Instruction::LocalGet(result_count)); ctx.emit(Instruction::F64ConvertI32U); ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(dst_ptr)); ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `any? xs f` — return 1.0 if any element satisfies f, else 0.0.
pub(super) fn emit_any(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "any?")?;
  let src_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src_ptr);
  let kind = resolve_unary_callee(ctx, &args[1]).map_err(|e| format!("any?: {e}"))?;
  let result = ctx.alloc_local();
  ctx.emit(f64_const(0.0)); ctx.emit(Instruction::LocalSet(result));
  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0)); ctx.emit(Instruction::LocalSet(i));
  let elem = ctx.alloc_local();
  ctx.begin_block();
  ctx.begin_loop();
  ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::LocalGet(count)); ctx.emit(Instruction::I32GeU); ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(src_ptr)); ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::I32Const(8)); ctx.emit(Instruction::I32Mul); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::F64Load(mem_arg_f64(0))); ctx.emit(Instruction::LocalSet(elem));
  emit_unary_step(ctx, &kind, elem)?;
  ctx.emit(f64_const(0.0)); ctx.emit(Instruction::F64Ne);
  ctx.begin_block_if();
  ctx.emit(f64_const(1.0)); ctx.emit(Instruction::LocalSet(result)); ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::LocalSet(i)); ctx.br_loop();
  ctx.end_block_loop();
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
  ctx.emit(f64_const(1.0)); ctx.emit(Instruction::LocalSet(result));
  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0)); ctx.emit(Instruction::LocalSet(i));
  let elem = ctx.alloc_local();
  ctx.begin_block();
  ctx.begin_loop();
  ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::LocalGet(count)); ctx.emit(Instruction::I32GeU); ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(src_ptr)); ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::I32Const(8)); ctx.emit(Instruction::I32Mul); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::F64Load(mem_arg_f64(0))); ctx.emit(Instruction::LocalSet(elem));
  emit_unary_step(ctx, &kind, elem)?;
  ctx.emit(f64_const(0.0)); ctx.emit(Instruction::F64Eq);
  ctx.begin_block_if();
  ctx.emit(f64_const(0.0)); ctx.emit(Instruction::LocalSet(result)); ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::LocalSet(i)); ctx.br_loop();
  ctx.end_block_loop();
  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

/// `find xs f` — return first element satisfying f, or nil (0.0) if none.
pub(super) fn emit_find(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "find")?;
  let src_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src_ptr);
  let kind = resolve_unary_callee(ctx, &args[1]).map_err(|e| format!("find: {e}"))?;
  let result = ctx.alloc_local();
  ctx.emit(f64_const(0.0)); ctx.emit(Instruction::LocalSet(result));
  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0)); ctx.emit(Instruction::LocalSet(i));
  let elem = ctx.alloc_local();
  ctx.begin_block();
  ctx.begin_loop();
  ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::LocalGet(count)); ctx.emit(Instruction::I32GeU); ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(src_ptr)); ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::I32Const(8)); ctx.emit(Instruction::I32Mul); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::F64Load(mem_arg_f64(0))); ctx.emit(Instruction::LocalSet(elem));
  emit_unary_step(ctx, &kind, elem)?;
  ctx.emit(f64_const(0.0)); ctx.emit(Instruction::F64Ne);
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(elem)); ctx.emit(Instruction::LocalSet(result)); ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::LocalSet(i)); ctx.br_loop();
  ctx.end_block_loop();
  ctx.emit(Instruction::LocalGet(result));
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
  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::LocalSet(total_slots));
  let dst_ptr = emit_alloc_with_count(ctx, count, total_slots, "list");
  let result_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0)); ctx.emit(Instruction::LocalSet(result_count));
  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0)); ctx.emit(Instruction::LocalSet(i));
  let elem = ctx.alloc_local();
  ctx.begin_block();
  ctx.begin_loop();
  ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::LocalGet(count)); ctx.emit(Instruction::I32GeU); ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(src_ptr)); ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::I32Const(8)); ctx.emit(Instruction::I32Mul); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::F64Load(mem_arg_f64(0))); ctx.emit(Instruction::LocalSet(elem));
  emit_unary_step(ctx, &kind, elem)?;
  ctx.emit(f64_const(0.0)); ctx.emit(Instruction::F64Eq);  // keep when f returns falsy (== 0.0)
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(dst_ptr)); ctx.emit(Instruction::LocalGet(result_count)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::I32Const(8)); ctx.emit(Instruction::I32Mul); ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(elem)); ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(result_count)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::LocalSet(result_count));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::LocalSet(i)); ctx.br_loop();
  ctx.end_block_loop();
  ctx.emit(Instruction::LocalGet(dst_ptr)); ctx.emit(Instruction::LocalGet(result_count)); ctx.emit(Instruction::F64ConvertI32U); ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(dst_ptr)); ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `update m k f` — return new map with `k` mapped to `f(m[k])`.
/// Only supports map operand (not list/tuple).
pub(super) fn emit_update(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(3, args, "update")?;
  // Evaluate map → i32 ptr
  let map_ptr = ctx.alloc_local_typed(ValType::I32);
  emit_expr(ctx, &args[0])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(map_ptr));

  // Evaluate key → f64 local
  let key_local = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(key_local));

  // cur_val = __rt_map_get_value(map_ptr, key)
  let get_fn_idx = *ctx.runtime_fn_index.get("__rt_map_get_value")
    .expect("runtime helper __rt_map_get_value must exist");
  ctx.emit(Instruction::LocalGet(map_ptr));
  ctx.emit(Instruction::LocalGet(key_local));
  ctx.emit(Instruction::Call(get_fn_idx));
  let cur_val = ctx.alloc_local();
  ctx.emit(Instruction::LocalSet(cur_val));

  // new_val = f(cur_val)
  let kind = resolve_unary_callee(ctx, &args[2]).map_err(|e| format!("update: {e}"))?;
  let new_val = ctx.alloc_local();
  emit_unary_step(ctx, &kind, cur_val)?;
  ctx.emit(Instruction::LocalSet(new_val));

  // result = __rt_map_assoc(map_ptr, key, new_val)
  let assoc_fn_idx = *ctx.runtime_fn_index.get("__rt_map_assoc")
    .expect("runtime helper __rt_map_assoc must exist");
  ctx.emit(Instruction::LocalGet(map_ptr));
  ctx.emit(Instruction::LocalGet(key_local));
  ctx.emit(Instruction::LocalGet(new_val));
  ctx.emit(Instruction::Call(assoc_fn_idx));
  ctx.emit(Instruction::F64ConvertI32U);
  Ok(())
}

/// `find-index xs f` — return f64 index of first matching element, or -1.0.
pub(super) fn emit_find_index(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "find-index")?;
  let src_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src_ptr);
  let kind = resolve_unary_callee(ctx, &args[1]).map_err(|e| format!("find-index: {e}"))?;
  let result = ctx.alloc_local();
  ctx.emit(f64_const(-1.0)); ctx.emit(Instruction::LocalSet(result));
  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0)); ctx.emit(Instruction::LocalSet(i));
  let elem = ctx.alloc_local();
  ctx.begin_block();
  ctx.begin_loop();
  ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::LocalGet(count)); ctx.emit(Instruction::I32GeU); ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(src_ptr)); ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::I32Const(8)); ctx.emit(Instruction::I32Mul); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::F64Load(mem_arg_f64(0))); ctx.emit(Instruction::LocalSet(elem));
  emit_unary_step(ctx, &kind, elem)?;
  ctx.emit(f64_const(0.0)); ctx.emit(Instruction::F64Ne);
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::F64ConvertI32U); ctx.emit(Instruction::LocalSet(result)); ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::LocalSet(i)); ctx.br_loop();
  ctx.end_block_loop();
  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

/// `map-kv xs f` — apply binary `f(k, v) → [k', v']` to every entry of a map, returning a new map.
/// Iterates via `__rt_map_linearize` which returns a flat `[n_pairs, k0, v0, k1, v1, ...]` buffer.
pub(super) fn emit_map_kv(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "map-kv")?;

  // Resolve binary callee: f(k, v) → [k', v']
  let kind = if let Some((params, body)) = try_extract_inline_lambda(&args[1]) {
    if params.len() < 2 {
      return Err("map-kv: inline lambda needs at least 2 params".into());
    }
    FoldlCallKind::Inline(params, body)
  } else {
    return Err("map-kv: callee must be an inline lambda in WASM".into());
  };

  // Evaluate map source → i32 ptr
  emit_expr(ctx, &args[0])?;
  let map_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(map_ptr));

  // __rt_map_linearize(map_ptr) → flat [n_pairs, k0, v0, ...] buffer (i32 ptr)
  let linearize_idx = *ctx.runtime_fn_index.get("__rt_map_linearize")
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

  let i = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(i));
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
  ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::I32Const(16)); ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(8)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(key));

  // val = flat_ptr[16 + i*16]
  ctx.emit(Instruction::LocalGet(flat_ptr));
  ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::I32Const(16)); ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(16)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(val));

  // result = f(key, val) — inline the lambda
  match &kind {
    FoldlCallKind::Inline(params, body) => {
      let old0 = ctx.locals.insert(params[0].clone(), key);
      let old1 = ctx.locals.insert(params[1].clone(), val);
      emit_body(ctx, body)?;
      match old0 { Some(v) => { ctx.locals.insert(params[0].clone(), v); } None => { ctx.locals.remove(&params[0]); } }
      match old1 { Some(v) => { ctx.locals.insert(params[1].clone(), v); } None => { ctx.locals.remove(&params[1]); } }
    }
    _ => unreachable!(),
  }

  // result is [k', v'] list on stack; get its pointer
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(result_ptr));

  // new_key = result[0] at offset 8, new_val = result[1] at offset 16
  ctx.emit(Instruction::LocalGet(result_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalSet(new_key));
  ctx.emit(Instruction::LocalGet(result_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(16)));
  ctx.emit(Instruction::LocalSet(new_val));

  // acc = __rt_map_assoc(acc, new_key, new_val)
  let assoc_idx = *ctx.runtime_fn_index.get("__rt_map_assoc").expect("__rt_map_assoc");
  ctx.emit(Instruction::LocalGet(acc));
  ctx.emit(Instruction::LocalGet(new_key));
  ctx.emit(Instruction::LocalGet(new_val));
  ctx.emit(Instruction::Call(assoc_idx));
  ctx.emit(Instruction::LocalSet(acc));

  ctx.emit(Instruction::LocalGet(i)); ctx.emit(Instruction::I32Const(1)); ctx.emit(Instruction::I32Add); ctx.emit(Instruction::LocalSet(i));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.ptr_to_f64(acc);
  Ok(())
}
