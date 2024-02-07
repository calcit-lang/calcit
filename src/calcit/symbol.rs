use std::{
  hash::{Hash, Hasher},
  sync::Arc,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalcitSymbolInfo {
  pub at_ns: Arc<str>,
  pub at_def: Arc<str>,
}

/// defRule: ns def
#[derive(Debug, Clone, PartialOrd, Ord)]
pub enum ImportInfo {
  /// ns imported via `:as`
  NsAs {
    at_ns: Arc<str>,
    at_def: Arc<str>,
    alias: Arc<str>,
  },
  /// (ns, def) imported via `:refer`
  NsReferDef { at_ns: Arc<str>, at_def: Arc<str> },
  /// used from calcit.core , forget about at_def for now
  Core { at_ns: Arc<str> },
  /// ns imported via `:default`, js only
  JsDefault {
    /// variable name used in code
    alias: Arc<str>,
    at_ns: Arc<str>,
    at_def: Arc<str>,
  },
  /// use def from same file
  SameFile { at_def: Arc<str> },
}

impl Hash for ImportInfo {
  fn hash<H: Hasher>(&self, state: &mut H) {
    match self {
      ImportInfo::NsAs { at_ns, alias, .. } => {
        "as".hash(state);
        at_ns.hash(state);
        alias.hash(state);
      }
      ImportInfo::NsReferDef { at_ns, .. } => {
        "refer".hash(state);
        at_ns.hash(state);
      }
      ImportInfo::Core { at_ns } => {
        "core".hash(state);
        at_ns.hash(state);
      }
      ImportInfo::JsDefault { at_ns, alias, .. } => {
        "js-default".hash(state);
        at_ns.hash(state);
        alias.hash(state);
      }
      ImportInfo::SameFile { .. } => {
        "same-file".hash(state);
      }
    }
  }
}

/// compare at namespace level, ignore at_def
impl PartialEq for ImportInfo {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (ImportInfo::NsAs { at_ns: a, alias: b, .. }, ImportInfo::NsAs { at_ns: c, alias: d, .. }) => a == c && b == d,
      (ImportInfo::NsReferDef { at_ns: a, .. }, ImportInfo::NsReferDef { at_ns: b, .. }) => a == b,
      (ImportInfo::Core { at_ns: a }, ImportInfo::Core { at_ns: b }) => a == b,
      (ImportInfo::JsDefault { at_ns: a, alias: b, .. }, ImportInfo::JsDefault { at_ns: c, alias: d, .. }) => a == c && b == d,
      (ImportInfo::SameFile { .. }, ImportInfo::SameFile { .. }) => true,
      _ => false,
    }
  }
}

impl Eq for ImportInfo {}

#[derive(Debug, Clone, PartialOrd, Ord)]
pub struct CalcitImport {
  /// real namespace
  /// detects string syntax for npm package
  pub ns: Arc<str>,
  /// real def
  /// from npm package, use `default` and asterisk in js
  pub def: Arc<str>,
  pub info: Arc<ImportInfo>,
  pub coord: Option<(usize, usize)>,
}

/// compare at namespace level, ignore at_def
impl PartialEq for CalcitImport {
  fn eq(&self, other: &Self) -> bool {
    self.ns == other.ns && self.info == other.info && self.def == other.def
  }
}

impl Eq for CalcitImport {}

impl Hash for CalcitImport {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.ns.hash(state);
    // ignores different in def
    self.info.hash(state);
  }
}
