//! Edit and Tree subcommand handlers and shared utilities
//!
//! Handles: cr edit - code editing operations (definitions, namespaces, modules, configs)
//! Shared by: cr tree - fine-grained tree operations (replace, insert, delete, swap, wrap)
//!
//! Supports code input via:
//! - `--file <path>` - read from file (auto-detects JSON vs Cirru)
//! - `--code <string>` - inline text (auto-detects JSON vs Cirru)
//! - stdin - pipe or redirect input (auto-detects JSON vs Cirru)

use calcit::calcit::{CalcitTypeAnnotation, DYNAMIC_TYPE};
use calcit::cli_args::{
  EditAddExampleCommand, EditAddImportCommand, EditAddNsCommand, EditCommand, EditCpCommand, EditDefCommand, EditDocCommand,
  EditExamplesCommand, EditFormatCommand, EditImportsCommand, EditIncCommand, EditMvDefCommand, EditMvNodeCommand, EditNsDocCommand,
  EditRenameCommand, EditRmDefCommand, EditRmExampleCommand, EditRmImportCommand, EditRmNsCommand, EditSchemaCommand,
  EditSplitDefCommand, EditSubcommand, EditTagsCommand, EditTransactionCommand,
};
use calcit::program::validate_import_rules;
use calcit::program_diff::{CirruEditStrategy, analyze_cirru_edit_advice};
use calcit::snapshot::{
  self, ChangesDict, CodeEntry, FileChangeInfo, FileInSnapShot, NsEntry, Snapshot, render_snapshot_content, save_snapshot_to_file,
  validate_schema_for_write,
};
use cirru_edn::EdnTag;
use cirru_parser::Cirru;
use colored::Colorize;
use md5::{Digest, Md5};
use semver::Version;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use super::common::{
  ERR_CODE_INPUT_REQUIRED, format_path, parse_input_to_cirru, parse_path, parse_quoted_cirru_nodes, print_cli_warning_block,
  read_code_input, resolve_definition_lookup,
};
use super::cursor::{
  TreeCursorMutation, maintain_cursor_after_any_mutation, maintain_cursor_after_definition_delete,
  maintain_cursor_after_definition_move, maintain_cursor_after_definition_replace, maintain_cursor_after_namespace_delete,
  maintain_cursor_after_node_move, maintain_cursor_after_split_definition, maintain_cursor_after_tree_mutation,
  resolve_active_cursor_reference, resolve_cursor_path_argument, resolve_cursor_target_argument,
};
use super::tips::{Tips, command_guidance_enabled};

/// Parse "namespace/definition" format into (namespace, definition)
/// Splits at the FIRST '/' so operator definitions like '/' and '/=' are handled correctly.
pub(crate) fn parse_target(target: &str) -> Result<(&str, &str), String> {
  target
    .split_once('/')
    .ok_or_else(|| format!("Invalid target format: '{target}'. Expected 'namespace/definition' (e.g. 'app.core/main')"))
}

/// Process a node by replacing placeholders with references to original node or its branches
pub(crate) fn process_node_with_references(
  node: &Cirru,
  references: &std::collections::BTreeMap<String, Cirru>,
) -> Result<Cirru, String> {
  match node {
    Cirru::Leaf(s) => {
      // Check if this leaf matches any of the placeholders
      if let Some(replacement) = references.get(s.as_ref()) {
        return Ok(replacement.clone());
      }
      Ok(node.clone())
    }
    Cirru::List(items) => {
      let processed_items: Result<Vec<Cirru>, String> =
        items.iter().map(|item| process_node_with_references(item, references)).collect();
      Ok(Cirru::List(processed_items?))
    }
  }
}

pub fn handle_edit_command(cmd: &EditCommand, snapshot_file: &str) -> Result<(), String> {
  let mut resolved = cmd.clone();
  resolve_edit_cursor_references(&mut resolved, snapshot_file)?;
  let result = match &resolved.subcommand {
    EditSubcommand::Format(opts) => handle_format(opts, snapshot_file),
    EditSubcommand::Transaction(opts) => handle_transaction(opts, snapshot_file),
    EditSubcommand::Def(opts) => handle_def(opts, snapshot_file),
    EditSubcommand::MvDef(opts) => handle_mv_def(opts, snapshot_file),
    EditSubcommand::RmDef(opts) => handle_rm_def(opts, snapshot_file),
    EditSubcommand::Cp(opts) => handle_cp_node(opts, snapshot_file),
    EditSubcommand::Mv(opts) => handle_mv_node(opts, snapshot_file),
    EditSubcommand::Rename(opts) => handle_rename(opts, snapshot_file),
    EditSubcommand::SplitDef(opts) => handle_split_def(opts, snapshot_file),
    EditSubcommand::Doc(opts) => handle_doc(opts, snapshot_file),
    EditSubcommand::Schema(opts) => handle_schema(opts, snapshot_file),
    EditSubcommand::Examples(opts) => handle_examples(opts, snapshot_file),
    EditSubcommand::AddExample(opts) => handle_add_example(opts, snapshot_file),
    EditSubcommand::RmExample(opts) => handle_rm_example(opts, snapshot_file),
    EditSubcommand::Tags(opts) => handle_tags(opts, snapshot_file),
    EditSubcommand::AddNs(opts) => handle_add_ns(opts, snapshot_file),
    EditSubcommand::RmNs(opts) => handle_rm_ns(opts, snapshot_file),
    EditSubcommand::Imports(opts) => handle_imports(opts, snapshot_file),
    EditSubcommand::AddImport(opts) => handle_add_import(opts, snapshot_file),
    EditSubcommand::RmImport(opts) => handle_rm_import(opts, snapshot_file),
    EditSubcommand::NsDoc(opts) => handle_ns_doc(opts, snapshot_file),
    EditSubcommand::Inc(opts) => handle_inc(opts, snapshot_file),
  };
  result?;
  maintain_cursor_after_edit(&resolved, snapshot_file)
}

fn resolve_edit_cursor_references(cmd: &mut EditCommand, snapshot_file: &str) -> Result<(), String> {
  let target = match &mut cmd.subcommand {
    EditSubcommand::Def(opts) => Some(&mut opts.target),
    EditSubcommand::MvDef(opts) => Some(&mut opts.source),
    EditSubcommand::RmDef(opts) => Some(&mut opts.target),
    EditSubcommand::Doc(opts) => Some(&mut opts.target),
    EditSubcommand::Schema(opts) => Some(&mut opts.target),
    EditSubcommand::Examples(opts) => Some(&mut opts.target),
    EditSubcommand::AddExample(opts) => Some(&mut opts.target),
    EditSubcommand::RmExample(opts) => Some(&mut opts.target),
    EditSubcommand::Tags(opts) => Some(&mut opts.target),
    EditSubcommand::Cp(opts) => Some(&mut opts.target),
    EditSubcommand::Mv(opts) => Some(&mut opts.target),
    EditSubcommand::Rename(opts) => Some(&mut opts.source),
    EditSubcommand::SplitDef(opts) => Some(&mut opts.target),
    EditSubcommand::Format(_)
    | EditSubcommand::Transaction(_)
    | EditSubcommand::AddNs(_)
    | EditSubcommand::RmNs(_)
    | EditSubcommand::Imports(_)
    | EditSubcommand::AddImport(_)
    | EditSubcommand::RmImport(_)
    | EditSubcommand::NsDoc(_)
    | EditSubcommand::Inc(_) => None,
  };
  let active_reference = if target.as_deref().is_some_and(|target| target == "@cursor") {
    Some(resolve_active_cursor_reference(snapshot_file)?)
  } else {
    None
  };
  if let Some(target) = target {
    *target = match &active_reference {
      Some((active_target, _)) => active_target.clone(),
      None => resolve_cursor_target_argument(snapshot_file, target)?,
    };
  }

  let resolve_path = |target: &str, path: &str| -> Result<String, String> {
    if path == "@cursor"
      && let Some((active_target, active_path)) = &active_reference
    {
      if active_target != target {
        return Err(format!(
          "Cursor target mismatch: cursor points to '{active_target}', but command targets '{target}'."
        ));
      }
      Ok(active_path.clone())
    } else {
      resolve_cursor_path_argument(snapshot_file, target, path)
    }
  };

  match &mut cmd.subcommand {
    EditSubcommand::Cp(opts) => {
      opts.from = resolve_path(&opts.target, &opts.from)?;
      opts.path = resolve_path(&opts.target, &opts.path)?;
    }
    EditSubcommand::Mv(opts) => {
      opts.from = resolve_path(&opts.target, &opts.from)?;
      opts.path = resolve_path(&opts.target, &opts.path)?;
    }
    EditSubcommand::SplitDef(opts) => {
      opts.path = resolve_path(&opts.target, &opts.path)?;
    }
    _ => {}
  }
  Ok(())
}

fn cursor_insertion_mutation(path: Vec<usize>, at: &str) -> Result<TreeCursorMutation, String> {
  match at {
    "before" => Ok(TreeCursorMutation::InsertBefore { path }),
    "after" => Ok(TreeCursorMutation::InsertAfter { path }),
    "prepend-child" => Ok(TreeCursorMutation::InsertChild { path }),
    "append-child" => Ok(TreeCursorMutation::NoPathShift),
    "replace" => Ok(TreeCursorMutation::Replace { path }),
    other => Err(format!(
      "Unsupported position '{other}'. Use: before, after, append-child, prepend-child, replace"
    )),
  }
}

fn maintain_cursor_after_edit(cmd: &EditCommand, snapshot_file: &str) -> Result<(), String> {
  match &cmd.subcommand {
    EditSubcommand::Def(opts) => maintain_cursor_after_definition_replace(snapshot_file, &opts.target),
    EditSubcommand::MvDef(opts) => maintain_cursor_after_definition_move(snapshot_file, &opts.source, &opts.target),
    EditSubcommand::RmDef(opts) => maintain_cursor_after_definition_delete(snapshot_file, &opts.target),
    EditSubcommand::RmNs(opts) => maintain_cursor_after_namespace_delete(snapshot_file, &opts.namespace),
    EditSubcommand::Cp(opts) => maintain_cursor_after_tree_mutation(
      snapshot_file,
      &opts.target,
      &cursor_insertion_mutation(parse_path(&opts.path)?, &opts.at)?,
    ),
    EditSubcommand::Mv(_) => Ok(()),
    EditSubcommand::Rename(opts) => {
      let (namespace, _) = parse_target(&opts.source)?;
      maintain_cursor_after_definition_move(snapshot_file, &opts.source, &format!("{namespace}/{}", opts.new_name))
    }
    EditSubcommand::SplitDef(opts) => {
      let (namespace, _) = parse_target(&opts.target)?;
      maintain_cursor_after_split_definition(
        snapshot_file,
        &opts.target,
        &parse_path(&opts.path)?,
        &format!("{namespace}/{}", opts.new_name),
      )
    }
    EditSubcommand::Doc(opts) => maintain_cursor_after_tree_mutation(snapshot_file, &opts.target, &TreeCursorMutation::NoPathShift),
    EditSubcommand::Schema(opts) => maintain_cursor_after_tree_mutation(snapshot_file, &opts.target, &TreeCursorMutation::NoPathShift),
    EditSubcommand::Examples(opts) => {
      maintain_cursor_after_tree_mutation(snapshot_file, &opts.target, &TreeCursorMutation::NoPathShift)
    }
    EditSubcommand::AddExample(opts) => {
      maintain_cursor_after_tree_mutation(snapshot_file, &opts.target, &TreeCursorMutation::NoPathShift)
    }
    EditSubcommand::RmExample(opts) => {
      maintain_cursor_after_tree_mutation(snapshot_file, &opts.target, &TreeCursorMutation::NoPathShift)
    }
    EditSubcommand::Tags(opts) if opts.tags.is_some() => {
      maintain_cursor_after_tree_mutation(snapshot_file, &opts.target, &TreeCursorMutation::NoPathShift)
    }
    EditSubcommand::Transaction(opts) if !opts.dry_run => {
      maintain_cursor_after_any_mutation(snapshot_file, "validated after edit transaction")
    }
    _ => Ok(()),
  }
}

fn handle_format(_opts: &EditFormatCommand, snapshot_file: &str) -> Result<(), String> {
  let original_content = fs::read_to_string(snapshot_file).map_err(|e| format!("Failed to read {snapshot_file}: {e}"))?;
  let snapshot = load_snapshot(snapshot_file)?;
  let formatted_content = render_snapshot_content(&snapshot)?;

  if formatted_content == original_content {
    println!("{} No formatting changes for '{}'", "·".dimmed(), snapshot_file.dimmed());
    return Ok(());
  }

  fs::write(snapshot_file, formatted_content).map_err(|e| format!("Failed to write {snapshot_file}: {e}"))?;

  println!("{} Formatted snapshot file '{}'", "✓".green(), snapshot_file.cyan());
  Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct TransactionOperationReport {
  index: usize,
  args: Vec<String>,
  stdout: String,
  stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct TransactionReport {
  schema_version: u8,
  command: &'static str,
  dry_run: bool,
  changed: bool,
  original_revision: String,
  new_revision: String,
  operations: Vec<TransactionOperationReport>,
}

struct StagedSnapshot {
  path: PathBuf,
  remove_on_drop: bool,
}

impl Drop for StagedSnapshot {
  fn drop(&mut self) {
    if self.remove_on_drop {
      let _ = fs::remove_file(&self.path);
    }
  }
}

fn snapshot_content_revision(content: &str) -> String {
  let mut hasher = Md5::new();
  hasher.update(content.as_bytes());
  format!("md5:{:x}", hasher.finalize())
}

fn transaction_edn_arg(value: &cirru_edn::Edn) -> Result<String, String> {
  match value {
    cirru_edn::Edn::Str(value) | cirru_edn::Edn::Symbol(value) => Ok(value.to_string()),
    cirru_edn::Edn::Tag(value) => Ok(format!(":{}", value.ref_str())),
    cirru_edn::Edn::Bool(value) => Ok(value.to_string()),
    cirru_edn::Edn::Number(value) => Ok(value.to_string()),
    cirru_edn::Edn::Nil => Ok("nil".to_string()),
    cirru_edn::Edn::Quote(code) => {
      let quoted = Cirru::List(vec![Cirru::leaf("quote"), code.clone()]);
      cirru_parser::format(
        std::slice::from_ref(&quoted),
        cirru_parser::CirruWriterOptions { use_inline: false },
      )
      .map_err(|error| format!("Failed to format quoted transaction code argument: {error}"))
    }
    other => Err(format!("Transaction arguments must be scalar values or quoted code, got: {other}")),
  }
}

fn parse_transaction_operations(raw: &str) -> Result<Vec<Vec<String>>, String> {
  let trimmed = raw.trim();
  if trimmed.is_empty() {
    return Err("Transaction input is empty. Provide a Cirru EDN list of command argument lists (JSON is also accepted).".to_string());
  }

  let operations = if trimmed.starts_with('[') && !trimmed.starts_with("[]") {
    serde_json::from_str::<Vec<Vec<String>>>(trimmed)
      .map_err(|error| format!("Failed to parse transaction JSON as a list of argument lists: {error}"))?
  } else {
    let value = cirru_edn::parse(trimmed).map_err(|error| format!("Failed to parse transaction Cirru EDN: {error}"))?;
    let cirru_edn::Edn::List(outer) = value else {
      return Err("Transaction Cirru EDN must be a list (`[]`) of command argument lists.".to_string());
    };
    outer
      .0
      .iter()
      .enumerate()
      .map(|(index, operation)| {
        let cirru_edn::Edn::List(args) = operation else {
          return Err(format!(
            "Transaction operation {} must be a list (`[]`) of CLI arguments.",
            index + 1
          ));
        };
        args.0.iter().map(transaction_edn_arg).collect::<Result<Vec<_>, _>>()
      })
      .collect::<Result<Vec<_>, _>>()?
  };

  if operations.is_empty() {
    return Err("Transaction must contain at least one operation.".to_string());
  }
  for (index, operation) in operations.iter().enumerate() {
    let Some(group) = operation.first().map(String::as_str) else {
      return Err(format!("Transaction operation {} is empty.", index + 1));
    };
    if !matches!(group, "edit" | "tree" | "config") {
      return Err(format!(
        "Transaction operation {} starts with unsupported command group '{group}'. Only edit, tree, and config mutations are allowed.",
        index + 1
      ));
    }
    if group == "edit" && operation.get(1).is_some_and(|subcommand| subcommand == "transaction") {
      return Err(format!(
        "Transaction operation {} cannot contain a nested edit transaction.",
        index + 1
      ));
    }
    let Some(subcommand) = operation.get(1).map(String::as_str) else {
      return Err(format!(
        "Transaction operation {} is missing a subcommand after '{group}'.",
        index + 1
      ));
    };
    let supported = match group {
      "edit" => matches!(
        subcommand,
        "format"
          | "def"
          | "mv-def"
          | "rm-def"
          | "doc"
          | "schema"
          | "examples"
          | "add-example"
          | "rm-example"
          | "tags"
          | "add-ns"
          | "rm-ns"
          | "imports"
          | "add-import"
          | "rm-import"
          | "ns-doc"
          | "cp"
          | "mv"
          | "rename"
          | "split-def"
      ),
      "tree" => matches!(
        subcommand,
        "rewrite"
          | "search-replace"
          | "batch-delete"
          | "replace"
          | "replace-leaf"
          | "delete"
          | "insert-before"
          | "insert-after"
          | "insert-child"
          | "append-child"
          | "swap-next"
          | "swap-prev"
          | "unwrap"
          | "raise"
          | "wrap"
      ),
      "config" => matches!(
        subcommand,
        "version" | "set" | "add-module" | "rm-module" | "set-type-slot" | "rm-type-slot"
      ),
      _ => false,
    };
    if !supported {
      return Err(format!(
        "Transaction operation {} uses unsupported staged mutation '{group} {subcommand}'. Read-only commands and edits with external side effects (such as `edit inc`) must run outside the transaction.",
        index + 1
      ));
    }
    if group == "config" && subcommand == "version" && operation.len() < 3 {
      return Err(format!(
        "Transaction operation {} uses read-only `config version`; provide a version or patch/minor/major value to mutate the staged snapshot.",
        index + 1
      ));
    }
  }
  Ok(operations)
}

fn create_staged_snapshot(snapshot_file: &Path, original_content: &str) -> Result<StagedSnapshot, String> {
  let parent = snapshot_file.parent().unwrap_or_else(|| Path::new("."));
  let file_name = snapshot_file.file_name().and_then(|name| name.to_str()).unwrap_or("calcit.cirru");
  let original_permissions = fs::metadata(snapshot_file)
    .map_err(|error| format!("Failed to read permissions for snapshot '{}': {error}", snapshot_file.display()))?
    .permissions();
  let nonce = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_err(|error| format!("System clock is before UNIX epoch: {error}"))?
    .as_nanos();

  for attempt in 0..32_u8 {
    let path = parent.join(format!(".{file_name}.transaction-{}-{nonce}-{attempt}", std::process::id()));
    match OpenOptions::new().write(true).create_new(true).open(&path) {
      Ok(mut file) => {
        file
          .write_all(original_content.as_bytes())
          .map_err(|error| format!("Failed to initialize staged snapshot '{}': {error}", path.display()))?;
        file
          .sync_all()
          .map_err(|error| format!("Failed to flush staged snapshot '{}': {error}", path.display()))?;
        fs::set_permissions(&path, original_permissions.clone())
          .map_err(|error| format!("Failed to preserve permissions on staged snapshot '{}': {error}", path.display()))?;
        return Ok(StagedSnapshot {
          path,
          remove_on_drop: true,
        });
      }
      Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
      Err(error) => return Err(format!("Failed to create staged snapshot in '{}': {error}", parent.display())),
    }
  }
  Err(format!(
    "Failed to allocate a unique staged snapshot path in '{}'.",
    parent.display()
  ))
}

fn run_staged_transaction_with<F>(
  snapshot_file: &Path,
  operations: &[Vec<String>],
  expected_revision: Option<&str>,
  dry_run: bool,
  mut run_operation: F,
) -> Result<TransactionReport, String>
where
  F: FnMut(&Path, usize, &[String]) -> Result<TransactionOperationReport, String>,
{
  let original_content =
    fs::read_to_string(snapshot_file).map_err(|error| format!("Failed to read snapshot '{}': {error}", snapshot_file.display()))?;
  let original_revision = snapshot_content_revision(&original_content);
  if let Some(expected) = expected_revision
    && expected != original_revision
  {
    return Err(format!(
      "Snapshot revision mismatch: expected '{expected}', current revision is '{original_revision}'. Re-run the query and rebuild the transaction."
    ));
  }

  let mut staged = create_staged_snapshot(snapshot_file, &original_content)?;
  let mut operation_reports = Vec::with_capacity(operations.len());
  for (index, operation) in operations.iter().enumerate() {
    operation_reports.push(run_operation(&staged.path, index, operation)?);
  }

  let staged_path = staged.path.to_string_lossy();
  let staged_snapshot = load_snapshot(&staged_path)?;
  let staged_content = render_snapshot_content(&staged_snapshot)?;
  fs::write(&staged.path, &staged_content)
    .map_err(|error| format!("Failed to finalize staged snapshot '{}': {error}", staged.path.display()))?;
  OpenOptions::new()
    .read(true)
    .open(&staged.path)
    .and_then(|file| file.sync_all())
    .map_err(|error| format!("Failed to flush staged snapshot '{}': {error}", staged.path.display()))?;

  let new_revision = snapshot_content_revision(&staged_content);
  let changed = original_content != staged_content;

  let current_content =
    fs::read_to_string(snapshot_file).map_err(|error| format!("Failed to re-read snapshot '{}': {error}", snapshot_file.display()))?;
  let current_revision = snapshot_content_revision(&current_content);
  if current_revision != original_revision {
    return Err(format!(
      "Snapshot changed while transaction was running: started at '{original_revision}', now '{current_revision}'. No transaction changes were written."
    ));
  }

  if !dry_run && changed {
    fs::rename(&staged.path, snapshot_file).map_err(|error| {
      format!(
        "Failed to atomically replace snapshot '{}' with staged file '{}': {error}",
        snapshot_file.display(),
        staged.path.display()
      )
    })?;
    staged.remove_on_drop = false;
  }

  Ok(TransactionReport {
    schema_version: 1,
    command: "edit.transaction",
    dry_run,
    changed,
    original_revision,
    new_revision,
    operations: operation_reports,
  })
}

fn run_transaction_child(stage_path: &Path, index: usize, args: &[String]) -> Result<TransactionOperationReport, String> {
  let executable = std::env::current_exe().map_err(|error| format!("Failed to locate current cr executable: {error}"))?;
  let output = Command::new(&executable)
    .arg("--tips-level")
    .arg("none")
    .env("CALCIT_CURSOR_MAINTENANCE", "disabled")
    .arg(stage_path)
    .args(args)
    .output()
    .map_err(|error| format!("Failed to run transaction operation {} ({args:?}): {error}", index + 1))?;
  let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
  let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
  if !output.status.success() {
    return Err(format!(
      "Transaction operation {} failed: {}\nstdout:\n{}\nstderr:\n{}",
      index + 1,
      args.join(" "),
      stdout.trim_end(),
      stderr.trim_end()
    ));
  }
  Ok(TransactionOperationReport {
    index,
    args: args.to_vec(),
    stdout,
    stderr,
  })
}

fn handle_transaction(opts: &EditTransactionCommand, snapshot_file: &str) -> Result<(), String> {
  if !matches!(opts.format.as_str(), "human" | "json") {
    return Err(format!("Unsupported transaction format '{}'. Expected human or json.", opts.format));
  }
  let raw = read_code_input(&opts.file, &opts.code)?.ok_or(
    "Transaction input required: use --file, --code, or pipe a Cirru EDN list of argument lists via stdin (JSON is also accepted)",
  )?;
  let operations = parse_transaction_operations(&raw)?;
  let report = run_staged_transaction_with(
    Path::new(snapshot_file),
    &operations,
    opts.expect_revision.as_deref(),
    opts.dry_run,
    run_transaction_child,
  )?;

  if opts.format == "json" {
    println!(
      "{}",
      serde_json::to_string(&report).map_err(|error| format!("Failed to serialize transaction result: {error}"))?
    );
  } else {
    let action = if opts.dry_run { "Validated" } else { "Applied" };
    println!("{} {action} {} transaction operation(s)", "✓".green(), report.operations.len());
    println!("  revision: {} -> {}", report.original_revision, report.new_revision);
    println!("  changed: {}", report.changed);
    for operation in &report.operations {
      println!("  {}. {}", operation.index + 1, operation.args.join(" "));
    }
  }
  Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Utility functions
// ═══════════════════════════════════════════════════════════════════════════════

pub(crate) fn load_snapshot(snapshot_file: &str) -> Result<Snapshot, String> {
  let content = fs::read_to_string(snapshot_file).map_err(|e| format!("Failed to read {snapshot_file}: {e}"))?;

  let edn = cirru_edn::parse(&content).map_err(|e| format!("Failed to parse EDN: {e}"))?;

  snapshot::load_snapshot_data(&edn, snapshot_file).map_err(|e| format!("Failed to load snapshot: {e}"))
}

pub(crate) fn save_snapshot(snapshot: &Snapshot, snapshot_file: &str) -> Result<(), String> {
  save_snapshot_to_file(snapshot_file, snapshot)
}

/// Check if namespace belongs to the current package (can be edited)
pub(crate) fn check_ns_editable(snapshot: &Snapshot, namespace: &str) -> Result<(), String> {
  let pkg = &snapshot.package;
  // Namespace must match package name or start with "package."
  if namespace == pkg || namespace.starts_with(&format!("{pkg}.")) {
    Ok(())
  } else {
    Err(format!(
      "Cannot modify namespace '{namespace}': only namespaces under package '{pkg}' can be edited.\nThis namespace belongs to a dependency or core library."
    ))
  }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Definition operations
// ═══════════════════════════════════════════════════════════════════════════════

fn handle_def(opts: &EditDefCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;

  let raw = read_code_input(&opts.file, &opts.code)?.ok_or(ERR_CODE_INPUT_REQUIRED)?;

  let syntax_tree = parse_input_to_cirru(&raw)?;

  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace can be edited
  check_ns_editable(&snapshot, namespace)?;

  // Check if namespace exists
  let file_data = snapshot
    .files
    .get_mut(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let exact_exists = file_data.defs.contains_key(definition);
  let lookup = if exact_exists {
    None
  } else {
    match resolve_definition_lookup(
      namespace,
      definition,
      file_data.defs.keys().map(|name| name.as_str()),
      opts.overwrite,
    ) {
      Ok(lookup) => Some(lookup),
      Err(err) if err == format!("Definition '{definition}' not found in namespace '{namespace}'") => None,
      Err(err) => return Err(err),
    }
  };
  if let Some(warning) = lookup.as_ref().and_then(|it| it.warning.as_deref()) {
    print_cli_warning_block(warning);
  }
  let resolved_definition = lookup.as_ref().map(|it| it.resolved.as_str()).unwrap_or(definition);

  let exists = file_data.defs.contains_key(resolved_definition);
  let previous_entry = if exists {
    file_data.defs.get(resolved_definition).cloned()
  } else {
    None
  };
  let existing_edit_advice = previous_entry
    .as_ref()
    .and_then(|entry| format_existing_definition_advice(namespace, resolved_definition, &entry.code, &syntax_tree));

  if opts.overwrite
    && let Some(advice) = existing_edit_advice.as_deref()
  {
    print_cli_warning_block(advice);
  }

  if exists && !opts.overwrite {
    if let Some(advice) = existing_edit_advice.as_deref() {
      print_cli_warning_block(advice);
    }
    return Err(format!(
      "Definition '{resolved_definition}' already exists in namespace '{namespace}'.\n\
       Use --overwrite to replace it. For full-definition rewrites, prefer: cr edit def {namespace}/{resolved_definition} --overwrite --file <file>"
    ));
  }

  // Create or overwrite definition.
  // For overwrite, preserve existing metadata (doc/examples/schema) and only replace code.
  let code_entry = if let Some(mut updated_entry) = previous_entry {
    updated_entry.code = syntax_tree;
    updated_entry
  } else {
    CodeEntry::from_code(syntax_tree)
  };
  file_data.defs.insert(resolved_definition.to_string(), code_entry);

  save_snapshot(&snapshot, snapshot_file)?;

  let action_label = if exists { "Updated" } else { "Created" };

  println!(
    "{} {} definition '{}' in namespace '{}'",
    "✓".green(),
    action_label,
    resolved_definition.cyan(),
    namespace
  );
  if command_guidance_enabled() {
    println!();
    println!("{}", "Next steps:".blue().bold());
    println!(
      "  • View definition: {} '{}/{}'",
      "cr query def".cyan(),
      namespace,
      resolved_definition
    );
    println!("  • Check errors: {}", "cr query error".cyan());
    println!(
      "  • Find usages: {} '{}/{}'",
      "cr query usages".cyan(),
      namespace,
      resolved_definition
    );
    println!(
      "  • Add to imports: {} <target-ns> '{}' --refer '{}'",
      "cr edit add-import".cyan(),
      namespace,
      resolved_definition
    );
    println!();
    let mut tips = Tips::new();
    tips.add(format!(
      "Use single quotes around '{namespace}/{resolved_definition}' to avoid shell escaping issues."
    ));
    tips.add(format!("Example: cr tree show '{namespace}/{resolved_definition}'"));
    tips.print();
  }
  Ok(())
}

fn format_existing_definition_advice(namespace: &str, definition: &str, existing: &Cirru, incoming: &Cirru) -> Option<String> {
  let advice = analyze_cirru_edit_advice(existing, incoming)?;
  let changed_nodes = advice.stats.added + advice.stats.removed + advice.stats.modified;
  let target = format!("{namespace}/{definition}");
  let mut lines = vec![format!(
    "Incoming code is {:.0}% structurally similar to the existing definition (changed nodes: ~{} +{} -{}).",
    advice.similarity * 100.0,
    advice.stats.modified,
    advice.stats.added,
    advice.stats.removed,
  )];

  match advice.strategy {
    CirruEditStrategy::Identical => {
      lines.push("The incoming definition is identical. Prefer skipping the write and inspect the current code first.".to_string());
      lines.push(format!("Inspect: cr query def '{target}'"));
    }
    CirruEditStrategy::Replace => {
      lines.push("Most differences are replacements. Prefer a local tree edit instead of a full overwrite.".to_string());
      lines.push(format!(
        "Try: cr tree search-replace '{target}' --pattern '<leaf>' --code '(quote |<new-leaf>')"
      ));
      lines.push(format!("Or: cr tree replace '{target}' --path '<path>' --code '(quote |<code>)'"));
    }
    CirruEditStrategy::Insert => {
      lines.push("Most differences are additive. Prefer inserting nodes into the existing tree.".to_string());
      lines.push(format!("Try: cr tree insert-before '{target}' --path '<path>' --code '<node>'"));
      lines.push(format!(
        "Or: cr tree insert-after '{target}' --path '<path>' --code '<node>' / cr tree append-child '{target}' --path '<path>' --code '<node>'"
      ));
    }
    CirruEditStrategy::Delete => {
      lines.push("Most differences are removals. Prefer deleting or lifting nodes from the existing tree.".to_string());
      lines.push(format!("Try: cr tree delete '{target}' --path '<path>'"));
      lines.push(format!("Or: cr tree raise '{target}' --path '<child-path>'"));
    }
    CirruEditStrategy::Rewrite => {
      lines.push("The trees are still close, but the change mixes insert/remove/replace. Prefer a structural rewrite over a blind full overwrite.".to_string());
      lines.push(format!(
        "Try: cr tree rewrite '{target}' --path '<path>' --with self=. --code '<code>'"
      ));
      lines.push(format!("Or: cr tree replace '{target}' --path '<path>' --code '<code>'"));
    }
  }

  if advice.strategy != CirruEditStrategy::Identical || changed_nodes > 0 {
    lines.push(format!(
      "Locate the smallest path first: cr query search '<keyword>' --filter '{target}' && cr tree show '{target}' --path '<path>'"
    ));
  }

  Some(lines.join("\n"))
}

// ═══════════════════════════════════════════════════════════════════════════════
// AST node copy / move helpers
// ═══════════════════════════════════════════════════════════════════════════════

/// Returns true if `to_path` is inside the subtree rooted at `from_path`
fn to_path_is_inside_from(from_path: &[usize], to_path: &[usize]) -> bool {
  to_path.len() > from_path.len() && to_path[..from_path.len()] == *from_path
}

/// After inserting at `to_path` with `operation`, compute the adjusted source path
/// (the source index may shift if insertion happened at a sibling position before it).
fn compute_adjusted_from_path(from_path: &[usize], to_path: &[usize], operation: &str) -> Vec<usize> {
  let mut adjusted = from_path.to_vec();
  // Only insert-before and insert-after affect sibling indices
  if operation != "insert-before" && operation != "insert-after" {
    return adjusted;
  }
  // Must be the same depth and same parent
  if from_path.len() != to_path.len() {
    return adjusted;
  }
  let parent_depth = from_path.len() - 1;
  if from_path[..parent_depth] != to_path[..parent_depth] {
    return adjusted;
  }
  let from_idx = from_path[parent_depth];
  let to_idx = to_path[parent_depth];
  // Effective insertion position
  let insert_pos = if operation == "insert-before" { to_idx } else { to_idx + 1 };
  if insert_pos <= from_idx {
    adjusted[parent_depth] += 1;
  }
  adjusted
}

fn map_at_to_operation(at: &str) -> Result<&'static str, String> {
  match at {
    "before" => Ok("insert-before"),
    "after" => Ok("insert-after"),
    "prepend-child" => Ok("insert-child"),
    "append-child" => Ok("append-child"),
    "replace" => Ok("replace"),
    other => Err(format!(
      "Unsupported position '{other}'. Use: before, after, prepend-child, append-child, replace"
    )),
  }
}

fn handle_cp_node(opts: &EditCpCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;
  let from_path = parse_path(&opts.from)?;
  let to_path = parse_path(&opts.path)?;

  let operation = map_at_to_operation(&opts.at)?;

  let mut snapshot = load_snapshot(snapshot_file)?;
  check_ns_editable(&snapshot, namespace)?;

  let file_data = snapshot
    .files
    .get_mut(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let resolved_definition =
    resolve_definition_lookup(namespace, definition, file_data.defs.keys().map(|name| name.as_str()), false)?.resolved;

  let code_entry = file_data
    .defs
    .get_mut(resolved_definition.as_str())
    .expect("resolved definition exists");

  let source_node = navigate_to_path(&code_entry.code, &from_path)?.clone();
  let new_code = apply_operation_at_path(&code_entry.code, &to_path, operation, Some(&source_node))?;
  code_entry.code = new_code;

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Copied node from [{}] to [{}] ({}) in '{}/{}'",
    "✓".green(),
    opts.from,
    opts.path,
    opts.at,
    namespace,
    resolved_definition
  );
  Ok(())
}

fn handle_mv_node(opts: &EditMvNodeCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;
  let from_path = parse_path(&opts.from)?;
  let to_path = parse_path(&opts.path)?;

  if from_path.is_empty() {
    return Err("Cannot move root node".to_string());
  }
  if from_path == to_path {
    return Err("Source and destination paths are identical; nothing to move.".to_string());
  }
  if to_path_is_inside_from(&from_path, &to_path) {
    return Err(format!(
      "Cannot move node at [{}] into its own subtree at [{}]",
      opts.from, opts.path
    ));
  }

  let operation = map_at_to_operation(&opts.at)?;

  let mut snapshot = load_snapshot(snapshot_file)?;
  check_ns_editable(&snapshot, namespace)?;

  let file_data = snapshot
    .files
    .get_mut(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let resolved_definition =
    resolve_definition_lookup(namespace, definition, file_data.defs.keys().map(|name| name.as_str()), false)?.resolved;

  let code_entry = file_data
    .defs
    .get_mut(resolved_definition.as_str())
    .expect("resolved definition exists");

  // Step 1: read source node
  let source_node = navigate_to_path(&code_entry.code, &from_path)?.clone();
  let append_index = if operation == "append-child" {
    match navigate_to_path(&code_entry.code, &to_path)? {
      Cirru::List(children) => children.len(),
      Cirru::Leaf(_) => 0,
    }
  } else {
    0
  };

  // Step 2: insert at destination
  let after_insert = apply_operation_at_path(&code_entry.code, &to_path, operation, Some(&source_node))?;

  // Step 3: compute adjusted source path (insertion may have shifted sibling indices)
  let adjusted_from = compute_adjusted_from_path(&from_path, &to_path, operation);

  // Step 4: delete source at adjusted path
  let final_code = apply_operation_at_path(&after_insert, &adjusted_from, "delete", None)?;
  code_entry.code = final_code;

  save_snapshot(&snapshot, snapshot_file)?;

  maintain_cursor_after_node_move(
    snapshot_file,
    &format!("{namespace}/{resolved_definition}"),
    &from_path,
    &to_path,
    operation,
    append_index,
  )?;

  println!(
    "{} Moved node from [{}] to [{}] ({}) in '{}/{}'",
    "✓".green(),
    opts.from,
    opts.path,
    opts.at,
    namespace,
    resolved_definition
  );
  Ok(())
}

fn rename_definition_declaration(code: &Cirru, old_name: &str, new_name: &str) -> Result<(Cirru, bool), String> {
  let Cirru::List(items) = code else {
    return Ok((code.clone(), false));
  };
  let Some(Cirru::Leaf(head)) = items.first() else {
    return Ok((code.clone(), false));
  };
  if !head.starts_with("def") {
    return Ok((code.clone(), false));
  }

  let Some(name_node) = items.get(1) else {
    return Err(format!("Definition form `{head}` has no declaration name"));
  };
  let Cirru::Leaf(declared_name) = name_node else {
    return Err(format!("Definition form `{head}` has a non-leaf declaration name"));
  };
  if declared_name.as_ref() != old_name {
    return Err(format!(
      "Definition key `{old_name}` does not match declaration name `{declared_name}`. Repair the mismatch before renaming."
    ));
  }

  let mut next_items = items.clone();
  next_items[1] = Cirru::Leaf(Arc::from(new_name));
  Ok((Cirru::List(next_items), true))
}

fn handle_rename(opts: &EditRenameCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.source)?;

  let mut snapshot = load_snapshot(snapshot_file)?;
  check_ns_editable(&snapshot, namespace)?;

  let file_data = snapshot
    .files
    .get_mut(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let resolved_definition =
    resolve_definition_lookup(namespace, definition, file_data.defs.keys().map(|name| name.as_str()), false)?.resolved;
  if file_data.defs.contains_key(&opts.new_name) {
    return Err(format!(
      "Definition '{}' already exists in namespace '{}'. Use 'cr edit mv-def' to move to a different namespace.",
      opts.new_name, namespace
    ));
  }

  let mut entry = file_data
    .defs
    .remove(resolved_definition.as_str())
    .expect("resolved definition exists");
  (entry.code, _) = rename_definition_declaration(&entry.code, &resolved_definition, &opts.new_name)?;
  file_data.defs.insert(opts.new_name.clone(), entry);

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Renamed '{}' to '{}' in namespace '{}'",
    "✓".green(),
    resolved_definition.cyan(),
    opts.new_name.cyan(),
    namespace
  );

  Ok(())
}

fn handle_split_def(opts: &EditSplitDefCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;
  let path = parse_path(&opts.path)?;
  let new_name = opts.new_name.trim();

  if path.is_empty() {
    return Err(
      "Cannot split at the root path: the root IS the definition. Use 'cr edit def' to create a new definition from scratch."
        .to_string(),
    );
  }
  if new_name.is_empty() {
    return Err("New definition name cannot be empty".to_string());
  }

  let mut snapshot = load_snapshot(snapshot_file)?;
  check_ns_editable(&snapshot, namespace)?;

  let file_data = snapshot
    .files
    .get_mut(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let resolved_definition =
    resolve_definition_lookup(namespace, definition, file_data.defs.keys().map(|name| name.as_str()), false)?.resolved;
  if file_data.defs.contains_key(new_name) {
    return Err(format!(
      "Definition '{new_name}' already exists in namespace '{namespace}'. Choose a different name or remove the existing one first."
    ));
  }

  // Extract the sub-expression at path
  let extracted = navigate_to_path(&file_data.defs[resolved_definition.as_str()].code, &path)?.clone();

  // Replace the path in the original definition with the new name (a leaf)
  let new_ref = Cirru::Leaf(Arc::from(new_name));
  let updated_code = apply_operation_at_path(&file_data.defs[resolved_definition.as_str()].code, &path, "replace", Some(&new_ref))?;

  // Write updated code back to original definition
  file_data
    .defs
    .get_mut(resolved_definition.as_str())
    .expect("resolved definition exists")
    .code = updated_code;

  // Create the new definition with the extracted sub-expression as its body
  let new_entry = CodeEntry::from_code(extracted);
  file_data.defs.insert(new_name.to_string(), new_entry);

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Extracted node at [{}] from '{}/{}' → new definition '{}'",
    "✓".green(),
    opts.path,
    namespace,
    resolved_definition.cyan(),
    new_name.cyan()
  );
  if command_guidance_enabled() {
    println!();
    println!("{}", "Next steps:".blue().bold());
    println!("  • Inspect new def:  {} '{}/{}'", "cr query def".cyan(), namespace, new_name);
    println!(
      "  • Inspect source:   {} '{}/{}'",
      "cr query def".cyan(),
      namespace,
      resolved_definition
    );
    println!(
      "  • Wrap in defn:     {} '{}/{}' --path '' --code 'quote (defn {} ...)'",
      "cr tree replace".cyan(),
      namespace,
      new_name,
      new_name
    );
  }
  Ok(())
}

fn handle_rm_def(opts: &EditRmDefCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;

  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace can be edited
  check_ns_editable(&snapshot, namespace)?;

  let file_data = snapshot
    .files
    .get_mut(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let resolved_definition =
    resolve_definition_lookup(namespace, definition, file_data.defs.keys().map(|name| name.as_str()), false)?.resolved;
  file_data
    .defs
    .remove(resolved_definition.as_str())
    .expect("resolved definition exists");

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Deleted definition '{}' from namespace '{}'",
    "✓".green(),
    resolved_definition.cyan(),
    namespace
  );

  Ok(())
}

fn handle_mv_def(opts: &EditMvDefCommand, snapshot_file: &str) -> Result<(), String> {
  let (source_ns, source_def) = parse_target(&opts.source)?;
  let (target_ns, target_def) = parse_target(&opts.target)?;

  let mut snapshot = load_snapshot(snapshot_file)?;

  check_ns_editable(&snapshot, source_ns)?;
  check_ns_editable(&snapshot, target_ns)?;

  let resolved_source_def = {
    let source_file = snapshot
      .files
      .get(source_ns)
      .ok_or_else(|| format!("Namespace '{source_ns}' not found"))?;
    resolve_definition_lookup(source_ns, source_def, source_file.defs.keys().map(|name| name.as_str()), false)?.resolved
  };

  if source_ns == target_ns && resolved_source_def == target_def {
    return Err("Source and target are identical; nothing to move.".to_string());
  }

  if source_ns == target_ns {
    let file_data = snapshot
      .files
      .get_mut(source_ns)
      .ok_or_else(|| format!("Namespace '{source_ns}' not found"))?;

    if file_data.defs.contains_key(target_def) {
      return Err(format!("Definition '{target_def}' already exists in namespace '{source_ns}'"));
    }

    let mut entry = file_data
      .defs
      .remove(resolved_source_def.as_str())
      .expect("resolved definition exists");
    (entry.code, _) = rename_definition_declaration(&entry.code, &resolved_source_def, target_def)?;
    file_data.defs.insert(target_def.to_string(), entry);
  } else {
    let target_exists = snapshot
      .files
      .get(target_ns)
      .ok_or_else(|| format!("Namespace '{target_ns}' not found"))?
      .defs
      .contains_key(target_def);
    if target_exists {
      return Err(format!("Definition '{target_def}' already exists in namespace '{target_ns}'"));
    }

    let mut entry = {
      let source_file = snapshot
        .files
        .get_mut(source_ns)
        .ok_or_else(|| format!("Namespace '{source_ns}' not found"))?;
      source_file
        .defs
        .remove(resolved_source_def.as_str())
        .expect("resolved definition exists")
    };
    (entry.code, _) = rename_definition_declaration(&entry.code, &resolved_source_def, target_def)?;

    let target_file = snapshot
      .files
      .get_mut(target_ns)
      .ok_or_else(|| format!("Namespace '{target_ns}' not found"))?;
    target_file.defs.insert(target_def.to_string(), entry);
  }

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Moved definition '{}' from '{}' to '{}'",
    "✓".green(),
    resolved_source_def.cyan(),
    source_ns.cyan(),
    format!("{target_ns}/{target_def}").cyan()
  );

  Ok(())
}

fn handle_doc(opts: &EditDocCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;

  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace can be edited
  check_ns_editable(&snapshot, namespace)?;

  let file_data = snapshot
    .files
    .get_mut(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let resolved_definition =
    resolve_definition_lookup(namespace, definition, file_data.defs.keys().map(|name| name.as_str()), false)?.resolved;

  let code_entry = file_data
    .defs
    .get_mut(resolved_definition.as_str())
    .expect("resolved definition exists");

  code_entry.doc = opts.doc.clone();

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Updated documentation for '{}' in namespace '{}'",
    "✓".green(),
    resolved_definition.cyan(),
    namespace
  );

  Ok(())
}

fn parse_schema_input(raw: &str) -> Result<Cirru, String> {
  parse_input_to_cirru(raw).map_err(|error| {
    format!("Failed to parse schema code input: {error}\nSchema examples: `quote :string` or `quote $ :: :ref :bool`.")
  })
}

fn strip_name_field_from_schema(schema: Cirru) -> Cirru {
  match schema {
    Cirru::List(items) => {
      if items.is_empty() {
        return Cirru::List(items);
      }

      if let Some(Cirru::Leaf(head)) = items.first() {
        if &**head == ":optional" && items.len() == 2 {
          return Cirru::List(vec![items[0].clone(), strip_name_field_from_schema(items[1].clone())]);
        }
        if &**head == "::" && items.len() == 3 && matches!(items.get(1), Some(Cirru::Leaf(tag)) if &**tag == ":optional") {
          return Cirru::List(vec![
            items[0].clone(),
            items[1].clone(),
            strip_name_field_from_schema(items[2].clone()),
          ]);
        }

        if &**head == "{}" {
          let mut next_items = vec![items[0].clone()];
          for pair in items.iter().skip(1) {
            if let Cirru::List(xs) = pair
              && xs.len() == 2
              && matches!(xs.first(), Some(Cirru::Leaf(key)) if &**key == ":name")
            {
              continue;
            }
            next_items.push(pair.clone());
          }
          return Cirru::List(next_items);
        }

        if &**head == "&{}" {
          let mut next_items = vec![items[0].clone()];
          let mut idx = 1usize;
          while idx < items.len() {
            if idx + 1 < items.len() && matches!(&items[idx], Cirru::Leaf(key) if &**key == ":name") {
              idx += 2;
              continue;
            }
            next_items.push(items[idx].clone());
            idx += 1;
          }
          return Cirru::List(next_items);
        }
      }

      Cirru::List(items)
    }
    other => other,
  }
}

fn handle_schema(opts: &EditSchemaCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;

  let mut snapshot = load_snapshot(snapshot_file)?;
  check_ns_editable(&snapshot, namespace)?;

  let file_data = snapshot
    .files
    .get_mut(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let resolved_definition =
    resolve_definition_lookup(namespace, definition, file_data.defs.keys().map(|name| name.as_str()), false)?.resolved;

  let code_entry = file_data
    .defs
    .get_mut(resolved_definition.as_str())
    .expect("resolved definition exists");

  if opts.clear {
    code_entry.schema = DYNAMIC_TYPE.clone();
    save_snapshot(&snapshot, snapshot_file)?;
    println!(
      "{} Cleared schema for '{}' in namespace '{}'",
      "✓".green(),
      resolved_definition.cyan(),
      namespace
    );
    return Ok(());
  }

  let raw = read_code_input(&opts.file, &opts.code)?.ok_or(ERR_CODE_INPUT_REQUIRED)?;
  let schema_payload = strip_name_field_from_schema(parse_schema_input(&raw)?);

  validate_schema_for_write(&schema_payload).map_err(|e| format!("Schema validation failed: {e}"))?;

  // Primitive type tag leaf (e.g. --code 'quote :string') — store directly without going through fn-schema parsing.
  if let Cirru::Leaf(tag) = &schema_payload {
    let tag_name = tag.trim_start_matches(':');
    code_entry.schema = Arc::new(CalcitTypeAnnotation::from_tag_name(tag_name));
    save_snapshot(&snapshot, snapshot_file)?;
    println!(
      "{} Updated schema for '{}' in namespace '{}'",
      "✓".green(),
      resolved_definition.cyan(),
      namespace
    );
    return Ok(());
  }

  snapshot::parse_schema_data(&schema_payload)?;
  let schema_edn = snapshot::schema_cirru_to_edn(schema_payload);
  code_entry.schema = CalcitTypeAnnotation::parse_fn_schema_from_edn(&schema_edn)
    .map(|s| Arc::new(CalcitTypeAnnotation::Fn(Arc::new(s))))
    .unwrap_or_else(|| CalcitTypeAnnotation::parse_type_annotation_from_edn(&schema_edn));

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Updated schema for '{}' in namespace '{}'",
    "✓".green(),
    resolved_definition.cyan(),
    namespace
  );

  Ok(())
}

fn parse_examples_input(raw: &str) -> Result<Vec<Cirru>, String> {
  parse_quoted_cirru_nodes(raw).map_err(|error| {
    format!(
      "Failed to parse examples: {error}\nEach top-level example needs its own `quote`, so both leaves and expressions remain representable."
    )
  })
}

fn handle_examples(opts: &EditExamplesCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;

  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace can be edited
  check_ns_editable(&snapshot, namespace)?;

  let file_data = snapshot
    .files
    .get_mut(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let resolved_definition =
    resolve_definition_lookup(namespace, definition, file_data.defs.keys().map(|name| name.as_str()), false)?.resolved;

  let code_entry = file_data
    .defs
    .get_mut(resolved_definition.as_str())
    .expect("resolved definition exists");

  // Handle --clear flag
  if opts.clear {
    let old_count = code_entry.examples.len();
    code_entry.examples.clear();
    save_snapshot(&snapshot, snapshot_file)?;
    println!(
      "{} Cleared {} example(s) for '{}' in namespace '{}'",
      "✓".green(),
      old_count,
      resolved_definition.cyan(),
      namespace
    );
    return Ok(());
  }

  // Read examples input
  let code_input = read_code_input(&opts.file, &opts.code)?;
  let raw = code_input
    .as_deref()
    .ok_or("Examples input required: use --file, --code, or pipe input via stdin")?;

  // Each top-level quote contributes exactly one example. Requiring a quote
  // per item preserves the leaf/expression distinction in batch input.
  let examples = parse_examples_input(raw)?;

  let count = examples.len();
  code_entry.examples = examples;

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Set {} example(s) for '{}' in namespace '{}'",
    "✓".green(),
    count,
    resolved_definition.cyan(),
    namespace
  );

  Ok(())
}

fn handle_add_example(opts: &EditAddExampleCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;

  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace can be edited
  check_ns_editable(&snapshot, namespace)?;

  let file_data = snapshot
    .files
    .get_mut(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let resolved_definition =
    resolve_definition_lookup(namespace, definition, file_data.defs.keys().map(|name| name.as_str()), false)?.resolved;

  let code_entry = file_data
    .defs
    .get_mut(resolved_definition.as_str())
    .expect("resolved definition exists");

  // Read example input
  let code_input = read_code_input(&opts.file, &opts.code)?;
  let raw = code_input
    .as_deref()
    .ok_or("Example input required: use --file, --code, or pipe via stdin")?;

  // Parse example
  let example: Cirru = parse_input_to_cirru(raw)?;

  // Insert at specified position or append
  let position = opts.at.unwrap_or(code_entry.examples.len());
  if position > code_entry.examples.len() {
    return Err(format!("Position {} out of range (max: {})", position, code_entry.examples.len()));
  }

  code_entry.examples.insert(position, example);

  let total_count = code_entry.examples.len();

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Added example at position {} for '{}' in namespace '{}' (total: {})",
    "✓".green(),
    position,
    resolved_definition.cyan(),
    namespace,
    total_count
  );

  Ok(())
}

fn parse_tag_token(raw: &str) -> Result<EdnTag, String> {
  let trimmed = raw.trim();
  if trimmed.is_empty() {
    return Err("empty tag".to_string());
  }
  let name = trimmed.strip_prefix(':').unwrap_or(trimmed);
  if name.is_empty() {
    return Err(format!("invalid tag: {raw}"));
  }
  Ok(EdnTag::new(name))
}

fn parse_tags_csv(raw: &str) -> Result<HashSet<EdnTag>, String> {
  let trimmed = raw.trim();
  if trimmed.is_empty() {
    return Ok(HashSet::new());
  }
  let mut tags = HashSet::new();
  for token in trimmed.split(',') {
    let piece = token.trim();
    if piece.is_empty() {
      return Err("tags must be comma-separated without empty items".to_string());
    }
    tags.insert(parse_tag_token(piece)?);
  }
  Ok(tags)
}

fn get_code_entry<'a>(snapshot: &'a Snapshot, namespace: &str, definition: &str) -> Result<(String, &'a CodeEntry), String> {
  let file_data = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;
  let resolved_definition =
    resolve_definition_lookup(namespace, definition, file_data.defs.keys().map(|name| name.as_str()), false)?.resolved;
  let code_entry = file_data
    .defs
    .get(resolved_definition.as_str())
    .ok_or_else(|| format!("Definition '{resolved_definition}' not found in namespace '{namespace}'"))?;
  Ok((resolved_definition, code_entry))
}

fn get_code_entry_mut<'a>(
  snapshot: &'a mut Snapshot,
  namespace: &str,
  definition: &str,
) -> Result<(String, &'a mut CodeEntry), String> {
  check_ns_editable(snapshot, namespace)?;
  let file_data = snapshot
    .files
    .get_mut(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;
  let resolved_definition =
    resolve_definition_lookup(namespace, definition, file_data.defs.keys().map(|name| name.as_str()), false)?.resolved;
  let code_entry = file_data
    .defs
    .get_mut(resolved_definition.as_str())
    .ok_or_else(|| format!("Definition '{resolved_definition}' not found in namespace '{namespace}'"))?;
  Ok((resolved_definition, code_entry))
}

fn format_tags_csv(tags: &HashSet<EdnTag>) -> String {
  let mut items: Vec<String> = tags.iter().map(|tag| format!(":{}", tag.ref_str())).collect();
  items.sort();
  items.join(",")
}

fn handle_tags(opts: &EditTagsCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;

  if opts.tags.is_none() {
    let snapshot = load_snapshot(snapshot_file)?;
    let (resolved_definition, code_entry) = get_code_entry(&snapshot, namespace, definition)?;
    let tags_text = format_tags_csv(&code_entry.tags);
    if tags_text.is_empty() {
      println!("{namespace}/{resolved_definition}: (none)");
    } else {
      println!("{namespace}/{resolved_definition}: {tags_text}");
    }
    return Ok(());
  }

  let mut snapshot = load_snapshot(snapshot_file)?;
  let (resolved_definition, code_entry) = get_code_entry_mut(&mut snapshot, namespace, definition)?;
  let tags = parse_tags_csv(opts.tags.as_deref().unwrap_or(""))?;
  let cleared = tags.is_empty();
  let tags_summary = format_tags_csv(&tags);
  code_entry.tags = tags;
  save_snapshot(&snapshot, snapshot_file)?;

  if cleared {
    println!(
      "{} Cleared tags for '{}' in namespace '{}'",
      "✓".green(),
      resolved_definition.cyan(),
      namespace
    );
  } else {
    println!(
      "{} Set tags for '{}' in namespace '{}': {tags_summary}",
      "✓".green(),
      resolved_definition.cyan(),
      namespace
    );
  }

  Ok(())
}

fn handle_rm_example(opts: &EditRmExampleCommand, snapshot_file: &str) -> Result<(), String> {
  let (namespace, definition) = parse_target(&opts.target)?;

  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace can be edited
  check_ns_editable(&snapshot, namespace)?;

  let file_data = snapshot
    .files
    .get_mut(namespace)
    .ok_or_else(|| format!("Namespace '{namespace}' not found"))?;

  let resolved_definition =
    resolve_definition_lookup(namespace, definition, file_data.defs.keys().map(|name| name.as_str()), false)?.resolved;

  let code_entry = file_data
    .defs
    .get_mut(resolved_definition.as_str())
    .expect("resolved definition exists");

  // Validate index
  if opts.index >= code_entry.examples.len() {
    return Err(format!(
      "Index {} out of range (max: {})",
      opts.index,
      code_entry.examples.len().saturating_sub(1)
    ));
  }

  // Remove example
  code_entry.examples.remove(opts.index);

  let remaining_count = code_entry.examples.len();

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Removed example at index {} from '{}' in namespace '{}' (remaining: {})",
    "✓".green(),
    opts.index,
    resolved_definition.cyan(),
    namespace,
    remaining_count
  );

  Ok(())
}

pub(crate) fn apply_operation_at_path(
  code: &Cirru,
  path: &[usize],
  operation: &str,
  new_node: Option<&Cirru>,
) -> Result<Cirru, String> {
  if path.is_empty() {
    // Operating on root
    return match operation {
      "replace" => {
        let node = new_node.ok_or("Code input required for replace operation")?;
        Ok(node.clone())
      }
      "delete" => Err("Cannot delete root node".to_string()),
      _ => Err(format!("Operation '{operation}' not supported at root level")),
    };
  }

  // Navigate to parent and operate on child
  apply_operation_recursive(code, path, 0, operation, new_node)
}

fn apply_operation_recursive(
  code: &Cirru,
  path: &[usize],
  depth: usize,
  operation: &str,
  new_node: Option<&Cirru>,
) -> Result<Cirru, String> {
  match code {
    Cirru::Leaf(_) => Err(format!("Cannot navigate into leaf node at depth {depth}")),
    Cirru::List(items) => {
      let idx = path[depth];
      if idx >= items.len() {
        return Err(format!("Path index {} out of bounds (list has {} items)", idx, items.len()));
      }

      if depth == path.len() - 1 {
        // At target position, apply operation
        let mut new_items = items.clone();

        match operation {
          "delete" => {
            new_items.remove(idx);
          }
          "replace" => {
            let newn = new_node.ok_or("Code input required for replace operation")?;
            new_items[idx] = newn.clone();
          }
          "insert-before" => {
            let newn = new_node.ok_or("Code input required for insert-before operation")?;
            new_items.insert(idx, newn.clone());
          }
          "insert-after" => {
            let newn = new_node.ok_or("Code input required for insert-after operation")?;
            new_items.insert(idx + 1, newn.clone());
          }
          "insert-child" => {
            // Insert as first child of the node at idx
            let newn = new_node.ok_or("Code input required for insert-child operation")?;
            match &new_items[idx] {
              Cirru::List(children) => {
                let mut new_children = vec![newn.clone()];
                new_children.extend(children.clone());
                new_items[idx] = Cirru::List(new_children);
              }
              Cirru::Leaf(_) => {
                return Err("Cannot insert child into leaf node".to_string());
              }
            }
          }
          "append-child" => {
            // Insert as last child of the node at idx
            let newn = new_node.ok_or("Code input required for append-child operation")?;
            match &new_items[idx] {
              Cirru::List(children) => {
                let mut new_children = children.clone();
                new_children.push(newn.clone());
                new_items[idx] = Cirru::List(new_children);
              }
              Cirru::Leaf(_) => {
                return Err("Cannot append child to leaf node".to_string());
              }
            }
          }
          "swap-next-sibling" => {
            // Swap current node with next sibling
            if idx + 1 >= new_items.len() {
              return Err(format!("Cannot swap: no next sibling at index {idx}"));
            }
            new_items.swap(idx, idx + 1);
          }
          "swap-prev-sibling" => {
            // Swap current node with previous sibling
            if idx == 0 {
              return Err("Cannot swap: no previous sibling at index 0".to_string());
            }
            new_items.swap(idx - 1, idx);
          }
          _ => {
            return Err(format!("Unknown operation: {operation}"));
          }
        }

        Ok(Cirru::List(new_items))
      } else {
        // Continue navigating
        let mut new_items = items.clone();
        new_items[idx] = apply_operation_recursive(&items[idx], path, depth + 1, operation, new_node)?;
        Ok(Cirru::List(new_items))
      }
    }
  }
}

pub(crate) fn navigate_to_path(code: &Cirru, path: &[usize]) -> Result<Cirru, String> {
  if path.is_empty() {
    return Ok(code.clone());
  }

  let mut current = code;
  for (depth, &idx) in path.iter().enumerate() {
    match current {
      Cirru::Leaf(_) => {
        let partial = format_path(&path[..depth]);
        return Err(format!(
          "Cannot navigate into leaf node at depth {depth}\n   Valid path stops at: {}\n   Tip: Use 'cr tree show --path {}' to explore the tree structure (use dot-separated indices, e.g. '@2.1.0')",
          format_path(&path[..depth]),
          partial,
        ));
      }
      Cirru::List(items) => {
        if idx >= items.len() {
          let partial = format_path(&path[..depth]);
          return Err(format!(
            "Path index {} out of bounds at depth {} (list has {} items)\n   Attempted path: {}\n   Valid path up to: {}\n   Valid index range at this level: 0-{}\n   Tip: Use 'cr tree show --path {}' to see available children",
            idx,
            depth,
            items.len(),
            format_path(path),
            format_path(&path[..depth]),
            items.len().saturating_sub(1),
            partial,
          ));
        }
        current = &items[idx];
      }
    }
  }

  Ok(current.clone())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Namespace operations
// ═══════════════════════════════════════════════════════════════════════════════

fn handle_add_ns(opts: &EditAddNsCommand, snapshot_file: &str) -> Result<(), String> {
  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace can be edited (must be under current package)
  check_ns_editable(&snapshot, &opts.namespace)?;

  if snapshot.files.contains_key(&opts.namespace) {
    return Err(format!("Namespace '{}' already exists", opts.namespace));
  }

  // Create ns code
  let ns_code = if let Some(raw) = read_code_input(&opts.file, &opts.code)? {
    let code = parse_input_to_cirru(&raw)?;
    // Validate: if input looks like a `ns` expression, the name inside must match
    if let Cirru::List(ref items) = code
      && let Some(Cirru::Leaf(kw)) = items.first()
      && kw.as_ref() == "ns"
      && let Some(Cirru::Leaf(ns_in_expr)) = items.get(1)
      && ns_in_expr.as_ref() != opts.namespace.as_str()
    {
      return Err(format!(
        "Namespace name mismatch: positional argument is '{}' but ns expression contains '{}'. They must be identical.",
        opts.namespace, ns_in_expr
      ));
    }
    code
  } else {
    // Default minimal ns declaration: (ns namespace-name)
    Cirru::List(vec![Cirru::Leaf(Arc::from("ns")), Cirru::Leaf(Arc::from(opts.namespace.as_str()))])
  };

  let file_entry = FileInSnapShot {
    ns: NsEntry {
      doc: String::new(),
      code: ns_code,
    },
    defs: HashMap::new(),
  };

  snapshot.files.insert(opts.namespace.clone(), file_entry);

  save_snapshot(&snapshot, snapshot_file)?;

  println!("{} Created namespace '{}'", "✓".green(), opts.namespace.cyan());

  Ok(())
}

fn handle_rm_ns(opts: &EditRmNsCommand, snapshot_file: &str) -> Result<(), String> {
  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace can be edited
  check_ns_editable(&snapshot, &opts.namespace)?;

  if snapshot.files.remove(&opts.namespace).is_none() {
    return Err(format!("Namespace '{}' not found", opts.namespace));
  }

  save_snapshot(&snapshot, snapshot_file)?;

  println!("{} Deleted namespace '{}'", "✓".green(), opts.namespace.cyan());

  Ok(())
}

fn handle_imports(opts: &EditImportsCommand, snapshot_file: &str) -> Result<(), String> {
  let raw = read_code_input(&opts.file, &opts.code)?.ok_or("Imports input required: use --file, --code, or pipe via stdin")?;

  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace can be edited
  check_ns_editable(&snapshot, &opts.namespace)?;

  let file_data = snapshot
    .files
    .get_mut(&opts.namespace)
    .ok_or_else(|| format!("Namespace '{}' not found", opts.namespace))?;

  // Determine input format: auto-detect JSON array vs Cirru EDN
  let imports_json: serde_json::Value = if raw.trim().starts_with('[') {
    serde_json::from_str(&raw).map_err(|e| format!("Failed to parse imports JSON: {e}"))?
  } else {
    // Parse as cirru and convert to JSON value
    let cirru_node = parse_input_to_cirru(&raw)?;
    use super::common::cirru_to_json_value;
    cirru_to_json_value(&cirru_node)
  };

  // Build new ns code with imports.
  // Always use build_ns_code to produce the correct nested structure:
  //   ["ns", "namespace", [":require", rule1, rule2, ...]]
  let ns_name = &opts.namespace;

  let rules: Vec<Cirru> = if let serde_json::Value::Array(ref elems) = imports_json {
    use super::common::json_value_to_cirru;
    if elems.is_empty() {
      vec![]
    } else {
      // Detect: array-of-arrays => multiple rules; array-of-strings => single flat rule
      let first_is_array = elems.first().map(|e| e.is_array()).unwrap_or(false);
      if first_is_array {
        // Each element is one import rule
        elems.iter().map(json_value_to_cirru).collect::<Result<Vec<_>, _>>()?
      } else {
        // The whole array is a single import rule (e.g. from `-e 'src-ns :refer $ sym'`)
        // Guard: user may have accidentally included ':require' prefix
        if let Some(serde_json::Value::String(first_str)) = elems.first()
          && first_str == ":require"
        {
          return Err(
            "Do not include ':require' as a prefix in the imports input. \
               Pass rules directly, e.g. --code 'src-ns :refer $ sym' or use --file for multiple rules."
              .to_string(),
          );
        }
        vec![json_value_to_cirru(&imports_json)?]
      }
    }
  } else {
    return Err("Imports must be a Cirru list or JSON array of import rules.".to_string());
  };

  for warning in validate_import_rules(&rules)? {
    eprintln!("{} in namespace '{}': {warning}", "Warning:".yellow(), opts.namespace);
  }

  // Extract old imports for comparison
  let old_imports = extract_require_list(&file_data.ns.code);

  file_data.ns.code = build_ns_code(ns_name, &rules);

  // Extract new imports
  let new_imports = extract_require_list(&file_data.ns.code);

  save_snapshot(&snapshot, snapshot_file)?;

  // Show what changed
  println!("{} Updated imports for namespace '{}'", "✓".green(), opts.namespace.cyan());

  // Show removed imports
  let removed: Vec<_> = old_imports.iter().filter(|old| !new_imports.contains(old)).collect();
  if !removed.is_empty() {
    println!("  {} Removed:", "-".red());
    for import in removed {
      println!("    {}", import.dimmed());
    }
  }

  // Show added imports
  let added: Vec<_> = new_imports.iter().filter(|new| !old_imports.contains(new)).collect();
  let mut added_namespaces = Vec::new();
  if !added.is_empty() {
    println!("  {} Added:", "+".green());
    for import in &added {
      println!("    {import}");
      // Extract namespace from import (first token before :refer or :as)
      if let Some(first_token) = import.split_whitespace().next() {
        if first_token.starts_with('(') {
          if let Some(ns) = import.split_whitespace().next().and_then(|s| s.strip_prefix('(')) {
            added_namespaces.push(ns.to_string());
          }
        } else {
          added_namespaces.push(first_token.to_string());
        }
      }
    }
  }

  // Show unchanged count if there are any
  let unchanged_count = old_imports.iter().filter(|old| new_imports.contains(old)).count();
  if unchanged_count > 0 {
    println!("  {} {} unchanged", "·".dimmed(), format!("{unchanged_count}").dimmed());
  }

  // Show detailed tips for newly added imports
  if !added.is_empty() {
    println!();
    println!("{}", "Usage tips for new imports:".dimmed());

    // Parse each added import string to provide tips
    for added_str in &added {
      // Parse the import string back to Cirru to analyze it
      if let Ok(parsed) = cirru_parser::parse(added_str)
        && let Some(rule) = parsed.first()
        && let Some(source_ns) = get_require_source_ns(rule)
      {
        print_import_usage_tips(rule, &source_ns);
      }
    }
  }

  Ok(())
}

/// Extract formatted import list from ns code for comparison
fn extract_require_list(ns_code: &Cirru) -> Vec<String> {
  let mut imports = Vec::new();

  if let Cirru::List(items) = ns_code {
    let mut in_require = false;
    for item in items {
      if let Cirru::Leaf(s) = item
        && s.as_ref() == ":require"
      {
        in_require = true;
        continue;
      }
      if in_require {
        // Format each import as one-liner
        if let Ok(formatted) = item.format_one_liner() {
          imports.push(formatted);
        }
      }
    }
  }

  imports
}

/// Extract the source namespace from a require rule
/// e.g. from `(calcit.core :refer ...)` extract `calcit.core`
fn get_require_source_ns(rule: &Cirru) -> Option<String> {
  match rule {
    Cirru::List(items) if !items.is_empty() => match &items[0] {
      Cirru::Leaf(s) => Some(s.to_string()),
      _ => None,
    },
    _ => None,
  }
}

/// Extract existing require rules from ns code
/// Handles structure: ["ns", "namespace", [":require", rule1, rule2, ...]]
fn extract_require_rules(ns_code: &Cirru) -> Vec<Cirru> {
  let mut rules = vec![];
  if let Cirru::List(items) = ns_code {
    for item in items.iter().skip(2) {
      // skip "ns" and namespace name
      if let Cirru::List(inner) = item
        && let Some(Cirru::Leaf(first)) = inner.first()
        && first.as_ref() == ":require"
      {
        // Found [:require rule1 rule2 ...]
        rules.extend(inner.iter().skip(1).cloned());
        break;
      }
    }
  }
  rules
}

/// Build ns code from namespace name and require rules
/// Produces structure: ["ns", "namespace", [":require", rule1, rule2, ...]]
fn build_ns_code(ns_name: &str, rules: &[Cirru]) -> Cirru {
  let mut items = vec![Cirru::Leaf(Arc::from("ns")), Cirru::Leaf(Arc::from(ns_name))];

  if !rules.is_empty() {
    let mut require_list = vec![Cirru::Leaf(Arc::from(":require"))];
    require_list.extend(rules.iter().cloned());
    items.push(Cirru::List(require_list));
  }

  Cirru::List(items)
}

fn handle_add_import(opts: &EditAddImportCommand, snapshot_file: &str) -> Result<(), String> {
  let raw = read_code_input(&opts.file, &opts.code)?.ok_or("Import rule input required: use --file, --code, or pipe via stdin")?;

  let new_rule = parse_input_to_cirru(&raw)?;

  let _ = validate_import_rules(std::slice::from_ref(&new_rule))?;

  // Validate that the rule has a source namespace
  let new_source_ns =
    get_require_source_ns(&new_rule).ok_or("Invalid require rule: first element must be a namespace name (e.g. 'calcit.core')")?;

  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace can be edited
  check_ns_editable(&snapshot, &opts.namespace)?;

  let file_data = snapshot
    .files
    .get_mut(&opts.namespace)
    .ok_or_else(|| format!("Namespace '{}' not found", opts.namespace))?;

  // Extract existing rules
  let mut rules = extract_require_rules(&file_data.ns.code);

  // Check if rule for this source namespace already exists
  let existing_idx = rules
    .iter()
    .position(|r| get_require_source_ns(r).as_deref() == Some(&new_source_ns));

  let replaced = if let Some(idx) = existing_idx {
    if opts.overwrite {
      rules[idx] = new_rule.clone();
      true
    } else {
      return Err(format!(
        "Require rule for '{}' already exists in namespace '{}'. Use --overwrite to replace.",
        new_source_ns, opts.namespace
      ));
    }
  } else {
    rules.push(new_rule.clone());
    false
  };

  for warning in validate_import_rules(&rules)? {
    eprintln!("{} in namespace '{}': {warning}", "Warning:".yellow(), opts.namespace);
  }

  if replaced {
    println!(
      "{} Replaced require rule for '{}' in namespace '{}'",
      "✓".green(),
      new_source_ns.cyan(),
      opts.namespace
    );
  } else {
    println!(
      "{} Added require rule for '{}' in namespace '{}'",
      "✓".green(),
      new_source_ns.cyan(),
      opts.namespace
    );
  }

  // Rebuild ns code
  file_data.ns.code = build_ns_code(&opts.namespace, &rules);

  save_snapshot(&snapshot, snapshot_file)?;

  // Show usage tips based on import type
  print_import_usage_tips(&new_rule, &new_source_ns);

  Ok(())
}

fn handle_rm_import(opts: &EditRmImportCommand, snapshot_file: &str) -> Result<(), String> {
  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace can be edited
  check_ns_editable(&snapshot, &opts.namespace)?;

  let file_data = snapshot
    .files
    .get_mut(&opts.namespace)
    .ok_or_else(|| format!("Namespace '{}' not found", opts.namespace))?;

  // Extract existing rules
  let mut rules = extract_require_rules(&file_data.ns.code);

  // Find and remove the rule for the specified source namespace
  let original_len = rules.len();
  rules.retain(|r| get_require_source_ns(r).as_deref() != Some(&opts.source_ns));

  if rules.len() == original_len {
    return Err(format!(
      "No require rule found for '{}' in namespace '{}'",
      opts.source_ns, opts.namespace
    ));
  }

  // Rebuild ns code
  file_data.ns.code = build_ns_code(&opts.namespace, &rules);

  save_snapshot(&snapshot, snapshot_file)?;

  println!(
    "{} Removed require rule for '{}' from namespace '{}'",
    "✓".green(),
    opts.source_ns.cyan(),
    opts.namespace
  );

  Ok(())
}

fn handle_ns_doc(opts: &EditNsDocCommand, snapshot_file: &str) -> Result<(), String> {
  let mut snapshot = load_snapshot(snapshot_file)?;

  // Check if namespace can be edited
  check_ns_editable(&snapshot, &opts.namespace)?;

  let file_data = snapshot
    .files
    .get_mut(&opts.namespace)
    .ok_or_else(|| format!("Namespace '{}' not found", opts.namespace))?;

  file_data.ns.doc = opts.doc.clone();

  save_snapshot(&snapshot, snapshot_file)?;

  println!("{} Updated documentation for namespace '{}'", "✓".green(), opts.namespace.cyan());

  Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Semver helpers (pub(crate) so cr config can reuse them)
// ═══════════════════════════════════════════════════════════════════════════════

pub(crate) fn parse_semver_value(v: &str) -> Result<Version, String> {
  if v.starts_with('|') {
    return Err(format!(
      "Invalid version '{v}': do not include the '|' Cirru string prefix; use bare semver, e.g. '0.0.17'"
    ));
  }
  Version::parse(v).map_err(|_| format!("Invalid version '{v}': expected semver format, e.g. '0.0.17'"))
}

pub(crate) fn bump_semver_value(current: &str, level: &str) -> Result<String, String> {
  let mut version = parse_semver_value(current)?;
  match level {
    "patch" => {
      version.patch += 1;
    }
    "minor" => {
      version.minor += 1;
      version.patch = 0;
    }
    "major" => {
      version.major += 1;
      version.minor = 0;
      version.patch = 0;
    }
    _ => {
      return Err(format!("Unknown bump level '{level}'. Valid levels: patch, minor, major"));
    }
  }
  version.pre = semver::Prerelease::EMPTY;
  version.build = semver::BuildMetadata::EMPTY;
  Ok(version.to_string())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Incremental change export
// ═══════════════════════════════════════════════════════════════════════════════

fn handle_inc(opts: &EditIncCommand, snapshot_file: &str) -> Result<(), String> {
  let inc_file = ".compact-inc.cirru";
  let project_directory = calcit::project_state::project_directory_for_snapshot(snapshot_file);
  let error_file = calcit::project_state::state_file(project_directory, calcit::project_state::ERROR_STATE_FILE);
  let legacy_error_file = project_directory.join(".calcit-error.cirru");
  calcit::project_state::migrate_legacy_file(&legacy_error_file, &error_file)
    .map_err(|error| format!("Failed to migrate legacy error file: {error}"))?;
  calcit::project_state::ensure_state_directory(project_directory)
    .map_err(|error| format!("Failed to create project state directory: {error}"))?;

  // Clear error file at the beginning
  if let Err(e) = fs::write(&error_file, "") {
    eprintln!("{} Failed to clear {}: {}", "⚠".yellow(), error_file.display(), e);
  } else {
    println!("{} Cleared {}", "→".cyan(), error_file.display());
  }

  if opts.added.is_empty()
    && opts.removed.is_empty()
    && opts.changed.is_empty()
    && opts.added_ns.is_empty()
    && opts.removed_ns.is_empty()
    && opts.ns_updated.is_empty()
  {
    return Err("No change hints provided. Use --added/--removed/--changed or namespace flags.".to_string());
  }

  let snapshot = load_snapshot(snapshot_file)?;

  let mut changes = ChangesDict::default();
  let mut changed_entries: HashMap<Arc<str>, FileChangeInfo> = HashMap::new();

  for ns in &opts.added_ns {
    check_ns_editable(&snapshot, ns)?;
    let file = snapshot
      .files
      .get(ns)
      .ok_or_else(|| format!("Namespace '{ns}' not found in snapshot. Did you save calcit.cirru (or legacy compact.cirru)?"))?;
    changes.added.insert(Arc::from(ns.as_str()), file.clone());
  }

  for ns in &opts.removed_ns {
    check_ns_editable(&snapshot, ns)?;
    changes.removed.insert(Arc::from(ns.as_str()));
  }

  for ns in &opts.ns_updated {
    check_ns_editable(&snapshot, ns)?;
    let file = snapshot
      .files
      .get(ns)
      .ok_or_else(|| format!("Namespace '{ns}' not found in snapshot. Did you save calcit.cirru (or legacy compact.cirru)?"))?;
    let entry = ensure_change_entry(&mut changed_entries, ns);
    entry.ns = Some(file.ns.code.clone());
  }

  for target in &opts.added {
    let (namespace, definition) = parse_target(target)?;
    check_ns_editable(&snapshot, namespace)?;
    let file = snapshot
      .files
      .get(namespace)
      .ok_or_else(|| format!("Namespace '{namespace}' not found in snapshot"))?;
    let resolved_definition =
      resolve_definition_lookup(namespace, definition, file.defs.keys().map(|name| name.as_str()), false)?.resolved;
    let code_entry = file.defs.get(resolved_definition.as_str()).expect("resolved definition exists");
    let entry = ensure_change_entry(&mut changed_entries, namespace);
    entry.added_defs.insert(resolved_definition, code_entry.code.clone());
  }

  for target in &opts.changed {
    let (namespace, definition) = parse_target(target)?;
    check_ns_editable(&snapshot, namespace)?;
    let file = snapshot
      .files
      .get(namespace)
      .ok_or_else(|| format!("Namespace '{namespace}' not found in snapshot"))?;
    let resolved_definition =
      resolve_definition_lookup(namespace, definition, file.defs.keys().map(|name| name.as_str()), false)?.resolved;
    let code_entry = file.defs.get(resolved_definition.as_str()).expect("resolved definition exists");
    let entry = ensure_change_entry(&mut changed_entries, namespace);
    entry.changed_defs.insert(resolved_definition, code_entry.code.clone());
  }

  for target in &opts.removed {
    let (namespace, definition) = parse_target(target)?;
    check_ns_editable(&snapshot, namespace)?;
    let entry = ensure_change_entry(&mut changed_entries, namespace);
    entry.removed_defs.insert(definition.to_string());
  }

  if !changed_entries.is_empty() {
    changes.changed = changed_entries;
  }

  if changes.added.is_empty() && changes.removed.is_empty() && changes.changed.is_empty() {
    return Err(
      "No change data collected. Confirm the flags match definitions saved in calcit.cirru (or legacy compact.cirru).".to_string(),
    );
  }

  let namespace_total = changes.added.len() + changes.removed.len() + changes.changed.len();

  let edn_data: cirru_edn::Edn = changes
    .try_into()
    .map_err(|e| format!("Failed to serialize change dictionary: {e}"))?;
  let content = cirru_edn::format(&edn_data, true).map_err(|e| format!("Failed to format change dictionary: {e}"))?;

  fs::write(inc_file, &content).map_err(|e| format!("Failed to write {inc_file}: {e}"))?;

  println!(
    "{} Wrote incremental changes (namespaces: {}) to {}",
    "✓".green(),
    namespace_total,
    inc_file.cyan()
  );
  println!(
    "{}",
    "Watcher will process changes. Wait ~300ms then run 'cr query error' to check result."
      .to_string()
      .dimmed()
  );

  Ok(())
}

fn ensure_change_entry<'a>(changed_entries: &'a mut HashMap<Arc<str>, FileChangeInfo>, namespace: &str) -> &'a mut FileChangeInfo {
  let key: Arc<str> = Arc::from(namespace.to_string());
  changed_entries.entry(key).or_insert_with(|| FileChangeInfo {
    ns: None,
    added_defs: HashMap::new(),
    removed_defs: HashSet::new(),
    changed_defs: HashMap::new(),
  })
}

/// Print usage tips based on the import rule type
fn print_import_usage_tips(rule: &Cirru, source_ns: &str) {
  // Analyze the import rule to determine its type
  if let Cirru::List(items) = rule {
    let mut import_type = None;
    let mut symbols = Vec::new();
    let mut alias = None;

    // Parse the import rule: (namespace :refer [symbols...]) or (namespace :as alias) or (namespace :default symbol)
    let mut i = 1; // Skip the namespace (first element)
    while i < items.len() {
      if let Cirru::Leaf(tag) = &items[i] {
        match tag.as_ref() {
          ":refer" => {
            import_type = Some("refer");
            // Next item should be a list of symbols or a single symbol
            if i + 1 < items.len() {
              match &items[i + 1] {
                Cirru::List(syms) => {
                  for sym in syms {
                    if let Cirru::Leaf(s) = sym {
                      symbols.push(s.to_string());
                    }
                  }
                }
                Cirru::Leaf(s) => symbols.push(s.to_string()),
              }
            }
            break;
          }
          ":as" => {
            import_type = Some("as");
            if i + 1 < items.len()
              && let Cirru::Leaf(a) = &items[i + 1]
            {
              alias = Some(a.to_string());
            }
            break;
          }
          ":default" => {
            import_type = Some("default");
            if i + 1 < items.len()
              && let Cirru::Leaf(s) = &items[i + 1]
            {
              symbols.push(s.to_string());
            }
            break;
          }
          _ => {}
        }
      }
      i += 1;
    }

    // Print tips based on import type
    println!();
    println!("{}", "Usage tips:".dimmed());

    match import_type {
      Some("refer") => {
        if symbols.is_empty() {
          println!(
            "  {} Use imported symbols directly: {}",
            "·".dimmed(),
            "(symbol-name ...)".to_string().cyan()
          );
        } else {
          println!("  {} Use imported symbols directly:", "·".dimmed());
          for symbol in symbols.iter().take(3) {
            println!("    {}", format!("({symbol} ...)").cyan());
          }
          if symbols.len() > 3 {
            println!("    {}", format!("... and {} more", symbols.len() - 3).dimmed());
          }
        }
      }
      Some("as") => {
        if let Some(a) = alias {
          println!("  {} Use with alias: {}", "·".dimmed(), format!("({a}/symbol-name ...)").cyan());
          println!(
            "  {} List definitions: {}",
            "·".dimmed(),
            format!("cr query defs {source_ns}").cyan()
          );
        }
      }
      Some("default") => {
        if !symbols.is_empty() {
          println!(
            "  {} Default import available as: {}",
            "·".dimmed(),
            format!("({} ...)", symbols[0]).cyan()
          );
        }
      }
      None => {
        // Plain import without :refer/:as/:default
        println!(
          "  {} Use with full namespace: {}",
          "·".dimmed(),
          format!("({source_ns}/symbol-name ...)").cyan()
        );
        println!(
          "  {} List definitions: {}",
          "·".dimmed(),
          format!("cr query defs {source_ns}").cyan()
        );
      }
      _ => {
        // Unknown import type
        println!(
          "  {} Use with full namespace: {}",
          "·".dimmed(),
          format!("({source_ns}/symbol-name ...)").cyan()
        );
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{
    TransactionOperationReport, bump_semver_value, load_snapshot, parse_examples_input, parse_input_to_cirru, parse_schema_input,
    parse_transaction_operations, rename_definition_declaration, run_staged_transaction_with, save_snapshot,
  };
  use cirru_parser::Cirru;
  use std::fs;
  use std::path::{Path, PathBuf};
  use std::time::{SystemTime, UNIX_EPOCH};

  struct TestSnapshot {
    path: PathBuf,
  }

  impl TestSnapshot {
    fn from_fixture() -> Self {
      let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("test clock should be valid")
        .as_nanos();
      let path = std::env::temp_dir().join(format!("calcit-transaction-test-{}-{nonce}.cirru", std::process::id()));
      fs::copy("calcit/test.cirru", &path).expect("test snapshot fixture should copy");
      Self { path }
    }
  }

  impl Drop for TestSnapshot {
    fn drop(&mut self) {
      let _ = fs::remove_file(&self.path);
    }
  }

  fn fake_version_operation(stage_path: &Path, index: usize, args: &[String]) -> Result<TransactionOperationReport, String> {
    let version = args.get(2).ok_or_else(|| "fake operation needs version at index 2".to_string())?;
    let mut snapshot = load_snapshot(&stage_path.to_string_lossy())?;
    snapshot.version = version.to_string();
    save_snapshot(&snapshot, &stage_path.to_string_lossy())?;
    Ok(TransactionOperationReport {
      index,
      args: args.to_vec(),
      stdout: String::new(),
      stderr: String::new(),
    })
  }

  fn leaf(value: &str) -> Cirru {
    Cirru::Leaf(value.into())
  }

  fn list(items: Vec<Cirru>) -> Cirru {
    Cirru::List(items)
  }

  #[test]
  fn rename_updates_definition_declaration_name() {
    let code = list(vec![leaf("defatom"), leaf("*old"), list(vec![leaf("{}")])]);
    let (renamed, changed) = rename_definition_declaration(&code, "*old", "*next").expect("rename should work");

    assert!(changed);
    assert_eq!(renamed, list(vec![leaf("defatom"), leaf("*next"), list(vec![leaf("{}")])]));
  }

  #[test]
  fn transaction_parses_json_and_cirru_argument_lists() {
    let expected = vec![
      vec![
        "edit".to_string(),
        "doc".to_string(),
        "app.main/main!".to_string(),
        "hello".to_string(),
      ],
      vec![
        "tree".to_string(),
        "delete".to_string(),
        "app.main/main!".to_string(),
        "--path".to_string(),
        "@3".to_string(),
      ],
    ];
    let json = r#"[["edit","doc","app.main/main!","hello"],["tree","delete","app.main/main!","--path","@3"]]"#;
    let cirru = "[]\n  [] |edit |doc |app.main/main! |hello\n  [] |tree |delete |app.main/main! |--path |@3";

    assert_eq!(parse_transaction_operations(json).expect("JSON transaction should parse"), expected);
    assert_eq!(
      parse_transaction_operations(cirru).expect("Cirru transaction should parse"),
      expected
    );
  }

  #[test]
  fn transaction_cirru_embeds_quoted_code_without_string_escaping() {
    let cirru = "[]\n  []\n    , |tree\n    , |replace\n    , |app.main/main!\n    , |--path\n    , |@3.2\n    , |--code\n    quote $ println |done";
    let operations = parse_transaction_operations(cirru).expect("Cirru transaction with quoted code should parse");
    let code = operations[0].get(6).expect("formatted --code argument should exist");

    assert_eq!(
      parse_input_to_cirru(code).expect("embedded quoted code should remain valid edit input"),
      list(vec![leaf("println"), leaf("|done")])
    );
  }

  #[test]
  fn transaction_rejects_unsupported_and_nested_commands() {
    let unsupported =
      parse_transaction_operations(r#"[["query","def","app.main/main!"]]"#).expect_err("read-only command should be rejected");
    assert!(unsupported.contains("Only edit, tree, and config"), "error: {unsupported}");

    let nested = parse_transaction_operations(r#"[["edit","transaction","--file","again.cirru"]]"#)
      .expect_err("nested transaction should be rejected");
    assert!(nested.contains("nested edit transaction"), "error: {nested}");

    let external = parse_transaction_operations(r#"[["edit","inc","--changed","app.main/main!"]]"#)
      .expect_err("external side effect should be rejected");
    assert!(external.contains("external side effects"), "error: {external}");

    let read_only =
      parse_transaction_operations(r#"[["tree","show","app.main/main!"]]"#).expect_err("read-only command should be rejected");
    assert!(read_only.contains("unsupported staged mutation"), "error: {read_only}");
  }

  #[test]
  fn transaction_rejects_stale_revision_without_running_operations() {
    let fixture = TestSnapshot::from_fixture();
    let original = fs::read_to_string(&fixture.path).expect("fixture should read");
    let operations = vec![vec!["config".to_string(), "version".to_string(), "9.0.0".to_string()]];
    let mut called = false;

    let error = run_staged_transaction_with(&fixture.path, &operations, Some("md5:stale"), false, |_, _, _| {
      called = true;
      Err("must not run".to_string())
    })
    .expect_err("stale transaction should fail");

    assert!(!called);
    assert!(error.contains("revision mismatch"), "error: {error}");
    assert_eq!(fs::read_to_string(&fixture.path).expect("fixture should remain"), original);
  }

  #[test]
  fn transaction_dry_run_validates_but_keeps_original_snapshot() {
    let fixture = TestSnapshot::from_fixture();
    let original = fs::read_to_string(&fixture.path).expect("fixture should read");
    let operations = vec![vec!["config".to_string(), "version".to_string(), "9.0.0".to_string()]];

    let report = run_staged_transaction_with(&fixture.path, &operations, None, true, fake_version_operation)
      .expect("dry-run transaction should validate");

    assert!(report.dry_run);
    assert!(report.changed);
    assert_ne!(report.original_revision, report.new_revision);
    assert_eq!(fs::read_to_string(&fixture.path).expect("fixture should remain"), original);
  }

  #[test]
  fn transaction_failure_discards_prior_staged_operations() {
    let fixture = TestSnapshot::from_fixture();
    let original = fs::read_to_string(&fixture.path).expect("fixture should read");
    let operations = vec![
      vec!["config".to_string(), "version".to_string(), "9.0.0".to_string()],
      vec!["config".to_string(), "version".to_string(), "9.0.1".to_string()],
    ];

    let error = run_staged_transaction_with(&fixture.path, &operations, None, false, |path, index, args| {
      if index == 0 {
        fake_version_operation(path, index, args)
      } else {
        Err("simulated second operation failure".to_string())
      }
    })
    .expect_err("failed operation should abort transaction");

    assert!(error.contains("simulated second operation failure"), "error: {error}");
    assert_eq!(fs::read_to_string(&fixture.path).expect("fixture should remain"), original);
  }

  #[test]
  fn successful_transaction_replaces_snapshot_once() {
    let fixture = TestSnapshot::from_fixture();
    let operations = vec![vec!["config".to_string(), "version".to_string(), "9.0.0".to_string()]];

    let report =
      run_staged_transaction_with(&fixture.path, &operations, None, false, fake_version_operation).expect("transaction should commit");

    assert!(!report.dry_run);
    assert!(report.changed);
    assert_eq!(
      load_snapshot(&fixture.path.to_string_lossy())
        .expect("committed snapshot should load")
        .version,
      "9.0.0"
    );
  }

  #[test]
  fn rename_rejects_mismatched_definition_key_and_declaration() {
    let code = list(vec![leaf("defn"), leaf("actual"), list(vec![]), leaf("nil")]);
    let error = rename_definition_declaration(&code, "stored-key", "next").expect_err("mismatch should fail");

    assert!(error.contains("does not match declaration name"), "error: {error}");
  }

  #[test]
  fn rename_keeps_anonymous_payloads_unchanged() {
    let code = list(vec![leaf("fn"), list(vec![]), leaf("nil")]);
    let (renamed, changed) = rename_definition_declaration(&code, "old", "next").expect("anonymous value should be movable");

    assert!(!changed);
    assert_eq!(renamed, code);
  }

  #[test]
  fn examples_require_one_quote_per_ast_node() {
    let error = parse_examples_input("inc 1").expect_err("bare examples should fail");
    assert!(error.contains("needs its own `quote`"), "error: {error}");
  }

  #[test]
  fn examples_preserve_quoted_expressions_and_leaves() {
    let examples = parse_examples_input("quote $ inc 1\nquote |literal").expect("examples should parse");

    assert_eq!(examples, vec![list(vec![leaf("inc"), leaf("1")]), leaf("|literal")]);
  }

  #[test]
  fn schema_input_requires_quote_for_leaf_and_expression() {
    let expected = list(vec![leaf("::"), leaf(":ref"), leaf(":bool")]);

    assert_eq!(
      parse_schema_input("quote $ :: :ref :bool").expect("quoted schema should parse"),
      expected
    );
    assert_eq!(
      parse_schema_input("quote :string").expect("primitive schema should parse"),
      leaf(":string")
    );
    let error = parse_schema_input(":: :ref :bool").expect_err("bare schema should fail");
    assert!(error.contains("Schema examples: `quote :string`"), "error: {error}");
  }

  #[test]
  fn bumps_patch_version() {
    assert_eq!(bump_semver_value("0.0.0", "patch"), Ok("0.0.1".to_string()));
  }

  #[test]
  fn bumps_minor_version() {
    assert_eq!(bump_semver_value("1.2.3", "minor"), Ok("1.3.0".to_string()));
  }

  #[test]
  fn bumps_major_version() {
    assert_eq!(bump_semver_value("1.2.3", "major"), Ok("2.0.0".to_string()));
  }

  #[test]
  fn rejects_unknown_bump_level() {
    assert_eq!(
      bump_semver_value("1.2.3", "build"),
      Err("Unknown bump level 'build'. Valid levels: patch, minor, major".to_string())
    );
  }

  #[test]
  fn clears_prerelease_and_build_metadata_when_bumping() {
    assert_eq!(bump_semver_value("1.2.3-alpha.1+build.2", "patch"), Ok("1.2.4".to_string()));
  }
}
