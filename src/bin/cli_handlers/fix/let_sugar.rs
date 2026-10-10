//! `let-sugar-to-let-v1`: migrate `let-sugar` to `let` and `let[]`.
//!
//! `let-sugar ((a x) ([] b c) y) body...` binds each pair in order through
//! `let-destruct`, which turns a symbol into `&let` and a `([] ...)` pattern
//! into `let[]`. The rewrite writes that sequence back to source: consecutive
//! symbol pairs share one sequential `let`, and each list pattern becomes a
//! nested `let[]`, so every value is evaluated once in the original order.
//! `({} ...)` patterns depend on the deprecated `let{}` and stay review-only,
//! as do calls in templates, under shadowing, or under macros that may observe
//! their spelling.

use super::case_default::{ContextSubject, MacroHeadCache, unstable_macro_context};
use super::*;

pub(super) fn plan_let_sugar_fixes(
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
    suggestions.extend(plan_let_sugar_source(snapshot, snapshot_file, namespace, definition, &entry.code)?);
  }
  Ok(suggestions)
}

/// Plan one suggestion per `let-sugar` call. A proven call nested inside
/// another proven call is folded into the outer replacement.
pub(super) fn plan_let_sugar_source(
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
  collect_let_sugar_calls(source, &mut Vec::new(), false, &mut calls);
  if calls.is_empty() {
    return Ok(Vec::new());
  }
  let mut local_bindings = HashSet::new();
  collect_potential_local_bindings(source, &mut local_bindings);
  let shadowed = |name: &str| local_bindings.contains(name) || namespace_binds_name(snapshot, namespace, name);
  let mut macro_heads = MacroHeadCache::new();
  let mut reviews = BTreeMap::new();
  for (path, in_template) in &calls {
    let node = navigate_to_path(source, path)?;
    let review = if *in_template {
      Some("`let-sugar` is inside a quasiquote template and resolves at each expansion site; review the macro users.".to_owned())
    } else if let Some(name) = ["let-sugar", "let", "let[]"].into_iter().find(|name| shadowed(name)) {
      Some(format!(
        "`{name}` is bound by a local, a namespace definition or an import in `{namespace}`; review which definition the call resolves to."
      ))
    } else if let Some(reason) = unstable_macro_context(
      snapshot,
      snapshot_file,
      namespace,
      source,
      path,
      ContextSubject {
        name: "let-sugar",
        transparent: &[],
      },
      &mut macro_heads,
    ) {
      Some(reason)
    } else {
      unsupported_shape(&node).err()
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
    let replacement = machine_applicable.then(|| rewrite_let_sugar_tree(&original, &mut path.clone(), &proven));
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
      rule_id: LET_SUGAR_LET_RULE,
      diagnostic_code: LET_SUGAR_LET_DIAGNOSTIC,
      semantic_layer: "surface",
      source_file: snapshot_file.to_owned(),
      definition: format!("{namespace}/{definition}"),
      path: format!("code{}", format_path(&path)),
      fingerprint: node_fingerprint(&original),
      origin_chain: vec![serde_json::json!({
        "kind": "core-macro-expansion",
        "target": "calcit.core/let-sugar",
        "replacement": "let / let[]",
      })],
      original: quoted_json(&original),
      replacement: replacement.as_ref().map(quoted_json),
      applicability: if machine_applicable {
        "machine-applicable"
      } else {
        "requires-review"
      },
      message: review.unwrap_or_else(|| {
        "Bind symbols with `let` and list patterns with `let[]` in the original order; this is the expansion of `let-sugar`.".to_owned()
      }),
      target_path: path,
      operation,
    });
  }
  Ok(suggestions)
}

/// Collect `let-sugar` call paths outside quoted data and comments, marking
/// calls that sit in a quasiquote template outside any unquote.
fn collect_let_sugar_calls(node: &Cirru, path: &mut Vec<usize>, in_template: bool, output: &mut Vec<(Vec<usize>, bool)>) {
  let Cirru::List(items) = node else {
    return;
  };
  let in_template = match items.first().and_then(leaf_value) {
    Some("quote" | "cirru-quote" | ";") => return,
    Some("quasiquote") => true,
    Some("~" | "~@") => false,
    _ => in_template,
  };
  if items.first().and_then(leaf_value) == Some("let-sugar") {
    output.push((path.clone(), in_template));
  }
  for (index, child) in items.iter().enumerate() {
    path.push(index);
    collect_let_sugar_calls(child, path, in_template, output);
    path.pop();
  }
}

enum Binding<'a> {
  /// `(name value)`, bound through `&let`.
  Symbol,
  /// `([] vars...) value`, bound through `let[]`.
  List(&'a [Cirru], &'a Cirru),
}

/// Read the pairs `let-sugar` accepts without review, or explain why the call
/// needs a person.
fn unsupported_shape(node: &Cirru) -> Result<(Vec<Binding<'_>>, &[Cirru]), String> {
  let Cirru::List(items) = node else {
    return Err("`let-sugar` call is not a list.".to_owned());
  };
  let (Some(Cirru::List(pairs)), true) = (items.get(1), items.len() > 2) else {
    return Err("`let-sugar` needs a binding list and at least one body expression; review the call.".to_owned());
  };
  if pairs.is_empty() {
    return Err("`let-sugar` has no bindings; replace it with its body by hand.".to_owned());
  }
  let mut bindings = Vec::with_capacity(pairs.len());
  for pair in pairs {
    let Cirru::List(pair) = pair else {
      return Err("`let-sugar` binding is not a `(pattern value)` pair; review the call.".to_owned());
    };
    let [pattern, value] = pair.as_slice() else {
      return Err("`let-sugar` binding is not a `(pattern value)` pair; review the call.".to_owned());
    };
    match pattern {
      Cirru::Leaf(name) if !name.is_empty() && name.as_ref() != "&" => bindings.push(Binding::Symbol),
      Cirru::List(parts) if parts.len() > 1 && list_head(pattern) == Some("[]") => bindings.push(Binding::List(&parts[1..], value)),
      Cirru::List(_) if list_head(pattern) == Some("{}") => {
        return Err("A `({} ...)` pattern expands to the deprecated `let{}`; bind each field with `let` by hand.".to_owned());
      }
      _ => {
        return Err(format!(
          "Pattern `{}` is neither a symbol nor a `([] ...)` list pattern; review the call.",
          pattern.format_one_liner().unwrap_or_default()
        ));
      }
    }
  }
  Ok((bindings, &items[2..]))
}

/// Rewrite every proven `let-sugar` call in `node`, innermost first.
fn rewrite_let_sugar_tree(node: &Cirru, path: &mut Vec<usize>, proven: &HashSet<Vec<usize>>) -> Cirru {
  let Cirru::List(items) = node else {
    return node.clone();
  };
  let rewritten = Cirru::List(
    items
      .iter()
      .enumerate()
      .map(|(index, item)| {
        path.push(index);
        let rewritten = rewrite_let_sugar_tree(item, path, proven);
        path.pop();
        rewritten
      })
      .collect(),
  );
  if !proven.contains(path) {
    return rewritten;
  }
  let Ok((bindings, body)) = unsupported_shape(&rewritten) else {
    return rewritten;
  };
  let Cirru::List(rewritten_items) = &rewritten else {
    return rewritten;
  };
  let Cirru::List(rewritten_pairs) = &rewritten_items[1] else {
    return rewritten;
  };
  let mut inner = body.to_vec();
  let mut index = bindings.len();
  while index > 0 {
    match &bindings[index - 1] {
      Binding::List(vars, value) => {
        let mut node = vec![Cirru::leaf("let[]"), Cirru::List(vars.to_vec()), (*value).clone()];
        node.extend(inner);
        inner = vec![Cirru::List(node)];
        index -= 1;
      }
      Binding::Symbol => {
        let start = (0..index)
          .rev()
          .take_while(|position| matches!(bindings[*position], Binding::Symbol))
          .last()
          .unwrap_or(index - 1);
        let mut node = vec![Cirru::leaf("let"), Cirru::List(rewritten_pairs[start..index].to_vec())];
        node.extend(inner);
        inner = vec![Cirru::List(node)];
        index = start;
      }
    }
  }
  inner.into_iter().next().unwrap_or(rewritten)
}
