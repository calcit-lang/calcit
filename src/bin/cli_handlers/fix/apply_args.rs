//! `apply-args-to-loop-v1`: migrate `apply-args` with a literal function to `loop`.
//!
//! `apply-args (a b) (fn (x y) body...)` calls the literal function with `a`
//! and `b` (a leading `[]` in the argument list is dropped), so `recur` in the
//! body restarts that function. `loop ((x a) (y b)) body...` evaluates the
//! same values in the same order and runs the same body in a generated
//! function, so `recur` keeps its meaning. A named `defn` callee is rewritten
//! only when its body never refers to that name, because the generated loop
//! function has another name. Other callees, rest or optional parameters,
//! arity mismatches, templates, shadowing and macros that may observe the
//! spelling are review-only.

use super::case_default::{ContextSubject, MacroHeadCache, unstable_macro_context};
use super::*;

pub(super) fn plan_apply_args_fixes(
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
    suggestions.extend(plan_apply_args_source(snapshot, snapshot_file, namespace, definition, &entry.code)?);
  }
  Ok(suggestions)
}

/// Plan one suggestion per `apply-args` call. A proven call nested inside
/// another proven call is folded into the outer replacement.
pub(super) fn plan_apply_args_source(
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
  collect_apply_args_calls(source, &mut Vec::new(), false, &mut calls);
  if calls.is_empty() {
    return Ok(Vec::new());
  }
  let mut local_bindings = HashSet::new();
  collect_potential_local_bindings(source, &mut local_bindings);
  // Inside `calcit.core` the namespace definitions are the core macros themselves.
  let shadowed = |name: &str| {
    local_bindings.contains(name) || (namespace != calcit::calcit::CORE_NS && namespace_binds_name(snapshot, namespace, name))
  };
  let mut macro_heads = MacroHeadCache::new();
  let mut reviews = BTreeMap::new();
  for (path, in_template) in &calls {
    let node = navigate_to_path(source, path)?;
    let review = if *in_template {
      Some("`apply-args` is inside a quasiquote template and resolves at each expansion site; review the macro users.".to_owned())
    } else if let Some(name) = ["apply-args", "loop"].into_iter().find(|name| shadowed(name)) {
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
        name: "apply-args",
        transparent: &[],
      },
      &mut macro_heads,
    ) {
      Some(reason)
    } else {
      literal_call(&node).err()
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
    let replacement = machine_applicable.then(|| rewrite_apply_args_tree(&original, &mut path.clone(), &proven));
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
      rule_id: APPLY_ARGS_LOOP_RULE,
      diagnostic_code: APPLY_ARGS_LOOP_DIAGNOSTIC,
      semantic_layer: "surface",
      source_file: snapshot_file.to_owned(),
      definition: format!("{namespace}/{definition}"),
      path: format!("code{}", format_path(&path)),
      fingerprint: node_fingerprint(&original),
      origin_chain: vec![serde_json::json!({
        "kind": "core-macro-expansion",
        "target": "calcit.core/apply-args",
        "replacement": "calcit.core/loop",
      })],
      original: quoted_json(&original),
      replacement: replacement.as_ref().map(quoted_json),
      applicability: if machine_applicable {
        "machine-applicable"
      } else {
        "requires-review"
      },
      message: review.unwrap_or_else(|| {
        "Use `loop` with one binding per parameter; it evaluates the same arguments in order and runs the same body, so `recur` keeps its meaning."
          .to_owned()
      }),
      target_path: path,
      operation,
    });
  }
  Ok(suggestions)
}

/// Collect `apply-args` call paths outside quoted data and comments, marking
/// calls that sit in a quasiquote template outside any unquote.
fn collect_apply_args_calls(node: &Cirru, path: &mut Vec<usize>, in_template: bool, output: &mut Vec<(Vec<usize>, bool)>) {
  let Cirru::List(items) = node else {
    return;
  };
  let in_template = match items.first().and_then(leaf_value) {
    Some("quote" | "cirru-quote" | ";") => return,
    Some("quasiquote") => true,
    Some("~" | "~@") => false,
    _ => in_template,
  };
  if items.first().and_then(leaf_value) == Some("apply-args") {
    output.push((path.clone(), in_template));
  }
  for (index, child) in items.iter().enumerate() {
    path.push(index);
    collect_apply_args_calls(child, path, in_template, output);
    path.pop();
  }
}

/// The loop bindings and body of a call whose callee is a literal `fn` or `defn`.
type LiteralCall<'a> = (Vec<(&'a Cirru, &'a Cirru)>, &'a [Cirru]);

fn literal_call(node: &Cirru) -> Result<LiteralCall<'_>, String> {
  let Cirru::List(items) = node else {
    return Err("`apply-args` call is not a list.".to_owned());
  };
  let [_, Cirru::List(arguments), callee] = items.as_slice() else {
    return Err("`apply-args` needs an argument list and a callee; review the call.".to_owned());
  };
  let arguments = match arguments.first().and_then(leaf_value) {
    Some("[]") => &arguments[1..],
    _ => &arguments[..],
  };
  let (name, params, body) = match callee {
    Cirru::List(parts) if list_head(callee) == Some("fn") && parts.len() > 2 => (None, &parts[1], &parts[2..]),
    Cirru::List(parts) if list_head(callee) == Some("defn") && parts.len() > 3 => (leaf_value(&parts[1]), &parts[2], &parts[3..]),
    _ => {
      return Err("The callee is not a literal `fn` or `defn`; keep the call, or write the loop by hand.".to_owned());
    }
  };
  let Cirru::List(params) = params else {
    return Err("The callee has no parameter list; review the call.".to_owned());
  };
  if params
    .iter()
    .any(|param| !matches!(leaf_value(param), Some(leaf) if leaf != "&" && leaf != "?"))
  {
    return Err("The callee has rest, optional or destructuring parameters; review the call.".to_owned());
  }
  if params.len() != arguments.len() {
    return Err(format!(
      "The callee takes {} parameter(s) but `apply-args` passes {}; review the call.",
      params.len(),
      arguments.len()
    ));
  }
  if let Some(name) = name
    && body.iter().any(|form| mentions_leaf(form, name))
  {
    return Err(format!(
      "The body refers to `{name}`, which the generated loop function does not bind; replace that call with `recur` by hand."
    ));
  }
  Ok((params.iter().zip(arguments).collect(), body))
}

fn mentions_leaf(node: &Cirru, name: &str) -> bool {
  match node {
    Cirru::Leaf(leaf) => leaf.as_ref() == name,
    Cirru::List(items) => items.iter().any(|item| mentions_leaf(item, name)),
  }
}

/// Rewrite every proven `apply-args` call in `node`, innermost first.
fn rewrite_apply_args_tree(node: &Cirru, path: &mut Vec<usize>, proven: &HashSet<Vec<usize>>) -> Cirru {
  let Cirru::List(items) = node else {
    return node.clone();
  };
  let rewritten = Cirru::List(
    items
      .iter()
      .enumerate()
      .map(|(index, item)| {
        path.push(index);
        let rewritten = rewrite_apply_args_tree(item, path, proven);
        path.pop();
        rewritten
      })
      .collect(),
  );
  if !proven.contains(path) {
    return rewritten;
  }
  let Ok((bindings, body)) = literal_call(&rewritten) else {
    return rewritten;
  };
  let pairs = bindings
    .into_iter()
    .map(|(param, value)| Cirru::List(vec![param.clone(), value.clone()]))
    .collect();
  let mut output = vec![Cirru::leaf("loop"), Cirru::List(pairs)];
  output.extend(body.iter().cloned());
  Cirru::List(output)
}
