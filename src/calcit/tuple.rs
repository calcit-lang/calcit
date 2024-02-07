use std::sync::Arc;

use crate::Calcit;

#[derive(Debug, Clone, PartialEq)]
pub struct CalcitTuple {
  pub tag: Arc<Calcit>,
  pub extra: Vec<Calcit>,
  pub class: Arc<Calcit>,
}
