use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Weak};

use cirru_edn::EdnTag;

use super::{CalcitGenericBound, CalcitImpl, CalcitTypeAnnotation, type_annotation::same_data_schema_annotation};

const STRUCT_FIELD_INDEX_CACHE_THRESHOLD: usize = 32;
const STRUCT_FIELD_INDEX_CACHE_LIMIT: usize = 256;

type FieldIndexCacheEntry = (Weak<Vec<EdnTag>>, Arc<HashMap<String, usize>>);

thread_local! {
  /// Wide struct definitions are immutable. Cache by the live `fields` Arc
  /// allocation, not by name or declaration contents, so entry changes and hot
  /// reloads cannot reuse an index from a stale nominal definition.
  static STRUCT_FIELD_INDEX_CACHE: RefCell<HashMap<usize, FieldIndexCacheEntry>> = RefCell::new(HashMap::new());
}

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

  /// Return the declaration-order field position. Small structs stay on the
  /// allocation-free linear path; wide structs use a read-only pointer/Weak
  /// cache without sorting or changing field layout.
  pub fn index_of(&self, y: &str) -> Option<usize> {
    if self.fields.len() < STRUCT_FIELD_INDEX_CACHE_THRESHOLD {
      return self.fields.iter().position(|field| field.ref_str() == y);
    }

    let cache_key = Arc::as_ptr(&self.fields) as usize;
    STRUCT_FIELD_INDEX_CACHE.with(|cache| {
      let mut cache = cache.borrow_mut();
      if let Some((fields, index)) = cache.get(&cache_key)
        && fields.upgrade().is_some_and(|fields| Arc::ptr_eq(&fields, &self.fields))
      {
        return index.get(y).copied();
      }

      if cache.len() >= STRUCT_FIELD_INDEX_CACHE_LIMIT {
        cache.retain(|_, (fields, _)| fields.strong_count() > 0);
        if cache.len() >= STRUCT_FIELD_INDEX_CACHE_LIMIT {
          cache.clear();
        }
      }
      let index = Arc::new(self.fields.iter().enumerate().fold(HashMap::new(), |mut index, (position, field)| {
        // Preserve the historical `position` behavior for malformed
        // duplicate declarations too: the first field wins.
        index.entry(field.ref_str().to_owned()).or_insert(position);
        index
      }));
      let result = index.get(y).copied();
      cache.insert(cache_key, (Arc::downgrade(&self.fields), index));
      result
    })
  }

  #[cfg(test)]
  fn has_cached_field_index(&self) -> bool {
    let cache_key = Arc::as_ptr(&self.fields) as usize;
    STRUCT_FIELD_INDEX_CACHE.with(|cache| {
      cache
        .borrow()
        .get(&cache_key)
        .is_some_and(|(fields, _)| fields.upgrade().is_some_and(|fields| Arc::ptr_eq(&fields, &self.fields)))
    })
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
  fn wide_struct_field_index_is_cached_without_changing_declaration_order() {
    for width in [32, 256, 1_024] {
      let fields = (0..width)
        .map(|index| EdnTag::new(format!("field-{index:04}")))
        .rev()
        .collect::<Vec<_>>();
      let definition = CalcitStructDef::from_fields(EdnTag::new("Wide"), fields.clone());
      assert!(!definition.has_cached_field_index());
      let cold_started = std::time::Instant::now();
      assert_eq!(definition.index_of("field-0000"), Some(width - 1));
      let cold_elapsed = cold_started.elapsed();
      assert!(definition.has_cached_field_index());
      let warm_started = std::time::Instant::now();
      assert_eq!(definition.index_of(&format!("field-{:04}", width - 1)), Some(0));
      let warm_elapsed = warm_started.elapsed();
      assert_eq!(definition.fields.as_ref(), &fields, "cache must preserve declaration layout");
      let reloaded = CalcitStructDef::from_fields(EdnTag::new("Wide"), fields.clone());
      assert!(
        !reloaded.has_cached_field_index(),
        "a new entry allocation must not inherit the old cache"
      );
      eprintln!(
        "struct-index width={width} cold-us={} warm-us={} cache=yes",
        cold_elapsed.as_micros(),
        warm_elapsed.as_micros()
      );
    }

    let mut duplicate_fields = (0..32).map(|index| EdnTag::new(format!("field-{index:04}"))).collect::<Vec<_>>();
    duplicate_fields[31] = duplicate_fields[0].clone();
    let malformed = CalcitStructDef::from_fields(EdnTag::new("DuplicateWide"), duplicate_fields);
    assert_eq!(
      malformed.index_of("field-0000"),
      Some(0),
      "wide cache must preserve first-match semantics"
    );
  }
}
