use std::hash::{Hash, Hasher};
use std::sync::Arc;

use cirru_edn::EdnTag;

use super::{CalcitFn, CalcitTypeAnnotation};

fn defaults_eq(left: &Option<Arc<CalcitFn>>, right: &Option<Arc<CalcitFn>>) -> bool {
  match (left, right) {
    (None, None) => true,
    (Some(_), None) | (None, Some(_)) => false,
    (Some(left_fn), Some(right_fn)) => {
      let left_ref = left_fn.def_ref.as_ref();
      let right_ref = right_fn.def_ref.as_ref();
      match (left_ref, right_ref) {
        (Some(a), Some(b)) => {
          a.def_ns == b.def_ns
            && a.def_name == b.def_name
            && a.coord == b.coord
            && a.is_defn == b.is_defn
            && a.is_macro_gen == b.is_macro_gen
        }
        _ => {
          left_fn.name == right_fn.name
            && left_fn.def_ns == right_fn.def_ns
            && left_fn.args.as_ref().param_len() == right_fn.args.as_ref().param_len()
            && left_fn.body.len() == right_fn.body.len()
        }
      }
    }
  }
}

fn hash_default_impl<H: Hasher>(default_impl: &Option<Arc<CalcitFn>>, state: &mut H) {
  match default_impl {
    None => {
      0u8.hash(state);
    }
    Some(info) => {
      1u8.hash(state);
      if let Some(def_ref) = info.def_ref.as_ref() {
        1u8.hash(state);
        def_ref.def_ns.hash(state);
        def_ref.def_name.hash(state);
        def_ref.coord.hash(state);
        def_ref.is_defn.hash(state);
        def_ref.is_macro_gen.hash(state);
      } else {
        0u8.hash(state);
        info.name.hash(state);
        info.def_ns.hash(state);
        info.args.as_ref().param_len().hash(state);
        info.body.len().hash(state);
      }
    }
  }
}

/// A Trait definition in Calcit
/// Traits define a set of method signatures that types can implement
/// Similar to Rust traits or Haskell type classes
#[derive(Debug, Clone)]
pub struct CalcitTrait {
  /// Name of the trait
  pub name: EdnTag,
  /// Method names defined by this trait
  pub methods: Arc<Vec<EdnTag>>,
  /// Default implementations for methods (as functions)
  /// If a method has no default, it's None
  pub defaults: Arc<Vec<Option<Arc<CalcitFn>>>>,
  /// Type annotations for method signatures
  pub method_types: Arc<Vec<Arc<CalcitTypeAnnotation>>>,
  /// Required traits (trait inheritance/composition)
  pub requires: Arc<Vec<Arc<CalcitTrait>>>,
}

// Manual implementation since CalcitFn doesn't implement Eq/Hash.
// For defaults, use stable function identity (prefer def_ref, fallback to fn metadata).
impl PartialEq for CalcitTrait {
  fn eq(&self, other: &Self) -> bool {
    self.name == other.name
      && self.methods == other.methods
      && self.method_types == other.method_types
      && self.requires == other.requires
      && self.defaults.len() == other.defaults.len()
      && self
        .defaults
        .iter()
        .zip(other.defaults.iter())
        .all(|(left, right)| defaults_eq(left, right))
  }
}

impl Eq for CalcitTrait {}

impl CalcitTrait {
  /// Create a new trait with the given name and methods
  pub fn new(name: EdnTag, methods: Vec<EdnTag>, method_types: Vec<Arc<CalcitTypeAnnotation>>) -> Self {
    let defaults = vec![None; methods.len()];
    assert!(
      methods.len() == method_types.len(),
      "CalcitTrait::new expects method_types to match methods length"
    );
    CalcitTrait {
      name,
      methods: Arc::new(methods),
      defaults: Arc::new(defaults),
      method_types: Arc::new(method_types),
      requires: Arc::new(vec![]),
    }
  }

  /// Get the method names
  pub fn method_names(&self) -> &[EdnTag] {
    &self.methods
  }

  /// Check if this trait has a method with the given name
  pub fn has_method(&self, name: &str) -> bool {
    self.methods.iter().any(|m| m.ref_str() == name)
  }

  /// Get the index of a method by name
  pub fn method_index(&self, name: &str) -> Option<usize> {
    self.methods.iter().position(|m| m.ref_str() == name)
  }

  /// Get the default implementation for a method
  pub fn get_default(&self, name: &str) -> Option<&Arc<CalcitFn>> {
    self
      .method_index(name)
      .and_then(|idx| self.defaults.get(idx).and_then(|d| d.as_ref()))
  }
}

impl Hash for CalcitTrait {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.name.hash(state);
    self.methods.hash(state);
    self.method_types.hash(state);
    self.requires.hash(state);
    self.defaults.len().hash(state);
    for default_impl in self.defaults.iter() {
      hash_default_impl(default_impl, state);
    }
  }
}

impl std::fmt::Display for CalcitTrait {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "(trait {} ", self.name)?;
    for (i, method) in self.methods.iter().enumerate() {
      if i > 0 {
        write!(f, " ")?;
      }
      write!(f, ":{method}")?;
    }
    write!(f, ")")
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::hash_map::DefaultHasher;

  use cirru_edn::EdnTag;

  use crate::calcit::{CalcitFnArgs, CalcitFnDefRef, CalcitFnUsageMeta, CalcitScope};

  fn build_default_fn(name: &str, def_ref: Option<CalcitFnDefRef>, body_len: usize) -> Arc<CalcitFn> {
    Arc::new(CalcitFn {
      name: Arc::from(name),
      def_ns: Arc::from("unit.test"),
      def_ref,
      usage: CalcitFnUsageMeta::default(),
      scope: Arc::new(CalcitScope::default()),
      args: Arc::new(CalcitFnArgs::Args(vec![1])),
      body: vec![crate::Calcit::Nil; body_len],
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      return_type: crate::calcit::DYNAMIC_TYPE.clone(),
      arg_types: vec![crate::calcit::DYNAMIC_TYPE.clone()],
    })
  }

  fn build_trait_with_default(default_impl: Option<Arc<CalcitFn>>) -> CalcitTrait {
    CalcitTrait {
      name: EdnTag::new("TraitX"),
      methods: Arc::new(vec![EdnTag::new("foo")]),
      defaults: Arc::new(vec![default_impl]),
      method_types: Arc::new(vec![crate::calcit::DYNAMIC_TYPE.clone()]),
      requires: Arc::new(vec![]),
    }
  }

  fn hash_trait(value: &CalcitTrait) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
  }

  #[test]
  fn defaults_with_same_def_ref_are_equal_and_hash_equal() {
    let def_ref = CalcitFnDefRef {
      def_ns: Arc::from("unit.test"),
      def_name: Arc::from("foo-default"),
      coord: Some((1, 2)),
      is_defn: true,
      is_macro_gen: false,
    };

    let left = build_trait_with_default(Some(build_default_fn("foo_impl_a", Some(def_ref.clone()), 1)));
    let right = build_trait_with_default(Some(build_default_fn("foo_impl_b", Some(def_ref), 3)));

    assert_eq!(left, right);
    assert_eq!(hash_trait(&left), hash_trait(&right));
  }

  #[test]
  fn defaults_without_def_ref_use_fn_metadata_identity() {
    let left = build_trait_with_default(Some(build_default_fn("foo", None, 2)));
    let right = build_trait_with_default(Some(build_default_fn("foo", None, 2)));
    let different = build_trait_with_default(Some(build_default_fn("foo", None, 4)));

    assert_eq!(left, right);
    assert_eq!(hash_trait(&left), hash_trait(&right));
    assert_ne!(left, different);
  }
}
