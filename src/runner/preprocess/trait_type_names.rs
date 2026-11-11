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
