use std::hash::{Hash, Hasher};
use std::sync::Arc;

use cirru_edn::EdnTag;

use super::{CalcitFn, CalcitTypeAnnotation};

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

// Manual implementation since CalcitFn doesn't implement Eq/PartialEq
impl PartialEq for CalcitTrait {
  fn eq(&self, other: &Self) -> bool {
    // Traits are equal if they have the same name
    self.name == other.name
  }
}

impl Eq for CalcitTrait {}

impl CalcitTrait {
  /// Create a new trait with the given name and methods
  pub fn new(name: EdnTag, methods: Vec<EdnTag>) -> Self {
    let defaults = vec![None; methods.len()];
    let dynamic = Arc::new(CalcitTypeAnnotation::Dynamic);
    let method_types = vec![dynamic; methods.len()];
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
    // Don't hash defaults or method_types as they contain complex types
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
