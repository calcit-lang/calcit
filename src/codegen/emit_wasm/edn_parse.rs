use super::*;

const MAX_EDN_INPUT_BYTES: i32 = 64 * 1024;
const INPUT_LIMIT_ERROR: &str = "E_WASM_EDN_INPUT_LIMIT: Cirru EDN input exceeds 65536 bytes";
const SYNTAX_ERROR: &str = "E_WASM_EDN_SYNTAX: invalid Cirru EDN scalar";
const RANGE_ERROR: &str = "E_WASM_EDN_RANGE: numeric value is outside the requested type";
const TAG_ERROR: &str = "E_WASM_EDN_TAG: tag is not present in the compiled program";

pub(super) fn emit_try_parse_cirru_edn_as(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 3 {
    return Err(format!(
      "try-parse-cirru-edn-as expects preprocessed text, type and data shape arguments, got {}",
      args.len()
    ));
  }
  let graph = DataShapeGraph::from_calcit_handle(&args[2])
    .ok_or_else(|| "E_WASM_EDN_SHAPE: typed Cirru EDN parser is missing its preprocessed data shape".to_string())?;
  let node = graph
    .nodes
    .get(graph.root)
    .ok_or_else(|| "E_WASM_EDN_SHAPE: typed Cirru EDN parser has an invalid root node".to_string())?;
  validate_scalar_shape(node)?;

  emit_expr(ctx, &args[0])?;
  let input = ctx.alloc_local();
  ctx.emit(Instruction::LocalSet(input));
  let input_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(input));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(input_ptr));
  let input_len = load_string_len(ctx, input_ptr);

  let result = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(input_len));
  ctx.emit(Instruction::I32Const(MAX_EDN_INPUT_BYTES));
  ctx.emit(Instruction::I32GtU);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  let error = literal_local(ctx, INPUT_LIMIT_ERROR)?;
  emit_result_enum(ctx, "err", error)?;
  ctx.emit(Instruction::Else);

  ctx.emit(Instruction::LocalGet(input));
  ctx.call_rt("__rt_trim_ws");
  let token = ctx.alloc_local();
  ctx.emit(Instruction::LocalSet(token));
  let token_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(token));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(token_ptr));

  // Native Cirru EDN accepts both a bare scalar token and the formatter's
  // top-level `do <token>` representation. Normalize that wrapper here.
  if_then_string_prefix(ctx, token_ptr, "do ", |ctx| {
    let len = load_string_len(ctx, token_ptr);
    let sliced = slice_string(ctx, token_ptr, 3, len);
    ctx.emit(Instruction::LocalGet(sliced));
    ctx.emit(Instruction::F64ConvertI32U);
    ctx.call_rt("__rt_trim_ws");
    ctx.emit(Instruction::LocalSet(token));
    ctx.emit(Instruction::LocalGet(token));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(token_ptr));
    Ok(())
  })?;

  let parsed = ctx.alloc_local();
  let ok = ctx.alloc_local_typed(ValType::I32);
  let error_kind = ctx.alloc_local_typed(ValType::I32); // 0 syntax, 1 range, 2 tag
  emit_parse_scalar(ctx, node, token, token_ptr, parsed, ok, error_kind)?;

  ctx.emit(Instruction::LocalGet(ok));
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_result_enum(ctx, "ok", parsed)?;
  ctx.emit(Instruction::Else);
  let parse_error = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(error_kind));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_literal(ctx, RANGE_ERROR)?;
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(error_kind));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_literal(ctx, TAG_ERROR)?;
  ctx.emit(Instruction::Else);
  emit_literal(ctx, SYNTAX_ERROR)?;
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalSet(parse_error));
  emit_result_enum(ctx, "err", parse_error)?;
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

fn validate_scalar_shape(node: &DataShapeNode) -> Result<(), String> {
  match node {
    DataShapeNode::Nil
    | DataShapeNode::Unit
    | DataShapeNode::Bool
    | DataShapeNode::Number
    | DataShapeNode::Numeric(_)
    | DataShapeNode::String
    | DataShapeNode::Tag => Ok(()),
    other => Err(format!(
      "E_WASM_EDN_SHAPE: {other:?} is not yet supported by the typed Cirru EDN scalar parser"
    )),
  }
}

fn emit_parse_scalar(
  ctx: &mut WasmGenCtx,
  node: &DataShapeNode,
  token: u32,
  token_ptr: u32,
  parsed: u32,
  ok: u32,
  error_kind: u32,
) -> Result<(), String> {
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(ok));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(error_kind));
  match node {
    DataShapeNode::Nil => {
      set_if_literal(ctx, token_ptr, "nil", parsed, ok, 0.0)?;
    }
    // Cirru EDN has nil but no representation for Calcit's distinct Unit.
    // Keep the shape compilable for the safe API and return a stable error.
    DataShapeNode::Unit => {}
    DataShapeNode::Bool => {
      set_if_literal(ctx, token_ptr, "true", parsed, ok, 1.0)?;
      set_if_literal(ctx, token_ptr, "false", parsed, ok, 0.0)?;
    }
    DataShapeNode::String => emit_parse_string(ctx, token_ptr, parsed, ok),
    DataShapeNode::Tag => emit_parse_tag(ctx, token_ptr, parsed, ok, error_kind)?,
    DataShapeNode::Number => emit_parse_number(ctx, token, token_ptr, None, parsed, ok, error_kind),
    DataShapeNode::Numeric(kind) => emit_parse_number(ctx, token, token_ptr, Some(*kind), parsed, ok, error_kind),
    _ => unreachable!("shape was validated before scalar parser emission"),
  }
  Ok(())
}

fn set_if_literal(ctx: &mut WasmGenCtx, token_ptr: u32, literal: &str, parsed: u32, ok: u32, value: f64) -> Result<(), String> {
  emit_string_equals_literal(ctx, token_ptr, literal)?;
  ctx.begin_block_if();
  ctx.emit(f64_const(value));
  ctx.emit(Instruction::LocalSet(parsed));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(ok));
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_parse_tag(ctx: &mut WasmGenCtx, token_ptr: u32, parsed: u32, ok: u32, error_kind: u32) -> Result<(), String> {
  let mut tags = ctx.tag_index.iter().map(|(name, id)| (name.clone(), *id)).collect::<Vec<_>>();
  tags.sort_by(|a, b| a.0.cmp(&b.0));
  for (name, id) in tags {
    emit_string_equals_literal(ctx, token_ptr, &format!(":{name}"))?;
    ctx.begin_block_if();
    ctx.emit(f64_const(id as f64));
    ctx.emit(Instruction::LocalSet(parsed));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::LocalSet(ok));
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::LocalGet(ok));
  ctx.emit(Instruction::I32Eqz);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::LocalSet(error_kind));
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_parse_string(ctx: &mut WasmGenCtx, token_ptr: u32, parsed: u32, ok: u32) {
  let len = load_string_len(ctx, token_ptr);
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32GeU);
  ctx.begin_block_if();
  let first = load_byte(ctx, token_ptr, 0);
  ctx.emit(Instruction::LocalGet(first));
  ctx.emit(Instruction::I32Const(b'|' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  let sliced = slice_string(ctx, token_ptr, 1, len);
  ctx.emit(Instruction::LocalGet(sliced));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(parsed));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(ok));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  // Quoted Cirru leaf: `"|..."`. Decode the four escapes emitted by the
  // formatter (`\n`, `\t`, `\"`, `\\`) directly into a bounded string.
  ctx.emit(Instruction::LocalGet(ok));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32Const(3));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::I32And);
  ctx.begin_block_if();
  let quote = load_byte(ctx, token_ptr, 0);
  let marker = load_byte(ctx, token_ptr, 1);
  let last_index = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(last_index));
  let last = load_byte_at(ctx, token_ptr, last_index);
  ctx.emit(Instruction::LocalGet(quote));
  ctx.emit(Instruction::I32Const(b'"' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::LocalGet(marker));
  ctx.emit(Instruction::I32Const(b'|' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalGet(last));
  ctx.emit(Instruction::I32Const(b'"' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::I32And);
  ctx.begin_block_if();

  let max_output_len = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32Const(3));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(max_output_len));
  let (output, output_bytes) = emit_str_alloc(ctx, max_output_len);
  let input_index = ctx.alloc_local_typed(ValType::I32);
  let output_index = ctx.alloc_local_typed(ValType::I32);
  let byte = ctx.alloc_local_typed(ValType::I32);
  let valid = ctx.alloc_local_typed(ValType::I32);
  let accepted_escape = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::LocalSet(input_index));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(output_index));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(valid));

  ctx.begin_block();
  ctx.begin_loop();
  ctx.emit(Instruction::LocalGet(input_index));
  ctx.emit(Instruction::LocalGet(last_index));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  let current = load_byte_at(ctx, token_ptr, input_index);
  ctx.emit(Instruction::LocalGet(current));
  ctx.emit(Instruction::LocalSet(byte));
  ctx.emit(Instruction::LocalGet(byte));
  ctx.emit(Instruction::I32Const(b'\\' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  ctx.i32_inc(input_index);
  ctx.emit(Instruction::LocalGet(input_index));
  ctx.emit(Instruction::LocalGet(last_index));
  ctx.emit(Instruction::I32GeU);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::Else);
  let escaped = load_byte_at(ctx, token_ptr, input_index);
  ctx.emit(Instruction::LocalGet(escaped));
  ctx.emit(Instruction::LocalSet(byte));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(accepted_escape));

  ctx.emit(Instruction::LocalGet(byte));
  ctx.emit(Instruction::I32Const(b'n' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(b'\n' as i32));
  ctx.emit(Instruction::LocalSet(byte));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(accepted_escape));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(byte));
  ctx.emit(Instruction::I32Const(b't' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(b'\t' as i32));
  ctx.emit(Instruction::LocalSet(byte));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(accepted_escape));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(byte));
  ctx.emit(Instruction::I32Const(b'"' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::LocalGet(byte));
  ctx.emit(Instruction::I32Const(b'\\' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::I32Or);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(accepted_escape));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(accepted_escape));
  ctx.emit(Instruction::I32Eqz);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(valid));
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(output_bytes));
  ctx.emit(Instruction::LocalGet(output_index));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(byte));
  ctx.emit(Instruction::I32Store8(mem_arg_byte(0)));
  ctx.i32_inc(output_index);
  ctx.emit(Instruction::End);
  ctx.i32_inc(input_index);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(valid));
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(output));
  ctx.emit(Instruction::LocalGet(output_index));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(output));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(parsed));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(ok));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
}

fn emit_parse_number(
  ctx: &mut WasmGenCtx,
  token: u32,
  token_ptr: u32,
  refinement: Option<CalcitNumericRefinement>,
  parsed: u32,
  ok: u32,
  error_kind: u32,
) {
  let len = load_string_len(ctx, token_ptr);
  let index = ctx.alloc_local_typed(ValType::I32);
  let digits = ctx.alloc_local_typed(ValType::I32);
  let dot_seen = ctx.alloc_local_typed(ValType::I32);
  let valid = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(index));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(digits));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(dot_seen));

  // Optional leading sign.
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::I32GtU);
  ctx.begin_block_if();
  let first = load_byte(ctx, token_ptr, 0);
  ctx.emit(Instruction::LocalGet(first));
  ctx.emit(Instruction::I32Const(b'-' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::LocalGet(first));
  ctx.emit(Instruction::I32Const(b'+' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::I32Or);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(index));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.begin_block();
  ctx.begin_loop();
  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  let byte = load_byte_at(ctx, token_ptr, index);
  ctx.emit(Instruction::LocalGet(byte));
  ctx.emit(Instruction::I32Const(b'0' as i32));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::LocalGet(byte));
  ctx.emit(Instruction::I32Const(b'9' as i32));
  ctx.emit(Instruction::I32LeU);
  ctx.emit(Instruction::I32And);
  ctx.begin_block_if();
  ctx.i32_inc(digits);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(byte));
  ctx.emit(Instruction::I32Const(b'.' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::LocalGet(dot_seen));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::I32And);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(dot_seen));
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.i32_inc(index);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(digits));
  ctx.emit(Instruction::I32Eqz);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(valid));
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(token));
  ctx.call_rt("__rt_parse_float");
  ctx.emit(Instruction::LocalSet(parsed));

  if let Some(kind) = refinement {
    let in_range = ctx.alloc_local_typed(ValType::I32);
    emit_numeric_refinement_check(ctx, parsed, kind, in_range);
    ctx.emit(Instruction::LocalGet(in_range));
    ctx.begin_block_if();
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::LocalSet(ok));
    ctx.emit(Instruction::Else);
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::LocalSet(error_kind));
    ctx.emit(Instruction::End);
  } else {
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::LocalSet(ok));
  }
  ctx.emit(Instruction::End);
}

fn emit_numeric_refinement_check(ctx: &mut WasmGenCtx, parsed: u32, kind: CalcitNumericRefinement, output: u32) {
  let bounds = match kind {
    CalcitNumericRefinement::Int8 => Some((-128.0, 127.0)),
    CalcitNumericRefinement::UInt8 => Some((0.0, 255.0)),
    CalcitNumericRefinement::Int16 => Some((-32768.0, 32767.0)),
    CalcitNumericRefinement::UInt16 => Some((0.0, 65535.0)),
    CalcitNumericRefinement::Int32 => Some((-2147483648.0, 2147483647.0)),
    CalcitNumericRefinement::UInt32 => Some((0.0, 4294967295.0)),
    CalcitNumericRefinement::Int64 => Some((-9007199254740991.0, 9007199254740991.0)),
    CalcitNumericRefinement::UInt64 => Some((0.0, 9007199254740991.0)),
    CalcitNumericRefinement::Float32 | CalcitNumericRefinement::Float64 => None,
  };
  if let Some((min, max)) = bounds {
    ctx.emit(Instruction::LocalGet(parsed));
    ctx.emit(Instruction::LocalGet(parsed));
    ctx.emit(Instruction::F64Trunc);
    ctx.emit(Instruction::F64Eq);
    ctx.emit(Instruction::LocalGet(parsed));
    ctx.emit(f64_const(min));
    ctx.emit(Instruction::F64Ge);
    ctx.emit(Instruction::I32And);
    ctx.emit(Instruction::LocalGet(parsed));
    ctx.emit(f64_const(max));
    ctx.emit(Instruction::F64Le);
    ctx.emit(Instruction::I32And);
  } else if matches!(kind, CalcitNumericRefinement::Float32) {
    ctx.emit(Instruction::LocalGet(parsed));
    ctx.emit(Instruction::LocalGet(parsed));
    ctx.emit(Instruction::F32DemoteF64);
    ctx.emit(Instruction::F64PromoteF32);
    ctx.emit(Instruction::F64Eq);
  } else {
    ctx.emit(Instruction::I32Const(1));
  }
  ctx.emit(Instruction::LocalSet(output));
}

fn if_then_string_prefix<F>(ctx: &mut WasmGenCtx, ptr: u32, prefix: &str, body: F) -> Result<(), String>
where
  F: FnOnce(&mut WasmGenCtx) -> Result<(), String>,
{
  let prefix_ptr = literal_ptr(ctx, prefix)?;
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(prefix_ptr as i32));
  ctx.call_rt("__rt_str_starts_with");
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Ne);
  ctx.begin_block_if();
  body(ctx)?;
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_string_equals_literal(ctx: &mut WasmGenCtx, ptr: u32, literal: &str) -> Result<(), String> {
  let literal_ptr = literal_ptr(ctx, literal)?;
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(literal_ptr as i32));
  ctx.call_rt("__rt_str_compare");
  ctx.emit(f64_const(0.0));
  ctx.emit(Instruction::F64Eq);
  Ok(())
}

fn literal_ptr(ctx: &WasmGenCtx, text: &str) -> Result<u32, String> {
  ctx
    .string_pool
    .get(text)
    .copied()
    .ok_or_else(|| format!("internal typed Cirru EDN literal missing from string pool: {text:?}"))
}

fn emit_literal(ctx: &mut WasmGenCtx, text: &str) -> Result<(), String> {
  ctx.emit(f64_const(literal_ptr(ctx, text)? as f64));
  Ok(())
}

fn literal_local(ctx: &mut WasmGenCtx, text: &str) -> Result<u32, String> {
  let local = ctx.alloc_local();
  emit_literal(ctx, text)?;
  ctx.emit(Instruction::LocalSet(local));
  Ok(local)
}

fn load_string_len(ctx: &mut WasmGenCtx, ptr: u32) -> u32 {
  let len = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(len));
  len
}

fn load_byte(ctx: &mut WasmGenCtx, ptr: u32, offset: i32) -> u32 {
  let byte = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(8 + offset));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  ctx.emit(Instruction::LocalSet(byte));
  byte
}

fn load_byte_at(ctx: &mut WasmGenCtx, ptr: u32, index: u32) -> u32 {
  let byte = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(index));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  ctx.emit(Instruction::LocalSet(byte));
  byte
}

fn slice_string(ctx: &mut WasmGenCtx, ptr: u32, start: i32, end: u32) -> u32 {
  let len = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(end));
  ctx.emit(Instruction::I32Const(start));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(len));
  let (output, content) = emit_str_alloc(ctx, len);
  ctx.emit(Instruction::LocalGet(content));
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(8 + start));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
  output
}
