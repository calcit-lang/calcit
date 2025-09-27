use crate::mcp::tools::OperateDefinitionAtWithLeafRequest;

use super::AppState;
use super::error_handling::{create_protocol_error, create_tool_execution_error, create_tool_success, error_codes};
use super::jsonrpc::*;
use super::tools::{
  AddNamespaceRequest, CreateModuleRequest, DeleteDefinitionRequest, DeleteModuleRequest, DeleteNamespaceRequest,
  FeedbackToCalcitMcpServerRequest, FetchCalcitLibrariesRequest, FormatJsonToCirruRequest, GenerateCalcitIncrementalRequest,
  GetCurrentModuleRequest, GetPackageNameRequest, GrabCalcitRunnerLogsRequest, ListApiDocsRequest, ListCalcitWorkMemoryRequest,
  ListDefinitionsRequest, ListDependencyDocsRequest, ListGuidebookDocsRequest, ListModulesRequest, ListNamespacesRequest, McpRequest,
  OperateDefinitionAtRequest, ParseCirruEdnToJsonRequest, ParseCirruToJsonRequest, QueryCalcitApisRequest, QueryCalcitReferenceRequest,
  ReadCalcitWorkMemoryRequest, ReadConfigsRequest, ReadDefinitionAtRequest, ReadDefinitionDocRequest,
  ReadDependencyDefinitionDocRequest, ReadDependencyModuleDocRequest, ReadNamespaceRequest, StartCalcitRunnerRequest,
  StopCalcitRunnerRequest, UpdateConfigsRequest, UpdateDefinitionDocRequest, UpdateNamespaceDocRequest, UpdateNamespaceImportsRequest,
  UpsertDefinitionRequest, WriteCalcitWorkMemoryRequest, get_standard_mcp_tools,
};
use axum::response::Json as ResponseJson;
use colored::*;
use serde_json::Value;
use std::sync::{Arc, Mutex, OnceLock};

// Global session storage (in a real implementation, this would be per-connection)
static GLOBAL_SESSION: OnceLock<Arc<Mutex<McpSession>>> = OnceLock::new();

fn get_session() -> Arc<Mutex<McpSession>> {
  GLOBAL_SESSION.get_or_init(|| Arc::new(Mutex::new(McpSession::new()))).clone()
}

/// Handle JSON-RPC 2.0 requests (Axum version)
pub async fn handle_jsonrpc_axum(data: Arc<AppState>, req: JsonRpcRequest) -> ResponseJson<Value> {
  // Check if this is a notification (no id field)
  let is_notification = req.id.is_none();

  // Extract tool name for tools/call method
  let tool_name = if req.method == "tools/call" {
    req
      .params
      .as_ref()
      .and_then(|p| p.get("name"))
      .and_then(|n| n.as_str())
      .unwrap_or("unknown")
  } else {
    ""
  };

  // Log request with colors
  if is_notification {
    println!("{} {}", "📢 NOTIFICATION".cyan().bold(), req.method.green().bold());
  } else if req.method == "tools/call" {
    println!("{} {}", "⚡️ TOOL CALL".blue().bold(), tool_name.yellow().bold());
  } else {
    println!("{} {}", "📡 RPC".blue().bold(), req.method.green().bold());
  }

  // Show simplified request info
  if let Some(params) = &req.params {
    if req.method == "tools/call" {
      if let Some(args) = params.get("arguments") {
        let args_json = serde_json::to_string_pretty(args).unwrap_or_else(|_| "<invalid>".to_string());
        println!("{}\n{}", "   Arguments:".dimmed(), args_json.dimmed());
      }
    } else {
      let params_json = serde_json::to_string_pretty(params).unwrap_or_else(|_| "<invalid>".to_string());
      println!("{}\n{}", "   Params:".dimmed(), params_json.dimmed());
    }
  }

  // Handle notifications separately (they don't expect responses)
  if is_notification {
    match req.method.as_str() {
      "notifications/initialized" => {
        handle_initialized_notification(&req);
        println!("{}", "✅ NOTIFICATION PROCESSED".green().bold());
      }
      _ => {
        println!("{} Unknown notification: {}", "⚠️ WARNING".yellow().bold(), req.method.yellow());
      }
    }
    println!(); // Add blank line for separation
    // For notifications, return an empty response (this won't be sent to client)
    return ResponseJson(serde_json::json!({}));
  }

  let response = match req.method.as_str() {
    "initialize" => handle_initialize_axum(&data, &req),
    "tools/list" => handle_tools_list_axum(&data, &req),
    "tools/call" => handle_tools_call_axum(&data, &req).await,
    _ => {
      println!("{} Unknown method: {}", "❌ ERROR".red().bold(), req.method.red());
      let error = JsonRpcError::method_not_found();
      serde_json::to_value(JsonRpcResponse::error(req.id.clone(), error)).unwrap()
    }
  };

  // Log response with colors
  if response.get("error").is_some() {
    println!("{}", "❌ RESPONSE ERROR".red().bold());
    if let Some(error) = response.get("error") {
      let error_json = serde_json::to_string_pretty(error).unwrap_or_else(|_| "<invalid>".to_string());
      println!("{}\n{}", "   Error:".dimmed(), error_json.dimmed());
    }
  } else {
    if req.method == "tools/call" {
      println!("{} {}", "√ TOOL RESULT".green().bold(), tool_name.dimmed());
    } else {
      println!("{} {}", "√ RPC RESULT".green().bold(), req.method.dimmed());
    }
    if let Some(result) = response.get("result") {
      // Check if result is a text-only response with standard MCP structure
      let is_simple_text = is_simple_text_result(result);

      if is_simple_text {
        // Simple text result - show status and content only
        let content = result.get("content").unwrap();
        let content_array = content.as_array().unwrap();
        let first_item = content_array.first().unwrap();
        let text = first_item.get("text").unwrap();
        let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
        let status = if is_error { "Error" } else { "Success" };
        let text_content = text.as_str().unwrap_or("<invalid text>");
        println!("{} {}", format!("   {status}:").dimmed(), text_content.dimmed());
      } else {
        // Fallback to full JSON display for complex results
        let result_json = serde_json::to_string_pretty(result).unwrap_or_else(|_| "<invalid>".to_string());
        println!("{}\n{}", "   Result:".dimmed(), result_json.dimmed());
      }
    }
  }
  println!(); // Add blank line for separation

  ResponseJson(response)
}

/// Check if the result is a simple text response with standard MCP structure
fn is_simple_text_result(result: &Value) -> bool {
  if let Some(content) = result.get("content") {
    if let Some(content_array) = content.as_array() {
      if content_array.len() == 1 {
        if let Some(first_item) = content_array.first() {
          if let Some(type_val) = first_item.get("type") {
            if let Some(type_str) = type_val.as_str() {
              if type_str == "text" {
                return first_item.get("text").is_some();
              }
            }
          }
        }
      }
    }
  }
  false
}

/// Handle notifications/initialized notification
fn handle_initialized_notification(_req: &JsonRpcRequest) {
  println!("{} Client initialization complete", "✅ INITIALIZED".green().bold());
  // Notifications don't require responses according to JSON-RPC 2.0 spec
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
      version: env!("CARGO_PKG_VERSION").to_string(),
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
      return create_protocol_error(
        req.id.clone(),
        error_codes::INTERNAL_ERROR,
        "Failed to acquire session lock".to_string(),
      );
    }
  };

  if !session_guard.is_initialized() {
    return create_protocol_error(
      req.id.clone(),
      error_codes::SESSION_NOT_INITIALIZED,
      "Session not initialized".to_string(),
    );
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
  match serde_json::from_value(parameters.clone()) {
    Ok(request) => Ok(request),
    Err(e) => {
      // Provide detailed error information with specific fix suggestions
      let error_details = e.to_string();
      let received_params = serde_json::to_string_pretty(&parameters).unwrap_or_else(|_| "<unparseable>".to_string());

      let mut fix_suggestions = Vec::new();

      // Analyze common parameter errors and provide specific fixes
      if error_details.contains("missing field") {
        if let Some(field_name) = extract_missing_field(&error_details) {
          fix_suggestions.push(format!("Add the required field '{field_name}' to your parameters"));
        }
      }

      if error_details.contains("invalid type") {
        fix_suggestions.push("Check that all parameter types match the expected schema (strings, arrays, objects)".to_string());
      }

      if parameters.get("coord").is_some() {
        if let Some(coord_val) = parameters.get("coord") {
          if coord_val.is_string() {
            fix_suggestions.push(
              "The 'coord' parameter must be a JSON array of integers, not a string. Index starts from 0 (zero-based indexing). Example: [1, 2] instead of \"[1, 2]\"".to_string(),
            );
          }
        }
      }

      if parameters.get("code").is_some() {
        if let Some(code_val) = parameters.get("code") {
          if code_val.is_string() {
            fix_suggestions.push("The 'code' parameter must be a JSON array, not a string. Example: [\"fn\", [\"x\"], [\"*\", \"x\", \"x\"]] instead of \"[fn [x] [* x x]]\"".to_string());
          }
        }
      }

      let fix_text = if fix_suggestions.is_empty() {
        "Please check the tool documentation for the correct parameter format.".to_string()
      } else {
        format!(
          "Suggested fixes:\n{}",
          fix_suggestions.iter().map(|s| format!("• {s}")).collect::<Vec<_>>().join("\n")
        )
      };

      let error_message = format!("Invalid parameters: {error_details}\n\nReceived parameters:\n{received_params}\n\n{fix_text}");
      Err(create_protocol_error(req_id, error_codes::INVALID_PARAMS, error_message))
    }
  }
}

// Helper function to extract missing field name from error message
fn extract_missing_field(error_msg: &str) -> Option<String> {
  if let Some(start) = error_msg.find("missing field `") {
    let start = start + "missing field `".len();
    if let Some(end) = error_msg[start..].find('`') {
      return Some(error_msg[start..start + end].to_string());
    }
  }
  None
}

/// Check if a tool handler result contains an error and convert it to appropriate MCP response
fn handle_tool_result(req_id: Option<Value>, handler_result: ResponseJson<Value>) -> Value {
  let result_value = handler_result.0;

  // Check if the result contains an "error" field (business logic error)
  if let Some(error_message) = result_value.get("error") {
    if let Some(error_str) = error_message.as_str() {
      return create_tool_execution_error(req_id, error_str.to_string());
    }
  }

  // Success case
  create_tool_success(req_id, result_value)
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
        return create_protocol_error(req.id.clone(), error_codes::INVALID_PARAMS, error_message);
      }
    },
    None => {
      return create_protocol_error(
        req.id.clone(),
        error_codes::INVALID_PARAMS,
        "Missing required parameters".to_string(),
      );
    }
  };

  // Convert arguments to tool format
  let tool_params = params.arguments.unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

  let tool_request = McpRequest {
    tool_name: params.name.clone(),
    parameters: tool_params,
  };

  // Call the appropriate handler based on tool name
  let _ = match params.name.as_str() {
    // Read operations
    "list_namespace_definitions" => {
      let request = match deserialize_params::<ListDefinitionsRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::read_handlers::list_namespace_definitions(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    "list_namespaces" => {
      let request = match deserialize_params::<ListNamespacesRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::namespace_handlers::list_namespaces(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    "get_package_name" => {
      let request = match deserialize_params::<GetPackageNameRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::read_handlers::get_package_name(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    "read_namespace" => {
      let request = match deserialize_params::<ReadNamespaceRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::read_handlers::read_namespace(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    // Namespace operations
    "add_namespace" => {
      let request = match deserialize_params::<AddNamespaceRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::namespace_handlers::add_namespace(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    "delete_namespace" => {
      let request = match deserialize_params::<DeleteNamespaceRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::namespace_handlers::delete_namespace(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    "update_namespace_imports" => {
      let request = match deserialize_params::<UpdateNamespaceImportsRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::namespace_handlers::update_namespace_imports(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    "update_namespace_doc" => {
      let request = match deserialize_params::<UpdateNamespaceDocRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::namespace_handlers::update_namespace_doc(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }

    // Definition operations
    "upsert_definition" => {
      let request = match deserialize_params::<UpsertDefinitionRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::definition_handlers::upsert_definition(
        app_state,
        request.namespace,
        request.definition,
        request.syntax_tree,
        request.doc,
        request.replacing,
      );
      return handle_tool_result(req.id.clone(), result);
    }
    "delete_definition" => {
      let request = match deserialize_params::<DeleteDefinitionRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::definition_handlers::delete_definition(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    "update_definition_doc" => {
      let request = match deserialize_params::<UpdateDefinitionDocRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::definition_handlers::update_definition_doc(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    "operate_definition_at" => {
      let request = match deserialize_params::<OperateDefinitionAtRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::definition_handlers::operate_definition_at(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    "operate_definition_at_with_leaf" => {
      let request = match deserialize_params::<OperateDefinitionAtWithLeafRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::definition_handlers::operate_definition_at_with_leaf(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    "read_definition_at" => {
      let request = match deserialize_params::<ReadDefinitionAtRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::definition_handlers::read_definition_at(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    "read_definition_doc" => {
      let request = match deserialize_params::<ReadDefinitionDocRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::definition_handlers::read_definition_doc(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }

    // Module management
    "list_modules" => {
      let request = match deserialize_params::<ListModulesRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::module_handlers::list_modules(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    "get_current_module" => {
      let request = match deserialize_params::<GetCurrentModuleRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::module_handlers::get_current_module(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    "create_config_entry" => {
      let request = match deserialize_params::<CreateModuleRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::module_handlers::create_config_entry(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    "delete_config_entry" => {
      let request = match deserialize_params::<DeleteModuleRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::module_handlers::delete_config_entry(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }

    // Cirru conversion tools
    "calcit_parse_cirru_to_json" => {
      let request = match deserialize_params::<ParseCirruToJsonRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::cirru_handlers::parse_cirru_to_json(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    "calcit_format_json_to_cirru" => {
      let request = match deserialize_params::<FormatJsonToCirruRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::cirru_handlers::format_json_to_cirru(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }

    // Documentation query tools
    "query_calcit_apis" => {
      let request = match deserialize_params::<QueryCalcitApisRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::docs_handlers::handle_query_calcit_apis(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    "query_calcit_reference" => {
      let request = match deserialize_params::<QueryCalcitReferenceRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::docs_handlers::handle_query_calcit_reference(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    // Documentation tools
    "list_api_docs" => {
      let request = match deserialize_params::<ListApiDocsRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::docs_handlers::handle_list_api_docs(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    "list_guidebook_docs" => {
      let request = match deserialize_params::<ListGuidebookDocsRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::docs_handlers::handle_list_guidebook_docs(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }

    // Configuration management tools
    "read_configs" => {
      let request = match deserialize_params::<ReadConfigsRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::config_handlers::read_configs(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    "update_configs" => {
      let request = match deserialize_params::<UpdateConfigsRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::config_handlers::update_configs(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }

    // Library tools
    "fetch_calcit_libraries" => {
      let request = match deserialize_params::<FetchCalcitLibrariesRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::library_handlers::handle_fetch_calcit_libraries(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    "parse_cirru_edn_to_json" => {
      let request = match deserialize_params::<ParseCirruEdnToJsonRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::library_handlers::handle_parse_cirru_edn_to_json(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }

    // Dependency documentation tools (read-only)
    "list_dependency_docs" => {
      let request = match deserialize_params::<ListDependencyDocsRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::dependency_doc_handlers::list_dependency_docs(&app_state.state_manager, request);
      return handle_tool_result(req.id.clone(), result);
    }
    "read_dependency_definition_doc" => {
      let request = match deserialize_params::<ReadDependencyDefinitionDocRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::dependency_doc_handlers::read_dependency_definition_doc(&app_state.state_manager, request);
      return handle_tool_result(req.id.clone(), result);
    }

    "read_dependency_module_doc" => {
      let request = match deserialize_params::<ReadDependencyModuleDocRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::dependency_doc_handlers::read_dependency_module_doc(&app_state.state_manager, request);
      return handle_tool_result(req.id.clone(), result);
    }

    // Calcit Runner Management Tools
    "start_calcit_runner" => {
      let request = match deserialize_params::<StartCalcitRunnerRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::calcit_runner_handlers::start_calcit_runner(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    "grab_calcit_runner_logs" => {
      let request = match deserialize_params::<GrabCalcitRunnerLogsRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::calcit_runner_handlers::grab_calcit_runner_logs(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    "stop_calcit_runner" => {
      let request = match deserialize_params::<StopCalcitRunnerRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::calcit_runner_handlers::stop_calcit_runner(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    "generate_calcit_incremental" => {
      let request = match deserialize_params::<GenerateCalcitIncrementalRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::calcit_runner_handlers::generate_incremental_file(app_state, request);
      return handle_tool_result(req.id.clone(), result);
    }
    // Memory Management Tools
    "list_calcit_work_memory" => {
      let request = match deserialize_params::<ListCalcitWorkMemoryRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::memory_handlers::list_calcit_work_memory(request);
      return handle_tool_result(req.id.clone(), result);
    }
    "read_calcit_work_memory" => {
      let request = match deserialize_params::<ReadCalcitWorkMemoryRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::memory_handlers::read_calcit_work_memory(request);
      return handle_tool_result(req.id.clone(), result);
    }
    "write_calcit_work_memory" => {
      let request = match deserialize_params::<WriteCalcitWorkMemoryRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::memory_handlers::write_calcit_work_memory(request);
      return handle_tool_result(req.id.clone(), result);
    }
    "feedback_to_calcit_mcp_server" => {
      let request = match deserialize_params::<FeedbackToCalcitMcpServerRequest>(tool_request.parameters, req.id.clone()) {
        Ok(req) => req,
        Err(error_response) => return error_response,
      };
      let result = super::memory_handlers::feedback_to_calcit_mcp_server(request);
      return handle_tool_result(req.id.clone(), result);
    }

    _ => {
      return create_protocol_error(
        req.id.clone(),
        error_codes::TOOL_NOT_FOUND,
        format!("Unknown tool: {}", params.name),
      );
    }
  };
}
