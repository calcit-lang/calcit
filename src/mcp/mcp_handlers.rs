use super::AppState;
use super::jsonrpc::*;
use super::tools::{McpRequest, get_standard_mcp_tools};
use axum::response::Json as ResponseJson;
use serde_json::Value;
use std::sync::{Arc, Mutex, OnceLock};

// Global session storage (in a real implementation, this would be per-connection)
static GLOBAL_SESSION: OnceLock<Arc<Mutex<McpSession>>> = OnceLock::new();

fn get_session() -> Arc<Mutex<McpSession>> {
  GLOBAL_SESSION.get_or_init(|| Arc::new(Mutex::new(McpSession::new()))).clone()
}

/// Handle JSON-RPC 2.0 requests (Axum version)
pub async fn handle_jsonrpc_axum(data: Arc<AppState>, req: JsonRpcRequest) -> ResponseJson<Value> {
  // 记录请求输入
  let req_json = match serde_json::to_string_pretty(&req) {
    Ok(json) => json,
    Err(_) => "Failed to serialize request".to_string(),
  };
  println!("[JSON-RPC REQUEST] method={}, id={:?}", req.method, req.id);
  println!("[JSON-RPC INPUT] {req_json}");

  let response = match req.method.as_str() {
    "initialize" => handle_initialize_axum(&data, &req),
    "tools/list" => handle_tools_list_axum(&data, &req),
    "tools/call" => handle_tools_call_axum(&data, &req).await,
    _ => {
      println!("[JSON-RPC ERROR] Unknown method: {}", req.method);
      let error = JsonRpcError::method_not_found();
      serde_json::to_value(JsonRpcResponse::error(req.id.clone(), error)).unwrap()
    }
  };

  // 记录响应输出
  let response_json = match serde_json::to_string_pretty(&response) {
    Ok(json) => json,
    Err(_) => "Failed to serialize response".to_string(),
  };
  println!("[JSON-RPC OUTPUT] {response_json}");

  ResponseJson(response)
}

/// Handle initialize request (Axum version)
fn handle_initialize_axum(_app_state: &AppState, req: &JsonRpcRequest) -> Value {
  let session = get_session();
  let mut session_guard = match session.lock() {
    Ok(guard) => guard,
    Err(_) => {
      let error = JsonRpcError::internal_error();
      return serde_json::to_value(JsonRpcResponse::error(req.id.clone(), error)).unwrap();
    }
  };

  // Allow re-initialization - just log if already initialized
  if session_guard.is_initialized() {
    println!("Session already initialized, re-initializing...");
  }

  let params: InitializeParams = match req.params.as_ref() {
    Some(params) => match serde_json::from_value(params.clone()) {
      Ok(p) => p,
      Err(_) => {
        let error = JsonRpcError::invalid_params();
        return serde_json::to_value(JsonRpcResponse::error(req.id.clone(), error)).unwrap();
      }
    },
    None => {
      let error = JsonRpcError::invalid_params();
      return serde_json::to_value(JsonRpcResponse::error(req.id.clone(), error)).unwrap();
    }
  };

  // Log client capabilities for debugging
  println!("Client capabilities: {:?}", params.capabilities);
  println!("Client info: {:?}", params.client_info);
  println!("Protocol version: {}", params.protocol_version);

  // Check protocol version compatibility
  let supported_versions = ["2024-11-05", "2025-06-18"];
  if !supported_versions.contains(&params.protocol_version.as_str()) {
    println!(
      "Warning: Client protocol version {} may not be fully compatible",
      params.protocol_version
    );
  }

  // Declare server capabilities based on what we support
  let server_capabilities = ServerCapabilities {
    tools: Some(ToolsCapability { list_changed: Some(false) }),
    resources: None,
    prompts: None,
  };

  // Initialize the session
  session_guard.initialize(params.client_info.clone(), params.capabilities.clone(), server_capabilities.clone());

  let result = InitializeResult {
    protocol_version: params.protocol_version.clone(),
    capabilities: server_capabilities,
    server_info: ServerInfo {
      name: "calcit-mcp-server".to_string(),
      version: "0.9.18".to_string(),
    },
  };

  println!("Initialization successful, session state: {:?}", session_guard.state);
  serde_json::to_value(JsonRpcResponse::success(req.id.clone(), serde_json::to_value(result).unwrap())).unwrap()
}

/// Handle tools/list request (Axum version)
fn handle_tools_list_axum(_app_state: &AppState, req: &JsonRpcRequest) -> Value {
  // Check session state
  let session = get_session();
  let session_guard = match session.lock() {
    Ok(guard) => guard,
    Err(_) => {
      let error = JsonRpcError::internal_error();
      return serde_json::to_value(JsonRpcResponse::error(req.id.clone(), error)).unwrap();
    }
  };

  if !session_guard.is_initialized() {
    let error = JsonRpcError::new(-32002, "Session not initialized".to_string());
    return serde_json::to_value(JsonRpcResponse::error(req.id.clone(), error)).unwrap();
  }
  drop(session_guard); // Release the lock

  let tools = get_standard_mcp_tools();

  let result = ToolsListResult { tools, next_cursor: None };

  serde_json::to_value(JsonRpcResponse::success(req.id.clone(), serde_json::to_value(result).unwrap())).unwrap()
}

/// Handle tools/call request (Axum version)
async fn handle_tools_call_axum(app_state: &AppState, req: &JsonRpcRequest) -> Value {
  // Check session state
  let session = get_session();
  let session_guard = match session.lock() {
    Ok(guard) => guard,
    Err(_) => {
      let error = JsonRpcError::internal_error();
      return serde_json::to_value(JsonRpcResponse::error(req.id.clone(), error)).unwrap();
    }
  };

  if !session_guard.is_initialized() {
    let error = JsonRpcError::new(-32002, "Session not initialized".to_string());
    return serde_json::to_value(JsonRpcResponse::error(req.id.clone(), error)).unwrap();
  }
  drop(session_guard); // Release the lock

  let params: ToolsCallParams = match req.params.as_ref() {
    Some(params) => match serde_json::from_value(params.clone()) {
      Ok(p) => p,
      Err(_) => {
        let error = JsonRpcError::invalid_params();
        return serde_json::to_value(JsonRpcResponse::error(req.id.clone(), error)).unwrap();
      }
    },
    None => {
      let error = JsonRpcError::invalid_params();
      return serde_json::to_value(JsonRpcResponse::error(req.id.clone(), error)).unwrap();
    }
  };

  // Convert arguments to legacy format
  let legacy_params = match params.arguments {
    Some(args) => match args.as_object() {
      Some(obj) => obj.clone().into_iter().collect(),
      None => std::collections::HashMap::new(),
    },
    None => std::collections::HashMap::new(),
  };

  let legacy_request = McpRequest {
    tool_name: params.name.clone(),
    parameters: legacy_params,
  };

  // Call the appropriate handler based on tool name
  let handler_result = match params.name.as_str() {
    // 读取操作
    "list_definitions" => super::read_handlers::list_definitions(app_state, legacy_request),
    "list_namespaces" => super::read_handlers::list_namespaces(app_state, legacy_request),
    "get_package_name" => super::read_handlers::get_package_name(app_state, legacy_request),
    "read_namespace" => super::read_handlers::read_namespace(app_state, legacy_request),
    "read_definition" => super::read_handlers::read_definition(app_state, legacy_request),

    // 命名空间操作
    "add_namespace" => super::namespace_handlers::add_namespace(app_state, legacy_request),
    "delete_namespace" => super::namespace_handlers::delete_namespace(app_state, legacy_request),
    "update_namespace_imports" => super::namespace_handlers::update_namespace_imports(app_state, legacy_request),

    // 定义操作
    "add_definition" => super::definition_handlers::add_definition(app_state, legacy_request),
    "delete_definition" => super::definition_handlers::delete_definition(app_state, legacy_request),
    "update_definition" => super::definition_handlers::update_definition(app_state, legacy_request),

    // 模块管理
    "list_modules" => super::module_handlers::list_modules(app_state, legacy_request),
    "get_current_module" => super::module_handlers::get_current_module(app_state, legacy_request),
    "switch_module" => super::module_handlers::switch_module(app_state, legacy_request),
    "create_module" => super::module_handlers::create_module(app_state, legacy_request),
    "delete_module" => super::module_handlers::delete_module(app_state, legacy_request),

    // Cirru 转换工具
    "parse_to_json" => super::cirru_handlers::parse_to_json(app_state, legacy_request),
    "format_from_json" => super::cirru_handlers::format_from_json(app_state, legacy_request),

    // 文档查询工具
    "query_api_docs" => super::docs_handlers::handle_query_api_docs(app_state, legacy_request),
    "query_guidebook" => super::docs_handlers::handle_query_guidebook(app_state, legacy_request),
    "list_api_docs" => super::docs_handlers::handle_list_api_docs(app_state, legacy_request),
    "list_guidebook_docs" => super::docs_handlers::handle_list_guidebook_docs(app_state, legacy_request),

    _ => {
      let error = JsonRpcError::tool_not_found(&params.name);
      return serde_json::to_value(JsonRpcResponse::error(req.id.clone(), error)).unwrap();
    }
  };

  // Wrap the result in proper MCP ToolsCallResult format
  let tool_call_result = ToolsCallResult {
    content: vec![ToolCallContent::Text {
      text: serde_json::to_string(&handler_result.0).unwrap_or_else(|_| "null".to_string()),
    }],
    is_error: None,
  };

  serde_json::to_value(JsonRpcResponse::success(
    req.id.clone(),
    serde_json::to_value(tool_call_result).unwrap(),
  ))
  .unwrap()
}

/// Legacy endpoint for backward compatibility (Axum version)
pub async fn legacy_discover_axum() -> ResponseJson<Value> {
  println!("[LEGACY REQUEST] /mcp/discover");
  let tools = super::tools::get_mcp_tools();
  let response = serde_json::json!({
      "tools": tools
  });

  // 记录响应输出
  let response_json = match serde_json::to_string_pretty(&response) {
    Ok(json) => json,
    Err(_) => "Failed to serialize response".to_string(),
  };
  println!("[LEGACY OUTPUT] {response_json}");

  ResponseJson(response)
}

/// Legacy endpoint for backward compatibility (Axum version)
pub async fn legacy_execute_axum(data: Arc<AppState>, req: McpRequest) -> ResponseJson<Value> {
  // 记录请求输入
  let req_json = match serde_json::to_string_pretty(&req) {
    Ok(json) => json,
    Err(_) => "Failed to serialize request".to_string(),
  };
  println!("[LEGACY REQUEST] /mcp/execute with tool: {}", req.tool_name);
  println!("[LEGACY INPUT] {req_json}");

  let response = match req.tool_name.as_str() {
    // 读取操作
    "list_definitions" => super::read_handlers::list_definitions(&data, req),
    "list_namespaces" => super::read_handlers::list_namespaces(&data, req),
    "get_package_name" => super::read_handlers::get_package_name(&data, req),
    "read_namespace" => super::read_handlers::read_namespace(&data, req),
    "read_definition" => super::read_handlers::read_definition(&data, req),

    // 命名空间操作
    "add_namespace" => super::namespace_handlers::add_namespace(&data, req),
    "delete_namespace" => super::namespace_handlers::delete_namespace(&data, req),
    "update_namespace_imports" => super::namespace_handlers::update_namespace_imports(&data, req),

    // 定义操作
    "add_definition" => super::definition_handlers::add_definition(&data, req),
    "delete_definition" => super::definition_handlers::delete_definition(&data, req),
    "update_definition" => super::definition_handlers::update_definition(&data, req),

    // 模块管理
    "list_modules" => super::module_handlers::list_modules(&data, req),
    "get_current_module" => super::module_handlers::get_current_module(&data, req),
    "switch_module" => super::module_handlers::switch_module(&data, req),
    "create_module" => super::module_handlers::create_module(&data, req),
    "delete_module" => super::module_handlers::delete_module(&data, req),

    // Cirru 转换工具
    "parse_to_json" => super::cirru_handlers::parse_to_json(&data, req),
    "format_from_json" => super::cirru_handlers::format_from_json(&data, req),

    // 文档查询工具
    "query_api_docs" => super::docs_handlers::handle_query_api_docs(&data, req),
    "query_guidebook" => super::docs_handlers::handle_query_guidebook(&data, req),
    "list_api_docs" => super::docs_handlers::handle_list_api_docs(&data, req),
    "list_guidebook_docs" => super::docs_handlers::handle_list_guidebook_docs(&data, req),

    _ => {
      println!("[LEGACY ERROR] Unknown tool: {}", req.tool_name);
      ResponseJson(serde_json::json!({
        "error": format!("Unknown tool: {}", req.tool_name)
      }))
    }
  };

  // 记录响应输出
  let response_json = match serde_json::to_string_pretty(&response.0) {
    Ok(json) => json,
    Err(_) => "Failed to serialize response".to_string(),
  };
  println!("[LEGACY OUTPUT] {response_json}");

  response
}
