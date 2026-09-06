use std::hash::{Hash, Hasher};
use std::sync::Arc;

use cirru_edn::EdnTag;

use super::{CalcitGenericBound, CalcitImpl, CalcitTypeAnnotation};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalcitStructDef {
  /// Qualified source declaration (`namespace/definition`) when known.
  /// Runtime values and static annotations retain this identity across
  /// `impl-traits` decoration.
  pub definition_ref: Option<Arc<str>>,
  pub name: EdnTag,
  pub fields: Arc<Vec<EdnTag>>,
  pub field_types: Arc<Vec<Arc<CalcitTypeAnnotation>>>,
  pub generics: Arc<Vec<Arc<str>>>,
  pub where_bounds: Arc<Vec<CalcitGenericBound>>,
  /// Trait implementations attached to this struct (multiple allowed for composition)
  pub impls: Vec<Arc<CalcitImpl>>,
}

impl CalcitStructDef {
  pub fn from_fields(name: EdnTag, fields: Vec<EdnTag>) -> Self {
    let field_types = vec![super::DYNAMIC_TYPE.clone(); fields.len()];
    let generics = Arc::new(vec![]);
    CalcitStructDef {
      definition_ref: None,
      name,
      fields: Arc::new(fields),
      field_types: Arc::new(field_types),
      generics,
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    }
  }

  pub fn with_definition_ref(mut self, ns: &str, def: &str) -> Self {
    self.definition_ref = Some(Arc::from(format!("{ns}/{def}")));
    self
  }

  /// Data-schema identity deliberately excludes attached impls: decorating a
  /// declaration changes method evidence, not the nominal data type.
  pub fn same_nominal_definition(&self, other: &Self) -> bool {
    match (&self.definition_ref, &other.definition_ref) {
      (Some(actual), Some(expected)) => {
        actual == expected
          && self.name == other.name
          && self.fields == other.fields
          && self.field_types.len() == other.field_types.len()
          && self
            .field_types
            .iter()
            .zip(other.field_types.iter())
            .all(|(actual, expected)| actual.to_type_edn() == expected.to_type_edn())
          && self.generics == other.generics
          && self.where_bounds == other.where_bounds
      }
      _ => false,
    }
  }

  /// Binary search for the position of a field name (fields must be sorted)
  pub fn index_of(&self, y: &str) -> Option<usize> {
    self.fields.iter().position(|f| f.ref_str() == y)
  }
}

impl Hash for CalcitStructDef {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.definition_ref.hash(state);
    self.name.hash(state);
    self.fields.hash(state);
    self.field_types.hash(state);
    self.generics.hash(state);
    self.where_bounds.hash(state);
    for imp in &self.impls {
      imp.name().hash(state);
      imp.fields().hash(state);
    }
  }
}
