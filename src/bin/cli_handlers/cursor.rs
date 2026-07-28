use calcit::cli_args::{CursorCommand, CursorSubcommand};
use calcit::snapshot;
use cirru_edn::{Edn, EdnListView};
use cirru_parser::Cirru;
use colored::Colorize;
use md5::{Digest, Md5};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::common::{cirru_to_json_value, format_path, parse_path};
use super::edit::{apply_operation_at_path, check_ns_editable, load_snapshot, navigate_to_path, parse_target, save_snapshot};

const CURSOR_FILE: &str = ".calcit-cursor.cirru";
const ACTIVE_CURSOR: &str = "main";
const CURSOR_MAINTENANCE_ENV: &str = "CALCIT_CURSOR_MAINTENANCE";
const CURSOR_SCHEMA_VERSION: u8 = 2;
const CURSOR_HISTORY_LIMIT: usize = 32;
const CURSOR_STACK_LIMIT: usize = 16;

/// 0 = none, 1 = summary, 2 = focus.
static CURSOR_AFTER_MODE: AtomicU8 = AtomicU8::new(1);

#[derive(Debug, Clone, PartialEq)]
struct CursorState {
  snapshot: String,
  target: String,
  path: Vec<usize>,
  definition_revision: String,
  fingerprint: String,
  preview: Cirru,
}

#[derive(Debug, Clone, PartialEq)]
struct CursorClipboard {
  mode: String,
  source_target: String,
  source_path: Vec<usize>,
  fingerprint: String,
  tree: Cirru,
}

#[derive(Debug, Clone, PartialEq)]
struct CursorDocument {
  active: CursorState,
  history: Vec<CursorState>,
  stack: Vec<CursorState>,
  clipboard: Option<CursorClipboard>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TreeCursorMutation {
  NoPathShift,
  Replace { path: Vec<usize> },
  InsertBefore { path: Vec<usize> },
  InsertAfter { path: Vec<usize> },
  InsertChild { path: Vec<usize> },
  Delete { path: Vec<usize> },
  SwapNext { path: Vec<usize> },
  SwapPrev { path: Vec<usize> },
  Unwrap { path: Vec<usize>, child_count: usize },
  Raise { path: Vec<usize> },
  Wrap { path: Vec<usize> },
}

pub fn set_cursor_after_mode(mode: &str) -> Result<(), String> {
  let value = match mode {
    "none" => 0,
    "summary" => 1,
    "focus" => 2,
    other => {
      return Err(format!(
        "Unsupported --cursor-after mode '{other}'. Expected none, summary, or focus."
      ));
    }
  };
  CURSOR_AFTER_MODE.store(value, Ordering::Relaxed);
  Ok(())
}

pub fn handle_cursor_command(cmd: &CursorCommand, snapshot_file: &str) -> Result<(), String> {
  match &cmd.subcommand {
    CursorSubcommand::Set(opts) => {
      if opts.path == "@cursor" {
        return Err("`cursor set --path` requires a concrete path, not `@cursor`.".to_string());
      }
      let path = parse_path(&opts.path)?;
      let state = set_cursor_selection(snapshot_file, &opts.target, path)?;
      warn_cursor_gitignore(snapshot_file);
      println!("{} Cursor set: {} {}", "✓".green(), state.target, format_path(&state.path));
      Ok(())
    }
    CursorSubcommand::Show(opts) => handle_cursor_show(snapshot_file, &opts.format, &opts.view),
    CursorSubcommand::Clear(_) => {
      let file = cursor_file_path(snapshot_file);
      if file.exists() {
        fs::remove_file(&file).map_err(|error| format!("Failed to remove cursor file '{}': {error}", file.display()))?;
        println!("{} Cursor cleared", "✓".green());
      } else {
        println!("{} No cursor is set", "•".dimmed());
      }
      Ok(())
    }
    CursorSubcommand::Parent(_) => move_cursor(snapshot_file, |path| {
      if path.is_empty() {
        Err("Cursor is already at the definition root.".to_string())
      } else {
        path.pop();
        Ok(())
      }
    }),
    CursorSubcommand::Child(opts) => move_cursor_to_child(snapshot_file, opts.index, opts.last),
    CursorSubcommand::Next(opts) => move_cursor_across_siblings(snapshot_file, opts.count, true),
    CursorSubcommand::Prev(opts) => move_cursor_across_siblings(snapshot_file, opts.count, false),
    CursorSubcommand::Back(opts) => restore_cursor(snapshot_file, RestoreSource::History, opts.count),
    CursorSubcommand::Push(_) => push_cursor(snapshot_file),
    CursorSubcommand::Pop(_) => restore_cursor(snapshot_file, RestoreSource::Stack, 1),
    CursorSubcommand::Copy(_) => store_cursor_clipboard(snapshot_file, "copy", false),
    CursorSubcommand::Cut(_) => store_cursor_clipboard(snapshot_file, "cut", true),
    CursorSubcommand::Paste(opts) => paste_cursor_clipboard(snapshot_file, &opts.at),
    CursorSubcommand::Clipboard(opts) => show_cursor_clipboard(snapshot_file, &opts.format),
    CursorSubcommand::ClearClipboard(_) => clear_cursor_clipboard(snapshot_file),
  }
}

fn set_cursor_selection(snapshot_file: &str, target: &str, path: Vec<usize>) -> Result<CursorState, String> {
  let mut state = CursorState {
    snapshot: snapshot_file.to_string(),
    target: target.to_string(),
    path,
    definition_revision: String::new(),
    fingerprint: String::new(),
    preview: Cirru::List(vec![]),
  };
  refresh_cursor_state(&mut state, snapshot_file)?;
  let mut document = load_cursor_document_optional(snapshot_file)?.unwrap_or_else(|| CursorDocument {
    active: state.clone(),
    history: vec![],
    stack: vec![],
    clipboard: None,
  });
  if document.active.target != state.target || document.active.path != state.path {
    push_bounded(&mut document.history, document.active.clone(), CURSOR_HISTORY_LIMIT);
  }
  document.active = state.clone();
  save_cursor_document(snapshot_file, &document)?;
  Ok(state)
}

pub(crate) fn set_cursor_from_query_match(
  snapshot_file: &str,
  target: &str,
  path: Vec<usize>,
  match_index: usize,
) -> Result<(), String> {
  let state = set_cursor_selection(snapshot_file, target, path)
    .map_err(|error| format!("Search match #{match_index} cannot become the project cursor: {error}"))?;
  warn_cursor_gitignore(snapshot_file);
  eprintln!(
    "{} Cursor set from search match #{}: {} {}",
    "✓".green(),
    match_index,
    state.target,
    format_path(&state.path)
  );
  Ok(())
}

fn handle_cursor_show(snapshot_file: &str, format: &str, view: &str) -> Result<(), String> {
  if !matches!(format, "human" | "json") {
    return Err(format!("Unsupported cursor format '{format}'. Expected human or json."));
  }
  if !matches!(view, "focus" | "node" | "full") {
    return Err(format!("Unsupported cursor view '{view}'. Expected focus, node, or full."));
  }
  let (state, status) = validate_cursor_with_status(snapshot_file, true)?;
  let (node, _) = read_cursor_target(snapshot_file, &state.target, &state.path)?;
  let preview = build_cursor_preview(snapshot_file, &state, view)?;

  if format == "json" {
    println!(
      "{}",
      serde_json::json!({
        "schema_version": 2,
        "command": "cursor.show",
        "status": status,
        "view": view,
        "target": state.target,
        "path": format_path(&state.path),
        "definition_revision": state.definition_revision,
        "fingerprint": state.fingerprint,
        "tree": cirru_to_json_value(&node),
        "preview_tree": cirru_to_json_value(&preview),
      })
    );
  } else {
    println!("{}", render_cursor_human(&state, status, &preview)?);
  }
  Ok(())
}

fn move_cursor<F>(snapshot_file: &str, update: F) -> Result<(), String>
where
  F: FnOnce(&mut Vec<usize>) -> Result<(), String>,
{
  let mut state = validate_cursor(snapshot_file, true)?;
  let old_state = state.clone();
  update(&mut state.path)?;
  commit_cursor_move(snapshot_file, old_state, state)
}

fn commit_cursor_move(snapshot_file: &str, old_state: CursorState, mut state: CursorState) -> Result<(), String> {
  let old_path = old_state.path.clone();
  refresh_cursor_state(&mut state, snapshot_file)?;
  let mut document = load_cursor_document(snapshot_file)?;
  push_bounded(&mut document.history, old_state, CURSOR_HISTORY_LIMIT);
  document.active = state.clone();
  save_cursor_document(snapshot_file, &document)?;
  println!(
    "{} Cursor moved: {} → {}",
    "✓".green(),
    format_path(&old_path),
    format_path(&state.path)
  );
  Ok(())
}

fn move_cursor_to_child(snapshot_file: &str, index: Option<usize>, last: bool) -> Result<(), String> {
  if last && index.is_some() {
    return Err("`cursor child` accepts either an index or `--last`, not both.".to_string());
  }
  let mut state = validate_cursor(snapshot_file, true)?;
  let old_state = state.clone();
  let (node, _) = read_cursor_target(snapshot_file, &state.target, &state.path)?;
  let Cirru::List(children) = node else {
    return Err("Selected leaf has no children.".to_string());
  };
  if children.is_empty() {
    return Err("Selected list has no children.".to_string());
  }
  let child_index = if last { children.len() - 1 } else { index.unwrap_or(0) };
  if child_index >= children.len() {
    return Err(format!(
      "Child index {child_index} is out of range; selected list has {} child(ren).",
      children.len()
    ));
  }
  state.path.push(child_index);
  commit_cursor_move(snapshot_file, old_state, state)
}

fn move_cursor_across_siblings(snapshot_file: &str, count: usize, forward: bool) -> Result<(), String> {
  if count == 0 {
    return Err("`--count` must be greater than zero.".to_string());
  }
  let mut state = validate_cursor(snapshot_file, true)?;
  let old_state = state.clone();
  let Some(&current_index) = state.path.last() else {
    return Err(format!(
      "Definition root has no {} sibling.",
      if forward { "next" } else { "previous" }
    ));
  };
  let parent_path = &state.path[..state.path.len() - 1];
  let (parent, _) = read_cursor_target(snapshot_file, &state.target, parent_path)?;
  let Cirru::List(siblings) = parent else {
    return Err("Cursor parent is not a list and has no sibling sequence.".to_string());
  };
  let new_index = if forward {
    current_index.checked_add(count).ok_or("Sibling index overflowed.")?
  } else {
    current_index.checked_sub(count).ok_or_else(|| {
      format!("Cannot move back {count} sibling(s) from index {current_index}; only {current_index} previous sibling(s) exist.")
    })?
  };
  if new_index >= siblings.len() {
    return Err(format!(
      "Cannot move forward {count} sibling(s) from index {current_index}; only {} next sibling(s) exist.",
      siblings.len().saturating_sub(current_index + 1)
    ));
  }
  *state.path.last_mut().expect("non-root cursor path has a final index") = new_index;
  commit_cursor_move(snapshot_file, old_state, state)
}

fn build_cursor_preview(snapshot_file: &str, state: &CursorState, view: &str) -> Result<Cirru, String> {
  let (node, _) = read_cursor_target(snapshot_file, &state.target, &state.path)?;
  if view == "node" {
    return Ok(Cirru::List(vec![Cirru::leaf("CURSOR"), node]));
  }

  let (definition, _) = read_cursor_definition(snapshot_file, &state.target)?;
  if view == "focus" {
    let options = cirru_parser::CirruFocusOptions::default()
      .with_focus_marker("CURSOR")
      .with_root_prefix(3);
    Ok(cirru_parser::focus_cirru_preview_with_options(&definition, &state.path, &options))
  } else {
    let marker = Cirru::List(vec![Cirru::leaf("CURSOR"), node]);
    apply_operation_at_path(&definition, &state.path, "replace", Some(&marker))
  }
}

fn render_cursor_human(state: &CursorState, status: &str, preview: &Cirru) -> Result<String, String> {
  let rendered = cirru_parser::format(
    std::slice::from_ref(preview),
    cirru_parser::CirruWriterOptions { use_inline: false },
  )
  .map_err(|error| format!("Failed to render cursor preview: {error}"))?;
  Ok(format!(
    "{}: {}\n{}: {}\n{}: {}\n{}: {}\n\n{}",
    "Target".green().bold(),
    state.target,
    "Path".green().bold(),
    format_path(&state.path),
    "Revision".green().bold(),
    state.definition_revision,
    "Status".green().bold(),
    status,
    rendered
  ))
}

fn push_bounded<T>(items: &mut Vec<T>, value: T, limit: usize) {
  if items.len() == limit {
    items.remove(0);
  }
  items.push(value);
}

#[derive(Clone, Copy)]
enum RestoreSource {
  History,
  Stack,
}

fn restore_cursor(snapshot_file: &str, source: RestoreSource, count: usize) -> Result<(), String> {
  if count == 0 {
    return Err("`--count` must be greater than zero.".to_string());
  }
  let mut document = load_cursor_document(snapshot_file)?;
  let available = match source {
    RestoreSource::History => document.history.len(),
    RestoreSource::Stack => document.stack.len(),
  };
  if count > available {
    return Err(format!(
      "Cannot restore {count} cursor location(s); only {available} {} entr{} available.",
      if matches!(source, RestoreSource::History) {
        "history"
      } else {
        "stack"
      },
      if available == 1 { "y is" } else { "ies are" }
    ));
  }
  let mut restored = None;
  for _ in 0..count {
    restored = Some(match source {
      RestoreSource::History => document.history.pop().expect("history length checked"),
      RestoreSource::Stack => document.stack.pop().expect("stack length checked"),
    });
  }
  let restored = restored.expect("positive restore count");
  let current = document.active.clone();
  let restored = relocate_saved_state(snapshot_file, restored)?;
  if matches!(source, RestoreSource::Stack) {
    push_bounded(&mut document.history, current, CURSOR_HISTORY_LIMIT);
  }
  document.active = restored.clone();
  save_cursor_document(snapshot_file, &document)?;
  let action = match source {
    RestoreSource::History => "Cursor moved back",
    RestoreSource::Stack => "Cursor popped",
  };
  println!(
    "{} {action}{}: {} {}",
    "✓".green(),
    if count == 1 { String::new() } else { format!(" {count} steps") },
    restored.target,
    format_path(&restored.path)
  );
  Ok(())
}

fn push_cursor(snapshot_file: &str) -> Result<(), String> {
  let state = validate_cursor(snapshot_file, true)?;
  let mut document = load_cursor_document(snapshot_file)?;
  push_bounded(&mut document.stack, state.clone(), CURSOR_STACK_LIMIT);
  document.active = state;
  save_cursor_document(snapshot_file, &document)?;
  println!("{} Cursor pushed (stack depth {})", "✓".green(), document.stack.len());
  Ok(())
}

fn store_cursor_clipboard(snapshot_file: &str, mode: &str, cut: bool) -> Result<(), String> {
  let state = validate_cursor(snapshot_file, true)?;
  if cut && state.path.is_empty() {
    return Err("Cannot cut the definition root. Use `cr edit mv-def` or replace the definition explicitly.".to_string());
  }
  let (node, _) = read_cursor_target(snapshot_file, &state.target, &state.path)?;
  let clipboard = CursorClipboard {
    mode: mode.to_string(),
    source_target: state.target.clone(),
    source_path: state.path.clone(),
    fingerprint: node_fingerprint(&node),
    tree: node,
  };
  let mut document = load_cursor_document(snapshot_file)?;
  document.clipboard = Some(clipboard);

  if cut {
    let (namespace, definition) = parse_target(&state.target)?;
    let mut snapshot = load_snapshot(snapshot_file)?;
    check_ns_editable(&snapshot, namespace)?;
    let entry = snapshot
      .files
      .get_mut(namespace)
      .and_then(|file| file.defs.get_mut(definition))
      .ok_or_else(|| format!("Definition '{}' not found", state.target))?;
    entry.code = apply_operation_at_path(&entry.code, &state.path, "delete", None)?;
    save_snapshot(&snapshot, snapshot_file)?;

    document.active.path = transform_delete(&state.path, &state.path).0;
    refresh_cursor_state(&mut document.active, snapshot_file)?;
  }
  save_cursor_document(snapshot_file, &document)?;
  println!(
    "{} {} expression at {} {} into cursor clipboard",
    "✓".green(),
    if cut { "Cut" } else { "Copied" },
    state.target,
    format_path(&state.path)
  );
  if cut {
    emit_cursor_after(snapshot_file, &document.active, "selected expression was cut; moved to parent")?;
  }
  Ok(())
}

fn paste_cursor_clipboard(snapshot_file: &str, at: &str) -> Result<(), String> {
  let state = validate_cursor(snapshot_file, true)?;
  let mut document = load_cursor_document(snapshot_file)?;
  let clipboard = document
    .clipboard
    .clone()
    .ok_or("Cursor clipboard is empty. Use `cr cursor copy` or `cr cursor cut` first.")?;
  let operation = match at {
    "before" => "insert-before",
    "after" => "insert-after",
    "prepend-child" => "insert-child",
    "append-child" => "append-child",
    "replace" => "replace",
    other => {
      return Err(format!(
        "Unsupported paste position '{other}'. Use before, after, prepend-child, append-child, or replace."
      ));
    }
  };

  if state.path.is_empty() && matches!(at, "before" | "after") {
    return Err("Cannot paste before or after the definition root; use --at prepend-child, append-child, or replace.".to_string());
  }

  let (namespace, definition) = parse_target(&state.target)?;
  let mut snapshot = load_snapshot(snapshot_file)?;
  check_ns_editable(&snapshot, namespace)?;
  let entry = snapshot
    .files
    .get_mut(namespace)
    .and_then(|file| file.defs.get_mut(definition))
    .ok_or_else(|| format!("Definition '{}' not found", state.target))?;
  let selected = navigate_to_path(&entry.code, &state.path)?;
  let append_index = match selected {
    Cirru::List(children) => children.len(),
    Cirru::Leaf(_) => 0,
  };
  entry.code = apply_operation_at_path(&entry.code, &state.path, operation, Some(&clipboard.tree))?;
  save_snapshot(&snapshot, snapshot_file)?;

  let mut pasted_path = state.path.clone();
  match at {
    "after" => *pasted_path.last_mut().expect("root was rejected above") += 1,
    "prepend-child" => pasted_path.push(0),
    "append-child" => pasted_path.push(append_index),
    "before" | "replace" => {}
    _ => unreachable!(),
  }
  push_bounded(&mut document.history, state, CURSOR_HISTORY_LIMIT);
  document.active.path = pasted_path;
  refresh_cursor_state(&mut document.active, snapshot_file)?;
  save_cursor_document(snapshot_file, &document)?;
  println!(
    "{} Pasted cursor clipboard {} selection; cursor now at {}",
    "✓".green(),
    at,
    format_path(&document.active.path)
  );
  emit_cursor_after(snapshot_file, &document.active, "cursor follows pasted expression")?;
  Ok(())
}

fn show_cursor_clipboard(snapshot_file: &str, format: &str) -> Result<(), String> {
  if !matches!(format, "human" | "json") {
    return Err(format!("Unsupported clipboard format '{format}'. Expected human or json."));
  }
  let document = load_cursor_document(snapshot_file)?;
  let clipboard = document.clipboard.ok_or("Cursor clipboard is empty.")?;
  if format == "json" {
    println!(
      "{}",
      serde_json::json!({
        "schema_version": 1,
        "command": "cursor.clipboard",
        "mode": clipboard.mode,
        "source_target": clipboard.source_target,
        "source_path": format_path(&clipboard.source_path),
        "fingerprint": clipboard.fingerprint,
        "tree": cirru_to_json_value(&clipboard.tree),
      })
    );
  } else {
    let rendered = cirru_parser::format(
      &[Cirru::List(vec![Cirru::leaf("CLIPBOARD"), clipboard.tree])],
      cirru_parser::CirruWriterOptions { use_inline: false },
    )
    .map_err(|error| format!("Failed to render cursor clipboard: {error}"))?;
    println!(
      "Mode: {}\nSource: {} {}\n\n{rendered}",
      clipboard.mode,
      clipboard.source_target,
      format_path(&clipboard.source_path)
    );
  }
  Ok(())
}

fn clear_cursor_clipboard(snapshot_file: &str) -> Result<(), String> {
  let mut document = load_cursor_document(snapshot_file)?;
  document.clipboard = None;
  save_cursor_document(snapshot_file, &document)?;
  println!("{} Cursor clipboard cleared", "✓".green());
  Ok(())
}

pub(crate) fn resolve_cursor_path_argument(snapshot_file: &str, target: &str, path: &str) -> Result<String, String> {
  if path != "@cursor" {
    return Ok(path.to_string());
  }
  let state = validate_cursor(snapshot_file, true)?;
  if state.target != target {
    return Err(format!(
      "Cursor target mismatch: cursor points to '{}', but command targets '{}'.",
      state.target, target
    ));
  }
  Ok(if state.path.is_empty() {
    String::new()
  } else {
    format_path(&state.path)
  })
}

pub(crate) fn maintain_cursor_after_tree_mutation(
  snapshot_file: &str,
  target: &str,
  mutation: &TreeCursorMutation,
) -> Result<(), String> {
  if std::env::var(CURSOR_MAINTENANCE_ENV).is_ok_and(|value| value == "disabled") {
    return Ok(());
  }
  if !cursor_file_path(snapshot_file).exists() {
    return Ok(());
  }

  let mut document = load_cursor_document(snapshot_file)?;
  if document.active.target != target {
    return Ok(());
  }

  let old_path = document.active.path.clone();
  let (new_path, note) = transform_cursor_path(&document.active.path, mutation);
  document.active.path = new_path;
  refresh_cursor_state(&mut document.active, snapshot_file).map_err(|error| {
    format!("Snapshot mutation succeeded, but cursor maintenance failed: {error}. Run `cr cursor set` to select it again.")
  })?;
  save_cursor_document(snapshot_file, &document).map_err(|error| {
    format!("Snapshot mutation succeeded, but cursor state could not be saved: {error}. Run `cr cursor show` before editing again.")
  })?;

  let detail = if old_path != document.active.path {
    format!("{} → {} ({note})", format_path(&old_path), format_path(&document.active.path))
  } else {
    format!("{} ({note})", format_path(&document.active.path))
  };
  emit_cursor_after(snapshot_file, &document.active, &detail)?;
  Ok(())
}

pub(crate) fn maintain_cursor_after_definition_move(snapshot_file: &str, source: &str, target: &str) -> Result<(), String> {
  if std::env::var(CURSOR_MAINTENANCE_ENV).is_ok_and(|value| value == "disabled") || !cursor_file_path(snapshot_file).exists() {
    return Ok(());
  }
  let mut document = load_cursor_document(snapshot_file)?;
  if document.active.target != source {
    return Ok(());
  }
  document.active.target = target.to_string();
  refresh_cursor_state(&mut document.active, snapshot_file).map_err(|error| {
    format!("Definition move succeeded, but cursor maintenance failed: {error}. Run `cr cursor set` to select it again.")
  })?;
  save_cursor_document(snapshot_file, &document)?;
  emit_cursor_after(snapshot_file, &document.active, &format!("target updated: {source} → {target}"))
}

pub(crate) fn maintain_cursor_after_definition_replace(snapshot_file: &str, target: &str) -> Result<(), String> {
  maintain_cursor_after_tree_mutation(snapshot_file, target, &TreeCursorMutation::Replace { path: vec![] })
}

pub(crate) fn maintain_cursor_after_definition_delete(snapshot_file: &str, target: &str) -> Result<(), String> {
  if std::env::var(CURSOR_MAINTENANCE_ENV).is_ok_and(|value| value == "disabled") || !cursor_file_path(snapshot_file).exists() {
    return Ok(());
  }
  let document = load_cursor_document(snapshot_file)?;
  if document.active.target == target && CURSOR_AFTER_MODE.load(Ordering::Relaxed) != 0 {
    eprintln!(
      "{} Selected definition '{target}' was deleted; the cursor is stale. Use `cr cursor back` or `cr cursor set`.",
      "[Cursor]".yellow().bold()
    );
  }
  Ok(())
}

pub(crate) fn maintain_cursor_after_namespace_delete(snapshot_file: &str, namespace: &str) -> Result<(), String> {
  if std::env::var(CURSOR_MAINTENANCE_ENV).is_ok_and(|value| value == "disabled") || !cursor_file_path(snapshot_file).exists() {
    return Ok(());
  }
  let document = load_cursor_document(snapshot_file)?;
  let cursor_namespace = parse_target(&document.active.target)?.0;
  if cursor_namespace == namespace && CURSOR_AFTER_MODE.load(Ordering::Relaxed) != 0 {
    eprintln!(
      "{} Selected namespace '{namespace}' was deleted; the cursor is stale. Use `cr cursor back` or `cr cursor set`.",
      "[Cursor]".yellow().bold()
    );
  }
  Ok(())
}

pub(crate) fn maintain_cursor_after_split_definition(
  snapshot_file: &str,
  source: &str,
  split_path: &[usize],
  target: &str,
) -> Result<(), String> {
  if std::env::var(CURSOR_MAINTENANCE_ENV).is_ok_and(|value| value == "disabled") || !cursor_file_path(snapshot_file).exists() {
    return Ok(());
  }
  let mut document = load_cursor_document(snapshot_file)?;
  if document.active.target != source {
    return Ok(());
  }
  let detail;
  if document.active.path.starts_with(split_path) {
    document.active.target = target.to_string();
    document.active.path = document.active.path[split_path.len()..].to_vec();
    detail = format!("followed extracted subtree: {source} → {target}");
  } else {
    document.active.path = transform_cursor_path(&document.active.path, &TreeCursorMutation::Replace { path: split_path.to_vec() }).0;
    detail = "split expression replaced by new definition reference".to_string();
  }
  refresh_cursor_state(&mut document.active, snapshot_file)?;
  save_cursor_document(snapshot_file, &document)?;
  emit_cursor_after(snapshot_file, &document.active, &detail)
}

pub(crate) fn maintain_cursor_after_node_move(
  snapshot_file: &str,
  target: &str,
  source_path: &[usize],
  destination_path: &[usize],
  operation: &str,
  append_index: usize,
) -> Result<(), String> {
  if std::env::var(CURSOR_MAINTENANCE_ENV).is_ok_and(|value| value == "disabled") || !cursor_file_path(snapshot_file).exists() {
    return Ok(());
  }
  let mut document = load_cursor_document(snapshot_file)?;
  if document.active.target != target {
    return Ok(());
  }

  let mut inserted_path = destination_path.to_vec();
  let insertion = match operation {
    "insert-before" => TreeCursorMutation::InsertBefore {
      path: destination_path.to_vec(),
    },
    "insert-after" => {
      *inserted_path.last_mut().ok_or("Cannot move after the definition root.")? += 1;
      TreeCursorMutation::InsertAfter {
        path: destination_path.to_vec(),
      }
    }
    "insert-child" => {
      inserted_path.push(0);
      TreeCursorMutation::InsertChild {
        path: destination_path.to_vec(),
      }
    }
    "append-child" => {
      inserted_path.push(append_index);
      TreeCursorMutation::NoPathShift
    }
    "replace" => TreeCursorMutation::Replace {
      path: destination_path.to_vec(),
    },
    other => return Err(format!("Unsupported node move operation '{other}'.")),
  };

  let adjusted_source = adjusted_source_path_after_insertion(source_path, destination_path, operation);
  let next_path = if document.active.path.starts_with(source_path) {
    let mut moved = inserted_path;
    moved.extend_from_slice(&document.active.path[source_path.len()..]);
    transform_delete(&moved, &adjusted_source).0
  } else {
    let after_insert = transform_cursor_path(&document.active.path, &insertion).0;
    transform_delete(&after_insert, &adjusted_source).0
  };
  document.active.path = next_path;
  refresh_cursor_state(&mut document.active, snapshot_file)?;
  save_cursor_document(snapshot_file, &document)?;
  emit_cursor_after(snapshot_file, &document.active, "cursor followed node move")
}

fn adjusted_source_path_after_insertion(source: &[usize], destination: &[usize], operation: &str) -> Vec<usize> {
  let mut adjusted = source.to_vec();
  if !matches!(operation, "insert-before" | "insert-after") || source.len() != destination.len() || source.is_empty() {
    return adjusted;
  }
  let parent_depth = source.len() - 1;
  if source[..parent_depth] != destination[..parent_depth] {
    return adjusted;
  }
  let insert_position = if operation == "insert-before" {
    destination[parent_depth]
  } else {
    destination[parent_depth] + 1
  };
  if insert_position <= source[parent_depth] {
    adjusted[parent_depth] += 1;
  }
  adjusted
}

pub(crate) fn maintain_cursor_after_any_mutation(snapshot_file: &str, detail: &str) -> Result<(), String> {
  if std::env::var(CURSOR_MAINTENANCE_ENV).is_ok_and(|value| value == "disabled") || !cursor_file_path(snapshot_file).exists() {
    return Ok(());
  }
  match validate_cursor(snapshot_file, true) {
    Ok(state) => emit_cursor_after(snapshot_file, &state, detail),
    Err(error) => {
      if CURSOR_AFTER_MODE.load(Ordering::Relaxed) != 0 {
        eprintln!(
          "{} Transaction committed, but the cursor needs attention: {error} Use `cr cursor back` or `cr cursor set`.",
          "[Cursor]".yellow().bold()
        );
      }
      Ok(())
    }
  }
}

fn emit_cursor_after(snapshot_file: &str, state: &CursorState, detail: &str) -> Result<(), String> {
  match CURSOR_AFTER_MODE.load(Ordering::Relaxed) {
    0 => Ok(()),
    1 => {
      eprintln!(
        "{} {} {} — {detail}",
        "[Cursor]".cyan().bold(),
        state.target,
        format_path(&state.path)
      );
      Ok(())
    }
    2 => {
      let preview = build_cursor_preview(snapshot_file, state, "focus")?;
      let rendered = cirru_parser::format(
        std::slice::from_ref(&preview),
        cirru_parser::CirruWriterOptions { use_inline: false },
      )
      .map_err(|error| format!("Failed to render automatic cursor preview: {error}"))?;
      eprintln!(
        "{} {} {} — {detail}\n{}",
        "[Cursor]".cyan().bold(),
        state.target,
        format_path(&state.path),
        rendered
      );
      Ok(())
    }
    _ => unreachable!(),
  }
}

fn cursor_file_path(snapshot_file: &str) -> PathBuf {
  let snapshot = Path::new(snapshot_file);
  let parent = snapshot
    .parent()
    .filter(|path| !path.as_os_str().is_empty())
    .unwrap_or(Path::new("."));
  parent.join(CURSOR_FILE)
}

fn node_fingerprint(node: &Cirru) -> String {
  let encoded = cirru_to_json_value(node).to_string();
  let mut hasher = Md5::new();
  hasher.update(encoded.as_bytes());
  format!("md5:{:x}", hasher.finalize())
}

fn read_cursor_target(snapshot_file: &str, target: &str, path: &[usize]) -> Result<(Cirru, String), String> {
  let (definition, revision) = read_cursor_definition(snapshot_file, target)?;
  let node = navigate_to_path(&definition, path)
    .map_err(|error| format!("Cursor path {} is no longer valid in '{}': {error}", format_path(path), target))?;
  Ok((node, revision))
}

fn read_cursor_definition(snapshot_file: &str, target: &str) -> Result<(Cirru, String), String> {
  let (namespace, definition) = parse_target(target)?;
  let snapshot = load_snapshot(snapshot_file)?;
  let file = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Cursor namespace '{namespace}' no longer exists."))?;
  let entry = file
    .defs
    .get(definition)
    .ok_or_else(|| format!("Cursor definition '{target}' no longer exists."))?;
  Ok((entry.code.clone(), snapshot::definition_revision(entry)?))
}

fn refresh_cursor_state(state: &mut CursorState, snapshot_file: &str) -> Result<(), String> {
  let (node, revision) = read_cursor_target(snapshot_file, &state.target, &state.path)?;
  state.snapshot = snapshot_file.to_string();
  state.definition_revision = revision;
  state.fingerprint = node_fingerprint(&node);
  state.preview = node;
  Ok(())
}

fn validate_cursor(snapshot_file: &str, announce: bool) -> Result<CursorState, String> {
  Ok(validate_cursor_with_status(snapshot_file, announce)?.0)
}

fn validate_cursor_with_status(snapshot_file: &str, announce: bool) -> Result<(CursorState, &'static str), String> {
  let mut document = load_cursor_document(snapshot_file)?;
  let mut state = document.active.clone();
  let (namespace, definition) = parse_target(&state.target)?;
  let snapshot = load_snapshot(snapshot_file)?;
  let file = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Cursor namespace '{namespace}' no longer exists."))?;
  let entry = file
    .defs
    .get(definition)
    .ok_or_else(|| format!("Cursor definition '{}' no longer exists.", state.target))?;

  if let Ok(node) = navigate_to_path(&entry.code, &state.path)
    && node_fingerprint(&node) == state.fingerprint
  {
    let revision = snapshot::definition_revision(entry)?;
    let status = if state.definition_revision != revision || state.snapshot != snapshot_file {
      "verified-at-path"
    } else {
      "exact"
    };
    if state.definition_revision != revision || state.snapshot != snapshot_file {
      state.definition_revision = revision;
      state.snapshot = snapshot_file.to_string();
      state.preview = node;
      document.active = state.clone();
      save_cursor_document(snapshot_file, &document)?;
    }
    return Ok((state, status));
  }

  let mut matches = vec![];
  collect_fingerprint_paths(&entry.code, &state.fingerprint, &mut vec![], &mut matches);
  if matches.len() == 1 {
    let old_path = state.path.clone();
    state.path = matches.remove(0);
    refresh_cursor_state(&mut state, snapshot_file)?;
    document.active = state.clone();
    save_cursor_document(snapshot_file, &document)?;
    if announce {
      eprintln!(
        "{} Cursor relocated: {} → {} after external tree changes",
        "[Cursor]".cyan().bold(),
        format_path(&old_path),
        format_path(&state.path)
      );
    }
    Ok((state, "relocated"))
  } else {
    Err(format!(
      "Cursor is stale: saved node at {} in '{}' has {} fingerprint match(es). Refusing to guess; run `cr cursor set <target> --path <path>`.",
      format_path(&state.path),
      state.target,
      matches.len()
    ))
  }
}

fn relocate_saved_state(snapshot_file: &str, mut state: CursorState) -> Result<CursorState, String> {
  let (definition, _) = read_cursor_definition(snapshot_file, &state.target)?;
  if let Ok(node) = navigate_to_path(&definition, &state.path)
    && node_fingerprint(&node) == state.fingerprint
  {
    refresh_cursor_state(&mut state, snapshot_file)?;
    return Ok(state);
  }

  let mut matches = vec![];
  collect_fingerprint_paths(&definition, &state.fingerprint, &mut vec![], &mut matches);
  if matches.len() != 1 {
    return Err(format!(
      "Saved cursor location {} in '{}' has {} fingerprint match(es); refusing to restore ambiguously.",
      format_path(&state.path),
      state.target,
      matches.len()
    ));
  }
  state.path = matches.remove(0);
  refresh_cursor_state(&mut state, snapshot_file)?;
  Ok(state)
}

fn collect_fingerprint_paths(node: &Cirru, fingerprint: &str, path: &mut Vec<usize>, output: &mut Vec<Vec<usize>>) {
  if node_fingerprint(node) == fingerprint {
    output.push(path.clone());
  }
  if let Cirru::List(children) = node {
    for (index, child) in children.iter().enumerate() {
      path.push(index);
      collect_fingerprint_paths(child, fingerprint, path, output);
      path.pop();
    }
  }
}

fn load_cursor_document_optional(snapshot_file: &str) -> Result<Option<CursorDocument>, String> {
  let file = cursor_file_path(snapshot_file);
  let content = match fs::read_to_string(&file) {
    Ok(content) => content,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
    Err(error) => return Err(format!("Failed to read cursor file '{}': {error}", file.display())),
  };
  let edn = cirru_edn::parse(&content).map_err(|error| format!("Failed to parse cursor Cirru EDN '{}': {error}", file.display()))?;
  let root = edn.view_map().map_err(|error| format!("Cursor root must be a map: {error}"))?;
  let version = root
    .tag_get("schema-version")
    .ok_or("Cursor file is missing :schema-version")?
    .read_number()?;
  if (version - 1.0).abs() > f64::EPSILON && (version - f64::from(CURSOR_SCHEMA_VERSION)).abs() > f64::EPSILON {
    return Err(format!("Unsupported cursor schema version {version}."));
  }
  let active = root
    .tag_get("active")
    .ok_or("Cursor file is missing :active")?
    .read_tag_str()?
    .to_string();
  let cursors = root.tag_get("cursors").ok_or("Cursor file is missing :cursors")?.view_map()?;
  let entry = cursors
    .get(&Edn::tag(active.as_str()))
    .ok_or_else(|| format!("Cursor file has no entry for active cursor :{active}"))?
    .clone();
  let active = cursor_state_from_edn(&entry)?;
  let history = optional_state_list(&root, "history")?;
  let stack = optional_state_list(&root, "stack")?;
  let clipboard = match root.tag_get("clipboard") {
    None | Some(Edn::Nil) => None,
    Some(value) => Some(cursor_clipboard_from_edn(value)?),
  };

  Ok(Some(CursorDocument {
    active,
    history,
    stack,
    clipboard,
  }))
}

fn load_cursor_document(snapshot_file: &str) -> Result<CursorDocument, String> {
  load_cursor_document_optional(snapshot_file)?.ok_or_else(|| {
    format!(
      "No cursor is set. Use `cr cursor set <target> --path <path>` first (expected '{}').",
      cursor_file_path(snapshot_file).display()
    )
  })
}

#[cfg(test)]
fn load_cursor_state(snapshot_file: &str) -> Result<CursorState, String> {
  Ok(load_cursor_document(snapshot_file)?.active)
}

fn cursor_state_from_edn(value: &Edn) -> Result<CursorState, String> {
  let entry = value.view_map()?;
  let section = entry.tag_get("section").ok_or("Cursor entry is missing :section")?.read_tag_str()?;
  if section.as_ref() != "code" {
    return Err(format!("Unsupported cursor section :{section}; expected :code."));
  }
  let path = entry
    .tag_get("path")
    .ok_or("Cursor entry is missing :path")?
    .view_list()?
    .0
    .iter()
    .map(|value| {
      let number = value.read_number()?;
      if number < 0.0 || number.fract().abs() > f64::EPSILON {
        Err(format!("Cursor path index must be a non-negative integer, got {number}."))
      } else {
        Ok(number as usize)
      }
    })
    .collect::<Result<Vec<_>, _>>()?;

  Ok(CursorState {
    snapshot: required_string(&entry, "snapshot")?,
    target: required_string(&entry, "target")?,
    path,
    definition_revision: required_string(&entry, "definition-revision")?,
    fingerprint: required_string(&entry, "fingerprint")?,
    preview: entry
      .tag_get("preview")
      .ok_or("Cursor entry is missing :preview")?
      .read_quoted_cirru()?,
  })
}

fn cursor_clipboard_from_edn(value: &Edn) -> Result<CursorClipboard, String> {
  let entry = value.view_map()?;
  Ok(CursorClipboard {
    mode: required_string(&entry, "mode")?,
    source_target: required_string(&entry, "source-target")?,
    source_path: required_path(&entry, "source-path")?,
    fingerprint: required_string(&entry, "fingerprint")?,
    tree: entry
      .tag_get("tree")
      .ok_or("Cursor clipboard is missing :tree")?
      .read_quoted_cirru()?,
  })
}

fn optional_state_list(map: &cirru_edn::EdnMapView, key: &str) -> Result<Vec<CursorState>, String> {
  let Some(value) = map.tag_get(key) else {
    return Ok(vec![]);
  };
  value
    .view_list()?
    .0
    .iter()
    .map(cursor_state_from_edn)
    .collect::<Result<Vec<_>, _>>()
}

fn required_path(map: &cirru_edn::EdnMapView, key: &str) -> Result<Vec<usize>, String> {
  map
    .tag_get(key)
    .ok_or_else(|| format!("Cursor entry is missing :{key}"))?
    .view_list()?
    .0
    .iter()
    .map(|value| {
      let number = value.read_number()?;
      if number < 0.0 || number.fract().abs() > f64::EPSILON {
        Err(format!("Cursor path index must be a non-negative integer, got {number}."))
      } else {
        Ok(number as usize)
      }
    })
    .collect::<Result<Vec<_>, _>>()
}

fn required_string(map: &cirru_edn::EdnMapView, key: &str) -> Result<String, String> {
  map
    .tag_get(key)
    .ok_or_else(|| format!("Cursor entry is missing :{key}"))?
    .read_string()
}

fn cursor_state_to_edn(state: &CursorState) -> Edn {
  Edn::map_from_iter([
    (Edn::tag("snapshot"), Edn::str(state.snapshot.as_str())),
    (Edn::tag("target"), Edn::str(state.target.as_str())),
    (Edn::tag("section"), Edn::tag("code")),
    (
      Edn::tag("path"),
      Edn::List(EdnListView(state.path.iter().copied().map(Edn::from).collect())),
    ),
    (Edn::tag("definition-revision"), Edn::str(state.definition_revision.as_str())),
    (Edn::tag("fingerprint"), Edn::str(state.fingerprint.as_str())),
    (Edn::tag("preview"), Edn::Quote(state.preview.clone())),
  ])
}

fn cursor_clipboard_to_edn(clipboard: &CursorClipboard) -> Edn {
  Edn::map_from_iter([
    (Edn::tag("mode"), Edn::str(clipboard.mode.as_str())),
    (Edn::tag("source-target"), Edn::str(clipboard.source_target.as_str())),
    (
      Edn::tag("source-path"),
      Edn::List(EdnListView(clipboard.source_path.iter().copied().map(Edn::from).collect())),
    ),
    (Edn::tag("fingerprint"), Edn::str(clipboard.fingerprint.as_str())),
    (Edn::tag("tree"), Edn::Quote(clipboard.tree.clone())),
  ])
}

#[cfg(test)]
fn save_cursor_state(snapshot_file: &str, state: &CursorState) -> Result<(), String> {
  let mut document = load_cursor_document_optional(snapshot_file)?.unwrap_or_else(|| CursorDocument {
    active: state.clone(),
    history: vec![],
    stack: vec![],
    clipboard: None,
  });
  document.active = state.clone();
  save_cursor_document(snapshot_file, &document)
}

fn save_cursor_document(snapshot_file: &str, document: &CursorDocument) -> Result<(), String> {
  let content = cirru_edn::format(
    &Edn::map_from_iter([
      (Edn::tag("schema-version"), Edn::from(CURSOR_SCHEMA_VERSION)),
      (Edn::tag("active"), Edn::tag(ACTIVE_CURSOR)),
      (
        Edn::tag("cursors"),
        Edn::map_from_iter([(Edn::tag(ACTIVE_CURSOR), cursor_state_to_edn(&document.active))]),
      ),
      (
        Edn::tag("history"),
        Edn::List(EdnListView(document.history.iter().map(cursor_state_to_edn).collect())),
      ),
      (
        Edn::tag("stack"),
        Edn::List(EdnListView(document.stack.iter().map(cursor_state_to_edn).collect())),
      ),
      (
        Edn::tag("clipboard"),
        document.clipboard.as_ref().map(cursor_clipboard_to_edn).unwrap_or(Edn::Nil),
      ),
    ]),
    false,
  )
  .map_err(|error| format!("Failed to format cursor Cirru EDN: {error}"))?;

  let destination = cursor_file_path(snapshot_file);
  let parent = destination.parent().unwrap_or(Path::new("."));
  let nonce = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_err(|error| format!("System clock error while saving cursor: {error}"))?
    .as_nanos();
  let temporary = parent.join(format!(".calcit-cursor.{}.{nonce}.tmp", std::process::id()));
  let mut file = OpenOptions::new()
    .write(true)
    .create_new(true)
    .open(&temporary)
    .map_err(|error| format!("Failed to create cursor temporary file '{}': {error}", temporary.display()))?;
  file
    .write_all(content.as_bytes())
    .and_then(|_| file.write_all(b"\n"))
    .and_then(|_| file.sync_all())
    .map_err(|error| format!("Failed to write cursor temporary file '{}': {error}", temporary.display()))?;
  fs::rename(&temporary, &destination).map_err(|error| {
    let _ = fs::remove_file(&temporary);
    format!(
      "Failed to atomically replace cursor file '{}' with '{}': {error}",
      destination.display(),
      temporary.display()
    )
  })?;
  Ok(())
}

fn warn_cursor_gitignore(snapshot_file: &str) {
  let cursor_path = cursor_file_path(snapshot_file);
  let project_dir = cursor_path.parent().unwrap_or(Path::new("."));
  let gitignore = project_dir.join(".gitignore");
  let ignored = fs::read_to_string(&gitignore).is_ok_and(|content| {
    content.lines().any(|line| {
      let pattern = line.trim();
      matches!(
        pattern,
        ".calcit-cursor.cirru" | "/.calcit-cursor.cirru" | "**/.calcit-cursor.cirru" | "*.cirru"
      )
    })
  });
  if !ignored {
    eprintln!(
      "{} Add `.calcit-cursor.cirru` to '{}' so local cursor state is not committed.",
      "[Cursor]".yellow().bold(),
      gitignore.display()
    );
  }
}

fn is_prefix(prefix: &[usize], path: &[usize]) -> bool {
  path.starts_with(prefix)
}

fn shift_sibling(path: &mut [usize], mutation_path: &[usize], include_equal: bool, delta: isize) {
  if mutation_path.is_empty() || path.len() < mutation_path.len() {
    return;
  }
  let depth = mutation_path.len() - 1;
  if path[..depth] != mutation_path[..depth] {
    return;
  }
  let threshold = mutation_path[depth];
  if path[depth] > threshold || (include_equal && path[depth] == threshold) {
    path[depth] = path[depth].saturating_add_signed(delta);
  }
}

fn transform_delete(cursor: &[usize], deleted: &[usize]) -> (Vec<usize>, &'static str) {
  if is_prefix(deleted, cursor) {
    return (
      deleted.get(..deleted.len().saturating_sub(1)).unwrap_or(&[]).to_vec(),
      "selected subtree was deleted; moved to parent",
    );
  }
  let mut next = cursor.to_vec();
  shift_sibling(&mut next, deleted, false, -1);
  if next == cursor {
    (next, "deletion did not shift cursor")
  } else {
    (next, "sibling deleted before cursor")
  }
}

fn transform_cursor_path(cursor: &[usize], mutation: &TreeCursorMutation) -> (Vec<usize>, &'static str) {
  match mutation {
    TreeCursorMutation::NoPathShift => (cursor.to_vec(), "tree content changed without shifting cursor path"),
    TreeCursorMutation::Replace { path } => {
      if is_prefix(path, cursor) && cursor.len() > path.len() {
        (path.clone(), "cursor ancestor was replaced; moved to replacement root")
      } else {
        (cursor.to_vec(), "selected node was refreshed after replacement")
      }
    }
    TreeCursorMutation::InsertBefore { path } => {
      let mut next = cursor.to_vec();
      shift_sibling(&mut next, path, true, 1);
      if next == cursor {
        (next, "insertion did not shift cursor")
      } else {
        (next, "node inserted before cursor")
      }
    }
    TreeCursorMutation::InsertAfter { path } => {
      let mut next = cursor.to_vec();
      shift_sibling(&mut next, path, false, 1);
      if next == cursor {
        (next, "insertion did not shift cursor")
      } else {
        (next, "node inserted before cursor")
      }
    }
    TreeCursorMutation::InsertChild { path } => {
      let mut next = cursor.to_vec();
      if is_prefix(path, cursor) && cursor.len() > path.len() {
        next[path.len()] += 1;
      }
      if next == cursor {
        (next, "child insertion did not shift cursor")
      } else {
        (next, "first child inserted before cursor descendant")
      }
    }
    TreeCursorMutation::Delete { path } => transform_delete(cursor, path),
    TreeCursorMutation::SwapNext { path } | TreeCursorMutation::SwapPrev { path } => {
      if path.is_empty() || cursor.len() < path.len() {
        return (cursor.to_vec(), "sibling swap did not affect cursor");
      }
      let depth = path.len() - 1;
      if cursor[..depth] != path[..depth] {
        return (cursor.to_vec(), "sibling swap did not affect cursor");
      }
      let other = match mutation {
        TreeCursorMutation::SwapNext { .. } => path[depth] + 1,
        TreeCursorMutation::SwapPrev { .. } => path[depth].saturating_sub(1),
        _ => unreachable!(),
      };
      let mut next = cursor.to_vec();
      if cursor[depth] == path[depth] {
        next[depth] = other;
      } else if cursor[depth] == other {
        next[depth] = path[depth];
      }
      (next, "cursor followed swapped subtree")
    }
    TreeCursorMutation::Unwrap { path, child_count } => {
      if path.is_empty() {
        return (cursor.to_vec(), "root unwrap is unsupported");
      }
      let parent = &path[..path.len() - 1];
      let wrapper_index = path[path.len() - 1];
      if is_prefix(path, cursor) {
        if cursor.len() == path.len() {
          return (parent.to_vec(), "selected wrapper was removed; moved to parent");
        }
        let mut next = parent.to_vec();
        next.push(wrapper_index + cursor[path.len()]);
        next.extend_from_slice(&cursor[path.len() + 1..]);
        return (next, "cursor followed child out of unwrapped node");
      }
      let mut next = cursor.to_vec();
      if cursor.len() >= path.len() && cursor[..parent.len()] == *parent && cursor[parent.len()] > wrapper_index {
        next[parent.len()] = next[parent.len()].saturating_add(child_count.saturating_sub(1));
      }
      (next, "unwrapped siblings shifted cursor")
    }
    TreeCursorMutation::Raise { path } => {
      if path.is_empty() {
        return (cursor.to_vec(), "root raise is unsupported");
      }
      let parent = &path[..path.len() - 1];
      if is_prefix(path, cursor) {
        let mut next = parent.to_vec();
        next.extend_from_slice(&cursor[path.len()..]);
        (next, "cursor followed raised subtree")
      } else if is_prefix(parent, cursor) {
        (parent.to_vec(), "cursor subtree was discarded by raise; moved to raised root")
      } else {
        (cursor.to_vec(), "raise did not affect cursor")
      }
    }
    TreeCursorMutation::Wrap { path } => {
      if is_prefix(path, cursor) && cursor.len() > path.len() {
        (path.clone(), "cursor ancestor was wrapped; moved to wrapper root")
      } else {
        (cursor.to_vec(), "cursor now selects wrapper")
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{
    CursorClipboard, CursorDocument, CursorState, RestoreSource, TreeCursorMutation, build_cursor_preview, cursor_file_path,
    cursor_state_to_edn, load_cursor_document, load_cursor_state, maintain_cursor_after_node_move, maintain_cursor_after_tree_mutation,
    move_cursor_across_siblings, move_cursor_to_child, node_fingerprint, paste_cursor_clipboard, read_cursor_target, restore_cursor,
    save_cursor_document, save_cursor_state, set_cursor_selection, store_cursor_clipboard, transform_cursor_path,
  };
  use crate::cli_handlers::edit::{apply_operation_at_path, load_snapshot, save_snapshot};
  use cirru_edn::Edn;
  use cirru_parser::Cirru;
  use std::fs;
  use std::path::PathBuf;
  use std::time::{SystemTime, UNIX_EPOCH};

  struct TestCursorSnapshot {
    directory: PathBuf,
    snapshot: PathBuf,
  }

  impl TestCursorSnapshot {
    fn from_fixture() -> Self {
      let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("test clock should be valid")
        .as_nanos();
      let directory = std::env::temp_dir().join(format!("calcit-cursor-test-{}-{nonce}", std::process::id()));
      fs::create_dir(&directory).expect("cursor test directory should be created");
      let snapshot = directory.join("calcit.cirru");
      fs::copy("calcit/test.cirru", &snapshot).expect("cursor fixture should copy");
      Self { directory, snapshot }
    }

    fn snapshot_string(&self) -> String {
      self.snapshot.to_string_lossy().into_owned()
    }
  }

  impl Drop for TestCursorSnapshot {
    fn drop(&mut self) {
      let _ = fs::remove_dir_all(&self.directory);
    }
  }

  #[test]
  fn insertion_before_cursor_shifts_matching_sibling_branch() {
    let mutation = TreeCursorMutation::InsertBefore { path: vec![3, 2] };
    assert_eq!(transform_cursor_path(&[3, 4, 1], &mutation).0, vec![3, 5, 1]);
    assert_eq!(transform_cursor_path(&[2, 4], &mutation).0, vec![2, 4]);
  }

  #[test]
  fn deletion_before_cursor_shifts_and_selected_delete_moves_parent() {
    let before = TreeCursorMutation::Delete { path: vec![3, 2] };
    assert_eq!(transform_cursor_path(&[3, 4, 1], &before).0, vec![3, 3, 1]);

    let selected = TreeCursorMutation::Delete { path: vec![3, 4] };
    assert_eq!(transform_cursor_path(&[3, 4, 1], &selected).0, vec![3]);
  }

  #[test]
  fn swap_and_unwrap_keep_cursor_attached_to_subtree() {
    let swap = TreeCursorMutation::SwapPrev { path: vec![3, 4] };
    assert_eq!(transform_cursor_path(&[3, 3, 1], &swap).0, vec![3, 4, 1]);

    let unwrap = TreeCursorMutation::Unwrap {
      path: vec![3, 2],
      child_count: 3,
    };
    assert_eq!(transform_cursor_path(&[3, 2, 1, 4], &unwrap).0, vec![3, 3, 4]);
    assert_eq!(transform_cursor_path(&[3, 5], &unwrap).0, vec![3, 7]);
  }

  #[test]
  fn cursor_state_round_trips_as_cirru_edn() {
    let fixture = TestCursorSnapshot::from_fixture();
    let snapshot_file = fixture.snapshot_string();
    let path = vec![48, 1];
    let (node, revision) = read_cursor_target(&snapshot_file, "app.main/main!", &path).expect("fixture cursor target should exist");
    let state = CursorState {
      snapshot: snapshot_file.clone(),
      target: "app.main/main!".to_string(),
      path,
      definition_revision: revision,
      fingerprint: node_fingerprint(&node),
      preview: node,
    };

    save_cursor_state(&snapshot_file, &state).expect("cursor state should save");
    assert_eq!(load_cursor_state(&snapshot_file).expect("cursor state should load"), state);
  }

  #[test]
  fn persisted_cursor_moves_after_insertion_before_it() {
    let fixture = TestCursorSnapshot::from_fixture();
    let snapshot_file = fixture.snapshot_string();
    let cursor_path = vec![48, 1];
    let (node, revision) =
      read_cursor_target(&snapshot_file, "app.main/main!", &cursor_path).expect("fixture cursor target should exist");
    save_cursor_state(
      &snapshot_file,
      &CursorState {
        snapshot: snapshot_file.clone(),
        target: "app.main/main!".to_string(),
        path: cursor_path,
        definition_revision: revision,
        fingerprint: node_fingerprint(&node),
        preview: node,
      },
    )
    .expect("cursor state should save");

    let mut snapshot = load_snapshot(&snapshot_file).expect("fixture snapshot should load");
    let entry = snapshot
      .files
      .get_mut("app.main")
      .and_then(|file| file.defs.get_mut("main!"))
      .expect("fixture main definition should exist");
    entry.code = apply_operation_at_path(&entry.code, &[48, 1], "insert-before", Some(&cirru_parser::Cirru::leaf("false")))
      .expect("fixture insertion should succeed");
    save_snapshot(&snapshot, &snapshot_file).expect("mutated fixture should save");

    maintain_cursor_after_tree_mutation(
      &snapshot_file,
      "app.main/main!",
      &TreeCursorMutation::InsertBefore { path: vec![48, 1] },
    )
    .expect("cursor maintenance should succeed");
    let updated = load_cursor_state(&snapshot_file).expect("updated cursor should load");
    assert_eq!(updated.path, vec![48, 2]);
    assert_eq!(updated.preview, cirru_parser::Cirru::leaf("true"));
  }

  #[test]
  fn cursor_document_round_trips_history_stack_and_clipboard() {
    let fixture = TestCursorSnapshot::from_fixture();
    let snapshot_file = fixture.snapshot_string();
    let path = vec![48, 1];
    let (node, revision) = read_cursor_target(&snapshot_file, "app.main/main!", &path).expect("fixture cursor target should exist");
    let state = CursorState {
      snapshot: snapshot_file.clone(),
      target: "app.main/main!".to_string(),
      path: path.clone(),
      definition_revision: revision,
      fingerprint: node_fingerprint(&node),
      preview: node.clone(),
    };
    let document = CursorDocument {
      active: state.clone(),
      history: vec![state.clone()],
      stack: vec![state.clone()],
      clipboard: Some(CursorClipboard {
        mode: "copy".to_string(),
        source_target: state.target.clone(),
        source_path: path,
        fingerprint: node_fingerprint(&node),
        tree: node,
      }),
    };

    save_cursor_document(&snapshot_file, &document).expect("cursor document should save");
    assert_eq!(load_cursor_document(&snapshot_file).expect("cursor document should load"), document);
  }

  #[test]
  fn legacy_v1_cursor_document_loads_with_empty_extensions() {
    let fixture = TestCursorSnapshot::from_fixture();
    let snapshot_file = fixture.snapshot_string();
    let path = vec![48, 1];
    let (node, revision) = read_cursor_target(&snapshot_file, "app.main/main!", &path).expect("fixture cursor target should exist");
    let state = CursorState {
      snapshot: snapshot_file.clone(),
      target: "app.main/main!".to_string(),
      path,
      definition_revision: revision,
      fingerprint: node_fingerprint(&node),
      preview: node,
    };
    let legacy = cirru_edn::format(
      &Edn::map_from_iter([
        (Edn::tag("schema-version"), Edn::from(1_u8)),
        (Edn::tag("active"), Edn::tag("main")),
        (
          Edn::tag("cursors"),
          Edn::map_from_iter([(Edn::tag("main"), cursor_state_to_edn(&state))]),
        ),
      ]),
      false,
    )
    .expect("legacy cursor should format");
    fs::write(cursor_file_path(&snapshot_file), legacy).expect("legacy cursor should write");

    let loaded = load_cursor_document(&snapshot_file).expect("legacy cursor should load");
    assert_eq!(loaded.active, state);
    assert!(loaded.history.is_empty());
    assert!(loaded.stack.is_empty());
    assert!(loaded.clipboard.is_none());
  }

  #[test]
  fn focused_preview_preserves_definition_signature() {
    let fixture = TestCursorSnapshot::from_fixture();
    let snapshot_file = fixture.snapshot_string();
    let path = vec![48, 1];
    let (node, revision) = read_cursor_target(&snapshot_file, "app.main/main!", &path).expect("fixture cursor target should exist");
    let state = CursorState {
      snapshot: snapshot_file.clone(),
      target: "app.main/main!".to_string(),
      path,
      definition_revision: revision,
      fingerprint: node_fingerprint(&node),
      preview: node,
    };

    let preview = build_cursor_preview(&snapshot_file, &state, "focus").expect("focused preview should build");
    let Cirru::List(items) = preview else {
      panic!("definition preview should remain a list")
    };
    assert_eq!(items[0], Cirru::leaf("defn"));
    assert_eq!(items[1], Cirru::leaf("main!"));
    assert!(matches!(&items[2], Cirru::List(_)));
    let rendered = cirru_parser::format(&[Cirru::List(items)], false.into()).expect("focused preview should render");
    assert!(rendered.contains("CURSOR"));
    assert!(!rendered.contains("FOCUSED"));
  }

  #[test]
  fn cut_and_paste_round_trip_tree_and_cursor_clipboard() {
    let fixture = TestCursorSnapshot::from_fixture();
    let snapshot_file = fixture.snapshot_string();
    let path = vec![48, 1];
    let (node, revision) = read_cursor_target(&snapshot_file, "app.main/main!", &path).expect("fixture cursor target should exist");
    save_cursor_state(
      &snapshot_file,
      &CursorState {
        snapshot: snapshot_file.clone(),
        target: "app.main/main!".to_string(),
        path: path.clone(),
        definition_revision: revision,
        fingerprint: node_fingerprint(&node),
        preview: node.clone(),
      },
    )
    .expect("cursor state should save");

    store_cursor_clipboard(&snapshot_file, "cut", true).expect("cursor cut should succeed");
    let cut = load_cursor_document(&snapshot_file).expect("cut cursor document should load");
    assert_eq!(cut.active.path, vec![48]);
    assert_eq!(cut.clipboard.as_ref().map(|value| &value.tree), Some(&node));

    paste_cursor_clipboard(&snapshot_file, "append-child").expect("cursor paste should succeed");
    let pasted = load_cursor_document(&snapshot_file).expect("pasted cursor document should load");
    assert_eq!(pasted.active.path, path);
    assert_eq!(
      read_cursor_target(&snapshot_file, "app.main/main!", &path)
        .expect("pasted node should exist")
        .0,
      node
    );
  }

  #[test]
  fn cursor_follows_moved_subtree_even_when_fingerprint_is_duplicated() {
    let fixture = TestCursorSnapshot::from_fixture();
    let snapshot_file = fixture.snapshot_string();
    let path = vec![48, 1];
    let (node, revision) = read_cursor_target(&snapshot_file, "app.main/main!", &path).expect("fixture cursor target should exist");
    save_cursor_state(
      &snapshot_file,
      &CursorState {
        snapshot: snapshot_file.clone(),
        target: "app.main/main!".to_string(),
        path: path.clone(),
        definition_revision: revision,
        fingerprint: node_fingerprint(&node),
        preview: node.clone(),
      },
    )
    .expect("cursor state should save");

    let mut snapshot = load_snapshot(&snapshot_file).expect("fixture snapshot should load");
    let entry = snapshot
      .files
      .get_mut("app.main")
      .and_then(|file| file.defs.get_mut("main!"))
      .expect("fixture main definition should exist");
    let duplicated = apply_operation_at_path(&entry.code, &[48, 1], "insert-after", Some(&node)).expect("duplicate should insert");
    let after_move_insert =
      apply_operation_at_path(&duplicated, &[48, 0], "insert-after", Some(&node)).expect("move destination should insert");
    entry.code = apply_operation_at_path(&after_move_insert, &[48, 2], "delete", None).expect("old source should delete");
    save_snapshot(&snapshot, &snapshot_file).expect("mutated fixture should save");

    maintain_cursor_after_node_move(&snapshot_file, "app.main/main!", &[48, 1], &[48, 0], "insert-after", 0)
      .expect("cursor should follow the moved duplicate deterministically");
    assert_eq!(load_cursor_state(&snapshot_file).expect("moved cursor should load").path, path);
  }

  #[test]
  fn cursor_navigation_supports_last_child_sibling_counts_and_multi_back() {
    let fixture = TestCursorSnapshot::from_fixture();
    let snapshot_file = fixture.snapshot_string();
    set_cursor_selection(&snapshot_file, "app.main/main!", vec![48]).expect("cursor should select parent list");

    move_cursor_to_child(&snapshot_file, None, true).expect("cursor should enter the last child");
    assert_eq!(
      load_cursor_state(&snapshot_file).expect("last child cursor should load").path,
      vec![48, 1]
    );

    restore_cursor(&snapshot_file, RestoreSource::History, 1).expect("cursor should return to parent");
    move_cursor_to_child(&snapshot_file, Some(0), false).expect("cursor should enter the first child");
    move_cursor_across_siblings(&snapshot_file, 1, true).expect("cursor should skip one sibling forward");
    assert_eq!(
      load_cursor_state(&snapshot_file).expect("next cursor should load").path,
      vec![48, 1]
    );

    restore_cursor(&snapshot_file, RestoreSource::History, 2).expect("cursor should rewind two recorded locations");
    assert_eq!(
      load_cursor_state(&snapshot_file).expect("rewound cursor should load").path,
      vec![48]
    );

    set_cursor_selection(&snapshot_file, "app.main/main!", vec![48, 0]).expect("cursor should select first sibling");
    let error = move_cursor_across_siblings(&snapshot_file, 2, true).expect_err("out-of-range skip should fail");
    assert!(error.contains("only 1 next sibling"), "error: {error}");
    assert_eq!(
      load_cursor_state(&snapshot_file).expect("failed move should preserve cursor").path,
      vec![48, 0]
    );
  }
}
