use cirru_edn::Edn;
use cirru_parser::Cirru;
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotConfigs {
  pub init_fn: Box<str>,
  pub reload_fn: Box<str>,
  pub modules: Vec<Box<str>>,
  pub version: Box<str>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileInSnapShot {
  pub ns: Cirru,
  pub defs: HashMap<Box<str>, Cirru>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
  pub package: Box<str>,
  pub configs: SnapshotConfigs,
  pub files: HashMap<Box<str>, FileInSnapShot>,
}

fn load_configs(data: Edn) -> Result<SnapshotConfigs, String> {
  let c = SnapshotConfigs {
    init_fn: match data.map_get("init-fn")?.read_str() {
      Ok(v) => v,
      Err(e) => return Err(format!("failed to load init-fn from: {}", e)),
    },
    reload_fn: match data.map_get("reload-fn")?.read_str() {
      Ok(v) => v,
      Err(e) => return Err(format!("failed to load reload-fn from: {}", e)),
    },
    version: match data.map_get("version")? {
      Edn::Nil => String::from("").into_boxed_str(),
      x => match x.read_str() {
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

fn load_modules(data: Edn) -> Result<Vec<Box<str>>, String> {
  match data.read_list() {
    Ok(xs) => {
      let mut ys: Vec<Box<str>> = Vec::with_capacity(xs.len());
      for x in xs {
        ys.push(x.read_str()?)
      }
      Ok(ys)
    }
    Err(e) => Err(format!("failed to load modules, {}", e)),
  }
}

fn load_file_info(data: Edn) -> Result<FileInSnapShot, String> {
  let ns_code = data.map_get("ns")?.read_quoted_cirru()?;
  let defs = data.map_get("defs")?.read_map().map_err(|e| format!("failed get `defs`:{}", e))?;
  let mut defs_info: HashMap<Box<str>, Cirru> = HashMap::with_capacity(defs.len());
  for (k, v) in defs {
    let var = k.read_str()?;
    let def_code = v.read_quoted_cirru()?;
    defs_info.insert(var, def_code);
  }
  let file = FileInSnapShot {
    ns: ns_code,
    defs: defs_info,
  };
  Ok(file)
}

fn load_files(data: Edn) -> Result<HashMap<Box<str>, FileInSnapShot>, String> {
  let xs = data.read_map().map_err(|e| format!("failed loading files, {}", e))?;
  let mut ys: HashMap<Box<str>, FileInSnapShot> = HashMap::with_capacity(xs.len());
  for (k, v) in xs {
    let key = k.read_str()?;
    let file = load_file_info(v)?;
    ys.insert(key, file);
  }
  Ok(ys)
}

/// parse snapshot
pub fn load_snapshot_data(data: Edn, path: &str) -> Result<Snapshot, String> {
  let pkg = data.map_get("package")?.read_str()?;
  let mut files = load_files(data.map_get("files")?)?;
  let meta_ns = format!("{}.$meta", pkg);
  files.insert(meta_ns.to_owned().into_boxed_str(), gen_meta_ns(&meta_ns, path));
  let s = Snapshot {
    package: pkg,
    configs: load_configs(data.map_get("configs")?)?,
    files,
  };
  Ok(s)
}

pub fn gen_meta_ns(ns: &str, path: &str) -> FileInSnapShot {
  let mut def_dict: HashMap<Box<str>, Cirru> = HashMap::with_capacity(2);
  def_dict.insert(
    String::from("calcit-filename").into_boxed_str(),
    Cirru::List(vec![
      Cirru::leaf("def"),
      Cirru::leaf("calcit-filename"),
      Cirru::leaf(format!("|{}", path.escape_default())),
    ]),
  );
  let path_data = Path::new(path);
  let parent = path_data.parent().unwrap();
  let parent_str = parent.to_str().unwrap();

  def_dict.insert(
    String::from("calcit-dirname").into_boxed_str(),
    Cirru::List(vec![
      Cirru::leaf("def"),
      Cirru::leaf("calcit-dirname"),
      Cirru::leaf(format!("|{}", parent_str.escape_default())),
    ]),
  );

  FileInSnapShot {
    ns: Cirru::List(vec![Cirru::leaf("ns"), Cirru::Leaf(ns.to_owned().into_boxed_str())]),
    defs: def_dict,
  }
}

pub fn gen_default() -> Snapshot {
  Snapshot {
    package: String::from("app").into_boxed_str(),
    configs: SnapshotConfigs {
      init_fn: String::from("app.main/main!").into_boxed_str(),
      reload_fn: String::from("app.main/reload!").into_boxed_str(),
      version: String::from("0.0.0").into_boxed_str(),
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
      let mut def_dict: HashMap<Box<str>, Cirru> = HashMap::with_capacity(2);
      def_dict.insert(
        String::from("main!").into_boxed_str(),
        Cirru::List(vec![Cirru::leaf("defn"), Cirru::leaf("main!"), Cirru::List(vec![]), code]),
      );
      def_dict.insert(
        String::from("reload!").into_boxed_str(),
        Cirru::List(vec![Cirru::leaf("defn"), Cirru::leaf("reload!"), Cirru::List(vec![])]),
      );
      Ok(FileInSnapShot {
        ns: Cirru::List(vec![Cirru::leaf("ns"), Cirru::leaf("app.main")]),
        defs: def_dict,
      })
    }
    Err(e) => Err(format!("failed to make snapshot: {}", e)),
  }
}

#[derive(Debug, PartialEq, Clone)]
pub struct FileChangeInfo {
  pub ns: Option<Cirru>,
  pub added_defs: HashMap<Box<str>, Cirru>,
  pub removed_defs: HashSet<Box<str>>,
  pub changed_defs: HashMap<Box<str>, Cirru>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct ChangesDict {
  pub added: HashMap<Box<str>, FileInSnapShot>,
  pub removed: HashSet<Box<str>>,
  pub changed: HashMap<Box<str>, FileChangeInfo>,
}

pub fn load_changes_info(data: Edn) -> Result<ChangesDict, String> {
  // println!("loading changes: {}", data);
  let mut added: HashMap<Box<str>, FileInSnapShot> = HashMap::new();
  for (ns, file) in &data.map_get("added")?.read_map_or_nil()? {
    added.insert(ns.read_str()?, load_file_info(file.to_owned())?);
  }

  let mut removed: HashSet<Box<str>> = HashSet::new();
  for item in &data.map_get("removed")?.read_set_or_nil()? {
    removed.insert(item.read_str()?);
  }

  let mut changed: HashMap<Box<str>, FileChangeInfo> = HashMap::new();
  for (ns, file) in &data.map_get("changed")?.read_map_or_nil()? {
    changed.insert(ns.read_str()?, extract_changed_info(file.to_owned())?);
  }

  Ok(ChangesDict { added, removed, changed })
}

pub fn extract_changed_info(data: Edn) -> Result<FileChangeInfo, String> {
  let ns_info = match data.map_get("ns")? {
    Edn::Nil => None,
    Edn::Quote(code) => Some(code),
    a => return Err(format!("invalid information for ns code: {}", a)),
  };

  let mut added_defs: HashMap<Box<str>, Cirru> = HashMap::new();

  for (def, code) in data.map_get("added-defs")?.read_map_or_nil()? {
    added_defs.insert(def.read_str()?, code.read_quoted_cirru()?);
  }

  let mut removed_defs: HashSet<Box<str>> = HashSet::new();

  for def in data.map_get("removed-defs")?.read_set_or_nil()? {
    removed_defs.insert(def.read_str()?);
  }

  let mut changed_defs: HashMap<Box<str>, Cirru> = HashMap::new();
  for (def, code) in data.map_get("changed-defs")?.read_map_or_nil()? {
    changed_defs.insert(def.read_str()?, code.read_quoted_cirru()?);
  }

  Ok(FileChangeInfo {
    ns: ns_info,
    added_defs,
    removed_defs,
    changed_defs,
  })
}
