//! Construction of the nominal `calcit.core/Option` values returned by
//! primitives that used to return `nil` for absence (#1554).
//!
//! The enum definition is resolved from the program registry, so values built
//! here carry the same `Option` definition (and method impls) as values created
//! by `Option :some x` in Calcit source.

use super::core_enum_value;
use crate::calcit::{Calcit, CalcitErr};

/// `Option :some value`
pub(crate) fn some(value: Calcit) -> Result<Calcit, CalcitErr> {
  core_enum_value::value("Option", "some", vec![value])
}

/// `Option :none`
pub(crate) fn none() -> Result<Calcit, CalcitErr> {
  core_enum_value::value("Option", "none", vec![])
}

/// Wrap a lookup result: `Some(v)` becomes `Option :some v`, `None` becomes `Option :none`.
pub(crate) fn from_lookup(value: Option<Calcit>) -> Result<Calcit, CalcitErr> {
  match value {
    Some(v) => some(v),
    None => none(),
  }
}
