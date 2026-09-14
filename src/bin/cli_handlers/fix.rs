use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;
use std::process::Command;

use calcit::calcit::LocatedWarning;
use calcit::call_stack::CallStackList;
use calcit::cli_args::FixCommand;
use calcit::snapshot::Snapshot;
use calcit::{program, runner};
use cirru_parser::Cirru;
use md5::{Digest, Md5};
use serde::Serialize;
use serde_json::Value;

use super::common::{cirru_to_json_value, format_path, json_value_to_cirru, markdown_cirru_section};
use super::edit::{load_snapshot, navigate_to_path, run_staged_fix_transaction, snapshot_content_revision};
use super::structured_output::{StructuredOutputFormat, format_json_value_as_edn};

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
  ReplaceNode { original: String, replacement: String },
  SpliceDo,
  ReplaceImports { code: String },
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
  let selected_definitions = if semantic_rename {
    select_project_definitions(compiled_snapshot, project_namespaces)?
  } else {
    select_definitions(options, compiled_snapshot, project_namespaces)?
  };
  let validation_only = std::env::var("CALCIT_FIX_VALIDATE_ONLY").as_deref() == Ok("1");
  let warnings = if validation_only {
    compile_selected_definitions(&selected_definitions)?
  } else {
    compile_selected_definitions_for_migration(&selected_definitions)?
  };
  let mut suggestions = Vec::new();
  if semantic_rename {
    suggestions.extend(plan_definition_rename(
      options,
      &source_snapshot,
      snapshot_file,
      &selected_definitions,
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
    let unexpected = if semantic_rename {
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
  let transaction = run_staged_fix_transaction(Path::new(snapshot_file), &operations, Some(&revision), dry_run, &validation_args)?;

  let mode = if options.apply { "apply" } else { "preview" };
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
        rule_id: options.rule.as_deref().unwrap_or("all"),
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
  Ok(())
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
  if let Some(replacement) = &options.replacement_name {
    args.push("--to".to_owned());
    args.push(replacement.clone());
  }
  args
}

/// Reject ambiguous modes, incomplete scopes, and unknown stable rule IDs.
fn validate_options(options: &FixCommand) -> Result<(), String> {
  StructuredOutputFormat::parse(&options.format, "fix")?;
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
        | TAG_MATCH_RULE
        | REQUIRED_STRUCT_FIELD_RULE
    )
  {
    return Err(format!(
      "Unknown fix rule `{rule}`. Available rules: `{REMOVED_DATA_API_RULE}`, `{REDUNDANT_DO_RULE}`, `{SINGLE_EXPRESSION_DO_RULE}`, `{NAMED_ENUM_CONSTRUCTOR_RULE}`, `{NAMED_STRUCT_CONSTRUCTOR_RULE}`, `{RENAME_DEFINITION_RULE}`. The retired 0.14.x migration bridge rules are `{TAG_MATCH_RULE}` and `{REQUIRED_STRUCT_FIELD_RULE}`."
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
  if let Some(rule) = options.rule.as_deref() {
    if rule == RENAME_DEFINITION_RULE {
      return vec![RENAME_DEFINITION_RULE];
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
      .map_err(|failure| failure.msg)?;
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
      return Ok(vec![]);
    }
    return Err(format!("Semantic rename source `{target_ns}/{old_name}` does not exist."));
  }
  if target_file.defs.contains_key(new_name) {
    return Err(format!("Semantic rename destination `{target_ns}/{new_name}` already exists."));
  }

  let warnings = RefCell::new(Vec::new());
  let mut resolved = BTreeMap::<(String, String, Vec<usize>), (String, Vec<String>)>::new();
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
    for test in &entry.tests {
      if cirru_contains_target_reference(&test.code, owner_ns, target_ns, old_name) {
        blockers.push(format!(
          "{owner_ns}/{owner_def}#{}: definition-attached test references the target; test-source refactoring is not yet supported",
          test.name
        ));
      }
    }
    for (index, example) in entry.examples.iter().enumerate() {
      if cirru_contains_target_reference(example, owner_ns, target_ns, old_name) {
        blockers.push(format!(
          "{owner_ns}/{owner_def} example {index}: example source references the target; example refactoring is not yet supported"
        ));
      }
    }
    let schema_edn = calcit::snapshot::schema_annotation_to_edn(entry.schema.as_ref());
    if edn_contains_target_type_reference(&schema_edn, owner_ns, target_ns, old_name) {
      blockers.push(format!(
        "{owner_ns}/{owner_def} schema: type reference targets `{target_ns}/{old_name}` and schema refactoring is not yet supported"
      ));
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

fn edn_contains_target_type_reference(value: &cirru_edn::Edn, owner_ns: &str, target_ns: &str, target_def: &str) -> bool {
  use cirru_edn::Edn;
  match value {
    Edn::Symbol(name) => source_name_resolves_to_target(owner_ns, name, target_ns, target_def),
    Edn::List(items) => items
      .0
      .iter()
      .any(|item| edn_contains_target_type_reference(item, owner_ns, target_ns, target_def)),
    Edn::Map(items) => items.0.iter().any(|(key, item)| {
      edn_contains_target_type_reference(key, owner_ns, target_ns, target_def)
        || edn_contains_target_type_reference(item, owner_ns, target_ns, target_def)
    }),
    Edn::Set(items) => items
      .0
      .iter()
      .any(|item| edn_contains_target_type_reference(item, owner_ns, target_ns, target_def)),
    Edn::Struct(data) => data
      .pairs
      .iter()
      .any(|(_, item)| edn_contains_target_type_reference(item, owner_ns, target_ns, target_def)),
    Edn::Enum(data) => data
      .extra
      .iter()
      .any(|item| edn_contains_target_type_reference(item, owner_ns, target_ns, target_def)),
    Edn::Atom(item) => edn_contains_target_type_reference(item, owner_ns, target_ns, target_def),
    _ => false,
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
    prototype_is_shadowed, resolve_fix_target, rewrite_named_constructor_tree, struct_fields_are_complete, suggestion_operations,
  };
  use cirru_parser::Cirru;
  use serde_json::Value;
  use std::collections::BTreeMap;

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
    ]);
    let mut paths = Vec::new();
    collect_redundant_do_paths(&code, &mut Vec::new(), &mut paths);
    assert_eq!(paths, vec![vec![3], vec![5, 2]]);
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
}
