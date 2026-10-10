//! Whole-namespace data view: `query ns <ns> --format cirru` and `edit ns <ns>`.
//!
//! The view is the namespace's `%{} 'FileEntry` value exactly as the Snapshot
//! stores it under `:files` (its `:ns` entry and the `:defs` map of
//! `%{} 'CodeEntry` values), rendered with the Snapshot serializer. `edit ns`
//! reads the same data back through the Snapshot loader and writes it per
//! definition; every field in the data is authoritative.

use calcit::cli_args::EditNsCommand;
use calcit::program::validate_import_rules;
use calcit::snapshot::{self, CodeEntry, FileInSnapShot, Snapshot};
use calcit::util::string::strip_shebang;
use cirru_edn::Edn;
use cirru_parser::Cirru;
use colored::Colorize;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use super::common::read_code_input;
use super::edit::{
  TransactionOperationReport, check_ns_editable, extract_require_rules, find_edn_map_value_mut, load_snapshot,
  run_staged_transaction_with_options, save_snapshot, snapshot_content_revision, validate_definition_shape,
};
use super::structured_output::{StructuredOutputFormat, format_json_value_as_edn};

/// Render one namespace as the `FileEntry` data stored in the Snapshot.
pub(crate) fn render_ns_view(file: &FileInSnapShot) -> Result<String, String> {
  let text = snapshot::render_file_entry_content(file)?;
  let text = text.trim_start_matches('\n');
  Ok(if text.ends_with('\n') {
    text.to_owned()
  } else {
    format!("{text}\n")
  })
}

const FILE_ENTRY_KEYS: &[&str] = &["defs", "ns"];
const NS_ENTRY_KEYS: &[&str] = &["code", "doc"];
const CODE_ENTRY_KEYS: &[&str] = &["code", "doc", "examples", "ffi", "schema", "tags", "tests"];

fn truncate_message(text: &str, limit: usize) -> String {
  match text.char_indices().nth(limit) {
    Some((index, _)) => format!("{}… ({} more bytes)", &text[..index], text.len() - index),
    None => text.to_owned(),
  }
}

/// Key text of a record or map entry, without its `:`, `'` or `|` prefix.
fn entry_key(node: &Cirru) -> Option<&str> {
  match node {
    Cirru::Leaf(text) => Some(text.strip_prefix([':', '\'', '|']).unwrap_or(text)),
    Cirru::List(_) => None,
  }
}

/// `(key value)` pairs of a `%{} 'Name ...` record or `{} ...` map node; `None` for other shapes,
/// which the Snapshot loader reports itself.
fn entry_pairs(node: &Cirru) -> Option<Vec<(&str, &Cirru)>> {
  let Cirru::List(items) = node else { return None };
  let pairs = match items.first() {
    Some(head) if head.eq_leaf("%{}") => items.get(2..)?,
    Some(head) if head.eq_leaf("{}") => &items[1..],
    _ => return None,
  };
  pairs
    .iter()
    .map(|pair| match pair {
      Cirru::List(kv) if kv.len() == 2 => entry_key(&kv[0]).map(|key| (key, &kv[1])),
      _ => None,
    })
    .collect()
}

/// Reject duplicate and unknown keys before the loader collapses duplicates or ignores unknown fields,
/// which would silently drop data such as tests renamed to `:test`.
fn checked_fields<'a>(node: &'a Cirru, allowed: &[&str], owner: &str) -> Result<Vec<(&'a str, &'a Cirru)>, String> {
  let Some(pairs) = entry_pairs(node) else { return Ok(vec![]) };
  let mut seen = BTreeSet::new();
  for (key, _) in &pairs {
    if !allowed.contains(key) {
      return Err(format!(
        "Unknown key `:{key}` in {owner}; expected one of {}",
        allowed.iter().map(|key| format!(":{key}")).collect::<Vec<_>>().join(", ")
      ));
    }
    if !seen.insert(*key) {
      return Err(format!("Duplicate key `:{key}` in {owner}"));
    }
  }
  Ok(pairs)
}

fn check_record_keys(raw: &str, namespace: &str) -> Result<(), String> {
  // Unparsable input is reported by the EDN parser.
  let Ok(nodes) = cirru_parser::parse(raw) else { return Ok(()) };
  let [file] = nodes.as_slice() else { return Ok(()) };
  for (key, value) in checked_fields(file, FILE_ENTRY_KEYS, &format!("the FileEntry of '{namespace}'"))? {
    if key == "ns" {
      checked_fields(value, NS_ENTRY_KEYS, &format!("the NsEntry of '{namespace}'"))?;
      continue;
    }
    let Some(definitions) = entry_pairs(value) else { continue };
    let mut seen = BTreeSet::new();
    for (name, entry) in definitions {
      if !seen.insert(name) {
        return Err(format!("Definition '{namespace}/{name}' appears more than once in :defs"));
      }
      checked_fields(entry, CODE_ENTRY_KEYS, &format!("definition '{namespace}/{name}'"))?;
    }
  }
  Ok(())
}

/// Load the Snapshot with one namespace's `:files` entry replaced by `file_data`,
/// so the incoming data passes through the same loader as the Snapshot file.
fn load_with_file_entry(snapshot_file: &str, namespace: &str, file_data: Edn) -> Result<Snapshot, String> {
  let mut content = fs::read_to_string(snapshot_file).map_err(|error| format!("Failed to read {snapshot_file}: {error}"))?;
  strip_shebang(&mut content);
  let mut root = cirru_edn::parse(&content).map_err(|error| format!("Failed to parse EDN: {error}"))?;
  let Edn::Map(root_map) = &mut root else {
    return Err("Snapshot root must be an EDN map".to_owned());
  };
  let Some(Edn::Map(files)) = find_edn_map_value_mut(root_map, "files") else {
    return Err("Snapshot :files must be an EDN map".to_owned());
  };
  let slot = find_edn_map_value_mut(files, namespace).ok_or_else(|| {
    format!("Namespace '{namespace}' is not stored in this Snapshot file (generated `.$meta` namespaces are read-only)")
  })?;
  *slot = file_data;
  snapshot::load_snapshot_data(&root, snapshot_file).map_err(|error| format!("Invalid namespace data for '{namespace}': {error}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum DefinitionStatus {
  Added,
  Changed,
  Unchanged,
  Removed,
}

impl DefinitionStatus {
  fn label(self) -> &'static str {
    match self {
      Self::Added => "added",
      Self::Changed => "changed",
      Self::Unchanged => "unchanged",
      Self::Removed => "removed",
    }
  }
}

#[derive(Debug, Clone, Serialize)]
struct DefinitionChange {
  name: String,
  status: DefinitionStatus,
  /// Which `CodeEntry` fields differ for a changed definition.
  fields: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
struct NsViewReport {
  schema_version: u8,
  command: &'static str,
  namespace: String,
  dry_run: bool,
  changed: bool,
  original_revision: String,
  new_revision: String,
  /// Pre-edit fingerprints of only the units this write changes; pass it to `--expect-revision`.
  scoped_revision: Option<String>,
  ns_changed: bool,
  definitions: Vec<DefinitionChange>,
}

struct NsViewPlan {
  ns: Option<snapshot::NsEntry>,
  /// New entries for added or changed definitions.
  updates: BTreeMap<String, CodeEntry>,
  removals: Vec<String>,
  changes: Vec<DefinitionChange>,
}

impl NsViewPlan {
  fn is_empty(&self) -> bool {
    self.ns.is_none() && self.updates.is_empty() && self.removals.is_empty()
  }

  fn apply(&self, file: &mut FileInSnapShot) {
    if let Some(ns) = &self.ns {
      file.ns = ns.clone();
    }
    for name in &self.removals {
      file.defs.remove(name);
    }
    for (name, entry) in &self.updates {
      file.defs.insert(name.clone(), entry.clone());
    }
  }
}

fn changed_fields(before: &CodeEntry, after: &CodeEntry) -> Vec<&'static str> {
  let mut fields = vec![];
  if before.code != after.code {
    fields.push("code");
  }
  if before.doc != after.doc {
    fields.push("doc");
  }
  if before.schema != after.schema {
    fields.push("schema");
  }
  if before.examples != after.examples {
    fields.push("examples");
  }
  if before.tests != after.tests {
    fields.push("tests");
  }
  if before.tags != after.tags {
    fields.push("tags");
  }
  if before.ffi != after.ffi {
    fields.push("ffi");
  }
  fields
}

fn plan_ns_view(
  snapshot_file: &str,
  current: &Snapshot,
  incoming: &Snapshot,
  namespace: &str,
  allow_remove: bool,
  allow_unknown_head: bool,
) -> Result<NsViewPlan, String> {
  let file = &current.files[namespace];
  let next = &incoming.files[namespace];
  let ns = (file.ns != next.ns).then(|| next.ns.clone());
  if file.ns.code != next.ns.code {
    // An unchanged historical `ns` form may carry another name; a new form must match.
    match &next.ns.code {
      Cirru::List(items) if items.first().is_some_and(|head| head.eq_leaf("ns")) => match items.get(1) {
        Some(Cirru::Leaf(name)) if name.as_ref() == namespace => {}
        Some(Cirru::Leaf(name)) => {
          return Err(format!(
            "Namespace name mismatch: the command targets '{namespace}' but the data declares `ns {name}`"
          ));
        }
        _ => return Err("The `:ns` code must name the namespace: `ns <namespace> ...`".to_owned()),
      },
      _ => return Err(format!("The `:ns` code must be an `ns {namespace} ...` form")),
    }
    for warning in validate_import_rules(&extract_require_rules(&next.ns.code))? {
      eprintln!("{} in namespace '{namespace}': {warning}", "Warning:".yellow());
    }
  }

  let mut removals = file
    .defs
    .keys()
    .filter(|name| !next.defs.contains_key(name.as_str()))
    .cloned()
    .collect::<Vec<_>>();
  removals.sort();
  if !removals.is_empty() && !allow_remove {
    return Err(format!(
      "The data omits {} definition(s) of '{namespace}': {}. Pass --allow-remove to delete them, or add them back.",
      removals.len(),
      removals.join(", ")
    ));
  }

  // Resolve definition heads against the namespace as the data describes it:
  // its imports, plus macros the data itself adds or changes.
  let mut validation_snapshot = current.clone();
  validation_snapshot.files.insert(namespace.to_owned(), next.clone());

  let mut updates = BTreeMap::new();
  let mut changes = vec![];
  let names = next.defs.keys().collect::<BTreeSet<_>>();
  for name in names {
    let owner = format!("{namespace}/{name}");
    let previous = file.defs.get(name);
    let entry = &next.defs[name];
    if previous.is_none_or(|previous| previous.code != entry.code) {
      snapshot::validate_defexternal_shorthand(&entry.code, &owner)?;
      // Check this definition against its stored code (or its absence), so the
      // name-mismatch exception for unchanged historical code does not apply.
      let validation_defs = &mut validation_snapshot.files.get_mut(namespace).expect("namespace exists").defs;
      let staged = match previous {
        Some(previous) => validation_defs.insert(name.clone(), previous.clone()),
        None => validation_defs.remove(name),
      };
      let checked = validate_definition_shape(
        snapshot_file,
        &validation_snapshot,
        namespace,
        name,
        &entry.code,
        allow_unknown_head,
      );
      let validation_defs = &mut validation_snapshot.files.get_mut(namespace).expect("namespace exists").defs;
      if let Some(staged) = staged {
        validation_defs.insert(name.clone(), staged);
      }
      checked?;
    }
    let (status, fields) = match previous {
      None => (DefinitionStatus::Added, vec![]),
      Some(previous) if previous != entry => (DefinitionStatus::Changed, changed_fields(previous, entry)),
      Some(_) => (DefinitionStatus::Unchanged, vec![]),
    };
    if status != DefinitionStatus::Unchanged {
      updates.insert(name.clone(), entry.clone());
    }
    changes.push(DefinitionChange {
      name: name.clone(),
      status,
      fields,
    });
  }
  changes.extend(removals.iter().map(|name| DefinitionChange {
    name: name.clone(),
    status: DefinitionStatus::Removed,
    fields: vec![],
  }));
  changes.sort_by(|a, b| a.name.cmp(&b.name));
  Ok(NsViewPlan {
    ns,
    updates,
    removals,
    changes,
  })
}

/// Check `--expect-revision` when nothing would be written.
fn check_unchanged_revision(snapshot_file: &str, snapshot: &Snapshot, expected: Option<&str>) -> Result<String, String> {
  let content = fs::read_to_string(snapshot_file).map_err(|error| format!("Failed to read snapshot '{snapshot_file}': {error}"))?;
  let revision = snapshot_content_revision(&content);
  match expected {
    Some(expected) if super::snapshot_scope::is_scoped_revision(expected) => {
      super::snapshot_scope::check_scoped_revision(expected, &super::snapshot_scope::snapshot_units(snapshot)?)?;
    }
    Some(expected) if expected != revision => {
      return Err(format!(
        "Snapshot revision mismatch: expected '{expected}', current revision is '{revision}'. Re-read the namespace data and redo the edit."
      ));
    }
    _ => {}
  }
  Ok(revision)
}

pub(crate) fn handle_edit_ns(opts: &EditNsCommand, snapshot_file: &str) -> Result<(), String> {
  let output_format = StructuredOutputFormat::parse(&opts.format, "edit ns")?;
  let namespace = opts.namespace.as_str();
  let raw = read_code_input(&opts.file, &opts.code)?
    .ok_or("Namespace data required: use --file, --code, or pipe the `query ns <ns> --format cirru` output via stdin")?;
  check_record_keys(&raw, namespace)?;
  let file_data = cirru_edn::parse(&raw).map_err(|error| {
    format!(
      "Failed to parse namespace data as Cirru EDN: {}",
      truncate_message(&error.to_string(), 400)
    )
  })?;
  let current = load_snapshot(snapshot_file)?;
  check_ns_editable(&current, namespace)?;
  if !current.files.contains_key(namespace) {
    return Err(format!(
      "Namespace '{namespace}' not found; create it with `calcit edit add-ns {namespace}` first"
    ));
  }
  let incoming = load_with_file_entry(snapshot_file, namespace, file_data)?;
  let plan = plan_ns_view(
    snapshot_file,
    &current,
    &incoming,
    namespace,
    opts.allow_remove,
    opts.allow_unknown_head,
  )?;

  let report = if plan.is_empty() {
    // Nothing to write: leave the file untouched, even if it is not canonically formatted.
    let revision = check_unchanged_revision(snapshot_file, &current, opts.expect_revision.as_deref())?;
    NsViewReport {
      schema_version: 1,
      command: "edit.ns",
      namespace: namespace.to_owned(),
      dry_run: opts.dry_run,
      changed: false,
      original_revision: revision.clone(),
      new_revision: revision,
      scoped_revision: None,
      ns_changed: false,
      definitions: plan.changes,
    }
  } else {
    let operation = vec!["edit".to_owned(), "ns".to_owned(), namespace.to_owned()];
    let transaction = run_staged_transaction_with_options(
      Path::new(snapshot_file),
      std::slice::from_ref(&operation),
      opts.expect_revision.as_deref(),
      opts.dry_run,
      true,
      |stage_path, index, args| {
        let stage = stage_path.to_string_lossy();
        let mut staged = load_snapshot(&stage)?;
        let file = staged
          .files
          .get_mut(namespace)
          .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;
        plan.apply(file);
        save_snapshot(&staged, &stage)?;
        Ok(TransactionOperationReport {
          index,
          args: args.to_vec(),
          stdout: String::new(),
          stderr: String::new(),
        })
      },
    )?;
    NsViewReport {
      schema_version: 1,
      command: "edit.ns",
      namespace: namespace.to_owned(),
      dry_run: opts.dry_run,
      changed: transaction.changed,
      original_revision: transaction.original_revision,
      new_revision: transaction.new_revision,
      scoped_revision: transaction.scoped_revision,
      ns_changed: plan.ns.is_some(),
      definitions: plan.changes,
    }
  };
  print_report(&report, output_format)
}

fn print_report(report: &NsViewReport, format: StructuredOutputFormat) -> Result<(), String> {
  match format {
    StructuredOutputFormat::Json => println!(
      "{}",
      serde_json::to_string(report).map_err(|error| format!("Failed to serialize edit ns result: {error}"))?
    ),
    StructuredOutputFormat::Edn => {
      let value = serde_json::to_value(report).map_err(|error| format!("Failed to encode edit ns result: {error}"))?;
      println!("{}", format_json_value_as_edn(&value)?);
    }
    StructuredOutputFormat::Human => {
      println!("# Edit namespace `{}`\n", report.namespace);
      println!("- mode: `{}`", if report.dry_run { "preview" } else { "apply" });
      println!("- original revision: `{}`", report.original_revision);
      println!("- new revision: `{}`", report.new_revision);
      if let Some(scoped) = &report.scoped_revision {
        println!("- scoped revision: `{scoped}`");
      }
      println!("- changed: `{}`", report.changed);
      println!("- ns entry: `{}`", if report.ns_changed { "changed" } else { "unchanged" });
      let mut unchanged = 0;
      println!("\n## Definitions\n");
      for change in &report.definitions {
        match change.status {
          DefinitionStatus::Unchanged => unchanged += 1,
          DefinitionStatus::Changed => println!("- changed `{}` ({})", change.name, change.fields.join(", ")),
          status => println!("- {} `{}`", status.label(), change.name),
        }
      }
      // Structured output lists every definition; human output only counts unchanged ones.
      println!("- unchanged: {unchanged}");
      if report.dry_run && report.changed {
        let revision = report.scoped_revision.as_deref().unwrap_or(&report.original_revision);
        println!("\nApply with `--expect-revision '{revision}'` and without `--dry-run`.");
      }
    }
  }
  Ok(())
}
