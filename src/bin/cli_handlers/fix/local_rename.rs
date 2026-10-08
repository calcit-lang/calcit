//! `rename-local-v1`: rename one local binding together with every use in its scope.
//!
//! Scopes come from the core binding forms (`defn`/`fn` parameters, `let`,
//! `&let`, `loop`). Every rewritten use must also be a local reference in the
//! preprocessed definition, so a same-named global or a macro-produced binding
//! is never renamed by text alone.

use std::collections::{BTreeSet, HashMap};

use calcit::calcit::Calcit;
use calcit::program;
use calcit::snapshot::Snapshot;
use cirru_parser::Cirru;

use super::{
  FixOperation, FixSuggestion, RENAME_LOCAL_DIAGNOSTIC, RENAME_LOCAL_RULE, format_path, navigate_to_path, node_fingerprint, quoted_json,
};

/// Forms whose arguments are data or generated code; a use inside them cannot be proven.
const QUOTING_HEADS: &[&str] = &["quote", "quasiquote", "calcit.core/quote", "calcit.core/quasiquote"];

/// Core macros that rearrange or wrap their arguments without introducing bindings,
/// so a local inside them still refers to the enclosing scope.
const NON_BINDING_CORE_MACROS: &[&str] = &[
  "->",
  "->>",
  "and",
  "or",
  "assert",
  "assert=",
  "assert-detect",
  "case",
  "case-default",
  "&case",
  "cond",
  "do",
  "either",
  "flipped",
  "if-not",
  "when",
  "when-not",
  "is",
  "is=",
  "is-not=",
  "is-throws",
  "throws?",
  "noted",
  "swap!",
  "with-cpu-time",
  "w-log",
  "w-js-log",
  "wo-log",
  "wo-js-log",
  "{}",
  "{,}",
  "%{}",
  "js-object",
];

/// How a list head resolves for scope analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MacroHead {
  /// a function, syntax, or local value
  Plain,
  /// a macro from `calcit.core`
  Core,
  /// a project or dependency macro
  Other,
}

struct ScopeWalk<'a> {
  old_name: &'a str,
  new_name: &'a str,
  target: &'a [usize],
  is_macro: &'a mut dyn FnMut(&str) -> Result<MacroHead, String>,
  uses: Vec<Vec<usize>>,
  target_found: bool,
  blockers: Vec<String>,
}

fn leaf_name(node: &Cirru) -> Option<&str> {
  match node {
    Cirru::Leaf(leaf) => Some(leaf.as_ref()),
    Cirru::List(_) => None,
  }
}

fn child_path(path: &[usize], index: usize) -> Vec<usize> {
  let mut next = path.to_vec();
  next.push(index);
  next
}

/// Paths of every leaf equal to `name` under `node`.
fn leaf_paths(node: &Cirru, name: &str, path: &[usize], found: &mut Vec<Vec<usize>>) {
  match node {
    Cirru::Leaf(leaf) if leaf.as_ref() == name => found.push(path.to_vec()),
    Cirru::Leaf(_) => {}
    Cirru::List(items) => {
      for (index, item) in items.iter().enumerate() {
        leaf_paths(item, name, &child_path(path, index), found);
      }
    }
  }
}

impl ScopeWalk<'_> {
  /// Walk `node`; `active` means `old_name` refers to the target binding here.
  fn walk(&mut self, node: &Cirru, path: &[usize], active: bool) -> Result<(), String> {
    let Cirru::List(items) = node else {
      if active {
        match leaf_name(node) {
          Some(name) if name == self.old_name => self.uses.push(path.to_vec()),
          Some(name) if name == self.new_name => self.blockers.push(format!(
            "code{}: `{}` is already used in the binding's scope",
            format_path(path),
            self.new_name
          )),
          _ => {}
        }
      }
      return Ok(());
    };
    let head = items.first().and_then(leaf_name).unwrap_or_default();
    match head {
      "defn" | "defmacro" => self.walk_params(items, path, 2, 3, active),
      "fn" => self.walk_params(items, path, 1, 2, active),
      "let" | "loop" => self.walk_let(items, path, active, head == "loop"),
      "&let" => self.walk_single_let(items, path, active),
      _ if QUOTING_HEADS.contains(&head) => {
        self.reject_inside(node, path, active, "quoted source");
        Ok(())
      }
      _ if !head.is_empty()
        && match (self.is_macro)(head)? {
          MacroHead::Plain => false,
          MacroHead::Core => !NON_BINDING_CORE_MACROS.contains(&head),
          MacroHead::Other => true,
        } =>
      {
        self.reject_inside(node, path, active, &format!("arguments of macro `{head}`"));
        Ok(())
      }
      _ => {
        for (index, item) in items.iter().enumerate() {
          self.walk(item, &child_path(path, index), active)?;
        }
        Ok(())
      }
    }
  }

  fn reject_inside(&mut self, node: &Cirru, path: &[usize], active: bool, place: &str) {
    if !active {
      return;
    }
    let mut found = vec![];
    leaf_paths(node, self.old_name, path, &mut found);
    leaf_paths(node, self.new_name, path, &mut found);
    for occurrence in found {
      self.blockers.push(format!(
        "code{}: an occurrence inside {place} cannot be proven to refer to the binding",
        format_path(&occurrence)
      ));
    }
  }

  /// `defn name (params) body...` / `fn (params) body...`
  fn walk_params(&mut self, items: &[Cirru], path: &[usize], params_at: usize, body_at: usize, active: bool) -> Result<(), String> {
    let mut body_active = active;
    if let Some(Cirru::List(params)) = items.get(params_at) {
      let params_path = child_path(path, params_at);
      for (index, param) in params.iter().enumerate() {
        let param_path = child_path(&params_path, index);
        match leaf_name(param) {
          _ if param_path == self.target => {
            self.target_found = true;
            body_active = true;
          }
          Some(name) if name == self.old_name => body_active = false,
          Some(name) if active && name == self.new_name => self.blockers.push(format!(
            "code{}: `{}` is already bound in the binding's scope",
            format_path(&param_path),
            self.new_name
          )),
          _ => {}
        }
      }
    }
    for (index, item) in items.iter().enumerate().skip(body_at) {
      self.walk(item, &child_path(path, index), body_active)?;
    }
    Ok(())
  }

  /// `let ((a v) (b v)) body...` binds sequentially; `loop` values see only the outer scope.
  fn walk_let(&mut self, items: &[Cirru], path: &[usize], active: bool, parallel: bool) -> Result<(), String> {
    let mut scope_active = active;
    if let Some(Cirru::List(pairs)) = items.get(1) {
      let pairs_path = child_path(path, 1);
      for (index, pair) in pairs.iter().enumerate() {
        let pair_path = child_path(&pairs_path, index);
        let Cirru::List(pair_items) = pair else {
          self.walk(pair, &pair_path, active)?;
          continue;
        };
        if let Some(value) = pair_items.get(1) {
          self.walk(value, &child_path(&pair_path, 1), if parallel { active } else { scope_active })?;
        }
        let name_path = child_path(&pair_path, 0);
        match pair_items.first().and_then(leaf_name) {
          _ if name_path == self.target => {
            self.target_found = true;
            scope_active = true;
          }
          Some(name) if name == self.old_name => scope_active = false,
          Some(name) if scope_active && name == self.new_name => self.blockers.push(format!(
            "code{}: `{}` is already bound in the binding's scope",
            format_path(&name_path),
            self.new_name
          )),
          _ => {}
        }
      }
    }
    for (index, item) in items.iter().enumerate().skip(2) {
      self.walk(item, &child_path(path, index), scope_active)?;
    }
    Ok(())
  }

  /// `&let (a v) body...`
  fn walk_single_let(&mut self, items: &[Cirru], path: &[usize], active: bool) -> Result<(), String> {
    let mut body_active = active;
    if let Some(Cirru::List(pair)) = items.get(1) {
      let pair_path = child_path(path, 1);
      if let Some(value) = pair.get(1) {
        self.walk(value, &child_path(&pair_path, 1), active)?;
      }
      let name_path = child_path(&pair_path, 0);
      match pair.first().and_then(leaf_name) {
        _ if name_path == self.target => {
          self.target_found = true;
          body_active = true;
        }
        Some(name) if name == self.old_name => body_active = false,
        _ => {}
      }
    }
    for (index, item) in items.iter().enumerate().skip(2) {
      self.walk(item, &child_path(path, index), body_active)?;
    }
    Ok(())
  }
}

/// Source paths of local references named `name` after preprocessing.
fn preprocessed_local_paths(code: &Calcit, name: &str, found: &mut BTreeSet<Vec<usize>>) {
  match code {
    Calcit::Local(local) if local.sym.as_ref() == name => {
      if let Some(location) = &local.location {
        found.insert(location.iter().map(|index| usize::from(*index)).collect());
      }
    }
    Calcit::List(items) => {
      for item in items.iter() {
        preprocessed_local_paths(item, name, found);
      }
    }
    _ => {}
  }
}

fn valid_local_name(name: &str) -> bool {
  !name.is_empty()
    && !name.contains('/')
    && !name.contains(char::is_whitespace)
    && !matches!(
      name.chars().next(),
      Some(':' | '|' | '"' | '\'' | '.' | '@' | '~' | ',' | '&' | '?')
    )
    && !matches!(name, "nil" | "true" | "false")
}

pub(super) fn plan_local_rename(
  namespace: &str,
  definition: &str,
  at: &[usize],
  new_name: &str,
  snapshot: &Snapshot,
  snapshot_file: &str,
  is_macro: &mut dyn FnMut(&str) -> Result<MacroHead, String>,
) -> Result<Vec<FixSuggestion>, String> {
  if !valid_local_name(new_name) {
    return Err(format!("`--to {new_name}` is not a plain local name."));
  }
  let entry = snapshot
    .files
    .get(namespace)
    .and_then(|file| file.defs.get(definition))
    .ok_or_else(|| format!("Local rename target `{namespace}/{definition}` does not exist."))?;
  let binding = navigate_to_path(&entry.code, at)?;
  let Cirru::Leaf(old_name) = &binding else {
    return Err(format!("code{} is not a binding name leaf.", format_path(at)));
  };
  let old_name = old_name.to_string();
  // Already renamed: a second preview has nothing to do.
  if old_name == new_name {
    return Ok(vec![]);
  }

  let mut walk = ScopeWalk {
    old_name: &old_name,
    new_name,
    target: at,
    is_macro,
    uses: vec![],
    target_found: false,
    blockers: vec![],
  };
  walk.walk(&entry.code, &[], false)?;
  if !walk.target_found {
    return Err(format!(
      "code{} is not a `defn`/`fn` parameter or a `let`/`&let`/`loop` binding name.",
      format_path(at)
    ));
  }

  // Every rewritten use must be a local reference after preprocessing.
  let compiled = program::lookup_compiled_def(namespace, definition)
    .ok_or_else(|| format!("`{namespace}/{definition}` did not preprocess; fix its errors before renaming locals."))?;
  let mut locals = BTreeSet::new();
  preprocessed_local_paths(&compiled.preprocessed_code, &old_name, &mut locals);
  for use_path in &walk.uses {
    if !locals.contains(use_path) {
      walk.blockers.push(format!(
        "code{}: preprocessing does not resolve this `{old_name}` as a local reference",
        format_path(use_path)
      ));
    }
  }
  if !walk.blockers.is_empty() {
    walk.blockers.sort();
    walk.blockers.dedup();
    return Err(format!(
      "Local rename of `{old_name}` at code{} in `{namespace}/{definition}` is not provably safe; no changes were written:\n- {}",
      format_path(at),
      walk.blockers.join("\n- ")
    ));
  }

  let mut paths = vec![at.to_vec()];
  paths.extend(walk.uses);
  Ok(
    paths
      .into_iter()
      .map(|path| {
        let original = Cirru::leaf(old_name.as_str());
        let replacement = Cirru::leaf(new_name);
        FixSuggestion {
          rule_id: RENAME_LOCAL_RULE,
          diagnostic_code: RENAME_LOCAL_DIAGNOSTIC,
          semantic_layer: "surface",
          source_file: snapshot_file.to_owned(),
          definition: format!("{namespace}/{definition}"),
          path: format!("code{}", format_path(&path)),
          fingerprint: node_fingerprint(&original),
          origin_chain: vec![serde_json::json!({"kind": "local-binding", "binding": format!("code{}", format_path(at))})],
          original: quoted_json(&original),
          replacement: Some(quoted_json(&replacement)),
          applicability: "machine-applicable",
          message: if path == at {
            format!("Rename the local binding `{old_name}` to `{new_name}`.")
          } else {
            format!("Rewrite this use of local `{old_name}` to `{new_name}`.")
          },
          target_path: path,
          operation: Some(FixOperation::ReplaceLeaf {
            original: old_name.clone(),
            replacement: new_name.to_owned(),
          }),
        }
      })
      .collect(),
  )
}

/// Cache macro lookups for one plan; module loading behind them is not free.
pub(super) fn macro_head_cache<'a>(
  check: impl Fn(&str) -> Result<MacroHead, String> + 'a,
) -> impl FnMut(&str) -> Result<MacroHead, String> + 'a {
  let mut cache = HashMap::<String, MacroHead>::new();
  move |head: &str| {
    if let Some(known) = cache.get(head) {
      return Ok(*known);
    }
    let result = check(head)?;
    cache.insert(head.to_owned(), result);
    Ok(result)
  }
}
