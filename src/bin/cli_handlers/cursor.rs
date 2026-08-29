use calcit::cli_args::{
  CursorApplyCommand, CursorCommand, CursorDuplicateCommand, CursorSubcommand, EditCommand, EditMvNodeCommand, EditSubcommand,
  TreeAppendChildCommand, TreeCommand, TreeDeleteCommand, TreeInsertAfterCommand, TreeInsertBeforeCommand, TreeInsertChildCommand,
  TreeRaiseCommand, TreeReplaceCommand, TreeSubcommand, TreeSwapNextCommand, TreeSwapPrevCommand, TreeUnwrapCommand, TreeWrapCommand,
};
use calcit::project_state::{self, CURSOR_STATE_FILE};
use calcit::snapshot;
use cirru_edn::{Edn, EdnListView};
use cirru_parser::Cirru;
use colored::Colorize;
use md5::{Digest, Md5};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};

use super::atomic_write::{StagedFile, stage_atomic_file};
use super::common::{cirru_to_json_value, format_path, guard_snapshot_mutation_toolchain, parse_path};
use super::edit::{apply_operation_at_path, check_ns_editable, load_snapshot, navigate_to_path, parse_target};
use super::tree_mutation::{
  TreeCursorMutation, TreeOperation, adjusted_source_path_after_insertion, transform_cursor_path, transform_delete,
};

const LEGACY_CURSOR_FILE: &str = ".calcit-cursor.cirru";
const DEFAULT_CURSOR: &str = "default";
const CURSOR_MAINTENANCE_ENV: &str = "CALCIT_CURSOR_MAINTENANCE";
const CURSOR_SCHEMA_VERSION: u8 = 4;
const CURSOR_HISTORY_LIMIT: usize = 32;
const CURSOR_STACK_LIMIT: usize = 16;
const CURSOR_MARK_LIMIT: usize = 16;
const CURSOR_FILE_MAX_BYTES: usize = 64 * 1024;

/// 0 = none, 1 = summary, 2 = focus.
static CURSOR_AFTER_MODE: AtomicU8 = AtomicU8::new(1);

#[derive(Debug, Clone, PartialEq)]
struct CursorState {
  snapshot: String,
  target: String,
  path: Vec<usize>,
  definition_revision: String,
  fingerprint: String,
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
pub(crate) struct CursorLastQuery {
  pub command: String,
  pub pattern: String,
  pub filter: Option<String>,
  pub exact: bool,
  pub regex: bool,
  pub max_depth: usize,
  pub start_path: Option<String>,
  pub entry: Option<String>,
  pub pattern_is_json: bool,
  pub selected_index: usize,
  pub snapshot_revision: String,
}

#[derive(Debug, Clone, PartialEq)]
struct CursorDocument {
  active_name: String,
  active: CursorState,
  inactive_cursors: BTreeMap<String, CursorState>,
  history: Vec<CursorState>,
  stack: Vec<CursorState>,
  anchor: Option<CursorState>,
  marks: BTreeMap<String, CursorState>,
  last_query: Option<CursorLastQuery>,
  clipboard: Option<CursorClipboard>,
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
  if matches!(
    &cmd.subcommand,
    CursorSubcommand::Cut(_)
      | CursorSubcommand::Paste(_)
      | CursorSubcommand::Apply(_)
      | CursorSubcommand::SlurpNext(_)
      | CursorSubcommand::SlurpPrev(_)
      | CursorSubcommand::BarfLast(_)
      | CursorSubcommand::BarfFirst(_)
      | CursorSubcommand::Duplicate(_)
  ) {
    guard_snapshot_mutation_toolchain(snapshot_file)?;
  }
  match &cmd.subcommand {
    CursorSubcommand::Set(opts) => {
      if opts.path == "@cursor" {
        return Err("`cursor set --path` requires a concrete path, not `@cursor`.".to_string());
      }
      let path = parse_path(&opts.path)?;
      let state = set_cursor_selection(snapshot_file, &opts.target, path)?;
      warn_cursor_gitignore(snapshot_file);
      println!("{} Cursor set: {} {}", "✓".green(), state.target, format_path(&state.path));
      emit_focus_after_cursor_action(snapshot_file, &state, "cursor selected")
    }
    CursorSubcommand::Show(opts) => handle_cursor_show(snapshot_file, &opts.format, &opts.view),
    CursorSubcommand::Clear(_) => {
      let file = cursor_file_path(snapshot_file);
      let legacy_file = legacy_cursor_file_path(snapshot_file);
      let mut removed = false;
      if file.exists() {
        fs::remove_file(&file).map_err(|error| format!("Failed to remove cursor file '{}': {error}", file.display()))?;
        removed = true;
      }
      if legacy_file.exists() {
        fs::remove_file(&legacy_file)
          .map_err(|error| format!("Failed to remove legacy cursor file '{}': {error}", legacy_file.display()))?;
        removed = true;
      }
      if removed {
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
    CursorSubcommand::Anchor(_) => set_cursor_anchor(snapshot_file),
    CursorSubcommand::ClearAnchor(_) => clear_cursor_anchor(snapshot_file),
    CursorSubcommand::Region(opts) => show_cursor_region(snapshot_file, &opts.format),
    CursorSubcommand::Mark(opts) => save_cursor_mark(snapshot_file, &opts.name),
    CursorSubcommand::Goto(opts) => goto_cursor_mark(snapshot_file, &opts.name),
    CursorSubcommand::Marks(opts) => show_cursor_marks(snapshot_file, &opts.format),
    CursorSubcommand::RmMark(opts) => remove_cursor_mark(snapshot_file, &opts.name),
    CursorSubcommand::Copy(_) => store_cursor_clipboard(snapshot_file, "copy", false),
    CursorSubcommand::Cut(_) => store_cursor_clipboard(snapshot_file, "cut", true),
    CursorSubcommand::Paste(opts) => paste_cursor_clipboard(snapshot_file, &opts.at),
    CursorSubcommand::Clipboard(opts) => show_cursor_clipboard(snapshot_file, &opts.format),
    CursorSubcommand::ClearClipboard(_) => clear_cursor_clipboard(snapshot_file),
    CursorSubcommand::Apply(opts) => apply_at_cursor(snapshot_file, opts),
    CursorSubcommand::SlurpNext(_) => slurp_next(snapshot_file),
    CursorSubcommand::SlurpPrev(_) => slurp_prev(snapshot_file),
    CursorSubcommand::BarfLast(_) => barf_last(snapshot_file),
    CursorSubcommand::BarfFirst(_) => barf_first(snapshot_file),
    CursorSubcommand::Duplicate(opts) => duplicate_cursor(snapshot_file, opts),
    CursorSubcommand::Forward(opts) => move_cursor_depth_first(snapshot_file, opts.count, true),
    CursorSubcommand::Backward(opts) => move_cursor_depth_first(snapshot_file, opts.count, false),
  }
}

fn set_cursor_selection(snapshot_file: &str, target: &str, path: Vec<usize>) -> Result<CursorState, String> {
  set_cursor_selection_with_last_query(snapshot_file, target, path, None)
}

fn set_cursor_selection_with_last_query(
  snapshot_file: &str,
  target: &str,
  path: Vec<usize>,
  last_query: Option<CursorLastQuery>,
) -> Result<CursorState, String> {
  let mut state = CursorState {
    snapshot: snapshot_file.to_string(),
    target: target.to_string(),
    path,
    definition_revision: String::new(),
    fingerprint: String::new(),
  };
  refresh_cursor_state(&mut state, snapshot_file)?;
  let mut document = load_cursor_document_optional(snapshot_file)?.unwrap_or_else(|| CursorDocument {
    active_name: DEFAULT_CURSOR.to_string(),
    active: state.clone(),
    inactive_cursors: BTreeMap::new(),
    history: vec![],
    stack: vec![],
    anchor: None,
    marks: BTreeMap::new(),
    last_query: None,
    clipboard: None,
  });
  if document.active.target != state.target || document.active.path != state.path {
    push_bounded(&mut document.history, document.active.clone(), CURSOR_HISTORY_LIMIT);
  }
  document.active = state.clone();
  if let Some(last_query) = last_query {
    document.last_query = Some(last_query);
  }
  save_cursor_document(snapshot_file, &document)?;
  Ok(state)
}

pub(crate) fn set_cursor_from_query_match(
  snapshot_file: &str,
  target: &str,
  path: Vec<usize>,
  match_index: usize,
  last_query: CursorLastQuery,
) -> Result<(), String> {
  let state = set_cursor_selection_with_last_query(snapshot_file, target, path, Some(last_query))
    .map_err(|error| format!("Search match #{match_index} cannot become the project cursor: {error}"))?;
  warn_cursor_gitignore(snapshot_file);
  eprintln!(
    "{} Cursor set from search match #{}: {} {}",
    "✓".green(),
    match_index,
    state.target,
    format_path(&state.path)
  );
  emit_focus_after_cursor_action(snapshot_file, &state, "cursor selected from search")
}

pub(crate) fn load_cursor_last_query(snapshot_file: &str) -> Result<CursorLastQuery, String> {
  load_cursor_document(snapshot_file)?
    .last_query
    .ok_or("No repeatable cursor search is saved. Run `calcit query search ... --set-cursor <index>` first.".to_string())
}

fn concrete_cursor_path(path: &[usize]) -> String {
  if path.is_empty() { String::new() } else { format_path(path) }
}

fn apply_at_cursor(snapshot_file: &str, opts: &CursorApplyCommand) -> Result<(), String> {
  let state = validate_cursor(snapshot_file, true)?;
  let target = state.target;
  let path = concrete_cursor_path(&state.path);
  let accepts_code = matches!(
    opts.operation.as_str(),
    "replace" | "wrap" | "insert-before" | "insert-after" | "insert-child" | "append-child"
  );
  if !accepts_code && (opts.file.is_some() || opts.code.is_some()) {
    return Err(format!("Cursor operation '{}' does not accept --file or --code.", opts.operation));
  }

  let subcommand = match opts.operation.as_str() {
    "delete" => TreeSubcommand::Delete(TreeDeleteCommand {
      target,
      path,
      expect: None,
      depth: opts.depth,
    }),
    "swap-next" => TreeSubcommand::SwapNext(TreeSwapNextCommand {
      target,
      path,
      depth: opts.depth,
    }),
    "swap-prev" => TreeSubcommand::SwapPrev(TreeSwapPrevCommand {
      target,
      path,
      depth: opts.depth,
    }),
    "unwrap" | "splice" => TreeSubcommand::Unwrap(TreeUnwrapCommand {
      target,
      path,
      depth: opts.depth,
    }),
    "raise" => TreeSubcommand::Raise(TreeRaiseCommand {
      target,
      path,
      depth: opts.depth,
    }),
    "replace" => TreeSubcommand::Replace(TreeReplaceCommand {
      target,
      path,
      file: opts.file.clone(),
      code: opts.code.clone(),
      expect: None,
      depth: opts.depth,
    }),
    "wrap" => TreeSubcommand::Wrap(TreeWrapCommand {
      target,
      path,
      code: opts.code.clone(),
      file: opts.file.clone(),
      depth: opts.depth,
    }),
    "insert-before" => TreeSubcommand::InsertBefore(TreeInsertBeforeCommand {
      target,
      path,
      file: opts.file.clone(),
      code: opts.code.clone(),
      expect: None,
      depth: opts.depth,
    }),
    "insert-after" => TreeSubcommand::InsertAfter(TreeInsertAfterCommand {
      target,
      path,
      file: opts.file.clone(),
      code: opts.code.clone(),
      expect: None,
      depth: opts.depth,
    }),
    "insert-child" => TreeSubcommand::InsertChild(TreeInsertChildCommand {
      target,
      path,
      file: opts.file.clone(),
      code: opts.code.clone(),
      expect: None,
      depth: opts.depth,
    }),
    "append-child" => TreeSubcommand::AppendChild(TreeAppendChildCommand {
      target,
      path,
      file: opts.file.clone(),
      code: opts.code.clone(),
      expect: None,
      depth: opts.depth,
    }),
    other => {
      return Err(format!(
        "Unsupported cursor operation '{other}'. Use delete, swap-next, swap-prev, unwrap, splice, raise, replace, wrap, insert-before, insert-after, insert-child, or append-child."
      ));
    }
  };
  super::tree::handle_tree_command(&TreeCommand { subcommand }, snapshot_file)
}

fn move_node_for_paredit(snapshot_file: &str, state: &CursorState, from_path: &[usize], at: &str) -> Result<(), String> {
  super::edit::handle_edit_command(
    &EditCommand {
      subcommand: EditSubcommand::Mv(EditMvNodeCommand {
        target: state.target.clone(),
        from: concrete_cursor_path(from_path),
        path: concrete_cursor_path(&state.path),
        at: at.to_string(),
      }),
    },
    snapshot_file,
  )
}

fn slurp_next(snapshot_file: &str) -> Result<(), String> {
  let state = validate_cursor(snapshot_file, true)?;
  if state.path.is_empty() {
    return Err("Definition root has no next sibling to slurp.".to_string());
  }
  let (selected, _) = read_cursor_target(snapshot_file, &state.target, &state.path)?;
  if !matches!(selected, Cirru::List(_)) {
    return Err("`cursor slurp-next` requires the selected node to be a list.".to_string());
  }
  let mut next_path = state.path.clone();
  let current_index = *next_path.last().expect("non-root cursor has a final index");
  let parent_path = &state.path[..state.path.len() - 1];
  let (parent, _) = read_cursor_target(snapshot_file, &state.target, parent_path)?;
  let Cirru::List(siblings) = parent else {
    return Err("Cursor parent is not a list and has no sibling sequence.".to_string());
  };
  if current_index + 1 >= siblings.len() {
    return Err("Selected list has no next sibling to slurp.".to_string());
  }
  *next_path.last_mut().expect("non-root cursor has a final index") += 1;
  move_node_for_paredit(snapshot_file, &state, &next_path, "append-child")
}

fn slurp_prev(snapshot_file: &str) -> Result<(), String> {
  let state = validate_cursor(snapshot_file, true)?;
  if state.path.is_empty() {
    return Err("Definition root has no previous sibling to slurp.".to_string());
  }
  let (selected, _) = read_cursor_target(snapshot_file, &state.target, &state.path)?;
  if !matches!(selected, Cirru::List(_)) {
    return Err("`cursor slurp-prev` requires the selected node to be a list.".to_string());
  }
  let mut previous_path = state.path.clone();
  let current_index = *previous_path.last().expect("non-root cursor has a final index");
  if current_index == 0 {
    return Err("Selected list has no previous sibling to slurp.".to_string());
  }
  *previous_path.last_mut().expect("non-root cursor has a final index") -= 1;
  move_node_for_paredit(snapshot_file, &state, &previous_path, "prepend-child")
}

fn barf_last(snapshot_file: &str) -> Result<(), String> {
  let state = validate_cursor(snapshot_file, true)?;
  if state.path.is_empty() {
    return Err("Cannot barf a child out of the definition root.".to_string());
  }
  let (selected, _) = read_cursor_target(snapshot_file, &state.target, &state.path)?;
  let Cirru::List(children) = selected else {
    return Err("`cursor barf-last` requires the selected node to be a list.".to_string());
  };
  let Some(last_index) = children.len().checked_sub(1) else {
    return Err("Selected list has no child to barf.".to_string());
  };
  let mut child_path = state.path.clone();
  child_path.push(last_index);
  move_node_for_paredit(snapshot_file, &state, &child_path, "after")
}

fn barf_first(snapshot_file: &str) -> Result<(), String> {
  let state = validate_cursor(snapshot_file, true)?;
  if state.path.is_empty() {
    return Err("Cannot barf a child out of the definition root.".to_string());
  }
  let (selected, _) = read_cursor_target(snapshot_file, &state.target, &state.path)?;
  let Cirru::List(children) = selected else {
    return Err("`cursor barf-first` requires the selected node to be a list.".to_string());
  };
  if children.is_empty() {
    return Err("Selected list has no child to barf.".to_string());
  }
  let mut child_path = state.path.clone();
  child_path.push(0);
  let first_child = children[0].clone();
  let mut document = load_cursor_document(snapshot_file)?;
  let (namespace, definition) = parse_target(&state.target)?;
  let mut snapshot = load_snapshot(snapshot_file)?;
  check_ns_editable(&snapshot, namespace)?;
  let entry = snapshot
    .files
    .get_mut(namespace)
    .and_then(|file| file.defs.get_mut(definition))
    .ok_or_else(|| format!("Definition '{}' not found", state.target))?;
  let without_first = apply_operation_at_path(&entry.code, &child_path, TreeOperation::Delete, None)?;
  entry.code = apply_operation_at_path(&without_first, &state.path, TreeOperation::InsertBefore, Some(&first_child))?;

  let mut selected_path = state.path.clone();
  *selected_path.last_mut().expect("root was rejected above") += 1;
  push_bounded(&mut document.history, state.clone(), CURSOR_HISTORY_LIMIT);
  document.active.path = selected_path;
  refresh_cursor_state_from_snapshot(&mut document.active, snapshot_file, &snapshot)?;

  let staged_snapshot = stage_snapshot(snapshot_file, &snapshot)?;
  let staged_cursor = stage_cursor_document(snapshot_file, &document)?;
  commit_snapshot_then_cursor_staged_files(staged_snapshot, staged_cursor, "Barf-first", false)?;
  println!(
    "{} Moved the selected list's first child out before it; cursor now at {}",
    "✓".green(),
    format_path(&document.active.path)
  );
  emit_cursor_after(snapshot_file, &document.active, "cursor follows list after barf-first")?;
  Ok(())
}

fn duplicate_cursor(snapshot_file: &str, opts: &CursorDuplicateCommand) -> Result<(), String> {
  if !matches!(opts.at.as_str(), "before" | "after") {
    return Err(format!("Unsupported duplicate position '{}'. Use before or after.", opts.at));
  }
  let state = validate_cursor(snapshot_file, true)?;
  if state.path.is_empty() {
    return Err("Cannot duplicate the definition root as a sibling.".to_string());
  }
  let mut document = load_cursor_document(snapshot_file)?;
  let (namespace, definition) = parse_target(&state.target)?;
  let mut snapshot = load_snapshot(snapshot_file)?;
  check_ns_editable(&snapshot, namespace)?;
  let entry = snapshot
    .files
    .get_mut(namespace)
    .and_then(|file| file.defs.get_mut(definition))
    .ok_or_else(|| format!("Definition '{}' not found", state.target))?;
  let selected = navigate_to_path(&entry.code, &state.path)?;
  entry.code = apply_operation_at_path(
    &entry.code,
    &state.path,
    if opts.at == "before" {
      TreeOperation::InsertBefore
    } else {
      TreeOperation::InsertAfter
    },
    Some(&selected),
  )?;

  let mut duplicated_path = state.path.clone();
  if opts.at == "after" {
    *duplicated_path.last_mut().expect("root was rejected above") += 1;
  }
  push_bounded(&mut document.history, state, CURSOR_HISTORY_LIMIT);
  document.active.path = duplicated_path;
  refresh_cursor_state_from_snapshot(&mut document.active, snapshot_file, &snapshot)?;

  let staged_snapshot = stage_snapshot(snapshot_file, &snapshot)?;
  let staged_cursor = stage_cursor_document(snapshot_file, &document)?;
  commit_snapshot_then_cursor_staged_files(staged_snapshot, staged_cursor, "Duplicate", false)?;
  println!(
    "{} Duplicated selection {} cursor; cursor now at {}",
    "✓".green(),
    opts.at,
    format_path(&document.active.path)
  );
  emit_cursor_after(snapshot_file, &document.active, "cursor follows duplicated expression")?;
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
  let document = load_cursor_document(snapshot_file)?;
  let (definition, node, _) = read_cursor_context(snapshot_file, &state.target, &state.path)?;
  let preview = build_cursor_preview_from_tree(&definition, &node, &state.path, view)?;

  if format == "json" {
    println!(
      "{}",
      serde_json::json!({
        "schema_version": 3,
        "command": "cursor.show",
        "status": status,
        "view": view,
        "target": state.target,
        "path": format_path(&state.path),
        "definition_revision": state.definition_revision,
        "fingerprint": state.fingerprint,
        "state_file": cursor_file_path(snapshot_file),
        "anchor": document.anchor.as_ref().map(|anchor| serde_json::json!({
          "target": anchor.target,
          "path": format_path(&anchor.path),
          "fingerprint": anchor.fingerprint,
        })),
        "marks": document.marks.iter().map(|(name, mark)| serde_json::json!({
          "name": name,
          "target": mark.target,
          "path": format_path(&mark.path),
          "fingerprint": mark.fingerprint,
        })).collect::<Vec<_>>(),
        "last_query": document.last_query.as_ref().map(|query| serde_json::json!({
          "command": query.command,
          "pattern": query.pattern,
          "filter": query.filter,
          "selected_index": query.selected_index,
          "snapshot_revision": query.snapshot_revision,
        })),
        "tree": cirru_to_json_value(&node),
        "preview_tree": cirru_to_json_value(&preview),
      })
    );
  } else {
    println!("{}", render_cursor_human(&state, &document, status, &preview)?);
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
  emit_focus_after_cursor_action(snapshot_file, &state, "cursor moved")
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

fn depth_first_step(root: &Cirru, path: &[usize], forward: bool) -> Result<Option<Vec<usize>>, String> {
  if forward {
    if let Cirru::List(children) = navigate_to_path(root, path)?
      && !children.is_empty()
    {
      let mut child = path.to_vec();
      child.push(0);
      return Ok(Some(child));
    }

    let mut candidate = path.to_vec();
    while let Some(index) = candidate.pop() {
      let Cirru::List(siblings) = navigate_to_path(root, &candidate)? else {
        return Err(format!("Cursor parent {} is not a list.", format_path(&candidate)));
      };
      if index + 1 < siblings.len() {
        candidate.push(index + 1);
        return Ok(Some(candidate));
      }
    }
    Ok(None)
  } else {
    let mut candidate = path.to_vec();
    let Some(index) = candidate.pop() else {
      return Ok(None);
    };
    if index == 0 {
      return Ok(Some(candidate));
    }
    candidate.push(index - 1);
    loop {
      match navigate_to_path(root, &candidate)? {
        Cirru::List(children) if !children.is_empty() => candidate.push(children.len() - 1),
        _ => return Ok(Some(candidate)),
      }
    }
  }
}

fn move_cursor_depth_first(snapshot_file: &str, count: usize, forward: bool) -> Result<(), String> {
  if count == 0 {
    return Err("`--count` must be greater than zero.".to_string());
  }
  let mut state = validate_cursor(snapshot_file, true)?;
  let old_state = state.clone();
  let (definition, _) = read_cursor_definition(snapshot_file, &state.target)?;
  for completed in 0..count {
    let Some(next) = depth_first_step(&definition, &state.path, forward)? else {
      return Err(format!(
        "Cannot move {} {count} structural node(s) from {}; reached the definition {} after {completed} step(s).",
        if forward { "forward" } else { "backward" },
        format_path(&old_state.path),
        if forward { "end" } else { "start" }
      ));
    };
    state.path = next;
  }
  commit_cursor_move(snapshot_file, old_state, state)
}

fn build_cursor_preview(snapshot_file: &str, state: &CursorState, view: &str) -> Result<Cirru, String> {
  let (definition, node, _) = read_cursor_context(snapshot_file, &state.target, &state.path)?;
  build_cursor_preview_from_tree(&definition, &node, &state.path, view)
}

fn build_cursor_preview_from_tree(definition: &Cirru, node: &Cirru, path: &[usize], view: &str) -> Result<Cirru, String> {
  if view == "node" {
    return Ok(Cirru::List(vec![Cirru::leaf("CURSOR"), node.clone()]));
  }

  if view == "focus" {
    let options = cirru_parser::CirruFocusOptions::default()
      .with_focus_marker("CURSOR")
      .with_root_prefix(3);
    Ok(cirru_parser::focus_cirru_preview_with_options(definition, path, &options))
  } else {
    let marker = Cirru::List(vec![Cirru::leaf("CURSOR"), node.clone()]);
    apply_operation_at_path(definition, path, TreeOperation::Replace, Some(&marker))
  }
}

fn render_cursor_human(state: &CursorState, document: &CursorDocument, status: &str, preview: &Cirru) -> Result<String, String> {
  let rendered = cirru_parser::format(
    std::slice::from_ref(preview),
    cirru_parser::CirruWriterOptions { use_inline: false },
  )
  .map_err(|error| format!("Failed to render cursor preview: {error}"))?;
  Ok(format!(
    "{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}/{}\n{}: {}\n\n{}",
    "Target".green().bold(),
    state.target,
    "Path".green().bold(),
    format_path(&state.path),
    "Revision".green().bold(),
    state.definition_revision,
    "Status".green().bold(),
    status,
    "Anchor".green().bold(),
    document
      .anchor
      .as_ref()
      .map(|anchor| format!("{} {}", anchor.target, format_path(&anchor.path)))
      .unwrap_or_else(|| "(none)".to_string()),
    "Marks".green().bold(),
    document.marks.len(),
    CURSOR_MARK_LIMIT,
    "Last query".green().bold(),
    document
      .last_query
      .as_ref()
      .map(|query| format!("{} #{}", query.command, query.selected_index))
      .unwrap_or_else(|| "(none)".to_string()),
    rendered
  ))
}

fn push_bounded<T>(items: &mut Vec<T>, value: T, limit: usize) {
  while items.len() >= limit && !items.is_empty() {
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
  emit_focus_after_cursor_action(snapshot_file, &restored, "cursor restored")
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

fn set_cursor_anchor(snapshot_file: &str) -> Result<(), String> {
  let state = validate_cursor(snapshot_file, true)?;
  if state.path.is_empty() {
    return Err("A region anchor must select a child node, not the definition root.".to_string());
  }
  let mut document = load_cursor_document(snapshot_file)?;
  document.anchor = Some(state.clone());
  save_cursor_document(snapshot_file, &document)?;
  println!("{} Region anchor set: {} {}", "✓".green(), state.target, format_path(&state.path));
  Ok(())
}

fn clear_cursor_anchor(snapshot_file: &str) -> Result<(), String> {
  let mut document = load_cursor_document(snapshot_file)?;
  if document.anchor.take().is_some() {
    save_cursor_document(snapshot_file, &document)?;
    println!("{} Region anchor cleared", "✓".green());
  } else {
    println!("{} No region anchor is set", "•".dimmed());
  }
  Ok(())
}

fn cursor_region_bounds(anchor: &CursorState, active: &CursorState) -> Result<(Vec<usize>, usize, usize), String> {
  if anchor.target != active.target {
    return Err(format!(
      "Region endpoints target different definitions: '{}' and '{}'.",
      anchor.target, active.target
    ));
  }
  let (Some(&anchor_index), Some(&active_index)) = (anchor.path.last(), active.path.last()) else {
    return Err("A region cannot use the definition root as an endpoint.".to_string());
  };
  let anchor_parent = &anchor.path[..anchor.path.len() - 1];
  let active_parent = &active.path[..active.path.len() - 1];
  if anchor_parent != active_parent {
    return Err(format!(
      "Region endpoints must be contiguous siblings under one parent; anchor is {} and cursor is {}.",
      format_path(&anchor.path),
      format_path(&active.path)
    ));
  }
  Ok((
    anchor_parent.to_vec(),
    anchor_index.min(active_index),
    anchor_index.max(active_index),
  ))
}

fn show_cursor_region(snapshot_file: &str, format: &str) -> Result<(), String> {
  if !matches!(format, "human" | "json") {
    return Err(format!("Unsupported region format '{format}'. Expected human or json."));
  }
  let active = validate_cursor(snapshot_file, true)?;
  let mut document = load_cursor_document(snapshot_file)?;
  let saved_anchor = document
    .anchor
    .clone()
    .ok_or("No region anchor is set. Run `calcit cursor anchor` first.")?;
  let anchor = relocate_saved_state(snapshot_file, saved_anchor)?;
  let (parent_path, start, end) = cursor_region_bounds(&anchor, &active)?;
  let (parent, _) = read_cursor_target(snapshot_file, &active.target, &parent_path)?;
  let Cirru::List(children) = parent else {
    return Err("Region parent is no longer a list.".to_string());
  };
  if end >= children.len() {
    return Err(format!(
      "Region endpoint {end} is outside a parent with {} child(ren).",
      children.len()
    ));
  }
  let selected = children[start..=end].to_vec();
  if document.anchor.as_ref() != Some(&anchor) {
    document.anchor = Some(anchor.clone());
    save_cursor_document(snapshot_file, &document)?;
  }

  if format == "json" {
    println!(
      "{}",
      serde_json::json!({
        "schema_version": 1,
        "command": "cursor.region",
        "target": active.target,
        "parent_path": format_path(&parent_path),
        "start": start,
        "end": end,
        "count": selected.len(),
        "anchor_path": format_path(&anchor.path),
        "cursor_path": format_path(&active.path),
        "trees": selected.iter().map(cirru_to_json_value).collect::<Vec<_>>(),
      })
    );
  } else {
    let preview = Cirru::List(std::iter::once(Cirru::leaf("REGION")).chain(selected).collect());
    let rendered = cirru_parser::format(&[preview], cirru_parser::CirruWriterOptions { use_inline: false })
      .map_err(|error| format!("Failed to render cursor region: {error}"))?;
    println!(
      "{}: {}\n{}: {}..{} ({} siblings)\n{}: {}\n{}: {}\n\n{}",
      "Target".green().bold(),
      active.target,
      "Region".green().bold(),
      start,
      end,
      end - start + 1,
      "Anchor".green().bold(),
      format_path(&anchor.path),
      "Cursor".green().bold(),
      format_path(&active.path),
      rendered
    );
  }
  Ok(())
}

fn validate_mark_name(name: &str) -> Result<(), String> {
  if name.is_empty()
    || name.len() > 32
    || !name
      .chars()
      .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
  {
    return Err("Mark name must contain 1..32 ASCII letters, digits, underscores, or hyphens.".to_string());
  }
  Ok(())
}

fn save_cursor_mark(snapshot_file: &str, name: &str) -> Result<(), String> {
  validate_mark_name(name)?;
  let state = validate_cursor(snapshot_file, true)?;
  let mut document = load_cursor_document(snapshot_file)?;
  if !document.marks.contains_key(name) && document.marks.len() == CURSOR_MARK_LIMIT {
    return Err(format!(
      "Cursor mark limit ({CURSOR_MARK_LIMIT}) reached. Remove an unused mark with `calcit cursor rm-mark <name>`."
    ));
  }
  document.marks.insert(name.to_string(), state.clone());
  save_cursor_document(snapshot_file, &document)?;
  println!(
    "{} Mark '{}' saved at {} {}",
    "✓".green(),
    name,
    state.target,
    format_path(&state.path)
  );
  Ok(())
}

fn goto_cursor_mark(snapshot_file: &str, name: &str) -> Result<(), String> {
  validate_mark_name(name)?;
  let mut document = load_cursor_document(snapshot_file)?;
  let saved = document
    .marks
    .get(name)
    .cloned()
    .ok_or_else(|| format!("Cursor mark '{name}' does not exist."))?;
  let restored = relocate_saved_state(snapshot_file, saved)?;
  if document.active.target != restored.target || document.active.path != restored.path {
    push_bounded(&mut document.history, document.active.clone(), CURSOR_HISTORY_LIMIT);
  }
  document.active = restored.clone();
  document.marks.insert(name.to_string(), restored.clone());
  save_cursor_document(snapshot_file, &document)?;
  println!(
    "{} Cursor moved to mark '{}': {} {}",
    "✓".green(),
    name,
    restored.target,
    format_path(&restored.path)
  );
  emit_focus_after_cursor_action(snapshot_file, &restored, "cursor restored from named mark")
}

fn show_cursor_marks(snapshot_file: &str, format: &str) -> Result<(), String> {
  if !matches!(format, "human" | "json") {
    return Err(format!("Unsupported marks format '{format}'. Expected human or json."));
  }
  let mut document = load_cursor_document(snapshot_file)?;
  let snapshot = load_snapshot(snapshot_file)?;
  let mut changed = false;
  let mut rows = vec![];
  for (name, saved) in document.marks.clone() {
    match relocate_saved_state_from_snapshot(snapshot_file, saved, &snapshot) {
      Ok(state) => {
        if document.marks.get(&name) != Some(&state) {
          document.marks.insert(name.clone(), state.clone());
          changed = true;
        }
        rows.push((name, Some(state), None));
      }
      Err(error) => rows.push((name, None, Some(error))),
    }
  }
  if changed {
    save_cursor_document(snapshot_file, &document)?;
  }

  if format == "json" {
    println!(
      "{}",
      serde_json::json!({
        "schema_version": 1,
        "command": "cursor.marks",
        "limit": CURSOR_MARK_LIMIT,
        "marks": rows.iter().map(|(name, state, error)| serde_json::json!({
          "name": name,
          "status": if state.is_some() { "ready" } else { "stale" },
          "target": state.as_ref().map(|value| value.target.as_str()),
          "path": state.as_ref().map(|value| format_path(&value.path)),
          "error": error,
        })).collect::<Vec<_>>(),
      })
    );
  } else if rows.is_empty() {
    println!("{} No cursor marks are saved", "•".dimmed());
  } else {
    println!("{} ({}/{CURSOR_MARK_LIMIT})", "Cursor marks:".bold(), rows.len());
    for (name, state, error) in rows {
      if let Some(state) = state {
        println!("  {}  {} {}", name.cyan(), state.target, format_path(&state.path));
      } else {
        println!("  {}  {}", name.cyan(), format!("stale: {}", error.unwrap_or_default()).yellow());
      }
    }
  }
  Ok(())
}

fn remove_cursor_mark(snapshot_file: &str, name: &str) -> Result<(), String> {
  validate_mark_name(name)?;
  let mut document = load_cursor_document(snapshot_file)?;
  if document.marks.remove(name).is_none() {
    return Err(format!("Cursor mark '{name}' does not exist."));
  }
  save_cursor_document(snapshot_file, &document)?;
  println!("{} Mark '{}' removed", "✓".green(), name);
  Ok(())
}

fn store_cursor_clipboard(snapshot_file: &str, mode: &str, cut: bool) -> Result<(), String> {
  let state = validate_cursor(snapshot_file, true)?;
  if cut && state.path.is_empty() {
    return Err("Cannot cut the definition root. Use `calcit edit mv-def` or replace the definition explicitly.".to_string());
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
    entry.code = apply_operation_at_path(&entry.code, &state.path, TreeOperation::Delete, None)?;
    document.active.path = transform_delete(&state.path, &state.path).0;
    refresh_cursor_state_from_snapshot(&mut document.active, snapshot_file, &snapshot)?;
    transform_auxiliary_positions(&mut document, &state.target, snapshot_file, &snapshot, |path| {
      transform_delete(path, &state.path).0
    })?;

    let staged_cursor = stage_cursor_document(snapshot_file, &document)?;
    let staged_snapshot = stage_snapshot(snapshot_file, &snapshot)?;
    commit_cut_staged_files(staged_cursor, staged_snapshot)?;
  } else {
    save_cursor_document(snapshot_file, &document)?;
  }
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
    .ok_or("Cursor clipboard is empty. Use `calcit cursor copy` or `calcit cursor cut` first.")?;
  let operation = TreeOperation::from_insert_position(at)
    .ok_or_else(|| format!("Unsupported paste position '{at}'. Use before, after, prepend-child, append-child, or replace."))?;

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

  let pasted_path = operation.inserted_path(&state.path, append_index)?;
  push_bounded(&mut document.history, state.clone(), CURSOR_HISTORY_LIMIT);
  document.active.path = pasted_path;
  refresh_cursor_state_from_snapshot(&mut document.active, snapshot_file, &snapshot)?;
  let mutation = operation.cursor_mutation(state.path.clone());
  transform_auxiliary_positions(&mut document, &state.target, snapshot_file, &snapshot, |path| {
    transform_cursor_path(path, &mutation).0
  })?;

  let staged_snapshot = stage_snapshot(snapshot_file, &snapshot)?;
  let staged_cursor = stage_cursor_document(snapshot_file, &document)?;
  commit_paste_staged_files(staged_snapshot, staged_cursor)?;
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
  let (cursor_target, cursor_path) = resolve_active_cursor_reference(snapshot_file)?;
  if cursor_target != target {
    return Err(format!(
      "Cursor target mismatch: cursor points to '{}', but command targets '{}'.",
      cursor_target, target
    ));
  }
  Ok(cursor_path)
}

pub(crate) fn resolve_cursor_target_argument(snapshot_file: &str, target: &str) -> Result<String, String> {
  if target != "@cursor" {
    return Ok(target.to_string());
  }
  Ok(resolve_active_cursor_reference(snapshot_file)?.0)
}

pub(crate) fn resolve_active_cursor_reference(snapshot_file: &str) -> Result<(String, String), String> {
  let state = validate_cursor(snapshot_file, true)?;
  let path = if state.path.is_empty() {
    String::new()
  } else {
    format_path(&state.path)
  };
  Ok((state.target, path))
}

pub(crate) fn maintain_cursor_after_tree_mutation(
  snapshot_file: &str,
  target: &str,
  mutation: &TreeCursorMutation,
) -> Result<(), String> {
  if std::env::var(CURSOR_MAINTENANCE_ENV).is_ok_and(|value| value == "disabled") {
    return Ok(());
  }
  if !cursor_state_exists(snapshot_file) {
    return Ok(());
  }

  let mut document = load_cursor_document(snapshot_file)?;
  if document.active.target != target {
    let has_auxiliary_target = document.anchor.as_ref().is_some_and(|state| state.target == target)
      || document.marks.values().any(|state| state.target == target);
    if !has_auxiliary_target {
      return Ok(());
    }
  }

  let snapshot = load_snapshot(snapshot_file)?;
  let old_path = document.active.path.clone();
  let mut note = "auxiliary cursor state updated";
  if document.active.target == target {
    let (new_path, active_note) = transform_cursor_path(&document.active.path, mutation);
    document.active.path = new_path;
    note = active_note;
    refresh_cursor_state_from_snapshot(&mut document.active, snapshot_file, &snapshot).map_err(|error| {
      format!("Snapshot mutation succeeded, but cursor maintenance failed: {error}. Run `calcit cursor set` to select it again.")
    })?;
  }
  transform_auxiliary_positions(&mut document, target, snapshot_file, &snapshot, |path| {
    transform_cursor_path(path, mutation).0
  })?;
  save_cursor_document(snapshot_file, &document).map_err(|error| {
    format!("Snapshot mutation succeeded, but cursor state could not be saved: {error}. Run `calcit cursor show` before editing again.")
  })?;

  let detail = if document.active.target != target {
    note.to_string()
  } else if old_path != document.active.path {
    format!("{} → {} ({note})", format_path(&old_path), format_path(&document.active.path))
  } else {
    format!("{} ({note})", format_path(&document.active.path))
  };
  emit_cursor_after(snapshot_file, &document.active, &detail)?;
  Ok(())
}

pub(crate) fn maintain_cursor_after_definition_move(snapshot_file: &str, source: &str, target: &str) -> Result<(), String> {
  if std::env::var(CURSOR_MAINTENANCE_ENV).is_ok_and(|value| value == "disabled") || !cursor_state_exists(snapshot_file) {
    return Ok(());
  }
  let mut document = load_cursor_document(snapshot_file)?;
  let has_auxiliary_target =
    document.anchor.as_ref().is_some_and(|state| state.target == source) || document.marks.values().any(|state| state.target == source);
  if document.active.target != source && !has_auxiliary_target {
    return Ok(());
  }
  let snapshot = load_snapshot(snapshot_file)?;
  if document.active.target == source {
    document.active.target = target.to_string();
    refresh_cursor_state_from_snapshot(&mut document.active, snapshot_file, &snapshot).map_err(|error| {
      format!("Definition move succeeded, but cursor maintenance failed: {error}. Run `calcit cursor set` to select it again.")
    })?;
  }
  retarget_auxiliary_positions(&mut document, source, target, snapshot_file, &snapshot)?;
  save_cursor_document(snapshot_file, &document)?;
  emit_cursor_after(snapshot_file, &document.active, &format!("target updated: {source} → {target}"))
}

fn transform_auxiliary_positions<F>(
  document: &mut CursorDocument,
  target: &str,
  snapshot_file: &str,
  snapshot: &snapshot::Snapshot,
  transform: F,
) -> Result<(), String>
where
  F: Fn(&[usize]) -> Vec<usize>,
{
  if let Some(anchor) = document.anchor.as_mut()
    && anchor.target == target
  {
    anchor.path = transform(&anchor.path);
    refresh_cursor_state_from_snapshot(anchor, snapshot_file, snapshot)?;
  }
  for mark in document.marks.values_mut() {
    if mark.target == target {
      mark.path = transform(&mark.path);
      refresh_cursor_state_from_snapshot(mark, snapshot_file, snapshot)?;
    }
  }
  Ok(())
}

fn retarget_auxiliary_positions(
  document: &mut CursorDocument,
  source: &str,
  target: &str,
  snapshot_file: &str,
  snapshot: &snapshot::Snapshot,
) -> Result<(), String> {
  if let Some(anchor) = document.anchor.as_mut()
    && anchor.target == source
  {
    anchor.target = target.to_string();
    refresh_cursor_state_from_snapshot(anchor, snapshot_file, snapshot)?;
  }
  for mark in document.marks.values_mut() {
    if mark.target == source {
      mark.target = target.to_string();
      refresh_cursor_state_from_snapshot(mark, snapshot_file, snapshot)?;
    }
  }
  Ok(())
}

pub(crate) fn maintain_cursor_after_definition_replace(snapshot_file: &str, target: &str) -> Result<(), String> {
  maintain_cursor_after_tree_mutation(snapshot_file, target, &TreeCursorMutation::Replace { path: vec![] })
}

pub(crate) fn maintain_cursor_after_definition_delete(snapshot_file: &str, target: &str) -> Result<(), String> {
  if std::env::var(CURSOR_MAINTENANCE_ENV).is_ok_and(|value| value == "disabled") || !cursor_state_exists(snapshot_file) {
    return Ok(());
  }
  let document = load_cursor_document(snapshot_file)?;
  if document.active.target == target && CURSOR_AFTER_MODE.load(Ordering::Relaxed) != 0 {
    eprintln!(
      "{} Selected definition '{target}' was deleted; the cursor is stale. Use `calcit cursor back` or `calcit cursor set`.",
      "[Cursor]".yellow().bold()
    );
  }
  Ok(())
}

pub(crate) fn maintain_cursor_after_namespace_delete(snapshot_file: &str, namespace: &str) -> Result<(), String> {
  if std::env::var(CURSOR_MAINTENANCE_ENV).is_ok_and(|value| value == "disabled") || !cursor_state_exists(snapshot_file) {
    return Ok(());
  }
  let document = load_cursor_document(snapshot_file)?;
  let cursor_namespace = parse_target(&document.active.target)?.0;
  if cursor_namespace == namespace && CURSOR_AFTER_MODE.load(Ordering::Relaxed) != 0 {
    eprintln!(
      "{} Selected namespace '{namespace}' was deleted; the cursor is stale. Use `calcit cursor back` or `calcit cursor set`.",
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
  if std::env::var(CURSOR_MAINTENANCE_ENV).is_ok_and(|value| value == "disabled") || !cursor_state_exists(snapshot_file) {
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
  operation: TreeOperation,
  append_index: usize,
) -> Result<(), String> {
  if std::env::var(CURSOR_MAINTENANCE_ENV).is_ok_and(|value| value == "disabled") || !cursor_state_exists(snapshot_file) {
    return Ok(());
  }
  let mut document = load_cursor_document(snapshot_file)?;
  if document.active.target != target {
    return Ok(());
  }

  let inserted_path = operation.inserted_path(destination_path, append_index)?;
  let insertion = operation.cursor_mutation(destination_path.to_vec());
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

pub(crate) fn maintain_cursor_after_any_mutation(snapshot_file: &str, detail: &str) -> Result<(), String> {
  if std::env::var(CURSOR_MAINTENANCE_ENV).is_ok_and(|value| value == "disabled") || !cursor_state_exists(snapshot_file) {
    return Ok(());
  }
  match validate_cursor(snapshot_file, true) {
    Ok(state) => emit_cursor_after(snapshot_file, &state, detail),
    Err(error) => {
      if CURSOR_AFTER_MODE.load(Ordering::Relaxed) != 0 {
        eprintln!(
          "{} Transaction committed, but the cursor needs attention: {error} Use `calcit cursor back` or `calcit cursor set`.",
          "[Cursor]".yellow().bold()
        );
      }
      Ok(())
    }
  }
}

fn emit_focus_after_cursor_action(snapshot_file: &str, state: &CursorState, detail: &str) -> Result<(), String> {
  if CURSOR_AFTER_MODE.load(Ordering::Relaxed) == 2 {
    emit_cursor_after(snapshot_file, state, detail)
  } else {
    Ok(())
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
  project_state::state_file_for_snapshot(snapshot_file, CURSOR_STATE_FILE)
}

fn legacy_cursor_file_path(snapshot_file: &str) -> PathBuf {
  project_state::project_directory_for_snapshot(snapshot_file).join(LEGACY_CURSOR_FILE)
}

fn migrate_legacy_cursor(snapshot_file: &str) -> Result<bool, String> {
  let legacy = legacy_cursor_file_path(snapshot_file);
  let destination = cursor_file_path(snapshot_file);
  project_state::migrate_legacy_file(&legacy, &destination).map_err(|error| {
    format!(
      "Failed to migrate legacy cursor file '{}' to '{}': {error}",
      legacy.display(),
      destination.display()
    )
  })
}

fn cursor_state_exists(snapshot_file: &str) -> bool {
  cursor_file_path(snapshot_file).exists() || legacy_cursor_file_path(snapshot_file).exists()
}

fn node_fingerprint(node: &Cirru) -> String {
  let encoded = cirru_to_json_value(node).to_string();
  let mut hasher = Md5::new();
  hasher.update(encoded.as_bytes());
  format!("md5:{}", hex::encode(hasher.finalize()))
}

fn read_cursor_context(snapshot_file: &str, target: &str, path: &[usize]) -> Result<(Cirru, Cirru, String), String> {
  let (namespace, definition_name) = parse_target(target)?;
  let snapshot = load_snapshot(snapshot_file)?;
  let file = snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Cursor namespace '{namespace}' no longer exists."))?;
  let entry = file
    .defs
    .get(definition_name)
    .ok_or_else(|| format!("Cursor definition '{target}' no longer exists."))?;
  let node = navigate_to_path(&entry.code, path)
    .map_err(|error| format!("Cursor path {} is no longer valid in '{}': {error}", format_path(path), target))?
    .clone();
  Ok((entry.code.clone(), node, snapshot::definition_revision(entry)?))
}

fn read_cursor_target(snapshot_file: &str, target: &str, path: &[usize]) -> Result<(Cirru, String), String> {
  let (_, node, revision) = read_cursor_context(snapshot_file, target, path)?;
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
  let snapshot = load_snapshot(snapshot_file)?;
  refresh_cursor_state_from_snapshot(state, snapshot_file, &snapshot)
}

fn refresh_cursor_state_from_snapshot(state: &mut CursorState, snapshot_file: &str, value: &snapshot::Snapshot) -> Result<(), String> {
  let (namespace, definition_name) = parse_target(&state.target)?;
  let file = value
    .files
    .get(namespace)
    .ok_or_else(|| format!("Cursor namespace '{namespace}' no longer exists."))?;
  let entry = file
    .defs
    .get(definition_name)
    .ok_or_else(|| format!("Cursor definition '{}' no longer exists.", state.target))?;
  let node = navigate_to_path(&entry.code, &state.path).map_err(|error| {
    format!(
      "Cursor path {} is no longer valid in '{}': {error}",
      format_path(&state.path),
      state.target
    )
  })?;
  state.snapshot = snapshot_file.to_string();
  state.definition_revision = snapshot::definition_revision(entry)?;
  state.fingerprint = node_fingerprint(&node);
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
      "Cursor is stale: saved node at {} in '{}' has {} fingerprint match(es). Refusing to guess; run `calcit cursor set <target> --path <path>`.",
      format_path(&state.path),
      state.target,
      matches.len()
    ))
  }
}

fn relocate_saved_state(snapshot_file: &str, state: CursorState) -> Result<CursorState, String> {
  let snapshot = load_snapshot(snapshot_file)?;
  relocate_saved_state_from_snapshot(snapshot_file, state, &snapshot)
}

fn relocate_saved_state_from_snapshot(
  snapshot_file: &str,
  mut state: CursorState,
  snapshot: &snapshot::Snapshot,
) -> Result<CursorState, String> {
  let (namespace, definition_name) = parse_target(&state.target)?;
  let definition = &snapshot
    .files
    .get(namespace)
    .ok_or_else(|| format!("Saved cursor namespace '{namespace}' no longer exists."))?
    .defs
    .get(definition_name)
    .ok_or_else(|| format!("Saved cursor definition '{}' no longer exists.", state.target))?
    .code;
  if let Ok(node) = navigate_to_path(definition, &state.path)
    && node_fingerprint(&node) == state.fingerprint
  {
    refresh_cursor_state_from_snapshot(&mut state, snapshot_file, snapshot)?;
    return Ok(state);
  }

  let mut matches = vec![];
  collect_fingerprint_paths(definition, &state.fingerprint, &mut vec![], &mut matches);
  if matches.len() != 1 {
    return Err(format!(
      "Saved cursor location {} in '{}' has {} fingerprint match(es); refusing to restore ambiguously.",
      format_path(&state.path),
      state.target,
      matches.len()
    ));
  }
  state.path = matches.remove(0);
  refresh_cursor_state_from_snapshot(&mut state, snapshot_file, snapshot)?;
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
  if migrate_legacy_cursor(snapshot_file)? {
    eprintln!(
      "{} Moved legacy cursor state into '{}'.",
      "[Cursor]".cyan().bold(),
      cursor_file_path(snapshot_file).display()
    );
  }
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
  if ![1.0, 2.0, 3.0, f64::from(CURSOR_SCHEMA_VERSION)]
    .iter()
    .any(|supported| (version - supported).abs() <= f64::EPSILON)
  {
    return Err(format!("Unsupported cursor schema version {version}."));
  }
  let active_name = root
    .tag_get("active")
    .ok_or("Cursor file is missing :active")?
    .read_tag_str()?
    .to_string();
  let cursors = root.tag_get("cursors").ok_or("Cursor file is missing :cursors")?.view_map()?;
  let mut cursor_states = cursors
    .0
    .iter()
    .map(|(name, value)| Ok((name.read_tag_str()?.to_string(), cursor_state_from_edn(value)?)))
    .collect::<Result<BTreeMap<_, _>, String>>()?;
  let active = cursor_states
    .remove(&active_name)
    .ok_or_else(|| format!("Cursor file has no entry for active cursor :{active_name}"))?;
  let history = optional_state_list(&root, "history")?;
  let stack = optional_state_list(&root, "stack")?;
  let anchor = match root.tag_get("anchor") {
    None | Some(Edn::Nil) => None,
    Some(value) => Some(cursor_state_from_edn(value)?),
  };
  let marks = optional_marks(&root)?;
  let last_query = match root.tag_get("last-query") {
    None | Some(Edn::Nil) => None,
    Some(value) => Some(cursor_last_query_from_edn(value)?),
  };
  let clipboard = match root.tag_get("clipboard") {
    None | Some(Edn::Nil) => None,
    Some(value) => Some(cursor_clipboard_from_edn(value)?),
  };

  Ok(Some(CursorDocument {
    active_name,
    active,
    inactive_cursors: cursor_states,
    history,
    stack,
    anchor,
    marks,
    last_query,
    clipboard,
  }))
}

fn load_cursor_document(snapshot_file: &str) -> Result<CursorDocument, String> {
  load_cursor_document_optional(snapshot_file)?.ok_or_else(|| {
    format!(
      "No cursor is set. Use `calcit cursor set <target> --path <path>` first (expected '{}').",
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

fn cursor_last_query_from_edn(value: &Edn) -> Result<CursorLastQuery, String> {
  let entry = value.view_map()?;
  Ok(CursorLastQuery {
    command: required_string(&entry, "command")?,
    pattern: required_string(&entry, "pattern")?,
    filter: optional_string(&entry, "filter")?,
    exact: required_bool(&entry, "exact")?,
    regex: required_bool(&entry, "regex")?,
    max_depth: required_usize(&entry, "max-depth")?,
    start_path: optional_string(&entry, "start-path")?,
    entry: optional_string(&entry, "entry")?,
    pattern_is_json: required_bool(&entry, "pattern-is-json")?,
    selected_index: required_usize(&entry, "selected-index")?,
    snapshot_revision: required_string(&entry, "snapshot-revision")?,
  })
}

fn optional_marks(map: &cirru_edn::EdnMapView) -> Result<BTreeMap<String, CursorState>, String> {
  let Some(value) = map.tag_get("marks") else {
    return Ok(BTreeMap::new());
  };
  let values = value.view_map()?;
  if values.0.len() > CURSOR_MARK_LIMIT {
    return Err(format!(
      "Cursor file contains {} marks, exceeding the supported limit of {CURSOR_MARK_LIMIT}.",
      values.0.len()
    ));
  }
  values
    .0
    .iter()
    .map(|(key, value)| {
      let name = key.read_tag_str()?.to_string();
      validate_mark_name(&name)?;
      Ok((name, cursor_state_from_edn(value)?))
    })
    .collect()
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

fn optional_string(map: &cirru_edn::EdnMapView, key: &str) -> Result<Option<String>, String> {
  match map.tag_get(key) {
    None | Some(Edn::Nil) => Ok(None),
    Some(value) => value.read_string().map(Some),
  }
}

fn required_bool(map: &cirru_edn::EdnMapView, key: &str) -> Result<bool, String> {
  match map.tag_get(key) {
    Some(Edn::Bool(value)) => Ok(*value),
    Some(value) => Err(format!("Cursor entry :{key} must be a boolean, got {value:?}.")),
    None => Err(format!("Cursor entry is missing :{key}")),
  }
}

fn required_usize(map: &cirru_edn::EdnMapView, key: &str) -> Result<usize, String> {
  let number = map
    .tag_get(key)
    .ok_or_else(|| format!("Cursor entry is missing :{key}"))?
    .read_number()?;
  if number < 0.0 || number.fract().abs() > f64::EPSILON {
    Err(format!("Cursor entry :{key} must be a non-negative integer, got {number}."))
  } else {
    Ok(number as usize)
  }
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

fn cursor_last_query_to_edn(query: &CursorLastQuery) -> Edn {
  Edn::map_from_iter([
    (Edn::tag("command"), Edn::str(query.command.as_str())),
    (Edn::tag("pattern"), Edn::str(query.pattern.as_str())),
    (
      Edn::tag("filter"),
      query.filter.as_ref().map(|value| Edn::str(value.as_str())).unwrap_or(Edn::Nil),
    ),
    (Edn::tag("exact"), Edn::Bool(query.exact)),
    (Edn::tag("regex"), Edn::Bool(query.regex)),
    (Edn::tag("max-depth"), Edn::from(query.max_depth)),
    (
      Edn::tag("start-path"),
      query.start_path.as_ref().map(|value| Edn::str(value.as_str())).unwrap_or(Edn::Nil),
    ),
    (
      Edn::tag("entry"),
      query.entry.as_ref().map(|value| Edn::str(value.as_str())).unwrap_or(Edn::Nil),
    ),
    (Edn::tag("pattern-is-json"), Edn::Bool(query.pattern_is_json)),
    (Edn::tag("selected-index"), Edn::from(query.selected_index)),
    (Edn::tag("snapshot-revision"), Edn::str(query.snapshot_revision.as_str())),
  ])
}

#[cfg(test)]
fn save_cursor_state(snapshot_file: &str, state: &CursorState) -> Result<(), String> {
  let mut document = load_cursor_document_optional(snapshot_file)?.unwrap_or_else(|| CursorDocument {
    active_name: DEFAULT_CURSOR.to_string(),
    active: state.clone(),
    inactive_cursors: BTreeMap::new(),
    history: vec![],
    stack: vec![],
    anchor: None,
    marks: BTreeMap::new(),
    last_query: None,
    clipboard: None,
  });
  document.active = state.clone();
  save_cursor_document(snapshot_file, &document)
}

fn render_cursor_document(document: &CursorDocument) -> Result<String, String> {
  let mut cursors = document.inactive_cursors.clone();
  cursors.insert(document.active_name.clone(), document.active.clone());
  let mut content = cirru_edn::format(
    &Edn::map_from_iter([
      (Edn::tag("schema-version"), Edn::from(CURSOR_SCHEMA_VERSION)),
      (Edn::tag("active"), Edn::tag(document.active_name.as_str())),
      (
        Edn::tag("cursors"),
        Edn::map_from_iter(
          cursors
            .iter()
            .map(|(name, state)| (Edn::tag(name.as_str()), cursor_state_to_edn(state))),
        ),
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
        Edn::tag("anchor"),
        document.anchor.as_ref().map(cursor_state_to_edn).unwrap_or(Edn::Nil),
      ),
      (
        Edn::tag("marks"),
        Edn::map_from_iter(
          document
            .marks
            .iter()
            .map(|(name, state)| (Edn::tag(name.as_str()), cursor_state_to_edn(state))),
        ),
      ),
      (
        Edn::tag("last-query"),
        document.last_query.as_ref().map(cursor_last_query_to_edn).unwrap_or(Edn::Nil),
      ),
      (
        Edn::tag("clipboard"),
        document.clipboard.as_ref().map(cursor_clipboard_to_edn).unwrap_or(Edn::Nil),
      ),
    ]),
    false,
  )
  .map_err(|error| format!("Failed to format cursor Cirru EDN: {error}"))?;
  content.push('\n');
  if content.len() > CURSOR_FILE_MAX_BYTES {
    return Err(format!(
      "Cursor state is {} bytes, exceeding the {} byte limit. Clear an oversized clipboard or remove unused marks before retrying.",
      content.len(),
      CURSOR_FILE_MAX_BYTES
    ));
  }
  Ok(content)
}

fn stage_cursor_document(snapshot_file: &str, document: &CursorDocument) -> Result<StagedFile, String> {
  let destination = cursor_file_path(snapshot_file);
  let content = render_cursor_document(document)?;
  stage_atomic_file(&destination, content.as_bytes(), "cursor")
}

fn stage_snapshot(snapshot_file: &str, value: &snapshot::Snapshot) -> Result<StagedFile, String> {
  let content = snapshot::render_snapshot_content(value)?;
  stage_atomic_file(Path::new(snapshot_file), content.as_bytes(), "snapshot")
}

fn commit_cut_staged_files(staged_cursor: StagedFile, staged_snapshot: StagedFile) -> Result<(), String> {
  staged_cursor.commit().map_err(|error| {
    format!("Cut was not applied because the clipboard could not be persisted safely: {error}. The source expression is unchanged.")
  })?;
  staged_snapshot.commit().map_err(|error| {
    format!(
      "Cut was not applied because the staged snapshot could not be committed: {error}. The expression remains in source and is also recoverable from `calcit cursor clipboard`."
    )
  })
}

fn commit_paste_staged_files(staged_snapshot: StagedFile, staged_cursor: StagedFile) -> Result<(), String> {
  commit_snapshot_then_cursor_staged_files(staged_snapshot, staged_cursor, "Paste", true)
}

fn commit_snapshot_then_cursor_staged_files(
  staged_snapshot: StagedFile,
  staged_cursor: StagedFile,
  action: &str,
  clipboard_available: bool,
) -> Result<(), String> {
  staged_snapshot
    .commit()
    .map_err(|error| format!("{action} was not applied: {error}"))?;
  staged_cursor.commit().map_err(|error| {
    format!(
      "{action} succeeded in the snapshot, but cursor state could not be updated: {error}. Do not retry blindly; run `calcit cursor show` or `calcit cursor set` first.{}",
      if clipboard_available {
        " The clipboard remains available."
      } else {
        ""
      }
    )
  })
}

fn save_cursor_document(snapshot_file: &str, document: &CursorDocument) -> Result<(), String> {
  stage_cursor_document(snapshot_file, document)?.commit()
}

fn warn_cursor_gitignore(snapshot_file: &str) {
  let project_dir = project_state::project_directory_for_snapshot(snapshot_file);
  let gitignore = project_dir.join(".gitignore");
  let ignored = fs::read_to_string(&gitignore).is_ok_and(|content| {
    content.lines().any(|line| {
      let pattern = line.trim();
      matches!(
        pattern,
        ".calcit" | ".calcit/" | "/.calcit" | "/.calcit/" | "**/.calcit/" | ".calcit/cursor.cirru" | "*.cirru"
      )
    })
  });
  if !ignored {
    eprintln!(
      "{} Add `.calcit/` to '{}' so local Calcit state is not committed.",
      "[Cursor]".yellow().bold(),
      gitignore.display()
    );
  }
}

#[cfg(test)]
mod tests {
  use super::{
    CursorClipboard, CursorDocument, CursorLastQuery, CursorState, DEFAULT_CURSOR, RestoreSource, TreeCursorMutation, TreeOperation,
    apply_at_cursor, barf_first, barf_last, build_cursor_preview, commit_cut_staged_files, commit_paste_staged_files, cursor_file_path,
    cursor_region_bounds, cursor_state_to_edn, duplicate_cursor, legacy_cursor_file_path, load_cursor_document, load_cursor_state,
    maintain_cursor_after_node_move, maintain_cursor_after_tree_mutation, move_cursor_across_siblings, move_cursor_depth_first,
    move_cursor_to_child, node_fingerprint, paste_cursor_clipboard, push_bounded, read_cursor_target, render_cursor_document,
    restore_cursor, save_cursor_document, save_cursor_mark, save_cursor_state, set_cursor_anchor, set_cursor_selection, slurp_next,
    slurp_prev, stage_atomic_file, store_cursor_clipboard, transform_cursor_path,
  };
  use crate::cli_handlers::edit::{apply_operation_at_path, load_snapshot, save_snapshot};
  use crate::cli_handlers::test_support::TestProject;
  use calcit::cli_args::{CursorApplyCommand, CursorDuplicateCommand};
  use cirru_edn::Edn;
  use cirru_parser::Cirru;
  use std::collections::BTreeMap;
  use std::fs;

  type TestCursorSnapshot = TestProject;

  #[test]
  fn bounded_push_trims_oversized_persisted_state() {
    let mut items = vec![1, 2, 3, 4];
    push_bounded(&mut items, 5, 3);
    assert_eq!(items, vec![3, 4, 5]);
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
  fn region_requires_contiguous_siblings_and_normalizes_direction() {
    let anchor = CursorState {
      snapshot: "calcit.cirru".to_string(),
      target: "app.main/view".to_string(),
      path: vec![3, 5],
      definition_revision: "revision".to_string(),
      fingerprint: "fingerprint".to_string(),
    };
    let mut active = anchor.clone();
    active.path = vec![3, 2];
    assert_eq!(
      cursor_region_bounds(&anchor, &active).expect("sibling region should resolve"),
      (vec![3], 2, 5)
    );

    active.path = vec![4, 2];
    let error = cursor_region_bounds(&anchor, &active).expect_err("different parents should fail");
    assert!(error.contains("contiguous siblings"), "error: {error}");
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
    };

    save_cursor_state(&snapshot_file, &state).expect("cursor state should save");
    assert_eq!(load_cursor_state(&snapshot_file).expect("cursor state should load"), state);
    let content = fs::read_to_string(cursor_file_path(&snapshot_file)).expect("cursor state file should read");
    let root = cirru_edn::parse(&content)
      .expect("cursor state should parse")
      .view_map()
      .expect("cursor state should be a map");
    assert_eq!(
      root
        .tag_get("active")
        .expect("cursor active name should exist")
        .read_tag_str()
        .expect("cursor active name should be a tag")
        .as_ref(),
      DEFAULT_CURSOR,
    );
    assert_eq!(
      root
        .tag_get("schema-version")
        .expect("cursor schema version should exist")
        .read_number()
        .expect("cursor schema version should be numeric"),
      4.0
    );
    assert!(
      !content.contains(":preview"),
      "cursor history should not persist full subtree previews"
    );
  }

  #[test]
  fn staged_cut_keeps_clipboard_recoverable_when_snapshot_commit_fails() {
    let fixture = TestCursorSnapshot::from_fixture();
    let cursor_destination = fixture.directory.join("cut-cursor.cirru");
    let snapshot_destination = fixture.directory.join("cut-snapshot.cirru");
    fs::write(&snapshot_destination, "source").expect("cut snapshot destination should initialize");
    let staged_cursor = stage_atomic_file(&cursor_destination, b"clipboard", "test cursor").expect("cursor should stage");
    let staged_snapshot = stage_atomic_file(&snapshot_destination, b"cut", "test snapshot").expect("snapshot should stage");
    fs::remove_file(&snapshot_destination).expect("snapshot destination should be replaced for failure injection");
    fs::create_dir(&snapshot_destination).expect("snapshot failure directory should be created");

    let error = commit_cut_staged_files(staged_cursor, staged_snapshot).expect_err("snapshot commit should fail");
    assert!(error.contains("recoverable from `calcit cursor clipboard`"), "error: {error}");
    assert_eq!(
      fs::read_to_string(&cursor_destination).expect("clipboard checkpoint should persist"),
      "clipboard"
    );
  }

  #[test]
  fn staged_paste_reports_snapshot_success_when_cursor_commit_fails() {
    let fixture = TestCursorSnapshot::from_fixture();
    let snapshot_destination = fixture.directory.join("paste-snapshot.cirru");
    let cursor_destination = fixture.directory.join("paste-cursor.cirru");
    fs::write(&snapshot_destination, "source").expect("paste snapshot destination should initialize");
    fs::write(&cursor_destination, "cursor").expect("paste cursor destination should initialize");
    let staged_snapshot = stage_atomic_file(&snapshot_destination, b"pasted", "test snapshot").expect("snapshot should stage");
    let staged_cursor = stage_atomic_file(&cursor_destination, b"next cursor", "test cursor").expect("cursor should stage");
    fs::remove_file(&cursor_destination).expect("cursor destination should be replaced for failure injection");
    fs::create_dir(&cursor_destination).expect("cursor failure directory should be created");

    let error = commit_paste_staged_files(staged_snapshot, staged_cursor).expect_err("cursor commit should fail");
    assert!(error.contains("Paste succeeded in the snapshot"), "error: {error}");
    assert!(error.contains("Do not retry blindly"), "error: {error}");
    assert_eq!(
      fs::read_to_string(&snapshot_destination).expect("pasted snapshot should persist"),
      "pasted"
    );
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
      },
    )
    .expect("cursor state should save");
    set_cursor_anchor(&snapshot_file).expect("cursor anchor should save");
    save_cursor_mark(&snapshot_file, "insertion").expect("cursor mark should save");

    let mut snapshot = load_snapshot(&snapshot_file).expect("fixture snapshot should load");
    let entry = snapshot
      .files
      .get_mut("app.main")
      .and_then(|file| file.defs.get_mut("main!"))
      .expect("fixture main definition should exist");
    entry.code = apply_operation_at_path(
      &entry.code,
      &[48, 1],
      TreeOperation::InsertBefore,
      Some(&cirru_parser::Cirru::leaf("false")),
    )
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
    let document = load_cursor_document(&snapshot_file).expect("cursor document should load");
    assert_eq!(
      document.anchor.as_ref().map(|state| state.path.as_slice()),
      Some([48, 2].as_slice())
    );
    assert_eq!(
      document.marks.get("insertion").map(|state| state.path.as_slice()),
      Some([48, 2].as_slice())
    );
    assert_eq!(
      read_cursor_target(&snapshot_file, "app.main/main!", &updated.path)
        .expect("updated cursor target should exist")
        .0,
      cirru_parser::Cirru::leaf("true")
    );
  }

  #[test]
  fn cursor_document_round_trips_named_cursors_history_stack_and_clipboard() {
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
    };
    let inactive_path = vec![48, 0];
    let (inactive_node, inactive_revision) =
      read_cursor_target(&snapshot_file, "app.main/main!", &inactive_path).expect("inactive cursor target should exist");
    let inactive_state = CursorState {
      snapshot: snapshot_file.clone(),
      target: "app.main/main!".to_string(),
      path: inactive_path,
      definition_revision: inactive_revision,
      fingerprint: node_fingerprint(&inactive_node),
    };
    let document = CursorDocument {
      active_name: "agent-a".to_string(),
      active: state.clone(),
      inactive_cursors: BTreeMap::from([("agent-b".to_string(), inactive_state.clone())]),
      history: vec![state.clone()],
      stack: vec![state.clone()],
      anchor: Some(state.clone()),
      marks: BTreeMap::from([("main".to_string(), state.clone())]),
      last_query: Some(CursorLastQuery {
        command: "search".to_string(),
        pattern: "true".to_string(),
        filter: Some("app.main/main!".to_string()),
        exact: true,
        regex: false,
        max_depth: 0,
        start_path: None,
        entry: None,
        pattern_is_json: false,
        selected_index: 0,
        snapshot_revision: "md5:test".to_string(),
      }),
      clipboard: Some(CursorClipboard {
        mode: "copy".to_string(),
        source_target: state.target.clone(),
        source_path: path,
        fingerprint: node_fingerprint(&node),
        tree: node,
      }),
    };

    save_cursor_document(&snapshot_file, &document).expect("cursor document should save");
    let loaded = load_cursor_document(&snapshot_file).expect("cursor document should load");
    assert_eq!(loaded.active, state);
    assert_eq!(loaded.inactive_cursors.get("agent-b"), Some(&inactive_state));
    assert_eq!(loaded, document);
  }

  #[test]
  fn oversized_cursor_clipboard_is_rejected_before_writing() {
    let fixture = TestCursorSnapshot::from_fixture();
    let snapshot_file = fixture.snapshot_string();
    let path = vec![48, 1];
    let (node, revision) = read_cursor_target(&snapshot_file, "app.main/main!", &path).expect("fixture cursor target should exist");
    let state = CursorState {
      snapshot: snapshot_file,
      target: "app.main/main!".to_string(),
      path: path.clone(),
      definition_revision: revision,
      fingerprint: node_fingerprint(&node),
    };
    let document = CursorDocument {
      active_name: DEFAULT_CURSOR.to_string(),
      active: state.clone(),
      inactive_cursors: BTreeMap::new(),
      history: vec![],
      stack: vec![],
      anchor: None,
      marks: BTreeMap::new(),
      last_query: None,
      clipboard: Some(CursorClipboard {
        mode: "copy".to_string(),
        source_target: state.target,
        source_path: path,
        fingerprint: state.fingerprint,
        tree: Cirru::leaf(format!("|{}", "x".repeat(70 * 1024))),
      }),
    };

    let error = render_cursor_document(&document).expect_err("oversized clipboard should fail");
    assert!(error.contains("exceeding the 65536 byte limit"), "error: {error}");
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
    fs::write(legacy_cursor_file_path(&snapshot_file), legacy).expect("legacy cursor should write");

    let loaded = load_cursor_document(&snapshot_file).expect("legacy cursor should load");
    assert_eq!(loaded.active, state);
    assert!(loaded.history.is_empty());
    assert!(loaded.stack.is_empty());
    assert!(loaded.anchor.is_none());
    assert!(loaded.marks.is_empty());
    assert!(loaded.last_query.is_none());
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
      },
    )
    .expect("cursor state should save");

    let mut snapshot = load_snapshot(&snapshot_file).expect("fixture snapshot should load");
    let entry = snapshot
      .files
      .get_mut("app.main")
      .and_then(|file| file.defs.get_mut("main!"))
      .expect("fixture main definition should exist");
    let duplicated =
      apply_operation_at_path(&entry.code, &[48, 1], TreeOperation::InsertAfter, Some(&node)).expect("duplicate should insert");
    let after_move_insert =
      apply_operation_at_path(&duplicated, &[48, 0], TreeOperation::InsertAfter, Some(&node)).expect("move destination should insert");
    entry.code = apply_operation_at_path(&after_move_insert, &[48, 2], TreeOperation::Delete, None).expect("old source should delete");
    save_snapshot(&snapshot, &snapshot_file).expect("mutated fixture should save");

    maintain_cursor_after_node_move(&snapshot_file, "app.main/main!", &[48, 1], &[48, 0], TreeOperation::InsertAfter, 0)
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

  #[test]
  fn cursor_native_swap_paredit_and_duplicate_keep_selection_attached() {
    let fixture = TestCursorSnapshot::from_fixture();
    let snapshot_file = fixture.snapshot_string();
    let mut snapshot = load_snapshot(&snapshot_file).expect("cursor paredit fixture should load");
    let entry = snapshot
      .files
      .get_mut("app.main")
      .and_then(|file| file.defs.get_mut("main!"))
      .expect("cursor paredit definition should exist");
    entry.code = Cirru::List(vec![
      Cirru::leaf("head"),
      Cirru::List(vec![Cirru::leaf("a"), Cirru::leaf("b")]),
      Cirru::leaf("tail"),
    ]);
    save_snapshot(&snapshot, &snapshot_file).expect("cursor paredit fixture should save");

    set_cursor_selection(&snapshot_file, "app.main/main!", vec![1, 0]).expect("cursor should select first list child");
    apply_at_cursor(
      &snapshot_file,
      &CursorApplyCommand {
        operation: "swap-next".to_string(),
        file: None,
        code: None,
        depth: 2,
      },
    )
    .expect("cursor-native swap should succeed");
    assert_eq!(
      load_cursor_state(&snapshot_file).expect("swapped cursor should load").path,
      vec![1, 1]
    );

    set_cursor_selection(&snapshot_file, "app.main/main!", vec![1]).expect("cursor should select list");
    slurp_next(&snapshot_file).expect("selected list should slurp its next sibling");
    assert_eq!(load_cursor_state(&snapshot_file).expect("slurped cursor should load").path, vec![1]);
    assert_eq!(
      read_cursor_target(&snapshot_file, "app.main/main!", &[1])
        .expect("slurped list should exist")
        .0,
      Cirru::List(vec![Cirru::leaf("b"), Cirru::leaf("a"), Cirru::leaf("tail")])
    );

    barf_last(&snapshot_file).expect("selected list should barf its last child");
    assert_eq!(load_cursor_state(&snapshot_file).expect("barfed cursor should load").path, vec![1]);
    slurp_prev(&snapshot_file).expect("selected list should slurp its previous sibling");
    assert_eq!(
      load_cursor_state(&snapshot_file).expect("backward-slurped cursor should load").path,
      vec![0]
    );
    assert_eq!(
      read_cursor_target(&snapshot_file, "app.main/main!", &[0])
        .expect("backward-slurped list should exist")
        .0,
      Cirru::List(vec![Cirru::leaf("head"), Cirru::leaf("b"), Cirru::leaf("a")])
    );

    barf_first(&snapshot_file).expect("selected list should barf its first child");
    assert_eq!(
      load_cursor_state(&snapshot_file).expect("first-barfed cursor should load").path,
      vec![1]
    );
    duplicate_cursor(&snapshot_file, &CursorDuplicateCommand { at: "after".to_string() }).expect("cursor duplicate should succeed");
    assert_eq!(
      load_cursor_state(&snapshot_file).expect("duplicate cursor should load").path,
      vec![2]
    );
    assert_eq!(
      read_cursor_target(&snapshot_file, "app.main/main!", &[])
        .expect("duplicated definition should exist")
        .0,
      Cirru::List(vec![
        Cirru::leaf("head"),
        Cirru::List(vec![Cirru::leaf("b"), Cirru::leaf("a")]),
        Cirru::List(vec![Cirru::leaf("b"), Cirru::leaf("a")]),
        Cirru::leaf("tail"),
      ])
    );
  }

  #[test]
  fn cursor_depth_first_navigation_crosses_list_boundaries_once_per_command() {
    let fixture = TestCursorSnapshot::from_fixture();
    let snapshot_file = fixture.snapshot_string();
    let mut snapshot = load_snapshot(&snapshot_file).expect("cursor traversal fixture should load");
    let entry = snapshot
      .files
      .get_mut("app.main")
      .and_then(|file| file.defs.get_mut("main!"))
      .expect("cursor traversal definition should exist");
    entry.code = Cirru::List(vec![
      Cirru::leaf("a"),
      Cirru::List(vec![Cirru::leaf("b"), Cirru::List(vec![Cirru::leaf("c")])]),
      Cirru::leaf("d"),
    ]);
    save_snapshot(&snapshot, &snapshot_file).expect("cursor traversal fixture should save");

    set_cursor_selection(&snapshot_file, "app.main/main!", vec![]).expect("cursor should select definition root");
    move_cursor_depth_first(&snapshot_file, 5, true).expect("cursor should cross into a nested list");
    assert_eq!(
      load_cursor_state(&snapshot_file).expect("forward cursor should load").path,
      vec![1, 1, 0]
    );

    move_cursor_depth_first(&snapshot_file, 3, false).expect("cursor should cross back out of a nested list");
    assert_eq!(
      load_cursor_state(&snapshot_file).expect("backward cursor should load").path,
      vec![1]
    );

    let error = move_cursor_depth_first(&snapshot_file, 10, true).expect_err("cursor should not move past definition end");
    assert!(error.contains("reached the definition end"), "error: {error}");
    assert_eq!(
      load_cursor_state(&snapshot_file)
        .expect("failed traversal should preserve cursor")
        .path,
      vec![1]
    );
  }

  #[test]
  fn cursor_native_edits_reject_invalid_boundaries_without_changing_snapshot() {
    let fixture = TestCursorSnapshot::from_fixture();
    let snapshot_file = fixture.snapshot_string();
    let mut snapshot = load_snapshot(&snapshot_file).expect("cursor boundary fixture should load");
    let entry = snapshot
      .files
      .get_mut("app.main")
      .and_then(|file| file.defs.get_mut("main!"))
      .expect("cursor boundary definition should exist");
    entry.code = Cirru::List(vec![Cirru::leaf("a"), Cirru::List(vec![])]);
    save_snapshot(&snapshot, &snapshot_file).expect("cursor boundary fixture should save");
    let original = fs::read_to_string(&snapshot_file).expect("cursor boundary fixture should read");

    set_cursor_selection(&snapshot_file, "app.main/main!", vec![]).expect("cursor should select definition root");
    assert!(
      slurp_next(&snapshot_file)
        .expect_err("root cannot slurp")
        .contains("Definition root")
    );
    assert!(barf_last(&snapshot_file).expect_err("root cannot barf").contains("definition root"));
    assert!(
      barf_first(&snapshot_file)
        .expect_err("root cannot barf first")
        .contains("definition root")
    );
    assert!(
      duplicate_cursor(&snapshot_file, &CursorDuplicateCommand { at: "after".to_string() },)
        .expect_err("root cannot duplicate")
        .contains("definition root")
    );
    assert!(
      duplicate_cursor(
        &snapshot_file,
        &CursorDuplicateCommand {
          at: "sideways".to_string(),
        },
      )
      .expect_err("invalid duplicate position should fail")
      .contains("Use before or after")
    );

    set_cursor_selection(&snapshot_file, "app.main/main!", vec![0]).expect("cursor should select leaf");
    assert!(
      slurp_next(&snapshot_file)
        .expect_err("leaf cannot slurp")
        .contains("requires the selected node to be a list")
    );
    assert!(
      slurp_prev(&snapshot_file)
        .expect_err("first sibling cannot slurp previous")
        .contains("requires the selected node to be a list")
    );
    let error = apply_at_cursor(
      &snapshot_file,
      &CursorApplyCommand {
        operation: "swap-next".to_string(),
        file: None,
        code: Some("quote ignored".to_string()),
        depth: 2,
      },
    )
    .expect_err("code should be rejected for a code-free operation");
    assert!(error.contains("does not accept --file or --code"), "error: {error}");

    set_cursor_selection(&snapshot_file, "app.main/main!", vec![1]).expect("cursor should select empty list");
    assert!(
      slurp_next(&snapshot_file)
        .expect_err("last sibling cannot slurp")
        .contains("no next sibling")
    );
    assert!(barf_last(&snapshot_file).expect_err("empty list cannot barf").contains("no child"));
    assert!(
      barf_first(&snapshot_file)
        .expect_err("empty list cannot barf first")
        .contains("no child")
    );
    assert_eq!(
      fs::read_to_string(&snapshot_file).expect("cursor boundary fixture should reread"),
      original
    );
  }
}
