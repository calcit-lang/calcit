use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;
use std::process::Command;

use calcit::calcit::LocatedWarning;
use calcit::call_stack::CallStackList;
use calcit::cli_args::{DeprecatedCommand, FixCommand};
use calcit::runner;
use calcit::snapshot::Snapshot;
use cirru_parser::Cirru;
use md5::{Digest, Md5};
use serde::Serialize;
use serde_json::Value;

use super::common::{cirru_to_json_value, format_path};
use super::edit::{load_snapshot, navigate_to_path, run_staged_fix_transaction, snapshot_content_revision};
use crate::deprecated_api;

const REMOVED_DATA_API_RULE: &str = "removed-data-api-v1";
const REMOVED_DATA_API_DIAGNOSTIC: &str = "W_REMOVED_DATA_API";
const REDUNDANT_DO_RULE: &str = "redundant-do-v1";
const REDUNDANT_DO_DIAGNOSTIC: &str = "FIX_REDUNDANT_DO";
const TAG_MATCH_RULE: &str = "tag-match-to-match-v1";
const TAG_MATCH_DIAGNOSTIC: &str = "W_DEPRECATED_API";

#[derive(Debug, Clone, PartialEq, Eq)]
enum FixOperation {
  ReplaceLeaf { original: String, replacement: String },
  SpliceDo,
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
  rule_id: &'a str,
}

/// Plan deterministic source migrations, validate them on a staged Snapshot, and optionally commit them atomically.
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
  let mut suggestions = Vec::new();
  if options.rule.as_deref().is_none_or(|rule| rule == REMOVED_DATA_API_RULE) {
    suggestions.extend(plan_removed_data_api_fixes(options, &source_snapshot, snapshot_file, &warnings)?);
  }
  if options.rule.as_deref().is_none_or(|rule| rule == REDUNDANT_DO_RULE) {
    suggestions.extend(plan_redundant_do_fixes(&source_snapshot, snapshot_file, &selected_definitions)?);
  }
  if options.rule.as_deref().is_none_or(|rule| rule == TAG_MATCH_RULE) {
    suggestions.extend(plan_tag_match_fixes(
      options,
      compiled_snapshot,
      &source_snapshot,
      snapshot_file,
      &selected_definitions,
    )?);
  }
  let operations = suggestions.iter().flat_map(suggestion_operations).collect::<Vec<_>>();

  if std::env::var("CALCIT_FIX_VALIDATE_ONLY").as_deref() == Ok("1") {
    if suggestions.iter().any(|suggestion| suggestion.operation.is_some()) {
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
        rule_id: options.rule.as_deref().unwrap_or("all"),
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
  args
}

/// Reject ambiguous modes, incomplete scopes, and unknown stable rule IDs.
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
    && !matches!(rule, REMOVED_DATA_API_RULE | REDUNDANT_DO_RULE | TAG_MATCH_RULE)
  {
    return Err(format!(
      "Unknown fix rule `{rule}`. Available rules: `{REMOVED_DATA_API_RULE}`, `{REDUNDANT_DO_RULE}`, `{TAG_MATCH_RULE}`."
    ));
  }
  Ok(())
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
        definition: format!("{namespace}/{definition}"),
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

/// Rewrite source call heads proven to resolve to the deprecated core `tag-match` macro.
fn plan_tag_match_fixes(
  options: &FixCommand,
  compiled_snapshot: &Snapshot,
  source_snapshot: &Snapshot,
  snapshot_file: &str,
  selected_definitions: &[(String, String)],
) -> Result<Vec<FixSuggestion>, String> {
  let deprecated_options = DeprecatedCommand {
    ns: options.ns.clone(),
    ns_prefix: None,
    format: "json".to_owned(),
    deps: false,
    summary_only: false,
  };
  let selected = selected_definitions.iter().cloned().collect::<HashSet<_>>();
  let rows = deprecated_api::collect_deprecated_api_rows(&deprecated_options, compiled_snapshot)?;
  let mut suggestions = BTreeMap::new();

  for row in rows {
    if !selected.contains(&(row.namespace.clone(), row.definition.clone())) {
      continue;
    }
    let Some(entry) = source_snapshot
      .files
      .get(&row.namespace)
      .and_then(|file| file.defs.get(&row.definition))
    else {
      continue;
    };
    for usage in row.uses {
      if usage.target_namespace != calcit::calcit::CORE_NS || usage.target_name != "tag-match" {
        continue;
      }
      let Some(call_path) = parse_code_path(&usage.path) else {
        continue;
      };
      if !source_path_is_runtime_expression(&entry.code, &call_path) {
        continue;
      }
      let Ok(Cirru::List(call)) = navigate_to_path(&entry.code, &call_path) else {
        continue;
      };
      if call.len() < 3 {
        continue;
      }
      let Some(Cirru::Leaf(original)) = call.first() else {
        continue;
      };
      if source_call_is_lexically_shadowed(&entry.code, &call_path, original.as_ref()) {
        continue;
      }
      let mut target_path = call_path;
      target_path.push(0);
      let original_leaf = original.to_string();
      let original_node = Cirru::leaf(original_leaf.as_str());
      let replacement_node = Cirru::leaf("match");
      let suggestion = FixSuggestion {
        rule_id: TAG_MATCH_RULE,
        diagnostic_code: TAG_MATCH_DIAGNOSTIC,
        semantic_layer: "surface",
        source_file: snapshot_file.to_owned(),
        definition: format!("{}/{}", row.namespace, row.definition),
        path: format!("code{}", format_path(&target_path)),
        fingerprint: node_fingerprint(&original_node),
        origin_chain: vec![serde_json::json!({
          "kind": "resolved-definition",
          "target": "calcit.core/tag-match",
        })],
        original: quoted_json(&original_node),
        replacement: Some(quoted_json(&replacement_node)),
        applicability: "machine-applicable",
        message: "Replace the call resolved to deprecated `calcit.core/tag-match` with native `match`; the branch AST and evaluation order are unchanged. Run affected Calcit tests and a generated-JS build after applying."
          .to_owned(),
        target_path: target_path.clone(),
        operation: Some(FixOperation::ReplaceLeaf {
          original: original_leaf,
          replacement: "match".to_owned(),
        }),
      };
      insert_fix_suggestion(
        &mut suggestions,
        (row.namespace.clone(), row.definition.clone(), target_path),
        suggestion,
      )?;
    }
  }

  Ok(suggestions.into_values().collect())
}

/// Parse the stable `code@0.1` path emitted by deprecated API analysis.
fn parse_code_path(path: &str) -> Option<Vec<usize>> {
  if path == "code" {
    return Some(vec![]);
  }
  path
    .strip_prefix("code@")?
    .split('.')
    .map(|segment| segment.parse::<usize>().ok())
    .collect()
}

/// Keep fixes out of parameter patterns and quoted syntax while allowing evaluated binding values.
fn source_path_is_runtime_expression(root: &Cirru, path: &[usize]) -> bool {
  let mut node = root;
  for (depth, child_index) in path.iter().copied().enumerate() {
    let Cirru::List(items) = node else {
      return false;
    };
    let head = items.first().and_then(|item| match item {
      Cirru::Leaf(value) => Some(value.as_ref()),
      Cirru::List(_) => None,
    });
    match head {
      Some("quote" | "quasiquote") if child_index >= 1 => return false,
      Some("defn" | "defmacro") if child_index <= 2 => return false,
      Some("fn") if child_index <= 1 => return false,
      Some("let" | "loop") if child_index == 1 => {
        let remaining = &path[depth + 1..];
        if remaining.len() < 2 || remaining[1] == 0 {
          return false;
        }
      }
      Some("&let") if child_index == 1 => {
        let remaining = &path[depth + 1..];
        if remaining.first() != Some(&1) {
          return false;
        }
      }
      _ => {}
    }
    let Some(child) = items.get(child_index) else {
      return false;
    };
    node = child;
  }
  true
}

/// Reject analyzer hits whose unqualified source head is bound by an enclosing lexical form.
fn source_call_is_lexically_shadowed(root: &Cirru, call_path: &[usize], symbol: &str) -> bool {
  if symbol.contains('/') {
    return false;
  }
  let mut node = root;
  for (depth, child_index) in call_path.iter().copied().enumerate() {
    let Cirru::List(items) = node else {
      return false;
    };
    let remaining = &call_path[depth + 1..];
    if lexical_form_binds(items, child_index, remaining, symbol) {
      return true;
    }
    let Some(child) = items.get(child_index) else {
      return false;
    };
    node = child;
  }
  false
}

fn lexical_form_binds(items: &[Cirru], child_index: usize, remaining: &[usize], symbol: &str) -> bool {
  let Some(Cirru::Leaf(head)) = items.first() else {
    return false;
  };
  match head.as_ref() {
    "defn" | "defmacro" if child_index >= 3 => items.get(2).is_some_and(|params| pattern_binds(params, symbol)),
    "fn" if child_index >= 2 => items.get(1).is_some_and(|params| pattern_binds(params, symbol)),
    "let" | "loop" => {
      let Some(Cirru::List(bindings)) = items.get(1) else {
        return false;
      };
      if child_index >= 2 {
        return bindings.iter().any(|binding| binding_name_matches(binding, symbol));
      }
      if child_index != 1 || remaining.len() < 2 || remaining[1] == 0 {
        return false;
      }
      bindings
        .iter()
        .take(remaining[0])
        .any(|binding| binding_name_matches(binding, symbol))
    }
    "&let" if child_index >= 2 => items.get(1).is_some_and(|binding| binding_name_matches(binding, symbol)),
    _ => false,
  }
}

fn binding_name_matches(binding: &Cirru, symbol: &str) -> bool {
  let Cirru::List(pair) = binding else {
    return false;
  };
  pair.first().is_some_and(|pattern| pattern_binds(pattern, symbol))
}

fn pattern_binds(pattern: &Cirru, symbol: &str) -> bool {
  match pattern {
    Cirru::Leaf(value) => value.as_ref() == symbol,
    Cirru::List(items) => items.iter().any(|item| pattern_binds(item, symbol)),
  }
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
  use super::{
    FixOperation, FixSuggestion, collect_redundant_do_paths, insert_fix_suggestion, migration_for_source_leaf, resolve_fix_target,
    source_call_is_lexically_shadowed, source_path_is_runtime_expression, suggestion_operations,
  };
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
  fn source_guards_reject_parameters_quoted_data_and_lexical_shadows() {
    let parameter_shadow = Cirru::List(vec![
      leaf("defn"),
      leaf("demo"),
      Cirru::List(vec![leaf("tag-match"), leaf("value")]),
      Cirru::List(vec![leaf("tag-match"), leaf("value"), leaf("branch")]),
    ]);
    assert!(!source_path_is_runtime_expression(&parameter_shadow, &[2]));
    assert!(source_path_is_runtime_expression(&parameter_shadow, &[3]));
    assert!(source_call_is_lexically_shadowed(&parameter_shadow, &[3], "tag-match"));

    let quoted = Cirru::List(vec![
      leaf("defn"),
      leaf("demo"),
      Cirru::List(vec![]),
      Cirru::List(vec![
        leaf("quote"),
        Cirru::List(vec![leaf("tag-match"), leaf("value"), leaf("branch")]),
      ]),
    ]);
    assert!(!source_path_is_runtime_expression(&quoted, &[3, 1]));

    let let_shadow = Cirru::List(vec![
      leaf("let"),
      Cirru::List(vec![Cirru::List(vec![leaf("tag-match"), leaf("callback")])]),
      Cirru::List(vec![leaf("tag-match"), leaf("value"), leaf("branch")]),
    ]);
    assert!(source_call_is_lexically_shadowed(&let_shadow, &[2], "tag-match"));

    let internal_let_shadow = Cirru::List(vec![
      leaf("&let"),
      Cirru::List(vec![leaf("tag-match"), leaf("callback")]),
      Cirru::List(vec![leaf("tag-match"), leaf("value"), leaf("branch")]),
    ]);
    assert!(!source_path_is_runtime_expression(&internal_let_shadow, &[1, 0]));
    assert!(source_path_is_runtime_expression(&internal_let_shadow, &[1, 1]));
    assert!(source_call_is_lexically_shadowed(&internal_let_shadow, &[2], "tag-match"));
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
