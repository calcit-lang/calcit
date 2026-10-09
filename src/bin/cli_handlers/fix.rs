use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use calcit::calcit::{Calcit, CalcitFnTypeAnnotation, CalcitProc, CalcitTypeAnnotation, LocatedWarning, SchemaKind};
use calcit::call_stack::CallStackList;
use calcit::cli_args::{FixCommand, WeakTypesCommand};
use calcit::data::cirru::code_to_calcit;
use calcit::snapshot::Snapshot;
use calcit::{program, runner};
use cirru_parser::Cirru;
use md5::{Digest, Md5};
use serde::Serialize;
use serde_json::Value;

use super::common::{cirru_to_json_value, format_path, json_value_to_cirru, markdown_cirru_section};
use super::edit::{load_snapshot, navigate_to_path, run_staged_fix_transaction, snapshot_content_revision};
use super::structured_output::{StructuredOutputFormat, format_json_value_as_edn};
use crate::type_coverage;

const REMOVED_DATA_API_RULE: &str = "removed-data-api-v1";
const REMOVED_DATA_API_DIAGNOSTIC: &str = "W_REMOVED_DATA_API";
const REDUNDANT_DO_RULE: &str = "redundant-do-v1";
const REDUNDANT_DO_DIAGNOSTIC: &str = "FIX_REDUNDANT_DO";
const SINGLE_EXPRESSION_DO_RULE: &str = "single-expression-do-v1";
const SINGLE_EXPRESSION_DO_DIAGNOSTIC: &str = "FIX_SINGLE_EXPRESSION_DO";
const NAMED_ENUM_CONSTRUCTOR_RULE: &str = "named-enum-constructor-v1";
const NAMED_ENUM_CONSTRUCTOR_DIAGNOSTIC: &str = "FIX_NAMED_ENUM_CONSTRUCTOR";
const NAMED_STRUCT_CONSTRUCTOR_RULE: &str = "named-struct-constructor-v1";
const NAMED_STRUCT_CONSTRUCTOR_DIAGNOSTIC: &str = "FIX_NAMED_STRUCT_CONSTRUCTOR";
const CORE_NOMINAL_CONSTRUCTOR_RULE: &str = "core-nominal-constructor-v1";
const CORE_NOMINAL_CONSTRUCTOR_DIAGNOSTIC: &str = "FIX_CORE_NOMINAL_CONSTRUCTOR";
const CORE_OPTION_METHOD_RULE: &str = "core-option-method-v1";
const CORE_OPTION_METHOD_DIAGNOSTIC: &str = "FIX_CORE_OPTION_METHOD";
const CORE_RESULT_METHOD_RULE: &str = "core-result-method-v1";
const CORE_RESULT_METHOD_DIAGNOSTIC: &str = "FIX_CORE_RESULT_METHOD";
const CORE_FUNCTION_ALIAS_RULE: &str = "core-function-alias-v1";
const CORE_FUNCTION_ALIAS_DIAGNOSTIC: &str = "FIX_CORE_FUNCTION_ALIAS";
const CORE_INTEGER_PREDICATE_RULE: &str = "core-integer-predicate-v1";
const CORE_INTEGER_PREDICATE_DIAGNOSTIC: &str = "FIX_CORE_INTEGER_PREDICATE";
const CORE_IDENTITY_CONVERSION_RULE: &str = "core-identity-conversion-v1";
const CORE_IDENTITY_CONVERSION_DIAGNOSTIC: &str = "FIX_CORE_IDENTITY_CONVERSION";
const CORE_LIST_ADD_RULE: &str = "core-list-add-v1";
const CORE_LIST_ADD_DIAGNOSTIC: &str = "FIX_CORE_LIST_ADD";
const CORE_COLLECTION_LEN_RULE: &str = "core-collection-len-v1";
const CORE_COLLECTION_LEN_DIAGNOSTIC: &str = "FIX_CORE_COLLECTION_LEN";
const CORE_EFFECT_METHOD_RULE: &str = "core-effect-method-v1";
const CORE_EFFECT_METHOD_DIAGNOSTIC: &str = "FIX_CORE_EFFECT_METHOD";
const CORE_REF_CONSTRUCTOR_RULE: &str = "core-ref-constructor-v1";
const CORE_REF_CONSTRUCTOR_DIAGNOSTIC: &str = "FIX_CORE_REF_CONSTRUCTOR";
const RENAME_DEFINITION_RULE: &str = "rename-definition-v1";
const RENAME_DEFINITION_DIAGNOSTIC: &str = "REFACTOR_RENAME_DEFINITION";
const RENAME_LOCAL_RULE: &str = "rename-local-v1";
const RENAME_LOCAL_DIAGNOSTIC: &str = "REFACTOR_RENAME_LOCAL";
const VALUE_TO_ZERO_ARG_FN_RULE: &str = "value-to-zero-arg-fn-v1";
const VALUE_TO_ZERO_ARG_FN_DIAGNOSTIC: &str = "REFACTOR_VALUE_TO_ZERO_ARG_FN";
const SYNTHESIZE_SCHEMA_RULE: &str = "synthesize-schema-v1";
const SYNTHESIZE_SCHEMA_DIAGNOSTIC: &str = "REFACTOR_SYNTHESIZE_SCHEMA";
const SPREAD_CALL_PROOF_RULE: &str = "spread-call-proof-v1";
const SPREAD_CALL_PROOF_DIAGNOSTIC: &str = "REFACTOR_SPREAD_CALL";
const UNSAFE_COERCE_BOUNDARY_RULE: &str = "unsafe-coerce-boundary-v1";
const UNSAFE_COERCE_BOUNDARY_DIAGNOSTIC: &str = "E_UNSCOPED_UNSAFE_COERCE";
const ASSERT_TYPE_PROOF_RULE: &str = "assert-type-proof-v1";
const ASSERT_TYPE_PROOF_DIAGNOSTIC: &str = "E_ASSERT_TYPE_UNPROVEN";
const CONCRETE_RETURN_PROOF_RULE: &str = "concrete-return-proof-v1";
const CONCRETE_RETURN_PROOF_DIAGNOSTIC: &str = "E_FN_RETURN_UNPROVEN";
const CALLABLE_CONTRACT_PROOF_RULE: &str = "callable-contract-proof-v1";
const NOMINAL_WRITE_PROOF_RULE: &str = "nominal-write-proof-v1";
const OPTIONAL_PARAMETERS_RULE: &str = "optional-parameters-v1";
const OPTIONAL_PARAMETERS_DIAGNOSTIC: &str = "E_LEGACY_OPTIONAL_PARAM";
const SURFACE_LATEST_V1_PRESET: &str = "surface-latest-v1";
const SURFACE_LATEST_V2_PRESET: &str = "surface-latest-v2";
const CORE_API_028_V1_PRESET: &str = "core-api-0.28-v1";
const CORE_API_029_V1_PRESET: &str = "core-api-0.29-v1";
const AVAILABLE_RULES: [&str; 5] = [
  REMOVED_DATA_API_RULE,
  NAMED_ENUM_CONSTRUCTOR_RULE,
  NAMED_STRUCT_CONSTRUCTOR_RULE,
  REDUNDANT_DO_RULE,
  SINGLE_EXPRESSION_DO_RULE,
];
const SURFACE_LATEST_V1_RULES: [&str; 4] = [
  REMOVED_DATA_API_RULE,
  NAMED_ENUM_CONSTRUCTOR_RULE,
  NAMED_STRUCT_CONSTRUCTOR_RULE,
  REDUNDANT_DO_RULE,
];
const SURFACE_LATEST_V2_RULES: [&str; 5] = [
  REMOVED_DATA_API_RULE,
  NAMED_ENUM_CONSTRUCTOR_RULE,
  NAMED_STRUCT_CONSTRUCTOR_RULE,
  REDUNDANT_DO_RULE,
  SINGLE_EXPRESSION_DO_RULE,
];
// These proven renames replace leaves only, so nested calls retain their source paths.
const CORE_API_028_V1_RULES: [&str; 5] = [
  CORE_INTEGER_PREDICATE_RULE,
  CORE_IDENTITY_CONVERSION_RULE,
  CORE_LIST_ADD_RULE,
  CORE_COLLECTION_LEN_RULE,
  CORE_EFFECT_METHOD_RULE,
];
// The 0.29 preset keeps every 0.28 rule this CLI still ships and adds the Ref
// constructor rename, so `core-api-0.28-v1` keeps its published meaning.
const CORE_API_029_V1_RULES: [&str; 6] = [
  CORE_INTEGER_PREDICATE_RULE,
  CORE_IDENTITY_CONVERSION_RULE,
  CORE_LIST_ADD_RULE,
  CORE_COLLECTION_LEN_RULE,
  CORE_EFFECT_METHOD_RULE,
  CORE_REF_CONSTRUCTOR_RULE,
];
// Alias rules whose old names were removed in 0.29.0; only 0.28.x CLIs ship them.
const RETIRED_CORE_API_028_RULES: [&str; 10] = [
  "core-list-intersperse-v1",
  "core-map-distinct-values-v1",
  "core-predicate-method-v1",
  "core-set-include-v1",
  "core-list-fold-v1",
  "core-list-flat-map-v1",
  "core-list-join-string-v1",
  "core-list-get-v1",
  "core-collection-combine-v1",
  "core-non-nil-predicate-v1",
];
const TAG_MATCH_RULE: &str = "tag-match-to-match-v1";
const REQUIRED_STRUCT_FIELD_RULE: &str = "required-struct-field-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
enum FixOperation {
  ReplaceLeaf { original: String, replacement: String },
  WrapLeafCall { original: String },
  ReplaceNode { original: String, replacement: String },
  SpliceDo,
  ReplaceImports { code: String },
  ReplaceExamples { code: String },
  ReplaceTest { name: String, tags: String, code: String },
  ReplaceSchema { code: String },
  ReplaceDefinition { code: String },
  RenameDefinition { new_name: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct FixSuggestion {
  rule_id: &'static str,
  #[serde(skip_serializing_if = "str::is_empty")]
  diagnostic_code: &'static str,
  semantic_layer: &'static str,
  source_file: String,
  definition: String,
  path: String,
  fingerprint: String,
  origin_chain: Vec<Value>,
  original: Value,
  replacement: Option<Value>,
  applicability: &'static str,
  message: String,
  #[serde(skip)]
  target_path: Vec<usize>,
  #[serde(skip)]
  operation: Option<FixOperation>,
}

#[derive(Debug, Serialize)]
struct FixReport<'a> {
  schema_version: u8,
  command: &'static str,
  revision: &'a str,
  data: FixReportData<'a>,
  diagnostics: Vec<Value>,
  next: Vec<Value>,
}

#[derive(Debug, Serialize)]
struct FixReportData<'a> {
  mode: &'static str,
  filters: FixFilters<'a>,
  changed: bool,
  new_revision: &'a str,
  validation: FixValidation,
  suggestions: &'a [FixSuggestion],
  #[serde(skip_serializing_if = "Option::is_none")]
  workflow: Option<StrictWorkflowManifest>,
}

#[derive(Debug, Serialize)]
struct FixValidation {
  status: &'static str,
  staged_scope_preprocess: bool,
  checked_operations: usize,
}

#[derive(Debug, Serialize)]
struct FixFilters<'a> {
  namespace: Option<&'a str>,
  definition: Option<&'a str>,
  replacement_name: Option<&'a str>,
  rule_id: &'a str,
  preset_id: Option<&'a str>,
  expanded_rule_ids: Vec<&'static str>,
  expanded_rules: Vec<FixRuleMetadata>,
  #[serde(skip_serializing_if = "Option::is_none")]
  source_coverage: Option<FixSourceCoverage>,
}

#[derive(Debug, Serialize)]
struct FixSourceCoverage {
  scanned_regions: &'static [&'static str],
  manual_review_regions: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct FixRuleMetadata {
  rule_id: &'static str,
  diagnostic_code: &'static str,
  evidence_source: &'static str,
  lifecycle: &'static str,
  source_version_required: bool,
}

#[derive(Debug, Serialize)]
struct StrictWorkflowManifest {
  workflow: &'static str,
  mode: &'static str,
  status: &'static str,
  entries: Vec<StrictWorkflowEntry>,
  safe_fixes: StrictWorkflowSafeFixes,
  review_required: StrictWorkflowReviewRequired,
  retained_type_boundaries: Vec<StrictWorkflowTypeFinding>,
  preflight: crate::verification::PreflightReport,
  verification: StrictWorkflowVerification,
  resume: StrictWorkflowResume,
}

#[derive(Debug, Serialize)]
struct StrictWorkflowEntry {
  name: String,
  mode: String,
  target: String,
  init_fn: String,
  reload_fn: String,
  type_slots: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
struct StrictWorkflowSafeFixes {
  preset: &'static str,
  suggestions: usize,
  status: &'static str,
}

#[derive(Debug, Serialize)]
struct StrictWorkflowReviewRequired {
  source_fixes: Vec<StrictWorkflowSourceReview>,
  type_findings: Vec<StrictWorkflowTypeFinding>,
  ffi_boundaries: Vec<type_coverage::FfiBoundaryEvidence>,
  schema_candidates: Vec<schema_synthesis::SchemaEvidenceCandidate>,
  structural_candidates: type_coverage::StructuralCandidates,
}

#[derive(Debug, Serialize)]
struct StrictWorkflowSourceReview {
  rule_id: &'static str,
  definition: String,
  path: String,
  message: String,
}

#[derive(Debug, Serialize)]
struct StrictWorkflowTypeFinding {
  definition: String,
  kind: String,
  intent: String,
  path: String,
  detail: String,
}

struct StrictWorkflowTypeEvidence {
  review_required: Vec<StrictWorkflowTypeFinding>,
  retained_boundaries: Vec<StrictWorkflowTypeFinding>,
  ffi_boundaries: Vec<type_coverage::FfiBoundaryEvidence>,
  schema_candidates: Vec<schema_synthesis::SchemaEvidenceCandidate>,
  structural_candidates: type_coverage::StructuralCandidates,
}

#[derive(Debug, Serialize)]
struct StrictWorkflowVerification {
  commands: Vec<Vec<String>>,
  results: Vec<StrictWorkflowVerificationResult>,
  external_commands: Vec<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct StrictWorkflowVerificationResult {
  scope: String,
  status: &'static str,
  report: Value,
}

#[derive(Debug, Serialize)]
struct StrictWorkflowResume {
  revision: String,
  apply_command: Vec<String>,
}

fn strict_workflow_entries(snapshot: &Snapshot) -> Vec<StrictWorkflowEntry> {
  let mut entries = snapshot.entries.iter().collect::<Vec<_>>();
  entries.sort_by_key(|(name, _)| *name);
  entries
    .into_iter()
    .map(|(name, entry)| StrictWorkflowEntry {
      name: name.clone(),
      mode: entry.mode.as_str().to_owned(),
      target: entry.target.map(|target| target.as_str()).unwrap_or("unspecified").to_owned(),
      init_fn: entry.init_fn.clone(),
      reload_fn: entry.reload_fn.clone(),
      type_slots: entry.type_slots.iter().map(|(name, value)| (name.clone(), value.clone())).collect(),
    })
    .collect()
}

fn strict_workflow_type_findings(snapshot: &Snapshot) -> Result<StrictWorkflowTypeEvidence, String> {
  let options = WeakTypesCommand {
    ns: None,
    ns_prefix: None,
    deps: false,
    only: None,
    intent: None,
    summary_only: false,
    format: "json".to_owned(),
    ffi_evidence: true,
    schema_evidence: true,
    incremental: false,
  };
  let mut review_required = Vec::new();
  let mut retained_boundaries = Vec::new();
  for row in type_coverage::collect_weak_type_rows(&options, snapshot)? {
    let definition = format!("{}/{}", row.ns, row.def);
    for occurrence in row.occurrences {
      let finding = StrictWorkflowTypeFinding {
        definition: definition.clone(),
        kind: occurrence.kind.as_str().to_owned(),
        intent: occurrence.intent.as_str().to_owned(),
        path: occurrence.path,
        detail: occurrence.detail,
      };
      match occurrence.intent {
        type_coverage::WeakTypeIntent::Unresolved
        | type_coverage::WeakTypeIntent::DeclaredOptional
        | type_coverage::WeakTypeIntent::ExplicitUnsafe => review_required.push(finding),
        type_coverage::WeakTypeIntent::IntentionalJsFfi | type_coverage::WeakTypeIntent::IntentionalTypeSlotDynamic => {
          retained_boundaries.push(finding)
        }
        type_coverage::WeakTypeIntent::IntentionalMacroSyntax | type_coverage::WeakTypeIntent::DeclaredUnit => {}
      }
    }
  }
  for findings in [&mut review_required, &mut retained_boundaries] {
    findings.sort_by(|left, right| {
      left
        .definition
        .cmp(&right.definition)
        .then(left.path.cmp(&right.path))
        .then(left.kind.cmp(&right.kind))
        .then(left.intent.cmp(&right.intent))
    });
  }
  let ffi_boundaries = type_coverage::collect_ffi_boundary_evidence(&options, snapshot)?;
  let schema_candidates = type_coverage::collect_schema_candidates(&options, snapshot)?;
  let structural_candidates = type_coverage::collect_structural_candidates(&options, snapshot)?;
  Ok(StrictWorkflowTypeEvidence {
    review_required,
    retained_boundaries,
    ffi_boundaries,
    schema_candidates,
    structural_candidates,
  })
}

fn strict_workflow_commands(snapshot_file: &str, snapshot: &Snapshot) -> Vec<(String, Vec<String>)> {
  let mut commands = Vec::new();
  let mut entries = snapshot.entries.keys().cloned().collect::<Vec<_>>();
  entries.sort();
  for entry in entries {
    commands.push((
      format!("entry:{entry}"),
      vec![
        snapshot_file.to_owned(),
        "--entry".to_owned(),
        entry,
        "--check-only".to_owned(),
        "--keep-going".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
        "--tips-level".to_owned(),
        "none".to_owned(),
      ],
    ));
  }
  let mut profiles = snapshot.verification.profiles.keys().cloned().collect::<Vec<_>>();
  profiles.sort();
  for profile in profiles {
    commands.push((
      format!("profile:{profile}"),
      vec![
        snapshot_file.to_owned(),
        "--tips-level".to_owned(),
        "none".to_owned(),
        "analyze".to_owned(),
        "verify".to_owned(),
        "--profile".to_owned(),
        profile,
        "--format".to_owned(),
        "json".to_owned(),
      ],
    ));
  }
  commands
}

fn run_strict_workflow_verification(commands: &[(String, Vec<String>)]) -> Result<Vec<StrictWorkflowVerificationResult>, String> {
  let executable = std::env::current_exe().map_err(|error| format!("Failed to locate current Calcit executable: {error}"))?;
  let mut results = Vec::new();
  for (scope, arguments) in commands {
    let output = Command::new(&executable)
      .args(arguments)
      .output()
      .map_err(|error| format!("Failed to run strict workflow verification for `{scope}`: {error}"))?;
    let report = serde_json::from_slice::<Value>(&output.stdout).unwrap_or_else(|error| {
      serde_json::json!({
        "schema_version": 1,
        "command": "fix.workflow.verify",
        "data": null,
        "diagnostics": [{
          "code": "E_WORKFLOW_OUTPUT",
          "phase": "verification",
          "severity": "error",
          "message": format!("Expected one JSON report from `{scope}`: {error}"),
          "stderr": String::from_utf8_lossy(&output.stderr),
        }],
      })
    });
    results.push(StrictWorkflowVerificationResult {
      scope: scope.clone(),
      status: if output.status.success() { "passed" } else { "failed" },
      report,
    });
  }
  Ok(results)
}

/// Plan deterministic source migrations, validate them on a staged Snapshot, and optionally commit them atomically.
pub(crate) fn handle_fix_command(
  options: &FixCommand,
  compiled_snapshot: &Snapshot,
  project_namespaces: &HashSet<String>,
  snapshot_file: &str,
) -> Result<(), String> {
  if options.pattern.is_some() || options.replacement.is_some() {
    return structural_rewrite::handle_pattern_rewrite(options, compiled_snapshot, project_namespaces, snapshot_file);
  }
  validate_options(options)?;
  // A whole-project plan covers definitions from every named entry, so it has
  // no single host target. Entry-scoped checks still enforce their own target.
  let _target_scope = FixTargetScope::new(options.ns.is_none());
  let selected_rules = selected_rule_ids(options);
  let source_snapshot = load_snapshot(snapshot_file)?;
  let source_content = fs::read_to_string(snapshot_file).map_err(|error| format!("Failed to read {snapshot_file}: {error}"))?;
  let revision = snapshot_content_revision(&source_content);
  if let Some(expected) = options.expect_revision.as_deref()
    && expected != revision
  {
    return Err(format!(
      "Snapshot revision mismatch: expected '{expected}', current revision is '{revision}'. Re-run `calcit fix` and review the new plan."
    ));
  }

  let semantic_rename = selected_rules.contains(&RENAME_DEFINITION_RULE);
  let local_rename = selected_rules.contains(&RENAME_LOCAL_RULE);
  let value_to_zero_arg_fn = selected_rules.contains(&VALUE_TO_ZERO_ARG_FN_RULE);
  let schema_synthesis = selected_rules.contains(&SYNTHESIZE_SCHEMA_RULE);
  let optional_parameters = selected_rules.contains(&OPTIONAL_PARAMETERS_RULE);
  let core_predicate_rename = selected_rules.contains(&CORE_INTEGER_PREDICATE_RULE)
    || selected_rules.contains(&CORE_FUNCTION_ALIAS_RULE)
    || selected_rules.contains(&CORE_REF_CONSTRUCTOR_RULE);
  let semantic_refactor = semantic_rename || value_to_zero_arg_fn || local_rename;
  let migration_rule =
    semantic_refactor || schema_synthesis || optional_parameters || core_predicate_rename || options.workflow.is_some();
  let validation_only = std::env::var("CALCIT_FIX_VALIDATE_ONLY").as_deref() == Ok("1");
  let project_definitions = if (semantic_refactor && !local_rename) || schema_synthesis || optional_parameters {
    select_project_definitions(compiled_snapshot, project_namespaces)?
  } else {
    select_definitions(options, compiled_snapshot, project_namespaces)?
  };
  let selected_definitions = if (schema_synthesis && validation_only) || optional_parameters {
    select_definitions(options, compiled_snapshot, project_namespaces)?
  } else {
    project_definitions.clone()
  };
  let mut boundary_suggestions = Vec::new();
  let mut boundary_diagnostics = Vec::new();
  let review_rule = selected_rules.iter().copied().find(|rule| {
    matches!(
      *rule,
      UNSAFE_COERCE_BOUNDARY_RULE
        | ASSERT_TYPE_PROOF_RULE
        | CONCRETE_RETURN_PROOF_RULE
        | CALLABLE_CONTRACT_PROOF_RULE
        | NOMINAL_WRITE_PROOF_RULE
    )
  });
  let warnings = if let Some(rule) = review_rule.filter(|_| !validation_only) {
    // Preserve the complete migration evidence even when a later proof pass
    // stops early in an individual definition.
    let migration_warnings = if options.workflow.is_some() {
      Some(with_legacy_migration_mode(|| {
        let warnings = RefCell::new(Vec::new());
        for (namespace, definition) in &selected_definitions {
          if let Err(error) = runner::preprocess::ensure_ns_def_compiled(namespace, definition, &warnings, &CallStackList::default()) {
            // Legacy mode still rejects contradictions. Keep their diagnostics
            // in the manifest while collecting other definitions' migration evidence.
            let owner = error.location.as_ref().map_or_else(
              || format!("{namespace}/{definition}"),
              |location| format!("{}/{}", location.ns, location.def),
            );
            let diagnostic = compiler_review::boundary_diagnostic(&error, &owner);
            if !boundary_diagnostics.contains(&diagnostic) {
              boundary_diagnostics.push(diagnostic);
            }
          }
        }
        warnings.into_inner()
      }))
    } else {
      None
    };
    let review = compiler_review::compile_boundary_review(
      &source_snapshot,
      snapshot_file,
      &selected_definitions,
      rule,
      options.workflow.is_some(),
    )?;
    boundary_suggestions = review.suggestions;
    for diagnostic in review.diagnostics {
      if !boundary_diagnostics.contains(&diagnostic) {
        boundary_diagnostics.push(diagnostic);
      }
    }
    migration_warnings.unwrap_or(review.warnings)
  } else if validation_only {
    compile_selected_definitions(&selected_definitions)?
  } else if migration_rule {
    compile_selected_definitions_for_migration(&selected_definitions)?
  } else {
    compile_selected_definitions(&selected_definitions)?
  };
  let semantic_warning_identities = if semantic_rename {
    Some(semantic_rename_warning_identities(
      &warnings,
      options.ns.as_deref().expect("semantic rename requires namespace"),
      options.definition.as_deref().expect("semantic rename requires definition"),
      options.replacement_name.as_deref().expect("semantic rename requires replacement"),
    ))
  } else if value_to_zero_arg_fn || schema_synthesis || local_rename {
    Some(warning_identities(&warnings))
  } else {
    None
  };
  let mut suggestions = boundary_suggestions;
  if selected_rules.contains(&SPREAD_CALL_PROOF_RULE) {
    suggestions.extend(spread_call::plan_spread_call_fixes(
      &source_snapshot,
      snapshot_file,
      &selected_definitions,
    )?);
  }
  if semantic_rename {
    suggestions.extend(plan_definition_rename(
      options,
      &source_snapshot,
      snapshot_file,
      &project_definitions,
    )?);
  } else if local_rename {
    let namespace = options.ns.as_deref().expect("local rename requires namespace");
    let definition = options.definition.as_deref().expect("local rename requires definition");
    let at = super::common::parse_path(options.at.as_deref().expect("local rename requires --at"))?;
    let file = source_snapshot
      .files
      .get(namespace)
      .ok_or_else(|| format!("Local rename namespace `{namespace}` is not an editable project namespace."))?;
    let imports = program::extract_import_map(&file.ns.code, namespace)?;
    let mut is_macro = local_rename::macro_head_cache(|head: &str| {
      if !super::edit::definition_head_is_macro(snapshot_file, &source_snapshot, namespace, head)? {
        return Ok(local_rename::MacroHead::Plain);
      }
      // Unqualified heads that are neither local definitions nor imports resolve to core.
      let core = !head.contains('/') && !file.defs.contains_key(head) && !imports.contains_key(head);
      Ok(if core {
        local_rename::MacroHead::Core
      } else {
        local_rename::MacroHead::Other
      })
    });
    suggestions.extend(local_rename::plan_local_rename(
      namespace,
      definition,
      &at,
      options.replacement_name.as_deref().expect("local rename requires --to"),
      &source_snapshot,
      snapshot_file,
      &mut is_macro,
    )?);
  } else if value_to_zero_arg_fn {
    suggestions.extend(plan_value_to_zero_arg_fn(
      options,
      &source_snapshot,
      snapshot_file,
      &selected_definitions,
    )?);
  } else if schema_synthesis {
    suggestions.extend(plan_schema_synthesis(
      options,
      &source_snapshot,
      snapshot_file,
      &project_definitions,
    )?);
  } else if optional_parameters {
    suggestions.extend(plan_optional_parameter_diagnostics(
      options,
      &source_snapshot,
      snapshot_file,
      &project_definitions,
    )?);
  } else if selected_rules.contains(&REMOVED_DATA_API_RULE) {
    suggestions.extend(plan_removed_data_api_fixes(options, &source_snapshot, snapshot_file, &warnings)?);
  }
  if selected_rules.contains(&CORE_NOMINAL_CONSTRUCTOR_RULE) {
    suggestions.extend(plan_core_nominal_constructor_fixes(
      &source_snapshot,
      snapshot_file,
      &selected_definitions,
    )?);
  }
  if selected_rules.contains(&CORE_OPTION_METHOD_RULE) {
    suggestions.extend(plan_core_nominal_method_fixes(
      &source_snapshot,
      snapshot_file,
      &selected_definitions,
      CoreNominalMethodKind::Option,
    )?);
  }
  if selected_rules.contains(&CORE_RESULT_METHOD_RULE) {
    suggestions.extend(plan_core_nominal_method_fixes(
      &source_snapshot,
      snapshot_file,
      &selected_definitions,
      CoreNominalMethodKind::Result,
    )?);
  }
  if selected_rules.contains(&CORE_FUNCTION_ALIAS_RULE) {
    for alias in [
      CorePredicateRename::Optionally,
      CorePredicateRename::Join,
      CorePredicateRename::Vals,
    ] {
      suggestions.extend(plan_core_predicate_rename_fixes(
        &source_snapshot,
        snapshot_file,
        &selected_definitions,
        alias,
      )?);
    }
  }
  if selected_rules.contains(&CORE_INTEGER_PREDICATE_RULE) {
    suggestions.extend(plan_core_predicate_rename_fixes(
      &source_snapshot,
      snapshot_file,
      &selected_definitions,
      CorePredicateRename::Integer,
    )?);
  }
  if selected_rules.contains(&CORE_IDENTITY_CONVERSION_RULE) {
    suggestions.extend(plan_core_identity_conversion_fixes(
      &source_snapshot,
      snapshot_file,
      &selected_definitions,
    )?);
  }
  if selected_rules.contains(&CORE_LIST_ADD_RULE) {
    suggestions.extend(plan_core_list_add_fixes(&source_snapshot, snapshot_file, &selected_definitions)?);
  }
  if selected_rules.contains(&CORE_COLLECTION_LEN_RULE) {
    suggestions.extend(plan_core_collection_len_fixes(
      &source_snapshot,
      snapshot_file,
      &selected_definitions,
    )?);
  }
  if selected_rules.contains(&CORE_EFFECT_METHOD_RULE) {
    for rule in CORE_EFFECT_METHOD_ALIASES {
      suggestions.extend(plan_core_method_alias_fixes(
        &source_snapshot,
        snapshot_file,
        &selected_definitions,
        *rule,
      )?);
    }
  }
  if selected_rules.contains(&CORE_REF_CONSTRUCTOR_RULE) {
    suggestions.extend(plan_core_ref_constructor_fixes(
      &source_snapshot,
      snapshot_file,
      &selected_definitions,
    )?);
  }
  let mut constructor_kinds = Vec::new();
  if selected_rules.contains(&NAMED_ENUM_CONSTRUCTOR_RULE) {
    constructor_kinds.push(NominalKind::Enum);
  }
  if selected_rules.contains(&NAMED_STRUCT_CONSTRUCTOR_RULE) {
    constructor_kinds.push(NominalKind::Struct);
  }
  if options.include_attached {
    suggestions.extend(plan_attached_fixes(
      &source_snapshot,
      snapshot_file,
      &selected_definitions,
      &selected_rules,
    )?);
  }
  let compose_redundant_do = selected_rules.contains(&REDUNDANT_DO_RULE);
  let compose_single_expression_do = selected_rules.contains(&SINGLE_EXPRESSION_DO_RULE);
  let mut constructor_regions = Vec::new();
  if !constructor_kinds.is_empty() {
    let constructor_suggestions = plan_named_constructor_fixes(
      &source_snapshot,
      snapshot_file,
      &selected_definitions,
      &constructor_kinds,
      compose_redundant_do,
      compose_single_expression_do,
    )?;
    constructor_regions.extend(
      constructor_suggestions
        .iter()
        .map(|suggestion| (suggestion.definition.clone(), suggestion.target_path.clone())),
    );
    suggestions.extend(constructor_suggestions);
  }
  let redundant_do_suggestions = if compose_redundant_do {
    plan_redundant_do_fixes(&source_snapshot, snapshot_file, &selected_definitions, &constructor_regions)?
  } else {
    vec![]
  };
  let redundant_do_regions = redundant_do_suggestions
    .iter()
    .map(|suggestion| (suggestion.definition.clone(), suggestion.target_path.clone()))
    .collect::<Vec<_>>();
  if compose_single_expression_do {
    suggestions.extend(plan_single_expression_do_fixes(
      &source_snapshot,
      snapshot_file,
      &selected_definitions,
      &constructor_regions,
      &redundant_do_regions,
    )?);
  }
  suggestions.extend(redundant_do_suggestions);
  if compose_single_expression_do {
    suggestions.sort_by(|left, right| {
      left
        .definition
        .cmp(&right.definition)
        .then_with(|| right.target_path.cmp(&left.target_path))
        .then_with(|| left.rule_id.cmp(right.rule_id))
    });
  }
  let operations = suggestions.iter().flat_map(suggestion_operations).collect::<Vec<_>>();

  if validation_only {
    if suggestions.iter().any(|suggestion| suggestion.operation.is_some()) {
      return Err("Staged fix validation found an applicable migration that was not removed.".to_owned());
    }
    let unexpected = if semantic_refactor || schema_synthesis {
      println!(
        "{}",
        serde_json::to_string(semantic_warning_identities.as_deref().unwrap_or_default())
          .map_err(|error| format!("Failed to encode staged semantic-rename warnings: {error}"))?
      );
      vec![]
    } else if options.workflow.is_some() {
      vec![]
    } else {
      warnings
        .iter()
        .filter(|warning| warning.code() != Some(REMOVED_DATA_API_DIAGNOSTIC))
        .map(ToString::to_string)
        .collect::<Vec<_>>()
    };
    if !unexpected.is_empty() {
      return Err(format!(
        "Staged fix validation produced unrelated compiler warnings:\n{}",
        unexpected.join("\n")
      ));
    }
    return Ok(());
  }

  if options.apply && !operations.is_empty() {
    guard_git_worktree(snapshot_file, options.allow_dirty, options.allow_no_vcs)?;
  }
  let dry_run = !options.apply;
  let validation_args = fix_scope_args(options);
  let transaction = run_staged_fix_transaction(
    Path::new(snapshot_file),
    &operations,
    Some(&revision),
    dry_run,
    compiled_snapshot.active_entry_name(),
    &validation_args,
    semantic_warning_identities.as_deref(),
  )?;

  let mode = if options.verify {
    "verify"
  } else if options.apply {
    "apply"
  } else {
    "preview"
  };
  let mut workflow_failed = false;
  let workflow = if options.workflow.as_deref() == Some("strict") {
    let commands = strict_workflow_commands(snapshot_file, &source_snapshot);
    let type_evidence = strict_workflow_type_findings(&source_snapshot)?;
    let mut active_entries = source_snapshot.entries.keys().cloned().collect::<Vec<_>>();
    active_entries.sort();
    let preflight = crate::verification::collect_preflight(&source_snapshot, snapshot_file, &revision, &active_entries)?;
    let results = if options.verify {
      run_strict_workflow_verification(&commands)?
    } else {
      Vec::new()
    };
    workflow_failed = options.verify
      && (!boundary_diagnostics.is_empty()
        || !operations.is_empty()
        || preflight.is_failed()
        || results.iter().any(|result| result.status != "passed"));
    let resume_revision = if options.apply {
      transaction.new_revision.clone()
    } else {
      transaction.original_revision.clone()
    };
    Some(StrictWorkflowManifest {
      workflow: "strict-v1",
      mode,
      status: if workflow_failed {
        "failed"
      } else if !boundary_diagnostics.is_empty() {
        "requires-review"
      } else if options.verify {
        "passed"
      } else if options.apply {
        "applied"
      } else {
        "planned"
      },
      entries: strict_workflow_entries(&source_snapshot),
      safe_fixes: StrictWorkflowSafeFixes {
        preset: SURFACE_LATEST_V2_PRESET,
        suggestions: operations.len(),
        status: if operations.is_empty() {
          "clear"
        } else if options.apply {
          "applied"
        } else {
          "pending"
        },
      },
      review_required: StrictWorkflowReviewRequired {
        source_fixes: suggestions
          .iter()
          .filter(|suggestion| suggestion.operation.is_none())
          .map(|suggestion| StrictWorkflowSourceReview {
            rule_id: suggestion.rule_id,
            definition: suggestion.definition.clone(),
            path: suggestion.path.clone(),
            message: suggestion.message.clone(),
          })
          .collect(),
        type_findings: type_evidence.review_required,
        ffi_boundaries: type_evidence.ffi_boundaries,
        schema_candidates: type_evidence.schema_candidates,
        structural_candidates: type_evidence.structural_candidates,
      },
      retained_type_boundaries: type_evidence.retained_boundaries,
      preflight,
      verification: StrictWorkflowVerification {
        commands: commands
          .iter()
          .map(|(_, arguments)| std::iter::once("calcit".to_owned()).chain(arguments.iter().cloned()).collect())
          .collect(),
        results,
        external_commands: Vec::new(),
      },
      resume: StrictWorkflowResume {
        revision: resume_revision.clone(),
        apply_command: vec![
          "calcit".to_owned(),
          snapshot_file.to_owned(),
          "fix".to_owned(),
          "--workflow".to_owned(),
          "strict".to_owned(),
          "--apply".to_owned(),
          "--expect-revision".to_owned(),
          resume_revision,
          "--format".to_owned(),
          "edn".to_owned(),
        ],
      },
    })
  } else {
    None
  };
  let expanded_rules = selected_rules.iter().copied().map(fix_rule_metadata).collect();
  let report = FixReport {
    schema_version: 1,
    command: "fix",
    revision: &transaction.original_revision,
    data: FixReportData {
      mode,
      filters: FixFilters {
        namespace: options.ns.as_deref(),
        definition: options.definition.as_deref(),
        replacement_name: options.replacement_name.as_deref(),
        rule_id: options
          .rule
          .as_deref()
          .unwrap_or(if options.workflow.is_some() { "workflow:strict" } else { "all" }),
        preset_id: options.preset.as_deref(),
        expanded_rule_ids: selected_rules,
        expanded_rules,
        source_coverage: (options.include_attached
          || options.workflow.is_some()
          || matches!(options.preset.as_deref(), Some(CORE_API_028_V1_PRESET | CORE_API_029_V1_PRESET))
          || matches!(
            options.rule.as_deref(),
            Some(
              SPREAD_CALL_PROOF_RULE
                | UNSAFE_COERCE_BOUNDARY_RULE
                | ASSERT_TYPE_PROOF_RULE
                | CONCRETE_RETURN_PROOF_RULE
                | CALLABLE_CONTRACT_PROOF_RULE
                | NOMINAL_WRITE_PROOF_RULE
            )
          ))
        .then_some(FixSourceCoverage {
          scanned_regions: if options.include_attached {
            &["code", "tests", "examples"]
          } else {
            &["code"]
          },
          manual_review_regions: if options.include_attached { &[] } else { &["tests", "examples"] },
        }),
      },
      changed: transaction.changed,
      new_revision: &transaction.new_revision,
      validation: FixValidation {
        status: if !boundary_diagnostics.is_empty() {
          "requires-review"
        } else if operations.is_empty() {
          "not-needed"
        } else {
          "passed"
        },
        staged_scope_preprocess: !operations.is_empty(),
        checked_operations: operations.len(),
      },
      suggestions: &suggestions,
      workflow,
    },
    diagnostics: boundary_diagnostics,
    next: vec![],
  };

  emit_fix_report(&report, &options.format)?;
  if !report.diagnostics.is_empty() {
    Err(if options.apply && transaction.changed {
      "Compiler proof review required; only the reported safe migrations were applied. Review the remaining input evidence or lexical boundary; no proof repair, cast or FFI permission was inserted."
    } else {
      "Compiler proof review required; the source is unchanged. Review the reported input evidence or lexical boundary without automatically adding casts or granting FFI permission."
    }.to_owned())
  } else if workflow_failed {
    Err("Strict project workflow verification failed; inspect the structured workflow results.".to_owned())
  } else {
    Ok(())
  }
}

fn emit_fix_report(report: &FixReport<'_>, format: &str) -> Result<(), String> {
  match StructuredOutputFormat::parse(format, "fix")? {
    StructuredOutputFormat::Json => println!(
      "{}",
      serde_json::to_string(report).map_err(|error| format!("Failed to serialize fix result: {error}"))?
    ),
    StructuredOutputFormat::Edn => {
      let value = serde_json::to_value(report).map_err(|error| format!("Failed to encode fix result: {error}"))?;
      println!("{}", format_json_value_as_edn(&value)?);
    }
    StructuredOutputFormat::Human => print_human_report(report),
  }
  Ok(())
}

struct FixTargetScope(Option<crate::snapshot::SnapshotTarget>);

impl FixTargetScope {
  fn new(whole_project: bool) -> Option<Self> {
    if whole_project {
      let previous = program::active_entry_target();
      program::configure_entry_target(None);
      Some(Self(previous))
    } else {
      None
    }
  }
}

impl Drop for FixTargetScope {
  fn drop(&mut self) {
    program::configure_entry_target(self.0);
  }
}

/// Recreate the exact selection arguments for staged post-fix validation.
fn fix_scope_args(options: &FixCommand) -> Vec<String> {
  let mut args = Vec::new();
  if options.include_attached {
    args.push("--include-attached".to_owned());
  }
  if let Some(namespace) = &options.ns {
    args.push("--ns".to_owned());
    args.push(namespace.clone());
  }
  if let Some(definition) = &options.definition {
    args.push("--def".to_owned());
    args.push(definition.clone());
  }
  if let Some(rule) = &options.rule {
    args.push("--rule".to_owned());
    args.push(rule.clone());
  }
  if let Some(preset) = &options.preset {
    args.push("--preset".to_owned());
    args.push(preset.clone());
  }
  if let Some(workflow) = &options.workflow {
    args.push("--workflow".to_owned());
    args.push(workflow.clone());
  }
  if let Some(replacement) = &options.replacement_name {
    args.push("--to".to_owned());
    args.push(replacement.clone());
  }
  if let Some(at) = &options.at {
    args.push("--at".to_owned());
    args.push(at.clone());
  }
  args
}

/// Reject ambiguous modes, incomplete scopes, and unknown stable rule IDs.
fn validate_options(options: &FixCommand) -> Result<(), String> {
  StructuredOutputFormat::parse(&options.format, "fix")?;
  if options.include_attached {
    let selected = selected_rule_ids(options);
    let unsupported = selected
      .iter()
      .copied()
      .filter(|rule| !supports_attached_migrations(rule))
      .collect::<Vec<_>>();
    if selected.is_empty() || !unsupported.is_empty() {
      return Err(format!(
        "`--include-attached` requires a fully supported explicit rule or preset; attached coverage is not implemented for: {}.",
        if unsupported.is_empty() {
          "the selected rule".to_owned()
        } else {
          unsupported.join(", ")
        }
      ));
    }
  }
  if let Some(workflow) = options.workflow.as_deref()
    && workflow != "strict"
  {
    return Err(format!("Unknown fix workflow `{workflow}`. Available workflows: `strict`."));
  }
  if options.workflow.is_none() && options.verify {
    return Err("`calcit fix --verify` requires `--workflow strict`.".to_owned());
  }
  if options.workflow.is_some() && options.apply && options.expect_revision.is_none() {
    return Err("`calcit fix --workflow strict --apply` requires `--expect-revision` from a reviewed workflow plan.".to_owned());
  }
  if options.workflow.is_some()
    && (options.ns.is_some()
      || options.definition.is_some()
      || options.rule.is_some()
      || options.preset.is_some()
      || options.replacement_name.is_some())
  {
    return Err(
      "`calcit fix --workflow strict` is project-scoped and conflicts with --ns, --def, --rule, --preset, and --to.".to_owned(),
    );
  }
  if options.verify && (options.apply || options.dry_run || options.allow_dirty || options.allow_no_vcs) {
    return Err(
      "`calcit fix --workflow strict --verify` is read-only and conflicts with --apply, --dry-run, --allow-dirty, and --allow-no-vcs."
        .to_owned(),
    );
  }
  if options.apply && options.dry_run {
    return Err("`calcit fix --apply` conflicts with `--dry-run`; omit both flags to preview.".to_owned());
  }
  if options.definition.is_some() && options.ns.is_none() {
    return Err("`calcit fix --def` requires an exact `--ns` scope.".to_owned());
  }
  if options.rule.is_some() && options.preset.is_some() {
    return Err("`calcit fix --rule` conflicts with `--preset`; choose one explicit migration selection.".to_owned());
  }
  let semantic_rename = options.rule.as_deref() == Some(RENAME_DEFINITION_RULE);
  let local_rename = options.rule.as_deref() == Some(RENAME_LOCAL_RULE);
  if local_rename {
    if options.ns.is_none() || options.definition.is_none() || options.at.is_none() || options.replacement_name.is_none() {
      return Err(format!(
        "Fix rule `{RENAME_LOCAL_RULE}` requires exact `--ns`, `--def`, `--at <binding path>`, and `--to` arguments."
      ));
    }
    super::common::parse_path(options.at.as_deref().expect("checked --at"))?;
  } else if options.at.is_some() {
    return Err(format!("`calcit fix --at` is only valid with `--rule {RENAME_LOCAL_RULE}`."));
  }
  let value_to_zero_arg_fn = options.rule.as_deref() == Some(VALUE_TO_ZERO_ARG_FN_RULE);
  let schema_synthesis = options.rule.as_deref() == Some(SYNTHESIZE_SCHEMA_RULE);
  let optional_parameters = options.rule.as_deref() == Some(OPTIONAL_PARAMETERS_RULE);
  if optional_parameters && options.apply {
    return Err(format!(
      "Fix rule `{OPTIONAL_PARAMETERS_RULE}` is review-only until body and caller semantics can be proven; omit `--apply` to inspect the migration evidence."
    ));
  }
  if semantic_rename {
    if options.ns.is_none() || options.definition.is_none() || options.replacement_name.is_none() {
      return Err(format!(
        "Fix rule `{RENAME_DEFINITION_RULE}` requires exact `--ns`, `--def`, and `--to` arguments."
      ));
    }
    let replacement = options.replacement_name.as_deref().expect("checked replacement name");
    if replacement.is_empty() || replacement.contains('/') {
      return Err("`calcit fix --to` must be one non-empty unqualified definition name.".to_owned());
    }
  } else if value_to_zero_arg_fn || schema_synthesis || optional_parameters {
    if options.ns.is_none() || options.definition.is_none() {
      return Err(format!(
        "Fix rule `{}` requires exact `--ns` and `--def` arguments.",
        options.rule.as_deref().expect("checked exact schema/refactor rule")
      ));
    }
    if options.replacement_name.is_some() {
      return Err(format!(
        "Fix rule `{}` does not accept `--to`.",
        options.rule.as_deref().expect("checked exact schema/refactor rule")
      ));
    }
  } else if options.replacement_name.is_some() && !local_rename {
    return Err(format!(
      "`calcit fix --to` is only valid with `--rule {RENAME_DEFINITION_RULE}` or `--rule {RENAME_LOCAL_RULE}`."
    ));
  }
  if let Some(preset) = options.preset.as_deref()
    && !matches!(
      preset,
      SURFACE_LATEST_V1_PRESET | SURFACE_LATEST_V2_PRESET | CORE_API_028_V1_PRESET | CORE_API_029_V1_PRESET
    )
  {
    return Err(format!(
      "Unknown fix preset `{preset}`. Available presets: `{SURFACE_LATEST_V1_PRESET}`, `{SURFACE_LATEST_V2_PRESET}`, `{CORE_API_028_V1_PRESET}`, `{CORE_API_029_V1_PRESET}`."
    ));
  }
  if let Some(rule) = options.rule.as_deref()
    && RETIRED_CORE_API_028_RULES.contains(&rule)
  {
    return Err(format!(
      "Fix rule `{rule}` was retired in 0.29.0 together with the core aliases it migrated. Run it with a 0.28.x Calcit CLI before upgrading; in this CLI, strict checks report `E_RETIRED_METHOD` for a remaining method call and an unknown-name warning with the preferred spelling for a remaining function call."
    ));
  }
  if let Some(rule) = options.rule.as_deref()
    && !matches!(
      rule,
      REMOVED_DATA_API_RULE
        | REDUNDANT_DO_RULE
        | SINGLE_EXPRESSION_DO_RULE
        | NAMED_ENUM_CONSTRUCTOR_RULE
        | NAMED_STRUCT_CONSTRUCTOR_RULE
        | CORE_NOMINAL_CONSTRUCTOR_RULE
        | CORE_OPTION_METHOD_RULE
        | CORE_RESULT_METHOD_RULE
        | CORE_FUNCTION_ALIAS_RULE
        | CORE_INTEGER_PREDICATE_RULE
        | CORE_IDENTITY_CONVERSION_RULE
        | CORE_LIST_ADD_RULE
        | CORE_COLLECTION_LEN_RULE
        | CORE_EFFECT_METHOD_RULE
        | CORE_REF_CONSTRUCTOR_RULE
        | RENAME_DEFINITION_RULE
        | RENAME_LOCAL_RULE
        | VALUE_TO_ZERO_ARG_FN_RULE
        | SYNTHESIZE_SCHEMA_RULE
        | SPREAD_CALL_PROOF_RULE
        | UNSAFE_COERCE_BOUNDARY_RULE
        | ASSERT_TYPE_PROOF_RULE
        | CONCRETE_RETURN_PROOF_RULE
        | CALLABLE_CONTRACT_PROOF_RULE
        | NOMINAL_WRITE_PROOF_RULE
        | OPTIONAL_PARAMETERS_RULE
        | TAG_MATCH_RULE
        | REQUIRED_STRUCT_FIELD_RULE
    )
  {
    return Err(
      format!(
        "Unknown fix rule `{rule}`. Available rules: `{REMOVED_DATA_API_RULE}`, `{REDUNDANT_DO_RULE}`, `{SINGLE_EXPRESSION_DO_RULE}`, `{NAMED_ENUM_CONSTRUCTOR_RULE}`, `{NAMED_STRUCT_CONSTRUCTOR_RULE}`, `{CORE_OPTION_METHOD_RULE}`, `{CORE_RESULT_METHOD_RULE}`, `{CORE_INTEGER_PREDICATE_RULE}`, `{CORE_FUNCTION_ALIAS_RULE}`, `{CORE_IDENTITY_CONVERSION_RULE}`, `{CORE_LIST_ADD_RULE}`, `{CORE_COLLECTION_LEN_RULE}`, `{CORE_EFFECT_METHOD_RULE}`, `{CORE_REF_CONSTRUCTOR_RULE}`, `{RENAME_DEFINITION_RULE}`, `{RENAME_LOCAL_RULE}`, `{VALUE_TO_ZERO_ARG_FN_RULE}`, `{SYNTHESIZE_SCHEMA_RULE}`, `{SPREAD_CALL_PROOF_RULE}`, `{OPTIONAL_PARAMETERS_RULE}`. The retired 0.14.x migration bridge rules are `{TAG_MATCH_RULE}` and `{REQUIRED_STRUCT_FIELD_RULE}`."
      ) + &format!(
        " Review-only compiler rules: `{UNSAFE_COERCE_BOUNDARY_RULE}`, `{ASSERT_TYPE_PROOF_RULE}`, `{CONCRETE_RETURN_PROOF_RULE}`, `{CALLABLE_CONTRACT_PROOF_RULE}`, `{NOMINAL_WRITE_PROOF_RULE}`."
      ),
    );
  }
  if let Some(rule @ (TAG_MATCH_RULE | REQUIRED_STRUCT_FIELD_RULE)) = options.rule.as_deref() {
    return Err(format!(
      "Fix rule `{rule}` belongs to the published 0.14.x migration bridge and is not part of the 0.15 surface. Run it with Calcit 0.14.15 before upgrading, then retry the 0.15 check."
    ));
  }
  Ok(())
}

/// Expand one explicit rule or versioned preset into a deterministic rule sequence.
fn selected_rule_ids(options: &FixCommand) -> Vec<&'static str> {
  if options.workflow.as_deref() == Some("strict") {
    return SURFACE_LATEST_V2_RULES
      .iter()
      .copied()
      .chain([
        SPREAD_CALL_PROOF_RULE,
        UNSAFE_COERCE_BOUNDARY_RULE,
        ASSERT_TYPE_PROOF_RULE,
        CONCRETE_RETURN_PROOF_RULE,
        CALLABLE_CONTRACT_PROOF_RULE,
        NOMINAL_WRITE_PROOF_RULE,
      ])
      .collect();
  }
  if let Some(rule) = options.rule.as_deref() {
    if matches!(
      rule,
      RENAME_DEFINITION_RULE
        | RENAME_LOCAL_RULE
        | VALUE_TO_ZERO_ARG_FN_RULE
        | SYNTHESIZE_SCHEMA_RULE
        | SPREAD_CALL_PROOF_RULE
        | UNSAFE_COERCE_BOUNDARY_RULE
        | ASSERT_TYPE_PROOF_RULE
        | CONCRETE_RETURN_PROOF_RULE
        | CALLABLE_CONTRACT_PROOF_RULE
        | NOMINAL_WRITE_PROOF_RULE
        | OPTIONAL_PARAMETERS_RULE
        | CORE_NOMINAL_CONSTRUCTOR_RULE
        | CORE_OPTION_METHOD_RULE
        | CORE_RESULT_METHOD_RULE
        | CORE_FUNCTION_ALIAS_RULE
        | CORE_INTEGER_PREDICATE_RULE
        | CORE_IDENTITY_CONVERSION_RULE
        | CORE_LIST_ADD_RULE
        | CORE_COLLECTION_LEN_RULE
        | CORE_EFFECT_METHOD_RULE
        | CORE_REF_CONSTRUCTOR_RULE
    ) {
      return vec![match rule {
        RENAME_DEFINITION_RULE => RENAME_DEFINITION_RULE,
        RENAME_LOCAL_RULE => RENAME_LOCAL_RULE,
        VALUE_TO_ZERO_ARG_FN_RULE => VALUE_TO_ZERO_ARG_FN_RULE,
        SYNTHESIZE_SCHEMA_RULE => SYNTHESIZE_SCHEMA_RULE,
        SPREAD_CALL_PROOF_RULE => SPREAD_CALL_PROOF_RULE,
        UNSAFE_COERCE_BOUNDARY_RULE => UNSAFE_COERCE_BOUNDARY_RULE,
        ASSERT_TYPE_PROOF_RULE => ASSERT_TYPE_PROOF_RULE,
        CONCRETE_RETURN_PROOF_RULE => CONCRETE_RETURN_PROOF_RULE,
        CALLABLE_CONTRACT_PROOF_RULE => CALLABLE_CONTRACT_PROOF_RULE,
        NOMINAL_WRITE_PROOF_RULE => NOMINAL_WRITE_PROOF_RULE,
        CORE_NOMINAL_CONSTRUCTOR_RULE => CORE_NOMINAL_CONSTRUCTOR_RULE,
        CORE_OPTION_METHOD_RULE => CORE_OPTION_METHOD_RULE,
        CORE_RESULT_METHOD_RULE => CORE_RESULT_METHOD_RULE,
        CORE_FUNCTION_ALIAS_RULE => CORE_FUNCTION_ALIAS_RULE,
        CORE_INTEGER_PREDICATE_RULE => CORE_INTEGER_PREDICATE_RULE,
        CORE_IDENTITY_CONVERSION_RULE => CORE_IDENTITY_CONVERSION_RULE,
        CORE_LIST_ADD_RULE => CORE_LIST_ADD_RULE,
        CORE_COLLECTION_LEN_RULE => CORE_COLLECTION_LEN_RULE,
        CORE_EFFECT_METHOD_RULE => CORE_EFFECT_METHOD_RULE,
        CORE_REF_CONSTRUCTOR_RULE => CORE_REF_CONSTRUCTOR_RULE,
        _ => OPTIONAL_PARAMETERS_RULE,
      }];
    }
    return AVAILABLE_RULES.iter().copied().filter(|candidate| *candidate == rule).collect();
  }
  match options.preset.as_deref() {
    Some(SURFACE_LATEST_V1_PRESET) => SURFACE_LATEST_V1_RULES.to_vec(),
    Some(SURFACE_LATEST_V2_PRESET) => SURFACE_LATEST_V2_RULES.to_vec(),
    Some(CORE_API_028_V1_PRESET) => CORE_API_028_V1_RULES.to_vec(),
    Some(CORE_API_029_V1_PRESET) => CORE_API_029_V1_RULES.to_vec(),
    _ => AVAILABLE_RULES.to_vec(),
  }
}

/// Describe how a current normalization rule is derived without coupling it to a source release.
fn fix_rule_metadata(rule_id: &'static str) -> FixRuleMetadata {
  match rule_id {
    CALLABLE_CONTRACT_PROOF_RULE | NOMINAL_WRITE_PROOF_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: "E_CALL_ARGUMENT_UNPROVEN",
      evidence_source: "current-diagnostic",
      lifecycle: "current-semantics",
      source_version_required: false,
    },
    CONCRETE_RETURN_PROOF_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: CONCRETE_RETURN_PROOF_DIAGNOSTIC,
      evidence_source: "current-diagnostic",
      lifecycle: "current-semantics",
      source_version_required: false,
    },
    ASSERT_TYPE_PROOF_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: ASSERT_TYPE_PROOF_DIAGNOSTIC,
      evidence_source: "current-diagnostic",
      lifecycle: "current-semantics",
      source_version_required: false,
    },
    UNSAFE_COERCE_BOUNDARY_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: UNSAFE_COERCE_BOUNDARY_DIAGNOSTIC,
      evidence_source: "current-diagnostic",
      lifecycle: "current-semantics",
      source_version_required: false,
    },
    REMOVED_DATA_API_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: REMOVED_DATA_API_DIAGNOSTIC,
      evidence_source: "current-diagnostic",
      lifecycle: "current-semantics",
      source_version_required: false,
    },
    REDUNDANT_DO_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: REDUNDANT_DO_DIAGNOSTIC,
      evidence_source: "resolved-source-ast",
      lifecycle: "current-semantics",
      source_version_required: false,
    },
    SINGLE_EXPRESSION_DO_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: SINGLE_EXPRESSION_DO_DIAGNOSTIC,
      evidence_source: "resolved-source-ast",
      lifecycle: "current-semantics",
      source_version_required: false,
    },
    NAMED_ENUM_CONSTRUCTOR_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: NAMED_ENUM_CONSTRUCTOR_DIAGNOSTIC,
      evidence_source: "resolved-source-ast",
      lifecycle: "current-semantics",
      source_version_required: false,
    },
    NAMED_STRUCT_CONSTRUCTOR_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: NAMED_STRUCT_CONSTRUCTOR_DIAGNOSTIC,
      evidence_source: "resolved-source-ast",
      lifecycle: "current-semantics",
      source_version_required: false,
    },
    CORE_NOMINAL_CONSTRUCTOR_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: CORE_NOMINAL_CONSTRUCTOR_DIAGNOSTIC,
      evidence_source: "compiler-resolved-reference",
      lifecycle: "semantic-refactor",
      source_version_required: false,
    },
    CORE_OPTION_METHOD_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: CORE_OPTION_METHOD_DIAGNOSTIC,
      evidence_source: "compiler-resolved-reference-and-proven-receiver-method",
      lifecycle: "semantic-refactor",
      source_version_required: false,
    },
    CORE_RESULT_METHOD_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: CORE_RESULT_METHOD_DIAGNOSTIC,
      evidence_source: "compiler-resolved-reference-and-proven-receiver-method",
      lifecycle: "semantic-refactor",
      source_version_required: false,
    },
    CORE_FUNCTION_ALIAS_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: CORE_FUNCTION_ALIAS_DIAGNOSTIC,
      evidence_source: "compiler-resolved-reference",
      lifecycle: "semantic-refactor",
      source_version_required: false,
    },
    CORE_INTEGER_PREDICATE_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: CORE_INTEGER_PREDICATE_DIAGNOSTIC,
      evidence_source: "reader-resolved-builtin-proc",
      lifecycle: "semantic-refactor",
      source_version_required: false,
    },
    CORE_IDENTITY_CONVERSION_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: CORE_IDENTITY_CONVERSION_DIAGNOSTIC,
      evidence_source: "reader-resolved-builtin-proc-and-proven-conversion-argument",
      lifecycle: "semantic-refactor",
      source_version_required: false,
    },
    CORE_LIST_ADD_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: CORE_LIST_ADD_DIAGNOSTIC,
      evidence_source: "proven-receiver-and-method-implementation",
      lifecycle: "semantic-refactor",
      source_version_required: false,
    },
    CORE_COLLECTION_LEN_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: CORE_COLLECTION_LEN_DIAGNOSTIC,
      evidence_source: "proven-builtin-receiver-and-method-implementation",
      lifecycle: "semantic-refactor",
      source_version_required: false,
    },
    CORE_EFFECT_METHOD_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: CORE_EFFECT_METHOD_DIAGNOSTIC,
      evidence_source: "proven-core-nominal-receiver-and-identical-method-implementation",
      lifecycle: "semantic-refactor",
      source_version_required: false,
    },
    CORE_REF_CONSTRUCTOR_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: CORE_REF_CONSTRUCTOR_DIAGNOSTIC,
      evidence_source: "reader-resolved-builtin-proc-and-unshadowed-core-syntax",
      lifecycle: "semantic-refactor",
      source_version_required: false,
    },
    RENAME_DEFINITION_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: RENAME_DEFINITION_DIAGNOSTIC,
      evidence_source: "compiler-resolved-reference",
      lifecycle: "semantic-refactor",
      source_version_required: false,
    },
    RENAME_LOCAL_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: RENAME_LOCAL_DIAGNOSTIC,
      evidence_source: "core-binding-scope-and-preprocessed-locals",
      lifecycle: "semantic-refactor",
      source_version_required: false,
    },
    VALUE_TO_ZERO_ARG_FN_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: VALUE_TO_ZERO_ARG_FN_DIAGNOSTIC,
      evidence_source: "compiler-resolved-reference",
      lifecycle: "semantic-refactor",
      source_version_required: false,
    },
    SYNTHESIZE_SCHEMA_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: SYNTHESIZE_SCHEMA_DIAGNOSTIC,
      evidence_source: "compiled-type-inference",
      lifecycle: "semantic-refactor",
      source_version_required: false,
    },
    SPREAD_CALL_PROOF_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: SPREAD_CALL_PROOF_DIAGNOSTIC,
      evidence_source: "compiler-fixed-call-proof",
      lifecycle: "semantic-refactor",
      source_version_required: false,
    },
    OPTIONAL_PARAMETERS_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: OPTIONAL_PARAMETERS_DIAGNOSTIC,
      evidence_source: "source-parameters-and-declared-schema",
      lifecycle: "review-required",
      source_version_required: false,
    },
    _ => unreachable!("selected fix rule must have current-semantics metadata"),
  }
}

fn select_project_definitions(snapshot: &Snapshot, project_namespaces: &HashSet<String>) -> Result<Vec<(String, String)>, String> {
  let mut selected = Vec::new();
  for namespace in project_namespaces {
    let file = snapshot
      .files
      .get(namespace)
      .ok_or_else(|| format!("Project namespace `{namespace}` is missing from the compiled snapshot."))?;
    selected.extend(file.defs.keys().map(|definition| (namespace.clone(), definition.clone())));
  }
  selected.sort();
  if selected.is_empty() {
    Err("Fix scope selected no project definitions; refusing to report an empty plan as success.".to_owned())
  } else {
    Ok(selected)
  }
}

/// Resolve an editable, deterministic definition list from the requested project scope.
fn select_definitions(
  options: &FixCommand,
  snapshot: &Snapshot,
  project_namespaces: &HashSet<String>,
) -> Result<Vec<(String, String)>, String> {
  if let Some(namespace) = options.ns.as_deref()
    && !project_namespaces.contains(namespace)
  {
    return Err(format!("Fix namespace `{namespace}` is not an editable project namespace."));
  }

  let mut selected = Vec::new();
  for namespace in project_namespaces {
    if options.ns.as_deref().is_some_and(|filter| namespace != filter) {
      continue;
    }
    let file = snapshot
      .files
      .get(namespace)
      .ok_or_else(|| format!("Project namespace `{namespace}` is missing from the compiled snapshot."))?;
    for definition in file.defs.keys() {
      if options.definition.as_deref().is_none_or(|filter| definition == filter) {
        selected.push((namespace.clone(), definition.clone()));
      }
    }
  }
  selected.sort();
  if selected.is_empty() {
    return Err("Fix scope selected no definitions; refusing to report an empty plan as success.".to_owned());
  }
  Ok(selected)
}

/// Preprocess selected definitions once and retain compiler evidence used by fix planners.
fn compile_selected_definitions(definitions: &[(String, String)]) -> Result<Vec<LocatedWarning>, String> {
  let warnings = RefCell::new(Vec::new());
  for (namespace, definition) in definitions {
    runner::preprocess::ensure_ns_def_compiled(namespace, definition, &warnings, &CallStackList::default())
      .map_err(|failure| {
        let hint = if failure.msg.starts_with("unknown ns/def in program:") {
          " Check each entry's declared modules with `calcit <snapshot> config modules --entry <entry>`, then add or install the missing dependency."
        } else {
          ""
        };
        format!("Failed to preprocess fix target {namespace}/{definition}: {failure}{hint}")
      })?;
  }
  Ok(warnings.into_inner())
}

/// Preprocess legacy source without strict rejection so fix planning can consume compiler warnings and type evidence.
/// The staged validation subprocess still uses strict mode after applying the planned operations.
fn with_legacy_migration_mode<T>(run: impl FnOnce() -> T) -> T {
  struct StrictTypesGuard(bool);

  impl Drop for StrictTypesGuard {
    fn drop(&mut self) {
      runner::preprocess::set_strict_types(self.0);
    }
  }

  let guard = StrictTypesGuard(runner::preprocess::is_strict_types_enabled());
  runner::preprocess::set_strict_types(false);
  let result = run();
  drop(guard);
  result
}

fn compile_selected_definitions_for_migration(definitions: &[(String, String)]) -> Result<Vec<LocatedWarning>, String> {
  with_legacy_migration_mode(|| compile_selected_definitions(definitions))
}

/// Serialize the pre-edit warning multiset for semantic-refactor validation.
fn warning_identities(warnings: &[LocatedWarning]) -> Vec<String> {
  let mut identities = warnings
    .iter()
    .map(|warning| serde_json::to_string(&warning.as_json()).expect("warning identity must serialize"))
    .collect::<Vec<_>>();
  identities.sort();
  identities
}

/// Build stable warning identities while treating the renamed declaration as the same source owner.
fn semantic_rename_warning_identities(warnings: &[LocatedWarning], target_ns: &str, old_name: &str, new_name: &str) -> Vec<String> {
  let old_target = format!("{target_ns}/{old_name}");
  let new_target = format!("{target_ns}/{new_name}");
  let normalize_text = |text: &str| text.replace(&old_target, "<rename-target>").replace(&new_target, "<rename-target>");
  let mut identities = warnings
    .iter()
    .map(|warning| {
      let mut value = warning.as_json();
      if let Some(message) = value.get_mut("message")
        && let Some(text) = message.as_str()
      {
        *message = Value::String(normalize_text(text));
      }
      if let Some(hint) = value.get_mut("hint")
        && let Some(text) = hint.as_str()
      {
        *hint = Value::String(normalize_text(text));
      }
      if value.pointer("/location/ns").and_then(Value::as_str) == Some(target_ns)
        && matches!(value.pointer("/location/def").and_then(Value::as_str), Some(name) if name == old_name || name == new_name)
        && let Some(definition) = value.pointer_mut("/location/def")
      {
        *definition = Value::String("<rename-target>".to_owned());
      }
      serde_json::to_string(&value).expect("warning identity must serialize")
    })
    .collect::<Vec<_>>();
  identities.sort();
  identities
}

fn plan_definition_rename(
  options: &FixCommand,
  snapshot: &Snapshot,
  snapshot_file: &str,
  project_definitions: &[(String, String)],
) -> Result<Vec<FixSuggestion>, String> {
  let target_ns = options.ns.as_deref().expect("semantic rename requires namespace");
  let old_name = options.definition.as_deref().expect("semantic rename requires definition");
  let new_name = options.replacement_name.as_deref().expect("semantic rename requires replacement");
  let target_file = snapshot
    .files
    .get(target_ns)
    .ok_or_else(|| format!("Semantic rename namespace `{target_ns}` is not an editable project namespace."))?;
  if !target_file.defs.contains_key(old_name) {
    if target_file.defs.contains_key(new_name) {
      if std::env::var("CALCIT_FIX_VALIDATE_ONLY").as_deref() == Ok("1") {
        validate_no_stale_attached_references(snapshot, project_definitions, target_ns, old_name, new_name)?;
      }
      return Ok(vec![]);
    }
    return Err(format!("Semantic rename source `{target_ns}/{old_name}` does not exist."));
  }
  if target_file.defs.contains_key(new_name) {
    return Err(format!("Semantic rename destination `{target_ns}/{new_name}` already exists."));
  }

  let warnings = RefCell::new(Vec::new());
  let mut resolved = BTreeMap::<(String, String, Vec<usize>), (String, Vec<String>)>::new();
  let mut attached_suggestions = Vec::new();
  let mut blockers = Vec::new();
  for (owner_ns, owner_def) in project_definitions {
    let entry = snapshot
      .files
      .get(owner_ns)
      .and_then(|file| file.defs.get(owner_def))
      .ok_or_else(|| format!("Project definition `{owner_ns}/{owner_def}` is missing from the source snapshot."))?;
    if list_head(&entry.code) == Some("defmacro") && cirru_contains_target_reference(&entry.code, owner_ns, target_ns, old_name) {
      blockers.push(format!(
        "{owner_ns}/{owner_def}: macro source may produce `{target_ns}/{old_name}` and cannot be rewritten from expanded output"
      ));
    }
    if quoted_region_contains_target_reference(&entry.code, owner_ns, target_ns, old_name) {
      blockers.push(format!(
        "{owner_ns}/{owner_def}: quoted source contains `{target_ns}/{old_name}` and may be consumed dynamically"
      ));
    }
    for (index, test) in entry.tests.iter().enumerate() {
      if !cirru_contains_target_reference(&test.code, owner_ns, target_ns, old_name) {
        continue;
      }
      let source_label = format!("{owner_ns}/{owner_def}#{}", test.name);
      match plan_attached_source_rewrite(
        &test.code,
        owner_ns,
        &format!("&calcit:rename-test:{owner_def}:{index}"),
        &source_label,
        target_ns,
        old_name,
        new_name,
      ) {
        Ok(Some(rewrite)) => {
          let mut tag_names = test.tags.iter().map(|tag| tag.ref_str()).collect::<Vec<_>>();
          tag_names.sort_unstable();
          attached_suggestions.push(FixSuggestion {
            rule_id: RENAME_DEFINITION_RULE,
            diagnostic_code: RENAME_DEFINITION_DIAGNOSTIC,
            semantic_layer: "surface",
            source_file: snapshot_file.to_owned(),
            definition: format!("{owner_ns}/{owner_def}"),
            path: format!("tests.{}", test.name),
            fingerprint: node_fingerprint(&test.code),
            origin_chain: rewrite.origin_chain,
            original: quoted_json(&test.code),
            replacement: Some(quoted_json(&rewrite.code)),
            applicability: "machine-applicable",
            message: format!("Rewrite compiler-resolved references in definition-attached test `{}`.", test.name),
            target_path: vec![],
            operation: Some(FixOperation::ReplaceTest {
              name: test.name.clone(),
              tags: tag_names.join(","),
              code: format_quoted_nodes(std::slice::from_ref(&rewrite.code))?,
            }),
          });
        }
        Ok(None) => {}
        Err(error) => blockers.push(error),
      }
    }
    let mut rewritten_examples = entry.examples.clone();
    let mut example_origins = Vec::new();
    let mut changed_examples = Vec::new();
    for (index, example) in entry.examples.iter().enumerate() {
      if !cirru_contains_target_reference(example, owner_ns, target_ns, old_name) {
        continue;
      }
      let source_label = format!("{owner_ns}/{owner_def} example {index}");
      match plan_attached_source_rewrite(
        example,
        owner_ns,
        &format!("&calcit:rename-example:{owner_def}:{index}"),
        &source_label,
        target_ns,
        old_name,
        new_name,
      ) {
        Ok(Some(rewrite)) => {
          rewritten_examples[index] = rewrite.code;
          example_origins.extend(rewrite.origin_chain);
          changed_examples.push(index);
        }
        Ok(None) => {}
        Err(error) => blockers.push(error),
      }
    }
    if !changed_examples.is_empty() {
      let original_examples = Cirru::List(entry.examples.clone());
      let replacement_examples = Cirru::List(rewritten_examples.clone());
      attached_suggestions.push(FixSuggestion {
        rule_id: RENAME_DEFINITION_RULE,
        diagnostic_code: RENAME_DEFINITION_DIAGNOSTIC,
        semantic_layer: "surface",
        source_file: snapshot_file.to_owned(),
        definition: format!("{owner_ns}/{owner_def}"),
        path: "examples".to_owned(),
        fingerprint: node_fingerprint(&original_examples),
        origin_chain: example_origins,
        original: quoted_json(&original_examples),
        replacement: Some(quoted_json(&replacement_examples)),
        applicability: "machine-applicable",
        message: format!("Rewrite compiler-resolved references in examples at indices {changed_examples:?}."),
        target_path: vec![],
        operation: Some(FixOperation::ReplaceExamples {
          code: format_quoted_nodes(&rewritten_examples)?,
        }),
      });
    }
    let (rewritten_annotation, replacement_count) =
      rewrite_loaded_schema_type_references(entry.schema.clone(), owner_ns, target_ns, old_name, new_name)?;
    if replacement_count > 0 {
      let schema_edn = calcit::snapshot::schema_annotation_to_edn(entry.schema.as_ref());
      let rewritten_schema = calcit::snapshot::schema_annotation_to_edn(rewritten_annotation.as_ref());
      let original_node = schema_edn_to_source_node(&schema_edn)?;
      let replacement_node = schema_edn_to_source_node(&rewritten_schema)?;
      attached_suggestions.push(FixSuggestion {
        rule_id: RENAME_DEFINITION_RULE,
        diagnostic_code: RENAME_DEFINITION_DIAGNOSTIC,
        semantic_layer: "surface",
        source_file: snapshot_file.to_owned(),
        definition: format!("{owner_ns}/{owner_def}"),
        path: "schema".to_owned(),
        fingerprint: node_fingerprint(&original_node),
        origin_chain: vec![serde_json::json!({
          "kind": "resolved-schema-type",
          "target": format!("{target_ns}/{old_name}"),
          "occurrences": replacement_count,
        })],
        original: quoted_json(&original_node),
        replacement: Some(quoted_json(&replacement_node)),
        applicability: "machine-applicable",
        message: format!("Rewrite {replacement_count} compiler-loaded schema type reference(s) to `{target_ns}/{new_name}`."),
        target_path: vec![],
        operation: Some(FixOperation::ReplaceSchema {
          code: format_quoted_nodes(std::slice::from_ref(&replacement_node))?,
        }),
      });
    }
    let usages = runner::preprocess::trace_definition_source_usages(owner_ns, owner_def, &warnings, &CallStackList::default())
      .map_err(|failure| failure.msg)?;
    for usage in usages {
      if usage.target_ns.as_ref() != target_ns || usage.target_def.as_ref() != old_name {
        continue;
      }
      if !usage.macro_origin.is_empty() {
        blockers.push(format!(
          "{owner_ns}/{owner_def}: target reference is produced across macro boundary {}",
          usage.macro_origin.join(" -> ")
        ));
        continue;
      }
      let Some(location) = usage.location else {
        blockers.push(format!(
          "{owner_ns}/{owner_def}: compiler resolved a target reference without a source coordinate"
        ));
        continue;
      };
      if location.ns.as_ref() != owner_ns || location.def.as_ref() != owner_def {
        blockers.push(format!(
          "{owner_ns}/{owner_def}: target reference points outside its editable source ({location})"
        ));
        continue;
      }
      let path = location.coord.iter().map(|value| usize::from(*value)).collect::<Vec<_>>();
      let entry = snapshot
        .files
        .get(owner_ns)
        .and_then(|file| file.defs.get(owner_def))
        .ok_or_else(|| format!("Traced definition `{owner_ns}/{owner_def}` is missing from the source snapshot."))?;
      let node = navigate_to_path(&entry.code, &path)?;
      let Cirru::Leaf(source_leaf) = node else {
        blockers.push(format!(
          "{owner_ns}/{owner_def}{}: resolved reference is not a source leaf",
          format_path(&path)
        ));
        continue;
      };
      let replacement = if let Some((prefix, source_name)) = source_leaf.rsplit_once('/') {
        if source_name != old_name {
          blockers.push(format!(
            "{owner_ns}/{owner_def}{}: resolved leaf `{source_leaf}` does not name `{old_name}`",
            format_path(&path)
          ));
          continue;
        }
        format!("{prefix}/{new_name}")
      } else if source_leaf.as_ref() == old_name {
        format!("{target_ns}/{new_name}")
      } else {
        blockers.push(format!(
          "{owner_ns}/{owner_def}{}: resolved leaf `{source_leaf}` does not name `{old_name}`",
          format_path(&path)
        ));
        continue;
      };
      resolved.insert(
        (owner_ns.clone(), owner_def.clone(), path),
        (source_leaf.to_string(), vec![replacement]),
      );
    }
  }
  if !blockers.is_empty() {
    blockers.sort();
    blockers.dedup();
    return Err(format!(
      "Semantic rename `{target_ns}/{old_name}` is not provably source-editable; no changes were written:\n- {}",
      blockers.join("\n- ")
    ));
  }

  let mut suggestions = Vec::new();
  suggestions.extend(attached_suggestions);
  for ((owner_ns, owner_def, target_path), (original_leaf, replacements)) in resolved {
    let replacement = replacements.into_iter().next().expect("one deterministic replacement");
    let original_node = Cirru::leaf(original_leaf.as_str());
    let replacement_node = Cirru::leaf(replacement.as_str());
    suggestions.push(FixSuggestion {
      rule_id: RENAME_DEFINITION_RULE,
      diagnostic_code: RENAME_DEFINITION_DIAGNOSTIC,
      semantic_layer: "surface",
      source_file: snapshot_file.to_owned(),
      definition: format!("{owner_ns}/{owner_def}"),
      path: format!("code{}", format_path(&target_path)),
      fingerprint: node_fingerprint(&original_node),
      origin_chain: vec![serde_json::json!({"kind": "resolved-definition", "target": format!("{target_ns}/{old_name}")})],
      original: quoted_json(&original_node),
      replacement: Some(quoted_json(&replacement_node)),
      applicability: "machine-applicable",
      message: format!("Rewrite the compiler-resolved reference to `{target_ns}/{new_name}`."),
      target_path,
      operation: Some(FixOperation::ReplaceLeaf {
        original: original_leaf,
        replacement,
      }),
    });
  }

  suggestions.extend(plan_referred_import_removals(
    snapshot,
    snapshot_file,
    target_ns,
    old_name,
    new_name,
  )?);

  let target_entry = target_file.defs.get(old_name).expect("checked semantic rename source");
  let (renamed_code, _) = super::edit::rename_definition_declaration(&target_entry.code, old_name, new_name)?;
  suggestions.push(FixSuggestion {
    rule_id: RENAME_DEFINITION_RULE,
    diagnostic_code: RENAME_DEFINITION_DIAGNOSTIC,
    semantic_layer: "surface",
    source_file: snapshot_file.to_owned(),
    definition: format!("{target_ns}/{old_name}"),
    path: "definition".to_owned(),
    fingerprint: node_fingerprint(&target_entry.code),
    origin_chain: vec![serde_json::json!({"kind": "definition", "target": format!("{target_ns}/{old_name}")})],
    original: quoted_json(&target_entry.code),
    replacement: Some(quoted_json(&renamed_code)),
    applicability: "machine-applicable",
    message: format!("Rename the definition and declaration to `{target_ns}/{new_name}`."),
    target_path: vec![],
    operation: Some(FixOperation::RenameDefinition {
      new_name: new_name.to_owned(),
    }),
  });
  Ok(suggestions)
}

mod compiler_review;
mod local_rename;
pub(crate) mod schema_synthesis;
mod spread_call;
mod structural_rewrite;
use schema_synthesis::plan_schema_synthesis;

/// Surface legacy omission markers from the unexpanded Snapshot without implying a safe rewrite.
fn plan_optional_parameter_diagnostics(
  options: &FixCommand,
  snapshot: &Snapshot,
  snapshot_file: &str,
  project_definitions: &[(String, String)],
) -> Result<Vec<FixSuggestion>, String> {
  let namespace = options.ns.as_deref().expect("optional parameter rule requires namespace");
  let definition = options.definition.as_deref().expect("optional parameter rule requires definition");
  let entry = snapshot
    .files
    .get(namespace)
    .and_then(|file| file.defs.get(definition))
    .ok_or_else(|| format!("Optional parameter source `{namespace}/{definition}` does not exist in the editable Snapshot."))?;
  let Cirru::List(code) = &entry.code else {
    return Err(format!(
      "Optional parameter source `{namespace}/{definition}` is not a definition form."
    ));
  };
  let head = code.first().and_then(leaf_value);
  if !matches!(head, Some("defn" | "defcomp" | "defn-js")) {
    return Err(format!(
      "Fix rule `{OPTIONAL_PARAMETERS_RULE}` requires a source function definition, found `{}` at `{namespace}/{definition}`.",
      head.unwrap_or("<non-leaf>")
    ));
  }
  let Some(Cirru::List(args)) = code.get(2) else {
    return Err(format!(
      "Source function `{namespace}/{definition}` has no inspectable argument list."
    ));
  };
  let signature = match entry.schema.as_ref() {
    CalcitTypeAnnotation::Fn(signature) => Some(signature.as_ref()),
    _ => None,
  };
  let mut optional = false;
  let mut argument_index = 0;
  let mut suggestions = Vec::new();
  let mut candidate_arg_types = signature.map(|signature| signature.arg_types.clone());
  for (source_index, arg) in args.iter().enumerate() {
    if arg.eq_leaf("?") {
      optional = true;
      continue;
    }
    if arg.eq_leaf("&") {
      optional = false;
      continue;
    }
    if optional {
      let declared = signature.and_then(|signature| signature.arg_types.get(argument_index));
      let candidate = declared
        .filter(|annotation| optional_parameter_schema_type(annotation).is_some())
        .and_then(|annotation| optional_parameter_candidate(annotation));
      candidate_arg_types = candidate_arg_types.and_then(|mut arg_types| {
        let candidate_type = declared.and_then(optional_parameter_schema_type)?;
        *arg_types.get_mut(argument_index)? = candidate_type;
        Some(arg_types)
      });
      let parameter = leaf_value(arg).unwrap_or("<destructured>");
      let reason = if candidate.is_some() {
        "declared-type-only; body presence checks, explicit nil, generated/external callers, and call evaluation are not yet proven"
      } else {
        "no precise declared parameter type; type and call semantics require review"
      };
      suggestions.push(FixSuggestion {
        rule_id: OPTIONAL_PARAMETERS_RULE,
        diagnostic_code: OPTIONAL_PARAMETERS_DIAGNOSTIC,
        semantic_layer: "surface",
        source_file: snapshot_file.to_owned(),
        definition: format!("{namespace}/{definition}"),
        path: format!("code{}", format_path(&[2, source_index])),
        fingerprint: node_fingerprint(arg),
        origin_chain: vec![serde_json::json!({
          "kind": "legacy-optional-parameter",
          "parameter": parameter,
          "argument_index": argument_index,
          "declared_type": declared.map(|annotation| annotation.to_brief_string()),
          "candidate_type": candidate,
          "reason": reason,
        })],
        original: quoted_json(arg),
        replacement: None,
        applicability: "needs-review",
        message: format!(
          "Legacy optional parameter `{parameter}` needs an explicit Option contract and coordinated caller/body review; {reason}."
        ),
        target_path: vec![2, source_index],
        operation: None,
      });
    }
    argument_index += 1;
  }
  if !suggestions.is_empty() {
    let candidate_fn_schema_edn = signature
      .zip(candidate_arg_types)
      .filter(|(signature, arg_types)| signature.arg_types.len() == argument_index && arg_types.len() == argument_index)
      .and_then(|(signature, arg_types)| {
        let mut candidate = signature.clone();
        candidate.arg_types = arg_types;
        if !optional_candidate_signature_is_closed(&candidate) {
          return None;
        }
        cirru_edn::format(&candidate.to_schema_edn(), true).ok()
      });
    let usage_evidence = collect_optional_parameter_usages(snapshot, namespace, definition, project_definitions);
    for suggestion in &mut suggestions {
      suggestion.origin_chain[0]["candidate_fn_schema_edn"] = serde_json::json!(candidate_fn_schema_edn.as_deref());
      suggestion.origin_chain.extend(usage_evidence.iter().cloned());
    }
  }
  Ok(suggestions)
}

/// Collect resolver-backed local source references; attached sources and external consumers remain outside this proof.
fn collect_optional_parameter_usages(
  snapshot: &Snapshot,
  target_ns: &str,
  target_def: &str,
  project_definitions: &[(String, String)],
) -> Vec<Value> {
  with_legacy_migration_mode(|| {
    let warnings = RefCell::new(Vec::new());
    let mut evidence = Vec::new();
    let mut failed_definitions = Vec::new();
    for (owner_ns, owner_def) in project_definitions {
      let Some(entry) = snapshot.files.get(owner_ns).and_then(|file| file.defs.get(owner_def)) else {
        failed_definitions.push(format!("{owner_ns}/{owner_def}"));
        continue;
      };
      let usages = match runner::preprocess::trace_definition_source_usages(owner_ns, owner_def, &warnings, &CallStackList::default()) {
        Ok(usages) => usages,
        Err(_) => {
          failed_definitions.push(format!("{owner_ns}/{owner_def}"));
          continue;
        }
      };
      for usage in usages {
        if usage.target_ns.as_ref() != target_ns || usage.target_def.as_ref() != target_def {
          continue;
        }
        let owner = format!("{owner_ns}/{owner_def}");
        if !usage.macro_origin.is_empty() {
          evidence.push(serde_json::json!({
            "kind": "macro-origin-reference",
            "definition": owner,
            "macro_origin": usage.macro_origin,
          }));
          continue;
        }
        let Some(location) = usage.location else {
          evidence.push(serde_json::json!({"kind": "unlocated-reference", "definition": owner}));
          continue;
        };
        if location.ns.as_ref() != owner_ns || location.def.as_ref() != owner_def {
          evidence.push(serde_json::json!({
            "kind": "outside-editable-source",
            "definition": owner,
            "location": location.to_string(),
          }));
          continue;
        }
        let path = location.coord.iter().map(|value| usize::from(*value)).collect::<Vec<_>>();
        let parent = path
          .split_last()
          .and_then(|(index, parent_path)| navigate_to_path(&entry.code, parent_path).ok().map(|node| (*index, node)));
        let Some((index, Cirru::List(call))) = parent else {
          evidence.push(serde_json::json!({
            "kind": "unlocated-reference",
            "definition": owner,
            "path": format!("code{}", format_path(&path)),
          }));
          continue;
        };
        let call_kind = if index == 0 && !call.iter().skip(1).any(|item| item.eq_leaf("&")) {
          "direct-call"
        } else if index == 0 {
          "spread-call"
        } else {
          "function-value"
        };
        evidence.push(serde_json::json!({
          "kind": "resolved-project-reference",
          "definition": owner,
          "path": format!("code{}", format_path(&path)),
          "call_kind": call_kind,
          "provided_arguments": if call_kind == "direct-call" { Some(call.len().saturating_sub(1)) } else { None },
          "explicit_nil_arguments": if call_kind == "direct-call" {
            call.iter().skip(1).enumerate().filter_map(|(index, item)| item.eq_leaf("nil").then_some(index)).collect::<Vec<_>>()
          } else {
            Vec::new()
          },
          "explicit_false_arguments": if call_kind == "direct-call" {
            call.iter().skip(1).enumerate().filter_map(|(index, item)| item.eq_leaf("false").then_some(index)).collect::<Vec<_>>()
          } else {
            Vec::new()
          },
        }));
      }
    }
    evidence.sort_by_key(|value| value.to_string());
    evidence.insert(
      0,
      serde_json::json!({
        "kind": "project-reference-scan",
        "scope": "definition-code-only",
        "scanned_definitions": project_definitions.len(),
        "failed_definitions": failed_definitions,
        "external_consumers": "unproven",
        "attached_sources": "untraced",
      }),
    );
    evidence
  })
}

fn optional_parameter_candidate(annotation: &CalcitTypeAnnotation) -> Option<String> {
  match annotation {
    CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::DynFn => None,
    CalcitTypeAnnotation::Optional(inner) => optional_parameter_candidate(inner),
    CalcitTypeAnnotation::TypeRef(name, args) if matches!(name.as_ref(), "Option" | "calcit.core/Option") && args.len() == 1 => {
      if matches!(args[0].as_ref(), CalcitTypeAnnotation::Dynamic) {
        None
      } else {
        Some(annotation.to_brief_string())
      }
    }
    other => Some(format!("Option<{}>", other.to_brief_string())),
  }
}

/// Build a declaration-backed nominal Option slot without guessing through aliases or open types.
fn optional_parameter_schema_type(annotation: &std::sync::Arc<CalcitTypeAnnotation>) -> Option<std::sync::Arc<CalcitTypeAnnotation>> {
  let mut inner = annotation.as_ref();
  while let CalcitTypeAnnotation::Optional(next) = inner {
    inner = next.as_ref();
  }
  if !optional_candidate_type_is_closed(inner) {
    return None;
  }
  match inner {
    CalcitTypeAnnotation::TypeRef(name, args) if matches!(name.as_ref(), "Option" | "calcit.core/Option") && args.len() == 1 => {
      Some(std::sync::Arc::new(inner.clone()))
    }
    CalcitTypeAnnotation::TypeRef(..) | CalcitTypeAnnotation::TypeSlot(..) => None,
    _ => Some(std::sync::Arc::new(CalcitTypeAnnotation::TypeRef(
      "calcit.core/Option".into(),
      std::sync::Arc::new(vec![std::sync::Arc::new(inner.clone())]),
    ))),
  }
}

/// A review-only candidate must not preserve explicit open members in any signature position.
fn optional_candidate_signature_is_closed(signature: &CalcitFnTypeAnnotation) -> bool {
  signature.arg_types.iter().all(|arg| optional_candidate_type_is_closed(arg))
    && optional_candidate_type_is_closed(&signature.return_type)
    && signature
      .rest_type
      .as_ref()
      .is_none_or(|rest| optional_candidate_type_is_closed(rest))
}

fn optional_candidate_type_is_closed(annotation: &CalcitTypeAnnotation) -> bool {
  match annotation {
    CalcitTypeAnnotation::Dynamic
    | CalcitTypeAnnotation::DynFn
    | CalcitTypeAnnotation::AnonymousEnum
    | CalcitTypeAnnotation::Custom(_)
    | CalcitTypeAnnotation::TypeSlot(_)
    | CalcitTypeAnnotation::Macro(_)
    | CalcitTypeAnnotation::Syntax(_) => false,
    CalcitTypeAnnotation::List(inner)
    | CalcitTypeAnnotation::Set(inner)
    | CalcitTypeAnnotation::Ref(inner)
    | CalcitTypeAnnotation::Variadic(inner)
    | CalcitTypeAnnotation::Optional(inner)
    | CalcitTypeAnnotation::JsNullish(inner) => optional_candidate_type_is_closed(inner),
    CalcitTypeAnnotation::Map(key, value) => optional_candidate_type_is_closed(key) && optional_candidate_type_is_closed(value),
    CalcitTypeAnnotation::TypeRef(_, args) => {
      args.iter().all(|arg| optional_candidate_type_is_closed(arg))
        && (annotation.resolve_to_struct().is_some() || annotation.resolve_to_enum().is_some())
    }
    CalcitTypeAnnotation::Struct(_, args) | CalcitTypeAnnotation::Enum(_, args) => {
      args.iter().all(|arg| optional_candidate_type_is_closed(arg))
    }
    CalcitTypeAnnotation::Fn(signature) => optional_candidate_signature_is_closed(signature),
    _ => true,
  }
}

fn plan_value_to_zero_arg_fn(
  options: &FixCommand,
  snapshot: &Snapshot,
  snapshot_file: &str,
  project_definitions: &[(String, String)],
) -> Result<Vec<FixSuggestion>, String> {
  let target_ns = options.ns.as_deref().expect("value refactor requires namespace");
  let target_def = options.definition.as_deref().expect("value refactor requires definition");
  let target_entry = snapshot
    .files
    .get(target_ns)
    .and_then(|file| file.defs.get(target_def))
    .ok_or_else(|| format!("Semantic refactor source `{target_ns}/{target_def}` does not exist."))?;
  let Cirru::List(target_items) = &target_entry.code else {
    return Err(format!(
      "Semantic refactor source `{target_ns}/{target_def}` is not a definition form."
    ));
  };
  let target_head = target_items.first().and_then(leaf_value);
  if target_head == Some("defn") && matches!(target_items.get(2), Some(Cirru::List(args)) if args.is_empty()) {
    if std::env::var("CALCIT_FIX_VALIDATE_ONLY").as_deref() == Ok("1") {
      validate_zero_arg_value_refactor(snapshot, project_definitions, target_ns, target_def)?;
    }
    return Ok(vec![]);
  }
  if target_head != Some("def") || target_items.len() != 3 || !target_items.get(1).is_some_and(|name| name.eq_leaf(target_def)) {
    return Err(format!(
      "Fix rule `{VALUE_TO_ZERO_ARG_FN_RULE}` requires `{target_ns}/{target_def}` to be an exact `(def {target_def} value)` form."
    ));
  }

  let warnings = RefCell::new(Vec::new());
  let mut suggestions = Vec::new();
  let mut blockers = Vec::new();
  for (owner_ns, owner_def) in project_definitions {
    let entry = snapshot
      .files
      .get(owner_ns)
      .and_then(|file| file.defs.get(owner_def))
      .ok_or_else(|| format!("Project definition `{owner_ns}/{owner_def}` is missing from the source snapshot."))?;
    let direct_macro_reference =
      list_head(&entry.code) == Some("defmacro") && cirru_contains_target_reference(&entry.code, owner_ns, target_ns, target_def);
    if direct_macro_reference {
      blockers.push(format!(
        "{owner_ns}/{owner_def}: macro source may produce `{target_ns}/{target_def}` and cannot be converted as a runtime value read"
      ));
    }
    if quoted_region_contains_target_reference(&entry.code, owner_ns, target_ns, target_def) {
      blockers.push(format!(
        "{owner_ns}/{owner_def}: quoted source contains `{target_ns}/{target_def}` and may be consumed dynamically"
      ));
    }

    for (index, test) in entry.tests.iter().enumerate() {
      if !cirru_contains_target_reference(&test.code, owner_ns, target_ns, target_def) {
        continue;
      }
      let source_label = format!("{owner_ns}/{owner_def}#{}", test.name);
      match plan_attached_value_call_rewrite(
        &test.code,
        owner_ns,
        &format!("&calcit:value-to-fn-test:{owner_def}:{index}"),
        &source_label,
        target_ns,
        target_def,
      ) {
        Ok(Some(rewrite)) => {
          let mut tag_names = test.tags.iter().map(|tag| tag.ref_str()).collect::<Vec<_>>();
          tag_names.sort_unstable();
          suggestions.push(FixSuggestion {
            rule_id: VALUE_TO_ZERO_ARG_FN_RULE,
            diagnostic_code: VALUE_TO_ZERO_ARG_FN_DIAGNOSTIC,
            semantic_layer: "surface",
            source_file: snapshot_file.to_owned(),
            definition: format!("{owner_ns}/{owner_def}"),
            path: format!("tests.{}", test.name),
            fingerprint: node_fingerprint(&test.code),
            origin_chain: rewrite.origin_chain,
            original: quoted_json(&test.code),
            replacement: Some(quoted_json(&rewrite.code)),
            applicability: "machine-applicable",
            message: format!("Call the converted zero-argument function from attached test `{}`.", test.name),
            target_path: vec![],
            operation: Some(FixOperation::ReplaceTest {
              name: test.name.clone(),
              tags: tag_names.join(","),
              code: format_quoted_nodes(std::slice::from_ref(&rewrite.code))?,
            }),
          });
        }
        Ok(None) => {}
        Err(error) => blockers.push(error),
      }
    }

    let mut rewritten_examples = entry.examples.clone();
    let mut example_origins = Vec::new();
    let mut changed_examples = Vec::new();
    for (index, example) in entry.examples.iter().enumerate() {
      if !cirru_contains_target_reference(example, owner_ns, target_ns, target_def) {
        continue;
      }
      let source_label = format!("{owner_ns}/{owner_def} example {index}");
      match plan_attached_value_call_rewrite(
        example,
        owner_ns,
        &format!("&calcit:value-to-fn-example:{owner_def}:{index}"),
        &source_label,
        target_ns,
        target_def,
      ) {
        Ok(Some(rewrite)) => {
          rewritten_examples[index] = rewrite.code;
          example_origins.extend(rewrite.origin_chain);
          changed_examples.push(index);
        }
        Ok(None) => {}
        Err(error) => blockers.push(error),
      }
    }
    if !changed_examples.is_empty() {
      let original_examples = Cirru::List(entry.examples.clone());
      let replacement_examples = Cirru::List(rewritten_examples.clone());
      suggestions.push(FixSuggestion {
        rule_id: VALUE_TO_ZERO_ARG_FN_RULE,
        diagnostic_code: VALUE_TO_ZERO_ARG_FN_DIAGNOSTIC,
        semantic_layer: "surface",
        source_file: snapshot_file.to_owned(),
        definition: format!("{owner_ns}/{owner_def}"),
        path: "examples".to_owned(),
        fingerprint: node_fingerprint(&original_examples),
        origin_chain: example_origins,
        original: quoted_json(&original_examples),
        replacement: Some(quoted_json(&replacement_examples)),
        applicability: "machine-applicable",
        message: format!("Call the converted zero-argument function from examples at indices {changed_examples:?}."),
        target_path: vec![],
        operation: Some(FixOperation::ReplaceExamples {
          code: format_quoted_nodes(&rewritten_examples)?,
        }),
      });
    }

    let (_, schema_reference_count) =
      rewrite_loaded_schema_type_references(entry.schema.clone(), owner_ns, target_ns, target_def, target_def)?;
    if schema_reference_count > 0 {
      blockers.push(format!(
        "{owner_ns}/{owner_def} schema: `{target_ns}/{target_def}` is used as a type reference and cannot be converted to a value call"
      ));
    }

    if direct_macro_reference {
      continue;
    }

    let usages = runner::preprocess::trace_definition_source_usages(owner_ns, owner_def, &warnings, &CallStackList::default())
      .map_err(|failure| failure.msg)?;
    for usage in usages {
      if usage.target_ns.as_ref() != target_ns || usage.target_def.as_ref() != target_def {
        continue;
      }
      if owner_ns == target_ns && owner_def == target_def {
        blockers.push(format!(
          "{target_ns}/{target_def}: self-referential value initialization cannot be converted without changing its recursive evaluation model"
        ));
        continue;
      }
      if !usage.macro_origin.is_empty() {
        blockers.push(format!(
          "{owner_ns}/{owner_def}: target reference is produced across macro boundary {}",
          usage.macro_origin.join(" -> ")
        ));
        continue;
      }
      let Some(location) = usage.location else {
        blockers.push(format!(
          "{owner_ns}/{owner_def}: compiler resolved a target reference without a source coordinate"
        ));
        continue;
      };
      if location.ns.as_ref() != owner_ns || location.def.as_ref() != owner_def {
        blockers.push(format!(
          "{owner_ns}/{owner_def}: target reference points outside its editable source ({location})"
        ));
        continue;
      }
      let path = location.coord.iter().map(|value| usize::from(*value)).collect::<Vec<_>>();
      let original_node = navigate_to_path(&entry.code, &path)?;
      let Cirru::Leaf(original_leaf) = &original_node else {
        blockers.push(format!(
          "{owner_ns}/{owner_def}{}: resolved reference is not a source leaf",
          format_path(&path)
        ));
        continue;
      };
      let replacement_node = Cirru::List(vec![original_node.clone()]);
      let original_code = original_leaf.to_string();
      suggestions.push(FixSuggestion {
        rule_id: VALUE_TO_ZERO_ARG_FN_RULE,
        diagnostic_code: VALUE_TO_ZERO_ARG_FN_DIAGNOSTIC,
        semantic_layer: "surface",
        source_file: snapshot_file.to_owned(),
        definition: format!("{owner_ns}/{owner_def}"),
        path: format!("code{}", format_path(&path)),
        fingerprint: node_fingerprint(&original_node),
        origin_chain: vec![serde_json::json!({
          "kind": "resolved-value-read",
          "target": format!("{target_ns}/{target_def}"),
          "path": format_path(&path),
          "macro_origin": [],
        })],
        original: quoted_json(&original_node),
        replacement: Some(quoted_json(&replacement_node)),
        applicability: "machine-applicable",
        message: format!("Call the converted zero-argument function `{target_ns}/{target_def}`."),
        target_path: path,
        operation: Some(FixOperation::WrapLeafCall { original: original_code }),
      });
    }
  }
  if !blockers.is_empty() {
    blockers.sort();
    blockers.dedup();
    return Err(format!(
      "Semantic value-to-function refactor is not safe; no changes were written:\n{}",
      blockers.into_iter().map(|item| format!("- {item}")).collect::<Vec<_>>().join("\n")
    ));
  }

  let rewritten_schema = std::sync::Arc::new(CalcitTypeAnnotation::Fn(std::sync::Arc::new(CalcitFnTypeAnnotation {
    generics: std::sync::Arc::new(vec![]),
    where_bounds: std::sync::Arc::new(vec![]),
    arg_types: vec![],
    return_type: target_entry.schema.clone(),
    fn_kind: SchemaKind::Fn,
    rest_type: None,
    features: std::sync::Arc::new(std::collections::HashSet::new()),
  })));
  let original_schema_node = schema_edn_to_source_node(&calcit::snapshot::schema_annotation_to_edn(target_entry.schema.as_ref()))?;
  let rewritten_schema_node = schema_edn_to_source_node(&calcit::snapshot::schema_annotation_to_edn(rewritten_schema.as_ref()))?;
  suggestions.push(FixSuggestion {
    rule_id: VALUE_TO_ZERO_ARG_FN_RULE,
    diagnostic_code: VALUE_TO_ZERO_ARG_FN_DIAGNOSTIC,
    semantic_layer: "surface",
    source_file: snapshot_file.to_owned(),
    definition: format!("{target_ns}/{target_def}"),
    path: "schema".to_owned(),
    fingerprint: node_fingerprint(&original_schema_node),
    origin_chain: vec![serde_json::json!({"kind": "derived-zero-arg-function-schema"})],
    original: quoted_json(&original_schema_node),
    replacement: Some(quoted_json(&rewritten_schema_node)),
    applicability: "machine-applicable",
    message: "Wrap the existing value schema as the return type of a zero-argument function.".to_owned(),
    target_path: vec![],
    operation: Some(FixOperation::ReplaceSchema {
      code: format_quoted_nodes(std::slice::from_ref(&rewritten_schema_node))?,
    }),
  });

  let rewritten_definition = Cirru::List(vec![
    Cirru::leaf("defn"),
    Cirru::leaf(target_def),
    Cirru::List(vec![]),
    target_items[2].clone(),
  ]);
  suggestions.push(FixSuggestion {
    rule_id: VALUE_TO_ZERO_ARG_FN_RULE,
    diagnostic_code: VALUE_TO_ZERO_ARG_FN_DIAGNOSTIC,
    semantic_layer: "surface",
    source_file: snapshot_file.to_owned(),
    definition: format!("{target_ns}/{target_def}"),
    path: "definition".to_owned(),
    fingerprint: node_fingerprint(&target_entry.code),
    origin_chain: vec![serde_json::json!({"kind": "definition", "target": format!("{target_ns}/{target_def}")})],
    original: quoted_json(&target_entry.code),
    replacement: Some(quoted_json(&rewritten_definition)),
    applicability: "machine-applicable",
    message: format!("Convert `{target_ns}/{target_def}` from `def` to a zero-argument `defn`."),
    target_path: vec![],
    operation: Some(FixOperation::ReplaceDefinition {
      code: format_quoted_nodes(std::slice::from_ref(&rewritten_definition))?,
    }),
  });
  Ok(suggestions)
}

fn validate_zero_arg_value_refactor(
  snapshot: &Snapshot,
  project_definitions: &[(String, String)],
  target_ns: &str,
  target_def: &str,
) -> Result<(), String> {
  let warnings = RefCell::new(Vec::new());
  for (owner_ns, owner_def) in project_definitions {
    let entry = snapshot
      .files
      .get(owner_ns)
      .and_then(|file| file.defs.get(owner_def))
      .ok_or_else(|| format!("Project definition `{owner_ns}/{owner_def}` is missing from the staged snapshot."))?;
    if list_head(&entry.code) == Some("defmacro") && cirru_contains_target_reference(&entry.code, owner_ns, target_ns, target_def) {
      return Err(format!(
        "{owner_ns}/{owner_def}: staged macro source retains `{target_ns}/{target_def}`"
      ));
    }
    if quoted_region_contains_target_reference(&entry.code, owner_ns, target_ns, target_def) {
      return Err(format!(
        "{owner_ns}/{owner_def}: staged source retains quoted `{target_ns}/{target_def}` data"
      ));
    }
    let (_, schema_reference_count) =
      rewrite_loaded_schema_type_references(entry.schema.clone(), owner_ns, target_ns, target_def, target_def)?;
    if schema_reference_count > 0 {
      return Err(format!(
        "{owner_ns}/{owner_def} schema: staged source retains `{target_ns}/{target_def}` as a type reference"
      ));
    }
    for usage in runner::preprocess::trace_definition_source_usages(owner_ns, owner_def, &warnings, &CallStackList::default())
      .map_err(|failure| failure.msg)?
    {
      if usage.target_ns.as_ref() != target_ns || usage.target_def.as_ref() != target_def {
        continue;
      }
      if !usage.macro_origin.is_empty() {
        return Err(format!(
          "{owner_ns}/{owner_def}: staged target reference crosses macro boundary {}",
          usage.macro_origin.join(" -> ")
        ));
      }
      let location = usage
        .location
        .ok_or_else(|| format!("{owner_ns}/{owner_def}: staged target reference has no source coordinate"))?;
      let path = location.coord.iter().map(|value| usize::from(*value)).collect::<Vec<_>>();
      if !source_path_is_zero_arg_call(&entry.code, &path)? {
        return Err(format!(
          "{owner_ns}/{owner_def}{}: staged value reference is not a zero-argument call",
          format_path(&path)
        ));
      }
    }
    for (index, test) in entry.tests.iter().enumerate() {
      validate_attached_zero_arg_calls(
        &test.code,
        owner_ns,
        &format!("&calcit:validate-value-to-fn-test:{owner_def}:{index}"),
        &format!("{owner_ns}/{owner_def}#{}", test.name),
        target_ns,
        target_def,
      )?;
    }
    for (index, example) in entry.examples.iter().enumerate() {
      validate_attached_zero_arg_calls(
        example,
        owner_ns,
        &format!("&calcit:validate-value-to-fn-example:{owner_def}:{index}"),
        &format!("{owner_ns}/{owner_def} example {index}"),
        target_ns,
        target_def,
      )?;
    }
  }
  Ok(())
}

fn validate_attached_zero_arg_calls(
  source: &Cirru,
  owner_ns: &str,
  synthetic_def: &str,
  source_label: &str,
  target_ns: &str,
  target_def: &str,
) -> Result<(), String> {
  if quoted_region_contains_target_reference(source, owner_ns, target_ns, target_def) {
    return Err(format!(
      "{source_label}: staged attached source retains quoted `{target_ns}/{target_def}` data"
    ));
  }
  if !cirru_contains_target_reference(source, owner_ns, target_ns, target_def) {
    return Ok(());
  }
  let wrapper = Cirru::List(vec![Cirru::leaf("fn"), Cirru::List(vec![]), source.clone()]);
  let parsed = code_to_calcit(&wrapper, owner_ns, synthetic_def, vec![])
    .map_err(|error| format!("{source_label}: failed to parse staged attached source: {error}"))?;
  let warnings = RefCell::new(Vec::new());
  for usage in runner::preprocess::trace_source_usages(&parsed, owner_ns, synthetic_def, &warnings, &CallStackList::default())
    .map_err(|failure| format!("{source_label}: failed to preprocess staged attached source: {}", failure.msg))?
  {
    if usage.target_ns.as_ref() != target_ns || usage.target_def.as_ref() != target_def {
      continue;
    }
    let macro_origin = usage
      .macro_origin
      .into_iter()
      .filter(|origin| origin != "calcit.core/fn")
      .collect::<Vec<_>>();
    if !macro_origin.is_empty() {
      return Err(format!(
        "{source_label}: staged target reference crosses macro boundary {}",
        macro_origin.join(" -> ")
      ));
    }
    let location = usage
      .location
      .ok_or_else(|| format!("{source_label}: staged target reference has no source coordinate"))?;
    let wrapper_path = location.coord.iter().map(|value| usize::from(*value)).collect::<Vec<_>>();
    let path = wrapper_path.strip_prefix(&[2]).ok_or_else(|| {
      format!(
        "{source_label}{}: staged coordinate is outside attached source",
        format_path(&wrapper_path)
      )
    })?;
    if !source_path_is_zero_arg_call(source, path)? {
      return Err(format!(
        "{source_label}{}: staged value reference is not a zero-argument call",
        format_path(path)
      ));
    }
  }
  Ok(())
}

fn source_path_is_zero_arg_call(source: &Cirru, path: &[usize]) -> Result<bool, String> {
  let Some((&child_index, parent_path)) = path.split_last() else {
    return Ok(false);
  };
  let parent = navigate_to_path(source, parent_path)?;
  Ok(matches!(parent, Cirru::List(items) if child_index == 0 && items.len() == 1))
}

fn validate_no_stale_attached_references(
  snapshot: &Snapshot,
  project_definitions: &[(String, String)],
  target_ns: &str,
  old_name: &str,
  new_name: &str,
) -> Result<(), String> {
  for (owner_ns, owner_def) in project_definitions {
    let entry = snapshot
      .files
      .get(owner_ns)
      .and_then(|file| file.defs.get(owner_def))
      .ok_or_else(|| format!("Project definition `{owner_ns}/{owner_def}` is missing from the staged snapshot."))?;
    for (index, test) in entry.tests.iter().enumerate() {
      if !cirru_contains_target_reference(&test.code, owner_ns, target_ns, old_name) {
        continue;
      }
      let source_label = format!("{owner_ns}/{owner_def}#{}", test.name);
      if plan_attached_source_rewrite(
        &test.code,
        owner_ns,
        &format!("&calcit:validate-rename-test:{owner_def}:{index}"),
        &source_label,
        target_ns,
        old_name,
        new_name,
      )?
      .is_some()
      {
        return Err(format!(
          "{source_label}: staged semantic rename left a resolved reference to `{target_ns}/{old_name}`"
        ));
      }
    }
    for (index, example) in entry.examples.iter().enumerate() {
      if !cirru_contains_target_reference(example, owner_ns, target_ns, old_name) {
        continue;
      }
      let source_label = format!("{owner_ns}/{owner_def} example {index}");
      if plan_attached_source_rewrite(
        example,
        owner_ns,
        &format!("&calcit:validate-rename-example:{owner_def}:{index}"),
        &source_label,
        target_ns,
        old_name,
        new_name,
      )?
      .is_some()
      {
        return Err(format!(
          "{source_label}: staged semantic rename left a resolved reference to `{target_ns}/{old_name}`"
        ));
      }
    }
    let (_, stale_count) = rewrite_loaded_schema_type_references(entry.schema.clone(), owner_ns, target_ns, old_name, new_name)?;
    if stale_count > 0 {
      return Err(format!(
        "{owner_ns}/{owner_def} schema: staged semantic rename left a type reference to `{target_ns}/{old_name}`"
      ));
    }
  }
  Ok(())
}

fn source_name_resolves_to_target(owner_ns: &str, source: &str, target_ns: &str, target_def: &str) -> bool {
  if let Some((prefix, definition)) = source.rsplit_once('/') {
    if definition != target_def {
      return false;
    }
    if prefix == target_ns {
      return true;
    }
    return program::lookup_ns_target_in_import(owner_ns, prefix).is_some_and(|namespace| namespace.as_ref() == target_ns);
  }
  source == target_def
    && (owner_ns == target_ns
      || imported_definition_target(owner_ns, source)
        .is_some_and(|(namespace, definition)| namespace == target_ns && definition == target_def)
      // Core names are implicitly visible after local definitions and imports.
      // Lexical bindings are still checked by the compiler usage trace.
      || (target_ns == "calcit.core"
        && program::lookup_def_id(owner_ns, source).is_none()
        && imported_definition_target(owner_ns, source).is_none()
        && program::lookup_def_id("calcit.core", source).is_some()))
}

fn cirru_contains_target_reference(node: &Cirru, owner_ns: &str, target_ns: &str, target_def: &str) -> bool {
  match node {
    Cirru::Leaf(value) => source_name_resolves_to_target(owner_ns, value, target_ns, target_def),
    Cirru::List(items) => items
      .iter()
      .any(|item| cirru_contains_target_reference(item, owner_ns, target_ns, target_def)),
  }
}

fn quoted_region_contains_target_reference(node: &Cirru, owner_ns: &str, target_ns: &str, target_def: &str) -> bool {
  let Cirru::List(items) = node else { return false };
  if items
    .first()
    .is_some_and(|head| head.eq_leaf("quote") || head.eq_leaf("quasiquote"))
  {
    return items
      .iter()
      .skip(1)
      .any(|item| cirru_contains_target_reference(item, owner_ns, target_ns, target_def));
  }
  items
    .iter()
    .any(|item| quoted_region_contains_target_reference(item, owner_ns, target_ns, target_def))
}

struct AttachedSourceRewrite {
  code: Cirru,
  origin_chain: Vec<Value>,
}

struct AttachedRewriteTarget<'a> {
  namespace: &'a str,
  definition: &'a str,
  allow_preserving_macros: bool,
}

/// Resolve references inside one definition-attached executable source without guessing from leaf text.
fn plan_attached_source_rewrite(
  source: &Cirru,
  owner_ns: &str,
  synthetic_def: &str,
  source_label: &str,
  target_ns: &str,
  old_name: &str,
  new_name: &str,
) -> Result<Option<AttachedSourceRewrite>, String> {
  plan_attached_source_rewrite_with(
    source,
    owner_ns,
    synthetic_def,
    source_label,
    AttachedRewriteTarget {
      namespace: target_ns,
      definition: old_name,
      allow_preserving_macros: false,
    },
    &|source_leaf, _| semantic_rename_leaf_replacement(source_leaf, old_name, target_ns, new_name).map(Cirru::leaf),
  )
}

fn plan_attached_value_call_rewrite(
  source: &Cirru,
  owner_ns: &str,
  synthetic_def: &str,
  source_label: &str,
  target_ns: &str,
  target_def: &str,
) -> Result<Option<AttachedSourceRewrite>, String> {
  plan_attached_source_rewrite_with(
    source,
    owner_ns,
    synthetic_def,
    source_label,
    AttachedRewriteTarget {
      namespace: target_ns,
      definition: target_def,
      allow_preserving_macros: false,
    },
    &|source_leaf, _| Ok(Cirru::List(vec![Cirru::leaf(source_leaf)])),
  )
}

fn plan_attached_source_rewrite_with<F>(
  source: &Cirru,
  owner_ns: &str,
  synthetic_def: &str,
  source_label: &str,
  target: AttachedRewriteTarget<'_>,
  rewrite_leaf: &F,
) -> Result<Option<AttachedSourceRewrite>, String>
where
  F: Fn(&str, &[usize]) -> Result<Cirru, String>,
{
  let AttachedRewriteTarget {
    namespace: target_ns,
    definition: target_def,
    allow_preserving_macros,
  } = target;
  if quoted_region_contains_target_reference(source, owner_ns, target_ns, target_def) {
    return Err(format!(
      "{source_label}: quoted source contains `{target_ns}/{target_def}` and may be consumed dynamically"
    ));
  }
  let wrapper = Cirru::List(vec![Cirru::leaf("fn"), Cirru::List(vec![]), source.clone()]);
  let parsed = code_to_calcit(&wrapper, owner_ns, synthetic_def, vec![])
    .map_err(|error| format!("{source_label}: failed to parse attached source for semantic refactor: {error}"))?;
  let warnings = RefCell::new(Vec::new());
  let usages = runner::preprocess::trace_source_usages(&parsed, owner_ns, synthetic_def, &warnings, &CallStackList::default())
    .map_err(|failure| {
      format!(
        "{source_label}: failed to preprocess attached source for semantic refactor: {}",
        failure.msg
      )
    })?;
  let mut rewrites = BTreeMap::<Vec<usize>, (String, Cirru, Vec<String>)>::new();
  for usage in usages {
    if usage.target_ns.as_ref() != target_ns || usage.target_def.as_ref() != target_def {
      continue;
    }
    let macro_origin = usage
      .macro_origin
      .into_iter()
      .filter(|origin| origin != "calcit.core/fn")
      .collect::<Vec<_>>();
    if !macro_origin.is_empty()
      && !(allow_preserving_macros
        && macro_origin
          .iter()
          .all(|origin| preserves_nominal_method_call_through_macro(origin)))
    {
      return Err(format!(
        "{source_label}: target reference is produced across macro boundary {}",
        macro_origin.join(" -> ")
      ));
    }
    let Some(location) = usage.location else {
      return Err(format!(
        "{source_label}: compiler resolved a target reference without a source coordinate"
      ));
    };
    if location.ns.as_ref() != owner_ns || location.def.as_ref() != synthetic_def {
      return Err(format!(
        "{source_label}: target reference points outside its editable attached source ({location})"
      ));
    }
    let wrapper_path = location.coord.iter().map(|value| usize::from(*value)).collect::<Vec<_>>();
    let Some(path) = wrapper_path.strip_prefix(&[2]) else {
      return Err(format!(
        "{source_label}{}: compiler coordinate does not point into the attached expression",
        format_path(&wrapper_path)
      ));
    };
    let node = navigate_to_path(source, path)?;
    let Cirru::Leaf(source_leaf) = node else {
      return Err(format!(
        "{source_label}{}: resolved reference is not a source leaf",
        format_path(path)
      ));
    };
    let replacement = rewrite_leaf(&source_leaf, path).map_err(|error| format!("{source_label}{}: {error}", format_path(path)))?;
    rewrites.insert(path.to_vec(), (source_leaf.to_string(), replacement, macro_origin));
  }
  if rewrites.is_empty() {
    return Ok(None);
  }

  let mut rewritten = source.clone();
  let mut origin_chain = Vec::with_capacity(rewrites.len());
  for (path, (original, replacement, macro_origin)) in rewrites {
    replace_attached_source_node(&mut rewritten, &path, &original, &replacement)?;
    origin_chain.push(serde_json::json!({
      "kind": "resolved-attached-source",
      "target": format!("{target_ns}/{target_def}"),
      "path": format_path(&path),
      "macro_origin": macro_origin,
    }));
  }
  Ok(Some(AttachedSourceRewrite {
    code: rewritten,
    origin_chain,
  }))
}

fn semantic_rename_leaf_replacement(source_leaf: &str, old_name: &str, target_ns: &str, new_name: &str) -> Result<String, String> {
  if let Some((prefix, source_name)) = source_leaf.rsplit_once('/') {
    if source_name != old_name {
      return Err(format!("resolved leaf `{source_leaf}` does not name `{old_name}`"));
    }
    Ok(format!("{prefix}/{new_name}"))
  } else if source_leaf == old_name {
    Ok(format!("{target_ns}/{new_name}"))
  } else {
    Err(format!("resolved leaf `{source_leaf}` does not name `{old_name}`"))
  }
}

fn replace_attached_source_node(node: &mut Cirru, path: &[usize], expected: &str, replacement: &Cirru) -> Result<(), String> {
  replace_attached_source_tree(node, path, &Cirru::leaf(expected), replacement)
}

/// Replace a checked source subtree without interpreting its payload as executable code.
fn replace_attached_source_tree(node: &mut Cirru, path: &[usize], expected: &Cirru, replacement: &Cirru) -> Result<(), String> {
  if path.is_empty() {
    if node != expected {
      return Err(format!(
        "Attached source changed while planning rewrite: expected `{expected}`, got `{node}`"
      ));
    }
    *node = replacement.clone();
    return Ok(());
  }
  let Cirru::List(items) = node else {
    return Err(format!("Attached source path {} traverses a leaf", format_path(path)));
  };
  let index = path[0];
  let child = items
    .get_mut(index)
    .ok_or_else(|| format!("Attached source path {} is out of range", format_path(path)))?;
  replace_attached_source_tree(child, &path[1..], expected, replacement)
}

fn format_quoted_nodes(nodes: &[Cirru]) -> Result<String, String> {
  let quoted = nodes
    .iter()
    .map(|node| Cirru::List(vec![Cirru::leaf("quote"), node.clone()]))
    .collect::<Vec<_>>();
  cirru_parser::format(&quoted, true.into()).map_err(|error| format!("Failed to encode attached source rewrite: {error}"))
}

fn rewrite_loaded_schema_type_references(
  schema: std::sync::Arc<CalcitTypeAnnotation>,
  owner_ns: &str,
  target_ns: &str,
  old_name: &str,
  new_name: &str,
) -> Result<(std::sync::Arc<CalcitTypeAnnotation>, usize), String> {
  let replacements = Cell::new(0);
  let rewritten = runner::preprocess::map_schema_references(
    schema,
    &|name, args| {
      if source_name_resolves_to_target(owner_ns, name, target_ns, old_name) {
        let replacement = semantic_rename_leaf_replacement(name, old_name, target_ns, new_name)
          .expect("a compiler-loaded matching TypeRef must preserve the selected definition name");
        replacements.set(replacements.get() + 1);
        std::sync::Arc::new(CalcitTypeAnnotation::TypeRef(replacement.into(), args))
      } else {
        std::sync::Arc::new(CalcitTypeAnnotation::TypeRef(name.clone(), args))
      }
    },
    &|trait_def| {
      let source_name = trait_def.definition_ref.as_deref().unwrap_or_else(|| trait_def.name.ref_str());
      if !source_name_resolves_to_target(owner_ns, source_name, target_ns, old_name) {
        return trait_def;
      }
      let replacement = semantic_rename_leaf_replacement(source_name, old_name, target_ns, new_name)
        .expect("a compiler-loaded matching trait bound must preserve the selected definition name");
      let mut rewritten = trait_def.as_ref().clone();
      rewritten.name = cirru_edn::EdnTag::new(new_name);
      if rewritten.definition_ref.is_some() {
        rewritten.definition_ref = Some(replacement.trim_start_matches('\'').into());
      }
      replacements.set(replacements.get() + 1);
      std::sync::Arc::new(rewritten)
    },
  );
  Ok((rewritten, replacements.get()))
}

fn schema_edn_to_source_node(schema: &cirru_edn::Edn) -> Result<Cirru, String> {
  let rendered = cirru_edn::format(schema, true).map_err(|error| format!("Failed to format rewritten schema: {error}"))?;
  let nodes = cirru_parser::parse(&rendered).map_err(|error| format!("Failed to parse rewritten schema source: {error}"))?;
  match nodes.as_slice() {
    [node] => Ok(node.clone()),
    _ => Err(format!("Rewritten schema must render as one source node, got {}.", nodes.len())),
  }
}

fn plan_referred_import_removals(
  snapshot: &Snapshot,
  snapshot_file: &str,
  target_ns: &str,
  old_name: &str,
  new_name: &str,
) -> Result<Vec<FixSuggestion>, String> {
  let mut suggestions = Vec::new();
  let mut namespaces = snapshot.files.keys().cloned().collect::<Vec<_>>();
  namespaces.sort();
  for namespace in namespaces {
    if namespace != snapshot.package && !namespace.starts_with(&format!("{}.", snapshot.package)) {
      continue;
    }
    let file = &snapshot.files[&namespace];
    let Cirru::List(ns_items) = &file.ns.code else { continue };
    let Some(Cirru::List(require_items)) = ns_items.get(2) else {
      continue;
    };
    if !require_items.first().is_some_and(|head| head.eq_leaf(":require")) {
      continue;
    }
    let original_rule_nodes = require_items.iter().skip(1).cloned().collect::<Vec<_>>();
    let mut rules = Vec::with_capacity(original_rule_nodes.len());
    let mut changed = false;
    for rule in &original_rule_nodes {
      let Cirru::List(items) = rule else {
        rules.push(rule.clone());
        continue;
      };
      if items.len() != 3 || !items[0].eq_leaf(target_ns) || !items[1].eq_leaf(":refer") {
        rules.push(rule.clone());
        continue;
      }
      let Cirru::List(referred) = &items[2] else {
        rules.push(rule.clone());
        continue;
      };
      let next_referred = referred.iter().filter(|item| !item.eq_leaf(old_name)).cloned().collect::<Vec<_>>();
      if next_referred.len() != referred.len() {
        changed = true;
        if next_referred.iter().any(|item| !item.eq_leaf("[]")) {
          let mut next_items = items.clone();
          next_items[2] = Cirru::List(next_referred);
          rules.push(Cirru::List(next_items));
        }
      } else {
        rules.push(rule.clone());
      }
    }
    if !changed {
      continue;
    }
    let original_rules = Cirru::List(original_rule_nodes);
    let replacement_rules = Cirru::List(rules.clone());
    let rules_json = Value::Array(rules.iter().map(cirru_to_json_value).collect());
    suggestions.push(FixSuggestion {
      rule_id: RENAME_DEFINITION_RULE,
      diagnostic_code: RENAME_DEFINITION_DIAGNOSTIC,
      semantic_layer: "surface",
      source_file: snapshot_file.to_owned(),
      definition: namespace.clone(),
      path: "ns.:require".to_owned(),
      fingerprint: node_fingerprint(&original_rules),
      origin_chain: vec![serde_json::json!({"kind": "refer-import", "target": format!("{target_ns}/{old_name}")})],
      original: quoted_json(&original_rules),
      replacement: Some(quoted_json(&replacement_rules)),
      applicability: "machine-applicable",
      message: format!(
        "Remove the stale `:refer` binding for `{target_ns}/{old_name}`; rewritten usages name `{target_ns}/{new_name}` explicitly."
      ),
      target_path: vec![],
      operation: Some(FixOperation::ReplaceImports {
        code: serde_json::to_string(&rules_json).map_err(|error| format!("Failed to encode import rewrite: {error}"))?,
      }),
    });
  }
  Ok(suggestions)
}

/// Convert removed-data diagnostics back to guarded source-leaf replacements.
fn plan_removed_data_api_fixes(
  options: &FixCommand,
  snapshot: &Snapshot,
  snapshot_file: &str,
  warnings: &[LocatedWarning],
) -> Result<Vec<FixSuggestion>, String> {
  let mut suggestions = BTreeMap::new();
  for warning in warnings {
    if warning.code() != Some(REMOVED_DATA_API_DIAGNOSTIC) {
      continue;
    }
    let location = warning.location();
    if options.ns.as_deref().is_some_and(|filter| location.ns.as_ref() != filter)
      || options.definition.as_deref().is_some_and(|filter| location.def.as_ref() != filter)
    {
      continue;
    }
    let Some(file) = snapshot.files.get(location.ns.as_ref()) else {
      continue;
    };
    let Some(entry) = file.defs.get(location.def.as_ref()) else {
      continue;
    };
    let Some(suggestion) = removed_data_api_source_suggestion(snapshot_file, &entry.code, warning) else {
      continue;
    };
    let key = (location.ns.to_string(), location.def.to_string(), suggestion.target_path.clone());
    insert_fix_suggestion(&mut suggestions, key, suggestion)?;
  }
  Ok(suggestions.into_values().collect())
}

/// Map an existing compiler diagnostic back to its source leaf in any source region.
fn removed_data_api_source_suggestion(snapshot_file: &str, source: &Cirru, warning: &LocatedWarning) -> Option<FixSuggestion> {
  let location = warning.location();
  let coordinate = location.coord.iter().map(|value| usize::from(*value)).collect::<Vec<_>>();
  let (target_path, original_leaf) = resolve_fix_target(source, &coordinate)?;
  let (replacement_leaf, guidance) = migration_for_source_leaf(&original_leaf)?;
  let original_node = Cirru::leaf(original_leaf.as_str());
  let replacement = replacement_leaf.as_ref().map(|leaf| quoted_json(&Cirru::leaf(leaf.as_str())));
  let applicability = if replacement.is_some() {
    "machine-applicable"
  } else {
    "requires-review"
  };
  let message = if replacement.is_some() {
    format!("Replace `{original_leaf}` with `{guidance}`.")
  } else {
    format!("Choose the value or definition predicate for `{original_leaf}`: {guidance}.")
  };
  Some(FixSuggestion {
    rule_id: REMOVED_DATA_API_RULE,
    diagnostic_code: REMOVED_DATA_API_DIAGNOSTIC,
    semantic_layer: "surface",
    source_file: snapshot_file.to_owned(),
    definition: format!("{}/{}", location.ns, location.def),
    path: format!("code{}", format_path(&target_path)),
    fingerprint: node_fingerprint(&original_node),
    origin_chain: vec![],
    original: quoted_json(&original_node),
    replacement,
    applicability,
    message,
    target_path: target_path.clone(),
    operation: replacement_leaf.map(|replacement| FixOperation::ReplaceLeaf {
      original: original_leaf,
      replacement,
    }),
  })
}

fn plan_removed_data_api_source(
  snapshot_file: &str,
  namespace: &str,
  definition: &str,
  source: &Cirru,
) -> Result<Vec<FixSuggestion>, String> {
  let parsed = code_to_calcit(source, namespace, definition, vec![]).map_err(|error| error.to_string())?;
  let warnings = RefCell::new(Vec::new());
  let usages = with_legacy_migration_mode(|| {
    runner::preprocess::trace_source_usages(&parsed, namespace, definition, &warnings, &CallStackList::default())
      .map_err(|failure| failure.msg)
  })?;
  let mut suggestions = BTreeMap::new();
  for warning in warnings.borrow().iter() {
    let location = warning.location();
    if warning.code() != Some(REMOVED_DATA_API_DIAGNOSTIC) || location.ns.as_ref() != namespace || location.def.as_ref() != definition {
      continue;
    }
    let Some(mut suggestion) = removed_data_api_source_suggestion(snapshot_file, source, warning) else {
      continue;
    };
    let call_path = suggestion.target_path.strip_suffix(&[0]).unwrap_or(&suggestion.target_path);
    if !method_source_context_is_stable(source, call_path, namespace, definition, &usages) {
      suggestion.applicability = "requires-review";
      suggestion.replacement = None;
      suggestion.operation = None;
      suggestion.message =
        "The removed API reference crosses an unknown source context; review whether expansion observes its spelling.".to_owned();
    }
    let key = (namespace.to_owned(), definition.to_owned(), suggestion.target_path.clone());
    insert_fix_suggestion(&mut suggestions, key, suggestion)?;
  }
  Ok(suggestions.into_values().collect())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CoreNominalConstructor {
  nominal: &'static str,
  variant: &'static str,
  arity: usize,
}

fn core_nominal_constructor(name: &str) -> Option<CoreNominalConstructor> {
  let definition = name.rsplit_once('/').map_or(name, |(_, definition)| definition);
  match definition {
    "%some" => Some(CoreNominalConstructor {
      nominal: "Option",
      variant: ":some",
      arity: 1,
    }),
    "%none" => Some(CoreNominalConstructor {
      nominal: "Option",
      variant: ":none",
      arity: 0,
    }),
    "%ok" => Some(CoreNominalConstructor {
      nominal: "Result",
      variant: ":ok",
      arity: 1,
    }),
    "%err" => Some(CoreNominalConstructor {
      nominal: "Result",
      variant: ":err",
      arity: 1,
    }),
    _ => None,
  }
}

fn core_nominal_type_is_unshadowed(snapshot: &Snapshot, namespace: &str, nominal: &str, bindings: &HashSet<String>) -> bool {
  let has_local_definition = snapshot.files.get(namespace).is_some_and(|file| file.defs.contains_key(nominal));
  let has_import = program::PROGRAM_CODE_DATA
    .read()
    .ok()
    .and_then(|data| data.get(namespace).map(|entry| entry.import_map.contains_key(nominal)))
    .unwrap_or(false);
  !bindings.contains(nominal) && !has_local_definition && !has_import
}

fn preserves_constructor_argument_through_macro(origin: &str) -> bool {
  // Core bodies and validated Struct fields preserve one evaluation of each
  // payload; none consumes nested call heads as data.
  matches!(origin, "calcit.core/let" | "calcit.core/cond" | "calcit.core/%{}")
}

fn rewrite_core_nominal_constructor_tree(
  node: &Cirru,
  path: &mut Vec<usize>,
  selected: &BTreeMap<Vec<usize>, (CoreNominalConstructor, String)>,
) -> Cirru {
  let Cirru::List(items) = node else {
    return node.clone();
  };
  let rewritten = items
    .iter()
    .enumerate()
    .map(|(index, item)| {
      path.push(index);
      let rewritten = rewrite_core_nominal_constructor_tree(item, path, selected);
      path.pop();
      rewritten
    })
    .collect::<Vec<_>>();
  if let Some((constructor, nominal)) = selected.get(path) {
    let mut direct = Vec::with_capacity(rewritten.len() + 1);
    direct.push(Cirru::leaf(nominal.as_str()));
    direct.push(Cirru::leaf(constructor.variant));
    direct.extend(rewritten.into_iter().skip(1));
    Cirru::List(direct)
  } else {
    Cirru::List(rewritten)
  }
}

/// Detect source-level legacy constructors before attempting compiler-resolved rewrites.
fn contains_core_nominal_constructor_candidate(node: &Cirru) -> bool {
  match node {
    Cirru::Leaf(name) => core_nominal_constructor(name.as_ref()).is_some(),
    Cirru::List(items) => items.iter().any(contains_core_nominal_constructor_candidate),
  }
}

/// Plan direct nominal construction only from compiler-resolved core helper calls.
fn plan_core_nominal_constructor_fixes(
  snapshot: &Snapshot,
  snapshot_file: &str,
  selected_definitions: &[(String, String)],
) -> Result<Vec<FixSuggestion>, String> {
  let mut suggestions = Vec::new();
  for (namespace, definition) in selected_definitions {
    let entry = snapshot
      .files
      .get(namespace)
      .and_then(|file| file.defs.get(definition))
      .ok_or_else(|| format!("Selected definition `{namespace}/{definition}` is missing from the source snapshot."))?;
    suggestions.extend(plan_core_nominal_constructor_source(
      snapshot,
      snapshot_file,
      namespace,
      definition,
      &entry.code,
    )?);
  }
  Ok(suggestions)
}

/// Share resolved constructor calls, arity and shadowing proof with attached executable source.
fn plan_core_nominal_constructor_source(
  snapshot: &Snapshot,
  snapshot_file: &str,
  namespace: &str,
  definition: &str,
  source: &Cirru,
) -> Result<Vec<FixSuggestion>, String> {
  let mut suggestions = Vec::new();
  if list_head(source) == Some("defmacro") || !contains_core_nominal_constructor_candidate(source) {
    return Ok(suggestions);
  }
  let mut bindings = HashSet::new();
  collect_potential_local_bindings(source, &mut bindings);
  let mut safe_calls = BTreeMap::<Vec<usize>, (CoreNominalConstructor, String)>::new();
  let mut review_calls = BTreeMap::<Vec<usize>, String>::new();
  let warnings = RefCell::new(Vec::new());
  let parsed = code_to_calcit(source, namespace, definition, vec![]).map_err(|error| error.to_string())?;
  let usages = runner::preprocess::trace_source_usages(&parsed, namespace, definition, &warnings, &CallStackList::default())
    .map_err(|failure| failure.msg)?;
  for usage in usages {
    if usage.target_ns.as_ref() != "calcit.core" {
      continue;
    }
    let Some(constructor) = core_nominal_constructor(usage.target_def.as_ref()) else {
      continue;
    };
    let Some(location) = usage.location else {
      continue;
    };
    if location.ns.as_ref() != namespace || location.def.as_ref() != definition {
      continue;
    }
    let leaf_path = location.coord.iter().map(|value| usize::from(*value)).collect::<Vec<_>>();
    if leaf_path.last() != Some(&0) {
      let node = navigate_to_path(source, &leaf_path)?;
      if matches!(&node, Cirru::Leaf(name) if core_nominal_constructor(name.as_ref()) == Some(constructor)) {
        review_calls.insert(
          leaf_path,
          "The helper is referenced as a value rather than called with proven arity; review this use manually.".to_owned(),
        );
      }
      continue;
    }
    let Some((&0, call_path)) = leaf_path.split_last() else {
      continue;
    };
    let source_leaf = navigate_to_path(source, &leaf_path)?;
    let Cirru::Leaf(source_name) = source_leaf else {
      continue;
    };
    if core_nominal_constructor(source_name.as_ref()) != Some(constructor) {
      continue;
    }
    let call = navigate_to_path(source, call_path)?;
    let Cirru::List(items) = call else {
      continue;
    };
    if !usage
      .macro_origin
      .iter()
      .all(|origin| preserves_nominal_method_call_through_macro(origin))
    {
      review_calls.insert(
        call_path.to_vec(),
        format!(
          "The reference crosses macro expansion {}; review the source call manually.",
          usage.macro_origin.join(" -> ")
        ),
      );
    } else if items.len() != constructor.arity + 1 {
      review_calls.insert(
        call_path.to_vec(),
        format!(
          "The helper call does not have exactly {} payload argument(s); review the source call manually.",
          constructor.arity
        ),
      );
    } else if !core_nominal_type_is_unshadowed(snapshot, namespace, constructor.nominal, &bindings) {
      review_calls.insert(
        call_path.to_vec(),
        format!(
          "The nominal name `{}` is shadowed in this definition; review a qualified constructor call manually.",
          constructor.nominal
        ),
      );
    } else {
      safe_calls.insert(call_path.to_vec(), (constructor, constructor.nominal.to_owned()));
    }
  }
  for (path, message) in review_calls {
    safe_calls.remove(&path);
    let node = navigate_to_path(source, &path)?;
    suggestions.push(FixSuggestion {
      rule_id: CORE_NOMINAL_CONSTRUCTOR_RULE,
      diagnostic_code: CORE_NOMINAL_CONSTRUCTOR_DIAGNOSTIC,
      semantic_layer: "surface",
      source_file: snapshot_file.to_owned(),
      definition: format!("{namespace}/{definition}"),
      path: format!("code{}", format_path(&path)),
      fingerprint: node_fingerprint(&node),
      origin_chain: vec![],
      original: quoted_json(&node),
      replacement: None,
      applicability: "requires-review",
      message,
      target_path: path,
      operation: None,
    });
  }
  let mut outermost = Vec::<Vec<usize>>::new();
  for path in safe_calls.keys() {
    if !safe_calls.keys().any(|other| other.len() < path.len() && path.starts_with(other)) {
      outermost.push(path.clone());
    }
  }
  for path in outermost {
    let original = navigate_to_path(source, &path)?;
    let replacement = rewrite_core_nominal_constructor_tree(&original, &mut path.clone(), &safe_calls);
    let original_code = original
      .format_one_liner()
      .map_err(|error| format!("Failed to format constructor source at {namespace}/{definition}: {error}"))?;
    let replacement_code = replacement
      .format_one_liner()
      .map_err(|error| format!("Failed to format constructor replacement at {namespace}/{definition}: {error}"))?;
    suggestions.push(FixSuggestion {
        rule_id: CORE_NOMINAL_CONSTRUCTOR_RULE,
        diagnostic_code: CORE_NOMINAL_CONSTRUCTOR_DIAGNOSTIC,
        semantic_layer: "surface",
        source_file: snapshot_file.to_owned(),
        definition: format!("{namespace}/{definition}"),
        path: format!("code{}", format_path(&path)),
        fingerprint: node_fingerprint(&original),
        origin_chain: vec![serde_json::json!({"kind":"compiler-resolved-core-constructor","calls":safe_calls.keys().filter(|candidate| candidate.starts_with(&path)).count()})],
        original: quoted_json(&original),
        replacement: Some(quoted_json(&replacement)),
        applicability: "machine-applicable",
        message: "Call the nominal Option/Result constructor directly; each payload remains in its original position and is evaluated once.".to_owned(),
        target_path: path,
        operation: Some(FixOperation::ReplaceNode {
          original: original_code,
          replacement: replacement_code,
        }),
      });
  }
  Ok(suggestions)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CoreNominalMethod {
  method: &'static str,
  definition: &'static str,
  arity: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CoreNominalMethodKind {
  Option,
  Result,
}

impl CoreNominalMethodKind {
  fn rule_id(self) -> &'static str {
    match self {
      Self::Option => CORE_OPTION_METHOD_RULE,
      Self::Result => CORE_RESULT_METHOD_RULE,
    }
  }

  fn diagnostic(self) -> &'static str {
    match self {
      Self::Option => CORE_OPTION_METHOD_DIAGNOSTIC,
      Self::Result => CORE_RESULT_METHOD_DIAGNOSTIC,
    }
  }

  fn nominal(self) -> &'static str {
    match self {
      Self::Option => "Option",
      Self::Result => "Result",
    }
  }
}

fn core_nominal_method(name: &str, kind: CoreNominalMethodKind) -> Option<CoreNominalMethod> {
  match (kind, name.rsplit_once('/').map_or(name, |(_, definition)| definition)) {
    (CoreNominalMethodKind::Option, "option:some?") => Some(CoreNominalMethod {
      method: ".some?",
      definition: "calcit.core/option:some?",
      arity: 1,
    }),
    (CoreNominalMethodKind::Option, "option:none?") => Some(CoreNominalMethod {
      method: ".none?",
      definition: "calcit.core/option:none?",
      arity: 1,
    }),
    (CoreNominalMethodKind::Option, "option:unwrap") => Some(CoreNominalMethod {
      method: ".unwrap",
      definition: "calcit.core/option:unwrap",
      arity: 1,
    }),
    (CoreNominalMethodKind::Option, "option:unwrap-or") => Some(CoreNominalMethod {
      method: ".unwrap-or",
      definition: "calcit.core/option:unwrap-or",
      arity: 2,
    }),
    (CoreNominalMethodKind::Result, "result:unwrap-or") => Some(CoreNominalMethod {
      method: ".unwrap-or",
      definition: "calcit.core/result:unwrap-or",
      arity: 2,
    }),
    (CoreNominalMethodKind::Result, "result:ok?") => Some(CoreNominalMethod {
      method: ".ok?",
      definition: "calcit.core/result:ok?",
      arity: 1,
    }),
    (CoreNominalMethodKind::Result, "result:err?") => Some(CoreNominalMethod {
      method: ".err?",
      definition: "calcit.core/result:err?",
      arity: 1,
    }),
    _ => None,
  }
}

fn preserves_nominal_method_call_through_macro(origin: &str) -> bool {
  // These core forms retain one executable evaluation of the nested call.
  // assert= also quotes its source for diagnostics, but never evaluates that copy.
  preserves_constructor_argument_through_macro(origin)
    || matches!(
      origin,
      "calcit.core/def" | "calcit.core/do" | "calcit.core/fn" | "calcit.core/assert="
    )
}

fn rewrite_core_nominal_method_tree(node: &Cirru, path: &mut Vec<usize>, selected: &BTreeMap<Vec<usize>, CoreNominalMethod>) -> Cirru {
  let Cirru::List(items) = node else {
    return node.clone();
  };
  let rewritten = items
    .iter()
    .enumerate()
    .map(|(index, item)| {
      path.push(index);
      let rewritten = rewrite_core_nominal_method_tree(item, path, selected);
      path.pop();
      rewritten
    })
    .collect::<Vec<_>>();
  if let Some(method) = selected.get(path) {
    let mut direct = Vec::with_capacity(rewritten.len());
    direct.push(rewritten[1].clone());
    direct.push(Cirru::leaf(method.method));
    direct.extend(rewritten.into_iter().skip(2));
    Cirru::List(direct)
  } else {
    Cirru::List(rewritten)
  }
}

/// Rewrite only core helper calls whose receiver resolves to the same proven nominal method.
fn plan_core_nominal_method_fixes(
  snapshot: &Snapshot,
  snapshot_file: &str,
  selected_definitions: &[(String, String)],
  kind: CoreNominalMethodKind,
) -> Result<Vec<FixSuggestion>, String> {
  let mut suggestions = Vec::new();
  for (namespace, definition) in selected_definitions {
    let entry = snapshot
      .files
      .get(namespace)
      .and_then(|file| file.defs.get(definition))
      .ok_or_else(|| format!("Selected definition `{namespace}/{definition}` is missing from the source snapshot."))?;
    suggestions.extend(plan_core_nominal_method_source(
      snapshot,
      snapshot_file,
      namespace,
      definition,
      &entry.code,
      kind,
    )?);
  }
  Ok(suggestions)
}

/// Use the same nominal receiver proof for definition and attached executable source.
fn plan_core_nominal_method_source(
  snapshot: &Snapshot,
  snapshot_file: &str,
  namespace: &str,
  definition: &str,
  source: &Cirru,
  kind: CoreNominalMethodKind,
) -> Result<Vec<FixSuggestion>, String> {
  let mut suggestions = Vec::new();
  if list_head(source) == Some("defmacro") {
    return Ok(suggestions);
  }
  let mut local_bindings = HashSet::new();
  collect_potential_local_bindings(source, &mut local_bindings);
  let compiled = program::lookup_compiled_def(namespace, definition);
  let parsed = code_to_calcit(source, namespace, definition, vec![]).map_err(|error| error.to_string())?;
  let mut safe_calls = BTreeMap::<Vec<usize>, CoreNominalMethod>::new();
  let mut review_calls = BTreeMap::<Vec<usize>, String>::new();
  let warnings = RefCell::new(Vec::new());
  let usages = runner::preprocess::trace_source_usages(&parsed, namespace, definition, &warnings, &CallStackList::default())
    .map_err(|failure| failure.msg)?;
  let core_absent_heads = usages
    .iter()
    .filter(|usage| {
      usage.target_ns.as_ref() == "calcit.core"
        && match kind {
          CoreNominalMethodKind::Option => matches!(usage.target_def.as_ref(), "%none" | "Option"),
          CoreNominalMethodKind::Result => matches!(usage.target_def.as_ref(), "%err" | "Result"),
        }
    })
    .filter_map(|usage| usage.location.as_ref())
    .filter(|location| location.ns.as_ref() == namespace && location.def.as_ref() == definition)
    .map(|location| location.coord.iter().map(|value| usize::from(*value)).collect::<Vec<_>>())
    .collect::<HashSet<_>>();
  let (preprocessed, expressions) =
    runner::preprocess::trace_source_expressions(&parsed, namespace, definition, &RefCell::new(Vec::new()), &CallStackList::default())
      .map_err(|failure| failure.msg)?;
  for usage in usages {
    if usage.target_ns.as_ref() != "calcit.core" {
      continue;
    }
    let Some(method) = core_nominal_method(usage.target_def.as_ref(), kind) else {
      continue;
    };
    let Some(location) = usage.location else {
      continue;
    };
    if location.ns.as_ref() != namespace || location.def.as_ref() != definition {
      continue;
    }
    let leaf_path = location.coord.iter().map(|value| usize::from(*value)).collect::<Vec<_>>();
    if leaf_path.last() != Some(&0) {
      let node = navigate_to_path(source, &leaf_path)?;
      if matches!(&node, Cirru::Leaf(name) if core_nominal_method(name.as_ref(), kind) == Some(method)) {
        review_calls.insert(
          leaf_path,
          "The helper is referenced as a function value; a receiver method cannot replace it without changing its callable shape."
            .to_owned(),
        );
      }
      continue;
    }
    let Some((&0, call_path)) = leaf_path.split_last() else {
      continue;
    };
    let source_leaf = navigate_to_path(source, &leaf_path)?;
    if !matches!(&source_leaf, Cirru::Leaf(name) if core_nominal_method(name.as_ref(), kind) == Some(method)) {
      continue;
    }
    let call = navigate_to_path(source, call_path)?;
    let Cirru::List(items) = call else {
      continue;
    };
    if !usage
      .macro_origin
      .iter()
      .all(|origin| preserves_nominal_method_call_through_macro(origin))
    {
      review_calls.insert(
        call_path.to_vec(),
        format!(
          "The reference crosses macro expansion {}; review the source call manually.",
          usage.macro_origin.join(" -> ")
        ),
      );
    } else if items.len() != method.arity + 1 {
      review_calls.insert(
        call_path.to_vec(),
        format!(
          "The helper call does not have exactly {} argument(s); review this use manually.",
          method.arity
        ),
      );
    } else {
      let mut receiver_path = call_path.to_vec();
      receiver_path.push(1);
      let receiver = &items[1];
      let short_constructor_is_shadowed = matches!(receiver, Cirru::List(parts)
        if matches!(parts.first(), Some(Cirru::Leaf(name)) if name.as_ref() == kind.nominal()))
        && !core_nominal_type_is_unshadowed(snapshot, namespace, kind.nominal(), &local_bindings);
      let source_receiver = code_to_calcit(receiver, namespace, definition, vec![]).map_err(|error| error.to_string())?;
      // Reader shorthand is a call, not the located leaf inside that expansion.
      let direct_source_shape = !matches!(receiver, Cirru::Leaf(_)) || !matches!(source_receiver, Calcit::List(_));
      let inferred = compiled
        .as_ref()
        .filter(|_| direct_source_shape)
        .and_then(|compiled| {
          super::query::find_preprocessed_node_at_path(
            &compiled.preprocessed_code,
            namespace,
            definition,
            &receiver_path,
            matches!(receiver, Cirru::List(_)),
          )
        })
        .and_then(runner::preprocess::infer_static_type_from_expr)
        .or_else(|| {
          if !direct_source_shape {
            return None;
          }
          super::query::find_preprocessed_node_at_path(
            &preprocessed,
            namespace,
            definition,
            &receiver_path,
            matches!(receiver, Cirru::List(_)),
          )
          .and_then(runner::preprocess::infer_static_type_from_expr)
        })
        .or_else(|| {
          runner::preprocess::unique_source_expression_at_path(&expressions, namespace, definition, &receiver_path)
            .and_then(|item| item.inferred_type.clone())
        });
      let proven = !short_constructor_is_shadowed
        && (inferred.as_ref().is_some_and(|receiver_type| {
          let receiver_type = runner::preprocess::resolve_namespace_type_refs_for_body(receiver_type.clone(), namespace);
          let expected = format!("calcit.core/{}", kind.nominal());
          let arity = if kind == CoreNominalMethodKind::Option { 1 } else { 2 };
          let is_core_nominal = match receiver_type.as_ref() {
            CalcitTypeAnnotation::TypeRef(name, args) => name.as_ref() == expected && args.len() == arity,
            CalcitTypeAnnotation::Enum(def, args) => {
              def.definition_ref().is_some_and(|name| name.as_ref() == expected) && args.len() == arity
            }
            _ => false,
          };
          let contract = runner::preprocess::static_method_contract(receiver_type.as_ref(), method.method);
          is_core_nominal && contract.status == "proven" && contract.definition.as_deref() == Some(method.definition)
        }) || (method.method == ".unwrap-or"
          && matches!(receiver, Cirru::List(parts) if match kind {
            CoreNominalMethodKind::Option =>
              matches!(parts.as_slice(), [Cirru::Leaf(name)] if matches!(name.as_ref(), "%none" | "calcit.core/%none"))
                || matches!(parts.as_slice(), [Cirru::Leaf(name), Cirru::Leaf(variant)]
                  if matches!(name.as_ref(), "Option" | "calcit.core/Option") && variant.as_ref() == ":none"),
            CoreNominalMethodKind::Result =>
              matches!(parts.as_slice(), [Cirru::Leaf(name), _] if matches!(name.as_ref(), "%err" | "calcit.core/%err"))
                || matches!(parts.as_slice(), [Cirru::Leaf(name), Cirru::Leaf(variant), _]
                  if matches!(name.as_ref(), "Result" | "calcit.core/Result") && variant.as_ref() == ":err"),
          })
          && {
            let mut constructor_head_path = receiver_path.clone();
            constructor_head_path.push(0);
            core_absent_heads.contains(&constructor_head_path)
          }
          && {
            let mut fallback_path = call_path.to_vec();
            fallback_path.push(2);
            runner::preprocess::unique_source_expression_at_path(&expressions, namespace, definition, &fallback_path)
              .and_then(|item| item.inferred_type.clone())
              .or_else(|| {
                if matches!(&items[2], Cirru::Leaf(_)) {
                  code_to_calcit(&items[2], namespace, definition, vec![])
                    .ok()
                    .and_then(|value| runner::preprocess::infer_static_type_from_expr(&value))
                } else {
                  None
                }
              })
              .is_some_and(|fallback_type| {
                let fallback_type = runner::preprocess::resolve_namespace_type_refs_for_body(fallback_type, namespace);
                if matches!(
                  fallback_type.as_ref(),
                  CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::TypeVar(_)
                ) {
                  return false;
                }
                let args = match kind {
                  CoreNominalMethodKind::Option => vec![fallback_type],
                  CoreNominalMethodKind::Result => {
                    let Some(error_type) = inferred.as_ref().and_then(|receiver_type| match receiver_type.as_ref() {
                      CalcitTypeAnnotation::TypeRef(name, args)
                        if name.as_ref() == "calcit.core/Result" && matches!(args.as_slice(), [_, _]) =>
                      {
                        Some(args[1].clone())
                      }
                      CalcitTypeAnnotation::Enum(def, args)
                        if def.definition_ref().is_some_and(|name| name.as_ref() == "calcit.core/Result")
                          && matches!(args.as_slice(), [_, _]) =>
                      {
                        Some(args[1].clone())
                      }
                      _ => None,
                    }) else {
                      return false;
                    };
                    vec![fallback_type, error_type]
                  }
                };
                let narrowed = CalcitTypeAnnotation::TypeRef(Arc::from(format!("calcit.core/{}", kind.nominal())), Arc::new(args));
                let contract = runner::preprocess::static_method_contract(&narrowed, method.method);
                contract.status == "proven" && contract.definition.as_deref() == Some(method.definition)
              })
          }));
      if proven {
        safe_calls.insert(call_path.to_vec(), method);
      } else {
        review_calls.insert(
          call_path.to_vec(),
          format!(
            "The receiver type {} is not proven to dispatch to the matching {} method; retain the helper until its type is explicit.",
            inferred
              .as_ref()
              .map(|annotation| annotation.to_string())
              .unwrap_or_else(|| "unknown".to_owned()),
            kind.nominal()
          ),
        );
      }
    }
  }
  for (path, message) in review_calls {
    safe_calls.remove(&path);
    let node = navigate_to_path(source, &path)?;
    suggestions.push(FixSuggestion {
      rule_id: kind.rule_id(),
      diagnostic_code: kind.diagnostic(),
      semantic_layer: "surface",
      source_file: snapshot_file.to_owned(),
      definition: format!("{namespace}/{definition}"),
      path: format!("code{}", format_path(&path)),
      fingerprint: node_fingerprint(&node),
      origin_chain: vec![],
      original: quoted_json(&node),
      replacement: None,
      applicability: "requires-review",
      message,
      target_path: path,
      operation: None,
    });
  }
  let outermost = safe_calls
    .keys()
    .filter(|path| !safe_calls.keys().any(|other| other.len() < path.len() && path.starts_with(other)))
    .cloned()
    .collect::<Vec<_>>();
  for path in outermost {
    let original = navigate_to_path(source, &path)?;
    let replacement = rewrite_core_nominal_method_tree(&original, &mut path.clone(), &safe_calls);
    let original_code = original.format_one_liner().map_err(|error| {
      format!(
        "Failed to format {} helper source at {namespace}/{definition}: {error}",
        kind.nominal()
      )
    })?;
    let replacement_code = replacement.format_one_liner().map_err(|error| {
      format!(
        "Failed to format {} method replacement at {namespace}/{definition}: {error}",
        kind.nominal()
      )
    })?;
    suggestions.push(FixSuggestion {
      rule_id: kind.rule_id(),
      diagnostic_code: kind.diagnostic(),
      semantic_layer: "surface",
      source_file: snapshot_file.to_owned(),
      definition: format!("{namespace}/{definition}"),
      path: format!("code{}", format_path(&path)),
      fingerprint: node_fingerprint(&original),
      origin_chain: vec![serde_json::json!({"kind":format!("compiler-proven-{}-method", kind.nominal().to_ascii_lowercase()),"calls":safe_calls.keys().filter(|candidate| candidate.starts_with(&path)).count()})],
      original: quoted_json(&original),
      replacement: Some(quoted_json(&replacement)),
      applicability: "machine-applicable",
      message: format!("Use the proven {} receiver method; receiver and fallback expressions stay in their original order and are evaluated once.", kind.nominal()),
      target_path: path,
      operation: Some(FixOperation::ReplaceNode {
        original: original_code,
        replacement: replacement_code,
      }),
    });
  }
  Ok(suggestions)
}

#[derive(Clone, Copy)]
enum CorePredicateRename {
  Integer,
  Optionally,
  Join,
  Vals,
}

impl CorePredicateRename {
  /// Function aliases are distinct functions, so only direct calls are equivalent.
  fn requires_call_head(self) -> bool {
    matches!(self, Self::Optionally | Self::Join | Self::Vals)
  }

  fn names(self) -> (&'static str, &'static str, &'static str, &'static str) {
    match self {
      Self::Integer => ("round?", "integer?", CORE_INTEGER_PREDICATE_RULE, CORE_INTEGER_PREDICATE_DIAGNOSTIC),
      Self::Optionally => (
        "optionally",
        "nil->option",
        CORE_FUNCTION_ALIAS_RULE,
        CORE_FUNCTION_ALIAS_DIAGNOSTIC,
      ),
      Self::Join => ("join", "intersperse", CORE_FUNCTION_ALIAS_RULE, CORE_FUNCTION_ALIAS_DIAGNOSTIC),
      Self::Vals => ("vals", "distinct-values", CORE_FUNCTION_ALIAS_RULE, CORE_FUNCTION_ALIAS_DIAGNOSTIC),
    }
  }
}

fn supports_attached_migrations(rule: &str) -> bool {
  matches!(
    rule,
    REMOVED_DATA_API_RULE
      | CORE_FUNCTION_ALIAS_RULE
      | CORE_INTEGER_PREDICATE_RULE
      | CORE_NOMINAL_CONSTRUCTOR_RULE
      | CORE_OPTION_METHOD_RULE
      | CORE_RESULT_METHOD_RULE
      | CORE_IDENTITY_CONVERSION_RULE
      | CORE_LIST_ADD_RULE
      | CORE_COLLECTION_LEN_RULE
      | CORE_REF_CONSTRUCTOR_RULE
      | NAMED_ENUM_CONSTRUCTOR_RULE
      | NAMED_STRUCT_CONSTRUCTOR_RULE
      | REDUNDANT_DO_RULE
      | SINGLE_EXPRESSION_DO_RULE
  ) || CORE_EFFECT_METHOD_ALIASES.iter().any(|alias| alias.rule_id == rule)
}

/// Apply existing source proofs to one metadata region without registering synthetic definitions.
fn plan_attached_fixes(
  snapshot: &Snapshot,
  snapshot_file: &str,
  selected_definitions: &[(String, String)],
  selected_rules: &[&'static str],
) -> Result<Vec<FixSuggestion>, String> {
  let mut suggestions = Vec::new();
  for (namespace, definition) in selected_definitions {
    let entry = &snapshot.files[namespace].defs[definition];
    let mut regions = entry
      .tests
      .iter()
      .map(|test| {
        let mut tags = test.tags.iter().map(|tag| tag.ref_str()).collect::<Vec<_>>();
        tags.sort_unstable();
        (
          format!("tests.{}", test.name),
          vec![test.code.clone()],
          Some((test.name.clone(), tags.join(","))),
        )
      })
      .collect::<Vec<_>>();
    if !entry.examples.is_empty() {
      regions.push(("examples".to_owned(), entry.examples.clone(), None));
    }
    for (region, sources, test) in regions {
      let original = if test.is_some() {
        sources[0].clone()
      } else {
        Cirru::List(sources.clone())
      };
      let mut rewritten = sources;
      let mut origins = Vec::new();
      let mut blockers = Vec::new();
      // Rules that rewrote or blocked this region; the merged suggestion is labelled from them,
      // never from the preset's first rule.
      let mut region_rules = Vec::<&'static str>::new();
      for (index, source) in rewritten.iter_mut().enumerate() {
        for rule in [REDUNDANT_DO_RULE, SINGLE_EXPRESSION_DO_RULE] {
          if !selected_rules.contains(&rule) {
            continue;
          }
          match plan_attached_do_rewrite(
            source,
            namespace,
            &format!("&calcit:fix-attached:{definition}:{region}:{index}"),
            rule,
          ) {
            Ok(Some(rewrite)) => {
              *source = rewrite.code;
              origins.extend(rewrite.origin_chain);
              region_rules.push(rule);
            }
            Ok(None) => {}
            Err(error) => {
              blockers.push(format!("{namespace}/{definition} {region}[{index}]: {error}"));
              region_rules.push(rule);
            }
          }
        }
        for alias in [
          CorePredicateRename::Optionally,
          CorePredicateRename::Join,
          CorePredicateRename::Vals,
        ] {
          let (old, new, rule_id, _) = alias.names();
          if !selected_rules.contains(&rule_id) || !cirru_contains_target_reference(source, namespace, "calcit.core", old) {
            continue;
          }
          let label = format!("{namespace}/{definition} {region}[{index}]");
          match plan_attached_source_rewrite_with(
            source,
            namespace,
            &format!("&calcit:fix-attached:{definition}:{region}:{index}"),
            &label,
            AttachedRewriteTarget {
              namespace: "calcit.core",
              definition: old,
              allow_preserving_macros: true,
            },
            &|leaf, path| {
              if alias.requires_call_head() && path.last() != Some(&0) {
                return Err(format!("`{old}` is a first-class value; `{new}` has a different function identity"));
              }
              semantic_rename_leaf_replacement(leaf, old, "calcit.core", new).map(|_| Cirru::leaf(format!("calcit.core/{new}")))
            },
          ) {
            Ok(Some(rewrite)) => {
              *source = rewrite.code;
              origins.extend(rewrite.origin_chain.into_iter().map(|mut origin| {
                origin["rule_id"] = rule_id.into();
                origin
              }));
              region_rules.push(rule_id);
            }
            Ok(None) => {}
            Err(error) => {
              blockers.push(error);
              region_rules.push(rule_id);
            }
          }
        }
        // Re-prove each rule against the source produced by preceding rules.
        // Subtree replacements may change coordinates or fingerprints of nested calls.
        for selected_rule in selected_rules {
          let blockers_before = blockers.len();
          let selected_rules = std::slice::from_ref(selected_rule);
          let wrapper = Cirru::List(vec![Cirru::leaf("fn"), Cirru::List(vec![]), source.clone()]);
          let synthetic_def = format!("&calcit:fix-attached:{definition}:{region}:{index}");
          let mut candidates = Vec::new();
          if selected_rules.contains(&REMOVED_DATA_API_RULE) {
            match plan_removed_data_api_source(snapshot_file, namespace, &synthetic_def, &wrapper) {
              Ok(planned) => candidates.extend(planned),
              Err(error) => blockers.push(format!("{namespace}/{definition} {region}[{index}]: {error}")),
            }
          }
          let kinds = [NominalKind::Enum, NominalKind::Struct]
            .into_iter()
            .filter(|kind| selected_rules.contains(&kind.rule_id()))
            .collect::<Vec<_>>();
          if !kinds.is_empty() {
            match plan_named_constructor_source(
              snapshot,
              snapshot_file,
              (namespace, &synthetic_def),
              &wrapper,
              &kinds,
              (false, false),
            ) {
              Ok(planned) => candidates.extend(planned),
              Err(error) => blockers.push(format!("{namespace}/{definition} {region}[{index}]: {error}")),
            }
          }
          if selected_rules.contains(&CORE_NOMINAL_CONSTRUCTOR_RULE) {
            match plan_core_nominal_constructor_source(snapshot, snapshot_file, namespace, &synthetic_def, &wrapper) {
              Ok(planned) => candidates.extend(planned),
              Err(error) => blockers.push(format!("{namespace}/{definition} {region}[{index}]: {error}")),
            }
          }
          for kind in [CoreNominalMethodKind::Option, CoreNominalMethodKind::Result] {
            if !selected_rules.contains(&kind.rule_id()) {
              continue;
            }
            match plan_core_nominal_method_source(snapshot, snapshot_file, namespace, &synthetic_def, &wrapper, kind) {
              Ok(planned) => candidates.extend(planned),
              Err(error) => blockers.push(format!("{namespace}/{definition} {region}[{index}]: {error}")),
            }
          }
          if selected_rules.contains(&CORE_INTEGER_PREDICATE_RULE) {
            match plan_core_predicate_rename_source(snapshot_file, namespace, &synthetic_def, &wrapper, CorePredicateRename::Integer) {
              Ok(planned) => candidates.extend(planned),
              Err(error) => blockers.push(format!("{namespace}/{definition} {region}[{index}]: {error}")),
            }
          }
          if selected_rules.contains(&CORE_IDENTITY_CONVERSION_RULE) {
            match plan_core_identity_conversion_source(snapshot, snapshot_file, namespace, &synthetic_def, &wrapper) {
              Ok(planned) => candidates.extend(planned),
              Err(error) => blockers.push(format!("{namespace}/{definition} {region}[{index}]: {error}")),
            }
          }
          if selected_rules.contains(&CORE_LIST_ADD_RULE) {
            match plan_core_list_add_source(snapshot_file, namespace, &synthetic_def, &wrapper) {
              Ok(planned) => candidates.extend(planned),
              Err(error) => blockers.push(format!("{namespace}/{definition} {region}[{index}]: {error}")),
            }
          }
          if selected_rules.contains(&CORE_COLLECTION_LEN_RULE) {
            match plan_core_collection_len_source(snapshot_file, namespace, &synthetic_def, &wrapper) {
              Ok(planned) => candidates.extend(planned),
              Err(error) => blockers.push(format!("{namespace}/{definition} {region}[{index}]: {error}")),
            }
          }
          if selected_rules.contains(&CORE_REF_CONSTRUCTOR_RULE) {
            candidates.extend(plan_core_ref_constructor_source(
              snapshot,
              snapshot_file,
              namespace,
              &synthetic_def,
              &wrapper,
            ));
          }
          for alias in CORE_EFFECT_METHOD_ALIASES
            .iter()
            .filter(|alias| selected_rules.contains(&alias.rule_id))
          {
            match plan_core_method_alias_source(snapshot_file, namespace, &synthetic_def, &wrapper, None, *alias) {
              Ok(planned) => candidates.extend(planned),
              Err(error) => blockers.push(format!("{namespace}/{definition} {region}[{index}]: {error}")),
            }
          }
          if blockers.len() > blockers_before {
            region_rules.push(selected_rule);
          }
          for candidate in candidates {
            region_rules.push(candidate.rule_id);
            let Some(path) = candidate.target_path.strip_prefix(&[2]) else {
              blockers.push("Compiler suggestion points outside its attached source wrapper.".to_owned());
              continue;
            };
            match candidate.operation {
              Some(FixOperation::ReplaceNode { .. }) => {
                let original = fix_source_json_to_cirru(&candidate.original)?;
                let replacement =
                  fix_source_json_to_cirru(candidate.replacement.as_ref().ok_or("Missing attached subtree replacement")?)?;
                replace_attached_source_tree(source, path, &original, &replacement)?;
                origins.extend(candidate.origin_chain);
                origins.push(
                  serde_json::json!({"kind": "resolved-attached-source", "path": format_path(path), "rule_id": candidate.rule_id}),
                );
              }
              Some(FixOperation::ReplaceLeaf { original, replacement }) => {
                replace_attached_source_node(source, path, &original, &Cirru::leaf(replacement))?;
                origins.extend(candidate.origin_chain);
                origins.push(
                  serde_json::json!({"kind": "resolved-attached-source", "path": format_path(path), "rule_id": candidate.rule_id}),
                );
              }
              None => blockers.push(candidate.message),
              _ => return Err("Attached source proof produced an unsupported operation.".to_owned()),
            }
          }
        }
      }
      if origins.is_empty() && blockers.is_empty() {
        continue;
      }
      // One region-level operation may merge several rules; report the earliest one in the
      // selected (preset) order, while each origin entry keeps its own `rule_id`.
      let Some(rule_id) = selected_rules
        .iter()
        .copied()
        .find(|rule| region_rules.contains(rule))
        .or_else(|| region_rules.first().copied())
      else {
        return Err(format!(
          "{namespace}/{definition} {region}: attached rewrite has no contributing rule among the selected rules."
        ));
      };
      let metadata = fix_rule_metadata(rule_id);
      // Merge all alias replacements in a metadata region into one operation.
      // A blocked region is never partially rewritten or used as a test oracle.
      let safe = blockers.is_empty();
      let replacement = if test.is_some() {
        rewritten[0].clone()
      } else {
        Cirru::List(rewritten.clone())
      };
      let operation = if safe {
        let code = format_quoted_nodes(&rewritten)?;
        Some(match test {
          Some((name, tags)) => FixOperation::ReplaceTest { name, tags, code },
          None => FixOperation::ReplaceExamples { code },
        })
      } else {
        None
      };
      suggestions.push(FixSuggestion {
        rule_id: metadata.rule_id,
        diagnostic_code: metadata.diagnostic_code,
        semantic_layer: "surface",
        source_file: snapshot_file.to_owned(),
        definition: format!("{namespace}/{definition}"),
        path: region,
        fingerprint: node_fingerprint(&original),
        origin_chain: origins,
        original: quoted_json(&original),
        replacement: safe.then(|| quoted_json(&replacement)),
        applicability: if safe { "machine-applicable" } else { "requires-review" },
        message: if safe {
          "Apply proven source rewrites without changing assertions, tags, or argument evaluation.".to_owned()
        } else {
          blockers.join("; ")
        },
        target_path: vec![],
        operation,
      });
    }
  }
  Ok(suggestions)
}

fn plan_core_predicate_rename_fixes(
  snapshot: &Snapshot,
  snapshot_file: &str,
  selected_definitions: &[(String, String)],
  rule: CorePredicateRename,
) -> Result<Vec<FixSuggestion>, String> {
  let (old_name, _, _, _) = rule.names();
  let mut suggestions = Vec::new();
  for (namespace, definition) in selected_definitions {
    let entry = snapshot
      .files
      .get(namespace)
      .and_then(|file| file.defs.get(definition))
      .ok_or_else(|| format!("Selected definition `{namespace}/{definition}` is missing from the source snapshot."))?;
    if list_head(&entry.code) == Some("defmacro") {
      continue;
    }
    if !matches!(rule, CorePredicateRename::Integer)
      && !program::lookup_def_id("calcit.core", old_name).is_some_and(|target| {
        program::lookup_compiled_def(namespace, definition).is_some_and(|compiled| compiled.deps.contains(&target))
      })
    {
      // Avoid re-tracing unrelated declaration definitions.
      continue;
    }
    suggestions.extend(plan_core_predicate_rename_source(
      snapshot_file,
      namespace,
      definition,
      &entry.code,
      rule,
    )?);
  }
  Ok(suggestions)
}

/// Share resolved predicate references and reader call-head proofs with attached source.
fn plan_core_predicate_rename_source(
  snapshot_file: &str,
  namespace: &str,
  definition: &str,
  source: &Cirru,
  rule: CorePredicateRename,
) -> Result<Vec<FixSuggestion>, String> {
  let (old_name, new_name, rule_id, diagnostic_code) = rule.names();
  let mut suggestions = Vec::new();
  if list_head(source) == Some("defmacro") {
    return Ok(suggestions);
  }
  let mut integer_heads = Vec::new();
  if matches!(rule, CorePredicateRename::Integer) {
    let mut local_bindings = HashSet::new();
    collect_potential_local_bindings(source, &mut local_bindings);
    if local_bindings.contains(old_name) {
      return Ok(suggestions);
    }
    collect_builtin_round_call_heads(source, &mut Vec::new(), &mut integer_heads);
    if integer_heads.is_empty() {
      return Ok(suggestions);
    }
  }

  let parsed = code_to_calcit(source, namespace, definition, vec![]).map_err(|error| error.to_string())?;
  let usages =
    runner::preprocess::trace_source_usages(&parsed, namespace, definition, &RefCell::new(Vec::new()), &CallStackList::default())
      .map_err(|failure| failure.msg)?;
  let mut planned = BTreeMap::<Vec<usize>, (String, String, Vec<String>, Option<String>)>::new();
  for usage in &usages {
    if usage.target_ns.as_ref() != "calcit.core" || usage.target_def.as_ref() != old_name {
      continue;
    }
    let Some(location) = &usage.location else {
      continue;
    };
    if location.ns.as_ref() != namespace || location.def.as_ref() != definition {
      continue;
    }
    let path = location.coord.iter().map(|value| usize::from(*value)).collect::<Vec<_>>();
    let node = navigate_to_path(source, &path)?;
    let Cirru::Leaf(source_leaf) = node else {
      continue;
    };
    let Ok(replacement) = semantic_rename_leaf_replacement(source_leaf.as_ref(), old_name, "calcit.core", new_name) else {
      continue;
    };
    let macro_origin = usage.macro_origin.clone();
    // Preferred aliases are separate forwarding functions, so a first-class
    // reference changes function identity even though direct calls agree.
    let first_class_alias = rule.requires_call_head() && path.last() != Some(&0);
    let review = if first_class_alias {
      Some(format!(
        "`{old_name}` is used as a first-class value, and `{new_name}` is a separate function with its own identity; review before renaming."
      ))
    } else {
      (!macro_origin
        .iter()
        .all(|origin| preserves_nominal_method_call_through_macro(origin)))
      .then(|| {
        format!(
          "The core reference crosses macro expansion {}; review whether the macro observes the source spelling.",
          macro_origin.join(" -> ")
        )
      })
    };
    planned
      .entry(path)
      .and_modify(|(_, _, origins, current_review)| {
        for origin in &macro_origin {
          if !origins.contains(origin) {
            origins.push(origin.clone());
          }
        }
        if current_review.is_none() {
          *current_review = review.clone();
        }
      })
      .or_insert_with(|| (source_leaf.to_string(), replacement, macro_origin, review));
  }

  if matches!(rule, CorePredicateRename::Integer) {
    // The Cirru reader resolves `round?` directly to a built-in Proc before
    // preprocessing, so it has no source location in definition-usage traces.
    // A source call head is therefore sufficient evidence for this one proc.
    for head_path in integer_heads {
      let mut call_path = head_path.clone();
      call_path.pop();
      let review = (!method_source_context_is_stable(source, &call_path, namespace, definition, &usages)).then(|| {
        "The built-in predicate call crosses an unknown source context; review whether macro expansion observes its spelling."
          .to_owned()
      });
      planned.insert(
        head_path,
        (old_name.to_owned(), format!("calcit.core/{new_name}"), Vec::new(), review),
      );
    }
  }

  for (target_path, (original_leaf, replacement, macro_origin, review)) in planned {
    let original_node = Cirru::leaf(original_leaf.as_str());
    let replacement_node = Cirru::leaf(replacement.as_str());
    let machine_applicable = review.is_none();
    let message = review.unwrap_or_else(|| format!("Use the preferred core name `{new_name}`; the direct calls have equivalent results and preserve argument evaluation. First-class function identity is not rewritten."));
    suggestions.push(FixSuggestion {
        rule_id,
        diagnostic_code,
        semantic_layer: "surface",
        source_file: snapshot_file.to_owned(),
        definition: format!("{namespace}/{definition}"),
        path: format!("code{}", format_path(&target_path)),
        fingerprint: node_fingerprint(&original_node),
        origin_chain: vec![serde_json::json!({
          "kind": if matches!(rule, CorePredicateRename::Integer) { "reader-resolved-builtin-proc" } else { "compiler-resolved-core-predicate" },
          "target": format!("calcit.core/{old_name}"),
          "replacement": format!("calcit.core/{new_name}"),
          "macro_origin": macro_origin,
        })],
        original: quoted_json(&original_node),
        replacement: machine_applicable.then(|| quoted_json(&replacement_node)),
        applicability: if machine_applicable {
          "machine-applicable"
        } else {
          "requires-review"
        },
        message,
        target_path,
        operation: machine_applicable.then_some(FixOperation::ReplaceLeaf {
          original: original_leaf,
          replacement,
        }),
      });
  }
  Ok(suggestions)
}

/// Plan `atom` -> `ref` and `defatom` -> `defref` in definition code.
fn plan_core_ref_constructor_fixes(
  snapshot: &Snapshot,
  snapshot_file: &str,
  selected_definitions: &[(String, String)],
) -> Result<Vec<FixSuggestion>, String> {
  let mut suggestions = Vec::new();
  for (namespace, definition) in selected_definitions {
    let entry = snapshot
      .files
      .get(namespace)
      .and_then(|file| file.defs.get(definition))
      .ok_or_else(|| format!("Selected definition `{namespace}/{definition}` is missing from the source snapshot."))?;
    suggestions.extend(plan_core_ref_constructor_source(
      snapshot,
      snapshot_file,
      namespace,
      definition,
      &entry.code,
    ));
  }
  Ok(suggestions)
}

/// The reader turns both `atom` and `ref` into the same built-in proc before any
/// scope lookup, so any evaluated `atom` leaf keeps its identity after the
/// rename, including inside a quasiquote template: the template already holds
/// `(&proc ref)` and no local, definition or import can shadow either spelling
/// at the expansion site. `defatom` is
/// core syntax resolved by name, so it is renamed only as a call head that no
/// namespace definition, import, local binding, macro template or unknown
/// macro context can observe; other occurrences require review.
fn plan_core_ref_constructor_source(
  snapshot: &Snapshot,
  snapshot_file: &str,
  namespace: &str,
  definition: &str,
  source: &Cirru,
) -> Vec<FixSuggestion> {
  let mut occurrences = Vec::new();
  collect_ref_constructor_leaves(source, &mut Vec::new(), false, &mut occurrences);
  if occurrences.is_empty() {
    return Vec::new();
  }
  let mut local_bindings = HashSet::new();
  collect_potential_local_bindings(source, &mut local_bindings);
  let shadowed = |name: &str| local_bindings.contains(name) || namespace_binds_name(snapshot, namespace, name);
  occurrences
    .into_iter()
    .map(|(target_path, old_name, in_template)| {
      let syntax = old_name == "defatom";
      let new_name = if syntax { "defref" } else { "ref" };
      let review = if !syntax {
        None
      } else if target_path.last() != Some(&0) {
        Some("`defatom` is not a call head here; review whether this leaf is meant as core syntax.".to_owned())
      } else if let Some(name) = ["defatom", "defref"].into_iter().find(|name| shadowed(name)) {
        Some(format!(
          "`{name}` is bound by a local, a namespace definition or an import in `{namespace}`; review which definition the call resolves to."
        ))
      } else if in_template {
        Some("`defatom` is inside a quasiquote template and resolves at each expansion site; review the macro users.".to_owned())
      } else if !ref_definition_context_is_stable(source, &target_path) {
        Some("`defatom` is an argument of an unknown macro that may observe its spelling; review before renaming.".to_owned())
      } else {
        None
      };
      let original_node = Cirru::leaf(old_name);
      let replacement_node = Cirru::leaf(new_name);
      let machine_applicable = review.is_none();
      FixSuggestion {
        rule_id: CORE_REF_CONSTRUCTOR_RULE,
        diagnostic_code: CORE_REF_CONSTRUCTOR_DIAGNOSTIC,
        semantic_layer: "surface",
        source_file: snapshot_file.to_owned(),
        definition: format!("{namespace}/{definition}"),
        path: format!("code{}", format_path(&target_path)),
        fingerprint: node_fingerprint(&original_node),
        origin_chain: vec![serde_json::json!({
          "kind": if syntax { "unshadowed-core-syntax" } else { "reader-resolved-builtin-proc" },
          "target": format!("calcit.core/{old_name}"),
          "replacement": format!("calcit.core/{new_name}"),
        })],
        original: quoted_json(&original_node),
        replacement: machine_applicable.then(|| quoted_json(&replacement_node)),
        applicability: if machine_applicable {
          "machine-applicable"
        } else {
          "requires-review"
        },
        message: review.unwrap_or_else(|| {
          format!(
            "Use the preferred Ref constructor `{new_name}`; `{old_name}` is the same built-in implementation, so evaluation and Ref identity are unchanged."
          )
        }),
        target_path,
        operation: machine_applicable.then(|| FixOperation::ReplaceLeaf {
          original: old_name.to_owned(),
          replacement: new_name.to_owned(),
        }),
      }
    })
    .collect()
}

/// Collect `atom` / `defatom` leaves outside quoted data and comments, marking
/// leaves that sit in a quasiquote template outside any unquote.
fn collect_ref_constructor_leaves(
  node: &Cirru,
  path: &mut Vec<usize>,
  in_template: bool,
  output: &mut Vec<(Vec<usize>, &'static str, bool)>,
) {
  match node {
    Cirru::Leaf(leaf) => match leaf.as_ref() {
      "atom" => output.push((path.clone(), "atom", in_template)),
      "defatom" => output.push((path.clone(), "defatom", in_template)),
      _ => {}
    },
    Cirru::List(items) => {
      let in_template = match items.first().and_then(leaf_value) {
        Some("quote" | "cirru-quote" | ";") => return,
        Some("quasiquote") => true,
        Some("~" | "~@") => false,
        _ => in_template,
      };
      for (index, child) in items.iter().enumerate() {
        path.push(index);
        collect_ref_constructor_leaves(child, path, in_template, output);
        path.pop();
      }
    }
  }
}

/// Whether `name` is defined in, or imported into, the namespace. An unreadable
/// ns form counts as bound so that the caller asks for review.
fn namespace_binds_name(snapshot: &Snapshot, namespace: &str, name: &str) -> bool {
  let Some(file) = snapshot.files.get(namespace) else {
    return true;
  };
  file.defs.contains_key(name)
    || program::extract_import_map(&file.ns.code, namespace).map_or(true, |imports| imports.contains_key(name))
}

/// Every enclosing form of the call must be a known binding/control form or a
/// built-in proc call, so no user macro receives the syntax spelling.
fn ref_definition_context_is_stable(code: &Cirru, head_path: &[usize]) -> bool {
  let call_path = &head_path[..head_path.len().saturating_sub(1)];
  (0..call_path.len()).all(|depth| {
    let Ok(Cirru::List(items)) = navigate_to_path(code, &call_path[..depth]) else {
      return false;
    };
    match items.first() {
      Some(Cirru::Leaf(head)) => {
        matches!(
          head.as_ref(),
          "defn" | "defwasm-export" | "fn" | "let" | "&let" | "do" | "if" | "when" | "when-not" | "cond"
        ) || head.as_ref().parse::<CalcitProc>().is_ok()
      }
      _ => false,
    }
  })
}

fn collect_builtin_round_call_heads(node: &Cirru, path: &mut Vec<usize>, heads: &mut Vec<Vec<usize>>) {
  let Cirru::List(items) = node else {
    return;
  };
  if matches!(items.first(), Some(Cirru::Leaf(head)) if matches!(head.as_ref(), "quote" | "quasiquote")) {
    return;
  }
  if items.len() == 2 && matches!(items.first(), Some(Cirru::Leaf(head)) if head.as_ref() == "round?") {
    let mut head_path = path.clone();
    head_path.push(0);
    heads.push(head_path);
  }
  for (index, child) in items.iter().enumerate() {
    path.push(index);
    collect_builtin_round_call_heads(child, path, heads);
    path.pop();
  }
}

/// Collect complete built-in conversion calls without entering quoted source.
fn collect_identity_conversion_calls(node: &Cirru, path: &mut Vec<usize>, calls: &mut Vec<(Vec<usize>, Vec<usize>, Vec<usize>)>) {
  let Cirru::List(items) = node else {
    return;
  };
  if matches!(items.first(), Some(Cirru::Leaf(head)) if matches!(head.as_ref(), "quote" | "quasiquote")) {
    return;
  }
  if items.len() == 2
    && matches!(items.first(), Some(Cirru::Leaf(head)) if matches!(head.as_ref(), "turn-symbol" | "turn-tag" | "turn-string"))
  {
    let mut head_path = path.clone();
    head_path.push(0);
    let mut argument_path = path.clone();
    argument_path.push(1);
    calls.push((path.clone(), head_path, argument_path));
  }
  for (index, child) in items.iter().enumerate() {
    path.push(index);
    collect_identity_conversion_calls(child, path, calls);
    path.pop();
  }
}

/// Rename resolved core conversions only when the argument proves the same conversion.
fn plan_core_identity_conversion_fixes(
  snapshot: &Snapshot,
  snapshot_file: &str,
  selected_definitions: &[(String, String)],
) -> Result<Vec<FixSuggestion>, String> {
  let mut suggestions = Vec::new();
  for (namespace, definition) in selected_definitions {
    let file = snapshot
      .files
      .get(namespace)
      .ok_or_else(|| format!("Selected namespace `{namespace}` is missing from the source snapshot."))?;
    let entry = file
      .defs
      .get(definition)
      .ok_or_else(|| format!("Selected definition `{namespace}/{definition}` is missing from the source snapshot."))?;
    suggestions.extend(plan_core_identity_conversion_source(
      snapshot,
      snapshot_file,
      namespace,
      definition,
      &entry.code,
    )?);
  }
  Ok(suggestions)
}

/// Share the existing source and receiver proof with attached executable expressions.
fn plan_core_identity_conversion_source(
  snapshot: &Snapshot,
  snapshot_file: &str,
  namespace: &str,
  definition: &str,
  source: &Cirru,
) -> Result<Vec<FixSuggestion>, String> {
  let mut suggestions = Vec::new();
  let file = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Selected namespace `{namespace}` is missing from the source snapshot."))?;
  if list_head(source) == Some("defmacro") {
    return Ok(suggestions);
  }
  let mut calls = Vec::new();
  collect_identity_conversion_calls(source, &mut Vec::new(), &mut calls);
  if calls.is_empty() {
    return Ok(suggestions);
  }
  let mut local_bindings = HashSet::new();
  collect_potential_local_bindings(source, &mut local_bindings);
  let compiled = program::lookup_compiled_def(namespace, definition);
  let parsed = code_to_calcit(source, namespace, definition, vec![]).map_err(|error| error.to_string())?;
  let (preprocessed, expressions) =
    runner::preprocess::trace_source_expressions(&parsed, namespace, definition, &RefCell::new(Vec::new()), &CallStackList::default())
      .map_err(|failure| failure.msg)?;
  let usages =
    runner::preprocess::trace_source_usages(&parsed, namespace, definition, &RefCell::new(Vec::new()), &CallStackList::default())
      .map_err(|failure| failure.msg)?;
  for (call_path, head_path, argument_path) in calls {
    let head = navigate_to_path(source, &head_path)?;
    let Cirru::Leaf(old_name) = &head else {
      continue;
    };
    if local_bindings.contains(old_name.as_ref()) || file.defs.contains_key(old_name.as_ref()) {
      continue;
    }
    let expected_proc = match old_name.as_ref() {
      "turn-symbol" => CalcitProc::TurnSymbol,
      "turn-tag" => CalcitProc::TurnTag,
      "turn-string" => CalcitProc::TurnString,
      _ => continue,
    };
    let parsed_head = code_to_calcit(
      &head,
      namespace,
      definition,
      head_path
        .iter()
        .map(|index| u16::try_from(*index).map_err(|_| format!("Path index {index} exceeds Snapshot coordinate range")))
        .collect::<Result<Vec<_>, _>>()?,
    )
    .map_err(|error| error.to_string())?;
    let resolved_core_wrapper = old_name.as_ref() == "turn-string"
      && matches!(parsed_head, Calcit::Symbol { .. })
      && usages.iter().any(|usage| {
        usage.target_ns.as_ref() == calcit::calcit::CORE_NS
          && usage.target_def.as_ref() == "turn-string"
          && usage.location.as_ref().is_some_and(|location| {
            location.ns.as_ref() == namespace
              && location.def.as_ref() == definition
              && location.coord.iter().map(|index| usize::from(*index)).eq(head_path.iter().copied())
          })
      });
    if !matches!(parsed_head, Calcit::Proc(proc) if proc == expected_proc) && !resolved_core_wrapper {
      continue;
    }
    let argument = navigate_to_path(source, &argument_path)?;
    let parsed_argument = code_to_calcit(
      &argument,
      namespace,
      definition,
      argument_path
        .iter()
        .map(|index| u16::try_from(*index).map_err(|_| format!("Path index {index} exceeds Snapshot coordinate range")))
        .collect::<Result<Vec<_>, _>>()?,
    )
    .map_err(|error| error.to_string())?;
    let direct_source_shape = !matches!(argument, Cirru::Leaf(_)) || !matches!(parsed_argument, Calcit::List(_));
    let inferred = compiled
      .as_ref()
      .filter(|_| direct_source_shape)
      .and_then(|compiled| {
        super::query::find_preprocessed_node_at_path(
          &compiled.preprocessed_code,
          namespace,
          definition,
          &argument_path,
          matches!(argument, Cirru::List(_)),
        )
      })
      .and_then(runner::preprocess::infer_static_type_from_expr)
      .or_else(|| {
        if !direct_source_shape {
          return None;
        }
        super::query::find_preprocessed_node_at_path(
          &preprocessed,
          namespace,
          definition,
          &argument_path,
          matches!(argument, Cirru::List(_)),
        )
        .and_then(runner::preprocess::infer_static_type_from_expr)
      })
      .or_else(|| {
        runner::preprocess::unique_source_expression_at_path(&expressions, namespace, definition, &argument_path)
          .and_then(|item| item.inferred_type.clone())
      })
      .or_else(|| super::query::infer_type_at_target(&parsed_argument, None));
    let resolved = inferred
      .as_ref()
      .map(|annotation| runner::preprocess::resolve_namespace_type_refs_for_body(annotation.clone(), namespace));
    let proven_builtin_scalar = resolved.as_ref().is_some_and(|annotation| {
      matches!(
        annotation.as_ref(),
        CalcitTypeAnnotation::Nil
          | CalcitTypeAnnotation::Bool
          | CalcitTypeAnnotation::Number
          | CalcitTypeAnnotation::String
          | CalcitTypeAnnotation::Tag
          | CalcitTypeAnnotation::Symbol
      )
    });
    let proven_argument = if expected_proc == CalcitProc::TurnString {
      proven_builtin_scalar
    } else {
      resolved
        .as_ref()
        .is_some_and(|annotation| matches!(annotation.as_ref(), CalcitTypeAnnotation::String))
    };
    if resolved.as_ref().is_some_and(|annotation| {
      !proven_argument
        && !matches!(
          annotation.as_ref(),
          CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::TypeVar(_)
        )
    }) {
      continue;
    }
    let argument_crosses_macro = usages.iter().any(|usage| {
      let Some(location) = &usage.location else {
        return false;
      };
      location.ns.as_ref() == namespace
        && location.def.as_ref() == definition
        && location.coord.len() > argument_path.len()
        && location
          .coord
          .iter()
          .map(|index| usize::from(*index))
          .zip(argument_path.iter().copied())
          .all(|(index, expected)| index == expected)
        && (!usage.macro_origin.is_empty()
          || matches!(
            program::lookup_compiled_def(&usage.target_ns, &usage.target_def).map(|compiled| compiled.kind),
            Some(program::CompiledDefKind::Macro)
          ))
    });
    let machine_applicable =
      proven_argument && !argument_crosses_macro && method_source_context_is_stable(source, &call_path, namespace, definition, &usages);
    let new_name = match expected_proc {
      CalcitProc::TurnTag => "to-tag",
      CalcitProc::TurnSymbol => "to-symbol",
      CalcitProc::TurnString => "to-string",
      _ => unreachable!("only supported conversions are collected"),
    };
    let replacement = format!("calcit.core/{new_name}");
    let replacement_node = Cirru::leaf(replacement.as_str());
    suggestions.push(FixSuggestion {
      rule_id: CORE_IDENTITY_CONVERSION_RULE,
      diagnostic_code: CORE_IDENTITY_CONVERSION_DIAGNOSTIC,
      semantic_layer: "surface",
      source_file: snapshot_file.to_owned(),
      definition: format!("{namespace}/{definition}"),
      path: format!("code{}", format_path(&head_path)),
      fingerprint: node_fingerprint(&head),
      origin_chain: vec![serde_json::json!({
        "kind": "resolved-core-conversion-and-argument-type",
        "old_proc": old_name.as_ref(),
        "argument_type": inferred.as_ref().map(|annotation| annotation.describe()),
        "target": format!("calcit.core/{new_name}"),
      })],
      original: quoted_json(&head),
      replacement: machine_applicable.then(|| quoted_json(&replacement_node)),
      applicability: if machine_applicable {
        "machine-applicable"
      } else {
        "requires-review"
      },
      message: if machine_applicable {
        format!("Use `{new_name}` for a proven built-in argument; both paths call the same conversion once.")
      } else if argument_crosses_macro {
        format!("The argument to `{old_name}` crosses macro expansion; review this conversion manually.")
      } else {
        format!("Cannot prove an equivalent argument and stable source context for `{old_name}`; review this conversion manually.")
      },
      target_path: head_path,
      operation: machine_applicable.then_some(FixOperation::ReplaceLeaf {
        original: old_name.to_string(),
        replacement,
      }),
    });
  }
  Ok(suggestions)
}

/// Locate only one-argument method calls; a first-class method value cannot be renamed safely.
fn collect_list_add_calls(node: &Cirru, path: &mut Vec<usize>, calls: &mut Vec<(Vec<usize>, Vec<usize>, Vec<usize>)>) {
  let Cirru::List(items) = node else {
    return;
  };
  if matches!(items.first(), Some(Cirru::Leaf(head)) if matches!(head.as_ref(), "quote" | "quasiquote")) {
    return;
  }
  if items.len() == 3 {
    let indices = if matches!(&items[0], Cirru::Leaf(name) if name.as_ref() == ".add") {
      Some((0, 1))
    } else if matches!(&items[1], Cirru::Leaf(name) if name.as_ref() == ".add") {
      Some((1, 0))
    } else {
      None
    };
    if let Some((method_index, receiver_index)) = indices {
      let mut method_path = path.clone();
      method_path.push(method_index);
      let mut receiver_path = path.clone();
      receiver_path.push(receiver_index);
      calls.push((path.clone(), method_path, receiver_path));
    }
  }
  for (index, child) in items.iter().enumerate() {
    path.push(index);
    collect_list_add_calls(child, path, calls);
    path.pop();
  }
}

/// Allow evaluated call arguments only when the reader or resolver proves their enclosing call is not an unknown macro.
fn method_source_context_is_stable(
  code: &Cirru,
  call_path: &[usize],
  namespace: &str,
  definition: &str,
  usages: &[runner::preprocess::ResolvedSourceUsage],
) -> bool {
  (0..call_path.len()).all(|depth| {
    let Ok(Cirru::List(items)) = navigate_to_path(code, &call_path[..depth]) else {
      return false;
    };
    let Some(head) = items.first() else {
      return false;
    };
    if matches!(head, Cirru::Leaf(head) if matches!(head.as_ref(), "defn" | "defwasm-export" | "fn" | "let" | "let[]" | "do" | "if" | "cond")) {
      return true;
    }
    if matches!(
      code_to_calcit(head, namespace, definition, Vec::new()),
      Ok(Calcit::Proc(..) | Calcit::Method(..))
    ) {
      return true;
    }
    let mut head_path = call_path[..depth].to_vec();
    head_path.push(0);
    let matching = usages
      .iter()
      .filter(|usage| {
        usage.location.as_ref().is_some_and(|location| {
          location.ns.as_ref() == namespace
            && location.def.as_ref() == definition
            && location.coord.iter().map(|index| usize::from(*index)).eq(head_path.iter().copied())
        })
      })
      .collect::<Vec<_>>();
    !matching.is_empty()
      && matching.iter().all(|usage| {
        usage
          .macro_origin
          .iter()
          .all(|origin| preserves_nominal_method_call_through_macro(origin))
          && (matches!(
            program::lookup_compiled_def(&usage.target_ns, &usage.target_def).map(|compiled| compiled.kind),
            Some(program::CompiledDefKind::Fn | program::CompiledDefKind::Proc)
          ) || preserves_nominal_method_call_through_macro(&format!("{}/{}", usage.target_ns, usage.target_def)))
      })
  })
}

/// Plan List `.add` migrations only when source location and method contracts are proven.
fn plan_core_list_add_fixes(
  snapshot: &Snapshot,
  snapshot_file: &str,
  selected_definitions: &[(String, String)],
) -> Result<Vec<FixSuggestion>, String> {
  let mut suggestions = Vec::new();
  for (namespace, definition) in selected_definitions {
    let entry = snapshot
      .files
      .get(namespace)
      .and_then(|file| file.defs.get(definition))
      .ok_or_else(|| format!("Selected definition `{namespace}/{definition}` is missing from the source snapshot."))?;
    suggestions.extend(plan_core_list_add_source(snapshot_file, namespace, definition, &entry.code)?);
  }
  Ok(suggestions)
}

/// Share the existing source and receiver proof with attached executable expressions.
fn plan_core_list_add_source(
  snapshot_file: &str,
  namespace: &str,
  definition: &str,
  source: &Cirru,
) -> Result<Vec<FixSuggestion>, String> {
  let mut suggestions = Vec::new();
  if list_head(source) == Some("defmacro") {
    return Ok(suggestions);
  }
  let mut calls = Vec::new();
  collect_list_add_calls(source, &mut Vec::new(), &mut calls);
  if calls.is_empty() {
    return Ok(suggestions);
  }
  let parsed = code_to_calcit(source, namespace, definition, vec![]).map_err(|error| error.to_string())?;
  let usages =
    runner::preprocess::trace_source_usages(&parsed, namespace, definition, &RefCell::new(Vec::new()), &CallStackList::default())
      .map_err(|failure| failure.msg)?;
  let compiled = program::lookup_compiled_def(namespace, definition);
  let (preprocessed, expressions) =
    runner::preprocess::trace_source_expressions(&parsed, namespace, definition, &RefCell::new(Vec::new()), &CallStackList::default())
      .map_err(|failure| failure.msg)?;
  for (call_path, method_path, receiver_path) in calls {
    let receiver = navigate_to_path(source, &receiver_path)?;
    let source_receiver = code_to_calcit(
      &receiver,
      namespace,
      definition,
      receiver_path
        .iter()
        .map(|index| u16::try_from(*index).map_err(|_| format!("Path index {index} exceeds Snapshot coordinate range")))
        .collect::<Result<Vec<_>, _>>()?,
    )
    .map_err(|error| error.to_string())?;
    let direct_source_shape = !matches!(receiver, Cirru::Leaf(_)) || !matches!(source_receiver, Calcit::List(_));
    let inferred = compiled
      .as_ref()
      .filter(|_| direct_source_shape)
      .and_then(|compiled| {
        super::query::find_preprocessed_node_at_path(
          &compiled.preprocessed_code,
          namespace,
          definition,
          &receiver_path,
          matches!(receiver, Cirru::List(_)),
        )
      })
      .and_then(runner::preprocess::infer_static_type_from_expr)
      .or_else(|| {
        if !direct_source_shape {
          return None;
        }
        super::query::find_preprocessed_node_at_path(
          &preprocessed,
          namespace,
          definition,
          &receiver_path,
          matches!(receiver, Cirru::List(_)),
        )
        .and_then(runner::preprocess::infer_static_type_from_expr)
      })
      .or_else(|| {
        runner::preprocess::unique_source_expression_at_path(&expressions, namespace, definition, &receiver_path)
          .and_then(|item| item.inferred_type.clone())
      })
      .or_else(|| super::query::infer_type_at_target(&source_receiver, None));
    let resolved = inferred
      .as_ref()
      .map(|annotation| runner::preprocess::resolve_namespace_type_refs_for_body(annotation.clone(), namespace));
    let concrete_list = resolved
      .as_ref()
      .is_some_and(|annotation| matches!(annotation.as_ref(), CalcitTypeAnnotation::List(_)));
    let proven_same_impl = resolved
      .as_ref()
      .is_some_and(|annotation| list_add_alias_is_proven(annotation.as_ref()));
    let definitely_other_type = resolved.as_ref().is_some_and(|resolved| {
      !matches!(
        resolved.as_ref(),
        CalcitTypeAnnotation::List(_) | CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::TypeVar(_)
      )
    });
    if definitely_other_type {
      continue;
    }
    let machine_applicable =
      concrete_list && proven_same_impl && method_source_context_is_stable(source, &call_path, namespace, definition, &usages);
    let original_node = navigate_to_path(source, &method_path)?;
    let replacement_node = Cirru::leaf(".append");
    suggestions.push(FixSuggestion {
      rule_id: CORE_LIST_ADD_RULE,
      diagnostic_code: CORE_LIST_ADD_DIAGNOSTIC,
      semantic_layer: "surface",
      source_file: snapshot_file.to_owned(),
      definition: format!("{namespace}/{definition}"),
      path: format!("code{}", format_path(&method_path)),
      fingerprint: node_fingerprint(&original_node),
      origin_chain: vec![serde_json::json!({
        "kind": "receiver-method-query",
        "receiver_type": inferred.as_ref().map(|annotation| annotation.describe()),
        "same_core_append_implementation": proven_same_impl,
      })],
      original: quoted_json(&original_node),
      replacement: machine_applicable.then(|| quoted_json(&replacement_node)),
      applicability: if machine_applicable {
        "machine-applicable"
      } else {
        "requires-review"
      },
      message: if machine_applicable {
        "Use `.append` for one List element; both methods resolve to the same core implementation and preserve evaluation order."
          .to_owned()
      } else {
        "Cannot prove a concrete List receiver, matching method implementation, or stable source context; review `.add` manually."
          .to_owned()
      },
      target_path: method_path,
      operation: machine_applicable.then_some(FixOperation::ReplaceLeaf {
        original: ".add".to_owned(),
        replacement: ".append".to_owned(),
      }),
    });
  }
  Ok(suggestions)
}

#[derive(Clone, Copy)]
enum MethodReceiverKind {
  CoreStruct {
    definition: &'static str,
    trait_origin: &'static str,
  },
}

impl MethodReceiverKind {
  fn matches(self, annotation: &CalcitTypeAnnotation) -> bool {
    match self {
      Self::CoreStruct { definition, .. } => annotation.resolve_to_struct().is_some_and(|base| {
        base.definition_ref.as_deref() == Some(definition)
          || (base.definition_ref.is_none() && base.name.ref_str() == definition.rsplit('/').next().unwrap_or(definition))
      }),
    }
  }

  fn name(self) -> &'static str {
    match self {
      Self::CoreStruct { definition, .. } => definition,
    }
  }
}

#[derive(Clone, Copy)]
struct MethodAliasRule {
  rule_id: &'static str,
  diagnostic_code: &'static str,
  receiver: MethodReceiverKind,
  old_method: &'static str,
  new_method: &'static str,
  implementation: &'static str,
  call_size: usize,
  variadic: bool,
  message: &'static str,
}

const CORE_EFFECT_METHOD_ALIASES: &[MethodAliasRule] = &[
  MethodAliasRule {
    rule_id: CORE_EFFECT_METHOD_RULE,
    diagnostic_code: CORE_EFFECT_METHOD_DIAGNOSTIC,
    receiver: MethodReceiverKind::CoreStruct {
      definition: "calcit.core/FfiTask",
      trait_origin: "calcit.core/FfiTaskOps",
    },
    old_method: ".cancel",
    new_method: ".cancel!",
    implementation: "calcit.core/ffi-task:cancel",
    call_size: 2,
    variadic: false,
    message: "Use `.cancel!` for a proven core FfiTask cancellation; the lifecycle implementation is unchanged.",
  },
  MethodAliasRule {
    rule_id: CORE_EFFECT_METHOD_RULE,
    diagnostic_code: CORE_EFFECT_METHOD_DIAGNOSTIC,
    receiver: MethodReceiverKind::CoreStruct {
      definition: "calcit.core/FfiTask",
      trait_origin: "calcit.core/FfiTaskOps",
    },
    old_method: ".cancel-with",
    new_method: ".cancel-with!",
    implementation: "calcit.core/ffi-task:cancel-with",
    call_size: 3,
    variadic: false,
    message: "Use `.cancel-with!` for a proven core FfiTask cancellation; the reason is evaluated once.",
  },
];

pub(super) struct ProvenMethodAlias {
  pub old_method: &'static str,
  pub new_method: &'static str,
  pub fix_rule: &'static str,
}

/// Require two methods to share a proven implementation and complete call contract.
fn same_proven_method_contract(receiver: &CalcitTypeAnnotation, old_method: &str, new_method: &str, implementation: &str) -> bool {
  let old = runner::preprocess::static_method_contract(receiver, old_method);
  let new = runner::preprocess::static_method_contract(receiver, new_method);
  old.status == "proven"
    && new.status == "proven"
    && old.definition.as_deref() == Some(implementation)
    && old.definition == new.definition
    && old.arg_types == new.arg_types
    && old.rest_type == new.rest_type
    && old.return_type == new.return_type
}

/// Check a generic alias rule against its receiver family and shared contract.
fn method_alias_contract_is_proven(receiver: &CalcitTypeAnnotation, rule: MethodAliasRule) -> bool {
  if !rule.receiver.matches(receiver) || !same_proven_method_contract(receiver, rule.old_method, rule.new_method, rule.implementation) {
    return false;
  }
  match rule.receiver {
    MethodReceiverKind::CoreStruct { trait_origin, .. } => {
      runner::preprocess::static_method_contracts(receiver).is_some_and(|methods| {
        [rule.old_method, rule.new_method].iter().all(|name| {
          methods
            .iter()
            .any(|(descriptor, contract)| descriptor.name == *name && descriptor.origin == trait_origin && contract.status == "proven")
        })
      })
    }
  }
}

/// Exclude open List element contracts from `.add` recommendations and fixes.
fn list_add_alias_is_proven(receiver: &CalcitTypeAnnotation) -> bool {
  matches!(receiver, CalcitTypeAnnotation::List(_)) && same_proven_method_contract(receiver, ".add", ".append", "calcit.core/append")
}

/// Return the expected core count implementation for supported collection types.
fn collection_count_definition(receiver: &CalcitTypeAnnotation) -> Option<&'static str> {
  match receiver {
    CalcitTypeAnnotation::List(_) => Some("calcit.core/&list:count"),
    CalcitTypeAnnotation::Map(_, _) => Some("calcit.core/&map:count"),
    CalcitTypeAnnotation::Set(_) => Some("calcit.core/&set:count"),
    _ => None,
  }
}

/// Prove `.count` and `.len` equivalent for a supported concrete receiver.
fn collection_len_alias_is_proven(receiver: &CalcitTypeAnnotation) -> bool {
  collection_count_definition(receiver).is_some_and(|definition| same_proven_method_contract(receiver, ".count", ".len", definition))
}

/// Expose only alias roles backed by the same proof used by existing fix rules.
pub(super) fn proven_method_aliases(receiver: &CalcitTypeAnnotation) -> Vec<ProvenMethodAlias> {
  // Query and fix share these proven alias contracts; this is not a second API registry.
  let mut aliases = CORE_EFFECT_METHOD_ALIASES
    .iter()
    .copied()
    .filter(|rule| method_alias_contract_is_proven(receiver, *rule))
    .map(|rule| ProvenMethodAlias {
      old_method: rule.old_method,
      new_method: rule.new_method,
      fix_rule: rule.rule_id,
    })
    .collect::<Vec<_>>();
  if list_add_alias_is_proven(receiver) {
    aliases.push(ProvenMethodAlias {
      old_method: ".add",
      new_method: ".append",
      fix_rule: CORE_LIST_ADD_RULE,
    });
  }
  if collection_len_alias_is_proven(receiver) {
    aliases.push(ProvenMethodAlias {
      old_method: ".count",
      new_method: ".len",
      fix_rule: CORE_COLLECTION_LEN_RULE,
    });
  }
  aliases
}

struct MethodAliasCall {
  call_path: Vec<usize>,
  method_path: Vec<usize>,
  receiver_path: Vec<usize>,
  compact_receiver: Option<String>,
}

/// Extract the receiver from a compact source leaf such as `task.cancel`.
fn compact_method_receiver<'a>(source: &'a str, method: &str) -> Option<&'a str> {
  let receiver = source.strip_suffix(method)?;
  if receiver.is_empty()
    || receiver.starts_with(|character: char| character.is_ascii_digit())
    || !receiver
      .chars()
      .all(|character| character.is_alphanumeric() || matches!(character, '-' | '_' | '?' | '!' | '*'))
  {
    return None;
  }
  Some(receiver)
}

/// Only rewrite complete method calls. First-class method values are not equivalent.
fn collect_method_alias_calls(node: &Cirru, path: &mut Vec<usize>, calls: &mut Vec<MethodAliasCall>, rule: MethodAliasRule) {
  if let Cirru::Leaf(source) = node {
    if rule.rule_id == CORE_EFFECT_METHOD_RULE
      && rule.call_size == 2
      && let Some(receiver) = compact_method_receiver(source, rule.old_method)
    {
      calls.push(MethodAliasCall {
        call_path: path.clone(),
        method_path: path.clone(),
        receiver_path: path.clone(),
        compact_receiver: Some(receiver.to_owned()),
      });
    }
    return;
  }
  let Cirru::List(items) = node else {
    return;
  };
  if matches!(items.first(), Some(Cirru::Leaf(head)) if matches!(head.as_ref(), "quote" | "quasiquote")) {
    return;
  }
  if items.len() == rule.call_size || (rule.variadic && items.len() > rule.call_size) {
    let indices = if matches!(&items[0], Cirru::Leaf(name) if name.as_ref() == rule.old_method) {
      Some((0, 1))
    } else if matches!(&items[1], Cirru::Leaf(name) if name.as_ref() == rule.old_method) {
      Some((1, 0))
    } else {
      None
    };
    if let Some((method_index, receiver_index)) = indices {
      let mut method_path = path.clone();
      method_path.push(method_index);
      let mut receiver_path = path.clone();
      receiver_path.push(receiver_index);
      calls.push(MethodAliasCall {
        call_path: path.clone(),
        method_path,
        receiver_path,
        compact_receiver: None,
      });
    }
  }
  if rule.rule_id == CORE_EFFECT_METHOD_RULE
    && rule.call_size > 2
    && (items.len() + 1 == rule.call_size || (rule.variadic && items.len() + 1 > rule.call_size))
    && let Some(Cirru::Leaf(source)) = items.first()
    && let Some(receiver) = compact_method_receiver(source, rule.old_method)
  {
    let mut leaf_path = path.clone();
    leaf_path.push(0);
    calls.push(MethodAliasCall {
      call_path: path.clone(),
      method_path: leaf_path.clone(),
      receiver_path: leaf_path,
      compact_receiver: Some(receiver.to_owned()),
    });
  }
  for (index, child) in items.iter().enumerate() {
    path.push(index);
    collect_method_alias_calls(child, path, calls, rule);
    path.pop();
  }
}

/// Plan a leaf-only rename when receiver, core method contract, and source origin agree.
fn plan_core_method_alias_fixes(
  snapshot: &Snapshot,
  snapshot_file: &str,
  selected_definitions: &[(String, String)],
  rule: MethodAliasRule,
) -> Result<Vec<FixSuggestion>, String> {
  let mut suggestions = Vec::new();
  for (namespace, definition) in selected_definitions {
    let entry = snapshot
      .files
      .get(namespace)
      .and_then(|file| file.defs.get(definition))
      .ok_or_else(|| format!("Selected definition `{namespace}/{definition}` is missing from the source snapshot."))?;
    suggestions.extend(plan_core_method_alias_source(
      snapshot_file,
      namespace,
      definition,
      &entry.code,
      program::lookup_compiled_def(namespace, definition),
      rule,
    )?);
  }
  Ok(suggestions)
}

/// Share receiver and method proof between definition code and attached source.
fn plan_core_method_alias_source(
  snapshot_file: &str,
  namespace: &str,
  definition: &str,
  source: &Cirru,
  compiled: Option<program::CompiledDef>,
  rule: MethodAliasRule,
) -> Result<Vec<FixSuggestion>, String> {
  let mut suggestions = Vec::new();
  if list_head(source) == Some("defmacro") {
    return Ok(suggestions);
  }
  let mut calls = Vec::new();
  collect_method_alias_calls(source, &mut Vec::new(), &mut calls, rule);
  if calls.is_empty() {
    return Ok(suggestions);
  }
  let parsed = code_to_calcit(source, namespace, definition, vec![]).map_err(|error| error.to_string())?;
  let usages =
    runner::preprocess::trace_source_usages(&parsed, namespace, definition, &RefCell::new(Vec::new()), &CallStackList::default())
      .map_err(|failure| failure.msg)?;
  let (preprocessed, expressions) =
    runner::preprocess::trace_source_expressions(&parsed, namespace, definition, &RefCell::new(Vec::new()), &CallStackList::default())
      .map_err(|failure| failure.msg)?;
  for call in calls {
    let receiver = match &call.compact_receiver {
      Some(name) => Cirru::leaf(name.as_str()),
      None => navigate_to_path(source, &call.receiver_path)?,
    };
    let source_receiver = code_to_calcit(
      &receiver,
      namespace,
      definition,
      call
        .receiver_path
        .iter()
        .map(|index| u16::try_from(*index).map_err(|_| format!("Path index {index} exceeds Snapshot coordinate range")))
        .collect::<Result<Vec<_>, _>>()?,
    )
    .map_err(|error| error.to_string())?;
    // Reader shorthand such as @cell is a call, not the located Ref leaf inside it.
    let direct_source_shape = !matches!(receiver, Cirru::Leaf(_)) || !matches!(source_receiver, Calcit::List(_));
    let inferred = compiled
      .as_ref()
      .filter(|_| direct_source_shape)
      .and_then(|compiled| {
        super::query::find_preprocessed_node_at_path(
          &compiled.preprocessed_code,
          namespace,
          definition,
          &call.receiver_path,
          matches!(receiver, Cirru::List(_)),
        )
      })
      .and_then(runner::preprocess::infer_static_type_from_expr)
      .or_else(|| {
        if !direct_source_shape {
          return None;
        }
        super::query::find_preprocessed_node_at_path(
          &preprocessed,
          namespace,
          definition,
          &call.receiver_path,
          matches!(receiver, Cirru::List(_)),
        )
        .and_then(runner::preprocess::infer_static_type_from_expr)
      })
      .or_else(|| {
        runner::preprocess::unique_source_expression_at_path(&expressions, namespace, definition, &call.receiver_path)
          .and_then(|item| item.inferred_type.clone())
      })
      .or_else(|| super::query::infer_type_at_target(&source_receiver, None));
    let resolved = inferred
      .as_ref()
      .map(|annotation| runner::preprocess::resolve_namespace_type_refs_for_body(annotation.clone(), namespace));
    if resolved.as_ref().is_some_and(|annotation| {
      !rule.receiver.matches(annotation.as_ref())
        && !matches!(
          annotation.as_ref(),
          CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::TypeVar(_)
        )
    }) {
      continue;
    }
    let proven_same_impl = resolved
      .as_ref()
      .is_some_and(|annotation| method_alias_contract_is_proven(annotation.as_ref(), rule));
    let machine_applicable =
      proven_same_impl && method_source_context_is_stable(source, &call.call_path, namespace, definition, &usages);
    let original_node = navigate_to_path(source, &call.method_path)?;
    let original_leaf = match &original_node {
      Cirru::Leaf(name) => name.as_ref(),
      Cirru::List(_) => return Err("Method alias candidate must be a source leaf.".to_owned()),
    };
    let replacement_leaf = match &call.compact_receiver {
      Some(name) => format!("{name}{}", rule.new_method),
      None => rule.new_method.to_owned(),
    };
    let replacement_node = Cirru::leaf(replacement_leaf.as_str());
    let method_evidence = serde_json::json!({
      "kind": "receiver-method-query",
      "receiver_type": inferred.as_ref().map(|annotation| annotation.describe()),
      "same_core_implementation": proven_same_impl,
    });
    suggestions.push(FixSuggestion {
      rule_id: rule.rule_id,
      diagnostic_code: rule.diagnostic_code,
      semantic_layer: "surface",
      source_file: snapshot_file.to_owned(),
      definition: format!("{namespace}/{definition}"),
      path: format!("code{}", format_path(&call.method_path)),
      fingerprint: node_fingerprint(&original_node),
      origin_chain: vec![method_evidence],
      original: quoted_json(&original_node),
      replacement: machine_applicable.then(|| quoted_json(&replacement_node)),
      applicability: if machine_applicable {
        "machine-applicable"
      } else {
        "requires-review"
      },
      message: if machine_applicable {
        rule.message.to_owned()
      } else {
        format!(
          "Cannot prove a concrete {} receiver, matching method implementation, or stable source context; review `{}` manually.",
          rule.receiver.name(),
          rule.old_method
        )
      },
      target_path: call.method_path,
      operation: machine_applicable.then_some(FixOperation::ReplaceLeaf {
        original: original_leaf.to_owned(),
        replacement: replacement_leaf,
      }),
    });
  }
  Ok(suggestions)
}

/// A zero-argument method call has two source nodes in either method-first or receiver-first form.
fn collect_collection_count_calls(node: &Cirru, path: &mut Vec<usize>, calls: &mut Vec<(Vec<usize>, Vec<usize>, Vec<usize>)>) {
  let Cirru::List(items) = node else {
    return;
  };
  if matches!(items.first(), Some(Cirru::Leaf(head)) if matches!(head.as_ref(), "quote" | "quasiquote")) {
    return;
  }
  if items.len() == 2 {
    let indices = if matches!(&items[0], Cirru::Leaf(name) if name.as_ref() == ".count") {
      Some((0, 1))
    } else if matches!(&items[1], Cirru::Leaf(name) if name.as_ref() == ".count") {
      Some((1, 0))
    } else {
      None
    };
    if let Some((method_index, receiver_index)) = indices {
      let mut method_path = path.clone();
      method_path.push(method_index);
      let mut receiver_path = path.clone();
      receiver_path.push(receiver_index);
      calls.push((path.clone(), method_path, receiver_path));
    }
  }
  for (index, child) in items.iter().enumerate() {
    path.push(index);
    collect_collection_count_calls(child, path, calls);
    path.pop();
  }
}

/// Plan `.count` migrations using the receiver-specific count implementation.
fn plan_core_collection_len_fixes(
  snapshot: &Snapshot,
  snapshot_file: &str,
  selected_definitions: &[(String, String)],
) -> Result<Vec<FixSuggestion>, String> {
  let mut suggestions = Vec::new();
  for (namespace, definition) in selected_definitions {
    let entry = snapshot
      .files
      .get(namespace)
      .and_then(|file| file.defs.get(definition))
      .ok_or_else(|| format!("Selected definition `{namespace}/{definition}` is missing from the source snapshot."))?;
    suggestions.extend(plan_core_collection_len_source(snapshot_file, namespace, definition, &entry.code)?);
  }
  Ok(suggestions)
}

/// Share the existing source and receiver proof with attached executable expressions.
fn plan_core_collection_len_source(
  snapshot_file: &str,
  namespace: &str,
  definition: &str,
  source: &Cirru,
) -> Result<Vec<FixSuggestion>, String> {
  let mut suggestions = Vec::new();
  if list_head(source) == Some("defmacro") {
    return Ok(suggestions);
  }
  let mut calls = Vec::new();
  collect_collection_count_calls(source, &mut Vec::new(), &mut calls);
  if calls.is_empty() {
    return Ok(suggestions);
  }
  let parsed = code_to_calcit(source, namespace, definition, vec![]).map_err(|error| error.to_string())?;
  let usages =
    runner::preprocess::trace_source_usages(&parsed, namespace, definition, &RefCell::new(Vec::new()), &CallStackList::default())
      .map_err(|failure| failure.msg)?;
  let compiled = program::lookup_compiled_def(namespace, definition);
  let (preprocessed, expressions) =
    runner::preprocess::trace_source_expressions(&parsed, namespace, definition, &RefCell::new(Vec::new()), &CallStackList::default())
      .map_err(|failure| failure.msg)?;
  for (call_path, method_path, receiver_path) in calls {
    let receiver = navigate_to_path(source, &receiver_path)?;
    let source_receiver = code_to_calcit(
      &receiver,
      namespace,
      definition,
      receiver_path
        .iter()
        .map(|index| u16::try_from(*index).map_err(|_| format!("Path index {index} exceeds Snapshot coordinate range")))
        .collect::<Result<Vec<_>, _>>()?,
    )
    .map_err(|error| error.to_string())?;
    let direct_source_shape = !matches!(receiver, Cirru::Leaf(_)) || !matches!(source_receiver, Calcit::List(_));
    let inferred = compiled
      .as_ref()
      .filter(|_| direct_source_shape)
      .and_then(|compiled| {
        super::query::find_preprocessed_node_at_path(
          &compiled.preprocessed_code,
          namespace,
          definition,
          &receiver_path,
          matches!(receiver, Cirru::List(_)),
        )
      })
      .and_then(runner::preprocess::infer_static_type_from_expr)
      .or_else(|| {
        if !direct_source_shape {
          return None;
        }
        super::query::find_preprocessed_node_at_path(
          &preprocessed,
          namespace,
          definition,
          &receiver_path,
          matches!(receiver, Cirru::List(_)),
        )
        .and_then(runner::preprocess::infer_static_type_from_expr)
      })
      .or_else(|| {
        runner::preprocess::unique_source_expression_at_path(&expressions, namespace, definition, &receiver_path)
          .and_then(|item| item.inferred_type.clone())
      })
      .or_else(|| super::query::infer_type_at_target(&source_receiver, None));
    let resolved = inferred
      .as_ref()
      .map(|annotation| runner::preprocess::resolve_namespace_type_refs_for_body(annotation.clone(), namespace));
    let expected_definition = resolved
      .as_ref()
      .and_then(|annotation| collection_count_definition(annotation.as_ref()));
    if resolved.as_ref().is_some_and(|annotation| {
      !matches!(
        annotation.as_ref(),
        CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::TypeVar(_)
      ) && expected_definition.is_none()
    }) {
      continue;
    }
    let proven_same_impl = resolved
      .as_ref()
      .is_some_and(|annotation| collection_len_alias_is_proven(annotation.as_ref()));
    let machine_applicable = proven_same_impl && method_source_context_is_stable(source, &call_path, namespace, definition, &usages);
    let original_node = navigate_to_path(source, &method_path)?;
    let replacement_node = Cirru::leaf(".len");
    suggestions.push(FixSuggestion {
      rule_id: CORE_COLLECTION_LEN_RULE,
      diagnostic_code: CORE_COLLECTION_LEN_DIAGNOSTIC,
      semantic_layer: "surface",
      source_file: snapshot_file.to_owned(),
      definition: format!("{namespace}/{definition}"),
      path: format!("code{}", format_path(&method_path)),
      fingerprint: node_fingerprint(&original_node),
      origin_chain: vec![serde_json::json!({
        "kind": "receiver-method-query",
        "receiver_type": inferred.as_ref().map(|annotation| annotation.describe()),
        "same_core_count_implementation": proven_same_impl,
      })],
      original: quoted_json(&original_node),
      replacement: machine_applicable.then(|| quoted_json(&replacement_node)),
      applicability: if machine_applicable {
        "machine-applicable"
      } else {
        "requires-review"
      },
      message: if machine_applicable {
        "Use `.len` for built-in List, Map or Set length; both methods resolve to the same core implementation."
          .to_owned()
      } else {
        "Cannot prove a built-in List, Map or Set receiver, matching method implementation, or stable source context; review `.count` manually."
          .to_owned()
      },
      target_path: method_path,
      operation: machine_applicable.then_some(FixOperation::ReplaceLeaf {
        original: ".count".to_owned(),
        replacement: ".len".to_owned(),
      }),
    });
  }
  Ok(suggestions)
}

/// Reuse structural body rules, checking actual attached coordinates rather than the synthetic function body.
fn plan_attached_do_rewrite(
  source: &Cirru,
  namespace: &str,
  definition: &str,
  rule: &str,
) -> Result<Option<AttachedSourceRewrite>, String> {
  let mut paths = Vec::new();
  if rule == REDUNDANT_DO_RULE {
    collect_redundant_do_paths(source, &mut Vec::new(), &mut paths);
  } else {
    collect_single_expression_do_paths(source, &mut Vec::new(), &mut paths);
  }
  if paths.is_empty() {
    return Ok(None);
  }
  let wrapper = Cirru::List(vec![Cirru::leaf("fn"), Cirru::List(vec![]), source.clone()]);
  let parsed = code_to_calcit(&wrapper, namespace, definition, vec![]).map_err(|error| error.to_string())?;
  let usages =
    runner::preprocess::trace_source_usages(&parsed, namespace, definition, &RefCell::new(Vec::new()), &CallStackList::default())
      .map_err(|error| error.msg)?;
  for path in &paths {
    let mut wrapper_path = vec![2];
    wrapper_path.extend(path);
    if !method_source_context_is_stable(&wrapper, &wrapper_path, namespace, definition, &usages) {
      return Err(format!(
        "Cannot prove stable executable context for `do` at {}; preserve the metadata region for review.",
        format_path(path)
      ));
    }
  }
  // Redundant wrappers are spliced by their actual enclosing body. A root multi-expression do has no such parent.
  let mut targets = if rule == REDUNDANT_DO_RULE {
    paths.iter().map(|path| path[..path.len() - 1].to_vec()).collect::<Vec<_>>()
  } else {
    paths.clone()
  };
  targets.sort_by(|left, right| right.cmp(left));
  targets.dedup();
  let mut rewritten = source.clone();
  for path in targets {
    let original = navigate_to_path(&rewritten, &path)?;
    let replacement = if rule == REDUNDANT_DO_RULE {
      splice_redundant_do_children(original.clone())
    } else {
      unwrap_single_expression_do(original.clone())
    };
    replace_attached_source_tree(&mut rewritten, &path, &original, &replacement)?;
  }
  Ok(Some(AttachedSourceRewrite {
    code: rewritten,
    origin_chain: paths
      .into_iter()
      .map(|path| serde_json::json!({"kind": "resolved-attached-source", "path": format_path(&path), "rule_id": rule}))
      .collect(),
  }))
}

/// Build structural splice suggestions for redundant `do` wrappers in proven variadic bodies.
fn plan_redundant_do_fixes(
  snapshot: &Snapshot,
  snapshot_file: &str,
  selected_definitions: &[(String, String)],
  excluded_regions: &[(String, Vec<usize>)],
) -> Result<Vec<FixSuggestion>, String> {
  let mut suggestions = Vec::new();
  for (namespace, definition) in selected_definitions {
    let entry = snapshot
      .files
      .get(namespace)
      .and_then(|file| file.defs.get(definition))
      .ok_or_else(|| format!("Selected definition `{namespace}/{definition}` is missing from the source snapshot."))?;
    let mut paths = Vec::new();
    collect_redundant_do_paths(&entry.code, &mut Vec::new(), &mut paths);
    let target_definition = format!("{namespace}/{definition}");
    paths.retain(|path| {
      !excluded_regions
        .iter()
        .any(|(region_definition, region_path)| region_definition == &target_definition && path.starts_with(region_path))
    });
    paths.sort_by(|left, right| right.cmp(left));
    for target_path in paths {
      let original_node = navigate_to_path(&entry.code, &target_path)?;
      let Cirru::List(ref items) = original_node else {
        continue;
      };
      let replacement_items = items.iter().skip(1).map(cirru_to_json_value).collect::<Vec<_>>();
      suggestions.push(FixSuggestion {
        rule_id: REDUNDANT_DO_RULE,
        diagnostic_code: REDUNDANT_DO_DIAGNOSTIC,
        semantic_layer: "surface",
        source_file: snapshot_file.to_owned(),
        definition: target_definition.clone(),
        path: format!("code{}", format_path(&target_path)),
        fingerprint: node_fingerprint(&original_node),
        origin_chain: vec![],
        original: quoted_json(&original_node),
        replacement: Some(serde_json::json!({
          "$type": "splice",
          "value": replacement_items,
        })),
        applicability: "machine-applicable",
        message: "Splice a redundant `do` into the surrounding variadic body; sequencing and the final-value type are unchanged."
          .to_owned(),
        target_path,
        operation: Some(FixOperation::SpliceDo),
      });
    }
  }
  Ok(suggestions)
}

/// Collect source paths in evaluation order while treating quoted trees as data.
fn collect_redundant_do_paths(node: &Cirru, path: &mut Vec<usize>, output: &mut Vec<Vec<usize>>) {
  let Cirru::List(items) = node else {
    return;
  };
  let head = items.first().and_then(|item| match item {
    Cirru::Leaf(value) => Some(value.as_ref()),
    Cirru::List(_) => None,
  });
  if matches!(head, Some("quote" | "quasiquote")) {
    return;
  }
  let body_start = match head {
    Some("defn") => Some(3),
    Some("fn" | "let") => Some(2),
    Some("let[]") => Some(3),
    Some("do") => Some(1),
    _ => None,
  };
  if let Some(body_start) = body_start {
    for (index, child) in items.iter().enumerate().skip(body_start) {
      if matches!(child, Cirru::List(children) if children.len() > 1 && matches!(children.first(), Some(Cirru::Leaf(head)) if head.as_ref() == "do"))
      {
        let mut target = path.clone();
        target.push(index);
        output.push(target);
      }
    }
  }
  for (index, child) in items.iter().enumerate() {
    path.push(index);
    collect_redundant_do_paths(child, path, output);
    path.pop();
  }
}

/// Build unwrap suggestions for `do` forms whose single payload already is one expression.
fn plan_single_expression_do_fixes(
  snapshot: &Snapshot,
  snapshot_file: &str,
  selected_definitions: &[(String, String)],
  composed_regions: &[(String, Vec<usize>)],
  redundant_do_regions: &[(String, Vec<usize>)],
) -> Result<Vec<FixSuggestion>, String> {
  let mut suggestions = Vec::new();
  for (namespace, definition) in selected_definitions {
    let entry = snapshot
      .files
      .get(namespace)
      .and_then(|file| file.defs.get(definition))
      .ok_or_else(|| format!("Selected definition `{namespace}/{definition}` is missing from the source snapshot."))?;
    let mut paths = Vec::new();
    collect_single_expression_do_paths(&entry.code, &mut Vec::new(), &mut paths);
    let target_definition = format!("{namespace}/{definition}");
    paths.retain(|path| {
      !composed_regions
        .iter()
        .any(|(region_definition, region_path)| region_definition == &target_definition && path.starts_with(region_path))
        && !redundant_do_regions
          .iter()
          .any(|(region_definition, region_path)| region_definition == &target_definition && path == region_path)
    });
    paths.sort_by(|left, right| right.cmp(left));
    for target_path in paths {
      let original_node = navigate_to_path(&entry.code, &target_path)?;
      let Cirru::List(items) = &original_node else {
        continue;
      };
      let Some(replacement_node) = items.get(1) else {
        continue;
      };
      suggestions.push(FixSuggestion {
        rule_id: SINGLE_EXPRESSION_DO_RULE,
        diagnostic_code: SINGLE_EXPRESSION_DO_DIAGNOSTIC,
        semantic_layer: "surface",
        source_file: snapshot_file.to_owned(),
        definition: target_definition.clone(),
        path: format!("code{}", format_path(&target_path)),
        fingerprint: node_fingerprint(&original_node),
        origin_chain: vec![],
        original: quoted_json(&original_node),
        replacement: Some(quoted_json(replacement_node)),
        applicability: "machine-applicable",
        message: "Unwrap a single-expression `do`; evaluation count, order, failure behavior, and result type are unchanged."
          .to_owned(),
        target_path,
        operation: Some(FixOperation::SpliceDo),
      });
    }
  }
  Ok(suggestions)
}

/// Collect executable `(do expression)` paths while preserving macro and quoted data boundaries.
fn collect_single_expression_do_paths(node: &Cirru, path: &mut Vec<usize>, output: &mut Vec<Vec<usize>>) {
  let Cirru::List(items) = node else {
    return;
  };
  let head = items.first().and_then(leaf_value);
  if matches!(head, Some("quote" | "quasiquote" | "defmacro")) {
    return;
  }
  if head == Some("do") && items.len() == 2 {
    output.push(path.clone());
  }
  for (index, child) in items.iter().enumerate() {
    path.push(index);
    collect_single_expression_do_paths(child, path, output);
    path.pop();
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NominalKind {
  Enum,
  Struct,
}

impl NominalKind {
  fn definition_head(self) -> &'static str {
    match self {
      Self::Enum => "defenum",
      Self::Struct => "defstruct",
    }
  }

  fn legacy_head(self) -> &'static str {
    match self {
      Self::Enum => "%::",
      Self::Struct => "%{}",
    }
  }

  fn rule_id(self) -> &'static str {
    match self {
      Self::Enum => NAMED_ENUM_CONSTRUCTOR_RULE,
      Self::Struct => NAMED_STRUCT_CONSTRUCTOR_RULE,
    }
  }

  fn diagnostic_code(self) -> &'static str {
    match self {
      Self::Enum => NAMED_ENUM_CONSTRUCTOR_DIAGNOSTIC,
      Self::Struct => NAMED_STRUCT_CONSTRUCTOR_DIAGNOSTIC,
    }
  }

  fn display_name(self) -> &'static str {
    match self {
      Self::Enum => "enum",
      Self::Struct => "struct",
    }
  }
}

/// Rewrite legacy prototype constructors only when the prototype resolves to a project nominal definition.
fn plan_named_constructor_fixes(
  snapshot: &Snapshot,
  snapshot_file: &str,
  selected_definitions: &[(String, String)],
  kinds: &[NominalKind],
  compose_redundant_do: bool,
  compose_single_expression_do: bool,
) -> Result<Vec<FixSuggestion>, String> {
  let mut suggestions = Vec::new();
  for (namespace, definition) in selected_definitions {
    let entry = snapshot
      .files
      .get(namespace)
      .and_then(|file| file.defs.get(definition))
      .ok_or_else(|| format!("Selected definition `{namespace}/{definition}` is missing from the source snapshot."))?;
    suggestions.extend(plan_named_constructor_source(
      snapshot,
      snapshot_file,
      (namespace, definition),
      &entry.code,
      kinds,
      (compose_redundant_do, compose_single_expression_do),
    )?);
  }
  Ok(suggestions)
}

/// Share nominal resolution and field completeness with source-origin checks for attached constructors.
///
/// Constructors in executable code are proven by the compiler usage trace. A quasiquote template is
/// not compiled until each expansion, so a template constructor is proven statically instead: its
/// namespace-qualified prototype resolves through the macro namespace, the same way the expanded
/// symbol resolves at every expansion site. An unqualified template prototype can be captured by a
/// local at an expansion site, so it is reported for review.
fn plan_named_constructor_source(
  snapshot: &Snapshot,
  snapshot_file: &str,
  owner: (&str, &str),
  source: &Cirru,
  kinds: &[NominalKind],
  compose_do: (bool, bool),
) -> Result<Vec<FixSuggestion>, String> {
  let (namespace, definition) = owner;
  let (compose_redundant_do, compose_single_expression_do) = compose_do;
  let mut suggestions = Vec::new();
  let mut shadowed = HashSet::new();
  collect_potential_local_bindings(source, &mut shadowed);
  let scope = ConstructorScope {
    snapshot,
    namespace,
    shadowed: &shadowed,
    kinds,
    macro_body: list_head(source) == Some("defmacro"),
  };
  let mut paths = Vec::new();
  collect_named_constructor_paths(source, &mut Vec::new(), 0, &mut paths, &scope);
  paths.sort();
  if paths.is_empty() {
    return Ok(suggestions);
  }
  let usages = if paths.iter().any(|(_, depth)| *depth == 0) {
    let parsed = code_to_calcit(source, namespace, definition, vec![]).map_err(|error| error.to_string())?;
    runner::preprocess::trace_source_usages(&parsed, namespace, definition, &RefCell::new(Vec::new()), &CallStackList::default())
      .map_err(|error| error.msg)?
  } else {
    Vec::new()
  };
  let all_paths = paths;
  let targets = all_paths
    .iter()
    .filter(|(path, _)| {
      !all_paths
        .iter()
        .any(|(other, _)| other.len() < path.len() && path.starts_with(other))
    })
    .cloned()
    .collect::<Vec<_>>();
  for (target_path, target_depth) in targets {
    let original_node = navigate_to_path(source, &target_path)?;
    let Cirru::List(items) = &original_node else {
      continue;
    };
    let Some(kind) = scope.legacy_kind(items) else {
      continue;
    };
    let nested = all_paths
      .iter()
      .filter(|(path, _)| path.starts_with(&target_path))
      .collect::<Vec<_>>();
    let unqualified_template = nested.iter().any(|(path, depth)| {
      *depth == 1
        && navigate_to_path(source, path)
          .ok()
          .and_then(|node| match node {
            Cirru::List(parts) => parts.get(1).and_then(leaf_value).map(|prototype| !prototype.contains('/')),
            Cirru::Leaf(_) => None,
          })
          .unwrap_or(true)
    });
    let proven = !unqualified_template
      && nested.iter().all(|(path, depth)| {
        let Ok(Cirru::List(parts)) = navigate_to_path(source, path) else {
          return false;
        };
        let Some(kind) = scope.legacy_kind(&parts) else {
          return false;
        };
        if *depth == 1 {
          // Resolution and field completeness were checked against the macro namespace.
          return true;
        }
        let Some(prototype) = parts.get(1).and_then(leaf_value) else {
          return false;
        };
        let Some((target_ns, target_def)) = resolve_project_nominal_target(snapshot, namespace, prototype, kind) else {
          return false;
        };
        let mut prototype_path = path.clone();
        prototype_path.push(1);
        let matching = usages
          .iter()
          .filter(|usage| {
            usage.location.as_ref().is_some_and(|location| {
              location.ns.as_ref() == namespace
                && location.def.as_ref() == definition
                && location
                  .coord
                  .iter()
                  .map(|index| usize::from(*index))
                  .eq(prototype_path.iter().copied())
            })
          })
          .collect::<Vec<_>>();
        !matching.is_empty()
          && matching.iter().all(|usage| {
            usage.target_ns.as_ref() == target_ns
              && usage.target_def.as_ref() == target_def
              && usage
                .macro_origin
                .iter()
                .all(|origin| preserves_nominal_method_call_through_macro(origin))
          })
      });
    let replacement_node = rewrite_named_constructor_tree(
      &original_node,
      target_depth,
      &scope,
      compose_redundant_do,
      compose_single_expression_do,
    );
    let original_code = original_node
      .format_one_liner()
      .map_err(|error| format!("Failed to format constructor source at {namespace}/{definition}: {error}"))?;
    let replacement_code = replacement_node
      .format_one_liner()
      .map_err(|error| format!("Failed to format constructor replacement at {namespace}/{definition}: {error}"))?;
    let prototype = items.get(1).and_then(leaf_value).unwrap_or("<unknown>");
    let origin_kind = if target_depth == 1 {
      "macro-namespace-resolved-template-constructor"
    } else {
      "compiler-resolved-nominal-constructor"
    };
    suggestions.push(FixSuggestion {
      rule_id: kind.rule_id(),
      diagnostic_code: kind.diagnostic_code(),
      semantic_layer: "surface",
      source_file: snapshot_file.to_owned(),
      definition: format!("{namespace}/{definition}"),
      path: format!("code{}", format_path(&target_path)),
      fingerprint: node_fingerprint(&original_node),
      origin_chain: vec![serde_json::json!({"kind": origin_kind, "proven": proven})],
      original: quoted_json(&original_node),
      replacement: proven.then(|| quoted_json(&replacement_node)),
      applicability: if proven { "machine-applicable" } else { "requires-review" },
      message: if proven {
        format!(
          "Replace legacy `{}` {} construction with the directly callable `{prototype}` constructor.",
          kind.legacy_head(),
          kind.display_name()
        )
      } else if unqualified_template {
        "An unqualified prototype in a quasiquote template resolves at each expansion site, where a local may capture it; qualify the prototype with its namespace or review the macro users.".to_owned()
      } else {
        "Cannot prove the constructor prototype and executable macro origin; retain this source region for review.".to_owned()
      },
      target_path,
      operation: proven.then_some(FixOperation::ReplaceNode {
        original: original_code,
        replacement: replacement_code,
      }),
    });
  }
  Ok(suggestions)
}

/// Resolution context shared by legacy nominal constructor collection and rewriting.
struct ConstructorScope<'a> {
  snapshot: &'a Snapshot,
  namespace: &'a str,
  shadowed: &'a HashSet<String>,
  kinds: &'a [NominalKind],
  /// A `defmacro` body runs at expansion time; only its quasiquote templates are migrated.
  macro_body: bool,
}

impl ConstructorScope<'_> {
  fn legacy_kind(&self, items: &[Cirru]) -> Option<NominalKind> {
    legacy_constructor_kind(items, self.snapshot, self.namespace, self.shadowed, self.kinds)
  }

  /// Depth 0 is executable code and depth 1 is a quasiquote template outside any unquote.
  fn migrates_at(&self, depth: usize) -> bool {
    depth == 1 || (depth == 0 && !self.macro_body)
  }

  /// Quasiquote depth of a list's children. `None` marks quoted data, nested templates and nested
  /// macros, which stay unchanged.
  fn child_depth(&self, items: &[Cirru], depth: usize, is_root: bool) -> Option<usize> {
    match items.first().and_then(leaf_value) {
      Some("quote" | "cirru-quote" | ";") => None,
      Some("defmacro") if !(is_root && self.macro_body) => None,
      Some("quasiquote") => (depth == 0).then_some(1),
      Some("~" | "~@") => depth.checked_sub(1),
      _ => Some(depth),
    }
  }
}

/// Collect legacy constructor calls in executable code and quasiquote templates while preserving
/// quoted data and rejecting unresolved or shadowed prototypes.
fn collect_named_constructor_paths(
  node: &Cirru,
  path: &mut Vec<usize>,
  depth: usize,
  output: &mut Vec<(Vec<usize>, usize)>,
  scope: &ConstructorScope,
) {
  let Cirru::List(items) = node else {
    return;
  };
  let Some(child_depth) = scope.child_depth(items, depth, path.is_empty()) else {
    return;
  };
  if scope.migrates_at(depth) && scope.legacy_kind(items).is_some() {
    output.push((path.clone(), depth));
  }
  for (index, child) in items.iter().enumerate() {
    path.push(index);
    collect_named_constructor_paths(child, path, child_depth, output, scope);
    path.pop();
  }
}

fn legacy_constructor_kind(
  items: &[Cirru],
  snapshot: &Snapshot,
  namespace: &str,
  shadowed: &HashSet<String>,
  kinds: &[NominalKind],
) -> Option<NominalKind> {
  let head = items.first().and_then(leaf_value)?;
  let prototype = items.get(1).and_then(leaf_value)?;
  kinds.iter().copied().find(|kind| {
    let target = resolve_project_nominal_target(snapshot, namespace, prototype, *kind);
    head == kind.legacy_head()
      && !prototype.starts_with('_')
      && !prototype_is_shadowed(prototype, shadowed)
      && target.is_some()
      && (*kind != NominalKind::Struct
        || target.is_some_and(|(target_ns, target_def)| struct_fields_are_complete(snapshot, &target_ns, &target_def, items)))
      && legacy_constructor_replacement(items, *kind).is_some()
  })
}

/// Recursively compose selected constructor and redundant-body rewrites inside one guarded replacement.
/// `depth` is the quasiquote depth of `node`; template code keeps its `do` structure.
fn rewrite_named_constructor_tree(
  node: &Cirru,
  depth: usize,
  scope: &ConstructorScope,
  compose_redundant_do: bool,
  compose_single_expression_do: bool,
) -> Cirru {
  let Cirru::List(items) = node else {
    return node.clone();
  };
  let Some(child_depth) = scope.child_depth(items, depth, false) else {
    return node.clone();
  };
  let rewritten_items = items
    .iter()
    .map(|item| rewrite_named_constructor_tree(item, child_depth, scope, compose_redundant_do, compose_single_expression_do))
    .collect::<Vec<_>>();
  let rewritten_node = match scope.legacy_kind(items) {
    Some(kind) if scope.migrates_at(depth) => {
      legacy_constructor_replacement(&rewritten_items, kind).unwrap_or(Cirru::List(rewritten_items))
    }
    _ => Cirru::List(rewritten_items),
  };
  if depth != 0 {
    return rewritten_node;
  }
  let rewritten_node = if compose_redundant_do {
    splice_redundant_do_children(rewritten_node)
  } else {
    rewritten_node
  };
  if compose_single_expression_do {
    unwrap_single_expression_do(rewritten_node)
  } else {
    rewritten_node
  }
}

/// Remove one single-expression `do` after recursively rewriting its payload.
fn unwrap_single_expression_do(node: Cirru) -> Cirru {
  match node {
    Cirru::List(items) if items.len() == 2 && items.first().and_then(leaf_value) == Some("do") => {
      items.into_iter().nth(1).expect("single-expression do should contain one payload")
    }
    other => other,
  }
}

/// Convert one validated legacy constructor node to direct surface syntax.
fn legacy_constructor_replacement(items: &[Cirru], kind: NominalKind) -> Option<Cirru> {
  let prototype = items.get(1)?.clone();
  match kind {
    NominalKind::Enum => Some(Cirru::List(items.iter().skip(1).cloned().collect())),
    NominalKind::Struct => {
      let mut replacement = vec![prototype];
      for field in items.iter().skip(2) {
        let Cirru::List(pair) = field else {
          return None;
        };
        if pair.len() != 2 || !pair.first().and_then(leaf_value).is_some_and(|key| key.starts_with(':')) {
          return None;
        }
        replacement.extend(pair.iter().cloned());
      }
      Some(Cirru::List(replacement))
    }
  }
}

/// Splice direct `do` children only where the enclosing form accepts a variadic body.
fn splice_redundant_do_children(node: Cirru) -> Cirru {
  let Cirru::List(items) = node else {
    return node;
  };
  let body_start = match items.first().and_then(leaf_value) {
    Some("defn") => Some(3),
    Some("fn" | "let") => Some(2),
    Some("let[]") => Some(3),
    Some("do") => Some(1),
    _ => None,
  };
  let Some(body_start) = body_start else {
    return Cirru::List(items);
  };
  let mut output = Vec::with_capacity(items.len());
  for (index, item) in items.into_iter().enumerate() {
    if index >= body_start
      && let Cirru::List(children) = &item
      && children.len() > 1
      && children.first().and_then(leaf_value) == Some("do")
    {
      output.extend(children.iter().skip(1).cloned());
    } else {
      output.push(item);
    }
  }
  Cirru::List(output)
}

/// Read the leaf head of one source list.
fn list_head(node: &Cirru) -> Option<&str> {
  match node {
    Cirru::List(items) => items.first().and_then(leaf_value),
    Cirru::Leaf(_) => None,
  }
}

/// Borrow a source leaf value without accepting nested forms.
fn leaf_value(node: &Cirru) -> Option<&str> {
  match node {
    Cirru::Leaf(value) => Some(value.as_ref()),
    Cirru::List(_) => None,
  }
}

/// Treat common lexical binders conservatively: one matching local name suppresses migration in the whole definition.
fn collect_potential_local_bindings(node: &Cirru, output: &mut HashSet<String>) {
  let Cirru::List(items) = node else {
    return;
  };
  if matches!(items.first().and_then(leaf_value), Some("quote" | "quasiquote")) {
    return;
  }
  match items.first().and_then(leaf_value) {
    Some("defn" | "defmacro") => collect_binding_tree(items.get(2), output),
    Some("fn") => collect_binding_tree(items.get(1), output),
    Some("let" | "loop") => collect_binding_positions(items.get(1), true, output),
    Some("&let" | "doseq" | "if-let" | "when-let") => collect_binding_positions(items.get(1), false, output),
    _ => {}
  }
  for child in items {
    collect_potential_local_bindings(child, output);
  }
}

/// Collect only binding patterns, never leaves from initializer expressions.
fn collect_binding_positions(node: Option<&Cirru>, paired: bool, output: &mut HashSet<String>) {
  let Some(Cirru::List(items)) = node else {
    return;
  };
  if paired {
    for binding in items {
      if let Cirru::List(pair) = binding {
        collect_binding_tree(pair.first(), output);
      }
    }
  } else {
    for binding in items.iter().step_by(2) {
      collect_binding_tree(Some(binding), output);
    }
  }
}

/// Conservatively collect leaves from one supported binding pattern.
fn collect_binding_tree(node: Option<&Cirru>, output: &mut HashSet<String>) {
  match node {
    Some(Cirru::Leaf(value)) if !value.starts_with('&') => {
      output.insert(value.to_string());
    }
    Some(Cirru::List(items)) => {
      for item in items {
        collect_binding_tree(Some(item), output);
      }
    }
    _ => {}
  }
}

/// Reject bare nominal names that may resolve to a local binding instead of a definition.
fn prototype_is_shadowed(prototype: &str, shadowed: &HashSet<String>) -> bool {
  if prototype.contains('/') {
    false
  } else {
    shadowed.contains(prototype)
  }
}

/// Resolve a nominal prototype to its exact editable project definition.
fn resolve_project_nominal_target(snapshot: &Snapshot, at_ns: &str, prototype: &str, kind: NominalKind) -> Option<(String, String)> {
  let target = if let Some((prefix, definition)) = prototype.rsplit_once('/') {
    let namespace = if snapshot.files.contains_key(prefix) {
      prefix.to_owned()
    } else {
      let namespace =
        program::lookup_ns_target_in_import(at_ns, prefix).or_else(|| program::lookup_default_target_in_import(at_ns, prefix))?;
      namespace.to_string()
    };
    (namespace, definition.to_owned())
  } else if nominal_definition_matches(snapshot, at_ns, prototype, kind) {
    return Some((at_ns.to_owned(), prototype.to_owned()));
  } else {
    imported_definition_target(at_ns, prototype)?
  };
  nominal_definition_matches(snapshot, &target.0, &target.1, kind).then_some(target)
}

/// Resolve a referred local import name without losing a renamed target definition.
fn imported_definition_target(at_ns: &str, local_name: &str) -> Option<(String, String)> {
  let program_data = program::PROGRAM_CODE_DATA.read().ok()?;
  let rule = program_data.get(at_ns)?.import_map.get(local_name)?;
  match rule.as_ref() {
    program::ImportRule::NsReferDef(namespace, definition) => Some((namespace.to_string(), definition.to_string())),
    program::ImportRule::NsAs(_) | program::ImportRule::NsDefault(_) => None,
  }
}

/// Check that a project definition has the expected nominal declaration kind.
fn nominal_definition_matches(snapshot: &Snapshot, namespace: &str, definition: &str, kind: NominalKind) -> bool {
  snapshot
    .files
    .get(namespace)
    .and_then(|file| file.defs.get(definition))
    .is_some_and(|entry| list_head(&entry.code) == Some(kind.definition_head()))
}

/// Require every declared Struct field exactly once before flattening legacy field pairs.
fn struct_fields_are_complete(snapshot: &Snapshot, namespace: &str, definition: &str, constructor: &[Cirru]) -> bool {
  let Some(Cirru::List(definition_items)) = snapshot
    .files
    .get(namespace)
    .and_then(|file| file.defs.get(definition))
    .map(|entry| &entry.code)
  else {
    return false;
  };
  let declared = definition_items
    .iter()
    .skip(2)
    .filter_map(|field| match field {
      Cirru::List(pair) if pair.len() >= 2 => pair.first().and_then(leaf_value).filter(|name| name.starts_with(':')),
      _ => None,
    })
    .collect::<HashSet<_>>();
  let mut provided = HashSet::new();
  for field in constructor.iter().skip(2) {
    let Some(name) = (match field {
      Cirru::List(pair) if pair.len() == 2 => pair.first().and_then(leaf_value).filter(|name| name.starts_with(':')),
      _ => None,
    }) else {
      return false;
    };
    if !provided.insert(name) {
      return false;
    }
  }
  provided == declared
}

/// Deduplicate identical compiler suggestions and fail closed on conflicting replacements.
fn insert_fix_suggestion(
  suggestions: &mut BTreeMap<(String, String, Vec<usize>), FixSuggestion>,
  key: (String, String, Vec<usize>),
  suggestion: FixSuggestion,
) -> Result<(), String> {
  if let Some(existing) = suggestions.get(&key) {
    if existing != &suggestion {
      return Err(format!(
        "Fix suggestions overlap at {}/{} {}; refusing to choose between different replacements.",
        key.0,
        key.1,
        format_path(&key.2)
      ));
    }
  } else {
    suggestions.insert(key, suggestion);
  }
  Ok(())
}

/// Resolve a diagnostic coordinate to the exact source leaf that owns the migration.
fn resolve_fix_target(code: &Cirru, coordinate: &[usize]) -> Option<(Vec<usize>, String)> {
  let node = navigate_to_path(code, coordinate).ok()?;
  match node {
    Cirru::Leaf(value) => Some((coordinate.to_vec(), value.to_string())),
    Cirru::List(items) => match items.first() {
      Some(Cirru::Leaf(value)) => {
        let mut head_path = coordinate.to_vec();
        head_path.push(0);
        Some((head_path, value.to_string()))
      }
      _ => None,
    },
  }
}

/// Preserve namespace qualification while resolving a removed data API migration.
fn migration_for_source_leaf(source: &str) -> Option<(Option<String>, String)> {
  let (prefix, name) = source.rsplit_once('/').map_or(("", source), |(prefix, name)| (prefix, name));
  let migration = runner::preprocess::removed_data_api_migration(name)?;
  let replacement = migration.replacement.map(|replacement| {
    if prefix.is_empty() {
      replacement
    } else {
      format!("{prefix}/{replacement}")
    }
  });
  let guidance = replacement.clone().unwrap_or(migration.guidance);
  Some((replacement, guidance))
}

/// Lower one typed suggestion into guarded tree commands for the staged transaction.
fn suggestion_operations(suggestion: &FixSuggestion) -> Vec<Vec<String>> {
  match &suggestion.operation {
    Some(FixOperation::ReplaceLeaf { original, replacement }) => vec![vec![
      "tree".to_owned(),
      "replace".to_owned(),
      suggestion.definition.clone(),
      "--path".to_owned(),
      format_path(&suggestion.target_path),
      "--expect".to_owned(),
      format!("quote {original}"),
      "--code".to_owned(),
      format!("quote {replacement}"),
    ]],
    Some(FixOperation::WrapLeafCall { original }) => vec![vec![
      "tree".to_owned(),
      "replace".to_owned(),
      suggestion.definition.clone(),
      "--path".to_owned(),
      format_path(&suggestion.target_path),
      "--expect".to_owned(),
      format!("quote {original}"),
      "--code".to_owned(),
      format!("quote $ {original}"),
    ]],
    Some(FixOperation::ReplaceNode { original, replacement }) => vec![vec![
      "tree".to_owned(),
      "replace".to_owned(),
      suggestion.definition.clone(),
      "--path".to_owned(),
      format_path(&suggestion.target_path),
      "--expect".to_owned(),
      format!("quote $ {original}"),
      "--code".to_owned(),
      format!("quote $ {replacement}"),
    ]],
    Some(FixOperation::SpliceDo) => {
      let mut head_path = suggestion.target_path.clone();
      head_path.push(0);
      vec![
        vec![
          "tree".to_owned(),
          "delete".to_owned(),
          suggestion.definition.clone(),
          "--path".to_owned(),
          format_path(&head_path),
          "--expect".to_owned(),
          "quote do".to_owned(),
        ],
        vec![
          "tree".to_owned(),
          "unwrap".to_owned(),
          suggestion.definition.clone(),
          "--path".to_owned(),
          format_path(&suggestion.target_path),
        ],
      ]
    }
    Some(FixOperation::ReplaceImports { code }) => vec![vec![
      "edit".to_owned(),
      "imports".to_owned(),
      suggestion.definition.clone(),
      "--code".to_owned(),
      code.clone(),
    ]],
    Some(FixOperation::ReplaceExamples { code }) => vec![vec![
      "edit".to_owned(),
      "examples".to_owned(),
      suggestion.definition.clone(),
      "--code".to_owned(),
      code.clone(),
    ]],
    Some(FixOperation::ReplaceTest { name, tags, code }) => {
      let mut args = vec![
        "edit".to_owned(),
        "add-test".to_owned(),
        suggestion.definition.clone(),
        name.clone(),
        "--code".to_owned(),
        code.clone(),
        "--overwrite".to_owned(),
      ];
      if !tags.is_empty() {
        args.push("--tags".to_owned());
        args.push(tags.clone());
      }
      vec![args]
    }
    Some(FixOperation::ReplaceSchema { code }) => vec![vec![
      "edit".to_owned(),
      "schema".to_owned(),
      suggestion.definition.clone(),
      "--code".to_owned(),
      code.clone(),
    ]],
    Some(FixOperation::ReplaceDefinition { code }) => vec![vec![
      "edit".to_owned(),
      "def".to_owned(),
      suggestion.definition.clone(),
      "--code".to_owned(),
      code.clone(),
      "--overwrite".to_owned(),
    ]],
    Some(FixOperation::RenameDefinition { new_name }) => vec![vec![
      "edit".to_owned(),
      "rename".to_owned(),
      suggestion.definition.clone(),
      new_name.clone(),
    ]],
    None => vec![],
  }
}

/// Encode one Cirru node as the public quoted-AST JSON envelope.
fn quoted_json(node: &Cirru) -> Value {
  serde_json::json!({
    "$type": "quote",
    "value": cirru_to_json_value(node),
  })
}

/// Decode a public quote or splice envelope for a human source preview.
fn fix_source_json_to_cirru(value: &Value) -> Result<Cirru, String> {
  let Value::Object(fields) = value else {
    return Err("expected a source AST object".to_owned());
  };
  if !matches!(fields.get("$type").and_then(Value::as_str), Some("quote" | "splice")) {
    return Err("expected `$type: quote` or `$type: splice` in source preview".to_owned());
  }
  let node = fields
    .get("value")
    .ok_or_else(|| "quoted AST source preview is missing `value`".to_owned())?;
  json_value_to_cirru(node)
}

/// Fingerprint the canonical JSON shape used in machine-readable fix plans.
fn node_fingerprint(node: &Cirru) -> String {
  let mut hasher = Md5::new();
  hasher.update(cirru_to_json_value(node).to_string().as_bytes());
  format!("md5:{}", hex::encode(hasher.finalize()))
}

/// Require an inspectable, clean VCS boundary unless the caller explicitly opts out.
fn guard_git_worktree(snapshot_file: &str, allow_dirty: bool, allow_no_vcs: bool) -> Result<(), String> {
  let project_dir = Path::new(snapshot_file).parent().unwrap_or_else(|| Path::new("."));
  let probe = Command::new("git")
    .arg("-C")
    .arg(project_dir)
    .args(["rev-parse", "--show-toplevel"])
    .output();
  let Ok(probe) = probe else {
    if allow_no_vcs {
      return Ok(());
    }
    return Err("Cannot verify a Git worktree. Re-run with `--allow-no-vcs` only after reviewing the preview.".to_owned());
  };
  if !probe.status.success() {
    if allow_no_vcs {
      return Ok(());
    }
    return Err("Snapshot is not inside a Git worktree. Re-run with `--allow-no-vcs` only after reviewing the preview.".to_owned());
  }
  if allow_dirty {
    return Ok(());
  }
  let status = Command::new("git")
    .arg("-C")
    .arg(project_dir)
    .args(["status", "--porcelain", "--untracked-files=normal"])
    .output()
    .map_err(|error| format!("Failed to inspect Git worktree: {error}"))?;
  if !status.status.success() {
    return Err(format!(
      "Failed to inspect Git worktree: {}",
      String::from_utf8_lossy(&status.stderr).trim_end()
    ));
  }
  if !status.stdout.is_empty() {
    return Err("Git worktree is dirty. Commit/stash changes, or re-run with `--allow-dirty` after reviewing the preview.".to_owned());
  }
  Ok(())
}

/// Render the compact human view without changing the JSON protocol.
fn print_human_report(report: &FixReport<'_>) {
  println!(
    "# {}\n",
    if report.data.filters.rule_id == "pattern" {
      "Reviewed structural source rewrites"
    } else {
      "Compiler-guided source fixes"
    }
  );
  println!("- mode: `{}`", report.data.mode);
  println!("- revision: `{}`", report.revision);
  if let Some(preset) = report.data.filters.preset_id {
    println!("- preset: `{preset}`");
  }
  if let Some(coverage) = &report.data.filters.source_coverage {
    println!("- scanned source regions: `{}`", coverage.scanned_regions.join(", "));
    println!("- manual review source regions: `{}`", coverage.manual_review_regions.join(", "));
  }
  if let Some(replacement) = report.data.filters.replacement_name {
    println!("- replacement definition: `{replacement}`");
  }
  println!("- rules: `{}`", report.data.filters.expanded_rule_ids.join(", "));
  println!("- suggestions: `{}`", report.data.suggestions.len());
  println!("- changed: `{}`", report.data.changed);
  if let Some(workflow) = &report.data.workflow {
    println!("- workflow: `{}`", workflow.workflow);
    println!("- workflow status: `{}`", workflow.status);
    println!("- entries: `{}`", workflow.entries.len());
    println!("- review-required source fixes: `{}`", workflow.review_required.source_fixes.len());
    println!(
      "- review-required type findings: `{}`",
      workflow.review_required.type_findings.len()
    );
    println!("- retained type boundaries: `{}`", workflow.retained_type_boundaries.len());
    println!("- FFI boundaries: `{}`", workflow.review_required.ffi_boundaries.len());
    println!("- verification results: `{}`", workflow.verification.results.len());
  }
  for (index, suggestion) in report.data.suggestions.iter().enumerate() {
    println!("\n## Suggestion {}\n", index + 1);
    println!("- applicability: `{}`", suggestion.applicability);
    println!("- rule: `{}`", suggestion.rule_id);
    println!("- definition: `{}`", suggestion.definition);
    println!("- path: `{}`", suggestion.path);
    println!("- message: {}\n", suggestion.message);
    match fix_source_json_to_cirru(&suggestion.original).and_then(|node| markdown_cirru_section(3, "Before", &node, 0)) {
      Ok(section) => println!("{section}"),
      Err(error) => println!("### Before\n\n_Unable to render source preview: {error}_"),
    }
    if let Some(replacement) = &suggestion.replacement {
      match fix_source_json_to_cirru(replacement).and_then(|node| markdown_cirru_section(3, "After", &node, 0)) {
        Ok(section) => println!("\n{section}"),
        Err(error) => println!("\n### After\n\n_Unable to render source preview: {error}_"),
      }
    }
  }
  if report.data.mode == "preview" && report.data.changed {
    println!("\n## Next step\n\nRun `calcit fix --apply` after reviewing this plan.");
  }
  if report.data.mode == "preview" && report.data.filters.rule_id == "pattern" && !report.data.suggestions.is_empty() {
    println!(
      "\n## Next step\n\nReview the pattern/template and every candidate, then repeat the same selection with `--apply --expect-revision '{}'`. Type validation runs before publishing; it does not prove semantic equivalence.",
      report.revision
    );
  }
}

#[cfg(test)]
mod tests {
  use super::{
    ConstructorScope, FixOperation, FixSuggestion, NominalKind, REMOVED_DATA_API_RULE, collect_builtin_round_call_heads,
    collect_potential_local_bindings, collect_redundant_do_paths, fix_rule_metadata, fix_source_json_to_cirru, insert_fix_suggestion,
    legacy_constructor_replacement, migration_for_source_leaf, optional_candidate_signature_is_closed,
    optional_candidate_type_is_closed, optional_parameter_candidate, prototype_is_shadowed, resolve_fix_target,
    rewrite_loaded_schema_type_references, rewrite_named_constructor_tree, struct_fields_are_complete, suggestion_operations,
  };
  use calcit::calcit::{CalcitFnTypeAnnotation, CalcitGenericBound, CalcitTrait, CalcitTypeAnnotation, SchemaKind};
  use cirru_parser::Cirru;
  use serde_json::Value;
  use std::collections::{BTreeMap, HashSet};
  use std::sync::Arc;

  use super::super::common::markdown_cirru_section;

  fn leaf(value: &str) -> Cirru {
    Cirru::leaf(value)
  }

  #[test]
  fn optional_parameter_candidates_do_not_invent_dynamic_or_double_wrap_option() {
    assert_eq!(optional_parameter_candidate(&CalcitTypeAnnotation::Dynamic), None);
    assert_eq!(
      optional_parameter_candidate(&CalcitTypeAnnotation::Number),
      Some("Option<:number>".to_owned())
    );
    assert_eq!(
      optional_parameter_candidate(&CalcitTypeAnnotation::Optional(Arc::new(CalcitTypeAnnotation::String))),
      Some("Option<:string>".to_owned())
    );
    let option = CalcitTypeAnnotation::TypeRef(
      Arc::from("calcit.core/Option"),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::String)]),
    );
    assert_eq!(optional_parameter_candidate(&option), Some(option.to_brief_string()));
  }

  #[test]
  fn optional_fn_candidate_checks_nested_members_and_rest_type() {
    let open_map = Arc::new(CalcitTypeAnnotation::Map(
      Arc::new(CalcitTypeAnnotation::Number),
      Arc::new(CalcitTypeAnnotation::Dynamic),
    ));
    let closed_map = Arc::new(CalcitTypeAnnotation::Map(
      Arc::new(CalcitTypeAnnotation::Number),
      Arc::new(CalcitTypeAnnotation::String),
    ));
    assert!(!optional_candidate_type_is_closed(&CalcitTypeAnnotation::List(open_map)));
    assert!(optional_candidate_type_is_closed(&CalcitTypeAnnotation::List(closed_map)));

    let mut signature = CalcitFnTypeAnnotation {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![Arc::new(CalcitTypeAnnotation::Number)],
      return_type: Arc::new(CalcitTypeAnnotation::Unit),
      fn_kind: SchemaKind::Fn,
      rest_type: Some(Arc::new(CalcitTypeAnnotation::Dynamic)),
      features: Arc::new(std::collections::HashSet::new()),
    };
    assert!(!optional_candidate_signature_is_closed(&signature));
    signature.rest_type = Some(Arc::new(CalcitTypeAnnotation::String));
    assert!(optional_candidate_signature_is_closed(&signature));
    signature.return_type = Arc::new(CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Dynamic)));
    assert!(!optional_candidate_signature_is_closed(&signature));
  }

  #[test]
  fn splice_fix_source_decodes_the_replacement_sequence() {
    let replacement = serde_json::json!({
      "$type": "splice",
      "value": [["println", "working"], ["finish", "value"]],
    });
    let node = fix_source_json_to_cirru(&replacement).expect("splice replacement should decode");
    assert_eq!(
      node,
      Cirru::List(vec![
        Cirru::List(vec![leaf("println"), leaf("working")]),
        Cirru::List(vec![leaf("finish"), leaf("value")]),
      ])
    );
    let section = markdown_cirru_section(3, "After", &node, 0).expect("splice replacement should render");
    assert!(section.contains("### After\n"));
    assert!(section.contains("```cirru\n"));
    assert!(section.contains("println working"));
    assert!(section.contains("finish value"));
  }

  #[test]
  fn preserves_source_qualifier_for_exact_removed_api_migration() {
    assert_eq!(
      migration_for_source_leaf("core/tuple-enum"),
      Some((Some("core/enum-definition".to_owned()), "core/enum-definition".to_owned()))
    );
  }

  #[test]
  fn removed_data_fix_is_derived_from_the_current_diagnostic() {
    let metadata = fix_rule_metadata(REMOVED_DATA_API_RULE);
    assert_eq!(metadata.diagnostic_code, "W_REMOVED_DATA_API");
    assert_eq!(metadata.evidence_source, "current-diagnostic");
    assert_eq!(metadata.lifecycle, "current-semantics");
    assert!(!metadata.source_version_required);
  }

  #[test]
  fn keeps_ambiguous_tuple_predicate_for_review() {
    assert_eq!(
      migration_for_source_leaf("tuple?"),
      Some((None, "enum? (values) or enum-def? (definitions)".to_owned()))
    );
  }

  #[test]
  fn warning_coordinate_can_identify_a_call_or_a_leaf() {
    let code = Cirru::List(vec![leaf("do"), Cirru::List(vec![leaf("tuple-enum"), leaf("value")])]);
    assert_eq!(resolve_fix_target(&code, &[1]), Some((vec![1, 0], "tuple-enum".to_owned())));
    assert_eq!(resolve_fix_target(&code, &[1, 0]), Some((vec![1, 0], "tuple-enum".to_owned())));
  }

  #[test]
  fn generated_operation_carries_path_and_expected_leaf() {
    let suggestion = FixSuggestion {
      rule_id: "removed-data-api-v1",
      diagnostic_code: "W_REMOVED_DATA_API",
      semantic_layer: "surface",
      source_file: "calcit.cirru".to_owned(),
      definition: "app.main/main!".to_owned(),
      path: "code@3.0".to_owned(),
      fingerprint: "md5:test".to_owned(),
      origin_chain: vec![],
      original: Value::Null,
      replacement: Value::Null.into(),
      applicability: "machine-applicable",
      message: String::new(),
      target_path: vec![3, 0],
      operation: Some(FixOperation::ReplaceLeaf {
        original: "tuple-enum".to_owned(),
        replacement: "enum-definition".to_owned(),
      }),
    };
    let operations = suggestion_operations(&suggestion);
    assert_eq!(
      operations,
      vec![vec![
        "tree",
        "replace",
        "app.main/main!",
        "--path",
        "@3.0",
        "--expect",
        "quote tuple-enum",
        "--code",
        "quote enum-definition",
      ]]
    );
  }

  #[test]
  fn redundant_do_is_found_only_in_variadic_sequence_positions() {
    let code = Cirru::List(vec![
      leaf("defn"),
      leaf("demo"),
      Cirru::List(vec![]),
      Cirru::List(vec![leaf("do"), leaf("a"), leaf("b")]),
      Cirru::List(vec![
        leaf("if"),
        leaf("ready?"),
        Cirru::List(vec![leaf("do"), leaf("c"), leaf("d")]),
        leaf("e"),
      ]),
      Cirru::List(vec![
        leaf("let"),
        Cirru::List(vec![]),
        Cirru::List(vec![leaf("do"), leaf("f"), leaf("g")]),
      ]),
      Cirru::List(vec![
        leaf("quote"),
        Cirru::List(vec![leaf("fn"), Cirru::List(vec![]), Cirru::List(vec![leaf("do"), leaf("h")])]),
      ]),
      Cirru::List(vec![
        leaf("defmacro"),
        leaf("pack"),
        Cirru::List(vec![]),
        Cirru::List(vec![leaf("do"), leaf("i"), leaf("j")]),
      ]),
      Cirru::List(vec![
        leaf("let[]"),
        Cirru::List(vec![]),
        Cirru::List(vec![]),
        Cirru::List(vec![leaf("do"), leaf("k"), leaf("l")]),
      ]),
    ]);
    let mut paths = Vec::new();
    collect_redundant_do_paths(&code, &mut Vec::new(), &mut paths);
    assert_eq!(paths, vec![vec![3], vec![5, 2], vec![8, 3]]);
  }

  #[test]
  fn redundant_do_operation_deletes_the_head_then_splices_the_body() {
    let suggestion = FixSuggestion {
      rule_id: "redundant-do-v1",
      diagnostic_code: "FIX_REDUNDANT_DO",
      semantic_layer: "surface",
      source_file: "calcit.cirru".to_owned(),
      definition: "app.main/demo".to_owned(),
      path: "code@3".to_owned(),
      fingerprint: "md5:test".to_owned(),
      origin_chain: vec![],
      original: Value::Null,
      replacement: Some(Value::Null),
      applicability: "machine-applicable",
      message: String::new(),
      target_path: vec![3],
      operation: Some(FixOperation::SpliceDo),
    };
    assert_eq!(
      suggestion_operations(&suggestion),
      vec![
        vec!["tree", "delete", "app.main/demo", "--path", "@3.0", "--expect", "quote do"],
        vec!["tree", "unwrap", "app.main/demo", "--path", "@3"],
      ]
    );
  }

  #[test]
  fn constructor_replacement_keeps_argument_order() {
    let suggestion = FixSuggestion {
      rule_id: "named-enum-constructor-v1",
      diagnostic_code: "FIX_NAMED_ENUM_CONSTRUCTOR",
      semantic_layer: "surface",
      source_file: "calcit.cirru".to_owned(),
      definition: "app.main/demo".to_owned(),
      path: "code@3".to_owned(),
      fingerprint: "md5:test".to_owned(),
      origin_chain: vec![],
      original: Value::Null,
      replacement: Some(Value::Null),
      applicability: "machine-applicable",
      message: String::new(),
      target_path: vec![3],
      operation: Some(FixOperation::ReplaceNode {
        original: "%:: Result :ok value".to_owned(),
        replacement: "Result :ok value".to_owned(),
      }),
    };
    assert_eq!(
      suggestion_operations(&suggestion),
      vec![vec![
        "tree",
        "replace",
        "app.main/demo",
        "--path",
        "@3",
        "--expect",
        "quote $ %:: Result :ok value",
        "--code",
        "quote $ Result :ok value",
      ]]
    );
  }

  #[test]
  fn local_nominal_name_is_treated_as_shadowed() {
    let code = Cirru::List(vec![
      leaf("defn"),
      leaf("demo"),
      Cirru::List(vec![leaf("Result")]),
      Cirru::List(vec![leaf("%::"), leaf("Result"), leaf(":ok")]),
    ]);
    let mut shadowed = std::collections::HashSet::new();
    collect_potential_local_bindings(&code, &mut shadowed);
    assert!(prototype_is_shadowed("Result", &shadowed));
    assert!(!prototype_is_shadowed("app.schema/Result", &shadowed));
  }

  #[test]
  fn integer_predicate_scanner_rejects_binder_shaped_source() {
    let parameter = Cirru::List(vec![
      leaf("defn"),
      leaf("demo"),
      Cirru::List(vec![leaf("round?"), leaf("x")]),
      Cirru::List(vec![leaf("round?"), leaf("x")]),
    ]);
    let binding = Cirru::List(vec![
      leaf("defn"),
      leaf("demo"),
      Cirru::List(vec![]),
      Cirru::List(vec![
        leaf("let"),
        Cirru::List(vec![Cirru::List(vec![leaf("round?"), leaf("x")])]),
        Cirru::List(vec![leaf("round?"), leaf("x")]),
      ]),
    ]);
    for code in [parameter, binding] {
      let mut bindings = HashSet::new();
      collect_potential_local_bindings(&code, &mut bindings);
      assert!(bindings.contains("round?"));
      let mut heads = Vec::new();
      collect_builtin_round_call_heads(&code, &mut Vec::new(), &mut heads);
      assert!(!heads.is_empty(), "the binder guard must precede shape-based call scanning");
    }
  }

  #[test]
  fn legacy_struct_pairs_are_flattened_for_direct_construction() {
    let items = vec![
      leaf("%{}"),
      leaf("Person"),
      Cirru::List(vec![leaf(":name"), leaf("name")]),
      Cirru::List(vec![leaf(":age"), leaf("age")]),
    ];
    assert_eq!(
      legacy_constructor_replacement(&items, NominalKind::Struct),
      Some(Cirru::List(vec![
        leaf("Person"),
        leaf(":name"),
        leaf("name"),
        leaf(":age"),
        leaf("age")
      ]))
    );
    assert_eq!(
      legacy_constructor_replacement(&[leaf("%{}"), leaf("Person"), leaf("dynamic-fields")], NominalKind::Struct),
      None
    );
  }

  #[test]
  fn equivalent_readable_constructors_converge_to_the_same_current_tree() {
    let snapshot = super::load_snapshot("tests/fixtures/fix-command.cirru").expect("fix fixture should load");
    let legacy = Cirru::List(vec![leaf("%::"), leaf("FixPersonChoice"), leaf(":none")]);
    let current = Cirru::List(vec![leaf("FixPersonChoice"), leaf(":none")]);
    let shadowed = std::collections::HashSet::new();
    let kinds = [NominalKind::Enum];

    let scope = ConstructorScope {
      snapshot: &snapshot,
      namespace: "fix-command.main",
      shadowed: &shadowed,
      kinds: &kinds,
      macro_body: false,
    };

    let normalized_legacy = rewrite_named_constructor_tree(&legacy, 0, &scope, false, false);
    let normalized_current = rewrite_named_constructor_tree(&current, 0, &scope, false, false);

    assert_eq!(normalized_legacy, current);
    assert_eq!(normalized_current, current);
  }

  #[test]
  fn legacy_struct_requires_complete_unique_declared_fields() {
    let snapshot = super::load_snapshot("tests/fixtures/fix-command.cirru").expect("fix fixture should load");
    let complete = vec![
      leaf("%{}"),
      leaf("FixPerson"),
      Cirru::List(vec![leaf(":name"), leaf("name")]),
      Cirru::List(vec![leaf(":age"), leaf("age")]),
    ];
    assert!(struct_fields_are_complete(&snapshot, "fix-command.main", "FixPerson", &complete));
    assert!(!struct_fields_are_complete(
      &snapshot,
      "fix-command.main",
      "FixPerson",
      &complete[..3]
    ));

    let duplicate = vec![
      leaf("%{}"),
      leaf("FixPerson"),
      Cirru::List(vec![leaf(":name"), leaf("first")]),
      Cirru::List(vec![leaf(":name"), leaf("second")]),
      Cirru::List(vec![leaf(":age"), leaf("age")]),
    ];
    assert!(!struct_fields_are_complete(&snapshot, "fix-command.main", "FixPerson", &duplicate));
  }

  #[test]
  fn quoted_binding_shapes_do_not_suppress_real_migrations() {
    let code = Cirru::List(vec![
      leaf("defn"),
      leaf("demo"),
      Cirru::List(vec![]),
      Cirru::List(vec![
        leaf("quote"),
        Cirru::List(vec![leaf("fn"), Cirru::List(vec![leaf("Result")]), leaf("Result")]),
      ]),
    ]);
    let mut shadowed = std::collections::HashSet::new();
    collect_potential_local_bindings(&code, &mut shadowed);
    assert!(!shadowed.contains("Result"));
  }

  #[test]
  fn conflicting_suggestions_for_one_source_node_are_rejected() {
    let mut suggestions = BTreeMap::new();
    let key = ("app.main".to_owned(), "main!".to_owned(), vec![3, 0]);
    let first = FixSuggestion {
      rule_id: "removed-data-api-v1",
      diagnostic_code: "W_REMOVED_DATA_API",
      semantic_layer: "surface",
      source_file: "calcit.cirru".to_owned(),
      definition: "app.main/main!".to_owned(),
      path: "code@3.0".to_owned(),
      fingerprint: "md5:test".to_owned(),
      origin_chain: vec![],
      original: Value::Null,
      replacement: Some(Value::String("enum-definition".to_owned())),
      applicability: "machine-applicable",
      message: String::new(),
      target_path: vec![3, 0],
      operation: Some(FixOperation::ReplaceLeaf {
        original: "tuple-enum".to_owned(),
        replacement: "enum-definition".to_owned(),
      }),
    };
    insert_fix_suggestion(&mut suggestions, key.clone(), first.clone()).expect("first suggestion should insert");
    insert_fix_suggestion(&mut suggestions, key.clone(), first.clone()).expect("identical duplicate should be idempotent");

    let mut conflicting = first;
    conflicting.operation = Some(FixOperation::ReplaceLeaf {
      original: "tuple-enum".to_owned(),
      replacement: "enum?".to_owned(),
    });
    let error = insert_fix_suggestion(&mut suggestions, key, conflicting).expect_err("conflicting overlap should fail");

    assert!(error.contains("overlap at app.main/main! @3.0"), "error: {error}");
    assert_eq!(suggestions.len(), 1);
  }

  #[test]
  fn schema_reference_rewrite_only_changes_loaded_type_refs() {
    let schema = Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from("app.schema/Order"), Arc::new(vec![])));
    let (rewritten, count) =
      rewrite_loaded_schema_type_references(schema, "app.main", "app.schema", "Order", "Purchase").expect("rewrite TypeRef");
    assert_eq!(count, 1);
    assert!(matches!(
      rewritten.as_ref(),
      CalcitTypeAnnotation::TypeRef(name, args) if name.as_ref() == "app.schema/Purchase" && args.is_empty()
    ));

    let generic = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("Order")));
    let (unchanged, count) = rewrite_loaded_schema_type_references(generic.clone(), "app.schema", "app.schema", "Order", "Purchase")
      .expect("preserve TypeVar");
    assert_eq!(count, 0);
    assert_eq!(unchanged, generic);
  }

  #[test]
  fn schema_reference_rewrite_changes_resolved_trait_bounds() {
    let direct_trait = Arc::new(CalcitTrait::new_reference("app.schema/Show"));
    let schema = Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(vec![CalcitGenericBound {
        name: Arc::from("T"),
        traits: Arc::new(vec![Arc::new(CalcitTrait::new_reference("app.schema/Show"))]),
      }]),
      arg_types: vec![
        Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T"))),
        Arc::new(CalcitTypeAnnotation::Trait(direct_trait)),
      ],
      return_type: Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T"))),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(std::collections::HashSet::new()),
    })));

    let (rewritten, count) =
      rewrite_loaded_schema_type_references(schema, "app.main", "app.schema", "Show", "Display").expect("rewrite trait bound");
    let CalcitTypeAnnotation::Fn(signature) = rewritten.as_ref() else {
      panic!("rewritten schema should remain a function");
    };
    let trait_def = &signature.where_bounds[0].traits[0];
    assert_eq!(count, 2);
    assert_eq!(trait_def.name.ref_str(), "Display");
    assert_eq!(trait_def.definition_ref.as_deref(), Some("app.schema/Display"));
    assert!(matches!(signature.arg_types[0].as_ref(), CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "T"));
    assert!(matches!(
      signature.arg_types[1].as_ref(),
      CalcitTypeAnnotation::Trait(trait_def)
        if trait_def.name.ref_str() == "Display" && trait_def.definition_ref.as_deref() == Some("app.schema/Display")
    ));
  }
}
