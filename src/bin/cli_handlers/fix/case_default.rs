//! `case-default` -> `match` migration for calls whose patterns are all literals.
//!
//! `case-default item default (p v)...` expands to `match item (p v)... (_ default)`
//! when every pattern reads as a tag, string, number or bool. The rewrite writes
//! that exact expansion back to source, so evaluation order and the no-match
//! default stay identical. Other calls go through `&case` and are review-only.

use calcit::snapshot::FileInSnapShot;

use super::*;

/// Resolved enclosing macro heads, keyed by spelling within one definition.
type MacroHeadCache = BTreeMap<String, Result<Option<(Cirru, FileInSnapShot)>, String>>;

/// Core macros that evaluate nested argument forms in place and never inspect
/// or reorder them. Threading macros such as `->` insert arguments into the
/// call and must not be listed here.
const ARGUMENT_PRESERVING_CORE_MACROS: &[&str] = &[
  "fn",
  "def",
  "do",
  "and",
  "or",
  "assert",
  "assert=",
  "case",
  "case-default",
  "cond",
  "either",
  "if-let",
  "if-not",
  "let",
  "let[]",
  "loop",
  "when",
  "when-let",
  "when-not",
  "{}",
  "is",
  "is=",
  "is-not=",
];

pub(super) fn plan_case_default_fixes(
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
    suggestions.extend(plan_case_default_source(
      snapshot,
      snapshot_file,
      namespace,
      definition,
      &entry.code,
    )?);
  }
  Ok(suggestions)
}

/// Plan one suggestion per `case-default` call. A proven call nested inside
/// another proven call is folded into the outer replacement.
pub(super) fn plan_case_default_source(
  snapshot: &Snapshot,
  snapshot_file: &str,
  namespace: &str,
  definition: &str,
  source: &Cirru,
) -> Result<Vec<FixSuggestion>, String> {
  let mut calls = Vec::new();
  collect_case_default_calls(source, &mut Vec::new(), false, &mut calls);
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
      Some("`case-default` is inside a quasiquote template and resolves at each expansion site; review the macro users.".to_owned())
    } else if let Some(name) = ["case-default", "match"].into_iter().find(|name| shadowed(name)) {
      Some(format!(
        "`{name}` is bound by a local, a namespace definition or an import in `{namespace}`; review which definition the call resolves to."
      ))
    } else if let Some(reason) = unstable_macro_context(snapshot, snapshot_file, namespace, source, path, &mut macro_heads) {
      Some(reason)
    } else {
      non_literal_pattern(&node, namespace, definition)
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
    let replacement = machine_applicable.then(|| rewrite_case_default_tree(&original, &mut path.clone(), &proven));
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
      rule_id: CASE_DEFAULT_MATCH_RULE,
      diagnostic_code: CASE_DEFAULT_MATCH_DIAGNOSTIC,
      semantic_layer: "surface",
      source_file: snapshot_file.to_owned(),
      definition: format!("{namespace}/{definition}"),
      path: format!("code{}", format_path(&path)),
      fingerprint: node_fingerprint(&original),
      origin_chain: vec![serde_json::json!({
        "kind": "core-macro-expansion",
        "target": "calcit.core/case-default",
        "replacement": "match",
      })],
      original: quoted_json(&original),
      replacement: replacement.as_ref().map(quoted_json),
      applicability: if machine_applicable {
        "machine-applicable"
      } else {
        "requires-review"
      },
      message: review.unwrap_or_else(|| {
        "Use `match` with a trailing `_` branch; every pattern is a literal, so this is the exact expansion of `case-default`."
          .to_owned()
      }),
      target_path: path,
      operation,
    });
  }
  Ok(suggestions)
}

/// Collect `case-default` call paths outside quoted data and comments, marking
/// calls that sit in a quasiquote template outside any unquote.
fn collect_case_default_calls(node: &Cirru, path: &mut Vec<usize>, in_template: bool, output: &mut Vec<(Vec<usize>, bool)>) {
  let Cirru::List(items) = node else {
    return;
  };
  let in_template = match items.first().and_then(leaf_value) {
    Some("quote" | "cirru-quote" | ";") => return,
    Some("quasiquote") => true,
    Some("~" | "~@") => false,
    _ => in_template,
  };
  if items.first().and_then(leaf_value) == Some("case-default") {
    output.push((path.clone(), in_template));
  }
  for (index, child) in items.iter().enumerate() {
    path.push(index);
    collect_case_default_calls(child, path, in_template, output);
    path.pop();
  }
}

/// Mirror the macro's own literal check: every pattern must read as a tag,
/// string, number or bool, otherwise the macro falls back to `&case`.
fn non_literal_pattern(node: &Cirru, namespace: &str, definition: &str) -> Option<String> {
  let Cirru::List(items) = node else {
    return Some("`case-default` call is not a list.".to_owned());
  };
  if items.len() < 4 {
    return Some("`case-default` needs an item, a default and at least one pattern pair.".to_owned());
  }
  for pair in &items[3..] {
    let Cirru::List(pair) = pair else {
      return Some("`case-default` pattern is not a `(pattern value)` pair; review the call.".to_owned());
    };
    if pair.len() != 2 {
      return Some("`case-default` pattern is not a `(pattern value)` pair; review the call.".to_owned());
    }
    let literal = matches!(
      code_to_calcit(&pair[0], namespace, definition, Vec::new()),
      Ok(Calcit::Tag(_) | Calcit::Str(_) | Calcit::Number(_) | Calcit::Bool(_))
    );
    if !literal {
      return Some(format!(
        "Pattern `{}` is not a literal, so `case-default` evaluates it through `&case`; rewrite with `match` or `cond` by hand.",
        leaf_value(&pair[0]).map_or_else(|| pair[0].format_one_liner().unwrap_or_default(), str::to_owned)
      ));
    }
  }
  None
}

/// Every enclosing call must be a function, syntax, a core macro that keeps its
/// argument forms unchanged, or a macro whose source only forwards the argument
/// holding this call, so no macro observes the spelling.
fn unstable_macro_context(
  snapshot: &Snapshot,
  snapshot_file: &str,
  namespace: &str,
  source: &Cirru,
  call_path: &[usize],
  cache: &mut MacroHeadCache,
) -> Option<String> {
  for depth in 0..call_path.len() {
    let Ok(Cirru::List(items)) = navigate_to_path(source, &call_path[..depth]) else {
      return Some("Cannot read an enclosing form; review the call.".to_owned());
    };
    let Some(Cirru::Leaf(head)) = items.first() else {
      continue;
    };
    if ARGUMENT_PRESERVING_CORE_MACROS.contains(&head.as_ref()) && !namespace_binds_name(snapshot, namespace, head) {
      continue;
    }
    let macro_code = cache
      .entry(head.to_string())
      .or_insert_with(|| super::super::edit::definition_head_macro_code(snapshot_file, snapshot, namespace, head));
    match macro_code {
      Ok(None) => {}
      Ok(Some((code, file))) => {
        if !macro_forwards_argument(code, file, call_path[depth] - 1) {
          return Some(format!(
            "`case-default` is an argument of the macro `{head}`, which may observe its spelling; review before rewriting."
          ));
        }
      }
      Err(error) => return Some(format!("Cannot resolve the enclosing head `{head}`: {error}")),
    }
  }
  None
}

/// Whether a `defmacro` only splices the parameter that receives argument
/// `index` into its expansion (`~p`, `~@p`), or reads a rest parameter's
/// length. Each spliced copy must land in evaluated code: every template form
/// around it is syntax, a core function, an argument-preserving core macro or
/// a function of the macro's own namespace. Any other use may inspect the
/// argument and fails the proof.
fn macro_forwards_argument(code: &Cirru, file: &FileInSnapShot, index: usize) -> bool {
  let Cirru::List(items) = code else {
    return false;
  };
  let Some(Cirru::List(params)) = items.get(2) else {
    return false;
  };
  let mut position = 0;
  let mut target = None;
  let mut params = params.iter().filter(|param| leaf_value(param) != Some("?"));
  while let Some(param) = params.next() {
    let Some(name) = leaf_value(param) else {
      return false;
    };
    if name == "&" {
      let Some(rest) = params.next().and_then(leaf_value) else {
        return false;
      };
      target = Some((rest, true));
      break;
    }
    if position == index {
      target = Some((name, false));
      break;
    }
    position += 1;
  }
  let Some((name, rest)) = target else {
    return false;
  };
  let imports = match &file.ns.code {
    Cirru::List(ns) => match ns.get(1).and_then(leaf_value) {
      Some(ns_name) => program::extract_import_map(&file.ns.code, ns_name).ok(),
      None => None,
    },
    Cirru::Leaf(_) => None,
  };
  let Some(imports) = imports else {
    return false;
  };
  let parameter = MacroParameter {
    name,
    rest,
    file,
    imports: imports.keys().map(|name| name.to_string()).collect(),
  };
  items[3..].iter().all(|form| parameter.only_forwarded(form, None))
}

struct MacroParameter<'a> {
  name: &'a str,
  rest: bool,
  file: &'a FileInSnapShot,
  imports: HashSet<String>,
}

impl MacroParameter<'_> {
  /// `template` holds the template heads around this node once inside `quasiquote`.
  fn only_forwarded(&self, node: &Cirru, template: Option<&[&str]>) -> bool {
    let spliced = || template.is_some_and(|heads| heads.iter().all(|head| self.evaluates_arguments(head)));
    let Cirru::List(items) = node else {
      // The reader also accepts the compact `~p` / `~@p` leaf spelling.
      return match leaf_value(node).map(|leaf| leaf.trim_start_matches("~@").trim_start_matches('~')) {
        Some(name) if name == self.name => leaf_value(node) != Some(self.name) && spliced(),
        _ => true,
      };
    };
    let head = items.first().and_then(leaf_value);
    if items.len() == 2 && leaf_value(&items[1]) == Some(self.name) {
      match head {
        Some("~" | "~@") => return spliced(),
        Some("count" | "&list:count" | "empty?" | "&list:empty?") if self.rest => return true,
        _ => {}
      }
    }
    match (head, template) {
      (Some("quasiquote"), None) => items[1..].iter().all(|item| self.only_forwarded(item, Some(&[]))),
      (Some("~" | "~@"), Some(_)) => items[1..].iter().all(|item| self.only_forwarded(item, None)),
      (_, Some(heads)) => {
        let mut heads = heads.to_vec();
        heads.push(head.unwrap_or(""));
        items.iter().all(|item| self.only_forwarded(item, Some(&heads)))
      }
      (_, None) => items.iter().all(|item| self.only_forwarded(item, None)),
    }
  }

  /// Whether a template call head cannot receive its argument forms as data.
  /// Heads that are neither namespace definitions, imports nor core macros are
  /// syntax, core functions or locals, and none of those is a macro.
  fn evaluates_arguments(&self, head: &str) -> bool {
    if head.is_empty() {
      return true;
    }
    if matches!(head, "quote" | "cirru-quote" | "quasiquote" | "&raw-code") {
      return false;
    }
    if let Some(entry) = self.file.defs.get(head) {
      return !matches!(&entry.code, Cirru::List(items) if items.first().and_then(leaf_value) == Some("defmacro"));
    }
    if self.imports.contains(head) || head.contains('/') {
      return false;
    }
    !core_macro_names().contains(head) || ARGUMENT_PRESERVING_CORE_MACROS.contains(&head)
  }
}

fn core_macro_names() -> &'static HashSet<String> {
  static NAMES: std::sync::OnceLock<HashSet<String>> = std::sync::OnceLock::new();
  NAMES.get_or_init(|| {
    calcit::load_core_snapshot()
      .map(|core| {
        core.files[calcit::calcit::CORE_NS]
          .defs
          .iter()
          .filter(|(_, entry)| matches!(&entry.code, Cirru::List(items) if items.first().and_then(leaf_value) == Some("defmacro")))
          .map(|(name, _)| name.to_owned())
          .collect()
      })
      .unwrap_or_default()
  })
}

/// Rewrite every proven `case-default` call in `node`, innermost first.
fn rewrite_case_default_tree(node: &Cirru, path: &mut Vec<usize>, proven: &HashSet<Vec<usize>>) -> Cirru {
  let Cirru::List(items) = node else {
    return node.clone();
  };
  let rewritten = items
    .iter()
    .enumerate()
    .map(|(index, item)| {
      path.push(index);
      let rewritten = rewrite_case_default_tree(item, path, proven);
      path.pop();
      rewritten
    })
    .collect::<Vec<_>>();
  if !proven.contains(path) {
    return Cirru::List(rewritten);
  }
  let mut output = Vec::with_capacity(rewritten.len());
  output.push(Cirru::leaf("match"));
  output.push(rewritten[1].clone());
  output.extend(rewritten[3..].iter().cloned());
  output.push(Cirru::List(vec![Cirru::leaf("_"), rewritten[2].clone()]));
  Cirru::List(output)
}
