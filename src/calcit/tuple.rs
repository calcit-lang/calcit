use std::sync::Arc;

use crate::Calcit;

#[derive(Debug, Clone)]
pub struct CalcitTuple {
  pub tag: Arc<Calcit>,
  pub extra: Vec<Calcit>,
  pub class: Arc<Calcit>,
}

impl PartialEq for CalcitTuple {
  fn eq(&self, other: &Self) -> bool {
    self.tag == other.tag && self.extra == other.extra
  }
}

impl Eq for CalcitTuple {}
