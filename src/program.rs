use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

use cirru_parser::Cirru;

use crate::calcit::{Calcit, ImportRule};
use crate::data::cirru::code_to_calcit;
use crate::snapshot;
use crate::snapshot::Snapshot;
use crate::util::string::extract_pkg_from_ns;

pub type ProgramEvaledData = HashMap<Arc<str>, HashMap<Arc<str>, Calcit>>;

/// information extracted from snapshot
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramFileData {
  pub import_map: HashMap<Arc<str>, Arc<ImportRule>>,
  pub defs: HashMap<Arc<str>, Calcit>,
}

type ImportMapPair = (Arc<str>, Arc<ImportRule>);

pub type ProgramCodeData = HashMap<Arc<str>, ProgramFileData>;

lazy_static! {
  /// data of program running
  static ref PROGRAM_EVALED_DATA_STATE: RwLock<ProgramEvaledData> = RwLock::new(HashMap::new());
  /// raw code information before program running
  pub static ref PROGRAM_CODE_DATA: RwLock<ProgramCodeData> = RwLock::new(HashMap::new());
}

fn extract_import_rule(nodes: &Cirru) -> Result<Vec<ImportMapPair>, String> {
  match nodes {
    Cirru::Leaf(_) => Err(String::from("Expected import rule in expr")),
    Cirru::List(rule_nodes) => {
      let mut xs = rule_nodes.to_owned();
      match xs.first() {
        // strip leading `[]` symbols
        Some(Cirru::Leaf(s)) if &**s == "[]" => xs = xs[1..4].to_vec(),
        // allow using comment
        Some(Cirru::Leaf(s)) if &**s == ";" => return Ok(vec![]),
        _ => (),
      }
      match (&xs[0], &xs[1], &xs[2]) {
        (Cirru::Leaf(ns), x, Cirru::Leaf(alias)) if x.eq_leaf(":as") => Ok(vec![(
          (*alias.to_owned()).into(),
          Arc::new(ImportRule::NsAs((*ns.to_owned()).into())),
        )]),
        (Cirru::Leaf(ns), x, Cirru::Leaf(alias)) if x.eq_leaf(":default") => Ok(vec![(
          (*alias.to_owned()).into(),
          Arc::new(ImportRule::NsDefault((*ns.to_owned()).into())),
        )]),
        (Cirru::Leaf(ns), x, Cirru::List(ys)) if x.eq_leaf(":refer") => {
          let mut rules: Vec<(Arc<str>, Arc<ImportRule>)> = Vec::with_capacity(ys.len());
          for y in ys {
            match y {
              Cirru::Leaf(s) if &**s == "[]" => (), // `[]` symbol are ignored
              Cirru::Leaf(s) => rules.push((
                (*s.to_owned()).into(),
                Arc::new(ImportRule::NsReferDef((*ns.to_owned()).into(), (*s.to_owned()).into())),
              )),
              Cirru::List(_defs) => return Err(String::from("invalid refer values")),
            }
          }
          Ok(rules)
        }
        (_, x, _) if x.eq_leaf(":as") => Err(format!("invalid import rule: {nodes}")),
        (_, x, _) if x.eq_leaf(":default") => Err(format!("invalid default rule: {nodes}")),
        (_, x, _) if x.eq_leaf(":refer") => Err(format!("invalid import rule: {nodes}")),
        _ if xs.len() != 3 => Err(format!("expected import rule has length 3: {nodes}")),
        _ => Err(String::from("unknown rule")),
      }
    }
  }
}

fn extract_import_map(nodes: &Cirru) -> Result<HashMap<Arc<str>, Arc<ImportRule>>, String> {
  match nodes {
    Cirru::Leaf(_) => unreachable!("Expected expr for ns"),
    Cirru::List(xs) => match (xs.first(), xs.get(1), xs.get(2)) {
      // Too many clones
      (Some(x), Some(Cirru::Leaf(_)), Some(Cirru::List(xs))) if x.eq_leaf("ns") => {
        if !xs.is_empty() && xs[0].eq_leaf(":require") {
          let mut ys: HashMap<Arc<str>, Arc<ImportRule>> = HashMap::with_capacity(xs.len());
          for x in xs.iter().skip(1) {
            let rules = extract_import_rule(x)?;
            for (target, rule) in rules {
              ys.insert(target, rule);
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

fn extract_file_data(file: &snapshot::FileInSnapShot, ns: Arc<str>) -> Result<ProgramFileData, String> {
  let import_map = extract_import_map(&file.ns.code)?;
  let mut defs: HashMap<Arc<str>, Calcit> = HashMap::with_capacity(file.defs.len());
  for (def, entry) in &file.defs {
    let at_def = def.to_owned();
    defs.insert(def.to_owned(), code_to_calcit(&entry.code, &ns, &at_def, vec![])?);
  }
  Ok(ProgramFileData { import_map, defs })
}

pub fn extract_program_data(s: &Snapshot) -> Result<ProgramCodeData, String> {
  let mut xs: ProgramCodeData = HashMap::with_capacity(s.files.len());
  for (ns, file) in &s.files {
    let file_info = extract_file_data(file, ns.to_owned())?;
    xs.insert(ns.to_owned(), file_info);
  }
  Ok(xs)
}

// lookup without cloning
pub fn has_def_code(ns: &str, def: &str) -> bool {
  let program_code = { PROGRAM_CODE_DATA.read().expect("read program code") };
  match &program_code.get(ns) {
    Some(v) => v.defs.contains_key(def),
    None => false,
  }
}

pub fn lookup_def_code(ns: &str, def: &str) -> Option<Calcit> {
  let program_code = { PROGRAM_CODE_DATA.read().expect("read program code") };
  let file = program_code.get(ns)?;
  let data = file.defs.get(def)?;
  Some(data.to_owned())
}

pub fn lookup_def_target_in_import(ns: &str, def: &str) -> Option<Arc<str>> {
  let program = { PROGRAM_CODE_DATA.read().expect("read program code") };
  let file = program.get(ns)?;
  let import_rule = file.import_map.get(def)?;
  match &**import_rule {
    ImportRule::NsReferDef(ns, _def) => Some(ns.to_owned()),
    ImportRule::NsAs(_ns) => None,
    ImportRule::NsDefault(_ns) => None,
  }
}

pub fn lookup_ns_target_in_import(ns: &str, alias: &str) -> Option<Arc<str>> {
  let program = { PROGRAM_CODE_DATA.read().expect("read program code") };
  let file = program.get(ns)?;
  let import_rule = file.import_map.get(alias)?;
  match &**import_rule {
    ImportRule::NsReferDef(_ns, _def) => None,
    ImportRule::NsAs(ns) => Some(ns.to_owned()),
    ImportRule::NsDefault(_ns) => None,
  }
}

// imported via :default
pub fn lookup_default_target_in_import(ns: &str, alias: &str) -> Option<Arc<str>> {
  let program = { PROGRAM_CODE_DATA.read().expect("read program code") };
  let file = program.get(ns)?;
  let import_rule = file.import_map.get(alias)?;
  match &**import_rule {
    ImportRule::NsReferDef(_ns, _def) => None,
    ImportRule::NsAs(_ns) => None,
    ImportRule::NsDefault(ns) => Some(ns.to_owned()),
  }
}

/// lookup and return value
pub fn lookup_evaled_def(ns: &str, def: &str) -> Option<Calcit> {
  let s2 = PROGRAM_EVALED_DATA_STATE.read().expect("read program data");
  s2.get(ns)?.get(def).cloned()
}

// Dirty mutating global states
pub fn write_evaled_def(ns: &str, def: &str, value: Calcit) -> Result<(), String> {
  // println!("writing {} {}", ns, def);
  let mut program = PROGRAM_EVALED_DATA_STATE.write().expect("read program data");

  match (*program).entry(Arc::from(ns)) {
    Entry::Occupied(_) => (),
    Entry::Vacant(entry) => {
      entry.insert(HashMap::new());
    }
  }

  let file = program.get_mut(ns).ok_or_else(|| format!("can not write to: {ns}"))?;
  file.insert(String::from(def).into(), value);

  Ok(())
}

// take a snapshot for codegen
pub fn clone_evaled_program() -> ProgramEvaledData {
  let program = &PROGRAM_EVALED_DATA_STATE.read().expect("read program data");

  let mut xs: ProgramEvaledData = HashMap::new();
  let ys = program.keys();
  for k in ys {
    xs.insert(k.to_owned(), program[k].to_owned());
  }
  xs
}

pub fn apply_code_changes(changes: &snapshot::ChangesDict) -> Result<(), String> {
  let mut program_code = PROGRAM_CODE_DATA.write().expect("open program code");
  let coord0 = vec![];

  for (ns, file) in &changes.added {
    program_code.insert(ns.to_owned(), extract_file_data(file, ns.to_owned())?);
  }
  for ns in &changes.removed {
    program_code.remove(ns);
  }
  for (ns, info) in &changes.changed {
    // println!("handling ns: {:?} {}", ns, program_code.contains_key(ns));
    let file = program_code.get_mut(ns).ok_or_else(|| format!("can not load ns: {ns}"))?;
    if let Some(v) = &info.ns {
      file.import_map = extract_import_map(v)?;
    }
    for (def, code) in &info.added_defs {
      file.defs.insert(def.to_owned(), code_to_calcit(code, ns, def, coord0.clone())?);
    }
    for def in &info.removed_defs {
      file.defs.remove(def);
    }
    for (def, code) in &info.changed_defs {
      file.defs.insert(def.to_owned(), code_to_calcit(code, ns, def, coord0.clone())?);
    }
  }

  Ok(())
}

/// clear evaled data after reloading
pub fn clear_all_program_evaled_defs(init_ns: Arc<str>, reload_ns: Arc<str>, reload_libs: bool) -> Result<(), String> {
  let mut program = PROGRAM_EVALED_DATA_STATE.write().expect("open program data");
  if reload_libs {
    (*program).clear();
  } else {
    // reduce changes of libs. could be dirty in some cases
    let init_pkg = extract_pkg_from_ns(init_ns.to_owned()).ok_or_else(|| format!("failed to extract pkg from: {init_ns}"))?;
    let reload_pkg = extract_pkg_from_ns(reload_ns.to_owned()).ok_or_else(|| format!("failed to extract pkg from: {reload_ns}"))?;
    let mut to_remove: Vec<Arc<str>> = vec![];
    let xs = program.keys();
    for k in xs {
      if k == &init_pkg || k == &reload_pkg || k.starts_with(&format!("{init_pkg}.")) || k.starts_with(&format!("{reload_pkg}.")) {
        to_remove.push(k.to_owned());
      } else {
        continue;
      }
    }
    for k in to_remove {
      (*program).remove(&k);
    }
  }
  Ok(())
}
