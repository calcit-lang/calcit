use super::*;
use calcit::calcit::CalcitSyntax;

fn collect_spread_calls(node: &Cirru, path: &mut Vec<usize>, calls: &mut Vec<Vec<usize>>) {
  let Cirru::List(items) = node else { return };
  if matches!(items.first(), Some(Cirru::Leaf(head)) if matches!(head.as_ref(), "quote" | "quasiquote")) {
    return;
  }
  let parameter_index = match list_head(node) {
    Some("defn" | "defwasm-export" | "defwasm-import") => Some(2),
    Some("fn") => Some(1),
    Some("hint-fn") => return,
    _ => None,
  };
  if items
    .iter()
    .skip(1)
    .any(|item| matches!(item, Cirru::Leaf(name) if name.as_ref() == "&"))
  {
    calls.push(path.clone());
  }
  for (index, child) in items.iter().enumerate() {
    if parameter_index == Some(index) {
      continue;
    }
    path.push(index);
    collect_spread_calls(child, path, calls);
    path.pop();
  }
}

/// Require an actual compiler CallSpread and List constructor, not matching source text.
fn fixed_spread_is_proven(source: &Cirru, processed: &Calcit) -> bool {
  let Cirru::List(source_items) = source else { return false };
  if source_items.len() < 3
    || !matches!(&source_items[source_items.len() - 2], Cirru::Leaf(name) if name.as_ref() == "&")
    || source_items[1..source_items.len() - 2]
      .iter()
      .any(|item| matches!(item, Cirru::Leaf(name) if name.as_ref() == "&"))
  {
    return false;
  }
  let Some(Cirru::List(literal)) = source_items.last() else {
    return false;
  };
  if !matches!(literal.first(), Some(Cirru::Leaf(head)) if matches!(head.as_ref(), "[]" | "calcit.core/[]")) {
    return false;
  }
  let Calcit::List(call) = processed else { return false };
  let items = call.to_vec();
  if items.len() != source_items.len() + 1
    || !matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::CallSpread, _)))
    || !matches!(&items[items.len() - 2], Calcit::Syntax(CalcitSyntax::ArgSpread, _))
  {
    return false;
  }
  let Some(Calcit::List(list)) = items.last() else { return false };
  let elements = list.to_vec();
  if elements.len() != literal.len() || !matches!(elements.first(), Some(Calcit::Proc(CalcitProc::List))) {
    return false;
  }
  let mut arguments = items[2..items.len() - 2].to_vec();
  arguments.extend_from_slice(&elements[1..]);
  runner::preprocess::fixed_call_arguments_are_proven(&items[1], &arguments)
}

fn rewrite_proven_spreads(node: &Cirru, path: &mut Vec<usize>, proven: &HashSet<Vec<usize>>) -> Cirru {
  let Cirru::List(items) = node else { return node.clone() };
  let mut rewritten = items
    .iter()
    .enumerate()
    .map(|(index, child)| {
      path.push(index);
      let result = rewrite_proven_spreads(child, path, proven);
      path.pop();
      result
    })
    .collect::<Vec<_>>();
  if proven.contains(path) {
    let Some(Cirru::List(literal)) = rewritten.pop() else {
      unreachable!("proven spread has a literal List")
    };
    rewritten.pop();
    rewritten.extend_from_slice(&literal[1..]);
  }
  Cirru::List(rewritten)
}

/// Keep unknown contracts and ambiguous macro origins visible, without speculative writes.
pub(super) fn plan_spread_call_fixes(
  snapshot: &Snapshot,
  snapshot_file: &str,
  selected_definitions: &[(String, String)],
) -> Result<Vec<FixSuggestion>, String> {
  let mut suggestions = Vec::new();
  for (namespace, definition) in selected_definitions {
    let entry = snapshot
      .files
      .get(namespace)
      .and_then(|file| file.defs.get(definition))
      .ok_or_else(|| format!("Missing source definition {namespace}/{definition}"))?;
    if list_head(&entry.code) == Some("defmacro") {
      continue;
    }
    let mut calls = Vec::new();
    collect_spread_calls(&entry.code, &mut Vec::new(), &mut calls);
    if calls.is_empty() {
      continue;
    }
    let usages =
      runner::preprocess::trace_definition_source_usages(namespace, definition, &RefCell::new(Vec::new()), &CallStackList::default())
        .map_err(|failure| failure.msg)?;
    let expressions = runner::preprocess::trace_definition_source_expressions(
      namespace,
      definition,
      &RefCell::new(Vec::new()),
      &CallStackList::default(),
    )
    .map_err(|failure| failure.msg)?;
    let proven = calls
      .iter()
      .filter(|path| {
        let Ok(source) = navigate_to_path(&entry.code, path) else {
          return false;
        };
        let Some(evidence) = runner::preprocess::unique_source_expression_at_path(&expressions, namespace, definition, path) else {
          return false;
        };
        let mut head_path = path.to_vec();
        head_path.push(0);
        let source_head_is_macro = usages.iter().any(|usage| {
          usage.location.as_ref().is_some_and(|location| {
            location.ns.as_ref() == namespace
              && location.def.as_ref() == definition
              && location.coord.iter().map(|index| usize::from(*index)).eq(head_path.iter().copied())
              && matches!(
                program::lookup_compiled_def(&usage.target_ns, &usage.target_def).map(|compiled| compiled.kind),
                Some(program::CompiledDefKind::Macro)
              )
          })
        });
        !source_head_is_macro
          && fixed_spread_is_proven(&source, &evidence.processed)
          && method_source_context_is_stable(&entry.code, path, namespace, definition, &usages)
          && !usages.iter().any(|usage| {
            usage.location.as_ref().is_some_and(|location| {
              location.ns.as_ref() == namespace
                && location.def.as_ref() == definition
                && location.coord.len() >= path.len()
                && location
                  .coord
                  .iter()
                  .map(|index| usize::from(*index))
                  .zip(path.iter().copied())
                  .all(|(actual, expected)| actual == expected)
                && usage
                  .macro_origin
                  .iter()
                  .any(|origin| !preserves_nominal_method_call_through_macro(origin))
            })
          })
      })
      .cloned()
      .collect::<HashSet<_>>();
    for path in calls {
      let machine_applicable = proven.contains(&path);
      if machine_applicable && proven.iter().any(|parent| parent.len() < path.len() && path.starts_with(parent)) {
        continue;
      }
      let original = navigate_to_path(&entry.code, &path)?;
      let replacement = machine_applicable.then(|| rewrite_proven_spreads(&original, &mut path.clone(), &proven));
      let operation = replacement
        .as_ref()
        .map(|replacement| {
          Ok::<_, String>(FixOperation::ReplaceNode {
            original: original.format_one_liner().map_err(|error| error.to_string())?,
            replacement: replacement.format_one_liner().map_err(|error| error.to_string())?,
          })
        })
        .transpose()?;
      suggestions.push(FixSuggestion {
        rule_id: SPREAD_CALL_PROOF_RULE,
        diagnostic_code: SPREAD_CALL_PROOF_DIAGNOSTIC,
        semantic_layer: "surface",
        source_file: snapshot_file.to_owned(),
        definition: format!("{namespace}/{definition}"),
        path: format!("code{}", format_path(&path)),
        fingerprint: node_fingerprint(&original),
        origin_chain: vec![serde_json::json!({"kind":"compiler-fixed-call-proof", "unique_source_call": runner::preprocess::unique_source_expression_at_path(&expressions, namespace, definition, &path).is_some(), "fixed_call_proven": machine_applicable})],
        original: quoted_json(&original),
        replacement: replacement.as_ref().map(quoted_json),
        applicability: if machine_applicable { "machine-applicable" } else { "requires-review" },
        message: if machine_applicable {
          "Expand a proven fixed literal spread; preserve the head and each argument exactly once in their original order."
        } else {
          "Cannot prove a fixed literal, exact callable contract, item types, and unique source mapping; retain the original call for review."
        }.to_owned(),
        target_path: path,
        operation,
      });
    }
  }
  Ok(suggestions)
}
