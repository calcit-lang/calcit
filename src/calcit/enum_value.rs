use std::sync::Arc;

use crate::Calcit;

use super::{CalcitEnumDef, CalcitImpl};

#[derive(Debug, Clone)]
pub struct CalcitEnumValue {
  pub tag: Arc<Calcit>,
  pub extra: Vec<Calcit>,
  pub sum_type: Option<Arc<CalcitEnumDef>>,
}

impl CalcitEnumValue {
  pub fn impls(&self) -> &[Arc<CalcitImpl>] {
    match &self.sum_type {
      Some(s) => s.impls(),
      None => &[],
    }
  }
}

impl PartialEq for CalcitEnumValue {
  fn eq(&self, other: &Self) -> bool {
    self.tag == other.tag && self.extra == other.extra
  }
}

impl Eq for CalcitEnumValue {}
