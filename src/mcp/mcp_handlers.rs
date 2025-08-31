use super::AppState;
use super::jsonrpc::*;
use super::tools::{McpRequest, get_standard_mcp_tools};
use super::*;
use actix_web::{HttpResponse, web};
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};

// Global session storage (in a real implementation, this would be per-connection)
static GLOBAL_SESSION: OnceLock<Arc<Mutex<McpSession>>> = OnceLock::new();

fn get_session() -> Arc<Mutex<McpSession>> {
  GLOBAL_SESSION.get_or_init(|| Arc::new(Mutex::new(McpSession::new()))).clone()
}

/// Handle JSON-RPC 2.0 requests
pub async fn handle_jsonrpc(data: web::Data<AppState>, req: web::Json<JsonRpcRequest>) -> HttpResponse {
  println!("Handling JSON-RPC request: method={}, id={:?}", req.method, req.id);

  match req.method.as_str() {
    "initialize" => handle_initialize(&data, &req),
    "tools/list" => handle_tools_list(&data, &req),
    "tools/call" => handle_tools_call(&data, &req).await,
    _ => {
      let error = JsonRpcError::method_not_found();
      HttpResponse::Ok().json(JsonRpcResponse::error(req.id.clone(), error))
    }
  }
}

/// Handle initialize request
fn handle_initialize(_app_state: &AppState, req: &JsonRpcRequest) -> HttpResponse {
  let session = get_session();
  let mut session_guard = match session.lock() {
    Ok(guard) => guard,
    Err(_) => {
      let error = JsonRpcError::internal_error();
      return HttpResponse::Ok().json(JsonRpcResponse::error(req.id.clone(), error));
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
        return HttpResponse::Ok().json(JsonRpcResponse::error(req.id.clone(), error));
      }
    },
    None => {
      let error = JsonRpcError::invalid_params();
      return HttpResponse::Ok().json(JsonRpcResponse::error(req.id.clone(), error));
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
  HttpResponse::Ok().json(JsonRpcResponse::success(req.id.clone(), serde_json::to_value(result).unwrap()))
}

/// Handle tools/list request
fn handle_tools_list(_app_state: &AppState, req: &JsonRpcRequest) -> HttpResponse {
  // Check session state
  let session = get_session();
  let session_guard = match session.lock() {
    Ok(guard) => guard,
    Err(_) => {
      let error = JsonRpcError::internal_error();
      return HttpResponse::Ok().json(JsonRpcResponse::error(req.id.clone(), error));
    }
  };

  if !session_guard.is_initialized() {
    let error = JsonRpcError::new(-32002, "Session not initialized".to_string());
    return HttpResponse::Ok().json(JsonRpcResponse::error(req.id.clone(), error));
  }
  drop(session_guard); // Release the lock

  let tools = get_standard_mcp_tools();

  let result = ToolsListResult { tools, next_cursor: None };

  HttpResponse::Ok().json(JsonRpcResponse::success(req.id.clone(), serde_json::to_value(result).unwrap()))
}

/// Handle tools/call request
async fn handle_tools_call(app_state: &AppState, req: &JsonRpcRequest) -> HttpResponse {
  // Check session state
  let session = get_session();
  let session_guard = match session.lock() {
    Ok(guard) => guard,
    Err(_) => {
      let error = JsonRpcError::internal_error();
      return HttpResponse::Ok().json(JsonRpcResponse::error(req.id.clone(), error));
    }
  };

  if !session_guard.is_initialized() {
    let error = JsonRpcError::new(-32002, "Session not initialized".to_string());
    return HttpResponse::Ok().json(JsonRpcResponse::error(req.id.clone(), error));
  }
  drop(session_guard); // Release the lock

  let params: ToolsCallParams = match req.params.as_ref() {
    Some(params) => match serde_json::from_value(params.clone()) {
      Ok(p) => p,
      Err(_) => {
        let error = JsonRpcError::invalid_params();
        return HttpResponse::Ok().json(JsonRpcResponse::error(req.id.clone(), error));
      }
    },
    None => {
      let error = JsonRpcError::invalid_params();
      return HttpResponse::Ok().json(JsonRpcResponse::error(req.id.clone(), error));
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
  let response = match params.name.as_str() {
    // 读取操作
    "list_definitions" => list_definitions(app_state, legacy_request),
    "list_namespaces" => list_namespaces(app_state, legacy_request),
    "read_namespace" => read_namespace(app_state, legacy_request),
    "read_definition" => read_definition(app_state, legacy_request),

    // 命名空间操作
    "add_namespace" => add_namespace(app_state, legacy_request),
    "delete_namespace" => delete_namespace(app_state, legacy_request),
    "update_namespace_imports" => update_namespace_imports(app_state, legacy_request),

    // 定义操作
    "add_definition" => add_definition(app_state, legacy_request),
    "delete_definition" => delete_definition(app_state, legacy_request),
    "update_definition" => update_definition(app_state, legacy_request),

    // 模块管理
    "list_modules" => list_modules(app_state, legacy_request),
    "read_module" => read_module(app_state, legacy_request),
    "add_module_dependency" => add_module_dependency(app_state, legacy_request),
    "remove_module_dependency" => remove_module_dependency(app_state, legacy_request),
    "clear_module_cache" => clear_module_cache(app_state, legacy_request),

    // Cirru 转换工具
    "parse_to_json" => parse_to_json(app_state, legacy_request),
    "format_from_json" => format_from_json(app_state, legacy_request),

    _ => {
      let error = JsonRpcError::tool_not_found(&params.name);
      return HttpResponse::Ok().json(JsonRpcResponse::error(req.id.clone(), error));
    }
  };

  // Convert legacy response to MCP format
  match response.status().as_u16() {
    200 => {
      // For now, we'll create a simple text response
      // In a real implementation, you'd need to properly extract the body
      let body = "Tool executed successfully".to_string();

      let result = ToolsCallResult {
        content: vec![ToolCallContent::Text { text: body }],
        is_error: Some(false),
      };

      HttpResponse::Ok().json(JsonRpcResponse::success(req.id.clone(), serde_json::to_value(result).unwrap()))
    }
    _ => {
      let error = JsonRpcError::tool_execution_error("Tool execution failed".to_string());
      HttpResponse::Ok().json(JsonRpcResponse::error(req.id.clone(), error))
    }
  }
}

/// Legacy endpoint for backward compatibility
pub async fn legacy_discover() -> HttpResponse {
  println!("handling legacy /mcp/discover");
  let tools = super::tools::get_mcp_tools();
  HttpResponse::Ok().json(serde_json::json!({
      "tools": tools
  }))
}

/// Legacy endpoint for backward compatibility
pub async fn legacy_execute(data: web::Data<AppState>, req: web::Json<McpRequest>) -> HttpResponse {
  println!("handling legacy /mcp/execute with tool: {}", req.tool_name);
  match req.tool_name.as_str() {
    // 读取操作
    "list_definitions" => list_definitions(&data, req.into_inner()),
    "list_namespaces" => list_namespaces(&data, req.into_inner()),
    "read_namespace" => read_namespace(&data, req.into_inner()),
    "read_definition" => read_definition(&data, req.into_inner()),

    // 命名空间操作
    "add_namespace" => add_namespace(&data, req.into_inner()),
    "delete_namespace" => delete_namespace(&data, req.into_inner()),
    "update_namespace_imports" => update_namespace_imports(&data, req.into_inner()),

    // 定义操作
    "add_definition" => add_definition(&data, req.into_inner()),
    "delete_definition" => delete_definition(&data, req.into_inner()),
    "update_definition" => update_definition(&data, req.into_inner()),

    // 模块管理
    "list_modules" => list_modules(&data, req.into_inner()),
    "read_module" => read_module(&data, req.into_inner()),
    "add_module_dependency" => add_module_dependency(&data, req.into_inner()),
    "remove_module_dependency" => remove_module_dependency(&data, req.into_inner()),
    "clear_module_cache" => clear_module_cache(&data, req.into_inner()),

    // Cirru 转换工具
    "parse_to_json" => parse_to_json(&data, req.into_inner()),
    "format_from_json" => format_from_json(&data, req.into_inner()),

    _ => HttpResponse::BadRequest().body(format!("Unknown tool: {}", req.tool_name)),
  }
}
