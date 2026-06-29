//! Additional `calcit.cli/*` functions for edit/tree/config/cirru utilities.

use calcit::calcit::{Calcit, CalcitErr};
use calcit::call_stack::CallStackList;
use calcit::snapshot::{ChangesDict, FileChangeInfo, render_snapshot_content};
use cirru_edn::EdnTag;
use cirru_parser::Cirru;
use semver::Version;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::calcit_cli::{
  build_ns_code, check_ns_editable, get_def, get_file_mut, load_calcit_snapshot, navigate_to_node, parse_path, parse_single_cirru,
  save_calcit_snapshot,
};
use super::calcit_cli_args::{parse_target, resolve_cli_args};
use super::calcit_cli_specs::{
  ADD_MODULE, BUMP_VERSION, CIRRU_FORMAT, CIRRU_PARSE, CIRRU_PARSE_EDN, CIRRU_SHOW_GUIDE, CLEAR_EXAMPLES, DOCS_SEARCH, EDIT_NS_DOC,
  FORMAT_FILE, LIST_TAGS, READ_TEXT_FILE, RM_EXAMPLE, RM_MODULE, RM_NS, SET_CONFIG, SET_EXAMPLES, SET_IMPORTS, SET_TAGS, SHOW_DOC,
  SHOW_NS_DOC, TREE_BATCH_DELETE, TREE_REPLACE_LEAF, TREE_REPLACE_LEAF_REGEX, TREE_REWRITE, TREE_SWAP_NEXT, TREE_SWAP_PREV,
  TRIGGER_INC,
};
use super::calcit_cli_tree::{apply_operation_at_path, find_exact_leaf_paths, find_regex_leaf_paths, process_node_with_references};

// ─── edit / ns ───────────────────────────────────────────────────────────────

pub fn rm_ns(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/rm-ns", &xs, RM_NS)?;
  let file_path = args.string("file-path")?;
  let ns_name = args.string("namespace")?;
  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  if snapshot.files.remove(&ns_name).is_none() {
    return Err(CalcitErr::from(format!("rm-ns: namespace `{ns_name}` not found")));
  }
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("removed")))
}

pub fn set_imports(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/set-imports", &xs, SET_IMPORTS)?;
  let file_path = args.string("file-path")?;
  let ns_name = args.string("namespace")?;
  let rules_code = args.string("rules-code")?;
  let rules = parse_import_rules("set-imports", &rules_code)?;

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  file_data.ns.code = build_ns_code(&ns_name, &rules);
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from(format!("updated {} rule(s)", rules.len()))))
}

pub fn format_file(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/format-file", &xs, FORMAT_FILE)?;
  let file_path = args.string("file-path")?;
  let original =
    fs::read_to_string(&file_path).map_err(|e| CalcitErr::from(format!("format-file: failed to read `{file_path}`: {e}")))?;
  let snapshot = load_calcit_snapshot(&file_path)?;
  let formatted =
    render_snapshot_content(&snapshot).map_err(|e| CalcitErr::from(format!("format-file: failed to render snapshot: {e}")))?;
  if formatted == original {
    return Ok(Calcit::Str(Arc::from("unchanged")));
  }
  fs::write(&file_path, formatted).map_err(|e| CalcitErr::from(format!("format-file: failed to write `{file_path}`: {e}")))?;
  Ok(Calcit::Str(Arc::from("formatted")))
}

pub fn show_doc(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/show-doc", &xs, SHOW_DOC)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let snapshot = load_calcit_snapshot(&file_path)?;
  let entry = get_def(&snapshot, &ns_name, &def_name)?;
  Ok(Calcit::Str(Arc::from(entry.doc.as_str())))
}

pub fn show_ns_doc(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/show-ns-doc", &xs, SHOW_NS_DOC)?;
  let file_path = args.string("file-path")?;
  let ns_name = args.string("namespace")?;
  let snapshot = load_calcit_snapshot(&file_path)?;
  let file = snapshot
    .files
    .get(&ns_name)
    .ok_or_else(|| CalcitErr::from(format!("show-ns-doc: namespace `{ns_name}` not found")))?;
  Ok(Calcit::Str(Arc::from(file.ns.doc.as_str())))
}

pub fn edit_ns_doc(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/edit-ns-doc", &xs, EDIT_NS_DOC)?;
  let file_path = args.string("file-path")?;
  let ns_name = args.string("namespace")?;
  let doc = args.string("doc")?;

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  file_data.ns.doc = doc;
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("updated")))
}

pub fn bump_version(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/bump-version", &xs, BUMP_VERSION)?;
  let file_path = args.string("file-path")?;
  let level = args.string("kind")?;
  if !matches!(level.as_str(), "patch" | "minor" | "major") {
    return Err(CalcitErr::from(format!(
      "bump-version: unknown level `{level}`; use patch, minor, or major"
    )));
  }

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  let previous = snapshot.configs.version.clone();
  let next = bump_semver_value(&previous, &level).map_err(CalcitErr::from)?;
  snapshot.configs.version = next.clone();
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from(format!("{previous} -> {next}"))))
}

fn bump_semver_value(current: &str, level: &str) -> Result<String, String> {
  let mut version = Version::parse(current).map_err(|_| format!("Invalid version `{current}`"))?;
  match level {
    "patch" => version.patch += 1,
    "minor" => {
      version.minor += 1;
      version.patch = 0;
    }
    "major" => {
      version.major += 1;
      version.minor = 0;
      version.patch = 0;
    }
    _ => return Err(format!("Unknown bump level `{level}`")),
  }
  version.pre = semver::Prerelease::EMPTY;
  version.build = semver::BuildMetadata::EMPTY;
  Ok(version.to_string())
}

pub fn list_tags(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/list-tags", &xs, LIST_TAGS)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let snapshot = load_calcit_snapshot(&file_path)?;
  let entry = get_def(&snapshot, &ns_name, &def_name)?;
  Ok(Calcit::Str(Arc::from(format_tags_csv(&entry.tags))))
}

pub fn set_tags(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/set-tags", &xs, SET_TAGS)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let tags_csv = args.string("tags")?;
  let tags = parse_tags_csv("set-tags", &tags_csv)?;

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  let entry = file_data
    .defs
    .get_mut(&def_name)
    .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?;
  entry.tags = tags;
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("updated")))
}

pub fn rm_example(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/rm-example", &xs, RM_EXAMPLE)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let index = args.usize("index")?;

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  let entry = file_data
    .defs
    .get_mut(&def_name)
    .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?;
  if index >= entry.examples.len() {
    return Err(CalcitErr::from(format!(
      "rm-example: index {index} out of range (max: {})",
      entry.examples.len().saturating_sub(1)
    )));
  }
  entry.examples.remove(index);
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("removed")))
}

pub fn clear_examples(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/clear-examples", &xs, CLEAR_EXAMPLES)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  let entry = file_data
    .defs
    .get_mut(&def_name)
    .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?;
  let count = entry.examples.len();
  entry.examples.clear();
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from(format!("cleared {count} example(s)"))))
}

pub fn set_examples(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/set-examples", &xs, SET_EXAMPLES)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let code = args.string("examples-code")?;
  let examples = cirru_parser::parse(&code).map_err(|e| CalcitErr::from(format!("set-examples: failed to parse Cirru: {e}")))?;

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  let entry = file_data
    .defs
    .get_mut(&def_name)
    .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?;
  let count = examples.len();
  entry.examples = examples;
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from(format!("set {count} example(s)"))))
}

// ─── tree ────────────────────────────────────────────────────────────────────

pub fn tree_replace_leaf(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/tree-replace-leaf", &xs, TREE_REPLACE_LEAF)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let pattern = args.string("pattern")?;
  let replacement_code = args.string("replacement-code")?;
  let replacement = parse_single_cirru("tree-replace-leaf", &replacement_code)?;

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  let entry = file_data
    .defs
    .get_mut(&def_name)
    .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?;

  let mut matches = Vec::new();
  find_exact_leaf_paths(&entry.code, &pattern, &mut vec![], &mut matches);
  if matches.is_empty() {
    return Err(CalcitErr::from(format!("tree-replace-leaf: pattern `{pattern}` not found")));
  }
  matches.sort_by(|a, b| b.cmp(a));
  let mut code = entry.code.clone();
  for path in matches {
    code = apply_operation_at_path(&code, &path, "replace", Some(&replacement))?;
  }
  entry.code = code;
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("replaced")))
}

pub fn tree_replace_leaf_regex(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/tree-replace-leaf-regex", &xs, TREE_REPLACE_LEAF_REGEX)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let pattern_str = args.string("regex")?;
  let replacement_code = args.string("replacement-code")?;
  let pattern = regex::Regex::new(&pattern_str)
    .map_err(|e| CalcitErr::from(format!("tree-replace-leaf-regex: invalid regex `{pattern_str}`: {e}")))?;
  let replacement = parse_single_cirru("tree-replace-leaf-regex", &replacement_code)?;

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  let entry = file_data
    .defs
    .get_mut(&def_name)
    .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?;

  let mut matches = Vec::new();
  find_regex_leaf_paths(&entry.code, &pattern, &mut vec![], &mut matches);
  if matches.is_empty() {
    return Err(CalcitErr::from(format!(
      "tree-replace-leaf-regex: pattern `{pattern_str}` matched no leaves"
    )));
  }
  matches.sort_by(|a, b| b.cmp(a));
  let mut code = entry.code.clone();
  for path in matches {
    code = apply_operation_at_path(&code, &path, "replace", Some(&replacement))?;
  }
  entry.code = code;
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("replaced")))
}

pub fn tree_swap_next(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  tree_swap(xs, "calcit.cli/tree-swap-next", TREE_SWAP_NEXT, "swap-next-sibling")
}

pub fn tree_swap_prev(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  tree_swap(xs, "calcit.cli/tree-swap-prev", TREE_SWAP_PREV, "swap-prev-sibling")
}

fn tree_swap(
  xs: Vec<Calcit>,
  proc_name: &str,
  specs: &'static [super::calcit_cli_args::CliArgSpec],
  operation: &str,
) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args(proc_name, &xs, specs)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let path_str = args.string("path")?;
  let indices = parse_path(proc_name, &path_str, false)?;

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  let entry = file_data
    .defs
    .get_mut(&def_name)
    .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?;
  entry.code = apply_operation_at_path(&entry.code, &indices, operation, None)?;
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("swapped")))
}

pub fn tree_batch_delete(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/tree-batch-delete", &xs, TREE_BATCH_DELETE)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let paths_raw = args.string("paths")?;

  let mut paths: Vec<Vec<usize>> = paths_raw
    .split(',')
    .map(|p| p.trim())
    .filter(|p| !p.is_empty())
    .map(|p| parse_path("tree-batch-delete", p, false))
    .collect::<Result<Vec<_>, _>>()?;
  if paths.is_empty() {
    return Err(CalcitErr::from(
      "tree-batch-delete: no paths provided (comma-separated)".to_string(),
    ));
  }
  paths.sort_by(|a, b| b.cmp(a));

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  let entry = file_data
    .defs
    .get_mut(&def_name)
    .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?;
  let mut code = entry.code.clone();
  for path in paths {
    code = apply_operation_at_path(&code, &path, "delete", None)?;
  }
  entry.code = code;
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("deleted")))
}

pub fn tree_rewrite(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/tree-rewrite", &xs, TREE_REWRITE)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let path_str = args.string("path")?;
  let template_code = args.string("template-code")?;
  let refs_raw = args.string("refs")?;
  let indices = parse_path("calcit.cli/tree-rewrite", &path_str, true)?;
  let template = parse_single_cirru("tree-rewrite", &template_code)?;

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  check_ns_editable(&snapshot, &ns_name)?;
  let file_data = get_file_mut(&mut snapshot, &ns_name)?;
  let entry = file_data
    .defs
    .get_mut(&def_name)
    .ok_or_else(|| CalcitErr::from(format!("Definition '{def_name}' not found")))?;
  let original = navigate_to_node(&entry.code, &indices)?.clone();
  let references = parse_rewrite_refs("tree-rewrite", &refs_raw, &original)?;
  let processed = process_node_with_references(&template, &references)?;
  entry.code = apply_operation_at_path(&entry.code, &indices, "replace", Some(&processed))?;
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("rewritten")))
}

// ─── config ──────────────────────────────────────────────────────────────────

pub fn set_config(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/set-config", &xs, SET_CONFIG)?;
  let file_path = args.string("file-path")?;
  let key = args.string("key")?;
  let value = args.string("value")?;
  let entry_name = args.optional_string("entry");

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  let configs = if let Some(name) = entry_name {
    if !snapshot.entries.contains_key(&name) {
      return Err(CalcitErr::from(format!("set-config: entry `{name}` not found")));
    }
    snapshot.entries.get_mut(&name).expect("checked")
  } else {
    &mut snapshot.configs
  };

  match normalize_config_key(&key) {
    "init-fn" => configs.init_fn = value,
    "reload-fn" => configs.reload_fn = value,
    "version" => {
      if matches!(value.as_str(), "patch" | "minor" | "major") {
        configs.version = bump_semver(&configs.version, &value)?;
      } else {
        parse_semver(&value)?;
        configs.version = value;
      }
    }
    other => {
      return Err(CalcitErr::from(format!(
        "set-config: unknown key `{other}`; use init-fn, reload-fn, or version"
      )));
    }
  }
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("updated")))
}

pub fn add_module(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/add-module", &xs, ADD_MODULE)?;
  let file_path = args.string("file-path")?;
  let module_path = args.string("module-path")?;
  let entry_name = args.optional_string("entry");

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  let modules = if let Some(name) = entry_name {
    if !snapshot.entries.contains_key(&name) {
      return Err(CalcitErr::from(format!("add-module: entry `{name}` not found")));
    }
    &mut snapshot.entries.get_mut(&name).expect("checked").modules
  } else {
    &mut snapshot.configs.modules
  };
  if modules.iter().any(|m| m == &module_path) {
    return Err(CalcitErr::from(format!("add-module: module `{module_path}` already exists")));
  }
  modules.push(module_path);
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("added")))
}

pub fn rm_module(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/rm-module", &xs, RM_MODULE)?;
  let file_path = args.string("file-path")?;
  let module_path = args.string("module-path")?;
  let entry_name = args.optional_string("entry");

  let mut snapshot = load_calcit_snapshot(&file_path)?;
  let modules = if let Some(name) = entry_name {
    if !snapshot.entries.contains_key(&name) {
      return Err(CalcitErr::from(format!("rm-module: entry `{name}` not found")));
    }
    &mut snapshot.entries.get_mut(&name).expect("checked").modules
  } else {
    &mut snapshot.configs.modules
  };
  let before = modules.len();
  modules.retain(|m| m != &module_path);
  if modules.len() == before {
    return Err(CalcitErr::from(format!("rm-module: module `{module_path}` not found")));
  }
  save_calcit_snapshot(&file_path, &snapshot)?;
  Ok(Calcit::Str(Arc::from("removed")))
}

// ─── cirru / file utilities ──────────────────────────────────────────────────

pub fn cirru_parse(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/cirru-parse", &xs, CIRRU_PARSE)?;
  let code = args.string("code")?;
  let one_liner = args.bool("one-liner")?;
  let nodes = if one_liner {
    vec![cirru_parser::parse_expr_one_liner(&code).map_err(|e| CalcitErr::from(format!("cirru-parse: {e}")))?]
  } else {
    cirru_parser::parse(&code).map_err(|e| CalcitErr::from(format!("cirru-parse: {e}")))?
  };
  let json = serde_json::to_string_pretty(&nodes.iter().map(cirru_to_json).collect::<Vec<_>>())
    .map_err(|e| CalcitErr::from(format!("cirru-parse: failed to serialize JSON: {e}")))?;
  Ok(Calcit::Str(Arc::from(json)))
}

pub fn cirru_format(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/cirru-format", &xs, CIRRU_FORMAT)?;
  let json_str = args.string("json")?;
  let value: serde_json::Value =
    serde_json::from_str(&json_str).map_err(|e| CalcitErr::from(format!("cirru-format: invalid JSON: {e}")))?;
  let node = json_to_cirru(&value)?;
  let formatted =
    cirru_parser::format(std::slice::from_ref(&node), true.into()).map_err(|e| CalcitErr::from(format!("cirru-format: {e}")))?;
  Ok(Calcit::Str(Arc::from(formatted)))
}

pub fn read_text_file(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/read-text-file", &xs, READ_TEXT_FILE)?;
  let path = args.string("path")?;
  if !Path::new(&path).exists() {
    return Err(CalcitErr::from(format!("read-text-file: file not found: `{path}`")));
  }
  let content = fs::read_to_string(&path).map_err(|e| CalcitErr::from(format!("read-text-file: {e}")))?;
  Ok(Calcit::Str(Arc::from(content)))
}

pub fn cirru_parse_edn(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/cirru-parse-edn", &xs, CIRRU_PARSE_EDN)?;
  let edn_str = args.string("edn")?;
  let edn = cirru_edn::parse(&edn_str).map_err(|e| CalcitErr::from(format!("cirru-parse-edn: {e}")))?;
  let json = serde_json::to_string_pretty(&edn).map_err(|e| CalcitErr::from(format!("cirru-parse-edn: {e}")))?;
  Ok(Calcit::Str(Arc::from(json)))
}

pub fn cirru_show_guide(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let _args = resolve_cli_args("calcit.cli/cirru-show-guide", &xs, CIRRU_SHOW_GUIDE)?;
  let home_dir = std::env::var("HOME").map_err(|_| CalcitErr::from("cirru-show-guide: failed to get HOME directory".to_string()))?;
  let guide_path = format!("{home_dir}/.config/calcit/docs/cirru-syntax.md");
  let content = fs::read_to_string(&guide_path).map_err(|_| {
    CalcitErr::from(format!(
      "cirru-show-guide: guide not found at `{guide_path}`. Ensure docs are linked under ~/.config/calcit/docs"
    ))
  })?;
  Ok(Calcit::Str(Arc::from(content)))
}

pub fn docs_search(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/docs-search", &xs, DOCS_SEARCH)?;
  let keyword = args.string("keyword")?;
  if keyword.is_empty() {
    return Err(CalcitErr::from("docs-search: keyword cannot be empty".to_string()));
  }
  let keyword_lower = keyword.to_lowercase();
  let mut roots: Vec<PathBuf> = Vec::new();
  if let Some(dir) = args.optional_string("docs-dir") {
    roots.push(PathBuf::from(dir));
  } else {
    roots.push(PathBuf::from("docs"));
    if let Ok(home) = std::env::var("HOME") {
      roots.push(PathBuf::from(home).join(".config/calcit/docs"));
    }
  }

  let mut hits: Vec<String> = Vec::new();
  for root in roots {
    if !root.is_dir() {
      continue;
    }
    collect_doc_hits(&root, &root, &keyword_lower, &mut hits)?;
  }

  if hits.is_empty() {
    return Ok(Calcit::Str(Arc::from(format!("docs-search: no matches for `{keyword}`"))));
  }
  Ok(Calcit::Str(Arc::from(hits.join("\n"))))
}

pub fn trigger_inc(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/trigger-inc", &xs, TRIGGER_INC)?;
  let file_path = args.string("file-path")?;
  let changed_csv = args.string("changed")?;
  let added_csv = args.string("added")?;
  let removed_csv = args.string("removed")?;
  let added_ns_csv = args.string("added-ns")?;
  let removed_ns_csv = args.string("removed-ns")?;
  let ns_updated_csv = args.string("ns-updated")?;

  let changed = parse_csv_targets(&changed_csv);
  let added = parse_csv_targets(&added_csv);
  let removed = parse_csv_targets(&removed_csv);
  let added_ns = parse_csv_targets(&added_ns_csv);
  let removed_ns = parse_csv_targets(&removed_ns_csv);
  let ns_updated = parse_csv_targets(&ns_updated_csv);

  if changed.is_empty()
    && added.is_empty()
    && removed.is_empty()
    && added_ns.is_empty()
    && removed_ns.is_empty()
    && ns_updated.is_empty()
  {
    return Err(CalcitErr::from(
      "trigger-inc: no change hints provided. Pass changed/added/removed targets as comma-separated strings".to_string(),
    ));
  }

  let inc_file = ".compact-inc.cirru";
  let error_file = ".calcit-error.cirru";
  let _ = fs::write(error_file, "");

  let snapshot = load_calcit_snapshot(&file_path)?;
  let mut changes = ChangesDict::default();
  let mut changed_entries: HashMap<Arc<str>, FileChangeInfo> = HashMap::new();

  for ns in &added_ns {
    check_ns_editable(&snapshot, ns)?;
    let file = snapshot
      .files
      .get(ns)
      .ok_or_else(|| CalcitErr::from(format!("trigger-inc: namespace `{ns}` not found in snapshot")))?;
    changes.added.insert(Arc::from(ns.as_str()), file.clone());
  }

  for ns in &removed_ns {
    check_ns_editable(&snapshot, ns)?;
    changes.removed.insert(Arc::from(ns.as_str()));
  }

  for ns in &ns_updated {
    check_ns_editable(&snapshot, ns)?;
    let file = snapshot
      .files
      .get(ns)
      .ok_or_else(|| CalcitErr::from(format!("trigger-inc: namespace `{ns}` not found in snapshot")))?;
    let entry = ensure_change_entry(&mut changed_entries, ns);
    entry.ns = Some(file.ns.code.clone());
  }

  for target in &added {
    collect_def_change(&snapshot, target, "trigger-inc", &mut changed_entries, DefChangeKind::Added)?;
  }
  for target in &changed {
    collect_def_change(&snapshot, target, "trigger-inc", &mut changed_entries, DefChangeKind::Changed)?;
  }
  for target in &removed {
    let (namespace, definition) = parse_target("trigger-inc", target)?;
    check_ns_editable(&snapshot, &namespace)?;
    let entry = ensure_change_entry(&mut changed_entries, &namespace);
    entry.removed_defs.insert(definition);
  }

  if !changed_entries.is_empty() {
    changes.changed = changed_entries;
  }

  if changes.added.is_empty() && changes.removed.is_empty() && changes.changed.is_empty() {
    return Err(CalcitErr::from(
      "trigger-inc: no change data collected. Confirm targets exist in the snapshot file".to_string(),
    ));
  }

  let namespace_total = changes.added.len() + changes.removed.len() + changes.changed.len();
  let edn_data: cirru_edn::Edn = changes
    .try_into()
    .map_err(|e| CalcitErr::from(format!("trigger-inc: failed to serialize changes: {e}")))?;
  let content =
    cirru_edn::format(&edn_data, true).map_err(|e| CalcitErr::from(format!("trigger-inc: failed to format changes: {e}")))?;
  fs::write(inc_file, &content).map_err(|e| CalcitErr::from(format!("trigger-inc: failed to write `{inc_file}`: {e}")))?;

  Ok(Calcit::Str(Arc::from(format!("wrote {inc_file} (namespaces: {namespace_total})"))))
}

// ─── helpers ─────────────────────────────────────────────────────────────────

enum DefChangeKind {
  Added,
  Changed,
}

fn parse_csv_targets(raw: &str) -> Vec<String> {
  raw.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect()
}

fn ensure_change_entry<'a>(changed_entries: &'a mut HashMap<Arc<str>, FileChangeInfo>, namespace: &str) -> &'a mut FileChangeInfo {
  let key: Arc<str> = Arc::from(namespace);
  changed_entries.entry(key).or_insert_with(|| FileChangeInfo {
    ns: None,
    added_defs: HashMap::new(),
    removed_defs: HashSet::new(),
    changed_defs: HashMap::new(),
  })
}

fn collect_def_change(
  snapshot: &calcit::snapshot::Snapshot,
  target: &str,
  fn_name: &str,
  changed_entries: &mut HashMap<Arc<str>, FileChangeInfo>,
  kind: DefChangeKind,
) -> Result<(), CalcitErr> {
  let (namespace, definition) = parse_target(fn_name, target)?;
  check_ns_editable(snapshot, &namespace)?;
  let file = snapshot
    .files
    .get(&namespace)
    .ok_or_else(|| CalcitErr::from(format!("{fn_name}: namespace `{namespace}` not found")))?;
  let code_entry = file
    .defs
    .get(&definition)
    .ok_or_else(|| CalcitErr::from(format!("{fn_name}: definition `{definition}` not found in `{namespace}`")))?;
  let entry = ensure_change_entry(changed_entries, &namespace);
  match kind {
    DefChangeKind::Added => {
      entry.added_defs.insert(definition, code_entry.code.clone());
    }
    DefChangeKind::Changed => {
      entry.changed_defs.insert(definition, code_entry.code.clone());
    }
  }
  Ok(())
}

fn collect_doc_hits(base: &Path, dir: &Path, keyword_lower: &str, hits: &mut Vec<String>) -> Result<(), CalcitErr> {
  for entry in fs::read_dir(dir).map_err(|e| CalcitErr::from(format!("docs-search: failed to read `{}`: {e}", dir.display())))? {
    let entry = entry.map_err(|e| CalcitErr::from(format!("docs-search: {e}")))?;
    let path = entry.path();
    if path.is_dir() {
      collect_doc_hits(base, &path, keyword_lower, hits)?;
      continue;
    }
    if path.extension().and_then(|s| s.to_str()) != Some("md") {
      continue;
    }
    let content = fs::read_to_string(&path).map_err(|e| CalcitErr::from(format!("docs-search: {e}")))?;
    let rel = path.strip_prefix(base).unwrap_or(&path);
    for (idx, line) in content.lines().enumerate() {
      if line.to_lowercase().contains(keyword_lower) {
        hits.push(format!("{}:{}: {}", rel.display(), idx + 1, line.trim()));
      }
    }
  }
  Ok(())
}

fn parse_import_rules(fn_name: &str, code: &str) -> Result<Vec<Cirru>, CalcitErr> {
  let trimmed = code.trim();
  if trimmed.is_empty() {
    return Ok(vec![]);
  }
  let parsed = cirru_parser::parse(trimmed).map_err(|e| CalcitErr::from(format!("{fn_name}: failed to parse import rules: {e}")))?;
  if parsed.len() == 1 {
    if let Cirru::List(items) = &parsed[0] {
      if items
        .first()
        .is_some_and(|n| matches!(n, Cirru::Leaf(s) if s.as_ref() == ":require"))
      {
        return Ok(items.iter().skip(1).cloned().collect());
      }
    }
    return Ok(vec![parsed[0].clone()]);
  }
  Ok(parsed)
}

fn parse_rewrite_refs(fn_name: &str, raw: &str, original: &Cirru) -> Result<BTreeMap<String, Cirru>, CalcitErr> {
  let mut references = BTreeMap::new();
  for part in raw.split(',') {
    let piece = part.trim();
    if piece.is_empty() {
      continue;
    }
    let (name, path_str) = piece
      .split_once('=')
      .ok_or_else(|| CalcitErr::from(format!("{fn_name}: invalid ref `{piece}`, expected name=path")))?;
    if name.trim().is_empty() {
      return Err(CalcitErr::from(format!("{fn_name}: ref name cannot be empty in `{piece}`")));
    }
    let path = if path_str.trim() == "." {
      vec![]
    } else {
      parse_path(fn_name, path_str.trim(), true)?
    };
    let node = navigate_to_node(original, &path)?.clone();
    references.insert(name.trim().to_string(), node);
  }
  if references.is_empty() {
    return Err(CalcitErr::from(format!("{fn_name}: at least one ref mapping required")));
  }
  Ok(references)
}

fn parse_tags_csv(fn_name: &str, raw: &str) -> Result<HashSet<EdnTag>, CalcitErr> {
  let trimmed = raw.trim();
  if trimmed.is_empty() {
    return Ok(HashSet::new());
  }
  let mut tags = HashSet::new();
  for token in trimmed.split(',') {
    let piece = token.trim();
    if piece.is_empty() {
      return Err(CalcitErr::from(format!(
        "{fn_name}: tags must be comma-separated without empty items"
      )));
    }
    let name = piece.strip_prefix(':').unwrap_or(piece);
    tags.insert(EdnTag::new(name));
  }
  Ok(tags)
}

fn format_tags_csv(tags: &HashSet<EdnTag>) -> String {
  let mut items: Vec<String> = tags.iter().map(|t| format!(":{t}")).collect();
  items.sort();
  items.join(",")
}

fn normalize_config_key(key: &str) -> &str {
  match key {
    "init_fn" => "init-fn",
    "reload_fn" => "reload-fn",
    other => other,
  }
}

fn parse_semver(v: &str) -> Result<(), CalcitErr> {
  Version::parse(v).map_err(|_| CalcitErr::from(format!("set-config: invalid semver `{v}`")))?;
  Ok(())
}

fn bump_semver(current: &str, level: &str) -> Result<String, CalcitErr> {
  let version = Version::parse(current).map_err(|_| CalcitErr::from(format!("set-config: invalid current version `{current}`")))?;
  let next = match level {
    "patch" => Version::new(version.major, version.minor, version.patch + 1),
    "minor" => Version::new(version.major, version.minor + 1, 0),
    "major" => Version::new(version.major + 1, 0, 0),
    _ => return Err(CalcitErr::from(format!("set-config: unknown bump level `{level}`"))),
  };
  Ok(next.to_string())
}

fn cirru_to_json(cirru: &Cirru) -> serde_json::Value {
  match cirru {
    Cirru::Leaf(s) => serde_json::Value::String(s.to_string()),
    Cirru::List(items) => serde_json::Value::Array(items.iter().map(cirru_to_json).collect()),
  }
}

fn json_to_cirru(json: &serde_json::Value) -> Result<Cirru, CalcitErr> {
  match json {
    serde_json::Value::String(s) => Ok(Cirru::Leaf(Arc::from(s.as_str()))),
    serde_json::Value::Array(arr) => {
      let items: Result<Vec<Cirru>, CalcitErr> = arr.iter().map(json_to_cirru).collect();
      Ok(Cirru::List(items?))
    }
    serde_json::Value::Number(n) => Ok(Cirru::Leaf(Arc::from(n.to_string()))),
    serde_json::Value::Bool(b) => Ok(Cirru::Leaf(Arc::from(b.to_string()))),
    serde_json::Value::Null => Ok(Cirru::Leaf(Arc::from("nil"))),
    serde_json::Value::Object(_) => Err(CalcitErr::from("cirru-format: JSON objects are not supported".to_string())),
  }
}
