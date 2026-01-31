use std::{fmt::Display, sync::Arc};

use im_ternary_tree::TernaryTreeList;

use crate::Calcit;

use super::{CalcitLocal, CalcitTypeAnnotation};

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
      CalcitArgLabel::Idx(s) => write!(f, "{}", CalcitLocal::read_name(*s)),
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

impl CalcitFnArgs {
  /// Counts positional parameters(either indexed locals or symbols) while ignoring markers.
  pub fn param_len(&self) -> usize {
    match self {
      CalcitFnArgs::MarkedArgs(xs) => xs.iter().filter(|label| matches!(label, CalcitArgLabel::Idx(_))).count(),
      CalcitFnArgs::Args(xs) => xs.len(),
    }
  }

  /// Produce a Vec<Arc<...>> aligned with current parameter arity for storing type hints.
  pub fn empty_arg_types(&self) -> Vec<Arc<CalcitTypeAnnotation>> {
    let data = Arc::new(CalcitTypeAnnotation::Dynamic);
    vec![data; self.param_len()]
  }
}

#[derive(Debug, Clone)]
pub struct CalcitFn {
  pub name: Arc<str>,
  /// where it was defined
  pub def_ns: Arc<str>,
  pub scope: Arc<CalcitScope>,
  pub args: Arc<CalcitFnArgs>,
  pub body: Vec<Calcit>,
  /// return type declared by hint-fn
  pub return_type: Arc<CalcitTypeAnnotation>,
  /// argument types declared by assert-type
  pub arg_types: Vec<Arc<CalcitTypeAnnotation>>,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn counts_plain_args() {
    let args = CalcitFnArgs::Args(vec![1, 2, 3]);
    assert_eq!(args.param_len(), 3);
    assert_eq!(args.empty_arg_types().len(), 3);
    assert!(
      args
        .empty_arg_types()
        .iter()
        .all(|item| matches!(**item, CalcitTypeAnnotation::Dynamic))
    );
  }

  #[test]
  fn counts_marked_args_only_on_locals() {
    let args = CalcitFnArgs::MarkedArgs(vec![
      CalcitArgLabel::Idx(1),
      CalcitArgLabel::OptionalMark,
      CalcitArgLabel::Idx(2),
      CalcitArgLabel::RestMark,
    ]);
    assert_eq!(args.param_len(), 2, "only locals should be counted toward arity");
    assert_eq!(args.empty_arg_types().len(), 2);
    assert!(
      args
        .empty_arg_types()
        .iter()
        .all(|item| matches!(**item, CalcitTypeAnnotation::Dynamic))
    );
  }
}

/// Macro variant of Calcit data
#[derive(Debug, Clone)]
pub struct CalcitMacro {
  pub name: Arc<str>,
  /// where it was defined
  pub def_ns: Arc<str>,
  pub args: Arc<Vec<CalcitArgLabel>>,
  pub body: Arc<Vec<Calcit>>,
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
