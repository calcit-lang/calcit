use crate::snapshot::{CodeEntry, FileInSnapShot, NsEntry, Snapshot, SnapshotConfigs, load_snapshot_data};
use crate::util::string::strip_shebang;
use cirru_edn::Edn;
use cirru_parser::Cirru;
use colored::Colorize;
use std::collections::{BTreeSet, HashMap};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffStatus {
  Unchanged,
  Added,
  Removed,
  Modified,
}

impl DiffStatus {
  fn symbol(self) -> &'static str {
    match self {
      DiffStatus::Unchanged => "=",
      DiffStatus::Added => "+",
      DiffStatus::Removed => "-",
      DiffStatus::Modified => "~",
    }
  }

  fn badge(self) -> String {
    match self {
      DiffStatus::Unchanged => self.symbol().dimmed().to_string(),
      DiffStatus::Added => format!(" {} ", self.symbol()).black().on_green().bold().to_string(),
      DiffStatus::Removed => format!(" {} ", self.symbol()).black().on_bright_blue().bold().to_string(),
      DiffStatus::Modified => self.symbol().dimmed().to_string(),
    }
  }

  fn paint(self, text: String) -> String {
    match self {
      DiffStatus::Unchanged => text.dimmed().to_string(),
      DiffStatus::Added => text.green().to_string(),
      DiffStatus::Removed => text.cyan().to_string(),
      DiffStatus::Modified => text.dimmed().to_string(),
    }
  }
}

#[derive(Debug, Clone)]
pub struct DiffNode {
  pub label: String,
  pub status: DiffStatus,
  pub detail: Option<String>,
  pub body: Option<String>,
  pub children: Vec<DiffNode>,
}

impl DiffNode {
  fn new(label: impl Into<String>, status: DiffStatus) -> Self {
    DiffNode {
      label: label.into(),
      status,
      detail: None,
      body: None,
      children: vec![],
    }
  }

  fn with_detail(mut self, detail: impl Into<String>) -> Self {
    self.detail = Some(detail.into());
    self
  }

  fn with_children(mut self, children: Vec<DiffNode>) -> Self {
    self.children = children;
    self
  }

  fn with_body(mut self, body: impl Into<String>) -> Self {
    self.body = Some(body.into());
    self
  }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProgramDiffStats {
  pub unchanged: usize,
  pub added: usize,
  pub removed: usize,
  pub modified: usize,
}

#[derive(Debug, Clone)]
pub struct ProgramDiffResult {
  pub git_ref: String,
  pub file_path: String,
  pub root: DiffNode,
  pub stats: ProgramDiffStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CirruEditStrategy {
  Identical,
  Replace,
  Insert,
  Delete,
  Rewrite,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CirruEditAdvice {
  pub similarity: f64,
  pub stats: ProgramDiffStats,
  pub strategy: CirruEditStrategy,
}

pub fn analyze_program_diff(git_ref: &str, input_path: &str) -> Result<ProgramDiffResult, String> {
  let cwd = env::current_dir().map_err(|e| format!("Failed to read current directory: {e}"))?;
  let input_abs = resolve_input_path(&cwd, input_path)?;
  let repo_search_dir = input_abs.parent().unwrap_or(cwd.as_path());
  let repo_root = git_root(repo_search_dir)?;
  let repo_rel_path = repo_relative_path(&input_abs, &repo_root)?;

  let snapshot_path = repo_rel_path.to_string_lossy().to_string();

  let current_content = fs::read_to_string(&input_abs).map_err(|e| format!("Failed to read {}: {e}", input_abs.display()))?;
  let current_snapshot = parse_snapshot(&current_content, input_path, &snapshot_path)?;

  let historical_content = git_show_file(&repo_root, git_ref, &repo_rel_path)?;
  let historical_label = format!("{git_ref}:{}", repo_rel_path.display());
  let historical_snapshot = parse_snapshot(&historical_content, &historical_label, &snapshot_path)?;

  let root = diff_snapshot(&historical_snapshot, &current_snapshot);
  let stats = collect_stats(&root);

  Ok(ProgramDiffResult {
    git_ref: git_ref.to_string(),
    file_path: repo_rel_path.to_string_lossy().to_string(),
    root,
    stats,
  })
}

pub fn format_program_diff(result: &ProgramDiffResult) -> String {
  let mut output = String::new();
  output.push_str("# Program Diff\n\n");
  output.push_str(&format!("- ref: {}\n", result.git_ref));
  output.push_str(&format!("- file: {}\n", result.file_path));
  output.push_str(&format!(
    "- changes: ~{} +{} -{} ={}\n\n",
    result.stats.modified, result.stats.added, result.stats.removed, result.stats.unchanged
  ));
  output.push_str("## Tree Diff\n\n");
  format_tree_node(&result.root, &mut output, "", true, true, true);
  output
}

pub fn analyze_cirru_edit_advice(old: &Cirru, new: &Cirru) -> Option<CirruEditAdvice> {
  if old == new {
    return Some(CirruEditAdvice {
      similarity: 1.0,
      stats: ProgramDiffStats::default(),
      strategy: CirruEditStrategy::Identical,
    });
  }

  let similarity = cirru_similarity(old, new);
  if similarity < 0.58 {
    return None;
  }

  let stats = collect_cirru_change_stats(old, new);
  let total_changed = stats.added + stats.removed + stats.modified;
  if total_changed == 0 {
    return Some(CirruEditAdvice {
      similarity,
      stats,
      strategy: CirruEditStrategy::Identical,
    });
  }

  let added_ratio = stats.added as f64 / total_changed as f64;
  let removed_ratio = stats.removed as f64 / total_changed as f64;
  let modified_ratio = stats.modified as f64 / total_changed as f64;
  let has_mixed_add_remove = stats.added > 0 && stats.removed > 0;
  let mixed_change_floor = stats.added.min(stats.removed);

  let strategy = if has_mixed_add_remove && mixed_change_floor >= 3 {
    CirruEditStrategy::Rewrite
  } else if modified_ratio >= 0.58 && added_ratio <= 0.22 && removed_ratio <= 0.22 {
    CirruEditStrategy::Replace
  } else if added_ratio >= 0.55 && removed_ratio <= 0.10 {
    CirruEditStrategy::Insert
  } else if removed_ratio >= 0.55 && added_ratio <= 0.10 {
    CirruEditStrategy::Delete
  } else {
    CirruEditStrategy::Rewrite
  };

  Some(CirruEditAdvice {
    similarity,
    stats,
    strategy,
  })
}

fn collect_cirru_change_stats(old: &Cirru, new: &Cirru) -> ProgramDiffStats {
  let mut stats = ProgramDiffStats::default();
  tally_cirru_changes(old, new, &mut stats);
  stats
}

fn tally_cirru_changes(old: &Cirru, new: &Cirru, stats: &mut ProgramDiffStats) {
  if old == new {
    stats.unchanged += count_cirru_nodes(new);
    return;
  }

  match (old, new) {
    (Cirru::Leaf(_), Cirru::Leaf(_)) => {
      stats.modified += 1;
    }
    (Cirru::List(old_items), Cirru::List(new_items)) => {
      let edits = align_sequence(old_items, new_items);
      for edit in edits {
        match edit {
          SeqEdit::Match(i, j) => tally_cirru_changes(&old_items[i], &new_items[j], stats),
          SeqEdit::Replace(i, j) => match (&old_items[i], &new_items[j]) {
            (Cirru::Leaf(_), Cirru::Leaf(_)) => {
              stats.modified += 1;
            }
            (Cirru::List(_), Cirru::List(_)) if cirru_similarity(&old_items[i], &new_items[j]) >= 0.58 => {
              tally_cirru_changes(&old_items[i], &new_items[j], stats);
            }
            _ => {
              stats.removed += count_cirru_nodes(&old_items[i]);
              stats.added += count_cirru_nodes(&new_items[j]);
            }
          },
          SeqEdit::Remove(i) => {
            stats.removed += count_cirru_nodes(&old_items[i]);
          }
          SeqEdit::Insert(j) => {
            stats.added += count_cirru_nodes(&new_items[j]);
          }
        }
      }
    }
    _ => {
      stats.removed += count_cirru_nodes(old);
      stats.added += count_cirru_nodes(new);
    }
  }
}

fn count_cirru_nodes(node: &Cirru) -> usize {
  match node {
    Cirru::Leaf(_) => 1,
    Cirru::List(items) => 1 + items.iter().map(count_cirru_nodes).sum::<usize>(),
  }
}

pub(crate) fn resolve_input_path(cwd: &Path, input_path: &str) -> Result<PathBuf, String> {
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

pub(crate) fn git_root(cwd: &Path) -> Result<PathBuf, String> {
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

pub(crate) fn repo_relative_path(input_abs: &Path, repo_root: &Path) -> Result<PathBuf, String> {
  input_abs.strip_prefix(repo_root).map(|path| path.to_path_buf()).map_err(|_| {
    format!(
      "Input file '{}' is not inside git repository '{}'",
      input_abs.display(),
      repo_root.display()
    )
  })
}

pub(crate) fn git_show_file(repo_root: &Path, git_ref: &str, repo_rel_path: &Path) -> Result<String, String> {
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

pub(crate) fn parse_snapshot(content: &str, error_label: &str, snapshot_path: &str) -> Result<Snapshot, String> {
  let mut content = content.to_string();
  strip_shebang(&mut content);
  let parsed: Edn = cirru_edn::parse(&content).map_err(|e| format!("Failed to parse '{error_label}' as Cirru EDN: {e}"))?;
  load_snapshot_data(&parsed, snapshot_path).map_err(|e| format!("Failed to load snapshot '{error_label}': {e}"))
}

fn diff_snapshot(old: &Snapshot, new: &Snapshot) -> DiffNode {
  let children = vec![
    diff_string("package", Some(old.package.as_str()), Some(new.package.as_str())),
    diff_optional_string("about", old.about.as_deref(), new.about.as_deref()),
    diff_configs("configs", Some(&old.configs), Some(&new.configs)),
    diff_entries("entries", &old.entries, &new.entries),
    diff_files("files", &old.files, &new.files),
  ];

  DiffNode::new("program", aggregate_status(&children)).with_children(children)
}

fn diff_configs(label: &str, old: Option<&SnapshotConfigs>, new: Option<&SnapshotConfigs>) -> DiffNode {
  match (old, new) {
    (Some(old), Some(new)) => {
      let children = vec![
        diff_string("init-fn", Some(old.init_fn.as_str()), Some(new.init_fn.as_str())),
        diff_string("reload-fn", Some(old.reload_fn.as_str()), Some(new.reload_fn.as_str())),
        diff_string("version", Some(old.version.as_str()), Some(new.version.as_str())),
        diff_string_list("modules", &old.modules, &new.modules),
      ];
      DiffNode::new(label, aggregate_status(&children)).with_children(children)
    }
    (None, Some(value)) => build_configs_tree(label, value, DiffStatus::Added),
    (Some(value), None) => build_configs_tree(label, value, DiffStatus::Removed),
    (None, None) => DiffNode::new(label, DiffStatus::Unchanged),
  }
}

fn diff_entries(label: &str, old: &HashMap<String, SnapshotConfigs>, new: &HashMap<String, SnapshotConfigs>) -> DiffNode {
  let mut keys = BTreeSet::new();
  keys.extend(old.keys().cloned());
  keys.extend(new.keys().cloned());

  let children = keys
    .into_iter()
    .map(|key| diff_configs(&key, old.get(&key), new.get(&key)))
    .collect::<Vec<_>>();

  DiffNode::new(label, aggregate_status(&children)).with_children(children)
}

fn diff_files(label: &str, old: &HashMap<String, FileInSnapShot>, new: &HashMap<String, FileInSnapShot>) -> DiffNode {
  let mut keys = BTreeSet::new();
  keys.extend(old.keys().cloned());
  keys.extend(new.keys().cloned());

  let children = keys
    .into_iter()
    .map(|key| diff_file(&key, old.get(&key), new.get(&key)))
    .collect::<Vec<_>>();

  DiffNode::new(label, aggregate_status(&children)).with_children(children)
}

fn diff_file(label: &str, old: Option<&FileInSnapShot>, new: Option<&FileInSnapShot>) -> DiffNode {
  match (old, new) {
    (Some(old), Some(new)) => {
      let children = vec![
        diff_ns_entry("ns", Some(&old.ns), Some(&new.ns)),
        diff_defs("defs", &old.defs, &new.defs),
      ];
      DiffNode::new(label, aggregate_status(&children)).with_children(children)
    }
    (None, Some(value)) => build_file_tree(label, value, DiffStatus::Added),
    (Some(value), None) => build_file_tree(label, value, DiffStatus::Removed),
    (None, None) => DiffNode::new(label, DiffStatus::Unchanged),
  }
}

fn diff_ns_entry(label: &str, old: Option<&NsEntry>, new: Option<&NsEntry>) -> DiffNode {
  match (old, new) {
    (Some(old), Some(new)) => {
      let children = vec![
        diff_string("doc", Some(old.doc.as_str()), Some(new.doc.as_str())),
        diff_cirru("code", Some(&old.code), Some(&new.code), "0"),
      ];
      DiffNode::new(label, aggregate_status(&children)).with_children(children)
    }
    (None, Some(value)) => build_ns_tree(label, value, DiffStatus::Added),
    (Some(value), None) => build_ns_tree(label, value, DiffStatus::Removed),
    (None, None) => DiffNode::new(label, DiffStatus::Unchanged),
  }
}

fn diff_defs(label: &str, old: &HashMap<String, CodeEntry>, new: &HashMap<String, CodeEntry>) -> DiffNode {
  let mut keys = BTreeSet::new();
  keys.extend(old.keys().cloned());
  keys.extend(new.keys().cloned());

  let children = keys
    .into_iter()
    .map(|key| diff_code_entry(&key, old.get(&key), new.get(&key)))
    .collect::<Vec<_>>();

  DiffNode::new(label, aggregate_status(&children)).with_children(children)
}

pub(crate) fn diff_code_entry(label: &str, old: Option<&CodeEntry>, new: Option<&CodeEntry>) -> DiffNode {
  match (old, new) {
    (Some(old), Some(new)) => {
      let children = vec![
        diff_string("doc", Some(old.doc.as_str()), Some(new.doc.as_str())),
        diff_string("schema", Some(&old.schema.to_string()), Some(&new.schema.to_string())),
        diff_cirru_list("examples", &old.examples, &new.examples),
        diff_cirru("code", Some(&old.code), Some(&new.code), "0"),
      ];
      DiffNode::new(label, aggregate_status(&children)).with_children(children)
    }
    (None, Some(value)) => build_code_entry_tree(label, value, DiffStatus::Added),
    (Some(value), None) => build_code_entry_tree(label, value, DiffStatus::Removed),
    (None, None) => DiffNode::new(label, DiffStatus::Unchanged),
  }
}

fn diff_string(label: &str, old: Option<&str>, new: Option<&str>) -> DiffNode {
  match (old, new) {
    (Some(old), Some(new)) if old == new => DiffNode::new(label, DiffStatus::Unchanged).with_detail(render_text(old)),
    (Some(old), Some(new)) => {
      DiffNode::new(label, DiffStatus::Modified).with_detail(format!("{} -> {}", render_text(old), render_text(new)))
    }
    (None, Some(new)) => DiffNode::new(label, DiffStatus::Added).with_detail(render_text(new)),
    (Some(old), None) => DiffNode::new(label, DiffStatus::Removed).with_detail(render_text(old)),
    (None, None) => DiffNode::new(label, DiffStatus::Unchanged),
  }
}

fn diff_optional_string(label: &str, old: Option<&str>, new: Option<&str>) -> DiffNode {
  diff_string(label, old, new)
}

fn diff_string_list(label: &str, old: &[String], new: &[String]) -> DiffNode {
  let edits = align_sequence(old, new);
  let mut children = Vec::with_capacity(edits.len());

  for edit in edits {
    match edit {
      SeqEdit::Match(i, _) => {
        children.push(DiffNode::new(format!("[{i}]"), DiffStatus::Unchanged).with_detail(render_text(&old[i])));
      }
      SeqEdit::Replace(i, j) => {
        children.push(DiffNode::new(format!("[{i}]"), DiffStatus::Modified).with_detail(format!(
          "{} -> {}",
          render_text(&old[i]),
          render_text(&new[j])
        )));
      }
      SeqEdit::Remove(i) => {
        children.push(DiffNode::new(format!("[{i}]"), DiffStatus::Removed).with_detail(render_text(&old[i])));
      }
      SeqEdit::Insert(j) => {
        children.push(DiffNode::new(format!("[{j}]"), DiffStatus::Added).with_detail(render_text(&new[j])));
      }
    }
  }

  DiffNode::new(label, aggregate_status(&children)).with_children(children)
}

fn diff_cirru_list(label: &str, old: &[Cirru], new: &[Cirru]) -> DiffNode {
  let edits = align_sequence(old, new);
  let mut children = Vec::with_capacity(edits.len());

  for edit in edits {
    match edit {
      SeqEdit::Match(i, _) => children.push(diff_cirru(&format!("[{i}]"), Some(&old[i]), Some(&old[i]), &i.to_string())),
      SeqEdit::Replace(i, j) => children.push(diff_cirru(&format!("[{i}]"), Some(&old[i]), Some(&new[j]), &i.to_string())),
      SeqEdit::Remove(i) => children.push(build_cirru_tree(&format!("[{i}]"), &old[i], DiffStatus::Removed, &i.to_string())),
      SeqEdit::Insert(j) => children.push(build_cirru_tree(&format!("[{j}]"), &new[j], DiffStatus::Added, &j.to_string())),
    }
  }

  DiffNode::new(label, aggregate_status(&children)).with_children(children)
}

fn diff_cirru(label: &str, old: Option<&Cirru>, new: Option<&Cirru>, coord: &str) -> DiffNode {
  match (old, new) {
    (Some(old), Some(new)) if old == new => build_cirru_tree(label, new, DiffStatus::Unchanged, coord),
    (Some(old_node), Some(new_node)) => {
      DiffNode::new(label, DiffStatus::Modified).with_body(render_cirru_diff(old_node, new_node, coord, 0, true))
    }
    (None, Some(value)) => build_cirru_tree(label, value, DiffStatus::Added, coord),
    (Some(value), None) => build_cirru_tree(label, value, DiffStatus::Removed, coord),
    (None, None) => DiffNode::new(label, DiffStatus::Unchanged),
  }
}

fn build_configs_tree(label: &str, value: &SnapshotConfigs, status: DiffStatus) -> DiffNode {
  DiffNode::new(label, status).with_children(vec![
    DiffNode::new("init-fn", status).with_detail(render_text(&value.init_fn)),
    DiffNode::new("reload-fn", status).with_detail(render_text(&value.reload_fn)),
    DiffNode::new("version", status).with_detail(render_text(&value.version)),
    build_string_list_tree("modules", &value.modules, status),
  ])
}

fn build_string_list_tree(label: &str, items: &[String], status: DiffStatus) -> DiffNode {
  let children = items
    .iter()
    .enumerate()
    .map(|(idx, item)| DiffNode::new(format!("[{idx}]"), status).with_detail(render_text(item)))
    .collect::<Vec<_>>();
  DiffNode::new(label, status).with_children(children)
}

fn build_file_tree(label: &str, value: &FileInSnapShot, status: DiffStatus) -> DiffNode {
  DiffNode::new(label, status).with_children(vec![
    build_ns_tree("ns", &value.ns, status),
    build_defs_tree("defs", &value.defs, status),
  ])
}

fn build_defs_tree(label: &str, defs: &HashMap<String, CodeEntry>, status: DiffStatus) -> DiffNode {
  let mut keys = defs.keys().cloned().collect::<Vec<_>>();
  keys.sort();
  let children = keys
    .into_iter()
    .map(|key| build_code_entry_tree(&key, defs.get(&key).expect("definition exists"), status))
    .collect::<Vec<_>>();
  DiffNode::new(label, status).with_children(children)
}

fn build_ns_tree(label: &str, value: &NsEntry, status: DiffStatus) -> DiffNode {
  DiffNode::new(label, status).with_children(vec![
    DiffNode::new("doc", status).with_detail(render_text(&value.doc)),
    build_cirru_tree("code", &value.code, status, "0"),
  ])
}

fn build_code_entry_tree(label: &str, value: &CodeEntry, status: DiffStatus) -> DiffNode {
  let examples = value
    .examples
    .iter()
    .enumerate()
    .map(|(idx, node)| build_cirru_tree(&format!("[{idx}]"), node, status, &idx.to_string()))
    .collect::<Vec<_>>();

  DiffNode::new(label, status).with_children(vec![
    DiffNode::new("doc", status).with_detail(render_text(&value.doc)),
    DiffNode::new("schema", status).with_detail(render_text(&value.schema.to_string())),
    DiffNode::new("examples", status).with_children(examples),
    build_cirru_tree("code", &value.code, status, "0"),
  ])
}

fn build_cirru_tree(label: &str, node: &Cirru, status: DiffStatus, coord: &str) -> DiffNode {
  match (status, node) {
    (DiffStatus::Added, _) => DiffNode::new(label, status)
      .with_detail(format_cirru_preview(node))
      .with_body(render_change_block("NEW", coord, node, 0)),
    (DiffStatus::Removed, _) => DiffNode::new(label, status)
      .with_detail(format_cirru_preview(node))
      .with_body(render_change_block("OLD", coord, node, 0)),
    (_, Cirru::Leaf(text)) => DiffNode::new(label, status).with_detail(format!(", {}", render_cirru_leaf_value(text))),
    (_, Cirru::List(_)) => DiffNode::new(label, status).with_detail(format_cirru_preview(node)),
  }
}

fn render_text(text: &str) -> String {
  let escaped = text.replace('\n', "⏎");
  let shortened = if escaped.chars().count() > 72 {
    let head = escaped.chars().take(69).collect::<String>();
    format!("{head}...")
  } else {
    escaped
  };
  format!("{shortened:?}")
}

fn render_cirru_leaf_value(text: &str) -> String {
  truncate_preview(&cirru_parser::generate_leaf(text), 72)
}

fn cirru_list_summary(items: &[Cirru]) -> String {
  let head = list_head(items).unwrap_or_else(|| "list".to_string());
  format!("list {head} ({} item(s))", items.len())
}

fn list_head(items: &[Cirru]) -> Option<String> {
  items.first().map(|node| match node.as_leaf_str() {
    Some(text) => render_cirru_leaf_value(text),
    None => "<list>".to_string(),
  })
}

fn format_cirru_preview(node: &Cirru) -> String {
  if let Some(text) = node.as_leaf_str() {
    format!(", {}", render_cirru_leaf_value(text))
  } else if let Cirru::List(items) = node {
    format_cirru_list_preview(items)
  } else {
    unreachable!()
  }
}

fn format_cirru_list_preview(items: &[Cirru]) -> String {
  cirru_parser::format_expr_one_liner(&Cirru::List(items.to_vec()))
    .map(|text| truncate_preview(&text, 96))
    .unwrap_or_else(|_| cirru_list_summary(items))
}

fn format_cirru_block(node: &Cirru) -> String {
  cirru_parser::format(std::slice::from_ref(node), cirru_parser::CirruWriterOptions { use_inline: false })
    .map(|text| text.trim().to_string())
    .unwrap_or_else(|_| format_cirru_preview(node))
}

fn render_cirru_diff(old: &Cirru, new: &Cirru, coord: &str, depth: usize, starts_expression: bool) -> String {
  if old == new {
    return render_context_node(new, depth, 3, starts_expression).join("\n");
  }

  match (old, new) {
    (Cirru::Leaf(_), Cirru::Leaf(_)) => {
      let mut lines = vec![];
      lines.extend(render_change_lines(
        "OLD",
        coord,
        &format_cirru_block_for_role(old, starts_expression),
        DiffStatus::Removed,
        depth,
      ));
      lines.extend(render_change_lines(
        "NEW",
        coord,
        &format_cirru_block_for_role(new, starts_expression),
        DiffStatus::Added,
        depth,
      ));
      lines.join("\n")
    }
    (Cirru::List(old_items), Cirru::List(new_items)) => {
      let edits = align_sequence(old_items, new_items);
      if should_show_as_whole_cirru_change(old_items, new_items, &edits) {
        let mut lines = vec![];
        lines.extend(render_change_lines(
          "OLD",
          coord,
          &format_cirru_block(old),
          DiffStatus::Removed,
          depth,
        ));
        lines.extend(render_change_lines(
          "NEW",
          coord,
          &format_cirru_block(new),
          DiffStatus::Added,
          depth,
        ));
        return lines.join("\n");
      }

      let mut lines = vec![];
      let mut cursor = 0usize;

      while cursor < edits.len() {
        match edits[cursor] {
          SeqEdit::Match(start_old, _) => {
            let mut run_len = 1usize;
            cursor += 1;
            while cursor < edits.len() {
              match edits[cursor] {
                SeqEdit::Match(next_old, _) if next_old == start_old + run_len => {
                  run_len += 1;
                  cursor += 1;
                }
                _ => break,
              }
            }

            let shown = run_len.min(6);
            lines.extend(render_context_children(
              &old_items[start_old..start_old + shown],
              start_old,
              depth + 1,
              2,
            ));
            if run_len > shown {
              lines.push(indent(depth + 1, &"...".dimmed().to_string()));
            }
          }
          SeqEdit::Replace(i, j) => {
            let old_child = &old_items[i];
            let new_child = &new_items[j];
            let child_starts_expression = child_starts_expression(i, old_child) || child_starts_expression(j, new_child);
            let child_depth = nested_child_depth(depth, child_starts_expression);
            lines.push(render_cirru_diff(
              old_child,
              new_child,
              &join_coord(coord, i),
              child_depth,
              child_starts_expression,
            ));
            cursor += 1;
          }
          SeqEdit::Remove(i) => {
            let child = &old_items[i];
            let child_starts_expression = child_starts_expression(i, child);
            lines.extend(render_change_lines(
              "OLD",
              &join_coord(coord, i),
              &format_cirru_block_for_role(child, child_starts_expression),
              DiffStatus::Removed,
              nested_child_depth(depth, child_starts_expression),
            ));
            cursor += 1;
          }
          SeqEdit::Insert(j) => {
            let child = &new_items[j];
            let child_starts_expression = child_starts_expression(j, child);
            lines.extend(render_change_lines(
              "NEW",
              &join_coord(coord, j),
              &format_cirru_block_for_role(child, child_starts_expression),
              DiffStatus::Added,
              nested_child_depth(depth, child_starts_expression),
            ));
            cursor += 1;
          }
        }
      }

      lines.join("\n")
    }
    _ => {
      let mut lines = vec![];
      lines.extend(render_change_lines(
        "OLD",
        coord,
        &format_cirru_block_for_role(old, starts_expression),
        DiffStatus::Removed,
        depth,
      ));
      lines.extend(render_change_lines(
        "NEW",
        coord,
        &format_cirru_block_for_role(new, starts_expression),
        DiffStatus::Added,
        depth,
      ));
      lines.join("\n")
    }
  }
}

fn render_change_block(kind: &str, coord: &str, node: &Cirru, depth: usize) -> String {
  render_change_lines(
    kind,
    coord,
    &format_cirru_block(node),
    if kind == "OLD" { DiffStatus::Removed } else { DiffStatus::Added },
    depth,
  )
  .join("\n")
}

fn render_change_lines(kind: &str, coord: &str, body: &str, status: DiffStatus, depth: usize) -> Vec<String> {
  let mut lines = vec![indent(depth, &status.paint(format!("{kind}@{coord}")))];
  for line in body.lines() {
    lines.push(indent(depth + 1, &status.paint(line.to_string())));
  }
  lines
}

fn render_context_node(node: &Cirru, depth: usize, expand_depth: usize, starts_expression: bool) -> Vec<String> {
  if let Some(text) = node.as_leaf_str() {
    return vec![indent(depth, &format_cirru_leaf(text, starts_expression).dimmed().to_string())];
  }
  let Cirru::List(items) = node else { unreachable!() };
  if expand_depth == 0 || items.is_empty() {
    return vec![indent(depth, &format_cirru_list_preview(items).dimmed().to_string())];
  }
  let max_children = if depth <= 1 { 8 } else { 4 };
  let shown = items.len().min(max_children);
  let mut lines = render_context_children(&items[..shown], 0, depth + 1, expand_depth - 1);
  if items.len() > shown {
    lines.push(indent(depth + 1, &"...".dimmed().to_string()));
  }
  lines
}

fn render_context_children(children: &[Cirru], start_index: usize, depth: usize, expand_depth: usize) -> Vec<String> {
  let mut lines = vec![];
  let mut cursor = 0usize;

  while cursor < children.len() {
    let child_index = start_index + cursor;
    let child = &children[cursor];

    if let Some(text) = child.as_leaf_str() {
      if !child_starts_expression(child_index, child) {
        let mut parts = vec![render_cirru_leaf_value(text)];
        cursor += 1;

        while cursor < children.len() {
          let sibling_index = start_index + cursor;
          let sibling = &children[cursor];
          if !child_starts_expression(sibling_index, sibling) {
            if let Some(sib_text) = sibling.as_leaf_str() {
              parts.push(render_cirru_leaf_value(sib_text));
              cursor += 1;
              continue;
            }
          }
          break;
        }

        lines.push(indent(depth + 1, &format!(", {}", parts.join(" ")).dimmed().to_string()));
        continue;
      }
    }

    lines.extend(render_context_node(
      child,
      depth,
      expand_depth,
      child_starts_expression(child_index, child),
    ));
    cursor += 1;
  }

  lines
}

fn child_starts_expression(index: usize, node: &Cirru) -> bool {
  index == 0 || node.is_list()
}

fn nested_child_depth(parent_depth: usize, starts_expression: bool) -> usize {
  parent_depth + if starts_expression { 1 } else { 2 }
}

fn format_cirru_leaf(text: &str, starts_expression: bool) -> String {
  let rendered = render_cirru_leaf_value(text);
  if starts_expression { rendered } else { format!(", {rendered}") }
}

fn format_cirru_block_for_role(node: &Cirru, starts_expression: bool) -> String {
  match node.as_leaf_str() {
    Some(text) => format_cirru_leaf(text, starts_expression),
    None => format_cirru_block(node),
  }
}

fn join_coord(parent: &str, index: usize) -> String {
  if parent.is_empty() {
    index.to_string()
  } else {
    format!("{parent}.{index}")
  }
}

fn indent(depth: usize, line: &str) -> String {
  format!("{}{}", "  ".repeat(depth), line)
}

fn truncate_preview(text: &str, max_chars: usize) -> String {
  if text.chars().count() <= max_chars {
    text.to_string()
  } else {
    let head = text.chars().take(max_chars.saturating_sub(3)).collect::<String>();
    format!("{head}...")
  }
}

fn aggregate_status(children: &[DiffNode]) -> DiffStatus {
  if children.iter().all(|child| child.status == DiffStatus::Unchanged) {
    DiffStatus::Unchanged
  } else {
    DiffStatus::Modified
  }
}

fn should_show_as_whole_cirru_change(old_items: &[Cirru], new_items: &[Cirru], edits: &[SeqEdit]) -> bool {
  let changed_edits = edits
    .iter()
    .filter(|edit| !matches!(edit, SeqEdit::Match(_, _)))
    .collect::<Vec<_>>();
  let changed_children = changed_edits.len();
  let total_children = old_items.len().max(new_items.len());
  if !(changed_children > 3 && changed_children * 2 > total_children) {
    return false;
  }

  let similarity_sum = changed_edits
    .iter()
    .map(|edit| match edit {
      SeqEdit::Replace(i, j) => cirru_similarity(&old_items[*i], &new_items[*j]),
      SeqEdit::Remove(_) | SeqEdit::Insert(_) => 0.0,
      SeqEdit::Match(_, _) => 1.0,
    })
    .sum::<f64>();
  let avg_similarity = similarity_sum / changed_children as f64;
  avg_similarity < 0.45
}

fn cirru_similarity(old: &Cirru, new: &Cirru) -> f64 {
  if old == new {
    return 1.0;
  }

  match (old, new) {
    (Cirru::Leaf(a), Cirru::Leaf(b)) => {
      if a == b {
        1.0
      } else if a.starts_with('|') && b.starts_with('|') {
        0.6
      } else if (a.starts_with('"') && b.starts_with('|')) || (a.starts_with('|') && b.starts_with('"')) {
        0.55
      } else {
        0.0
      }
    }
    (Cirru::List(a), Cirru::List(b)) => {
      let same_head = match (a.first(), b.first()) {
        (Some(Cirru::Leaf(x)), Some(Cirru::Leaf(y))) if x == y => 0.35,
        (Some(Cirru::List(_)), Some(Cirru::List(_))) => 0.2,
        _ => 0.0,
      };
      let len_score = 1.0 - ((a.len() as isize - b.len() as isize).unsigned_abs() as f64 / a.len().max(b.len()).max(1) as f64);
      let pair_count = a.len().min(b.len()).min(6);
      let child_score = if pair_count == 0 {
        0.0
      } else {
        (0..pair_count).map(|idx| cirru_similarity(&a[idx], &b[idx])).sum::<f64>() / pair_count as f64
      };
      (same_head + 0.2 * len_score + 0.45 * child_score).min(1.0)
    }
    _ => 0.0,
  }
}

pub(crate) fn collect_stats(root: &DiffNode) -> ProgramDiffStats {
  fn walk(node: &DiffNode, stats: &mut ProgramDiffStats, include_self: bool) {
    if include_self {
      match node.status {
        DiffStatus::Unchanged => stats.unchanged += 1,
        DiffStatus::Added => stats.added += 1,
        DiffStatus::Removed => stats.removed += 1,
        DiffStatus::Modified => stats.modified += 1,
      }
    }
    for child in &node.children {
      walk(child, stats, true);
    }
  }

  let mut stats = ProgramDiffStats::default();
  walk(root, &mut stats, false);
  stats
}

fn descendant_count(node: &DiffNode) -> usize {
  node.children.iter().map(|child| 1 + descendant_count(child)).sum()
}

pub(crate) fn format_tree_node(node: &DiffNode, output: &mut String, prefix: &str, is_last: bool, expand: bool, is_root: bool) {
  let is_old_new_block = node.label.starts_with("OLD@") || node.label.starts_with("NEW@");
  let connector = if is_root || is_old_new_block {
    ""
  } else if is_last {
    "└── "
  } else {
    "├── "
  };
  let mut line = if is_root {
    node.label.bold().to_string()
  } else {
    format!("{} {}", node.status.badge(), node.label)
  };

  if let Some(detail) = &node.detail {
    line.push(' ');
    line.push_str(detail);
  }

  if !expand && !node.children.is_empty() {
    line.push_str(&format!(" {}", format!("({} folded)", descendant_count(node)).dimmed()));
  }

  let rendered_line = if is_root { line } else { node.status.paint(line) };
  let line_prefix = if is_old_new_block {
    prefix.replace('│', " ")
  } else {
    prefix.to_string()
  };
  output.push_str(&format!("{line_prefix}{connector}{rendered_line}\n"));

  let child_prefix = if is_root {
    String::new()
  } else if is_old_new_block {
    format!("{}    ", prefix.replace('│', " "))
  } else {
    format!("{}{}   ", prefix, if is_last { " " } else { "│" })
  };

  if let Some(body) = &node.body {
    for line in body.lines() {
      output.push_str(&format!("{child_prefix}{line}\n"));
    }
  }

  if !expand {
    return;
  }

  for (idx, child) in node.children.iter().enumerate() {
    let child_expand = child.status != DiffStatus::Unchanged;
    format_tree_node(child, output, &child_prefix, idx + 1 == node.children.len(), child_expand, false);
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SeqEdit {
  Match(usize, usize),
  Replace(usize, usize),
  Remove(usize),
  Insert(usize),
}

fn align_sequence<T: Eq>(old: &[T], new: &[T]) -> Vec<SeqEdit> {
  let mut prefix = 0usize;
  while prefix < old.len() && prefix < new.len() && old[prefix] == new[prefix] {
    prefix += 1;
  }

  let mut suffix = 0usize;
  while suffix < old.len().saturating_sub(prefix)
    && suffix < new.len().saturating_sub(prefix)
    && old[old.len() - 1 - suffix] == new[new.len() - 1 - suffix]
  {
    suffix += 1;
  }

  let old_mid_end = old.len() - suffix;
  let new_mid_end = new.len() - suffix;
  let old_mid = &old[prefix..old_mid_end];
  let new_mid = &new[prefix..new_mid_end];

  let mut edits = Vec::new();
  for i in 0..prefix {
    edits.push(SeqEdit::Match(i, i));
  }

  let mut dp = vec![vec![0usize; new_mid.len() + 1]; old_mid.len() + 1];

  for (i, row) in dp.iter_mut().enumerate().take(old_mid.len() + 1) {
    row[0] = i;
  }
  for j in 0..=new_mid.len() {
    dp[0][j] = j;
  }

  for i in 1..=old_mid.len() {
    for j in 1..=new_mid.len() {
      let replace_cost = if old_mid[i - 1] == new_mid[j - 1] { 0 } else { 2 };
      let replace = dp[i - 1][j - 1] + replace_cost;
      let remove = dp[i - 1][j] + 1;
      let insert = dp[i][j - 1] + 1;
      dp[i][j] = replace.min(remove.min(insert));
    }
  }

  let mut middle = vec![];
  let mut i = old_mid.len();
  let mut j = new_mid.len();

  while i > 0 || j > 0 {
    if i > 0 && j > 0 {
      let replace_cost = if old_mid[i - 1] == new_mid[j - 1] { 0 } else { 2 };
      if dp[i][j] == dp[i - 1][j - 1] + replace_cost {
        middle.push(if replace_cost == 0 {
          SeqEdit::Match(prefix + i - 1, prefix + j - 1)
        } else {
          SeqEdit::Replace(prefix + i - 1, prefix + j - 1)
        });
        i -= 1;
        j -= 1;
        continue;
      }
    }

    if i > 0 && dp[i][j] == dp[i - 1][j] + 1 {
      middle.push(SeqEdit::Remove(prefix + i - 1));
      i -= 1;
      continue;
    }

    if j > 0 {
      middle.push(SeqEdit::Insert(prefix + j - 1));
      j -= 1;
    }
  }

  middle.reverse();
  edits.extend(middle);

  for offset in 0..suffix {
    let old_idx = old_mid_end + offset;
    let new_idx = new_mid_end + offset;
    edits.push(SeqEdit::Match(old_idx, new_idx));
  }

  edits
}

#[cfg(test)]
mod tests {
  use super::{
    CirruEditStrategy, DiffStatus, SeqEdit, align_sequence, analyze_cirru_edit_advice, cirru_similarity, diff_cirru, render_cirru_diff,
    render_text,
  };
  use cirru_parser::Cirru;

  fn leaf(text: &str) -> Cirru {
    Cirru::Leaf(text.into())
  }

  fn list(items: Vec<Cirru>) -> Cirru {
    Cirru::List(items)
  }

  #[test]
  fn aligns_insert_without_turning_everything_into_replace() {
    let old = vec![1, 2, 3];
    let new = vec![1, 4, 2, 3];
    let edits = align_sequence(&old, &new);
    assert_eq!(edits.len(), 4);
  }

  #[test]
  fn preserves_prefix_and_suffix_in_long_lists() {
    let old = vec![1, 2, 3, 4, 5, 6, 7, 8];
    let new = vec![1, 2, 3, 40, 50, 6, 7, 8];
    let edits = align_sequence(&old, &new);
    let matched = edits.iter().filter(|edit| matches!(edit, SeqEdit::Match(_, _))).count();
    assert!(matched >= 6);
  }

  #[test]
  fn keeps_structurally_similar_lists_out_of_whole_change_mode() {
    let old = list(vec![
      leaf("{}"),
      list(vec![leaf(":title"), leaf("\"A")]),
      list(vec![leaf(":url"), leaf("\"/a")]),
    ]);
    let new = list(vec![
      leaf("{}"),
      list(vec![leaf(":title"), leaf("|A")]),
      list(vec![leaf(":url"), leaf("|/a")]),
    ]);
    assert!(cirru_similarity(&old, &new) > 0.45);
  }

  #[test]
  fn diffs_leaf_change() {
    colored::control::set_override(false);
    let diff = diff_cirru("code", Some(&leaf("a")), Some(&leaf("b")), "0");
    assert_eq!(diff.status, DiffStatus::Modified);
    assert!(diff.body.is_some());
  }

  #[test]
  fn diffs_list_change_recursively() {
    colored::control::set_override(false);
    let old = list(vec![leaf("defn"), leaf("foo"), leaf("x")]);
    let new = list(vec![leaf("defn"), leaf("foo"), leaf("y")]);
    let diff = diff_cirru("code", Some(&old), Some(&new), "0");
    assert_eq!(diff.status, DiffStatus::Modified);
    assert!(diff.body.is_some());
  }

  #[test]
  fn classifies_additive_similar_edits_as_insert_strategy() {
    let old = list(vec![leaf("defn"), leaf("demo"), leaf("x")]);
    let new = list(vec![leaf("defn"), leaf("demo"), leaf("x"), leaf("y")]);
    let advice = analyze_cirru_edit_advice(&old, &new).expect("expected advice for similar edit");
    assert_eq!(advice.strategy, CirruEditStrategy::Insert);
  }

  #[test]
  fn classifies_leaf_updates_as_replace_strategy() {
    let old = list(vec![leaf("defn"), leaf("demo"), leaf("x")]);
    let new = list(vec![leaf("defn"), leaf("demo"), leaf("y")]);
    let advice = analyze_cirru_edit_advice(&old, &new).expect("expected advice for similar edit");
    assert_eq!(advice.strategy, CirruEditStrategy::Replace);
  }

  #[test]
  fn reports_identical_trees() {
    let old = list(vec![leaf("defn"), leaf("demo"), leaf("x")]);
    let advice = analyze_cirru_edit_advice(&old, &old).expect("expected advice for identical trees");
    assert_eq!(advice.strategy, CirruEditStrategy::Identical);
  }

  #[test]
  fn classifies_large_mixed_add_remove_as_rewrite_strategy() {
    let old = list(vec![
      leaf("defn"),
      leaf("main!"),
      list(vec![]),
      list(vec![leaf("println"), leaf("|a")]),
      list(vec![leaf("println"), leaf("|b")]),
      list(vec![leaf("println"), leaf("|c")]),
      list(vec![leaf("println"), leaf("|d")]),
      list(vec![leaf("println"), leaf("|e")]),
      list(vec![leaf("println"), leaf("|f")]),
      list(vec![leaf("do"), leaf("true")]),
    ]);
    let new = list(vec![
      leaf("defn"),
      leaf("main!"),
      list(vec![]),
      list(vec![leaf("println"), leaf("|a")]),
      list(vec![leaf("println"), leaf("|extra-1")]),
      list(vec![leaf("println"), leaf("|extra-2")]),
      list(vec![leaf("println"), leaf("|extra-3")]),
      list(vec![leaf("println"), leaf("|extra-4")]),
      list(vec![leaf("println"), leaf("|extra-5")]),
      list(vec![leaf("println"), leaf("|extra-6")]),
      list(vec![leaf("do"), leaf("true")]),
    ]);
    let advice = analyze_cirru_edit_advice(&old, &new).expect("expected advice for mixed structural edit");
    assert_eq!(advice.strategy, CirruEditStrategy::Rewrite);
  }

  #[test]
  fn truncates_rendered_text() {
    colored::control::set_override(false);
    let rendered = render_text(&"x".repeat(100));
    assert!(rendered.contains("..."));
  }

  #[test]
  fn omits_comma_for_expression_head_leaf_in_list_diff() {
    colored::control::set_override(false);
    let old = list(vec![
      leaf("def"),
      leaf("entries"),
      list(vec![leaf("{}"), leaf(":sites"), leaf("nil")]),
    ]);
    let new = list(vec![
      leaf("def"),
      leaf("entries"),
      list(vec![leaf("{}"), leaf(":sites"), leaf("[]")]),
    ]);
    let rendered = render_cirru_diff(&old, &new, "0", 0, true);
    assert!(rendered.contains("  def\n    , entries\n"), "unexpected render:\n{rendered}");
    assert!(rendered.contains("    {}\n      , :sites\n"), "unexpected render:\n{rendered}");
    assert!(rendered.contains("NEW@0.2.2\n        , []"), "unexpected render:\n{rendered}");
  }

  #[test]
  fn keeps_comma_for_non_head_leaf_change() {
    colored::control::set_override(false);
    let old = list(vec![leaf("def"), leaf("a")]);
    let new = list(vec![leaf("def"), leaf("b")]);
    let rendered = render_cirru_diff(&old, &new, "0", 0, true);
    assert!(rendered.contains("OLD@0.1\n      , a"), "unexpected render:\n{rendered}");
    assert!(rendered.contains("NEW@0.1\n      , b"), "unexpected render:\n{rendered}");
    assert!(rendered.starts_with("  def\n"), "unexpected render:\n{rendered}");
  }

  #[test]
  fn merges_consecutive_comma_siblings_into_one_line() {
    colored::control::set_override(false);
    let node = list(vec![
      leaf("str-spaced"),
      leaf("css/global"),
      leaf("css/fullscreen"),
      leaf("css/column"),
    ]);
    let rendered = render_cirru_diff(&node, &node, "0", 0, true);
    assert!(
      rendered.contains("  str-spaced\n    , css/global css/fullscreen css/column"),
      "unexpected render:\n{rendered}"
    );
  }

  #[test]
  fn uses_cirru_leaf_quoting_rules() {
    assert_eq!(cirru_parser::generate_leaf("defn"), "defn");
    assert_eq!(cirru_parser::generate_leaf("hello world"), "\"hello world\"");
    assert_eq!(cirru_parser::generate_leaf("line\nbreak"), "\"line\\nbreak\"");
  }
}
