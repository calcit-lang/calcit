use super::*;

// ===========================================================================
// Set operations — layout: [count:f64] [elem0:f64] [elem1:f64] ...
// ===========================================================================

/// `#{} elem0 elem1 ...` — create a set, deduplicating elements in insertion order.
pub(super) fn emit_set_new(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  let elem_types = args
    .iter()
    .map(|arg| {
      infer_wasm_static_type(ctx, arg)
        .map(|ty| expand_wasm_value_type(&ty))
        .unwrap_or_else(|| DYNAMIC_TYPE.clone())
    })
    .collect::<Vec<_>>();
  for (i, ty_i) in elem_types.iter().enumerate() {
    if set_comparison_can_erase_kind(ty_i, ty_i, 0) {
      return Err(
        "E_WASM_SET_ELEMENT_KIND: set construction needs element types that rule out distinct scalar kinds sharing a WASM value".into(),
      );
    }
    for ty_j in &elem_types[..i] {
      if set_comparison_can_erase_kind(ty_i, ty_j, 0) && !are_disjoint_number_and_heap_types(ty_i, ty_j) {
        return Err(
          "E_WASM_SET_ELEMENT_KIND: set construction needs element types that rule out distinct scalar kinds sharing a WASM value"
            .into(),
        );
      }
    }
  }

  let count = args.len();
  let total_bytes = ((1 + count) * 8) as i32;
  let ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, total_bytes, ptr, "set");

  if count <= 1 {
    ctx.emit(Instruction::LocalGet(ptr));
    ctx.emit(f64_const(count as f64));
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

    for (i, arg) in args.iter().enumerate() {
      ctx.emit(Instruction::LocalGet(ptr));
      emit_expr(ctx, arg)?;
      ctx.emit(Instruction::F64Store(mem_arg_f64(((1 + i) * 8) as u64)));
    }
  } else {
    ctx.emit(Instruction::LocalGet(ptr));
    ctx.emit(f64_const(0.0));
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));

    let write_idx = ctx.alloc_i32(0);
    let mut elem_locals = Vec::with_capacity(count);
    for (i, arg) in args.iter().enumerate() {
      let elem = ctx.alloc_local();
      emit_expr(ctx, arg)?;
      ctx.emit(Instruction::LocalSet(elem));
      elem_locals.push(elem);
      let compatible_prev = (0..i)
        .filter(|&j| !are_disjoint_number_and_heap_types(&elem_types[i], &elem_types[j]))
        .collect::<Vec<_>>();
      if compatible_prev.is_empty() {
        emit_list_store_elem(ctx, ptr, write_idx, elem);
        ctx.i32_inc(write_idx);
        ctx.emit(Instruction::LocalGet(ptr));
        ctx.ptr_to_f64(write_idx);
        ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
      } else if compatible_prev.len() == i {
        let found_idx = emit_set_find_structural(ctx, ptr, elem);
        ctx.emit(Instruction::LocalGet(found_idx));
        ctx.emit(Instruction::I32Const(-1));
        ctx.emit(Instruction::I32Eq);
        ctx.begin_block_if();
        emit_list_store_elem(ctx, ptr, write_idx, elem);
        ctx.i32_inc(write_idx);
        ctx.emit(Instruction::LocalGet(ptr));
        ctx.ptr_to_f64(write_idx);
        ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
        ctx.emit(Instruction::End);
      } else {
        let is_dup = ctx.alloc_i32(0);
        for j in compatible_prev {
          ctx.emit(Instruction::LocalGet(elem));
          ctx.emit(Instruction::LocalGet(elem_locals[j]));
          ctx.call_rt("__rt_value_equal");
          ctx.emit(Instruction::LocalGet(is_dup));
          ctx.emit(Instruction::I32Or);
          ctx.emit(Instruction::LocalSet(is_dup));
        }
        ctx.emit(Instruction::LocalGet(is_dup));
        ctx.emit(Instruction::I32Eqz);
        ctx.begin_block_if();
        emit_list_store_elem(ctx, ptr, write_idx, elem);
        ctx.i32_inc(write_idx);
        ctx.emit(Instruction::LocalGet(ptr));
        ctx.ptr_to_f64(write_idx);
        ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
        ctx.emit(Instruction::End);
      }
    }
  }

  ctx.ptr_to_f64(ptr);
  Ok(())
}

/// Tag for which native set binary operation to apply in `emit_set_op_variadic`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SetOpKind {
  Union,
  Difference,
  Intersection,
  Include,
  Exclude,
}

pub(super) fn is_proven_non_scalar_heap_type(annotation: &Arc<CalcitTypeAnnotation>) -> bool {
  let expanded = expand_wasm_value_type(annotation);
  match expanded.as_ref() {
    CalcitTypeAnnotation::String
    | CalcitTypeAnnotation::Buffer
    | CalcitTypeAnnotation::List(_)
    | CalcitTypeAnnotation::Tuple(_)
    | CalcitTypeAnnotation::Set(_)
    | CalcitTypeAnnotation::Map(_, _)
    | CalcitTypeAnnotation::Struct(_, _)
    | CalcitTypeAnnotation::StructValue(_)
    | CalcitTypeAnnotation::Enum(_, _)
    | CalcitTypeAnnotation::EnumValue(_) => true,
    CalcitTypeAnnotation::TypeRef(_, _) => expanded.resolve_to_struct().is_some() || expanded.resolve_to_enum().is_some(),
    _ => false,
  }
}

pub(super) fn are_disjoint_number_and_heap_types(left: &Arc<CalcitTypeAnnotation>, right: &Arc<CalcitTypeAnnotation>) -> bool {
  let left = expand_wasm_value_type(left);
  let right = expand_wasm_value_type(right);
  if set_comparison_can_erase_kind(&left, &left, 0) || set_comparison_can_erase_kind(&right, &right, 0) {
    return false;
  }
  (proven_scalar_kind(&left) == Some("number") && is_proven_non_scalar_heap_type(&right))
    || (is_proven_non_scalar_heap_type(&left) && proven_scalar_kind(&right) == Some("number"))
}

pub(super) fn infer_set_member_types(ctx: &WasmGenCtx, set: &Calcit) -> Vec<Arc<CalcitTypeAnnotation>> {
  if let Some(evidence) = infer_wasm_value_evidence(ctx, set) {
    if let Some(members) = evidence.set_member_types
      && !members.is_empty()
    {
      return members;
    }
    let annotation = expand_wasm_value_type(&evidence.annotation);
    match annotation.as_ref() {
      CalcitTypeAnnotation::Set(element) => return vec![element.clone()],
      CalcitTypeAnnotation::Never => return vec![annotation],
      _ => {}
    }
  }
  vec![DYNAMIC_TYPE.clone()]
}

fn check_set_comparison_types(ctx: &WasmGenCtx, left: &Calcit, right: &Calcit) -> Result<(), String> {
  let left_members = infer_set_member_types(ctx, left);
  let right_members = infer_set_member_types(ctx, right);
  if left_members.iter().any(|m| set_comparison_can_erase_kind(m, m, 0))
    || right_members.iter().any(|m| set_comparison_can_erase_kind(m, m, 0))
    || left_members
      .iter()
      .any(|l| right_members.iter().any(|r| set_comparison_can_erase_kind(l, r, 0)))
  {
    return Err(
      "E_WASM_SET_ELEMENT_KIND: set comparison needs element types that rule out distinct scalar kinds sharing a WASM value".into(),
    );
  }
  Ok(())
}

pub(super) fn check_set_union_types(ctx: &WasmGenCtx, left: &Calcit, right: &Calcit) -> Result<bool, String> {
  let left_members = infer_set_member_types(ctx, left);
  let right_members = infer_set_member_types(ctx, right);
  if left_members.iter().any(|m| set_comparison_can_erase_kind(m, m, 0))
    || right_members.iter().any(|m| set_comparison_can_erase_kind(m, m, 0))
  {
    return Err(
      "E_WASM_SET_ELEMENT_KIND: set comparison needs element types that rule out distinct scalar kinds sharing a WASM value".into(),
    );
  }
  let mut all_disjoint = !left_members.is_empty() && !right_members.is_empty();
  for l in &left_members {
    for r in &right_members {
      if matches!(l.as_ref(), CalcitTypeAnnotation::Never) || matches!(r.as_ref(), CalcitTypeAnnotation::Never) {
        all_disjoint = false;
        continue;
      }
      if are_disjoint_number_and_heap_types(l, r) {
        continue;
      }
      all_disjoint = false;
      if set_comparison_can_erase_kind(l, r, 0) {
        return Err(
          "E_WASM_SET_ELEMENT_KIND: set comparison needs element types that rule out distinct scalar kinds sharing a WASM value".into(),
        );
      }
    }
  }
  Ok(all_disjoint)
}

fn check_set_include_types(ctx: &WasmGenCtx, set: &Calcit, elem: &Calcit) -> Result<bool, String> {
  let set_members = infer_set_member_types(ctx, set);
  let elem_type = infer_wasm_static_type(ctx, elem)
    .map(|ty| expand_wasm_value_type(&ty))
    .unwrap_or_else(|| DYNAMIC_TYPE.clone());
  if set_comparison_can_erase_kind(&elem_type, &elem_type, 0) || set_members.iter().any(|m| set_comparison_can_erase_kind(m, m, 0)) {
    return Err(
      "E_WASM_SET_ELEMENT_KIND: set comparison needs element types that rule out distinct scalar kinds sharing a WASM value".into(),
    );
  }
  let mut all_disjoint = !set_members.is_empty();
  for m in &set_members {
    if matches!(m.as_ref(), CalcitTypeAnnotation::Never) || matches!(elem_type.as_ref(), CalcitTypeAnnotation::Never) {
      all_disjoint = false;
      continue;
    }
    if are_disjoint_number_and_heap_types(m, &elem_type) {
      continue;
    }
    all_disjoint = false;
    if set_comparison_can_erase_kind(m, &elem_type, 0) {
      return Err(
        "E_WASM_SET_ELEMENT_KIND: set comparison needs element types that rule out distinct scalar kinds sharing a WASM value".into(),
      );
    }
  }
  Ok(all_disjoint)
}

fn check_set_membership_types(ctx: &WasmGenCtx, set: &Calcit, target: &Calcit) -> Result<(), String> {
  let set_members = infer_set_member_types(ctx, set);
  let target_type = infer_wasm_static_type(ctx, target)
    .map(|ty| expand_wasm_value_type(&ty))
    .unwrap_or_else(|| DYNAMIC_TYPE.clone());
  if set_comparison_can_erase_kind(&target_type, &target_type, 0)
    || set_members
      .iter()
      .any(|m| set_comparison_can_erase_kind(m, m, 0) || set_comparison_can_erase_kind(m, &target_type, 0))
  {
    return Err(
      "E_WASM_SET_ELEMENT_KIND: set comparison needs element types that rule out distinct scalar kinds sharing a WASM value".into(),
    );
  }
  Ok(())
}

/// The structural comparator uses untagged f64 values, including when it
/// descends into containers. Compare corresponding slots so a heterogeneous
/// tuple can still be compared with another tuple of the same shape.
pub(super) fn set_comparison_can_erase_kind(left: &Arc<CalcitTypeAnnotation>, right: &Arc<CalcitTypeAnnotation>, depth: usize) -> bool {
  let mut visited = HashSet::new();
  set_comparison_can_erase_kind_inner(left, right, depth, &mut visited)
}

pub(super) fn nominal_cycle_key(annotation: &CalcitTypeAnnotation, definition_ref: Option<&str>, fallback_name: &str) -> String {
  use CalcitTypeAnnotation as T;

  let base = definition_ref.unwrap_or(fallback_name);
  let args: &[Arc<CalcitTypeAnnotation>] = match annotation {
    T::Struct(_, args) | T::Enum(_, args) | T::TypeRef(_, args) => args.as_slice(),
    _ => &[],
  };
  if args.is_empty() {
    base.to_owned()
  } else {
    let rendered = args.iter().map(|arg| arg.to_brief_string()).collect::<Vec<_>>().join(",");
    format!("{base}<{rendered}>")
  }
}

fn set_comparison_can_erase_kind_inner(
  left: &Arc<CalcitTypeAnnotation>,
  right: &Arc<CalcitTypeAnnotation>,
  depth: usize,
  visited: &mut HashSet<(String, String)>,
) -> bool {
  use CalcitTypeAnnotation as T;

  let left = expand_wasm_value_type(left);
  let right = expand_wasm_value_type(right);
  if matches!(left.as_ref(), T::Never) || matches!(right.as_ref(), T::Never) {
    return false;
  }
  // Unresolved or non-cyclic overly deep shapes cannot serve as evidence.
  if depth >= 32 {
    return true;
  }
  let scalar_kind = proven_scalar_kind;
  if let (Some(left), Some(right)) = (scalar_kind(&left), scalar_kind(&right)) {
    // Tags start at 1, while Nil and Unit use 0.
    return left != right && !matches!((left, right), ("tag", "nil" | "unit") | ("nil" | "unit", "tag"));
  }
  match (left.as_ref(), right.as_ref()) {
    (T::Nullable(inner, NullableOrigin::Inferred), _) => {
      set_comparison_can_erase_kind_inner(inner, &right, depth + 1, visited)
        || set_comparison_can_erase_kind_inner(&Arc::new(T::Nil), &right, depth + 1, visited)
    }
    (_, T::Nullable(inner, NullableOrigin::Inferred)) => {
      set_comparison_can_erase_kind_inner(&left, inner, depth + 1, visited)
        || set_comparison_can_erase_kind_inner(&left, &Arc::new(T::Nil), depth + 1, visited)
    }
    (T::List(left), T::List(right)) | (T::Set(left), T::Set(right)) => {
      set_comparison_can_erase_kind_inner(left, right, depth + 1, visited)
    }
    (T::Tuple(left), T::Tuple(right)) => {
      left.len() == right.len()
        && left
          .iter()
          .zip(right.iter())
          .any(|(left, right)| set_comparison_can_erase_kind_inner(left, right, depth + 1, visited))
    }
    (T::List(member), T::Tuple(slots)) => slots
      .iter()
      .any(|slot| set_comparison_can_erase_kind_inner(member, slot, depth + 1, visited)),
    (T::Tuple(slots), T::List(member)) => slots
      .iter()
      .any(|slot| set_comparison_can_erase_kind_inner(slot, member, depth + 1, visited)),
    (T::Map(left_key, left_value), T::Map(right_key, right_value)) => {
      let norm_key = |k: &Arc<T>| {
        if matches!(k.as_ref(), T::StructValue(_)) {
          Arc::new(T::Tag)
        } else {
          k.clone()
        }
      };
      set_comparison_can_erase_kind_inner(&norm_key(left_key), &norm_key(right_key), depth + 1, visited)
        || set_comparison_can_erase_kind_inner(left_value, right_value, depth + 1, visited)
    }
    (T::Struct(..) | T::StructValue(_) | T::TypeRef(..), T::Struct(..) | T::StructValue(_) | T::TypeRef(..))
      if let (Some(left_def), Some(right_def)) = (left.resolve_to_struct(), right.resolve_to_struct()) =>
    {
      if left_def.fields != right_def.fields {
        return false;
      }
      let key = (
        nominal_cycle_key(&left, left_def.definition_ref.as_deref(), left_def.name.ref_str()),
        nominal_cycle_key(&right, right_def.definition_ref.as_deref(), right_def.name.ref_str()),
      );
      if !visited.insert(key.clone()) {
        // Coinductive cycle hit: already checking this nominal pair on the active stack.
        return false;
      }
      let can_erase = left_def
        .field_types
        .iter()
        .zip(right_def.field_types.iter())
        .any(|(left_field, right_field)| {
          set_comparison_can_erase_kind_inner(
            &set_comparison_payload(&left, &left_def.generics, left_field, left_def.definition_ref.as_deref()),
            &set_comparison_payload(&right, &right_def.generics, right_field, right_def.definition_ref.as_deref()),
            depth + 1,
            visited,
          )
        });
      visited.remove(&key);
      can_erase
    }
    (T::Enum(..) | T::EnumValue(_) | T::TypeRef(..), T::Enum(..) | T::EnumValue(_) | T::TypeRef(..))
      if let (Some(left_def), Some(right_def)) = (left.resolve_to_enum(), right.resolve_to_enum()) =>
    {
      let key = (
        nominal_cycle_key(&left, left_def.definition_ref().map(AsRef::as_ref), left_def.name().ref_str()),
        nominal_cycle_key(&right, right_def.definition_ref().map(AsRef::as_ref), right_def.name().ref_str()),
      );
      if !visited.insert(key.clone()) {
        // Coinductive cycle hit: an Enum is always a heap object, so recursive self-references
        // do not introduce scalar-kind collisions unless a non-cyclic variant payload does.
        return false;
      }
      let can_erase = left_def.variants().iter().any(|left_variant| {
        right_def
          .find_variant_by_name(left_variant.tag.ref_str())
          .is_some_and(|right_variant| {
            left_variant.arity() == right_variant.arity()
              && left_variant
                .payload_types()
                .iter()
                .zip(right_variant.payload_types())
                .any(|(left_payload, right_payload)| {
                  set_comparison_can_erase_kind_inner(
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
                    depth + 1,
                    visited,
                  )
                })
          })
      });
      visited.remove(&key);
      can_erase
    }
    (T::String, T::String) | (T::Buffer, T::Buffer) => false,
    // Different known heap layouts do not share an address or recurse.
    (
      T::String | T::Buffer | T::List(_) | T::Tuple(_) | T::Set(_) | T::Map(_, _),
      T::String | T::Buffer | T::List(_) | T::Tuple(_) | T::Set(_) | T::Map(_, _),
    ) => false,
    // Number may equal a heap pointer; the other scalar/heap comparisons
    // have the same representation constraints as elsewhere in codegen.
    (T::String | T::Buffer | T::List(_) | T::Tuple(_) | T::Set(_) | T::Map(_, _), T::Bool | T::Nil | T::Unit | T::Tag)
    | (T::Bool | T::Nil | T::Unit | T::Tag, T::String | T::Buffer | T::List(_) | T::Tuple(_) | T::Set(_) | T::Map(_, _)) => false,
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
      SetOpKind::Exclude => emit_set_exclude(ctx, args),
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
      SetOpKind::Exclude => emit_set_exclude(ctx, &first_two)?,
    }
    ctx.emit(Instruction::LocalSet(acc));
  }
  for (extra_idx, extra) in args[2..].iter().enumerate() {
    match kind {
      SetOpKind::Difference | SetOpKind::Intersection => {
        for prev in &args[..2 + extra_idx] {
          check_set_comparison_types(ctx, prev, extra)?;
        }
      }
      SetOpKind::Union => {
        for prev in &args[..2 + extra_idx] {
          check_set_union_types(ctx, prev, extra)?;
        }
      }
      SetOpKind::Include => {
        check_set_include_types(ctx, &args[0], extra)?;
        for prev_elem in &args[1..2 + extra_idx] {
          let prev_ty = infer_wasm_static_type(ctx, prev_elem)
            .map(|ty| expand_wasm_value_type(&ty))
            .unwrap_or_else(|| DYNAMIC_TYPE.clone());
          let extra_ty = infer_wasm_static_type(ctx, extra)
            .map(|ty| expand_wasm_value_type(&ty))
            .unwrap_or_else(|| DYNAMIC_TYPE.clone());
          if set_comparison_can_erase_kind(&prev_ty, &extra_ty, 0) && !are_disjoint_number_and_heap_types(&prev_ty, &extra_ty) {
            return Err(
              "E_WASM_SET_ELEMENT_KIND: set comparison needs element types that rule out distinct scalar kinds sharing a WASM value"
                .into(),
            );
          }
        }
      }
      SetOpKind::Exclude => {
        let set_type = infer_wasm_static_type(ctx, &args[0]).map(|ty| expand_wasm_value_type(&ty));
        let target_type = infer_wasm_static_type(ctx, extra).map(|ty| expand_wasm_value_type(&ty));
        let disjoint_scalar = if let (Some(CalcitTypeAnnotation::Set(item_type)), Some(target_type)) =
          (set_type.as_deref(), target_type.as_deref())
          && let (Some(item_kind), Some(target_kind)) = (
            proven_scalar_kind(&expand_wasm_value_type(item_type)),
            proven_scalar_kind(target_type),
          ) {
          item_kind != target_kind
        } else {
          false
        };
        if !disjoint_scalar {
          check_set_membership_types(ctx, &args[0], extra)?;
        }
      }
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
      SetOpKind::Exclude => emit_set_exclude_from_ptrs(ctx, acc_ptr, extra_val)?,
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
  // Only the copied prefix from `a` is initialized before scanning `dst` for duplicates from `b`.
  let dst = emit_alloc_with_count(ctx, a_count, total_slots, "set");

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
  ctx.emit(Instruction::LocalGet(dst));
  ctx.ptr_to_f64(write_idx);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
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

/// Structural set-membership check via `__rt_set_find_elem` (which uses recursive `__rt_value_equal`).
/// Returns i32 local: index of found element (-1 = not found).
fn emit_set_find_structural(ctx: &mut WasmGenCtx, set_ptr: u32, needle: u32) -> u32 {
  let result = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(set_ptr));
  ctx.emit(Instruction::LocalGet(needle));
  ctx.call_rt("__rt_set_find_elem");
  ctx.emit(Instruction::LocalSet(result));
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
  let set_members = infer_set_member_types(ctx, &args[0]);
  let target_ev = infer_wasm_evidence(ctx, &args[1]);
  let target_type = expand_wasm_value_type(target_ev.scalar_origin.as_ref().unwrap_or(&target_ev.annotation));
  if !set_members.is_empty()
    && let Some(target_kind) = proven_disjoint_value_kind(&target_type)
    && set_members
      .iter()
      .all(|item_type| proven_disjoint_value_kind(&expand_wasm_value_type(item_type)).is_some_and(|k| k != target_kind))
  {
    emit_expr(ctx, &args[0])?;
    ctx.emit(Instruction::Drop);
    emit_expr(ctx, &args[1])?;
    ctx.emit(Instruction::Drop);
    ctx.emit(f64_const(0.0));
    return Ok(());
  }
  check_set_membership_types(ctx, &args[0], &args[1])?;
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
  let disjoint_append = check_set_include_types(ctx, &args[0], &args[1])?;
  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let count = emit_load_count_i32(ctx, src);
  let elem = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(elem));

  if disjoint_append {
    let nc = ctx.i32_offset(count, 1);
    let ts = ctx.i32_offset(nc, 1);
    let d = emit_alloc_with_count(ctx, nc, ts, "set");
    let db = emit_addr_offset(ctx, d, 8);
    let sb = emit_addr_offset(ctx, src, 8);
    emit_copy_f64_loop(ctx, db, sb, count);
    emit_list_store_elem(ctx, d, count, elem);
    ctx.ptr_to_f64(d);
    return Ok(());
  }

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
  // A statically different scalar kind cannot be present. Avoid comparing
  // their shared f64 representation, but still evaluate both arguments.
  let set_members = infer_set_member_types(ctx, &args[0]);
  let target_ev = infer_wasm_evidence(ctx, &args[1]);
  let target_type = expand_wasm_value_type(target_ev.scalar_origin.as_ref().unwrap_or(&target_ev.annotation));
  if !set_members.is_empty()
    && let Some(target_kind) = proven_disjoint_value_kind(&target_type)
    && set_members
      .iter()
      .all(|item_type| proven_disjoint_value_kind(&expand_wasm_value_type(item_type)).is_some_and(|k| k != target_kind))
  {
    emit_expr(ctx, &args[0])?;
    emit_expr(ctx, &args[1])?;
    ctx.emit(Instruction::Drop);
    return Ok(());
  }
  check_set_membership_types(ctx, &args[0], &args[1])?;

  let src = emit_ptr_to_i32(ctx, &args[0])?;
  let target = ctx.alloc_local();
  emit_expr(ctx, &args[1])?;
  ctx.emit(Instruction::LocalSet(target));
  let result = emit_set_exclude_from_ptrs(ctx, src, target)?;
  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

pub(super) fn emit_set_exclude_from_ptrs(ctx: &mut WasmGenCtx, src: u32, target: u32) -> Result<u32, String> {
  let count = emit_load_count_i32(ctx, src);
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

  let result = ctx.alloc_local();
  ctx.ptr_to_f64(dst);
  ctx.emit(Instruction::LocalSet(result));
  Ok(result)
}

/// `&difference a b` — set of elements in `a` not in `b`.
pub(super) fn emit_set_difference(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&difference expects 2 args")?;
  check_set_comparison_types(ctx, &args[0], &args[1])?;
  let a = emit_ptr_to_i32(ctx, &args[0])?;
  let b = emit_ptr_to_i32(ctx, &args[1])?;
  let result = emit_set_difference_from_ptrs(ctx, a, b)?;
  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

/// `&union a b` — set of all elements in `a` and `b` (no duplicates).
pub(super) fn emit_set_union(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  expect_arity(2, args, "&union expects 2 args")?;
  let disjoint_concat = check_set_union_types(ctx, &args[0], &args[1])?;
  let a = emit_ptr_to_i32(ctx, &args[0])?;
  let b = emit_ptr_to_i32(ctx, &args[1])?;
  if disjoint_concat {
    let a_count = emit_load_count_i32(ctx, a);
    let b_count = emit_load_count_i32(ctx, b);
    let total_count = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(a_count));
    ctx.emit(Instruction::LocalGet(b_count));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(total_count));
    let total_slots = ctx.i32_offset(total_count, 1);
    let dst = emit_alloc_with_count(ctx, total_count, total_slots, "set");
    let db1 = emit_addr_offset(ctx, dst, 8);
    let sb1 = emit_addr_offset(ctx, a, 8);
    emit_copy_f64_loop(ctx, db1, sb1, a_count);
    let db2 = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(dst));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(a_count));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Mul);
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(db2));
    let sb2 = emit_addr_offset(ctx, b, 8);
    emit_copy_f64_loop(ctx, db2, sb2, b_count);
    ctx.ptr_to_f64(dst);
    return Ok(());
  }
  let result = emit_set_union_from_ptrs(ctx, a, b)?;
  ctx.emit(Instruction::LocalGet(result));
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
