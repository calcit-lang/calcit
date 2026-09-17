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

fn finish_definition(definition: &str, graph: &DefinitionGraph, visited: &mut HashSet<String>, finished: &mut Vec<String>) {
  if !visited.insert(definition.to_owned()) {
    return;
  }
  if let Some(dependencies) = graph.get(definition) {
    for dependency in dependencies {
      finish_definition(dependency, graph, visited, finished);
    }
  }
  finished.push(definition.to_owned());
}

fn collect_reverse_component(definition: &str, reverse: &DefinitionGraph, visited: &mut HashSet<String>, component: &mut Vec<String>) {
  if !visited.insert(definition.to_owned()) {
    return;
  }
  component.push(definition.to_owned());
  if let Some(dependents) = reverse.get(definition) {
    for dependent in dependents {
      collect_reverse_component(dependent, reverse, visited, component);
    }
  }
}

fn visit_component(component: usize, dependencies: &[BTreeSet<usize>], visited: &mut HashSet<usize>, ordered: &mut Vec<usize>) {
  if !visited.insert(component) {
    return;
  }
  for dependency in &dependencies[component] {
    visit_component(*dependency, dependencies, visited, ordered);
  }
  ordered.push(component);
}

fn strongly_connected_components(graph: &DefinitionGraph) -> Vec<Vec<String>> {
  let mut nodes = BTreeSet::new();
  for (definition, dependencies) in graph {
    nodes.insert(definition.clone());
    nodes.extend(dependencies.iter().cloned());
  }

  let mut finished = Vec::new();
  let mut visited = HashSet::new();
  for definition in &nodes {
    finish_definition(definition, graph, &mut visited, &mut finished);
  }

  let mut reverse = DefinitionGraph::new();
  for definition in &nodes {
    reverse.entry(definition.clone()).or_default();
  }
  for (definition, dependencies) in graph {
    for dependency in dependencies {
      reverse.entry(dependency.clone()).or_default().insert(definition.clone());
    }
  }

  let mut components = Vec::new();
  visited.clear();
  for definition in finished.iter().rev() {
    if visited.contains(definition) {
      continue;
    }
    let mut component = Vec::new();
    collect_reverse_component(definition, &reverse, &mut visited, &mut component);
    component.sort();
    components.push(component);
  }
  components.sort_by(|a, b| a[0].cmp(&b[0]));

  let component_by_definition = components
    .iter()
    .enumerate()
    .flat_map(|(index, component)| component.iter().map(move |definition| (definition.clone(), index)))
    .collect::<BTreeMap<_, _>>();
  let mut dependencies = vec![BTreeSet::new(); components.len()];
  for (definition, definition_dependencies) in graph {
    let component = component_by_definition[definition];
    for dependency in definition_dependencies {
      let dependency_component = component_by_definition[dependency];
      if component != dependency_component {
        dependencies[component].insert(dependency_component);
      }
    }
  }

  let mut ordered = Vec::new();
  let mut visited_components = HashSet::new();
  for component in 0..components.len() {
    visit_component(component, &dependencies, &mut visited_components, &mut ordered);
  }
  ordered.into_iter().map(|index| components[index].clone()).collect()
}

fn reachable_definitions(entries: &ProgramEntries) -> Result<(DefinitionGraph, Vec<Vec<String>>), String> {
  let mut graph = BTreeMap::new();
  for (ns, definition) in [
    (entries.init_ns.as_ref(), entries.init_def.as_ref()),
    (entries.reload_ns.as_ref(), entries.reload_def.as_ref()),
  ] {
    let mut analyzer = CallTreeAnalyzer::new(CallTreeConfig::default());
    let result = analyzer.analyze(ns, definition)?;
    collect_graph(&result.tree, &mut graph);
  }

  let components = strongly_connected_components(&graph);
  Ok((graph, components))
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

fn push_unique_diagnostic(diagnostics: &mut Vec<CheckDiagnostic>, diagnostic: CheckDiagnostic) {
  if !diagnostics.iter().any(|item| {
    item.severity == diagnostic.severity
      && item.code == diagnostic.code
      && item.message == diagnostic.message
      && item.definition == diagnostic.definition
      && item.path == diagnostic.path
  }) {
    diagnostics.push(diagnostic);
  }
}

fn take_definition_outcome(
  definition: &str,
  confirmed: &mut BTreeMap<String, Vec<CheckDiagnostic>>,
  cascaded: &mut BTreeMap<String, Vec<CheckDiagnostic>>,
  blockers: &[String],
) -> (&'static str, Vec<String>, Vec<CheckDiagnostic>) {
  if let Some(mut diagnostics) = confirmed.remove(definition) {
    if let Some(extra) = cascaded.remove(definition) {
      for diagnostic in extra {
        push_unique_diagnostic(&mut diagnostics, diagnostic);
      }
    }
    ("failed", Vec::new(), diagnostics)
  } else if let Some(diagnostics) = cascaded.remove(definition) {
    ("cascaded", Vec::new(), diagnostics)
  } else if blockers.is_empty() {
    ("passed", Vec::new(), Vec::new())
  } else {
    ("blocked", blockers.to_vec(), Vec::new())
  }
}

fn check_definitions(graph: &DefinitionGraph, components: &[Vec<String>]) -> Vec<DefinitionResult> {
  let mut statuses = BTreeMap::new();
  let mut results = Vec::new();

  for component in components {
    let component_set = component.iter().cloned().collect::<HashSet<_>>();
    let external_blockers = component
      .iter()
      .flat_map(|definition| graph.get(definition).into_iter().flatten())
      .filter(|dependency| !component_set.contains(*dependency))
      .filter(|dependency| matches!(statuses.get(*dependency), Some(&"failed" | &"blocked" | &"cascaded")))
      .cloned()
      .collect::<BTreeSet<_>>()
      .into_iter()
      .collect::<Vec<_>>();
    if !external_blockers.is_empty() {
      for definition in component {
        statuses.insert(definition.clone(), "blocked");
        results.push(DefinitionResult {
          definition: definition.clone(),
          status: "blocked",
          blocked_by: external_blockers.clone(),
          diagnostics: Vec::new(),
        });
      }
      continue;
    }

    let mut confirmed = BTreeMap::<String, Vec<CheckDiagnostic>>::new();
    let mut cascaded = BTreeMap::<String, Vec<CheckDiagnostic>>::new();
    for definition in component {
      let Some((ns, def)) = definition.split_once('/') else {
        cascaded.entry(definition.clone()).or_default().push(CheckDiagnostic {
          severity: "error",
          code: Some("E_INVALID_DEFINITION_ID".to_owned()),
          message: format!("Invalid qualified definition `{definition}`"),
          definition: definition.clone(),
          path: Vec::new(),
          hint: None,
          expected: None,
          actual: None,
          provenance: Vec::new(),
        });
        continue;
      };

      let warnings = RefCell::new(Vec::new());
      let outcome = runner::preprocess::ensure_ns_def_compiled(ns, def, &warnings, &CallStackList::default());
      for diagnostic in warnings.borrow().iter().map(warning_diagnostic) {
        if component_set.contains(&diagnostic.definition) {
          push_unique_diagnostic(confirmed.entry(diagnostic.definition.clone()).or_default(), diagnostic);
        } else {
          push_unique_diagnostic(cascaded.entry(definition.clone()).or_default(), diagnostic);
        }
      }
      if let Err(error) = outcome {
        for diagnostic in error.warnings.iter().map(warning_diagnostic) {
          if component_set.contains(&diagnostic.definition) {
            push_unique_diagnostic(confirmed.entry(diagnostic.definition.clone()).or_default(), diagnostic);
          } else {
            push_unique_diagnostic(cascaded.entry(definition.clone()).or_default(), diagnostic);
          }
        }
        let owner = error.location.as_ref().map(|location| format!("{}/{}", location.ns, location.def));
        let diagnostic = error_diagnostic(&error, definition);
        if let Some(owner) = owner.filter(|owner| component_set.contains(owner)) {
          push_unique_diagnostic(confirmed.entry(owner).or_default(), diagnostic);
        } else {
          push_unique_diagnostic(cascaded.entry(definition.clone()).or_default(), diagnostic);
        }
      }
    }

    let blockers = confirmed
      .keys()
      .chain(cascaded.keys())
      .cloned()
      .collect::<BTreeSet<_>>()
      .into_iter()
      .collect::<Vec<_>>();
    for definition in component {
      let (status, blocked_by, diagnostics) = take_definition_outcome(definition, &mut confirmed, &mut cascaded, &blockers);
      statuses.insert(definition.clone(), status);
      results.push(DefinitionResult {
        definition: definition.clone(),
        status,
        blocked_by,
        diagnostics,
      });
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

#[cfg(test)]
mod tests {
  use super::{CheckDiagnostic, take_definition_outcome};
  use std::collections::BTreeMap;

  fn diagnostic(message: &str, definition: &str) -> CheckDiagnostic {
    CheckDiagnostic {
      severity: "error",
      code: None,
      message: message.to_owned(),
      definition: definition.to_owned(),
      path: Vec::new(),
      hint: None,
      expected: None,
      actual: None,
      provenance: Vec::new(),
    }
  }

  #[test]
  fn failed_definition_retains_its_cascaded_diagnostics() {
    let definition = "app.main/failing".to_owned();
    let mut confirmed = BTreeMap::from([(definition.clone(), vec![diagnostic("own error", &definition)])]);
    let mut cascaded = BTreeMap::from([(definition.clone(), vec![diagnostic("dependency error", "app.main/dependency")])]);

    let (status, blocked_by, diagnostics) =
      take_definition_outcome(&definition, &mut confirmed, &mut cascaded, &["app.main/dependency".to_owned()]);

    assert_eq!(status, "failed");
    assert!(blocked_by.is_empty());
    assert_eq!(diagnostics.len(), 2);
    assert!(confirmed.is_empty());
    assert!(cascaded.is_empty());
  }
}
