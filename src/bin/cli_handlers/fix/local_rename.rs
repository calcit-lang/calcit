//! `rename-local-v1`: rename one local binding together with every use in its scope.
//!
//! Scopes come from the core binding forms (`defn`/`fn` parameters, `let`,
//! `&let`, `loop`). Every rewritten use must also be a local reference in the
//! preprocessed definition, so a same-named global or a macro-produced binding
//! is never renamed by text alone.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use calcit::calcit::{Calcit, LocatedWarning};
use calcit::call_stack::CallStackList;
use calcit::snapshot::{ChangesDict, FileChangeInfo, Snapshot};
use calcit::{program, runner};
use cirru_parser::Cirru;

use super::{
  FixOperation, FixSuggestion, RENAME_LOCAL_DIAGNOSTIC, RENAME_LOCAL_RULE, format_path, navigate_to_path, node_fingerprint, quoted_json,
};

/// Forms whose arguments are data or generated code; a use inside them cannot be proven.
const QUOTING_HEADS: &[&str] = &["quote", "quasiquote", "calcit.core/quote", "calcit.core/quasiquote"];

/// Source forms whose scope/data boundaries this walker understands.
pub(crate) const SOURCE_SCOPE_HEADS: &[&str] = &["defn", "defmacro", "fn", "let", "loop", "&let", "quote", "quasiquote"];

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
  core_heads: Option<&'a BTreeMap<String, String>>,
  local_heads: Vec<Arc<str>>,
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
    let core_head = match self.core_heads {
      Some(heads) if !self.local_heads.iter().any(|local| local.as_ref() == head) => heads.get(head).map(String::as_str),
      Some(_) => None,
      None => Some(head),
    };
    // A binding form's operator is itself a global reference, even though its
    // parameter/binding positions are not. Rename mode only tracks locals.
    if self.core_heads.is_some()
      && core_head.is_some()
      && let Some(operator) = items.first()
    {
      self.walk(operator, &child_path(path, 0), active)?;
    }
    match core_head {
      Some("defn" | "defmacro") => self.walk_params(items, path, 2, 3, active),
      Some("fn") => self.walk_params(items, path, 1, 2, active),
      Some("let" | "loop") => self.walk_let(items, path, active, core_head == Some("loop")),
      Some("&let") => self.walk_single_let(items, path, active),
      Some("quasiquote") if self.core_heads.is_some() => {
        for (index, template) in items.iter().enumerate().skip(1) {
          self.walk_template(template, &child_path(path, index), active)?;
        }
        Ok(())
      }
      Some(name) if QUOTING_HEADS.contains(&name) => {
        if self.core_heads.is_none() {
          self.reject_inside(node, path, active, "quoted source");
        }
        Ok(())
      }
      // Outside the binding's scope a macro call is only searched for the binding
      // (for example `defcomp` wrapping a body); inside it, uses cannot be proven.
      _ if active
        && !head.is_empty()
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

  /// Match Calcit's quasiquote preprocessing: template leaves stay data, while
  /// escapes execute in the enclosing scope, including inside nested templates.
  fn walk_template(&mut self, node: &Cirru, path: &[usize], active: bool) -> Result<(), String> {
    match node {
      Cirru::Leaf(name) => {
        // The reader expands ~name / ~@name, but their only source coordinate
        // is the original leaf, not a manufactured child path.
        let escaped = name.strip_prefix("~@").or_else(|| name.strip_prefix('~'));
        if active && escaped == Some(self.old_name) {
          self.uses.push(path.to_vec());
        }
      }
      Cirru::List(items) if matches!(items.first().and_then(leaf_name), Some("~" | "~@")) => {
        for (index, expression) in items.iter().enumerate() {
          self.walk(expression, &child_path(path, index), active)?;
        }
      }
      Cirru::List(items) => {
        for (index, template) in items.iter().enumerate() {
          self.walk_template(template, &child_path(path, index), active)?;
        }
      }
    }
    Ok(())
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

  fn is_target_binding(&self, name: &str) -> bool {
    name == self.old_name && (self.core_heads.is_none() || !name.contains('/'))
  }

  fn remember_head_binding(&mut self, node: &Cirru) {
    if let Some(heads) = self.core_heads
      && let Cirru::Leaf(name) = node
      && !name.contains('/')
      && heads.contains_key(name.as_ref())
    {
      self.local_heads.push(name.clone());
    }
  }

  /// `defn name (params) body...` / `fn (params) body...`
  fn walk_params(&mut self, items: &[Cirru], path: &[usize], params_at: usize, body_at: usize, active: bool) -> Result<(), String> {
    let head_count = self.local_heads.len();
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
          Some(name) if self.is_target_binding(name) => body_active = false,
          Some(name) if active && name == self.new_name => self.blockers.push(format!(
            "code{}: `{}` is already bound in the binding's scope",
            format_path(&param_path),
            self.new_name
          )),
          _ => {}
        }
        self.remember_head_binding(param);
      }
    }
    for (index, item) in items.iter().enumerate().skip(body_at) {
      self.walk(item, &child_path(path, index), body_active)?;
    }
    self.local_heads.truncate(head_count);
    Ok(())
  }

  /// `let ((a v) (b v)) body...` binds sequentially; `loop` values see only the outer scope.
  fn walk_let(&mut self, items: &[Cirru], path: &[usize], active: bool, parallel: bool) -> Result<(), String> {
    let head_count = self.local_heads.len();
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
          Some(name) if self.is_target_binding(name) => scope_active = false,
          Some(name) if scope_active && name == self.new_name => self.blockers.push(format!(
            "code{}: `{}` is already bound in the binding's scope",
            format_path(&name_path),
            self.new_name
          )),
          _ => {}
        }
        if !parallel && let Some(binding) = pair_items.first() {
          self.remember_head_binding(binding);
        }
      }
      if parallel {
        for pair in pairs {
          if let Cirru::List(pair) = pair
            && let Some(binding) = pair.first()
          {
            self.remember_head_binding(binding);
          }
        }
      }
    }
    for (index, item) in items.iter().enumerate().skip(2) {
      self.walk(item, &child_path(path, index), scope_active)?;
    }
    self.local_heads.truncate(head_count);
    Ok(())
  }

  /// `&let (a v) body...`
  fn walk_single_let(&mut self, items: &[Cirru], path: &[usize], active: bool) -> Result<(), String> {
    let head_count = self.local_heads.len();
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
        Some(name) if self.is_target_binding(name) => body_active = false,
        _ => {}
      }
      if let Some(binding) = pair.first() {
        self.remember_head_binding(binding);
      }
    }
    for (index, item) in items.iter().enumerate().skip(2) {
      self.walk(item, &child_path(path, index), body_active)?;
    }
    self.local_heads.truncate(head_count);
    Ok(())
  }
}

/// Source-reference candidates outside known lexical bindings. Unknown macro
/// arguments are retained conservatively; no user code is preprocessed here.
pub(crate) fn source_reference_paths(code: &Cirru, name: &str, core_heads: &BTreeMap<String, String>) -> Vec<Vec<usize>> {
  let mut plain = |_: &str| Ok(MacroHead::Plain);
  let mut walk = ScopeWalk {
    old_name: name,
    new_name: name,
    target: &[],
    is_macro: &mut plain,
    core_heads: Some(core_heads),
    local_heads: vec![],
    uses: vec![],
    target_found: false,
    blockers: vec![],
  };
  walk.walk(code, &[], true).expect("the plain macro classifier cannot fail");
  walk.uses
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
) -> Result<LocalRenamePlan, String> {
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
    return Ok(LocalRenamePlan {
      suggestions: vec![],
      warnings: vec![],
    });
  }

  let mut walk = ScopeWalk {
    old_name: &old_name,
    new_name,
    target: at,
    is_macro,
    core_heads: None,
    local_heads: vec![],
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

  // Every rewritten use must be a local reference to the new name after
  // preprocessing the renamed definition. Proving the staged form also covers
  // an old name the reader resolves as a builtin, which cannot preprocess as a
  // binding before the rename (#2049).
  let mut staged = entry.code.clone();
  for path in std::iter::once(at).chain(walk.uses.iter().map(Vec::as_slice)) {
    replace_leaf(&mut staged, path, new_name)?;
  }
  let (staged_code, warnings) = preprocess_staged_definition(namespace, definition, &entry.code, &staged)?;
  let mut locals = BTreeSet::new();
  preprocessed_local_paths(&staged_code, new_name, &mut locals);
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
  Ok(LocalRenamePlan {
    warnings,
    suggestions: paths
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
  })
}

/// Replace the leaf at `path` in a definition body.
fn replace_leaf(node: &mut Cirru, path: &[usize], name: &str) -> Result<(), String> {
  let mut current = node;
  for index in path {
    let Cirru::List(items) = current else {
      return Err(format!("code{} is not inside a list.", format_path(path)));
    };
    current = items
      .get_mut(*index)
      .ok_or_else(|| format!("code{} does not exist.", format_path(path)))?;
  }
  *current = Cirru::leaf(name);
  Ok(())
}

/// Preprocess `staged` in place of `original` and restore the program before returning,
/// whether or not the staged definition compiles.
fn preprocess_staged_definition(
  namespace: &str,
  definition: &str,
  original: &Cirru,
  staged: &Cirru,
) -> Result<(Calcit, Vec<LocatedWarning>), String> {
  let swap = |code: &Cirru| {
    let mut changes = ChangesDict::default();
    changes.changed.insert(
      Arc::from(namespace),
      FileChangeInfo {
        ns: None,
        added_defs: HashMap::new(),
        removed_defs: HashSet::new(),
        changed_defs: HashMap::from([(definition.to_owned(), code.clone())]),
      },
    );
    program::apply_code_changes(&changes)
  };
  swap(staged)?;
  let warnings = RefCell::new(vec![]);
  let compiled = super::with_legacy_migration_mode(|| {
    runner::preprocess::ensure_ns_def_compiled(namespace, definition, &warnings, &CallStackList::default())
  })
  .map_err(|failure| format!("Failed to preprocess the renamed `{namespace}/{definition}`: {failure}"))
  .and_then(|_| {
    program::lookup_compiled_def(namespace, definition)
      .map(|compiled| compiled.preprocessed_code)
      .ok_or_else(|| format!("`{namespace}/{definition}` did not preprocess after renaming; fix its errors before renaming locals."))
  });
  swap(original)?;
  Ok((compiled?, warnings.into_inner()))
}

/// Planned edits plus the compiler warnings of the renamed definition, which
/// staged validation must reproduce.
pub(super) struct LocalRenamePlan {
  pub(super) suggestions: Vec<FixSuggestion>,
  pub(super) warnings: Vec<LocatedWarning>,
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
