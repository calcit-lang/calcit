use super::AppState;
use super::document_handlers::{
  DeleteModuleDocRequest, ReadDefinitionDocRequest, ReadModuleDocRequest, RenameModuleDocRequest, UpdateDefinitionDocRequest,
  UpdateModuleDocRequest,
};
use super::jsonrpc::*;
use super::tools::{
  AddDefinitionRequest, AddNamespaceRequest, CreateModuleRequest, DeleteDefinitionRequest, DeleteModuleRequest, DeleteNamespaceRequest,
  FormatJsonToCirruRequest, GetCurrentModuleRequest, GetPackageNameRequest, ListApiDocsRequest, ListDefinitionsRequest,
  ListDependencyDocsRequest, ListGuidebookDocsRequest, ListModuleDocsRequest, ListModulesRequest, ListNamespacesRequest, McpRequest,
  OverwriteDefinitionRequest, ParseCirruToJsonRequest, QueryApiDocsRequest, QueryGuidebookRequest, ReadConfigsRequest,
  ReadDefinitionAtRequest, ReadDefinitionRequest, ReadDependencyDefinitionDocRequest, ReadDependencyModuleDocRequest,
  ReadNamespaceRequest, SwitchModuleRequest, UpdateConfigsRequest, UpdateDefinitionAtRequest, UpdateNamespaceImportsRequest,
  get_standard_mcp_tools,
};
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
  // Log request input
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

  // Log response output
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

// Helper function to deserialize parameters and handle errors
fn deserialize_params<T: serde::de::DeserializeOwned>(
  parameters: serde_json::Value,
  req_id: Option<serde_json::Value>,
) -> Result<T, Value> {
  match serde_json::from_value(parameters) {
    Ok(request) => Ok(request),
    Err(e) => {
      let error_message = format!("Invalid parameters: {e}");
      let error = JsonRpcError::new(-32602, error_message);
      Err(serde_json::to_value(JsonRpcResponse::error(req_id, error)).unwrap())
    }
  }
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
      Err(e) => {
        let error_message = format!("Invalid ToolsCallParams: {e}");
        let error = JsonRpcError::new(-32602, error_message);
        return serde_json::to_value(JsonRpcResponse::error(req.id.clone(), error)).unwrap();
      }
    },
    None => {
      let error_message = "Missing required parameters".to_string();
      let error = JsonRpcError::new(-32602, error_message);
      return serde_json::to_value(JsonRpcResponse::error(req.id.clone(), error)).unwrap();
    }
  };

  // Convert arguments to tool format
  let tool_params = params.arguments.unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

  let tool_request = McpRequest {
    tool_name: params.name.clone(),
    parameters: tool_params,
  };

  // Call the appropriate handler based on tool name
  let handler_result = match params.name.as_str() {
    // Read operations
    "list_definitions" => {
      let request = match deserialize_params::<ListDefinitionsRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::read_handlers::list_definitions(app_state, request)
    }
    "list_namespaces" => {
      let request = match deserialize_params::<ListNamespacesRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::namespace_handlers::list_namespaces(app_state, request)
    }
    "get_package_name" => {
      let request = match deserialize_params::<GetPackageNameRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::read_handlers::get_package_name(app_state, request)
    }
    "read_namespace" => {
      let request = match deserialize_params::<ReadNamespaceRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::read_handlers::read_namespace(app_state, request)
    }
    "read_definition" => {
      let request = match deserialize_params::<ReadDefinitionRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::read_handlers::read_definition(app_state, request)
    }

    // Namespace operations
    "add_namespace" => {
      let request = match deserialize_params::<AddNamespaceRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::namespace_handlers::add_namespace(app_state, request)
    }
    "delete_namespace" => {
      let request = match deserialize_params::<DeleteNamespaceRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::namespace_handlers::delete_namespace(app_state, request)
    }
    "update_namespace_imports" => {
      let request = match deserialize_params::<UpdateNamespaceImportsRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::namespace_handlers::update_namespace_imports(app_state, request)
    }

    // Definition operations
    "add_definition" => {
      let request = match deserialize_params::<AddDefinitionRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::definition_handlers::add_definition(app_state, request)
    }
    "delete_definition" => {
      let request = match deserialize_params::<DeleteDefinitionRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::definition_handlers::delete_definition(app_state, request)
    }
    "overwrite_definition" => {
      let request = match deserialize_params::<OverwriteDefinitionRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::definition_handlers::overwrite_definition(app_state, request)
    }
    "update_definition_at" => {
      let request = match deserialize_params::<UpdateDefinitionAtRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::definition_handlers::update_definition_at(app_state, request)
    }
    "read_definition_at" => {
      let request = match deserialize_params::<ReadDefinitionAtRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::definition_handlers::read_definition_at(app_state, request)
    }

    // Module management
    "list_modules" => {
      let request = match deserialize_params::<ListModulesRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::module_handlers::list_modules(app_state, request)
    }
    "get_current_module" => {
      let request = match deserialize_params::<GetCurrentModuleRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::module_handlers::get_current_module(app_state, request)
    }
    "switch_module" => {
      let request = match deserialize_params::<SwitchModuleRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::module_handlers::switch_module(app_state, request)
    }
    "create_config_entry" => {
      let request = match deserialize_params::<CreateModuleRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::module_handlers::create_config_entry(app_state, request)
    }
    "delete_config_entry" => {
      let request = match deserialize_params::<DeleteModuleRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::module_handlers::delete_config_entry(app_state, request)
    }

    // Cirru conversion tools
    "calcit_parse_cirru_to_json" => {
      let request = match deserialize_params::<ParseCirruToJsonRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::cirru_handlers::parse_cirru_to_json(app_state, request)
    }
    "calcit_format_json_to_cirru" => {
      let request = match deserialize_params::<FormatJsonToCirruRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::cirru_handlers::format_json_to_cirru(app_state, request)
    }

    // Documentation query tools
    "query_api_docs" => {
      let request = match deserialize_params::<QueryApiDocsRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::docs_handlers::handle_query_api_docs(app_state, request)
    }
    "query_guidebook" => {
      let request = match deserialize_params::<QueryGuidebookRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::docs_handlers::handle_query_guidebook(app_state, request)
    }
    // Documentation tools
    "list_api_docs" => {
      let request = match deserialize_params::<ListApiDocsRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::docs_handlers::handle_list_api_docs(app_state, request)
    }
    "list_guidebook_docs" => {
      let request = match deserialize_params::<ListGuidebookDocsRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::docs_handlers::handle_list_guidebook_docs(app_state, request)
    }

    // Configuration management tools
    "read_configs" => {
      let request = match deserialize_params::<ReadConfigsRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::config_handlers::read_configs(app_state, request)
    }
    "update_configs" => {
      let request = match deserialize_params::<UpdateConfigsRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::config_handlers::update_configs(app_state, request)
    }

    // Definition documentation tools
    "read_definition_doc" => match serde_json::from_value::<ReadDefinitionDocRequest>(tool_request.parameters) {
      Ok(request) => super::document_handlers::read_definition_doc(app_state, request),
      Err(e) => {
        let error_message = format!("Invalid parameters for read_definition_doc: {e}");
        let error = JsonRpcError::new(-32602, error_message);
        return serde_json::to_value(JsonRpcResponse::error(req.id.clone(), error)).unwrap();
      }
    },
    "update_definition_doc" => match serde_json::from_value::<UpdateDefinitionDocRequest>(tool_request.parameters) {
      Ok(request) => super::document_handlers::update_definition_doc(app_state, request),
      Err(e) => {
        let error_message = format!("Invalid parameters for update_definition_doc: {e}");
        let error = JsonRpcError::new(-32602, error_message);
        return serde_json::to_value(JsonRpcResponse::error(req.id.clone(), error)).unwrap();
      }
    },

    // Module documentation tools
    "list_module_docs" => {
      let request = match deserialize_params::<ListModuleDocsRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::document_handlers::list_module_docs(app_state, request)
    }
    "read_module_doc" => match serde_json::from_value::<ReadModuleDocRequest>(tool_request.parameters) {
      Ok(request) => super::document_handlers::read_module_doc(app_state, request),
      Err(e) => {
        let error_message = format!("Invalid parameters for read_module_doc: {e}");
        let error = JsonRpcError::new(-32602, error_message);
        return serde_json::to_value(JsonRpcResponse::error(req.id.clone(), error)).unwrap();
      }
    },
    "update_module_doc" => match serde_json::from_value::<UpdateModuleDocRequest>(tool_request.parameters) {
      Ok(request) => super::document_handlers::update_module_doc(app_state, request),
      Err(e) => {
        let error_message = format!("Invalid parameters for update_module_doc: {e}");
        let error = JsonRpcError::new(-32602, error_message);
        return serde_json::to_value(JsonRpcResponse::error(req.id.clone(), error)).unwrap();
      }
    },
    "rename_module_doc" => match serde_json::from_value::<RenameModuleDocRequest>(tool_request.parameters) {
      Ok(request) => super::document_handlers::rename_module_doc(app_state, request),
      Err(e) => {
        let error_message = format!("Invalid parameters for rename_module_doc: {e}");
        let error = JsonRpcError::new(-32602, error_message);
        return serde_json::to_value(JsonRpcResponse::error(req.id.clone(), error)).unwrap();
      }
    },
    "delete_module_doc" => match serde_json::from_value::<DeleteModuleDocRequest>(tool_request.parameters) {
      Ok(request) => super::document_handlers::delete_module_doc(app_state, request),
      Err(e) => {
        let error_message = format!("Invalid parameters for delete_module_doc: {e}");
        let error = JsonRpcError::new(-32602, error_message);
        return serde_json::to_value(JsonRpcResponse::error(req.id.clone(), error)).unwrap();
      }
    },

    // Dependency documentation tools (read-only)
    "list_dependency_docs" => {
      let request = match deserialize_params::<ListDependencyDocsRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::document_handlers::list_dependency_docs(app_state, request)
    }
    "read_dependency_definition_doc" => {
      let request = match deserialize_params::<ReadDependencyDefinitionDocRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::document_handlers::read_dependency_definition_doc(app_state, request)
    }
    "read_dependency_module_doc" => {
      let request = match deserialize_params::<ReadDependencyModuleDocRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      super::document_handlers::read_dependency_module_doc(app_state, request)
    }

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
