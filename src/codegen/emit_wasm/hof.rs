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
pub(super) fn emit_foldl(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
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
pub(super) enum FoldlCallKind {
  Static(u32),
  Inline(Vec<String>, Vec<Calcit>),
}

/// `foldl-shortcut xs acc default fn` — like foldl but fn returns `:: bool new_acc`.
/// If bool is true, return new_acc immediately (short-circuit). Otherwise continue.
/// After exhausting xs, return default.
/// fn must be statically resolvable or an inline lambda.
pub(super) fn emit_foldl_shortcut(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
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
pub(super) fn emit_foldr_shortcut(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
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
