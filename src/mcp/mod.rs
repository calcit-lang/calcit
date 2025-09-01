//! MCP (Model Context Protocol) server implementation for Calcit
//!
//! This module provides tools and handlers for interacting with Calcit projects
//! through the MCP protocol.

use crate::snapshot;
use std::collections::HashMap;

pub mod cirru_handlers;
pub mod cirru_utils;
pub mod config_handlers;
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

// Re-export main structs and functions
pub use cirru_handlers::*;
pub use cirru_utils::*;
pub use config_handlers::*;
pub use definition_handlers::*;
pub use docs_handlers::*;
pub use jsonrpc::*;
pub use mcp_handlers::*;
pub use module_handlers::*;
pub use namespace_handlers::*;
// read_handlers functions are imported individually to avoid conflicts
pub use read_handlers::{list_definitions, read_namespace, read_definition, get_package_name};
pub use tools::{McpRequest, McpTool, McpToolParameter, get_mcp_tools, get_standard_mcp_tools};
