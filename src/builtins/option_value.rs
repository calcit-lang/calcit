//! Construction of the nominal `calcit.core/Option` values returned by
//! primitives that used to return `nil` for absence (#1554).
//!
//! The enum definition is resolved from the program registry, so values built
//! here carry the same `Option` definition (and method impls) as values created
//! by `Option :some x` in Calcit source.

use std::sync::Arc;

use cirru_edn::EdnTag;

use crate::calcit::{CORE_NS, Calcit, CalcitEnumDef, CalcitEnumValue, CalcitErr, CalcitErrKind};
use crate::call_stack::CallStackList;

fn core_option_def() -> Result<Arc<CalcitEnumDef>, CalcitErr> {
  match crate::runner::eval_symbol_from_program("Option", CORE_NS, &CallStackList::default())? {
    Some(Calcit::EnumDef(def)) => Ok(Arc::new(def)),
    Some(other) => Err(CalcitErr::use_str(
      CalcitErrKind::Type,
      format!("calcit.core/Option is expected to be an enum definition, but resolved to: {other}"),
    )),
    None => Err(CalcitErr::use_str(
      CalcitErrKind::Var,
      "calcit.core/Option is unavailable, cannot build an Option result",
    )),
  }
}

/// `Option :some value`
pub(crate) fn some(value: Calcit) -> Result<Calcit, CalcitErr> {
  Ok(Calcit::Enum(CalcitEnumValue {
    tag: Arc::new(Calcit::Tag(EdnTag::new("some"))),
    extra: vec![value],
    sum_type: Some(core_option_def()?),
  }))
}

/// `Option :none`
pub(crate) fn none() -> Result<Calcit, CalcitErr> {
  Ok(Calcit::Enum(CalcitEnumValue {
    tag: Arc::new(Calcit::Tag(EdnTag::new("none"))),
    extra: vec![],
    sum_type: Some(core_option_def()?),
  }))
}

/// Wrap a lookup result: `Some(v)` becomes `Option :some v`, `None` becomes `Option :none`.
pub(crate) fn from_lookup(value: Option<Calcit>) -> Result<Calcit, CalcitErr> {
  match value {
    Some(v) => some(v),
    None => none(),
  }
}
