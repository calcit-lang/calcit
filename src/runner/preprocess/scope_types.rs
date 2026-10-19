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
  refinements: HashMap<Arc<str>, Option<Annotation>>,
}

impl ScopeTypes {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn with_capacity(capacity: usize) -> Self {
    Self {
      values: HashMap::with_capacity(capacity),
      refinements: HashMap::new(),
    }
  }

  pub fn insert(&mut self, name: Arc<str>, annotation: Annotation) -> Option<Annotation> {
    self.refinements.remove(&name);
    self.values.insert(name, annotation)
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

  pub fn remove<Q: ?Sized + Hash + Eq>(&mut self, name: &Q) -> Option<Annotation>
  where
    Arc<str>: Borrow<Q>,
  {
    self.refinements.remove(name);
    self.values.remove(name)
  }

  pub fn clear(&mut self) {
    self.values.clear();
    self.refinements.clear();
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
      refinements: HashMap::new(),
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
