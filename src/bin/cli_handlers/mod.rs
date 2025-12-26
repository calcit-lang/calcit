//! CLI handlers for standalone commands that don't need full program loading
//!
//! These handlers implement: query, docs, cirru, libs, edit, tree subcommands

mod cirru;
mod common;
mod docs;
mod edit;
mod libs;
mod query;
mod tree;

pub use cirru::handle_cirru_command;
pub use docs::handle_docs_command;
pub use edit::handle_edit_command;
pub use libs::handle_libs_command;
pub use query::handle_query_command;
pub use tree::handle_tree_command;
