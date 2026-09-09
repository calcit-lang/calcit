//! Versioned compilation-unit contract for the Calx program backend.
//!
//! This module selects lifecycle roots from one configured Snapshot entry. It
//! does not lower or execute them: the current VM still exposes the kernel
//! compatibility entry named `main` only.

use std::fmt;
use std::sync::Arc;

use crate::snapshot::{Snapshot, SnapshotTarget};

use super::CalxDefinitionRef;

/// First program-level contract. Kernel editions `/1` and `/2` remain separate.
pub const CALX_PROGRAM_ABI_EDITION: &str = "calcit-calx-program/1";

/// Lifecycle role preserved when multiple configured roots share one definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CalxProgramRootRole {
  Init,
  Reload,
}

impl CalxProgramRootRole {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Init => "init",
      Self::Reload => "reload",
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalxProgramRoot {
  pub role: CalxProgramRootRole,
  pub definition: CalxDefinitionRef,
}

/// Deterministic program input selected before eligibility and lowering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalxProgramCompilationUnit {
  pub abi_edition: Arc<str>,
  pub entry_name: Arc<str>,
  pub host_target: Option<SnapshotTarget>,
  pub roots: Vec<CalxProgramRoot>,
}

impl CalxProgramCompilationUnit {
  /// Select the active Snapshot entry without changing the configured native/JS mode.
  pub fn from_snapshot(snapshot: &Snapshot) -> Result<Self, CalxProgramContractError> {
    let entry_name: Arc<str> = Arc::from(snapshot.active_entry_name());
    let entry = snapshot.active_entry().map_err(|message| CalxProgramContractError {
      code: CalxProgramContractCode::EntrySelection,
      entry_name: entry_name.clone(),
      role: None,
      message,
    })?;
    let init = parse_root(&entry_name, CalxProgramRootRole::Init, &entry.init_fn)?;
    let reload = parse_root(&entry_name, CalxProgramRootRole::Reload, &entry.reload_fn)?;
    Ok(Self {
      abi_edition: Arc::from(CALX_PROGRAM_ABI_EDITION),
      entry_name,
      host_target: entry.target,
      roots: vec![init, reload],
    })
  }

  /// Deterministic diagnostic/golden text; not a serialized compiler ABI.
  pub fn stable_summary(&self) -> String {
    let target = self.host_target.map(SnapshotTarget::as_str).unwrap_or("unspecified");
    let mut output = format!("abi {}\nentry {}\nhost-target {target}\n", self.abi_edition, self.entry_name);
    for root in &self.roots {
      output.push_str(&format!("root {} {}\n", root.role.as_str(), root.definition.qualified()));
    }
    output
  }
}

fn parse_root(entry_name: &Arc<str>, role: CalxProgramRootRole, configured: &str) -> Result<CalxProgramRoot, CalxProgramContractError> {
  let parsed = configured.split_once('/').and_then(|(namespace, definition)| {
    if configured.trim() == configured && !namespace.is_empty() && !definition.is_empty() {
      Some(CalxDefinitionRef::new(namespace, definition))
    } else {
      None
    }
  });
  let Some(definition) = parsed else {
    return Err(CalxProgramContractError {
      code: CalxProgramContractCode::DefinitionRef,
      entry_name: entry_name.clone(),
      role: Some(role),
      message: format!(
        "entry `{entry_name}` {} root must be a non-empty `namespace/definition`, got `{configured}`",
        role.as_str()
      ),
    });
  };
  Ok(CalxProgramRoot { role, definition })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalxProgramContractCode {
  EntrySelection,
  DefinitionRef,
}

impl CalxProgramContractCode {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::EntrySelection => "CALX_PROGRAM_ENTRY_SELECTION",
      Self::DefinitionRef => "CALX_PROGRAM_DEFINITION_REF",
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalxProgramContractError {
  pub code: CalxProgramContractCode,
  pub entry_name: Arc<str>,
  pub role: Option<CalxProgramRootRole>,
  pub message: String,
}

impl fmt::Display for CalxProgramContractError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "[{}] {}", self.code.as_str(), self.message)
  }
}

impl std::error::Error for CalxProgramContractError {}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;

  use crate::snapshot::{SnapshotEntry, SnapshotRunMode};

  use super::*;

  fn snapshot_with(entry_name: &str, entry: SnapshotEntry) -> Snapshot {
    Snapshot {
      package: "demo".to_owned(),
      about: None,
      version: "0.0.1".to_owned(),
      entries: HashMap::from([(entry_name.to_owned(), entry)]),
      files: HashMap::new(),
      active_entry: entry_name.to_owned(),
    }
  }

  fn entry(init_fn: &str, reload_fn: &str, target: Option<SnapshotTarget>) -> SnapshotEntry {
    SnapshotEntry {
      mode: SnapshotRunMode::Native,
      init_fn: init_fn.to_owned(),
      reload_fn: reload_fn.to_owned(),
      description: String::new(),
      modules: vec![],
      type_slots: HashMap::new(),
      feature_policy: HashMap::new(),
      target,
    }
  }

  #[test]
  fn preserves_lifecycle_roles_even_when_they_share_a_definition() {
    let snapshot = snapshot_with(
      "server",
      entry("app.server/main!", "app.server/main!", Some(SnapshotTarget::Native)),
    );
    let unit = CalxProgramCompilationUnit::from_snapshot(&snapshot).expect("program unit");
    assert_eq!(unit.abi_edition.as_ref(), CALX_PROGRAM_ABI_EDITION);
    assert_eq!(unit.roots.len(), 2);
    assert_eq!(unit.roots[0].role, CalxProgramRootRole::Init);
    assert_eq!(unit.roots[1].role, CalxProgramRootRole::Reload);
    assert_eq!(unit.roots[0].definition, unit.roots[1].definition);
    assert_eq!(
      unit.stable_summary(),
      "abi calcit-calx-program/1\nentry server\nhost-target native\nroot init app.server/main!\nroot reload app.server/main!\n"
    );
  }

  #[test]
  fn preserves_operator_definition_names_after_the_first_separator() {
    let snapshot = snapshot_with("default", entry("calcit.core//", "app.main/reload!", None));
    let unit = CalxProgramCompilationUnit::from_snapshot(&snapshot).expect("program unit");
    assert_eq!(unit.roots[0].definition.namespace.as_ref(), "calcit.core");
    assert_eq!(unit.roots[0].definition.definition.as_ref(), "/");
  }

  #[test]
  fn rejects_malformed_roots_with_a_stable_code_and_role() {
    let snapshot = snapshot_with("default", entry("app.main", "app.main/reload!", None));
    let error = CalxProgramCompilationUnit::from_snapshot(&snapshot).expect_err("invalid root");
    assert_eq!(error.code, CalxProgramContractCode::DefinitionRef);
    assert_eq!(error.role, Some(CalxProgramRootRole::Init));
    assert_eq!(error.code.as_str(), "CALX_PROGRAM_DEFINITION_REF");
  }

  #[test]
  fn reports_missing_selected_entry_without_falling_back_to_default() {
    let mut snapshot = snapshot_with("default", entry("app.main/main!", "app.main/reload!", None));
    snapshot.active_entry = "missing".to_owned();
    let error = CalxProgramCompilationUnit::from_snapshot(&snapshot).expect_err("missing entry");
    assert_eq!(error.code, CalxProgramContractCode::EntrySelection);
    assert_eq!(error.entry_name.as_ref(), "missing");
  }
}
