use crate::snapshot::Snapshot;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::data::cirru::code_to_calcit;
use crate::primes::CalcitData;

use cirru_parser::CirruNode;
use cirru_parser::CirruNode::*;

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

lazy_static! {
  static ref PROGRAM_EVALED_DATA_STATE: Mutex<ProgramEvaledData> = Mutex::new(HashMap::new());
}

fn extract_import_rule(nodes: &CirruNode) -> Result<Vec<(String, ImportRule)>, String> {
  match nodes {
    CirruLeaf(_) => Err(String::from("Expected import rule in expr")),
    CirruList(rule_nodes) => {
      let mut xs = rule_nodes.clone();
      match xs.get(0) {
        // strip leading `[]` symbols
        Some(CirruLeaf(s)) if s == "[]" => xs = xs[1..4].to_vec(),
        _ => (),
      }
      match (xs[0].clone(), xs[1].clone(), xs[2].clone()) {
        (CirruLeaf(ns), x, CirruLeaf(alias)) if x == CirruLeaf(String::from(":as")) => {
          Ok(vec![(alias, ImportRule::ImportNsRule(ns))])
        }
        (CirruLeaf(ns), x, CirruList(ys)) if x == CirruLeaf(String::from(":refer")) => {
          let mut rules: Vec<(String, ImportRule)> = vec![];
          for y in ys {
            match y {
              CirruLeaf(s) if &s == "[]" => (), // `[]` symbol are ignored
              CirruLeaf(s) => {
                rules.push((s.clone(), ImportRule::ImportDefRule(ns.clone(), s.clone())))
              }
              CirruList(_defs) => return Err(String::from("invalid refer values")),
            }
          }
          Ok(rules)
        }
        (_, x, _) if x == CirruLeaf(String::from(":as")) => {
          Err(String::from("invalid import rule"))
        }
        (_, x, _) if x == CirruLeaf(String::from(":refer")) => {
          Err(String::from("invalid import rule"))
        }
        _ if xs.len() != 3 => Err(format!(
          "expected import rule has length 3: {}",
          CirruList(xs.clone())
        )),
        _ => Err(String::from("unknown rule")),
      }
    }
  }
}

fn extract_import_map(nodes: &CirruNode) -> Result<HashMap<String, ImportRule>, String> {
  match nodes {
    CirruLeaf(_) => unreachable!("Expected expr for ns"),
    CirruList(xs) => match (xs.get(0), xs.get(1), xs.get(2)) {
      // Too many clones
      (Some(x), Some(CirruLeaf(_)), Some(CirruList(xs))) if *x == CirruLeaf(String::from("ns")) => {
        if !xs.is_empty() && xs[0] == CirruLeaf(String::from(":require")) {
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
    let mut defs: HashMap<String, CalcitData> = HashMap::new();
    for (def, code) in file.defs {
      defs.insert(def, code_to_calcit(&code, &ns)?);
    }
    let file_info = ProgramFileData { import_map, defs };
    xs.insert(ns, file_info);
  }
  Ok(xs)
}

pub fn lookup_ns_def(ns: &str, def: &str, program: &ProgramCodeData) -> Option<CalcitData> {
  let file = program.get(ns)?;
  let data = file.defs.get(def)?;
  Some(data.clone())
}

pub fn lookup_def_target_in_import(
  ns: &str,
  def: &str,
  program: &ProgramCodeData,
) -> Option<String> {
  let file = program.get(ns)?;
  let import_rule = file.import_map.get(def)?;
  match import_rule {
    ImportRule::ImportDefRule(ns, _def) => Some(String::from(ns)),
    ImportRule::ImportNsRule(_ns) => None,
  }
}

pub fn lookup_ns_target_in_import(
  ns: &str,
  def: &str,
  program: &ProgramCodeData,
) -> Option<String> {
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
    // println!("failed to lookup {} {}", ns, def);
    None
  }
}

// Dirty mutating global states
pub fn write_evaled_def(ns: &str, def: &str, value: CalcitData) -> Result<(), String> {
  // println!("writing {} {}", ns, def);
  let program = &mut PROGRAM_EVALED_DATA_STATE.lock().unwrap();
  if !program.contains_key(ns) {
    program.insert(String::from(ns), HashMap::new());
  }

  let file = program.get_mut(ns).unwrap();
  file.insert(String::from(def), value);

  Ok(())
}
