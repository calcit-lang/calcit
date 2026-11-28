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
  let is_explicit_import = matches!(callee, Calcit::Import(_));
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
    .or_else(|| (!is_explicit_import).then(|| ctx.fn_index.get(&short)).flatten())
    .copied()
    .ok_or_else(|| format!("unknown HOF callee: {qualified}"))?;
  // Rest calls pack their arguments or trap when fixed arguments are missing.
  // Ordinary functions must match exactly, including Imports and known locals.
  if !ctx.fn_has_rest.contains_key(&qualified) && (is_explicit_import || !ctx.fn_has_rest.contains_key(&short)) {
    let arity = ctx
      .fn_arity
      .get(&qualified)
      .or_else(|| (!is_explicit_import).then(|| ctx.fn_arity.get(&short)).flatten())
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
  let is_explicit_import = matches!(callee, Calcit::Import(_));
  let (qualified, short) = match callee {
    Calcit::Fn { info, .. } => {
      let def_ref = info.def_ref.as_ref()?;
      (format!("{}/{}", def_ref.def_ns, def_ref.def_name), def_ref.def_name.to_string())
    }
    Calcit::Import(import) => (format!("{}/{}", import.ns, import.def), import.def.to_string()),
    Calcit::Symbol { sym, .. } => (sym.to_string(), sym.to_string()),
    _ => return None,
  };
  ctx
    .fn_has_rest
    .get(&qualified)
    .or_else(|| (!is_explicit_import).then(|| ctx.fn_has_rest.get(&short)).flatten())
    .copied()
}

/// Calls a static HOF callee with argument locals. For a `&` rest callee the
/// arguments after its fixed parameters are packed into the rest list, and a
/// call without every fixed argument traps like the native arity error (#1964).
fn emit_static_callee_call(ctx: &mut WasmGenCtx, fn_idx: u32, rest_fixed: Option<u32>, args: &[u32]) {
  match rest_fixed {
    None => {
      for arg in args {
        ctx.emit(Instruction::LocalGet(*arg));
      }
      emit_call_fn_with_f64_stack_args(ctx, fn_idx, args.len() as u32);
    }
    Some(fixed) if fixed as usize <= args.len() => {
      let fixed = fixed as usize;
      let rest = emit_list_from_locals(ctx, &args[fixed..]);
      for arg in &args[..fixed] {
        ctx.emit(Instruction::LocalGet(*arg));
      }
      ctx.emit(Instruction::LocalGet(rest));
      ctx.emit(Instruction::Call(fn_idx));
    }
    Some(_) => {
      ctx.emit(Instruction::Unreachable);
    }
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

pub(super) fn infer_collection_element_evidence(ctx: &WasmGenCtx, source: &Calcit) -> Option<WasmTypeEvidence> {
  let ev = infer_wasm_value_evidence(ctx, source)?;
  let elem_codec_hint = ev
    .codec_hint
    .as_ref()
    .and_then(|hint| match CalcitTypeAnnotation::expand_schema_alias(hint).as_ref() {
      CalcitTypeAnnotation::List(inner) | CalcitTypeAnnotation::Set(inner) => Some(inner.clone()),
      CalcitTypeAnnotation::Tuple(slots) => slots.first().filter(|first| slots.iter().all(|s| s == *first)).cloned(),
      _ => None,
    });
  let expanded = CalcitTypeAnnotation::expand_schema_alias(&ev.annotation);
  match expanded.as_ref() {
    CalcitTypeAnnotation::List(element) => Some(WasmTypeEvidence {
      codec_hint: elem_codec_hint,
      set_member_types: ev.set_member_types,
      ..WasmTypeEvidence::contract(element.clone())
    }),
    CalcitTypeAnnotation::Set(element) => Some(WasmTypeEvidence {
      annotation: element.clone(),
      codec_hint: elem_codec_hint,
      ..WasmTypeEvidence::contract(element.clone())
    }),
    CalcitTypeAnnotation::Tuple(slots) => {
      let annotation = if let Some(first) = slots.first()
        && slots.iter().all(|s| s == first)
      {
        first.clone()
      } else {
        DYNAMIC_TYPE.clone()
      };
      Some(WasmTypeEvidence {
        codec_hint: elem_codec_hint,
        set_member_types: ev.set_member_types,
        ..WasmTypeEvidence::contract(annotation)
      })
    }
    CalcitTypeAnnotation::Map(k, v) => Some(WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::Tuple(Arc::new(vec![
      k.clone(),
      v.clone(),
    ]))))),
    _ => None,
  }
}

fn infer_map_key_val_evidences(ctx: &WasmGenCtx, source: &Calcit) -> (Option<WasmTypeEvidence>, Option<WasmTypeEvidence>) {
  let Some(ev) = infer_wasm_value_evidence(ctx, source) else {
    return (None, None);
  };
  let expanded = CalcitTypeAnnotation::expand_schema_alias(&ev.annotation);
  if let CalcitTypeAnnotation::Map(k, v) = expanded.as_ref() {
    let k_exp = CalcitTypeAnnotation::expand_schema_alias(k);
    let key_ty = if matches!(k_exp.as_ref(), CalcitTypeAnnotation::Dynamic)
      && let Some(members) = &ev.set_member_types
      && let Some(first) = members.first()
      && members.iter().all(|m| m == first)
    {
      first.clone()
    } else {
      k.clone()
    };
    (
      Some(WasmTypeEvidence::contract(key_ty)),
      Some(WasmTypeEvidence::contract(v.clone())),
    )
  } else {
    (None, None)
  }
}

/// `foldl xs init fn` — iterate xs left-to-right, calling fn(acc, elem) each step.
/// Lists and sets yield elements. Maps yield two-item lists, as on native and JS.
pub(super) fn emit_foldl(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(3, args, "foldl")?;

  let acc_ev = infer_wasm_value_evidence(ctx, &args[1]);
  let elem_ev = infer_collection_element_evidence(ctx, &args[0]);
  let list_ptr = emit_foldl_collection(ctx, &args[0])?;
  // Load count (number of elements)
  let count = emit_load_container_count_i32(ctx, list_ptr);

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
  let acc_ev = validate_loop_accumulator_evidence(ctx, &fn_call_kind, acc_ev, std::slice::from_ref(&elem_ev));

  // Block + Loop pattern
  ctx.begin_block();
  ctx.begin_loop();

  // if i >= count: break out of block
  ctx.loop_exit_if_ge(i, count);

  // elem = list_ptr[(1+i)*8] — skip the count slot at offset 0
  emit_list_load_elem(ctx, list_ptr, i);
  ctx.emit(Instruction::LocalSet(elem));

  // acc = fn(acc, elem)
  emit_foldl_step(ctx, &fn_call_kind, acc, elem, [acc_ev, elem_ev])?;
  ctx.emit(Instruction::LocalSet(acc));

  // i += 1; continue loop
  ctx.i32_inc(i);
  ctx.br_loop();

  ctx.end_block_loop();

  // Push acc as result
  ctx.emit(Instruction::LocalGet(acc));
  Ok(())
}

fn emit_foldl_collection(ctx: &mut WasmGenCtx, source: &Calcit) -> Result<u32, String> {
  let annotation = infer_wasm_static_type(ctx, source).map(|ty| CalcitTypeAnnotation::expand_schema_alias(&ty));
  match annotation.as_deref() {
    Some(CalcitTypeAnnotation::List(_) | CalcitTypeAnnotation::Set(_)) => emit_ptr_to_i32(ctx, source),
    Some(CalcitTypeAnnotation::Map(_, _)) => {
      let map = emit_ptr_to_i32(ctx, source)?;
      Ok(emit_map_pair_collection(ctx, map, "list"))
    }
    _ => {
      let value = ctx.alloc_local();
      emit_expr(ctx, source)?;
      ctx.emit(Instruction::LocalSet(value));
      let is_heap = emit_is_heap_value(ctx, value);
      ctx.emit(Instruction::LocalGet(is_heap));
      ctx.emit(Instruction::I32Eqz);
      ctx.begin_block_if();
      ctx.emit(Instruction::Unreachable);
      ctx.emit(Instruction::End);

      let ptr = ctx.alloc_local_typed(ValType::I32);
      ctx.emit(Instruction::LocalGet(value));
      ctx.emit(Instruction::I32TruncF64U);
      ctx.emit(Instruction::LocalTee(ptr));
      ctx.emit(Instruction::I32Const(4));
      ctx.emit(Instruction::I32Sub);
      ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
      let tag = ctx.alloc_local_typed(ValType::I32);
      ctx.emit(Instruction::LocalTee(tag));
      ctx.emit(Instruction::I32Const(get_type_tag(ctx, "map") as i32));
      ctx.emit(Instruction::I32Eq);
      ctx.begin_block_if();
      let pairs = emit_map_pair_collection(ctx, ptr, "list");
      ctx.emit(Instruction::LocalGet(pairs));
      ctx.emit(Instruction::LocalSet(ptr));
      ctx.emit(Instruction::Else);
      ctx.emit(Instruction::LocalGet(tag));
      ctx.emit(Instruction::I32Const(get_type_tag(ctx, "list") as i32));
      ctx.emit(Instruction::I32Ne);
      ctx.emit(Instruction::LocalGet(tag));
      ctx.emit(Instruction::I32Const(get_type_tag(ctx, "set") as i32));
      ctx.emit(Instruction::I32Ne);
      ctx.emit(Instruction::I32And);
      ctx.begin_block_if();
      ctx.emit(Instruction::Unreachable);
      ctx.emit(Instruction::End);
      ctx.emit(Instruction::End);
      Ok(ptr)
    }
  }
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

/// Keep loop accumulator evidence only when an inline callback preserves that exact
/// type/origin across iterations; otherwise fall back to open accumulator evidence.
fn validate_loop_accumulator_evidence(
  ctx: &WasmGenCtx,
  fn_call_kind: &FoldlCallKind,
  acc_ev: Option<WasmTypeEvidence>,
  other_evs: &[Option<WasmTypeEvidence>],
) -> Option<WasmTypeEvidence> {
  match fn_call_kind {
    FoldlCallKind::Inline(_) => {}
    FoldlCallKind::Proc(proc) => {
      let preserves_type = matches!(
        proc,
        CalcitProc::NativeAdd
          | CalcitProc::NativeMinus
          | CalcitProc::NativeMultiply
          | CalcitProc::NativeDivide
          | CalcitProc::NativeMerge
          | CalcitProc::NativeUnion
          | CalcitProc::NativeSetIntersection
          | CalcitProc::NativeDifference
          | CalcitProc::NativeListConcat
          | CalcitProc::NativeStrConcat
          | CalcitProc::BitAnd
          | CalcitProc::BitOr
          | CalcitProc::BitXor
      );
      return if preserves_type { acc_ev } else { None };
    }
    FoldlCallKind::Static(_, _) | FoldlCallKind::Dynamic(_) => return acc_ev,
  }
  let FoldlCallKind::Inline(closure) = fn_call_kind else {
    unreachable!();
  };
  let init_acc = acc_ev?;
  let mut arg_evs = Vec::with_capacity(1 + other_evs.len());
  arg_evs.push(init_acc.clone());
  for ev in other_evs {
    arg_evs.push(
      ev.clone()
        .unwrap_or_else(|| WasmTypeEvidence::contract(crate::wasm::host::calcit::DYNAMIC_TYPE.clone())),
    );
  }
  let mut candidate = init_acc;
  for _ in 0..4 {
    arg_evs[0] = candidate.clone();
    let ret_ev = infer_inline_closure_result_with_evidences(ctx, closure, &arg_evs, &mut HashSet::new())?;
    let next_annotation = if ret_ev.annotation == candidate.annotation {
      candidate.annotation.clone()
    } else if candidate.set_member_types.as_deref() == Some(&[]) {
      let cand_exp = CalcitTypeAnnotation::expand_schema_alias(&candidate.annotation);
      let ret_exp = CalcitTypeAnnotation::expand_schema_alias(&ret_ev.annotation);
      match (cand_exp.as_ref(), ret_exp.as_ref()) {
        (CalcitTypeAnnotation::Map(_, _), CalcitTypeAnnotation::Map(_, _))
        | (CalcitTypeAnnotation::Set(_), CalcitTypeAnnotation::Set(_)) => ret_ev.annotation.clone(),
        _ => return None,
      }
    } else {
      return None;
    };
    let next_exact_impls = candidate.exact_impls && ret_ev.exact_impls;
    let next_codec_hint = (candidate.codec_hint == ret_ev.codec_hint)
      .then(|| candidate.codec_hint.clone())
      .flatten();
    let next_scalar_origin = (candidate.scalar_origin == ret_ev.scalar_origin)
      .then(|| candidate.scalar_origin.clone())
      .flatten();
    let next_set_member_types = super::join_branch_member_types(candidate.set_member_types.clone(), ret_ev.set_member_types);
    if next_annotation == candidate.annotation
      && next_exact_impls == candidate.exact_impls
      && next_codec_hint == candidate.codec_hint
      && next_scalar_origin == candidate.scalar_origin
      && next_set_member_types == candidate.set_member_types
    {
      return Some(candidate);
    }
    candidate.annotation = next_annotation;
    candidate.exact_impls = next_exact_impls;
    candidate.codec_hint = next_codec_hint;
    candidate.scalar_origin = next_scalar_origin;
    candidate.set_member_types = next_set_member_types;
  }
  None
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
  let [type_a, type_b] = types;
  let prev_type_a = match &type_a {
    Some(ty) => ctx.local_types.insert(sym_a.to_string(), WasmTypeEvidence::contract(ty.clone())),
    None => ctx.local_types.remove(sym_a.as_ref()),
  };
  let prev_type_b = match &type_b {
    Some(ty) => ctx.local_types.insert(sym_b.to_string(), WasmTypeEvidence::contract(ty.clone())),
    None => ctx.local_types.remove(sym_b.as_ref()),
  };
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
  match prev_type_a {
    Some(annotation) => {
      ctx.local_types.insert(sym_a.to_string(), annotation);
    }
    None => {
      ctx.local_types.remove(sym_a.as_ref());
    }
  }
  match prev_type_b {
    Some(annotation) => {
      ctx.local_types.insert(sym_b.to_string(), annotation);
    }
    None => {
      ctx.local_types.remove(sym_b.as_ref());
    }
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
pub(super) fn resolve_callee_kind(ctx: &mut WasmGenCtx, callee: &Calcit, _acc: u32, _elem: u32) -> Result<FoldlCallKind, String> {
  resolve_hof_callee(ctx, callee, 2)
}

/// Emit the body of a single foldl iteration step:
/// `acc = call_kind(acc, elem)`. Leaves the result on the stack (caller must LocalSet acc).
pub(super) fn emit_foldl_step(
  ctx: &mut WasmGenCtx,
  fn_call_kind: &FoldlCallKind,
  acc: u32,
  elem: u32,
  types: [Option<WasmTypeEvidence>; 2],
) -> Result<(), String> {
  match fn_call_kind {
    FoldlCallKind::Static(fn_idx, rest_fixed) => {
      emit_static_callee_call(ctx, *fn_idx, *rest_fixed, &[acc, elem]);
      Ok(())
    }
    FoldlCallKind::Inline(closure) => {
      let saved_resets = ctx.reset_initialized_atoms.clone();
      let res = emit_inline_closure_body_with_evidences(ctx, closure, &[acc, elem], &types);
      ctx.reset_initialized_atoms = saved_resets;
      res
    }
    FoldlCallKind::Proc(proc) => emit_foldl_proc_call(
      ctx,
      proc,
      acc,
      elem,
      [
        types[0].as_ref().map(|e| e.annotation.clone()),
        types[1].as_ref().map(|e| e.annotation.clone()),
      ],
    ),
    // Dynamic dispatch via closure BrTable trampoline: (closure_f64, acc_f64, elem_f64) -> f64.
    FoldlCallKind::Dynamic(fn_local_idx) => super::closure::emit_closure_call_from_locals(ctx, *fn_local_idx, &[acc, elem]),
  }
}

fn emit_stack_is_truthy_i32(ctx: &mut WasmGenCtx) {
  let value = ctx.alloc_local();
  ctx.emit(Instruction::LocalSet(value));
  super::emit_value_is_truthy_i32(ctx, value);
}

/// `&list:foldl-shortcut xs acc default fn` — like foldl but fn returns `:: bool new_acc`.
/// If bool is true, return new_acc immediately (short-circuit). Otherwise continue.
/// After exhausting xs, return default.
/// fn must be statically resolvable or an inline lambda.
pub(super) fn emit_foldl_shortcut(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(4, args, "&list:foldl-shortcut")?;

  let elem_ev = infer_collection_element_evidence(ctx, &args[0]);
  // Evaluate collection → i32 pointer
  let list_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_container_count_i32(ctx, list_ptr);

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
  emit_foldl_step(ctx, &fn_call_kind, acc, elem, [None, elem_ev])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(enum_ptr));

  // tag = I32Load(enum_ptr + 4) — bool flag (truthy → early exit)
  ctx.emit(Instruction::LocalGet(enum_ptr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(4)));

  ctx.begin_block_if();
  // Early exit: result = payload at enum_ptr + 8
  ctx.emit(Instruction::LocalGet(enum_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::Br(2)); // break outer block (If=0, Loop=1, Block=2)
  ctx.emit(Instruction::End); // end if

  // else: acc = payload, continue
  ctx.emit(Instruction::LocalGet(enum_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
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

  let elem_ev = infer_collection_element_evidence(ctx, &args[0]);
  let list_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_container_count_i32(ctx, list_ptr);

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

  // Resolve callback after source, accumulator, and default values are evaluated.
  let fn_call_kind = resolve_callee_kind(ctx, &args[3], acc, elem).map_err(|e| format!("foldr-shortcut: {e}"))?;

  ctx.begin_block();
  ctx.begin_loop();

  // if i < 0: exit block (return default)
  ctx.loop_exit_if_neg(i);

  // elem = list_ptr[(1+i)*8]
  emit_list_load_elem(ctx, list_ptr, i);
  ctx.emit(Instruction::LocalSet(elem));

  // enum_ptr = trunc(fn(acc, elem))
  emit_foldl_step(ctx, &fn_call_kind, acc, elem, [None, elem_ev])?;
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(enum_ptr));

  // tag = I32Load(enum_ptr + 4) — bool flag
  ctx.emit(Instruction::LocalGet(enum_ptr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(4)));

  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(enum_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::Br(2)); // break outer block
  ctx.emit(Instruction::End); // end if

  // else: acc = payload, i--
  ctx.emit(Instruction::LocalGet(enum_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
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
  let count = emit_load_container_count_i32(ctx, list_ptr);

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
  emit_foldl_step(
    ctx,
    &fn_call_kind,
    acc,
    elem,
    [
      accumulator_type.map(WasmTypeEvidence::contract),
      element_type.map(WasmTypeEvidence::contract),
    ],
  )?;
  // if condition is falsy: result = false, break
  emit_stack_is_truthy_i32(ctx);
  ctx.emit(Instruction::I32Eqz); // i32: 1 if condition was falsy (false/nil)
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
  let evidence = source_collection.and_then(|source| infer_collection_element_evidence(ctx, source));
  emit_unary_step_with_evidence(ctx, kind, arg, evidence)
}

fn emit_unary_step_with_type(
  ctx: &mut WasmGenCtx,
  kind: &FoldlCallKind,
  arg: u32,
  annotation: Option<Arc<CalcitTypeAnnotation>>,
) -> Result<(), String> {
  emit_unary_step_with_evidence(ctx, kind, arg, annotation.map(WasmTypeEvidence::contract))
}

fn emit_unary_step_with_evidence(
  ctx: &mut WasmGenCtx,
  kind: &FoldlCallKind,
  arg: u32,
  evidence: Option<WasmTypeEvidence>,
) -> Result<(), String> {
  use crate::wasm::host::calcit::{CalcitLocal, CalcitSymbolInfo, DYNAMIC_TYPE};
  use std::sync::Arc;
  match kind {
    FoldlCallKind::Static(fn_idx, rest_fixed) => {
      emit_static_callee_call(ctx, *fn_idx, *rest_fixed, &[arg]);
      Ok(())
    }
    FoldlCallKind::Inline(closure) => {
      let saved_resets = ctx.reset_initialized_atoms.clone();
      let res = emit_inline_closure_body_with_evidences(ctx, closure, &[arg], &[evidence]);
      ctx.reset_initialized_atoms = saved_resets;
      res
    }
    FoldlCallKind::Proc(proc) => {
      let sym: Arc<str> = Arc::from("__hof_arg__");
      let dummy_info = Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("wasm"),
        at_def: Arc::from("__hof"),
      });
      let prev = ctx.locals.insert(sym.as_ref().to_owned(), arg);
      let prev_type = match &evidence {
        Some(ev) => ctx.local_types.insert(sym.to_string(), ev.clone()),
        None => ctx.local_types.remove(sym.as_ref()),
      };
      let expr = crate::wasm::host::calcit::Calcit::Local(CalcitLocal {
        idx: 0,
        sym: sym.clone(),
        info: dummy_info,
        location: None,
        type_info: evidence.map(|ev| ev.annotation).unwrap_or_else(|| DYNAMIC_TYPE.clone()),
      });
      let result = emit_proc_call(ctx, proc, &[expr]);
      match prev_type {
        Some(annotation) => {
          ctx.local_types.insert(sym.to_string(), annotation);
        }
        None => {
          ctx.local_types.remove(sym.as_ref());
        }
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
    // Dynamic dispatch via closure BrTable trampoline: (closure_f64, arg_f64) -> f64.
    FoldlCallKind::Dynamic(fn_local_idx) => super::closure::emit_closure_call_from_locals(ctx, *fn_local_idx, &[arg]),
  }
}

/// Emit a call to the HOF callee with two arguments (index, elem) for map-indexed,
/// matching the native `(f idx elem)` callback order.
fn emit_binary_step_ie(
  ctx: &mut WasmGenCtx,
  kind: &FoldlCallKind,
  idx: u32,
  elem: u32,
  elem_ev: Option<WasmTypeEvidence>,
) -> Result<(), String> {
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
      let saved_resets = ctx.reset_initialized_atoms.clone();
      let res = emit_inline_closure_body_with_evidences(
        ctx,
        closure,
        &[idx_f64, elem],
        &[Some(WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::Number))), elem_ev],
      );
      ctx.reset_initialized_atoms = saved_resets;
      res
    }
    FoldlCallKind::Proc(_) => Err("map-indexed proc callee not supported".into()),
    // Dynamic dispatch via closure BrTable trampoline: (closure_f64, idx_f64, elem_f64) -> f64.
    FoldlCallKind::Dynamic(fn_local_idx) => {
      let idx_f64 = ctx.alloc_local();
      ctx.ptr_to_f64(idx);
      ctx.emit(Instruction::LocalSet(idx_f64));
      super::closure::emit_closure_call_from_locals(ctx, *fn_local_idx, &[idx_f64, elem])
    }
  }
}

/// Resolve a HOF callee for the argument count supplied by its caller.
fn resolve_hof_callee(ctx: &mut WasmGenCtx, mut callee: &Calcit, expected_arity: u32) -> Result<FoldlCallKind, String> {
  while let Calcit::List(xs) = callee
    && matches!(xs.first(), Some(Calcit::Syntax(CalcitSyntax::AssertType, _)))
    && xs.len() >= 2
  {
    callee = &xs[1];
  }
  if expected_arity == 1
    && match callee {
      Calcit::Proc(CalcitProc::NilQuestion) => true,
      Calcit::Import(import) => import.ns.as_ref() == crate::wasm::host::calcit::CORE_NS && import.def.as_ref() == "nil?",
      Calcit::Symbol { sym, info, .. } if !ctx.locals.contains_key(sym.as_ref()) => {
        info.at_ns.as_ref() == crate::wasm::host::calcit::CORE_NS && sym.as_ref() == "nil?"
      }
      _ => false,
    }
  {
    return Ok(FoldlCallKind::Proc(CalcitProc::NilQuestion));
  }
  if let Some(closure) = resolve_inline_closure(ctx, callee) {
    if !closure.body.iter().any(check_uses_recur) && !super::closure::inline_closure_requires_materialization(&closure) {
      return Ok(FoldlCallKind::Inline(closure));
    }
    let local_idx = super::closure::emit_materialize_inline_closure(ctx, &closure, Some(callee))?;
    return Ok(FoldlCallKind::Dynamic(local_idx));
  }
  let qualified_binding = match callee {
    Calcit::Import(imp) => Some(format!("{}/{}", imp.ns, imp.def)),
    Calcit::Symbol { sym, info, .. }
      if !ctx.locals.contains_key(sym.as_ref()) && !ctx.self_closure.as_ref().is_some_and(|(s, _, _)| s == sym.as_ref()) =>
    {
      Some(format!("{}/{}", info.at_ns, sym))
    }
    _ => None,
  };
  if let Some(q) = qualified_binding.as_deref()
    && !ctx.fn_index.contains_key(q)
    && (ctx.value_imports.contains_key(q) || ctx.atom_globals.contains_key(q))
  {
    let local_idx = ctx.alloc_local();
    emit_expr(ctx, callee)?;
    ctx.emit(Instruction::LocalSet(local_idx));
    return Ok(FoldlCallKind::Dynamic(local_idx));
  }
  if let Ok(fn_idx) = resolve_callee_fn_idx(ctx, callee, expected_arity) {
    return Ok(FoldlCallKind::Static(fn_idx, resolve_callee_rest_fixed(ctx, callee)));
  }
  if let Calcit::Proc(proc) = callee {
    return Ok(FoldlCallKind::Proc(*proc));
  }
  // Local variable, lifted closure self-binding, or dynamic expression holding a closure pointer.
  if let Some(name) = match callee {
    Calcit::Local(local) => Some(local.sym.as_ref()),
    Calcit::Symbol { sym, .. }
      if ctx.locals.contains_key(sym.as_ref()) || ctx.self_closure.as_ref().is_some_and(|(s, _, _)| s == sym.as_ref()) =>
    {
      Some(sym.as_ref())
    }
    _ => None,
  } {
    if let Some(&local_idx) = ctx.locals.get(name) {
      return Ok(FoldlCallKind::Dynamic(local_idx));
    }
    if let Some((ref self_name, _, _)) = ctx.self_closure
      && self_name == name
    {
      let local_idx = ctx.alloc_local();
      ctx.emit(Instruction::LocalGet(0));
      ctx.emit(Instruction::F64ConvertI32U);
      ctx.emit(Instruction::LocalSet(local_idx));
      return Ok(FoldlCallKind::Dynamic(local_idx));
    }
    return Err(format!("HOF callee local not found: {name}"));
  }
  if matches!(callee, Calcit::Fn { .. }) || matches!(callee, Calcit::List(xs) if !xs.is_empty()) {
    let local_idx = ctx.alloc_local();
    emit_expr(ctx, callee)?;
    ctx.emit(Instruction::LocalSet(local_idx));
    return Ok(FoldlCallKind::Dynamic(local_idx));
  }
  Err(format!("HOF callee must be a static fn/import/symbol in WASM, got: {callee}"))
}

/// Resolve a single-arg HOF callee (map, filter, each, any?, etc.).
fn resolve_unary_callee(ctx: &mut WasmGenCtx, callee: &Calcit) -> Result<FoldlCallKind, String> {
  resolve_hof_callee(ctx, callee, 1)
}

/// `&map:fold-kv map init f` — fold map entries in iteration order, passing
/// the accumulator, key, and value to `f`.
pub(super) fn emit_map_fold_kv(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(3, args, "&map:fold-kv")?;

  let (key_ev, val_ev) = infer_map_key_val_evidences(ctx, &args[0]);
  let acc_ev = infer_wasm_value_evidence(ctx, &args[1]);
  let map_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let linearize_idx = *ctx
    .runtime_fn_index
    .get("__rt_map_linearize")
    .ok_or("runtime __rt_map_linearize missing")?;
  ctx.emit(Instruction::LocalGet(map_ptr));
  ctx.emit(Instruction::Call(linearize_idx));
  let flat_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalSet(flat_ptr));

  let count = emit_load_container_count_i32(ctx, flat_ptr);

  let acc = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(acc));

  let kind = resolve_hof_callee(ctx, &args[2], 3).map_err(|error| format!("&map:fold-kv: {error}"))?;
  if let FoldlCallKind::Inline(closure) = &kind
    && !closure.accepts(3)
  {
    return Err("&map:fold-kv inline lambda must accept 3 arguments".into());
  }
  let acc_ev = validate_loop_accumulator_evidence(ctx, &kind, acc_ev, &[key_ev.clone(), val_ev.clone()]);

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
    FoldlCallKind::Inline(closure) => {
      emit_inline_closure_body_with_evidences(ctx, closure, &[acc, key, value], &[acc_ev, key_ev, val_ev])?
    }
    FoldlCallKind::Proc(proc) => emit_map_fold_kv_proc_call(ctx, proc, acc, key, value)?,
    FoldlCallKind::Dynamic(fn_idx) => super::closure::emit_closure_call_from_locals(ctx, *fn_idx, &[acc, key, value])?,
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

fn infer_hof_result_slot_kind(
  ctx: &mut WasmGenCtx,
  kind: &FoldlCallKind,
  callee: &Calcit,
  elem_ev: Option<WasmTypeEvidence>,
  arity: u32,
) -> ContainerSlotKind {
  let guard_boxed_scalar = |ctx: &WasmGenCtx, slot_kind: ContainerSlotKind, inline_body: Option<&[Calcit]>| -> ContainerSlotKind {
    if slot_kind != ContainerSlotKind::PureScalar || !ctx.has_boxed_json_scalars {
      return slot_kind;
    }
    if matches!(kind, FoldlCallKind::Proc(_)) {
      return slot_kind;
    }
    if let Some(body) = inline_body
      && body.last().is_some_and(|expr| super::expr_is_guaranteed_unboxed_scalar(ctx, expr))
    {
      return slot_kind;
    }
    if let FoldlCallKind::Static(fn_idx, _) = kind
      && ctx.fn_signatures.get(fn_idx).is_some_and(|sig| sig.result != NativeValRepr::F64)
    {
      return slot_kind;
    }
    ContainerSlotKind::DynamicBoxed
  };
  if let FoldlCallKind::Inline(closure) = kind {
    let mut arg_evs = Vec::with_capacity(arity as usize);
    if arity == 2 {
      arg_evs.push(WasmTypeEvidence::contract(Arc::new(CalcitTypeAnnotation::Number)));
    }
    arg_evs.push(elem_ev.unwrap_or_else(|| WasmTypeEvidence::contract(crate::wasm::host::calcit::DYNAMIC_TYPE.clone())));
    if let Some(ret_ev) = infer_inline_closure_result_with_evidences(ctx, closure, &arg_evs, &mut HashSet::new()) {
      let slot_kind = guard_boxed_scalar(
        ctx,
        ContainerSlotKind::from_type_annotation(&ret_ev.annotation),
        Some(&closure.body),
      );
      if slot_kind != ContainerSlotKind::DynamicBoxed {
        return slot_kind;
      }
    }
  }
  if let Some(ty) = infer_wasm_static_type(ctx, callee) {
    let expanded = expand_wasm_value_type(&ty);
    if let CalcitTypeAnnotation::Fn(fn_ty) = expanded.as_ref() {
      let slot_kind = guard_boxed_scalar(ctx, ContainerSlotKind::from_type_annotation(&fn_ty.return_type), None);
      if slot_kind != ContainerSlotKind::DynamicBoxed {
        return slot_kind;
      }
    }
  }
  if let Some(key) = direct_definition_reference(callee)
    && let Some((ns, def)) = key.rsplit_once('/')
    && let Some(signature) = program::lookup_def_schema(ns, def).resolve_to_fn()
    && !signature.return_type.contains_type_var()
  {
    let slot_kind = guard_boxed_scalar(ctx, ContainerSlotKind::from_type_annotation(&signature.return_type), None);
    if slot_kind != ContainerSlotKind::DynamicBoxed {
      return slot_kind;
    }
  }
  if let FoldlCallKind::Proc(proc) = kind {
    return match proc {
      CalcitProc::NativeAdd
      | CalcitProc::NativeMinus
      | CalcitProc::NativeMultiply
      | CalcitProc::NativeDivide
      | CalcitProc::NumberQuestion
      | CalcitProc::NilQuestion
      | CalcitProc::Not
      | CalcitProc::NativeListEmpty
      | CalcitProc::NativeMapEmpty
      | CalcitProc::NativeSetEmpty
      | CalcitProc::NativeListCount
      | CalcitProc::NativeMapCount
      | CalcitProc::NativeSetCount
      | CalcitProc::NativeStrCount => ContainerSlotKind::PureScalar,
      CalcitProc::NativeStr => ContainerSlotKind::ExactHeapPtr,
      _ => ContainerSlotKind::DynamicBoxed,
    };
  }
  ContainerSlotKind::DynamicBoxed
}

pub(super) fn emit_map(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "map")?;
  let elem_ev = infer_collection_element_evidence(ctx, &args[0]);
  let src_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_container_count_i32(ctx, src_ptr);
  let kind = resolve_unary_callee(ctx, &args[1]).map_err(|e| format!("map: {e}"))?;
  let static_out_kind = infer_hof_result_slot_kind(ctx, &kind, &args[1], elem_ev, 1);
  let out_kind = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  ctx.emit(Instruction::I32Const(ContainerSlotKind::EmptyUntyped as i32));
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::I32Const(static_out_kind as i32));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalSet(out_kind));
  let dst_ptr = emit_alloc_typed_list_dyn(ctx, count, out_kind);
  // Preserve source type tag (set→set, list→list)
  emit_copy_type_tag(ctx, src_ptr, dst_ptr);
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
  let elem_ev = infer_collection_element_evidence(ctx, &args[0]);
  let src_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_container_count_i32(ctx, src_ptr);
  let kind = resolve_hof_callee(ctx, &args[1], 2).map_err(|e| format!("map-indexed: {e}"))?;
  let static_out_kind = infer_hof_result_slot_kind(ctx, &kind, &args[1], elem_ev.clone(), 2);
  let out_kind = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
  ctx.emit(Instruction::I32Const(ContainerSlotKind::EmptyUntyped as i32));
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::I32Const(static_out_kind as i32));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalSet(out_kind));
  let dst_ptr = emit_alloc_typed_list_dyn(ctx, count, out_kind);
  let (i, elem) = emit_list_iter_begin(ctx, src_ptr, count);
  // write f(i, elem) into dst[i]
  ctx.emit(Instruction::LocalGet(dst_ptr));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  emit_binary_step_ie(ctx, &kind, i, elem, elem_ev)?;
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  emit_list_iter_end(ctx, i);
  ctx.ptr_to_f64(dst_ptr);
  Ok(())
}

/// `each xs f` — apply f to every element for side effects; returns 0.0 (nil).
pub(super) fn emit_each(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "each")?;
  let src_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_container_count_i32(ctx, src_ptr);
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
  let count = emit_load_container_count_i32(ctx, src_ptr);
  let elem_kind = emit_load_container_kind_i32(ctx, src_ptr);
  let kind = resolve_unary_callee(ctx, &args[1]).map_err(|e| format!("filter: {e}"))?;
  let dst_ptr = emit_alloc_typed_list_dyn(ctx, count, elem_kind);
  emit_copy_type_tag(ctx, src_ptr, dst_ptr);
  let result_count = ctx.alloc_i32(0);
  let (i, elem) = emit_list_iter_begin(ctx, src_ptr, count);
  emit_unary_step(ctx, &kind, elem, Some(&args[0]))?;
  emit_stack_is_truthy_i32(ctx);
  ctx.begin_block_if();
  emit_list_store_elem(ctx, dst_ptr, result_count, elem);
  ctx.i32_inc(result_count);
  ctx.emit(Instruction::End);
  emit_list_iter_end(ctx, i);
  // update count header to actual result_count
  ctx.emit(Instruction::LocalGet(dst_ptr));
  ctx.emit(Instruction::LocalGet(result_count));
  ctx.emit(Instruction::I32Store(mem_arg_i32(0)));
  ctx.ptr_to_f64(dst_ptr);
  Ok(())
}

/// `any? xs f` — return 1.0 if any element satisfies f, else 0.0.
pub(super) fn emit_any(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "any?")?;
  let src_ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_container_count_i32(ctx, src_ptr);
  let kind = resolve_unary_callee(ctx, &args[1]).map_err(|e| format!("any?: {e}"))?;
  let result = ctx.alloc_local();
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalSet(result));
  let (i, elem) = emit_list_iter_begin(ctx, src_ptr, count);
  emit_unary_step(ctx, &kind, elem, Some(&args[0]))?;
  emit_stack_is_truthy_i32(ctx);
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
  let count = emit_load_container_count_i32(ctx, src_ptr);
  let kind = resolve_unary_callee(ctx, &args[1]).map_err(|e| format!("every?: {e}"))?;
  let result = ctx.alloc_local();
  ctx.emit(f64_const(1.0));
  ctx.emit(Instruction::LocalSet(result));
  let (i, elem) = emit_list_iter_begin(ctx, src_ptr, count);
  emit_unary_step(ctx, &kind, elem, Some(&args[0]))?;
  emit_stack_is_truthy_i32(ctx);
  ctx.emit(Instruction::I32Eqz);
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
  let count = emit_load_container_count_i32(ctx, src_ptr);
  let kind = resolve_unary_callee(ctx, &args[1]).map_err(|e| format!("find: {e}"))?;
  let result = ctx.alloc_local();
  let found = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(found));
  let (i, elem) = emit_list_iter_begin(ctx, src_ptr, count);
  emit_unary_step(ctx, &kind, elem, Some(&args[0]))?;
  emit_stack_is_truthy_i32(ctx);
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
  let count = emit_load_container_count_i32(ctx, src_ptr);
  let elem_kind = emit_load_container_kind_i32(ctx, src_ptr);
  let kind = resolve_unary_callee(ctx, &args[1]).map_err(|e| format!("filter-not: {e}"))?;
  let dst_ptr = emit_alloc_typed_list_dyn(ctx, count, elem_kind);
  emit_copy_type_tag(ctx, src_ptr, dst_ptr);
  let result_count = ctx.alloc_i32(0);
  let (i, elem) = emit_list_iter_begin(ctx, src_ptr, count);
  emit_unary_step(ctx, &kind, elem, Some(&args[0]))?;
  emit_stack_is_truthy_i32(ctx);
  ctx.emit(Instruction::I32Eqz); // keep when f returns falsy (false/nil)
  ctx.begin_block_if();
  emit_list_store_elem(ctx, dst_ptr, result_count, elem);
  ctx.i32_inc(result_count);
  ctx.emit(Instruction::End);
  emit_list_iter_end(ctx, i);
  // update count header to actual result_count
  ctx.emit(Instruction::LocalGet(dst_ptr));
  ctx.emit(Instruction::LocalGet(result_count));
  ctx.emit(Instruction::I32Store(mem_arg_i32(0)));
  ctx.ptr_to_f64(dst_ptr);
  Ok(())
}

/// `update x k f` — rewrite the slot `k` of a List, Map or Enum with `f(old)`.
/// Like the core definition, a missing list index or map key returns `x`
/// unchanged without calling `f`; the receiver kind is dispatched at runtime.
pub(super) fn emit_update(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(3, args, "update")?;
  let receiver_type = infer_wasm_static_type(ctx, &args[0]).map(|ty| expand_wasm_value_type(&ty));
  if let Some(struct_def) = receiver_type.as_ref().and_then(|ty| ty.resolve_to_struct())
    && let Calcit::Tag(field) = &args[1]
  {
    let receiver = ctx.alloc_local();
    emit_expr(ctx, &args[0])?;
    ctx.emit(Instruction::LocalSet(receiver));
    emit_expr(ctx, &args[1])?;
    ctx.emit(Instruction::Drop);
    let kind = resolve_unary_callee(ctx, &args[2]).map_err(|e| format!("update: {e}"))?;
    if let Some(slot) = struct_def.fields.iter().position(|f| f == field) {
      let field_type = struct_def.field_types.get(slot).map(|ft| {
        if let Some(CalcitTypeAnnotation::Struct(_, type_args)) = receiver_type.as_deref()
          && !struct_def.generics.is_empty()
          && !type_args.is_empty()
        {
          let bindings = struct_def
            .generics
            .iter()
            .cloned()
            .zip(type_args.iter().cloned())
            .collect::<HashMap<_, _>>();
          ft.substitute_type_vars(&bindings)
        } else {
          ft.clone()
        }
      });
      let struct_id = ctx.struct_layouts.id(&struct_def)?;
      let field_layout = ctx.struct_layouts.layout_by_id(struct_id)?.fields[slot];
      let recv_ptr = ctx.alloc_local_typed(ValType::I32);
      ctx.emit(Instruction::LocalGet(receiver));
      ctx.emit(Instruction::I32TruncF64U);
      ctx.emit(Instruction::LocalSet(recv_ptr));
      let cur_val = ctx.alloc_local();
      super::emit_load_struct_field_as_repr(ctx, recv_ptr, field_layout, NativeValRepr::F64);
      ctx.emit(Instruction::LocalSet(cur_val));
      let new_val = ctx.alloc_local();
      emit_unary_step_with_type(ctx, &kind, cur_val, field_type)?;
      ctx.emit(Instruction::LocalSet(new_val));
      return super::structs::emit_struct_update_from_locals(ctx, receiver, new_val, slot, field.ref_str());
    }
    ctx.emit(Instruction::LocalGet(receiver));
    return Ok(());
  }

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
  let count = emit_load_container_count_i32(ctx, src_ptr);
  let kind = resolve_unary_callee(ctx, &args[1]).map_err(|e| format!("find-index: {e}"))?;
  let result = ctx.alloc_local();
  let found = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(found));
  let (i, elem) = emit_list_iter_begin(ctx, src_ptr, count);
  emit_unary_step(ctx, &kind, elem, Some(&args[0]))?;
  emit_stack_is_truthy_i32(ctx);
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

  let (key_ev, val_ev) = infer_map_key_val_evidences(ctx, &args[0]);
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

  // n_pairs = flat_ptr[0] (`u32 @ 0`)
  let n_pairs = emit_load_container_count_i32(ctx, flat_ptr);

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
    FoldlCallKind::Inline(closure) => emit_inline_closure_body_with_evidences(ctx, closure, &[key, val], &[key_ev, val_ev])?,
    _ => unreachable!(),
  }

  // `map-kv` returns a [k', v'] pair list, while `filter-map-kv` returns a
  // MapEntryDecision enum. Both are heap values whose pointer is on the stack.
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(result_ptr));

  let assoc_idx = *ctx.runtime_fn_index.get("__rt_map_assoc").expect("__rt_map_assoc");
  if uses_decision {
    let keep_tag = *ctx.tag_index.get("keep").ok_or("filter-map-kv: :keep tag missing")?;

    // Compact enum layout is `[count:u32 @ 0][tag_id:u32 @ 4][payload_0:f64 @ 8]...`.
    // Only :keep carries the replacement key and value; :drop leaves acc intact.
    ctx.emit(Instruction::LocalGet(result_ptr));
    ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
    ctx.emit(Instruction::I32Const(keep_tag as i32));
    ctx.emit(Instruction::I32Eq);
    ctx.begin_block_if();

    ctx.emit(Instruction::LocalGet(result_ptr));
    ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
    ctx.emit(Instruction::LocalSet(new_key));
    ctx.emit(Instruction::LocalGet(result_ptr));
    ctx.emit(Instruction::F64Load(mem_arg_f64(16)));
    ctx.emit(Instruction::LocalSet(new_val));

    ctx.emit(Instruction::LocalGet(acc));
    ctx.emit(Instruction::LocalGet(new_key));
    ctx.emit(Instruction::LocalGet(new_val));
    ctx.emit(Instruction::I32Const(0));
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
    ctx.emit(Instruction::I32Const(0));
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
        dyn_traits: Default::default(),
        fn_index: HashMap::new(),
        fn_arity: HashMap::new(),
        fn_has_rest: HashMap::new(),
        fn_signatures: HashMap::new(),
        has_boxed_json_scalars: true,
        runtime_fn_index: HashMap::from([("__rt_copy_f64_slots".into(), 0)]),
        tag_index: HashMap::new(),
        struct_layouts: Default::default(),
        enum_layouts: Default::default(),
        string_pool: HashMap::new(),
        atom_globals: HashMap::new(),
        atom_is_scalar: HashMap::new(),
        value_imports: HashMap::new(),
        static_fn_defs: HashMap::new(),
        closures: Default::default(),
        host_imports: HashMap::new(),
        target: WasmTarget::Core,
        boundary: WasmBoundary::Native,
        world: WasiWorld::Command,
        non_coherent_nominal_defs: Default::default(),
        any_type_table: Default::default(),
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
          let kind = resolve_hof_callee(&mut ctx, callee, supplied).expect("local call strategy");
          if arity == supplied {
            assert!(matches!(kind, FoldlCallKind::Static(index, None) if index == arity));
          } else {
            assert!(matches!(kind, FoldlCallKind::Dynamic(index) if index == slot));
            assert!(resolve_hof_callee(&mut ctx, &Calcit::Import(definition.clone()), supplied).is_err());
          }
        }
      }
      assert_eq!(
        matches!(resolve_unary_callee(&mut ctx, &alias).unwrap(), FoldlCallKind::Static(_, _)),
        arity == 1
      );
      assert_eq!(
        matches!(resolve_callee_kind(&mut ctx, &alias, 0, 0).unwrap(), FoldlCallKind::Static(_, _)),
        arity == 2
      );
      // An unknown local shadows the same-named global, even with a matching arity.
      ctx.fn_index.insert(name.to_string(), arity);
      ctx.fn_arity.insert(name.to_string(), arity);
      ctx.local_function_values.remove(&slot);
      for supplied in [1, 2] {
        assert!(matches!(resolve_hof_callee(&mut ctx, &symbol, supplied).unwrap(), FoldlCallKind::Dynamic(index) if index == slot));
      }
      assert!(matches!(resolve_callee_kind(&mut ctx, &symbol, 0, 0).unwrap(), FoldlCallKind::Dynamic(index) if index == slot));
      ctx.local_function_values.insert(slot, definition);
      // Rest functions still use the shared pack-or-trap path at either call site.
      ctx.fn_has_rest.insert(format!("tests.hof/{name}"), arity - 1);
      for supplied in [1, 2] {
        assert!(
          matches!(resolve_hof_callee(&mut ctx, &alias, supplied).unwrap(), FoldlCallKind::Static(_, Some(fixed)) if fixed == arity - 1)
        );
      }
    }
  }

  #[test]
  fn struct_update_passes_field_type_to_nil_predicate_callback() {
    let mut struct_def = CalcitStructDef::from_fields(cirru_edn::EdnTag::from("Flagged"), vec![cirru_edn::EdnTag::from("flag")]);
    struct_def.field_types = Arc::new(vec![Arc::new(CalcitTypeAnnotation::Bool)]);
    let struct_def = Arc::new(struct_def);
    let mut struct_layouts = StructLayouts::default();
    struct_layouts
      .ids
      .insert(super::structs::StructIdentity::from(struct_def.as_ref()), 1);
    struct_layouts.field_tags.insert(1, vec![2]);
    struct_layouts
      .layouts
      .insert(1, StructLayout::compute(&[StructFieldKind::BoolU8]).expect("Flagged layout"));

    let mut ctx = WasmGenCtx::new(
      0,
      WasmCompileEnv {
        function_value_dependencies: Default::default(),
        dyn_traits: Default::default(),
        fn_index: HashMap::from([("calcit.core/nil?".into(), 1)]),
        fn_arity: HashMap::from([("calcit.core/nil?".into(), 1)]),
        fn_has_rest: HashMap::new(),
        fn_signatures: HashMap::new(),
        has_boxed_json_scalars: true,
        runtime_fn_index: HashMap::from([("__rt_copy_f64_slots".into(), 0)]),
        tag_index: HashMap::from([("struct".into(), 1), ("flag".into(), 2), ("number".into(), 3), ("list".into(), 4)]),
        struct_layouts,
        enum_layouts: Default::default(),
        string_pool: HashMap::new(),
        atom_globals: HashMap::new(),
        atom_is_scalar: HashMap::new(),
        value_imports: HashMap::new(),
        static_fn_defs: HashMap::new(),
        closures: Default::default(),
        host_imports: HashMap::new(),
        target: WasmTarget::Core,
        boundary: WasmBoundary::Native,
        world: WasiWorld::Command,
        non_coherent_nominal_defs: Default::default(),
        any_type_table: Default::default(),
      },
    );
    ctx.locals.insert("item".into(), 0);
    let struct_type = Arc::new(CalcitTypeAnnotation::Struct(struct_def, Arc::new(vec![])));
    ctx
      .local_types
      .insert("item".into(), WasmTypeEvidence::contract(struct_type.clone()));

    let receiver = Calcit::Local(CalcitLocal {
      idx: 0,
      sym: Arc::from("item"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.hof"),
        at_def: Arc::from("test"),
      }),
      location: None,
      type_info: struct_type,
    });
    let nil_callee = Calcit::Import(CalcitImport {
      ns: Arc::from(crate::wasm::host::calcit::CORE_NS),
      def: Arc::from("nil?"),
      info: Arc::new(ImportInfo::Core {
        at_ns: Arc::from("tests.hof"),
      }),
      def_id: None,
    });
    emit_update(
      &mut ctx,
      &[receiver.clone(), Calcit::Tag(cirru_edn::EdnTag::from("flag")), nil_callee],
    )
    .expect("Struct update with Bool field and nil? callback must receive field type evidence");

    let unresolved_param = Calcit::Local(CalcitLocal {
      idx: 0,
      sym: Arc::from("x"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.hof"),
        at_def: Arc::from("cb"),
      }),
      location: None,
      type_info: crate::wasm::host::calcit::DYNAMIC_TYPE.clone(),
    });
    let inline_nil_lambda = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::Defn, Arc::from("tests.hof")),
      Calcit::Symbol {
        sym: Arc::from("cb"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.hof"),
          at_def: Arc::from("cb"),
        }),
        location: None,
      },
      Calcit::from(vec![unresolved_param.clone()]),
      Calcit::from(vec![Calcit::Proc(CalcitProc::NilQuestion), unresolved_param]),
    ]);
    emit_update(
      &mut ctx,
      &[receiver, Calcit::Tag(cirru_edn::EdnTag::from("flag")), inline_nil_lambda.clone()],
    )
    .expect("Struct update with inline nil? callback and unresolved param must receive field type evidence");

    let list_type = Arc::new(CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Bool)));
    ctx.locals.insert("xs".into(), 1);
    ctx.local_types.insert("xs".into(), WasmTypeEvidence::contract(list_type.clone()));
    let list_receiver = Calcit::Local(CalcitLocal {
      idx: 0,
      sym: Arc::from("xs"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.hof"),
        at_def: Arc::from("test"),
      }),
      location: None,
      type_info: list_type,
    });
    emit_map(&mut ctx, &[list_receiver, inline_nil_lambda])
      .expect("map over List<Bool> with inline nil? callback and unresolved param must receive element type evidence");
  }

  #[test]
  fn resolve_hof_callee_loads_lifted_self_closure_when_unshadowed() {
    use crate::wasm::host::calcit::{CalcitLocal, CalcitSymbolInfo, DYNAMIC_TYPE};
    let mut ctx = WasmGenCtx::new(
      1,
      WasmCompileEnv {
        function_value_dependencies: Default::default(),
        dyn_traits: Default::default(),
        fn_index: HashMap::new(),
        fn_arity: HashMap::new(),
        fn_has_rest: HashMap::new(),
        fn_signatures: HashMap::new(),
        has_boxed_json_scalars: true,
        runtime_fn_index: HashMap::new(),
        tag_index: HashMap::new(),
        struct_layouts: StructLayouts::default(),
        enum_layouts: Default::default(),
        string_pool: HashMap::new(),
        atom_globals: HashMap::new(),
        atom_is_scalar: HashMap::new(),
        value_imports: HashMap::new(),
        static_fn_defs: HashMap::new(),
        closures: Default::default(),
        host_imports: HashMap::new(),
        target: WasmTarget::Core,
        boundary: WasmBoundary::Native,
        world: WasiWorld::Command,
        non_coherent_nominal_defs: Default::default(),
        any_type_table: Default::default(),
      },
    );
    ctx.self_closure = Some((
      "walk".into(),
      42,
      super::closure::ClosureParamShape {
        params: vec!["n".into()],
        required: 1,
        rest: false,
        option_types: Vec::new(),
      },
    ));
    let walk_local = Calcit::Local(CalcitLocal {
      idx: 0,
      sym: Arc::from("walk"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.hof"),
        at_def: Arc::from("walk"),
      }),
      location: None,
      type_info: DYNAMIC_TYPE.clone(),
    });
    let resolved = resolve_hof_callee(&mut ctx, &walk_local, 1).expect("unshadowed self_closure should resolve");
    assert!(matches!(resolved, FoldlCallKind::Dynamic(_)));
    ctx.locals.insert("walk".into(), 9);
    let shadowed = resolve_hof_callee(&mut ctx, &walk_local, 1).expect("shadowed local should win over self_closure");
    assert!(matches!(shadowed, FoldlCallKind::Dynamic(9)));
  }
}
