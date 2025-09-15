//! MCP (Model Context Protocol) server implementation for Calcit
//!
//! This module provides tools and handlers for interacting with Calcit projects
//! through the MCP protocol.

pub mod calcit_runner_handlers;
pub mod cirru_handlers;
pub mod cirru_utils;
pub mod config_handlers;
pub mod definition_handlers;
pub mod definition_update;
pub mod definition_utils;
pub mod dependency_doc_handlers;
pub mod docs_handlers;
pub mod error_handling;
pub mod jsonrpc;
pub mod library_handlers;
pub mod mcp_handlers;
pub mod memory_handlers;
pub mod module_handlers;
pub mod namespace_handlers;
pub mod read_handlers;
pub mod state_manager;
pub mod tools;
pub mod validation;

#[derive(Clone)]
pub struct AppState {
  pub compact_cirru_path: String,
  pub current_module_name: String,
  pub port: u16,
  pub state_manager: state_manager::StateManager,
}

// Re-export main structs and functions
pub use calcit_runner_handlers::*;
pub use cirru_handlers::*;
pub use cirru_utils::*;
pub use config_handlers::*;
pub use definition_handlers::*;
pub use dependency_doc_handlers::*;
pub use docs_handlers::*;
pub use jsonrpc::*;
pub use mcp_handlers::*;
pub use module_handlers::*;
pub use namespace_handlers::*;
// read_handlers functions are imported individually to avoid conflicts
pub use read_handlers::{get_package_name, list_namespace_definitions, read_namespace};
pub use state_manager::StateManager;
pub use tools::{McpRequest, McpToolWithSchema, get_mcp_tools_with_schema, get_standard_mcp_tools};
