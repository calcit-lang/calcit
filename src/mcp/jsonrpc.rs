use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 Request
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
  pub jsonrpc: String,
  pub id: Option<serde_json::Value>,
  pub method: String,
  pub params: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 Response
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
  pub jsonrpc: String,
  pub id: Option<serde_json::Value>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub result: Option<serde_json::Value>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 Error
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
  pub code: i32,
  pub message: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub data: Option<serde_json::Value>,
}

/// MCP Initialize Request Parameters
#[derive(Debug, Serialize, Deserialize)]
pub struct InitializeParams {
  #[serde(rename = "protocolVersion")]
  pub protocol_version: String,
  pub capabilities: ClientCapabilities,
  #[serde(rename = "clientInfo")]
  pub client_info: ClientInfo,
}

/// MCP Initialize Response Result
#[derive(Debug, Serialize, Deserialize)]
pub struct InitializeResult {
  #[serde(rename = "protocolVersion")]
  pub protocol_version: String,
  pub capabilities: ServerCapabilities,
  #[serde(rename = "serverInfo")]
  pub server_info: ServerInfo,
}

/// Client Capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCapabilities {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub roots: Option<RootsCapability>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub sampling: Option<SamplingCapability>,
}

/// Server Capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilities {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub tools: Option<ToolsCapability>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub resources: Option<ResourcesCapability>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub prompts: Option<PromptsCapability>,
}

/// Tools Capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsCapability {
  #[serde(rename = "listChanged", skip_serializing_if = "Option::is_none")]
  pub list_changed: Option<bool>,
}

/// Resources Capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesCapability {
  #[serde(rename = "listChanged", skip_serializing_if = "Option::is_none")]
  pub list_changed: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub subscribe: Option<bool>,
}

/// Prompts Capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptsCapability {
  #[serde(rename = "listChanged", skip_serializing_if = "Option::is_none")]
  pub list_changed: Option<bool>,
}

/// Roots Capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootsCapability {
  #[serde(rename = "listChanged", skip_serializing_if = "Option::is_none")]
  pub list_changed: Option<bool>,
}

/// Sampling Capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingCapability {}

/// Client Info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
  pub name: String,
  pub version: String,
}

/// Server Info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
  pub name: String,
  pub version: String,
}

/// Tools List Request Parameters
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolsListParams {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub cursor: Option<String>,
}

/// Tools List Response Result
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolsListResult {
  pub tools: Vec<Tool>,
  #[serde(rename = "nextCursor", skip_serializing_if = "Option::is_none")]
  pub next_cursor: Option<String>,
}

/// Tool Definition
#[derive(Debug, Serialize, Deserialize)]
pub struct Tool {
  pub name: String,
  pub description: String,
  #[serde(rename = "inputSchema")]
  pub input_schema: serde_json::Value,
}

/// Tools Call Request Parameters
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolsCallParams {
  pub name: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub arguments: Option<serde_json::Value>,
}

/// Tools Call Response Result
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolsCallResult {
  pub content: Vec<ToolCallContent>,
  #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
  pub is_error: Option<bool>,
}

/// Tool Call Content
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolCallContent {
  #[serde(rename = "text")]
  Text { text: String },
  #[serde(rename = "image")]
  Image {
    data: String,
    #[serde(rename = "mimeType")]
    mime_type: String,
  },
  #[serde(rename = "resource")]
  Resource { resource: ResourceReference },
}

/// Resource Reference
#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceReference {
  pub uri: String,
  pub text: Option<String>,
}

/// Standard JSON-RPC Error Codes
pub mod error_codes {
  pub const PARSE_ERROR: i32 = -32700;
  pub const INVALID_REQUEST: i32 = -32600;
  pub const METHOD_NOT_FOUND: i32 = -32601;
  pub const INVALID_PARAMS: i32 = -32602;
  pub const INTERNAL_ERROR: i32 = -32603;

  // MCP specific error codes
  pub const INVALID_PROTOCOL_VERSION: i32 = -32000;
  pub const TOOL_NOT_FOUND: i32 = -32001;
  pub const TOOL_EXECUTION_ERROR: i32 = -32002;
}

impl JsonRpcResponse {
  pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
    Self {
      jsonrpc: "2.0".to_string(),
      id,
      result: Some(result),
      error: None,
    }
  }

  pub fn error(id: Option<serde_json::Value>, error: JsonRpcError) -> Self {
    Self {
      jsonrpc: "2.0".to_string(),
      id,
      result: None,
      error: Some(error),
    }
  }
}

impl JsonRpcError {
  pub fn new(code: i32, message: String) -> Self {
    Self { code, message, data: None }
  }

  pub fn with_data(code: i32, message: String, data: serde_json::Value) -> Self {
    Self {
      code,
      message,
      data: Some(data),
    }
  }

  pub fn parse_error() -> Self {
    Self::new(error_codes::PARSE_ERROR, "Parse error".to_string())
  }

  pub fn invalid_request() -> Self {
    Self::new(error_codes::INVALID_REQUEST, "Invalid Request".to_string())
  }

  pub fn method_not_found() -> Self {
    Self::new(error_codes::METHOD_NOT_FOUND, "Method not found".to_string())
  }

  pub fn invalid_params() -> Self {
    Self::new(error_codes::INVALID_PARAMS, "Invalid params".to_string())
  }

  pub fn internal_error() -> Self {
    Self::new(error_codes::INTERNAL_ERROR, "Internal error".to_string())
  }

  pub fn tool_not_found(tool_name: &str) -> Self {
    Self::new(error_codes::TOOL_NOT_FOUND, format!("Tool '{tool_name}' not found"))
  }

  pub fn tool_execution_error(message: String) -> Self {
    Self::new(error_codes::TOOL_EXECUTION_ERROR, message)
  }
}

/// MCP Notification (no response expected)
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcNotification {
  pub jsonrpc: String,
  pub method: String,
  pub params: Option<serde_json::Value>,
}

/// Session state for MCP protocol lifecycle
#[derive(Debug, Clone)]
pub enum SessionState {
  Uninitialized,
  Initializing,
  Initialized,
  Shutdown,
}

/// Session management for MCP protocol
#[derive(Debug, Clone)]
pub struct McpSession {
  pub state: SessionState,
  pub client_info: Option<ClientInfo>,
  pub client_capabilities: Option<ClientCapabilities>,
  pub server_capabilities: Option<ServerCapabilities>,
}

impl Default for McpSession {
  fn default() -> Self {
    Self {
      state: SessionState::Uninitialized,
      client_info: None,
      client_capabilities: None,
      server_capabilities: None,
    }
  }
}

impl McpSession {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn is_initialized(&self) -> bool {
    matches!(self.state, SessionState::Initialized)
  }

  pub fn initialize(
    &mut self,
    client_info: ClientInfo,
    client_capabilities: ClientCapabilities,
    server_capabilities: ServerCapabilities,
  ) {
    self.state = SessionState::Initialized;
    self.client_info = Some(client_info);
    self.client_capabilities = Some(client_capabilities);
    self.server_capabilities = Some(server_capabilities);
  }

  pub fn shutdown(&mut self) {
    self.state = SessionState::Shutdown;
  }
}

impl JsonRpcNotification {
  pub fn new(method: String, params: Option<serde_json::Value>) -> Self {
    Self {
      jsonrpc: "2.0".to_string(),
      method,
      params,
    }
  }

  pub fn initialized() -> Self {
    Self::new("notifications/initialized".to_string(), None)
  }

  pub fn cancelled(id: serde_json::Value, reason: Option<String>) -> Self {
    let params = serde_json::json!({
        "requestId": id,
        "reason": reason
    });
    Self::new("notifications/cancelled".to_string(), Some(params))
  }

  pub fn progress(progress_token: serde_json::Value, progress: f64, total: Option<f64>) -> Self {
    let mut params = serde_json::json!({
        "progressToken": progress_token,
        "progress": progress
    });
    if let Some(total) = total {
      params["total"] = serde_json::json!(total);
    }
    Self::new("notifications/progress".to_string(), Some(params))
  }
}
