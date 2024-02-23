use std::{fmt::Display, sync::Arc};

use im_ternary_tree::TernaryTreeList;

use crate::Calcit;

use super::CalcitLocal;

/// structure of a function arguments
#[derive(Debug, Clone)]
pub enum CalcitArgLabel {
  /// variable
  Idx(u16),
  /// `?``
  OptionalMark,
  /// `&`
  RestMark,
}

impl Display for CalcitArgLabel {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      CalcitArgLabel::Idx(s) => write!(f, "{}", s),
      CalcitArgLabel::OptionalMark => write!(f, "?"),
      CalcitArgLabel::RestMark => write!(f, "&"),
    }
  }
}

#[derive(Debug, Clone)]
pub enum CalcitFnArgs {
  MarkedArgs(Vec<CalcitArgLabel>),
  Args(Vec<u16>),
}

#[derive(Debug, Clone)]
pub struct CalcitFn {
  pub name: Arc<str>,
  /// where it was defined
  pub def_ns: Arc<str>,
  pub scope: Arc<CalcitScope>,
  pub args: Arc<CalcitFnArgs>,
  pub body: Arc<TernaryTreeList<Calcit>>,
}

/// Macro variant of Calcit data
#[derive(Debug, Clone)]
pub struct CalcitMacro {
  pub name: Arc<str>,
  /// where it was defined
  pub def_ns: Arc<str>,
  pub args: Arc<Vec<CalcitArgLabel>>,
  pub body: Arc<TernaryTreeList<Calcit>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScopePair {
  pub key: u16,
  pub value: Calcit,
}

impl Display for ScopePair {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}: {}", self.key, self.value)
  }
}

/// scope in the semantics of persistent data structure
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalcitScope(TernaryTreeList<ScopePair>);

impl Default for CalcitScope {
  fn default() -> Self {
    Self(TernaryTreeList::Empty)
  }
}

impl CalcitScope {
  /// load value of a symbol from the scope
  pub fn get(&self, key: u16) -> Option<&Calcit> {
    let size = self.0.len();
    for i in 0..size {
      let idx = size - 1 - i;
      match self.0.get(idx) {
        Some(pair) => {
          if pair.key == key {
            return Some(&pair.value);
          }
        }
        None => continue,
      }
    }
    None
  }

  pub fn get_by_name(&self, s: &str) -> Option<&Calcit> {
    let key = CalcitLocal::track_sym(&Arc::from(s));
    self.get(key)
  }

  /// mutable insertiong of variable
  pub fn insert_mut(&mut self, key: u16, value: Calcit) {
    self.0 = self.0.push(ScopePair { key, value })
  }

  pub fn get_names(&self) -> String {
    let mut vars = String::new();
    for (i, k) in self.0.into_iter().enumerate() {
      if i > 0 {
        vars.push(',');
      }
      let name = CalcitLocal::read_name(k.key);
      vars.push_str(&name);
    }
    vars
  }
}
