//! MCP (Model Context Protocol) server implementation for Calcit
//!
//! This module provides tools and handlers for interacting with Calcit projects
//! through the MCP protocol.

use crate::snapshot;
use std::collections::HashMap;

pub mod cirru_handlers;
pub mod cirru_utils;
pub mod definition_handlers;
pub mod docs_handlers;
pub mod jsonrpc;
pub mod mcp_handlers;
pub mod module_handlers;
pub mod namespace_handlers;
pub mod read_handlers;
pub mod tools;

#[derive(Clone)]
pub struct AppState {
  pub compact_cirru_path: String,
  pub current_module_name: String,
  pub port: u16,
  pub module_cache: std::sync::Arc<std::sync::RwLock<HashMap<String, snapshot::Snapshot>>>,
}

// 重新导出主要的结构体和函数
pub use cirru_handlers::*;
pub use cirru_utils::*;
pub use definition_handlers::*;
pub use docs_handlers::*;
pub use jsonrpc::*;
pub use mcp_handlers::*;
pub use module_handlers::*;
pub use namespace_handlers::*;
pub use tools::{McpRequest, McpTool, McpToolParameter, get_mcp_tools, get_standard_mcp_tools};
