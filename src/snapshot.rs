use cirru_edn::Edn;
use cirru_parser::Cirru;
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotConfigs {
  pub init_fn: Arc<str>,
  pub reload_fn: Arc<str>,
  pub modules: Vec<Arc<str>>,
  pub version: Arc<str>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileInSnapShot {
  pub ns: Cirru,
  pub defs: HashMap<Arc<str>, Cirru>,
}

impl From<&FileInSnapShot> for Edn {
  fn from(data: &FileInSnapShot) -> Edn {
    Edn::map_from_iter([
      ("ns".into(), data.ns.to_owned().into()),
      ("defs".into(), data.defs.to_owned().into()),
    ])
  }
}

impl TryFrom<Edn> for FileInSnapShot {
  type Error = String;
  fn try_from(data: Edn) -> Result<Self, String> {
    Ok(FileInSnapShot {
      ns: data.map_get_some("ns")?.try_into()?,
      defs: data.map_get_some("defs")?.try_into()?,
    })
  }
}

impl From<FileInSnapShot> for Edn {
  fn from(data: FileInSnapShot) -> Edn {
    Edn::map_from_iter([("ns".into(), data.ns.into()), ("defs".into(), data.defs.into())])
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
  pub package: Arc<str>,
  pub configs: SnapshotConfigs,
  pub entries: HashMap<Arc<str>, SnapshotConfigs>,
  pub files: HashMap<Arc<str>, FileInSnapShot>,
}

fn load_configs(data: &Edn) -> Result<SnapshotConfigs, String> {
  let c = SnapshotConfigs {
    init_fn: match data.map_get("init-fn")?.read_str() {
      Ok(v) => (*v).into(),
      Err(e) => return Err(format!("failed to load init-fn from: {}", e)),
    },
    reload_fn: match data.map_get("reload-fn")?.read_str() {
      Ok(v) => (*v).into(),
      Err(e) => return Err(format!("failed to load reload-fn from: {}", e)),
    },
    version: match data.map_get("version")? {
      Edn::Nil => "".into(),
      x => match x.read_str() {
        Ok(v) => (*v).into(),
        Err(e) => return Err(format!("failed to load version, {}", e)),
      },
    },
    modules: match data.map_get("modules")? {
      Edn::Nil => vec![],
      x => load_modules(&x)?,
    },
  };
  Ok(c)
}

fn load_modules(data: &Edn) -> Result<Vec<Arc<str>>, String> {
  data.to_owned().try_into()
}

pub fn load_file_info(data: &Edn) -> Result<FileInSnapShot, String> {
  let ns_code = data.map_get("ns")?.read_quoted_cirru()?;
  let defs = data.map_get("defs")?.read_map().map_err(|e| format!("failed get `defs`:{}", e))?;
  let mut defs_info: HashMap<Arc<str>, Cirru> = HashMap::with_capacity(defs.len());
  for (k, v) in defs {
    let var = k.read_str()?;
    let def_code = v.read_quoted_cirru()?;
    defs_info.insert((*var).into(), def_code);
  }
  let file = FileInSnapShot {
    ns: ns_code,
    defs: defs_info,
  };
  Ok(file)
}

pub fn load_files(data: &Edn) -> Result<HashMap<Arc<str>, FileInSnapShot>, String> {
  data.to_owned().try_into()
}

fn load_entries(data: &Edn) -> Result<HashMap<Arc<str>, SnapshotConfigs>, String> {
  let xs = data.read_map_or_nil().map_err(|e| format!("failed loading entries, {}", e))?;
  let mut ys: HashMap<Arc<str>, SnapshotConfigs> = HashMap::with_capacity(xs.len());
  for (k, v) in xs {
    let key: Box<str> = match k {
      Edn::Keyword(s) => s.to_str(),
      Edn::Str(s) => s,
      _ => return Err(format!("unknown data for an entry: {}", k)),
    };
    let configs = load_configs(&v)?;
    ys.insert((*key).into(), configs);
  }
  Ok(ys)
}

/// parse snapshot
pub fn load_snapshot_data(data: &Edn, path: &str) -> Result<Snapshot, String> {
  let pkg = data.map_get("package")?.read_str()?;
  let mut files = load_files(&data.map_get("files")?)?;
  let meta_ns = format!("{}.$meta", pkg);
  files.insert(meta_ns.to_owned().into(), gen_meta_ns(&meta_ns, path));
  let s = Snapshot {
    package: (*pkg).into(),
    configs: load_configs(&data.map_get("configs")?)?,
    entries: load_entries(&data.map_get("entries")?)?,
    files,
  };
  Ok(s)
}

pub fn gen_meta_ns(ns: &str, path: &str) -> FileInSnapShot {
  let mut def_dict: HashMap<Arc<str>, Cirru> = HashMap::with_capacity(2);
  def_dict.insert(
    "calcit-filename".into(),
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
    "calcit-dirname".into(),
    Cirru::List(vec![
      Cirru::leaf("def"),
      Cirru::leaf("calcit-dirname"),
      Cirru::leaf(format!("|{}", parent_str.escape_default())),
    ]),
  );

  FileInSnapShot {
    ns: Cirru::List(vec![Cirru::leaf("ns"), Cirru::Leaf(ns.to_owned().into())]),
    defs: def_dict,
  }
}

pub fn gen_default() -> Snapshot {
  Snapshot {
    package: "app".into(),
    configs: SnapshotConfigs {
      init_fn: "app.main/main!".into(),
      reload_fn: "app.main/reload!".into(),
      version: "0.0.0".into(),
      modules: vec![],
    },
    entries: HashMap::new(),
    files: HashMap::new(),
  }
}

pub fn create_file_from_snippet(raw: &str) -> Result<FileInSnapShot, String> {
  match cirru_parser::parse(raw) {
    Ok(lines) => {
      let mut def_dict: HashMap<Arc<str>, Cirru> = HashMap::with_capacity(2);
      let mut func_code = vec![Cirru::leaf("defn"), Cirru::leaf("main!"), Cirru::List(vec![])];
      for line in lines {
        func_code.push(line.to_owned());
      }
      def_dict.insert("main!".into(), Cirru::List(func_code));
      def_dict.insert(
        "reload!".into(),
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
  pub added_defs: HashMap<Arc<str>, Cirru>,
  pub removed_defs: HashSet<Arc<str>>,
  pub changed_defs: HashMap<Arc<str>, Cirru>,
}

impl From<&FileChangeInfo> for Edn {
  fn from(data: &FileChangeInfo) -> Edn {
    let mut map: HashMap<Edn, Edn> = HashMap::new();
    if let Some(ns) = &data.ns {
      map.insert(Edn::kwd("ns"), Edn::Quote(ns.to_owned()));
    }

    if !data.added_defs.is_empty() {
      let defs: HashMap<Edn, Edn> = data
        .added_defs
        .iter()
        .map(|(name, def)| (Edn::str(&**name), Edn::Quote(def.to_owned())))
        .collect();
      map.insert(Edn::str("added-defs"), Edn::Map(defs));
    }
    if !data.removed_defs.is_empty() {
      map.insert(
        Edn::str("removed-defs"),
        Edn::Set(data.removed_defs.iter().map(|s| Edn::str(&**s)).collect()),
      );
    }
    if !data.changed_defs.is_empty() {
      map.insert(
        Edn::str("changed-defs"),
        Edn::Map(
          data
            .changed_defs
            .iter()
            .map(|(name, def)| (Edn::str(&**name), Edn::Quote(def.to_owned())))
            .collect(),
        ),
      );
    }
    Edn::Map(map)
  }
}

impl From<FileChangeInfo> for Edn {
  fn from(data: FileChangeInfo) -> Edn {
    (&data).into()
  }
}

impl TryFrom<Edn> for FileChangeInfo {
  type Error = String;

  fn try_from(data: Edn) -> Result<Self, Self::Error> {
    Ok(Self {
      ns: match data.map_get("ns")? {
        Edn::Nil => None,
        ns => Some(ns.try_into()?),
      },
      added_defs: data.map_get("added-defs")?.try_into()?,
      removed_defs: data.map_get("removed-defs")?.try_into()?,
      changed_defs: data.map_get("changed-defs")?.try_into()?,
    })
  }
}

#[derive(Debug, PartialEq, Clone, Default)]
pub struct ChangesDict {
  pub added: HashMap<Arc<str>, FileInSnapShot>,
  pub removed: HashSet<Arc<str>>,
  pub changed: HashMap<Arc<str>, FileChangeInfo>,
}

impl ChangesDict {
  pub fn is_empty(&self) -> bool {
    self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
  }
}

pub fn extract_changed_info(data: &Edn) -> Result<FileChangeInfo, String> {
  let ns_info = match data.map_get("ns")? {
    Edn::Nil => None,
    Edn::Quote(code) => Some(code),
    a => return Err(format!("invalid information for ns code: {}", a)),
  };

  let mut added_defs: HashMap<Arc<str>, Cirru> = HashMap::new();

  for (def, code) in data.map_get("added-defs")?.read_map_or_nil()? {
    added_defs.insert((*def.read_str()?).into(), code.read_quoted_cirru()?);
  }

  let mut removed_defs: HashSet<Arc<str>> = HashSet::new();

  for def in data.map_get("removed-defs")?.read_set_or_nil()? {
    removed_defs.insert((*def.read_str()?).into());
  }

  let mut changed_defs: HashMap<Arc<str>, Cirru> = HashMap::new();
  for (def, code) in data.map_get("changed-defs")?.read_map_or_nil()? {
    changed_defs.insert((*(def.read_str()?)).into(), code.read_quoted_cirru()?);
  }

  Ok(FileChangeInfo {
    ns: ns_info,
    added_defs,
    removed_defs,
    changed_defs,
  })
}

impl TryFrom<&Edn> for ChangesDict {
  type Error = String;

  fn try_from(data: &Edn) -> Result<Self, Self::Error> {
    Ok(Self {
      added: data.map_get_some("added")?.try_into()?,
      changed: data.map_get_some("changed")?.try_into()?,
      removed: data.map_get_some("removed")?.try_into()?,
    })
  }
}

impl TryFrom<ChangesDict> for Edn {
  type Error = String;

  fn try_from(x: ChangesDict) -> Result<Edn, Self::Error> {
    Ok(Edn::map_from_iter([
      ("removed".into(), x.removed.into()),
      ("added".into(), x.added.into()),
      ("changed".into(), x.changed.into()),
    ]))
  }
}
