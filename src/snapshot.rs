use cirru_edn::{Edn, EdnMapView, EdnRecordView, EdnSetView, EdnTag, from_edn};
use cirru_parser::Cirru;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;
use std::path::Path;
use std::sync::Arc;

fn default_version() -> String {
  "0.0.0".to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotConfigs {
  #[serde(rename = "init-fn")]
  pub init_fn: String,
  #[serde(rename = "reload-fn")]
  pub reload_fn: String,
  #[serde(default)]
  pub modules: Vec<String>,
  #[serde(default = "default_version")]
  pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentEntry {
  pub title: String,
  pub content: String,
}

impl From<DocumentEntry> for Edn {
  fn from(data: DocumentEntry) -> Self {
    Edn::record_from_pairs(
      "DocumentEntry".into(),
      &[("title".into(), data.title.into()), ("content".into(), data.content.into())],
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::HashMap;

  #[test]
  fn test_document_entry_roundtrip() {
    let original = DocumentEntry {
      title: "Test Title".to_string(),
      content: "Test Content\nWith multiple lines\nAnd special chars: !@#$%".to_string(),
    };

    // Convert to EDN and back
    let edn: cirru_edn::Edn = original.clone().into();
    let converted: Result<DocumentEntry, String> = edn.try_into();

    assert!(converted.is_ok());
    let result = converted.unwrap();
    assert_eq!(result.title, original.title);
    assert_eq!(result.content, original.content);
  }

  #[test]
  fn test_snapshot_with_docs() {
    let mut docs = HashMap::new();
    docs.insert(
      "guide".to_string(),
      DocumentEntry {
        title: "User Guide".to_string(),
        content: "How to use this application".to_string(),
      },
    );

    let snapshot = Snapshot {
      package: "test-package".to_string(),
      configs: SnapshotConfigs {
        init_fn: "app.main/main!".to_string(),
        reload_fn: "app.main/reload!".to_string(),
        modules: vec![],
        version: "0.0.1".to_string(),
      },
      entries: Default::default(),
      files: Default::default(),
      docs: Some(docs),
    };

    // Verify that docs field is properly set
    assert!(snapshot.docs.is_some());
    let snapshot_docs = snapshot.docs.as_ref().unwrap();
    assert_eq!(snapshot_docs.len(), 1);
    assert!(snapshot_docs.contains_key("guide"));

    let guide_doc = &snapshot_docs["guide"];
    assert_eq!(guide_doc.title, "User Guide");
    assert_eq!(guide_doc.content, "How to use this application");
  }
}

impl From<&DocumentEntry> for Edn {
  fn from(data: &DocumentEntry) -> Self {
    Edn::record_from_pairs(
      "DocumentEntry".into(),
      &[
        ("title".into(), data.title.to_owned().into()),
        ("content".into(), data.content.to_owned().into()),
      ],
    )
  }
}

impl TryFrom<Edn> for DocumentEntry {
  type Error = String;
  fn try_from(data: Edn) -> Result<Self, String> {
    from_edn(data).map_err(|e| format!("failed to parse DocumentEntry: {e}"))
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileInSnapShot {
  pub ns: CodeEntry,
  pub defs: HashMap<String, CodeEntry>,
}

impl From<&FileInSnapShot> for Edn {
  fn from(data: &FileInSnapShot) -> Edn {
    Edn::Record(EdnRecordView {
      tag: EdnTag::new("FileEntry"),
      pairs: vec![("defs".into(), Edn::from(data.defs.to_owned())), ("ns".into(), Edn::from(&data.ns))], // TODO
    })
  }
}

impl TryFrom<Edn> for FileInSnapShot {
  type Error = String;
  fn try_from(data: Edn) -> Result<Self, String> {
    from_edn(data).map_err(|e| format!("failed to parse FileInSnapShot: {e}"))
  }
}

impl From<FileInSnapShot> for Edn {
  fn from(data: FileInSnapShot) -> Edn {
    Edn::map_from_iter([("defs".into(), data.defs.into()), ("ns".into(), data.ns.into())])
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeEntry {
  pub doc: String,
  pub code: Cirru,
}

impl TryFrom<Edn> for CodeEntry {
  type Error = String;
  fn try_from(data: Edn) -> Result<Self, String> {
    from_edn(data).map_err(|e| format!("failed to parse CodeEntry: {e}"))
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

impl From<&CodeEntry> for Edn {
  fn from(data: &CodeEntry) -> Self {
    Edn::record_from_pairs(
      "CodeEntry".into(),
      &[
        ("doc".into(), data.doc.to_owned().into()),
        ("code".into(), data.code.to_owned().into()),
      ],
    )
  }
}

impl CodeEntry {
  pub fn from_code(code: Cirru) -> Self {
    CodeEntry { doc: "".to_owned(), code }
  }
}

/// structure of `compact.cirru` file
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
  pub package: String,
  pub docs: Option<HashMap<String, DocumentEntry>>,
  pub configs: SnapshotConfigs,
  pub entries: HashMap<String, SnapshotConfigs>,
  pub files: HashMap<String, FileInSnapShot>,
}

impl TryFrom<Edn> for SnapshotConfigs {
  type Error = String;
  fn try_from(data: Edn) -> Result<SnapshotConfigs, String> {
    from_edn(data)
  }
}

/// parse snapshot
pub fn load_snapshot_data(data: &Edn, path: &str) -> Result<Snapshot, String> {
  let data = data.view_map()?;
  let pkg: Arc<str> = data.get_or_nil("package").try_into()?;
  let mut files: HashMap<String, FileInSnapShot> = data.get_or_nil("files").try_into()?;
  let meta_ns = format!("{pkg}.$meta");
  files.insert(meta_ns.to_owned(), gen_meta_ns(&meta_ns, path));
  let docs: Option<HashMap<String, DocumentEntry>> = match data.get_or_nil("docs") {
    Edn::Nil => None,
    docs_edn => Some(docs_edn.try_into()?),
  };
  let s = Snapshot {
    package: pkg.to_string(),
    docs,
    configs: from_edn(data.get_or_nil("configs"))?,
    entries: data.get_or_nil("entries").try_into()?,
    files,
  };
  Ok(s)
}

pub fn gen_meta_ns(ns: &str, path: &str) -> FileInSnapShot {
  let path_data = Path::new(path);
  let parent = path_data.parent().expect("parent path");
  let parent_str = parent.to_str().expect("get path string");

  let def_dict: HashMap<String, CodeEntry> = HashMap::from_iter([
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
      package: "app".into(),
      docs: None,
      configs: SnapshotConfigs {
        init_fn: "app.main/main!".into(),
        reload_fn: "app.main/reload!".into(),
        version: "0.0.0".to_string(),
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
      let mut def_dict: HashMap<String, CodeEntry> = HashMap::with_capacity(2);
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
  pub added_defs: HashMap<String, Cirru>,
  pub removed_defs: HashSet<String>,
  pub changed_defs: HashMap<String, Cirru>,
}

impl From<&FileChangeInfo> for Edn {
  fn from(data: &FileChangeInfo) -> Edn {
    let mut map = EdnMapView::default();
    if let Some(ns) = &data.ns {
      map.insert_key("ns", Edn::Quote(ns.to_owned()));
    }

    if !data.added_defs.is_empty() {
      #[allow(clippy::mutable_key_type)]
      let defs: HashMap<Edn, Edn> = data
        .added_defs
        .iter()
        .map(|(name, def)| (Edn::str(&**name), Edn::Quote(def.to_owned())))
        .collect();
      map.insert_key("added-defs", Edn::from(defs));
    }
    if !data.removed_defs.is_empty() {
      map.insert_key(
        "removed-defs",
        Edn::Set(EdnSetView(data.removed_defs.iter().map(|s| Edn::str(&**s)).collect())),
      );
    }
    if !data.changed_defs.is_empty() {
      map.insert_key(
        "changed-defs",
        Edn::Map(EdnMapView(
          data
            .changed_defs
            .iter()
            .map(|(name, def)| (Edn::str(&**name), Edn::Quote(def.to_owned())))
            .collect(),
        )),
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
