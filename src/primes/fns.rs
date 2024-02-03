use std::sync::Arc;

use rpds::HashTrieMapSync;

use crate::{Calcit, CalcitItems};

#[derive(Debug, Clone)]
pub struct CalcitFn {
  pub name: Arc<str>,
  /// where it was defined
  pub def_ns: Arc<str>,
  pub scope: Arc<CalcitScope>,
  pub args: Arc<Vec<Arc<str>>>,
  pub body: Arc<CalcitItems>,
}

/// Macro variant of Calcit data
#[derive(Debug, Clone)]
pub struct CalcitMacro {
  pub name: Arc<str>,
  /// where it was defined
  pub def_ns: Arc<str>,
  pub args: Arc<Vec<Arc<str>>>,
  pub body: Arc<CalcitItems>,
}

/// scope in the semantics of persistent data structure
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalcitScope(pub rpds::HashTrieMapSync<Arc<str>, Calcit>);

impl Default for CalcitScope {
  fn default() -> Self {
    CalcitScope(HashTrieMapSync::new_sync())
  }
}

impl CalcitScope {
  /// create a new scope from a piece of hashmap
  pub fn new(data: rpds::HashTrieMapSync<Arc<str>, Calcit>) -> Self {
    CalcitScope(data)
  }
  /// load value of a symbol from the scope
  pub fn get(&self, key: &str) -> Option<&Calcit> {
    self.0.get(key)
  }
  /// mutable insertiong of variable
  pub fn insert_mut(&mut self, key: Arc<str>, value: Calcit) {
    self.0.insert_mut(key, value);
  }
}
