//! Runtime results use the same nominal definitions as core source constructors.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use crate::calcit::data_shape::program_registry_generation;
use crate::calcit::{CORE_NS, Calcit, CalcitEnumDef, CalcitEnumValue, CalcitErr, CalcitErrKind};
use crate::call_stack::CallStackList;

type CachedDefinitions = HashMap<&'static str, (usize, Arc<CalcitEnumDef>)>;
static CORE_ENUM_CACHE: LazyLock<Mutex<CachedDefinitions>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn core_enum_def(name: &'static str) -> Result<Arc<CalcitEnumDef>, CalcitErr> {
  // Resolving can preprocess a definition and change the generation. Never hold
  // the cache lock across that work; a changed generation forces a fresh lookup.
  let generation = program_registry_generation();
  if let Some((cached, def)) = CORE_ENUM_CACHE.lock().unwrap_or_else(|e| e.into_inner()).get(name)
    && *cached == generation
  {
    return Ok(def.clone());
  }
  let def = match crate::runner::eval_symbol_from_program(name, CORE_NS, &CallStackList::default())? {
    Some(Calcit::EnumDef(def)) => Arc::new(def),
    Some(other) => {
      return Err(CalcitErr::use_str(
        CalcitErrKind::Type,
        format!("calcit.core/{name} is expected to be an enum definition, but resolved to: {other}"),
      ));
    }
    None => {
      return Err(CalcitErr::use_str(
        CalcitErrKind::Var,
        format!("calcit.core/{name} is unavailable, cannot build a nominal result"),
      ));
    }
  };
  CORE_ENUM_CACHE
    .lock()
    .unwrap_or_else(|e| e.into_inner())
    .insert(name, (generation, def.clone()));
  Ok(def)
}

pub(crate) fn value(name: &'static str, tag: &str, extra: Vec<Calcit>) -> Result<Calcit, CalcitErr> {
  Ok(Calcit::Enum(CalcitEnumValue {
    tag: Arc::new(Calcit::tag(tag)),
    extra,
    sum_type: Some(core_enum_def(name)?),
  }))
}
