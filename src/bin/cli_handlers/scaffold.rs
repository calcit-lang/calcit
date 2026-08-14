//! Definition-graph architecture scaffold planning.
//!
//! It validates the canonical Cirru EDN plan, reconciles it with one Snapshot,
//! and emits a stable work-item view. A non-dry run atomically creates only
//! missing `:ensure` definitions as tagged `todo!` stubs; it never rewrites
//! existing definitions.

use calcit::calcit::CalcitTypeAnnotation;
use calcit::cli_args::EditScaffoldCommand;
use calcit::snapshot::{self, CodeEntry, Snapshot, definition_revision, render_snapshot_content};
use cirru_edn::{Edn, EdnListView, EdnMapView, EdnSetView, EdnTag};
use cirru_parser::Cirru;
use md5::{Digest, Md5};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use super::atomic_write::stage_atomic_file;
use super::common::read_code_input;
use super::edit::{check_ns_editable, load_snapshot, parse_target};

const ARCHITECTURE_SCHEMA_VERSION: f64 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DefinitionMode {
  Ensure,
  External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DefinitionKind {
  Function,
  Data,
}

impl DefinitionKind {
  fn as_tag(self) -> &'static str {
    match self {
      Self::Function => "fn",
      Self::Data => "data",
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ArchitectureEdge {
  kind: String,
  source: String,
  target: String,
}

#[derive(Debug, Clone)]
struct DefinitionSpec {
  mode: DefinitionMode,
  kind: DefinitionKind,
  doc: Option<String>,
  schema: Edn,
  schema_annotation: Arc<CalcitTypeAnnotation>,
  params: Vec<String>,
  raw: Edn,
}

#[derive(Debug, Clone)]
struct ArchitecturePlan {
  feature: String,
  doc: Option<String>,
  roots: BTreeSet<String>,
  definitions: BTreeMap<String, DefinitionSpec>,
  edges: BTreeSet<ArchitectureEdge>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReconciliationStatus {
  Create,
  ReusePending,
  ReuseComplete,
  External,
}

impl ReconciliationStatus {
  fn as_tag(self) -> &'static str {
    match self {
      Self::Create => "create",
      Self::ReusePending => "reuse-pending",
      Self::ReuseComplete => "reuse-complete",
      Self::External => "external",
    }
  }
}

#[derive(Debug, Clone)]
struct ReconciliationEntry {
  status: ReconciliationStatus,
  origin: String,
  existing: Option<Edn>,
  planned: Edn,
}

#[derive(Debug, Clone)]
struct WorkItem {
  id: String,
  plan_id: String,
  base_snapshot_revision: String,
  target: String,
  schema: Edn,
  doc: String,
  params: Vec<String>,
  planned_edges: Vec<ArchitectureEdge>,
}

#[derive(Debug, Clone)]
struct ScaffoldPlanReport {
  plan: ArchitecturePlan,
  plan_id: String,
  snapshot_revision: String,
  reconciliation: BTreeMap<String, ReconciliationEntry>,
  diagnostics: Vec<Edn>,
  work_items: Vec<WorkItem>,
}

#[derive(Debug, Clone)]
struct ScaffoldApplyResult {
  changed: bool,
  new_snapshot_revision: String,
}

pub(crate) fn handle_scaffold_command(opts: &EditScaffoldCommand, snapshot_file: &str) -> Result<(), String> {
  if !matches!(opts.format.as_str(), "human" | "edn" | "json") {
    return Err(format!(
      "Unsupported scaffold format '{}'. Expected human, edn, or json.",
      opts.format
    ));
  }

  let raw = read_code_input(&opts.file, &opts.code)?
    .ok_or("Architecture input required: use --file, --code, or pipe one Cirru EDN architecture map via stdin.")?;
  let plan = parse_architecture_plan(&raw)?;

  let snapshot_content =
    fs::read_to_string(snapshot_file).map_err(|error| format!("Failed to read snapshot '{snapshot_file}': {error}"))?;
  let snapshot_revision = content_revision(&snapshot_content);
  if let Some(expected) = opts.expect_revision.as_deref()
    && expected != snapshot_revision
  {
    return Err(format!(
      "Snapshot revision mismatch: expected '{expected}', current revision is '{snapshot_revision}'. Re-run the query and rebuild the scaffold plan."
    ));
  }
  let snapshot = load_snapshot(snapshot_file)?;
  let report = reconcile_plan(plan, &snapshot, snapshot_revision)?;
  let apply_result = if opts.dry_run {
    ScaffoldApplyResult {
      changed: false,
      new_snapshot_revision: report.snapshot_revision.clone(),
    }
  } else {
    apply_scaffold(&report, &snapshot, snapshot_file, &snapshot_content)?
  };

  match opts.format.as_str() {
    "human" => print_human_report(&report, opts.dry_run, &apply_result),
    "edn" => {
      let rendered = cirru_edn::format(&report_to_edn(&report, opts.dry_run, &apply_result), true)
        .map_err(|error| format!("Failed to render scaffold EDN result: {error}"))?;
      println!("{rendered}");
    }
    "json" => {
      let value = serde_json::to_value(report_to_edn(&report, opts.dry_run, &apply_result))
        .map_err(|error| format!("Failed to render scaffold JSON result: {error}"))?;
      println!(
        "{}",
        serde_json::to_string(&value).map_err(|error| format!("Failed to serialize scaffold JSON result: {error}"))?
      );
    }
    _ => unreachable!("format is validated above"),
  }
  Ok(())
}

fn apply_scaffold(
  report: &ScaffoldPlanReport,
  snapshot: &Snapshot,
  snapshot_file: &str,
  original_content: &str,
) -> Result<ScaffoldApplyResult, String> {
  let mut staged_snapshot = snapshot.clone();
  for (id, reconciliation) in &report.reconciliation {
    if reconciliation.status != ReconciliationStatus::Create {
      continue;
    }
    let spec = report
      .plan
      .definitions
      .get(id)
      .expect("reconciled definition must have a plan spec");
    let (namespace, definition) = parse_target(id)?;
    let file = staged_snapshot
      .files
      .get_mut(namespace)
      .ok_or_else(|| format!("Namespace '{namespace}' disappeared before scaffold apply."))?;
    if file.defs.contains_key(definition) {
      return Err(format!(
        "Definition '{id}' appeared before scaffold apply; no changes were written."
      ));
    }
    file.defs.insert(definition.to_owned(), scaffold_entry(id, definition, spec)?);
  }

  let staged_content = render_snapshot_content(&staged_snapshot)?;
  let changed = staged_content != original_content;
  let new_snapshot_revision = content_revision(&staged_content);
  if !changed {
    return Ok(ScaffoldApplyResult {
      changed,
      new_snapshot_revision,
    });
  }

  let staged = stage_atomic_file(Path::new(snapshot_file), original_content.as_bytes(), "scaffold snapshot")?;
  staged.write_and_sync(staged_content.as_bytes(), "scaffold snapshot")?;
  let current_content =
    fs::read_to_string(snapshot_file).map_err(|error| format!("Failed to re-read snapshot '{snapshot_file}': {error}"))?;
  if content_revision(&current_content) != report.snapshot_revision {
    return Err(format!(
      "Snapshot changed while scaffold was running: started at '{}', now '{}'. No scaffold changes were written.",
      report.snapshot_revision,
      content_revision(&current_content)
    ));
  }
  staged.commit()?;
  Ok(ScaffoldApplyResult {
    changed,
    new_snapshot_revision,
  })
}

fn scaffold_entry(id: &str, definition: &str, spec: &DefinitionSpec) -> Result<CodeEntry, String> {
  let code = match spec.kind {
    DefinitionKind::Function => Cirru::List(vec![
      Cirru::leaf("defn"),
      Cirru::leaf(definition),
      Cirru::List(spec.params.iter().map(|param| Cirru::leaf(param.as_str())).collect()),
      Cirru::List(vec![Cirru::leaf("todo!"), Cirru::leaf(format!("|TODO(scaffold): implement {id}"))]),
    ]),
    DefinitionKind::Data => match spec.raw.view_map()?.tag_get("code") {
      Some(Edn::Quote(code)) => code.clone(),
      _ => return Err(format!("Architecture data definition '{id}' is missing validated quoted :code.")),
    },
  };
  let mut entry = CodeEntry::from_code(code);
  entry.doc = spec.doc.clone().unwrap_or_default();
  entry.schema = spec.schema_annotation.clone();
  entry.tags.insert(EdnTag::new("scaffold"));
  Ok(entry)
}

fn parse_architecture_plan(raw: &str) -> Result<ArchitecturePlan, String> {
  let root = cirru_edn::parse(raw).map_err(|error| format!("Failed to parse architecture Cirru EDN: {error}"))?;
  let map = root
    .view_map()
    .map_err(|error| format!("Architecture root must be a map: {error}"))?;
  let version = required_number(&map, "schema-version")?;
  if (version - ARCHITECTURE_SCHEMA_VERSION).abs() > f64::EPSILON {
    return Err(format!(
      "Unsupported architecture schema version {version}. Expected {}.",
      ARCHITECTURE_SCHEMA_VERSION as u8
    ));
  }
  let feature = required_symbol(&map, "feature")?;
  let doc = optional_string(&map, "doc")?;
  let roots = required_symbol_set(&map, "roots")?;
  let definitions_value = map.tag_get("definitions").ok_or("Architecture is missing :definitions")?;
  let definitions_map = definitions_value
    .view_map()
    .map_err(|error| format!("Architecture :definitions must be a map: {error}"))?;
  let mut definitions = BTreeMap::new();
  for (id, value) in definitions_map.0.iter() {
    let id = edn_symbol(id, "Architecture definition key")?;
    if parse_target(&id).is_err() {
      return Err(format!(
        "Architecture definition key '{id}' must be a FQN in namespace/definition form."
      ));
    }
    if definitions.insert(id.clone(), parse_definition_spec(&id, value)?).is_some() {
      return Err(format!("Architecture repeats definition '{id}'."));
    }
  }
  if definitions.is_empty() {
    return Err("Architecture :definitions must not be empty.".to_string());
  }
  for root in &roots {
    if !definitions.contains_key(root) {
      return Err(format!("Architecture root '{root}' is not present in :definitions."));
    }
  }

  let mut edges = BTreeSet::new();
  if let Some(edges_value) = map.tag_get("edges") {
    let Edn::Set(values) = edges_value else {
      return Err("Architecture :edges must be a set of anonymous enum edges.".to_string());
    };
    for value in &values.0 {
      let edge = parse_edge(value)?;
      if !definitions.contains_key(&edge.source) || !definitions.contains_key(&edge.target) {
        return Err(format!(
          "Architecture edge :: :{} {} {} references a definition absent from :definitions.",
          edge.kind, edge.source, edge.target
        ));
      }
      edges.insert(edge);
    }
  }

  Ok(ArchitecturePlan {
    feature,
    doc,
    roots,
    definitions,
    edges,
  })
}

fn parse_definition_spec(id: &str, value: &Edn) -> Result<DefinitionSpec, String> {
  let map = value
    .view_map()
    .map_err(|error| format!("Architecture definition '{id}' must be a map: {error}"))?;
  let mode = match required_tag(&map, "mode")?.as_str() {
    "ensure" => DefinitionMode::Ensure,
    "external" => DefinitionMode::External,
    other => {
      return Err(format!(
        "Architecture definition '{id}' has unsupported :mode :{other}. Expected :ensure or :external."
      ));
    }
  };
  let kind = match required_tag(&map, "kind")?.as_str() {
    "fn" => DefinitionKind::Function,
    "data" => DefinitionKind::Data,
    other => {
      return Err(format!(
        "Architecture definition '{id}' has unsupported :kind :{other}. Expected :fn or :data."
      ));
    }
  };
  let doc = optional_string(&map, "doc")?;
  if mode == DefinitionMode::Ensure && doc.is_none() {
    return Err(format!("Architecture definition '{id}' with :mode :ensure is missing :doc."));
  }
  let schema = map
    .tag_get("schema")
    .ok_or_else(|| format!("Architecture definition '{id}' is missing :schema."))?
    .clone();
  let schema_cirru =
    snapshot::schema_edn_to_cirru(&schema).map_err(|error| format!("Architecture definition '{id}' has invalid :schema: {error}"))?;
  let schema_annotation = snapshot::parse_schema_annotation_for_write(&schema_cirru)
    .map_err(|error| format!("Architecture definition '{id}' has invalid :schema: {error}"))?;
  let params = match map.tag_get("params") {
    None => vec![],
    Some(value) => symbol_list(value, &format!("Architecture definition '{id}' :params"))?,
  };
  if kind == DefinitionKind::Function && mode == DefinitionMode::Ensure && map.tag_get("params").is_none() {
    return Err(format!("Architecture function '{id}' with :mode :ensure is missing :params."));
  }
  if kind == DefinitionKind::Function && mode == DefinitionMode::Ensure {
    let fn_schema = CalcitTypeAnnotation::parse_fn_schema_from_edn(&schema)
      .ok_or_else(|| format!("Architecture function '{id}' :schema must use the canonical `:: :fn` form."))?;
    if fn_schema.rest_type.is_none() && fn_schema.arg_types.len() != params.len() {
      return Err(format!(
        "Architecture function '{id}' has {} :params entries but its schema declares {} fixed arguments.",
        params.len(),
        fn_schema.arg_types.len()
      ));
    }
  }
  if kind == DefinitionKind::Data && map.tag_get("params").is_some() {
    return Err(format!("Architecture data definition '{id}' must not contain :params."));
  }
  if kind == DefinitionKind::Data && mode == DefinitionMode::Ensure && !matches!(map.tag_get("code"), Some(Edn::Quote(_))) {
    return Err(format!(
      "Architecture data definition '{id}' with :mode :ensure must provide quoted :code."
    ));
  }
  if let Some(code) = map.tag_get("code")
    && !matches!(code, Edn::Quote(_))
  {
    return Err(format!("Architecture definition '{id}' :code must be quoted Cirru AST."));
  }

  Ok(DefinitionSpec {
    mode,
    kind,
    doc,
    schema,
    schema_annotation,
    params,
    raw: value.clone(),
  })
}

fn parse_edge(value: &Edn) -> Result<ArchitectureEdge, String> {
  let Edn::Enum(edge) = value else {
    return Err("Architecture edges must be anonymous enum values such as `:: :call 'source/a 'target/b`.".to_string());
  };
  if edge.type_name.is_some() || edge.extra.len() != 2 {
    return Err("Architecture edge must be an anonymous enum with exactly source and target Symbol payloads.".to_string());
  }
  let kind = edge.variant.to_string();
  if !matches!(kind.as_str(), "call" | "type") {
    return Err(format!("Architecture edge has unsupported tag :{kind}. Expected :call or :type."));
  }
  Ok(ArchitectureEdge {
    kind,
    source: edn_symbol(&edge.extra[0], "Architecture edge source")?,
    target: edn_symbol(&edge.extra[1], "Architecture edge target")?,
  })
}

fn reconcile_plan(plan: ArchitecturePlan, snapshot: &Snapshot, snapshot_revision: String) -> Result<ScaffoldPlanReport, String> {
  let plan_id = plan_id(&plan)?;
  let mut reconciliation = BTreeMap::new();
  let mut diagnostics = vec![];
  let mut work_items = vec![];

  for (id, spec) in &plan.definitions {
    let (namespace, definition) = parse_target(id)?;
    let local_entry = snapshot.files.get(namespace).and_then(|file| file.defs.get(definition));
    let (status, origin, existing) = match spec.mode {
      DefinitionMode::External => {
        let origin = if local_entry.is_some() { "project" } else { "external" };
        (
          ReconciliationStatus::External,
          origin.to_string(),
          local_entry.map(existing_entry_to_edn),
        )
      }
      DefinitionMode::Ensure => {
        check_ns_editable(snapshot, namespace)?;
        if !snapshot.files.contains_key(namespace) {
          return Err(format!(
            "Architecture definition '{id}' targets missing namespace '{namespace}'. Create the namespace before scaffolding."
          ));
        }
        match local_entry {
          None => (ReconciliationStatus::Create, "project".to_string(), None),
          Some(entry) => {
            let existing_kind = code_entry_kind(entry);
            if existing_kind != spec.kind {
              return Err(format!(
                "Architecture conflict for '{id}': existing kind :{} does not match planned kind :{}.",
                existing_kind.as_tag(),
                spec.kind.as_tag()
              ));
            }
            if entry.schema.as_ref() != spec.schema_annotation.as_ref() {
              let existing_dynamic = matches!(entry.schema.as_ref(), CalcitTypeAnnotation::Dynamic);
              let planned_dynamic = matches!(spec.schema_annotation.as_ref(), CalcitTypeAnnotation::Dynamic);
              if existing_dynamic || planned_dynamic {
                diagnostics.push(diagnostic(
                  "warning",
                  "W_SCAFFOLD_DYNAMIC_SCHEMA",
                  id,
                  "Existing and planned schema differ because one side is Dynamic; scaffold will reuse the existing definition without narrowing it.",
                ));
              } else {
                return Err(format!(
                  "Architecture conflict for '{id}': existing schema does not match the planned schema."
                ));
              }
            }
            if spec.doc.as_deref().is_some_and(|doc| doc != entry.doc) {
              diagnostics.push(diagnostic(
                "warning",
                "W_SCAFFOLD_DOC_DIFF",
                id,
                "Existing documentation differs from the planned documentation; scaffold will preserve the existing value.",
              ));
            }
            let status = if entry_has_scaffold_todo(entry) {
              ReconciliationStatus::ReusePending
            } else {
              ReconciliationStatus::ReuseComplete
            };
            (status, "project".to_string(), Some(existing_entry_to_edn(entry)))
          }
        }
      }
    };
    reconciliation.insert(
      id.clone(),
      ReconciliationEntry {
        status,
        origin,
        existing,
        planned: spec.raw.clone(),
      },
    );
    if spec.kind == DefinitionKind::Function && matches!(status, ReconciliationStatus::Create | ReconciliationStatus::ReusePending) {
      let planned_edges = plan.edges.iter().filter(|edge| edge.source == *id).cloned().collect::<Vec<_>>();
      work_items.push(WorkItem {
        id: format!("{}/implement-{}", plan.feature, definition),
        plan_id: plan_id.clone(),
        base_snapshot_revision: snapshot_revision.clone(),
        target: id.clone(),
        schema: spec.schema.clone(),
        doc: spec.doc.clone().unwrap_or_default(),
        params: spec.params.clone(),
        planned_edges,
      });
    }
  }
  Ok(ScaffoldPlanReport {
    plan,
    plan_id,
    snapshot_revision,
    reconciliation,
    diagnostics,
    work_items,
  })
}

fn report_to_edn(report: &ScaffoldPlanReport, dry_run: bool, apply_result: &ScaffoldApplyResult) -> Edn {
  let reconciliation = Edn::map_from_iter(report.reconciliation.iter().map(|(id, entry)| {
    let mut fields = vec![
      (Edn::tag("status"), Edn::tag(entry.status.as_tag())),
      (Edn::tag("origin"), Edn::tag(entry.origin.as_str())),
      (Edn::tag("planned"), entry.planned.clone()),
    ];
    if let Some(existing) = &entry.existing {
      fields.push((Edn::tag("existing"), existing.clone()));
    }
    (Edn::Symbol(Arc::from(id.as_str())), Edn::map_from_iter(fields))
  }));
  let operations = report
    .reconciliation
    .iter()
    .filter(|(_, entry)| entry.status == ReconciliationStatus::Create)
    .map(|(id, _)| {
      Edn::map_from_iter([
        (Edn::tag("operation"), Edn::tag("create-definition")),
        (Edn::tag("target"), Edn::Symbol(Arc::from(id.as_str()))),
      ])
    })
    .collect::<Vec<_>>();
  Edn::map_from_iter([
    (Edn::tag("schema-version"), Edn::from(1)),
    (Edn::tag("ok"), Edn::Bool(true)),
    (Edn::tag("command"), Edn::tag("edit-scaffold")),
    (Edn::tag("feature"), Edn::Symbol(Arc::from(report.plan.feature.as_str()))),
    (Edn::tag("plan-id"), Edn::str(report.plan_id.as_str())),
    (Edn::tag("dry-run"), Edn::Bool(dry_run)),
    (Edn::tag("changed"), Edn::Bool(apply_result.changed)),
    (Edn::tag("snapshot-revision"), Edn::str(report.snapshot_revision.as_str())),
    (
      Edn::tag("new-snapshot-revision"),
      Edn::str(apply_result.new_snapshot_revision.as_str()),
    ),
    (Edn::tag("normalized-plan"), plan_to_edn(&report.plan)),
    (Edn::tag("reconciliation"), reconciliation),
    (Edn::tag("operations"), Edn::List(EdnListView(operations))),
    (
      Edn::tag("work-items"),
      Edn::List(EdnListView(report.work_items.iter().map(work_item_to_edn).collect())),
    ),
    (Edn::tag("diagnostics"), Edn::List(EdnListView(report.diagnostics.clone()))),
  ])
}

fn plan_to_edn(plan: &ArchitecturePlan) -> Edn {
  let definitions = Edn::map_from_iter(
    plan
      .definitions
      .iter()
      .map(|(id, spec)| (Edn::Symbol(Arc::from(id.as_str())), spec.raw.clone())),
  );
  let roots = Edn::Set(EdnSetView(
    plan
      .roots
      .iter()
      .map(|root| Edn::Symbol(Arc::from(root.as_str())))
      .collect::<HashSet<_>>(),
  ));
  let edges = Edn::Set(EdnSetView(plan.edges.iter().map(edge_to_edn).collect::<HashSet<_>>()));
  let mut fields = vec![
    (Edn::tag("schema-version"), Edn::from(1)),
    (Edn::tag("feature"), Edn::Symbol(Arc::from(plan.feature.as_str()))),
    (Edn::tag("roots"), roots),
    (Edn::tag("definitions"), definitions),
    (Edn::tag("edges"), edges),
  ];
  if let Some(doc) = &plan.doc {
    fields.insert(2, (Edn::tag("doc"), Edn::str(doc.as_str())));
  }
  Edn::map_from_iter(fields)
}

fn work_item_to_edn(item: &WorkItem) -> Edn {
  Edn::map_from_iter([
    (Edn::tag("id"), Edn::Symbol(Arc::from(item.id.as_str()))),
    (Edn::tag("plan-id"), Edn::str(item.plan_id.as_str())),
    (Edn::tag("base-snapshot-revision"), Edn::str(item.base_snapshot_revision.as_str())),
    (Edn::tag("target"), Edn::Symbol(Arc::from(item.target.as_str()))),
    (
      Edn::tag("write-set"),
      Edn::Set(EdnSetView(HashSet::from([Edn::Symbol(Arc::from(item.target.as_str()))]))),
    ),
    (Edn::tag("doc"), Edn::str(item.doc.as_str())),
    (
      Edn::tag("params"),
      Edn::List(EdnListView(
        item.params.iter().map(|param| Edn::Symbol(Arc::from(param.as_str()))).collect(),
      )),
    ),
    (Edn::tag("schema"), item.schema.clone()),
    (
      Edn::tag("planned-edges"),
      Edn::Set(EdnSetView(item.planned_edges.iter().map(edge_to_edn).collect::<HashSet<_>>())),
    ),
  ])
}

fn edge_to_edn(edge: &ArchitectureEdge) -> Edn {
  Edn::enum_value(
    edge.kind.as_str(),
    vec![
      Edn::Symbol(Arc::from(edge.source.as_str())),
      Edn::Symbol(Arc::from(edge.target.as_str())),
    ],
  )
}

fn existing_entry_to_edn(entry: &CodeEntry) -> Edn {
  Edn::map_from_iter([
    (Edn::tag("doc"), Edn::str(entry.doc.as_str())),
    (Edn::tag("kind"), Edn::tag(code_entry_kind(entry).as_tag())),
    (Edn::tag("schema"), snapshot::schema_annotation_to_edn(entry.schema.as_ref())),
    (
      Edn::tag("definition-revision"),
      Edn::str(definition_revision(entry).unwrap_or_else(|_| "unavailable".to_string())),
    ),
  ])
}

fn diagnostic(level: &str, code: &str, definition: &str, message: &str) -> Edn {
  Edn::map_from_iter([
    (Edn::tag("level"), Edn::tag(level)),
    (Edn::tag("code"), Edn::tag(code)),
    (Edn::tag("definition"), Edn::Symbol(Arc::from(definition))),
    (Edn::tag("message"), Edn::str(message)),
  ])
}

fn plan_id(plan: &ArchitecturePlan) -> Result<String, String> {
  let rendered =
    cirru_edn::format(&plan_to_edn(plan), true).map_err(|error| format!("Failed to canonicalize architecture plan: {error}"))?;
  Ok(content_revision(&rendered))
}

fn content_revision(content: &str) -> String {
  let mut hasher = Md5::new();
  hasher.update(content.as_bytes());
  format!("md5:{:x}", hasher.finalize())
}

fn code_entry_kind(entry: &CodeEntry) -> DefinitionKind {
  match &entry.code {
    Cirru::List(items) if matches!(items.first(), Some(Cirru::Leaf(head)) if matches!(head.as_ref(), "defn" | "defmacro")) => {
      DefinitionKind::Function
    }
    _ => DefinitionKind::Data,
  }
}

fn entry_has_scaffold_todo(entry: &CodeEntry) -> bool {
  entry.tags.iter().any(|tag| tag.ref_str() == "scaffold") || cirru_contains_leaf(&entry.code, "todo!")
}

fn cirru_contains_leaf(node: &Cirru, expected: &str) -> bool {
  match node {
    Cirru::Leaf(value) => value.as_ref() == expected,
    Cirru::List(items) => items.iter().any(|item| cirru_contains_leaf(item, expected)),
  }
}

fn print_human_report(report: &ScaffoldPlanReport, dry_run: bool, apply_result: &ScaffoldApplyResult) {
  println!("{} scaffold", if dry_run { "Validated" } else { "Applied" });
  println!("Feature: {}", report.plan.feature);
  println!("Plan: {}", report.plan_id);
  println!("Snapshot revision: {}", report.snapshot_revision);
  for (id, entry) in &report.reconciliation {
    println!("- {id} [{}]", entry.status.as_tag());
  }
  println!("Work items: {}", report.work_items.len());
  for item in &report.work_items {
    println!("  - {} ({})", item.target, item.id);
  }
  if !report.diagnostics.is_empty() {
    println!("Warnings: {}", report.diagnostics.len());
  }
  println!("Changed: {}", apply_result.changed);
  println!("New snapshot revision: {}", apply_result.new_snapshot_revision);
}

fn required_number(map: &EdnMapView, key: &str) -> Result<f64, String> {
  map
    .tag_get(key)
    .ok_or_else(|| format!("Architecture is missing :{key}."))?
    .read_number()
}

fn required_tag(map: &EdnMapView, key: &str) -> Result<String, String> {
  Ok(
    map
      .tag_get(key)
      .ok_or_else(|| format!("Architecture is missing :{key}."))?
      .read_tag_str()?
      .to_string(),
  )
}

fn required_symbol(map: &EdnMapView, key: &str) -> Result<String, String> {
  edn_symbol(
    map.tag_get(key).ok_or_else(|| format!("Architecture is missing :{key}."))?,
    &format!("Architecture :{key}"),
  )
}

fn optional_string(map: &EdnMapView, key: &str) -> Result<Option<String>, String> {
  match map.tag_get(key) {
    None | Some(Edn::Nil) => Ok(None),
    Some(Edn::Str(value)) => Ok(Some(value.to_string())),
    Some(other) => Err(format!("Architecture :{key} must be a string, got {}.", other.type_name())),
  }
}

fn required_symbol_set(map: &EdnMapView, key: &str) -> Result<BTreeSet<String>, String> {
  let value = map.tag_get(key).ok_or_else(|| format!("Architecture is missing :{key}."))?;
  let Edn::Set(values) = value else {
    return Err(format!("Architecture :{key} must be a set of Symbols."));
  };
  values
    .0
    .iter()
    .map(|value| edn_symbol(value, &format!("Architecture :{key}")))
    .collect()
}

fn symbol_list(value: &Edn, context: &str) -> Result<Vec<String>, String> {
  let values = value
    .view_list()
    .map_err(|error| format!("{context} must be a list of Symbols: {error}"))?;
  values.0.iter().map(|value| edn_symbol(value, context)).collect()
}

fn edn_symbol(value: &Edn, context: &str) -> Result<String, String> {
  match value {
    Edn::Symbol(value) => Ok(value.to_string()),
    other => Err(format!("{context} must be a Symbol, got {}.", other.type_name())),
  }
}

#[cfg(test)]
mod tests {
  use super::{apply_scaffold, parse_architecture_plan, plan_id, reconcile_plan};
  use crate::cli_handlers::edit::load_snapshot;
  use crate::cli_handlers::test_support::TestProject;

  const PLAN: &str = r#"
{}
  :schema-version 1
  :feature 'sample
  :roots $ #{} 'app.main/run!
  :definitions $ {}
    'app.main/run! $ {}
      :mode :ensure
      :kind :fn
      :doc "|Run sample."
      :params $ [] 'value
      :schema $ :: :fn
        {}
          :args $ [] :number
          :return :number
    'app.main/next-value $ {}
      :mode :ensure
      :kind :fn
      :doc "|Increment a value."
      :params $ [] 'value
      :schema $ :: :fn
        {}
          :args $ [] :number
          :return :number
  :edges $ #{}
    :: :call 'app.main/run! 'app.main/next-value
"#;

  const DATA_PLAN: &str = r#"
{}
  :schema-version 1
  :feature 'sample-data
  :roots $ #{} 'app.main/scaffold-answer
  :definitions $ {}
    'app.main/scaffold-answer $ {}
      :mode :ensure
      :kind :data
      :doc "|An answer created by the scaffold."
      :schema $ :: 'Number
      :code $ quote (def scaffold-answer 42)
"#;

  #[test]
  fn parses_flat_symbol_graph_and_has_stable_plan_id() {
    let plan = parse_architecture_plan(PLAN).expect("plan should parse");
    assert_eq!(plan.feature, "sample");
    assert_eq!(plan.definitions.len(), 2);
    assert_eq!(plan.edges.len(), 1);
    assert_eq!(
      plan_id(&plan).expect("plan id should render"),
      plan_id(&plan).expect("plan id should be stable")
    );
  }

  #[test]
  fn rejects_list_edge_that_mixes_tag_and_symbols() {
    let invalid = PLAN.replace(
      ":: :call 'app.main/run! 'app.main/next-value",
      "[] :call 'app.main/run! 'app.main/next-value",
    );
    let error = parse_architecture_plan(&invalid).expect_err("list edge should be rejected");
    assert!(error.contains("anonymous enum"), "unexpected error: {error}");
  }

  #[test]
  fn rejects_string_params_instead_of_symbols() {
    let invalid = PLAN.replace("[] 'value", "[] |value");
    let error = parse_architecture_plan(&invalid).expect_err("string params should be rejected");
    assert!(error.contains("Symbol"), "unexpected error: {error}");
  }

  #[test]
  fn missing_function_becomes_a_work_item_without_writing_snapshot() {
    let fixture = TestProject::from_fixture();
    let snapshot_file = fixture.snapshot_string();
    let snapshot = load_snapshot(&snapshot_file).expect("fixture snapshot should load");
    let original = std::fs::read_to_string(&snapshot_file).expect("fixture snapshot should be readable");
    let report = reconcile_plan(
      parse_architecture_plan(PLAN).expect("plan should parse"),
      &snapshot,
      "md5:test".to_string(),
    )
    .expect("plan should reconcile");
    assert_eq!(report.work_items.len(), 2);
    assert!(report.work_items.iter().all(|item| item.base_snapshot_revision == "md5:test"));
    assert_eq!(
      std::fs::read_to_string(&snapshot_file).expect("fixture snapshot should remain readable"),
      original
    );
  }

  #[test]
  fn apply_creates_all_missing_todo_stubs_atomically() {
    let fixture = TestProject::from_fixture();
    let snapshot_file = fixture.snapshot_string();
    let original = std::fs::read_to_string(&snapshot_file).expect("fixture snapshot should be readable");
    let snapshot = load_snapshot(&snapshot_file).expect("fixture snapshot should load");
    let report = reconcile_plan(
      parse_architecture_plan(PLAN).expect("plan should parse"),
      &snapshot,
      super::content_revision(&original),
    )
    .expect("plan should reconcile");

    let outcome = apply_scaffold(&report, &snapshot, &snapshot_file, &original).expect("scaffold should apply");
    assert!(outcome.changed);
    let applied = load_snapshot(&snapshot_file).expect("applied snapshot should load");
    for definition in ["run!", "next-value"] {
      let entry = applied
        .files
        .get("app.main")
        .and_then(|file| file.defs.get(definition))
        .expect("scaffolded definition should exist");
      assert!(entry.tags.iter().any(|tag| tag.ref_str() == "scaffold"));
      assert!(super::cirru_contains_leaf(&entry.code, "todo!"));
    }
  }

  #[test]
  fn apply_preserves_explicit_data_code() {
    let fixture = TestProject::from_fixture();
    let snapshot_file = fixture.snapshot_string();
    let original = std::fs::read_to_string(&snapshot_file).expect("fixture snapshot should be readable");
    let snapshot = load_snapshot(&snapshot_file).expect("fixture snapshot should load");
    let report = reconcile_plan(
      parse_architecture_plan(DATA_PLAN).expect("data plan should parse"),
      &snapshot,
      super::content_revision(&original),
    )
    .expect("data plan should reconcile");

    apply_scaffold(&report, &snapshot, &snapshot_file, &original).expect("data scaffold should apply");
    let applied = load_snapshot(&snapshot_file).expect("applied snapshot should load");
    let entry = applied
      .files
      .get("app.main")
      .and_then(|file| file.defs.get("scaffold-answer"))
      .expect("scaffolded data should exist");
    assert!(!super::cirru_contains_leaf(&entry.code, "todo!"));
    assert!(super::cirru_contains_leaf(&entry.code, "42"));
  }
}
