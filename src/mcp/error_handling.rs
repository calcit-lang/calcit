//! Unified error handling for MCP protocol compliance
//!
//! This module provides utilities to handle errors according to MCP protocol specifications:
//! - Protocol errors: Use JsonRpcError for invalid requests, unknown methods, etc.
//! - Tool execution errors: Use isError flag in tool results for business logic failures

use super::jsonrpc::{JsonRpcError, JsonRpcResponse, ToolCallContent, ToolsCallResult};
use serde_json::Value;

/// Create a protocol error response for invalid requests, unknown tools, etc.
pub fn create_protocol_error(req_id: Option<Value>, code: i32, message: String) -> Value {
  let error = JsonRpcError::new(code, message);
  serde_json::to_value(JsonRpcResponse::error(req_id, error)).unwrap()
}

/// Create a tool execution error result with isError flag
pub fn create_tool_execution_error(req_id: Option<Value>, error_message: String) -> Value {
  let tool_call_result = ToolsCallResult {
    content: vec![ToolCallContent::Text { text: error_message }],
    is_error: Some(true),
  };

  serde_json::to_value(JsonRpcResponse::success(req_id, serde_json::to_value(tool_call_result).unwrap())).unwrap()
}

/// Create a successful tool result
pub fn create_tool_success(req_id: Option<Value>, result_data: Value) -> Value {
  let tool_call_result = ToolsCallResult {
    content: vec![ToolCallContent::Text {
      text: serde_json::to_string(&result_data).unwrap_or_else(|_| "null".to_string()),
    }],
    is_error: None,
  };

  serde_json::to_value(JsonRpcResponse::success(req_id, serde_json::to_value(tool_call_result).unwrap())).unwrap()
}

/// Common MCP error codes
pub mod error_codes {
  /// Invalid Request
  pub const INVALID_REQUEST: i32 = -32600;
  /// Method not found
  pub const METHOD_NOT_FOUND: i32 = -32601;
  /// Invalid params
  pub const INVALID_PARAMS: i32 = -32602;
  /// Internal error
  pub const INTERNAL_ERROR: i32 = -32603;
  /// Parse error
  pub const PARSE_ERROR: i32 = -32700;

  /// MCP-specific: Tool not found
  pub const TOOL_NOT_FOUND: i32 = -32002;
  /// MCP-specific: Session not initialized
  pub const SESSION_NOT_INITIALIZED: i32 = -32001;
}
