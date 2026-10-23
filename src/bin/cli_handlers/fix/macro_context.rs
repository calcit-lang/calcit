//! Shared proof that no enclosing macro observes the spelling of a source call.
//!
//! Fix rules that move or rename a call use `unstable_macro_context` to confirm
//! every enclosing head is a function, syntax, a core macro that forwards its
//! arguments unchanged, or a project macro that only splices the argument.

use calcit::snapshot::FileInSnapShot;

use super::*;

/// Resolved enclosing macro heads, keyed by spelling within one definition.
pub(super) type MacroHeadCache = BTreeMap<String, Result<Option<(Cirru, FileInSnapShot)>, String>>;

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
  "cond",
  "either",
  "if-let",
  "if-not",
  "let",
  "let[]",
  "let-destruct",
  "loop",
  "when",
  "when-let",
  "when-not",
  "{}",
  "is",
  "is=",
  "is-not=",
];

/// The call being moved, and the extra macros that pass its form through
/// unchanged on top of `ARGUMENT_PRESERVING_CORE_MACROS`.
#[derive(Clone, Copy)]
pub(super) struct ContextSubject<'a> {
  pub(super) name: &'a str,
  pub(super) transparent: &'a [&'a str],
}

/// Every enclosing call must be a function, syntax, a core macro that keeps its
/// argument forms unchanged, or a macro whose source only forwards the argument
/// holding this call, so no macro observes the spelling.
pub(super) fn unstable_macro_context(
  snapshot: &Snapshot,
  snapshot_file: &str,
  namespace: &str,
  source: &Cirru,
  call_path: &[usize],
  subject: ContextSubject<'_>,
  cache: &mut MacroHeadCache,
) -> Option<String> {
  for depth in 0..call_path.len() {
    let Ok(Cirru::List(items)) = navigate_to_path(source, &call_path[..depth]) else {
      return Some("Cannot read an enclosing form; review the call.".to_owned());
    };
    let Some(Cirru::Leaf(head)) = items.first() else {
      continue;
    };
    if (ARGUMENT_PRESERVING_CORE_MACROS.contains(&head.as_ref()) || subject.transparent.contains(&head.as_ref()))
      && !namespace_binds_name(snapshot, namespace, head)
    {
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
            "`{}` is an argument of the macro `{head}`, which may observe its spelling; review before rewriting.",
            subject.name
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
