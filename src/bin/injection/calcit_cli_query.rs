//! Query/search builtins: pkg, ns, tags, validate, project search, host procs.

use calcit::builtins;
use calcit::calcit::{Calcit, CalcitErr};
use calcit::call_stack::CallStackList;
use cirru_edn::EdnTag;
use cirru_parser::Cirru;
use std::sync::Arc;

use super::calcit_cli::{
  calcit_str_list, format_path, get_def, load_calcit_snapshot, load_calcit_snapshot_with_deps, navigate_to_node, preview_leaf,
};
use super::calcit_cli_args::resolve_cli_args;
use super::calcit_cli_program::validate_snapshot_file;
use super::calcit_cli_specs::{
  LIST_DEFS_BY_TAG, LIST_HOST_PROCS, SEARCH_DEF_REGEX, SEARCH_EXPR, SEARCH_PROJECT, SHOW_NS, SHOW_PKG, VALIDATE_FILE,
};
use super::calcit_cli_tree::find_regex_leaf_paths;

pub fn show_pkg(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/show-pkg", &xs, SHOW_PKG)?;
  let file_path = args.string("file-path")?;
  let snapshot = load_calcit_snapshot_with_deps(&file_path, true)?;
  Ok(Calcit::Str(Arc::from(snapshot.package.as_str())))
}

pub fn show_ns(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/show-ns", &xs, SHOW_NS)?;
  let file_path = args.string("file-path")?;
  let ns_name = args.string("namespace")?;
  let snapshot = load_calcit_snapshot_with_deps(&file_path, true)?;
  let file = snapshot
    .files
    .get(&ns_name)
    .ok_or_else(|| CalcitErr::from(format!("show-ns: namespace `{ns_name}` not found")))?;

  let mut out = String::new();
  if !file.ns.doc.is_empty() {
    out.push_str(&format!("doc: {}\n", file.ns.doc));
  }
  out.push_str("ns:\n");
  out.push_str(&cirru_parser::format(&[file.ns.code.clone()], true.into()).unwrap_or_else(|_| "(failed to format)".to_string()));
  out.push_str(&format!("\ndefs: {}", file.defs.len()));
  Ok(Calcit::Str(Arc::from(out)))
}

pub fn list_defs_by_tag(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/list-defs-by-tag", &xs, LIST_DEFS_BY_TAG)?;
  let file_path = args.string("file-path")?;
  let tag_raw = args.string("tag")?;
  let tag = parse_tag("list-defs-by-tag", &tag_raw)?;
  let filter_ns = args.optional_string("namespace");

  let snapshot = load_calcit_snapshot_with_deps(&file_path, true)?;
  let pkg = snapshot.package.clone();
  let mut results = Vec::new();

  for (ns_name, file) in &snapshot.files {
    if ns_name != &pkg && !ns_name.starts_with(&format!("{pkg}.")) {
      continue;
    }
    if let Some(want) = &filter_ns
      && ns_name != want
    {
      continue;
    }
    for (def_name, entry) in &file.defs {
      if entry.tags.contains(&tag) {
        results.push(format!("{ns_name}/{def_name}"));
      }
    }
  }
  results.sort();
  Ok(calcit_str_list(results))
}

pub fn validate_file(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/validate-file", &xs, VALIDATE_FILE)?;
  let file_path = args.string("file-path")?;
  let status = validate_snapshot_file(&file_path)?;
  Ok(Calcit::Str(Arc::from(status)))
}

pub fn search_project(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/search-project", &xs, SEARCH_PROJECT)?;
  let file_path = args.string("file-path")?;
  let pattern = args.string("pattern")?;
  let filter = args.optional_string("filter");
  let exact = args.bool("exact")?;
  let max_depth = args.usize("max-depth")?;

  let snapshot = load_calcit_snapshot_with_deps(&file_path, true)?;
  let (filter_ns, filter_def) = parse_filter(&filter)?;
  let pkg = snapshot.package.clone();
  let mut lines = Vec::new();

  for (ns, file) in &snapshot.files {
    if ns != &pkg && !ns.starts_with(&format!("{pkg}.")) {
      continue;
    }
    if let Some(want) = &filter_ns
      && ns != want
    {
      continue;
    }
    for (def_name, entry) in &file.defs {
      if let Some(want) = &filter_def
        && def_name != want
      {
        continue;
      }
      let matches = search_leaf_nodes(&entry.code, &pattern, !exact, false, max_depth, &[]);
      for (path, node) in matches {
        let preview = match node {
          Cirru::Leaf(s) => preview_leaf(s.as_ref()),
          other => format!("({})", other.format_one_liner().unwrap_or_default()),
        };
        lines.push(format!("{ns}/{def_name} {} {}", format_path(&path), preview));
      }
    }
  }
  Ok(Calcit::Str(Arc::from(lines.join("\n"))))
}

pub fn search_def_regex(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/search-def-regex", &xs, SEARCH_DEF_REGEX)?;
  let file_path = args.string("file-path")?;
  let (ns_name, def_name) = args.target("target")?;
  let pattern_raw = args.string("regex")?;

  let re =
    regex::Regex::new(&pattern_raw).map_err(|e| CalcitErr::from(format!("search-def-regex: invalid regex `{pattern_raw}`: {e}")))?;

  let snapshot = load_calcit_snapshot(&file_path)?;
  let entry = get_def(&snapshot, &ns_name, &def_name)?;
  let mut paths = Vec::new();
  find_regex_leaf_paths(&entry.code, &re, &mut vec![], &mut paths);

  let mut lines: Vec<String> = paths
    .iter()
    .map(|path| {
      let node = navigate_to_node(&entry.code, path).expect("path from search");
      let preview = match node {
        Cirru::Leaf(s) => preview_leaf(s.as_ref()),
        other => other.format_one_liner().unwrap_or_default(),
      };
      format!("{} {}", format_path(path), preview)
    })
    .collect();
  lines.sort();
  Ok(Calcit::Str(Arc::from(lines.join("\n"))))
}

pub fn search_expr(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/search-expr", &xs, SEARCH_EXPR)?;
  let file_path = args.string("file-path")?;
  let pattern_raw = args.string("pattern")?;
  let filter = args.optional_string("filter");
  let json = args.bool("json")?;
  let exact = args.bool("exact")?;
  let max_depth = args.usize("max-depth")?;

  let pattern_node = if json {
    let json_val: serde_json::Value =
      serde_json::from_str(&pattern_raw).map_err(|e| CalcitErr::from(format!("search-expr: invalid JSON pattern: {e}")))?;
    json_to_cirru(&json_val)?
  } else {
    cirru_parser::parse(&pattern_raw)
      .map_err(|e| CalcitErr::from(format!("search-expr: failed to parse Cirru pattern: {e}")))?
      .into_iter()
      .next()
      .ok_or_else(|| CalcitErr::from("search-expr: pattern is empty".to_string()))?
  };

  let snapshot = load_calcit_snapshot_with_deps(&file_path, true)?;
  let (filter_ns, filter_def) = parse_filter(&filter)?;
  let pkg = snapshot.package.clone();
  let mut lines = Vec::new();

  for (ns, file) in &snapshot.files {
    if ns != &pkg && !ns.starts_with(&format!("{pkg}.")) {
      continue;
    }
    if let Some(want) = &filter_ns
      && ns != want
    {
      continue;
    }
    for (def_name, entry) in &file.defs {
      if let Some(want) = &filter_def
        && def_name != want
      {
        continue;
      }
      let matches = search_expr_nodes(&entry.code, &pattern_node, !exact, max_depth, &[]);
      for (path, node) in matches {
        let preview = node.format_one_liner().unwrap_or_default();
        lines.push(format!("{ns}/{def_name} {} {}", format_path(&path), preview));
      }
    }
  }
  Ok(Calcit::Str(Arc::from(lines.join("\n"))))
}

pub fn list_host_procs(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/list-host-procs", &xs, LIST_HOST_PROCS)?;
  let filter_tag = args
    .optional_string("tag")
    .map(|raw| parse_tag("list-host-procs", &raw))
    .transpose()?;

  let mut items = builtins::list_registered_procs();
  if let Some(tag) = &filter_tag {
    items.retain(|(_, descriptor)| descriptor.tags.contains(tag));
  }

  let mut lines: Vec<String> = items
    .into_iter()
    .map(|(name, descriptor)| {
      let tags = if descriptor.tags.is_empty() {
        "-".to_string()
      } else {
        let mut tag_names: Vec<String> = descriptor.tags.iter().map(|t| format!(":{}", t.ref_str())).collect();
        tag_names.sort();
        tag_names.join(",")
      };
      format!("{name} {tags}")
    })
    .collect();
  lines.sort();
  Ok(Calcit::Str(Arc::from(lines.join("\n"))))
}

fn parse_tag(fn_name: &str, raw: &str) -> Result<EdnTag, CalcitErr> {
  let trimmed = raw.trim();
  if trimmed.is_empty() {
    return Err(CalcitErr::from(format!("{fn_name}: empty tag")));
  }
  let name = trimmed.strip_prefix(':').unwrap_or(trimmed);
  if name.is_empty() {
    return Err(CalcitErr::from(format!("{fn_name}: invalid tag `{raw}`")));
  }
  Ok(EdnTag::new(name))
}

fn parse_filter(filter: &Option<String>) -> Result<(Option<String>, Option<String>), CalcitErr> {
  let Some(f) = filter else {
    return Ok((None, None));
  };
  if f.contains('/') {
    let (ns, def) = f
      .split_once('/')
      .ok_or_else(|| CalcitErr::from(format!("search: invalid filter `{f}`")))?;
    Ok((Some(ns.to_string()), Some(def.to_string())))
  } else {
    Ok((Some(f.clone()), None))
  }
}

fn search_leaf_nodes(
  node: &Cirru,
  pattern: &str,
  loose: bool,
  regex: bool,
  max_depth: usize,
  current_path: &[usize],
) -> Vec<(Vec<usize>, Cirru)> {
  let mut results = Vec::new();
  if max_depth > 0 && current_path.len() >= max_depth {
    return results;
  }

  let regex_pattern = if regex { regex::Regex::new(pattern).ok() } else { None };

  match node {
    Cirru::Leaf(s) => {
      let matched = if regex {
        regex_pattern.as_ref().is_some_and(|r| r.is_match(s))
      } else if loose {
        s.to_lowercase().contains(&pattern.to_lowercase())
      } else {
        s.as_ref() == pattern
      };
      if matched {
        results.push((current_path.to_vec(), node.clone()));
      }
    }
    Cirru::List(items) => {
      for (i, item) in items.iter().enumerate() {
        let mut new_path = current_path.to_vec();
        new_path.push(i);
        results.extend(search_leaf_nodes(item, pattern, loose, regex, max_depth, &new_path));
      }
    }
  }
  results
}

fn search_expr_nodes(node: &Cirru, pattern: &Cirru, loose: bool, max_depth: usize, current_path: &[usize]) -> Vec<(Vec<usize>, Cirru)> {
  let mut results = Vec::new();
  if max_depth > 0 && current_path.len() >= max_depth {
    return results;
  }

  let matched = if loose {
    contains_pattern(node, pattern)
  } else {
    matches_exact_structure(node, pattern)
  };
  if matched {
    results.push((current_path.to_vec(), node.clone()));
  }

  if let Cirru::List(items) = node {
    for (i, item) in items.iter().enumerate() {
      let mut new_path = current_path.to_vec();
      new_path.push(i);
      results.extend(search_expr_nodes(item, pattern, loose, max_depth, &new_path));
    }
  }
  results
}

fn contains_pattern(node: &Cirru, pattern: &Cirru) -> bool {
  match (node, pattern) {
    (Cirru::Leaf(s), Cirru::Leaf(p)) => s.to_lowercase().contains(&p.as_ref().to_lowercase()),
    (Cirru::List(items), Cirru::List(pattern_items)) => {
      if pattern_items.is_empty() {
        return true;
      }
      if pattern_items.len() > items.len() {
        return false;
      }
      pattern_items
        .iter()
        .enumerate()
        .all(|(i, pattern_item)| matches_prefix_structure(&items[i], pattern_item))
    }
    _ => false,
  }
}

fn matches_prefix_structure(node: &Cirru, pattern: &Cirru) -> bool {
  match (node, pattern) {
    (Cirru::Leaf(s1), Cirru::Leaf(s2)) => s1.as_ref() == s2.as_ref(),
    (Cirru::List(items1), Cirru::List(items2)) => {
      if items2.len() > items1.len() {
        return false;
      }
      items2
        .iter()
        .enumerate()
        .all(|(i, pattern_item)| matches_prefix_structure(&items1[i], pattern_item))
    }
    _ => false,
  }
}

fn matches_exact_structure(node: &Cirru, pattern: &Cirru) -> bool {
  match (node, pattern) {
    (Cirru::Leaf(s1), Cirru::Leaf(s2)) => s1.as_ref() == s2.as_ref(),
    (Cirru::List(items1), Cirru::List(items2)) => {
      items1.len() == items2.len() && items1.iter().zip(items2.iter()).all(|(n1, n2)| matches_exact_structure(n1, n2))
    }
    _ => false,
  }
}

fn json_to_cirru(value: &serde_json::Value) -> Result<Cirru, CalcitErr> {
  match value {
    serde_json::Value::String(s) => Ok(Cirru::Leaf(Arc::from(s.as_str()))),
    serde_json::Value::Array(items) => {
      let mut out = Vec::with_capacity(items.len());
      for item in items {
        out.push(json_to_cirru(item)?);
      }
      Ok(Cirru::List(out))
    }
    other => Err(CalcitErr::from(format!("search-expr: unsupported JSON pattern node: {other}"))),
  }
}
