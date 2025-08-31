//! MCP (Model Context Protocol) server implementation for Calcit
//! 
//! This module provides tools and handlers for interacting with Calcit projects
//! through the MCP protocol.

pub mod tools;
pub mod read_handlers;
pub mod namespace_handlers;
pub mod definition_handlers;
pub mod module_handlers;
pub mod cirru_handlers;
pub mod cirru_utils;

// 重新导出主要的结构体和函数
