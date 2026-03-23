//! CLI handlers for standalone commands that don't need full program loading
//!
//! These handlers implement: query, docs, cirru, libs, edit, tree subcommands

mod chunk_display;
mod cirru;
mod cirru_validator;
mod command_echo;
mod common;
mod docs;
mod edit;
mod libs;
mod markdown_read;
mod query;
mod tips;
mod tree;

pub use cirru::handle_cirru_command;
pub use command_echo::{print_command_echo, should_echo_command};
pub use docs::handle_docs_command;
pub use edit::handle_edit_command;
pub use libs::handle_libs_command;
pub use query::handle_query_command;
pub use tips::{set_tips_level, suppress_command_guidance};
pub use tree::handle_tree_command;
// Re-export when needed by other modules; keep internal for now to avoid unused-import warnings
