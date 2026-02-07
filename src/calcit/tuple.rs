use std::sync::Arc;

use crate::Calcit;

use super::{CalcitEnum, CalcitRecord};

#[derive(Debug, Clone)]
pub struct CalcitTuple {
  pub tag: Arc<Calcit>,
  pub extra: Vec<Calcit>,
  /// Trait implementations attached to this tuple (multiple allowed for composition)
  pub impls: Vec<Arc<CalcitRecord>>,
  pub sum_type: Option<Arc<CalcitEnum>>,
}

impl PartialEq for CalcitTuple {
  fn eq(&self, other: &Self) -> bool {
    self.tag == other.tag && self.extra == other.extra
  }
}

impl Eq for CalcitTuple {}
