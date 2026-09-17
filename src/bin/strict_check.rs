use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashSet};

use calcit::ProgramEntries;
use calcit::calcit::{CalcitErr, LocatedWarning};
use calcit::call_stack::CallStackList;
use calcit::call_tree::{CallTreeAnalyzer, CallTreeConfig, CallTreeNode};
use calcit::runner;
use serde::Serialize;

use crate::cli_handlers::{StructuredOutputFormat, format_json_value_as_edn};

type DefinitionGraph = BTreeMap<String, BTreeSet<String>>;

#[derive(Debug, Clone, Serialize)]
struct CheckDiagnostic {
  severity: &'static str,
  code: Option<String>,
  message: String,
  definition: String,
  path: Vec<u16>,
  hint: Option<String>,
  expected: Option<String>,
  actual: Option<String>,
  provenance: Vec<calcit::calcit::CalcitErrProvenance>,
}

#[derive(Debug, Clone, Serialize)]
struct DefinitionResult {
  definition: String,
  status: &'static str,
  blocked_by: Vec<String>,
  diagnostics: Vec<CheckDiagnostic>,
}

#[derive(Debug, Clone, Serialize)]
struct CheckSummary {
  total: usize,
  passed: usize,
  failed: usize,
  blocked: usize,
  cascaded: usize,
}

#[derive(Debug, Clone, Serialize)]
struct CheckData {
  status: &'static str,
  entries: Vec<String>,
  summary: CheckSummary,
  definitions: Vec<DefinitionResult>,
}

fn collect_graph(node: &CallTreeNode, graph: &mut DefinitionGraph) {
  let dependencies = graph.entry(node.fqn.clone()).or_default();
  for child in &node.calls {
    dependencies.insert(child.fqn.clone());
  }
  for child in &node.calls {
    collect_graph(child, graph);
  }
}

fn visit_definition(
  definition: &str,
  graph: &DefinitionGraph,
  visiting: &mut HashSet<String>,
  visited: &mut HashSet<String>,
  ordered: &mut Vec<String>,
) {
  if visited.contains(definition) || !visiting.insert(definition.to_owned()) {
    return;
  }
  if let Some(dependencies) = graph.get(definition) {
    for dependency in dependencies {
      visit_definition(dependency, graph, visiting, visited, ordered);
    }
  }
  visiting.remove(definition);
  if visited.insert(definition.to_owned()) {
    ordered.push(definition.to_owned());
  }
}

fn reachable_definitions(entries: &ProgramEntries) -> Result<(DefinitionGraph, Vec<String>), String> {
  let mut graph = BTreeMap::new();
  for (ns, definition) in [
    (entries.init_ns.as_ref(), entries.init_def.as_ref()),
    (entries.reload_ns.as_ref(), entries.reload_def.as_ref()),
  ] {
    let mut analyzer = CallTreeAnalyzer::new(CallTreeConfig::default());
    let result = analyzer.analyze(ns, definition)?;
    collect_graph(&result.tree, &mut graph);
  }

  let mut ordered = Vec::new();
  let mut visiting = HashSet::new();
  let mut visited = HashSet::new();
  for root in [entries.init_fn.as_ref(), entries.reload_fn.as_ref()] {
    visit_definition(root, &graph, &mut visiting, &mut visited, &mut ordered);
  }
  Ok((graph, ordered))
}

fn warning_diagnostic(warning: &LocatedWarning) -> CheckDiagnostic {
  let location = warning.location();
  CheckDiagnostic {
    severity: "warning",
    code: warning.code().map(str::to_owned),
    message: warning.message().to_owned(),
    definition: format!("{}/{}", location.ns, location.def),
    path: location.coord.to_vec(),
    hint: warning.hint().map(str::to_owned),
    expected: warning.expected().map(str::to_owned),
    actual: warning.actual().map(str::to_owned),
    provenance: Vec::new(),
  }
}

fn error_diagnostic(error: &CalcitErr, fallback: &str) -> CheckDiagnostic {
  let definition = error
    .location
    .as_ref()
    .map(|location| format!("{}/{}", location.ns, location.def))
    .unwrap_or_else(|| fallback.to_owned());
  let path = error.location.as_ref().map(|location| location.coord.to_vec()).unwrap_or_default();
  CheckDiagnostic {
    severity: "error",
    code: error.code.clone(),
    message: error.msg.clone(),
    definition,
    path,
    hint: error.hint.as_deref().map(str::to_owned),
    expected: error.provenance.first().map(|item| item.r#type.clone()),
    actual: error.provenance.first().map(|item| item.output_type.clone()),
    provenance: error.provenance.as_ref().clone(),
  }
}

fn check_definitions(graph: &DefinitionGraph, definitions: &[String]) -> Vec<DefinitionResult> {
  let mut statuses = BTreeMap::new();
  let mut results = Vec::with_capacity(definitions.len());

  for definition in definitions {
    let blocked_by = graph
      .get(definition)
      .into_iter()
      .flatten()
      .filter(|dependency| matches!(statuses.get(*dependency), Some(&"failed" | &"blocked" | &"cascaded")))
      .cloned()
      .collect::<Vec<_>>();
    if !blocked_by.is_empty() {
      statuses.insert(definition.clone(), "blocked");
      results.push(DefinitionResult {
        definition: definition.clone(),
        status: "blocked",
        blocked_by,
        diagnostics: Vec::new(),
      });
      continue;
    }

    let Some((ns, def)) = definition.split_once('/') else {
      statuses.insert(definition.clone(), "cascaded");
      results.push(DefinitionResult {
        definition: definition.clone(),
        status: "cascaded",
        blocked_by: Vec::new(),
        diagnostics: vec![CheckDiagnostic {
          severity: "error",
          code: Some("E_INVALID_DEFINITION_ID".to_owned()),
          message: format!("Invalid qualified definition `{definition}`"),
          definition: definition.clone(),
          path: Vec::new(),
          hint: None,
          expected: None,
          actual: None,
          provenance: Vec::new(),
        }],
      });
      continue;
    };

    let warnings = RefCell::new(Vec::new());
    match runner::preprocess::ensure_ns_def_compiled(ns, def, &warnings, &CallStackList::default()) {
      Ok(_) => {
        let diagnostics = warnings.borrow().iter().map(warning_diagnostic).collect::<Vec<_>>();
        let status = if diagnostics.is_empty() {
          "passed"
        } else if diagnostics.iter().any(|diagnostic| diagnostic.definition == *definition) {
          "failed"
        } else {
          "cascaded"
        };
        statuses.insert(definition.clone(), status);
        results.push(DefinitionResult {
          definition: definition.clone(),
          status,
          blocked_by: Vec::new(),
          diagnostics,
        });
      }
      Err(error) => {
        let belongs_to_definition = error
          .location
          .as_ref()
          .is_some_and(|location| format!("{}/{}", location.ns, location.def) == *definition);
        let diagnostic = error_diagnostic(&error, definition);
        let status = if belongs_to_definition { "failed" } else { "cascaded" };
        let mut diagnostics = error.warnings.iter().map(warning_diagnostic).collect::<Vec<_>>();
        diagnostics.push(diagnostic);
        statuses.insert(definition.clone(), status);
        results.push(DefinitionResult {
          definition: definition.clone(),
          status,
          blocked_by: Vec::new(),
          diagnostics,
        });
      }
    }
  }
  results
}

fn print_human(data: &CheckData) {
  println!("# Strict check\n");
  println!("- status: **{}**", data.status.to_uppercase());
  println!(
    "- entries: {}",
    data.entries.iter().map(|entry| format!("`{entry}`")).collect::<Vec<_>>().join(", ")
  );
  println!("- definitions: {}", data.summary.total);
  println!("- failed: {}", data.summary.failed);
  println!("- blocked: {}", data.summary.blocked);
  println!("- cascaded: {}", data.summary.cascaded);
  for result in &data.definitions {
    println!("\n## `{}`\n", result.definition);
    println!("- status: **{}**", result.status.to_uppercase());
    if !result.blocked_by.is_empty() {
      println!(
        "- blocked by: {}",
        result
          .blocked_by
          .iter()
          .map(|item| format!("`{item}`"))
          .collect::<Vec<_>>()
          .join(", ")
      );
    }
    for diagnostic in &result.diagnostics {
      let code = diagnostic.code.as_deref().unwrap_or("unclassified");
      println!("- {} `{code}`", diagnostic.severity);
      println!("\n```text\n{}\n```", diagnostic.message);
      if let Some(expected) = &diagnostic.expected {
        println!("\n- expected: `{expected}`");
      }
      if let Some(actual) = &diagnostic.actual {
        println!("- actual: `{actual}`");
      }
      if let Some(hint) = &diagnostic.hint {
        println!("\n> {hint}");
      }
    }
  }
}

pub fn run(entries: &ProgramEntries, raw_format: &str) -> Result<(), String> {
  let format = StructuredOutputFormat::parse(raw_format, "check-only")?;
  let (graph, definitions) = reachable_definitions(entries)?;
  let results = check_definitions(&graph, &definitions);
  let summary = CheckSummary {
    total: results.len(),
    passed: results.iter().filter(|result| result.status == "passed").count(),
    failed: results.iter().filter(|result| result.status == "failed").count(),
    blocked: results.iter().filter(|result| result.status == "blocked").count(),
    cascaded: results.iter().filter(|result| result.status == "cascaded").count(),
  };
  let passed = summary.failed == 0 && summary.blocked == 0 && summary.cascaded == 0;
  let data = CheckData {
    status: if passed { "passed" } else { "failed" },
    entries: vec![entries.init_fn.to_string(), entries.reload_fn.to_string()],
    summary,
    definitions: results,
  };

  let envelope = serde_json::json!({
    "schema_version": 1,
    "command": "check-only",
    "data": data,
    "diagnostics": [],
  });
  match format {
    StructuredOutputFormat::Human => print_human(&data),
    StructuredOutputFormat::Edn => println!("{}", format_json_value_as_edn(&envelope)?),
    StructuredOutputFormat::Json => println!("{envelope}"),
  }

  if passed {
    Ok(())
  } else {
    Err(format!(
      "Strict check found {} failed, {} blocked, and {} cascaded definition(s)",
      data.summary.failed, data.summary.blocked, data.summary.cascaded
    ))
  }
}
