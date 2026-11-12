//! `E_UNKNOWN_TYPE_NAME`: a nominal name in a type position that names no
//! definition. A misspelled `Nubmer` used to stay a symbolic reference, so a
//! schema such as `Fn (List Nubmer) (-> Number)` passed and callers later got
//! mismatches against a type nobody declared.

use std::sync::Arc;

use crate::calcit::{self, CalcitErr, CalcitErrKind, CalcitTypeAnnotation, NodeLocation};
use crate::call_stack::CallStackList;
use crate::program;

pub(crate) const UNKNOWN_TYPE_NAME_ERROR: &str = "E_UNKNOWN_TYPE_NAME";

/// Built-in type names that are not definitions, offered as candidates.
const BUILTIN_TYPE_NAMES: &[&str] = &[
  "Dynamic",
  "Nil",
  "Unit",
  "Bool",
  "Number",
  "String",
  "Symbol",
  "Tag",
  "List",
  "Map",
  "Set",
  "Fn",
  "Macro",
  "Enum",
  "Tuple",
  "Ref",
  "Buffer",
  "CirruQuote",
  "JsObject",
  "JsNullish",
  "Variadic",
  "Struct",
  "StructDef",
  "EnumDef",
  "UnionDef",
  "Trait",
  "Impl",
  "AnyFn",
  "AnyStruct",
  "AnyEnum",
];

/// A type reference that resolves to no definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnknownTypeName {
  pub path: String,
  pub name: Arc<str>,
  pub candidates: Vec<Arc<str>>,
}

/// Whether `name`, written in `file_ns`, reaches a definition: a local one,
/// a `:refer` import, an `alias/Def` or full path, or `calcit.core`.
fn type_name_resolves(file_ns: &str, name: &str) -> bool {
  let stripped = name.trim_start_matches('\'').trim_start_matches(':');
  if stripped.is_empty()
    || CalcitTypeAnnotation::is_builtin_type_symbol_name(stripped)
    || CalcitTypeAnnotation::open_family_type(stripped).is_some()
  {
    return true;
  }
  match program::resolve_type_name_in_ns(file_ns, stripped) {
    // An import whose namespace this entry does not load (a module listed
    // only for another entry) cannot be judged here, so it is left alone.
    Some((ns, def)) => program::has_def_code(&ns, &def) || (ns.as_ref() != file_ns && imports_unloaded_ns(file_ns, stripped, &ns)),
    None => false,
  }
}

/// Whether `name` points into `target_ns`, a namespace this entry does not
/// load, through a full path or an `:as` / `:refer` import of `file_ns`.
fn imports_unloaded_ns(file_ns: &str, name: &str, target_ns: &str) -> bool {
  if program::has_ns_code(target_ns) {
    return false;
  }
  match name.rsplit_once('/') {
    // A dotted prefix is a full namespace path, as an entry that leaves out
    // a module still writes `respo.dom/DomElement`; a bare prefix must be
    // an `:as` alias, so a misspelled alias is still reported.
    Some((prefix, _)) => prefix.contains('.') || program::lookup_ns_target_in_import(file_ns, prefix).is_some(),
    None => program::lookup_def_target_in_import(file_ns, name).is_some(),
  }
}

/// Find the first unresolved nominal reference in `annotation`.
pub(super) fn find_unknown_type_name(annotation: &Arc<CalcitTypeAnnotation>, file_ns: &str, path: &str) -> Option<UnknownTypeName> {
  super::find_schema_annotation(annotation, path, &mut |candidate, candidate_path| match candidate.as_ref() {
    CalcitTypeAnnotation::TypeRef(name, _) if !type_name_resolves(file_ns, name) => Some(UnknownTypeName {
      path: candidate_path.to_owned(),
      name: Arc::from(name.trim_start_matches('\'')),
      candidates: suggest_type_names(file_ns, name),
    }),
    _ => None,
  })
}

/// Names close to `name` by edit distance, nearest first, at most three.
fn suggest_type_names(file_ns: &str, name: &str) -> Vec<Arc<str>> {
  let stripped = name.trim_start_matches('\'');
  let (prefix, bare) = match stripped.rsplit_once('/') {
    Some((prefix, bare)) => (Some(prefix), bare),
    None => (None, stripped),
  };
  let mut pool: Vec<Arc<str>> = vec![];
  match prefix {
    Some(prefix) => {
      let target_ns = program::lookup_ns_target_in_import(file_ns, prefix).unwrap_or_else(|| Arc::from(prefix));
      pool.extend(program::list_source_def_names(&target_ns));
    }
    None => {
      pool.extend(BUILTIN_TYPE_NAMES.iter().map(|item| Arc::from(*item)));
      pool.extend(program::list_source_def_names(file_ns));
      pool.extend(program::list_refer_import_names(file_ns));
      pool.extend(program::list_source_def_names(calcit::CORE_NS));
    }
  }
  let limit = match bare.chars().count() {
    0..=3 => 1,
    4..=7 => 2,
    _ => 3,
  };
  let mut scored: Vec<(usize, Arc<str>)> = pool
    .into_iter()
    .filter(|item| item.as_ref() != bare && item.chars().next().is_some_and(char::is_uppercase))
    .filter_map(|item| {
      let distance = edit_distance(bare, &item);
      (distance <= limit).then_some((distance, item))
    })
    .collect();
  scored.sort();
  scored.dedup_by(|a, b| a.1 == b.1);
  scored
    .into_iter()
    .take(3)
    .map(|(_, item)| match prefix {
      Some(prefix) => Arc::from(format!("{prefix}/{item}")),
      None => item,
    })
    .collect()
}

/// Optimal string alignment distance, so a swapped pair (`Nubmer`) costs one.
fn edit_distance(left: &str, right: &str) -> usize {
  let a: Vec<char> = left.chars().collect();
  let b: Vec<char> = right.chars().collect();
  let mut rows = vec![vec![0usize; b.len() + 1]; a.len() + 1];
  for (i, row) in rows.iter_mut().enumerate() {
    row[0] = i;
  }
  for (j, cell) in rows[0].iter_mut().enumerate() {
    *cell = j;
  }
  for i in 1..=a.len() {
    for j in 1..=b.len() {
      let cost = usize::from(a[i - 1] != b[j - 1]);
      let mut best = (rows[i - 1][j] + 1).min(rows[i][j - 1] + 1).min(rows[i - 1][j - 1] + cost);
      if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
        best = best.min(rows[i - 2][j - 2] + 1);
      }
      rows[i][j] = best;
    }
  }
  rows[a.len()][b.len()]
}

/// The blocking error for an unresolved name, worded for its position
/// (`schema`, `hint-fn`, `assert-type`, a struct field, ...).
pub(super) fn unknown_type_name_error(
  owner: &str,
  position: &str,
  unknown: &UnknownTypeName,
  file_ns: &str,
  call_stack: &CallStackList,
  location: Option<NodeLocation>,
) -> CalcitErr {
  let hint = if unknown.candidates.is_empty() {
    match unknown.name.rsplit_once('/') {
      Some((prefix, def)) => {
        let target = program::lookup_ns_target_in_import(file_ns, prefix).unwrap_or_else(|| Arc::from(prefix));
        if program::has_ns_code(&target) {
          format!("namespace `{target}` has no definition `{def}`")
        } else {
          format!("`{prefix}` is neither a loaded namespace nor an `:as` alias of `{file_ns}`")
        }
      }
      None => format!(
        "define it, add `(target.ns :refer $ {name})` to the ns form of `{file_ns}`, or write `alias/{name}` / `target.ns/{name}`",
        name = unknown.name
      ),
    }
  } else {
    let names = unknown
      .candidates
      .iter()
      .map(|item| format!("`{item}`"))
      .collect::<Vec<_>>()
      .join(", ");
    format!("did you mean {names}?")
  };
  CalcitErr::use_msg_stack_location_with_code(
    CalcitErrKind::Type,
    format!(
      "{owner} uses unknown type name `{}` in {position} at {}; it is not a built-in type and resolves to no definition in `{file_ns}`; {hint}",
      unknown.name, unknown.path
    ),
    UNKNOWN_TYPE_NAME_ERROR,
    call_stack,
    location,
  )
}

pub(crate) const TYPE_ALIAS_ARITY_ERROR: &str = "E_TYPE_ALIAS_ARITY";

/// A `deftype` reference applied to the wrong number of type arguments:
/// `(path, name, generics, actual)`.
type AliasArityMismatch = (String, Arc<str>, Vec<Arc<str>>, usize);

/// The first `deftype` reference in `annotation` with the wrong arity.
fn find_type_alias_arity_mismatch(annotation: &Arc<CalcitTypeAnnotation>, file_ns: &str, path: &str) -> Option<AliasArityMismatch> {
  super::find_schema_annotation(annotation, path, &mut |candidate, candidate_path| match candidate.as_ref() {
    CalcitTypeAnnotation::TypeRef(name, args) => {
      let (ns, def) = program::resolve_type_name_in_ns(file_ns, name)?;
      let alias = crate::calcit::type_alias_declaration(&format!("{ns}/{def}"))?;
      (alias.generics.len() != args.len()).then(|| {
        (
          candidate_path.to_owned(),
          Arc::from(format!("{ns}/{def}")),
          alias.generics.clone(),
          args.len(),
        )
      })
    }
    _ => None,
  })
}

/// Reject an unresolved nominal reference in `annotation`.
pub(super) fn reject_unknown_type_names(
  owner: &str,
  position: &str,
  annotation: &Arc<CalcitTypeAnnotation>,
  file_ns: &str,
  call_stack: &CallStackList,
  location: Option<NodeLocation>,
) -> Result<(), CalcitErr> {
  if let Some(unknown) = find_unknown_type_name(annotation, file_ns, position) {
    return Err(unknown_type_name_error(owner, position, &unknown, file_ns, call_stack, location));
  }
  if let Some((path, name, generics, actual)) = find_type_alias_arity_mismatch(annotation, file_ns, position) {
    let short = name.rsplit_once('/').map_or(name.as_ref(), |(_, def)| def);
    let usage = if generics.is_empty() {
      short.to_owned()
    } else {
      format!("({short} {})", generics.join(" "))
    };
    return Err(CalcitErr::use_msg_stack_location_with_code(
      CalcitErrKind::Type,
      format!(
        "{owner} applies type alias `{name}` to {actual} type argument(s) in {position} at {path}, but its deftype declares {}; write it as `{usage}`",
        generics.len()
      ),
      TYPE_ALIAS_ARITY_ERROR,
      call_stack,
      location,
    ));
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::edit_distance;

  #[test]
  fn edit_distance_counts_a_swap_as_one() {
    assert_eq!(edit_distance("Nubmer", "Number"), 1);
    assert_eq!(edit_distance("Strng", "String"), 1);
    assert_eq!(edit_distance("Task", "Task"), 0);
    assert_eq!(edit_distance("Tsk", "Task"), 1);
    assert_eq!(edit_distance("Pt", "Vec3"), 4);
  }
}
