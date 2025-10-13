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
    match data {
      Edn::Map(_) => from_edn(data).map_err(|e| format!("failed to parse FileInSnapShot: {e}")),
      Edn::Record(record) => {
        let mut ns = None;
        let mut defs = None;

        for (key, value) in record.pairs.iter() {
          match key.arc_str().as_ref() {
            "ns" => {
              ns = Some(value.to_owned().try_into().map_err(|e| format!("failed to parse ns: {e}"))?);
            }
            "defs" => {
              defs = Some(value.to_owned().try_into().map_err(|e| format!("failed to parse defs: {e}"))?);
            }
            _ => {}
          }
        }

        let ns = ns.ok_or("Missing ns field in FileEntry")?;
        let defs = defs.ok_or("Missing defs field in FileEntry")?;
        Ok(FileInSnapShot { ns, defs })
      }
      _ => Err(format!("Expected FileInSnapShot map or record, but got: {data:?}")),
    }
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
  #[serde(default)]
  pub examples: Vec<Cirru>,
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
      &[
        ("doc".into(), data.doc.into()),
        ("examples".into(), data.examples.into()),
        ("code".into(), data.code.into()),
      ],
    )
  }
}

impl From<&CodeEntry> for Edn {
  fn from(data: &CodeEntry) -> Self {
    Edn::record_from_pairs(
      "CodeEntry".into(),
      &[
        ("doc".into(), data.doc.to_owned().into()),
        ("examples".into(), data.examples.to_owned().into()),
        ("code".into(), data.code.to_owned().into()),
      ],
    )
  }
}

impl CodeEntry {
  pub fn from_code(code: Cirru) -> Self {
    CodeEntry {
      doc: "".to_owned(),
      examples: vec![],
      code,
    }
  }
}

/// structure of `compact.cirru` file
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
  pub package: String,
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
  let s = Snapshot {
    package: pkg.to_string(),
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
      examples: vec![],
      code: vec!["ns", ns].into(),
    },
    defs: def_dict,
  }
}

impl Default for Snapshot {
  fn default() -> Snapshot {
    Snapshot {
      package: "app".into(),
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

/// TODO: Support for :doc and :examples fields has been added, needs to be handled properly
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

#[cfg(test)]
mod tests {
  use super::*;

  use std::fs;

  #[test]
  fn test_examples_field_parsing() {
    // 读取实际的 calcit-core.cirru 文件
    let core_file_content = fs::read_to_string("src/cirru/calcit-core.cirru").expect("Failed to read calcit-core.cirru");

    // 直接解析为 EDN
    let edn_data = cirru_edn::parse(&core_file_content).expect("Failed to parse cirru content as EDN");

    // 解析为 Snapshot
    let snapshot: Snapshot = load_snapshot_data(&edn_data, "calcit-core.cirru").expect("Failed to parse snapshot");

    // 验证文件存在
    assert!(snapshot.files.contains_key("calcit.core"));

    let core_file = &snapshot.files["calcit.core"];

    // 验证我们添加了 examples 的函数
    let functions_with_examples = vec![
      ("+", 2),
      ("-", 2),
      ("*", 3),
      ("/", 2),
      ("map", 2),
      ("filter", 2),
      ("first", 2),
      ("count", 2),
      ("concat", 1),
      ("inc", 2),
      ("reduce", 1), // 原本就有的，只有1个example
    ];

    println!("Verifying examples in calcit-core.cirru:");
    for (func_name, expected_count) in functions_with_examples {
      if let Some(func_def) = core_file.defs.get(func_name) {
        println!("  {}: {} examples", func_name, func_def.examples.len());
        assert_eq!(
          func_def.examples.len(),
          expected_count,
          "Function '{func_name}' should have {expected_count} examples"
        );
      } else {
        panic!("Function '{func_name}' not found in calcit.core");
      }
    }
  }

  #[test]
  fn test_code_entry_with_examples() {
    // 创建一个带有 examples 的 CodeEntry
    let examples = vec![
      Cirru::List(vec![Cirru::leaf("add"), Cirru::leaf("1"), Cirru::leaf("2")]),
      Cirru::List(vec![Cirru::leaf("add"), Cirru::leaf("10"), Cirru::leaf("20")]),
    ];

    let code_entry = CodeEntry {
      doc: "Test function".to_string(),
      code: Cirru::List(vec![
        Cirru::leaf("defn"),
        Cirru::leaf("add"),
        Cirru::List(vec![Cirru::leaf("a"), Cirru::leaf("b")]),
        Cirru::List(vec![Cirru::leaf("+"), Cirru::leaf("a"), Cirru::leaf("b")]),
      ]),
      examples,
    };

    // 验证 examples 字段
    assert_eq!(code_entry.examples.len(), 2);

    // 验证第一个 example
    if let Cirru::List(list) = &code_entry.examples[0] {
      assert_eq!(list.len(), 3);
      if let Cirru::Leaf(s) = &list[0] {
        assert_eq!(&**s, "add");
      }
    }

    // 转换为 EDN 再转换回来，验证序列化/反序列化
    let edn: Edn = code_entry.clone().into();
    let parsed_entry: CodeEntry = edn.try_into().expect("Failed to parse CodeEntry from EDN");

    assert_eq!(parsed_entry.examples.len(), 2);

    // 验证解析后的第一个 example
    if let Cirru::List(list) = &parsed_entry.examples[0] {
      assert_eq!(list.len(), 3);
      if let Cirru::Leaf(s) = &list[0] {
        assert_eq!(&**s, "add");
      }
    }

    println!("✅ CodeEntry with examples test passed!");
  }
}
