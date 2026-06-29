//! Calcit CLI builtin functions accessible from Calcit code via `cr exec`.
//!
//! These functions read a `.cirru` snapshot file from disk, parse it,
//! and return results as Calcit values — bypassing shell argument escaping entirely.
//!
//! Usage from Calcit code:
//! ```cirru.cli
//! calcit.cli/list-ns $ {} (:file-path |calcit.cirru)
//! calcit.cli/list-defs $ {} (:file-path |calcit.cirru) (:namespace |app.core)
//! calcit.cli/show-def $ {} (:file-path |calcit.cirru) (:target |app.core/main)
//! calcit.cli/peek-def $ {} (:file-path |calcit.cirru) (:target |app.core/main) (:lines 5)
//! calcit.cli/find-symbol $ {} (:file-path |calcit.cirru) (:symbol |main!)
//! ```

use calcit::calcit::{Calcit, CalcitErr, CalcitList, CalcitTypeAnnotation, DYNAMIC_TYPE};
use calcit::call_stack::CallStackList;
use calcit::snapshot::{self, CodeEntry, FileInSnapShot, NsEntry, validate_schema_for_write};
use calcit::util::string::strip_shebang;
use cirru_parser::Cirru;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use super::calcit_cli_args::resolve_cli_args;
use super::calcit_cli_specs::{
  ADD_EXAMPLE, ADD_IMPORT, ADD_NS, EDIT_DEF, EDIT_DOC, EDIT_SCHEMA, FIND_SYMBOL, LIST_CONFIG, LIST_DEFS, LIST_EXAMPLES, LIST_MODULES,
  LIST_NS, LIST_USAGES, MV_DEF, PEEK_DEF, RENAME_DEF, RM_DEF, RM_IMPORT, SEARCH_DEF, SEARCH_REPLACE, SHOW_DEF, SHOW_ERROR, SHOW_SCHEMA,
  SPLIT_DEF, TREE_CP, TREE_DELETE, TREE_INSERT, TREE_MV, TREE_RAISE, TREE_REPLACE, TREE_SHOW, TREE_UNWRAP, TREE_WRAP,
};
use super::calcit_cli_tree::{
  apply_operation_at_path, compute_adjusted_from_path, map_at_to_operation, process_node_with_references, splice_at_path,
  to_path_is_inside_from,
};

// ─── Query functions ─────────────────────────────────────────────────────────

/// `(calcit.cli/list-ns $ {} (:file-path <path>))` → list namespace names
pub fn list_namespaces(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/list-ns", &xs, LIST_NS)?;
  let file_path = args.string("file-path")?;
  let snapshot = load_calcit_snapshot(&file_path)?;
  let mut ns_names: Vec<Calcit> = snapshot.files.keys().map(|k| Calcit::Str(Arc::from(k.as_str()))).collect();
  ns_names.sort();
  Ok(Calcit::List(Arc::new(CalcitList::from(ns_names.as_slice()))))
}

/// `(calcit.cli/list-defs $ {} (:file-path <path>) (:namespace <ns>))` → list definition names in a namespace
pub fn list_defs(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/list-defs", &xs, LIST_DEFS)?;
  let file_path = args.string("file-path")?;
  let ns_name = args.string("namespace")?;
  let snapshot = load_calcit_snapshot(&file_path)?;
  let file = get_file(&snapshot, &ns_name)?;
  let mut def_names: Vec<Calcit> = file.defs.keys().map(|k| Calcit::Str(Arc::from(k.as_str()))).collect();
  def_names.sort();
  Ok(Calcit::List(Arc::new(CalcitList::from(def_names.as_slice()))))
}

/// `(calcit.cli/show-def $ {} (:file-path <path>) (:target <ns/def>))` → return full Cirru code of a definition
pub fn show_def(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/show-def", &xs, SHOW_DEF)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let snapshot = load_calcit_snapshot(&file_path)?;
  let entry = get_def(&snapshot, &ns_name, &def_name)?;
  let code_str = format_cirru("show-def", &entry.code)?;
  Ok(Calcit::Str(Arc::from(code_str)))
}

/// `(calcit.cli/peek-def $ {} (:file-path <path>) (:target <ns/def>) (:lines 5?))` → first N lines of a definition
pub fn peek_def(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/peek-def", &xs, PEEK_DEF)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let max_lines = args.usize("lines")?;
  if max_lines == 0 {
    return Err(CalcitErr::from("peek-def: lines must be greater than 0".to_string()));
  }
  let snapshot = load_calcit_snapshot(&file_path)?;
  let entry = get_def(&snapshot, &ns_name, &def_name)?;
  let code_str = format_cirru("peek-def", &entry.code)?;
  let lines: Vec<&str> = code_str.lines().take(max_lines).collect();
  Ok(Calcit::Str(Arc::from(lines.join("\n"))))
}

/// `(calcit.cli/search-def $ {} (:file-path <path>) (:target <ns/def>) (:keyword <text>))` → find leaf paths matching keyword
pub fn search_def(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/search-def", &xs, SEARCH_DEF)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let keyword = args.string("keyword")?;
  if keyword.is_empty() {
    return Err(CalcitErr::from("search-def: keyword cannot be empty".to_string()));
  }
  let snapshot = load_calcit_snapshot(&file_path)?;
  let entry = get_def(&snapshot, &ns_name, &def_name)?;
  let mut matches = Vec::new();
  search_cirru_for_keyword(&entry.code, &keyword, &mut vec![], &mut matches);
  Ok(calcit_str_list(matches))
}

fn search_cirru_for_keyword(node: &Cirru, keyword: &str, path: &mut Vec<usize>, results: &mut Vec<String>) {
  match node {
    Cirru::Leaf(s) => {
      if s.as_ref().contains(keyword) {
        results.push(format!("{} {}", format_path(path), preview_leaf(s)));
      }
    }
    Cirru::List(items) => {
      for (i, child) in items.iter().enumerate() {
        path.push(i);
        search_cirru_for_keyword(child, keyword, path, results);
        path.pop();
      }
    }
  }
}

/// `(calcit.cli/find-symbol $ {} (:file-path <path>) (:symbol <name>))` → find symbol across all namespaces
pub fn find_symbol(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/find-symbol", &xs, FIND_SYMBOL)?;
  let file_path = args.string("file-path")?;
  let symbol = args.string("symbol")?;
  if symbol.is_empty() {
    return Err(CalcitErr::from("find-symbol: symbol cannot be empty".to_string()));
  }
  let snapshot = load_calcit_snapshot(&file_path)?;
  let mut results = Vec::new();
  for (ns_name, file) in &snapshot.files {
    for def_name in file.defs.keys() {
      if def_name.contains(&symbol) || ns_name.contains(&symbol) {
        results.push(format!("{ns_name}/{def_name}"));
      }
    }
  }
  results.sort();
  Ok(calcit_str_list(results))
}

/// `(calcit.cli/show-schema $ {} (:file-path <path>) (:target <ns/def>))` → return schema as Cirru string
pub fn show_schema(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/show-schema", &xs, SHOW_SCHEMA)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let snapshot = load_calcit_snapshot(&file_path)?;
  let entry = get_def(&snapshot, &ns_name, &def_name)?;
  let schema_edn = entry.schema.to_type_edn();
  let schema_str = cirru_edn::format(&schema_edn, false).map_err(|e| CalcitErr::from(format!("Failed to format schema: {e}")))?;
  Ok(Calcit::Str(Arc::from(schema_str)))
}

/// `(calcit.cli/list-examples $ {} (:file-path <path>) (:target <ns/def>))` → list examples of a definition
pub fn list_examples(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/list-examples", &xs, LIST_EXAMPLES)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let snapshot = load_calcit_snapshot(&file_path)?;
  let entry = get_def(&snapshot, &ns_name, &def_name)?;
  let mut examples = Vec::new();
  for (i, ex) in entry.examples.iter().enumerate() {
    let ex_str = format_cirru("list-examples", ex)?;
    examples.push(format!("{i}: {ex_str}"));
  }
  Ok(calcit_str_list(examples))
}

/// `(calcit.cli/list-usages $ {} (:file-path <path>) (:target <ns/def>))` → find all references to a definition across the snapshot
pub fn list_usages(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/list-usages", &xs, LIST_USAGES)?;
  let file_path = args.string("file-path")?;
  let (target_ns, def_name) = args.target("target")?;
  let snapshot = load_calcit_snapshot(&file_path)?;
  get_def(&snapshot, &target_ns, &def_name)?;
  let mut results = Vec::new();
  for (ns_name, file) in &snapshot.files {
    for (d_name, entry) in &file.defs {
      if ns_name == &target_ns && d_name == &def_name {
        continue;
      }
      let mut paths = Vec::new();
      search_cirru_for_exact_leaf(&entry.code, &def_name, &mut vec![], &mut paths);
      for path in paths {
        results.push(format!("{ns_name}/{d_name} @{}", format_path(&path)));
      }
    }
  }
  results.sort();
  Ok(calcit_str_list(results))
}

// ─── Write functions (edit / tree) ──────────────────────────────────────────

/// `(calcit.cli/edit-def $ {} (:file-path <path>) (:target <ns/def>) (:code <str>) (:overwrite false?))` → create or update a definition
pub fn edit_def(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/edit-def", &xs, EDIT_DEF)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let code_str = args.string("code")?;
  let overwrite = args.bool("overwrite")?;

  let syntax_tree = parse_single_cirru("edit-def", &code_str)?;

  let mut snapshot = load_calcit_snapshot(&file_path)?;

  let file_data = get_file_mut(&mut snapshot, &ns_name)?;

  let exists = file_data.defs.contains_key(&def_name);
  if exists && !overwrite {
    return Err(CalcitErr::from(format!(
      "Definition '{def_name}' already exists. Use (:overwrite true) to overwrite."
    )));
  }

  let entry = if exists {
    let mut e = file_data.defs.remove(&def_name).unwrap();
    e.code = syntax_tree;
    e
  } else {
    CodeEntry::from_code(syntax_tree)
  };
  file_data.defs.insert(def_name, entry);

  save_calcit_snapshot(&file_path, &snapshot)?;

  Ok(Calcit::Str(Arc::from(if exists { "updated" } else { "created" })))
}

/// `(calcit.cli/tree-replace $ {} (:file-path <path>) (:target <ns/def>) (:path <path>) (:code <str>))` → replace node at path
pub fn tree_replace(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/tree-replace", &xs, TREE_REPLACE)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let path_str = args.string("path")?;
  let code_str = args.string("code")?;

  let indices = parse_path("tree-replace", &path_str, false)?;

  let replacement = parse_single_cirru("tree-replace", &code_str)?;

  let mut snapshot = load_calcit_snapshot(&file_path)?;

  let file_data = get_file_mut(&mut snapshot, &ns_name)?;

  let entry = file_data
    .defs
    .get_mut(&def_name)
    .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?;

  entry.code = apply_operation_at_path(&entry.code, &indices, "replace", Some(&replacement))?;

  save_calcit_snapshot(&file_path, &snapshot)?;

  Ok(Calcit::Str(Arc::from("replaced")))
}

/// `(calcit.cli/tree-delete $ {} (:file-path <path>) (:target <ns/def>) (:path <path>))` → delete AST node
pub fn tree_delete(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/tree-delete", &xs, TREE_DELETE)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let path_str = args.string("path")?;
  let indices = parse_path("tree-delete", &path_str, false)?;

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  let entry = file_data
    .defs
    .get_mut(&def_name)
    .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?;
  entry.code = apply_operation_at_path(&entry.code, &indices, "delete", None)?;
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("deleted")))
}

/// `(calcit.cli/tree-insert $ {} (:file-path <path>) (:target <ns/def>) (:path <path>) (:code <str>) (:position after?))` → insert node
pub fn tree_insert(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/tree-insert", &xs, TREE_INSERT)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let path_str = args.string("path")?;
  let code_str = args.string("code")?;
  let position = args.string("position")?;
  let operation = map_at_to_operation(position.trim_start_matches('|'))?;
  let indices = parse_path("tree-insert", &path_str, false)?;
  let new_node = parse_single_cirru("tree-insert", &code_str)?;

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  let entry = file_data
    .defs
    .get_mut(&def_name)
    .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?;
  entry.code = apply_operation_at_path(&entry.code, &indices, operation, Some(&new_node))?;
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("inserted")))
}

/// `(calcit.cli/tree-wrap $ {} (:file-path <path>) (:target <ns/def>) (:path <path>) (:wrapper-code <str>))` → wrap node; use `self` leaf for original
pub fn tree_wrap(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/tree-wrap", &xs, TREE_WRAP)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let path_str = args.string("path")?;
  let code_str = args.string("wrapper-code")?;
  let indices = parse_path("tree-wrap", &path_str, false)?;
  let template = parse_single_cirru("tree-wrap", &code_str)?;

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  let entry = file_data
    .defs
    .get_mut(&def_name)
    .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?;
  let original_node = navigate_to_node(&entry.code, &indices)?.clone();
  let mut references = BTreeMap::new();
  references.insert("self".to_string(), original_node);
  let wrapped = process_node_with_references(&template, &references)?;
  entry.code = apply_operation_at_path(&entry.code, &indices, "replace", Some(&wrapped))?;
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("wrapped")))
}

/// `(calcit.cli/tree-unwrap $ {} (:file-path <path>) (:target <ns/def>) (:path <path>))` → unwrap list node into parent
pub fn tree_unwrap(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/tree-unwrap", &xs, TREE_UNWRAP)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let path_str = args.string("path")?;
  let indices = parse_path("tree-unwrap", &path_str, false)?;
  if indices.is_empty() {
    return Err(CalcitErr::from("tree-unwrap: cannot unwrap root node".to_string()));
  }

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  let entry = file_data
    .defs
    .get_mut(&def_name)
    .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?;
  let node = navigate_to_node(&entry.code, &indices)?;
  match node {
    Cirru::List(children) if children.is_empty() => {
      return Err(CalcitErr::from("tree-unwrap: node has no children to splice".to_string()));
    }
    Cirru::Leaf(_) => return Err(CalcitErr::from("tree-unwrap: node at path is a leaf".to_string())),
    _ => {}
  }
  entry.code = splice_at_path(&entry.code, &indices)?;
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("unwrapped")))
}

/// `(calcit.cli/tree-raise $ {} (:file-path <path>) (:target <ns/def>) (:path <path>))` → replace parent with child at path
pub fn tree_raise(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/tree-raise", &xs, TREE_RAISE)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let path_str = args.string("path")?;
  let indices = parse_path("tree-raise", &path_str, false)?;
  if indices.is_empty() {
    return Err(CalcitErr::from("tree-raise: path must have at least one segment".to_string()));
  }
  let parent_path = &indices[..indices.len() - 1];

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  let entry = file_data
    .defs
    .get_mut(&def_name)
    .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?;
  let child_node = navigate_to_node(&entry.code, &indices)?.clone();
  entry.code = apply_operation_at_path(&entry.code, parent_path, "replace", Some(&child_node))?;
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("raised")))
}

/// `(calcit.cli/tree-cp $ {} (:file-path <path>) (:target <ns/def>) (:from-path <path>) (:to-path <path>) (:position after?))` → copy AST subtree
pub fn tree_cp(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/tree-cp", &xs, TREE_CP)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let from_str = args.string("from-path")?;
  let to_str = args.string("to-path")?;
  let position = args.string("position")?;
  let operation = map_at_to_operation(position.trim_start_matches('|'))?;
  let from_path = parse_path("tree-cp", &from_str, true)?;
  let to_path = parse_path("tree-cp", &to_str, false)?;

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  let entry = file_data
    .defs
    .get_mut(&def_name)
    .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?;
  let source_node = navigate_to_node(&entry.code, &from_path)?.clone();
  entry.code = apply_operation_at_path(&entry.code, &to_path, operation, Some(&source_node))?;
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("copied")))
}

/// `(calcit.cli/tree-mv $ {} (:file-path <path>) (:target <ns/def>) (:from-path <path>) (:to-path <path>) (:position after?))` → move AST subtree
pub fn tree_mv(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/tree-mv", &xs, TREE_MV)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let from_str = args.string("from-path")?;
  let to_str = args.string("to-path")?;
  let position = args.string("position")?;
  let operation = map_at_to_operation(position.trim_start_matches('|'))?;
  let from_path = parse_path("tree-mv", &from_str, false)?;
  let to_path = parse_path("tree-mv", &to_str, false)?;
  if from_path.is_empty() {
    return Err(CalcitErr::from("tree-mv: cannot move root node".to_string()));
  }
  if from_path == to_path {
    return Err(CalcitErr::from("tree-mv: source and destination paths are identical".to_string()));
  }
  if to_path_is_inside_from(&from_path, &to_path) {
    return Err(CalcitErr::from(format!(
      "tree-mv: cannot move node at `{from_str}` into its own subtree at `{to_str}`"
    )));
  }

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  let entry = file_data
    .defs
    .get_mut(&def_name)
    .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?;
  let source_node = navigate_to_node(&entry.code, &from_path)?.clone();
  let after_insert = apply_operation_at_path(&entry.code, &to_path, operation, Some(&source_node))?;
  let adjusted_from = compute_adjusted_from_path(&from_path, &to_path, operation);
  entry.code = apply_operation_at_path(&after_insert, &adjusted_from, "delete", None)?;
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("moved")))
}

/// `(calcit.cli/rename-def $ {} (:file-path <path>) (:target <ns/def>) (:new-name <name>))` → rename definition in namespace
pub fn rename_def(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/rename-def", &xs, RENAME_DEF)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let new_name = args.string("new-name")?;
  if new_name.is_empty() {
    return Err(CalcitErr::from("rename-def: new name cannot be empty".to_string()));
  }

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  if !file_data.defs.contains_key(&def_name) {
    return Err(CalcitErr::from(format!(
      "Definition '{def_name}' not found in namespace `{ns_name}`"
    )));
  }
  if file_data.defs.contains_key(&new_name) {
    return Err(CalcitErr::from(format!(
      "rename-def: definition '{new_name}' already exists in namespace `{ns_name}`"
    )));
  }
  let entry = file_data.defs.remove(&def_name).expect("checked exists");
  file_data.defs.insert(new_name, entry);
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("renamed")))
}

/// `(calcit.cli/rm-def $ {} (:file-path <path>) (:target <ns/def>))` → remove definition
pub fn rm_def(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/rm-def", &xs, RM_DEF)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  if file_data.defs.remove(&def_name).is_none() {
    return Err(CalcitErr::from(format!(
      "Definition '{def_name}' not found in namespace `{ns_name}`"
    )));
  }
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("removed")))
}

/// `(calcit.cli/mv-def $ {} (:file-path <path>) (:source <ns/def>) (:target <ns/def>))` → move definition across namespaces
pub fn mv_def(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/mv-def", &xs, MV_DEF)?;
  let file_path = args.string("file-path")?;
  let (source_ns, source_def) = args.target("source")?;
  let (target_ns, target_def) = args.target("target")?;

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &source_ns)?;
  check_ns_editable(&snapshot, &target_ns)?;

  if source_ns == target_ns && source_def == target_def {
    return Err(CalcitErr::from("mv-def: source and target are identical".to_string()));
  }

  if source_ns == target_ns {
    let file_data = get_file_mut(&mut snapshot, &source_ns)?;
    if !file_data.defs.contains_key(&source_def) {
      return Err(CalcitErr::from(format!(
        "Definition '{source_def}' not found in namespace `{source_ns}`"
      )));
    }
    if file_data.defs.contains_key(&target_def) {
      return Err(CalcitErr::from(format!(
        "mv-def: definition '{target_def}' already exists in namespace `{source_ns}`"
      )));
    }
    let entry = file_data.defs.remove(&source_def).expect("checked exists");
    file_data.defs.insert(target_def, entry);
  } else {
    let target_exists = get_file(&snapshot, &target_ns)?.defs.contains_key(&target_def);
    if target_exists {
      return Err(CalcitErr::from(format!(
        "mv-def: definition '{target_def}' already exists in namespace `{target_ns}`"
      )));
    }
    let entry = {
      let source_file = get_file_mut(&mut snapshot, &source_ns)?;
      source_file
        .defs
        .remove(&source_def)
        .ok_or_else(|| CalcitErr::from(format!("Definition '{source_def}' not found in namespace `{source_ns}`")))?
    };
    get_file_mut(&mut snapshot, &target_ns)?.defs.insert(target_def, entry);
  }

  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("moved")))
}

/// `(calcit.cli/split-def $ {} (:file-path <path>) (:target <ns/def>) (:path <path>) (:new-name <name>))` → extract sub-expression to new def
pub fn split_def(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/split-def", &xs, SPLIT_DEF)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let path_str = args.string("path")?;
  let new_name = args.string("new-name")?;
  if new_name.is_empty() {
    return Err(CalcitErr::from("split-def: new name cannot be empty".to_string()));
  }
  let path = parse_path("split-def", &path_str, false)?;
  if path.is_empty() {
    return Err(CalcitErr::from(
      "split-def: cannot split at root path; use edit-def to create a new definition".to_string(),
    ));
  }

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  if file_data.defs.contains_key(&new_name) {
    return Err(CalcitErr::from(format!(
      "split-def: definition '{new_name}' already exists in namespace `{ns_name}`"
    )));
  }
  let extracted = {
    let entry = file_data
      .defs
      .get(&def_name)
      .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?;
    navigate_to_node(&entry.code, &path)?.clone()
  };
  let updated_code = {
    let entry = file_data
      .defs
      .get(&def_name)
      .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?;
    let new_ref = Cirru::Leaf(Arc::from(new_name.as_str()));
    apply_operation_at_path(&entry.code, &path, "replace", Some(&new_ref))?
  };
  file_data.defs.get_mut(&def_name).expect("definition exists").code = updated_code;
  file_data.defs.insert(new_name.clone(), CodeEntry::from_code(extracted));
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("split")))
}

/// `(calcit.cli/add-ns $ {} (:file-path <path>) (:namespace <ns>) (:code <ns-expr>?))` → create namespace
pub fn add_ns(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/add-ns", &xs, ADD_NS)?;
  let file_path = args.string("file-path")?;
  let ns_name = args.string("namespace")?;
  let ns_code = if let Some(code_str) = args.optional_string("code") {
    let code = parse_single_cirru("add-ns", &code_str)?;
    if let Cirru::List(ref items) = code {
      if let Some(Cirru::Leaf(kw)) = items.first()
        && kw.as_ref() == "ns"
        && let Some(Cirru::Leaf(name_in_expr)) = items.get(1)
        && name_in_expr.as_ref() != ns_name
      {
        return Err(CalcitErr::from(format!(
          "add-ns: namespace name mismatch: expected `{ns_name}`, got `{name_in_expr}` in ns expression"
        )));
      }
    }
    code
  } else {
    Cirru::List(vec![Cirru::Leaf(Arc::from("ns")), Cirru::Leaf(Arc::from(ns_name.as_str()))])
  };

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  if snapshot.files.contains_key(&ns_name) {
    return Err(CalcitErr::from(format!("add-ns: namespace `{ns_name}` already exists")));
  }
  snapshot.files.insert(
    ns_name.clone(),
    FileInSnapShot {
      ns: NsEntry {
        doc: String::new(),
        code: ns_code,
      },
      defs: HashMap::new(),
    },
  );
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("created")))
}

/// `(calcit.cli/rm-import $ {} (:file-path <path>) (:namespace <ns>) (:source-ns <src>))` → remove require rule for source namespace
pub fn rm_import(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/rm-import", &xs, RM_IMPORT)?;
  let file_path = args.string("file-path")?;
  let ns_name = args.string("namespace")?;
  let src_ns = args.string("source-ns")?;

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  let mut rules = extract_require_rules(&file_data.ns.code);
  let original_len = rules.len();
  rules.retain(|r| get_require_source_ns(r).as_deref() != Some(src_ns.as_str()));
  if rules.len() == original_len {
    return Err(CalcitErr::from(format!(
      "rm-import: no require rule found for `{src_ns}` in namespace `{ns_name}`"
    )));
  }
  file_data.ns.code = build_ns_code(&ns_name, &rules);
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("removed")))
}

/// `(calcit.cli/edit-doc $ {} (:file-path <path>) (:target <ns/def>) (:doc <text>))` → update definition documentation
pub fn edit_doc(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/edit-doc", &xs, EDIT_DOC)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let doc = args.string("doc")?;

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  let entry = file_data
    .defs
    .get_mut(&def_name)
    .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?;
  entry.doc = doc;
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("updated")))
}

/// `(calcit.cli/edit-schema $ {} (:file-path <path>) (:target <ns/def>) (:schema-code <code>))` → update type schema
pub fn edit_schema(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/edit-schema", &xs, EDIT_SCHEMA)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let schema_str = args.string("schema-code")?;

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  let entry = file_data
    .defs
    .get_mut(&def_name)
    .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?;

  if schema_str.trim() == "nil" || schema_str.trim() == "|nil" {
    entry.schema = DYNAMIC_TYPE.clone();
    save_calcit_snapshot(&file_path, &snapshot)?;
    return Ok(Calcit::Str(Arc::from("cleared")));
  }

  let schema_node = parse_single_cirru("edit-schema", &schema_str)?;
  let schema_payload = strip_name_field_from_schema(unwrap_schema_quote_input(schema_node)?);
  validate_schema_for_write(&schema_payload).map_err(|e| CalcitErr::from(format!("edit-schema: schema validation failed: {e}")))?;

  if let Cirru::Leaf(tag) = &schema_payload {
    let tag_name = tag.trim_start_matches(':');
    entry.schema = Arc::new(CalcitTypeAnnotation::from_tag_name(tag_name));
  } else {
    snapshot::parse_schema_data(&schema_payload)?;
    let schema_edn = snapshot::schema_cirru_to_edn(schema_payload);
    entry.schema = CalcitTypeAnnotation::parse_fn_schema_from_edn(&schema_edn)
      .map(|s| Arc::new(CalcitTypeAnnotation::Fn(Arc::new(s))))
      .unwrap_or_else(|| DYNAMIC_TYPE.clone());
  }

  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("updated")))
}

/// `(calcit.cli/add-example $ {} (:file-path <path>) (:target <ns/def>) (:code <str>) (:index <n>?))` → append or insert example
pub fn add_example(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/add-example", &xs, ADD_EXAMPLE)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let code_str = args.string("code")?;
  let example = parse_single_cirru("add-example", &code_str)?;

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  let max_index = file_data
    .defs
    .get(&def_name)
    .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?
    .examples
    .len();
  let insert_at = if options_map_has_key(&xs, "index") {
    let pos = args.usize("index")?;
    if pos > max_index {
      return Err(CalcitErr::from(format!("add-example: index {pos} out of range (max: {max_index})")));
    }
    pos
  } else {
    max_index
  };
  let entry = file_data
    .defs
    .get_mut(&def_name)
    .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?;
  entry.examples.insert(insert_at, example);
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from(format!("added at {insert_at}"))))
}

/// `(calcit.cli/show-error $ {} (:error-file .calcit-error.cirru?))` → read last compile/runtime error stack
pub fn show_error(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/show-error", &xs, SHOW_ERROR)?;
  let error_file = args.string("error-file")?;
  let path = Path::new(&error_file);
  if !path.exists() {
    return Ok(Calcit::Str(Arc::from("no error file")));
  }
  let content = fs::read_to_string(path).map_err(|e| CalcitErr::from(format!("show-error: failed to read `{error_file}`: {e}")))?;
  if content.trim().is_empty() {
    Ok(Calcit::Str(Arc::from("no recent errors")))
  } else {
    Ok(Calcit::Str(Arc::from(content)))
  }
}

/// `(calcit.cli/search-replace $ {} (:file-path <path>) (:target <ns/def>) (:pattern <text>) (:replacement <text>))` → search and replace leaf nodes
pub fn search_replace(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/search-replace", &xs, SEARCH_REPLACE)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let pattern = args.string("pattern")?;
  let replacement_str = args.string("replacement")?;
  if pattern.is_empty() {
    return Err(CalcitErr::from("search-replace: pattern cannot be empty".to_string()));
  }

  let replacement = Cirru::Leaf(Arc::from(replacement_str.as_str()));

  let mut snapshot = load_calcit_snapshot(&file_path)?;

  let file_data = get_file_mut(&mut snapshot, &ns_name)?;

  let entry = file_data
    .defs
    .get_mut(&def_name)
    .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?;

  let mut count = 0u32;
  replace_leaf_nodes(&mut entry.code, &pattern, &replacement, &mut count);

  if count == 0 {
    let target = format!("{ns_name}/{def_name}");
    return Err(CalcitErr::from(format!("Pattern '{pattern}' not found in '{target}'")));
  }

  save_calcit_snapshot(&file_path, &snapshot)?;

  Ok(Calcit::Str(Arc::from(format!("replaced {count} occurrence(s)"))))
}

fn replace_leaf_nodes(node: &mut Cirru, pattern: &str, replacement: &Cirru, count: &mut u32) {
  match node {
    Cirru::Leaf(s) => {
      if s.as_ref() == pattern {
        *node = replacement.clone();
        *count += 1;
      }
    }
    Cirru::List(items) => {
      for item in items.iter_mut() {
        replace_leaf_nodes(item, pattern, replacement, count);
      }
    }
  }
}

/// `(calcit.cli/add-import $ {} (:file-path <path>) (:namespace <ns>) (:source-ns <src>) (:refer-sym <sym>))` → add a :refer import to a namespace
pub fn add_import(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/add-import", &xs, ADD_IMPORT)?;
  let file_path = args.string("file-path")?;
  let ns_name = args.string("namespace")?;
  let src_ns = args.string("source-ns")?;
  let refer_sym = args.string("refer-sym")?;

  let mut snapshot = load_calcit_snapshot(&file_path)?;

  let file_data = get_file_mut(&mut snapshot, &ns_name)?;

  // Build import rule Cirru: (src-ns :refer $ sym)
  let import_rule = Cirru::List(vec![
    Cirru::Leaf(Arc::from(src_ns.as_str())),
    Cirru::Leaf(Arc::from(":refer")),
    Cirru::Leaf(Arc::from("$")),
    Cirru::Leaf(Arc::from(refer_sym.as_str())),
  ]);

  // Append to :require section of ns code
  // ns code is a Cirru list: (ns name ...rules...)
  // We find :require and append to its children
  let ns_cirru = &mut file_data.ns.code;
  match ns_cirru {
    Cirru::List(items) => upsert_require_rule(items, import_rule)?,
    _ => return Err(CalcitErr::from("ns code is not a list".to_string())),
  }

  save_calcit_snapshot(&file_path, &snapshot)?;

  Ok(Calcit::Str(Arc::from("import added")))
}

// ─── Config functions ────────────────────────────────────────────────────────

/// `(calcit.cli/list-config $ {} (:file-path <path>))` → return config as Cirru EDN string
pub fn list_config(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/list-config", &xs, LIST_CONFIG)?;
  let file_path = args.string("file-path")?;
  let snapshot = load_calcit_snapshot(&file_path)?;
  let configs = &snapshot.configs;
  let mut entries_vec: Vec<String> = snapshot.entries.keys().cloned().collect();
  entries_vec.sort();
  let modules_str = configs.modules.iter().map(|m| format!("|{m}")).collect::<Vec<_>>().join(" ");
  let about_str = snapshot
    .about
    .as_ref()
    .map(|a| format!("|{a}"))
    .unwrap_or_else(|| "nil".to_string());
  let result = format!(
    "{{:init-fn |{} :reload-fn |{} :modules [{}] :entries [{}] :package |{} :about {}}}",
    configs.init_fn,
    configs.reload_fn,
    modules_str,
    entries_vec.join(" "),
    snapshot.package,
    about_str,
  );
  Ok(Calcit::Str(Arc::from(result)))
}

/// `(calcit.cli/list-modules $ {} (:file-path <path>))` → list module dependencies
pub fn list_modules(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/list-modules", &xs, LIST_MODULES)?;
  let file_path = args.string("file-path")?;
  let snapshot = load_calcit_snapshot(&file_path)?;
  let modules: Vec<Calcit> = snapshot
    .configs
    .modules
    .iter()
    .map(|m| Calcit::Str(Arc::from(m.as_str())))
    .collect();
  Ok(Calcit::List(Arc::new(CalcitList::from(modules.as_slice()))))
}

// ─── Tree functions ──────────────────────────────────────────────────────────

/// `(calcit.cli/tree-show $ {} (:file-path <path>) (:target <ns/def>) (:path <path>?) (:max-lines 80?))` → show AST subtree
pub fn tree_show(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/tree-show", &xs, TREE_SHOW)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let path_str = args.optional_string("path");
  let max_lines = args.usize("max-lines")?;
  let snapshot = load_calcit_snapshot(&file_path)?;
  let entry = get_def(&snapshot, &ns_name, &def_name)?;
  let code = &entry.code;
  let target_node = if let Some(p) = path_str {
    let indices = parse_path("tree-show", &p, true)?;
    navigate_to_node(code, &indices)?
  } else {
    code
  };
  let result = limit_lines(format_cirru("tree-show", target_node)?, max_lines);
  Ok(Calcit::Str(Arc::from(result)))
}

pub(crate) fn navigate_to_node<'a>(node: &'a Cirru, path: &[usize]) -> Result<&'a Cirru, CalcitErr> {
  let mut current = node;
  for &idx in path {
    match current {
      Cirru::List(items) => {
        current = items
          .get(idx)
          .ok_or_else(|| CalcitErr::from(format!("Path index {idx} out of bounds (len {})", items.len())))?;
      }
      _ => return Err(CalcitErr::from("Cannot navigate into leaf node at path".to_string())),
    }
  }
  Ok(current)
}

// ─── helpers (pub(crate) for calcit_cli_extra) ───────────────────────────────

fn options_map_has_key(xs: &[Calcit], key: &str) -> bool {
  let Some(Calcit::Map(map)) = xs.first() else {
    return false;
  };
  map.iter().any(|(k, _)| match k {
    Calcit::Tag(tag) => tag.ref_str().trim_start_matches(':') == key,
    Calcit::Str(s) => s.strip_prefix('|').unwrap_or(s) == key,
    _ => false,
  })
}

pub(crate) fn parse_path(fn_name: &str, raw: &str, allow_root: bool) -> Result<Vec<usize>, CalcitErr> {
  let trimmed = raw.trim();
  if trimmed.is_empty() || trimmed == "." {
    if allow_root {
      return Ok(Vec::new());
    }
    return Err(CalcitErr::from(format!("{fn_name}: root path is not allowed for this operation")));
  }

  let mut path = Vec::new();
  for (idx, segment) in trimmed.split('.').enumerate() {
    if segment.is_empty() {
      return Err(CalcitErr::from(format!(
        "{fn_name}: invalid path `{raw}`: empty segment at position {idx}"
      )));
    }
    let n = segment.parse::<usize>().map_err(|_| {
      CalcitErr::from(format!(
        "{fn_name}: invalid path `{raw}`: segment `{segment}` is not an unsigned integer"
      ))
    })?;
    path.push(n);
  }
  Ok(path)
}

pub(crate) fn format_path(path: &[usize]) -> String {
  if path.is_empty() {
    ".".to_string()
  } else {
    path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(".")
  }
}

fn format_cirru(fn_name: &str, node: &Cirru) -> Result<String, CalcitErr> {
  cirru_parser::format(std::slice::from_ref(node), true.into())
    .map_err(|e| CalcitErr::from(format!("{fn_name}: failed to format Cirru node: {e}")))
}

pub(crate) fn parse_single_cirru(fn_name: &str, code: &str) -> Result<Cirru, CalcitErr> {
  let parsed = cirru_parser::parse(code).map_err(|e| CalcitErr::from(format!("{fn_name}: failed to parse Cirru code: {e}")))?;
  if parsed.len() != 1 {
    return Err(CalcitErr::from(format!(
      "{fn_name}: expected exactly one Cirru expression, got {}",
      parsed.len()
    )));
  }
  Ok(parsed.into_iter().next().expect("checked one expression"))
}

pub(crate) fn calcit_str_list(items: Vec<String>) -> Calcit {
  let values = items.into_iter().map(|s| Calcit::Str(Arc::from(s))).collect::<Vec<_>>();
  Calcit::List(Arc::new(CalcitList::from(values.as_slice())))
}

fn get_file<'a>(snapshot: &'a snapshot::Snapshot, ns_name: &str) -> Result<&'a FileInSnapShot, CalcitErr> {
  snapshot
    .files
    .get(ns_name)
    .ok_or_else(|| CalcitErr::from(format!("Namespace `{ns_name}` not found")))
}

pub(crate) fn get_file_mut<'a>(snapshot: &'a mut snapshot::Snapshot, ns_name: &str) -> Result<&'a mut FileInSnapShot, CalcitErr> {
  snapshot
    .files
    .get_mut(ns_name)
    .ok_or_else(|| CalcitErr::from(format!("Namespace `{ns_name}` not found")))
}

pub(crate) fn get_def<'a>(snapshot: &'a snapshot::Snapshot, ns_name: &str, def_name: &str) -> Result<&'a CodeEntry, CalcitErr> {
  get_file(snapshot, ns_name)?
    .defs
    .get(def_name)
    .ok_or_else(|| CalcitErr::from(format!("Definition `{def_name}` not found in namespace `{ns_name}`")))
}

fn search_cirru_for_exact_leaf(node: &Cirru, target: &str, path: &mut Vec<usize>, results: &mut Vec<Vec<usize>>) {
  match node {
    Cirru::Leaf(s) if s.as_ref() == target => results.push(path.clone()),
    Cirru::List(items) => {
      for (i, child) in items.iter().enumerate() {
        path.push(i);
        search_cirru_for_exact_leaf(child, target, path, results);
        path.pop();
      }
    }
    _ => {}
  }
}

pub(crate) fn preview_leaf(s: &str) -> String {
  const MAX: usize = 64;
  if s.len() <= MAX {
    format!("|{s}")
  } else {
    format!("|{}...", &s[..MAX])
  }
}

fn limit_lines(text: String, max_lines: usize) -> String {
  if max_lines == 0 {
    return String::new();
  }
  let lines = text.lines().collect::<Vec<_>>();
  if lines.len() <= max_lines {
    text
  } else {
    format!("{}\n... ({} more lines)", lines[..max_lines].join("\n"), lines.len() - max_lines)
  }
}

pub(crate) fn check_ns_editable(snapshot: &snapshot::Snapshot, namespace: &str) -> Result<(), CalcitErr> {
  let pkg = &snapshot.package;
  if namespace == pkg || namespace.starts_with(&format!("{pkg}.")) {
    Ok(())
  } else {
    Err(CalcitErr::from(format!(
      "Cannot modify namespace `{namespace}`: only namespaces under package `{pkg}` can be edited"
    )))
  }
}

fn get_require_source_ns(rule: &Cirru) -> Option<String> {
  match rule {
    Cirru::List(items) => items.first().and_then(|item| match item {
      Cirru::Leaf(s) => Some(s.to_string()),
      _ => None,
    }),
    Cirru::Leaf(s) => Some(s.to_string()),
  }
}

pub(crate) fn extract_require_rules(ns_code: &Cirru) -> Vec<Cirru> {
  let mut rules = vec![];
  if let Cirru::List(items) = ns_code {
    for item in items.iter().skip(2) {
      if let Cirru::List(inner) = item {
        if let Some(Cirru::Leaf(first)) = inner.first()
          && first.as_ref() == ":require"
        {
          rules.extend(inner.iter().skip(1).cloned());
          break;
        }
      }
    }
  }
  rules
}

pub(crate) fn build_ns_code(ns_name: &str, rules: &[Cirru]) -> Cirru {
  let mut items = vec![Cirru::Leaf(Arc::from("ns")), Cirru::Leaf(Arc::from(ns_name))];
  if !rules.is_empty() {
    let mut require_list = vec![Cirru::Leaf(Arc::from(":require"))];
    require_list.extend(rules.iter().cloned());
    items.push(Cirru::List(require_list));
  }
  Cirru::List(items)
}

fn unwrap_schema_quote_input(schema: Cirru) -> Result<Cirru, CalcitErr> {
  match schema {
    Cirru::List(items) => {
      if let Some(Cirru::Leaf(head)) = items.first()
        && head.as_ref() == "quote"
      {
        if items.len() != 2 {
          return Err(CalcitErr::from("edit-schema: schema quote expects exactly one payload".to_string()));
        }
        return Ok(items[1].clone());
      }
      Ok(Cirru::List(items))
    }
    other => Ok(other),
  }
}

fn strip_name_field_from_schema(schema: Cirru) -> Cirru {
  match schema {
    Cirru::List(items) => {
      if items.is_empty() {
        return Cirru::List(items);
      }
      if let Some(Cirru::Leaf(head)) = items.first() {
        if head.as_ref() == ":optional" && items.len() == 2 {
          return Cirru::List(vec![items[0].clone(), strip_name_field_from_schema(items[1].clone())]);
        }
        if head.as_ref() == "::" && items.len() == 3 && matches!(items.get(1), Some(Cirru::Leaf(tag)) if tag.as_ref() == ":optional") {
          return Cirru::List(vec![
            items[0].clone(),
            items[1].clone(),
            strip_name_field_from_schema(items[2].clone()),
          ]);
        }
        if head.as_ref() == "{}" {
          let mut next_items = vec![items[0].clone()];
          for pair in items.iter().skip(1) {
            if let Cirru::List(xs) = pair
              && xs.len() == 2
              && matches!(xs.first(), Some(Cirru::Leaf(key)) if key.as_ref() == ":name")
            {
              continue;
            }
            next_items.push(pair.clone());
          }
          return Cirru::List(next_items);
        }
      }
      Cirru::List(items)
    }
    other => other,
  }
}

fn upsert_require_rule(items: &mut Vec<Cirru>, import_rule: Cirru) -> Result<(), CalcitErr> {
  for item in items.iter_mut().skip(2) {
    if let Cirru::List(inner) = item {
      if let Some(Cirru::Leaf(first)) = inner.first()
        && first.as_ref() == ":require"
      {
        if inner.iter().skip(1).any(|rule| rule == &import_rule) {
          return Err(CalcitErr::from("add-import: import rule already exists".to_string()));
        }
        inner.push(import_rule);
        return Ok(());
      }
    }
  }

  let require_list = vec![Cirru::Leaf(Arc::from(":require")), import_rule];
  items.push(Cirru::List(require_list));
  Ok(())
}

pub(crate) fn load_calcit_snapshot(file_path: &str) -> Result<snapshot::Snapshot, CalcitErr> {
  load_calcit_snapshot_with_deps(file_path, false)
}

/// Load snapshot; when `include_deps` is true, merge configured modules and calcit.core (like `cr query`).
pub(crate) fn load_calcit_snapshot_with_deps(file_path: &str, include_deps: bool) -> Result<snapshot::Snapshot, CalcitErr> {
  let path = Path::new(file_path);
  if !path.exists() {
    return Err(CalcitErr::from(format!("File not found: {file_path}")));
  }
  let mut content = fs::read_to_string(path).map_err(|e| CalcitErr::from(format!("Failed to read file '{file_path}': {e}")))?;
  strip_shebang(&mut content);
  let data = cirru_edn::parse(&content).map_err(|e| {
    let preview = if content.len() > 200 {
      format!("{}...", &content[..200])
    } else {
      content.clone()
    };
    CalcitErr::from(format!("Failed to parse '{file_path}': {e}\nPreview: {preview}"))
  })?;
  let mut snapshot = snapshot::load_snapshot_data(&data, file_path)
    .map_err(|e| CalcitErr::from(format!("Failed to load snapshot '{file_path}': {e}")))?;

  if !include_deps {
    return Ok(snapshot);
  }

  let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
  let module_folder = dirs::home_dir()
    .map(|buf| buf.as_path().join(".config/calcit/modules/"))
    .ok_or_else(|| CalcitErr::from("load snapshot: failed to resolve $HOME for module lookup".to_string()))?;

  let previous = calcit::quiet_tool_output();
  calcit::set_quiet_tool_output(true);
  for module_path in snapshot.configs.modules.clone() {
    match calcit::load_module(&module_path, base_dir, &module_folder) {
      Ok(module_snapshot) => {
        for (ns_name, file_data) in module_snapshot.files {
          if snapshot.files.contains_key(&ns_name) {
            calcit::set_quiet_tool_output(previous);
            return Err(CalcitErr::from(format!(
              "namespace `{ns_name}` already exists when loading module `{module_path}`"
            )));
          }
          snapshot.files.insert(ns_name, file_data);
        }
      }
      Err(e) => {
        eprintln!("Warning: failed to load module '{module_path}': {e}");
      }
    }
  }
  calcit::set_quiet_tool_output(previous);

  let core_snapshot = calcit::load_core_snapshot().map_err(|e| CalcitErr::from(format!("load snapshot: {e}")))?;
  for (ns_name, file_data) in core_snapshot.files {
    snapshot.files.entry(ns_name).or_insert(file_data);
  }

  Ok(snapshot)
}

pub(crate) fn save_calcit_snapshot(file_path: &str, snapshot: &snapshot::Snapshot) -> Result<(), CalcitErr> {
  snapshot::save_snapshot_to_file(file_path, snapshot)
    .map_err(|e| CalcitErr::from(format!("Failed to save snapshot '{file_path}': {e}")))
}

#[cfg(test)]
mod tests {
  use super::super::calcit_cli_args::build_cli_opts;
  use super::*;
  use calcit::call_stack::CallStackList;
  use std::time::{SystemTime, UNIX_EPOCH};

  fn empty_stack() -> CallStackList {
    CallStackList::default()
  }

  fn temp_copy_of_test_snapshot() -> String {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).expect("time").as_nanos();
    let path = std::env::temp_dir().join(format!("calcit-cli-test-{stamp}.cirru"));
    std::fs::copy("calcit/test.cirru", &path).expect("copy test snapshot");
    path.display().to_string()
  }

  #[test]
  fn parse_path_rejects_non_numeric_segment() {
    assert!(parse_path("tree-show", "bad.path", true).is_err());
  }

  #[test]
  fn list_ns_reads_test_snapshot() {
    calcit::set_quiet_tool_output(true);
    let stack = empty_stack();
    let result = list_namespaces(
      build_cli_opts(&[("file-path", Calcit::Str(Arc::from("calcit/test.cirru")))]),
      &stack,
    )
    .expect("list-ns");
    assert!(matches!(result, Calcit::List(_)));
  }

  #[test]
  fn tree_delete_and_insert_roundtrip() {
    calcit::set_quiet_tool_output(true);
    let file = temp_copy_of_test_snapshot();
    let stack = empty_stack();
    let _ = tree_insert(
      build_cli_opts(&[
        ("file-path", Calcit::Str(Arc::from(file.as_str()))),
        ("target", Calcit::Str(Arc::from("|app.main/main!"))),
        ("path", Calcit::Str(Arc::from("|0"))),
        ("code", Calcit::Str(Arc::from("|println |temp-marker"))),
        ("position", Calcit::Str(Arc::from("|after"))),
      ]),
      &stack,
    )
    .expect("tree-insert");
    tree_delete(
      build_cli_opts(&[
        ("file-path", Calcit::Str(Arc::from(file.as_str()))),
        ("target", Calcit::Str(Arc::from("|app.main/main!"))),
        ("path", Calcit::Str(Arc::from("|1"))),
      ]),
      &stack,
    )
    .expect("tree-delete");
    let _ = std::fs::remove_file(&file);
  }

  #[test]
  fn show_error_reads_missing_file_gracefully() {
    calcit::set_quiet_tool_output(true);
    let stack = empty_stack();
    let result = show_error(
      build_cli_opts(&[("error-file", Calcit::Str(Arc::from("|/no/such/error-file.cirru")))]),
      &stack,
    )
    .expect("show-error");
    assert!(matches!(result, Calcit::Str(_)));
  }
}
