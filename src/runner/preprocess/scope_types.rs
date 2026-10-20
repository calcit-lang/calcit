//! Lexical types and temporary predicate refinements share one scope lifetime.
//! Effects revoke refinements, not the independently established binding types.

use std::borrow::Borrow;
use std::collections::{HashMap, hash_map};
use std::hash::Hash;
use std::ops::Deref;
use std::sync::Arc;

use crate::calcit::CalcitTypeAnnotation;

type Annotation = Arc<CalcitTypeAnnotation>;

#[doc(hidden)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScopeTypes {
  values: HashMap<Arc<str>, Annotation>,
  parameter_shadowed_types: HashMap<Arc<str>, Annotation>,
  refinements: HashMap<Arc<str>, Option<Annotation>>,
  generics: HashMap<Arc<str>, Arc<str>>,
  generic_capabilities: Option<Arc<HashMap<Arc<str>, Annotation>>>,
}

impl ScopeTypes {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn with_capacity(capacity: usize) -> Self {
    Self {
      values: HashMap::with_capacity(capacity),
      parameter_shadowed_types: HashMap::new(),
      refinements: HashMap::new(),
      generics: HashMap::new(),
      generic_capabilities: None,
    }
  }

  pub fn insert(&mut self, name: Arc<str>, annotation: Annotation) -> Option<Annotation> {
    self.parameter_shadowed_types.remove(&name);
    self.refinements.remove(&name);
    self.values.insert(name, annotation)
  }

  /// Value parameters do not redeclare a captured nominal type alias.
  pub(super) fn shadow_parameter(&mut self, name: &Arc<str>) {
    if let Some(annotation) = self.type_reference(name).filter(|annotation| {
      matches!(
        annotation.as_ref(),
        CalcitTypeAnnotation::StructDef(_) | CalcitTypeAnnotation::EnumDef(_)
      )
    }) {
      self.parameter_shadowed_types.insert(name.clone(), annotation.clone());
    }
    self.remove(name);
  }

  pub(super) fn bind_parameter(&mut self, name: Arc<str>, annotation: Annotation) {
    self.refinements.remove(&name);
    self.values.insert(name, annotation);
  }

  pub(super) fn type_reference(&self, name: &str) -> Option<&Annotation> {
    self.parameter_shadowed_types.get(name).or_else(|| self.values.get(name))
  }

  /// Declarations, not values carrying a type variable, own lexical type names.
  pub(super) fn bind_generics(&mut self, names: &[Arc<str>]) {
    for name in names {
      let source = crate::calcit::type_annotation::source_type_variable(name);
      self.generics.insert(Arc::from(source), name.clone());
    }
  }

  pub(super) fn generic_names(&self) -> Vec<Arc<str>> {
    let mut names = self.generics.values().cloned().collect::<Vec<_>>();
    names.sort();
    names
  }

  pub(super) fn clear_generics(&mut self) {
    self.generics.clear();
    self.generic_capabilities = None;
  }

  /// Capability evidence belongs to the binder, not to values carrying it.
  pub(super) fn bind_generic_capability(&mut self, name: Arc<str>, capability: Annotation) {
    Arc::make_mut(self.generic_capabilities.get_or_insert_with(|| Arc::new(HashMap::new()))).insert(name, capability);
  }

  pub(super) fn bind_function_generics(&mut self, signature: &crate::calcit::CalcitFnTypeAnnotation, namespace: &str) {
    self.bind_generics(&signature.generics);
    for bound in signature.where_bounds.iter() {
      // A closure cannot manufacture a new capability for a captured binder.
      if signature.generics.contains(&bound.name)
        && let Some(capability) = super::resolve_where_bound_type_for_body(bound, namespace)
      {
        self.bind_generic_capability(bound.name.clone(), capability);
      }
    }
  }

  pub(super) fn generic_capability(&self, annotation: &CalcitTypeAnnotation) -> Option<&Annotation> {
    match annotation {
      CalcitTypeAnnotation::TypeVar(name) => self.generic_capabilities.as_deref().and_then(|items| items.get(name)),
      _ => None,
    }
  }

  pub(super) fn proves_trait_bound(&self, actual: &CalcitTypeAnnotation, required: &CalcitTypeAnnotation) -> bool {
    crate::calcit::type_annotation::proves_trait_bound(actual, required, self.generic_capabilities.as_deref())
  }

  pub(super) fn call_type_proof(
    &self,
    generics: &[Arc<str>],
    expected: &[Annotation],
    actual: &[Annotation],
  ) -> crate::calcit::type_annotation::CallTypeProof {
    crate::calcit::type_annotation::CallTypeProof::new(generics, expected, actual)
      .with_caller_capabilities(self.generic_capabilities.clone())
  }

  /// An annotation of an existing binding must retain its evidence lifetime.
  pub(super) fn annotate(&mut self, name: Arc<str>, annotation: Annotation) {
    self.values.insert(name, annotation);
  }

  pub(super) fn refine(&mut self, name: Arc<str>, annotation: Annotation) {
    if self
      .values
      .get(&name)
      .is_some_and(|current| current == &annotation || current.is_proven_for(&annotation))
    {
      return;
    }
    self
      .refinements
      .entry(name.clone())
      .or_insert_with(|| self.values.get(&name).cloned());
    self.values.insert(name, annotation);
  }

  pub(super) fn invalidate_refinements(&mut self) {
    for (name, original) in self.refinements.drain() {
      match original {
        Some(annotation) => {
          self.values.insert(name, annotation);
        }
        None => {
          self.values.remove(&name);
        }
      }
    }
  }

  pub(super) fn has_refinements(&self) -> bool {
    !self.refinements.is_empty()
  }

  /// Keep independent binding contracts available beneath temporary reads.
  pub(super) fn binding_type(&self, name: &str) -> Option<&Annotation> {
    self.refinements.get(name).map_or_else(|| self.values.get(name), Option::as_ref)
  }

  pub fn remove<Q: ?Sized + Hash + Eq>(&mut self, name: &Q) -> Option<Annotation>
  where
    Arc<str>: Borrow<Q>,
  {
    self.refinements.remove(name);
    self.values.remove(name)
  }

  pub fn clear(&mut self) {
    self.values.clear();
    self.parameter_shadowed_types.clear();
    self.refinements.clear();
    self.generics.clear();
    self.generic_capabilities = None;
  }

  pub fn retain(&mut self, mut predicate: impl FnMut(&Arc<str>, &Annotation) -> bool) {
    self.values.retain(|name, annotation| predicate(name, annotation));
    self.refinements.retain(|name, _| self.values.contains_key(name));
  }
}

impl Deref for ScopeTypes {
  type Target = HashMap<Arc<str>, Annotation>;

  fn deref(&self) -> &Self::Target {
    &self.values
  }
}

impl From<HashMap<Arc<str>, Annotation>> for ScopeTypes {
  fn from(values: HashMap<Arc<str>, Annotation>) -> Self {
    Self {
      values,
      parameter_shadowed_types: HashMap::new(),
      refinements: HashMap::new(),
      generics: HashMap::new(),
      generic_capabilities: None,
    }
  }
}

impl<const N: usize> From<[(Arc<str>, Annotation); N]> for ScopeTypes {
  fn from(values: [(Arc<str>, Annotation); N]) -> Self {
    values.into_iter().collect()
  }
}

impl FromIterator<(Arc<str>, Annotation)> for ScopeTypes {
  fn from_iter<T: IntoIterator<Item = (Arc<str>, Annotation)>>(items: T) -> Self {
    HashMap::from_iter(items).into()
  }
}

impl Extend<(Arc<str>, Annotation)> for ScopeTypes {
  fn extend<T: IntoIterator<Item = (Arc<str>, Annotation)>>(&mut self, items: T) {
    for (name, annotation) in items {
      self.insert(name, annotation);
    }
  }
}

impl IntoIterator for ScopeTypes {
  type Item = (Arc<str>, Annotation);
  type IntoIter = hash_map::IntoIter<Arc<str>, Annotation>;

  fn into_iter(self) -> Self::IntoIter {
    self.values.into_iter()
  }
}

impl<'a> IntoIterator for &'a ScopeTypes {
  type Item = (&'a Arc<str>, &'a Annotation);
  type IntoIter = hash_map::Iter<'a, Arc<str>, Annotation>;

  fn into_iter(self) -> Self::IntoIter {
    self.values.iter()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn generic_declarations_shadow_without_being_inferred_from_values() {
    let outer_name: Arc<str> = Arc::from("T");
    let inner_name: Arc<str> = Arc::from("#lexical:1\0T");
    let mut outer = ScopeTypes::new();
    outer.bind_generics(std::slice::from_ref(&outer_name));
    let capability = Arc::new(CalcitTypeAnnotation::TraitSet(Arc::new(vec![])));
    outer.bind_generic_capability(outer_name.clone(), capability.clone());
    outer.insert(Arc::from("outer"), Arc::new(CalcitTypeAnnotation::TypeVar(outer_name.clone())));
    let mut inner = outer.clone();
    inner.bind_generics(std::slice::from_ref(&inner_name));
    inner.insert(Arc::from("inner"), Arc::new(CalcitTypeAnnotation::TypeVar(inner_name.clone())));
    inner.insert(Arc::from("transported"), Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("U"))));
    assert_eq!(outer.generic_names(), vec![outer_name.clone()]);
    assert_eq!(
      inner.generic_capability(&CalcitTypeAnnotation::TypeVar(outer_name.clone())),
      Some(&capability)
    );
    assert!(
      inner
        .generic_capability(&CalcitTypeAnnotation::TypeVar(inner_name.clone()))
        .is_none()
    );
    inner.bind_generic_capability(inner_name.clone(), capability.clone());
    assert!(
      outer
        .generic_capability(&CalcitTypeAnnotation::TypeVar(inner_name.clone()))
        .is_none()
    );
    assert_eq!(inner.generic_names(), vec![inner_name.clone()]);
    inner.refine(Arc::from("inner"), Arc::new(CalcitTypeAnnotation::Number));
    inner.invalidate_refinements();
    assert_eq!(inner.generic_names(), vec![inner_name.clone()]);
    assert_eq!(
      inner.generic_capability(&CalcitTypeAnnotation::TypeVar(inner_name.clone())),
      Some(&capability)
    );
    inner.clear();
    assert!(inner.generic_names().is_empty());
    assert!(inner.generic_capability(&CalcitTypeAnnotation::TypeVar(inner_name)).is_none());
  }

  #[test]
  fn independent_contract_survives_reads_but_not_shadowing() {
    let mut scope = ScopeTypes::new();
    let variable = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")));
    let list = Arc::new(CalcitTypeAnnotation::List(crate::calcit::DYNAMIC_TYPE.clone()));
    scope.insert(Arc::from("value"), variable.clone());
    scope.refine(Arc::from("value"), list.clone());
    assert_eq!(scope.get("value"), Some(&list));
    assert_eq!(scope.binding_type("value"), Some(&variable));
    scope.annotate(Arc::from("value"), list.clone());
    assert_eq!(scope.binding_type("value"), Some(&variable));
    scope.invalidate_refinements();
    assert_eq!(scope.get("value"), Some(&variable));

    scope.refine(Arc::from("value"), list);
    let string = Arc::new(CalcitTypeAnnotation::String);
    scope.insert(Arc::from("value"), string.clone());
    assert_eq!(scope.binding_type("value"), Some(&string));
    assert!(!scope.has_refinements());

    scope.refine(Arc::from("unbound"), variable);
    assert!(scope.binding_type("unbound").is_none());
    scope.invalidate_refinements();
    assert!(!scope.contains_key("unbound"));
  }
}
