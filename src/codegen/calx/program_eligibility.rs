//! Whole-program eligibility for the versioned Calx compilation unit.
//!
//! Lifecycle roots are checked together and their reachable functions are
//! merged deterministically. No partial program is published when any root is
//! ineligible, and host capabilities remain explicit typed imports.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::program::CompiledProgram;
use crate::snapshot::SnapshotTarget;

use super::{
  CALX_PROGRAM_ABI_EDITION, CalxDefinitionRef, CalxEligibleFunction, CalxFallbackIssue, CalxHostImports, CalxProgramCompilationUnit,
  CalxProgramRoot, CalxProgramRootRole, analyze_calx_program_entry_with_imports, calx_kernel_abi_edition, write_eligible_functions,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CalxProgramEligibilityCode {
  AbiEdition,
  RootSet,
}

impl CalxProgramEligibilityCode {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::AbiEdition => "CALX_PROGRAM_ELIGIBILITY_ABI_EDITION",
      Self::RootSet => "CALX_PROGRAM_ELIGIBILITY_ROOT_SET",
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CalxProgramEligibilityIssue {
  pub code: CalxProgramEligibilityCode,
  pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CalxProgramRootEligibilityIssue {
  pub root_roles: Vec<CalxProgramRootRole>,
  pub root: CalxDefinitionRef,
  pub issue: CalxFallbackIssue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalxProgramEligibilityReport {
  pub abi_edition: Arc<str>,
  pub entry_name: Arc<str>,
  pub program_issues: Vec<CalxProgramEligibilityIssue>,
  pub root_issues: Vec<CalxProgramRootEligibilityIssue>,
}

impl CalxProgramEligibilityReport {
  /// Deterministic diagnostic/golden text; not a serialized compiler ABI.
  pub fn stable_summary(&self) -> String {
    let mut output = format!("abi {}\nentry {}\n", self.abi_edition, self.entry_name);
    for issue in &self.program_issues {
      output.push_str(&format!("program-issue {} message={}\n", issue.code.as_str(), issue.message));
    }
    for root_issue in &self.root_issues {
      let roles = root_issue.root_roles.iter().map(|role| role.as_str()).collect::<Vec<_>>().join(",");
      let source_path = root_issue
        .issue
        .source_path
        .as_ref()
        .map(|parts| parts.iter().map(u16::to_string).collect::<Vec<_>>().join("."))
        .unwrap_or_else(|| "-".to_owned());
      let call_path = root_issue
        .issue
        .call_path
        .iter()
        .map(CalxDefinitionRef::qualified)
        .collect::<Vec<_>>()
        .join(" -> ");
      output.push_str(&format!(
        "root-issue roles={roles} root={} {} {}/{} source={} call={} message={}\n",
        root_issue.root.qualified(),
        root_issue.issue.code.as_str(),
        root_issue.issue.namespace,
        root_issue.issue.definition,
        source_path,
        call_path,
        root_issue.issue.message
      ));
    }
    output
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalxEligibleProgram {
  pub abi_edition: Arc<str>,
  pub kernel_abi_edition: Arc<str>,
  pub entry_name: Arc<str>,
  pub host_target: Option<SnapshotTarget>,
  pub roots: Vec<CalxProgramRoot>,
  pub functions: Vec<CalxEligibleFunction>,
}

impl CalxEligibleProgram {
  /// Deterministic diagnostic/golden text; not a serialized compiler ABI.
  pub fn stable_summary(&self) -> String {
    let target = self.host_target.map(SnapshotTarget::as_str).unwrap_or("unspecified");
    let mut output = format!(
      "abi {}\nkernel-abi {}\nentry {}\nhost-target {target}\n",
      self.abi_edition, self.kernel_abi_edition, self.entry_name
    );
    for root in &self.roots {
      output.push_str(&format!("root {} {}\n", root.role.as_str(), root.definition.qualified()));
    }
    write_eligible_functions(&mut output, &self.functions);
    output
  }
}

pub fn analyze_calx_program_eligibility(
  program: &CompiledProgram,
  unit: &CalxProgramCompilationUnit,
) -> Result<CalxEligibleProgram, CalxProgramEligibilityReport> {
  analyze_calx_program_eligibility_with_imports(program, unit, &CalxHostImports::new())
}

pub fn analyze_calx_program_eligibility_with_imports(
  program: &CompiledProgram,
  unit: &CalxProgramCompilationUnit,
  imports: &CalxHostImports,
) -> Result<CalxEligibleProgram, CalxProgramEligibilityReport> {
  let mut roots = unit.roots.clone();
  roots.sort_by(|left, right| left.role.cmp(&right.role).then_with(|| left.definition.cmp(&right.definition)));

  let mut program_issues = vec![];
  if unit.abi_edition.as_ref() != CALX_PROGRAM_ABI_EDITION {
    program_issues.push(CalxProgramEligibilityIssue {
      code: CalxProgramEligibilityCode::AbiEdition,
      message: format!(
        "program unit edition `{}` does not match supported edition `{CALX_PROGRAM_ABI_EDITION}`",
        unit.abi_edition
      ),
    });
  }
  let init_count = roots.iter().filter(|root| root.role == CalxProgramRootRole::Init).count();
  let reload_count = roots.iter().filter(|root| root.role == CalxProgramRootRole::Reload).count();
  if init_count != 1 || reload_count != 1 {
    program_issues.push(CalxProgramEligibilityIssue {
      code: CalxProgramEligibilityCode::RootSet,
      message: format!("program unit requires exactly one init and one reload root, got init={init_count} reload={reload_count}"),
    });
  }
  if !program_issues.is_empty() {
    return Err(CalxProgramEligibilityReport {
      abi_edition: unit.abi_edition.clone(),
      entry_name: unit.entry_name.clone(),
      program_issues,
      root_issues: vec![],
    });
  }

  let mut roots_by_definition = BTreeMap::<CalxDefinitionRef, Vec<CalxProgramRootRole>>::new();
  for root in &roots {
    roots_by_definition.entry(root.definition.clone()).or_default().push(root.role);
  }
  for roles in roots_by_definition.values_mut() {
    roles.sort();
    roles.dedup();
  }

  let mut functions = BTreeMap::<CalxDefinitionRef, CalxEligibleFunction>::new();
  let mut root_issues = vec![];
  for (root, roles) in roots_by_definition {
    match analyze_calx_program_entry_with_imports(program, root.namespace.clone(), root.definition.clone(), imports) {
      Ok(graph) => {
        for function in graph.functions {
          functions.entry(function.definition.clone()).or_insert(function);
        }
      }
      Err(report) => {
        root_issues.extend(report.issues.into_iter().map(|issue| CalxProgramRootEligibilityIssue {
          root_roles: roles.clone(),
          root: root.clone(),
          issue,
        }));
      }
    }
  }
  root_issues.sort();
  root_issues.dedup();
  if !root_issues.is_empty() {
    return Err(CalxProgramEligibilityReport {
      abi_edition: unit.abi_edition.clone(),
      entry_name: unit.entry_name.clone(),
      program_issues,
      root_issues,
    });
  }

  let functions = functions.into_values().collect::<Vec<_>>();
  Ok(CalxEligibleProgram {
    abi_edition: unit.abi_edition.clone(),
    kernel_abi_edition: Arc::from(calx_kernel_abi_edition(functions.iter())),
    entry_name: unit.entry_name.clone(),
    host_target: unit.host_target,
    roots,
    functions,
  })
}
