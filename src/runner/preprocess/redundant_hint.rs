//! Body `hint-fn` declarations that repeat the signature a lambda already
//! receives from its call context (#2636).

use std::collections::BTreeSet;
use std::sync::{LazyLock, Mutex};

use crate::calcit::{CalcitFnTypeAnnotation, CalcitTypeAnnotation, NodeLocation};

/// One redundant `hint-fn` form, by definition and source coordinate.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RedundantHintSite {
  pub ns: String,
  pub def: String,
  pub coord: Vec<u16>,
}

static SITES: LazyLock<Mutex<BTreeSet<RedundantHintSite>>> = LazyLock::new(|| Mutex::new(BTreeSet::new()));

/// How much of a written body hint the expected callback contract repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HintRepeat {
  /// Inputs and result are the same concrete types.
  Whole,
  /// Inputs are the same; the context leaves the result to a callee-owned
  /// output variable, so the hint is redundant only when the body infers the
  /// same result by itself.
  InputsWithOpenResult,
  No,
}

/// Compares a written body hint with the expected callback contract. A hint
/// with generics, bounds, features, an open reason or an async marker of its
/// own always says more than the context.
pub(super) fn compare_with_expected(
  hint: &CalcitFnTypeAnnotation,
  expected: &CalcitFnTypeAnnotation,
  callee_owned_generics: &[std::sync::Arc<str>],
  file_ns: &str,
) -> HintRepeat {
  if !hint.generics.is_empty()
    || !hint.where_bounds.is_empty()
    || !hint.features.is_empty()
    || hint.open_reason.0.is_some()
    || hint.is_async_invocation()
    || hint.arg_types.len() != expected.arg_types.len()
  {
    return HintRepeat::No;
  }
  let resolve =
    |annotation: &std::sync::Arc<CalcitTypeAnnotation>| super::resolve_namespace_type_refs_for_body(annotation.clone(), file_ns);
  let same = |written: &std::sync::Arc<CalcitTypeAnnotation>, context: &std::sync::Arc<CalcitTypeAnnotation>| {
    let context = resolve(context);
    !context.contains_type_var() && resolve(written) == context
  };
  let rest_matches = match (&hint.rest_type, &expected.rest_type) {
    (None, None) => true,
    (Some(written), Some(context)) => same(written, context),
    _ => false,
  };
  if !rest_matches
    || !hint
      .arg_types
      .iter()
      .zip(expected.arg_types.iter())
      .all(|(written, context)| same(written, context))
  {
    return HintRepeat::No;
  }
  if same(&hint.return_type, &expected.return_type) {
    return HintRepeat::Whole;
  }
  match expected.return_type.as_ref() {
    CalcitTypeAnnotation::TypeVar(name) if callee_owned_generics.contains(name) && !hint.return_type.contains_type_var() => {
      HintRepeat::InputsWithOpenResult
    }
    _ => HintRepeat::No,
  }
}

/// Whether the result a body infers on its own is the written hint result.
pub(super) fn same_result(
  written: &std::sync::Arc<CalcitTypeAnnotation>,
  inferred: &std::sync::Arc<CalcitTypeAnnotation>,
  file_ns: &str,
) -> bool {
  let inferred = super::resolve_namespace_type_refs_for_body(inferred.clone(), file_ns);
  !inferred.contains_type_var()
    && !matches!(
      inferred.as_ref(),
      CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::DynFn | CalcitTypeAnnotation::Never
    )
    && super::resolve_namespace_type_refs_for_body(written.clone(), file_ns) == inferred
}

/// Records a redundant hint written directly in project source.
pub(super) fn record(location: &NodeLocation) {
  if location.coord.is_empty() {
    return;
  }
  SITES.lock().expect("lock redundant hint sites").insert(RedundantHintSite {
    ns: location.ns.to_string(),
    def: location.def.to_string(),
    coord: location.coord.as_ref().clone(),
  });
}

/// Redundant hints recorded so far, in namespace, definition and coordinate order.
pub fn redundant_hint_sites() -> Vec<RedundantHintSite> {
  SITES.lock().expect("lock redundant hint sites").iter().cloned().collect()
}

pub fn clear_redundant_hint_sites() {
  SITES.lock().expect("lock redundant hint sites").clear();
}
