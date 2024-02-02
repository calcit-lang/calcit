use std::sync::Arc;

/// resolved value of real meaning of a symbol
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolResolved {
  /// a local variable
  ResolvedLocal,
  /// raw syntax, no target, for example `&` is a raw syntax
  ResolvedRaw,
  /// registered from runtime
  ResolvedRegistered,
  /// definition attached on namespace
  ResolvedDef {
    ns: Arc<str>,
    def: Arc<str>,
    rule: Option<ImportRule>,
  },
}

/// defRule: ns def
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportRule {
  /// ns imported via `:as`
  NsAs(Arc<str>),
  /// (ns, def) imported via `:refer`
  NsReferDef(Arc<str>, Arc<str>),
  /// ns imported via `:default`, js only
  NsDefault(Arc<str>),
}

#[derive(Debug, Clone)]
pub struct CalcitSymbolInfo {
  pub ns: Arc<str>,
  pub at_def: Arc<str>,
  pub resolved: Option<SymbolResolved>,
}
