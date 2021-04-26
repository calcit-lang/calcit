use cirru_edn::Edn;
use cirru_parser::Cirru;
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;

use crate::data::edn;

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotConfigs {
  pub init_fn: String,
  pub reload_fn: String,
  pub modules: Vec<String>,
  pub version: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileInSnapShot {
  pub ns: Cirru,
  pub defs: HashMap<String, Cirru>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
  pub package: String,
  pub configs: SnapshotConfigs,
  pub files: HashMap<String, FileInSnapShot>,
}

fn load_configs(data: Edn) -> Result<SnapshotConfigs, String> {
  let c = SnapshotConfigs {
    init_fn: match edn::as_string(edn::map_get(&data, "init-fn")) {
      Ok(v) => v,
      Err(e) => return Err(format!("failed to load init-fn from: {}", e)),
    },
    reload_fn: match edn::as_string(edn::map_get(&data, "reload-fn")) {
      Ok(v) => v,
      Err(e) => return Err(format!("failed to load reload-fn from: {}", e)),
    },
    version: match edn::map_get(&data, "version") {
      Edn::Nil => String::from(""),
      x => match edn::as_string(x) {
        Ok(v) => v,
        Err(e) => return Err(format!("failed to load version, {}", e)),
      },
    },
    modules: match edn::map_get(&data, "modules") {
      Edn::Nil => vec![],
      x => load_modules(x)?,
    },
  };
  Ok(c)
}

fn load_modules(data: Edn) -> Result<Vec<String>, String> {
  match edn::as_vec(data) {
    Ok(xs) => {
      let mut ys: Vec<String> = vec![];
      for x in xs {
        ys.push(edn::as_string(x)?)
      }
      Ok(ys)
    }
    Err(e) => Err(format!("failed to load modules, {}", e)),
  }
}

fn load_file_info(data: Edn) -> Result<FileInSnapShot, String> {
  let ns_code = edn::as_cirru(edn::map_get(&data, "ns"))?;
  let defs = edn::as_map(edn::map_get(&data, "defs"))?;
  let mut defs_info: HashMap<String, Cirru> = HashMap::new();
  for (k, v) in defs {
    let var = edn::as_string(k)?;
    let def_code = edn::as_cirru(v)?;
    defs_info.insert(var, def_code);
  }
  let file = FileInSnapShot {
    ns: ns_code,
    defs: defs_info,
  };
  Ok(file)
}

fn load_files(data: Edn) -> Result<HashMap<String, FileInSnapShot>, String> {
  let xs = edn::as_map(data)?;
  let mut ys: HashMap<String, FileInSnapShot> = HashMap::new();
  for (k, v) in xs {
    let key = edn::as_string(k)?;
    let file = load_file_info(v)?;
    ys.insert(key, file);
  }
  Ok(ys)
}

/// parse snapshot
pub fn load_snapshot_data(data: Edn) -> Result<Snapshot, String> {
  let s = Snapshot {
    package: edn::as_string(edn::map_get(&data, "package"))?,
    configs: load_configs(edn::map_get(&data, "configs"))?,
    files: load_files(edn::map_get(&data, "files"))?,
  };
  Ok(s)
}

pub fn gen_default() -> Snapshot {
  Snapshot {
    package: String::from("app"),
    configs: SnapshotConfigs {
      init_fn: String::from("app.main/main!"),
      reload_fn: String::from("app.main/reload!"),
      version: String::from("0.0.0"),
      modules: vec![],
    },
    files: HashMap::new(),
  }
}

pub fn create_file_from_snippet(code: &str) -> Result<FileInSnapShot, String> {
  match cirru_parser::parse(code) {
    Ok(lines) => {
      let code = match lines {
        Cirru::List(line) => {
          if line.len() == 1 {
            line[0].clone()
          } else {
            return Err(format!("unexpected snippet: {}", code));
          }
        }
        Cirru::Leaf(s) => return Err(format!("unexpected snippet: {}", s)),
      };
      let mut def_dict: HashMap<String, Cirru> = HashMap::new();
      def_dict.insert(
        String::from("main!"),
        Cirru::List(vec![
          Cirru::Leaf(String::from("defn")),
          Cirru::Leaf(String::from("main!")),
          Cirru::List(vec![]),
          code,
        ]),
      );
      def_dict.insert(
        String::from("reload!"),
        Cirru::List(vec![
          Cirru::Leaf(String::from("defn")),
          Cirru::Leaf(String::from("reload!")),
          Cirru::List(vec![]),
        ]),
      );
      Ok(FileInSnapShot {
        ns: Cirru::List(vec![
          Cirru::Leaf(String::from("ns")),
          Cirru::Leaf(String::from("app.main")),
        ]),
        defs: def_dict,
      })
    }
    Err(e) => Err(format!("failed to make snapshot: {}", e)),
  }
}

#[derive(Debug, PartialEq, Clone)]
pub struct FileChangeInfo {
  pub ns: Option<Cirru>,
  pub added_defs: HashMap<String, Cirru>,
  pub removed_defs: HashSet<String>,
  pub changed_defs: HashMap<String, Cirru>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct ChangesDict {
  pub added: HashMap<String, FileInSnapShot>,
  pub removed: HashSet<String>,
  pub changed: HashMap<String, FileChangeInfo>,
}

pub fn load_changes_info(data: Edn) -> Result<ChangesDict, String> {
  // println!("loading changes: {}", data);
  let mut added: HashMap<String, FileInSnapShot> = HashMap::new();
  for (ns, file) in edn::as_optional_map(edn::map_get(&data, "added"))? {
    added.insert(edn::as_string(ns)?, load_file_info(file)?);
  }

  let mut removed: HashSet<String> = HashSet::new();
  for item in edn::as_optional_set(edn::map_get(&data, "removed"))? {
    removed.insert(edn::as_string(item)?);
  }

  let mut changed: HashMap<String, FileChangeInfo> = HashMap::new();
  for (ns, file) in edn::as_optional_map(edn::map_get(&data, "changed"))? {
    changed.insert(edn::as_string(ns)?, extract_changed_info(file)?);
  }

  Ok(ChangesDict {
    added,
    removed,
    changed,
  })
}

pub fn extract_changed_info(data: Edn) -> Result<FileChangeInfo, String> {
  let ns_info = match edn::map_get(&data, "ns") {
    Edn::Nil => Ok(None),
    Edn::Quote(code) => Ok(Some(code)),
    a => Err(format!("invalid information for ns code: {}", a)),
  };

  let mut added_defs: HashMap<String, Cirru> = HashMap::new();

  for (def, code) in edn::as_optional_map(edn::map_get(&data, "added-defs"))? {
    added_defs.insert(edn::as_string(def)?, edn::as_cirru(code)?);
  }

  let mut removed_defs: HashSet<String> = HashSet::new();

  for def in edn::as_optional_set(edn::map_get(&data, "removed-defs"))? {
    removed_defs.insert(edn::as_string(def)?);
  }

  let mut changed_defs: HashMap<String, Cirru> = HashMap::new();
  for (def, code) in edn::as_optional_map(edn::map_get(&data, "changed-defs"))? {
    changed_defs.insert(edn::as_string(def)?, edn::as_cirru(code)?);
  }

  Ok(FileChangeInfo {
    ns: ns_info?,
    added_defs,
    removed_defs,
    changed_defs,
  })
}
