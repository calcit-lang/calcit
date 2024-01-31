use std::sync::Arc;

use super::SymbolResolved;

#[derive(Debug, Clone)]
pub struct CalcitSymbolInfo {
  pub ns: Arc<str>,
  pub at_def: Arc<str>,
  pub resolved: Option<Arc<SymbolResolved>>,
}
