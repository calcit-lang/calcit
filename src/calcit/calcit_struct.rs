use std::hash::{Hash, Hasher};
use std::sync::Arc;

use cirru_edn::EdnTag;

use super::{CalcitGenericBound, CalcitImpl, CalcitTypeAnnotation, type_annotation::same_data_schema_annotation};

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
            .all(|(actual, expected)| same_data_schema_annotation(actual, expected))
          && self.generics == other.generics
          && self.where_bounds == other.where_bounds
      }
      _ => false,
    }
  }

  /// Find a field without changing declaration order. Some embedders create
  /// unsorted definitions directly, so this cannot assume parser sorting.
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

#[cfg(test)]
mod tests {
  use super::*;

  fn nested_struct(namespace: &str) -> Arc<CalcitTypeAnnotation> {
    Arc::new(CalcitTypeAnnotation::Struct(
      Arc::new(CalcitStructDef {
        definition_ref: Some(Arc::from(format!("{namespace}/Item"))),
        name: EdnTag::new("Item"),
        fields: Arc::new(vec![]),
        field_types: Arc::new(vec![]),
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        impls: vec![],
      }),
      Arc::new(vec![]),
    ))
  }

  fn wrapper(field_type: Arc<CalcitTypeAnnotation>) -> CalcitStructDef {
    CalcitStructDef {
      definition_ref: Some(Arc::from("app/Wrapper")),
      name: EdnTag::new("Wrapper"),
      fields: Arc::new(vec![EdnTag::new("item")]),
      field_types: Arc::new(vec![field_type]),
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    }
  }

  #[test]
  fn nested_nominal_field_identity_keeps_qualified_definition() {
    let left = wrapper(nested_struct("alpha.models"));
    let right = wrapper(nested_struct("beta.models"));

    assert!(!left.same_nominal_definition(&right));
  }

  #[test]
  fn wide_struct_lookup_preserves_unsorted_declaration_order() {
    for width in [32usize, 256, 1_024] {
      let fields = (0..width)
        .rev()
        .map(|idx| EdnTag::new(format!("field-{idx:04}")))
        .collect::<Vec<_>>();
      let target = fields.last().expect("wide struct field").clone();
      let definition = CalcitStructDef::from_fields(EdnTag::new("Wide"), fields);
      let started = std::time::Instant::now();
      for _ in 0..10_000 {
        assert_eq!(definition.index_of(target.ref_str()), Some(width - 1));
      }
      eprintln!(
        "struct-index width={width} lookups=10000 elapsed_us={}",
        started.elapsed().as_micros()
      );
    }
  }
}
