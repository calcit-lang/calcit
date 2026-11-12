//! `E_TRAIT_AS_TYPE`: a trait name written where a type is expected. A trait
//! describes a capability that an implementation is statically selected for;
//! it is not a value type, so `Fn Labeled (-> String)` used to reject every
//! implementor. The capability is written as a bound on a type parameter:
//! `Fn T (-> String) (:generics T) (:where $ T Labeled)`. External-object
//! traits are host handle types and are not affected.

use std::sync::Arc;

use crate::calcit::{CalcitErr, CalcitErrKind, CalcitTypeAnnotation, NodeLocation};
use crate::call_stack::CallStackList;
use crate::program;

pub(crate) const TRAIT_AS_TYPE_ERROR: &str = "E_TRAIT_AS_TYPE";

/// A type reference that resolves to a trait definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TraitTypeName {
  pub path: String,
  pub name: Arc<str>,
}

/// Whether `name`, written in `file_ns`, names a `deftrait` definition.
fn type_name_is_trait(file_ns: &str, name: &str) -> bool {
  let stripped = name.trim_start_matches('\'').trim_start_matches(':');
  if stripped.is_empty() || CalcitTypeAnnotation::is_builtin_type_symbol_name(stripped) {
    return false;
  }
  match program::resolve_type_name_in_ns(file_ns, stripped) {
    // An external-object trait (`defexternal`, or `deftrait` with `:ffi`)
    // is the static type of a host handle, so it stays a valid type.
    Some((ns, def)) => {
      program::has_def_code(&ns, &def)
        && program::lookup_def_ffi(&ns, &def).is_none()
        && super::lookup_source_backed_trait_def(&ns, &def).is_some()
    }
    None => false,
  }
}

/// Find the first trait name used as a type in `annotation`.
pub(super) fn find_trait_type_name(annotation: &Arc<CalcitTypeAnnotation>, file_ns: &str, path: &str) -> Option<TraitTypeName> {
  super::find_schema_annotation(annotation, path, &mut |candidate, candidate_path| match candidate.as_ref() {
    CalcitTypeAnnotation::TypeRef(name, _) if type_name_is_trait(file_ns, name) => Some(TraitTypeName {
      path: candidate_path.to_owned(),
      name: Arc::from(name.trim_start_matches('\'')),
    }),
    _ => None,
  })
}

/// Reject a trait name used as a type in `annotation`, pointing to the
/// `:where` bound that expresses the capability.
pub(super) fn reject_trait_type_names(
  owner: &str,
  position: &str,
  annotation: &Arc<CalcitTypeAnnotation>,
  file_ns: &str,
  call_stack: &CallStackList,
  location: Option<NodeLocation>,
) -> Result<(), CalcitErr> {
  let Some(found) = find_trait_type_name(annotation, file_ns, position) else {
    return Ok(());
  };
  let name = &found.name;
  let hint = if position == "assert-type" {
    format!("check the capability with `assert-traits x {name}` instead")
  } else {
    format!("bound a type parameter instead: add `(:generics T) (:where $ T {name})` to the Fn type and write `T` in place of `{name}`")
  };
  Err(CalcitErr::use_msg_stack_location_with_code(
    CalcitErrKind::Type,
    format!(
      "{owner} uses trait `{name}` as a type in {position} at {path}; a trait is a capability, not a value type, and no value has it as its type; {hint}",
      path = found.path
    ),
    TRAIT_AS_TYPE_ERROR,
    call_stack,
    location,
  ))
}

pub(crate) const TRAIT_SELF_RECEIVER_ERROR: &str = "E_TRAIT_SELF_RECEIVER";

/// Method signatures of a `deftrait` call: `(name, type form)` for each
/// `.method` member; `(requires …)` clauses and `:field` members are skipped.
fn deftrait_method_forms(args: &crate::calcit::CalcitList) -> Vec<(String, crate::calcit::Calcit, Option<NodeLocation>)> {
  use crate::calcit::Calcit;
  args
    .iter()
    .skip(1)
    .filter_map(|item| {
      let Calcit::List(entry) = item else { return None };
      if super::is_trait_requires_entry(item) {
        return None;
      }
      let items: Vec<&Calcit> = match entry.first() {
        Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "[]" => entry.iter().skip(1).collect(),
        _ => entry.iter().collect(),
      };
      if items.len() != 2 {
        return None;
      }
      let name = match items[0] {
        Calcit::Method(name, _) => name.to_string(),
        _ => return None,
      };
      Some((name, items[1].to_owned(), item.get_location()))
    })
    .collect()
}

/// `deftrait` signatures name the receiver `Self`: a signature that names the
/// trait itself is rejected with a hint to write `Self`, and once any method
/// uses `Self`, every method takes `Self` as its first parameter (methods are
/// dispatched on that receiver). Traits written with a concrete receiver, such
/// as `OptionOps` over `Option T`, keep their shape.
pub(super) fn check_deftrait_self_contract(
  args: &crate::calcit::CalcitList,
  file_ns: &str,
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  let Some(trait_name) = args.first().and_then(super::parse_trait_name_from_source) else {
    return Ok(());
  };
  let trait_name = trait_name.ref_str().to_owned();
  let qualified = format!("{file_ns}/{trait_name}");
  let mut parsed = vec![];
  for (method, form, location) in deftrait_method_forms(args) {
    let type_form = crate::builtins::meta::qualify_trait_member_type_form(&form, file_ns);
    let annotation = crate::calcit::with_type_annotation_warning_context(format!("trait:{method}"), || {
      CalcitTypeAnnotation::parse_type_annotation_form(&type_form)
    });
    let path = format!(".{method}");
    let self_reference = super::find_schema_annotation(&annotation, &path, &mut |candidate, candidate_path| match candidate.as_ref() {
      // An undeclared short name in a Fn schema parses as a type variable.
      CalcitTypeAnnotation::TypeRef(name, _) | CalcitTypeAnnotation::TypeVar(name) => {
        let name = name.trim_start_matches('\'');
        (name == trait_name || name == qualified).then(|| candidate_path.to_owned())
      }
      _ => None,
    });
    if let Some(at) = self_reference {
      return Err(CalcitErr::use_msg_stack_location_with_code(
        CalcitErrKind::Type,
        format!(
          "deftrait {qualified} names itself as a type in `.{method}` at {at}; a trait is a capability, not a value type; write `Self` for the receiver type, e.g. `.{method} $ Fn Self (-> Self)`"
        ),
        TRAIT_AS_TYPE_ERROR,
        call_stack,
        location,
      ));
    }
    parsed.push((method, annotation, location));
  }
  let uses_self = parsed.iter().any(|(_, annotation, _)| {
    matches!(annotation.as_ref(), CalcitTypeAnnotation::Fn(info) if info.generics.iter().any(|name| name.as_ref() == "Self"))
  });
  if !uses_self {
    return Ok(());
  }
  for (method, annotation, location) in parsed {
    let CalcitTypeAnnotation::Fn(info) = annotation.as_ref() else {
      continue;
    };
    let receiver_is_self =
      matches!(info.arg_types.first().map(AsRef::as_ref), Some(CalcitTypeAnnotation::TypeVar(name)) if name.as_ref() == "Self");
    if !receiver_is_self {
      let actual = info
        .arg_types
        .first()
        .map(|item| item.to_brief_string())
        .unwrap_or_else(|| "no parameter".to_owned());
      return Err(CalcitErr::use_msg_stack_location_with_code(
        CalcitErrKind::Type,
        format!(
          "deftrait {qualified} uses `Self`, so every method takes `Self` as its first parameter, but `.{method}` starts with {actual}; methods are dispatched on that receiver, e.g. `.{method} $ Fn Self … (-> …)`"
        ),
        TRAIT_SELF_RECEIVER_ERROR,
        call_stack,
        location,
      ));
    }
  }
  Ok(())
}
