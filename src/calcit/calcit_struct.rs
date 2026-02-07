use std::hash::{Hash, Hasher};
use std::sync::Arc;

use cirru_edn::EdnTag;

use super::{CalcitRecord, CalcitTypeAnnotation};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalcitStruct {
  pub name: EdnTag,
  pub fields: Arc<Vec<EdnTag>>,
  pub field_types: Arc<Vec<Arc<CalcitTypeAnnotation>>>,
  pub generics: Arc<Vec<Arc<str>>>,
  /// Trait implementations attached to this struct (multiple allowed for composition)
  pub impls: Vec<Arc<CalcitRecord>>,
}

impl CalcitStruct {
  pub fn from_fields(name: EdnTag, fields: Vec<EdnTag>) -> Self {
    let dynamic = Arc::new(CalcitTypeAnnotation::Dynamic);
    let field_types = vec![dynamic; fields.len()];
    let generics = Arc::new(vec![]);
    CalcitStruct {
      name,
      fields: Arc::new(fields),
      field_types: Arc::new(field_types),
      generics,
      impls: vec![],
    }
  }
}

impl Hash for CalcitStruct {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.name.hash(state);
    self.fields.hash(state);
    self.field_types.hash(state);
    self.generics.hash(state);
    for imp in &self.impls {
      imp.name().hash(state);
      imp.fields().hash(state);
    }
  }
}
