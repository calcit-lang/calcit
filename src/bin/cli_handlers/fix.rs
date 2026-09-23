use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;
use std::process::Command;

use calcit::calcit::{CalcitFnTypeAnnotation, CalcitTypeAnnotation, LocatedWarning, SchemaKind};
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
const RENAME_DEFINITION_RULE: &str = "rename-definition-v1";
const RENAME_DEFINITION_DIAGNOSTIC: &str = "REFACTOR_RENAME_DEFINITION";
const VALUE_TO_ZERO_ARG_FN_RULE: &str = "value-to-zero-arg-fn-v1";
const VALUE_TO_ZERO_ARG_FN_DIAGNOSTIC: &str = "REFACTOR_VALUE_TO_ZERO_ARG_FN";
const SYNTHESIZE_SCHEMA_RULE: &str = "synthesize-schema-v1";
const SYNTHESIZE_SCHEMA_DIAGNOSTIC: &str = "REFACTOR_SYNTHESIZE_SCHEMA";
const SURFACE_LATEST_V1_PRESET: &str = "surface-latest-v1";
const SURFACE_LATEST_V2_PRESET: &str = "surface-latest-v2";
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
  validate_options(options)?;
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
  let value_to_zero_arg_fn = selected_rules.contains(&VALUE_TO_ZERO_ARG_FN_RULE);
  let schema_synthesis = selected_rules.contains(&SYNTHESIZE_SCHEMA_RULE);
  let semantic_refactor = semantic_rename || value_to_zero_arg_fn;
  let migration_rule = semantic_refactor || schema_synthesis || options.workflow.is_some();
  let validation_only = std::env::var("CALCIT_FIX_VALIDATE_ONLY").as_deref() == Ok("1");
  let project_definitions = if semantic_refactor || schema_synthesis {
    select_project_definitions(compiled_snapshot, project_namespaces)?
  } else {
    select_definitions(options, compiled_snapshot, project_namespaces)?
  };
  let selected_definitions = if schema_synthesis && validation_only {
    select_definitions(options, compiled_snapshot, project_namespaces)?
  } else {
    project_definitions.clone()
  };
  let warnings = if validation_only {
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
  } else if value_to_zero_arg_fn || schema_synthesis {
    Some(warning_identities(&warnings))
  } else {
    None
  };
  let mut suggestions = Vec::new();
  if semantic_rename {
    suggestions.extend(plan_definition_rename(
      options,
      &source_snapshot,
      snapshot_file,
      &project_definitions,
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
  } else if selected_rules.contains(&REMOVED_DATA_API_RULE) {
    suggestions.extend(plan_removed_data_api_fixes(options, &source_snapshot, snapshot_file, &warnings)?);
  }
  let mut constructor_kinds = Vec::new();
  if selected_rules.contains(&NAMED_ENUM_CONSTRUCTOR_RULE) {
    constructor_kinds.push(NominalKind::Enum);
  }
  if selected_rules.contains(&NAMED_STRUCT_CONSTRUCTOR_RULE) {
    constructor_kinds.push(NominalKind::Struct);
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
    workflow_failed =
      options.verify && (!operations.is_empty() || preflight.is_failed() || results.iter().any(|result| result.status != "passed"));
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
      },
      changed: transaction.changed,
      new_revision: &transaction.new_revision,
      validation: FixValidation {
        status: if operations.is_empty() { "not-needed" } else { "passed" },
        staged_scope_preprocess: !operations.is_empty(),
        checked_operations: operations.len(),
      },
      suggestions: &suggestions,
      workflow,
    },
    diagnostics: vec![],
    next: vec![],
  };

  match StructuredOutputFormat::parse(&options.format, "fix")? {
    StructuredOutputFormat::Json => println!(
      "{}",
      serde_json::to_string(&report).map_err(|error| format!("Failed to serialize fix result: {error}"))?
    ),
    StructuredOutputFormat::Edn => {
      let value = serde_json::to_value(&report).map_err(|error| format!("Failed to encode fix result: {error}"))?;
      println!("{}", format_json_value_as_edn(&value)?);
    }
    StructuredOutputFormat::Human => print_human_report(&report),
  }
  if workflow_failed {
    Err("Strict project workflow verification failed; inspect the structured workflow results.".to_owned())
  } else {
    Ok(())
  }
}

/// Recreate the exact selection arguments for staged post-fix validation.
fn fix_scope_args(options: &FixCommand) -> Vec<String> {
  let mut args = Vec::new();
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
  args
}

/// Reject ambiguous modes, incomplete scopes, and unknown stable rule IDs.
fn validate_options(options: &FixCommand) -> Result<(), String> {
  StructuredOutputFormat::parse(&options.format, "fix")?;
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
  let value_to_zero_arg_fn = options.rule.as_deref() == Some(VALUE_TO_ZERO_ARG_FN_RULE);
  let schema_synthesis = options.rule.as_deref() == Some(SYNTHESIZE_SCHEMA_RULE);
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
  } else if value_to_zero_arg_fn || schema_synthesis {
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
  } else if options.replacement_name.is_some() {
    return Err(format!("`calcit fix --to` is only valid with `--rule {RENAME_DEFINITION_RULE}`."));
  }
  if let Some(preset) = options.preset.as_deref()
    && !matches!(preset, SURFACE_LATEST_V1_PRESET | SURFACE_LATEST_V2_PRESET)
  {
    return Err(format!(
      "Unknown fix preset `{preset}`. Available presets: `{SURFACE_LATEST_V1_PRESET}`, `{SURFACE_LATEST_V2_PRESET}`."
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
        | RENAME_DEFINITION_RULE
        | VALUE_TO_ZERO_ARG_FN_RULE
        | SYNTHESIZE_SCHEMA_RULE
        | TAG_MATCH_RULE
        | REQUIRED_STRUCT_FIELD_RULE
    )
  {
    return Err(format!(
      "Unknown fix rule `{rule}`. Available rules: `{REMOVED_DATA_API_RULE}`, `{REDUNDANT_DO_RULE}`, `{SINGLE_EXPRESSION_DO_RULE}`, `{NAMED_ENUM_CONSTRUCTOR_RULE}`, `{NAMED_STRUCT_CONSTRUCTOR_RULE}`, `{RENAME_DEFINITION_RULE}`, `{VALUE_TO_ZERO_ARG_FN_RULE}`, `{SYNTHESIZE_SCHEMA_RULE}`. The retired 0.14.x migration bridge rules are `{TAG_MATCH_RULE}` and `{REQUIRED_STRUCT_FIELD_RULE}`."
    ));
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
    return SURFACE_LATEST_V2_RULES.to_vec();
  }
  if let Some(rule) = options.rule.as_deref() {
    if matches!(rule, RENAME_DEFINITION_RULE | VALUE_TO_ZERO_ARG_FN_RULE | SYNTHESIZE_SCHEMA_RULE) {
      return vec![match rule {
        RENAME_DEFINITION_RULE => RENAME_DEFINITION_RULE,
        VALUE_TO_ZERO_ARG_FN_RULE => VALUE_TO_ZERO_ARG_FN_RULE,
        _ => SYNTHESIZE_SCHEMA_RULE,
      }];
    }
    return AVAILABLE_RULES.iter().copied().filter(|candidate| *candidate == rule).collect();
  }
  match options.preset.as_deref() {
    Some(SURFACE_LATEST_V1_PRESET) => SURFACE_LATEST_V1_RULES.to_vec(),
    Some(SURFACE_LATEST_V2_PRESET) => SURFACE_LATEST_V2_RULES.to_vec(),
    _ => AVAILABLE_RULES.to_vec(),
  }
}

/// Describe how a current normalization rule is derived without coupling it to a source release.
fn fix_rule_metadata(rule_id: &'static str) -> FixRuleMetadata {
  match rule_id {
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
    RENAME_DEFINITION_RULE => FixRuleMetadata {
      rule_id,
      diagnostic_code: RENAME_DEFINITION_DIAGNOSTIC,
      evidence_source: "compiler-resolved-reference",
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
      .map_err(|failure| format!("Failed to preprocess fix target {namespace}/{definition}: {failure}"))?;
  }
  Ok(warnings.into_inner())
}

/// Preprocess legacy source without strict rejection so fix planning can consume compiler warnings and type evidence.
/// The staged validation subprocess still uses strict mode after applying the planned operations.
fn compile_selected_definitions_for_migration(definitions: &[(String, String)]) -> Result<Vec<LocatedWarning>, String> {
  struct StrictTypesGuard(bool);

  impl Drop for StrictTypesGuard {
    fn drop(&mut self) {
      runner::preprocess::set_strict_types(self.0);
    }
  }

  let guard = StrictTypesGuard(runner::preprocess::is_strict_types_enabled());
  runner::preprocess::set_strict_types(false);
  let result = compile_selected_definitions(definitions);
  drop(guard);
  result
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

pub(crate) mod schema_synthesis;
use schema_synthesis::plan_schema_synthesis;

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
        .is_some_and(|(namespace, definition)| namespace == target_ns && definition == target_def))
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
  plan_attached_source_rewrite_with(source, owner_ns, synthetic_def, source_label, target_ns, old_name, &|source_leaf| {
    semantic_rename_leaf_replacement(source_leaf, old_name, target_ns, new_name).map(Cirru::leaf)
  })
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
    target_ns,
    target_def,
    &|source_leaf| Ok(Cirru::List(vec![Cirru::leaf(source_leaf)])),
  )
}

fn plan_attached_source_rewrite_with<F>(
  source: &Cirru,
  owner_ns: &str,
  synthetic_def: &str,
  source_label: &str,
  target_ns: &str,
  target_def: &str,
  rewrite_leaf: &F,
) -> Result<Option<AttachedSourceRewrite>, String>
where
  F: Fn(&str) -> Result<Cirru, String>,
{
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
    if !macro_origin.is_empty() {
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
    let replacement = rewrite_leaf(&source_leaf).map_err(|error| format!("{source_label}{}: {error}", format_path(path)))?;
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
  if path.is_empty() {
    return match node {
      Cirru::Leaf(value) if value.as_ref() == expected => {
        *node = replacement.clone();
        Ok(())
      }
      other => Err(format!(
        "Attached source changed while planning semantic rename: expected leaf `{expected}`, got `{other}`"
      )),
    };
  }
  let Cirru::List(items) = node else {
    return Err(format!("Attached source path {} traverses a leaf", format_path(path)));
  };
  let index = path[0];
  let child = items
    .get_mut(index)
    .ok_or_else(|| format!("Attached source path {} is out of range", format_path(path)))?;
  replace_attached_source_node(child, &path[1..], expected, replacement)
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
    let coordinate = location.coord.iter().map(|value| usize::from(*value)).collect::<Vec<_>>();
    let Some((target_path, original_leaf)) = resolve_fix_target(&entry.code, &coordinate) else {
      continue;
    };
    let Some((replacement_leaf, guidance)) = migration_for_source_leaf(&original_leaf) else {
      continue;
    };
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
    let suggestion = FixSuggestion {
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
    };
    let key = (location.ns.to_string(), location.def.to_string(), target_path);
    insert_fix_suggestion(&mut suggestions, key, suggestion)?;
  }
  Ok(suggestions.into_values().collect())
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
    if list_head(&entry.code) == Some("defmacro") {
      continue;
    }
    let mut shadowed = HashSet::new();
    collect_potential_local_bindings(&entry.code, &mut shadowed);
    let mut paths = Vec::new();
    collect_named_constructor_paths(&entry.code, &mut Vec::new(), &mut paths, snapshot, namespace, &shadowed, kinds);
    paths.sort();
    for target_path in paths {
      let original_node = navigate_to_path(&entry.code, &target_path)?;
      let Cirru::List(items) = &original_node else {
        continue;
      };
      let Some(kind) = legacy_constructor_kind(items, snapshot, namespace, &shadowed, kinds) else {
        continue;
      };
      let replacement_node = rewrite_named_constructor_tree(
        &original_node,
        snapshot,
        namespace,
        &shadowed,
        kinds,
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
      suggestions.push(FixSuggestion {
        rule_id: kind.rule_id(),
        diagnostic_code: kind.diagnostic_code(),
        semantic_layer: "surface",
        source_file: snapshot_file.to_owned(),
        definition: format!("{namespace}/{definition}"),
        path: format!("code{}", format_path(&target_path)),
        fingerprint: node_fingerprint(&original_node),
        origin_chain: vec![],
        original: quoted_json(&original_node),
        replacement: Some(quoted_json(&replacement_node)),
        applicability: "machine-applicable",
        message: format!(
          "Replace legacy `{}` {} construction with the directly callable `{prototype}` constructor.",
          kind.legacy_head(),
          kind.display_name()
        ),
        target_path,
        operation: Some(FixOperation::ReplaceNode {
          original: original_code,
          replacement: replacement_code,
        }),
      });
    }
  }
  Ok(suggestions)
}

/// Collect legacy constructor calls while preserving quoted data and rejecting unresolved or shadowed prototypes.
fn collect_named_constructor_paths(
  node: &Cirru,
  path: &mut Vec<usize>,
  output: &mut Vec<Vec<usize>>,
  snapshot: &Snapshot,
  namespace: &str,
  shadowed: &HashSet<String>,
  kinds: &[NominalKind],
) {
  let Cirru::List(items) = node else {
    return;
  };
  let head = items.first().and_then(leaf_value);
  if matches!(head, Some("quote" | "quasiquote" | "defmacro")) {
    return;
  }
  if legacy_constructor_kind(items, snapshot, namespace, shadowed, kinds).is_some() {
    output.push(path.clone());
    return;
  }
  for (index, child) in items.iter().enumerate() {
    path.push(index);
    collect_named_constructor_paths(child, path, output, snapshot, namespace, shadowed, kinds);
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
fn rewrite_named_constructor_tree(
  node: &Cirru,
  snapshot: &Snapshot,
  namespace: &str,
  shadowed: &HashSet<String>,
  kinds: &[NominalKind],
  compose_redundant_do: bool,
  compose_single_expression_do: bool,
) -> Cirru {
  let Cirru::List(items) = node else {
    return node.clone();
  };
  if matches!(items.first().and_then(leaf_value), Some("quote" | "quasiquote" | "defmacro")) {
    return node.clone();
  }
  let rewritten_items = items
    .iter()
    .map(|item| {
      rewrite_named_constructor_tree(
        item,
        snapshot,
        namespace,
        shadowed,
        kinds,
        compose_redundant_do,
        compose_single_expression_do,
      )
    })
    .collect::<Vec<_>>();
  let rewritten_node = if let Some(kind) = legacy_constructor_kind(items, snapshot, namespace, shadowed, kinds) {
    legacy_constructor_replacement(&rewritten_items, kind).unwrap_or(Cirru::List(rewritten_items))
  } else {
    Cirru::List(rewritten_items)
  };
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
    Some("let" | "loop" | "doseq" | "if-let" | "when-let") => collect_binding_positions(items.get(1), output),
    _ => {}
  }
  for child in items {
    collect_potential_local_bindings(child, output);
  }
}

/// Collect binding names from the alternating name/value positions of one binding vector.
fn collect_binding_positions(node: Option<&Cirru>, output: &mut HashSet<String>) {
  let Some(Cirru::List(items)) = node else {
    return;
  };
  for binding in items.iter().step_by(2) {
    collect_binding_tree(Some(binding), output);
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
  println!("# Compiler-guided source fixes\n");
  println!("- mode: `{}`", report.data.mode);
  println!("- revision: `{}`", report.revision);
  if let Some(preset) = report.data.filters.preset_id {
    println!("- preset: `{preset}`");
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
}

#[cfg(test)]
mod tests {
  use super::{
    FixOperation, FixSuggestion, NominalKind, REMOVED_DATA_API_RULE, collect_potential_local_bindings, collect_redundant_do_paths,
    fix_rule_metadata, fix_source_json_to_cirru, insert_fix_suggestion, legacy_constructor_replacement, migration_for_source_leaf,
    prototype_is_shadowed, resolve_fix_target, rewrite_loaded_schema_type_references, rewrite_named_constructor_tree,
    struct_fields_are_complete, suggestion_operations,
  };
  use calcit::calcit::{CalcitFnTypeAnnotation, CalcitGenericBound, CalcitTrait, CalcitTypeAnnotation, SchemaKind};
  use cirru_parser::Cirru;
  use serde_json::Value;
  use std::collections::BTreeMap;
  use std::sync::Arc;

  use super::super::common::markdown_cirru_section;

  fn leaf(value: &str) -> Cirru {
    Cirru::leaf(value)
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

    let normalized_legacy = rewrite_named_constructor_tree(&legacy, &snapshot, "fix-command.main", &shadowed, &kinds, false, false);
    let normalized_current = rewrite_named_constructor_tree(&current, &snapshot, "fix-command.main", &shadowed, &kinds, false, false);

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
