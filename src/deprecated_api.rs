//! Deprecated API usage analysis for `calcit analyze deprecated`.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Write;
use std::sync::Arc;

use calcit::calcit::{Calcit, CalcitSyntax};
use calcit::cli_args::DeprecatedCommand;
use calcit::data::cirru::{reader_path_to_source_path, source_path_to_reader_path};
use calcit::{program, snapshot};
use cirru_parser::Cirru;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeprecatedApiUse {
  pub path: String,
  pub target_namespace: String,
  pub target_name: String,
  pub target_doc: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeprecatedApiRow {
  pub namespace: String,
  pub definition: String,
  pub uses: Vec<DeprecatedApiUse>,
}

#[derive(Debug, Clone)]
struct DeprecatedTarget {
  doc: String,
}

fn is_selected_namespace(options: &DeprecatedCommand, snapshot: &snapshot::Snapshot, namespace: &str) -> Result<bool, String> {
  if let Some(expected) = &options.ns
    && !snapshot.files.contains_key(expected)
  {
    return Err(format!("Namespace not found: {expected}"));
  }

  if options.ns.as_deref().is_some_and(|expected| namespace != expected) {
    return Ok(false);
  }
  if options.ns_prefix.as_deref().is_some_and(|prefix| !namespace.starts_with(prefix)) {
    return Ok(false);
  }

  let explicit_scope = options.ns.is_some() || options.ns_prefix.is_some();
  if !explicit_scope && namespace.ends_with(".$meta") {
    return Ok(false);
  }
  if options.deps || explicit_scope {
    return Ok(true);
  }

  let package_prefix = format!("{}.", snapshot.package);
  Ok(namespace == snapshot.package || namespace.starts_with(&package_prefix))
}

fn deprecated_targets(snapshot: &snapshot::Snapshot) -> HashMap<(String, String), DeprecatedTarget> {
  let mut targets = HashMap::new();
  for (namespace, file) in &snapshot.files {
    for (definition, entry) in &file.defs {
      if entry.tags.iter().any(|tag| tag.ref_str() == "deprecated") {
        targets.insert((namespace.clone(), definition.clone()), DeprecatedTarget { doc: entry.doc.clone() });
      }
    }
  }
  targets
}

fn push_use(
  uses: &mut Vec<DeprecatedApiUse>,
  targets: &HashMap<(String, String), DeprecatedTarget>,
  namespace: &str,
  definition: &str,
  path: &[usize],
) {
  if let Some(target) = targets.get(&(namespace.to_owned(), definition.to_owned())) {
    uses.push(DeprecatedApiUse {
      path: format_path(path),
      target_namespace: namespace.to_owned(),
      target_name: definition.to_owned(),
      target_doc: target.doc.clone(),
    });
  }
}

fn collect_uses(
  node: &Calcit,
  current_namespace: &str,
  targets: &HashMap<(String, String), DeprecatedTarget>,
  definitions: &HashSet<(String, String)>,
  path: &mut Vec<usize>,
  uses: &mut Vec<DeprecatedApiUse>,
) {
  let Calcit::List(items) = node else {
    return;
  };

  // Quoted source is data, not an API invocation or quality-budget debt.
  if matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::Quote, _)))
    || matches!(items.first(), Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "quote" || sym.as_ref() == "calcit.core/quote")
  {
    return;
  }

  if let Some(head) = items.first() {
    match head {
      // Reader-resolved builtins have no Symbol/Import node left to inspect.
      Calcit::Proc(proc) => push_use(uses, targets, calcit::calcit::CORE_NS, proc.as_ref(), path),
      Calcit::Import(import) => push_use(uses, targets, import.ns.as_ref(), import.def.as_ref(), path),
      Calcit::Symbol { sym, .. } => {
        if let Some((namespace, definition)) = sym.rsplit_once('/') {
          let resolved_namespace = program::lookup_ns_target_in_import(current_namespace, namespace);
          push_use(uses, targets, resolved_namespace.as_deref().unwrap_or(namespace), definition, path);
        } else {
          push_use(uses, targets, current_namespace, sym.as_ref(), path);
          if let Some(imported_namespace) = program::lookup_def_target_in_import(current_namespace, sym.as_ref()) {
            push_use(uses, targets, imported_namespace.as_ref(), sym.as_ref(), path);
          } else if !definitions.contains(&(current_namespace.to_owned(), sym.to_string())) {
            push_use(uses, targets, calcit::calcit::CORE_NS, sym.as_ref(), path);
          }
        }
      }
      _ => {}
    }
  }

  for (index, item) in items.iter().enumerate() {
    path.push(index);
    collect_uses(item, current_namespace, targets, definitions, path, uses);
    path.pop();
  }
}

fn format_path(path: &[usize]) -> String {
  if path.is_empty() {
    "code".to_owned()
  } else {
    format!("code@{}", path.iter().map(usize::to_string).collect::<Vec<_>>().join("."))
  }
}

fn parse_path(path: &str) -> Option<Vec<usize>> {
  if path == "code" {
    return Some(vec![]);
  }
  path.strip_prefix("code@")?.split('.').map(|index| index.parse().ok()).collect()
}

fn source_node_at<'a>(mut node: &'a Calcit, path: &[usize]) -> Option<&'a Calcit> {
  for index in path {
    let Calcit::List(children) = node else { return None };
    node = children.get(*index)?;
  }
  Some(node)
}

/// Retain reader-resolved proc calls and use the compiler resolver for symbol calls.
/// Failed source proofs keep the existing conservative static report.
fn resolved_function_uses(
  node: &Calcit,
  source: &Cirru,
  namespace: &str,
  definition: &str,
  targets: &HashMap<(String, String), DeprecatedTarget>,
  candidates: &[DeprecatedApiUse],
) -> Option<Vec<DeprecatedApiUse>> {
  let Calcit::List(items) = node else { return None };
  if !matches!(items.first(), Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "defn") {
    return None;
  }
  let warnings = RefCell::new(vec![]);
  let references = calcit::runner::preprocess::trace_source_usages(
    node,
    namespace,
    definition,
    &warnings,
    &calcit::call_stack::CallStackList::default(),
  )
  .ok()?;
  let mut uses = candidates
    .iter()
    .filter(|usage| {
      let Some(path) = parse_path(&usage.path) else {
        return false;
      };
      matches!(source_node_at(node, &path), Some(Calcit::List(children)) if matches!(children.first(), Some(Calcit::Proc(_))))
    })
    .cloned()
    .collect::<Vec<_>>();
  for reference in references {
    let Some(location) = reference.location else { continue };
    if location.ns.as_ref() != namespace || location.def.as_ref() != definition {
      continue;
    }
    // Resolver locations are Snapshot source coordinates; candidates use reader paths.
    let source_coord = location.coord.iter().map(|index| usize::from(*index)).collect::<Vec<_>>();
    let Some(mut path) = source_path_to_reader_path(source, &source_coord) else {
      continue;
    };
    // A preferred method may legitimately lower to a legacy internal helper.
    // Only explicit function calls represent legacy source spelling here.
    if !matches!(source_node_at(node, &path), Some(Calcit::Symbol { .. } | Calcit::Import(_))) {
      continue;
    }
    if path.pop() != Some(0) {
      continue;
    }
    push_use(
      &mut uses,
      targets,
      reference.target_ns.as_ref(),
      reference.target_def.as_ref(),
      &path,
    );
  }
  Some(uses)
}

pub fn collect_deprecated_api_rows(
  options: &DeprecatedCommand,
  snapshot: &snapshot::Snapshot,
) -> Result<Vec<DeprecatedApiRow>, String> {
  let targets = deprecated_targets(snapshot);
  let program_data = program::extract_program_data(snapshot)?;
  struct ResolutionGuard(bool);
  impl Drop for ResolutionGuard {
    fn drop(&mut self) {
      calcit::runner::preprocess::set_strict_types(self.0);
      let _ = program::clear_runtime_caches_for_reload(Arc::from("analyze.deprecated"), Arc::from("analyze.deprecated"), true);
    }
  }
  let _guard = ResolutionGuard(calcit::runner::preprocess::is_strict_types_enabled());
  calcit::runner::preprocess::set_strict_types(false);
  let resolver_ready = !targets.is_empty() && crate::cli_handlers::prepare_program_for_type_query(snapshot).is_ok();
  let definitions = program_data
    .iter()
    .flat_map(|(namespace, file)| {
      file
        .defs
        .keys()
        .map(move |definition| (namespace.to_string(), definition.to_string()))
    })
    .collect::<HashSet<_>>();
  let mut rows = vec![];

  for (namespace, file) in &program_data {
    if !is_selected_namespace(options, snapshot, namespace)? {
      continue;
    }
    for (definition, entry) in &file.defs {
      let source_code = snapshot
        .files
        .get(namespace.as_ref())
        .and_then(|file| file.defs.get(definition.as_ref()))
        .map(|entry| &entry.code);
      let mut uses = vec![];
      collect_uses(&entry.code, namespace, &targets, &definitions, &mut vec![], &mut uses);
      // Do not compile unrelated call graphs merely to confirm an empty report.
      // Their macros may have compile-time effects that a metadata query must not trigger.
      if resolver_ready
        && !uses.is_empty()
        && let Some(resolved) =
          source_code.and_then(|source| resolved_function_uses(&entry.code, source, namespace, definition, &targets, &uses))
      {
        uses = resolved;
      }
      // Report Snapshot source paths so `query def` / `tree` address the same node.
      if let Some(source) = source_code {
        for usage in &mut uses {
          if let Some(reader_path) = parse_path(&usage.path)
            && let Some(source_path) = reader_path_to_source_path(source, &reader_path)
          {
            usage.path = format_path(&source_path);
          }
        }
      }
      uses.sort_by(|left, right| {
        left
          .path
          .cmp(&right.path)
          .then(left.target_namespace.cmp(&right.target_namespace))
          .then(left.target_name.cmp(&right.target_name))
      });
      uses.dedup_by(|left, right| {
        left.path == right.path && left.target_namespace == right.target_namespace && left.target_name == right.target_name
      });
      if !uses.is_empty() {
        rows.push(DeprecatedApiRow {
          namespace: namespace.to_string(),
          definition: definition.to_string(),
          uses,
        });
      }
    }
  }

  rows.sort_by(|left, right| left.namespace.cmp(&right.namespace).then(left.definition.cmp(&right.definition)));
  Ok(rows)
}

pub fn format_deprecated_api_report(options: &DeprecatedCommand, snapshot: &snapshot::Snapshot) -> Result<String, String> {
  let rows = collect_deprecated_api_rows(options, snapshot)?;
  let hit_count = rows.iter().map(|row| row.uses.len()).sum::<usize>();
  if rows.is_empty() {
    return Ok("No deprecated API usage found in selected namespace scope.\n".to_owned());
  }

  let mut out = String::new();
  let _ = writeln!(out, "Deprecated API usage check");
  let _ = writeln!(out, "- definitions with hits: {}", rows.len());
  let _ = writeln!(out, "- calls: {hit_count}");
  if !options.summary_only {
    for row in &rows {
      let _ = writeln!(out, "\n{}/{}", row.namespace, row.definition);
      for usage in &row.uses {
        let _ = writeln!(
          out,
          "  - {}: {}/{} is deprecated{}",
          usage.path,
          usage.target_namespace,
          usage.target_name,
          if usage.target_doc.is_empty() {
            String::new()
          } else {
            format!("; {}", usage.target_doc.lines().next().unwrap_or_default())
          }
        );
      }
    }
  }
  let _ = writeln!(
    out,
    "\nMigration: replace deprecated calls with the API named in each target's documentation."
  );
  Ok(out)
}

pub fn format_deprecated_api_json(options: &DeprecatedCommand, snapshot: &snapshot::Snapshot) -> Result<String, String> {
  let rows = collect_deprecated_api_rows(options, snapshot)?;
  let hit_count = rows.iter().map(|row| row.uses.len()).sum::<usize>();
  let namespaces = rows.iter().map(|row| row.namespace.as_str()).collect::<BTreeSet<_>>();
  let targets = rows
    .iter()
    .flat_map(|row| row.uses.iter())
    .map(|usage| format!("{}/{}", usage.target_namespace, usage.target_name))
    .collect::<BTreeSet<_>>();
  let definitions = rows
    .iter()
    .filter(|_| !options.summary_only)
    .map(|row| {
      serde_json::json!({
        "id": format!("{}/{}", row.namespace, row.definition),
        "namespace": row.namespace,
        "name": row.definition,
        "uses": row.uses.iter().map(|usage| serde_json::json!({
          "path": usage.path,
          "target": format!("{}/{}", usage.target_namespace, usage.target_name),
          "documentation": usage.target_doc,
          "suggestion": "Replace this deprecated API using the target documentation.",
        })).collect::<Vec<_>>(),
      })
    })
    .collect::<Vec<_>>();
  let diagnostics = if hit_count == 0 {
    vec![]
  } else {
    vec![serde_json::json!({
      "code": "W_DEPRECATED_API",
      "phase": "analysis",
      "severity": "warning",
      "message": format!("{hit_count} deprecated API call(s) found in {} definition(s).", rows.len()),
      "suggestion": "Replace each call using the migration guidance in the deprecated API documentation.",
    })]
  };
  let envelope = serde_json::json!({
    "schema_version": 1,
    "command": "analyze.deprecated",
    "revision": crate::type_coverage::analysis_revision(
      snapshot,
      &rows.iter().map(|row| (row.namespace.clone(), row.definition.clone())).collect::<Vec<_>>(),
    )?,
    "data": {
      "filters": {
        "namespace": options.ns,
        "namespace_prefix": options.ns_prefix,
        "include_dependencies": options.deps,
        "summary_only": options.summary_only,
      },
      "summary": {
        "namespaces": namespaces.len(),
        "definitions": rows.len(),
        "calls": hit_count,
        "targets": targets.into_iter().collect::<Vec<_>>(),
      },
      "definitions": definitions,
    },
    "diagnostics": diagnostics,
  });
  serde_json::to_string_pretty(&envelope).map_err(|error| format!("Failed to encode deprecated API JSON: {error}"))
}

#[cfg(test)]
mod tests {
  use super::*;
  use calcit::data::cirru::code_to_calcit;

  #[test]
  fn finds_implicit_core_deprecated_call_at_body_path() {
    let expression = cirru_parser::parse("defn demo () $ record? nil")
      .expect("parse expression")
      .into_iter()
      .next()
      .expect("one expression");
    let code = code_to_calcit(&expression, "app.main", "demo", vec![]).expect("convert expression");
    let targets = HashMap::from([(
      (calcit::calcit::CORE_NS.to_owned(), "record?".to_owned()),
      DeprecatedTarget {
        doc: "Replace with struct?.".to_owned(),
      },
    )]);
    let mut uses = vec![];

    collect_uses(&code, "app.main", &targets, &HashSet::new(), &mut vec![], &mut uses);

    assert_eq!(uses.len(), 1);
    assert_eq!(uses[0].path, "code@3");
    assert_eq!(uses[0].target_namespace, calcit::calcit::CORE_NS);
    assert_eq!(uses[0].target_name, "record?");
  }

  #[test]
  fn ignores_core_deprecated_call_shadowed_by_local_definition() {
    let expression = cirru_parser::parse("defn demo () $ record? nil")
      .expect("parse expression")
      .into_iter()
      .next()
      .expect("one expression");
    let code = code_to_calcit(&expression, "app.main", "demo", vec![]).expect("convert expression");
    let targets = HashMap::from([(
      (calcit::calcit::CORE_NS.to_owned(), "record?".to_owned()),
      DeprecatedTarget {
        doc: "Replace with struct?.".to_owned(),
      },
    )]);
    let definitions = HashSet::from([("app.main".to_owned(), "record?".to_owned())]);
    let mut uses = vec![];

    collect_uses(&code, "app.main", &targets, &definitions, &mut vec![], &mut uses);

    assert!(uses.is_empty());
  }

  #[test]
  fn finds_deprecated_call_inside_list_valued_callee() {
    let expression = cirru_parser::parse("defn demo () $ (record? nil)")
      .expect("parse expression")
      .into_iter()
      .next()
      .expect("one expression");
    let code = code_to_calcit(&expression, "app.main", "demo", vec![]).expect("convert expression");
    let targets = HashMap::from([(
      (calcit::calcit::CORE_NS.to_owned(), "record?".to_owned()),
      DeprecatedTarget {
        doc: "Replace with struct?.".to_owned(),
      },
    )]);
    let mut uses = vec![];

    collect_uses(&code, "app.main", &targets, &HashSet::new(), &mut vec![], &mut uses);

    assert_eq!(uses.len(), 1);
    assert_eq!(uses[0].target_name, "record?");
  }

  #[test]
  fn distinguishes_reader_resolved_builtin_calls_from_quoted_data() {
    let expression = cirru_parser::parse("defn demo () (cpu-time) (quote (cpu-time))")
      .expect("parse expression")
      .into_iter()
      .next()
      .expect("one expression");
    let code = code_to_calcit(&expression, "app.main", "demo", vec![]).expect("convert expression");
    let targets = HashMap::from([(
      (calcit::calcit::CORE_NS.to_owned(), "cpu-time".to_owned()),
      DeprecatedTarget {
        doc: "Prefer monotonic-time-ms.".to_owned(),
      },
    )]);
    let mut uses = vec![];
    collect_uses(&code, "app.main", &targets, &HashSet::new(), &mut vec![], &mut uses);
    assert_eq!(uses.len(), 1);
    assert_eq!(uses[0].path, "code@3");
    assert_eq!(uses[0].target_name, "cpu-time");
  }
}
