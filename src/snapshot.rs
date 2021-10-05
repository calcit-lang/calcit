use cirru_edn::Edn;
use cirru_parser::Cirru;
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;
use std::path::Path;

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
    init_fn: match data.map_get("init-fn")?.read_string() {
      Ok(v) => v,
      Err(e) => return Err(format!("failed to load init-fn from: {}", e)),
    },
    reload_fn: match data.map_get("reload-fn")?.read_string() {
      Ok(v) => v,
      Err(e) => return Err(format!("failed to load reload-fn from: {}", e)),
    },
    version: match data.map_get("version")? {
      Edn::Nil => String::from(""),
      x => match x.read_string() {
        Ok(v) => v,
        Err(e) => return Err(format!("failed to load version, {}", e)),
      },
    },
    modules: match data.map_get("modules")? {
      Edn::Nil => vec![],
      x => load_modules(x)?,
    },
  };
  Ok(c)
}

fn load_modules(data: Edn) -> Result<Vec<String>, String> {
  match data.read_list() {
    Ok(xs) => {
      let mut ys: Vec<String> = vec![];
      for x in xs {
        ys.push(x.read_string()?)
      }
      Ok(ys)
    }
    Err(e) => Err(format!("failed to load modules, {}", e)),
  }
}

fn load_file_info(data: Edn) -> Result<FileInSnapShot, String> {
  let ns_code = data.map_get("ns")?.read_quoted_cirru()?;
  let defs = data
    .map_get("defs")?
    .read_map()
    .map_err(|e| format!("failed get `defs`:{}", e))?;
  let mut defs_info: HashMap<String, Cirru> = HashMap::new();
  for (k, v) in defs {
    let var = k.read_string()?;
    let def_code = v.read_quoted_cirru()?;
    defs_info.insert(var, def_code);
  }
  let file = FileInSnapShot {
    ns: ns_code,
    defs: defs_info,
  };
  Ok(file)
}

fn load_files(data: Edn) -> Result<HashMap<String, FileInSnapShot>, String> {
  let xs = data.read_map().map_err(|e| format!("failed loading files, {}", e))?;
  let mut ys: HashMap<String, FileInSnapShot> = HashMap::new();
  for (k, v) in xs {
    let key = k.read_string()?;
    let file = load_file_info(v)?;
    ys.insert(key, file);
  }
  Ok(ys)
}

/// parse snapshot
pub fn load_snapshot_data(data: Edn, path: &str) -> Result<Snapshot, String> {
  let pkg = data.map_get("package")?.read_string()?;
  let mut files = load_files(data.map_get("files")?)?;
  let meta_ns = format!("{}.$meta", pkg);
  files.insert(meta_ns.to_owned(), gen_meta_ns(&meta_ns, path));
  let s = Snapshot {
    package: pkg,
    configs: load_configs(data.map_get("configs")?)?,
    files,
  };
  Ok(s)
}

pub fn gen_meta_ns(ns: &str, path: &str) -> FileInSnapShot {
  let mut def_dict: HashMap<String, Cirru> = HashMap::new();
  def_dict.insert(
    String::from("calcit-filename"),
    Cirru::List(vec![
      Cirru::Leaf(String::from("def")),
      Cirru::Leaf(String::from("calcit-filename")),
      Cirru::Leaf(format!("|{}", path.escape_default())),
    ]),
  );
  let path_data = Path::new(path);
  let parent = path_data.parent().unwrap();
  let parent_str = parent.to_str().unwrap();

  def_dict.insert(
    String::from("calcit-dirname"),
    Cirru::List(vec![
      Cirru::Leaf(String::from("def")),
      Cirru::Leaf(String::from("calcit-dirname")),
      Cirru::Leaf(format!("|{}", parent_str.escape_default())),
    ]),
  );

  FileInSnapShot {
    ns: Cirru::List(vec![Cirru::Leaf(String::from("ns")), Cirru::Leaf(ns.to_owned())]),
    defs: def_dict,
  }
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

pub fn create_file_from_snippet(raw: &str) -> Result<FileInSnapShot, String> {
  match cirru_parser::parse(raw) {
    Ok(lines) => {
      let code = if lines.len() == 1 {
        lines[0].to_owned()
      } else {
        return Err(format!("unexpected snippet: {:?}", raw));
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
  for (ns, file) in &data.map_get("added")?.read_map_or_nil()? {
    added.insert(ns.read_string()?, load_file_info(file.to_owned())?);
  }

  let mut removed: HashSet<String> = HashSet::new();
  for item in &data.map_get("removed")?.read_set_or_nil()? {
    removed.insert(item.read_string()?);
  }

  let mut changed: HashMap<String, FileChangeInfo> = HashMap::new();
  for (ns, file) in &data.map_get("changed")?.read_map_or_nil()? {
    changed.insert(ns.read_string()?, extract_changed_info(file.to_owned())?);
  }

  Ok(ChangesDict {
    added,
    removed,
    changed,
  })
}

pub fn extract_changed_info(data: Edn) -> Result<FileChangeInfo, String> {
  let ns_info = match data.map_get("ns")? {
    Edn::Nil => Ok(None),
    Edn::Quote(code) => Ok(Some(code)),
    a => Err(format!("invalid information for ns code: {}", a)),
  };

  let mut added_defs: HashMap<String, Cirru> = HashMap::new();

  for (def, code) in data.map_get("added-defs")?.read_map_or_nil()? {
    added_defs.insert(def.read_string()?, code.read_quoted_cirru()?);
  }

  let mut removed_defs: HashSet<String> = HashSet::new();

  for def in data.map_get("removed-defs")?.read_set_or_nil()? {
    removed_defs.insert(def.read_string()?);
  }

  let mut changed_defs: HashMap<String, Cirru> = HashMap::new();
  for (def, code) in data.map_get("changed-defs")?.read_map_or_nil()? {
    changed_defs.insert(def.read_string()?, code.read_quoted_cirru()?);
  }

  Ok(FileChangeInfo {
    ns: ns_info?,
    added_defs,
    removed_defs,
    changed_defs,
  })
}
