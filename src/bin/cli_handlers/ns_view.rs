//! Whole-namespace Cirru view: `query ns <ns> --format cirru` and `edit ns <ns>`.
//!
//! The view is a projection of one Snapshot namespace as plain Cirru source:
//!
//! ```cirru
//! ns app.demo
//!   :require
//!     app.lib :refer $ helper
//!
//! defn greet (name)
//!   str "|Hi " name
//!
//! :meta greet
//!   :doc "|Greets a person"
//!   :schema $ :: :fn $ {} (:args $ [] :string) (:return :string)
//! ```
//!
//! The first expression is the full `ns` form. Each following expression is one
//! definition's code, sorted by name, optionally followed by a `:meta <name>`
//! block holding doc and schema. A definition whose code does not carry its
//! Snapshot key as the second leaf (for example a top-level `fn`) is written as
//! `:def <name> <code>`. Tests, examples, tags and FFI metadata stay out of the
//! view and are kept unchanged on write-back.

use calcit::calcit::{CalcitTypeAnnotation, DYNAMIC_TYPE};
use calcit::cli_args::EditNsCommand;
use calcit::program::validate_import_rules;
use calcit::snapshot::{self, CodeEntry, FileInSnapShot, Snapshot};
use cirru_parser::Cirru;
use colored::Colorize;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use super::common::read_code_input;
use super::edit::{
  TransactionOperationReport, check_ns_editable, extract_require_rules, load_snapshot, run_staged_transaction_with_options,
  save_snapshot, snapshot_content_revision, strip_name_field_from_schema, validate_definition_shape,
};
use super::query::query_schema_cirru;
use super::structured_output::{StructuredOutputFormat, format_json_value_as_edn};

const META_HEAD: &str = ":meta";
const DEF_HEAD: &str = ":def";

/// The Snapshot key a definition's code declares, when the view can rely on it.
fn declared_definition_name(code: &Cirru) -> Option<&str> {
  let Cirru::List(items) = code else { return None };
  match (items.first(), items.get(1)) {
    (Some(Cirru::Leaf(head)), Some(Cirru::Leaf(name)))
      if !matches!(head.as_ref(), "fn" | "deftype-slot" | "ns" | META_HEAD | DEF_HEAD) =>
    {
      Some(name.as_ref())
    }
    _ => None,
  }
}

/// Format one top-level expression so that parsing the text yields the same node.
fn format_top_level(node: &Cirru) -> Result<String, String> {
  [true, false]
    .into_iter()
    .filter_map(|use_inline| cirru_parser::format(std::slice::from_ref(node), cirru_parser::CirruWriterOptions { use_inline }).ok())
    .find(|text| cirru_parser::parse(text).is_ok_and(|parsed| parsed.len() == 1 && &parsed[0] == node))
    .map(|text| text.trim_matches('\n').to_owned())
    .ok_or_else(|| "cannot render the expression as Cirru text that parses back to the same source".to_owned())
}

fn leaf(text: &str) -> Cirru {
  Cirru::Leaf(Arc::from(text))
}

/// Render the `:meta <name>` block, or `None` when the definition has neither doc nor schema.
fn meta_block(name: &str, entry: &CodeEntry) -> Result<Option<Cirru>, String> {
  let mut items = vec![leaf(META_HEAD), leaf(name)];
  if !entry.doc.is_empty() {
    items.push(Cirru::List(vec![leaf(":doc"), leaf(&format!("|{}", entry.doc))]));
  }
  if let Some(schema) = query_schema_cirru(entry.schema.as_ref(), true)? {
    items.push(Cirru::List(vec![leaf(":schema"), schema]));
  }
  Ok((items.len() > 2).then_some(Cirru::List(items)))
}

/// Render one namespace as the writable Cirru view.
pub(crate) fn render_ns_view(namespace: &str, file: &FileInSnapShot) -> Result<String, String> {
  let mut nodes = vec![file.ns.code.clone()];
  let mut names = file.defs.keys().collect::<Vec<_>>();
  names.sort();
  for name in names {
    let entry = &file.defs[name];
    if declared_definition_name(&entry.code) == Some(name.as_str()) {
      nodes.push(entry.code.clone());
    } else {
      nodes.push(Cirru::List(vec![leaf(DEF_HEAD), leaf(name), entry.code.clone()]));
    }
    if let Some(meta) = meta_block(name, entry).map_err(|error| format!("`{namespace}/{name}`: {error}"))? {
      nodes.push(meta);
    }
  }
  let mut parts = Vec::with_capacity(nodes.len());
  for node in &nodes {
    parts.push(format_top_level(node).map_err(|error| format!("Cannot render namespace '{namespace}': {error}"))?);
  }
  let text = format!("{}\n", parts.join("\n\n"));
  // Only print a view that parses back to exactly these expressions, so writing it back is lossless.
  if cirru_parser::parse(&text).ok().as_deref() != Some(nodes.as_slice()) {
    return Err(format!(
      "Cannot render namespace '{namespace}' as Cirru text that parses back to the same source"
    ));
  }
  Ok(text)
}

/// Metadata requested by a `:meta` block. A missing field keeps the stored value.
#[derive(Debug, Clone, Default, PartialEq)]
struct MetaPatch {
  doc: Option<String>,
  /// `Some(None)` clears the schema (`:schema nil`).
  schema: Option<Option<Cirru>>,
}

#[derive(Debug, Clone, PartialEq)]
struct NsView {
  ns_code: Cirru,
  defs: Vec<(String, Cirru)>,
  metas: BTreeMap<String, MetaPatch>,
}

fn parse_meta_block(items: &[Cirru]) -> Result<(String, MetaPatch), String> {
  let Some(Cirru::Leaf(name)) = items.get(1) else {
    return Err("`:meta` block must start with a definition name: `:meta <name>`".to_owned());
  };
  let mut patch = MetaPatch::default();
  for field in &items[2..] {
    let Cirru::List(pair) = field else {
      return Err(format!("`:meta {name}` fields must be `(:doc |text)` or `(:schema <type>)` pairs"));
    };
    let key = match pair.first() {
      Some(Cirru::Leaf(key)) => key.as_ref(),
      _ => "",
    };
    match key {
      ":doc" | ":schema" if pair.len() != 2 => {
        return Err(format!(
          "`:meta {name}` field {key} takes exactly one value; quote text with spaces as `\"|some text\"`"
        ));
      }
      ":doc" => {
        if patch.doc.is_some() {
          return Err(format!("`:meta {name}` repeats :doc"));
        }
        let doc = match &pair[1] {
          Cirru::Leaf(text) if text.starts_with('|') => text[1..].to_owned(),
          _ => {
            return Err(format!(
              "`:meta {name}` :doc must be a string leaf such as `|text` or `\"|some text\"`"
            ));
          }
        };
        patch.doc = Some(doc);
      }
      ":schema" => {
        if patch.schema.is_some() {
          return Err(format!("`:meta {name}` repeats :schema"));
        }
        patch.schema = Some(if pair[1].eq_leaf("nil") { None } else { Some(pair[1].clone()) });
      }
      _ => {
        return Err(format!(
          "`:meta {name}` accepts only `:doc |text` and `:schema <type>` (or `:schema nil` to clear); tests and examples are edited with `edit add-test` / `edit examples`"
        ));
      }
    }
  }
  Ok((name.to_string(), patch))
}

fn parse_ns_view(raw: &str, namespace: &str) -> Result<NsView, String> {
  let nodes = cirru_parser::parse(raw).map_err(|error| format!("Failed to parse namespace view as Cirru: {error}"))?;
  let mut nodes = nodes.into_iter();
  let ns_code = match nodes.next() {
    Some(Cirru::List(items)) if items.first().is_some_and(|head| head.eq_leaf("ns")) => {
      if !matches!(items.get(1), Some(Cirru::Leaf(_))) {
        return Err("The view's `ns` form must name the namespace: `ns <namespace> ...`".to_owned());
      }
      Cirru::List(items)
    }
    _ => return Err(format!("A namespace view must start with the full `ns {namespace} ...` form")),
  };

  let mut defs: Vec<(String, Cirru)> = vec![];
  let mut seen = HashSet::new();
  let mut metas = BTreeMap::new();
  for (index, node) in nodes.enumerate() {
    let position = index + 2;
    let Cirru::List(items) = &node else {
      return Err(format!(
        "Top-level expression {position} is a bare leaf; each expression must be a definition or a `:meta` block"
      ));
    };
    let head = items.first();
    if head.is_some_and(|head| head.eq_leaf(META_HEAD)) {
      let (name, patch) = parse_meta_block(items)?;
      if metas.insert(name.clone(), patch).is_some() {
        return Err(format!("The view has more than one `:meta {name}` block"));
      }
      continue;
    }
    let (name, code) = if head.is_some_and(|head| head.eq_leaf(DEF_HEAD)) {
      match items.as_slice() {
        [_, Cirru::Leaf(name), code] => (name.to_string(), code.clone()),
        _ => return Err("`:def` expects exactly a name and the definition code: `:def <name> <code>`".to_owned()),
      }
    } else if head.is_some_and(|head| head.eq_leaf("ns")) {
      return Err("A namespace view has exactly one `ns` form, as its first expression".to_owned());
    } else {
      let Some(name) = declared_definition_name(&node) else {
        return Err(format!(
          "Cannot read a definition name from top-level expression {position}; write it as `:def <name> <code>`"
        ));
      };
      (name.to_owned(), node.clone())
    };
    if !seen.insert(name.clone()) {
      return Err(format!("Definition '{name}' appears more than once in the view"));
    }
    defs.push((name, code));
  }
  if let Some(orphan) = metas.keys().find(|name| !seen.contains(name.as_str())) {
    return Err(format!("`:meta {orphan}` has no matching definition in the view"));
  }
  Ok(NsView { ns_code, defs, metas })
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
  /// Which parts differ for a changed definition: `code`, `doc`, `schema`.
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
  ns_code: Option<Cirru>,
  /// New entries for added or changed definitions.
  updates: BTreeMap<String, CodeEntry>,
  removals: Vec<String>,
  changes: Vec<DefinitionChange>,
}

impl NsViewPlan {
  fn is_empty(&self) -> bool {
    self.ns_code.is_none() && self.updates.is_empty() && self.removals.is_empty()
  }

  fn apply(&self, file: &mut FileInSnapShot) {
    if let Some(code) = &self.ns_code {
      file.ns.code = code.clone();
    }
    for name in &self.removals {
      file.defs.remove(name);
    }
    for (name, entry) in &self.updates {
      file.defs.insert(name.clone(), entry.clone());
    }
  }
}

/// Resolve a requested schema against the stored one; `None` means unchanged.
fn requested_schema(
  owner: &str,
  existing: &Arc<CalcitTypeAnnotation>,
  requested: &Option<Cirru>,
) -> Result<Option<Arc<CalcitTypeAnnotation>>, String> {
  let Some(requested) = requested else {
    return Ok((!matches!(existing.as_ref(), CalcitTypeAnnotation::Dynamic)).then(|| DYNAMIC_TYPE.clone()));
  };
  if query_schema_cirru(existing.as_ref(), true)?.as_ref() == Some(requested) {
    return Ok(None);
  }
  let parsed = snapshot::parse_schema_annotation_for_write(&strip_name_field_from_schema(requested.clone()))
    .map_err(|error| format!("Schema validation failed for `{owner}`: {error}"))?;
  Ok((parsed.as_ref() != existing.as_ref()).then_some(parsed))
}

fn plan_ns_view(
  snapshot_file: &str,
  snapshot: &Snapshot,
  namespace: &str,
  view: &NsView,
  allow_remove: bool,
  allow_unknown_head: bool,
) -> Result<NsViewPlan, String> {
  let file = &snapshot.files[namespace];
  let ns_code = (file.ns.code != view.ns_code).then(|| view.ns_code.clone());
  if ns_code.is_some() {
    // An unchanged historical `ns` form may carry another name; a new form must match.
    if let Cirru::List(items) = &view.ns_code
      && let Some(Cirru::Leaf(name)) = items.get(1)
      && name.as_ref() != namespace
    {
      return Err(format!(
        "Namespace name mismatch: the command targets '{namespace}' but the view declares `ns {name}`"
      ));
    }
    for warning in validate_import_rules(&extract_require_rules(&view.ns_code))? {
      eprintln!("{} in namespace '{namespace}': {warning}", "Warning:".yellow());
    }
  }
  // Resolve definition heads against the imports the view declares.
  let mut validation_snapshot = snapshot.clone();
  if let Some(code) = &ns_code {
    validation_snapshot.files.get_mut(namespace).expect("namespace exists").ns.code = code.clone();
  }

  let in_view = view.defs.iter().map(|(name, _)| name.as_str()).collect::<HashSet<_>>();
  let mut removals = file
    .defs
    .keys()
    .filter(|name| !in_view.contains(name.as_str()))
    .cloned()
    .collect::<Vec<_>>();
  removals.sort();
  if !removals.is_empty() && !allow_remove {
    return Err(format!(
      "The view omits {} definition(s) of '{namespace}': {}. Pass --allow-remove to delete them, or add them back to the view.",
      removals.len(),
      removals.join(", ")
    ));
  }

  let mut updates = BTreeMap::new();
  let mut changes = vec![];
  let empty_meta = MetaPatch::default();
  for (name, code) in &view.defs {
    let owner = format!("{namespace}/{name}");
    let meta = view.metas.get(name).unwrap_or(&empty_meta);
    let previous = file.defs.get(name);
    let code_changed = previous.is_none_or(|entry| &entry.code != code);
    let mut entry = match previous {
      Some(entry) => entry.clone(),
      None => CodeEntry::from_code(code.clone()),
    };
    let mut fields = vec![];
    if code_changed {
      snapshot::validate_defexternal_shorthand(code, &owner)?;
      let derived_macro_schema = snapshot::conservative_macro_schema(code, &format!("definition '{owner}'"))?;
      validate_definition_shape(snapshot_file, &validation_snapshot, namespace, name, code, allow_unknown_head)?;
      entry.code = code.clone();
      // Same rules as `edit def --overwrite`: shorthand regenerates external
      // metadata, and a macro gets a conservative schema unless it has one.
      if snapshot::code_declares_defexternal(code) {
        entry.ffi = None;
      }
      if let Some(schema) = derived_macro_schema
        && !matches!(entry.schema.as_ref(), CalcitTypeAnnotation::Macro(_))
      {
        entry.schema = schema;
      }
      fields.push("code");
    }
    if let Some(doc) = &meta.doc
      && doc != &entry.doc
    {
      entry.doc = doc.clone();
      fields.push("doc");
    }
    if let Some(requested) = &meta.schema
      && let Some(schema) = requested_schema(&owner, &entry.schema, requested)?
    {
      entry.schema = schema;
      fields.push("schema");
    }
    let status = match previous {
      None => DefinitionStatus::Added,
      Some(previous) if previous != &entry => DefinitionStatus::Changed,
      Some(_) => DefinitionStatus::Unchanged,
    };
    if status != DefinitionStatus::Unchanged {
      updates.insert(name.clone(), entry);
    }
    if status != DefinitionStatus::Changed {
      fields.clear();
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
    ns_code,
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
        "Snapshot revision mismatch: expected '{expected}', current revision is '{revision}'. Re-read the namespace view and redo the edit."
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
    .ok_or("Namespace view required: use --file, --code, or pipe the `query ns <ns> --format cirru` output via stdin")?;
  let view = parse_ns_view(&raw, namespace)?;
  let snapshot = load_snapshot(snapshot_file)?;
  check_ns_editable(&snapshot, namespace)?;
  if !snapshot.files.contains_key(namespace) {
    return Err(format!(
      "Namespace '{namespace}' not found; create it with `calcit edit add-ns {namespace}` first"
    ));
  }
  let plan = plan_ns_view(
    snapshot_file,
    &snapshot,
    namespace,
    &view,
    opts.allow_remove,
    opts.allow_unknown_head,
  )?;

  let report = if plan.is_empty() {
    // Nothing to write: leave the file untouched, even if it is not canonically formatted.
    let revision = check_unchanged_revision(snapshot_file, &snapshot, opts.expect_revision.as_deref())?;
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
      ns_changed: plan.ns_code.is_some(),
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
      println!("- ns form: `{}`", if report.ns_changed { "changed" } else { "unchanged" });
      let mut unchanged = vec![];
      let mut lines = vec![];
      for change in &report.definitions {
        match change.status {
          DefinitionStatus::Unchanged => unchanged.push(format!("`{}`", change.name)),
          DefinitionStatus::Changed => lines.push(format!("- changed `{}` ({})", change.name, change.fields.join(", "))),
          status => lines.push(format!("- {} `{}`", status.label(), change.name)),
        }
      }
      println!("\n## Definitions\n");
      for line in &lines {
        println!("{line}");
      }
      if !unchanged.is_empty() {
        println!("- unchanged ({}): {}", unchanged.len(), unchanged.join(", "));
      }
      if report.dry_run && report.changed {
        let revision = report.scoped_revision.as_deref().unwrap_or(&report.original_revision);
        println!("\nApply with `--expect-revision '{revision}'` and without `--dry-run`.");
      }
    }
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parse_one(text: &str) -> Cirru {
    cirru_parser::parse(text).unwrap().remove(0)
  }

  #[test]
  fn view_parsing_reads_definitions_meta_and_named_wrappers() {
    let view = parse_ns_view(
      "ns app.demo\n  :require $ app.lib :refer $ helper\n\ndefn greet (name) (str |Hi name)\n\n:meta greet\n  :doc \"|Greets a person\"\n  :schema nil\n\n:def handler $ fn (x) x\n",
      "app.demo",
    )
    .unwrap();
    assert_eq!(view.defs.len(), 2);
    assert_eq!(view.defs[0].0, "greet");
    assert_eq!(view.defs[1], ("handler".to_owned(), parse_one("fn (x) x")));
    assert_eq!(
      view.metas["greet"],
      MetaPatch {
        doc: Some("Greets a person".to_owned()),
        schema: Some(None),
      }
    );
  }

  #[test]
  fn view_parsing_rejects_malformed_views() {
    let cases = [
      ("defn f () 1", "must start with the full `ns"),
      ("ns (app.demo)", "must name the namespace"),
      ("ns app.demo\n\ndefn f () 1\n\ndefn f () 2", "more than once"),
      ("ns app.demo\n\n:meta g\n  :doc |x", "no matching definition"),
      ("ns app.demo\n\ndefn f () 1\n\n:meta f\n  :tests $ []", "accepts only"),
      ("ns app.demo\n\nfn (x) x", "write it as `:def <name> <code>`"),
      ("ns app.demo\n\nns app.demo", "exactly one `ns` form"),
      ("ns app.demo\n\ndefn f () 1\n\n:meta f\n  :doc plain", ":doc must be a string leaf"),
    ];
    for (text, expected) in cases {
      let error = parse_ns_view(text, "app.demo").unwrap_err();
      assert!(error.contains(expected), "{text:?} should fail with {expected:?}, got {error}");
    }
  }

  #[test]
  fn rendered_view_wraps_definitions_whose_code_lacks_the_key() {
    let mut defs = std::collections::HashMap::new();
    defs.insert("b".to_owned(), CodeEntry::from_code(parse_one("fn (x) x")));
    let mut documented = CodeEntry::from_code(parse_one("defn a () 1"));
    documented.doc = "First \"quoted\" line".to_owned();
    defs.insert("a".to_owned(), documented);
    let file = FileInSnapShot {
      ns: snapshot::NsEntry {
        doc: String::new(),
        code: parse_one("ns app.demo"),
      },
      defs,
    };
    let text = render_ns_view("app.demo", &file).unwrap();
    let view = parse_ns_view(&text, "app.demo").unwrap();
    assert_eq!(
      view.defs.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>(),
      ["a", "b"],
      "{text}"
    );
    assert!(text.contains(":def b"), "{text}");
    assert_eq!(view.metas["a"].doc.as_deref(), Some("First \"quoted\" line"));
  }
}
