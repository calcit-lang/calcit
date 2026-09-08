use std::cell::RefCell;
use std::collections::{BTreeSet, HashSet};
use std::time::Instant;

use calcit::calcit::{CalcitErr, LocatedWarning};
use calcit::call_stack::CallStackList;
use calcit::cli_args::CheckPublicCommand;
use calcit::runner;
use calcit::snapshot::{self, SnapshotTarget};
use serde_json::{Value, json};

use crate::type_coverage;

#[derive(Debug)]
struct DefinitionResult {
  id: String,
  namespace: String,
  definition: String,
  kind: &'static str,
  declared_target: Option<SnapshotTarget>,
  status: &'static str,
}

fn definition_target(entry: &snapshot::CodeEntry) -> Result<Option<SnapshotTarget>, String> {
  let Some(ffi) = &entry.ffi else {
    return Ok(None);
  };
  snapshot::parse_ffi_target(ffi)
}

fn error_diagnostic(error: &CalcitErr, definition_id: &str) -> Value {
  json!({
    "code": error.code.as_deref().unwrap_or("E_PUBLIC_CHECK_PREPROCESS"),
    "phase": "preprocess",
    "severity": "error",
    "definition": definition_id,
    "kind": error.kind.to_string().to_lowercase(),
    "message": error.headline(),
    "location": error.location.as_ref().map(|location| json!({
      "ns": location.ns.to_string(),
      "def": location.def.to_string(),
      "coord": location.coord.to_vec(),
    })),
    "hint": error.hint,
  })
}

fn warning_diagnostic(warning: &LocatedWarning) -> Value {
  let mut row = warning.as_json();
  if let Value::Object(fields) = &mut row {
    fields.insert("phase".to_owned(), json!("preprocess"));
    fields.insert("severity".to_owned(), json!("warning"));
  }
  row
}

fn result_json(row: &DefinitionResult) -> Value {
  json!({
    "id": row.id,
    "namespace": row.namespace,
    "definition": row.definition,
    "kind": row.kind,
    "declared_target": row.declared_target.map(SnapshotTarget::as_str),
    "status": row.status,
  })
}

pub fn run(options: &CheckPublicCommand, snapshot: &snapshot::Snapshot, project_namespaces: &HashSet<String>) -> Result<(), String> {
  if !matches!(options.format.as_str(), "human" | "text" | "json") {
    return Err(format!(
      "Unknown check-public output format `{}`. Expected `human` or `json`.",
      options.format
    ));
  }

  let started = Instant::now();
  let requested_namespaces = options.ns.iter().cloned().collect::<BTreeSet<_>>();
  let active_target = snapshot.active_entry()?.target;
  let mut diagnostics = Vec::<Value>::new();
  let mut selected = Vec::<(String, String, &snapshot::CodeEntry)>::new();

  if requested_namespaces.is_empty() {
    diagnostics.push(json!({
      "code": "E_PUBLIC_CHECK_SCOPE_REQUIRED",
      "phase": "selection",
      "severity": "error",
      "message": "check-public requires at least one --ns namespace.",
    }));
  }
  if active_target.is_none() {
    diagnostics.push(json!({
      "code": "E_PUBLIC_CHECK_TARGET_REQUIRED",
      "phase": "selection",
      "severity": "error",
      "message": format!("Entry `{}` has no :target; public checks require an explicit browser, node, native, or wasm target.", snapshot.active_entry_name()),
    }));
  }

  for namespace in &requested_namespaces {
    let Some(file) = snapshot.files.get(namespace) else {
      diagnostics.push(json!({
        "code": "E_PUBLIC_CHECK_NAMESPACE_NOT_FOUND",
        "phase": "selection",
        "severity": "error",
        "namespace": namespace,
        "message": format!("Selected namespace `{namespace}` is not loaded by entry `{}`.", snapshot.active_entry_name()),
      }));
      continue;
    };
    if !options.deps && !project_namespaces.contains(namespace) {
      diagnostics.push(json!({
        "code": "E_PUBLIC_CHECK_DEPENDENCY_SCOPE",
        "phase": "selection",
        "severity": "error",
        "namespace": namespace,
        "message": format!("Selected namespace `{namespace}` belongs to a dependency; pass --deps to admit it explicitly."),
      }));
      continue;
    }
    if file.defs.is_empty() {
      diagnostics.push(json!({
        "code": "E_PUBLIC_CHECK_EMPTY_NAMESPACE",
        "phase": "selection",
        "severity": "error",
        "namespace": namespace,
        "message": format!("Selected namespace `{namespace}` contains no definitions; zero coverage is not a successful check."),
      }));
      continue;
    }
    let mut definitions = file.defs.iter().collect::<Vec<_>>();
    definitions.sort_by_key(|(definition, _)| *definition);
    selected.extend(
      definitions
        .into_iter()
        .map(|(definition, entry)| (namespace.clone(), definition.clone(), entry)),
    );
  }

  let revision_ids = selected
    .iter()
    .map(|(namespace, definition, _)| (namespace.clone(), definition.clone()))
    .collect::<Vec<_>>();
  let selected_count = revision_ids.len();
  let selection_complete = diagnostics.is_empty();
  let revision = type_coverage::analysis_revision(snapshot, &revision_ids)?;
  let warnings = RefCell::new(Vec::<LocatedWarning>::new());
  let mut results = Vec::<DefinitionResult>::new();
  let mut checked_definition_ids = Vec::<String>::new();

  if let Some(target) = active_target {
    for (namespace, definition, entry) in selected {
      let id = format!("{namespace}/{definition}");
      let kind = type_coverage::analyze_code_entry(&namespace, &definition, entry).kind.as_str();
      let declared_target = match definition_target(entry) {
        Ok(value) => value,
        Err(message) => {
          diagnostics.push(json!({
            "code": "E_PUBLIC_CHECK_INVALID_TARGET",
            "phase": "target",
            "severity": "error",
            "definition": id,
            "message": message,
          }));
          results.push(DefinitionResult {
            id,
            namespace,
            definition,
            kind,
            declared_target: None,
            status: "rejected",
          });
          continue;
        }
      };
      if let Some(expected) = declared_target
        && expected != target
      {
        diagnostics.push(json!({
          "code": "E_JS_FFI_TARGET_MISMATCH",
          "phase": "target",
          "severity": "error",
          "definition": id,
          "message": format!("Definition `{id}` requires `{}` target, but entry `{}` targets `{}`.", expected.as_str(), snapshot.active_entry_name(), target.as_str()),
        }));
        results.push(DefinitionResult {
          id,
          namespace,
          definition,
          kind,
          declared_target,
          status: "rejected",
        });
        continue;
      }

      checked_definition_ids.push(id.clone());
      let warning_count = warnings.borrow().len();
      let result = runner::preprocess::ensure_ns_def_compiled(&namespace, &definition, &warnings, &CallStackList::default());
      let status = match result {
        Ok(()) if warnings.borrow().len() == warning_count => "passed",
        Ok(()) => "warning",
        Err(error) => {
          diagnostics.push(error_diagnostic(&error, &id));
          diagnostics.extend(error.warnings.iter().map(warning_diagnostic));
          "failed"
        }
      };
      results.push(DefinitionResult {
        id,
        namespace,
        definition,
        kind,
        declared_target,
        status,
      });
    }
  }

  diagnostics.extend(warnings.borrow().iter().map(warning_diagnostic));
  let checked_count = checked_definition_ids.len();
  let passed_count = results.iter().filter(|row| row.status == "passed").count();
  let complete = selection_complete && selected_count > 0 && checked_count == selected_count;
  let passed = complete && passed_count == selected_count && diagnostics.is_empty();
  if selected_count == 0 && diagnostics.is_empty() {
    diagnostics.push(json!({
      "code": "E_PUBLIC_CHECK_EMPTY_SCOPE",
      "phase": "selection",
      "severity": "error",
      "message": "The selected public namespace scope contains no definitions; zero coverage is not a successful check.",
    }));
  }
  let duration_ms = started.elapsed().as_secs_f64() * 1000.0;

  match options.format.as_str() {
    "json" => println!(
      "{}",
      json!({
        "schema_version": 1,
        "command": "analyze.check-public",
        "revision": revision,
        "data": {
          "entry": snapshot.active_entry_name(),
          "target": active_target.map(SnapshotTarget::as_str),
          "filters": {
            "namespaces": requested_namespaces,
            "include_dependencies": options.deps,
            "summary_only": options.summary_only,
          },
          "summary": {
            "definitions_selected": selected_count,
            "definitions_checked": checked_count,
            "definitions_passed": passed_count,
            "complete": complete,
            "passed": passed,
            "duration_ms": duration_ms,
          },
          "checked_definition_ids": checked_definition_ids,
          "definitions": if options.summary_only { Vec::new() } else { results.iter().map(result_json).collect::<Vec<_>>() },
        },
        "diagnostics": diagnostics,
      })
    ),
    "human" | "text" => {
      println!("Public definition check");
      println!("- entry: {}", snapshot.active_entry_name());
      println!("- target: {}", active_target.map(SnapshotTarget::as_str).unwrap_or("missing"));
      println!(
        "- namespaces: {}",
        requested_namespaces.iter().cloned().collect::<Vec<_>>().join(", ")
      );
      println!("- revision: {revision}");
      println!("- coverage: {checked_count}/{selected_count} definitions checked");
      println!("- result: {} ({duration_ms:.3}ms)", if passed { "PASS" } else { "FAIL" });
      if !options.summary_only {
        for row in &results {
          println!("- [{}] {} ({})", row.status, row.id, row.kind);
        }
      }
      for diagnostic in &diagnostics {
        println!("- diagnostic: {}", diagnostic["message"].as_str().unwrap_or("unknown diagnostic"));
      }
    }
    _ => unreachable!("check-public output format was validated before analysis"),
  }

  if passed {
    Ok(())
  } else {
    Err(format!(
      "Public definition check failed: {checked_count}/{selected_count} definitions checked, {passed_count} passed, {} diagnostic(s).",
      diagnostics.len()
    ))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::sync::Arc;

  use calcit::calcit::DYNAMIC_TYPE;
  use cirru_edn::Edn;
  use cirru_parser::Cirru;

  fn entry_with_ffi(ffi: Option<Edn>) -> snapshot::CodeEntry {
    snapshot::CodeEntry {
      doc: String::new(),
      examples: vec![],
      tests: vec![],
      tags: HashSet::new(),
      code: Cirru::Leaf(Arc::from("placeholder")),
      schema: DYNAMIC_TYPE.clone(),
      ffi,
    }
  }

  #[test]
  fn parses_definition_targets_and_rejects_unknown_values() {
    let ffi = Edn::map_from_iter([(Edn::tag("target"), Edn::tag("browser"))]);
    assert_eq!(
      definition_target(&entry_with_ffi(Some(ffi))).unwrap(),
      Some(SnapshotTarget::Browser)
    );

    let ffi = Edn::map_from_iter([(Edn::tag("target"), Edn::tag("desktop"))]);
    assert!(definition_target(&entry_with_ffi(Some(ffi))).is_err());
    assert_eq!(definition_target(&entry_with_ffi(None)).unwrap(), None);
  }
}
