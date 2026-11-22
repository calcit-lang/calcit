use std::collections::HashMap;

use wasm_encoder::{BlockType, Instruction, ValType};

use super::component::{push_load_compact_struct_field_f64, push_store_f64_into_compact_struct_field};
use super::runtime::{HostImport, RuntimeFnBuilder};
use super::structs::{StructIdentity, StructLayout, StructLayouts};
use super::*;

pub(super) const MAX_HTTP_BODY_BYTES: i32 = 4 * 1024 * 1024;
const HTTP_RETPTR_BYTES: i32 = 64;
const HTTP_READ_CHUNK_BYTES: i64 = 64 * 1024;
const HTTP_WRITE_CHUNK_BYTES: i32 = 4096;

pub(super) const HTTP_STATIC_CONTENT_TYPE_KEY: &str = "content-type";
pub(super) const HTTP_STATIC_CONTENT_TYPE_TEXT_PLAIN: &str = "text/plain; charset=utf-8";
pub(super) const HTTP_STATIC_BAD_REQUEST_MSG: &str = "invalid HTTP request";
pub(super) const HTTP_STATIC_BODY_TOO_LARGE_MSG: &str = "HTTP request body exceeds 4 MiB limit";
pub(super) const HTTP_STATIC_INTERNAL_ERROR_MSG: &str = "handler failed";

pub(super) struct WasiHttpLayoutInfo {
  pub(super) header_layout_id: u32,
  pub(super) header_field_count: u32,
  pub(super) header_layout: StructLayout,
  pub(super) header_name_idx: usize,
  pub(super) header_value_idx: usize,
  pub(super) request_layout_id: u32,
  pub(super) request_field_count: u32,
  pub(super) request_layout: StructLayout,
  pub(super) req_method_idx: usize,
  pub(super) req_path_idx: usize,
  pub(super) req_headers_idx: usize,
  pub(super) req_body_idx: usize,
  pub(super) response_layout: StructLayout,
  pub(super) resp_status_idx: usize,
  pub(super) resp_headers_idx: usize,
  pub(super) resp_body_idx: usize,
}

pub(super) fn wasi_http_proxy_imports(include_stdio: bool) -> Vec<HostImport> {
  let http_types = "wasi:http/types@0.2.12";
  let io_streams = "wasi:io/streams@0.2.12";
  let io_error = "wasi:io/error@0.2.12";
  let mut imports = vec![
    HostImport {
      module: http_types.into(),
      name: "[resource-drop]incoming-request".into(),
      params: vec![ValType::I32],
      results: vec![],
    },
    HostImport {
      module: http_types.into(),
      name: "[method]incoming-request.method".into(),
      params: vec![ValType::I32, ValType::I32],
      results: vec![],
    },
    HostImport {
      module: http_types.into(),
      name: "[method]incoming-request.path-with-query".into(),
      params: vec![ValType::I32, ValType::I32],
      results: vec![],
    },
    HostImport {
      module: http_types.into(),
      name: "[method]incoming-request.headers".into(),
      params: vec![ValType::I32],
      results: vec![ValType::I32],
    },
    HostImport {
      module: http_types.into(),
      name: "[method]incoming-request.consume".into(),
      params: vec![ValType::I32, ValType::I32],
      results: vec![],
    },
    HostImport {
      module: http_types.into(),
      name: "[resource-drop]incoming-body".into(),
      params: vec![ValType::I32],
      results: vec![],
    },
    HostImport {
      module: http_types.into(),
      name: "[method]incoming-body.stream".into(),
      params: vec![ValType::I32, ValType::I32],
      results: vec![],
    },
    HostImport {
      module: http_types.into(),
      name: "[constructor]fields".into(),
      params: vec![],
      results: vec![ValType::I32],
    },
    HostImport {
      module: http_types.into(),
      name: "[resource-drop]fields".into(),
      params: vec![ValType::I32],
      results: vec![],
    },
    HostImport {
      module: http_types.into(),
      name: "[method]fields.entries".into(),
      params: vec![ValType::I32, ValType::I32],
      results: vec![],
    },
    HostImport {
      module: http_types.into(),
      name: "[method]fields.append".into(),
      params: vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32, ValType::I32, ValType::I32],
      results: vec![],
    },
    HostImport {
      module: http_types.into(),
      name: "[constructor]outgoing-response".into(),
      params: vec![ValType::I32],
      results: vec![ValType::I32],
    },
    HostImport {
      module: http_types.into(),
      name: "[resource-drop]outgoing-response".into(),
      params: vec![ValType::I32],
      results: vec![],
    },
    HostImport {
      module: http_types.into(),
      name: "[method]outgoing-response.set-status-code".into(),
      params: vec![ValType::I32, ValType::I32],
      results: vec![ValType::I32],
    },
    HostImport {
      module: http_types.into(),
      name: "[method]outgoing-response.body".into(),
      params: vec![ValType::I32, ValType::I32],
      results: vec![],
    },
    HostImport {
      module: http_types.into(),
      name: "[resource-drop]outgoing-body".into(),
      params: vec![ValType::I32],
      results: vec![],
    },
    HostImport {
      module: http_types.into(),
      name: "[method]outgoing-body.write".into(),
      params: vec![ValType::I32, ValType::I32],
      results: vec![],
    },
    HostImport {
      module: http_types.into(),
      name: "[static]outgoing-body.finish".into(),
      params: vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32],
      results: vec![],
    },
    HostImport {
      module: http_types.into(),
      name: "[static]response-outparam.set".into(),
      params: vec![
        ValType::I32,
        ValType::I32,
        ValType::I32,
        ValType::I32,
        ValType::I64,
        ValType::I32,
        ValType::I32,
        ValType::I32,
        ValType::I32,
      ],
      results: vec![],
    },
    HostImport {
      module: io_streams.into(),
      name: "[method]input-stream.blocking-read".into(),
      params: vec![ValType::I32, ValType::I64, ValType::I32],
      results: vec![],
    },
    HostImport {
      module: io_streams.into(),
      name: "[resource-drop]input-stream".into(),
      params: vec![ValType::I32],
      results: vec![],
    },
    HostImport {
      module: io_streams.into(),
      name: "[method]output-stream.blocking-write-and-flush".into(),
      params: vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32],
      results: vec![],
    },
    HostImport {
      module: io_streams.into(),
      name: "[resource-drop]output-stream".into(),
      params: vec![ValType::I32],
      results: vec![],
    },
    HostImport {
      module: io_error.into(),
      name: "[resource-drop]error".into(),
      params: vec![ValType::I32],
      results: vec![],
    },
  ];
  if include_stdio {
    imports.push(HostImport {
      module: "wasi:cli/stdout@0.2.12".into(),
      name: "get-stdout".into(),
      params: vec![],
      results: vec![ValType::I32],
    });
    imports.push(HostImport {
      module: "wasi:cli/stderr@0.2.12".into(),
      name: "get-stderr".into(),
      params: vec![],
      results: vec![ValType::I32],
    });
  }
  imports
}

fn is_core_nominal_struct(annotation: &Arc<CalcitTypeAnnotation>, declaring_ns: &str, expected_name: &str) -> bool {
  let expanded = expand_wasm_value_type(annotation);
  if let Some((def, ref_pair)) = expanded.resolve_to_struct_with_ref() {
    let expected_ref = format!("{}/{expected_name}", crate::wasm::host::calcit::CORE_NS);
    return def.definition_ref.as_deref() == Some(expected_ref.as_str())
      || matches!(ref_pair, Some((ns, name)) if ns.as_ref() == crate::wasm::host::calcit::CORE_NS && name.as_ref() == expected_name);
  }
  if let CalcitTypeAnnotation::TypeRef(name, args) = expanded.as_ref()
    && args.is_empty()
  {
    return matches!(
      program::resolve_type_name_in_ns(declaring_ns, name.as_ref()),
      Some((ns, def)) if ns.as_ref() == crate::wasm::host::calcit::CORE_NS && def.as_ref() == expected_name
    );
  }
  false
}

fn resolve_core_result_args(
  annotation: &Arc<CalcitTypeAnnotation>,
  declaring_ns: &str,
) -> Option<(Arc<CalcitTypeAnnotation>, Arc<CalcitTypeAnnotation>)> {
  let expanded = expand_wasm_value_type(annotation);
  match expanded.as_ref() {
    CalcitTypeAnnotation::Enum(enum_def, args) if args.len() == 2 => {
      let expected_ref = format!("{}/Result", crate::wasm::host::calcit::CORE_NS);
      (enum_def.definition_ref().map(AsRef::as_ref) == Some(expected_ref.as_str())).then(|| (args[0].clone(), args[1].clone()))
    }
    CalcitTypeAnnotation::TypeRef(name, args) if args.len() == 2 => {
      match program::resolve_type_name_in_ns(declaring_ns, name.as_ref()) {
        Some((ns, def)) if ns.as_ref() == crate::wasm::host::calcit::CORE_NS && def.as_ref() == "Result" => {
          Some((args[0].clone(), args[1].clone()))
        }
        _ => None,
      }
    }
    _ => None,
  }
}

pub(super) fn validate_wasi_http_target_in_program(
  program_data: &program::CompiledProgram,
  init_ns: &str,
  init_def: &str,
) -> Result<(), String> {
  let qualified_init = format!("{init_ns}/{init_def}");
  let compiled_init = program_data
    .get(init_ns)
    .and_then(|file| file.defs.get(init_def))
    .ok_or_else(|| format!("E_WASI_HTTP_ENTRY: HTTP handler `{qualified_init}` was not compiled"))?;
  let (args, _) = extract_fn_parts(&compiled_init.preprocessed_code)
    .map_err(|reason| format!("E_WASI_HTTP_ENTRY: HTTP handler `{qualified_init}` is not a function: {reason}"))?;
  let (init_arity, rest) = compute_fn_arity(&args);
  if init_arity != 1 || rest.is_some() {
    return Err(format!(
      "E_WASI_HTTP_ENTRY: WASI HTTP handler `{qualified_init}` must take exactly 1 `HttpRequest` argument"
    ));
  }
  Ok(())
}

pub(super) fn validate_wasi_http_component_in_program(
  program_data: &program::CompiledProgram,
  init_ns: &str,
  init_def: &str,
) -> Result<(), String> {
  for namespace in deterministic_namespace_order(program_data, None) {
    let file = program_data.get(namespace.as_str()).expect("ordered namespace must exist");
    for (name, compiled) in deterministic_definition_order(file) {
      if is_wasm_export_def(&compiled.preprocessed_code) {
        return Err(format!(
          "E_WASI_HTTP_EXPORT: `{namespace}/{name}` declares a generic Component export; `calcit wasi --world http` only exports `wasi:http/incoming-handler@0.2.12`"
        ));
      }
    }
  }
  validate_wasi_http_target_in_program(program_data, init_ns, init_def)?;
  let qualified_init = format!("{init_ns}/{init_def}");
  let compiled_init = program_data
    .get(init_ns)
    .and_then(|file| file.defs.get(init_def))
    .ok_or_else(|| format!("E_WASI_HTTP_ENTRY: HTTP handler `{qualified_init}` was not compiled"))?;
  let signature = qualified_fn_schema(&compiled_init.schema, init_ns).ok_or_else(|| {
    format!("E_WASI_HTTP_ENTRY: `{qualified_init}` needs an explicit schema `Fn HttpRequest (-> (Result HttpResponse String))`")
  })?;
  if signature.rest_type.is_some()
    || signature.arg_types.len() != 1
    || !is_core_nominal_struct(&signature.arg_types[0], init_ns, "HttpRequest")
  {
    return Err(format!(
      "E_WASI_HTTP_ENTRY: `{qualified_init}` parameter schema must be `HttpRequest`"
    ));
  }
  let Some((ok_type, err_type)) = resolve_core_result_args(&signature.return_type, init_ns) else {
    return Err(format!(
      "E_WASI_HTTP_ENTRY: `{qualified_init}` return schema must be `Result HttpResponse String`"
    ));
  };
  let err_expanded = expand_wasm_value_type(&err_type);
  if !is_core_nominal_struct(&ok_type, init_ns, "HttpResponse") || !matches!(err_expanded.as_ref(), CalcitTypeAnnotation::String) {
    return Err(format!(
      "E_WASI_HTTP_ENTRY: `{qualified_init}` return schema must be `Result HttpResponse String`"
    ));
  }
  Ok(())
}

fn lookup_core_struct_def(program_data: &program::CompiledProgram, name: &str) -> Result<CalcitStructDef, String> {
  let core_ns = crate::wasm::host::calcit::CORE_NS;
  let ensure_ref = |def: CalcitStructDef| {
    if def.definition_ref.is_some() {
      def
    } else {
      def.with_definition_ref(core_ns, name)
    }
  };
  if let Some(Calcit::StructDef(def)) = program::lookup_runtime_ready(core_ns, name) {
    return Ok(ensure_ref(def.as_ref().clone()));
  }
  if let Some(compiled) = program_data.get(core_ns).and_then(|file| file.defs.get(name)) {
    match compiled.schema.as_ref() {
      CalcitTypeAnnotation::StructDef(def) | CalcitTypeAnnotation::Struct(def, _) | CalcitTypeAnnotation::StructValue(def) => {
        return Ok(ensure_ref(def.as_ref().clone()));
      }
      _ => {}
    }
    for code in compiled
      .source_code
      .iter()
      .chain([&compiled.preprocessed_code, &compiled.codegen_form])
    {
      if let Calcit::StructDef(def) = code {
        return Ok(ensure_ref(def.as_ref().clone()));
      }
      if let Some(def) = try_parse_defrecord_form(code) {
        return Ok(ensure_ref(def));
      }
      if let Some(Calcit::StructDef(def)) = crate::wasm::host::calcit::type_annotation::resolve_type_def_from_code(code) {
        return Ok(ensure_ref(def.as_ref().clone()));
      }
    }
  }
  Err(format!("missing `calcit.core/{name}` StructDef for WASI HTTP proxy"))
}

fn field_index(def: &CalcitStructDef, field_name: &str) -> Result<usize, String> {
  def
    .fields
    .iter()
    .position(|field| field.ref_str() == field_name)
    .ok_or_else(|| format!("missing field `:{field_name}` on `calcit.core/{}`", def.name))
}

pub(super) fn resolve_wasi_http_layout_info(
  program_data: &program::CompiledProgram,
  struct_layouts: &StructLayouts,
) -> Result<WasiHttpLayoutInfo, String> {
  let header_def = lookup_core_struct_def(program_data, "HttpHeader")?;
  let request_def = lookup_core_struct_def(program_data, "HttpRequest")?;
  let response_def = lookup_core_struct_def(program_data, "HttpResponse")?;

  let header_id = *struct_layouts
    .ids
    .get(&StructIdentity::from(&header_def))
    .ok_or_else(|| "missing WASM struct layout for `calcit.core/HttpHeader`".to_owned())?;
  let request_id = *struct_layouts
    .ids
    .get(&StructIdentity::from(&request_def))
    .ok_or_else(|| "missing WASM struct layout for `calcit.core/HttpRequest`".to_owned())?;
  let response_id = *struct_layouts
    .ids
    .get(&StructIdentity::from(&response_def))
    .ok_or_else(|| "missing WASM struct layout for `calcit.core/HttpResponse`".to_owned())?;

  Ok(WasiHttpLayoutInfo {
    header_layout_id: header_id,
    header_field_count: header_def.fields.len() as u32,
    header_layout: struct_layouts.layout_by_id(header_id)?.clone(),
    header_name_idx: field_index(&header_def, "name")?,
    header_value_idx: field_index(&header_def, "value")?,
    request_layout_id: request_id,
    request_field_count: request_def.fields.len() as u32,
    request_layout: struct_layouts.layout_by_id(request_id)?.clone(),
    req_method_idx: field_index(&request_def, "method")?,
    req_path_idx: field_index(&request_def, "path")?,
    req_headers_idx: field_index(&request_def, "headers")?,
    req_body_idx: field_index(&request_def, "body")?,
    response_layout: struct_layouts.layout_by_id(response_id)?.clone(),
    resp_status_idx: field_index(&response_def, "status")?,
    resp_headers_idx: field_index(&response_def, "headers")?,
    resp_body_idx: field_index(&response_def, "body")?,
  })
}

/// Write `len` bytes from `ptr` to WASI 0.2 `output-stream` in `<= 4096`-byte chunks.
/// Returns `0` on success or `1` on stream error after dropping any `io-error` handle.
pub(super) fn build_wasi_http_write_all_fn(host_imports: &HashMap<(String, String), u32>) -> CompiledFn {
  let blocking_write = host_imports[&(
    "wasi:io/streams@0.2.12".into(),
    "[method]output-stream.blocking-write-and-flush".into(),
  )];
  let error_drop = host_imports[&("wasi:io/error@0.2.12".into(), "[resource-drop]error".into())];

  // params: 0 = stream (i32), 1 = ptr (i32), 2 = len (i32), 3 = retptr (i32)
  let mut b = RuntimeFnBuilder::new(4);
  let offset = b.alloc_i32();
  let remaining = b.alloc_i32();
  let chunk = b.alloc_i32();

  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::LocalSet(offset));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::LocalSet(remaining));

  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(remaining));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::BrIf(1));

  b.emit(Instruction::LocalGet(remaining));
  b.emit(Instruction::I32Const(HTTP_WRITE_CHUNK_BYTES));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Result(ValType::I32)));
  b.emit(Instruction::I32Const(HTTP_WRITE_CHUNK_BYTES));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(remaining));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalSet(chunk));

  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(offset));
  b.emit(Instruction::LocalGet(chunk));
  b.emit(Instruction::LocalGet(3));
  b.emit(Instruction::Call(blocking_write));

  b.emit(Instruction::LocalGet(3));
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(3));
  b.emit(Instruction::I32Load8U(mem_arg_byte(4)));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(3));
  b.emit(Instruction::I32Load(mem_arg_i32(8)));
  b.emit(Instruction::Call(error_drop));
  b.emit(Instruction::End);
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(offset));
  b.emit(Instruction::LocalGet(chunk));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(offset));
  b.emit(Instruction::LocalGet(remaining));
  b.emit(Instruction::LocalGet(chunk));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::LocalSet(remaining));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::I32Const(0));
  b.finish(vec![ValType::I32; 4], vec![ValType::I32])
}

/// Emit a complete `text/plain; charset=utf-8` HTTP response directly via `response-outparam`.
pub(super) fn build_wasi_http_send_plain_fn(
  host_imports: &HashMap<(String, String), u32>,
  string_pool: &HashMap<String, u32>,
  write_all_idx: u32,
) -> CompiledFn {
  let http_types = "wasi:http/types@0.2.12";
  let io_streams = "wasi:io/streams@0.2.12";
  let fields_new = host_imports[&(http_types.into(), "[constructor]fields".into())];
  let fields_drop = host_imports[&(http_types.into(), "[resource-drop]fields".into())];
  let fields_append = host_imports[&(http_types.into(), "[method]fields.append".into())];
  let resp_new = host_imports[&(http_types.into(), "[constructor]outgoing-response".into())];
  let resp_drop = host_imports[&(http_types.into(), "[resource-drop]outgoing-response".into())];
  let resp_set_status = host_imports[&(http_types.into(), "[method]outgoing-response.set-status-code".into())];
  let resp_body = host_imports[&(http_types.into(), "[method]outgoing-response.body".into())];
  let body_drop = host_imports[&(http_types.into(), "[resource-drop]outgoing-body".into())];
  let body_write = host_imports[&(http_types.into(), "[method]outgoing-body.write".into())];
  let body_finish = host_imports[&(http_types.into(), "[static]outgoing-body.finish".into())];
  let outparam_set = host_imports[&(http_types.into(), "[static]response-outparam.set".into())];
  let stream_drop = host_imports[&(io_streams.into(), "[resource-drop]output-stream".into())];

  let ct_name_ptr = string_pool[HTTP_STATIC_CONTENT_TYPE_KEY] as i32 + 8;
  let ct_name_len = HTTP_STATIC_CONTENT_TYPE_KEY.len() as i32;
  let ct_val_ptr = string_pool[HTTP_STATIC_CONTENT_TYPE_TEXT_PLAIN] as i32 + 8;
  let ct_val_len = HTTP_STATIC_CONTENT_TYPE_TEXT_PLAIN.len() as i32;

  // params: 0 = response_out (i32), 1 = status (i32), 2 = msg_ptr (i32), 3 = msg_len (i32), 4 = retptr (i32)
  let mut b = RuntimeFnBuilder::new(5);
  let fields = b.alloc_i32();
  let resp = b.alloc_i32();
  let body = b.alloc_i32();
  let stream = b.alloc_i32();

  b.emit(Instruction::Call(fields_new));
  b.emit(Instruction::LocalTee(fields));
  b.emit(Instruction::I32Const(ct_name_ptr));
  b.emit(Instruction::I32Const(ct_name_len));
  b.emit(Instruction::I32Const(ct_val_ptr));
  b.emit(Instruction::I32Const(ct_val_len));
  b.emit(Instruction::LocalGet(4));
  b.emit(Instruction::Call(fields_append));
  b.emit(Instruction::LocalGet(4));
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(fields));
  b.emit(Instruction::Call(fields_drop));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(fields));
  b.emit(Instruction::Call(resp_new));
  b.emit(Instruction::LocalTee(resp));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::Call(resp_set_status));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(resp));
  b.emit(Instruction::Call(resp_drop));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(resp));
  b.emit(Instruction::LocalGet(4));
  b.emit(Instruction::Call(resp_body));
  b.emit(Instruction::LocalGet(4));
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(resp));
  b.emit(Instruction::Call(resp_drop));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(4));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalSet(body));

  b.emit(Instruction::LocalGet(body));
  b.emit(Instruction::LocalGet(4));
  b.emit(Instruction::Call(body_write));
  b.emit(Instruction::LocalGet(4));
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(body));
  b.emit(Instruction::Call(body_drop));
  b.emit(Instruction::LocalGet(resp));
  b.emit(Instruction::Call(resp_drop));
  b.emit(Instruction::Return);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(4));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalSet(stream));

  push_response_outparam_ok(&mut b, outparam_set, 0, resp);

  b.emit(Instruction::LocalGet(stream));
  b.emit(Instruction::LocalGet(2));
  b.emit(Instruction::LocalGet(3));
  b.emit(Instruction::LocalGet(4));
  b.emit(Instruction::Call(write_all_idx));
  b.emit(Instruction::Drop);

  b.emit(Instruction::LocalGet(stream));
  b.emit(Instruction::Call(stream_drop));
  b.emit(Instruction::LocalGet(body));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalGet(4));
  b.emit(Instruction::Call(body_finish));

  b.finish(vec![ValType::I32; 5], vec![])
}

fn push_response_outparam_ok(b: &mut RuntimeFnBuilder, outparam_set: u32, response_out_local: u32, resp_local: u32) {
  b.emit(Instruction::LocalGet(response_out_local));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalGet(resp_local));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I64Const(0));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Call(outparam_set));
}

fn push_cabi_free_if_nonzero(b: &mut RuntimeFnBuilder, cabi_free_idx: u32, ptr_local: u32, byte_len_expr: &[Instruction<'static>]) {
  b.emit(Instruction::LocalGet(ptr_local));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Ne);
  for instr in byte_len_expr {
    b.emit(instr.clone());
  }
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::I32And);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(ptr_local));
  for instr in byte_len_expr {
    b.emit(instr.clone());
  }
  b.emit(Instruction::Call(cabi_free_idx));
  b.emit(Instruction::End);
}

fn push_alloc_enum(
  b: &mut RuntimeFnBuilder,
  arena_alloc_idx: u32,
  enum_tag_id: i32,
  variant_tag_id_local: u32,
  payload_local: Option<u32>,
  dst_f64_local: u32,
) {
  let payload_count = i32::from(payload_local.is_some());
  let raw_base = b.alloc_i32();
  let ptr = b.alloc_i32();
  b.emit(Instruction::I32Const((payload_count + 2) * 8));
  b.emit(Instruction::Call(arena_alloc_idx));
  b.emit(Instruction::LocalTee(raw_base));
  b.emit(Instruction::I32Const(HEAP_MAGIC));
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(raw_base));
  b.emit(Instruction::I32Const(enum_tag_id));
  b.emit(Instruction::I32Store(mem_arg_i32(4)));
  b.emit(Instruction::LocalGet(raw_base));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalTee(ptr));
  b.emit(Instruction::I32Const(payload_count));
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::LocalGet(variant_tag_id_local));
  b.emit(Instruction::I32Store(mem_arg_i32(4)));
  if let Some(payload) = payload_local {
    b.emit(Instruction::LocalGet(ptr));
    b.emit(Instruction::LocalGet(payload));
    b.emit(Instruction::F64Store(mem_arg_f64(8)));
  }
  b.emit(Instruction::LocalGet(ptr));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::LocalSet(dst_f64_local));
}

fn push_turn_cleanup_and_return(b: &mut RuntimeFnBuilder, active_turns_global: u32, comp: ComponentMemoryGlobals, turn_exit_idx: u32) {
  // This exported request handler holds no post-return host buffers once finished.
  b.emit(Instruction::GlobalGet(active_turns_global));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::GlobalSet(comp.total_live));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::GlobalSet(comp.free_head));
  b.emit(Instruction::End);
  b.emit(Instruction::Call(turn_exit_idx));
  b.emit(Instruction::Return);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_wasi_http_incoming_handler_fn(
  handler_idx: u32,
  write_all_idx: u32,
  send_plain_idx: u32,
  layout_info: &WasiHttpLayoutInfo,
  host_imports: &HashMap<(String, String), u32>,
  runtime_fn_index: &HashMap<String, u32>,
  tag_index: &HashMap<String, u32>,
  string_pool: &HashMap<String, u32>,
  active_turns_global: u32,
  comp_globals: ComponentMemoryGlobals,
) -> CompiledFn {
  let http_types = "wasi:http/types@0.2.12";
  let io_streams = "wasi:io/streams@0.2.12";
  let io_error = "wasi:io/error@0.2.12";

  let req_drop = host_imports[&(http_types.into(), "[resource-drop]incoming-request".into())];
  let req_method = host_imports[&(http_types.into(), "[method]incoming-request.method".into())];
  let req_path = host_imports[&(http_types.into(), "[method]incoming-request.path-with-query".into())];
  let req_headers = host_imports[&(http_types.into(), "[method]incoming-request.headers".into())];
  let req_consume = host_imports[&(http_types.into(), "[method]incoming-request.consume".into())];
  let in_body_drop = host_imports[&(http_types.into(), "[resource-drop]incoming-body".into())];
  let in_body_stream = host_imports[&(http_types.into(), "[method]incoming-body.stream".into())];
  let fields_new = host_imports[&(http_types.into(), "[constructor]fields".into())];
  let fields_drop = host_imports[&(http_types.into(), "[resource-drop]fields".into())];
  let fields_entries = host_imports[&(http_types.into(), "[method]fields.entries".into())];
  let fields_append = host_imports[&(http_types.into(), "[method]fields.append".into())];
  let resp_new = host_imports[&(http_types.into(), "[constructor]outgoing-response".into())];
  let resp_drop = host_imports[&(http_types.into(), "[resource-drop]outgoing-response".into())];
  let resp_set_status = host_imports[&(http_types.into(), "[method]outgoing-response.set-status-code".into())];
  let resp_body = host_imports[&(http_types.into(), "[method]outgoing-response.body".into())];
  let out_body_drop = host_imports[&(http_types.into(), "[resource-drop]outgoing-body".into())];
  let out_body_write = host_imports[&(http_types.into(), "[method]outgoing-body.write".into())];
  let out_body_finish = host_imports[&(http_types.into(), "[static]outgoing-body.finish".into())];
  let outparam_set = host_imports[&(http_types.into(), "[static]response-outparam.set".into())];
  let in_stream_read = host_imports[&(io_streams.into(), "[method]input-stream.blocking-read".into())];
  let in_stream_drop = host_imports[&(io_streams.into(), "[resource-drop]input-stream".into())];
  let out_stream_drop = host_imports[&(io_streams.into(), "[resource-drop]output-stream".into())];
  let error_drop = host_imports[&(io_error.into(), "[resource-drop]error".into())];

  let arena_alloc_idx = runtime_fn_index["__rt_arena_alloc"];
  let cabi_free_idx = runtime_fn_index["__cabi_free"];
  let str_new_idx = runtime_fn_index["__str_new"];
  let utf8_valid_idx = runtime_fn_index["__rt_utf8_valid"];
  let turn_enter_idx = runtime_fn_index["__rt_component_turn_enter"];
  let turn_exit_idx = runtime_fn_index["__rt_component_turn_exit"];

  let enum_tag_id = tag_index["enum"] as i32;
  let struct_tag_id = tag_index["struct"] as i32;
  let list_tag_id = tag_index["list"] as i32;
  let str_tag_id = tag_index["string"] as i32;
  let buffer_tag_id = tag_index["buffer"] as i32;
  let ok_tag_id = tag_index["ok"] as i32;
  let err_tag_id = tag_index["err"] as i32;
  let body_empty_tag_id = tag_index["empty"] as i32;
  let body_text_tag_id = tag_index["text"] as i32;
  let body_bytes_tag_id = tag_index["bytes"] as i32;
  let method_tag_ids = [
    tag_index["get"] as i32,
    tag_index["head"] as i32,
    tag_index["post"] as i32,
    tag_index["put"] as i32,
    tag_index["delete"] as i32,
    tag_index["connect"] as i32,
    tag_index["options"] as i32,
    tag_index["trace"] as i32,
    tag_index["patch"] as i32,
  ];
  let method_other_tag_id = tag_index["other"] as i32;

  let bad_req_ptr = string_pool[HTTP_STATIC_BAD_REQUEST_MSG] as i32 + 8;
  let bad_req_len = HTTP_STATIC_BAD_REQUEST_MSG.len() as i32;
  let too_large_ptr = string_pool[HTTP_STATIC_BODY_TOO_LARGE_MSG] as i32 + 8;
  let too_large_len = HTTP_STATIC_BODY_TOO_LARGE_MSG.len() as i32;
  let internal_err_ptr = string_pool[HTTP_STATIC_INTERNAL_ERROR_MSG] as i32 + 8;
  let internal_err_len = HTTP_STATIC_INTERNAL_ERROR_MSG.len() as i32;

  // params: 0 = incoming_request (i32), 1 = response_outparam (i32)
  let mut b = RuntimeFnBuilder::new(2);
  let retptr = b.alloc_i32();
  let disc = b.alloc_i32();
  let raw_ptr = b.alloc_i32();
  let raw_len = b.alloc_i32();
  let method_f64 = b.alloc_f64();
  let path_f64 = b.alloc_f64();
  let var_tag_tmp = b.alloc_i32();
  let payload_f64_tmp = b.alloc_f64();

  let in_fields = b.alloc_i32();
  let entries_ptr = b.alloc_i32();
  let entries_len = b.alloc_i32();
  let entry_idx = b.alloc_i32();
  let entry_ptr = b.alloc_i32();
  let name_ptr = b.alloc_i32();
  let name_len = b.alloc_i32();
  let val_ptr = b.alloc_i32();
  let val_len = b.alloc_i32();
  let bad_header = b.alloc_i32();
  let list_raw = b.alloc_i32();
  let headers_list_ptr = b.alloc_i32();
  let headers_f64 = b.alloc_f64();
  let name_f64 = b.alloc_f64();
  let val_f64 = b.alloc_f64();
  let hdr_struct_raw = b.alloc_i32();
  let hdr_struct_ptr = b.alloc_i32();

  let in_body = b.alloc_i32();
  let in_stream = b.alloc_i32();
  let chunk_head = b.alloc_i32();
  let chunk_tail = b.alloc_i32();
  let chunk_node = b.alloc_i32();
  let chunk_next = b.alloc_i32();
  let body_obj_raw = b.alloc_i32();
  let body_obj_ptr = b.alloc_i32();
  let write_off = b.alloc_i32();
  let staging_len = b.alloc_i32();
  let body_status = b.alloc_i32(); // 0 = ok, 1 = stream_err, 2 = too_large
  let body_f64 = b.alloc_f64();

  let req_struct_raw = b.alloc_i32();
  let req_struct_ptr = b.alloc_i32();
  let req_f64 = b.alloc_f64();

  let result_f64 = b.alloc_f64();
  let result_ptr = b.alloc_i32();
  let result_tag = b.alloc_i32();
  let err_str_ptr = b.alloc_i32();
  let err_msg_len = b.alloc_i32();
  let resp_ptr = b.alloc_i32();
  let status_f64 = b.alloc_f64();
  let status_i32 = b.alloc_i32();
  let resp_headers_f64 = b.alloc_f64();
  let resp_headers_ptr = b.alloc_i32();
  let resp_body_f64 = b.alloc_f64();
  let resp_body_ptr = b.alloc_i32();
  let resp_body_tag = b.alloc_i32();

  let out_fields = b.alloc_i32();
  let out_bytes_ptr = b.alloc_i32();
  let out_bytes_len = b.alloc_i32();
  let payload_obj_ptr = b.alloc_i32();
  let out_resp = b.alloc_i32();
  let out_body = b.alloc_i32();
  let out_stream = b.alloc_i32();

  // 1. Enter request Scratch Arena turn and allocate 64-byte 8-byte-aligned retptr.
  b.emit(Instruction::Call(turn_enter_idx));
  b.emit(Instruction::I32Const(HTTP_RETPTR_BYTES));
  b.emit(Instruction::Call(arena_alloc_idx));
  b.emit(Instruction::LocalSet(retptr));

  // 2. Lift incoming-request.method -> calcit.core/HttpMethod.
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(req_method));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::LocalTee(disc));
  b.emit(Instruction::I32Const(9));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalSet(raw_ptr));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::I32Load(mem_arg_i32(8)));
  b.emit(Instruction::LocalSet(raw_len));
  b.emit(Instruction::LocalGet(raw_ptr));
  b.emit(Instruction::LocalGet(raw_len));
  b.emit(Instruction::Call(str_new_idx));
  b.emit(Instruction::LocalSet(payload_f64_tmp));
  push_cabi_free_if_nonzero(&mut b, cabi_free_idx, raw_ptr, &[Instruction::LocalGet(raw_len)]);
  b.emit(Instruction::I32Const(method_other_tag_id));
  b.emit(Instruction::LocalSet(var_tag_tmp));
  push_alloc_enum(&mut b, arena_alloc_idx, enum_tag_id, var_tag_tmp, Some(payload_f64_tmp), method_f64);
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(method_tag_ids[0]));
  b.emit(Instruction::LocalSet(var_tag_tmp));
  for (disc_idx, &tag_id) in method_tag_ids.iter().enumerate() {
    b.emit(Instruction::LocalGet(disc));
    b.emit(Instruction::I32Const(disc_idx as i32));
    b.emit(Instruction::I32Eq);
    b.emit(Instruction::If(BlockType::Empty));
    b.emit(Instruction::I32Const(tag_id));
    b.emit(Instruction::LocalSet(var_tag_tmp));
    b.emit(Instruction::End);
  }
  push_alloc_enum(&mut b, arena_alloc_idx, enum_tag_id, var_tag_tmp, None, method_f64);
  b.emit(Instruction::End);

  // 3. Lift incoming-request.path-with-query -> String (or "" on none).
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(req_path));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalSet(raw_ptr));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::I32Load(mem_arg_i32(8)));
  b.emit(Instruction::LocalSet(raw_len));
  b.emit(Instruction::LocalGet(raw_ptr));
  b.emit(Instruction::LocalGet(raw_len));
  b.emit(Instruction::Call(str_new_idx));
  b.emit(Instruction::LocalSet(path_f64));
  push_cabi_free_if_nonzero(&mut b, cabi_free_idx, raw_ptr, &[Instruction::LocalGet(raw_len)]);
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::Call(str_new_idx));
  b.emit(Instruction::LocalSet(path_f64));
  b.emit(Instruction::End);

  // 4. Lift incoming-request.headers -> List<HttpHeader>, freeing every entry buffer even on non-UTF-8 value.
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::Call(req_headers));
  b.emit(Instruction::LocalTee(in_fields));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(fields_entries));
  b.emit(Instruction::LocalGet(in_fields));
  b.emit(Instruction::Call(fields_drop));

  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(entries_ptr));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalSet(entries_len));

  // Allocate List<HttpHeader>: 8-byte header + 8-byte count f64 + entries_len * 8 slots.
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::LocalGet(entries_len));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::Call(arena_alloc_idx));
  b.emit(Instruction::LocalTee(list_raw));
  b.emit(Instruction::I32Const(HEAP_MAGIC));
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(list_raw));
  b.emit(Instruction::I32Const(list_tag_id));
  b.emit(Instruction::I32Store(mem_arg_i32(4)));
  b.emit(Instruction::LocalGet(list_raw));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalTee(headers_list_ptr));
  b.emit(Instruction::LocalGet(entries_len));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(headers_list_ptr));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::LocalSet(headers_f64));

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(bad_header));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(entry_idx));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(entry_idx));
  b.emit(Instruction::LocalGet(entries_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));

  b.emit(Instruction::LocalGet(entries_ptr));
  b.emit(Instruction::LocalGet(entry_idx));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalTee(entry_ptr));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(name_ptr));
  b.emit(Instruction::LocalGet(entry_ptr));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalSet(name_len));
  b.emit(Instruction::LocalGet(entry_ptr));
  b.emit(Instruction::I32Load(mem_arg_i32(8)));
  b.emit(Instruction::LocalSet(val_ptr));
  b.emit(Instruction::LocalGet(entry_ptr));
  b.emit(Instruction::I32Load(mem_arg_i32(12)));
  b.emit(Instruction::LocalSet(val_len));

  b.emit(Instruction::LocalGet(bad_header));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(val_ptr));
  b.emit(Instruction::LocalGet(val_len));
  b.emit(Instruction::Call(utf8_valid_idx));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(bad_header));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(name_ptr));
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::Call(str_new_idx));
  b.emit(Instruction::LocalSet(name_f64));
  b.emit(Instruction::LocalGet(val_ptr));
  b.emit(Instruction::LocalGet(val_len));
  b.emit(Instruction::Call(str_new_idx));
  b.emit(Instruction::LocalSet(val_f64));

  b.emit(Instruction::I32Const(8 + layout_info.header_layout.payload_bytes as i32));
  b.emit(Instruction::Call(arena_alloc_idx));
  b.emit(Instruction::LocalTee(hdr_struct_raw));
  b.emit(Instruction::I32Const(HEAP_MAGIC));
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(hdr_struct_raw));
  b.emit(Instruction::I32Const(struct_tag_id));
  b.emit(Instruction::I32Store(mem_arg_i32(4)));
  b.emit(Instruction::LocalGet(hdr_struct_raw));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalTee(hdr_struct_ptr));
  b.emit(Instruction::I32Const(layout_info.header_field_count as i32));
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(hdr_struct_ptr));
  b.emit(Instruction::I32Const(layout_info.header_layout_id as i32));
  b.emit(Instruction::I32Store(mem_arg_i32(4)));

  b.emit(Instruction::LocalGet(hdr_struct_ptr));
  b.emit(Instruction::LocalGet(name_f64));
  push_store_f64_into_compact_struct_field(&mut b.instructions, layout_info.header_layout.fields[layout_info.header_name_idx]);
  b.emit(Instruction::LocalGet(hdr_struct_ptr));
  b.emit(Instruction::LocalGet(val_f64));
  push_store_f64_into_compact_struct_field(&mut b.instructions, layout_info.header_layout.fields[layout_info.header_value_idx]);

  b.emit(Instruction::LocalGet(headers_list_ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(entry_idx));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(hdr_struct_ptr));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  push_cabi_free_if_nonzero(&mut b, cabi_free_idx, name_ptr, &[Instruction::LocalGet(name_len)]);
  push_cabi_free_if_nonzero(&mut b, cabi_free_idx, val_ptr, &[Instruction::LocalGet(val_len)]);

  b.emit(Instruction::LocalGet(entry_idx));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(entry_idx));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  push_cabi_free_if_nonzero(
    &mut b,
    cabi_free_idx,
    entries_ptr,
    &[Instruction::LocalGet(entries_len), Instruction::I32Const(16), Instruction::I32Mul],
  );

  b.emit(Instruction::LocalGet(bad_header));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::Call(req_drop));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(400));
  b.emit(Instruction::I32Const(bad_req_ptr));
  b.emit(Instruction::I32Const(bad_req_len));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(send_plain_idx));
  push_turn_cleanup_and_return(&mut b, active_turns_global, comp_globals, turn_exit_idx);
  b.emit(Instruction::End);

  // 5. Consume incoming-request body (always drop incoming-request after consume), bounded at MAX_HTTP_BODY_BYTES.
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(req_consume));
  b.emit(Instruction::LocalGet(0));
  b.emit(Instruction::Call(req_drop));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(400));
  b.emit(Instruction::I32Const(bad_req_ptr));
  b.emit(Instruction::I32Const(bad_req_len));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(send_plain_idx));
  push_turn_cleanup_and_return(&mut b, active_turns_global, comp_globals, turn_exit_idx);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalSet(in_body));

  b.emit(Instruction::LocalGet(in_body));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(in_body_stream));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(in_body));
  b.emit(Instruction::Call(in_body_drop));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(400));
  b.emit(Instruction::I32Const(bad_req_ptr));
  b.emit(Instruction::I32Const(bad_req_len));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(send_plain_idx));
  push_turn_cleanup_and_return(&mut b, active_turns_global, comp_globals, turn_exit_idx);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalSet(in_stream));

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(chunk_head));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(chunk_tail));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(staging_len));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(body_status));

  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(in_stream));
  b.emit(Instruction::I64Const(HTTP_READ_CHUNK_BYTES));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(in_stream_read));

  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  // Ok(chunk)
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalSet(raw_ptr));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::I32Load(mem_arg_i32(8)));
  b.emit(Instruction::LocalTee(raw_len));
  b.emit(Instruction::I32Const(MAX_HTTP_BODY_BYTES));
  b.emit(Instruction::LocalGet(staging_len));
  b.emit(Instruction::I32Sub);
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  push_cabi_free_if_nonzero(&mut b, cabi_free_idx, raw_ptr, &[Instruction::LocalGet(raw_len)]);
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::LocalSet(body_status));
  // br 3 breaks out of outer Block (depths: 0=If(exceed), 1=If(Ok), 2=Loop, 3=Block)
  b.emit(Instruction::Br(3));
  b.emit(Instruction::End);

  // Append incoming chunk pointer directly onto a linked list without doubling copies.
  b.emit(Instruction::LocalGet(raw_len));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::Call(arena_alloc_idx));
  b.emit(Instruction::LocalTee(chunk_node));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(chunk_node));
  b.emit(Instruction::LocalGet(raw_ptr));
  b.emit(Instruction::I32Store(mem_arg_i32(4)));
  b.emit(Instruction::LocalGet(chunk_node));
  b.emit(Instruction::LocalGet(raw_len));
  b.emit(Instruction::I32Store(mem_arg_i32(8)));
  b.emit(Instruction::LocalGet(chunk_head));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(chunk_node));
  b.emit(Instruction::LocalSet(chunk_head));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(chunk_tail));
  b.emit(Instruction::LocalGet(chunk_node));
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(chunk_node));
  b.emit(Instruction::LocalSet(chunk_tail));
  b.emit(Instruction::LocalGet(staging_len));
  b.emit(Instruction::LocalGet(raw_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(staging_len));
  b.emit(Instruction::End);
  b.emit(Instruction::Else);
  // Err(stream-error): 0 = last-operation-failed(error), 1 = closed (EOF)
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::I32Load8U(mem_arg_byte(4)));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::I32Load(mem_arg_i32(8)));
  b.emit(Instruction::Call(error_drop));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(body_status));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::I32Load8U(mem_arg_byte(4)));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::LocalSet(body_status));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  // Break out of read Block (depths: 0=If(Err), 1=Loop, 2=Block)
  b.emit(Instruction::Br(2));
  b.emit(Instruction::End);

  // Continue read Loop (depth: 0=Loop)
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Drop child input-stream before parent incoming-body.
  b.emit(Instruction::LocalGet(in_stream));
  b.emit(Instruction::Call(in_stream_drop));
  b.emit(Instruction::LocalGet(in_body));
  b.emit(Instruction::Call(in_body_drop));

  b.emit(Instruction::LocalGet(body_status));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(chunk_head));
  b.emit(Instruction::LocalSet(chunk_node));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(chunk_node));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(chunk_node));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(chunk_next));
  b.emit(Instruction::LocalGet(chunk_node));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalSet(raw_ptr));
  b.emit(Instruction::LocalGet(chunk_node));
  b.emit(Instruction::I32Load(mem_arg_i32(8)));
  b.emit(Instruction::LocalSet(raw_len));
  push_cabi_free_if_nonzero(&mut b, cabi_free_idx, raw_ptr, &[Instruction::LocalGet(raw_len)]);
  b.emit(Instruction::LocalGet(chunk_next));
  b.emit(Instruction::LocalSet(chunk_node));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(body_status));
  b.emit(Instruction::I32Const(2));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(413));
  b.emit(Instruction::I32Const(too_large_ptr));
  b.emit(Instruction::I32Const(too_large_len));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(send_plain_idx));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(400));
  b.emit(Instruction::I32Const(bad_req_ptr));
  b.emit(Instruction::I32Const(bad_req_len));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(send_plain_idx));
  b.emit(Instruction::End);
  push_turn_cleanup_and_return(&mut b, active_turns_global, comp_globals, turn_exit_idx);
  b.emit(Instruction::End);

  // Convert linked body chunks directly into one String or Buffer (:empty | :text String | :bytes Buffer).
  b.emit(Instruction::LocalGet(staging_len));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::I32Const(body_empty_tag_id));
  b.emit(Instruction::LocalSet(var_tag_tmp));
  push_alloc_enum(&mut b, arena_alloc_idx, enum_tag_id, var_tag_tmp, None, body_f64);
  b.emit(Instruction::Else);
  b.emit(Instruction::I32Const(16));
  b.emit(Instruction::LocalGet(staging_len));
  b.emit(Instruction::I32Const(7));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::I32Const(-8i32));
  b.emit(Instruction::I32And);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::Call(arena_alloc_idx));
  b.emit(Instruction::LocalTee(body_obj_raw));
  b.emit(Instruction::I32Const(HEAP_MAGIC));
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(body_obj_raw));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalTee(body_obj_ptr));
  b.emit(Instruction::LocalGet(staging_len));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::F64Store(mem_arg_f64(0)));
  b.emit(Instruction::LocalGet(body_obj_ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(write_off));

  b.emit(Instruction::LocalGet(chunk_head));
  b.emit(Instruction::LocalSet(chunk_node));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(chunk_node));
  b.emit(Instruction::I32Eqz);
  b.emit(Instruction::BrIf(1));
  b.emit(Instruction::LocalGet(chunk_node));
  b.emit(Instruction::I32Load(mem_arg_i32(0)));
  b.emit(Instruction::LocalSet(chunk_next));
  b.emit(Instruction::LocalGet(chunk_node));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalSet(raw_ptr));
  b.emit(Instruction::LocalGet(chunk_node));
  b.emit(Instruction::I32Load(mem_arg_i32(8)));
  b.emit(Instruction::LocalSet(raw_len));
  b.emit(Instruction::LocalGet(write_off));
  b.emit(Instruction::LocalGet(raw_ptr));
  b.emit(Instruction::LocalGet(raw_len));
  b.emit(Instruction::MemoryCopy { dst_mem: 0, src_mem: 0 });
  b.emit(Instruction::LocalGet(write_off));
  b.emit(Instruction::LocalGet(raw_len));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(write_off));
  push_cabi_free_if_nonzero(&mut b, cabi_free_idx, raw_ptr, &[Instruction::LocalGet(raw_len)]);
  b.emit(Instruction::LocalGet(chunk_next));
  b.emit(Instruction::LocalSet(chunk_node));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(body_obj_ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(staging_len));
  b.emit(Instruction::Call(utf8_valid_idx));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(body_obj_raw));
  b.emit(Instruction::I32Const(str_tag_id));
  b.emit(Instruction::I32Store(mem_arg_i32(4)));
  b.emit(Instruction::I32Const(body_text_tag_id));
  b.emit(Instruction::LocalSet(var_tag_tmp));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(body_obj_raw));
  b.emit(Instruction::I32Const(buffer_tag_id));
  b.emit(Instruction::I32Store(mem_arg_i32(4)));
  b.emit(Instruction::I32Const(body_bytes_tag_id));
  b.emit(Instruction::LocalSet(var_tag_tmp));
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(body_obj_ptr));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::LocalSet(payload_f64_tmp));
  push_alloc_enum(&mut b, arena_alloc_idx, enum_tag_id, var_tag_tmp, Some(payload_f64_tmp), body_f64);
  b.emit(Instruction::End);

  // 6. Construct calcit.core/HttpRequest compact struct.
  b.emit(Instruction::I32Const(8 + layout_info.request_layout.payload_bytes as i32));
  b.emit(Instruction::Call(arena_alloc_idx));
  b.emit(Instruction::LocalTee(req_struct_raw));
  b.emit(Instruction::I32Const(HEAP_MAGIC));
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(req_struct_raw));
  b.emit(Instruction::I32Const(struct_tag_id));
  b.emit(Instruction::I32Store(mem_arg_i32(4)));
  b.emit(Instruction::LocalGet(req_struct_raw));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalTee(req_struct_ptr));
  b.emit(Instruction::I32Const(layout_info.request_field_count as i32));
  b.emit(Instruction::I32Store(mem_arg_i32(0)));
  b.emit(Instruction::LocalGet(req_struct_ptr));
  b.emit(Instruction::I32Const(layout_info.request_layout_id as i32));
  b.emit(Instruction::I32Store(mem_arg_i32(4)));

  b.emit(Instruction::LocalGet(req_struct_ptr));
  b.emit(Instruction::LocalGet(method_f64));
  push_store_f64_into_compact_struct_field(&mut b.instructions, layout_info.request_layout.fields[layout_info.req_method_idx]);
  b.emit(Instruction::LocalGet(req_struct_ptr));
  b.emit(Instruction::LocalGet(path_f64));
  push_store_f64_into_compact_struct_field(&mut b.instructions, layout_info.request_layout.fields[layout_info.req_path_idx]);
  b.emit(Instruction::LocalGet(req_struct_ptr));
  b.emit(Instruction::LocalGet(headers_f64));
  push_store_f64_into_compact_struct_field(&mut b.instructions, layout_info.request_layout.fields[layout_info.req_headers_idx]);
  b.emit(Instruction::LocalGet(req_struct_ptr));
  b.emit(Instruction::LocalGet(body_f64));
  push_store_f64_into_compact_struct_field(&mut b.instructions, layout_info.request_layout.fields[layout_info.req_body_idx]);

  b.emit(Instruction::LocalGet(req_struct_ptr));
  b.emit(Instruction::F64ConvertI32U);
  b.emit(Instruction::LocalSet(req_f64));

  // 7. Invoke the Calcit HTTP handler and inspect Result<HttpResponse, String>.
  b.emit(Instruction::LocalGet(req_f64));
  b.emit(Instruction::Call(handler_idx));
  b.emit(Instruction::LocalTee(result_f64));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalTee(result_ptr));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalSet(result_tag));

  b.emit(Instruction::LocalGet(result_tag));
  b.emit(Instruction::I32Const(err_tag_id));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(result_ptr));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalTee(err_str_ptr));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(err_msg_len));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(500));
  b.emit(Instruction::LocalGet(err_str_ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(err_msg_len));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(send_plain_idx));
  push_turn_cleanup_and_return(&mut b, active_turns_global, comp_globals, turn_exit_idx);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(result_tag));
  b.emit(Instruction::I32Const(ok_tag_id));
  b.emit(Instruction::I32Ne);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(500));
  b.emit(Instruction::I32Const(internal_err_ptr));
  b.emit(Instruction::I32Const(internal_err_len));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(send_plain_idx));
  push_turn_cleanup_and_return(&mut b, active_turns_global, comp_globals, turn_exit_idx);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(result_ptr));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(resp_ptr));

  push_load_compact_struct_field_f64(
    &mut b.instructions,
    resp_ptr,
    layout_info.response_layout.fields[layout_info.resp_status_idx],
  );
  b.emit(Instruction::LocalSet(status_f64));
  push_load_compact_struct_field_f64(
    &mut b.instructions,
    resp_ptr,
    layout_info.response_layout.fields[layout_info.resp_headers_idx],
  );
  b.emit(Instruction::LocalSet(resp_headers_f64));
  push_load_compact_struct_field_f64(
    &mut b.instructions,
    resp_ptr,
    layout_info.response_layout.fields[layout_info.resp_body_idx],
  );
  b.emit(Instruction::LocalSet(resp_body_f64));

  // Validate HTTP status code is an integer in 100..=599.
  b.emit(Instruction::LocalGet(status_f64));
  b.emit(Instruction::LocalGet(status_f64));
  b.emit(Instruction::F64Trunc);
  b.emit(Instruction::F64Ne);
  b.emit(Instruction::LocalGet(status_f64));
  b.emit(f64_const(100.0));
  b.emit(Instruction::F64Lt);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::LocalGet(status_f64));
  b.emit(f64_const(599.0));
  b.emit(Instruction::F64Gt);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(500));
  b.emit(Instruction::I32Const(internal_err_ptr));
  b.emit(Instruction::I32Const(internal_err_len));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(send_plain_idx));
  push_turn_cleanup_and_return(&mut b, active_turns_global, comp_globals, turn_exit_idx);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(status_f64));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(status_i32));

  // Build outgoing headers from List<HttpHeader>.
  b.emit(Instruction::Call(fields_new));
  b.emit(Instruction::LocalSet(out_fields));
  b.emit(Instruction::LocalGet(resp_headers_f64));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalTee(resp_headers_ptr));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(entries_len));

  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(entry_idx));
  b.emit(Instruction::Block(BlockType::Empty));
  b.emit(Instruction::Loop(BlockType::Empty));
  b.emit(Instruction::LocalGet(entry_idx));
  b.emit(Instruction::LocalGet(entries_len));
  b.emit(Instruction::I32GeU);
  b.emit(Instruction::BrIf(1));

  b.emit(Instruction::LocalGet(resp_headers_ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(entry_idx));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Mul);
  b.emit(Instruction::I32Add);
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(hdr_struct_ptr));

  push_load_compact_struct_field_f64(
    &mut b.instructions,
    hdr_struct_ptr,
    layout_info.header_layout.fields[layout_info.header_name_idx],
  );
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalTee(name_ptr));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(name_len));

  push_load_compact_struct_field_f64(
    &mut b.instructions,
    hdr_struct_ptr,
    layout_info.header_layout.fields[layout_info.header_value_idx],
  );
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalTee(val_ptr));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(val_len));

  b.emit(Instruction::LocalGet(out_fields));
  b.emit(Instruction::LocalGet(name_ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(name_len));
  b.emit(Instruction::LocalGet(val_ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalGet(val_len));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(fields_append));

  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(out_fields));
  b.emit(Instruction::Call(fields_drop));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(500));
  b.emit(Instruction::I32Const(internal_err_ptr));
  b.emit(Instruction::I32Const(internal_err_len));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(send_plain_idx));
  push_turn_cleanup_and_return(&mut b, active_turns_global, comp_globals, turn_exit_idx);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(entry_idx));
  b.emit(Instruction::I32Const(1));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(entry_idx));
  b.emit(Instruction::Br(0));
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Extract outgoing body bytes before transferring response-outparam.
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(out_bytes_ptr));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalSet(out_bytes_len));

  b.emit(Instruction::LocalGet(resp_body_f64));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalTee(resp_body_ptr));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalTee(resp_body_tag));
  b.emit(Instruction::I32Const(body_empty_tag_id));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(resp_body_tag));
  b.emit(Instruction::I32Const(body_text_tag_id));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::LocalGet(resp_body_tag));
  b.emit(Instruction::I32Const(body_bytes_tag_id));
  b.emit(Instruction::I32Eq);
  b.emit(Instruction::I32Or);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(resp_body_ptr));
  b.emit(Instruction::F64Load(mem_arg_f64(8)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalTee(payload_obj_ptr));
  b.emit(Instruction::I32Const(8));
  b.emit(Instruction::I32Add);
  b.emit(Instruction::LocalSet(out_bytes_ptr));
  b.emit(Instruction::LocalGet(payload_obj_ptr));
  b.emit(Instruction::F64Load(mem_arg_f64(0)));
  b.emit(Instruction::I32TruncF64U);
  b.emit(Instruction::LocalSet(out_bytes_len));
  b.emit(Instruction::Else);
  b.emit(Instruction::LocalGet(out_fields));
  b.emit(Instruction::Call(fields_drop));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(500));
  b.emit(Instruction::I32Const(internal_err_ptr));
  b.emit(Instruction::I32Const(internal_err_len));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(send_plain_idx));
  push_turn_cleanup_and_return(&mut b, active_turns_global, comp_globals, turn_exit_idx);
  b.emit(Instruction::End);
  b.emit(Instruction::End);

  // Construct outgoing-response (consumes out_fields).
  b.emit(Instruction::LocalGet(out_fields));
  b.emit(Instruction::Call(resp_new));
  b.emit(Instruction::LocalTee(out_resp));
  b.emit(Instruction::LocalGet(status_i32));
  b.emit(Instruction::Call(resp_set_status));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(out_resp));
  b.emit(Instruction::Call(resp_drop));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(500));
  b.emit(Instruction::I32Const(internal_err_ptr));
  b.emit(Instruction::I32Const(internal_err_len));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(send_plain_idx));
  push_turn_cleanup_and_return(&mut b, active_turns_global, comp_globals, turn_exit_idx);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(out_resp));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(resp_body));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(out_resp));
  b.emit(Instruction::Call(resp_drop));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(500));
  b.emit(Instruction::I32Const(internal_err_ptr));
  b.emit(Instruction::I32Const(internal_err_len));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(send_plain_idx));
  push_turn_cleanup_and_return(&mut b, active_turns_global, comp_globals, turn_exit_idx);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalSet(out_body));

  b.emit(Instruction::LocalGet(out_body));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(out_body_write));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::I32Load8U(mem_arg_byte(0)));
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(out_body));
  b.emit(Instruction::Call(out_body_drop));
  b.emit(Instruction::LocalGet(out_resp));
  b.emit(Instruction::Call(resp_drop));
  b.emit(Instruction::LocalGet(1));
  b.emit(Instruction::I32Const(500));
  b.emit(Instruction::I32Const(internal_err_ptr));
  b.emit(Instruction::I32Const(internal_err_len));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(send_plain_idx));
  push_turn_cleanup_and_return(&mut b, active_turns_global, comp_globals, turn_exit_idx);
  b.emit(Instruction::End);
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::I32Load(mem_arg_i32(4)));
  b.emit(Instruction::LocalSet(out_stream));

  push_response_outparam_ok(&mut b, outparam_set, 1, out_resp);

  b.emit(Instruction::LocalGet(out_bytes_len));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32GtU);
  b.emit(Instruction::If(BlockType::Empty));
  b.emit(Instruction::LocalGet(out_stream));
  b.emit(Instruction::LocalGet(out_bytes_ptr));
  b.emit(Instruction::LocalGet(out_bytes_len));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(write_all_idx));
  b.emit(Instruction::Drop);
  b.emit(Instruction::End);

  b.emit(Instruction::LocalGet(out_stream));
  b.emit(Instruction::Call(out_stream_drop));
  b.emit(Instruction::LocalGet(out_body));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::I32Const(0));
  b.emit(Instruction::LocalGet(retptr));
  b.emit(Instruction::Call(out_body_finish));

  push_turn_cleanup_and_return(&mut b, active_turns_global, comp_globals, turn_exit_idx);

  b.finish_exported(
    Some("wasi:http/incoming-handler@0.2.12#handle".into()),
    vec![ValType::I32, ValType::I32],
    vec![],
  )
}
