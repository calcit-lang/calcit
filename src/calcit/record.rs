use std::sync::Arc;

use cirru_edn::EdnTag;

use crate::Calcit;

#[derive(Debug, Clone)]
pub struct CalcitRecord {
  pub name: EdnTag,
  pub fields: Arc<Vec<EdnTag>>,
  pub values: Arc<Vec<Calcit>>,
  pub class: Arc<Calcit>,
}
