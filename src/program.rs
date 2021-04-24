use crate::snapshot::Snapshot;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::data::cirru::code_to_calcit;
use crate::primes::Calcit;

use cirru_parser::Cirru;

pub type ProgramEvaledData = HashMap<String, HashMap<String, Calcit>>;

/// defRule: ns def
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportRule {
  ImportNsRule(String),          // ns
  ImportDefRule(String, String), // ns, def
}

/// information extracted from snapshot
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramFileData {
  pub import_map: HashMap<String, ImportRule>,
  pub defs: HashMap<String, Calcit>,
}

pub type ProgramCodeData = HashMap<String, ProgramFileData>;

lazy_static! {
  static ref PROGRAM_EVALED_DATA_STATE: Mutex<ProgramEvaledData> = Mutex::new(HashMap::new());
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
          Ok(vec![(alias, ImportRule::ImportNsRule(ns))])
        }
        (Cirru::Leaf(ns), x, Cirru::List(ys)) if x == Cirru::Leaf(String::from(":refer")) => {
          let mut rules: Vec<(String, ImportRule)> = vec![];
          for y in ys {
            match y {
              Cirru::Leaf(s) if &s == "[]" => (), // `[]` symbol are ignored
              Cirru::Leaf(s) => rules.push((s.clone(), ImportRule::ImportDefRule(ns.clone(), s.clone()))),
              Cirru::List(_defs) => return Err(String::from("invalid refer values")),
            }
          }
          Ok(rules)
        }
        (_, x, _) if x == Cirru::Leaf(String::from(":as")) => Err(String::from("invalid import rule")),
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

pub fn extract_program_data(s: &Snapshot) -> Result<ProgramCodeData, String> {
  let mut xs: ProgramCodeData = HashMap::new();
  for (ns, file) in s.files.clone() {
    let import_map = extract_import_map(&file.ns)?;
    let mut defs: HashMap<String, Calcit> = HashMap::new();
    for (def, code) in file.defs {
      defs.insert(def, code_to_calcit(&code, &ns)?);
    }
    let file_info = ProgramFileData { import_map, defs };
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
    ImportRule::ImportDefRule(ns, _def) => Some(String::from(ns)),
    ImportRule::ImportNsRule(_ns) => None,
  }
}

pub fn lookup_ns_target_in_import(ns: &str, alias: &str, program: &ProgramCodeData) -> Option<String> {
  let file = program.get(ns)?;
  let import_rule = file.import_map.get(alias)?;
  match import_rule {
    ImportRule::ImportDefRule(_ns, _def) => None,
    ImportRule::ImportNsRule(ns) => Some(String::from(ns)),
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
