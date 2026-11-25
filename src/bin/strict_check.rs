use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::sync::Arc;

use calcit::ProgramEntries;
use calcit::calcit::{CalcitErr, CalcitTypeAnnotation, LocatedWarning};
use calcit::call_stack::CallStackList;
use calcit::call_tree::{CallTreeAnalyzer, CallTreeConfig, CallTreeNode};
use calcit::program;
use calcit::runner;
use calcit::snapshot::{self, FeaturePolicy, SnapshotTarget};
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
  /// `reachable` (entry closure) or `all-defs` (entry closure plus all project definitions)
  scope: &'static str,
  entries: Vec<String>,
  summary: CheckSummary,
  definitions: Vec<DefinitionResult>,
  /// Non-failing diagnostics such as `W_OPEN_FAMILY`; they never change `status`.
  advisories: Vec<CheckDiagnostic>,
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

/// Per-entry runtime/checking context captured from the Snapshot.
#[derive(Debug, Clone)]
struct EntryCheckContext {
  name: String,
  init_fn: String,
  reload_fn: String,
  init_ns: String,
  init_def: String,
  reload_ns: String,
  reload_def: String,
  target: Option<SnapshotTarget>,
  type_slots: HashMap<String, String>,
  feature_policy: HashMap<String, FeaturePolicy>,
}

/// Extra roots for the all-definitions scope: every definition of the project namespaces.
/// Dependencies and core are never roots; they are only checked when a root reaches them.
pub struct AllDefsScope {
  /// `(namespace, definition)` roots, sorted so the sequence never depends on Snapshot or HashSet order.
  roots: Vec<(String, String)>,
  /// Project namespaces owned by this snapshot (used for namespace-level entry ownership).
  project_namespaces: HashSet<String>,
  /// Core builtin placeholders have no Calcit body to preprocess; they are excluded like `--check-only --ns` does.
  intrinsics: HashSet<String>,
  /// All entries in the snapshot, starting with the selected entry, so multi-target
  /// projects check each entry's closure and unwired definitions under their true target (#2917).
  entry_contexts: Vec<EntryCheckContext>,
  /// Declared `:ffi :target` metadata across all loaded definitions (project + dependencies).
  declared_targets: HashMap<String, SnapshotTarget>,
}

impl AllDefsScope {
  pub fn from_snapshot(
    snapshot: &snapshot::Snapshot,
    project_namespaces: &HashSet<String>,
    selected_entries: &ProgramEntries,
  ) -> Result<Self, String> {
    let mut roots = Vec::new();
    let mut intrinsics = HashSet::new();
    for ns in project_namespaces {
      let Some(file) = snapshot.files.get(ns) else {
        continue;
      };
      for (def, entry) in &file.defs {
        if crate::public_api_check::is_core_runtime_intrinsic(ns, def, entry) {
          intrinsics.insert(format!("{ns}/{def}"));
        } else {
          roots.push((ns.clone(), def.clone()));
        }
      }
    }
    roots.sort();

    let mut declared_targets = HashMap::new();
    for (ns, file) in &snapshot.files {
      for (def, entry) in &file.defs {
        if let Some(ffi) = &entry.ffi
          && let Ok(Some(target)) = snapshot::parse_ffi_target(ffi)
        {
          declared_targets.insert(format!("{ns}/{def}"), target);
        }
      }
    }

    let selected_name = snapshot.active_entry_name();
    let mut entry_names = snapshot.entries.keys().cloned().collect::<Vec<_>>();
    entry_names.sort_by_key(|name| (name != selected_name, name != snapshot::DEFAULT_ENTRY_NAME, name.clone()));

    let mut entry_contexts = Vec::with_capacity(entry_names.len());
    for name in entry_names {
      let entry = &snapshot.entries[&name];
      let mut feature_policy = entry.feature_policy.clone();
      feature_policy.entry("js-ffi".to_owned()).or_insert(FeaturePolicy::Error);
      let (init_fn, reload_fn, init_ns, init_def, reload_ns, reload_def) = if name == selected_name {
        (
          selected_entries.init_fn.to_string(),
          selected_entries.reload_fn.to_string(),
          selected_entries.init_ns.to_string(),
          selected_entries.init_def.to_string(),
          selected_entries.reload_ns.to_string(),
          selected_entries.reload_def.to_string(),
        )
      } else {
        let (init_ns, init_def) = calcit::util::string::extract_ns_def(&entry.init_fn)?;
        let (reload_ns, reload_def) = calcit::util::string::extract_ns_def(&entry.reload_fn)?;
        (
          entry.init_fn.clone(),
          entry.reload_fn.clone(),
          init_ns,
          init_def,
          reload_ns,
          reload_def,
        )
      };
      entry_contexts.push(EntryCheckContext {
        name,
        init_fn,
        reload_fn,
        init_ns,
        init_def,
        reload_ns,
        reload_def,
        target: entry.target,
        type_slots: entry.type_slots.clone(),
        feature_policy,
      });
    }

    Ok(AllDefsScope {
      roots,
      project_namespaces: project_namespaces.clone(),
      intrinsics,
      entry_contexts,
      declared_targets,
    })
  }

  fn has_multiple_targets(&self) -> bool {
    let mut targets = Vec::new();
    for ctx in &self.entry_contexts {
      if let Some(target) = ctx.target {
        push_unique_target(&mut targets, target);
      }
    }
    targets.len() >= 2
  }
}

/// Restores global entry target, feature policy, and type slots when leaving
/// multi-entry checking.
struct EntryContextGuard {
  target: Option<SnapshotTarget>,
  feature_policy: HashMap<String, FeaturePolicy>,
  type_slots: HashMap<Arc<str>, Arc<CalcitTypeAnnotation>>,
}

impl EntryContextGuard {
  fn capture(initial_feature_policy: &HashMap<String, FeaturePolicy>) -> Self {
    Self {
      target: program::active_entry_target(),
      feature_policy: initial_feature_policy.clone(),
      type_slots: calcit::calcit::entry_type_slots(),
    }
  }
}

impl Drop for EntryContextGuard {
  fn drop(&mut self) {
    program::configure_entry_target(self.target);
    program::configure_entry_feature_policy(&self.feature_policy);
    calcit::calcit::install_entry_type_slots(std::mem::take(&mut self.type_slots));
  }
}

fn resolve_type_ref_fqn(current_ns: &str, raw_name: &str) -> String {
  let trimmed = raw_name.trim_start_matches('\'').trim_start_matches(':');
  if let Some((prefix, def)) = trimmed.rsplit_once('/') {
    if let Some(resolved_ns) = program::lookup_ns_target_in_import(current_ns, prefix) {
      format!("{resolved_ns}/{def}")
    } else {
      format!("{prefix}/{def}")
    }
  } else if let Some((target_ns, target_def)) = program::lookup_refer_target_in_import(current_ns, trimmed) {
    format!("{target_ns}/{target_def}")
  } else {
    format!("{current_ns}/{trimmed}")
  }
}

fn push_unique_target(targets: &mut Vec<SnapshotTarget>, target: SnapshotTarget) {
  if !targets.contains(&target) {
    targets.push(target);
  }
}

fn collect_schema_declared_targets(
  current_ns: &str,
  annotation: &CalcitTypeAnnotation,
  declared_targets: &HashMap<String, SnapshotTarget>,
  out: &mut Vec<SnapshotTarget>,
) {
  match annotation {
    CalcitTypeAnnotation::TypeRef(name, args) => {
      let fqn = resolve_type_ref_fqn(current_ns, name);
      if let Some(&target) = declared_targets.get(&fqn) {
        push_unique_target(out, target);
      }
      for arg in args.iter() {
        collect_schema_declared_targets(current_ns, arg, declared_targets, out);
      }
    }
    CalcitTypeAnnotation::Trait(trait_def) | CalcitTypeAnnotation::DynTrait(trait_def) => {
      if let Some(def_ref) = trait_def.definition_ref.as_deref()
        && let Some(&target) = declared_targets.get(def_ref)
      {
        push_unique_target(out, target);
      }
    }
    CalcitTypeAnnotation::TraitSet(traits) => {
      for trait_def in traits.iter() {
        if let Some(def_ref) = trait_def.definition_ref.as_deref()
          && let Some(&target) = declared_targets.get(def_ref)
        {
          push_unique_target(out, target);
        }
      }
    }
    CalcitTypeAnnotation::Fn(fn_annot) => {
      for arg in &fn_annot.arg_types {
        collect_schema_declared_targets(current_ns, arg, declared_targets, out);
      }
      if let Some(rest) = &fn_annot.rest_type {
        collect_schema_declared_targets(current_ns, rest, declared_targets, out);
      }
      collect_schema_declared_targets(current_ns, &fn_annot.return_type, declared_targets, out);
    }
    CalcitTypeAnnotation::List(inner)
    | CalcitTypeAnnotation::Set(inner)
    | CalcitTypeAnnotation::Ref(inner)
    | CalcitTypeAnnotation::Variadic(inner)
    | CalcitTypeAnnotation::Nullable(inner, _) => {
      collect_schema_declared_targets(current_ns, inner, declared_targets, out);
    }
    CalcitTypeAnnotation::Map(k, v) => {
      collect_schema_declared_targets(current_ns, k, declared_targets, out);
      collect_schema_declared_targets(current_ns, v, declared_targets, out);
    }
    CalcitTypeAnnotation::Tuple(items) => {
      for item in items.iter() {
        collect_schema_declared_targets(current_ns, item, declared_targets, out);
      }
    }
    CalcitTypeAnnotation::Struct(_, args) | CalcitTypeAnnotation::Enum(_, args) => {
      for arg in args.iter() {
        collect_schema_declared_targets(current_ns, arg, declared_targets, out);
      }
    }
    _ => {}
  }
}

fn status_rank(status: &str) -> u8 {
  match status {
    "failed" => 3,
    "cascaded" => 2,
    "blocked" => 1,
    _ => 0,
  }
}

fn merge_definition_result(existing: &mut DefinitionResult, incoming: DefinitionResult) {
  let incoming_rank = status_rank(incoming.status);
  let existing_rank = status_rank(existing.status);

  for diagnostic in incoming.diagnostics {
    push_unique_diagnostic(&mut existing.diagnostics, diagnostic);
  }

  if incoming_rank > existing_rank {
    existing.status = incoming.status;
    if incoming.status == "blocked" {
      existing.blocked_by = incoming.blocked_by;
    } else {
      existing.blocked_by.clear();
    }
  } else if incoming_rank == existing_rank && existing.status == "blocked" {
    let mut blockers = existing.blocked_by.iter().cloned().collect::<BTreeSet<_>>();
    blockers.extend(incoming.blocked_by);
    existing.blocked_by = blockers.into_iter().collect();
  }
}

fn push_unique_warning(warnings: &mut Vec<LocatedWarning>, warning: LocatedWarning) {
  let loc = warning.location();
  let duplicate = warnings.iter().any(|existing| {
    let existing_loc = existing.location();
    existing.code() == warning.code()
      && existing.message() == warning.message()
      && existing_loc.ns == loc.ns
      && existing_loc.def == loc.def
      && existing_loc.coord == loc.coord
  });
  if !duplicate {
    warnings.push(warning);
  }
}

/// Build the dependency graph for a specific list of roots, excluding core runtime intrinsics.
fn build_graph_for_roots(
  roots: &[(String, String)],
  include_core: bool,
  intrinsics: Option<&HashSet<String>>,
) -> Result<(DefinitionGraph, Vec<Vec<String>>), String> {
  let mut graph = BTreeMap::new();
  // One analyzer is shared across roots: expanded nodes keep every outgoing edge in
  // `graph`, so later roots only add what is new and the final graph is root-order independent.
  let mut analyzer = CallTreeAnalyzer::new(CallTreeConfig {
    include_core,
    ..CallTreeConfig::default()
  });
  for (ns, definition) in roots {
    let result = analyzer.analyze(ns, definition)?;
    collect_graph(&result.tree, &mut graph);
  }

  if let Some(intrinsics) = intrinsics {
    graph.retain(|definition, _| !intrinsics.contains(definition));
    for dependencies in graph.values_mut() {
      dependencies.retain(|dependency| !intrinsics.contains(dependency));
    }
  }

  let components = strongly_connected_components(&graph);
  Ok((graph, components))
}

/// Collect the definition graph from the entry roots, plus every definition of the
/// project namespaces when the all-definitions scope is requested.
fn reachable_definitions(
  entries: &ProgramEntries,
  all_defs: Option<&AllDefsScope>,
) -> Result<(DefinitionGraph, Vec<Vec<String>>), String> {
  let mut roots = vec![
    (entries.init_ns.to_string(), entries.init_def.to_string()),
    (entries.reload_ns.to_string(), entries.reload_def.to_string()),
  ];
  // When the project itself is calcit.core, its definitions must stay in the graph as edges.
  let mut include_core = false;
  if let Some(scope) = all_defs {
    include_core = scope.roots.iter().any(|(ns, _)| calcit::calcit::is_bundled_core_ns(ns));
    roots.extend(scope.roots.iter().cloned());
  }
  build_graph_for_roots(&roots, include_core, all_defs.map(|scope| &scope.intrinsics))
}

fn warning_diagnostic(warning: &LocatedWarning) -> CheckDiagnostic {
  let location = warning.location();
  CheckDiagnostic {
    severity: located_warning_severity(warning),
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

/// Collected preprocessing diagnostics with an `E_` code (or an uncoded
/// `[Error]` message) block the check, so report them as errors; other
/// collected diagnostics keep warning severity.
fn located_warning_severity(warning: &LocatedWarning) -> &'static str {
  if warning
    .code()
    .map_or_else(|| warning.message().starts_with("[Error]"), |code| code.starts_with("E_"))
  {
    "error"
  } else {
    "warning"
  }
}

fn advisory_diagnostic(advisory: &LocatedWarning) -> CheckDiagnostic {
  CheckDiagnostic {
    severity: "note",
    ..warning_diagnostic(advisory)
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
      runner::preprocess::drop_macro_owned_proof_warnings(&warnings, 0);
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

/// Marks passed definitions blocked when a definition they depend on failed
/// after the ordinary check, such as for a bare broad type (#2504).
/// `components` are in dependency order, so blocks propagate in one pass.
fn block_dependents_of_failed(graph: &DefinitionGraph, components: &[Vec<String>], results: &mut [DefinitionResult]) {
  let mut statuses = results
    .iter()
    .map(|result| (result.definition.clone(), result.status))
    .collect::<HashMap<_, _>>();
  for component in components {
    let component_set = component.iter().collect::<HashSet<_>>();
    let blockers = component
      .iter()
      .flat_map(|definition| graph.get(definition).into_iter().flatten())
      .filter(|dependency| !component_set.contains(dependency))
      .chain(component.iter())
      .filter(|dependency| matches!(statuses.get(*dependency), Some(&"failed" | &"blocked" | &"cascaded")))
      .cloned()
      .collect::<BTreeSet<_>>();
    if blockers.is_empty() {
      continue;
    }
    for definition in component {
      let Some(result) = results
        .iter_mut()
        .find(|result| &result.definition == definition && result.status == "passed")
      else {
        continue;
      };
      result.status = "blocked";
      result.blocked_by = blockers.iter().filter(|blocker| *blocker != definition).cloned().collect();
      statuses.insert(definition.clone(), "blocked");
    }
  }
}

fn print_human(data: &CheckData) {
  println!("# Strict check\n");
  println!("- status: **{}**", data.status.to_uppercase());
  println!("- scope: {}", data.scope);
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

fn apply_broad_type_errors(
  graph: &DefinitionGraph,
  definitions: &[Vec<String>],
  results: &mut Vec<DefinitionResult>,
  broad_type_errors: &[LocatedWarning],
) {
  // Bare broad types (#2504) and Options erased to nil (#2124) are errors
  // since 0.36.0: they fail the definition that writes them, reachable or not.
  for error in broad_type_errors {
    let diagnostic = CheckDiagnostic {
      severity: "error",
      ..warning_diagnostic(error)
    };
    match results.iter_mut().find(|result| result.definition == diagnostic.definition) {
      Some(result) => {
        result.status = "failed";
        result.blocked_by.clear();
        push_unique_diagnostic(&mut result.diagnostics, diagnostic);
      }
      None => results.push(DefinitionResult {
        definition: diagnostic.definition.clone(),
        status: "failed",
        blocked_by: vec![],
        diagnostics: vec![diagnostic],
      }),
    }
  }
  if !broad_type_errors.is_empty() {
    block_dependents_of_failed(graph, definitions, results);
  }
}

/// Run the ordinary check + default proof audit on a single dependency graph
/// under the currently active entry context.
fn run_graph_pass(
  graph: &DefinitionGraph,
  definitions: &[Vec<String>],
) -> (Vec<DefinitionResult>, Vec<LocatedWarning>, Vec<LocatedWarning>) {
  let mut results = check_definitions(graph, definitions);
  let (broad_type_errors, advisories) = runner::preprocess::split_blocking_advisories(runner::preprocess::take_advisories());
  // The proof audit re-checks the components that passed the ordinary check,
  // so ordinary rejections keep their own diagnostics (#2121). It runs even
  // when another definition failed, so an unrelated warning cannot leave an
  // unproven definition counted as passed (#2591). One audit covers the
  // whole check, so shared dependencies are proved once.
  if runner::preprocess::default_proof_enabled() {
    let passed = results
      .iter()
      .filter(|result| result.status == "passed")
      .map(|result| result.definition.clone())
      .collect::<HashSet<_>>();
    // A passed definition never depends on a failed one (it would be blocked),
    // so these components are closed under their dependencies.
    let passed_components = definitions
      .iter()
      .filter(|component| component.iter().all(|definition| passed.contains(definition)))
      .cloned()
      .collect::<Vec<_>>();
    if !passed_components.is_empty() {
      let mut audited = runner::preprocess::with_default_proof(|| check_definitions(graph, &passed_components))
        .into_iter()
        .map(|result| (result.definition.clone(), result))
        .collect::<HashMap<_, _>>();
      for result in &mut results {
        if let Some(audited) = audited.remove(&result.definition) {
          *result = audited;
        }
      }
      // The audit re-preprocesses the same code; its advisories repeat the ordinary ones.
      drop(runner::preprocess::take_advisories());
    }
  }
  (results, broad_type_errors, advisories)
}

fn find_entry_with_target(entry_contexts: &[EntryCheckContext], target: SnapshotTarget) -> Option<usize> {
  entry_contexts.iter().position(|ctx| ctx.target == Some(target))
}

fn transitive_nodes_from(start: &str, graph: &DefinitionGraph) -> BTreeSet<String> {
  let mut visited = BTreeSet::new();
  let mut queue = VecDeque::new();
  visited.insert(start.to_owned());
  queue.push_back(start.to_owned());
  while let Some(node) = queue.pop_front() {
    if let Some(deps) = graph.get(&node) {
      for dep in deps {
        if visited.insert(dep.clone()) {
          queue.push_back(dep.clone());
        }
      }
    }
  }
  visited
}

fn assign_unwired_entry_owner(
  ns: &str,
  def: &str,
  scope: &AllDefsScope,
  ns_entry_targets: &HashMap<String, Vec<SnapshotTarget>>,
  ns_entry_indices: &HashMap<String, BTreeSet<usize>>,
  unwired_graph: &DefinitionGraph,
) -> usize {
  let fqn = format!("{ns}/{def}");

  // 1. Own declared `:ffi :target` on the definition itself.
  if let Some(&target) = scope.declared_targets.get(&fqn)
    && let Some(idx) = find_entry_with_target(&scope.entry_contexts, target)
  {
    return idx;
  }

  // 2. Exclusive namespace owner: all reached definitions in `ns` share one target.
  if let Some(targets) = ns_entry_targets.get(ns)
    && targets.len() == 1
    && let Some(&target) = targets.first()
  {
    if let Some(indices) = ns_entry_indices.get(ns)
      && let Some(&idx) = indices.iter().find(|&&idx| scope.entry_contexts[idx].target == Some(target))
    {
      return idx;
    }
    if let Some(idx) = find_entry_with_target(&scope.entry_contexts, target) {
      return idx;
    }
  }

  // 3. Unambiguous `:ffi :target` referenced by the definition's transitive call
  // graph or schemas. Conflicting targets (len > 1) fall back to the selected entry
  // so `E_JS_FFI_TARGET_MISMATCH` is still reported.
  let mut referenced_targets = Vec::new();
  for node in transitive_nodes_from(&fqn, unwired_graph) {
    if let Some(&target) = scope.declared_targets.get(&node) {
      push_unique_target(&mut referenced_targets, target);
    }
    if let Some((node_ns, node_def)) = node.rsplit_once('/') {
      let schema = program::lookup_def_schema(node_ns, node_def);
      collect_schema_declared_targets(node_ns, schema.as_ref(), &scope.declared_targets, &mut referenced_targets);
    }
  }
  if referenced_targets.len() == 1
    && let Some(&target) = referenced_targets.first()
    && let Some(idx) = find_entry_with_target(&scope.entry_contexts, target)
  {
    return idx;
  }

  // 4. Default fallback: selected entry.
  0
}

struct MultiEntryCheckOutcome {
  entries: Vec<String>,
  results: Vec<DefinitionResult>,
  advisories: Vec<LocatedWarning>,
}

fn run_multi_entry_all_defs(scope: &AllDefsScope) -> Result<MultiEntryCheckOutcome, String> {
  let include_core = scope.roots.iter().any(|(ns, _)| calcit::calcit::is_bundled_core_ns(ns));
  let entry_count = scope.entry_contexts.len();

  // Collect each entry's reachable closure so every definition reached by an
  // entry is checked under that entry's own target, type slots, and policy.
  let mut entry_roots: Vec<Vec<(String, String)>> = vec![Vec::new(); entry_count];
  let mut reached_definitions = HashSet::<String>::new();
  let mut ns_entry_targets = HashMap::<String, Vec<SnapshotTarget>>::new();
  let mut ns_entry_indices = HashMap::<String, BTreeSet<usize>>::new();

  for (idx, ctx) in scope.entry_contexts.iter().enumerate() {
    let mut roots = Vec::new();
    for (ns, def) in [(&ctx.init_ns, &ctx.init_def), (&ctx.reload_ns, &ctx.reload_def)] {
      let pair = (ns.clone(), def.clone());
      if !roots.contains(&pair) {
        roots.push(pair);
      }
    }
    if roots.is_empty() {
      continue;
    }
    let (graph, _) = build_graph_for_roots(&roots, include_core, Some(&scope.intrinsics))?;
    for fqn in graph.keys() {
      reached_definitions.insert(fqn.clone());
      if let Some((ns, _)) = fqn.rsplit_once('/')
        && scope.project_namespaces.contains(ns)
      {
        ns_entry_indices.entry(ns.to_owned()).or_default().insert(idx);
        if let Some(target) = ctx.target {
          push_unique_target(ns_entry_targets.entry(ns.to_owned()).or_default(), target);
        }
      }
    }
    entry_roots[idx] = roots;
  }

  // Assign unwired project definitions to their true entry context using a
  // single full graph so transitive target discovery is order-independent.
  let unwired_roots = scope
    .roots
    .iter()
    .filter(|(ns, def)| !reached_definitions.contains(&format!("{ns}/{def}")))
    .cloned()
    .collect::<Vec<_>>();
  if !unwired_roots.is_empty() {
    let (unwired_graph, _) = build_graph_for_roots(&unwired_roots, include_core, Some(&scope.intrinsics))?;
    for (ns, def) in &unwired_roots {
      let owner = assign_unwired_entry_owner(ns, def, scope, &ns_entry_targets, &ns_entry_indices, &unwired_graph);
      let pair = (ns.clone(), def.clone());
      if !entry_roots[owner].contains(&pair) {
        entry_roots[owner].push(pair);
      }
    }
  }

  // Drain snapshot-level advisories (e.g. bare broad types) before per-entry passes.
  let (snapshot_broad_errors, initial_advisories) =
    runner::preprocess::split_blocking_advisories(runner::preprocess::take_advisories());
  let mut all_advisories = Vec::new();
  for advisory in initial_advisories {
    push_unique_warning(&mut all_advisories, advisory);
  }

  let _context_guard = EntryContextGuard::capture(&scope.entry_contexts[0].feature_policy);
  let mut merged_results = BTreeMap::<String, DefinitionResult>::new();
  let mut union_graph = DefinitionGraph::new();
  let mut checked_entries = Vec::<String>::new();

  for (idx, ctx) in scope.entry_contexts.iter().enumerate() {
    if entry_roots[idx].is_empty() {
      continue;
    }
    for entry_fn in [&ctx.init_fn, &ctx.reload_fn] {
      if !checked_entries.contains(entry_fn) {
        checked_entries.push(entry_fn.clone());
      }
    }
    let (entry_graph, entry_components) = build_graph_for_roots(&entry_roots[idx], include_core, Some(&scope.intrinsics))?;
    for (node, deps) in &entry_graph {
      union_graph.entry(node.clone()).or_default().extend(deps.iter().cloned());
    }

    program::configure_entry_target(ctx.target);
    program::configure_entry_feature_policy(&ctx.feature_policy);
    calcit::calcit::configure_entry_type_slots(&ctx.type_slots)
      .map_err(|message| format!("entries.{}.type-slots: {message}", ctx.name))?;

    let (mut entry_results, entry_broad_errors, entry_advisories) =
      runner::preprocess::with_restored_compiled_program(|| run_graph_pass(&entry_graph, &entry_components));
    apply_broad_type_errors(&entry_graph, &entry_components, &mut entry_results, &entry_broad_errors);
    for advisory in entry_advisories {
      push_unique_warning(&mut all_advisories, advisory);
    }
    for result in entry_results {
      match merged_results.get_mut(&result.definition) {
        Some(existing) => merge_definition_result(existing, result),
        None => {
          merged_results.insert(result.definition.clone(), result);
        }
      }
    }
  }

  let union_components = strongly_connected_components(&union_graph);
  let mut results = Vec::with_capacity(merged_results.len());
  for component in &union_components {
    for definition in component {
      if let Some(result) = merged_results.remove(definition) {
        results.push(result);
      }
    }
  }
  results.extend(merged_results.into_values());

  apply_broad_type_errors(&union_graph, &union_components, &mut results, &snapshot_broad_errors);

  Ok(MultiEntryCheckOutcome {
    entries: checked_entries,
    results,
    advisories: all_advisories,
  })
}

pub fn run(entries: &ProgramEntries, raw_format: &str, all_defs: Option<&AllDefsScope>) -> Result<(), String> {
  let format = StructuredOutputFormat::parse(raw_format, "check-only")?;
  let _output_guard = crate::ProgramOutputGuard::new(format != StructuredOutputFormat::Human, false);
  let (checked_entries, results, advisories) = match all_defs {
    Some(scope) if scope.has_multiple_targets() => {
      let outcome = run_multi_entry_all_defs(scope)?;
      (outcome.entries, outcome.results, outcome.advisories)
    }
    _ => {
      let (graph, definitions) = reachable_definitions(entries, all_defs)?;
      let (mut results, broad_type_errors, advisories) = run_graph_pass(&graph, &definitions);
      apply_broad_type_errors(&graph, &definitions, &mut results, &broad_type_errors);
      (
        vec![entries.init_fn.to_string(), entries.reload_fn.to_string()],
        results,
        advisories,
      )
    }
  };
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
    scope: if all_defs.is_some() { "all-defs" } else { "reachable" },
    entries: checked_entries,
    summary,
    definitions: results,
    advisories: advisories.iter().map(advisory_diagnostic).collect(),
  };

  let envelope = serde_json::json!({
    "schema_version": 1,
    "command": "check-only",
    "data": data,
    "diagnostics": [],
  });
  match format {
    StructuredOutputFormat::Human => {
      print_human(&data);
      // Advisories never fail the check; stderr matches the plain check-only Notes.
      if !advisories.is_empty() {
        eprintln!("\nNotes: ({} notes, the check still passes)", advisories.len());
        LocatedWarning::print_list(&advisories);
      }
    }
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
  use super::{CheckDiagnostic, DefinitionResult, merge_definition_result, take_definition_outcome};
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

  #[test]
  fn merging_definition_results_prefers_failure_over_pass_or_blocked() {
    let mut shared = DefinitionResult {
      definition: "app.shared/helper".to_owned(),
      status: "passed",
      blocked_by: vec![],
      diagnostics: vec![],
    };
    merge_definition_result(
      &mut shared,
      DefinitionResult {
        definition: "app.shared/helper".to_owned(),
        status: "blocked",
        blocked_by: vec!["app.shared/dep".to_owned()],
        diagnostics: vec![],
      },
    );
    assert_eq!(shared.status, "blocked");
    assert_eq!(shared.blocked_by, vec!["app.shared/dep".to_owned()]);

    merge_definition_result(
      &mut shared,
      DefinitionResult {
        definition: "app.shared/helper".to_owned(),
        status: "failed",
        blocked_by: vec![],
        diagnostics: vec![diagnostic("target mismatch", "app.shared/helper")],
      },
    );
    assert_eq!(shared.status, "failed");
    assert!(shared.blocked_by.is_empty());
    assert_eq!(shared.diagnostics.len(), 1);
  }
}
