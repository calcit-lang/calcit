use cirru_edn::{Edn, EdnMapView, EdnTag};
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
    Edn::Record(
      EdnTag::new("FileEntry"),
      vec![
        ("ns".into(), data.ns.to_owned().into()),
        ("defs".into(), data.defs.to_owned().into()),
      ],
    )
  }
}

impl TryFrom<Edn> for FileInSnapShot {
  type Error = String;
  fn try_from(data: Edn) -> Result<Self, String> {
    let data = data.view_record()?;
    Ok(FileInSnapShot {
      ns: data["ns"].to_owned().try_into()?,
      defs: data["defs"].to_owned().try_into()?,
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
    let data = data.view_record()?;
    Ok(CodeEntry {
      doc: data["doc"].to_owned().try_into()?,
      code: data["code"].to_owned().try_into()?,
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
    let data = data.view_map()?;
    let c = SnapshotConfigs {
      init_fn: data.get_or_nil("init-fn").try_into()?,
      reload_fn: data.get_or_nil("reload-fn").try_into()?,
      version: match data.get_or_nil("version") {
        Edn::Nil => "".into(),
        x => x.try_into()?,
      },
      modules: match data.get_or_nil("modules") {
        Edn::Nil => vec![],
        v => v.try_into()?,
      },
    };
    Ok(c)
  }
}

/// parse snapshot
pub fn load_snapshot_data(data: &Edn, path: &str) -> Result<Snapshot, String> {
  let data = data.view_map()?;
  let pkg: Arc<str> = data.get_or_nil("package").try_into()?;
  let mut files: HashMap<Arc<str>, FileInSnapShot> = data.get_or_nil("files").try_into()?;
  let meta_ns = format!("{pkg}.$meta");
  files.insert(meta_ns.to_owned().into(), gen_meta_ns(&meta_ns, path));
  let s = Snapshot {
    package: pkg,
    configs: data.get_or_nil("configs").try_into()?,
    entries: data.get_or_nil("entries").try_into()?,
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

impl Default for Snapshot {
  fn default() -> Snapshot {
    Snapshot {
      package: "".into(),
      configs: SnapshotConfigs {
        init_fn: "".into(),
        reload_fn: "".into(),
        version: "".into(),
        modules: vec![],
      },
      entries: HashMap::new(),
      files: HashMap::new(),
    }
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
    let mut map = EdnMapView::default();
    if let Some(ns) = &data.ns {
      map.insert_key("ns", Edn::Quote(ns.to_owned()));
    }

    if !data.added_defs.is_empty() {
      let defs: HashMap<Edn, Edn> = data
        .added_defs
        .iter()
        .map(|(name, def)| (Edn::str(&**name), Edn::Quote(def.to_owned())))
        .collect();
      map.insert_key("added-defs", Edn::Map(defs));
    }
    if !data.removed_defs.is_empty() {
      map.insert_key("removed-defs", Edn::Set(data.removed_defs.iter().map(|s| Edn::str(&**s)).collect()));
    }
    if !data.changed_defs.is_empty() {
      map.insert_key(
        "changed-defs",
        Edn::Map(
          data
            .changed_defs
            .iter()
            .map(|(name, def)| (Edn::str(&**name), Edn::Quote(def.to_owned())))
            .collect(),
        ),
      );
    }
    map.into()
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
    let data = data.view_map()?;
    Ok(Self {
      ns: match data.get_or_nil("ns") {
        Edn::Nil => None,
        ns => Some(ns.try_into()?),
      },
      added_defs: data.get_or_nil("added-defs").try_into()?,
      removed_defs: data.get_or_nil("removed-defs").try_into()?,
      changed_defs: data.get_or_nil("changed-defs").try_into()?,
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
    let data = data.view_map()?;
    Ok(Self {
      added: data.get_or_nil("added").try_into()?,
      changed: data.get_or_nil("changed").try_into()?,
      removed: data.get_or_nil("removed").try_into()?,
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
