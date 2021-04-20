use cirru_edn::CirruEdn;
use cirru_edn::CirruEdn::*;
use cirru_parser::{CirruNode, CirruNode::*};
use std::collections::hash_map::HashMap;

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
  pub ns: CirruNode,
  pub defs: HashMap<String, CirruNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
  pub package: String,
  pub configs: SnapshotConfigs,
  pub files: HashMap<String, FileInSnapShot>,
}

fn load_configs(data: CirruEdn) -> Result<SnapshotConfigs, String> {
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
      CirruEdnNil => String::from(""),
      x => match edn::as_string(x) {
        Ok(v) => v,
        Err(e) => return Err(format!("failed to load version, {}", e)),
      },
    },
    modules: match edn::map_get(&data, "modules") {
      CirruEdnNil => vec![],
      x => load_modules(x)?,
    },
  };
  Ok(c)
}

fn load_modules(data: CirruEdn) -> Result<Vec<String>, String> {
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

fn load_file_info(data: CirruEdn) -> Result<FileInSnapShot, String> {
  let ns_code = edn::as_cirru(edn::map_get(&data, "ns"))?;
  let defs = edn::as_map(edn::map_get(&data, "defs"))?;
  let mut defs_info: HashMap<String, CirruNode> = HashMap::new();
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

fn load_files(data: CirruEdn) -> Result<HashMap<String, FileInSnapShot>, String> {
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
pub fn load_snapshot_data(data: CirruEdn) -> Result<Snapshot, String> {
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
  match cirru_parser::parse_cirru(code.to_string()) {
    Ok(lines) => {
      let code = match lines {
        CirruList(line) => {
          if line.len() == 1 {
            line[0].clone()
          } else {
            return Err(format!("unexpected snippet: {}", code));
          }
        }
        CirruLeaf(s) => return Err(format!("unexpected snippet: {}", s)),
      };
      let mut def_dict: HashMap<String, CirruNode> = HashMap::new();
      def_dict.insert(
        String::from("main!"),
        CirruList(vec![
          CirruLeaf(String::from("defn")),
          CirruLeaf(String::from("main!")),
          CirruList(vec![]),
          code,
        ]),
      );
      Ok(FileInSnapShot {
        ns: CirruList(vec![
          CirruLeaf(String::from("ns")),
          CirruLeaf(String::from("app.main")),
        ]),
        defs: def_dict,
      })
    }
    Err(e) => Err(format!("failed to make snapshot: {}", e)),
  }
}
