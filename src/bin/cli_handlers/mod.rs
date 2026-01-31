//! CLI handlers for standalone commands that don't need full program loading
//!
//! These handlers implement: query, docs, cirru, libs, edit, tree subcommands

mod cirru;
mod cirru_validator;
mod common;
mod docs;
mod edit;
mod libs;
mod query;
mod tips;
mod tree;

pub use cirru::handle_cirru_command;
pub use docs::handle_docs_command;
pub use edit::handle_edit_command;
pub use libs::handle_libs_command;
pub use query::handle_query_command;
pub use tree::handle_tree_command;
// Re-export when needed by other modules; keep internal for now to avoid unused-import warnings
