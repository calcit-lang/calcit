use std::collections::HashMap;
use std::sync::Mutex;

use cirru_parser::Cirru;

use crate::data::cirru::code_to_calcit;
use crate::primes::{Calcit, CalcitItems, ImportRule};
use crate::snapshot;
use crate::snapshot::Snapshot;
use crate::util::string::extract_pkg_from_def;

pub type ProgramEvaledData = HashMap<String, HashMap<String, Calcit>>;

/// information extracted from snapshot
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramFileData {
  pub import_map: HashMap<String, ImportRule>,
  pub defs: HashMap<String, Calcit>,
}

pub type ProgramCodeData = HashMap<String, ProgramFileData>;

lazy_static! {
  static ref PROGRAM_EVALED_DATA_STATE: Mutex<ProgramEvaledData> = Mutex::new(HashMap::new());
  // TODO need better soution for immediate calls
  /// to be read by external logics and used as FFI
  static ref PROGRAM_FFI_MESSAGES: Mutex<Vec<(String, CalcitItems)>> = Mutex::new(vec![]);
}

fn extract_import_rule(nodes: &Cirru) -> Result<Vec<(String, ImportRule)>, String> {
  match nodes {
    Cirru::Leaf(_) => Err(String::from("Expected import rule in expr")),
    Cirru::List(rule_nodes) => {
      let mut xs = rule_nodes.clone();
      match xs.get(0) {
        // strip leading `[]` symbols
        Some(Cirru::Leaf(s)) if s == "[]" => xs = xs[1..4].to_vec(),
        _ => (),
      }
      match (xs[0].clone(), xs[1].clone(), xs[2].clone()) {
        (Cirru::Leaf(ns), x, Cirru::Leaf(alias)) if x == Cirru::Leaf(String::from(":as")) => {
          Ok(vec![(alias, ImportRule::NsAs(ns))])
        }
        (Cirru::Leaf(ns), x, Cirru::Leaf(alias)) if x == Cirru::Leaf(String::from(":default")) => {
          Ok(vec![(alias, ImportRule::NsDefault(ns))])
        }
        (Cirru::Leaf(ns), x, Cirru::List(ys)) if x == Cirru::Leaf(String::from(":refer")) => {
          let mut rules: Vec<(String, ImportRule)> = vec![];
          for y in ys {
            match y {
              Cirru::Leaf(s) if &s == "[]" => (), // `[]` symbol are ignored
              Cirru::Leaf(s) => rules.push((s.clone(), ImportRule::NsReferDef(ns.clone(), s.clone()))),
              Cirru::List(_defs) => return Err(String::from("invalid refer values")),
            }
          }
          Ok(rules)
        }
        (_, x, _) if x == Cirru::Leaf(String::from(":as")) => Err(String::from("invalid import rule")),
        (_, x, _) if x == Cirru::Leaf(String::from(":default")) => Err(String::from("invalid default rule")),
        (_, x, _) if x == Cirru::Leaf(String::from(":refer")) => Err(String::from("invalid import rule")),
        _ if xs.len() != 3 => Err(format!(
          "expected import rule has length 3: {}",
          Cirru::List(xs.clone())
        )),
        _ => Err(String::from("unknown rule")),
      }
    }
  }
}

fn extract_import_map(nodes: &Cirru) -> Result<HashMap<String, ImportRule>, String> {
  match nodes {
    Cirru::Leaf(_) => unreachable!("Expected expr for ns"),
    Cirru::List(xs) => match (xs.get(0), xs.get(1), xs.get(2)) {
      // Too many clones
      (Some(x), Some(Cirru::Leaf(_)), Some(Cirru::List(xs))) if *x == Cirru::Leaf(String::from("ns")) => {
        if !xs.is_empty() && xs[0] == Cirru::Leaf(String::from(":require")) {
          let mut ys: HashMap<String, ImportRule> = HashMap::new();
          for (idx, x) in xs.iter().enumerate() {
            if idx > 0 {
              let rules = extract_import_rule(x)?;
              for (target, rule) in rules {
                ys.insert(target, rule);
              }
            }
          }
          Ok(ys)
        } else {
          Ok(HashMap::new())
        }
      }
      _ if xs.len() < 3 => Ok(HashMap::new()),
      _ => Err(String::from("invalid ns form")),
    },
  }
}

fn extract_file_data(file: snapshot::FileInSnapShot, ns: String) -> Result<ProgramFileData, String> {
  let import_map = extract_import_map(&file.ns)?;
  let mut defs: HashMap<String, Calcit> = HashMap::new();
  for (def, code) in file.defs {
    defs.insert(def, code_to_calcit(&code, &ns)?);
  }
  Ok(ProgramFileData { import_map, defs })
}

pub fn extract_program_data(s: &Snapshot) -> Result<ProgramCodeData, String> {
  let mut xs: ProgramCodeData = HashMap::new();
  for (ns, file) in s.files.clone() {
    let file_info = extract_file_data(file, ns.clone())?;
    xs.insert(ns, file_info);
  }
  Ok(xs)
}

// lookup without cloning
pub fn has_def_code(ns: &str, def: &str, program_code: &ProgramCodeData) -> bool {
  match program_code.get(ns) {
    Some(v) => v.defs.contains_key(def),
    None => false,
  }
}

pub fn lookup_def_code(ns: &str, def: &str, program_code: &ProgramCodeData) -> Option<Calcit> {
  let file = program_code.get(ns)?;
  let data = file.defs.get(def)?;
  Some(data.clone())
}

pub fn lookup_def_target_in_import(ns: &str, def: &str, program: &ProgramCodeData) -> Option<String> {
  let file = program.get(ns)?;
  let import_rule = file.import_map.get(def)?;
  match import_rule {
    ImportRule::NsReferDef(ns, _def) => Some(ns.to_owned()),
    ImportRule::NsAs(_ns) => None,
    ImportRule::NsDefault(_ns) => None,
  }
}

pub fn lookup_ns_target_in_import(ns: &str, alias: &str, program: &ProgramCodeData) -> Option<String> {
  let file = program.get(ns)?;
  let import_rule = file.import_map.get(alias)?;
  match import_rule {
    ImportRule::NsReferDef(_ns, _def) => None,
    ImportRule::NsAs(ns) => Some(ns.to_owned()),
    ImportRule::NsDefault(_ns) => None,
  }
}

// imported via :default
pub fn lookup_default_target_in_import(ns: &str, alias: &str, program: &ProgramCodeData) -> Option<String> {
  let file = program.get(ns)?;
  let import_rule = file.import_map.get(alias)?;
  match import_rule {
    ImportRule::NsReferDef(_ns, _def) => None,
    ImportRule::NsAs(_ns) => None,
    ImportRule::NsDefault(ns) => Some(ns.to_owned()),
  }
}

/// similar to lookup, but skipped cloning
#[allow(dead_code)]
pub fn has_evaled_def(ns: &str, def: &str) -> bool {
  let s2 = PROGRAM_EVALED_DATA_STATE.lock().unwrap();
  s2.contains_key(ns) && s2[ns].contains_key(def)
}

/// lookup and return value
pub fn lookup_evaled_def(ns: &str, def: &str) -> Option<Calcit> {
  let s2 = PROGRAM_EVALED_DATA_STATE.lock().unwrap();
  if s2.contains_key(ns) && s2[ns].contains_key(def) {
    Some(s2[ns][def].clone())
  } else {
    // println!("failed to lookup {} {}", ns, def);
    None
  }
}

// Dirty mutating global states
pub fn write_evaled_def(ns: &str, def: &str, value: Calcit) -> Result<(), String> {
  // println!("writing {} {}", ns, def);
  let program = &mut PROGRAM_EVALED_DATA_STATE.lock().unwrap();
  if !program.contains_key(ns) {
    program.insert(String::from(ns), HashMap::new());
  }

  let file = program.get_mut(ns).unwrap();
  file.insert(String::from(def), value);

  Ok(())
}

// take a snapshot for codegen
pub fn clone_evaled_program() -> ProgramEvaledData {
  let program = &PROGRAM_EVALED_DATA_STATE.lock().unwrap();

  let mut xs: ProgramEvaledData = HashMap::new();
  for k in program.keys() {
    xs.insert(k.clone(), program[k].clone());
  }
  xs
}

pub fn apply_code_changes(base: &ProgramCodeData, changes: &snapshot::ChangesDict) -> Result<ProgramCodeData, String> {
  let mut program_code = base.clone();

  for (ns, file) in &changes.added {
    program_code.insert(ns.to_owned(), extract_file_data(file.clone(), ns.to_owned())?);
  }
  for ns in &changes.removed {
    program_code.remove(ns);
  }
  for (ns, info) in &changes.changed {
    // println!("handling ns: {:?} {}", ns, program_code.contains_key(ns));
    let file = program_code.get_mut(ns).unwrap();
    if info.ns.is_some() {
      file.import_map = extract_import_map(&info.ns.clone().unwrap())?;
    }
    for (def, code) in &info.added_defs {
      file.defs.insert(def.to_owned(), code_to_calcit(code, ns)?);
    }
    for def in &info.removed_defs {
      file.defs.remove(def);
    }
    for (def, code) in &info.changed_defs {
      file.defs.insert(def.to_owned(), code_to_calcit(code, ns)?);
    }
  }

  Ok(program_code)
}

/// clear evaled data after reloading
pub fn clear_all_program_evaled_defs(init_fn: &str, reload_fn: &str, reload_libs: bool) -> Result<(), String> {
  let program = &mut PROGRAM_EVALED_DATA_STATE.lock().unwrap();
  if reload_libs {
    program.clear();
  } else {
    // reduce changes of libs. could be dirty in some cases
    let init_pkg = extract_pkg_from_def(init_fn).unwrap();
    let reload_pkg = extract_pkg_from_def(reload_fn).unwrap();
    let mut to_remove: Vec<String> = vec![];
    for k in program.keys() {
      if k == &init_pkg
        || k == &reload_pkg
        || k.starts_with(&format!("{}.", init_pkg))
        || k.starts_with(&format!("{}.", reload_pkg))
      {
        to_remove.push(k.to_owned());
      } else {
        continue;
      }
    }
    for k in to_remove {
      program.remove(&k);
    }
  }
  Ok(())
}

pub fn send_ffi_message(op: String, items: CalcitItems) {
  let ref_messages = &mut PROGRAM_FFI_MESSAGES.lock().unwrap();
  ref_messages.push((op, items))
}

pub fn take_ffi_messages() -> Result<Vec<(String, CalcitItems)>, String> {
  let mut messages: Vec<(String, CalcitItems)> = vec![];
  let ref_messages = &mut PROGRAM_FFI_MESSAGES.lock().unwrap();
  for m in ref_messages.iter() {
    messages.push(m.to_owned())
  }
  ref_messages.clear();
  Ok(messages)
}
