use super::*;

// ===========================================================================
// Set operations — layout: [count:f64] [elem0:f64] [elem1:f64] ...
// ===========================================================================

/// `#{} elem0 elem1 ...` — create a set.
pub(super) fn emit_set_new(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
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

  ctx.ptr_to_f64(ptr);
  Ok(())
}

/// Tag for which native set binary operation to apply in `emit_set_op_variadic`.
pub(super) enum SetOpKind {
  Union,
  Difference,
  Intersection,
  Include,
}

fn check_set_comparison_types(ctx: &WasmGenCtx, left: &Calcit, right: &Calcit) -> Result<(), String> {
  let element_type = |set| {
    let annotation = CalcitTypeAnnotation::expand_schema_alias(&infer_wasm_static_type(ctx, set)?);
    match annotation.as_ref() {
      CalcitTypeAnnotation::Set(element) => Some(element.clone()),
      CalcitTypeAnnotation::Never => Some(annotation),
      _ => None,
    }
  };
  let left = element_type(left).unwrap_or_else(|| DYNAMIC_TYPE.clone());
  let right = element_type(right).unwrap_or_else(|| DYNAMIC_TYPE.clone());
  if set_comparison_can_erase_kind(&left, &right, 0) {
    return Err(
      "E_WASM_SET_ELEMENT_KIND: set comparison needs element types that rule out distinct scalar kinds sharing a WASM value".into(),
    );
  }
  Ok(())
}

/// The structural comparator uses untagged f64 values, including when it
/// descends into containers. Compare corresponding slots so a heterogeneous
/// tuple can still be compared with another tuple of the same shape.
fn set_comparison_can_erase_kind(left: &Arc<CalcitTypeAnnotation>, right: &Arc<CalcitTypeAnnotation>, depth: usize) -> bool {
  use CalcitTypeAnnotation as T;

  let left = T::expand_schema_alias(left);
  let right = T::expand_schema_alias(right);
  if matches!(left.as_ref(), T::Never) || matches!(right.as_ref(), T::Never) {
    return false;
  }
  // Recursive declarations and unresolved shapes cannot serve as evidence.
  if depth >= 32 {
    return true;
  }
  let compare = |left: &Arc<T>, right: &Arc<T>| set_comparison_can_erase_kind(left, right, depth + 1);
  let scalar_kind = proven_scalar_kind;
  if let (Some(left), Some(right)) = (scalar_kind(&left), scalar_kind(&right)) {
    // Tags start at 1, while Nil and Unit use 0.
    return left != right && !matches!((left, right), ("tag", "nil" | "unit") | ("nil" | "unit", "tag"));
  }
  match (left.as_ref(), right.as_ref()) {
    (T::Nullable(inner, NullableOrigin::Inferred), _) => compare(inner, &right) || compare(&Arc::new(T::Nil), &right),
    (_, T::Nullable(inner, NullableOrigin::Inferred)) => compare(&left, inner) || compare(&left, &Arc::new(T::Nil)),
    (T::List(left), T::List(right)) | (T::Set(left), T::Set(right)) => compare(left, right),
    (T::Tuple(left), T::Tuple(right)) => {
      left.len() == right.len() && left.iter().zip(right.iter()).any(|(left, right)| compare(left, right))
    }
    (T::List(member), T::Tuple(slots)) => slots.iter().any(|slot| compare(member, slot)),
    (T::Tuple(slots), T::List(member)) => slots.iter().any(|slot| compare(slot, member)),
    (T::Map(left_key, left_value), T::Map(right_key, right_value)) => compare(left_key, right_key) || compare(left_value, right_value),
    (T::Struct(..) | T::StructValue(_) | T::TypeRef(..), T::Struct(..) | T::StructValue(_) | T::TypeRef(..))
      if let (Some(left_def), Some(right_def)) = (left.resolve_to_struct(), right.resolve_to_struct()) =>
    {
      left_def.fields == right_def.fields
        && left_def
          .field_types
          .iter()
          .zip(right_def.field_types.iter())
          .any(|(left_field, right_field)| {
            compare(
              &set_comparison_payload(&left, &left_def.generics, left_field, left_def.definition_ref.as_deref()),
              &set_comparison_payload(&right, &right_def.generics, right_field, right_def.definition_ref.as_deref()),
            )
          })
    }
    (T::Enum(..) | T::EnumValue(_) | T::TypeRef(..), T::Enum(..) | T::EnumValue(_) | T::TypeRef(..))
      if let (Some(left_def), Some(right_def)) = (left.resolve_to_enum(), right.resolve_to_enum()) =>
    {
      left_def.variants().iter().any(|left_variant| {
        right_def
          .find_variant_by_name(left_variant.tag.ref_str())
          .is_some_and(|right_variant| {
            left_variant.arity() == right_variant.arity()
              && left_variant
                .payload_types()
                .iter()
                .zip(right_variant.payload_types())
                .any(|(left_payload, right_payload)| {
                  compare(
                    &set_comparison_payload(
                      &left,
                      left_def.generics(),
                      left_payload,
                      left_def.definition_ref().map(AsRef::as_ref),
                    ),
                    &set_comparison_payload(
                      &right,
                      right_def.generics(),
                      right_payload,
                      right_def.definition_ref().map(AsRef::as_ref),
                    ),
                  )
                })
          })
      })
    }
    (T::String, T::String) => false,
    // Different known heap layouts do not share an address or recurse.
    (
      T::String | T::List(_) | T::Tuple(_) | T::Set(_) | T::Map(_, _),
      T::String | T::List(_) | T::Tuple(_) | T::Set(_) | T::Map(_, _),
    ) => false,
    // Number may equal a heap pointer; the other scalar/heap comparisons
    // have the same representation constraints as elsewhere in codegen.
    (T::String | T::List(_) | T::Tuple(_) | T::Set(_) | T::Map(_, _), T::Bool | T::Nil | T::Unit)
    | (T::Bool | T::Nil | T::Unit, T::String | T::List(_) | T::Tuple(_) | T::Set(_) | T::Map(_, _)) => false,
    _ => true,
  }
}

fn set_comparison_payload(
  annotation: &CalcitTypeAnnotation,
  generics: &[Arc<str>],
  payload: &Arc<CalcitTypeAnnotation>,
  definition_ref: Option<&str>,
) -> Arc<CalcitTypeAnnotation> {
  use CalcitTypeAnnotation as T;

  let payload = match definition_ref.and_then(|reference| reference.rsplit_once('/')) {
    Some((ns, _)) => crate::wasm::host::calcit::type_refs::qualify_namespace_type_refs(payload.clone(), ns),
    None => payload.clone(),
  };
  match annotation {
    T::Struct(_, args) | T::Enum(_, args) | T::TypeRef(_, args) if !args.is_empty() => {
      let bindings = generics.iter().cloned().zip(args.iter().cloned()).collect();
      payload.substitute_type_vars(&bindings)
    }
    _ => payload,
  }
}

/// High-level set operations `(op base & xs)` — fold the extra args using the
/// corresponding native 2-arg set operation.  Handles 1…N extra args.
pub(super) fn emit_set_op_variadic(ctx: &mut WasmGenCtx, args: &[Calcit], kind: SetOpKind) -> Result<(), String> {
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
      SetOpKind::Intersection => emit_set_intersection(ctx, args),
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
      SetOpKind::Intersection => emit_set_intersection(ctx, &first_two)?,
      SetOpKind::Include => emit_set_include(ctx, &first_two)?,
    }
    ctx.emit(Instruction::LocalSet(acc));
  }
  for extra in &args[2..] {
    if matches!(kind, SetOpKind::Difference | SetOpKind::Intersection) {
      check_set_comparison_types(ctx, &args[0], extra)?;
    }
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
      SetOpKind::Intersection => {
        let extra_ptr = ctx.alloc_local_typed(ValType::I32);
        ctx.emit(Instruction::LocalGet(extra_val));
        ctx.emit(Instruction::I32TruncF64U);
        ctx.emit(Instruction::LocalSet(extra_ptr));
        emit_set_intersection_from_ptrs(ctx, acc_ptr, extra_ptr)
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
pub(super) fn emit_set_union_from_ptrs(ctx: &mut WasmGenCtx, a: u32, b: u32) -> Result<u32, String> {
  let a_count = emit_load_count_i32(ctx, a);
  let b_count = emit_load_count_i32(ctx, b);

  let max_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::LocalGet(b_count));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(max_count));

  let total_slots = ctx.i32_offset(max_count, 1);
  let dst = emit_alloc_with_count(ctx, max_count, total_slots, "set");

  let db = emit_addr_offset(ctx, dst, 8);
  let sb = emit_addr_offset(ctx, a, 8);
  emit_copy_f64_loop(ctx, db, sb, a_count);

  let write_idx = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::LocalSet(write_idx));

  let bi = ctx.alloc_i32(0);

  ctx.begin_block();
  ctx.begin_loop();
  ctx.loop_exit_if_ge(bi, b_count);

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

  let found = emit_set_find_structural(ctx, dst, be);
  ctx.emit(Instruction::LocalGet(found));
  ctx.emit(Instruction::I32Const(-1));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  emit_list_store_elem(ctx, dst, write_idx, be);
  ctx.i32_inc(write_idx);
  ctx.emit(Instruction::End);

  ctx.i32_inc(bi);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.ptr_to_f64(write_idx);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  let result = ctx.alloc_local();
  ctx.ptr_to_f64(dst);
  ctx.emit(Instruction::LocalSet(result));
  Ok(result)
}

/// Inline structural set-membership check.
/// Returns i32 local: index of found element (-1 = not found).
/// Uses structural equality (emit_equals_core) so heap objects like lists compare correctly.
fn emit_set_find_structural(ctx: &mut WasmGenCtx, set_ptr: u32, needle: u32) -> u32 {
  let set_count = emit_load_count_i32(ctx, set_ptr);
  let result = ctx.alloc_i32(-1); // -1 = not found
  let si = ctx.alloc_i32(0);

  ctx.begin_block();
  ctx.begin_loop();
  ctx.loop_exit_if_ge(si, set_count);

  // Load element at set_ptr + 8 + si*8
  let elem_addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(set_ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(si));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(elem_addr));

  let elem = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(elem_addr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(elem));

  // Structural equality: needle == elem? (shallow: no recursive set comparison)
  let eq = emit_equals_core_shallow(ctx, needle, elem).expect("emit_equals_core_shallow in set_find_structural");

  ctx.emit(Instruction::LocalGet(eq));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Ne);
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(si));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::Br(2)); // break out of loop+block
  ctx.emit(Instruction::End);

  ctx.i32_inc(si);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End); // loop
  ctx.emit(Instruction::End); // block

  result
}

/// `&difference a b` from pre-loaded i32 pointers. Returns f64 local with result pointer.
pub(super) fn emit_set_difference_from_ptrs(ctx: &mut WasmGenCtx, a: u32, b: u32) -> Result<u32, String> {
  let a_count = emit_load_count_i32(ctx, a);

  let total_slots = ctx.i32_offset(a_count, 1);
  let dst = emit_alloc_with_count(ctx, a_count, total_slots, "set");

  let write_idx = ctx.alloc_i32(0);

  let ai = ctx.alloc_i32(0);

  ctx.begin_block();
  ctx.begin_loop();
  ctx.loop_exit_if_ge(ai, a_count);

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

  let found_idx = emit_set_find_structural(ctx, b, elem);

  ctx.emit(Instruction::LocalGet(found_idx));
  ctx.emit(Instruction::I32Const(-1));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  emit_list_store_elem(ctx, dst, write_idx, elem);
  ctx.i32_inc(write_idx);
  ctx.emit(Instruction::End);

  ctx.i32_inc(ai);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.ptr_to_f64(write_idx);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  let result = ctx.alloc_local();
  ctx.ptr_to_f64(dst);
  ctx.emit(Instruction::LocalSet(result));
  Ok(result)
}

/// `&include set elem` from pre-loaded i32 set pointer + f64 elem local. Returns f64 result local.
pub(super) fn emit_set_include_from_ptrs(ctx: &mut WasmGenCtx, src: u32, elem: u32) -> Result<u32, String> {
  let count = emit_load_count_i32(ctx, src);
  let found_idx = emit_set_find_structural(ctx, src, elem);

  let dst = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::LocalSet(dst));

  ctx.emit(Instruction::LocalGet(found_idx));
  ctx.emit(Instruction::I32Const(-1));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  {
    let nc = ctx.i32_offset(count, 1);
    let ts = ctx.i32_offset(nc, 1);
    let d = emit_alloc_with_count(ctx, nc, ts, "set");
    ctx.emit(Instruction::LocalGet(d));
    ctx.emit(Instruction::LocalSet(dst));

    let db = emit_addr_offset(ctx, d, 8);
    let sb = emit_addr_offset(ctx, src, 8);
    emit_copy_f64_loop(ctx, db, sb, count);

    emit_list_store_elem(ctx, d, count, elem);
  }
  ctx.emit(Instruction::End);

  let result = ctx.alloc_local();
  ctx.ptr_to_f64(dst);
  ctx.emit(Instruction::LocalSet(result));
  Ok(result)
}

/// `&set:includes? set value` — linear scan.
pub(super) fn emit_set_includes(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&set:includes? expects 2 args")?;
  let ptr = emit_ptr_to_i32(ctx, &args[0])?;
  let target = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(target));
  let found_idx = emit_set_find_structural(ctx, ptr, target);
  ctx.emit(f64_const(1.0));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalGet(found_idx));
  ctx.emit(Instruction::I32Const(-1));
  ctx.emit(Instruction::I32Ne);
  ctx.emit(Instruction::Select);
  Ok(())
}

/// `&set:to-list set` — copy set payload into a list.
pub(super) fn emit_set_to_list(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&set:to-list expects 1 arg")?;
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);
  let dst = emit_alloc_list(ctx, count);
  let dst_base = emit_addr_offset(ctx, dst, 8);
  let src_base = emit_addr_offset(ctx, src, 8);
  emit_copy_f64_loop(ctx, dst_base, src_base, count);

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// `&include set value` — new set with element added (if not present).
pub(super) fn emit_set_include(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&include expects 2 args")?;
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);
  let elem = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(elem));

  let found_idx = emit_set_find_structural(ctx, src, elem);

  let dst = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::LocalSet(dst)); // default: return same

  ctx.emit(Instruction::LocalGet(found_idx));
  ctx.emit(Instruction::I32Const(-1));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  {
    // Not found: append like list
    let nc = ctx.i32_offset(count, 1);
    let ts = ctx.i32_offset(nc, 1);
    let d = emit_alloc_with_count(ctx, nc, ts, "set");
    ctx.emit(Instruction::LocalGet(d));
    ctx.emit(Instruction::LocalSet(dst));

    let db = emit_addr_offset(ctx, d, 8);
    let sb = emit_addr_offset(ctx, src, 8);
    emit_copy_f64_loop(ctx, db, sb, count);

    // Append element
    emit_list_store_elem(ctx, d, count, elem);
  }
  ctx.emit(Instruction::End);

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// `&exclude set value` — new set without the element.
pub(super) fn emit_set_exclude(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&exclude expects 2 args")?;
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);
  let target = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(target));

  let found_idx = emit_set_find_structural(ctx, src, target);

  // If not found, return original
  let dst = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(src));
  ctx.emit(Instruction::LocalSet(dst));

  ctx.emit(Instruction::LocalGet(found_idx));
  ctx.emit(Instruction::I32Const(-1));
  ctx.emit(Instruction::I32Ne);
  ctx.begin_block_if();
  {
    let nc = ctx.i32_offset(count, -1);
    let ts = ctx.i32_offset(nc, 1);
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

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// `&difference a b` — set of elements in `a` not in `b`.
pub(super) fn emit_set_difference(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&difference expects 2 args")?;
  check_set_comparison_types(ctx, &args[0], &args[1])?;
  let a = emit_ptr_to_i32(ctx, &args[0])?;
  let a_count = emit_load_count_i32(ctx, a);
  let b = emit_ptr_to_i32(ctx, &args[1])?;

  // Over-allocate: max possible result is a_count elements
  let total_slots = ctx.i32_offset(a_count, 1);
  let dst = emit_alloc_with_count(ctx, a_count, total_slots, "set");

  let write_idx = ctx.alloc_i32(0);

  let ai = ctx.alloc_i32(0);

  ctx.begin_block();
  ctx.begin_loop();
  ctx.loop_exit_if_ge(ai, a_count);

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

  let found_idx = emit_set_find_structural(ctx, b, elem);

  // If not found in b, copy elem to result
  ctx.emit(Instruction::LocalGet(found_idx));
  ctx.emit(Instruction::I32Const(-1));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  emit_list_store_elem(ctx, dst, write_idx, elem);
  ctx.i32_inc(write_idx);
  ctx.emit(Instruction::End);

  // ai++
  ctx.i32_inc(ai);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // Patch actual count
  ctx.emit(Instruction::LocalGet(dst));
  ctx.ptr_to_f64(write_idx);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// `&union a b` — set of all elements in `a` and `b` (no duplicates).
pub(super) fn emit_set_union(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&union expects 2 args")?;
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

  let total_slots = ctx.i32_offset(max_count, 1);
  let dst = emit_alloc_with_count(ctx, max_count, total_slots, "set");

  // Copy all of a into result
  let db = emit_addr_offset(ctx, dst, 8);
  let sb = emit_addr_offset(ctx, a, 8);
  emit_copy_f64_loop(ctx, db, sb, a_count);

  let write_idx = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(a_count));
  ctx.emit(Instruction::LocalSet(write_idx));

  let bi = ctx.alloc_i32(0);

  ctx.begin_block();
  ctx.begin_loop();
  ctx.loop_exit_if_ge(bi, b_count);

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
  let found = ctx.alloc_i32(0);
  let di = ctx.alloc_i32(0);

  ctx.begin_block();
  ctx.begin_loop();
  ctx.loop_exit_if_ge(di, write_idx);
  ctx.emit(Instruction::LocalGet(dst));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(di));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(be));
  emit_f64_value_eq(ctx);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(found));
  ctx.emit(Instruction::Br(2));
  ctx.emit(Instruction::End);
  ctx.i32_inc(di);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(found));
  ctx.emit(Instruction::I32Eqz);
  ctx.begin_block_if();
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
    ctx.i32_inc(write_idx);
  }
  ctx.emit(Instruction::End);

  // bi++
  ctx.i32_inc(bi);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // Patch actual count
  ctx.emit(Instruction::LocalGet(dst));
  ctx.ptr_to_f64(write_idx);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  ctx.ptr_to_f64(dst);
  Ok(())
}

/// `&set:intersection a b` — set of elements common to both a and b.
pub(super) fn emit_set_intersection(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&set:intersection expects 2 args")?;
  check_set_comparison_types(ctx, &args[0], &args[1])?;
  let a = emit_ptr_to_i32(ctx, &args[0])?;
  let b = emit_ptr_to_i32(ctx, &args[1])?;
  let result = emit_set_intersection_from_ptrs(ctx, a, b);
  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

fn emit_set_intersection_from_ptrs(ctx: &mut WasmGenCtx, a: u32, b: u32) -> u32 {
  let a_count = emit_load_count_i32(ctx, a);

  // Max result size is a_count
  let total_slots = ctx.i32_offset(a_count, 1);
  let dst = emit_alloc_with_count(ctx, a_count, total_slots, "set");

  let write_idx = ctx.alloc_i32(0);

  let ai = ctx.alloc_i32(0);

  ctx.begin_block();
  ctx.begin_loop();
  ctx.loop_exit_if_ge(ai, a_count);

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

  let found_idx = emit_set_find_structural(ctx, b, elem);

  // If found in b, copy elem to result
  ctx.emit(Instruction::LocalGet(found_idx));
  ctx.emit(Instruction::I32Const(-1));
  ctx.emit(Instruction::I32Ne);
  ctx.begin_block_if();
  emit_list_store_elem(ctx, dst, write_idx, elem);
  ctx.i32_inc(write_idx);
  ctx.emit(Instruction::End);

  ctx.i32_inc(ai);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(dst));
  ctx.ptr_to_f64(write_idx);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

  let result = ctx.alloc_local();
  ctx.ptr_to_f64(dst);
  ctx.emit(Instruction::LocalSet(result));
  result
}

/// `&set:destruct set` returns SetDestruct with separate member and remainder payloads.
pub(super) fn emit_set_destruct(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(1, args, "&set:destruct expects 1 arg")?;
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);

  let result = ctx.alloc_local();

  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Eqz);
  ctx.begin_block_if();
  // Empty set -> SetDestruct :none.
  emit_core_enum(ctx, "none", &[])?;
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::Else);
  {
    // First element is at src + 8
    let elem0 = ctx.alloc_local();
    ctx.emit(Instruction::LocalGet(src));
    ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
    ctx.emit(Instruction::LocalSet(elem0));

    // rest-set = exclude(src, elem0)
    let found_idx = emit_set_find_structural(ctx, src, elem0);
    let nc = ctx.i32_offset(count, -1);
    let ts = ctx.i32_offset(nc, 1);
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
    ctx.ptr_to_f64(rest_set_ptr);
    ctx.emit(Instruction::LocalSet(rest_set_f64));

    emit_core_enum(ctx, "some", &[elem0, rest_set_f64])?;
    ctx.emit(Instruction::LocalSet(result));
  }
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}
