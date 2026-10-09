//! `core-macro-alias-v1`: migrate retired core convenience macros to the forms
//! they expand to.
//!
//! Each rewrite writes the macro's own expansion back to source, so evaluation
//! order and results stay identical:
//! - `w-log x` -> `dbg x` (`w-log` forwards to `dbg`);
//! - `wo-log x` / `wo-js-log x` -> `x`;
//! - `flipped f a b` -> `f b a`.
//!
//! `w-js-log` logs through `js/console.log` while `dbg` prints the formatted
//! value, so it is review-only. `noted` stays: `calcit query anchors` reads its
//! `@anchor:` notes. Calls that a local, namespace definition or import may shadow,
//! calls inside quasiquote templates and calls under macros that may observe
//! their spelling are review-only as well.

use super::case_default::{ContextSubject, MacroHeadCache, unstable_macro_context};
use super::*;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq)]
enum MacroRewrite {
  /// Replace the call head with another macro of identical expansion.
  Rename(&'static str),
  /// Replace the call with its only argument.
  Unwrap,
  /// Reverse the arguments after the callee.
  Flip,
}

struct RetiredMacro {
  name: &'static str,
  rewrite: MacroRewrite,
  /// Set when the rewrite is not an exact expansion and needs a person.
  review: Option<&'static str>,
}

const RETIRED_MACROS: &[RetiredMacro] = &[
  RetiredMacro {
    name: "w-log",
    rewrite: MacroRewrite::Rename("dbg"),
    review: None,
  },
  RetiredMacro {
    name: "w-js-log",
    rewrite: MacroRewrite::Rename("dbg"),
    review: Some(
      "`w-js-log` passes the value to `js/console.log` while `dbg` prints its formatted text; use `dbg` when the printed form is enough, otherwise keep an explicit `js/console.log`.",
    ),
  },
  RetiredMacro {
    name: "wo-log",
    rewrite: MacroRewrite::Unwrap,
    review: None,
  },
  RetiredMacro {
    name: "wo-js-log",
    rewrite: MacroRewrite::Unwrap,
    review: None,
  },
  RetiredMacro {
    name: "flipped",
    rewrite: MacroRewrite::Flip,
    review: None,
  },
];

/// Retired macros that pass their argument forms through unchanged. A call
/// nested in one of them keeps its meaning when both are rewritten. `w-log`,
/// `w-js-log` and `dbg` print the quoted source of their argument, so a call
/// nested in them changes the printed text and stays review-only.
const FORWARDING_MACROS: &[&str] = &["wo-log", "wo-js-log", "flipped", "noted"];

fn retired_macro(name: &str) -> Option<&'static RetiredMacro> {
  RETIRED_MACROS.iter().find(|item| item.name == name)
}

pub(super) fn plan_core_macro_alias_fixes(
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
    suggestions.extend(plan_core_macro_alias_source(
      snapshot,
      snapshot_file,
      namespace,
      definition,
      &entry.code,
    )?);
  }
  Ok(suggestions)
}

/// Plan one suggestion per retired macro call. A proven call nested inside
/// another proven call is folded into the outer replacement.
pub(super) fn plan_core_macro_alias_source(
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
  collect_retired_macro_calls(source, &mut Vec::new(), false, &mut calls);
  if calls.is_empty() {
    return Ok(Vec::new());
  }
  let mut local_bindings = HashSet::new();
  collect_potential_local_bindings(source, &mut local_bindings);
  let shadowed = |name: &str| local_bindings.contains(name) || namespace_binds_name(snapshot, namespace, name);
  let mut macro_heads = MacroHeadCache::new();
  let mut planned = BTreeMap::new();
  for (path, in_template) in calls {
    let node = navigate_to_path(source, &path)?;
    let Some(macro_info) = list_head(&node).and_then(retired_macro) else {
      continue;
    };
    let argument_count = match &node {
      Cirru::List(items) => items.len() - 1,
      Cirru::Leaf(_) => 0,
    };
    let names = match macro_info.rewrite {
      MacroRewrite::Rename(target) => vec![macro_info.name, target],
      _ => vec![macro_info.name],
    };
    let review = if in_template {
      Some(format!(
        "`{}` is inside a quasiquote template and resolves at each expansion site; review the macro users.",
        macro_info.name
      ))
    } else if let Some(name) = names.into_iter().find(|name| shadowed(name)) {
      Some(format!(
        "`{name}` is bound by a local, a namespace definition or an import in `{namespace}`; review which definition the call resolves to."
      ))
    } else if let Some(reason) = arity_review(macro_info, argument_count) {
      Some(reason)
    } else if let Some(reason) = unstable_macro_context(
      snapshot,
      snapshot_file,
      namespace,
      source,
      &path,
      ContextSubject {
        name: macro_info.name,
        transparent: FORWARDING_MACROS,
      },
      &mut macro_heads,
    ) {
      Some(reason)
    } else {
      macro_info.review.map(str::to_owned)
    };
    planned.insert(path, (macro_info, review));
  }

  let proven = planned
    .iter()
    .filter(|(_, (_, review))| review.is_none())
    .map(|(path, (macro_info, _))| (path.clone(), macro_info.rewrite))
    .collect::<HashMap<_, _>>();
  let mut suggestions = Vec::new();
  for (path, (macro_info, review)) in planned {
    let machine_applicable = review.is_none();
    if machine_applicable && proven.keys().any(|parent| parent.len() < path.len() && path.starts_with(parent)) {
      continue;
    }
    let original = navigate_to_path(source, &path)?;
    let replacement = machine_applicable.then(|| rewrite_macro_tree(&original, &mut path.clone(), &proven));
    let operation = replacement
      .as_ref()
      .map(|replacement| {
        Ok::<_, String>(FixOperation::ReplaceNodeQuoted {
          original: original.format_one_liner().map_err(|error| error.to_string())?,
          code: Cirru::List(vec![Cirru::leaf("quote"), replacement.clone()])
            .format_one_liner()
            .map_err(|error| error.to_string())?,
        })
      })
      .transpose()?;
    let target = match macro_info.rewrite {
      MacroRewrite::Rename(target) => format!("calcit.core/{target}"),
      MacroRewrite::Unwrap => "argument".to_owned(),
      MacroRewrite::Flip => "reversed-call".to_owned(),
    };
    suggestions.push(FixSuggestion {
      rule_id: CORE_MACRO_ALIAS_RULE,
      diagnostic_code: CORE_MACRO_ALIAS_DIAGNOSTIC,
      semantic_layer: "surface",
      source_file: snapshot_file.to_owned(),
      definition: format!("{namespace}/{definition}"),
      path: format!("code{}", format_path(&path)),
      fingerprint: node_fingerprint(&original),
      origin_chain: vec![serde_json::json!({
        "kind": "core-macro-expansion",
        "target": format!("calcit.core/{}", macro_info.name),
        "replacement": target,
      })],
      original: quoted_json(&original),
      replacement: replacement.as_ref().map(quoted_json),
      applicability: if machine_applicable {
        "machine-applicable"
      } else {
        "requires-review"
      },
      message: review.unwrap_or_else(|| success_message(macro_info)),
      target_path: path,
      operation,
    });
  }
  Ok(suggestions)
}

fn arity_review(macro_info: &RetiredMacro, argument_count: usize) -> Option<String> {
  let fits = match macro_info.rewrite {
    MacroRewrite::Rename(_) => argument_count == 1,
    MacroRewrite::Unwrap => argument_count == 1,
    MacroRewrite::Flip => argument_count >= 1,
  };
  (!fits).then(|| {
    format!(
      "`{}` is called with {argument_count} argument(s), which does not match its definition; fix the call by hand.",
      macro_info.name
    )
  })
}

fn success_message(macro_info: &RetiredMacro) -> String {
  match macro_info.rewrite {
    MacroRewrite::Rename(target) => {
      format!(
        "Use `{target}`; `{}` forwards to it, so the logged text and the returned value are unchanged.",
        macro_info.name
      )
    }
    MacroRewrite::Unwrap => format!(
      "Remove `{}`; it expands to its argument, so evaluation and the value are unchanged.",
      macro_info.name
    ),
    MacroRewrite::Flip => {
      "Write the call with its arguments reversed; this is the exact expansion of `flipped`, so evaluation order is unchanged."
        .to_owned()
    }
  }
}

/// Collect retired macro call paths outside quoted data and comments, marking
/// calls that sit in a quasiquote template outside any unquote.
fn collect_retired_macro_calls(node: &Cirru, path: &mut Vec<usize>, in_template: bool, output: &mut Vec<(Vec<usize>, bool)>) {
  let Cirru::List(items) = node else {
    return;
  };
  let in_template = match items.first().and_then(leaf_value) {
    Some("quote" | "cirru-quote" | ";") => return,
    Some("quasiquote") => true,
    Some("~" | "~@") => false,
    _ => in_template,
  };
  if items.first().and_then(leaf_value).and_then(retired_macro).is_some() {
    output.push((path.clone(), in_template));
  }
  for (index, child) in items.iter().enumerate() {
    path.push(index);
    collect_retired_macro_calls(child, path, in_template, output);
    path.pop();
  }
}

/// Rewrite every proven retired macro call in `node`, innermost first.
fn rewrite_macro_tree(node: &Cirru, path: &mut Vec<usize>, proven: &HashMap<Vec<usize>, MacroRewrite>) -> Cirru {
  let Cirru::List(items) = node else {
    return node.clone();
  };
  let mut rewritten = items
    .iter()
    .enumerate()
    .map(|(index, item)| {
      path.push(index);
      let rewritten = rewrite_macro_tree(item, path, proven);
      path.pop();
      rewritten
    })
    .collect::<Vec<_>>();
  match proven.get(path.as_slice()) {
    None => Cirru::List(rewritten),
    Some(MacroRewrite::Rename(target)) => {
      rewritten[0] = Cirru::leaf(String::from(*target));
      Cirru::List(rewritten)
    }
    Some(MacroRewrite::Unwrap) => rewritten.pop().unwrap_or_else(|| Cirru::List(vec![])),
    Some(MacroRewrite::Flip) => {
      let mut output = Vec::with_capacity(rewritten.len() - 1);
      output.push(rewritten[1].clone());
      output.extend(rewritten[2..].iter().rev().cloned());
      Cirru::List(output)
    }
  }
}
