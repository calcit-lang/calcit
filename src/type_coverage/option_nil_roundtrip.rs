//! Read-only source evidence for the Option migration, not a checker or fix.

use super::{QuoteContext, format_cirru_path, scoped_definition_entries};
use crate::cli_handlers;
use calcit::calcit::{CalcitTypeAnnotation, NullableOrigin};
use calcit::call_stack::CallStackList;
use calcit::cli_args::WeakTypesCommand;
use calcit::{runner, snapshot};
use cirru_parser::Cirru;
use serde_json::{Value, json};
use std::cell::RefCell;

struct Candidate<'a> {
  expression: &'a Cirru,
  path: Vec<usize>,
  input: &'a Cirru,
  input_path: Vec<usize>,
  consumer: Option<Value>,
}

fn leaf(node: &Cirru) -> Option<&str> {
  match node {
    Cirru::Leaf(text) => Some(text),
    Cirru::List(_) => None,
  }
}

fn is_nil(node: &Cirru) -> bool {
  match node {
    Cirru::Leaf(text) => text.as_ref() == "nil",
    // `;nil` is one macro name, not the comment head `;` followed by `nil`.
    // Its arguments are discarded; a bare `;nil` leaf is the macro value.
    Cirru::List(items) => matches!(items.first().and_then(leaf), Some(";nil" | "calcit.core/;nil")),
  }
}

fn collect<'a>(
  node: &'a Cirru,
  path: &mut Vec<usize>,
  consumer: Option<Value>,
  quote: QuoteContext,
  candidates: &mut Vec<Candidate<'a>>,
) {
  let Cirru::List(items) = node else { return };
  let head = items.first().and_then(leaf);
  let core_head = head.map(|head| head.strip_prefix("calcit.core/").unwrap_or(head));
  if core_head == Some(";") || (!quote.is_quoted() && core_head == Some(";nil")) {
    return;
  }
  if !quote.is_quoted() {
    let input_index = if items.len() == 3 && items.get(2).is_some_and(is_nil) {
      if head == Some(".unwrap-or") {
        Some(1)
      } else if items.get(1).and_then(leaf) == Some(".unwrap-or") {
        Some(0)
      } else {
        None
      }
    } else {
      None
    }
    .or_else(|| {
      (core_head == Some("match")
        && items.len() >= 3
        && items.iter().skip(2).any(|branch| {
          matches!(branch, Cirru::List(parts) if parts.len() == 2
            && matches!(parts.first(), Some(Cirru::List(pattern)) if pattern.len() == 1 && leaf(&pattern[0]) == Some(":none"))
            && parts.get(1).is_some_and(is_nil))
        }))
      .then_some(1)
    });
    if let Some(index) = input_index {
      let mut input_path = path.clone();
      input_path.push(index);
      candidates.push(Candidate {
        expression: node,
        path: path.clone(),
        input: &items[index],
        input_path,
        consumer,
      });
    }
  }
  for (index, child) in items.iter().enumerate() {
    let consumer = json!({
      "path": format_cirru_path("code", path),
      "head": head,
      "child_index": index,
    });
    path.push(index);
    collect(child, path, Some(consumer), quote.for_child(core_head, index), candidates);
    path.pop();
  }
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
  for (namespace, definition, entry) in entries {
    let mut candidates = Vec::new();
    collect(&entry.code, &mut Vec::new(), None, QuoteContext::default(), &mut candidates);
    if candidates.is_empty() {
      continue;
    }
    let warnings = RefCell::new(Vec::new());
    let compiled = runner::preprocess::compile_source_def_for_snapshot(namespace, definition, &warnings, &CallStackList::default());
    let evidence = if compiled.is_ok() {
      runner::preprocess::trace_snapshot_source_expressions(
        &entry.code,
        namespace,
        definition,
        &RefCell::new(Vec::new()),
        &CallStackList::default(),
      )
      .unwrap_or_default()
    } else {
      Vec::new()
    };
    for candidate in candidates {
      let inferred = compiled.as_ref().ok().and_then(|_| {
        cli_handlers::source_expression_type_at(candidate.input, namespace, definition, &candidate.input_path, &evidence)
      });
      // Syntactically identical methods/variants on another known type are not
      // Option evidence. Missing evidence remains visible instead of vanishing.
      if inferred.as_ref().is_some_and(|annotation| {
        !matches!(annotation.as_ref(), CalcitTypeAnnotation::TypeRef(name, args) if args.len() == 1 && matches!(name.as_ref(), "Option" | "calcit.core/Option"))
          && !matches!(annotation.as_ref(), CalcitTypeAnnotation::Enum(definition, _) if definition.definition_ref().map(|name| name.as_ref()) == Some("calcit.core/Option"))
          && !matches!(annotation.as_ref(), CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::Nullable(_, NullableOrigin::Calcit))
      }) {
        continue;
      }
      let inferred_type = inferred
        .as_ref()
        .map(|annotation| cli_handlers::format_type_query_annotation(annotation))
        .transpose()?;
      findings.push(json!({
        "definition": format!("{namespace}/{definition}"),
        "path": format_cirru_path("code", &candidate.path),
        "expression": cli_handlers::format_cirru_expression(candidate.expression),
        "input": {
          "path": format_cirru_path("code", &candidate.input_path),
          "expression": cli_handlers::format_cirru_expression(candidate.input),
          "inferred_type": inferred_type,
          "confidence": cli_handlers::type_at_confidence(inferred.as_deref()),
        },
        "consumer": candidate.consumer,
      }));
    }
  }
  let unknown = findings
    .iter()
    .filter(|finding| finding["input"]["confidence"] == "unknown")
    .count();
  let report = json!({
    "schema_version": 1,
    "command": "analyze.weak-types",
    "data": {
      "view": cli_handlers::VIEW_OPTION_NIL_ROUNDTRIP,
      "filters": { "ns": options.ns, "ns_prefix": options.ns_prefix, "deps": options.deps },
      "summary": { "findings": findings.len(), "unknown": unknown },
      "findings": if options.summary_only { Vec::<Value>::new() } else { findings.clone() },
    },
    "diagnostics": [],
  });
  match options.format.as_str() {
    "edn" => println!("{}", cli_handlers::format_json_value_as_edn(&report)?),
    "json" => println!("{}", serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?),
    _ => {
      println!(
        "# Option → nil\n\n{} source location(s); {unknown} input type(s) unknown.\n\nRead-only evidence, not a diagnostic or an automatic migration.",
        findings.len()
      );
      if !options.summary_only {
        for finding in &findings {
          println!(
            "\n## {} @ {}\n",
            finding["definition"].as_str().unwrap(),
            finding["path"].as_str().unwrap()
          );
          print!(
            "{}",
            cli_handlers::markdown_fenced_block("cirru", finding["expression"].as_str().unwrap())
          );
          println!(
            "\n- Input: {}\n- Inferred type: {}",
            finding["input"]["path"].as_str().unwrap(),
            finding["input"]["inferred_type"].as_str().unwrap_or("unknown")
          );
          if let Some(consumer) = finding.get("consumer").filter(|consumer| !consumer.is_null()) {
            println!(
              "- Direct consumer: {} @ {} (child {})",
              consumer["head"].as_str().unwrap_or("expression"),
              consumer["path"].as_str().unwrap(),
              consumer["child_index"]
            );
          }
        }
      }
    }
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn nil_macro_uses_a_single_head_and_discards_its_arguments() {
    let source = cirru_parser::parse("do\n  .unwrap-or value (;nil)\n  value .unwrap-or (calcit.core/;nil)\n  match value\n    (:some item) item\n    (:none) (;nil ignored)\n  ;nil $ .unwrap-or hidden nil\n  .unwrap-or value ;nil\n  .unwrap-or value (calcit.core/nil)\n").unwrap();
    let mut candidates = Vec::new();
    collect(&source[0], &mut Vec::new(), None, QuoteContext::default(), &mut candidates);
    assert_eq!(candidates.len(), 3);
    assert!(candidates.iter().all(|candidate| leaf(candidate.input) == Some("value")));
  }

  #[test]
  fn source_inventory_respects_nested_quote_and_unquote_boundaries() {
    let source = cirru_parser::parse(
      "do\n  quote $ .unwrap-or (get data :quoted) nil\n  calcit.core/quote $ .unwrap-or (get data :qualified) nil\n  quasiquote $ []\n    .unwrap-or (get data :template) nil\n    ~ $ .unwrap-or (get data :active) nil\n    quasiquote $ ~ $ .unwrap-or (get data :nested-template) nil\n  ; $ .unwrap-or (get data :comment) nil\n",
    )
    .unwrap();
    let mut candidates = Vec::new();
    collect(&source[0], &mut Vec::new(), None, QuoteContext::default(), &mut candidates);
    assert_eq!(candidates.len(), 1);
    assert_eq!(super::super::render_cirru_inline(candidates[0].input), "(get data :active)");
  }
}
