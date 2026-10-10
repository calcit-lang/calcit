//! `list-match-to-match-v1`: migrate `list-match` to `match` over `destruct-list`.
//!
//! `list-match xs (() empty...) ((head tail) body...)` checks that `xs` is a
//! list, then runs `empty...` for an empty list or binds the first item and
//! the remaining list. `match (destruct-list xs) ((:none) empty)
//! ((:some head tail) body)` evaluates `xs` once, binds the same values and
//! keeps the branch order; a branch with several expressions becomes one `do`. `destruct-list` requires `List<T>`, so the rewrite
//! is machine-applicable only when the compiler already proves `xs` is a list;
//! an open or Dynamic subject needs a decode or narrowing step a person chooses.
//! Calls in templates, under shadowing, or under macros that may observe their
//! spelling are review-only as well.

use super::case_default::{ContextSubject, MacroHeadCache, unstable_macro_context};
use super::*;

pub(super) fn plan_list_match_fixes(
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
      .ok_or_else(|| format!("Selected definition `{namespace}/{definition}` is missing from the source snapshot."))?;
    suggestions.extend(plan_list_match_source(snapshot, snapshot_file, namespace, definition, &entry.code)?);
  }
  Ok(suggestions)
}

/// Plan one suggestion per `list-match` call. A proven call nested inside
/// another proven call is folded into the outer replacement.
pub(super) fn plan_list_match_source(
  snapshot: &Snapshot,
  snapshot_file: &str,
  namespace: &str,
  definition: &str,
  source: &Cirru,
) -> Result<Vec<FixSuggestion>, String> {
  // A macro definition's own body is expansion-time code; leave it alone.
  if list_head(source) == Some("defmacro") {
    return Ok(Vec::new());
  }
  let mut calls = Vec::new();
  collect_list_match_calls(source, &mut Vec::new(), false, &mut calls);
  if calls.is_empty() {
    return Ok(Vec::new());
  }
  let mut local_bindings = HashSet::new();
  collect_potential_local_bindings(source, &mut local_bindings);
  let shadowed = |name: &str| local_bindings.contains(name) || namespace_binds_name(snapshot, namespace, name);
  let mut macro_heads = MacroHeadCache::new();
  let mut evidence = None;
  let mut reviews = BTreeMap::new();
  for (path, in_template) in &calls {
    let node = navigate_to_path(source, path)?;
    let review = if *in_template {
      Some("`list-match` is inside a quasiquote template and resolves at each expansion site; review the macro users.".to_owned())
    } else if let Some(name) = ["list-match", "match", "destruct-list"].into_iter().find(|name| shadowed(name)) {
      Some(format!(
        "`{name}` is bound by a local, a namespace definition or an import in `{namespace}`; review which definition the call resolves to."
      ))
    } else if let Some(reason) = unstable_macro_context(
      snapshot,
      snapshot_file,
      namespace,
      source,
      path,
      // `list-match` evaluates its subject and splices each branch body unchanged.
      ContextSubject {
        name: "list-match",
        transparent: &["list-match"],
      },
      &mut macro_heads,
    ) {
      Some(reason)
    } else if let Err(reason) = branches(&node) {
      Some(reason)
    } else {
      let evidence = evidence.get_or_insert_with(|| {
        runner::preprocess::trace_snapshot_source_expressions(
          source,
          namespace,
          definition,
          &RefCell::new(vec![]),
          &CallStackList::default(),
        )
        .unwrap_or_default()
      });
      let mut subject_path = path.clone();
      subject_path.push(1);
      (!subject_is_proven_list(source, namespace, definition, &subject_path, evidence)?).then(|| {
        "The matched value is not proven to be a `List`; narrow or decode it before using `match (destruct-list ...)`.".to_owned()
      })
    };
    reviews.insert(path.clone(), review);
  }
  let proven = reviews
    .iter()
    .filter(|(_, review)| review.is_none())
    .map(|(path, _)| path.clone())
    .collect::<HashSet<_>>();
  let mut suggestions = Vec::new();
  for (path, review) in reviews {
    let machine_applicable = review.is_none();
    if machine_applicable && proven.iter().any(|parent| parent.len() < path.len() && path.starts_with(parent)) {
      continue;
    }
    let original = navigate_to_path(source, &path)?;
    let replacement = machine_applicable.then(|| rewrite_list_match_tree(&original, &mut path.clone(), &proven));
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
      rule_id: LIST_MATCH_MATCH_RULE,
      diagnostic_code: LIST_MATCH_MATCH_DIAGNOSTIC,
      semantic_layer: "surface",
      source_file: snapshot_file.to_owned(),
      definition: format!("{namespace}/{definition}"),
      path: format!("code{}", format_path(&path)),
      fingerprint: node_fingerprint(&original),
      origin_chain: vec![serde_json::json!({
        "kind": "proven-list-subject",
        "target": "calcit.core/list-match",
        "replacement": "match + calcit.core/destruct-list",
      })],
      original: quoted_json(&original),
      replacement: replacement.as_ref().map(quoted_json),
      applicability: if machine_applicable {
        "machine-applicable"
      } else {
        "requires-review"
      },
      message: review.unwrap_or_else(|| {
        "Use `match (destruct-list xs)` with `(:none)` and `(:some head tail)` branches; the subject is a proven List, so both forms bind the same values."
          .to_owned()
      }),
      target_path: path,
      operation,
    });
  }
  Ok(suggestions)
}

/// Collect `list-match` call paths outside quoted data and comments, marking
/// calls that sit in a quasiquote template outside any unquote.
fn collect_list_match_calls(node: &Cirru, path: &mut Vec<usize>, in_template: bool, output: &mut Vec<(Vec<usize>, bool)>) {
  let Cirru::List(items) = node else {
    return;
  };
  let in_template = match items.first().and_then(leaf_value) {
    Some("quote" | "cirru-quote" | ";") => return,
    Some("quasiquote") => true,
    Some("~" | "~@") => false,
    _ => in_template,
  };
  if items.first().and_then(leaf_value) == Some("list-match") {
    output.push((path.clone(), in_template));
  }
  for (index, child) in items.iter().enumerate() {
    path.push(index);
    collect_list_match_calls(child, path, in_template, output);
    path.pop();
  }
}

/// The two branches in source order: `None` for the empty branch, or the
/// head and tail binding names for the destructuring branch.
type Branch<'a> = (Option<(&'a Cirru, &'a Cirru)>, &'a [Cirru]);

/// Mirror the macro's own shape check: exactly one `(() ...)` branch and one
/// `((head tail) ...)` branch, each with a body.
fn branches(node: &Cirru) -> Result<[Branch<'_>; 2], String> {
  let shape_error =
    || "`list-match` needs a subject, one `(() ...)` branch and one `((head tail) ...)` branch; review the call.".to_owned();
  let Cirru::List(items) = node else {
    return Err(shape_error());
  };
  let [_, _, first, second] = items.as_slice() else {
    return Err(shape_error());
  };
  fn read(branch: &Cirru) -> Option<(bool, Branch<'_>)> {
    let Cirru::List(parts) = branch else {
      return None;
    };
    let (Cirru::List(pattern), true) = (parts.first()?, parts.len() > 1) else {
      return None;
    };
    match pattern.as_slice() {
      [] => Some((true, (None, &parts[1..]))),
      [head, tail] if leaf_value(head).is_some() && leaf_value(tail).is_some() => Some((false, (Some((head, tail)), &parts[1..]))),
      _ => None,
    }
  }
  let (Some((first_empty, first)), Some((second_empty, second))) = (read(first), read(second)) else {
    return Err(shape_error());
  };
  if first_empty == second_empty {
    return Err(shape_error());
  }
  Ok([first, second])
}

/// Use the same compiler evidence as `query type-at`: the traced source
/// expression, then the compiled definition, then local inference.
fn subject_is_proven_list(
  source: &Cirru,
  namespace: &str,
  definition: &str,
  subject_path: &[usize],
  evidence: &[runner::preprocess::SourceExpressionEvidence],
) -> Result<bool, String> {
  let subject = navigate_to_path(source, subject_path)?;
  let source_subject = code_to_calcit(
    &subject,
    namespace,
    definition,
    subject_path
      .iter()
      .map(|index| u16::try_from(*index).map_err(|_| format!("Path index {index} exceeds Snapshot coordinate range")))
      .collect::<Result<Vec<_>, _>>()?,
  )
  .map_err(|error| error.to_string())?;
  let compiled = program::lookup_compiled_def(namespace, definition);
  let located = compiled.as_ref().and_then(|compiled| {
    super::super::query::find_preprocessed_node_at_path(
      &compiled.preprocessed_code,
      namespace,
      definition,
      subject_path,
      matches!(subject, Cirru::List(_)),
    )
  });
  let inferred = runner::preprocess::unique_source_expression_at_path(evidence, namespace, definition, subject_path)
    .and_then(|item| item.inferred_type.clone())
    .or_else(|| located.and_then(runner::preprocess::infer_static_type_from_expr))
    .or_else(|| super::super::query::infer_type_at_target(&source_subject, located));
  Ok(inferred.is_some_and(|annotation| {
    matches!(
      runner::preprocess::resolve_namespace_type_refs_for_body(annotation, namespace).as_ref(),
      CalcitTypeAnnotation::List(_)
    )
  }))
}

/// Rewrite every proven `list-match` call in `node`, innermost first.
fn rewrite_list_match_tree(node: &Cirru, path: &mut Vec<usize>, proven: &HashSet<Vec<usize>>) -> Cirru {
  let Cirru::List(items) = node else {
    return node.clone();
  };
  let rewritten = Cirru::List(
    items
      .iter()
      .enumerate()
      .map(|(index, item)| {
        path.push(index);
        let rewritten = rewrite_list_match_tree(item, path, proven);
        path.pop();
        rewritten
      })
      .collect(),
  );
  if !proven.contains(path) {
    return rewritten;
  }
  let Ok(arms) = branches(&rewritten) else {
    return rewritten;
  };
  let Cirru::List(rewritten_items) = &rewritten else {
    return rewritten;
  };
  let mut output = vec![
    Cirru::leaf("match"),
    Cirru::List(vec![Cirru::leaf("destruct-list"), rewritten_items[1].clone()]),
  ];
  for (binding, body) in arms {
    let pattern = match binding {
      None => Cirru::List(vec![Cirru::leaf(":none")]),
      Some((head, tail)) => Cirru::List(vec![Cirru::leaf(":some"), head.clone(), tail.clone()]),
    };
    // A `match` arm holds one body expression; the macro ran several in `&let ()`.
    let body = match body {
      [single] => single.clone(),
      _ => Cirru::List(std::iter::once(Cirru::leaf("do")).chain(body.iter().cloned()).collect()),
    };
    output.push(Cirru::List(vec![pattern, body]));
  }
  Cirru::List(output)
}
