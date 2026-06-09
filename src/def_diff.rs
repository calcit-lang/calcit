use crate::program_diff::{
  DiffNode, ProgramDiffStats, collect_stats, diff_code_entry, format_tree_node, git_root, git_show_file, parse_snapshot,
  repo_relative_path, resolve_input_path,
};
use crate::snapshot::{CodeEntry, Snapshot};
use crate::util::string::extract_ns_def;
use std::env;
use std::fs;

#[derive(Debug, Clone)]
pub struct DefDiffResult {
  pub target: String,
  pub git_ref: String,
  pub file_path: String,
  pub root: DiffNode,
  pub stats: ProgramDiffStats,
}

pub fn analyze_def_diff(target: &str, git_ref: &str, input_path: &str) -> Result<DefDiffResult, String> {
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

  let (ns, def) = extract_ns_def(target)?;
  let old_entry = lookup_code_entry(&historical_snapshot, &ns, &def);
  let new_entry = lookup_code_entry(&current_snapshot, &ns, &def);

  if old_entry.is_none() && new_entry.is_none() {
    return Err(format!("Definition not found in current file or ref '{git_ref}': {target}"));
  }

  let mut root = diff_code_entry(&def, old_entry, new_entry);
  root.label = target.to_string();
  let stats = collect_stats(&root);

  Ok(DefDiffResult {
    target: target.to_string(),
    git_ref: git_ref.to_string(),
    file_path: repo_rel_path.to_string_lossy().to_string(),
    root,
    stats,
  })
}

pub fn format_def_diff(result: &DefDiffResult) -> String {
  let mut output = String::new();
  output.push_str("# Definition Diff\n\n");
  output.push_str(&format!("- target: {}\n", result.target));
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

fn lookup_code_entry<'a>(snapshot: &'a Snapshot, ns: &str, def: &str) -> Option<&'a CodeEntry> {
  snapshot.files.get(ns).and_then(|file| file.defs.get(def))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::CalcitTypeAnnotation;
  use crate::program_diff::DiffStatus;
  use crate::snapshot::{CodeEntry, FileInSnapShot, NsEntry};
  use cirru_parser::Cirru;
  use std::collections::HashMap;
  use std::sync::Arc;

  fn code_entry(doc: &str, schema: CalcitTypeAnnotation, code: &str) -> CodeEntry {
    CodeEntry {
      doc: doc.to_string(),
      examples: vec![],
      tags: std::collections::HashSet::new(),
      code: Cirru::Leaf(Arc::from(code)),
      schema: Arc::new(schema),
    }
  }

  fn snapshot_with_def(ns: &str, def: &str, entry: CodeEntry) -> Snapshot {
    let mut snapshot = Snapshot::default();
    let mut defs = HashMap::new();
    defs.insert(def.to_string(), entry);
    snapshot.files.insert(
      ns.to_string(),
      FileInSnapShot {
        ns: NsEntry {
          doc: String::new(),
          code: Cirru::List(vec![]),
        },
        defs,
      },
    );
    snapshot
  }

  #[test]
  fn looks_up_definition_from_snapshot() {
    let snapshot = snapshot_with_def("app.main", "foo", code_entry("doc", CalcitTypeAnnotation::Dynamic, "a"));
    assert!(lookup_code_entry(&snapshot, "app.main", "foo").is_some());
    assert!(lookup_code_entry(&snapshot, "app.main", "bar").is_none());
  }

  #[test]
  fn diff_root_contains_doc_schema_and_code_changes() {
    let old = code_entry("old doc", CalcitTypeAnnotation::Dynamic, "a");
    let new = code_entry("new doc", CalcitTypeAnnotation::Number, "b");
    let root = diff_code_entry("foo", Some(&old), Some(&new));
    assert_eq!(root.status, DiffStatus::Modified);
    assert!(
      root
        .children
        .iter()
        .any(|child| child.label == "doc" && child.status == DiffStatus::Modified)
    );
    assert!(
      root
        .children
        .iter()
        .any(|child| child.label == "schema" && child.status == DiffStatus::Modified)
    );
    assert!(
      root
        .children
        .iter()
        .any(|child| child.label == "code" && child.status == DiffStatus::Modified)
    );
  }
}
