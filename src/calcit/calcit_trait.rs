use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use cirru_edn::EdnTag;

use super::{CalcitFn, CalcitTypeAnnotation};

static NEXT_TRAIT_RUNTIME_ID: AtomicU64 = AtomicU64::new(1);

/// Source member shape retained for traits. Ordinary traits use method
/// members; external-object traits may additionally expose typed fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CalcitTraitMemberKind {
  Method,
  Field,
}

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
  /// Nominal identity for an evaluated trait definition. Source/schema
  /// placeholders keep this empty and use `definition_ref` when available.
  pub runtime_id: Option<u64>,
  /// Stable source identity for a trait resolved before runtime evaluation.
  /// This keeps two source traits with the same short name nominally distinct.
  pub definition_ref: Option<Arc<str>>,
  /// Name of the trait
  pub name: EdnTag,
  /// Method names defined by this trait
  pub methods: Arc<Vec<EdnTag>>,
  /// Default implementations for methods (as functions)
  /// If a method has no default, it's None
  pub defaults: Arc<Vec<Option<Arc<CalcitFn>>>>,
  /// Type annotations for method signatures
  pub method_types: Arc<Vec<Arc<CalcitTypeAnnotation>>>,
  /// Source shape for each member, parallel to `methods` and `method_types`.
  pub member_kinds: Arc<Vec<CalcitTraitMemberKind>>,
  /// Required traits (trait inheritance/composition)
  pub requires: Arc<Vec<Arc<CalcitTrait>>>,
}

// Manual implementation since CalcitFn doesn't implement Eq/Hash.
// For defaults, use stable function identity (prefer def_ref, fallback to fn metadata).
impl PartialEq for CalcitTrait {
  fn eq(&self, other: &Self) -> bool {
    match (self.runtime_id, other.runtime_id) {
      (Some(left), Some(right)) => return left == right,
      (Some(_), None) | (None, Some(_)) => return false,
      (None, None) => {}
    }
    self.structural_eq(other)
  }
}

impl Eq for CalcitTrait {}

impl CalcitTrait {
  fn structural_eq(&self, other: &Self) -> bool {
    self.definition_ref == other.definition_ref
      && self.name == other.name
      && self.methods == other.methods
      && self.method_types == other.method_types
      && self.member_kinds == other.member_kinds
      && self.requires == other.requires
      && self.defaults.len() == other.defaults.len()
      && self
        .defaults
        .iter()
        .zip(other.defaults.iter())
        .all(|(left, right)| defaults_eq(left, right))
  }

  /// Create a new trait with the given name and methods
  pub fn new(name: EdnTag, methods: Vec<EdnTag>, method_types: Vec<Arc<CalcitTypeAnnotation>>) -> Self {
    Self::new_with_member_kinds(name, methods, method_types, None)
  }

  pub fn new_with_member_kinds(
    name: EdnTag,
    methods: Vec<EdnTag>,
    method_types: Vec<Arc<CalcitTypeAnnotation>>,
    member_kinds: Option<Vec<CalcitTraitMemberKind>>,
  ) -> Self {
    let defaults = vec![None; methods.len()];
    assert!(
      methods.len() == method_types.len(),
      "CalcitTrait::new expects method_types to match methods length"
    );
    let member_kinds = member_kinds.unwrap_or_else(|| vec![CalcitTraitMemberKind::Method; methods.len()]);
    assert!(
      methods.len() == member_kinds.len(),
      "CalcitTrait::new expects member_kinds to match methods length"
    );
    CalcitTrait {
      runtime_id: None,
      definition_ref: None,
      name,
      methods: Arc::new(methods),
      defaults: Arc::new(defaults),
      method_types: Arc::new(method_types),
      member_kinds: Arc::new(member_kinds),
      requires: Arc::new(vec![]),
    }
  }

  /// Create an evaluated trait with nominal runtime identity. Cloning the
  /// result preserves identity; evaluating the definition again creates a new
  /// one, so stale impls cannot accidentally satisfy a reloaded trait.
  pub fn new_runtime(name: EdnTag, methods: Vec<EdnTag>, method_types: Vec<Arc<CalcitTypeAnnotation>>) -> Self {
    let mut trait_def = Self::new(name, methods, method_types);
    trait_def.runtime_id = Some(NEXT_TRAIT_RUNTIME_ID.fetch_add(1, Ordering::Relaxed));
    trait_def
  }

  /// Attach the namespace-qualified definition identity used while the source
  /// trait is available but its runtime value has not been evaluated yet.
  pub fn with_definition_ref(mut self, ns: &str, def: &str) -> Self {
    self.definition_ref = Some(Arc::from(format!("{ns}/{def}")));
    self
  }

  /// Build an unresolved trait reference while retaining a qualified source
  /// path when one is present in the schema.
  pub fn new_reference(name: &str) -> Self {
    let normalized = name.trim_start_matches('\'');
    match normalized.rsplit_once('/') {
      Some((ns, def)) => Self::new(EdnTag::new(def), vec![], vec![]).with_definition_ref(ns, def),
      None => Self::new(EdnTag::new(normalized), vec![], vec![]),
    }
  }

  /// Match a trait reference stored in schema/static metadata. Evaluated
  /// references use runtime identity, source references use their qualified
  /// definition, and legacy bare placeholders fall back to shape or name.
  pub fn matches_reference(&self, expected: &Self) -> bool {
    if expected.runtime_id.is_none() && expected.definition_ref.is_none() && expected.methods.is_empty() {
      return self.name == expected.name;
    }
    if expected.runtime_id.is_some() {
      return self == expected;
    }
    if self.definition_ref.is_some() || expected.definition_ref.is_some() {
      return self.definition_ref == expected.definition_ref;
    }
    if self.runtime_id.is_some() {
      return false;
    }
    self.structural_eq(expected) || (expected.methods.is_empty() && self.name == expected.name)
  }

  /// Get the method names
  pub fn method_names(&self) -> &[EdnTag] {
    &self.methods
  }

  /// Check if this trait has a method with the given name
  pub fn has_method(&self, name: &str) -> bool {
    self
      .methods
      .iter()
      .zip(self.member_kinds.iter())
      .any(|(m, kind)| *kind == CalcitTraitMemberKind::Method && m.ref_str() == name)
  }

  /// Get the index of a method by name
  pub fn method_index(&self, name: &str) -> Option<usize> {
    self
      .methods
      .iter()
      .zip(self.member_kinds.iter())
      .position(|(m, kind)| *kind == CalcitTraitMemberKind::Method && m.ref_str() == name)
  }

  pub fn field_index(&self, name: &str) -> Option<usize> {
    self
      .methods
      .iter()
      .zip(self.member_kinds.iter())
      .position(|(m, kind)| *kind == CalcitTraitMemberKind::Field && m.ref_str() == name)
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
    self.runtime_id.hash(state);
    if self.runtime_id.is_some() {
      return;
    }
    self.definition_ref.hash(state);
    self.name.hash(state);
    self.methods.hash(state);
    self.method_types.hash(state);
    self.member_kinds.hash(state);
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
      call_shape: crate::calcit::CalcitFnCallShape::fixed(1),
      body: vec![crate::Calcit::Nil; body_len],
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      return_type: crate::calcit::DYNAMIC_TYPE.clone(),
      arg_types: vec![crate::calcit::DYNAMIC_TYPE.clone()],
      rest_type: None,
    })
  }

  fn build_trait_with_default(default_impl: Option<Arc<CalcitFn>>) -> CalcitTrait {
    CalcitTrait {
      runtime_id: None,
      definition_ref: None,
      name: EdnTag::new("TraitX"),
      methods: Arc::new(vec![EdnTag::new("foo")]),
      defaults: Arc::new(vec![default_impl]),
      method_types: Arc::new(vec![crate::calcit::DYNAMIC_TYPE.clone()]),
      member_kinds: Arc::new(vec![CalcitTraitMemberKind::Method]),
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
  fn source_definition_refs_are_nominal() {
    let core = CalcitTrait::new(EdnTag::new("Show"), vec![], vec![]).with_definition_ref("calcit.core", "Show");
    let user = CalcitTrait::new(EdnTag::new("Show"), vec![], vec![]).with_definition_ref("app.main", "Show");
    let same_core = CalcitTrait::new_reference("calcit.core/Show");
    let runtime_core = CalcitTrait::new_runtime(EdnTag::new("Show"), vec![], vec![]).with_definition_ref("calcit.core", "Show");
    let legacy_placeholder = CalcitTrait::new_reference("Show");

    assert_eq!(core, same_core);
    assert_eq!(hash_trait(&core), hash_trait(&same_core));
    assert_ne!(core, user);
    assert!(!core.matches_reference(&user));
    assert!(runtime_core.matches_reference(&same_core));
    assert!(core.matches_reference(&legacy_placeholder));
    assert!(user.matches_reference(&legacy_placeholder));
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

  #[test]
  fn evaluated_traits_with_same_shape_keep_nominal_identity() {
    let left = CalcitTrait::new_runtime(EdnTag::new("Visible"), vec![], vec![]);
    let right = CalcitTrait::new_runtime(EdnTag::new("Visible"), vec![], vec![]);

    assert_ne!(left, right);
    assert_eq!(left, left.clone());
  }

  #[test]
  fn schema_trait_placeholders_remain_structural() {
    let left = CalcitTrait::new(EdnTag::new("Visible"), vec![], vec![]);
    let right = CalcitTrait::new(EdnTag::new("Visible"), vec![], vec![]);

    assert_eq!(left, right);
    assert_eq!(hash_trait(&left), hash_trait(&right));
  }
}
