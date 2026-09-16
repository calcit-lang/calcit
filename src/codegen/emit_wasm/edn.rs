use super::*;
use crate::runner::preprocess::infer_static_type_from_expr;

const MAX_EDN_TYPE_DEPTH: usize = 32;
pub(super) const MAX_EDN_OUTPUT_BYTES: i32 = 64 * 1024;

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

  let value_type = infer_static_type_from_expr(&args[0])
    .ok_or_else(|| "E_WASM_EDN_TYPE: format-cirru-edn requires a closed inferred input type for WASM".to_string())?;

  emit_expr(ctx, &args[0])?;
  let value = ctx.alloc_local();
  ctx.emit(Instruction::LocalSet(value));

  // Preserve eager argument evaluation even though the native implementation
  // currently ignores the compatibility formatting flag.
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

pub(super) fn try_format_cirru_edn_literal(value: &Calcit) -> Option<String> {
  match value {
    Calcit::Number(number) => cirru_edn::format(&cirru_edn::Edn::Number(*number), true).ok(),
    _ => None,
  }
}

fn is_edn_scalar(value_type: &CalcitTypeAnnotation) -> bool {
  !matches!(value_type, CalcitTypeAnnotation::List(_) | CalcitTypeAnnotation::Map(_, _))
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
    CalcitTypeAnnotation::Nil => literal_local(ctx, "nil"),
    CalcitTypeAnnotation::Bool => {
      let result = ctx.alloc_local();
      ctx.emit(Instruction::LocalGet(value));
      ctx.emit(f64_const(0.0));
      ctx.emit(Instruction::F64Eq);
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

fn is_supported_map_scalar(value_type: &CalcitTypeAnnotation) -> bool {
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
    other if is_supported_map_scalar(other) => {
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

fn emit_edn_map(
  ctx: &mut WasmGenCtx,
  value: u32,
  key_type: &CalcitTypeAnnotation,
  value_type: &CalcitTypeAnnotation,
  depth: usize,
  nested: bool,
) -> Result<u32, String> {
  if nested || depth > 0 {
    return Err("E_WASM_EDN_MAP_SHAPE: nested Map formatting is not yet byte-identical to native Cirru layout".into());
  }
  if !is_supported_map_scalar(key_type) {
    return Err(format!(
      "E_WASM_EDN_MAP_KEY: {key_type} cannot preserve native Cirru EDN map ordering in WASM"
    ));
  }
  if !is_supported_map_scalar(value_type) {
    return Err(format!(
      "E_WASM_EDN_MAP_VALUE: {value_type} is not yet supported by the byte-identical scalar Map formatter"
    ));
  }
  validate_edn_type(key_type, depth + 1)?;
  validate_edn_type(value_type, depth + 1)?;

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
  emit_map_key_less_than(ctx, key, previous_key, key_type)?;
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

  let result = literal_local(ctx, "{}")?;
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
  let formatted_key = emit_edn_value(ctx, key, key_type, depth + 1, true)?;
  let with_key = concat_string_locals(ctx, result, formatted_key);
  ctx.emit(Instruction::LocalGet(with_key));
  ctx.emit(Instruction::LocalSet(result));
  let with_space = concat_with_literal_after(ctx, result, " ")?;
  ctx.emit(Instruction::LocalGet(with_space));
  ctx.emit(Instruction::LocalSet(result));
  let formatted_value = emit_edn_value(ctx, item_value, value_type, depth + 1, true)?;
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
    CalcitTypeAnnotation::Nil | CalcitTypeAnnotation::Bool | CalcitTypeAnnotation::String | CalcitTypeAnnotation::Tag => Ok(()),
    CalcitTypeAnnotation::Numeric(kind) if !matches!(kind, CalcitNumericRefinement::Float32 | CalcitNumericRefinement::Float64) => {
      Ok(())
    }
    CalcitTypeAnnotation::List(item) => validate_edn_type(item, depth + 1),
    CalcitTypeAnnotation::Map(key, value) if depth == 0 => {
      if !is_supported_map_scalar(key) {
        return Err(format!(
          "E_WASM_EDN_MAP_KEY: {key} cannot preserve native Cirru EDN map ordering in WASM"
        ));
      }
      if !is_supported_map_scalar(value) {
        return Err(format!(
          "E_WASM_EDN_MAP_VALUE: {value} is not yet supported by the byte-identical scalar Map formatter"
        ));
      }
      validate_edn_type(key, depth + 1)?;
      validate_edn_type(value, depth + 1)
    }
    CalcitTypeAnnotation::Map(_, _) => {
      Err("E_WASM_EDN_MAP_SHAPE: nested Map formatting is not yet byte-identical to native Cirru layout".into())
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
  fn accepts_only_byte_identical_scalar_root_maps() {
    let scalar_map = CalcitTypeAnnotation::Map(Arc::new(CalcitTypeAnnotation::Tag), Arc::new(CalcitTypeAnnotation::String));
    validate_edn_type(&scalar_map, 0).expect("closed scalar root Map should be supported");

    let nested_map = CalcitTypeAnnotation::List(Arc::new(scalar_map));
    let nested_error = validate_edn_type(&nested_map, 0).expect_err("nested Map layout must fail closed");
    assert!(nested_error.starts_with("E_WASM_EDN_MAP_SHAPE:"));

    let container_value = CalcitTypeAnnotation::Map(
      Arc::new(CalcitTypeAnnotation::Tag),
      Arc::new(CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::String))),
    );
    let value_error = validate_edn_type(&container_value, 0).expect_err("container Map values must fail closed");
    assert!(value_error.starts_with("E_WASM_EDN_MAP_VALUE:"));
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
}
