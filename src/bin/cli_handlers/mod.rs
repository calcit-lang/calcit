//! CLI handlers for standalone commands that don't need full program loading
//!
//! These handlers implement: query, docs, cirru, libs, edit subcommands

mod cirru;
mod docs;
mod edit;
mod libs;
mod query;

pub use cirru::handle_cirru_command;
pub use docs::handle_docs_command;
pub use edit::handle_edit_command;
pub use libs::handle_libs_command;
pub use query::handle_query_command;
