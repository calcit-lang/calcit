use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;
use std::process::Command;

use calcit::calcit::LocatedWarning;
use calcit::call_stack::CallStackList;
use calcit::cli_args::FixCommand;
use calcit::runner;
use calcit::snapshot::Snapshot;
use cirru_parser::Cirru;
use md5::{Digest, Md5};
use serde::Serialize;
use serde_json::Value;

use super::common::{cirru_to_json_value, format_path};
use super::edit::{load_snapshot, navigate_to_path, run_staged_fix_transaction, snapshot_content_revision};

const REMOVED_DATA_API_RULE: &str = "removed-data-api-v1";
const REMOVED_DATA_API_DIAGNOSTIC: &str = "W_REMOVED_DATA_API";

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
  original_leaf: String,
  #[serde(skip)]
  replacement_leaf: Option<String>,
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
  rule_id: &'static str,
}

pub(crate) fn handle_fix_command(
  options: &FixCommand,
  compiled_snapshot: &Snapshot,
  project_namespaces: &HashSet<String>,
  snapshot_file: &str,
) -> Result<(), String> {
  validate_options(options)?;
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

  let selected_definitions = select_definitions(options, compiled_snapshot, project_namespaces)?;
  let warnings = compile_selected_definitions(&selected_definitions)?;
  let suggestions = plan_removed_data_api_fixes(options, &source_snapshot, snapshot_file, &warnings)?;
  let operations = suggestions.iter().filter_map(suggestion_operation).collect::<Vec<_>>();

  if std::env::var("CALCIT_FIX_VALIDATE_ONLY").as_deref() == Ok("1") {
    if suggestions.iter().any(|suggestion| suggestion.replacement_leaf.is_some()) {
      return Err("Staged fix validation found an applicable migration that was not removed.".to_owned());
    }
    let unexpected = warnings
      .iter()
      .filter(|warning| warning.code() != Some(REMOVED_DATA_API_DIAGNOSTIC))
      .map(ToString::to_string)
      .collect::<Vec<_>>();
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
  let report = FixReport {
    schema_version: 1,
    command: "fix",
    revision: &transaction.original_revision,
    data: FixReportData {
      mode,
      filters: FixFilters {
        namespace: options.ns.as_deref(),
        definition: options.definition.as_deref(),
        rule_id: REMOVED_DATA_API_RULE,
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

  match options.format.as_str() {
    "json" => println!(
      "{}",
      serde_json::to_string(&report).map_err(|error| format!("Failed to serialize fix result: {error}"))?
    ),
    "human" | "text" => print_human_report(&report),
    _ => unreachable!("fix output format was validated"),
  }
  Ok(())
}

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
  args
}

fn validate_options(options: &FixCommand) -> Result<(), String> {
  if !matches!(options.format.as_str(), "human" | "text" | "json") {
    return Err(format!(
      "Unknown fix output format `{}`. Expected `human` or `json`.",
      options.format
    ));
  }
  if options.apply && options.dry_run {
    return Err("`calcit fix --apply` conflicts with `--dry-run`; omit both flags to preview.".to_owned());
  }
  if options.definition.is_some() && options.ns.is_none() {
    return Err("`calcit fix --def` requires an exact `--ns` scope.".to_owned());
  }
  if let Some(rule) = options.rule.as_deref()
    && rule != REMOVED_DATA_API_RULE
  {
    return Err(format!("Unknown fix rule `{rule}`. Available rule: `{REMOVED_DATA_API_RULE}`."));
  }
  Ok(())
}

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

fn compile_selected_definitions(definitions: &[(String, String)]) -> Result<Vec<LocatedWarning>, String> {
  let warnings = RefCell::new(Vec::new());
  for (namespace, definition) in definitions {
    runner::preprocess::ensure_ns_def_compiled(namespace, definition, &warnings, &CallStackList::default())
      .map_err(|failure| failure.msg)?;
  }
  Ok(warnings.into_inner())
}

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
      original_leaf,
      replacement_leaf,
    };
    let key = (location.ns.to_string(), location.def.to_string(), target_path);
    insert_fix_suggestion(&mut suggestions, key, suggestion)?;
  }
  Ok(suggestions.into_values().collect())
}

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

fn suggestion_operation(suggestion: &FixSuggestion) -> Option<Vec<String>> {
  let replacement = suggestion.replacement_leaf.as_deref()?;
  Some(vec![
    "tree".to_owned(),
    "replace".to_owned(),
    suggestion.definition.clone(),
    "--path".to_owned(),
    format_path(&suggestion.target_path),
    "--expect".to_owned(),
    format!("quote {}", suggestion.original_leaf),
    "--code".to_owned(),
    format!("quote {replacement}"),
  ])
}

fn quoted_json(node: &Cirru) -> Value {
  serde_json::json!({
    "$type": "quote",
    "value": cirru_to_json_value(node),
  })
}

fn node_fingerprint(node: &Cirru) -> String {
  let mut hasher = Md5::new();
  hasher.update(cirru_to_json_value(node).to_string().as_bytes());
  format!("md5:{}", hex::encode(hasher.finalize()))
}

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

fn print_human_report(report: &FixReport<'_>) {
  println!("Compiler-guided source fixes");
  println!("- mode: {}", report.data.mode);
  println!("- revision: {}", report.revision);
  println!("- suggestions: {}", report.data.suggestions.len());
  println!("- changed: {}", report.data.changed);
  for suggestion in report.data.suggestions {
    println!(
      "- [{}] {} {}: {}",
      suggestion.applicability, suggestion.definition, suggestion.path, suggestion.message
    );
  }
  if report.data.mode == "preview" && report.data.changed {
    println!("Run `calcit fix --apply` after reviewing this plan.");
  }
}

#[cfg(test)]
mod tests {
  use super::{FixSuggestion, insert_fix_suggestion, migration_for_source_leaf, resolve_fix_target, suggestion_operation};
  use cirru_parser::Cirru;
  use serde_json::Value;
  use std::collections::BTreeMap;

  fn leaf(value: &str) -> Cirru {
    Cirru::leaf(value)
  }

  #[test]
  fn preserves_source_qualifier_for_exact_removed_api_migration() {
    assert_eq!(
      migration_for_source_leaf("core/tuple-enum"),
      Some((Some("core/enum-definition".to_owned()), "core/enum-definition".to_owned()))
    );
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
      original_leaf: "tuple-enum".to_owned(),
      replacement_leaf: Some("enum-definition".to_owned()),
    };
    let operation = suggestion_operation(&suggestion).expect("applicable suggestion should produce an operation");
    assert_eq!(
      operation,
      vec![
        "tree",
        "replace",
        "app.main/main!",
        "--path",
        "@3.0",
        "--expect",
        "quote tuple-enum",
        "--code",
        "quote enum-definition",
      ]
    );
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
      original_leaf: "tuple-enum".to_owned(),
      replacement_leaf: Some("enum-definition".to_owned()),
    };
    insert_fix_suggestion(&mut suggestions, key.clone(), first.clone()).expect("first suggestion should insert");
    insert_fix_suggestion(&mut suggestions, key.clone(), first.clone()).expect("identical duplicate should be idempotent");

    let mut conflicting = first;
    conflicting.replacement_leaf = Some("enum?".to_owned());
    let error = insert_fix_suggestion(&mut suggestions, key, conflicting).expect_err("conflicting overlap should fail");

    assert!(error.contains("overlap at app.main/main! @3.0"), "error: {error}");
    assert_eq!(suggestions.len(), 1);
  }
}
