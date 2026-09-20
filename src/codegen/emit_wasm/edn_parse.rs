use super::*;
use cirru_edn::EdnTag;

const MAX_EDN_INPUT_BYTES: i32 = 64 * 1024;
const MAX_EDN_LIST_ITEMS: i32 = 4096;
const MAX_EDN_MAP_ENTRIES: i32 = 2048;
const MAX_EDN_DECODE_SHAPE_DEPTH: usize = 32;
const INPUT_LIMIT_ERROR: &str = "E_WASM_EDN_INPUT_LIMIT: Cirru EDN input exceeds 65536 bytes";
const SYNTAX_ERROR: &str = "E_WASM_EDN_SYNTAX: invalid Cirru EDN scalar";
const RANGE_ERROR: &str = "E_WASM_EDN_RANGE: numeric value is outside the requested type";
const TAG_ERROR: &str = "E_WASM_EDN_TAG: tag is not present in the compiled program";
const TOKEN_LIMIT_ERROR: &str = "E_WASM_EDN_TOKEN_LIMIT: Cirru EDN list exceeds 4096 items";
const MAP_LIMIT_ERROR: &str = "E_WASM_EDN_MAP_LIMIT: Cirru EDN map exceeds 2048 entries";
const ENUM_ERROR: &str = "E_WASM_EDN_ENUM: Cirru EDN enum type, variant, or payload does not match the requested type";

pub(super) fn emit_try_parse_cirru_edn_as(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 3 {
    return Err(format!(
      "try-parse-cirru-edn-as expects preprocessed text, type and data shape arguments, got {}",
      args.len()
    ));
  }
  let graph = DataShapeGraph::from_calcit_handle(&args[2])
    .ok_or_else(|| "E_WASM_EDN_SHAPE: typed Cirru EDN parser is missing its preprocessed data shape".to_string())?;
  validate_decode_shape(&graph, graph.root, 0)?;

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
  let error_kind = ctx.alloc_local_typed(ValType::I32); // 0 syntax, 1 range, 2 tag, 3 list limit, 4 map limit, 5 enum
  emit_parse_node(ctx, &graph, graph.root, token, token_ptr, (parsed, ok, error_kind))?;

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
  ctx.emit(Instruction::LocalGet(error_kind));
  ctx.emit(Instruction::I32Const(3));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_literal(ctx, TOKEN_LIMIT_ERROR)?;
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(error_kind));
  ctx.emit(Instruction::I32Const(4));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_literal(ctx, MAP_LIMIT_ERROR)?;
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(error_kind));
  ctx.emit(Instruction::I32Const(5));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_literal(ctx, ENUM_ERROR)?;
  ctx.emit(Instruction::Else);
  emit_literal(ctx, SYNTAX_ERROR)?;
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
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

fn validate_decode_shape(graph: &DataShapeGraph, node_id: usize, depth: usize) -> Result<(), String> {
  if depth > MAX_EDN_DECODE_SHAPE_DEPTH {
    return Err(format!(
      "E_WASM_EDN_DEPTH: typed Cirru EDN decode shape nesting exceeds {MAX_EDN_DECODE_SHAPE_DEPTH}"
    ));
  }
  let node = graph
    .nodes
    .get(node_id)
    .ok_or_else(|| format!("E_WASM_EDN_SHAPE: typed Cirru EDN parser references missing node #{node_id}"))?;
  match node {
    DataShapeNode::Nil
    | DataShapeNode::Unit
    | DataShapeNode::Bool
    | DataShapeNode::Number
    | DataShapeNode::Numeric(_)
    | DataShapeNode::String
    | DataShapeNode::Tag => Ok(()),
    DataShapeNode::List(item) => validate_decode_shape(graph, *item, depth + 1),
    DataShapeNode::Map { key, value } => {
      validate_scalar_map_key_shape(graph, *key)?;
      validate_decode_shape(graph, *value, depth + 1)
    }
    DataShapeNode::Struct { fields, .. } => {
      for (_, field) in fields {
        validate_decode_shape(graph, *field, depth + 1)?;
      }
      Ok(())
    }
    DataShapeNode::Enum { variants, .. } => {
      for (_, payloads) in variants {
        for payload in payloads {
          validate_decode_shape(graph, *payload, depth + 1)?;
        }
      }
      Ok(())
    }
    other => Err(format!(
      "E_WASM_EDN_SHAPE: {other:?} is not yet supported by the typed Cirru EDN parser"
    )),
  }
}

fn validate_scalar_map_key_shape(graph: &DataShapeGraph, node_id: usize) -> Result<(), String> {
  let node = graph
    .nodes
    .get(node_id)
    .ok_or_else(|| format!("E_WASM_EDN_SHAPE: typed Cirru EDN parser references missing node #{node_id}"))?;
  match node {
    DataShapeNode::Nil
    | DataShapeNode::Unit
    | DataShapeNode::Bool
    | DataShapeNode::Number
    | DataShapeNode::Numeric(_)
    | DataShapeNode::String
    | DataShapeNode::Tag => Ok(()),
    other => Err(format!(
      "E_WASM_EDN_MAP_KEY: {other:?} cannot preserve native Cirru EDN map ordering in WASM"
    )),
  }
}

/// Emit a parser specialized to a closed compiler-owned data-shape node.
/// Nested values stay statically typed: this dispatch is generated while
/// compiling and never constructs a Dynamic EDN tree at runtime.
fn emit_parse_node(
  ctx: &mut WasmGenCtx,
  graph: &DataShapeGraph,
  node_id: usize,
  token: u32,
  token_ptr: u32,
  outputs: (u32, u32, u32),
) -> Result<(), String> {
  let (parsed, ok, error_kind) = outputs;
  let node = graph
    .nodes
    .get(node_id)
    .ok_or_else(|| format!("E_WASM_EDN_SHAPE: typed Cirru EDN parser references missing node #{node_id}"))?;
  let (token, token_ptr) = emit_unwrap_nested_token(ctx, token, token_ptr);
  match node {
    DataShapeNode::List(item) => emit_parse_scalar_list(ctx, graph, *item, token_ptr, parsed, ok, error_kind),
    DataShapeNode::Map { key, value } => emit_parse_scalar_map(ctx, graph, (*key, *value), token_ptr, (parsed, ok, error_kind)),
    DataShapeNode::Struct { nominal, fields, .. } => {
      emit_parse_scalar_struct(ctx, graph, nominal, fields, token_ptr, (parsed, ok, error_kind))
    }
    DataShapeNode::Enum { nominal, variants, .. } => {
      emit_parse_scalar_enum(ctx, graph, nominal, variants, token_ptr, (parsed, ok, error_kind))
    }
    _ => emit_parse_scalar(ctx, node, token, token_ptr, parsed, ok, error_kind),
  }
}

/// Formatter-produced nested EDN values are parenthesized (`([] ...)`,
/// `({} ...)`, `(%{} ...)`). Remove exactly that outer expression wrapper
/// before dispatching to the specialized collection parser.
fn emit_unwrap_nested_token(ctx: &mut WasmGenCtx, token: u32, token_ptr: u32) -> (u32, u32) {
  let result = ctx.alloc_local();
  let result_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(token));
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::LocalGet(token_ptr));
  ctx.emit(Instruction::LocalSet(result_ptr));
  let len = load_string_len(ctx, token_ptr);
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32GtU);
  ctx.begin_block_if();
  let first = load_byte(ctx, token_ptr, 0);
  let last_index = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(last_index));
  let last = load_byte_at(ctx, token_ptr, last_index);
  ctx.emit(Instruction::LocalGet(first));
  ctx.emit(Instruction::I32Const(b'(' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::LocalGet(last));
  ctx.emit(Instruction::I32Const(b')' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::I32And);
  ctx.begin_block_if();
  let inner = slice_string(ctx, token_ptr, 1, last_index);
  ctx.emit(Instruction::LocalGet(inner));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::LocalGet(inner));
  ctx.emit(Instruction::LocalSet(result_ptr));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  (result, result_ptr)
}

fn emit_parse_scalar_struct(
  ctx: &mut WasmGenCtx,
  graph: &DataShapeGraph,
  nominal: &Arc<CalcitStructDef>,
  fields: &[(EdnTag, usize)],
  token_ptr: u32,
  outputs: (u32, u32, u32),
) -> Result<(), String> {
  let (parsed, ok, error_kind) = outputs;
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(ok));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(error_kind));
  let valid = ctx.alloc_i32(1);
  let len = load_string_len(ctx, token_ptr);

  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32Const(3));
  ctx.emit(Instruction::I32GtU);
  ctx.emit(Instruction::LocalSet(valid));
  for (offset, expected) in b"%{}".iter().copied().enumerate() {
    ctx.emit(Instruction::LocalGet(valid));
    ctx.begin_block_if();
    let byte = load_byte(ctx, token_ptr, offset as i32);
    ctx.emit(Instruction::LocalGet(byte));
    ctx.emit(Instruction::I32Const(expected as i32));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::LocalSet(valid));
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::LocalGet(valid));
  ctx.begin_block_if();
  let delimiter = load_byte(ctx, token_ptr, 3);
  emit_ascii_whitespace(ctx, delimiter);
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::End);

  let cursor = ctx.alloc_i32(3);
  emit_skip_ascii_whitespace(ctx, token_ptr, len, cursor);
  let (name_start, name_end) = emit_scan_map_scalar_token(ctx, token_ptr, len, cursor, valid);
  ctx.emit(Instruction::LocalGet(valid));
  ctx.begin_block_if();
  let name_ptr = slice_string_range(ctx, token_ptr, name_start, name_end);
  emit_string_equals_literal(ctx, name_ptr, &format!("'{}", nominal.name))?;
  emit_string_equals_literal(ctx, name_ptr, &format!(":{}", nominal.name))?;
  ctx.emit(Instruction::I32Or);
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::End);

  let values = fields.iter().map(|_| ctx.alloc_local()).collect::<Vec<_>>();
  let seen = fields.iter().map(|_| ctx.alloc_i32(0)).collect::<Vec<_>>();
  let count = ctx.alloc_i32(0);
  emit_scan_scalar_map_entries(
    ctx,
    token_ptr,
    len,
    cursor,
    (valid, count, error_kind),
    |ctx, key_start, key_end, value_start, value_end, _index| {
      let key_ptr = slice_string_range(ctx, token_ptr, key_start, key_end);
      let value_ptr = slice_string_range(ctx, token_ptr, value_start, value_end);
      let value_token = ctx.alloc_local();
      ctx.emit(Instruction::LocalGet(value_ptr));
      ctx.emit(Instruction::F64ConvertI32U);
      ctx.emit(Instruction::LocalSet(value_token));
      let matched = ctx.alloc_i32(0);

      for (index, (field, node_id)) in fields.iter().enumerate() {
        emit_string_equals_literal(ctx, key_ptr, &format!(":{field}"))?;
        ctx.begin_block_if();
        ctx.emit(Instruction::I32Const(1));
        ctx.emit(Instruction::LocalSet(matched));
        ctx.emit(Instruction::LocalGet(seen[index]));
        ctx.begin_block_if();
        ctx.emit(Instruction::I32Const(0));
        ctx.emit(Instruction::LocalSet(valid));
        ctx.emit(Instruction::Else);
        let field_ok = ctx.alloc_local_typed(ValType::I32);
        let field_error = ctx.alloc_local_typed(ValType::I32);
        emit_parse_node(ctx, graph, *node_id, value_token, value_ptr, (values[index], field_ok, field_error))?;
        ctx.emit(Instruction::LocalGet(field_ok));
        ctx.begin_block_if();
        ctx.emit(Instruction::I32Const(1));
        ctx.emit(Instruction::LocalSet(seen[index]));
        ctx.emit(Instruction::Else);
        ctx.emit(Instruction::I32Const(0));
        ctx.emit(Instruction::LocalSet(valid));
        ctx.emit(Instruction::LocalGet(field_error));
        ctx.emit(Instruction::LocalSet(error_kind));
        ctx.emit(Instruction::End);
        ctx.emit(Instruction::End);
        ctx.emit(Instruction::End);
      }

      ctx.emit(Instruction::LocalGet(matched));
      ctx.emit(Instruction::I32Eqz);
      ctx.begin_block_if();
      ctx.emit(Instruction::I32Const(0));
      ctx.emit(Instruction::LocalSet(valid));
      ctx.emit(Instruction::End);
      Ok(())
    },
  )?;

  ctx.emit(Instruction::LocalGet(valid));
  for field_seen in &seen {
    ctx.emit(Instruction::LocalGet(*field_seen));
    ctx.emit(Instruction::I32And);
  }
  ctx.begin_block_if();
  let struct_tag_id = *ctx.tag_index.get(nominal.name.ref_str()).ok_or_else(|| {
    format!(
      "E_WASM_EDN_SHAPE: struct tag :{} is not present in the compiled program",
      nominal.name
    )
  })?;
  let struct_ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, ((2 + fields.len()) * 8) as i32, struct_ptr, "struct");
  ctx.emit(Instruction::LocalGet(struct_ptr));
  ctx.emit(f64_const(fields.len() as f64));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(struct_ptr));
  ctx.emit(f64_const(struct_tag_id as f64));
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
  for (index, value) in values.iter().enumerate() {
    ctx.emit(Instruction::LocalGet(struct_ptr));
    ctx.emit(Instruction::LocalGet(*value));
    ctx.emit(Instruction::F64Store(mem_arg_f64(((2 + index) * 8) as u64)));
  }
  ctx.emit(Instruction::LocalGet(struct_ptr));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(parsed));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(ok));
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_parse_scalar_enum(
  ctx: &mut WasmGenCtx,
  graph: &DataShapeGraph,
  nominal: &Arc<CalcitEnumDef>,
  variants: &[(EdnTag, Vec<usize>)],
  token_ptr: u32,
  outputs: (u32, u32, u32),
) -> Result<(), String> {
  let (parsed, ok, error_kind) = outputs;
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(ok));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(error_kind));
  let valid = ctx.alloc_i32(1);
  let len = load_string_len(ctx, token_ptr);

  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32Const(3));
  ctx.emit(Instruction::I32GtU);
  ctx.emit(Instruction::LocalSet(valid));
  for (offset, expected) in b"%::".iter().copied().enumerate() {
    ctx.emit(Instruction::LocalGet(valid));
    ctx.begin_block_if();
    let byte = load_byte(ctx, token_ptr, offset as i32);
    ctx.emit(Instruction::LocalGet(byte));
    ctx.emit(Instruction::I32Const(expected as i32));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::LocalSet(valid));
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::LocalGet(valid));
  ctx.begin_block_if();
  let delimiter = load_byte(ctx, token_ptr, 3);
  emit_ascii_whitespace(ctx, delimiter);
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::End);

  let max_payloads = variants.iter().map(|(_, payloads)| payloads.len()).max().unwrap_or(0);
  let starts = (0..(2 + max_payloads))
    .map(|_| ctx.alloc_local_typed(ValType::I32))
    .collect::<Vec<_>>();
  let ends = (0..(2 + max_payloads))
    .map(|_| ctx.alloc_local_typed(ValType::I32))
    .collect::<Vec<_>>();
  let count = ctx.alloc_i32(0);
  emit_scan_edn_tokens(ctx, token_ptr, len, 3, (valid, count, error_kind), |ctx, start, end, index| {
    for slot in 0..starts.len() {
      ctx.emit(Instruction::LocalGet(index));
      ctx.emit(Instruction::I32Const(slot as i32));
      ctx.emit(Instruction::I32Eq);
      ctx.begin_block_if();
      ctx.emit(Instruction::LocalGet(start));
      ctx.emit(Instruction::LocalSet(starts[slot]));
      ctx.emit(Instruction::LocalGet(end));
      ctx.emit(Instruction::LocalSet(ends[slot]));
      ctx.emit(Instruction::End);
    }
    Ok(())
  })?;

  ctx.emit(Instruction::LocalGet(valid));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::LocalGet(valid));
  ctx.begin_block_if();
  let type_ptr = slice_string_range(ctx, token_ptr, starts[0], ends[0]);
  emit_string_equals_literal(ctx, type_ptr, &format!("'{}", nominal.name()))?;
  emit_string_equals_literal(ctx, type_ptr, &format!(":{}", nominal.name()))?;
  ctx.emit(Instruction::I32Or);
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::LocalGet(valid));
  ctx.emit(Instruction::I32Eqz);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(5));
  ctx.emit(Instruction::LocalSet(error_kind));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  let matched = ctx.alloc_i32(0);
  ctx.emit(Instruction::LocalGet(valid));
  ctx.begin_block_if();
  let variant_ptr = slice_string_range(ctx, token_ptr, starts[1], ends[1]);
  for (variant, payload_nodes) in variants {
    emit_string_equals_literal(ctx, variant_ptr, &format!("'{variant}"))?;
    emit_string_equals_literal(ctx, variant_ptr, &format!(":{variant}"))?;
    ctx.emit(Instruction::I32Or);
    ctx.begin_block_if();
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::LocalSet(matched));
    ctx.emit(Instruction::LocalGet(count));
    ctx.emit(Instruction::I32Const((2 + payload_nodes.len()) as i32));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::LocalSet(valid));

    let values = payload_nodes.iter().map(|_| ctx.alloc_local()).collect::<Vec<_>>();
    for (payload_index, node_id) in payload_nodes.iter().enumerate() {
      ctx.emit(Instruction::LocalGet(valid));
      ctx.begin_block_if();
      let payload_ptr = slice_string_range(ctx, token_ptr, starts[2 + payload_index], ends[2 + payload_index]);
      let payload_token = ctx.alloc_local();
      ctx.emit(Instruction::LocalGet(payload_ptr));
      ctx.emit(Instruction::F64ConvertI32U);
      ctx.emit(Instruction::LocalSet(payload_token));
      let payload_ok = ctx.alloc_local_typed(ValType::I32);
      let payload_error = ctx.alloc_local_typed(ValType::I32);
      emit_parse_node(
        ctx,
        graph,
        *node_id,
        payload_token,
        payload_ptr,
        (values[payload_index], payload_ok, payload_error),
      )?;
      ctx.emit(Instruction::LocalGet(payload_ok));
      ctx.emit(Instruction::I32Eqz);
      ctx.begin_block_if();
      ctx.emit(Instruction::I32Const(0));
      ctx.emit(Instruction::LocalSet(valid));
      ctx.emit(Instruction::LocalGet(payload_error));
      ctx.emit(Instruction::LocalSet(error_kind));
      ctx.emit(Instruction::End);
      ctx.emit(Instruction::End);
    }

    ctx.emit(Instruction::LocalGet(valid));
    ctx.begin_block_if();
    let tag_id = *ctx.tag_index.get(variant.ref_str()).ok_or_else(|| {
      format!(
        "E_WASM_EDN_SHAPE: enum :{} variant :{} is not present in the compiled program",
        nominal.name(),
        variant
      )
    })?;
    let enum_ptr = ctx.alloc_local_typed(ValType::I32);
    emit_bump_alloc(ctx, ((2 + payload_nodes.len()) * 8) as i32, enum_ptr, "enum");
    ctx.emit(Instruction::LocalGet(enum_ptr));
    ctx.emit(f64_const(payload_nodes.len() as f64));
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
    ctx.emit(Instruction::LocalGet(enum_ptr));
    ctx.emit(f64_const(tag_id as f64));
    ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
    for (payload_index, value) in values.iter().enumerate() {
      ctx.emit(Instruction::LocalGet(enum_ptr));
      ctx.emit(Instruction::LocalGet(*value));
      ctx.emit(Instruction::F64Store(mem_arg_f64(((2 + payload_index) * 8) as u64)));
    }
    ctx.emit(Instruction::LocalGet(enum_ptr));
    ctx.emit(Instruction::F64ConvertI32U);
    ctx.emit(Instruction::LocalSet(parsed));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::LocalSet(ok));
    ctx.emit(Instruction::End);
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::LocalGet(matched));
  ctx.emit(Instruction::I32Eqz);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::I32Const(5));
  ctx.emit(Instruction::LocalSet(error_kind));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_parse_scalar_list(
  ctx: &mut WasmGenCtx,
  graph: &DataShapeGraph,
  item_id: usize,
  token_ptr: u32,
  parsed: u32,
  ok: u32,
  error_kind: u32,
) -> Result<(), String> {
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(ok));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(error_kind));
  let valid = ctx.alloc_i32(0);
  let len = load_string_len(ctx, token_ptr);

  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::LocalGet(valid));
  ctx.begin_block_if();
  let first = load_byte(ctx, token_ptr, 0);
  let second = load_byte(ctx, token_ptr, 1);
  ctx.emit(Instruction::LocalGet(first));
  ctx.emit(Instruction::I32Const(b'[' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::LocalGet(second));
  ctx.emit(Instruction::I32Const(b']' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::End);

  // `[]` must be the complete collection marker, not a prefix of a token.
  ctx.emit(Instruction::LocalGet(valid));
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32GtU);
  ctx.emit(Instruction::I32And);
  ctx.begin_block_if();
  let delimiter = load_byte(ctx, token_ptr, 2);
  emit_ascii_whitespace(ctx, delimiter);
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::End);

  let count = ctx.alloc_i32(0);
  emit_scan_edn_tokens(ctx, token_ptr, len, 2, (valid, count, error_kind), |_ctx, _start, _end, _index| {
    Ok(())
  })?;

  ctx.emit(Instruction::LocalGet(valid));
  ctx.begin_block_if();
  let list_ptr = emit_alloc_list(ctx, count);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(count));
  emit_scan_edn_tokens(ctx, token_ptr, len, 2, (valid, count, error_kind), |ctx, start, end, index| {
    let item_ptr = slice_string_range(ctx, token_ptr, start, end);
    let item_token = ctx.alloc_local();
    ctx.emit(Instruction::LocalGet(item_ptr));
    ctx.emit(Instruction::F64ConvertI32U);
    ctx.emit(Instruction::LocalSet(item_token));
    let item_value = ctx.alloc_local();
    let item_ok = ctx.alloc_local_typed(ValType::I32);
    let item_error = ctx.alloc_local_typed(ValType::I32);
    emit_parse_node(ctx, graph, item_id, item_token, item_ptr, (item_value, item_ok, item_error))?;
    ctx.emit(Instruction::LocalGet(item_ok));
    ctx.begin_block_if();
    emit_list_store_elem(ctx, list_ptr, index, item_value);
    ctx.emit(Instruction::Else);
    ctx.emit(Instruction::I32Const(0));
    ctx.emit(Instruction::LocalSet(valid));
    ctx.emit(Instruction::LocalGet(item_error));
    ctx.emit(Instruction::LocalSet(error_kind));
    ctx.emit(Instruction::End);
    Ok(())
  })?;
  ctx.emit(Instruction::LocalGet(valid));
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(list_ptr));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(parsed));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(ok));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_scan_edn_tokens<F>(
  ctx: &mut WasmGenCtx,
  token_ptr: u32,
  len: u32,
  start_offset: i32,
  state: (u32, u32, u32),
  mut on_token: F,
) -> Result<(), String>
where
  F: FnMut(&mut WasmGenCtx, u32, u32, u32) -> Result<(), String>,
{
  let (valid, count, error_kind) = state;
  let cursor = ctx.alloc_i32(start_offset);
  let start = ctx.alloc_local_typed(ValType::I32);
  let end = ctx.alloc_local_typed(ValType::I32);

  ctx.begin_block();
  ctx.begin_loop();
  ctx.emit(Instruction::LocalGet(valid));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::BrIf(1));

  // Skip separators between compact top-level list items.
  ctx.begin_block();
  ctx.begin_loop();
  ctx.emit(Instruction::LocalGet(cursor));
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  let separator = load_byte_at(ctx, token_ptr, cursor);
  emit_ascii_whitespace(ctx, separator);
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::BrIf(1));
  ctx.i32_inc(cursor);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(cursor));
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  ctx.emit(Instruction::LocalGet(cursor));
  ctx.emit(Instruction::LocalSet(start));
  let initial = load_byte_at(ctx, token_ptr, cursor);
  ctx.emit(Instruction::LocalGet(initial));
  ctx.emit(Instruction::I32Const(b'"' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();

  // A quoted leaf may contain whitespace and escaped quotes. The scalar
  // decoder performs the stricter escape validation after tokenization.
  ctx.i32_inc(cursor);
  let closed = ctx.alloc_i32(0);
  ctx.begin_block();
  ctx.begin_loop();
  ctx.emit(Instruction::LocalGet(cursor));
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  let byte = load_byte_at(ctx, token_ptr, cursor);
  ctx.emit(Instruction::LocalGet(byte));
  ctx.emit(Instruction::I32Const(b'\\' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  ctx.i32_inc(cursor);
  ctx.emit(Instruction::LocalGet(cursor));
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32LtU);
  ctx.begin_block_if();
  ctx.i32_inc(cursor);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(byte));
  ctx.emit(Instruction::I32Const(b'"' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  ctx.i32_inc(cursor);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(closed));
  ctx.emit(Instruction::Br(3));
  ctx.emit(Instruction::Else);
  ctx.i32_inc(cursor);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(closed));
  ctx.emit(Instruction::I32Eqz);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(closed));
  ctx.emit(Instruction::LocalGet(cursor));
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32LtU);
  ctx.emit(Instruction::I32And);
  ctx.begin_block_if();
  let delimiter = load_byte_at(ctx, token_ptr, cursor);
  emit_ascii_whitespace(ctx, delimiter);
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::Else);
  emit_scan_unquoted_edn_token(ctx, token_ptr, len, cursor, valid, false);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(cursor));
  ctx.emit(Instruction::LocalSet(end));
  ctx.emit(Instruction::LocalGet(valid));
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(MAX_EDN_LIST_ITEMS));
  ctx.emit(Instruction::I32GeU);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::I32Const(3));
  ctx.emit(Instruction::LocalSet(error_kind));
  ctx.emit(Instruction::Else);
  on_token(ctx, start, end, count)?;
  ctx.i32_inc(count);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_parse_scalar_map(
  ctx: &mut WasmGenCtx,
  graph: &DataShapeGraph,
  nodes: (usize, usize),
  token_ptr: u32,
  outputs: (u32, u32, u32),
) -> Result<(), String> {
  let (key_id, value_id) = nodes;
  let (parsed, ok, error_kind) = outputs;
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(ok));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(error_kind));
  let valid = ctx.alloc_i32(0);
  let len = load_string_len(ctx, token_ptr);

  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::LocalGet(valid));
  ctx.begin_block_if();
  let first = load_byte(ctx, token_ptr, 0);
  let second = load_byte(ctx, token_ptr, 1);
  ctx.emit(Instruction::LocalGet(first));
  ctx.emit(Instruction::I32Const(b'{' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::LocalGet(second));
  ctx.emit(Instruction::I32Const(b'}' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(valid));
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32GtU);
  ctx.emit(Instruction::I32And);
  ctx.begin_block_if();
  let delimiter = load_byte(ctx, token_ptr, 2);
  emit_ascii_whitespace(ctx, delimiter);
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::End);

  let count = ctx.alloc_i32(0);
  let entries_start = ctx.alloc_i32(2);
  emit_scan_scalar_map_entries(
    ctx,
    token_ptr,
    len,
    entries_start,
    (valid, count, error_kind),
    |_ctx, _key_start, _key_end, _value_start, _value_end, _index| Ok(()),
  )?;

  ctx.emit(Instruction::LocalGet(valid));
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(count));
  let total_slots = ctx.alloc_i32(1);
  let flat_root = emit_alloc_with_count(ctx, count, total_slots, "map");
  ctx.emit(Instruction::LocalGet(flat_root));
  ctx.call_rt("__rt_map_root_from_flat");
  let hashed_root = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalSet(hashed_root));
  let map_ptr = emit_alloc_map_with_root(ctx, count, hashed_root);

  emit_scan_scalar_map_entries(
    ctx,
    token_ptr,
    len,
    entries_start,
    (valid, count, error_kind),
    |ctx, key_start, key_end, value_start, value_end, _index| {
      let key_ptr = slice_string_range(ctx, token_ptr, key_start, key_end);
      let key_token = ctx.alloc_local();
      ctx.emit(Instruction::LocalGet(key_ptr));
      ctx.emit(Instruction::F64ConvertI32U);
      ctx.emit(Instruction::LocalSet(key_token));
      let key_value = ctx.alloc_local();
      let key_ok = ctx.alloc_local_typed(ValType::I32);
      let key_error = ctx.alloc_local_typed(ValType::I32);
      emit_parse_node(ctx, graph, key_id, key_token, key_ptr, (key_value, key_ok, key_error))?;

      ctx.emit(Instruction::LocalGet(key_ok));
      ctx.begin_block_if();
      let value_ptr = slice_string_range(ctx, token_ptr, value_start, value_end);
      let value_token = ctx.alloc_local();
      ctx.emit(Instruction::LocalGet(value_ptr));
      ctx.emit(Instruction::F64ConvertI32U);
      ctx.emit(Instruction::LocalSet(value_token));
      let item_value = ctx.alloc_local();
      let value_ok = ctx.alloc_local_typed(ValType::I32);
      let value_error = ctx.alloc_local_typed(ValType::I32);
      emit_parse_node(ctx, graph, value_id, value_token, value_ptr, (item_value, value_ok, value_error))?;
      ctx.emit(Instruction::LocalGet(value_ok));
      ctx.begin_block_if();
      ctx.emit(Instruction::LocalGet(map_ptr));
      ctx.emit(Instruction::LocalGet(key_value));
      ctx.emit(Instruction::LocalGet(item_value));
      ctx.call_rt("__rt_map_assoc");
      ctx.emit(Instruction::LocalSet(map_ptr));
      ctx.emit(Instruction::Else);
      ctx.emit(Instruction::I32Const(0));
      ctx.emit(Instruction::LocalSet(valid));
      ctx.emit(Instruction::LocalGet(value_error));
      ctx.emit(Instruction::LocalSet(error_kind));
      ctx.emit(Instruction::End);
      ctx.emit(Instruction::Else);
      ctx.emit(Instruction::I32Const(0));
      ctx.emit(Instruction::LocalSet(valid));
      ctx.emit(Instruction::LocalGet(key_error));
      ctx.emit(Instruction::LocalSet(error_kind));
      ctx.emit(Instruction::End);
      Ok(())
    },
  )?;
  ctx.emit(Instruction::LocalGet(valid));
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(map_ptr));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(parsed));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(ok));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_scan_scalar_map_entries<F>(
  ctx: &mut WasmGenCtx,
  token_ptr: u32,
  len: u32,
  entries_start: u32,
  state: (u32, u32, u32),
  mut on_entry: F,
) -> Result<(), String>
where
  F: FnMut(&mut WasmGenCtx, u32, u32, u32, u32, u32) -> Result<(), String>,
{
  let (valid, count, error_kind) = state;
  let cursor = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(entries_start));
  ctx.emit(Instruction::LocalSet(cursor));
  ctx.begin_block();
  ctx.begin_loop();
  ctx.emit(Instruction::LocalGet(valid));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::BrIf(1));
  emit_skip_ascii_whitespace(ctx, token_ptr, len, cursor);
  ctx.emit(Instruction::LocalGet(cursor));
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));

  let open = load_byte_at(ctx, token_ptr, cursor);
  ctx.emit(Instruction::LocalGet(open));
  ctx.emit(Instruction::I32Const(b'(' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  ctx.i32_inc(cursor);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::End);

  emit_skip_ascii_whitespace(ctx, token_ptr, len, cursor);
  let (key_start, key_end) = emit_scan_map_scalar_token(ctx, token_ptr, len, cursor, valid);
  emit_skip_ascii_whitespace(ctx, token_ptr, len, cursor);
  let (value_start, value_end) = emit_scan_map_scalar_token(ctx, token_ptr, len, cursor, valid);
  emit_skip_ascii_whitespace(ctx, token_ptr, len, cursor);

  ctx.emit(Instruction::LocalGet(valid));
  ctx.emit(Instruction::LocalGet(cursor));
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32LtU);
  ctx.emit(Instruction::I32And);
  ctx.begin_block_if();
  let close = load_byte_at(ctx, token_ptr, cursor);
  ctx.emit(Instruction::LocalGet(close));
  ctx.emit(Instruction::I32Const(b')' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  ctx.i32_inc(cursor);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(valid));
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(MAX_EDN_MAP_ENTRIES));
  ctx.emit(Instruction::I32GeU);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::I32Const(4));
  ctx.emit(Instruction::LocalSet(error_kind));
  ctx.emit(Instruction::Else);
  on_entry(ctx, key_start, key_end, value_start, value_end, count)?;
  ctx.i32_inc(count);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_scan_map_scalar_token(ctx: &mut WasmGenCtx, token_ptr: u32, len: u32, cursor: u32, valid: u32) -> (u32, u32) {
  let start = ctx.alloc_local_typed(ValType::I32);
  let end = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(cursor));
  ctx.emit(Instruction::LocalSet(start));
  ctx.emit(Instruction::LocalGet(cursor));
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32GeU);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::Else);
  let initial = load_byte_at(ctx, token_ptr, cursor);
  ctx.emit(Instruction::LocalGet(initial));
  ctx.emit(Instruction::I32Const(b'"' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  ctx.i32_inc(cursor);
  let closed = ctx.alloc_i32(0);
  ctx.begin_block();
  ctx.begin_loop();
  ctx.emit(Instruction::LocalGet(cursor));
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  let byte = load_byte_at(ctx, token_ptr, cursor);
  ctx.emit(Instruction::LocalGet(byte));
  ctx.emit(Instruction::I32Const(b'\\' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  ctx.i32_inc(cursor);
  ctx.emit(Instruction::LocalGet(cursor));
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32LtU);
  ctx.begin_block_if();
  ctx.i32_inc(cursor);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(byte));
  ctx.emit(Instruction::I32Const(b'"' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  ctx.i32_inc(cursor);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(closed));
  ctx.emit(Instruction::Br(3));
  ctx.emit(Instruction::Else);
  ctx.i32_inc(cursor);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(closed));
  ctx.emit(Instruction::I32Eqz);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::Else);
  emit_scan_unquoted_edn_token(ctx, token_ptr, len, cursor, valid, true);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(cursor));
  ctx.emit(Instruction::LocalSet(end));
  (start, end)
}

/// Scan one bare or parenthesized EDN token while treating quoted string
/// contents as opaque bytes. This is shared by List items and Map/Struct entry
/// values so parentheses and whitespace inside strings never affect nesting.
fn emit_scan_unquoted_edn_token(
  ctx: &mut WasmGenCtx,
  token_ptr: u32,
  len: u32,
  cursor: u32,
  valid: u32,
  stop_at_closing_parenthesis: bool,
) {
  let parentheses = ctx.alloc_i32(0);
  let in_quote = ctx.alloc_i32(0);
  let handled_quote = ctx.alloc_i32(0);
  ctx.begin_block();
  ctx.begin_loop();
  ctx.emit(Instruction::LocalGet(cursor));
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  let byte = load_byte_at(ctx, token_ptr, cursor);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(handled_quote));

  ctx.emit(Instruction::LocalGet(in_quote));
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(handled_quote));
  ctx.emit(Instruction::LocalGet(byte));
  ctx.emit(Instruction::I32Const(b'\\' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  ctx.i32_inc(cursor);
  ctx.emit(Instruction::LocalGet(cursor));
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32GeU);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(byte));
  ctx.emit(Instruction::I32Const(b'"' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(in_quote));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(byte));
  ctx.emit(Instruction::I32Const(b'"' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(in_quote));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(handled_quote));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(handled_quote));
  ctx.begin_block_if();
  ctx.i32_inc(cursor);
  ctx.emit(Instruction::Br(1));
  ctx.emit(Instruction::End);

  emit_ascii_whitespace(ctx, byte);
  ctx.emit(Instruction::LocalGet(parentheses));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::I32And);
  if stop_at_closing_parenthesis {
    ctx.emit(Instruction::LocalGet(byte));
    ctx.emit(Instruction::I32Const(b')' as i32));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::LocalGet(parentheses));
    ctx.emit(Instruction::I32Eqz);
    ctx.emit(Instruction::I32And);
    ctx.emit(Instruction::I32Or);
  }
  ctx.emit(Instruction::BrIf(1));

  ctx.emit(Instruction::LocalGet(byte));
  ctx.emit(Instruction::I32Const(b'(' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  ctx.i32_inc(parentheses);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(byte));
  ctx.emit(Instruction::I32Const(b')' as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::LocalGet(parentheses));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::I32GtU);
  ctx.emit(Instruction::I32And);
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(parentheses));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(parentheses));
  ctx.emit(Instruction::End);
  ctx.i32_inc(cursor);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(parentheses));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::LocalGet(in_quote));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalGet(valid));
  ctx.emit(Instruction::I32And);
  ctx.emit(Instruction::LocalSet(valid));
}

fn emit_skip_ascii_whitespace(ctx: &mut WasmGenCtx, token_ptr: u32, len: u32, cursor: u32) {
  ctx.begin_block();
  ctx.begin_loop();
  ctx.emit(Instruction::LocalGet(cursor));
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::BrIf(1));
  let byte = load_byte_at(ctx, token_ptr, cursor);
  emit_ascii_whitespace(ctx, byte);
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::BrIf(1));
  ctx.i32_inc(cursor);
  ctx.emit(Instruction::Br(0));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
}

fn emit_ascii_whitespace(ctx: &mut WasmGenCtx, byte: u32) {
  for (index, expected) in b" \n\r\t".iter().copied().enumerate() {
    ctx.emit(Instruction::LocalGet(byte));
    ctx.emit(Instruction::I32Const(expected as i32));
    ctx.emit(Instruction::I32Eq);
    if index > 0 {
      ctx.emit(Instruction::I32Or);
    }
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
  let fractional_nonzero = ctx.alloc_local_typed(ValType::I32);
  let valid = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(index));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(digits));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(dot_seen));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(fractional_nonzero));

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
  ctx.emit(Instruction::LocalGet(dot_seen));
  ctx.emit(Instruction::LocalGet(byte));
  ctx.emit(Instruction::I32Const(b'0' as i32));
  ctx.emit(Instruction::I32Ne);
  ctx.emit(Instruction::I32And);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(fractional_nonzero));
  ctx.emit(Instruction::End);
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
    if matches!(
      kind,
      CalcitNumericRefinement::Int8
        | CalcitNumericRefinement::UInt8
        | CalcitNumericRefinement::Int16
        | CalcitNumericRefinement::UInt16
        | CalcitNumericRefinement::Int32
        | CalcitNumericRefinement::UInt32
        | CalcitNumericRefinement::Int64
        | CalcitNumericRefinement::UInt64
    ) {
      ctx.emit(Instruction::LocalGet(in_range));
      ctx.emit(Instruction::LocalGet(fractional_nonzero));
      ctx.emit(Instruction::I32Eqz);
      ctx.emit(Instruction::I32And);
      ctx.emit(Instruction::LocalSet(in_range));
    }
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rejects_recursive_map_keys_before_codegen() {
    let graph = DataShapeGraph::from_nodes(
      0,
      vec![
        DataShapeNode::Map { key: 1, value: 2 },
        DataShapeNode::List(2),
        DataShapeNode::String,
      ],
    )
    .expect("test shape should be well formed");

    let error = validate_decode_shape(&graph, graph.root, 0).expect_err("collection map keys must fail closed");
    assert!(error.starts_with("E_WASM_EDN_MAP_KEY:"));
  }
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

fn slice_string_range(ctx: &mut WasmGenCtx, ptr: u32, start: u32, end: u32) -> u32 {
  let len = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(end));
  ctx.emit(Instruction::LocalGet(start));
  ctx.emit(Instruction::I32Sub);
  ctx.emit(Instruction::LocalSet(len));
  let (output, content) = emit_str_alloc(ctx, len);
  ctx.emit(Instruction::LocalGet(content));
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(start));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(len));
  ctx.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
  output
}
