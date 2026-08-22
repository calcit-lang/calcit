use crate::call_tree::{self, CallTreeNode};
use crate::cli_args::CallGraphDiffCommand;
use crate::program::{self, PROGRAM_CODE_DATA};
use crate::program_diff::DiffStatus;
use crate::snapshot::{CodeEntry, Snapshot, load_snapshot_data};
use crate::util::string::strip_shebang;
use cirru_edn::Edn;
use colored::Colorize;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct CallGraphDiffNode {
  pub fqn: String,
  pub status: DiffStatus,
  pub code_changed: bool,
  pub detail: Option<String>,
  pub children: Vec<CallGraphDiffNode>,
}

#[derive(Debug, Clone, Default)]
pub struct CallGraphDiffStats {
  pub unchanged: usize,
  pub added: usize,
  pub removed: usize,
  pub modified: usize,
  pub code_changed: usize,
}

#[derive(Debug, Clone)]
pub struct CallGraphDiffResult {
  pub git_ref: String,
  pub file_path: String,
  pub entry: String,
  pub root: CallGraphDiffNode,
  pub stats: CallGraphDiffStats,
}

pub fn analyze_call_graph_diff(cmd: &CallGraphDiffCommand, input_path: &str) -> Result<CallGraphDiffResult, String> {
  let cwd = env::current_dir().map_err(|e| format!("Failed to read current directory: {e}"))?;
  let input_abs = resolve_input_path(&cwd, input_path)?;
  let repo_search_dir = input_abs.parent().unwrap_or(cwd.as_path());
  let repo_root = git_root(repo_search_dir)?;
  let repo_rel_path = repo_relative_path(&input_abs, &repo_root)?;
  let snapshot_path = repo_rel_path.to_string_lossy().to_string();

  let base_dir = input_abs.parent().unwrap_or(cwd.as_path());
  let module_folder = crate::project_module_folder(base_dir);
  let core_snapshot = crate::load_core_snapshot()?;

  let current_content = fs::read_to_string(&input_abs).map_err(|e| format!("Failed to read {}: {e}", input_abs.display()))?;
  let mut current_snapshot = parse_snapshot(&current_content, input_path, &snapshot_path)?;
  attach_modules_and_core(&mut current_snapshot, base_dir, &module_folder, &core_snapshot)?;

  let historical_content = git_show_file(&repo_root, &cmd.git_ref, &repo_rel_path)?;
  let historical_label = format!("{}:{}", cmd.git_ref, repo_rel_path.display());
  let mut historical_snapshot = parse_snapshot(&historical_content, &historical_label, &snapshot_path)?;
  attach_modules_and_core(&mut historical_snapshot, base_dir, &module_folder, &core_snapshot)?;

  let (entry_ns, entry_def) = resolve_entry_point(&current_snapshot, cmd.root.as_deref())?;
  let entry = format!("{entry_ns}/{entry_def}");

  let old_graph = analyze_snapshot_call_graph(&historical_snapshot, &entry_ns, &entry_def, cmd)?;
  let new_graph = analyze_snapshot_call_graph(&current_snapshot, &entry_ns, &entry_def, cmd)?;
  let changed_defs = collect_changed_definitions(&historical_snapshot, &current_snapshot);

  let root = diff_call_graph_node(&old_graph.tree, &new_graph.tree, &changed_defs);
  let stats = collect_stats(&root);

  Ok(CallGraphDiffResult {
    git_ref: cmd.git_ref.to_string(),
    file_path: repo_rel_path.to_string_lossy().to_string(),
    entry,
    root,
    stats,
  })
}

pub fn format_call_graph_diff(result: &CallGraphDiffResult) -> String {
  let mut output = String::new();
  output.push_str("# Call Graph Diff\n\n");
  output.push_str(&format!("- ref: {}\n", result.git_ref));
  output.push_str(&format!("- file: {}\n", result.file_path));
  output.push_str(&format!("- entry: {}\n", result.entry));
  output.push_str(&format!(
    "- graph changes: ~{} +{} -{} ={}\n",
    result.stats.modified, result.stats.added, result.stats.removed, result.stats.unchanged
  ));
  output.push_str(&format!("- code-changed reachable defs: {}\n\n", result.stats.code_changed));
  output.push_str("## Graph Diff\n\n");
  format_tree_node(&result.root, &mut output, "", true, true, true);
  output
}

fn analyze_snapshot_call_graph(
  snapshot: &Snapshot,
  entry_ns: &str,
  entry_def: &str,
  cmd: &CallGraphDiffCommand,
) -> Result<call_tree::CallTreeResult, String> {
  let program_data = program::extract_program_data(snapshot)?;
  {
    let mut program = PROGRAM_CODE_DATA.write().map_err(|e| format!("Failed to open program data: {e}"))?;
    *program = program_data;
  }

  call_tree::analyze_call_graph(
    entry_ns,
    entry_def,
    cmd.include_core,
    cmd.max_depth,
    false,
    Some(snapshot.package.to_string()),
    cmd.ns_prefix.clone(),
  )
}

fn attach_modules_and_core(
  snapshot: &mut Snapshot,
  base_dir: &Path,
  module_folder: &Path,
  core_snapshot: &Snapshot,
) -> Result<(), String> {
  let module_paths = snapshot.active_entry()?.modules.to_vec();
  for module_path in &module_paths {
    let module_data = crate::load_module(module_path, base_dir, module_folder)?;
    crate::merge_module_files(snapshot, &module_data, module_path)?;
  }

  for (ns, file) in &core_snapshot.files {
    snapshot.files.insert(ns.to_owned(), file.to_owned());
  }

  Ok(())
}

fn resolve_entry_point(snapshot: &Snapshot, root: Option<&str>) -> Result<(String, String), String> {
  let entry = root.unwrap_or(snapshot.active_entry()?.init_fn.as_str());
  let (ns, def) = entry
    .split_once('/')
    .ok_or_else(|| format!("Expected entry definition in format ns/def, got: {entry}"))?;
  Ok((ns.to_string(), def.to_string()))
}

fn collect_changed_definitions(old: &Snapshot, new: &Snapshot) -> HashSet<String> {
  let mut changed = HashSet::new();

  for (ns, old_file) in &old.files {
    if let Some(new_file) = new.files.get(ns) {
      for (def, old_entry) in &old_file.defs {
        if let Some(new_entry) = new_file.defs.get(def)
          && code_entry_changed(old_entry, new_entry)
        {
          changed.insert(format!("{ns}/{def}"));
        }
      }
    }
  }

  changed
}

fn code_entry_changed(old: &CodeEntry, new: &CodeEntry) -> bool {
  old.code != new.code
}

fn diff_call_graph_node(old: &CallTreeNode, new: &CallTreeNode, changed_defs: &HashSet<String>) -> CallGraphDiffNode {
  let children = diff_children(&old.calls, &new.calls, changed_defs);
  let meta_changed = old.circular != new.circular || old.seen != new.seen || old.source != new.source;
  let status = if meta_changed || children.iter().any(|child| child.status != DiffStatus::Unchanged) {
    DiffStatus::Modified
  } else {
    DiffStatus::Unchanged
  };

  CallGraphDiffNode {
    fqn: new.fqn.to_string(),
    status,
    code_changed: changed_defs.contains(&new.fqn),
    detail: build_detail(Some(old), Some(new)),
    children,
  }
}

fn diff_children(
  old_children: &[CallTreeNode],
  new_children: &[CallTreeNode],
  changed_defs: &HashSet<String>,
) -> Vec<CallGraphDiffNode> {
  let new_lookup = new_children
    .iter()
    .enumerate()
    .map(|(idx, node)| (node.fqn.as_str(), idx))
    .collect::<HashMap<_, _>>();
  let old_lookup = old_children
    .iter()
    .enumerate()
    .map(|(idx, node)| (node.fqn.as_str(), idx))
    .collect::<HashMap<_, _>>();

  let mut children = Vec::new();
  let mut used_new = HashSet::new();

  for old_child in old_children {
    if let Some(&new_idx) = new_lookup.get(old_child.fqn.as_str()) {
      children.push(diff_call_graph_node(old_child, &new_children[new_idx], changed_defs));
      used_new.insert(new_idx);
    } else {
      children.push(build_call_graph_subtree(old_child, DiffStatus::Removed));
    }
  }

  for (idx, new_child) in new_children.iter().enumerate() {
    if !used_new.contains(&idx) && !old_lookup.contains_key(new_child.fqn.as_str()) {
      children.push(build_call_graph_subtree(new_child, DiffStatus::Added));
    }
  }

  children
}

fn build_call_graph_subtree(node: &CallTreeNode, status: DiffStatus) -> CallGraphDiffNode {
  let children = node.calls.iter().map(|child| build_call_graph_subtree(child, status)).collect();
  CallGraphDiffNode {
    fqn: node.fqn.to_string(),
    status,
    code_changed: false,
    detail: build_detail(None, Some(node)),
    children,
  }
}

fn build_detail(old: Option<&CallTreeNode>, new: Option<&CallTreeNode>) -> Option<String> {
  match (old, new) {
    (Some(old), Some(new)) => {
      let old_tags = node_tags(old);
      let new_tags = node_tags(new);
      if old_tags == new_tags {
        render_tags(&new_tags)
      } else {
        Some(format!("{} -> {}", render_tags_or_dash(&old_tags), render_tags_or_dash(&new_tags)))
      }
    }
    (_, Some(node)) => render_tags(&node_tags(node)),
    (Some(node), None) => render_tags(&node_tags(node)),
    (None, None) => None,
  }
}

fn node_tags(node: &CallTreeNode) -> Vec<&'static str> {
  let mut tags = Vec::new();
  if node.circular {
    tags.push("CIRCULAR");
  }
  if node.seen {
    tags.push("seen");
  }
  match node.source.as_str() {
    "core" => tags.push("core"),
    "external" => tags.push("ext"),
    _ => {}
  }
  tags
}

fn render_tags(tags: &[&'static str]) -> Option<String> {
  if tags.is_empty() {
    None
  } else {
    Some(format!("[{}]", tags.join(", ")))
  }
}

fn render_tags_or_dash(tags: &[&'static str]) -> String {
  render_tags(tags).unwrap_or_else(|| "-".to_string())
}

fn collect_stats(root: &CallGraphDiffNode) -> CallGraphDiffStats {
  fn walk(node: &CallGraphDiffNode, stats: &mut CallGraphDiffStats) {
    match node.status {
      DiffStatus::Unchanged => stats.unchanged += 1,
      DiffStatus::Added => stats.added += 1,
      DiffStatus::Removed => stats.removed += 1,
      DiffStatus::Modified => stats.modified += 1,
    }
    if node.code_changed {
      stats.code_changed += 1;
    }
    for child in &node.children {
      walk(child, stats);
    }
  }

  let mut stats = CallGraphDiffStats::default();
  walk(root, &mut stats);
  stats
}

fn format_tree_node(node: &CallGraphDiffNode, output: &mut String, prefix: &str, is_last: bool, expand: bool, is_root: bool) {
  let connector = if is_root {
    ""
  } else if is_last {
    "└── "
  } else {
    "├── "
  };

  let mut suffix = String::new();
  if let Some(detail) = &node.detail {
    suffix.push(' ');
    suffix.push_str(detail);
  }
  if !expand && !node.children.is_empty() {
    suffix.push_str(&format!(" {}", format!("({} folded)", descendant_count(node)).dimmed()));
  }

  let mut line = format!("{} {}", status_badge(node.status), status_paint(node.status, node.fqn.to_string()));
  if !suffix.is_empty() {
    line.push_str(&status_paint(node.status, suffix));
  }
  if node.code_changed {
    line.push(' ');
    line.push_str(&modified_marker());
  }

  output.push_str(&format!("{prefix}{connector}{line}\n"));

  if !expand {
    return;
  }

  let child_prefix = if is_root {
    String::new()
  } else {
    format!("{}{}   ", prefix, if is_last { " " } else { "│" })
  };

  for (idx, child) in node.children.iter().enumerate() {
    let child_expand = child.status != DiffStatus::Unchanged || has_code_changes(child);
    format_tree_node(child, output, &child_prefix, idx + 1 == node.children.len(), child_expand, false);
  }
}

fn has_code_changes(node: &CallGraphDiffNode) -> bool {
  node.code_changed || node.children.iter().any(has_code_changes)
}

fn descendant_count(node: &CallGraphDiffNode) -> usize {
  node.children.iter().map(|child| 1 + descendant_count(child)).sum()
}

fn status_badge(status: DiffStatus) -> String {
  match status {
    DiffStatus::Unchanged => "=".dimmed().to_string(),
    DiffStatus::Added => "[+]".black().on_green().bold().to_string(),
    DiffStatus::Removed => "[-]".black().on_bright_blue().bold().to_string(),
    DiffStatus::Modified => "~".dimmed().to_string(),
  }
}

fn status_paint(status: DiffStatus, text: String) -> String {
  match status {
    DiffStatus::Unchanged => text.dimmed().to_string(),
    DiffStatus::Added => text.green().to_string(),
    DiffStatus::Removed => text.cyan().to_string(),
    DiffStatus::Modified => text.dimmed().to_string(),
  }
}

fn modified_marker() -> String {
  "[MODIFIED]".black().on_yellow().bold().to_string()
}

fn resolve_input_path(cwd: &Path, input_path: &str) -> Result<PathBuf, String> {
  let input = Path::new(input_path);
  let full_path = if input.is_absolute() {
    input.to_path_buf()
  } else {
    cwd.join(input)
  };
  let resolved = crate::resolve_snapshot_path_alias(&full_path);
  resolved
    .canonicalize()
    .map_err(|e| format!("Failed to resolve input path '{}': {e}", resolved.display()))
}

fn git_root(cwd: &Path) -> Result<PathBuf, String> {
  let output = Command::new("git")
    .current_dir(cwd)
    .args(["rev-parse", "--show-toplevel"])
    .output()
    .map_err(|e| format!("Failed to run git rev-parse: {e}"))?;

  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    return Err(format!("Failed to locate git repository root: {}", stderr.trim()));
  }

  let stdout = String::from_utf8(output.stdout).map_err(|e| format!("Failed to decode git output: {e}"))?;
  Ok(PathBuf::from(stdout.trim()))
}

fn repo_relative_path(input_abs: &Path, repo_root: &Path) -> Result<PathBuf, String> {
  input_abs.strip_prefix(repo_root).map(|path| path.to_path_buf()).map_err(|_| {
    format!(
      "Input file '{}' is not inside git repository '{}'",
      input_abs.display(),
      repo_root.display()
    )
  })
}

fn git_show_file(repo_root: &Path, git_ref: &str, repo_rel_path: &Path) -> Result<String, String> {
  let git_path = repo_rel_path.to_string_lossy().replace('\\', "/");
  let object = format!("{git_ref}:{git_path}");
  let output = Command::new("git")
    .current_dir(repo_root)
    .args(["show", &object])
    .output()
    .map_err(|e| format!("Failed to run git show for '{object}': {e}"))?;

  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    return Err(format!("Failed to load '{git_path}' from ref '{git_ref}': {}", stderr.trim()));
  }

  String::from_utf8(output.stdout).map_err(|e| format!("Failed to decode git show output: {e}"))
}

fn parse_snapshot(content: &str, error_label: &str, snapshot_path: &str) -> Result<Snapshot, String> {
  let mut content = content.to_string();
  strip_shebang(&mut content);
  let parsed: Edn = cirru_edn::parse(&content).map_err(|e| format!("Failed to parse '{error_label}' as Cirru EDN: {e}"))?;
  load_snapshot_data(&parsed, snapshot_path).map_err(|e| format!("Failed to load snapshot '{error_label}': {e}"))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn node(fqn: &str, calls: Vec<CallGraphDiffNode>) -> CallGraphDiffNode {
    CallGraphDiffNode {
      fqn: fqn.to_string(),
      status: DiffStatus::Unchanged,
      code_changed: false,
      detail: None,
      children: calls,
    }
  }

  fn tree(fqn: &str, calls: Vec<CallTreeNode>) -> CallTreeNode {
    let (ns, def) = fqn.split_once('/').unwrap();
    CallTreeNode {
      ns: ns.to_string(),
      def: def.to_string(),
      fqn: fqn.to_string(),
      doc: None,
      calls,
      circular: false,
      seen: false,
      source: "project".to_string(),
    }
  }

  #[test]
  fn marks_code_changed_without_graph_change() {
    colored::control::set_override(false);
    let old = tree("app.main/main", vec![tree("app.util/helper", vec![])]);
    let new = tree("app.main/main", vec![tree("app.util/helper", vec![])]);
    let changed = HashSet::from(["app.util/helper".to_string()]);

    let diff = diff_call_graph_node(&old, &new, &changed);
    assert_eq!(diff.status, DiffStatus::Unchanged);
    assert!(diff.children[0].code_changed);
  }

  #[test]
  fn marks_added_and_removed_children() {
    let old = tree("app.main/main", vec![tree("app.old/gone", vec![])]);
    let new = tree("app.main/main", vec![tree("app.new/here", vec![])]);
    let diff = diff_call_graph_node(&old, &new, &HashSet::new());

    assert_eq!(diff.status, DiffStatus::Modified);
    assert_eq!(diff.children[0].status, DiffStatus::Removed);
    assert_eq!(diff.children[1].status, DiffStatus::Added);
  }

  #[test]
  fn formats_code_change_badge() {
    colored::control::set_override(false);
    let mut root = node("app.main/main", vec![node("app.util/helper", vec![])]);
    root.children[0].code_changed = true;

    let mut output = String::new();
    format_tree_node(&root, &mut output, "", true, true, true);
    assert!(output.contains("[MODIFIED]"));
  }
}
