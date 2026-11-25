use super::*;
use crate::wasm::host::runner::preprocess::infer_static_type_from_expr;
use cirru_edn::EdnTag;

const MAX_EDN_TYPE_DEPTH: usize = 32;
pub(super) const MAX_EDN_OUTPUT_BYTES: i32 = 64 * 1024;
type ResolvedEnumVariant = (EdnTag, Vec<Arc<CalcitTypeAnnotation>>);

pub(super) fn emit_format_cirru_edn(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if !(1..=2).contains(&args.len()) {
    return Err(format!("format-cirru-edn expects 1~2 args, got {}", args.len()));
  }

  if let Some(formatted) = try_format_cirru_edn_literal(&args[0]) {
    // The literal itself has no effects. Keep eager evaluation of the optional
    // compatibility flag before returning the preformatted native-parity text.
    if let Some(flag) = args.get(1) {
      emit_expr(ctx, flag)?;
      ctx.emit(Instruction::Drop);
    }
    emit_literal(ctx, &formatted)?;
    return Ok(());
  }

  let inferred_type = super::infer_wasm_codec_hint(ctx, &args[0])
    .or_else(|| super::infer_wasm_static_type(ctx, &args[0]))
    .or_else(|| infer_static_type_from_expr(&args[0]))
    .ok_or_else(|| "E_WASM_EDN_TYPE: format-cirru-edn requires a closed inferred input type for WASM".to_string())?;
  let value_type = match inferred_type.as_ref() {
    CalcitTypeAnnotation::TypeRef(_, args) => {
      if let Some(nominal) = inferred_type.resolve_to_struct() {
        Arc::new(CalcitTypeAnnotation::Struct(Arc::new(nominal), args.clone()))
      } else if let Some(nominal) = inferred_type.resolve_to_enum() {
        Arc::new(CalcitTypeAnnotation::Enum(Arc::new(nominal), args.clone()))
      } else {
        inferred_type
      }
    }
    CalcitTypeAnnotation::StructValue(nominal) => Arc::new(CalcitTypeAnnotation::Struct(nominal.clone(), Arc::new(vec![]))),
    CalcitTypeAnnotation::EnumValue(nominal) => Arc::new(CalcitTypeAnnotation::Enum(nominal.clone(), Arc::new(vec![]))),
    _ => inferred_type,
  };

  validate_edn_format_mode(value_type.as_ref(), args.get(1))?;

  emit_expr(ctx, &args[0])?;
  let value = ctx.alloc_local();
  ctx.emit(Instruction::LocalSet(value));

  // Scalar layout is independent of the flag; supported container flags are
  // literal true. Preserve eager evaluation without silently discarding a mode.
  if let Some(flag) = args.get(1) {
    emit_expr(ctx, flag)?;
    ctx.emit(Instruction::Drop);
  }

  let body = emit_edn_value(ctx, value, value_type.as_ref(), 0, false)?;
  let with_prefix = if is_edn_scalar(value_type.as_ref()) {
    concat_with_literal(ctx, "do ", body)?
  } else {
    body
  };
  let with_leading_line = concat_with_literal(ctx, "\n", with_prefix)?;
  let output = concat_with_literal_after(ctx, with_leading_line, "\n")?;
  ctx.emit(Instruction::LocalGet(output));
  Ok(())
}

fn validate_edn_format_mode(value_type: &CalcitTypeAnnotation, flag: Option<&Calcit>) -> Result<(), String> {
  if !is_edn_scalar(value_type) && flag.is_some_and(|flag| !matches!(flag, Calcit::Bool(true))) {
    return Err(
      "E_WASM_EDN_FORMAT_MODE: container formatting requires an omitted or literal true inline flag; non-inline and runtime modes are unsupported"
        .to_string(),
    );
  }

  Ok(())
}

pub(super) fn try_format_cirru_edn_literal(value: &Calcit) -> Option<String> {
  match value {
    Calcit::Number(number) => cirru_edn::format(&cirru_edn::Edn::Number(*number), true).ok(),
    _ => None,
  }
}

fn is_edn_scalar(value_type: &CalcitTypeAnnotation) -> bool {
  !matches!(
    value_type,
    CalcitTypeAnnotation::List(_)
      | CalcitTypeAnnotation::Map(_, _)
      | CalcitTypeAnnotation::Struct(_, _)
      | CalcitTypeAnnotation::Enum(_, _)
  )
}

fn emit_edn_value(
  ctx: &mut WasmGenCtx,
  value: u32,
  value_type: &CalcitTypeAnnotation,
  depth: usize,
  nested: bool,
) -> Result<u32, String> {
  if depth > MAX_EDN_TYPE_DEPTH {
    return Err(format!(
      "E_WASM_EDN_DEPTH: format-cirru-edn type nesting exceeds {MAX_EDN_TYPE_DEPTH}"
    ));
  }

  match value_type {
    CalcitTypeAnnotation::TypeRef(_, args) => {
      if let Some(nominal) = value_type.resolve_to_struct() {
        emit_edn_struct(ctx, value, &nominal, args, depth, nested)
      } else if let Some(nominal) = value_type.resolve_to_enum() {
        emit_edn_enum(ctx, value, &nominal, args, depth, nested)
      } else {
        Err(format!(
          "E_WASM_EDN_TYPE: {value_type} cannot be resolved to a closed nominal type for WASM"
        ))
      }
    }
    CalcitTypeAnnotation::StructValue(nominal) => emit_edn_struct(ctx, value, nominal, &Arc::new(vec![]), depth, nested),
    CalcitTypeAnnotation::EnumValue(nominal) => emit_edn_enum(ctx, value, nominal, &Arc::new(vec![]), depth, nested),
    CalcitTypeAnnotation::Nil => literal_local(ctx, "nil"),
    CalcitTypeAnnotation::Bool => {
      let result = ctx.alloc_local();
      super::emit_value_is_truthy_i32(ctx, value);
      ctx.emit(Instruction::I32Eqz);
      ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
      emit_literal(ctx, "false")?;
      ctx.emit(Instruction::Else);
      emit_literal(ctx, "true")?;
      ctx.emit(Instruction::End);
      ctx.emit(Instruction::LocalSet(result));
      Ok(result)
    }
    CalcitTypeAnnotation::String => {
      let result = ctx.alloc_local();
      ctx.emit(Instruction::LocalGet(value));
      ctx.call_rt("__rt_edn_string");
      ctx.emit(Instruction::LocalSet(result));
      Ok(result)
    }
    CalcitTypeAnnotation::Tag => emit_edn_tag(ctx, value),
    CalcitTypeAnnotation::Numeric(kind) if !matches!(kind, CalcitNumericRefinement::Float32 | CalcitNumericRefinement::Float64) => {
      let result = ctx.alloc_local();
      ctx.emit(Instruction::LocalGet(value));
      ctx.call_rt("__rt_f64_to_str");
      ctx.emit(Instruction::LocalSet(result));
      Ok(result)
    }
    CalcitTypeAnnotation::List(item_type) => emit_edn_list(ctx, value, item_type.as_ref(), depth, nested),
    CalcitTypeAnnotation::Map(key_type, value_type) => emit_edn_map(ctx, value, key_type.as_ref(), value_type.as_ref(), depth, nested),
    CalcitTypeAnnotation::Struct(nominal, args) => emit_edn_struct(ctx, value, nominal, args, depth, nested),
    CalcitTypeAnnotation::Enum(nominal, args) => emit_edn_enum(ctx, value, nominal, args, depth, nested),
    CalcitTypeAnnotation::Number => Err(
      "E_WASM_EDN_TYPE: generic Number formatting is not yet exact in WASM; use an integer numeric refinement or defer this path"
        .into(),
    ),
    CalcitTypeAnnotation::Dynamic => {
      Err("E_WASM_EDN_TYPE: Dynamic cannot be formatted safely in WASM; decode or narrow it to a closed type first".into())
    }
    CalcitTypeAnnotation::TypeVar(name) => Err(format!(
      "E_WASM_EDN_TYPE: unresolved type variable {name} cannot be formatted in WASM"
    )),
    other => Err(format!(
      "E_WASM_EDN_TYPE: {other} is not yet supported by the WASM Cirru EDN formatter"
    )),
  }
}

fn resolved_enum_variants(nominal: &CalcitEnumDef, args: &[Arc<CalcitTypeAnnotation>]) -> Result<Vec<ResolvedEnumVariant>, String> {
  if nominal.generics().len() != args.len() {
    return Err(format!(
      "E_WASM_EDN_ENUM_SHAPE: enum :{} expects {} type arguments, got {}",
      nominal.name(),
      nominal.generics().len(),
      args.len()
    ));
  }
  let bindings = nominal
    .generics()
    .iter()
    .cloned()
    .zip(args.iter().cloned())
    .collect::<HashMap<_, _>>();
  Ok(
    nominal
      .variants()
      .iter()
      .map(|variant| {
        (
          variant.tag.clone(),
          variant
            .payload_types()
            .iter()
            .map(|payload| payload.substitute_type_vars(&bindings))
            .collect(),
        )
      })
      .collect(),
  )
}

fn emit_is_option_enum_runtime(ctx: &mut WasmGenCtx, value: u32) -> Result<u32, String> {
  let none_tag = *ctx.tag_index.get("none").ok_or("none tag missing")? as i32;
  let some_tag = *ctx.tag_index.get("some").ok_or("some tag missing")? as i32;
  let enum_tag = get_type_tag(ctx, "enum") as i32;
  let is_heap = emit_is_heap_value(ctx, value);
  let is_option_enum = ctx.alloc_i32(0);
  ctx.emit(Instruction::LocalGet(is_heap));
  ctx.begin_block_if();
  let ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(ptr));
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(4));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::I32Const(enum_tag));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  let count = emit_load_enum_count_i32(ctx, ptr);
  let vtag = emit_load_enum_tag_i32(ctx, ptr);
  ctx.emit(Instruction::LocalGet(vtag));
  ctx.emit(Instruction::I32Const(none_tag));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalGet(vtag));
  ctx.emit(Instruction::I32Const(some_tag));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::I32Or);
  ctx.emit(Instruction::LocalSet(is_option_enum));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(is_option_enum)
}

fn emit_edn_enum_variants(
  ctx: &mut WasmGenCtx,
  value: u32,
  nominal: &CalcitEnumDef,
  variants: Vec<ResolvedEnumVariant>,
  depth: usize,
  nested: bool,
) -> Result<u32, String> {
  let ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(ptr));
  let payload_count = emit_load_enum_count_i32(ctx, ptr);
  let variant_tag = emit_load_enum_tag_i32(ctx, ptr);

  let result = literal_local(ctx, if nested { "(%::" } else { "%::" })?;
  let with_name_space = concat_with_literal_after(ctx, result, " ")?;
  ctx.emit(Instruction::LocalGet(with_name_space));
  ctx.emit(Instruction::LocalSet(result));
  let name = literal_local(ctx, &format!("'{}", nominal.name()))?;
  let with_name = concat_string_locals(ctx, result, name);
  ctx.emit(Instruction::LocalGet(with_name));
  ctx.emit(Instruction::LocalSet(result));

  let matched = ctx.alloc_i32(0);
  for (variant, payloads) in variants {
    let tag_id = *ctx.tag_index.get(variant.ref_str()).ok_or_else(|| {
      format!(
        "E_WASM_EDN_ENUM_SHAPE: enum :{} variant :{} is not present in the compiled program",
        nominal.name(),
        variant
      )
    })?;
    ctx.emit(Instruction::LocalGet(variant_tag));
    ctx.emit(Instruction::I32Const(tag_id as i32));
    ctx.emit(Instruction::I32Eq);
    ctx.begin_block_if();
    ctx.emit(Instruction::LocalGet(payload_count));
    ctx.emit(Instruction::I32Const(payloads.len() as i32));
    ctx.emit(Instruction::I32Ne);
    ctx.begin_block_if();
    ctx.emit(Instruction::Unreachable);
    ctx.emit(Instruction::End);
    let with_variant_space = concat_with_literal_after(ctx, result, " ")?;
    ctx.emit(Instruction::LocalGet(with_variant_space));
    ctx.emit(Instruction::LocalSet(result));
    let variant_name = literal_local(ctx, &format!("'{variant}"))?;
    let with_variant = concat_string_locals(ctx, result, variant_name);
    ctx.emit(Instruction::LocalGet(with_variant));
    ctx.emit(Instruction::LocalSet(result));
    for (index, payload_type) in payloads.iter().enumerate() {
      let with_space = concat_with_literal_after(ctx, result, " ")?;
      ctx.emit(Instruction::LocalGet(with_space));
      ctx.emit(Instruction::LocalSet(result));
      let payload = ctx.alloc_local();
      ctx.emit(Instruction::LocalGet(ptr));
      ctx.emit(Instruction::F64Load(mem_arg_f64(((1 + index) * 8) as u64)));
      ctx.emit(Instruction::LocalSet(payload));
      let formatted = emit_edn_value(ctx, payload, payload_type, depth + 1, true)?;
      let with_payload = concat_string_locals(ctx, result, formatted);
      ctx.emit(Instruction::LocalGet(with_payload));
      ctx.emit(Instruction::LocalSet(result));
    }
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::LocalSet(matched));
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::LocalGet(matched));
  ctx.emit(Instruction::I32Eqz);
  ctx.begin_block_if();
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);
  if nested {
    let closed = concat_with_literal_after(ctx, result, ")")?;
    ctx.emit(Instruction::LocalGet(closed));
    ctx.emit(Instruction::LocalSet(result));
  }
  Ok(result)
}

fn append_literal(ctx: &mut WasmGenCtx, acc: u32, text: &str) -> Result<(), String> {
  let next = concat_with_literal_after(ctx, acc, text)?;
  ctx.emit(Instruction::LocalGet(next));
  ctx.emit(Instruction::LocalSet(acc));
  Ok(())
}

fn is_standard_option_variants(nominal: &CalcitEnumDef, args: &[Arc<CalcitTypeAnnotation>], variants: &[ResolvedEnumVariant]) -> bool {
  nominal.name().ref_str() == "Option"
    && args.len() == 1
    && variants.len() == 2
    && variants.iter().any(|(v, payloads)| v.ref_str() == "none" && payloads.is_empty())
    && variants
      .iter()
      .any(|(v, payloads)| v.ref_str() == "some" && payloads.len() == 1 && payloads[0] == args[0])
}

fn emit_extract_option_parts(ctx: &mut WasmGenCtx, value: u32, enum_name: &str) -> Result<(u32, u32, u32), String> {
  let none_tag = *ctx
    .tag_index
    .get("none")
    .ok_or_else(|| format!("E_WASM_EDN_ENUM_SHAPE: enum :{enum_name} variant :none is not present in the compiled program"))?
    as i32;
  let some_tag = *ctx
    .tag_index
    .get("some")
    .ok_or_else(|| format!("E_WASM_EDN_ENUM_SHAPE: enum :{enum_name} variant :some is not present in the compiled program"))?
    as i32;
  let is_option_enum = emit_is_option_enum_runtime(ctx, value)?;
  let is_none = ctx.alloc_i32(0);
  let is_some = ctx.alloc_i32(0);
  let payload = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::LocalSet(payload));

  ctx.emit(Instruction::LocalGet(is_option_enum));
  ctx.begin_block_if();
  let ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(ptr));
  let payload_count = emit_load_enum_count_i32(ctx, ptr);
  let variant_tag = emit_load_enum_tag_i32(ctx, ptr);

  ctx.emit(Instruction::LocalGet(variant_tag));
  ctx.emit(Instruction::I32Const(none_tag));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(payload_count));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::I32Ne);
  ctx.begin_block_if();
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(is_none));
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(variant_tag));
  ctx.emit(Instruction::I32Const(some_tag));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(payload_count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Ne);
  ctx.begin_block_if();
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalSet(payload));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(is_some));
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok((is_none, is_some, payload))
}

fn emit_edn_enum(
  ctx: &mut WasmGenCtx,
  value: u32,
  nominal: &CalcitEnumDef,
  args: &[Arc<CalcitTypeAnnotation>],
  depth: usize,
  nested: bool,
) -> Result<u32, String> {
  let variants = resolved_enum_variants(nominal, args)?;
  for (_, payloads) in &variants {
    for payload in payloads {
      validate_edn_type(payload, depth + 1)?;
    }
  }

  if nominal.name().ref_str() == "Option" && args.len() == 1 {
    if nested && is_standard_option_variants(nominal, args, &variants) {
      let (is_none, is_some, payload) = emit_extract_option_parts(ctx, value, nominal.name().ref_str())?;
      let enum_tag_lit = format!("'{}", nominal.name());
      let out = ctx.alloc_local();
      ctx.emit(Instruction::LocalGet(is_none));
      ctx.begin_block_if();
      let none_str = literal_local(ctx, "(%::")?;
      append_literal(ctx, none_str, " ")?;
      append_literal(ctx, none_str, &enum_tag_lit)?;
      append_literal(ctx, none_str, " ")?;
      append_literal(ctx, none_str, "'none")?;
      append_literal(ctx, none_str, ")")?;
      ctx.emit(Instruction::LocalGet(none_str));
      ctx.emit(Instruction::LocalSet(out));
      ctx.emit(Instruction::Else);
      let formatted = emit_edn_value(ctx, payload, &args[0], depth + 1, true)?;
      ctx.emit(Instruction::LocalGet(is_some));
      ctx.begin_block_if();
      let some_str = literal_local(ctx, "(%::")?;
      append_literal(ctx, some_str, " ")?;
      append_literal(ctx, some_str, &enum_tag_lit)?;
      append_literal(ctx, some_str, " ")?;
      append_literal(ctx, some_str, "'some")?;
      append_literal(ctx, some_str, " ")?;
      let with_payload = concat_string_locals(ctx, some_str, formatted);
      ctx.emit(Instruction::LocalGet(with_payload));
      ctx.emit(Instruction::LocalSet(some_str));
      append_literal(ctx, some_str, ")")?;
      ctx.emit(Instruction::LocalGet(some_str));
      ctx.emit(Instruction::LocalSet(out));
      ctx.emit(Instruction::Else);
      ctx.emit(Instruction::LocalGet(formatted));
      ctx.emit(Instruction::LocalSet(out));
      ctx.emit(Instruction::End);
      ctx.emit(Instruction::End);
      return Ok(out);
    }

    let is_option_enum = emit_is_option_enum_runtime(ctx, value)?;
    let out = ctx.alloc_local();
    ctx.emit(Instruction::LocalGet(is_option_enum));
    ctx.begin_block_if();
    let wrapped = emit_edn_enum_variants(ctx, value, nominal, variants, depth, nested)?;
    ctx.emit(Instruction::LocalGet(wrapped));
    ctx.emit(Instruction::LocalSet(out));
    ctx.emit(Instruction::Else);
    let unwrapped = emit_edn_value(ctx, value, &args[0], depth + 1, nested)?;
    ctx.emit(Instruction::LocalGet(unwrapped));
    ctx.emit(Instruction::LocalSet(out));
    ctx.emit(Instruction::End);
    return Ok(out);
  }

  emit_edn_enum_variants(ctx, value, nominal, variants, depth, nested)
}

pub(super) fn resolved_struct_fields(
  nominal: &CalcitStructDef,
  args: &[Arc<CalcitTypeAnnotation>],
) -> Result<Vec<(EdnTag, Arc<CalcitTypeAnnotation>)>, String> {
  if nominal.generics.len() != args.len() {
    return Err(format!(
      "E_WASM_EDN_STRUCT_SHAPE: struct :{} expects {} type arguments, got {}",
      nominal.name,
      nominal.generics.len(),
      args.len()
    ));
  }
  if nominal.fields.len() != nominal.field_types.len() {
    return Err(format!(
      "E_WASM_EDN_STRUCT_SHAPE: struct :{} has {} fields but {} field types",
      nominal.name,
      nominal.fields.len(),
      nominal.field_types.len()
    ));
  }
  let bindings = nominal
    .generics
    .iter()
    .cloned()
    .zip(args.iter().cloned())
    .collect::<HashMap<_, _>>();
  Ok(
    nominal
      .fields
      .iter()
      .cloned()
      .zip(
        nominal
          .field_types
          .iter()
          .map(|field_type| field_type.substitute_type_vars(&bindings)),
      )
      .collect(),
  )
}

fn struct_layout_for_nominal(ctx: &WasmGenCtx, nominal: &CalcitStructDef) -> Result<StructLayout, String> {
  if let Ok(id) = ctx.struct_layouts.id(nominal)
    && let Ok(layout) = ctx.struct_layouts.layout_by_id(id)
  {
    return Ok(layout.clone());
  }
  let kinds: Vec<StructFieldKind> = if nominal.field_types.len() == nominal.fields.len() {
    nominal.field_types.iter().map(StructFieldKind::from_type_annotation).collect()
  } else {
    vec![StructFieldKind::BoxedF64; nominal.fields.len()]
  };
  StructLayout::compute(&kinds)
}

fn emit_edn_struct(
  ctx: &mut WasmGenCtx,
  value: u32,
  nominal: &CalcitStructDef,
  args: &[Arc<CalcitTypeAnnotation>],
  depth: usize,
  nested: bool,
) -> Result<u32, String> {
  let fields = resolved_struct_fields(nominal, args)?;
  for (_, field_type) in &fields {
    validate_edn_type(field_type, depth + 1)?;
  }
  let struct_layout = struct_layout_for_nominal(ctx, nominal)?;

  let ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(ptr));
  let result = literal_local(ctx, if nested { "(%{}" } else { "%{}" })?;
  let with_name_space = concat_with_literal_after(ctx, result, " ")?;
  ctx.emit(Instruction::LocalGet(with_name_space));
  ctx.emit(Instruction::LocalSet(result));
  let name = literal_local(ctx, &format!("'{}", nominal.name))?;
  let with_name = concat_string_locals(ctx, result, name);
  ctx.emit(Instruction::LocalGet(with_name));
  ctx.emit(Instruction::LocalSet(result));

  for (index, (field, field_type)) in fields.iter().enumerate() {
    let opened = concat_with_literal_after(ctx, result, " (")?;
    ctx.emit(Instruction::LocalGet(opened));
    ctx.emit(Instruction::LocalSet(result));
    let field_name = literal_local(ctx, &format!(":{field}"))?;
    let with_field = concat_string_locals(ctx, result, field_name);
    ctx.emit(Instruction::LocalGet(with_field));
    ctx.emit(Instruction::LocalSet(result));
    let with_space = concat_with_literal_after(ctx, result, " ")?;
    ctx.emit(Instruction::LocalGet(with_space));
    ctx.emit(Instruction::LocalSet(result));

    let field_value = ctx.alloc_local();
    emit_load_struct_field_as_repr(ctx, ptr, struct_layout.fields[index], NativeValRepr::F64);
    ctx.emit(Instruction::LocalSet(field_value));
    let formatted = emit_edn_value(ctx, field_value, field_type, depth + 1, true)?;
    let with_value = concat_string_locals(ctx, result, formatted);
    ctx.emit(Instruction::LocalGet(with_value));
    ctx.emit(Instruction::LocalSet(result));
    let closed = concat_with_literal_after(ctx, result, ")")?;
    ctx.emit(Instruction::LocalGet(closed));
    ctx.emit(Instruction::LocalSet(result));
  }
  if nested {
    let closed = concat_with_literal_after(ctx, result, ")")?;
    ctx.emit(Instruction::LocalGet(closed));
    ctx.emit(Instruction::LocalSet(result));
  }
  Ok(result)
}

fn is_supported_edn_scalar(value_type: &CalcitTypeAnnotation) -> bool {
  matches!(
    value_type,
    CalcitTypeAnnotation::Nil
      | CalcitTypeAnnotation::Bool
      | CalcitTypeAnnotation::String
      | CalcitTypeAnnotation::Tag
      | CalcitTypeAnnotation::Numeric(
        CalcitNumericRefinement::Int8
          | CalcitNumericRefinement::UInt8
          | CalcitNumericRefinement::Int16
          | CalcitNumericRefinement::UInt16
          | CalcitNumericRefinement::Int32
          | CalcitNumericRefinement::UInt32
          | CalcitNumericRefinement::Int64
          | CalcitNumericRefinement::UInt64
      )
  )
}

fn emit_map_key_less_than(ctx: &mut WasmGenCtx, left: u32, right: u32, key_type: &CalcitTypeAnnotation) -> Result<(), String> {
  match key_type {
    CalcitTypeAnnotation::String => {
      ctx.emit(Instruction::LocalGet(left));
      ctx.emit(Instruction::I32TruncF64U);
      ctx.emit(Instruction::LocalGet(right));
      ctx.emit(Instruction::I32TruncF64U);
      ctx.call_rt("__rt_str_compare");
      ctx.emit(f64_const(0.0));
      ctx.emit(Instruction::F64Lt);
      Ok(())
    }
    other if is_supported_edn_scalar(other) => {
      ctx.emit(Instruction::LocalGet(left));
      ctx.emit(Instruction::LocalGet(right));
      ctx.emit(Instruction::F64Lt);
      Ok(())
    }
    other => Err(format!(
      "E_WASM_EDN_MAP_KEY: {other} cannot preserve native Cirru EDN map ordering in WASM"
    )),
  }
}

fn resolved_encoded_struct_fields(
  nominal: &CalcitStructDef,
  args: &[Arc<CalcitTypeAnnotation>],
) -> Result<Vec<(EdnTag, Arc<CalcitTypeAnnotation>)>, String> {
  if nominal.name.ref_str() == "__wasm_literal_map__" {
    return Ok(
      nominal
        .fields
        .iter()
        .cloned()
        .zip(nominal.field_types.iter().map(super::expand_wasm_value_type))
        .collect(),
    );
  }
  let raw_fields = resolved_struct_fields(nominal, args)?;
  let mut out = Vec::with_capacity(raw_fields.len());
  for (field, field_ty) in raw_fields {
    let expanded = super::expand_wasm_value_type(&field_ty);
    let encoded_ty = if let Some(enum_def) = expanded.resolve_to_enum()
      && enum_def.name().ref_str() == "Option"
      && let CalcitTypeAnnotation::Enum(_, opt_args) | CalcitTypeAnnotation::TypeRef(_, opt_args) = expanded.as_ref()
      && opt_args.len() == 1
    {
      let inner = super::expand_wasm_value_type(&opt_args[0]);
      let inner_is_option = inner
        .resolve_to_enum()
        .is_some_and(|inner_enum| inner_enum.name().ref_str() == "Option");
      if inner_is_option {
        Arc::new(CalcitTypeAnnotation::Enum(
          Arc::new(enum_def),
          Arc::new(vec![super::infer_encoded_codec_hint(&inner)]),
        ))
      } else {
        super::infer_encoded_codec_hint(&inner)
      }
    } else {
      super::infer_encoded_codec_hint(&expanded)
    };
    out.push((field, encoded_ty));
  }
  Ok(out)
}

fn emit_edn_map(
  ctx: &mut WasmGenCtx,
  value: u32,
  key_type: &CalcitTypeAnnotation,
  value_type: &CalcitTypeAnnotation,
  depth: usize,
  nested: bool,
) -> Result<u32, String> {
  if matches!(key_type, CalcitTypeAnnotation::Never) && matches!(value_type, CalcitTypeAnnotation::Never) {
    return literal_local(ctx, if nested { "({})" } else { "{}" });
  }
  let encoded_struct_fields = match (key_type, value_type) {
    (CalcitTypeAnnotation::StructValue(_), CalcitTypeAnnotation::Struct(nominal, args)) => {
      let fields = resolved_encoded_struct_fields(nominal, args)?;
      for (_, field_ty) in &fields {
        validate_edn_type(field_ty, depth + 1)?;
      }
      Some((nominal, fields))
    }
    _ => {
      if !is_supported_edn_scalar(key_type) {
        return Err(format!(
          "E_WASM_EDN_MAP_KEY: {key_type} cannot preserve native Cirru EDN map ordering in WASM"
        ));
      }
      validate_edn_type(key_type, depth + 1)?;
      validate_edn_type(value_type, depth + 1)?;
      None
    }
  };
  let effective_key_type = if encoded_struct_fields.is_some() {
    &CalcitTypeAnnotation::Tag
  } else {
    key_type
  };

  let map_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(map_ptr));
  let flat_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(map_ptr));
  ctx.call_rt("__rt_map_linearize");
  ctx.emit(Instruction::LocalSet(flat_ptr));
  let count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(flat_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(count));

  // Sort the fresh flat buffer in place. Closed homogeneous key types make the
  // native EDN ordering reducible to numeric/tag order or UTF-8 string order.
  let index = ctx.alloc_i32(1);
  let cursor = ctx.alloc_local_typed(ValType::I32);
  let key = ctx.alloc_local();
  let item_value = ctx.alloc_local();
  let previous_key = ctx.alloc_local();
  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  emit_load_map_pair_slot(ctx, flat_ptr, index, 8, key);
  emit_load_map_pair_slot(ctx, flat_ptr, index, 16, item_value);
  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::LocalSet(cursor));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(cursor));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::BrIf(1));
  let previous_index = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(cursor));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(previous_index));
  emit_load_map_pair_slot(ctx, flat_ptr, previous_index, 8, previous_key);
  emit_map_key_less_than(ctx, key, previous_key, effective_key_type)?;
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::BrIf(1));
  emit_copy_map_pair(ctx, flat_ptr, previous_index, cursor);
  ctx.emit(Instruction::LocalGet(previous_index));
  ctx.emit(Instruction::LocalSet(cursor));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  emit_store_map_pair_slot(ctx, flat_ptr, cursor, 8, key);
  emit_store_map_pair_slot(ctx, flat_ptr, cursor, 16, item_value);
  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(index));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  let result = literal_local(ctx, if nested { "({}" } else { "{}" })?;
  let format_index = ctx.alloc_i32(0);
  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(format_index));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  emit_load_map_pair_slot(ctx, flat_ptr, format_index, 8, key);
  emit_load_map_pair_slot(ctx, flat_ptr, format_index, 16, item_value);

  let opened = concat_with_literal_after(ctx, result, " (")?;
  ctx.emit(Instruction::LocalGet(opened));
  ctx.emit(Instruction::LocalSet(result));
  let formatted_key = emit_edn_value(ctx, key, effective_key_type, depth + 1, true)?;
  let with_key = concat_string_locals(ctx, result, formatted_key);
  ctx.emit(Instruction::LocalGet(with_key));
  ctx.emit(Instruction::LocalSet(result));
  let with_space = concat_with_literal_after(ctx, result, " ")?;
  ctx.emit(Instruction::LocalGet(with_space));
  ctx.emit(Instruction::LocalSet(result));
  let formatted_value = if let Some((nominal, fields)) = &encoded_struct_fields {
    let fv = ctx.alloc_local();
    ctx.emit(f64_const(0.0));
    ctx.emit(Instruction::LocalSet(fv));
    for (field, field_ty) in fields {
      let tag_id = *ctx.tag_index.get(field.ref_str()).ok_or_else(|| {
        format!(
          "E_WASM_EDN_STRUCT_SHAPE: struct :{} field :{} is not present in the compiled program",
          nominal.name, field
        )
      })?;
      ctx.emit(Instruction::LocalGet(key));
      ctx.emit(f64_const(tag_id as f64));
      ctx.emit(Instruction::F64Eq);
      ctx.begin_block_if();
      let candidate = emit_edn_value(ctx, item_value, field_ty, depth + 1, true)?;
      ctx.emit(Instruction::LocalGet(candidate));
      ctx.emit(Instruction::LocalSet(fv));
      ctx.emit(Instruction::End);
    }
    ctx.emit(Instruction::LocalGet(fv));
    ctx.emit(f64_const(0.0));
    ctx.emit(Instruction::F64Eq);
    ctx.begin_block_if();
    ctx.emit(Instruction::Unreachable);
    ctx.emit(Instruction::End);
    fv
  } else {
    emit_edn_value(ctx, item_value, value_type, depth + 1, true)?
  };
  let with_value = concat_string_locals(ctx, result, formatted_value);
  ctx.emit(Instruction::LocalGet(with_value));
  ctx.emit(Instruction::LocalSet(result));
  let closed = concat_with_literal_after(ctx, result, ")")?;
  ctx.emit(Instruction::LocalGet(closed));
  ctx.emit(Instruction::LocalSet(result));

  ctx.emit(Instruction::LocalGet(format_index));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(format_index));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  if nested {
    let closed = concat_with_literal_after(ctx, result, ")")?;
    ctx.emit(Instruction::LocalGet(closed));
    ctx.emit(Instruction::LocalSet(result));
  }
  Ok(result)
}

fn emit_map_pair_address(ctx: &mut WasmGenCtx, flat_ptr: u32, index: u32, offset: i32) {
  ctx.emit(Instruction::LocalGet(flat_ptr));
  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(offset));
  ctx.emit(Instruction::I32Add);
}

fn emit_load_map_pair_slot(ctx: &mut WasmGenCtx, flat_ptr: u32, index: u32, offset: i32, target: u32) {
  emit_map_pair_address(ctx, flat_ptr, index, offset);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(target));
}

fn emit_store_map_pair_slot(ctx: &mut WasmGenCtx, flat_ptr: u32, index: u32, offset: i32, value: u32) {
  emit_map_pair_address(ctx, flat_ptr, index, offset);
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
}

fn emit_copy_map_pair(ctx: &mut WasmGenCtx, flat_ptr: u32, source: u32, target: u32) {
  let source_key = ctx.alloc_local();
  let source_value = ctx.alloc_local();
  emit_load_map_pair_slot(ctx, flat_ptr, source, 8, source_key);
  emit_load_map_pair_slot(ctx, flat_ptr, source, 16, source_value);
  emit_store_map_pair_slot(ctx, flat_ptr, target, 8, source_key);
  emit_store_map_pair_slot(ctx, flat_ptr, target, 16, source_value);
}

fn emit_edn_tag(ctx: &mut WasmGenCtx, value: u32) -> Result<u32, String> {
  let result = ctx.alloc_local();
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalSet(result));

  let mut tags: Vec<(String, u32)> = ctx.tag_index.iter().map(|(name, id)| (name.clone(), *id)).collect();
  tags.sort_by_key(|(_, id)| *id);
  for (name, id) in tags {
    ctx.emit(Instruction::LocalGet(value));
    ctx.emit(f64_const(id as f64));
    ctx.emit(Instruction::F64Eq);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
    emit_literal(ctx, &format!(":{name}"))?;
    ctx.emit(Instruction::LocalSet(result));
    ctx.emit(Instruction::End);
  }

  ctx.emit(Instruction::LocalGet(result));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);
  Ok(result)
}

fn emit_edn_list(
  ctx: &mut WasmGenCtx,
  value: u32,
  item_type: &CalcitTypeAnnotation,
  depth: usize,
  nested: bool,
) -> Result<u32, String> {
  if matches!(item_type, CalcitTypeAnnotation::Never) {
    return literal_local(ctx, if nested { "([])" } else { "[]" });
  }
  // Validate the recursive schema before emitting a runtime loop.
  validate_edn_type(item_type, depth + 1)?;

  let ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(ptr));
  let count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(count));
  let index = ctx.alloc_i32(0);
  let result = literal_local(ctx, if nested { "([]" } else { "[]" })?;

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  let with_space = concat_with_literal_after(ctx, result, " ")?;
  ctx.emit(Instruction::LocalGet(with_space));
  ctx.emit(Instruction::LocalSet(result));

  let item = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(item));
  let formatted = emit_edn_value(ctx, item, item_type, depth + 1, true)?;
  let appended = concat_string_locals(ctx, result, formatted);
  ctx.emit(Instruction::LocalGet(appended));
  ctx.emit(Instruction::LocalSet(result));

  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(index));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  if nested {
    let closed = concat_with_literal_after(ctx, result, ")")?;
    ctx.emit(Instruction::LocalGet(closed));
    ctx.emit(Instruction::LocalSet(result));
  }
  Ok(result)
}

fn validate_edn_type(value_type: &CalcitTypeAnnotation, depth: usize) -> Result<(), String> {
  if depth > MAX_EDN_TYPE_DEPTH {
    return Err(format!(
      "E_WASM_EDN_DEPTH: format-cirru-edn type nesting exceeds {MAX_EDN_TYPE_DEPTH}"
    ));
  }
  match value_type {
    CalcitTypeAnnotation::TypeRef(_, args) => {
      if let Some(nominal) = value_type.resolve_to_struct() {
        validate_edn_type(&CalcitTypeAnnotation::Struct(Arc::new(nominal), args.clone()), depth)
      } else if let Some(nominal) = value_type.resolve_to_enum() {
        validate_edn_type(&CalcitTypeAnnotation::Enum(Arc::new(nominal), args.clone()), depth)
      } else {
        Err(format!(
          "E_WASM_EDN_TYPE: {value_type} cannot be resolved to a closed nominal type for WASM"
        ))
      }
    }
    CalcitTypeAnnotation::StructValue(nominal) => {
      validate_edn_type(&CalcitTypeAnnotation::Struct(nominal.clone(), Arc::new(vec![])), depth)
    }
    CalcitTypeAnnotation::EnumValue(nominal) => {
      validate_edn_type(&CalcitTypeAnnotation::Enum(nominal.clone(), Arc::new(vec![])), depth)
    }
    CalcitTypeAnnotation::Nil | CalcitTypeAnnotation::Bool | CalcitTypeAnnotation::String | CalcitTypeAnnotation::Tag => Ok(()),
    CalcitTypeAnnotation::Numeric(kind) if !matches!(kind, CalcitNumericRefinement::Float32 | CalcitNumericRefinement::Float64) => {
      Ok(())
    }
    CalcitTypeAnnotation::List(item) => {
      if matches!(item.as_ref(), CalcitTypeAnnotation::Never) {
        return Ok(());
      }
      validate_edn_type(item, depth + 1)
    }
    CalcitTypeAnnotation::Struct(nominal, args) => {
      for (_, field_type) in resolved_struct_fields(nominal, args)? {
        validate_edn_type(&field_type, depth + 1)?;
      }
      Ok(())
    }
    CalcitTypeAnnotation::Enum(nominal, args) => {
      for (_, payloads) in resolved_enum_variants(nominal, args)? {
        for payload in payloads {
          validate_edn_type(&payload, depth + 1)?;
        }
      }
      Ok(())
    }
    CalcitTypeAnnotation::Map(key, value) => {
      if matches!(key.as_ref(), CalcitTypeAnnotation::Never) && matches!(value.as_ref(), CalcitTypeAnnotation::Never) {
        return Ok(());
      }
      if let (CalcitTypeAnnotation::StructValue(_), CalcitTypeAnnotation::Struct(nominal, args)) = (key.as_ref(), value.as_ref()) {
        for (_, field_ty) in resolved_encoded_struct_fields(nominal, args)? {
          validate_edn_type(&field_ty, depth + 1)?;
        }
        return Ok(());
      }
      if !is_supported_edn_scalar(key) {
        return Err(format!(
          "E_WASM_EDN_MAP_KEY: {key} cannot preserve native Cirru EDN map ordering in WASM"
        ));
      }
      validate_edn_type(key, depth + 1)?;
      validate_edn_type(value, depth + 1)
    }
    CalcitTypeAnnotation::Dynamic => {
      Err("E_WASM_EDN_TYPE: Dynamic cannot be formatted safely in WASM; decode or narrow it to a closed type first".into())
    }
    CalcitTypeAnnotation::Number => Err(
      "E_WASM_EDN_TYPE: generic Number formatting is not yet exact in WASM; use an integer numeric refinement or defer this path"
        .into(),
    ),
    other => Err(format!(
      "E_WASM_EDN_TYPE: {other} is not yet supported by the WASM Cirru EDN formatter"
    )),
  }
}

pub(super) fn emit_json_stringify(ctx: &mut WasmGenCtx, args: &[Calcit], pretty: bool) -> Result<(), String> {
  let op_name = if pretty { "json-pretty" } else { "json-stringify" };
  if args.len() != 1 {
    return Err(format!("{op_name} expected 1 argument, got {}", args.len()));
  }

  let inferred_type = super::infer_wasm_codec_hint(ctx, &args[0])
    .or_else(|| super::infer_wasm_static_type(ctx, &args[0]))
    .or_else(|| infer_static_type_from_expr(&args[0]))
    .ok_or_else(|| format!("E_WASM_EDN_TYPE: {op_name} requires a closed inferred input type for WASM"))?;
  let value_type = match inferred_type.as_ref() {
    CalcitTypeAnnotation::TypeRef(_, type_args) => {
      if let Some(nominal) = inferred_type.resolve_to_struct() {
        Arc::new(CalcitTypeAnnotation::Struct(Arc::new(nominal), type_args.clone()))
      } else if let Some(nominal) = inferred_type.resolve_to_enum() {
        Arc::new(CalcitTypeAnnotation::Enum(Arc::new(nominal), type_args.clone()))
      } else {
        inferred_type
      }
    }
    CalcitTypeAnnotation::StructValue(nominal) => Arc::new(CalcitTypeAnnotation::Struct(nominal.clone(), Arc::new(vec![]))),
    CalcitTypeAnnotation::EnumValue(nominal) => Arc::new(CalcitTypeAnnotation::Enum(nominal.clone(), Arc::new(vec![]))),
    _ => inferred_type,
  };

  validate_json_type(value_type.as_ref(), 0, op_name)?;

  emit_expr(ctx, &args[0])?;
  let value = ctx.alloc_local();
  ctx.emit(Instruction::LocalSet(value));

  let output = emit_json_value(ctx, value, value_type.as_ref(), 0, pretty, op_name)?;
  ctx.emit(Instruction::LocalGet(output));
  Ok(())
}

fn validate_json_type(value_type: &CalcitTypeAnnotation, depth: usize, op_name: &str) -> Result<(), String> {
  if depth > MAX_EDN_TYPE_DEPTH {
    return Err(format!("E_WASM_EDN_DEPTH: {op_name} type nesting exceeds {MAX_EDN_TYPE_DEPTH}"));
  }
  match value_type {
    CalcitTypeAnnotation::TypeRef(_, args) => {
      if let Some(nominal) = value_type.resolve_to_struct() {
        validate_json_type(&CalcitTypeAnnotation::Struct(Arc::new(nominal), args.clone()), depth, op_name)
      } else if let Some(nominal) = value_type.resolve_to_enum() {
        validate_json_type(&CalcitTypeAnnotation::Enum(Arc::new(nominal), args.clone()), depth, op_name)
      } else {
        Err(format!(
          "E_WASM_EDN_TYPE: {value_type} cannot be resolved to a closed nominal type for WASM"
        ))
      }
    }
    CalcitTypeAnnotation::StructValue(nominal) => {
      validate_json_type(&CalcitTypeAnnotation::Struct(nominal.clone(), Arc::new(vec![])), depth, op_name)
    }
    CalcitTypeAnnotation::EnumValue(nominal) => {
      validate_json_type(&CalcitTypeAnnotation::Enum(nominal.clone(), Arc::new(vec![])), depth, op_name)
    }
    CalcitTypeAnnotation::Nil
    | CalcitTypeAnnotation::Bool
    | CalcitTypeAnnotation::Number
    | CalcitTypeAnnotation::Numeric(_)
    | CalcitTypeAnnotation::String
    | CalcitTypeAnnotation::Tag => Ok(()),
    CalcitTypeAnnotation::List(item) | CalcitTypeAnnotation::Set(item) => {
      if matches!(item.as_ref(), CalcitTypeAnnotation::Never) {
        return Ok(());
      }
      validate_json_type(item, depth + 1, op_name)
    }
    CalcitTypeAnnotation::Tuple(slots) => {
      for slot in slots.iter() {
        validate_json_type(slot, depth + 1, op_name)?;
      }
      Ok(())
    }
    CalcitTypeAnnotation::Struct(nominal, args) => {
      for (_, field_type) in resolved_struct_fields(nominal, args)? {
        validate_json_type(&field_type, depth + 1, op_name)?;
      }
      Ok(())
    }
    CalcitTypeAnnotation::Enum(nominal, args) => {
      for (_, payloads) in resolved_enum_variants(nominal, args)? {
        for payload in payloads {
          validate_json_type(&payload, depth + 1, op_name)?;
        }
      }
      Ok(())
    }
    CalcitTypeAnnotation::Map(key, value) => {
      if matches!(key.as_ref(), CalcitTypeAnnotation::Never) && matches!(value.as_ref(), CalcitTypeAnnotation::Never) {
        return Ok(());
      }
      if let (CalcitTypeAnnotation::StructValue(_), CalcitTypeAnnotation::Struct(nominal, args)) = (key.as_ref(), value.as_ref()) {
        for (_, field_ty) in resolved_encoded_struct_fields(nominal, args)? {
          validate_json_type(&field_ty, depth + 1, op_name)?;
        }
        return Ok(());
      }
      if !matches!(key.as_ref(), CalcitTypeAnnotation::Tag | CalcitTypeAnnotation::String) {
        return Err(format!(
          "E_WASM_EDN_MAP_KEY: {op_name} expected object keys to be :tag or :string, got {key}"
        ));
      }
      validate_json_type(key, depth + 1, op_name)?;
      validate_json_type(value, depth + 1, op_name)
    }
    CalcitTypeAnnotation::Dynamic => Err(format!(
      "E_WASM_EDN_TYPE: Dynamic cannot be formatted safely by {op_name} in WASM; decode or narrow it to a closed type first"
    )),
    other => Err(format!("E_WASM_EDN_TYPE: {other} is not supported by {op_name} in WASM")),
  }
}

fn emit_json_value(
  ctx: &mut WasmGenCtx,
  value: u32,
  value_type: &CalcitTypeAnnotation,
  depth: usize,
  pretty: bool,
  op_name: &str,
) -> Result<u32, String> {
  if depth > MAX_EDN_TYPE_DEPTH {
    return Err(format!("E_WASM_EDN_DEPTH: {op_name} type nesting exceeds {MAX_EDN_TYPE_DEPTH}"));
  }
  match value_type {
    CalcitTypeAnnotation::TypeRef(_, args) => {
      if let Some(nominal) = value_type.resolve_to_struct() {
        emit_json_struct(ctx, value, &nominal, args, depth, pretty, op_name)
      } else if let Some(nominal) = value_type.resolve_to_enum() {
        emit_json_enum(ctx, value, &nominal, args, depth, pretty, op_name)
      } else {
        Err(format!(
          "E_WASM_EDN_TYPE: {value_type} cannot be resolved to a closed nominal type for WASM"
        ))
      }
    }
    CalcitTypeAnnotation::StructValue(nominal) => emit_json_struct(ctx, value, nominal, &Arc::new(vec![]), depth, pretty, op_name),
    CalcitTypeAnnotation::EnumValue(nominal) => emit_json_enum(ctx, value, nominal, &Arc::new(vec![]), depth, pretty, op_name),
    CalcitTypeAnnotation::Nil => literal_local(ctx, "null"),
    CalcitTypeAnnotation::Bool => {
      let result = ctx.alloc_local();
      super::emit_value_is_truthy_i32(ctx, value);
      ctx.emit(Instruction::I32Eqz);
      ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
      emit_literal(ctx, "false")?;
      ctx.emit(Instruction::Else);
      emit_literal(ctx, "true")?;
      ctx.emit(Instruction::End);
      ctx.emit(Instruction::LocalSet(result));
      Ok(result)
    }
    CalcitTypeAnnotation::Number | CalcitTypeAnnotation::Numeric(_) => {
      ctx.emit(Instruction::LocalGet(value));
      ctx.emit(Instruction::LocalGet(value));
      ctx.emit(Instruction::F64Sub);
      ctx.emit(f64_const(0.0));
      ctx.emit(Instruction::F64Ne);
      ctx.begin_block_if();
      ctx.emit(Instruction::Unreachable);
      ctx.emit(Instruction::End);
      let result = ctx.alloc_local();
      ctx.emit(Instruction::LocalGet(value));
      ctx.call_rt("__rt_f64_to_str");
      ctx.emit(Instruction::LocalSet(result));
      Ok(result)
    }
    CalcitTypeAnnotation::String => {
      let result = ctx.alloc_local();
      ctx.emit(Instruction::LocalGet(value));
      ctx.call_rt("__rt_json_string");
      ctx.emit(Instruction::LocalSet(result));
      Ok(result)
    }
    CalcitTypeAnnotation::Tag => emit_json_tag(ctx, value),
    CalcitTypeAnnotation::List(item_type) | CalcitTypeAnnotation::Set(item_type) => {
      emit_json_list(ctx, value, item_type.as_ref(), depth, pretty, op_name)
    }
    CalcitTypeAnnotation::Tuple(slots) => emit_json_tuple(ctx, value, slots.as_ref(), depth, pretty, op_name),
    CalcitTypeAnnotation::Map(key_type, val_type) => {
      emit_json_map(ctx, value, key_type.as_ref(), val_type.as_ref(), depth, pretty, op_name)
    }
    CalcitTypeAnnotation::Struct(nominal, args) => emit_json_struct(ctx, value, nominal, args, depth, pretty, op_name),
    CalcitTypeAnnotation::Enum(nominal, args) => emit_json_enum(ctx, value, nominal, args, depth, pretty, op_name),
    other => Err(format!("E_WASM_EDN_TYPE: {other} is not supported by {op_name} in WASM")),
  }
}

fn emit_json_tag(ctx: &mut WasmGenCtx, value: u32) -> Result<u32, String> {
  let raw_tag_str = ctx.alloc_local();
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::LocalSet(raw_tag_str));

  let mut tags: Vec<(String, u32)> = ctx.tag_index.iter().map(|(name, id)| (name.clone(), *id)).collect();
  tags.sort_by_key(|(_, id)| *id);
  for (name, id) in tags {
    ctx.emit(Instruction::LocalGet(value));
    ctx.emit(f64_const(id as f64));
    ctx.emit(Instruction::F64Eq);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
    emit_literal(ctx, &name)?;
    ctx.emit(Instruction::LocalSet(raw_tag_str));
    ctx.emit(Instruction::End);
  }

  ctx.emit(Instruction::LocalGet(raw_tag_str));
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);

  let result = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(raw_tag_str));
  ctx.call_rt("__rt_json_string");
  ctx.emit(Instruction::LocalSet(result));
  Ok(result)
}

fn emit_edn_add_len_checked(ctx: &mut WasmGenCtx, total_len: u32, add_len: u32) {
  ctx.emit(Instruction::LocalGet(add_len));
  ctx.emit(Instruction::I32Const(MAX_EDN_OUTPUT_BYTES));
  ctx.emit(Instruction::LocalGet(total_len));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::I32GtU);
  ctx.begin_block_if();
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(total_len));
  ctx.emit(Instruction::LocalGet(add_len));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_len));
}

fn emit_edn_add_const_len_checked(ctx: &mut WasmGenCtx, total_len: u32, add_len: i32) {
  if add_len > 0 {
    let add_local = ctx.alloc_i32(add_len);
    emit_edn_add_len_checked(ctx, total_len, add_local);
  }
}

fn emit_alloc_string_of_len(ctx: &mut WasmGenCtx, byte_len: u32) -> u32 {
  let padded = ctx.alloc_local_typed(ValType::I32);
  let payload_size = ctx.alloc_local_typed(ValType::I32);
  let out_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(byte_len));
  ctx.emit(Instruction::I32Const(7));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(-8));
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalSet(padded));
  ctx.emit(Instruction::LocalGet(padded));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(payload_size));
  emit_bump_alloc_dynamic(ctx, payload_size, out_ptr, "string");
  ctx.emit(Instruction::LocalGet(out_ptr));
  ctx.emit(Instruction::LocalGet(byte_len));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  out_ptr
}

fn emit_copy_str_ptr(ctx: &mut WasmGenCtx, out_ptr: u32, write_off: u32, str_ptr: u32) {
  let str_len = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(str_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(str_len));
  ctx.emit(Instruction::LocalGet(out_ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(write_off));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(str_ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(str_len));
  ctx.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
  ctx.emit(Instruction::LocalGet(write_off));
  ctx.emit(Instruction::LocalGet(str_len));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(write_off));
}

fn emit_copy_str_literal(ctx: &mut WasmGenCtx, out_ptr: u32, write_off: u32, text: &str) -> Result<(), String> {
  let lit_ptr_val = *ctx
    .string_pool
    .get(text)
    .ok_or_else(|| format!("internal Cirru EDN literal missing from string pool: {text:?}"))?;
  let lit_len = text.len() as i32;
  if lit_len > 0 {
    ctx.emit(Instruction::LocalGet(out_ptr));
    ctx.emit(Instruction::I32Const(8));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(write_off));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::I32Const(lit_ptr_val as i32 + 8));
    ctx.emit(Instruction::I32Const(lit_len));
    ctx.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
    ctx.emit(Instruction::LocalGet(write_off));
    ctx.emit(Instruction::I32Const(lit_len));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(write_off));
  }
  Ok(())
}

fn emit_json_list(
  ctx: &mut WasmGenCtx,
  value: u32,
  item_type: &CalcitTypeAnnotation,
  depth: usize,
  pretty: bool,
  op_name: &str,
) -> Result<u32, String> {
  if matches!(item_type, CalcitTypeAnnotation::Never) {
    return literal_local(ctx, "[]");
  }
  validate_json_type(item_type, depth + 1, op_name)?;

  let ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(ptr));
  let count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(count));

  let result = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Eqz);
  ctx.begin_block_if();
  emit_literal(ctx, "[]")?;
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::Else);

  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(MAX_EDN_OUTPUT_BYTES));
  ctx.emit(Instruction::I32GtU);
  ctx.begin_block_if();
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);

  let child_indent = "  ".repeat(depth + 1);
  let close_indent = format!("\n{}", "  ".repeat(depth));
  let open_lit = if pretty { "[\n" } else { "[" };
  let sep_lit = if pretty { ",\n" } else { "," };
  let wrap_const = open_lit.len() + if pretty { child_indent.len() + close_indent.len() } else { 0 } + 1;
  let sep_const = sep_lit.len() + if pretty { child_indent.len() } else { 0 };

  let parts_bytes = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(4));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(7));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(-8));
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalSet(parts_bytes));
  let parts_ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc_dynamic(ctx, parts_bytes, parts_ptr, "string");

  let total_len = ctx.alloc_i32(wrap_const as i32);
  let index = ctx.alloc_i32(0);
  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::I32GtU);
  ctx.begin_block_if();
  emit_edn_add_const_len_checked(ctx, total_len, sep_const as i32);
  ctx.emit(Instruction::End);

  let item = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(item));
  let formatted = emit_json_value(ctx, item, item_type, depth + 1, pretty, op_name)?;
  let item_str_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(formatted));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(item_str_ptr));

  ctx.emit(Instruction::LocalGet(parts_ptr));
  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::I32Const(4));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(item_str_ptr));
  ctx.emit(Instruction::I32Store(mem_arg_i32(0)));

  let item_len = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(item_str_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(item_len));
  emit_edn_add_len_checked(ctx, total_len, item_len);

  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(index));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  let out_ptr = emit_alloc_string_of_len(ctx, total_len);
  let write_off = ctx.alloc_i32(0);
  emit_copy_str_literal(ctx, out_ptr, write_off, open_lit)?;
  if pretty {
    emit_copy_str_literal(ctx, out_ptr, write_off, &child_indent)?;
  }

  let copy_idx = ctx.alloc_i32(0);
  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(copy_idx));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  ctx.emit(Instruction::LocalGet(copy_idx));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::I32GtU);
  ctx.begin_block_if();
  emit_copy_str_literal(ctx, out_ptr, write_off, sep_lit)?;
  if pretty {
    emit_copy_str_literal(ctx, out_ptr, write_off, &child_indent)?;
  }
  ctx.emit(Instruction::End);

  let src_str_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(parts_ptr));
  ctx.emit(Instruction::LocalGet(copy_idx));
  ctx.emit(Instruction::I32Const(4));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::LocalSet(src_str_ptr));
  emit_copy_str_ptr(ctx, out_ptr, write_off, src_str_ptr);

  ctx.emit(Instruction::LocalGet(copy_idx));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(copy_idx));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  if pretty {
    emit_copy_str_literal(ctx, out_ptr, write_off, &close_indent)?;
  }
  emit_copy_str_literal(ctx, out_ptr, write_off, "]")?;
  ctx.ptr_to_f64(out_ptr);
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::End);
  Ok(result)
}

fn emit_json_tuple(
  ctx: &mut WasmGenCtx,
  value: u32,
  slots: &[Arc<CalcitTypeAnnotation>],
  depth: usize,
  pretty: bool,
  op_name: &str,
) -> Result<u32, String> {
  for slot in slots {
    validate_json_type(slot, depth + 1, op_name)?;
  }
  if slots.is_empty() {
    return literal_local(ctx, "[]");
  }

  let ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(ptr));

  let result = literal_local(ctx, if pretty { "[\n" } else { "[" })?;
  let child_indent = "  ".repeat(depth + 1);
  let close_indent = format!("\n{}", "  ".repeat(depth));

  for (idx, slot_ty) in slots.iter().enumerate() {
    if idx > 0 {
      let with_sep = concat_with_literal_after(ctx, result, if pretty { ",\n" } else { "," })?;
      ctx.emit(Instruction::LocalGet(with_sep));
      ctx.emit(Instruction::LocalSet(result));
    }
    if pretty {
      let with_indent = concat_with_literal_after(ctx, result, &child_indent)?;
      ctx.emit(Instruction::LocalGet(with_indent));
      ctx.emit(Instruction::LocalSet(result));
    }
    let item = ctx.alloc_local();
    ctx.emit(Instruction::LocalGet(ptr));
    ctx.emit(Instruction::F64Load(mem_arg_f64(((1 + idx) * 8) as u64)));
    ctx.emit(Instruction::LocalSet(item));
    let formatted = emit_json_value(ctx, item, slot_ty, depth + 1, pretty, op_name)?;
    let appended = concat_string_locals(ctx, result, formatted);
    ctx.emit(Instruction::LocalGet(appended));
    ctx.emit(Instruction::LocalSet(result));
  }

  if pretty {
    let with_close_indent = concat_with_literal_after(ctx, result, &close_indent)?;
    ctx.emit(Instruction::LocalGet(with_close_indent));
    ctx.emit(Instruction::LocalSet(result));
  }
  let closed = concat_with_literal_after(ctx, result, "]")?;
  ctx.emit(Instruction::LocalGet(closed));
  ctx.emit(Instruction::LocalSet(result));
  Ok(result)
}

fn emit_json_struct(
  ctx: &mut WasmGenCtx,
  value: u32,
  nominal: &CalcitStructDef,
  args: &[Arc<CalcitTypeAnnotation>],
  depth: usize,
  pretty: bool,
  op_name: &str,
) -> Result<u32, String> {
  let fields = resolved_struct_fields(nominal, args)?;
  for (_, field_type) in &fields {
    validate_json_type(field_type, depth + 1, op_name)?;
  }
  if fields.is_empty() {
    return literal_local(ctx, "{}");
  }

  let struct_layout = struct_layout_for_nominal(ctx, nominal)?;
  let ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(ptr));

  let mut indexed_fields = fields
    .into_iter()
    .enumerate()
    .map(|(idx, (field, field_ty))| (idx, field, field_ty))
    .collect::<Vec<_>>();
  indexed_fields.sort_by(|a, b| a.1.ref_str().cmp(b.1.ref_str()));

  let result = literal_local(ctx, if pretty { "{\n" } else { "{" })?;
  let child_indent = "  ".repeat(depth + 1);
  let close_indent = format!("\n{}", "  ".repeat(depth));

  for (out_idx, (field_idx, field, field_type)) in indexed_fields.iter().enumerate() {
    if out_idx > 0 {
      let with_sep = concat_with_literal_after(ctx, result, if pretty { ",\n" } else { "," })?;
      ctx.emit(Instruction::LocalGet(with_sep));
      ctx.emit(Instruction::LocalSet(result));
    }
    if pretty {
      let with_indent = concat_with_literal_after(ctx, result, &child_indent)?;
      ctx.emit(Instruction::LocalGet(with_indent));
      ctx.emit(Instruction::LocalSet(result));
    }
    let raw_field = literal_local(ctx, field.ref_str())?;
    let json_field = ctx.alloc_local();
    ctx.emit(Instruction::LocalGet(raw_field));
    ctx.call_rt("__rt_json_string");
    ctx.emit(Instruction::LocalSet(json_field));
    let with_field = concat_string_locals(ctx, result, json_field);
    ctx.emit(Instruction::LocalGet(with_field));
    ctx.emit(Instruction::LocalSet(result));
    let with_colon = concat_with_literal_after(ctx, result, if pretty { ": " } else { ":" })?;
    ctx.emit(Instruction::LocalGet(with_colon));
    ctx.emit(Instruction::LocalSet(result));

    let field_value = ctx.alloc_local();
    emit_load_struct_field_as_repr(ctx, ptr, struct_layout.fields[*field_idx], NativeValRepr::F64);
    ctx.emit(Instruction::LocalSet(field_value));
    let formatted = emit_json_value(ctx, field_value, field_type, depth + 1, pretty, op_name)?;
    let with_value = concat_string_locals(ctx, result, formatted);
    ctx.emit(Instruction::LocalGet(with_value));
    ctx.emit(Instruction::LocalSet(result));
  }

  if pretty {
    let with_close_indent = concat_with_literal_after(ctx, result, &close_indent)?;
    ctx.emit(Instruction::LocalGet(with_close_indent));
    ctx.emit(Instruction::LocalSet(result));
  }
  let closed = concat_with_literal_after(ctx, result, "}")?;
  ctx.emit(Instruction::LocalGet(closed));
  ctx.emit(Instruction::LocalSet(result));
  Ok(result)
}

fn emit_json_enum_variants(
  ctx: &mut WasmGenCtx,
  value: u32,
  nominal: &CalcitEnumDef,
  variants: Vec<ResolvedEnumVariant>,
  depth: usize,
  pretty: bool,
  op_name: &str,
) -> Result<u32, String> {
  let ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(ptr));
  let payload_count = emit_load_enum_count_i32(ctx, ptr);
  let variant_tag = emit_load_enum_tag_i32(ctx, ptr);

  let result = literal_local(ctx, if pretty { "[\n" } else { "[" })?;
  let child_indent = "  ".repeat(depth + 1);
  let close_indent = format!("\n{}", "  ".repeat(depth));

  let matched = ctx.alloc_i32(0);
  for (variant, payloads) in variants {
    let tag_id = *ctx.tag_index.get(variant.ref_str()).ok_or_else(|| {
      format!(
        "E_WASM_EDN_ENUM_SHAPE: enum :{} variant :{} is not present in the compiled program",
        nominal.name(),
        variant
      )
    })?;
    ctx.emit(Instruction::LocalGet(variant_tag));
    ctx.emit(Instruction::I32Const(tag_id as i32));
    ctx.emit(Instruction::I32Eq);
    ctx.begin_block_if();
    ctx.emit(Instruction::LocalGet(payload_count));
    ctx.emit(Instruction::I32Const(payloads.len() as i32));
    ctx.emit(Instruction::I32Ne);
    ctx.begin_block_if();
    ctx.emit(Instruction::Unreachable);
    ctx.emit(Instruction::End);

    if pretty {
      let with_indent = concat_with_literal_after(ctx, result, &child_indent)?;
      ctx.emit(Instruction::LocalGet(with_indent));
      ctx.emit(Instruction::LocalSet(result));
    }
    let raw_variant = literal_local(ctx, variant.ref_str())?;
    let json_variant = ctx.alloc_local();
    ctx.emit(Instruction::LocalGet(raw_variant));
    ctx.call_rt("__rt_json_string");
    ctx.emit(Instruction::LocalSet(json_variant));
    let with_variant = concat_string_locals(ctx, result, json_variant);
    ctx.emit(Instruction::LocalGet(with_variant));
    ctx.emit(Instruction::LocalSet(result));

    for (index, payload_type) in payloads.iter().enumerate() {
      let with_sep = concat_with_literal_after(ctx, result, if pretty { ",\n" } else { "," })?;
      ctx.emit(Instruction::LocalGet(with_sep));
      ctx.emit(Instruction::LocalSet(result));
      if pretty {
        let with_indent = concat_with_literal_after(ctx, result, &child_indent)?;
        ctx.emit(Instruction::LocalGet(with_indent));
        ctx.emit(Instruction::LocalSet(result));
      }
      let payload = ctx.alloc_local();
      ctx.emit(Instruction::LocalGet(ptr));
      ctx.emit(Instruction::F64Load(mem_arg_f64(((1 + index) * 8) as u64)));
      ctx.emit(Instruction::LocalSet(payload));
      let formatted = emit_json_value(ctx, payload, payload_type, depth + 1, pretty, op_name)?;
      let with_payload = concat_string_locals(ctx, result, formatted);
      ctx.emit(Instruction::LocalGet(with_payload));
      ctx.emit(Instruction::LocalSet(result));
    }
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::LocalSet(matched));
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::LocalGet(matched));
  ctx.emit(Instruction::I32Eqz);
  ctx.begin_block_if();
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);

  if pretty {
    let with_close_indent = concat_with_literal_after(ctx, result, &close_indent)?;
    ctx.emit(Instruction::LocalGet(with_close_indent));
    ctx.emit(Instruction::LocalSet(result));
  }
  let closed = concat_with_literal_after(ctx, result, "]")?;
  ctx.emit(Instruction::LocalGet(closed));
  ctx.emit(Instruction::LocalSet(result));
  Ok(result)
}

fn emit_json_enum(
  ctx: &mut WasmGenCtx,
  value: u32,
  nominal: &CalcitEnumDef,
  args: &[Arc<CalcitTypeAnnotation>],
  depth: usize,
  pretty: bool,
  op_name: &str,
) -> Result<u32, String> {
  let variants = resolved_enum_variants(nominal, args)?;
  for (_, payloads) in &variants {
    for payload in payloads {
      validate_json_type(payload, depth + 1, op_name)?;
    }
  }

  if nominal.name().ref_str() == "Option" && args.len() == 1 {
    if is_standard_option_variants(nominal, args, &variants) {
      let (is_none, is_some, payload) = emit_extract_option_parts(ctx, value, nominal.name().ref_str())?;
      let child_indent = "  ".repeat(depth + 1);
      let close_indent = format!("\n{}", "  ".repeat(depth));
      let out = ctx.alloc_local();

      let emit_json_tag_str = |ctx: &mut WasmGenCtx, acc: u32, tag_name: &str| -> Result<(), String> {
        let raw = literal_local(ctx, tag_name)?;
        let encoded = ctx.alloc_local();
        ctx.emit(Instruction::LocalGet(raw));
        ctx.call_rt("__rt_json_string");
        ctx.emit(Instruction::LocalSet(encoded));
        let next = concat_string_locals(ctx, acc, encoded);
        ctx.emit(Instruction::LocalGet(next));
        ctx.emit(Instruction::LocalSet(acc));
        Ok(())
      };

      ctx.emit(Instruction::LocalGet(is_none));
      ctx.begin_block_if();
      let none_str = literal_local(ctx, if pretty { "[\n" } else { "[" })?;
      if pretty {
        append_literal(ctx, none_str, &child_indent)?;
      }
      emit_json_tag_str(ctx, none_str, "none")?;
      if pretty {
        append_literal(ctx, none_str, &close_indent)?;
      }
      append_literal(ctx, none_str, "]")?;
      ctx.emit(Instruction::LocalGet(none_str));
      ctx.emit(Instruction::LocalSet(out));
      ctx.emit(Instruction::Else);
      let formatted = emit_json_value(ctx, payload, &args[0], depth + 1, pretty, op_name)?;
      ctx.emit(Instruction::LocalGet(is_some));
      ctx.begin_block_if();
      let some_str = literal_local(ctx, if pretty { "[\n" } else { "[" })?;
      if pretty {
        append_literal(ctx, some_str, &child_indent)?;
      }
      emit_json_tag_str(ctx, some_str, "some")?;
      append_literal(ctx, some_str, if pretty { ",\n" } else { "," })?;
      if pretty {
        append_literal(ctx, some_str, &child_indent)?;
      }
      let with_payload = concat_string_locals(ctx, some_str, formatted);
      ctx.emit(Instruction::LocalGet(with_payload));
      ctx.emit(Instruction::LocalSet(some_str));
      if pretty {
        append_literal(ctx, some_str, &close_indent)?;
      }
      append_literal(ctx, some_str, "]")?;
      ctx.emit(Instruction::LocalGet(some_str));
      ctx.emit(Instruction::LocalSet(out));
      ctx.emit(Instruction::Else);
      ctx.emit(Instruction::LocalGet(formatted));
      ctx.emit(Instruction::LocalSet(out));
      ctx.emit(Instruction::End);
      ctx.emit(Instruction::End);
      return Ok(out);
    }

    let is_option_enum = emit_is_option_enum_runtime(ctx, value)?;
    let out = ctx.alloc_local();
    ctx.emit(Instruction::LocalGet(is_option_enum));
    ctx.begin_block_if();
    let wrapped = emit_json_enum_variants(ctx, value, nominal, variants, depth, pretty, op_name)?;
    ctx.emit(Instruction::LocalGet(wrapped));
    ctx.emit(Instruction::LocalSet(out));
    ctx.emit(Instruction::Else);
    let unwrapped = emit_json_value(ctx, value, &args[0], depth + 1, pretty, op_name)?;
    ctx.emit(Instruction::LocalGet(unwrapped));
    ctx.emit(Instruction::LocalSet(out));
    ctx.emit(Instruction::End);
    return Ok(out);
  }

  emit_json_enum_variants(ctx, value, nominal, variants, depth, pretty, op_name)
}

fn emit_json_map(
  ctx: &mut WasmGenCtx,
  value: u32,
  key_type: &CalcitTypeAnnotation,
  value_type: &CalcitTypeAnnotation,
  depth: usize,
  pretty: bool,
  op_name: &str,
) -> Result<u32, String> {
  if matches!(key_type, CalcitTypeAnnotation::Never) && matches!(value_type, CalcitTypeAnnotation::Never) {
    return literal_local(ctx, "{}");
  }
  let encoded_struct_fields = match (key_type, value_type) {
    (CalcitTypeAnnotation::StructValue(_), CalcitTypeAnnotation::Struct(nominal, args)) => {
      let fields = resolved_encoded_struct_fields(nominal, args)?;
      for (_, field_ty) in &fields {
        validate_json_type(field_ty, depth + 1, op_name)?;
      }
      Some((nominal, fields))
    }
    _ => {
      if !matches!(key_type, CalcitTypeAnnotation::Tag | CalcitTypeAnnotation::String) {
        return Err(format!(
          "E_WASM_EDN_MAP_KEY: {op_name} expected object keys to be :tag or :string, got {key_type}"
        ));
      }
      validate_json_type(key_type, depth + 1, op_name)?;
      validate_json_type(value_type, depth + 1, op_name)?;
      None
    }
  };
  let effective_key_type = if encoded_struct_fields.is_some() {
    &CalcitTypeAnnotation::Tag
  } else {
    key_type
  };

  let map_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(value));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(map_ptr));
  let flat_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(map_ptr));
  ctx.call_rt("__rt_map_linearize");
  ctx.emit(Instruction::LocalSet(flat_ptr));
  let count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(flat_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(count));

  let result = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Eqz);
  ctx.begin_block_if();
  emit_literal(ctx, "{}")?;
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::Else);

  let index = ctx.alloc_i32(1);
  let cursor = ctx.alloc_local_typed(ValType::I32);
  let key = ctx.alloc_local();
  let item_value = ctx.alloc_local();
  let previous_key = ctx.alloc_local();
  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  emit_load_map_pair_slot(ctx, flat_ptr, index, 8, key);
  emit_load_map_pair_slot(ctx, flat_ptr, index, 16, item_value);
  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::LocalSet(cursor));

  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(cursor));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::BrIf(1));
  let previous_index = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(cursor));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(previous_index));
  emit_load_map_pair_slot(ctx, flat_ptr, previous_index, 8, previous_key);
  emit_map_key_less_than(ctx, key, previous_key, effective_key_type)?;
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::BrIf(1));
  emit_copy_map_pair(ctx, flat_ptr, previous_index, cursor);
  ctx.emit(Instruction::LocalGet(previous_index));
  ctx.emit(Instruction::LocalSet(cursor));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  emit_store_map_pair_slot(ctx, flat_ptr, cursor, 8, key);
  emit_store_map_pair_slot(ctx, flat_ptr, cursor, 16, item_value);
  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(index));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(MAX_EDN_OUTPUT_BYTES));
  ctx.emit(Instruction::I32GtU);
  ctx.begin_block_if();
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);

  let child_indent = "  ".repeat(depth + 1);
  let close_indent = format!("\n{}", "  ".repeat(depth));
  let open_lit = if pretty { "{\n" } else { "{" };
  let sep_lit = if pretty { ",\n" } else { "," };
  let colon_lit = if pretty { ": " } else { ":" };
  let wrap_const = open_lit.len() + if pretty { close_indent.len() } else { 0 } + 1;
  let entry_const = if pretty { child_indent.len() } else { 0 } + colon_lit.len();
  let sep_const = sep_lit.len();

  let parts_bytes = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::LocalSet(parts_bytes));
  let parts_ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc_dynamic(ctx, parts_bytes, parts_ptr, "string");

  let total_len = ctx.alloc_i32(wrap_const as i32);
  let format_index = ctx.alloc_i32(0);
  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(format_index));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  emit_load_map_pair_slot(ctx, flat_ptr, format_index, 8, key);
  emit_load_map_pair_slot(ctx, flat_ptr, format_index, 16, item_value);

  ctx.emit(Instruction::LocalGet(format_index));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::I32GtU);
  ctx.begin_block_if();
  emit_edn_add_const_len_checked(ctx, total_len, sep_const as i32);
  ctx.emit(Instruction::End);
  emit_edn_add_const_len_checked(ctx, total_len, entry_const as i32);

  let formatted_key = emit_json_value(ctx, key, effective_key_type, depth + 1, pretty, op_name)?;
  let key_str_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(formatted_key));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(key_str_ptr));

  let formatted_value = if let Some((nominal, fields)) = &encoded_struct_fields {
    let fv = ctx.alloc_local();
    ctx.emit(f64_const(0.0));
    ctx.emit(Instruction::LocalSet(fv));
    for (field, field_ty) in fields {
      let tag_id = *ctx.tag_index.get(field.ref_str()).ok_or_else(|| {
        format!(
          "E_WASM_EDN_STRUCT_SHAPE: struct :{} field :{} is not present in the compiled program",
          nominal.name, field
        )
      })?;
      ctx.emit(Instruction::LocalGet(key));
      ctx.emit(f64_const(tag_id as f64));
      ctx.emit(Instruction::F64Eq);
      ctx.begin_block_if();
      let candidate = emit_json_value(ctx, item_value, field_ty, depth + 1, pretty, op_name)?;
      ctx.emit(Instruction::LocalGet(candidate));
      ctx.emit(Instruction::LocalSet(fv));
      ctx.emit(Instruction::End);
    }
    ctx.emit(Instruction::LocalGet(fv));
    ctx.emit(f64_const(0.0));
    ctx.emit(Instruction::F64Eq);
    ctx.begin_block_if();
    ctx.emit(Instruction::Unreachable);
    ctx.emit(Instruction::End);
    fv
  } else {
    emit_json_value(ctx, item_value, value_type, depth + 1, pretty, op_name)?
  };
  let val_str_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(formatted_value));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(val_str_ptr));

  ctx.emit(Instruction::LocalGet(parts_ptr));
  ctx.emit(Instruction::LocalGet(format_index));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(key_str_ptr));
  ctx.emit(Instruction::I32Store(mem_arg_i32(0)));
  ctx.emit(Instruction::LocalGet(parts_ptr));
  ctx.emit(Instruction::LocalGet(format_index));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(val_str_ptr));
  ctx.emit(Instruction::I32Store(mem_arg_i32(4)));

  let key_len = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(key_str_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(key_len));
  emit_edn_add_len_checked(ctx, total_len, key_len);

  let val_len = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(val_str_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(val_len));
  emit_edn_add_len_checked(ctx, total_len, val_len);

  ctx.emit(Instruction::LocalGet(format_index));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(format_index));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  let out_ptr = emit_alloc_string_of_len(ctx, total_len);
  let write_off = ctx.alloc_i32(0);
  emit_copy_str_literal(ctx, out_ptr, write_off, open_lit)?;

  let copy_idx = ctx.alloc_i32(0);
  ctx.emit(Instruction::Block(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Loop(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::LocalGet(copy_idx));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  ctx.emit(Instruction::LocalGet(copy_idx));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::I32GtU);
  ctx.begin_block_if();
  emit_copy_str_literal(ctx, out_ptr, write_off, sep_lit)?;
  ctx.emit(Instruction::End);
  if pretty {
    emit_copy_str_literal(ctx, out_ptr, write_off, &child_indent)?;
  }

  let pair_addr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(parts_ptr));
  ctx.emit(Instruction::LocalGet(copy_idx));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(pair_addr));

  let src_key_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(pair_addr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::LocalSet(src_key_ptr));
  emit_copy_str_ptr(ctx, out_ptr, write_off, src_key_ptr);

  emit_copy_str_literal(ctx, out_ptr, write_off, colon_lit)?;

  let src_val_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(pair_addr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
  ctx.emit(Instruction::LocalSet(src_val_ptr));
  emit_copy_str_ptr(ctx, out_ptr, write_off, src_val_ptr);

  ctx.emit(Instruction::LocalGet(copy_idx));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(copy_idx));
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  if pretty {
    emit_copy_str_literal(ctx, out_ptr, write_off, &close_indent)?;
  }
  emit_copy_str_literal(ctx, out_ptr, write_off, "}")?;
  ctx.ptr_to_f64(out_ptr);
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::End);
  Ok(result)
}

fn literal_local(ctx: &mut WasmGenCtx, text: &str) -> Result<u32, String> {
  let local = ctx.alloc_local();
  emit_literal(ctx, text)?;
  ctx.emit(Instruction::LocalSet(local));
  Ok(local)
}

fn emit_literal(ctx: &mut WasmGenCtx, text: &str) -> Result<(), String> {
  let ptr = ctx
    .string_pool
    .get(text)
    .ok_or_else(|| format!("internal Cirru EDN literal missing from string pool: {text:?}"))?;
  ctx.emit(f64_const(*ptr as f64));
  Ok(())
}

fn concat_with_literal(ctx: &mut WasmGenCtx, prefix: &str, value: u32) -> Result<u32, String> {
  let prefix = literal_local(ctx, prefix)?;
  Ok(concat_string_locals(ctx, prefix, value))
}

fn concat_with_literal_after(ctx: &mut WasmGenCtx, value: u32, suffix: &str) -> Result<u32, String> {
  let suffix = literal_local(ctx, suffix)?;
  Ok(concat_string_locals(ctx, value, suffix))
}

fn concat_string_locals(ctx: &mut WasmGenCtx, left: u32, right: u32) -> u32 {
  let left_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(left));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(left_ptr));
  let right_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(right));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(right_ptr));
  emit_edn_concat_limit_check(ctx, left_ptr, right_ptr);
  emit_str_concat_from_ptrs(ctx, left_ptr, right_ptr);
  let output = ctx.alloc_local();
  ctx.emit(Instruction::LocalSet(output));
  output
}

fn emit_edn_concat_limit_check(ctx: &mut WasmGenCtx, left_ptr: u32, right_ptr: u32) {
  let left_len = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(left_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(left_len));
  let right_len = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(right_ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(right_len));

  // Check before adding lengths so an i32 wrap cannot turn oversized output
  // into a small allocation. This bound is local to Cirru EDN formatting and
  // does not constrain ordinary Calcit string operations.
  ctx.emit(Instruction::LocalGet(right_len));
  ctx.emit(Instruction::I32Const(MAX_EDN_OUTPUT_BYTES));
  ctx.emit(Instruction::I32GtU);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(left_len));
  ctx.emit(Instruction::I32Const(MAX_EDN_OUTPUT_BYTES));
  ctx.emit(Instruction::LocalGet(right_len));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::I32GtU);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Empty));
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn container_modes_fail_closed_without_affecting_scalar_layout() {
    let container = CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::String));
    validate_edn_format_mode(&container, None).expect("default inline layout is supported");
    validate_edn_format_mode(&container, Some(&Calcit::Bool(true))).expect("literal inline layout is supported");
    let error = validate_edn_format_mode(&container, Some(&Calcit::Bool(false))).expect_err("non-inline layout is not implemented");
    assert!(error.starts_with("E_WASM_EDN_FORMAT_MODE:"));
    validate_edn_format_mode(&CalcitTypeAnnotation::String, Some(&Calcit::Bool(false)))
      .expect("scalar layout is independent of the flag");
  }

  #[test]
  fn formats_number_literals_with_native_cirru_edn_bytes() {
    for number in [1.25, -0.5, 1e-7, 1e20] {
      let expected = cirru_edn::format(&cirru_edn::Edn::Number(number), true).expect("native formatter should accept f64");
      assert_eq!(try_format_cirru_edn_literal(&Calcit::Number(number)), Some(expected));
    }
  }

  #[test]
  fn rejects_open_or_inexact_wasm_edn_schemas() {
    let dynamic_error = validate_edn_type(&CalcitTypeAnnotation::Dynamic, 0).expect_err("Dynamic must fail closed");
    assert!(dynamic_error.starts_with("E_WASM_EDN_TYPE:"));

    let number_error =
      validate_edn_type(&CalcitTypeAnnotation::Number, 0).expect_err("generic Number must not serialize as a placeholder");
    assert!(number_error.contains("not yet exact"));
  }

  #[test]
  fn accepts_closed_nested_map_and_struct_shapes() {
    let scalar_map = CalcitTypeAnnotation::Map(Arc::new(CalcitTypeAnnotation::Tag), Arc::new(CalcitTypeAnnotation::String));
    validate_edn_type(&scalar_map, 0).expect("closed scalar root Map should be supported");

    let nested_map = CalcitTypeAnnotation::List(Arc::new(scalar_map));
    validate_edn_type(&nested_map, 0).expect("nested closed Map should use parenthesized Cirru EDN");

    let container_value = CalcitTypeAnnotation::Map(
      Arc::new(CalcitTypeAnnotation::Tag),
      Arc::new(CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::String))),
    );
    validate_edn_type(&container_value, 0).expect("closed container Map values should be recursive");
  }

  #[test]
  fn bounds_recursive_wasm_edn_schemas() {
    let mut schema = Arc::new(CalcitTypeAnnotation::String);
    for _ in 0..=MAX_EDN_TYPE_DEPTH {
      schema = Arc::new(CalcitTypeAnnotation::List(schema));
    }
    let error = validate_edn_type(schema.as_ref(), 0).expect_err("deep schemas must be rejected");
    assert!(error.starts_with("E_WASM_EDN_DEPTH:"));
  }

  #[test]
  fn nested_option_formatters_avoid_exponential_emission() {
    let mut struct_def = CalcitStructDef::from_fields(
      cirru_edn::EdnTag::new("Option"),
      vec![cirru_edn::EdnTag::new("none"), cirru_edn::EdnTag::new("some")],
    );
    struct_def.definition_ref = Some(Arc::from("calcit.core/Option"));
    struct_def.generics = Arc::new(vec![Arc::from("T")]);
    let t_sym = Calcit::Symbol {
      sym: Arc::from("T"),
      info: Arc::new(crate::wasm::host::calcit::CalcitSymbolInfo {
        at_ns: Arc::from("calcit.core"),
        at_def: Arc::from("Option"),
      }),
      location: None,
    };
    let option_enum = Arc::new(
      CalcitEnumDef::from_struct(crate::wasm::host::calcit::CalcitStructValue {
        struct_ref: Arc::new(struct_def),
        values: Arc::new(vec![Calcit::from(vec![]), Calcit::from(vec![t_sym])]),
      })
      .expect("valid Option enum def"),
    );

    let mut nested_12 = Arc::new(CalcitTypeAnnotation::String);
    for _ in 0..12 {
      nested_12 = Arc::new(CalcitTypeAnnotation::Enum(option_enum.clone(), Arc::new(vec![nested_12])));
    }
    let mut nested_24 = nested_12.clone();
    for _ in 0..12 {
      nested_24 = Arc::new(CalcitTypeAnnotation::Enum(option_enum.clone(), Arc::new(vec![nested_24])));
    }

    let mut string_pool = HashMap::new();
    for (idx, lit) in [
      "[", "]", ",", "none", "some", "Option", "(%::", "%::", "'Option", "'none", "'some", " ", ")",
    ]
    .into_iter()
    .enumerate()
    {
      string_pool.insert(lit.to_owned(), (super::HEAP_BASE as u32) + 8 + (idx as u32) * 32);
    }

    let env = WasmCompileEnv {
      function_value_dependencies: Default::default(),
      dyn_traits: Default::default(),
      fn_index: HashMap::new(),
      fn_arity: HashMap::new(),
      fn_has_rest: HashMap::new(),
      fn_signatures: HashMap::new(),
      has_boxed_json_scalars: true,
      runtime_fn_index: HashMap::from([("__rt_json_string".to_owned(), 0), ("__rt_edn_string".to_owned(), 1)]),
      tag_index: HashMap::from([
        ("string".to_owned(), 0),
        ("enum".to_owned(), 1),
        ("Option".to_owned(), 2),
        ("none".to_owned(), 3),
        ("some".to_owned(), 4),
      ]),
      struct_layouts: Default::default(),
      string_pool,
      atom_globals: HashMap::new(),
      atom_is_scalar: HashMap::new(),
      value_imports: HashMap::new(),
      static_fn_defs: HashMap::new(),
      fn_table_index: HashMap::new(),
      host_imports: HashMap::new(),
      target: WasmTarget::Core,
      boundary: WasmBoundary::Native,
      world: WasiWorld::Command,
      non_coherent_nominal_defs: Default::default(),
      any_type_table: Default::default(),
    };

    let mut json_12 = WasmGenCtx::new(1, env.clone());
    emit_json_value(&mut json_12, 0, &nested_12, 0, false, "json-stringify").expect("12-deep JSON Option must emit");
    let mut json_24 = WasmGenCtx::new(1, env.clone());
    emit_json_value(&mut json_24, 0, &nested_24, 0, false, "json-stringify").expect("24-deep JSON Option must emit linearly");
    assert!(
      json_24.instructions.len() < json_12.instructions.len() * 3 && json_24.instructions.len() < 40_000,
      "24-deep JSON Option should scale linearly: len12={}, len24={}",
      json_12.instructions.len(),
      json_24.instructions.len()
    );

    let mut edn_nested_12 = WasmGenCtx::new(1, env.clone());
    emit_edn_value(&mut edn_nested_12, 0, &nested_12, 0, true).expect("12-deep nested EDN Option must emit");
    let mut edn_nested_24 = WasmGenCtx::new(1, env.clone());
    emit_edn_value(&mut edn_nested_24, 0, &nested_24, 0, true).expect("24-deep nested EDN Option must emit linearly");
    assert!(
      edn_nested_24.instructions.len() < edn_nested_12.instructions.len() * 3 && edn_nested_24.instructions.len() < 50_000,
      "24-deep nested EDN Option should scale linearly: len12={}, len24={}",
      edn_nested_12.instructions.len(),
      edn_nested_24.instructions.len()
    );

    let mut edn_12 = WasmGenCtx::new(1, env.clone());
    emit_edn_value(&mut edn_12, 0, &nested_12, 0, false).expect("12-deep root EDN Option must emit");
    let mut edn_24 = WasmGenCtx::new(1, env);
    emit_edn_value(&mut edn_24, 0, &nested_24, 0, false).expect("24-deep root EDN Option must emit without exponential blowup");
    assert!(
      edn_24.instructions.len() < edn_12.instructions.len() * 5 && edn_24.instructions.len() < 650_000,
      "24-deep root EDN Option should avoid exponential growth: len12={}, len24={}",
      edn_12.instructions.len(),
      edn_24.instructions.len()
    );
  }
}
