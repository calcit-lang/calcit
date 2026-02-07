use std::sync::Arc;

use cirru_edn::EdnTag;

use super::CalcitTypeAnnotation;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CalcitStruct {
  pub name: EdnTag,
  pub fields: Arc<Vec<EdnTag>>,
  pub field_types: Arc<Vec<Arc<CalcitTypeAnnotation>>>,
  pub generics: Arc<Vec<Arc<str>>>,
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
    }
  }
}
