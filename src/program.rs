use std::collections::HashMap;
use std::sync::Mutex;

use crate::primes::{CalcitData, CalcitScope};

pub type ProgramEvaledData = HashMap<String, HashMap<String, CalcitData>>;

/// defRule: ns def
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportRule {
  ImportNsRule(String),
  ImportDefRule(String, String),
}

/// information extracted from snapshot
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramFileData {
  import_map: HashMap<String, ImportRule>,
  defs: HashMap<String, CalcitData>,
}

pub type ProgramCodeData = HashMap<String, ProgramFileData>;

pub type EvalFn =
  fn(data: CalcitData, scope: CalcitScope, code: &str, program: ProgramCodeData) -> CalcitData;

lazy_static! {
  static ref PROGRAM_EVALED_DATA_STATE: Mutex<ProgramEvaledData> = Mutex::new(HashMap::new());
}

fn extract_import_rule() {}

fn extract_import_map() {}

fn extract_program_data() {}

pub fn lookup_ns_def(ns: &str, def: &str, program: ProgramCodeData) -> Option<CalcitData> {
  let file = program.get(ns)?;
  let data = file.defs.get(def)?;
  Some(data.clone())
}

pub fn lookup_def_target_in_import(
  ns: &str,
  def: &str,
  program: ProgramCodeData,
) -> Option<String> {
  let file = program.get(ns)?;
  let import_rule = file.import_map.get(def)?;
  match import_rule {
    ImportRule::ImportDefRule(ns, _def) => Some(String::from(ns)),
    ImportRule::ImportNsRule(_ns) => None,
  }
}

pub fn lookup_ns_target_in_import(ns: &str, def: &str, program: ProgramCodeData) -> Option<String> {
  let file = program.get(ns)?;
  let import_rule = file.import_map.get(def)?;
  match import_rule {
    ImportRule::ImportDefRule(_ns, _def) => None,
    ImportRule::ImportNsRule(ns) => Some(String::from(ns)),
  }
}

pub fn lookup_evaled_def(ns: &str, def: &str) -> Option<CalcitData> {
  let s2 = PROGRAM_EVALED_DATA_STATE.lock().unwrap();
  if s2.contains_key(ns) && s2[ns].contains_key(def) {
    Some(s2[ns][def].clone())
  } else {
    None
  }
}

// Dirty mutating global states
pub fn write_evaled_def(ns: &str, def: &str, value: CalcitData) -> Result<(), String> {
  let program = &mut PROGRAM_EVALED_DATA_STATE.lock().unwrap();
  if !program.contains_key(ns) {
    program.insert(String::from(ns), HashMap::new());
  }

  let file = program.get_mut(ns).unwrap();
  file.insert(String::from(def), value);

  Ok(())
}
