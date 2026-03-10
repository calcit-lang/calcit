use std::sync::Arc;

use crate::Calcit;

use super::{CalcitEnum, CalcitImpl};

#[derive(Debug, Clone)]
pub struct CalcitTuple {
  pub tag: Arc<Calcit>,
  pub extra: Vec<Calcit>,
  pub sum_type: Option<Arc<CalcitEnum>>,
}

impl CalcitTuple {
  pub fn impls(&self) -> &[Arc<CalcitImpl>] {
    match &self.sum_type {
      Some(s) => s.impls(),
      None => &[],
    }
  }
}

impl PartialEq for CalcitTuple {
  fn eq(&self, other: &Self) -> bool {
    self.tag == other.tag && self.extra == other.extra
  }
}

impl Eq for CalcitTuple {}
