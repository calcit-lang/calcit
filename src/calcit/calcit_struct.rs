use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Weak};

use cirru_edn::EdnTag;

use super::{CalcitGenericBound, CalcitImpl, CalcitTypeAnnotation, type_annotation::same_data_schema_annotation};

const STRUCT_FIELD_INDEX_CACHE_THRESHOLD: usize = 32;
const STRUCT_FIELD_INDEX_CACHE_LIMIT: usize = 256;
const STRUCT_FIELD_INDEX_CACHE_MAX_FIELDS: usize = 65_536;
const STRUCT_FIELD_INDEX_CACHE_MAX_NAME_BYTES: usize = 8 * 1_024 * 1_024;

type FieldIndexCacheEntry = (Weak<Vec<EdnTag>>, Arc<HashMap<String, usize>>, usize);

/// Decide whether one more immutable field index fits every thread-local cache
/// budget. Field count bounds map overhead while copied-name bytes bound owned
/// string payloads.
fn field_index_cache_admissible(
  entries: usize,
  fields: usize,
  name_bytes: usize,
  added_fields: usize,
  added_name_bytes: usize,
) -> bool {
  entries < STRUCT_FIELD_INDEX_CACHE_LIMIT
    && fields.saturating_add(added_fields) <= STRUCT_FIELD_INDEX_CACHE_MAX_FIELDS
    && name_bytes.saturating_add(added_name_bytes) <= STRUCT_FIELD_INDEX_CACHE_MAX_NAME_BYTES
}

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
      if let Some((fields, index, _)) = cache.get(&cache_key)
        && fields.upgrade().is_some_and(|fields| Arc::ptr_eq(&fields, &self.fields))
      {
        return index.get(y).copied();
      }

      cache.retain(|_, (fields, _, _)| fields.strong_count() > 0);
      let added_name_bytes = self
        .fields
        .iter()
        .fold(0usize, |total, field| total.saturating_add(field.ref_str().len()));
      let (cached_fields, cached_name_bytes) = cache.values().fold((0usize, 0usize), |(field_total, byte_total), entry| {
        (
          field_total.saturating_add(entry.0.upgrade().map_or(0, |fields| fields.len())),
          byte_total.saturating_add(entry.2),
        )
      });
      if !field_index_cache_admissible(cache.len(), cached_fields, cached_name_bytes, self.fields.len(), added_name_bytes) {
        return self.fields.iter().position(|field| field.ref_str() == y);
      }
      let index = Arc::new(self.fields.iter().enumerate().fold(HashMap::new(), |mut index, (position, field)| {
        // Preserve the historical `position` behavior for malformed
        // duplicate declarations too: the first field wins.
        index.entry(field.ref_str().to_owned()).or_insert(position);
        index
      }));
      let result = index.get(y).copied();
      cache.insert(cache_key, (Arc::downgrade(&self.fields), index, added_name_bytes));
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
        .is_some_and(|(fields, _, _)| fields.upgrade().is_some_and(|fields| Arc::ptr_eq(&fields, &self.fields)))
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

  #[test]
  fn wide_struct_cache_keeps_live_entries_and_rejects_over_budget_admission() {
    STRUCT_FIELD_INDEX_CACHE.with(|cache| cache.borrow_mut().clear());
    let definitions = (0..STRUCT_FIELD_INDEX_CACHE_LIMIT)
      .map(|definition_index| {
        let definition = CalcitStructDef::from_fields(
          EdnTag::new(format!("Wide{definition_index}")),
          (0..STRUCT_FIELD_INDEX_CACHE_THRESHOLD)
            .map(|field_index| EdnTag::new(format!("field-{field_index:04}")))
            .collect(),
        );
        assert_eq!(definition.index_of("field-0031"), Some(31));
        definition
      })
      .collect::<Vec<_>>();
    assert!(definitions[0].has_cached_field_index());

    let overflow = CalcitStructDef::from_fields(
      EdnTag::new("Overflow"),
      (0..STRUCT_FIELD_INDEX_CACHE_THRESHOLD)
        .map(|field_index| EdnTag::new(format!("field-{field_index:04}")))
        .collect(),
    );
    assert_eq!(overflow.index_of("field-0031"), Some(31));
    assert!(!overflow.has_cached_field_index(), "over-budget definitions must use linear lookup");
    assert!(definitions[0].has_cached_field_index(), "existing live indexes must not be evicted");

    assert!(!field_index_cache_admissible(0, STRUCT_FIELD_INDEX_CACHE_MAX_FIELDS, 0, 1, 0));
    assert!(!field_index_cache_admissible(0, 0, STRUCT_FIELD_INDEX_CACHE_MAX_NAME_BYTES, 0, 1));
    STRUCT_FIELD_INDEX_CACHE.with(|cache| cache.borrow_mut().clear());
  }
}
