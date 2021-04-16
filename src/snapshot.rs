use cirru_edn::CirruEdn;
use cirru_parser::CirruNode;
use std::collections::hash_map::HashMap;

use crate::data::edn;

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotConfigs {
  init_fn: String,
  reload_fn: String,
  modules: Vec<String>,
  version: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileInSnapShot {
  ns: CirruNode,
  defs: HashMap<String, CirruNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
  package: String,
  configs: SnapshotConfigs,
  files: HashMap<String, FileInSnapShot>,
}

fn load_configs(data: CirruEdn) -> Result<SnapshotConfigs, String> {
  let c = SnapshotConfigs {
    init_fn: edn::as_string(edn::map_get(&data, "init-fn"))?,
    reload_fn: edn::as_string(edn::map_get(&data, "reload-fn"))?,
    version: edn::as_string(edn::map_get(&data, "version"))?,
    modules: load_modules(edn::map_get(&data, "modules"))?,
  };
  Ok(c)
}

fn load_modules(data: CirruEdn) -> Result<Vec<String>, String> {
  let xs = edn::as_vec(data)?;
  let mut ys: Vec<String> = vec![];
  for x in xs {
    ys.push(edn::as_string(x)?)
  }
  Ok(ys)
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
