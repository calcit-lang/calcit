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

fn emit_decode_result_envelope(ctx: &mut WasmGenCtx, value: u32, ok: u32, error_kind: u32, error_path: u32) -> Result<(), String> {
  ctx.emit(Instruction::LocalGet(ok));
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_result_enum(ctx, "ok", value)?;
  ctx.emit(Instruction::Else);
  let parse_error = ctx.alloc_local();
  let error_cases = [
    (1, RANGE_ERROR),
    (2, TAG_ERROR),
    (3, TOKEN_LIMIT_ERROR),
    (4, MAP_LIMIT_ERROR),
    (5, ENUM_ERROR),
  ];
  for (code, message) in error_cases {
    ctx.emit(Instruction::LocalGet(error_kind));
    ctx.emit(Instruction::I32Const(code));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
    emit_literal(ctx, message)?;
    ctx.emit(Instruction::Else);
  }
  emit_literal(ctx, SYNTAX_ERROR)?;
  for _ in error_cases {
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::LocalSet(parse_error));
  let error_path_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(error_path));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(error_path_ptr));
  let error_path_len = load_string_len(ctx, error_path_ptr);
  ctx.emit(Instruction::LocalGet(error_path_len));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::I32GtU);
  ctx.begin_block_if();
  let at_prefix = literal_local(ctx, " at $")?;
  let with_prefix = concat_string_locals(ctx, parse_error, at_prefix);
  let full_error = concat_string_locals(ctx, with_prefix, error_path);
  ctx.emit(Instruction::LocalGet(full_error));
  ctx.emit(Instruction::LocalSet(parse_error));
  ctx.emit(Instruction::End);
  emit_result_enum(ctx, "err", parse_error)?;
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_parse_cirru_edn_common(ctx: &mut WasmGenCtx, args: &[Calcit], op_name: &str, trap_on_error: bool) -> Result<(), String> {
  if args.len() != 3 {
    return Err(format!(
      "{op_name} expects preprocessed text, type and data shape arguments, got {}",
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

  let run_parse = |ctx: &mut WasmGenCtx| -> Result<(u32, u32, u32, u32), String> {
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
    let error_path = literal_local(ctx, "")?;
    emit_parse_node_with_path(ctx, &graph, graph.root, token, token_ptr, (parsed, ok, error_kind, error_path))?;
    Ok((parsed, ok, error_kind, error_path))
  };

  if trap_on_error {
    ctx.emit(Instruction::LocalGet(input_len));
    ctx.emit(Instruction::I32Const(MAX_EDN_INPUT_BYTES));
    ctx.emit(Instruction::I32GtU);
    ctx.begin_block_if();
    ctx.emit(Instruction::Unreachable);
    ctx.emit(Instruction::End);

    let (parsed, ok, _error_kind, _error_path) = run_parse(ctx)?;
    ctx.emit(Instruction::LocalGet(ok));
    ctx.emit(Instruction::I32Eqz);
    ctx.begin_block_if();
    ctx.emit(Instruction::Unreachable);
    ctx.emit(Instruction::End);
    ctx.emit(Instruction::LocalGet(parsed));
    return Ok(());
  }

  let result = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(input_len));
  ctx.emit(Instruction::I32Const(MAX_EDN_INPUT_BYTES));
  ctx.emit(Instruction::I32GtU);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  let error = literal_local(ctx, INPUT_LIMIT_ERROR)?;
  emit_result_enum(ctx, "err", error)?;
  ctx.emit(Instruction::Else);
  let (parsed, ok, error_kind, error_path) = run_parse(ctx)?;
  emit_decode_result_envelope(ctx, parsed, ok, error_kind, error_path)?;
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

pub(super) fn emit_try_parse_cirru_edn_as(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  emit_parse_cirru_edn_common(ctx, args, "try-parse-cirru-edn-as", false)
}

pub(super) fn emit_parse_cirru_edn_as(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  emit_parse_cirru_edn_common(ctx, args, "parse-cirru-edn-as", true)
}

pub(super) fn emit_json_parse(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err(format!("json-parse expected 1 argument, got {}", args.len()));
  }
  emit_expr(ctx, &args[0])?;
  let input = ctx.alloc_local();
  ctx.emit(Instruction::LocalSet(input));
  let is_str = emit_has_heap_tag(ctx, input, "string");
  ctx.emit(Instruction::LocalGet(is_str));
  ctx.emit(Instruction::I32Eqz);
  ctx.begin_block_if();
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);

  let input_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(input));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(input_ptr));
  let input_len = load_string_len(ctx, input_ptr);
  ctx.emit(Instruction::LocalGet(input_len));
  ctx.emit(Instruction::I32Const(MAX_EDN_INPUT_BYTES));
  ctx.emit(Instruction::I32GtU);
  ctx.begin_block_if();
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);

  let val = ctx.alloc_local();
  let ok = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(input));
  ctx.call_rt("__rt_json_parse_root");
  ctx.emit(Instruction::LocalSet(ok));
  ctx.emit(Instruction::LocalSet(val));
  ctx.emit(Instruction::LocalGet(ok));
  ctx.emit(Instruction::I32Eqz);
  ctx.begin_block_if();
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(val));
  Ok(())
}

fn emit_try_parse_json_with_rt(ctx: &mut WasmGenCtx, input: u32, rt_root_fn: &str) -> Result<(), String> {
  let is_str = emit_has_heap_tag(ctx, input, "string");
  ctx.emit(Instruction::LocalGet(is_str));
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  let input_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(input));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(input_ptr));
  let input_len = load_string_len(ctx, input_ptr);
  ctx.emit(Instruction::LocalGet(input_len));
  ctx.emit(Instruction::I32Const(MAX_EDN_INPUT_BYTES));
  ctx.emit(Instruction::I32GtU);
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  let err_msg = literal_local(ctx, "E_WASM_EDN_SYNTAX: invalid JSON input")?;
  emit_result_enum(ctx, "err", err_msg)?;
  ctx.emit(Instruction::Else);
  let val = ctx.alloc_local();
  let ok = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(input));
  ctx.call_rt(rt_root_fn);
  ctx.emit(Instruction::LocalSet(ok));
  ctx.emit(Instruction::LocalSet(val));
  ctx.emit(Instruction::LocalGet(ok));
  ctx.emit(Instruction::If(wasm_encoder::BlockType::Result(ValType::F64)));
  emit_result_enum(ctx, "ok", val)?;
  ctx.emit(Instruction::Else);
  let err_msg = literal_local(ctx, "E_WASM_EDN_SYNTAX: invalid JSON input")?;
  emit_result_enum(ctx, "err", err_msg)?;
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);
  Ok(())
}

pub(super) fn emit_try_parse_json_from_local(ctx: &mut WasmGenCtx, input: u32) -> Result<(), String> {
  emit_try_parse_json_with_rt(ctx, input, "__rt_json_parse_root")
}

pub(super) fn emit_try_parse_json(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err(format!("try-parse-json expected 1 argument, got {}", args.len()));
  }
  emit_expr(ctx, &args[0])?;
  let input = ctx.alloc_local();
  ctx.emit(Instruction::LocalSet(input));
  emit_try_parse_json_from_local(ctx, input)
}

pub(super) fn emit_try_parse_json_value_from_local(ctx: &mut WasmGenCtx, input: u32) -> Result<(), String> {
  emit_try_parse_json_with_rt(ctx, input, "__rt_json_parse_value_root")
}

pub(super) fn emit_try_parse_json_value(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 1 {
    return Err(format!("json:parse expected 1 argument, got {}", args.len()));
  }
  emit_expr(ctx, &args[0])?;
  let input = ctx.alloc_local();
  ctx.emit(Instruction::LocalSet(input));
  emit_try_parse_json_value_from_local(ctx, input)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MapCodecMode {
  Decode,
  Encode,
}

fn emit_decode_map_common(ctx: &mut WasmGenCtx, args: &[Calcit], op_name: &str, trap_on_error: bool) -> Result<(), String> {
  if args.len() != 3 {
    return Err(format!(
      "{op_name} expects preprocessed value, type and data shape arguments, got {}",
      args.len()
    ));
  }
  let graph = DataShapeGraph::from_calcit_handle(&args[2])
    .ok_or_else(|| "E_WASM_EDN_SHAPE: typed map decoder is missing its preprocessed data shape".to_string())?;
  validate_map_decode_shape(&graph, graph.root, 0)?;

  let static_hint = super::infer_wasm_codec_hint(ctx, &args[0]);
  emit_expr(ctx, &args[0])?;
  let input = ctx.alloc_local();
  ctx.emit(Instruction::LocalSet(input));

  let decoded = ctx.alloc_local();
  let ok = ctx.alloc_local_typed(ValType::I32);
  let error_kind = ctx.alloc_local_typed(ValType::I32);
  let error_path = literal_local(ctx, "")?;
  ctx.map_codec_expanded_nodes = 0;
  emit_decode_map_node(
    ctx,
    &graph,
    graph.root,
    input,
    static_hint.as_deref(),
    (decoded, ok, error_kind, error_path),
    MapCodecMode::Decode,
  )?;

  if trap_on_error {
    ctx.emit(Instruction::LocalGet(ok));
    ctx.emit(Instruction::I32Eqz);
    ctx.begin_block_if();
    ctx.emit(Instruction::Unreachable);
    ctx.emit(Instruction::End);
    ctx.emit(Instruction::LocalGet(decoded));
    return Ok(());
  }

  let result = ctx.alloc_local();
  emit_decode_result_envelope(ctx, decoded, ok, error_kind, error_path)?;
  ctx.emit(Instruction::LocalSet(result));
  ctx.emit(Instruction::LocalGet(result));
  Ok(())
}

pub(super) fn emit_try_decode_map_as(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  emit_decode_map_common(ctx, args, "try-decode-map-as", false)
}

pub(super) fn emit_decode_map_as(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  emit_decode_map_common(ctx, args, "decode-map-as", true)
}

pub(super) fn emit_encode_map_as(ctx: &mut WasmGenCtx, args: &[Calcit]) -> Result<(), String> {
  if args.len() != 3 {
    return Err(format!(
      "encode-map-as expects preprocessed value, type and data shape arguments, got {}",
      args.len()
    ));
  }
  let graph = DataShapeGraph::from_calcit_handle(&args[2])
    .ok_or_else(|| "E_WASM_EDN_SHAPE: typed map encoder is missing its preprocessed data shape".to_string())?;
  validate_map_decode_shape(&graph, graph.root, 0)?;

  let static_hint = super::infer_wasm_codec_hint(ctx, &args[0]);
  emit_expr(ctx, &args[0])?;
  let input = ctx.alloc_local();
  ctx.emit(Instruction::LocalSet(input));

  let encoded = ctx.alloc_local();
  let ok = ctx.alloc_local_typed(ValType::I32);
  let error_kind = ctx.alloc_local_typed(ValType::I32);
  let error_path = literal_local(ctx, "")?;
  ctx.map_codec_expanded_nodes = 0;
  emit_decode_map_node(
    ctx,
    &graph,
    graph.root,
    input,
    static_hint.as_deref(),
    (encoded, ok, error_kind, error_path),
    MapCodecMode::Encode,
  )?;
  ctx.emit(Instruction::LocalGet(ok));
  ctx.emit(Instruction::I32Eqz);
  ctx.begin_block_if();
  ctx.emit(Instruction::Unreachable);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::LocalGet(encoded));
  Ok(())
}

const MAX_MAP_CODEC_EXPANDED_NODES: usize = 512;

fn validate_map_decode_shape(graph: &DataShapeGraph, node_id: usize, depth: usize) -> Result<(), String> {
  let mut total_nodes = 0;
  let mut compound_nodes = 0;
  validate_map_decode_shape_with_budget(graph, node_id, depth, (&mut total_nodes, &mut compound_nodes))
}

fn validate_map_decode_shape_with_budget(
  graph: &DataShapeGraph,
  node_id: usize,
  depth: usize,
  (total_nodes, compound_nodes): (&mut usize, &mut usize),
) -> Result<(), String> {
  if depth > MAX_EDN_DECODE_SHAPE_DEPTH {
    return Err(format!(
      "E_WASM_EDN_DEPTH: typed map decode shape nesting exceeds {MAX_EDN_DECODE_SHAPE_DEPTH}"
    ));
  }
  *total_nodes = total_nodes.saturating_add(1);
  if *total_nodes > MAX_EDN_DECODE_EXPANDED_NODES {
    return Err(format!(
      "E_WASM_EDN_DEPTH: typed map decode shape expansion exceeds {MAX_EDN_DECODE_EXPANDED_NODES} nodes"
    ));
  }
  let node = graph
    .nodes
    .get(node_id)
    .ok_or_else(|| format!("E_WASM_EDN_SHAPE: typed map decoder references missing node #{node_id}"))?;
  match node {
    DataShapeNode::Dynamic
    | DataShapeNode::Nil
    | DataShapeNode::Unit
    | DataShapeNode::Bool
    | DataShapeNode::Number
    | DataShapeNode::Numeric(_)
    | DataShapeNode::String
    | DataShapeNode::Tag => Ok(()),
    _ => {
      *compound_nodes = compound_nodes.saturating_add(1);
      if *compound_nodes > MAX_MAP_CODEC_EXPANDED_NODES {
        return Err(format!(
          "E_WASM_EDN_DEPTH: typed map decode shape expansion exceeds {MAX_MAP_CODEC_EXPANDED_NODES} compound nodes"
        ));
      }
      match node {
        DataShapeNode::MapOption { inner, .. } => {
          if let Some(DataShapeNode::Enum {
            nominal: inner_enum,
            variants,
            ..
          }) = graph.nodes.get(*inner)
            && variants.iter().any(|(tag, payloads)| {
              (tag.ref_str() == "none" && payloads.is_empty()) || (tag.ref_str() == "some" && payloads.len() == 1)
            })
          {
            return Err(format!(
              "E_WASM_EDN_ENUM_SHAPE: Option<{}> cannot be unambiguously represented in the WASM enum ABI because :{} defines a conflicting :none or :some variant",
              inner_enum.name(),
              inner_enum.name()
            ));
          }
          validate_map_decode_shape_with_budget(graph, *inner, depth + 1, (total_nodes, compound_nodes))
        }
        DataShapeNode::List(inner) | DataShapeNode::Set(inner) => {
          validate_map_decode_shape_with_budget(graph, *inner, depth + 1, (total_nodes, compound_nodes))
        }
        DataShapeNode::Tuple(slots) => {
          for slot in slots {
            validate_map_decode_shape_with_budget(graph, *slot, depth + 1, (total_nodes, compound_nodes))?;
          }
          Ok(())
        }
        DataShapeNode::Map { key, value } => {
          validate_map_decode_shape_with_budget(graph, *key, depth + 1, (total_nodes, compound_nodes))?;
          validate_map_decode_shape_with_budget(graph, *value, depth + 1, (total_nodes, compound_nodes))
        }
        DataShapeNode::Struct { fields, .. } => {
          for (_, field) in fields {
            validate_map_decode_shape_with_budget(graph, *field, depth + 1, (total_nodes, compound_nodes))?;
          }
          Ok(())
        }
        DataShapeNode::Enum { variants, .. } => {
          for (_, payloads) in variants {
            for payload in payloads {
              validate_map_decode_shape_with_budget(graph, *payload, depth + 1, (total_nodes, compound_nodes))?;
            }
          }
          Ok(())
        }
        other => Err(format!("E_WASM_EDN_SHAPE: {other:?} is not yet supported by the typed map decoder")),
      }
    }
  }
}

fn prepend_static_path(ctx: &mut WasmGenCtx, prefix: &str, child_path: u32) -> Result<u32, String> {
  let prefix_local = literal_local(ctx, prefix)?;
  Ok(concat_string_locals(ctx, prefix_local, child_path))
}

fn prepend_indexed_path(ctx: &mut WasmGenCtx, open_prefix: &str, index: usize, child_path: u32) -> Result<u32, String> {
  let open_local = literal_local(ctx, open_prefix)?;
  let close_local = literal_local(ctx, "]")?;
  let idx_local = ctx.alloc_local();
  ctx.emit(f64_const(index as f64));
  ctx.call_rt("__rt_f64_to_str");
  ctx.emit(Instruction::LocalSet(idx_local));
  let step1 = concat_string_locals(ctx, open_local, idx_local);
  let step2 = concat_string_locals(ctx, step1, close_local);
  Ok(concat_string_locals(ctx, step2, child_path))
}

fn prepend_field_path(ctx: &mut WasmGenCtx, field: &str, child_path: u32) -> Result<u32, String> {
  let dot_local = literal_local(ctx, ".")?;
  let field_local = literal_local(ctx, field)?;
  let step1 = concat_string_locals(ctx, dot_local, field_local);
  Ok(concat_string_locals(ctx, step1, child_path))
}

fn expand_static_hint(hint: Option<&CalcitTypeAnnotation>) -> Option<CalcitTypeAnnotation> {
  hint.map(|ty| super::expand_wasm_value_type(&Arc::new(ty.clone())).as_ref().clone())
}

fn is_proven_numeric(hint: Option<&CalcitTypeAnnotation>) -> bool {
  hint.is_some_and(|ty| {
    matches!(
      ty,
      CalcitTypeAnnotation::Number | CalcitTypeAnnotation::Numeric(_) | CalcitTypeAnnotation::Never
    )
  })
}

fn hint_list_elem(hint: Option<&CalcitTypeAnnotation>) -> Option<CalcitTypeAnnotation> {
  match hint {
    Some(CalcitTypeAnnotation::Never) => Some(CalcitTypeAnnotation::Never),
    Some(CalcitTypeAnnotation::List(elem)) => Some(super::expand_wasm_value_type(elem).as_ref().clone()),
    _ => None,
  }
}

fn hint_tuple_slot(hint: Option<&CalcitTypeAnnotation>, idx: usize) -> Option<CalcitTypeAnnotation> {
  match hint {
    Some(CalcitTypeAnnotation::Never) => Some(CalcitTypeAnnotation::Never),
    Some(CalcitTypeAnnotation::Tuple(slots)) => slots.get(idx).map(|s| super::expand_wasm_value_type(s).as_ref().clone()),
    _ => None,
  }
}

fn hint_set_elem(hint: Option<&CalcitTypeAnnotation>) -> Option<CalcitTypeAnnotation> {
  match hint {
    Some(CalcitTypeAnnotation::Never) => Some(CalcitTypeAnnotation::Never),
    Some(CalcitTypeAnnotation::Set(elem)) => Some(super::expand_wasm_value_type(elem).as_ref().clone()),
    _ => None,
  }
}

fn hint_map_kv(hint: Option<&CalcitTypeAnnotation>) -> (Option<CalcitTypeAnnotation>, Option<CalcitTypeAnnotation>) {
  match hint {
    Some(CalcitTypeAnnotation::Never) => (Some(CalcitTypeAnnotation::Never), Some(CalcitTypeAnnotation::Never)),
    Some(CalcitTypeAnnotation::Map(k, v)) if matches!(k.as_ref(), CalcitTypeAnnotation::StructValue(_)) => {
      let val_hint = match v.as_ref() {
        CalcitTypeAnnotation::Struct(def, args) => super::infer_encoded_struct_map_value_type(def, args).as_ref().clone(),
        CalcitTypeAnnotation::StructValue(def) => super::infer_encoded_struct_map_value_type(def, &[]).as_ref().clone(),
        other => super::expand_wasm_value_type(&Arc::new(other.clone())).as_ref().clone(),
      };
      (Some(CalcitTypeAnnotation::Tag), Some(val_hint))
    }
    Some(CalcitTypeAnnotation::Map(k, v)) => (
      Some(super::expand_wasm_value_type(k).as_ref().clone()),
      Some(super::expand_wasm_value_type(v).as_ref().clone()),
    ),
    None | Some(CalcitTypeAnnotation::Dynamic) => (Some(CalcitTypeAnnotation::Tag), None),
    _ => (None, None),
  }
}

fn hint_option_inner(hint: Option<&CalcitTypeAnnotation>, option_nominal: &CalcitEnumDef) -> Option<CalcitTypeAnnotation> {
  if matches!(hint, Some(CalcitTypeAnnotation::Never)) {
    return Some(CalcitTypeAnnotation::Never);
  }
  if let Some(actual) = hint.and_then(|ty| ty.resolve_to_enum()) {
    if actual.name() != option_nominal.name() && !actual.same_nominal_definition(option_nominal) {
      return hint.cloned();
    }
    let args: &[Arc<CalcitTypeAnnotation>] = match hint {
      Some(CalcitTypeAnnotation::Enum(_, args) | CalcitTypeAnnotation::TypeRef(_, args)) => args.as_slice(),
      _ => &[],
    };
    return (args.len() == 1).then(|| super::expand_wasm_value_type(&args[0]).as_ref().clone());
  }
  hint.cloned()
}

fn substitute_type_vars_with_args(
  ty: &Arc<CalcitTypeAnnotation>,
  generics: &[Arc<str>],
  args: &[Arc<CalcitTypeAnnotation>],
) -> CalcitTypeAnnotation {
  let expanded = super::expand_wasm_value_type(ty);
  if generics.is_empty() || args.is_empty() {
    return expanded.as_ref().clone();
  }
  let bindings: std::collections::HashMap<Arc<str>, Arc<CalcitTypeAnnotation>> = generics
    .iter()
    .zip(args.iter())
    .map(|(name, arg)| (name.clone(), super::expand_wasm_value_type(arg)))
    .collect();
  expanded.substitute_type_vars(&bindings).as_ref().clone()
}

fn is_same_or_refined_struct_hint(actual: &CalcitStructDef, nominal: &CalcitStructDef) -> bool {
  if actual.same_nominal_definition(nominal) {
    return true;
  }
  actual.definition_ref.is_some()
    && actual.definition_ref == nominal.definition_ref
    && actual.name == nominal.name
    && actual.fields == nominal.fields
    && actual.generics == nominal.generics
    && actual.where_bounds == nominal.where_bounds
    && actual.field_types.len() == nominal.field_types.len()
    && actual
      .field_types
      .iter()
      .zip(nominal.field_types.iter())
      .all(|(a, n)| a == n || matches!(super::expand_wasm_value_type(n).as_ref(), CalcitTypeAnnotation::Dynamic))
}

fn hint_struct_field(hint: Option<&CalcitTypeAnnotation>, nominal: &Arc<CalcitStructDef>, idx: usize) -> Option<CalcitTypeAnnotation> {
  if matches!(hint, Some(CalcitTypeAnnotation::Never)) {
    return Some(CalcitTypeAnnotation::Never);
  }
  let actual = hint.and_then(|ty| ty.resolve_to_struct())?;
  if !is_same_or_refined_struct_hint(&actual, nominal) {
    return None;
  }
  let args: &[Arc<CalcitTypeAnnotation>] = match hint {
    Some(CalcitTypeAnnotation::Struct(_, args) | CalcitTypeAnnotation::TypeRef(_, args)) => args.as_slice(),
    _ => &[],
  };
  let field_ty = actual
    .field_types
    .get(idx)
    .filter(|ty| !matches!(ty.as_ref(), CalcitTypeAnnotation::Dynamic))
    .or_else(|| nominal.field_types.get(idx))
    .or_else(|| actual.field_types.get(idx))?;
  let generics = if !actual.generics.is_empty() {
    &actual.generics
  } else {
    &nominal.generics
  };
  Some(substitute_type_vars_with_args(field_ty, generics, args))
}

fn hint_encoded_struct_field(
  hint: Option<&CalcitTypeAnnotation>,
  nominal: &Arc<CalcitStructDef>,
  idx: usize,
) -> Option<CalcitTypeAnnotation> {
  if matches!(hint, Some(CalcitTypeAnnotation::Never)) {
    return Some(CalcitTypeAnnotation::Never);
  }
  let struct_hint = match hint {
    Some(CalcitTypeAnnotation::Map(k, v)) if matches!(k.as_ref(), CalcitTypeAnnotation::StructValue(_)) => v.as_ref(),
    Some(other @ (CalcitTypeAnnotation::Struct(..) | CalcitTypeAnnotation::StructValue(_) | CalcitTypeAnnotation::TypeRef(..))) => {
      other
    }
    _ => return None,
  };
  if let Some(actual) = struct_hint.resolve_to_struct()
    && actual.name.ref_str() == "__wasm_literal_map__"
  {
    let field_name = nominal.fields.get(idx)?;
    return Some(match actual.fields.iter().position(|f| f == field_name) {
      Some(pos) => super::expand_wasm_value_type(actual.field_types.get(pos)?).as_ref().clone(),
      None => CalcitTypeAnnotation::Never,
    });
  }
  let field_ty = Arc::new(hint_struct_field(Some(struct_hint), nominal, idx)?);
  let expanded = super::expand_wasm_value_type(&field_ty);
  if let Some(enum_def) = expanded.resolve_to_enum()
    && enum_def.name().ref_str() == "Option"
    && let CalcitTypeAnnotation::Enum(_, opt_args) | CalcitTypeAnnotation::TypeRef(_, opt_args) = expanded.as_ref()
    && opt_args.len() == 1
  {
    let inner = super::expand_wasm_value_type(&opt_args[0]);
    let inner_is_option = inner
      .resolve_to_enum()
      .is_some_and(|inner_enum| inner_enum.name().ref_str() == "Option");
    if inner_is_option {
      return Some(CalcitTypeAnnotation::Enum(
        Arc::new(enum_def),
        Arc::new(vec![super::infer_encoded_codec_hint(&inner)]),
      ));
    }
    return Some(super::infer_encoded_codec_hint(&inner).as_ref().clone());
  }
  Some(super::infer_encoded_codec_hint(&expanded).as_ref().clone())
}

fn hint_enum_payload(
  hint: Option<&CalcitTypeAnnotation>,
  nominal: &Arc<CalcitEnumDef>,
  variant: &cirru_edn::EdnTag,
  pidx: usize,
) -> Option<CalcitTypeAnnotation> {
  if matches!(hint, Some(CalcitTypeAnnotation::Never)) {
    return Some(CalcitTypeAnnotation::Never);
  }
  let actual = hint.and_then(|ty| ty.resolve_to_enum())?;
  if !actual.same_nominal_definition(nominal) {
    return None;
  }
  let args: &[Arc<CalcitTypeAnnotation>] = match hint {
    Some(CalcitTypeAnnotation::Enum(_, args) | CalcitTypeAnnotation::TypeRef(_, args)) => args.as_slice(),
    _ => &[],
  };
  actual
    .find_variant(variant)
    .and_then(|v| v.payload_types().get(pidx))
    .map(|ty| substitute_type_vars_with_args(ty, actual.generics(), args))
}

fn emit_decode_map_node(
  ctx: &mut WasmGenCtx,
  graph: &DataShapeGraph,
  node_id: usize,
  input: u32,
  static_hint: Option<&CalcitTypeAnnotation>,
  outputs: (u32, u32, u32, u32),
  mode: MapCodecMode,
) -> Result<(), String> {
  let (decoded, ok, error_kind, _error_path) = outputs;
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(ok));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(error_kind));

  let static_hint = expand_static_hint(static_hint);
  let static_hint = static_hint.as_ref();

  let node = graph
    .nodes
    .get(node_id)
    .ok_or_else(|| format!("E_WASM_EDN_SHAPE: typed map decoder references missing node #{node_id}"))?;

  if !matches!(
    node,
    DataShapeNode::Dynamic
      | DataShapeNode::Nil
      | DataShapeNode::Unit
      | DataShapeNode::Bool
      | DataShapeNode::Number
      | DataShapeNode::Numeric(_)
      | DataShapeNode::String
      | DataShapeNode::Tag
  ) {
    ctx.map_codec_expanded_nodes = ctx.map_codec_expanded_nodes.saturating_add(1);
    if ctx.map_codec_expanded_nodes > MAX_MAP_CODEC_EXPANDED_NODES {
      return Err(format!(
        "E_WASM_EDN_DEPTH: typed map decode shape expansion exceeds {MAX_MAP_CODEC_EXPANDED_NODES} compound nodes"
      ));
    }
  }

  if mode == MapCodecMode::Encode
    && matches!(
      node,
      DataShapeNode::Nil
        | DataShapeNode::Unit
        | DataShapeNode::Bool
        | DataShapeNode::Number
        | DataShapeNode::Numeric(_)
        | DataShapeNode::Tag
    )
    && static_hint.is_none_or(|ty| nil_type_evidence_is_open(ty) && !matches!(ty, CalcitTypeAnnotation::Never))
  {
    return Err(
      "E_WASM_NIL_TYPE_EVIDENCE: encode-map-as requires concrete static scalar type evidence in the current WASM scalar ABI".into(),
    );
  }
  if mode == MapCodecMode::Decode
    && matches!(node, DataShapeNode::Tag)
    && static_hint.is_none_or(|ty| nil_type_evidence_is_open(ty) && !matches!(ty, CalcitTypeAnnotation::Never))
  {
    return Err(
      "E_WASM_NIL_TYPE_EVIDENCE: decode-map-as requires concrete static Tag type evidence in the current WASM scalar ABI".into(),
    );
  }

  match node {
    DataShapeNode::Dynamic => {
      // Preserve boxed JSON scalar tags (`nil`, `bool`, `number`) so Dynamic
      // fields keep their runtime `type-of` kind until decoded or narrowed.
      ctx.emit(Instruction::LocalGet(input));
      ctx.emit(Instruction::LocalSet(decoded));
      ctx.emit(Instruction::I32Const(1));
      ctx.emit(Instruction::LocalSet(ok));
    }
    DataShapeNode::Nil => {
      let allow_raw_zero = matches!(static_hint, Some(CalcitTypeAnnotation::Nil | CalcitTypeAnnotation::Never))
        || matches!(
          static_hint,
          Some(CalcitTypeAnnotation::Nullable(inner, NullableOrigin::Inferred)) if super::wasm_non_nil_value_is_nonzero(inner)
        );
      if allow_raw_zero || static_hint.is_none_or(nil_type_evidence_is_open) {
        let is_boxed_nil = emit_has_heap_tag(ctx, input, "nil");
        if allow_raw_zero {
          let is_heap = emit_is_heap_value(ctx, input);
          ctx.emit(Instruction::LocalGet(is_heap));
          ctx.emit(Instruction::I32Eqz);
          ctx.emit(Instruction::LocalGet(input));
          ctx.emit(f64_const(0.0));
          ctx.emit(Instruction::F64Eq);
          ctx.emit(Instruction::I32And);
          ctx.emit(Instruction::LocalGet(is_boxed_nil));
          ctx.emit(Instruction::I32Or);
        } else {
          ctx.emit(Instruction::LocalGet(is_boxed_nil));
        }
        ctx.begin_block_if();
        ctx.emit(f64_const(0.0));
        ctx.emit(Instruction::LocalSet(decoded));
        ctx.emit(Instruction::I32Const(1));
        ctx.emit(Instruction::LocalSet(ok));
        ctx.emit(Instruction::End);
      }
    }
    DataShapeNode::Unit => {
      if static_hint
        .is_none_or(|ty| nil_type_evidence_is_open(ty) || matches!(ty, CalcitTypeAnnotation::Unit | CalcitTypeAnnotation::Never))
      {
        let is_heap = emit_is_heap_value(ctx, input);
        ctx.emit(Instruction::LocalGet(is_heap));
        ctx.emit(Instruction::I32Eqz);
        ctx.emit(Instruction::LocalGet(input));
        ctx.emit(f64_const(0.0));
        ctx.emit(Instruction::F64Eq);
        ctx.emit(Instruction::I32And);
        ctx.begin_block_if();
        ctx.emit(f64_const(0.0));
        ctx.emit(Instruction::LocalSet(decoded));
        ctx.emit(Instruction::I32Const(1));
        ctx.emit(Instruction::LocalSet(ok));
        ctx.emit(Instruction::End);
      }
    }
    DataShapeNode::Bool => {
      if static_hint
        .is_none_or(|ty| nil_type_evidence_is_open(ty) || matches!(ty, CalcitTypeAnnotation::Bool | CalcitTypeAnnotation::Never))
      {
        let allow_raw_bool = matches!(static_hint, Some(CalcitTypeAnnotation::Bool | CalcitTypeAnnotation::Never));
        let is_boxed_bool = emit_has_heap_tag(ctx, input, "bool");
        ctx.emit(Instruction::LocalGet(is_boxed_bool));
        ctx.begin_block_if();
        let bool_val = ctx.alloc_local();
        ctx.emit(Instruction::LocalGet(input));
        ctx.emit(Instruction::I32TruncF64U);
        ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
        ctx.emit(Instruction::LocalTee(bool_val));
        ctx.emit(f64_const(0.0));
        ctx.emit(Instruction::F64Eq);
        ctx.emit(Instruction::LocalGet(bool_val));
        ctx.emit(f64_const(1.0));
        ctx.emit(Instruction::F64Eq);
        ctx.emit(Instruction::I32Or);
        ctx.begin_block_if();
        ctx.emit(Instruction::LocalGet(bool_val));
        ctx.emit(Instruction::LocalSet(decoded));
        ctx.emit(Instruction::I32Const(1));
        ctx.emit(Instruction::LocalSet(ok));
        ctx.emit(Instruction::End);
        if allow_raw_bool {
          ctx.emit(Instruction::Else);
          let is_heap = emit_is_heap_value(ctx, input);
          ctx.emit(Instruction::LocalGet(is_heap));
          ctx.emit(Instruction::I32Eqz);
          ctx.emit(Instruction::LocalGet(input));
          ctx.emit(f64_const(0.0));
          ctx.emit(Instruction::F64Eq);
          ctx.emit(Instruction::LocalGet(input));
          ctx.emit(f64_const(1.0));
          ctx.emit(Instruction::F64Eq);
          ctx.emit(Instruction::I32Or);
          ctx.emit(Instruction::I32And);
          ctx.begin_block_if();
          ctx.emit(Instruction::LocalGet(input));
          ctx.emit(Instruction::LocalSet(decoded));
          ctx.emit(Instruction::I32Const(1));
          ctx.emit(Instruction::LocalSet(ok));
          ctx.emit(Instruction::End);
        }
        ctx.emit(Instruction::End);
      }
    }
    DataShapeNode::Number => {
      if static_hint.is_none_or(|ty| {
        nil_type_evidence_is_open(ty)
          || matches!(
            ty,
            CalcitTypeAnnotation::Number | CalcitTypeAnnotation::Numeric(_) | CalcitTypeAnnotation::Never
          )
      }) {
        if is_proven_numeric(static_hint) {
          ctx.emit(Instruction::LocalGet(input));
          ctx.emit(Instruction::LocalSet(decoded));
          ctx.emit(Instruction::I32Const(1));
          ctx.emit(Instruction::LocalSet(ok));
        } else {
          let is_boxed_num = emit_has_heap_tag(ctx, input, "number");
          ctx.emit(Instruction::LocalGet(is_boxed_num));
          ctx.begin_block_if();
          ctx.emit(Instruction::LocalGet(input));
          ctx.emit(Instruction::I32TruncF64U);
          ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
          ctx.emit(Instruction::LocalSet(decoded));
          ctx.emit(Instruction::I32Const(1));
          ctx.emit(Instruction::LocalSet(ok));
          ctx.emit(Instruction::Else);
          let is_heap = emit_is_heap_value(ctx, input);
          ctx.emit(Instruction::LocalGet(is_heap));
          ctx.emit(Instruction::I32Eqz);
          ctx.begin_block_if();
          ctx.emit(Instruction::LocalGet(input));
          ctx.emit(Instruction::LocalSet(decoded));
          ctx.emit(Instruction::I32Const(1));
          ctx.emit(Instruction::LocalSet(ok));
          ctx.emit(Instruction::End);
          ctx.emit(Instruction::End);
        }
      }
    }
    DataShapeNode::Numeric(kind) => {
      if static_hint.is_none_or(|ty| {
        nil_type_evidence_is_open(ty)
          || matches!(
            ty,
            CalcitTypeAnnotation::Number | CalcitTypeAnnotation::Numeric(_) | CalcitTypeAnnotation::Never
          )
      }) {
        let num_val = ctx.alloc_local();
        let is_num_candidate = ctx.alloc_i32(0);
        if is_proven_numeric(static_hint) {
          ctx.emit(Instruction::LocalGet(input));
          ctx.emit(Instruction::LocalSet(num_val));
          ctx.emit(Instruction::I32Const(1));
          ctx.emit(Instruction::LocalSet(is_num_candidate));
        } else {
          let is_boxed_num = emit_has_heap_tag(ctx, input, "number");
          ctx.emit(Instruction::LocalGet(is_boxed_num));
          ctx.begin_block_if();
          ctx.emit(Instruction::LocalGet(input));
          ctx.emit(Instruction::I32TruncF64U);
          ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
          ctx.emit(Instruction::LocalSet(num_val));
          ctx.emit(Instruction::I32Const(1));
          ctx.emit(Instruction::LocalSet(is_num_candidate));
          ctx.emit(Instruction::Else);
          let is_heap = emit_is_heap_value(ctx, input);
          ctx.emit(Instruction::LocalGet(is_heap));
          ctx.emit(Instruction::I32Eqz);
          ctx.begin_block_if();
          ctx.emit(Instruction::LocalGet(input));
          ctx.emit(Instruction::LocalSet(num_val));
          ctx.emit(Instruction::I32Const(1));
          ctx.emit(Instruction::LocalSet(is_num_candidate));
          ctx.emit(Instruction::End);
          ctx.emit(Instruction::End);
        }
        ctx.emit(Instruction::LocalGet(is_num_candidate));
        ctx.begin_block_if();
        let in_range = ctx.alloc_local_typed(ValType::I32);
        emit_numeric_refinement_check(ctx, num_val, *kind, in_range);
        ctx.emit(Instruction::LocalGet(in_range));
        ctx.begin_block_if();
        ctx.emit(Instruction::LocalGet(num_val));
        ctx.emit(Instruction::LocalSet(decoded));
        ctx.emit(Instruction::I32Const(1));
        ctx.emit(Instruction::LocalSet(ok));
        ctx.emit(Instruction::Else);
        ctx.emit(Instruction::I32Const(1));
        ctx.emit(Instruction::LocalSet(error_kind));
        ctx.emit(Instruction::End);
        ctx.emit(Instruction::End);
      }
    }
    DataShapeNode::String => {
      let is_str = emit_has_heap_tag(ctx, input, "string");
      ctx.emit(Instruction::LocalGet(is_str));
      ctx.begin_block_if();
      ctx.emit(Instruction::LocalGet(input));
      ctx.emit(Instruction::LocalSet(decoded));
      ctx.emit(Instruction::I32Const(1));
      ctx.emit(Instruction::LocalSet(ok));
      ctx.emit(Instruction::End);
    }
    DataShapeNode::Tag => {
      if matches!(static_hint, Some(CalcitTypeAnnotation::Tag | CalcitTypeAnnotation::Never)) {
        let max_tag = ctx.tag_index.values().copied().max().unwrap_or(0) as f64;
        let is_heap = emit_is_heap_value(ctx, input);
        ctx.emit(Instruction::LocalGet(is_heap));
        ctx.emit(Instruction::I32Eqz);
        ctx.emit(Instruction::LocalGet(input));
        ctx.emit(Instruction::LocalGet(input));
        ctx.emit(Instruction::F64Trunc);
        ctx.emit(Instruction::F64Eq);
        ctx.emit(Instruction::I32And);
        ctx.emit(Instruction::LocalGet(input));
        ctx.emit(f64_const(1.0));
        ctx.emit(Instruction::F64Ge);
        ctx.emit(Instruction::I32And);
        ctx.emit(Instruction::LocalGet(input));
        ctx.emit(f64_const(max_tag));
        ctx.emit(Instruction::F64Le);
        ctx.emit(Instruction::I32And);
        ctx.begin_block_if();
        ctx.emit(Instruction::LocalGet(input));
        ctx.emit(Instruction::LocalSet(decoded));
        ctx.emit(Instruction::I32Const(1));
        ctx.emit(Instruction::LocalSet(ok));
        ctx.emit(Instruction::Else);
        ctx.emit(Instruction::I32Const(2));
        ctx.emit(Instruction::LocalSet(error_kind));
        ctx.emit(Instruction::End);
      }
    }
    DataShapeNode::MapOption { nominal, inner, .. } => match mode {
      MapCodecMode::Decode => emit_decode_map_option(ctx, graph, nominal, *inner, input, static_hint, outputs)?,
      MapCodecMode::Encode => emit_encode_map_option(ctx, graph, nominal, *inner, input, static_hint, outputs)?,
    },
    DataShapeNode::List(item_id) => emit_decode_map_list(ctx, graph, *item_id, input, static_hint, outputs, mode)?,
    DataShapeNode::Tuple(slots) => emit_decode_map_tuple(ctx, graph, slots, input, static_hint, outputs, mode)?,
    DataShapeNode::Set(item_id) => emit_decode_map_set(ctx, graph, *item_id, input, static_hint, outputs, mode)?,
    DataShapeNode::Map { key, value } => emit_decode_map_table(ctx, graph, (*key, *value), input, static_hint, outputs, mode)?,
    DataShapeNode::Struct { nominal, fields, .. } => match mode {
      MapCodecMode::Decode => emit_decode_map_struct(ctx, graph, nominal, fields, input, static_hint, outputs)?,
      MapCodecMode::Encode => emit_encode_map_struct(ctx, graph, nominal, fields, input, static_hint, outputs)?,
    },
    DataShapeNode::Enum { nominal, variants, .. } => {
      emit_decode_map_enum(ctx, graph, (nominal, variants), input, static_hint, outputs, mode)?
    }
    _ => unreachable!("shape was validated before map decoder emission"),
  }
  Ok(())
}

fn emit_encode_map_option(
  ctx: &mut WasmGenCtx,
  graph: &DataShapeGraph,
  nominal: &Arc<CalcitEnumDef>,
  inner: usize,
  input: u32,
  static_hint: Option<&CalcitTypeAnnotation>,
  outputs: (u32, u32, u32, u32),
) -> Result<(), String> {
  let (decoded, ok, error_kind, error_path) = outputs;
  let none_tag = *ctx.tag_index.get("none").ok_or("none tag missing")?;
  let some_tag = *ctx.tag_index.get("some").ok_or("some tag missing")?;
  if static_hint.is_none_or(|ty| nil_type_evidence_is_open(ty) && !matches!(ty, CalcitTypeAnnotation::Never)) {
    return Err(
      "E_WASM_NIL_TYPE_EVIDENCE: encode-map-as on Option requires nominal Option type evidence in the current WASM enum ABI".into(),
    );
  }
  let allow_option_enum = static_hint.is_some_and(|ty| {
    matches!(ty, CalcitTypeAnnotation::Never)
      || ty
        .resolve_to_enum()
        .is_some_and(|actual| actual.name() == nominal.name() || actual.same_nominal_definition(nominal))
  });
  if !allow_option_enum {
    return Ok(());
  }
  let is_enum = emit_has_heap_tag(ctx, input, "enum");
  ctx.emit(Instruction::LocalGet(is_enum));
  ctx.begin_block_if();
  let ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(input));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(ptr));
  let count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::LocalSet(count));
  let vtag = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
  ctx.emit(Instruction::LocalSet(vtag));

  ctx.emit(Instruction::LocalGet(vtag));
  ctx.emit(Instruction::I32Const(none_tag as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::I32And);
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(input));
  ctx.emit(Instruction::LocalSet(decoded));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(ok));
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(vtag));
  ctx.emit(Instruction::I32Const(some_tag as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::I32And);
  ctx.begin_block_if();
  let item = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalSet(item));
  let inner_encoded = ctx.alloc_local();
  let inner_ok = ctx.alloc_local_typed(ValType::I32);
  let inner_err = ctx.alloc_local_typed(ValType::I32);
  let inner_path = literal_local(ctx, "")?;
  let inner_hint = hint_option_inner(static_hint, nominal);
  let inner_is_dynamic_option = if matches!(graph.nodes.get(inner), Some(DataShapeNode::Dynamic)) {
    let Some(hint_ty) = inner_hint.as_ref() else {
      return Err(
        "E_WASM_NIL_TYPE_EVIDENCE: encode-map-as on Option<Dynamic> requires concrete inner payload type evidence in the current WASM ABI"
          .into(),
      );
    };
    if nil_type_evidence_is_open(hint_ty) && !matches!(hint_ty, CalcitTypeAnnotation::Never) {
      return Err(
        "E_WASM_NIL_TYPE_EVIDENCE: encode-map-as on Option<Dynamic> requires concrete inner payload type evidence in the current WASM ABI"
          .into(),
      );
    }
    if let Some(actual) = hint_ty.resolve_to_enum() {
      if actual.name() == nominal.name() || actual.same_nominal_definition(nominal) {
        true
      } else if actual
        .variants()
        .iter()
        .any(|v| (v.tag.ref_str() == "none" && v.arity() == 0) || (v.tag.ref_str() == "some" && v.arity() == 1))
      {
        return Err(format!(
          "E_WASM_EDN_ENUM_SHAPE: Option<Dynamic> payload enum :{} cannot be unambiguously represented in the WASM enum ABI because :{} defines a conflicting :none or :some variant",
          actual.name(),
          actual.name()
        ));
      } else {
        false
      }
    } else {
      false
    }
  } else {
    false
  };
  emit_decode_map_node(
    ctx,
    graph,
    inner,
    item,
    inner_hint.as_ref(),
    (inner_encoded, inner_ok, inner_err, inner_path),
    MapCodecMode::Encode,
  )?;
  ctx.emit(Instruction::LocalGet(inner_ok));
  ctx.begin_block_if();
  if matches!(graph.nodes.get(inner), Some(DataShapeNode::MapOption { .. })) || inner_is_dynamic_option {
    emit_option_enum(ctx, Some(inner_encoded))?;
    ctx.emit(Instruction::LocalSet(decoded));
  } else {
    ctx.emit(Instruction::LocalGet(inner_encoded));
    ctx.emit(Instruction::LocalSet(decoded));
  }
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(ok));
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(inner_err));
  ctx.emit(Instruction::LocalSet(error_kind));
  ctx.emit(Instruction::LocalGet(inner_path));
  ctx.emit(Instruction::LocalSet(error_path));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_decode_map_option(
  ctx: &mut WasmGenCtx,
  graph: &DataShapeGraph,
  nominal: &Arc<CalcitEnumDef>,
  inner: usize,
  input: u32,
  static_hint: Option<&CalcitTypeAnnotation>,
  outputs: (u32, u32, u32, u32),
) -> Result<(), String> {
  let (decoded, ok, error_kind, error_path) = outputs;
  let none_tag = *ctx.tag_index.get("none").ok_or("none tag missing")?;
  let some_tag = *ctx.tag_index.get("some").ok_or("some tag missing")?;
  let allow_option_enum = static_hint
    .and_then(|ty| ty.resolve_to_enum())
    .is_none_or(|actual| actual.name() == nominal.name() || actual.same_nominal_definition(nominal));
  let is_enum = emit_has_heap_tag(ctx, input, "enum");
  let handled_as_option_enum = ctx.alloc_i32(0);
  if allow_option_enum {
    ctx.emit(Instruction::LocalGet(is_enum));
    ctx.begin_block_if();
    let ptr = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(input));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(ptr));
    let count = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(ptr));
    ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
    ctx.emit(Instruction::LocalSet(count));
    let vtag = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(ptr));
    ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
    ctx.emit(Instruction::LocalSet(vtag));

    ctx.emit(Instruction::LocalGet(vtag));
    ctx.emit(Instruction::I32Const(none_tag as i32));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::LocalGet(count));
    ctx.emit(Instruction::I32Const(0));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::I32And);
    ctx.begin_block_if();
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::LocalSet(handled_as_option_enum));
    ctx.emit(Instruction::LocalGet(input));
    ctx.emit(Instruction::LocalSet(decoded));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::LocalSet(ok));
    ctx.emit(Instruction::Else);
    ctx.emit(Instruction::LocalGet(vtag));
    ctx.emit(Instruction::I32Const(some_tag as i32));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::LocalGet(count));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::I32And);
    ctx.begin_block_if();
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::LocalSet(handled_as_option_enum));
    let item = ctx.alloc_local();
    ctx.emit(Instruction::LocalGet(ptr));
    ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
    ctx.emit(Instruction::LocalSet(item));
    let inner_decoded = ctx.alloc_local();
    let inner_ok = ctx.alloc_local_typed(ValType::I32);
    let inner_err = ctx.alloc_local_typed(ValType::I32);
    let inner_path = literal_local(ctx, "")?;
    let inner_hint = hint_option_inner(static_hint, nominal);
    emit_decode_map_node(
      ctx,
      graph,
      inner,
      item,
      inner_hint.as_ref(),
      (inner_decoded, inner_ok, inner_err, inner_path),
      MapCodecMode::Decode,
    )?;
    ctx.emit(Instruction::LocalGet(inner_ok));
    ctx.begin_block_if();
    ctx.emit(Instruction::LocalGet(inner_decoded));
    ctx.emit(Instruction::I64ReinterpretF64);
    ctx.emit(Instruction::LocalGet(item));
    ctx.emit(Instruction::I64ReinterpretF64);
    ctx.emit(Instruction::I64Eq);
    ctx.begin_block_if();
    ctx.emit(Instruction::LocalGet(input));
    ctx.emit(Instruction::LocalSet(decoded));
    ctx.emit(Instruction::Else);
    emit_option_enum(ctx, Some(inner_decoded))?;
    ctx.emit(Instruction::LocalSet(decoded));
    ctx.emit(Instruction::End);
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::LocalSet(ok));
    ctx.emit(Instruction::Else);
    ctx.emit(Instruction::LocalGet(inner_err));
    ctx.emit(Instruction::LocalSet(error_kind));
    ctx.emit(Instruction::LocalGet(inner_path));
    ctx.emit(Instruction::LocalSet(error_path));
    ctx.emit(Instruction::End);
    ctx.emit(Instruction::End);
    ctx.emit(Instruction::End);
    ctx.emit(Instruction::End);
  }

  ctx.emit(Instruction::LocalGet(handled_as_option_enum));
  ctx.emit(Instruction::I32Eqz);
  ctx.begin_block_if();
  let inner_decoded = ctx.alloc_local();
  let inner_ok = ctx.alloc_local_typed(ValType::I32);
  let inner_err = ctx.alloc_local_typed(ValType::I32);
  let inner_path = literal_local(ctx, "")?;
  let raw_inner_hint = hint_option_inner(static_hint, nominal);
  emit_decode_map_node(
    ctx,
    graph,
    inner,
    input,
    raw_inner_hint.as_ref(),
    (inner_decoded, inner_ok, inner_err, inner_path),
    MapCodecMode::Decode,
  )?;
  ctx.emit(Instruction::LocalGet(inner_ok));
  ctx.begin_block_if();
  emit_option_enum(ctx, Some(inner_decoded))?;
  ctx.emit(Instruction::LocalSet(decoded));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(ok));
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(inner_err));
  ctx.emit(Instruction::LocalSet(error_kind));
  ctx.emit(Instruction::LocalGet(inner_path));
  ctx.emit(Instruction::LocalSet(error_path));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_decode_map_list(
  ctx: &mut WasmGenCtx,
  graph: &DataShapeGraph,
  item_id: usize,
  input: u32,
  static_hint: Option<&CalcitTypeAnnotation>,
  outputs: (u32, u32, u32, u32),
  mode: MapCodecMode,
) -> Result<(), String> {
  let (decoded, ok, error_kind, error_path) = outputs;
  let is_list = emit_has_heap_tag(ctx, input, "list");
  ctx.emit(Instruction::LocalGet(is_list));
  ctx.begin_block_if();
  let src_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(input));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(src_ptr));
  let count = emit_load_count_i32(ctx, src_ptr);
  let dst_ptr = emit_alloc_list(ctx, count);
  let valid = ctx.alloc_i32(1);
  let all_identical = ctx.alloc_i32(1);
  let i = ctx.alloc_i32(0);
  ctx.begin_block();
  ctx.begin_loop();
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::LocalGet(valid));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::I32Or);
  ctx.emit(Instruction::BrIf(1));
  let elem = ctx.alloc_local();
  emit_list_load_elem(ctx, src_ptr, i);
  ctx.emit(Instruction::LocalSet(elem));
  let item_val = ctx.alloc_local();
  let item_ok = ctx.alloc_local_typed(ValType::I32);
  let item_err = ctx.alloc_local_typed(ValType::I32);
  let item_path = literal_local(ctx, "")?;
  let inner_hint = hint_list_elem(static_hint);
  emit_decode_map_node(
    ctx,
    graph,
    item_id,
    elem,
    inner_hint.as_ref(),
    (item_val, item_ok, item_err, item_path),
    mode,
  )?;
  ctx.emit(Instruction::LocalGet(item_ok));
  ctx.begin_block_if();
  emit_list_store_elem(ctx, dst_ptr, i, item_val);
  ctx.emit(Instruction::LocalGet(item_val));
  ctx.emit(Instruction::I64ReinterpretF64);
  ctx.emit(Instruction::LocalGet(elem));
  ctx.emit(Instruction::I64ReinterpretF64);
  ctx.emit(Instruction::I64Ne);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(all_identical));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::LocalGet(item_err));
  ctx.emit(Instruction::LocalSet(error_kind));
  let open_bracket = literal_local(ctx, "[")?;
  let close_bracket = literal_local(ctx, "]")?;
  let index_str = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.call_rt("__rt_f64_to_str");
  ctx.emit(Instruction::LocalSet(index_str));
  let step1 = concat_string_locals(ctx, open_bracket, index_str);
  let step2 = concat_string_locals(ctx, step1, close_bracket);
  let step3 = concat_string_locals(ctx, step2, item_path);
  ctx.emit(Instruction::LocalGet(step3));
  ctx.emit(Instruction::LocalSet(error_path));
  ctx.emit(Instruction::End);
  ctx.i32_inc(i);
  ctx.br_loop();
  ctx.end_block_loop();
  ctx.emit(Instruction::LocalGet(valid));
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(all_identical));
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(input));
  ctx.emit(Instruction::LocalSet(decoded));
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(dst_ptr));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(decoded));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(ok));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_decode_map_tuple(
  ctx: &mut WasmGenCtx,
  graph: &DataShapeGraph,
  slots: &[usize],
  input: u32,
  static_hint: Option<&CalcitTypeAnnotation>,
  outputs: (u32, u32, u32, u32),
  mode: MapCodecMode,
) -> Result<(), String> {
  let (decoded, ok, error_kind, error_path) = outputs;
  let is_list = emit_has_heap_tag(ctx, input, "list");
  ctx.emit(Instruction::LocalGet(is_list));
  ctx.begin_block_if();
  let src_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(input));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(src_ptr));
  let count = emit_load_count_i32(ctx, src_ptr);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(slots.len() as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  let dst_ptr = emit_alloc_list(ctx, count);
  let valid = ctx.alloc_i32(1);
  for (idx, slot_id) in slots.iter().enumerate() {
    ctx.emit(Instruction::LocalGet(valid));
    ctx.begin_block_if();
    let elem = ctx.alloc_local();
    ctx.emit(Instruction::LocalGet(src_ptr));
    ctx.emit(Instruction::F64Load(mem_arg_f64(((1 + idx) * 8) as u64)));
    ctx.emit(Instruction::LocalSet(elem));
    let item_val = ctx.alloc_local();
    let item_ok = ctx.alloc_local_typed(ValType::I32);
    let item_err = ctx.alloc_local_typed(ValType::I32);
    let item_path = literal_local(ctx, "")?;
    let slot_hint = hint_tuple_slot(static_hint, idx);
    emit_decode_map_node(
      ctx,
      graph,
      *slot_id,
      elem,
      slot_hint.as_ref(),
      (item_val, item_ok, item_err, item_path),
      mode,
    )?;
    ctx.emit(Instruction::LocalGet(item_ok));
    ctx.begin_block_if();
    ctx.emit(Instruction::LocalGet(dst_ptr));
    ctx.emit(Instruction::LocalGet(item_val));
    ctx.emit(Instruction::F64Store(mem_arg_f64(((1 + idx) * 8) as u64)));
    ctx.emit(Instruction::Else);
    ctx.emit(Instruction::I32Const(0));
    ctx.emit(Instruction::LocalSet(valid));
    ctx.emit(Instruction::LocalGet(item_err));
    ctx.emit(Instruction::LocalSet(error_kind));
    let full_path = prepend_indexed_path(ctx, "[", idx, item_path)?;
    ctx.emit(Instruction::LocalGet(full_path));
    ctx.emit(Instruction::LocalSet(error_path));
    ctx.emit(Instruction::End);
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::LocalGet(valid));
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(dst_ptr));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(decoded));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(ok));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_decode_map_set(
  ctx: &mut WasmGenCtx,
  graph: &DataShapeGraph,
  item_id: usize,
  input: u32,
  static_hint: Option<&CalcitTypeAnnotation>,
  outputs: (u32, u32, u32, u32),
  mode: MapCodecMode,
) -> Result<(), String> {
  let (decoded, ok, error_kind, error_path) = outputs;
  let is_set = emit_has_heap_tag(ctx, input, "set");
  ctx.emit(Instruction::LocalGet(is_set));
  ctx.begin_block_if();
  let src_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(input));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(src_ptr));
  let count = emit_load_count_i32(ctx, src_ptr);
  let ts = ctx.i32_offset(count, 1);
  let dst_ptr = emit_alloc_with_count(ctx, count, ts, "set");
  let hashes_ptr = emit_alloc_list(ctx, count);
  let valid = ctx.alloc_i32(1);
  let i = ctx.alloc_i32(0);
  ctx.begin_block();
  ctx.begin_loop();
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::LocalGet(valid));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::I32Or);
  ctx.emit(Instruction::BrIf(1));
  let elem = ctx.alloc_local();
  emit_list_load_elem(ctx, src_ptr, i);
  ctx.emit(Instruction::LocalSet(elem));
  let item_val = ctx.alloc_local();
  let item_ok = ctx.alloc_local_typed(ValType::I32);
  let item_err = ctx.alloc_local_typed(ValType::I32);
  let item_path = literal_local(ctx, "")?;
  let inner_hint = hint_set_elem(static_hint);
  emit_decode_map_node(
    ctx,
    graph,
    item_id,
    elem,
    inner_hint.as_ref(),
    (item_val, item_ok, item_err, item_path),
    mode,
  )?;
  ctx.emit(Instruction::LocalGet(item_ok));
  ctx.begin_block_if();
  let item_hash = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(item_val));
  ctx.call_rt("__rt_value_hash");
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(item_hash));
  let dup = ctx.alloc_i32(0);
  let j = ctx.alloc_i32(0);
  ctx.begin_block();
  ctx.begin_loop();
  ctx.emit(Instruction::LocalGet(j));
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::LocalGet(dup));
  ctx.emit(Instruction::I32Or);
  ctx.emit(Instruction::BrIf(1));
  let prev_hash = ctx.alloc_local();
  emit_list_load_elem(ctx, hashes_ptr, j);
  ctx.emit(Instruction::LocalSet(prev_hash));
  ctx.emit(Instruction::LocalGet(prev_hash));
  ctx.emit(Instruction::LocalGet(item_hash));
  ctx.emit(Instruction::F64Eq);
  ctx.begin_block_if();
  let prev = ctx.alloc_local();
  emit_list_load_elem(ctx, dst_ptr, j);
  ctx.emit(Instruction::LocalSet(prev));
  ctx.emit(Instruction::LocalGet(prev));
  ctx.emit(Instruction::LocalGet(item_val));
  ctx.call_rt("__rt_value_equal");
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(dup));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.i32_inc(j);
  ctx.br_loop();
  ctx.end_block_loop();
  ctx.emit(Instruction::LocalGet(dup));
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::Else);
  emit_list_store_elem(ctx, dst_ptr, i, item_val);
  emit_list_store_elem(ctx, hashes_ptr, i, item_hash);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::LocalGet(item_err));
  ctx.emit(Instruction::LocalSet(error_kind));
  let full_path = prepend_static_path(ctx, ".item", item_path)?;
  ctx.emit(Instruction::LocalGet(full_path));
  ctx.emit(Instruction::LocalSet(error_path));
  ctx.emit(Instruction::End);
  ctx.i32_inc(i);
  ctx.br_loop();
  ctx.end_block_loop();
  ctx.emit(Instruction::LocalGet(valid));
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(dst_ptr));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(decoded));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(ok));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_decode_map_table(
  ctx: &mut WasmGenCtx,
  graph: &DataShapeGraph,
  kv: (usize, usize),
  input: u32,
  static_hint: Option<&CalcitTypeAnnotation>,
  outputs: (u32, u32, u32, u32),
  mode: MapCodecMode,
) -> Result<(), String> {
  let (key, value) = &kv;
  let (decoded, ok, error_kind, error_path) = outputs;
  let is_map = emit_has_heap_tag(ctx, input, "map");
  ctx.emit(Instruction::LocalGet(is_map));
  ctx.begin_block_if();
  let map_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(input));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(map_ptr));
  let flat_ptr = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", map_ptr);
  let count = emit_load_count_i32(ctx, flat_ptr);
  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));
  let dst_flat = emit_alloc_with_count(ctx, count, total_slots, "map");
  let valid = ctx.alloc_i32(1);
  let i = ctx.alloc_i32(0);
  ctx.begin_block();
  ctx.begin_loop();
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::LocalGet(valid));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::I32Or);
  ctx.emit(Instruction::BrIf(1));
  let entry_off = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(entry_off));
  let raw_k = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(flat_ptr));
  ctx.emit(Instruction::LocalGet(entry_off));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(raw_k));
  let raw_v = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(flat_ptr));
  ctx.emit(Instruction::LocalGet(entry_off));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalSet(raw_v));
  let (k_hint, v_hint) = hint_map_kv(static_hint);
  let k_val = ctx.alloc_local();
  let k_ok = ctx.alloc_local_typed(ValType::I32);
  let k_err = ctx.alloc_local_typed(ValType::I32);
  let k_path = literal_local(ctx, "")?;
  emit_decode_map_node(ctx, graph, *key, raw_k, k_hint.as_ref(), (k_val, k_ok, k_err, k_path), mode)?;
  let v_val = ctx.alloc_local();
  let v_ok = ctx.alloc_local_typed(ValType::I32);
  let v_err = ctx.alloc_local_typed(ValType::I32);
  let v_path = literal_local(ctx, "")?;
  emit_decode_map_node(ctx, graph, *value, raw_v, v_hint.as_ref(), (v_val, v_ok, v_err, v_path), mode)?;
  ctx.emit(Instruction::LocalGet(k_ok));
  ctx.emit(Instruction::LocalGet(v_ok));
  ctx.emit(Instruction::I32And);
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(dst_flat));
  ctx.emit(Instruction::LocalGet(entry_off));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(k_val));
  ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalGet(dst_flat));
  ctx.emit(Instruction::LocalGet(entry_off));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalGet(v_val));
  ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::LocalGet(k_ok));
  ctx.emit(Instruction::I32Eqz);
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(k_err));
  ctx.emit(Instruction::LocalSet(error_kind));
  let full_k_path = prepend_static_path(ctx, ".key", k_path)?;
  ctx.emit(Instruction::LocalGet(full_k_path));
  ctx.emit(Instruction::LocalSet(error_path));
  ctx.emit(Instruction::Else);
  ctx.emit(Instruction::LocalGet(v_err));
  ctx.emit(Instruction::LocalSet(error_kind));
  let full_v_path = prepend_static_path(ctx, ".value", v_path)?;
  ctx.emit(Instruction::LocalGet(full_v_path));
  ctx.emit(Instruction::LocalSet(error_path));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.i32_inc(i);
  ctx.br_loop();
  ctx.end_block_loop();
  ctx.emit(Instruction::LocalGet(valid));
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(dst_flat));
  ctx.call_rt("__rt_map_root_from_flat");
  let hashed_root = ctx.alloc_local_typed(ValType::I32);
  let actual_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalSet(hashed_root));
  ctx.emit(Instruction::LocalSet(actual_count));
  ctx.emit(Instruction::LocalGet(actual_count));
  ctx.emit(Instruction::LocalGet(count));
  ctx.emit(Instruction::I32Eq);
  ctx.begin_block_if();
  let out_map = emit_alloc_map_with_root(ctx, actual_count, hashed_root);
  ctx.emit(Instruction::LocalGet(out_map));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(decoded));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(ok));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_encode_map_struct(
  ctx: &mut WasmGenCtx,
  graph: &DataShapeGraph,
  nominal: &Arc<CalcitStructDef>,
  fields: &[(EdnTag, usize)],
  input: u32,
  static_hint: Option<&CalcitTypeAnnotation>,
  outputs: (u32, u32, u32, u32),
) -> Result<(), String> {
  let (decoded, ok, error_kind, error_path) = outputs;
  let struct_tag_id = ctx.struct_layouts.id(nominal)?;
  let layout = ctx.struct_layouts.layout_by_id(struct_tag_id)?.clone();
  let is_struct = emit_has_heap_tag(ctx, input, "struct");
  ctx.emit(Instruction::LocalGet(is_struct));
  ctx.begin_block_if();
  let ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(input));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(ptr));
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::I32Const(fields.len() as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
  ctx.emit(Instruction::I32Const(struct_tag_id as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::I32And);
  ctx.begin_block_if();
  let valid = ctx.alloc_i32(1);
  let values = fields.iter().map(|_| ctx.alloc_local()).collect::<Vec<_>>();
  let include_field = fields.iter().map(|_| ctx.alloc_i32(0)).collect::<Vec<_>>();
  let included_count = ctx.alloc_i32(0);
  for (idx, (field, child)) in fields.iter().enumerate() {
    ctx.emit(Instruction::LocalGet(valid));
    ctx.begin_block_if();
    let raw_val = ctx.alloc_local();
    emit_load_struct_field_as_repr(ctx, ptr, layout.fields[idx], NativeValRepr::F64);
    ctx.emit(Instruction::LocalSet(raw_val));
    let f_ok = ctx.alloc_local_typed(ValType::I32);
    let f_err = ctx.alloc_local_typed(ValType::I32);
    let f_path = literal_local(ctx, "")?;
    let field_hint = hint_struct_field(static_hint, nominal, idx);
    emit_decode_map_node(
      ctx,
      graph,
      *child,
      raw_val,
      field_hint.as_ref(),
      (values[idx], f_ok, f_err, f_path),
      MapCodecMode::Encode,
    )?;
    ctx.emit(Instruction::LocalGet(f_ok));
    ctx.begin_block_if();
    if matches!(graph.nodes.get(*child), Some(DataShapeNode::MapOption { .. })) {
      ctx.emit(Instruction::LocalGet(raw_val));
      ctx.emit(Instruction::I32TruncF64U);
      ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
      ctx.emit(Instruction::I32Const(0));
      ctx.emit(Instruction::I32Ne);
      ctx.begin_block_if();
      ctx.emit(Instruction::I32Const(1));
      ctx.emit(Instruction::LocalSet(include_field[idx]));
      ctx.i32_inc(included_count);
      ctx.emit(Instruction::End);
    } else {
      ctx.emit(Instruction::I32Const(1));
      ctx.emit(Instruction::LocalSet(include_field[idx]));
      ctx.i32_inc(included_count);
    }
    ctx.emit(Instruction::Else);
    ctx.emit(Instruction::I32Const(0));
    ctx.emit(Instruction::LocalSet(valid));
    ctx.emit(Instruction::LocalGet(f_err));
    ctx.emit(Instruction::LocalSet(error_kind));
    let full_path = prepend_field_path(ctx, field.ref_str(), f_path)?;
    ctx.emit(Instruction::LocalGet(full_path));
    ctx.emit(Instruction::LocalSet(error_path));
    ctx.emit(Instruction::End);
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::LocalGet(valid));
  ctx.begin_block_if();
  let total_slots = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(included_count));
  ctx.emit(Instruction::I32Const(2));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(total_slots));
  let dst_flat = emit_alloc_with_count(ctx, included_count, total_slots, "map");
  let write_off = ctx.alloc_i32(8);
  for (idx, (field, _)) in fields.iter().enumerate() {
    let field_tag_id = *ctx
      .tag_index
      .get(field.ref_str())
      .ok_or_else(|| format!("unknown struct field tag in WASM map encode: {field}"))? as f64;
    ctx.emit(Instruction::LocalGet(include_field[idx]));
    ctx.begin_block_if();
    ctx.emit(Instruction::LocalGet(dst_flat));
    ctx.emit(Instruction::LocalGet(write_off));
    ctx.emit(Instruction::I32Add);
    ctx.emit(f64_const(field_tag_id));
    ctx.emit(Instruction::F64Store(mem_arg_f64(0)));
    ctx.emit(Instruction::LocalGet(dst_flat));
    ctx.emit(Instruction::LocalGet(write_off));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalGet(values[idx]));
    ctx.emit(Instruction::F64Store(mem_arg_f64(8)));
    ctx.emit(Instruction::LocalGet(write_off));
    ctx.emit(Instruction::I32Const(16));
    ctx.emit(Instruction::I32Add);
    ctx.emit(Instruction::LocalSet(write_off));
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::LocalGet(dst_flat));
  ctx.call_rt("__rt_map_root_from_flat");
  let hashed_root = ctx.alloc_local_typed(ValType::I32);
  let actual_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalSet(hashed_root));
  ctx.emit(Instruction::LocalSet(actual_count));
  let out_map = emit_alloc_map_with_root(ctx, actual_count, hashed_root);
  ctx.emit(Instruction::LocalGet(out_map));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(decoded));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(ok));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_decode_map_struct(
  ctx: &mut WasmGenCtx,
  graph: &DataShapeGraph,
  nominal: &Arc<CalcitStructDef>,
  fields: &[(EdnTag, usize)],
  input: u32,
  static_hint: Option<&CalcitTypeAnnotation>,
  outputs: (u32, u32, u32, u32),
) -> Result<(), String> {
  let (decoded, ok, error_kind, error_path) = outputs;
  let struct_tag_id = ctx.struct_layouts.id(nominal)?;
  let layout = ctx.struct_layouts.layout_by_id(struct_tag_id)?.clone();
  let valid = ctx.alloc_i32(0);
  let input_is_struct = ctx.alloc_i32(0);
  let raw_values = fields.iter().map(|_| ctx.alloc_local()).collect::<Vec<_>>();
  let seen = fields.iter().map(|_| ctx.alloc_i32(0)).collect::<Vec<_>>();

  let is_struct = emit_has_heap_tag(ctx, input, "struct");
  ctx.emit(Instruction::LocalGet(is_struct));
  ctx.begin_block_if();
  let ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(input));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(ptr));
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::I32Const(fields.len() as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
  ctx.emit(Instruction::I32Const(struct_tag_id as i32));
  ctx.emit(Instruction::I32Eq);
  ctx.emit(Instruction::I32And);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(input_is_struct));
  for idx in 0..fields.len() {
    emit_load_struct_field_as_repr(ctx, ptr, layout.fields[idx], NativeValRepr::F64);
    ctx.emit(Instruction::LocalSet(raw_values[idx]));
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::LocalSet(seen[idx]));
  }
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::Else);
  let is_map = emit_has_heap_tag(ctx, input, "map");
  ctx.emit(Instruction::LocalGet(is_map));
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(valid));
  let map_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(input));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(map_ptr));
  let flat_ptr = emit_runtime_lookup_i32_to_i32(ctx, "__rt_map_linearize", map_ptr);
  let entry_count = emit_load_count_i32(ctx, flat_ptr);
  let i = ctx.alloc_i32(0);
  ctx.begin_block();
  ctx.begin_loop();
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::LocalGet(entry_count));
  ctx.emit(Instruction::I32GeU);
  ctx.emit(Instruction::LocalGet(valid));
  ctx.emit(Instruction::I32Eqz);
  ctx.emit(Instruction::I32Or);
  ctx.emit(Instruction::BrIf(1));
  let entry_off = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(i));
  ctx.emit(Instruction::I32Const(16));
  ctx.emit(Instruction::I32Mul);
  ctx.emit(Instruction::I32Const(8));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::LocalSet(entry_off));
  let key = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(flat_ptr));
  ctx.emit(Instruction::LocalGet(entry_off));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(0)));
  ctx.emit(Instruction::LocalSet(key));
  let val = ctx.alloc_local();
  ctx.emit(Instruction::LocalGet(flat_ptr));
  ctx.emit(Instruction::LocalGet(entry_off));
  ctx.emit(Instruction::I32Add);
  ctx.emit(Instruction::F64Load(mem_arg_f64(8)));
  ctx.emit(Instruction::LocalSet(val));
  let key_is_str = emit_has_heap_tag(ctx, key, "string");
  let key_is_heap = emit_is_heap_value(ctx, key);
  let matched = ctx.alloc_i32(0);
  for (idx, (field, _)) in fields.iter().enumerate() {
    let field_tag_id = *ctx
      .tag_index
      .get(field.ref_str())
      .ok_or_else(|| format!("unknown struct field tag in WASM map decode: {field}"))? as f64;
    let is_this_field = ctx.alloc_i32(0);
    ctx.emit(Instruction::LocalGet(key_is_heap));
    ctx.emit(Instruction::I32Eqz);
    ctx.emit(Instruction::LocalGet(key));
    ctx.emit(f64_const(field_tag_id));
    ctx.emit(Instruction::F64Eq);
    ctx.emit(Instruction::I32And);
    ctx.begin_block_if();
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::LocalSet(is_this_field));
    ctx.emit(Instruction::Else);
    ctx.emit(Instruction::LocalGet(key_is_str));
    ctx.begin_block_if();
    let key_ptr = ctx.alloc_local_typed(ValType::I32);
    ctx.emit(Instruction::LocalGet(key));
    ctx.emit(Instruction::I32TruncF64U);
    ctx.emit(Instruction::LocalSet(key_ptr));
    emit_string_equals_literal(ctx, key_ptr, field.ref_str())?;
    ctx.emit(Instruction::LocalSet(is_this_field));
    ctx.emit(Instruction::End);
    ctx.emit(Instruction::End);

    ctx.emit(Instruction::LocalGet(is_this_field));
    ctx.begin_block_if();
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::LocalSet(matched));
    ctx.emit(Instruction::LocalGet(seen[idx]));
    ctx.begin_block_if();
    ctx.emit(Instruction::I32Const(0));
    ctx.emit(Instruction::LocalSet(valid));
    ctx.emit(Instruction::Else);
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::LocalSet(seen[idx]));
    ctx.emit(Instruction::LocalGet(val));
    ctx.emit(Instruction::LocalSet(raw_values[idx]));
    ctx.emit(Instruction::End);
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::LocalGet(matched));
  ctx.emit(Instruction::I32Eqz);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(0));
  ctx.emit(Instruction::LocalSet(valid));
  ctx.emit(Instruction::End);
  ctx.i32_inc(i);
  ctx.br_loop();
  ctx.end_block_loop();
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);

  ctx.emit(Instruction::LocalGet(valid));
  ctx.begin_block_if();
  let all_identical = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(input_is_struct));
  ctx.emit(Instruction::LocalSet(all_identical));
  let (_, map_val_hint) = hint_map_kv(static_hint);
  let decoded_values = fields.iter().map(|_| ctx.alloc_local()).collect::<Vec<_>>();
  for (idx, (field, child)) in fields.iter().enumerate() {
    ctx.emit(Instruction::LocalGet(valid));
    ctx.begin_block_if();
    ctx.emit(Instruction::LocalGet(seen[idx]));
    ctx.begin_block_if();
    let f_ok = ctx.alloc_local_typed(ValType::I32);
    let f_err = ctx.alloc_local_typed(ValType::I32);
    let f_path = literal_local(ctx, "")?;
    let field_hint = if matches!(static_hint, Some(CalcitTypeAnnotation::Never)) {
      Some(CalcitTypeAnnotation::Never)
    } else if let Some(hint) = static_hint
      && !nil_type_evidence_is_open(hint)
      && !matches!(
        hint,
        CalcitTypeAnnotation::Map(_, _) | CalcitTypeAnnotation::Struct(_, _) | CalcitTypeAnnotation::StructValue(_)
      )
      && hint.resolve_to_struct().is_none()
    {
      Some(CalcitTypeAnnotation::Never)
    } else if static_hint
      .is_some_and(|ty| matches!(ty, CalcitTypeAnnotation::Map(k, _) if matches!(k.as_ref(), CalcitTypeAnnotation::StructValue(_))))
    {
      hint_encoded_struct_field(static_hint, nominal, idx)
    } else {
      hint_struct_field(static_hint, nominal, idx).or_else(|| map_val_hint.clone())
    };
    emit_decode_map_node(
      ctx,
      graph,
      *child,
      raw_values[idx],
      field_hint.as_ref(),
      (decoded_values[idx], f_ok, f_err, f_path),
      MapCodecMode::Decode,
    )?;
    ctx.emit(Instruction::LocalGet(f_ok));
    ctx.begin_block_if();
    ctx.emit(Instruction::LocalGet(all_identical));
    ctx.emit(Instruction::LocalGet(decoded_values[idx]));
    ctx.emit(Instruction::I64ReinterpretF64);
    ctx.emit(Instruction::LocalGet(raw_values[idx]));
    ctx.emit(Instruction::I64ReinterpretF64);
    ctx.emit(Instruction::I64Eq);
    ctx.emit(Instruction::I32And);
    ctx.emit(Instruction::LocalSet(all_identical));
    ctx.emit(Instruction::Else);
    ctx.emit(Instruction::I32Const(0));
    ctx.emit(Instruction::LocalSet(valid));
    ctx.emit(Instruction::LocalGet(f_err));
    ctx.emit(Instruction::LocalSet(error_kind));
    let full_path = prepend_field_path(ctx, field.ref_str(), f_path)?;
    ctx.emit(Instruction::LocalGet(full_path));
    ctx.emit(Instruction::LocalSet(error_path));
    ctx.emit(Instruction::End);
    ctx.emit(Instruction::Else);
    if matches!(graph.nodes.get(*child), Some(DataShapeNode::MapOption { .. })) {
      emit_option_enum(ctx, None)?;
      ctx.emit(Instruction::LocalSet(decoded_values[idx]));
      ctx.emit(Instruction::I32Const(0));
      ctx.emit(Instruction::LocalSet(all_identical));
    } else {
      ctx.emit(Instruction::I32Const(0));
      ctx.emit(Instruction::LocalSet(valid));
    }
    ctx.emit(Instruction::End);
    ctx.emit(Instruction::End);
  }

  ctx.emit(Instruction::LocalGet(valid));
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(all_identical));
  ctx.begin_block_if();
  ctx.emit(Instruction::LocalGet(input));
  ctx.emit(Instruction::LocalSet(decoded));
  ctx.emit(Instruction::Else);
  let struct_ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, layout.payload_bytes as i32, struct_ptr, "struct");
  emit_store_compact_header(ctx, struct_ptr, fields.len() as u32, struct_tag_id);
  for (index, value) in decoded_values.iter().enumerate() {
    emit_store_struct_field_from_f64_local(ctx, struct_ptr, layout.fields[index], *value);
  }
  ctx.emit(Instruction::LocalGet(struct_ptr));
  ctx.emit(Instruction::F64ConvertI32U);
  ctx.emit(Instruction::LocalSet(decoded));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::I32Const(1));
  ctx.emit(Instruction::LocalSet(ok));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

fn emit_decode_map_enum(
  ctx: &mut WasmGenCtx,
  graph: &DataShapeGraph,
  (nominal, variants): (&Arc<CalcitEnumDef>, &[(EdnTag, Vec<usize>)]),
  input: u32,
  static_hint: Option<&CalcitTypeAnnotation>,
  outputs: (u32, u32, u32, u32),
  mode: MapCodecMode,
) -> Result<(), String> {
  let (decoded, ok, error_kind, error_path) = outputs;
  // WASM enum heap objects do not carry a runtime definition tag, so only inputs
  // with static enum declaration evidence matching `nominal` can be validated safely.
  let enum_matches_nominal = matches!(static_hint, Some(CalcitTypeAnnotation::Never))
    || static_hint
      .and_then(|ty| ty.resolve_to_enum())
      .is_some_and(|actual| actual.same_nominal_definition(nominal));
  if !enum_matches_nominal {
    ctx.emit(Instruction::I32Const(5));
    ctx.emit(Instruction::LocalSet(error_kind));
    return Ok(());
  }
  let is_enum = emit_has_heap_tag(ctx, input, "enum");
  ctx.emit(Instruction::LocalGet(is_enum));
  ctx.begin_block_if();
  let ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(input));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(ptr));
  let count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(0)));
  ctx.emit(Instruction::LocalSet(count));
  let vtag = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(ptr));
  ctx.emit(Instruction::I32Load(mem_arg_i32(4)));
  ctx.emit(Instruction::LocalSet(vtag));
  let matched = ctx.alloc_i32(0);
  for (variant, payload_nodes) in variants {
    let expected_tag = *ctx.tag_index.get(variant.ref_str()).ok_or_else(|| {
      format!(
        "E_WASM_EDN_SHAPE: enum :{} variant :{} is not present in the compiled program",
        nominal.name(),
        variant
      )
    })?;
    ctx.emit(Instruction::LocalGet(vtag));
    ctx.emit(Instruction::I32Const(expected_tag as i32));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::LocalGet(count));
    ctx.emit(Instruction::I32Const(payload_nodes.len() as i32));
    ctx.emit(Instruction::I32Eq);
    ctx.emit(Instruction::I32And);
    ctx.begin_block_if();
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::LocalSet(matched));
    let valid = ctx.alloc_i32(1);
    let all_identical = ctx.alloc_i32(1);
    let values = payload_nodes.iter().map(|_| ctx.alloc_local()).collect::<Vec<_>>();
    for (pidx, pnode) in payload_nodes.iter().enumerate() {
      ctx.emit(Instruction::LocalGet(valid));
      ctx.begin_block_if();
      let raw_p = ctx.alloc_local();
      ctx.emit(Instruction::LocalGet(ptr));
      ctx.emit(Instruction::F64Load(mem_arg_f64(((1 + pidx) * 8) as u64)));
      ctx.emit(Instruction::LocalSet(raw_p));
      let p_ok = ctx.alloc_local_typed(ValType::I32);
      let p_err = ctx.alloc_local_typed(ValType::I32);
      let p_path = literal_local(ctx, "")?;
      let payload_hint = hint_enum_payload(static_hint, nominal, variant, pidx);
      emit_decode_map_node(
        ctx,
        graph,
        *pnode,
        raw_p,
        payload_hint.as_ref(),
        (values[pidx], p_ok, p_err, p_path),
        mode,
      )?;
      ctx.emit(Instruction::LocalGet(p_ok));
      ctx.begin_block_if();
      ctx.emit(Instruction::LocalGet(all_identical));
      ctx.emit(Instruction::LocalGet(values[pidx]));
      ctx.emit(Instruction::I64ReinterpretF64);
      ctx.emit(Instruction::LocalGet(raw_p));
      ctx.emit(Instruction::I64ReinterpretF64);
      ctx.emit(Instruction::I64Eq);
      ctx.emit(Instruction::I32And);
      ctx.emit(Instruction::LocalSet(all_identical));
      ctx.emit(Instruction::Else);
      ctx.emit(Instruction::I32Const(0));
      ctx.emit(Instruction::LocalSet(valid));
      ctx.emit(Instruction::LocalGet(p_err));
      ctx.emit(Instruction::LocalSet(error_kind));
      let full_path = prepend_indexed_path(ctx, ".payload[", pidx, p_path)?;
      ctx.emit(Instruction::LocalGet(full_path));
      ctx.emit(Instruction::LocalSet(error_path));
      ctx.emit(Instruction::End);
      ctx.emit(Instruction::End);
    }
    ctx.emit(Instruction::LocalGet(valid));
    ctx.begin_block_if();
    ctx.emit(Instruction::LocalGet(all_identical));
    ctx.begin_block_if();
    ctx.emit(Instruction::LocalGet(input));
    ctx.emit(Instruction::LocalSet(decoded));
    ctx.emit(Instruction::Else);
    let enum_ptr = ctx.alloc_local_typed(ValType::I32);
    emit_bump_alloc(ctx, ((1 + payload_nodes.len()) * 8) as i32, enum_ptr, "enum");
    emit_store_compact_header(ctx, enum_ptr, payload_nodes.len() as u32, expected_tag);
    for (payload_index, value) in values.iter().enumerate() {
      ctx.emit(Instruction::LocalGet(enum_ptr));
      ctx.emit(Instruction::LocalGet(*value));
      ctx.emit(Instruction::F64Store(mem_arg_f64(((1 + payload_index) * 8) as u64)));
    }
    ctx.emit(Instruction::LocalGet(enum_ptr));
    ctx.emit(Instruction::F64ConvertI32U);
    ctx.emit(Instruction::LocalSet(decoded));
    ctx.emit(Instruction::End);
    ctx.emit(Instruction::I32Const(1));
    ctx.emit(Instruction::LocalSet(ok));
    ctx.emit(Instruction::End);
    ctx.emit(Instruction::End);
  }
  ctx.emit(Instruction::LocalGet(matched));
  ctx.emit(Instruction::I32Eqz);
  ctx.begin_block_if();
  ctx.emit(Instruction::I32Const(5));
  ctx.emit(Instruction::LocalSet(error_kind));
  ctx.emit(Instruction::End);
  ctx.emit(Instruction::End);
  Ok(())
}

const MAX_EDN_DECODE_EXPANDED_NODES: usize = 4096;

fn validate_decode_shape(graph: &DataShapeGraph, node_id: usize, depth: usize) -> Result<(), String> {
  let mut expanded_nodes = 0;
  validate_decode_shape_with_budget(graph, node_id, depth, &mut expanded_nodes)
}

fn validate_decode_shape_with_budget(
  graph: &DataShapeGraph,
  node_id: usize,
  depth: usize,
  expanded_nodes: &mut usize,
) -> Result<(), String> {
  if depth > MAX_EDN_DECODE_SHAPE_DEPTH {
    return Err(format!(
      "E_WASM_EDN_DEPTH: typed Cirru EDN decode shape nesting exceeds {MAX_EDN_DECODE_SHAPE_DEPTH}"
    ));
  }
  *expanded_nodes = expanded_nodes.saturating_add(1);
  if *expanded_nodes > MAX_EDN_DECODE_EXPANDED_NODES {
    return Err(format!(
      "E_WASM_EDN_DEPTH: typed Cirru EDN decode shape expansion exceeds {MAX_EDN_DECODE_EXPANDED_NODES} nodes"
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
    DataShapeNode::List(item) => validate_decode_shape_with_budget(graph, *item, depth + 1, expanded_nodes),
    DataShapeNode::Map { key, value } => {
      validate_scalar_map_key_shape(graph, *key)?;
      validate_decode_shape_with_budget(graph, *value, depth + 1, expanded_nodes)
    }
    DataShapeNode::Struct { fields, .. } => {
      for (_, field) in fields {
        validate_decode_shape_with_budget(graph, *field, depth + 1, expanded_nodes)?;
      }
      Ok(())
    }
    DataShapeNode::Enum { variants, .. } => {
      for (_, payloads) in variants {
        for payload in payloads {
          validate_decode_shape_with_budget(graph, *payload, depth + 1, expanded_nodes)?;
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
  let error_path = literal_local(ctx, "")?;
  emit_parse_node_with_path(ctx, graph, node_id, token, token_ptr, (parsed, ok, error_kind, error_path))
}

fn emit_parse_node_with_path(
  ctx: &mut WasmGenCtx,
  graph: &DataShapeGraph,
  node_id: usize,
  token: u32,
  token_ptr: u32,
  outputs: (u32, u32, u32, u32),
) -> Result<(), String> {
  let (parsed, ok, error_kind, error_path) = outputs;
  let node = graph
    .nodes
    .get(node_id)
    .ok_or_else(|| format!("E_WASM_EDN_SHAPE: typed Cirru EDN parser references missing node #{node_id}"))?;
  let (token, token_ptr) = emit_unwrap_nested_token(ctx, token, token_ptr);
  match node {
    DataShapeNode::List(item) => emit_parse_scalar_list(ctx, graph, *item, token_ptr, (parsed, ok, error_kind, error_path)),
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
  let struct_tag_id = ctx.struct_layouts.id(nominal)?;
  let layout = ctx.struct_layouts.layout_by_id(struct_tag_id)?.clone();
  let struct_ptr = ctx.alloc_local_typed(ValType::I32);
  emit_bump_alloc(ctx, layout.payload_bytes as i32, struct_ptr, "struct");
  emit_store_compact_header(ctx, struct_ptr, fields.len() as u32, struct_tag_id);
  for (index, value) in values.iter().enumerate() {
    emit_store_struct_field_from_f64_local(ctx, struct_ptr, layout.fields[index], *value);
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
    ctx.emit(Instruction::LocalGet(valid));
    ctx.emit(Instruction::I32Eqz);
    ctx.begin_block_if();
    ctx.emit(Instruction::I32Const(5));
    ctx.emit(Instruction::LocalSet(error_kind));
    ctx.emit(Instruction::End);

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
    emit_bump_alloc(ctx, ((1 + payload_nodes.len()) * 8) as i32, enum_ptr, "enum");
    emit_store_compact_header(ctx, enum_ptr, payload_nodes.len() as u32, tag_id);
    for (payload_index, value) in values.iter().enumerate() {
      ctx.emit(Instruction::LocalGet(enum_ptr));
      ctx.emit(Instruction::LocalGet(*value));
      ctx.emit(Instruction::F64Store(mem_arg_f64(((1 + payload_index) * 8) as u64)));
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
  outputs: (u32, u32, u32, u32),
) -> Result<(), String> {
  let (parsed, ok, error_kind, error_path) = outputs;
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
    let item_error_path = literal_local(ctx, "")?;
    emit_parse_node_with_path(
      ctx,
      graph,
      item_id,
      item_token,
      item_ptr,
      (item_value, item_ok, item_error, item_error_path),
    )?;
    ctx.emit(Instruction::LocalGet(item_ok));
    ctx.begin_block_if();
    emit_list_store_elem(ctx, list_ptr, index, item_value);
    ctx.emit(Instruction::Else);
    ctx.emit(Instruction::I32Const(0));
    ctx.emit(Instruction::LocalSet(valid));
    ctx.emit(Instruction::LocalGet(item_error));
    ctx.emit(Instruction::LocalSet(error_kind));
    let open_bracket = literal_local(ctx, "[")?;
    let close_bracket = literal_local(ctx, "]")?;
    let index_str = ctx.alloc_local();
    ctx.emit(Instruction::LocalGet(index));
    ctx.emit(Instruction::F64ConvertI32U);
    ctx.call_rt("__rt_f64_to_str");
    ctx.emit(Instruction::LocalSet(index_str));
    let step1 = concat_string_locals(ctx, open_bracket, index_str);
    let step2 = concat_string_locals(ctx, step1, close_bracket);
    let step3 = concat_string_locals(ctx, step2, item_error_path);
    ctx.emit(Instruction::LocalGet(step3));
    ctx.emit(Instruction::LocalSet(error_path));
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
  let actual_count = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalSet(hashed_root));
  ctx.emit(Instruction::LocalSet(actual_count));
  let map_ptr = emit_alloc_map_with_root(ctx, actual_count, hashed_root);

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

fn concat_string_locals(ctx: &mut WasmGenCtx, left: u32, right: u32) -> u32 {
  let left_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(left));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(left_ptr));
  let right_ptr = ctx.alloc_local_typed(ValType::I32);
  ctx.emit(Instruction::LocalGet(right));
  ctx.emit(Instruction::I32TruncF64U);
  ctx.emit(Instruction::LocalSet(right_ptr));
  emit_str_concat_from_ptrs(ctx, left_ptr, right_ptr);
  let output = ctx.alloc_local();
  ctx.emit(Instruction::LocalSet(output));
  output
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

  #[test]
  fn map_decode_validates_closed_shapes_and_rejects_union_or_ref() {
    let accepted = DataShapeGraph::from_nodes(
      0,
      vec![
        DataShapeNode::List(1),
        DataShapeNode::Tuple(vec![2, 3]),
        DataShapeNode::Number,
        DataShapeNode::Dynamic,
      ],
    )
    .expect("test shape should be well formed");
    assert!(validate_map_decode_shape(&accepted, accepted.root, 0).is_ok());

    let rejected_ref =
      DataShapeGraph::from_nodes(0, vec![DataShapeNode::Ref(1), DataShapeNode::Number]).expect("test shape should be well formed");
    let ref_err = validate_map_decode_shape(&rejected_ref, rejected_ref.root, 0).expect_err("Ref decode must fail closed in WASM");
    assert!(ref_err.starts_with("E_WASM_EDN_SHAPE:"));

    let option_enum = super::super::tests_option_enum_def_for_codec();
    let flag_enum = super::super::tests_flag_enum_def_with_none();
    let conflicting_option = DataShapeGraph::from_nodes(
      0,
      vec![
        DataShapeNode::MapOption {
          nominal: option_enum,
          nominal_path: None,
          inner: 1,
        },
        DataShapeNode::Enum {
          nominal: flag_enum,
          nominal_path: None,
          type_args: Arc::from([]),
          variants: vec![(EdnTag::new("none"), vec![])],
        },
      ],
    )
    .expect("test shape should be well formed");
    let conflict_err = validate_map_decode_shape(&conflicting_option, conflicting_option.root, 0)
      .expect_err("Option<Flag> with 0-payload :none must fail closed in WASM");
    assert!(conflict_err.starts_with("E_WASM_EDN_ENUM_SHAPE:"), "{conflict_err}");

    // A 10-layer binary Tuple DAG has depth 10 (<= 32) but expands > 512 nodes.
    let mut dag_nodes = Vec::new();
    for idx in 0..10 {
      dag_nodes.push(DataShapeNode::Tuple(vec![idx + 1, idx + 1]));
    }
    dag_nodes.push(DataShapeNode::Number);
    let dag = DataShapeGraph::from_nodes(0, dag_nodes).expect("test DAG shape should be well formed");
    let dag_err = validate_map_decode_shape(&dag, dag.root, 0).expect_err("exponential shared-node DAG must hit the expansion budget");
    assert!(dag_err.starts_with("E_WASM_EDN_DEPTH:"), "{dag_err}");

    let wide_fields = (0..512).map(|idx| (EdnTag::new(format!("f{idx}")), 1)).collect::<Vec<_>>();
    let number_field = Arc::new(CalcitTypeAnnotation::Number);
    let wide_struct = Arc::new(CalcitStructDef {
      field_defaults: Default::default(),
      definition_ref: Some(Arc::from("app.main/Wide512")),
      name: EdnTag::new("Wide512"),
      fields: Arc::new(wide_fields.iter().map(|(tag, _)| tag.clone()).collect()),
      field_types: Arc::new(vec![number_field; 512]),
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    });
    let wide_edn_graph = DataShapeGraph::from_nodes(
      0,
      vec![
        DataShapeNode::Struct {
          nominal: wide_struct,
          nominal_path: None,
          type_args: Arc::from([]),
          fields: wide_fields,
        },
        DataShapeNode::Number,
      ],
    )
    .expect("wide struct shape should be well formed");
    assert!(validate_decode_shape(&wide_edn_graph, wide_edn_graph.root, 0).is_ok());
    assert!(validate_map_decode_shape(&wide_edn_graph, wide_edn_graph.root, 0).is_ok());
  }

  #[test]
  fn map_encode_preserves_struct_field_enum_evidence_and_requires_scalar_proof() {
    let state_enum = super::super::tests_state_enum_def_for_codec();
    let state_ty = Arc::new(CalcitTypeAnnotation::Enum(state_enum, Arc::new(vec![])));
    let job_def = Arc::new(CalcitStructDef {
      field_defaults: Default::default(),
      definition_ref: Some(Arc::from("app.main/Job")),
      name: EdnTag::new("Job"),
      fields: Arc::new(vec![EdnTag::new("id"), EdnTag::new("state")]),
      field_types: Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number), state_ty.clone()]),
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    });
    let job_ty = Arc::new(CalcitTypeAnnotation::Struct(job_def.clone(), Arc::new(vec![])));
    let codec_hint = super::super::infer_encoded_codec_hint(&job_ty);
    assert_eq!(
      hint_encoded_struct_field(Some(codec_hint.as_ref()), &job_def, 0),
      Some(CalcitTypeAnnotation::Number)
    );
    assert_eq!(
      hint_encoded_struct_field(Some(codec_hint.as_ref()), &job_def, 1),
      Some(state_ty.as_ref().clone())
    );
    // An ordinary Map<Tag, Job> must NOT be treated as an encoded Job struct.
    let map_of_jobs = CalcitTypeAnnotation::Map(Arc::new(CalcitTypeAnnotation::Tag), job_ty);
    assert_eq!(hint_encoded_struct_field(Some(&map_of_jobs), &job_def, 0), None);

    let single_field_job = Arc::new(CalcitStructDef {
      field_defaults: Default::default(),
      definition_ref: Some(Arc::from("app.main/SingleFieldJob")),
      name: EdnTag::new("SingleFieldJob"),
      fields: Arc::new(vec![EdnTag::new("state")]),
      field_types: Arc::new(vec![state_ty.clone()]),
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    });
    let single_field_ty = Arc::new(CalcitTypeAnnotation::Struct(single_field_job, Arc::new(vec![])));
    assert_eq!(
      super::super::infer_encoded_map_type(&single_field_ty),
      Arc::new(CalcitTypeAnnotation::Map(Arc::new(CalcitTypeAnnotation::Tag), state_ty))
    );
  }

  #[test]
  fn nil_decode_guard_accepts_inferred_nonzero_nullable_raw_zero() {
    let env = WasmCompileEnv {
      function_value_dependencies: Default::default(),
      dyn_traits: Default::default(),
      fn_index: HashMap::new(),
      fn_arity: HashMap::new(),
      fn_has_rest: HashMap::new(),
      fn_signatures: HashMap::new(),
      has_boxed_json_scalars: true,
      runtime_fn_index: HashMap::new(),
      tag_index: HashMap::from([("nil".to_owned(), 0)]),
      struct_layouts: Default::default(),
      string_pool: HashMap::new(),
      atom_globals: HashMap::new(),
      atom_is_scalar: HashMap::new(),
      value_imports: HashMap::new(),
      static_fn_defs: HashMap::new(),
      fn_table_index: HashMap::new(),
      host_imports: HashMap::new(),
      target: WasmTarget::Core,
      boundary: WasmBoundary::Native,
      world: WasiWorld::Command,
      any_type_table: Default::default(),
    };
    let graph = DataShapeGraph::from_nodes(0, vec![DataShapeNode::Nil]).expect("nil shape");
    let path = literal_local(&mut WasmGenCtx::new(0, env.clone()), "").ok();
    assert!(
      path.is_none(),
      "minimal env omits string pool; emit_decode_map_node uses empty error_path"
    );

    let emit_nil_case = |hint: Option<&CalcitTypeAnnotation>| {
      let mut ctx = WasmGenCtx::new(1, env.clone());
      let input = 0;
      let decoded = ctx.alloc_local();
      let ok = ctx.alloc_i32(0);
      let err_kind = ctx.alloc_i32(0);
      let err_path = ctx.alloc_local();
      ctx.map_codec_expanded_nodes = 0;
      emit_decode_map_node(
        &mut ctx,
        &graph,
        0,
        input,
        hint,
        (decoded, ok, err_kind, err_path),
        MapCodecMode::Decode,
      )
      .expect("nil decode emission succeeds");
      ctx.instructions
    };

    let has_raw_zero_eq = |insns: &[Instruction]| {
      insns
        .windows(2)
        .any(|w| matches!(w, [Instruction::F64Const(v), Instruction::F64Eq] if f64::from_bits(v.bits()) == 0.0))
    };

    let inferred_nullable_str = CalcitTypeAnnotation::Nullable(
      Arc::new(CalcitTypeAnnotation::String),
      crate::wasm::host::calcit::NullableOrigin::Inferred,
    );
    let inferred_insns = emit_nil_case(Some(&inferred_nullable_str));
    assert!(
      has_raw_zero_eq(&inferred_insns),
      "Nullable(String, Inferred) must allow raw 0.0 nil check"
    );

    let dynamic_insns = emit_nil_case(Some(&CalcitTypeAnnotation::Dynamic));
    assert!(
      !has_raw_zero_eq(&dynamic_insns),
      "open Dynamic must require boxed nil tag rather than raw 0.0"
    );
  }
}
