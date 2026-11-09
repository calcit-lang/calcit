//! Locals and lambda parameters whose compiled type is still `Dynamic` (#2650).
//!
//! The source scans in `weak-types` only see a `Dynamic` that is written in the
//! code. A binding the compiler could not type leaves no source trace, so this
//! view compiles each definition and reads the binding nodes of its
//! preprocessed form instead.

use super::{format_cirru_path, scoped_definition_entries};
use crate::cli_handlers;
use calcit::calcit::{Calcit, CalcitLocal, CalcitSyntax, CalcitTypeAnnotation, MethodKind};
use calcit::call_stack::CallStackList;
use calcit::cli_args::WeakTypesCommand;
use calcit::{program, runner, snapshot};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const VIEW: &str = "inferred-dynamic";

/// Where an open binding got its type from, as far as the compiled form shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Source {
  /// Read from a local that is already `Dynamic`, usually a `Dynamic` parameter.
  OpenLocal,
  /// A field or key read from a map, record or tag access.
  MapField,
  /// A JavaScript global, native method call or host field read.
  HostValue,
  /// No open input is visible: the compiler did not infer a type.
  InferenceFailure,
}

impl Source {
  fn as_str(self) -> &'static str {
    match self {
      Self::OpenLocal => "dynamic-source",
      Self::MapField => "map-field",
      Self::HostValue => "host-value",
      Self::InferenceFailure => "inference-failure",
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Finding {
  pub coord: Vec<usize>,
  pub name: String,
  /// `let` or `param`.
  pub binding: &'static str,
  pub source: Source,
  /// `Dynamic` or `Option Dynamic`, as rendered by the type query.
  pub type_name: String,
}

/// `Dynamic`, or `Option Dynamic`: a field read from an open map is the same
/// unknown value behind an Option.
fn is_open(annotation: &CalcitTypeAnnotation) -> bool {
  match annotation {
    CalcitTypeAnnotation::Dynamic => true,
    CalcitTypeAnnotation::TypeRef(name, args) if matches!(name.as_ref(), "Option" | "calcit.core/Option") => {
      matches!(args.as_slice(), [only] if matches!(only.as_ref(), CalcitTypeAnnotation::Dynamic))
    }
    CalcitTypeAnnotation::Enum(definition, args)
      if definition.definition_ref().map(|name| name.as_ref()) == Some("calcit.core/Option") =>
    {
      matches!(args.as_slice(), [only] if matches!(only.as_ref(), CalcitTypeAnnotation::Dynamic))
    }
    _ => false,
  }
}

/// A local whose type is `Dynamic` or holds it, such as `List Dynamic`.
fn is_open_local(node: &Calcit) -> bool {
  matches!(node, Calcit::Local(local) if runner::preprocess::contains_dynamic_type(&local.type_info))
}

/// An open local read anywhere in `node`, outside nested lambdas.
fn reads_open_local(node: &Calcit) -> bool {
  match node {
    Calcit::List(items) => {
      !matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::Defn | CalcitSyntax::Quote, _))) && items.iter().any(reads_open_local)
    }
    other => is_open_local(other),
  }
}

fn is_host_node(node: &Calcit) -> bool {
  match node {
    Calcit::RawCode(..) => true,
    Calcit::Method(
      _,
      MethodKind::InvokeNative
      | MethodKind::InvokeNativeOptional
      | MethodKind::Access
      | MethodKind::AccessOptional
      | MethodKind::ExternalAccess(_)
      | MethodKind::ExternalGet(_)
      | MethodKind::ExternalInvoke(_),
    ) => true,
    Calcit::List(items) => items.first().is_some_and(is_host_node) || items.get(1).is_some_and(is_host_node),
    _ => false,
  }
}

fn is_field_read(node: &Calcit) -> bool {
  let Calcit::List(items) = node else { return false };
  let head_is_field = |head: &Calcit| match head {
    Calcit::Tag(_) => true,
    Calcit::Method(_, MethodKind::TagAccess) => true,
    Calcit::Proc(calcit::calcit::CalcitProc::NativeMapGet) => true,
    Calcit::Import(import) => import.ns.as_ref() == calcit::calcit::CORE_NS && matches!(import.def.as_ref(), "get" | "get-in"),
    _ => false,
  };
  items.first().is_some_and(head_is_field) || items.get(1).is_some_and(head_is_field)
}

/// Classify a `let` initializer by its outermost form; nested inputs are not
/// traced, so a mixed expression keeps the category of its own head.
fn classify_value(value: &Calcit) -> Source {
  if is_host_node(value) {
    Source::HostValue
  } else if is_field_read(value) {
    Source::MapField
  } else if reads_open_local(value) {
    Source::OpenLocal
  } else {
    Source::InferenceFailure
  }
}

/// Classify the parameters of a lambda passed as an argument by the other
/// arguments of that call: an open collection gives open elements. A lambda
/// with no open input in its call has no contract for its parameters.
fn classify_call_siblings(siblings: &[&Calcit]) -> Source {
  if siblings.iter().any(|node| is_host_node(node)) {
    Source::HostValue
  } else if siblings.iter().any(|node| reads_open_local(node)) {
    Source::OpenLocal
  } else {
    Source::InferenceFailure
  }
}

fn push_local(local: &CalcitLocal, binding: &'static str, source: Source, findings: &mut BTreeSet<Finding>) {
  // Generated bindings of macro expansions have no source coordinate.
  let Some(coord) = local.location.as_ref() else { return };
  if !is_open(&local.type_info) || local.sym.starts_with('&') {
    return;
  }
  findings.insert(Finding {
    coord: coord.iter().map(|index| usize::from(*index)).collect(),
    name: local.sym.to_string(),
    binding,
    source,
    type_name: cli_handlers::format_type_query_annotation(&local.type_info).unwrap_or_else(|_| local.type_info.to_brief_string()),
  });
}

fn collect(node: &Calcit, top_level: bool, lambda_source: Option<Source>, findings: &mut BTreeSet<Finding>) {
  let Calcit::List(items) = node else { return };
  match items.first() {
    Some(Calcit::Syntax(CalcitSyntax::Quote | CalcitSyntax::Quasiquote | CalcitSyntax::Defmacro, _)) => return,
    Some(Calcit::Syntax(CalcitSyntax::CoreLet, _)) => {
      if let Some(Calcit::List(pair)) = items.get(1)
        && let (Some(Calcit::Local(local)), Some(value)) = (pair.first(), pair.get(1))
      {
        push_local(local, "let", classify_value(value), findings);
      }
    }
    Some(Calcit::Syntax(CalcitSyntax::Defn, _)) => {
      // Parameters of the definition itself follow its schema, which the
      // `schema-dynamic` kind already reports.
      if !top_level && let Some(Calcit::List(params)) = items.get(2) {
        for param in params.iter() {
          if let Calcit::Local(local) = param {
            push_local(local, "param", lambda_source.unwrap_or(Source::InferenceFailure), findings);
          }
        }
      }
    }
    _ => {}
  }
  let children = items.iter().collect::<Vec<_>>();
  for (index, child) in children.iter().enumerate() {
    let is_lambda = matches!(child, Calcit::List(inner) if matches!(inner.first(), Some(Calcit::Syntax(CalcitSyntax::Defn, _))));
    let source = (is_lambda && index > 0).then(|| {
      let siblings = children
        .iter()
        .enumerate()
        .filter(|(other, _)| *other != index && *other > 0)
        .map(|(_, node)| *node)
        .collect::<Vec<_>>();
      classify_call_siblings(&siblings)
    });
    collect(child, false, source, findings);
  }
}

/// Open bindings in one compiled definition, ordered by source coordinate.
pub(crate) fn definition_findings(namespace: &str, definition: &str) -> Result<Vec<Finding>, String> {
  let warnings = RefCell::new(Vec::new());
  runner::preprocess::compile_source_def_for_snapshot(namespace, definition, &warnings, &CallStackList::default())
    .map_err(|failure| failure.msg)?;
  let compiled =
    program::lookup_compiled_def(namespace, definition).ok_or_else(|| format!("no compiled form for {namespace}/{definition}"))?;
  let mut findings = BTreeSet::new();
  collect(&compiled.preprocessed_code, true, None, &mut findings);
  Ok(findings.into_iter().collect())
}

pub(crate) fn run(options: &WeakTypesCommand, snapshot: &snapshot::Snapshot) -> Result<(), String> {
  if !matches!(options.format.as_str(), "human" | "text" | "edn" | "json") {
    return Err(format!(
      "Unknown weak-types output format `{}`. Expected `human`, `edn`, or `json`.",
      options.format
    ));
  }
  let entries = scoped_definition_entries(snapshot, options.ns.as_deref(), options.ns_prefix.as_deref(), options.deps)?;
  cli_handlers::prepare_program_for_type_query(snapshot)?;
  let mut findings = Vec::new();
  let mut failed = Vec::new();
  let mut by_source: BTreeMap<&'static str, usize> = BTreeMap::new();
  let mut by_binding: BTreeMap<&'static str, usize> = BTreeMap::new();
  for (namespace, definition, _) in entries {
    match definition_findings(namespace, definition) {
      Ok(rows) => {
        for row in rows {
          *by_source.entry(row.source.as_str()).or_default() += 1;
          *by_binding.entry(row.binding).or_default() += 1;
          findings.push(json!({
            "definition": format!("{namespace}/{definition}"),
            "path": format_cirru_path("code", &row.coord),
            "name": row.name,
            "binding": row.binding,
            "source": row.source.as_str(),
            "type": row.type_name,
          }));
        }
      }
      // A definition that does not compile has no compiled bindings to read;
      // list it so a gate does not mistake it for a clean one.
      Err(message) => failed.push(json!({ "definition": format!("{namespace}/{definition}"), "message": message })),
    }
  }
  let report = json!({
    "schema_version": 1,
    "command": "analyze.weak-types",
    "data": {
      "view": VIEW,
      "filters": { "ns": options.ns, "ns_prefix": options.ns_prefix, "deps": options.deps },
      "summary": {
        "findings": findings.len(),
        "by_source": by_source,
        "by_binding": by_binding,
        "uncompiled_definitions": failed.len(),
      },
      "findings": if options.summary_only { Vec::<Value>::new() } else { findings.clone() },
      "uncompiled": failed,
    },
    "diagnostics": [],
  });
  match options.format.as_str() {
    "edn" => println!("{}", cli_handlers::format_json_value_as_edn(&report)?),
    "json" => println!("{}", serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?),
    _ => {
      println!("# Inferred Dynamic bindings\n");
      println!(
        "{} binding(s) compiled as `Dynamic`: {} let, {} lambda param.",
        findings.len(),
        by_binding.get("let").copied().unwrap_or(0),
        by_binding.get("param").copied().unwrap_or(0)
      );
      for (source, count) in &by_source {
        println!("- {source}: {count}");
      }
      if !failed.is_empty() {
        println!("- definitions that did not compile: {}", failed.len());
      }
      if !options.summary_only {
        let mut current = "";
        for finding in &findings {
          let definition = finding["definition"].as_str().unwrap_or_default();
          if definition != current {
            println!("\n## {definition}\n");
            current = definition;
          }
          println!(
            "- {} `{}` @ {}: {} ({})",
            finding["binding"].as_str().unwrap_or_default(),
            finding["name"].as_str().unwrap_or_default(),
            finding["path"].as_str().unwrap_or_default(),
            finding["type"].as_str().unwrap_or_default(),
            finding["source"].as_str().unwrap_or_default()
          );
        }
        for item in &failed {
          println!(
            "\n- not compiled: {} ({})",
            item["definition"].as_str().unwrap_or_default(),
            item["message"].as_str().unwrap_or_default()
          );
        }
      }
    }
  }
  Ok(())
}
