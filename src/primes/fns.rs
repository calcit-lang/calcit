use std::sync::Arc;

use crate::CalcitItems;

#[derive(Debug, Clone)]
pub struct CalcitFn {}

/// Macro variant of Calcit data
#[derive(Debug, Clone)]
pub struct CalcitMacro {
  pub name: Arc<str>,
  /// where it was defined
  pub def_ns: Arc<str>,
  pub args: Arc<Vec<Arc<str>>>,
  pub body: Arc<CalcitItems>,
}
