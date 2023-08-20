use cirru_edn::Edn;
use cirru_parser::Cirru;
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotConfigs {
  pub init_fn: Arc<str>,
  pub reload_fn: Arc<str>,
  pub modules: Vec<Arc<str>>,
  pub version: Arc<str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInSnapShot {
  pub ns: CodeEntry,
  pub defs: HashMap<Arc<str>, CodeEntry>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeEntry {
  pub doc: String,
  pub code: Cirru,
}

impl TryFrom<Edn> for CodeEntry {
  type Error = String;
  fn try_from(data: Edn) -> Result<Self, String> {
    Ok(CodeEntry {
      doc: data.record_get("doc")?.try_into()?,
      code: data.record_get("code")?.try_into()?,
    })
  }
}

impl From<CodeEntry> for Edn {
  fn from(data: CodeEntry) -> Self {
    Edn::record_from_pairs(
      "CodeEntry".into(),
      &[("doc".into(), data.doc.into()), ("code".into(), data.code.into())],
    )
  }
}

impl CodeEntry {
  pub fn from_code(code: Cirru) -> Self {
    CodeEntry { doc: "".to_owned(), code }
  }
}

/// structure of `compact.cirru` file
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
  pub package: Arc<str>,
  pub configs: SnapshotConfigs,
  pub entries: HashMap<Arc<str>, SnapshotConfigs>,
  pub files: HashMap<Arc<str>, FileInSnapShot>,
}

impl TryFrom<Edn> for SnapshotConfigs {
  type Error = String;
  fn try_from(data: Edn) -> Result<SnapshotConfigs, String> {
    let c = SnapshotConfigs {
      init_fn: data.map_get("init-fn")?.try_into()?,
      reload_fn: data.map_get("reload-fn")?.try_into()?,
      version: match data.map_get("version")? {
        Edn::Nil => "".into(),
        x => x.try_into()?,
      },
      modules: match data.map_get("modules")? {
        Edn::Nil => vec![],
        v => v.try_into()?,
      },
    };
    Ok(c)
  }
}

/// parse snapshot
pub fn load_snapshot_data(data: &Edn, path: &str) -> Result<Snapshot, String> {
  let pkg: Arc<str> = data.map_get("package")?.try_into()?;
  let mut files: HashMap<Arc<str>, FileInSnapShot> = data.map_get("files")?.try_into()?;
  let meta_ns = format!("{pkg}.$meta");
  files.insert(meta_ns.to_owned().into(), gen_meta_ns(&meta_ns, path));
  let s = Snapshot {
    package: pkg,
    configs: data.map_get("configs")?.try_into()?,
    entries: data.map_get("entries")?.try_into()?,
    files,
  };
  Ok(s)
}

pub fn gen_meta_ns(ns: &str, path: &str) -> FileInSnapShot {
  let path_data = Path::new(path);
  let parent = path_data.parent().expect("parent path");
  let parent_str = parent.to_str().expect("get path string");

  let def_dict: HashMap<Arc<str>, CodeEntry> = HashMap::from_iter([
    (
      "calcit-filename".into(),
      CodeEntry::from_code(vec!["def", "calcit-filename", &format!("|{}", path.escape_default())].into()),
    ),
    (
      "calcit-dirname".into(),
      CodeEntry::from_code(vec!["def", "calcit-dirname", &format!("|{}", parent_str.escape_default())].into()),
    ),
  ]);

  FileInSnapShot {
    ns: CodeEntry {
      doc: "".to_owned(),
      code: vec!["ns", ns].into(),
    },
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
      let mut def_dict: HashMap<Arc<str>, CodeEntry> = HashMap::with_capacity(2);
      let mut func_code = vec![Cirru::leaf("defn"), "main!".into(), Cirru::List(vec![])];
      for line in lines {
        func_code.push(line.to_owned());
      }
      def_dict.insert("main!".into(), CodeEntry::from_code(Cirru::List(func_code)));
      def_dict.insert(
        "reload!".into(),
        CodeEntry::from_code(vec![Cirru::leaf("defn"), "reload!".into(), Cirru::List(vec![])].into()),
      );
      Ok(FileInSnapShot {
        ns: CodeEntry::from_code(vec!["ns", "app.main"].into()),
        defs: def_dict,
      })
    }
    Err(e) => Err(format!("failed to make snapshot: {e}")),
  }
}

#[derive(Debug, PartialEq, Clone, Eq)]
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
      map.insert(Edn::tag("ns"), Edn::Quote(ns.to_owned()));
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
    // call previous implementation to convert
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

#[derive(Debug, PartialEq, Clone, Eq, Default)]
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

impl TryFrom<Edn> for ChangesDict {
  type Error = String;

  fn try_from(data: Edn) -> Result<Self, Self::Error> {
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
