use bisection_key::LexiconKey;
use cirru_edn::Edn;
use cirru_parser::Cirru;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::snapshot::{CodeEntry, FileInSnapShot, gen_meta_ns};

/// Detailed Cirru structure with metadata for tracking changes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DetailCirru {
  List {
    data: HashMap<String, DetailCirru>, // key parsing use bisection_key::LexiconKey
    at: u64,
    by: String,
  },
  Leaf {
    at: u64,
    by: String,
    text: Option<String>,
  },
}

impl From<Cirru> for DetailCirru {
  fn from(cirru: Cirru) -> Self {
    // milliseconds
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;

    match cirru {
      Cirru::List(xs) => {
        let mut data = HashMap::new();
        let mut current_key = LexiconKey::default();

        for (idx, x) in xs.into_iter().enumerate() {
          let key_str = if idx == 0 {
            current_key = LexiconKey::default();
            current_key.to_string()
          } else {
            current_key = current_key.bisect_end().unwrap_or_else(|_| LexiconKey::default());
            current_key.to_string()
          };
          data.insert(key_str, x.into());
        }

        DetailCirru::List {
          data,
          at: now,
          by: String::from("sync"),
        }
      }
      Cirru::Leaf(s) => DetailCirru::Leaf {
        at: now,
        by: String::from("sync"),
        text: Some(s.to_string()),
      },
    }
  }
}

impl From<DetailCirru> for Cirru {
  fn from(detail: DetailCirru) -> Self {
    match detail {
      DetailCirru::List { data, .. } => {
        // Sort by BalancedKey order to maintain proper sequence
        let mut sorted_items: Vec<_> = data.into_iter().collect();
        sorted_items.sort_by(|a, b| {
          let key_a = LexiconKey::new(&a.0).unwrap_or_else(|_| LexiconKey::default());
          let key_b = LexiconKey::new(&b.0).unwrap_or_else(|_| LexiconKey::default());
          key_a.cmp(&key_b)
        });

        let items: Vec<Cirru> = sorted_items.into_iter().map(|(_, v)| v.into()).collect();
        Cirru::List(items)
      }
      DetailCirru::Leaf { text, .. } => Cirru::Leaf(text.unwrap_or_default().into()),
    }
  }
}

impl TryFrom<Edn> for DetailCirru {
  type Error = String;
  fn try_from(data: Edn) -> Result<Self, String> {
    match data {
      Edn::Record(record) => {
        let mut at = 0u64;
        let mut by = String::new();
        let mut text = None;
        let mut data_map = HashMap::new();

        for (key, value) in record.pairs.iter() {
          match key.arc_str().as_ref() {
            "at" => {
              if let Edn::Number(timestamp) = value {
                at = *timestamp as u64;
              }
            }
            "by" => {
              if let Edn::Str(author) = value {
                by = author.to_string();
              }
            }
            "text" => {
              if let Edn::Str(content) = value {
                text = Some(content.to_string());
              }
            }
            "data" => {
              if let Edn::Map(data_edn) = value {
                for (k, v) in data_edn.0.iter() {
                  if let (Edn::Str(key_str), Ok(detail_cirru)) = (k, v.to_owned().try_into()) {
                    data_map.insert(key_str.to_string(), detail_cirru);
                  }
                }
              }
            }
            _ => {}
          }
        }

        if text.is_some() {
          Ok(DetailCirru::Leaf { at, by, text })
        } else {
          Ok(DetailCirru::List { data: data_map, at, by })
        }
      }
      _ => Err("Expected record for DetailCirru".to_string()),
    }
  }
}

/// Detailed code entry with metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailedCodeEntry {
  pub doc: String,
  pub code: DetailCirru,
}

impl From<CodeEntry> for DetailedCodeEntry {
  fn from(entry: CodeEntry) -> Self {
    DetailedCodeEntry {
      doc: entry.doc,
      code: entry.code.into(),
    }
  }
}

impl From<DetailedCodeEntry> for CodeEntry {
  fn from(detailed: DetailedCodeEntry) -> Self {
    CodeEntry {
      doc: detailed.doc,
      code: detailed.code.into(),
    }
  }
}

impl TryFrom<Edn> for DetailedCodeEntry {
  type Error = String;
  fn try_from(data: Edn) -> Result<Self, String> {
    match data {
      Edn::Record(record) => {
        let mut doc = String::new();
        let mut code = None;

        for (key, value) in record.pairs.iter() {
          match key.arc_str().as_ref() {
            "doc" => {
              if let Edn::Str(doc_str) = value {
                doc = doc_str.to_string();
              }
            }
            "code" => {
              code = Some(value.to_owned().try_into()?);
            }
            _ => {}
          }
        }

        let code = code.ok_or("Missing code field")?;
        Ok(DetailedCodeEntry { doc, code })
      }
      _ => Err("Expected record for DetailedCodeEntry".to_string()),
    }
  }
}

/// Detailed file in snapshot with metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailedFileInSnapshot {
  pub ns: DetailedCodeEntry,
  pub defs: HashMap<String, DetailedCodeEntry>,
}

impl From<FileInSnapShot> for DetailedFileInSnapshot {
  fn from(file: FileInSnapShot) -> Self {
    let defs = file.defs.into_iter().map(|(k, v)| (k, v.into())).collect();
    DetailedFileInSnapshot { ns: file.ns.into(), defs }
  }
}

impl From<DetailedFileInSnapshot> for FileInSnapShot {
  fn from(detailed: DetailedFileInSnapshot) -> Self {
    FileInSnapShot {
      ns: detailed.ns.into(),
      defs: detailed.defs.into_iter().map(|(k, v)| (k, v.into())).collect(),
    }
  }
}

impl TryFrom<Edn> for DetailedFileInSnapshot {
  type Error = String;
  fn try_from(data: Edn) -> Result<Self, String> {
    match data {
      Edn::Record(record) => {
        let mut ns = None;
        let mut defs = HashMap::new();

        for (key, value) in record.pairs.iter() {
          match key.arc_str().as_ref() {
            "ns" => {
              ns = Some(value.to_owned().try_into()?);
            }
            "defs" => {
              if let Edn::Map(defs_map) = value {
                for (k, v) in defs_map.0.iter() {
                  if let (Edn::Str(key_str), Ok(def_entry)) = (k, v.to_owned().try_into()) {
                    defs.insert(key_str.to_string(), def_entry);
                  }
                }
              }
            }
            _ => {}
          }
        }

        let ns = ns.ok_or("Missing ns field")?;
        Ok(DetailedFileInSnapshot { ns, defs })
      }
      _ => Err("Expected record for DetailedFileInSnapshot".to_string()),
    }
  }
}

/// Detailed snapshot structure for calcit.cirru format with additional metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailedSnapshot {
  pub package: String,
  pub configs: Edn,
  pub entries: Edn,
  pub files: HashMap<String, DetailedFileInSnapshot>,
  /// Additional metadata for detailed snapshot
  pub users: Edn,
}

impl TryFrom<Edn> for DetailedSnapshot {
  type Error = String;
  fn try_from(data: Edn) -> Result<Self, String> {
    match data {
      Edn::Record(record) => {
        let mut package = String::new();
        let mut configs: Option<Edn> = None;
        let mut entries: Edn = Edn::Nil;
        let mut files = HashMap::new();
        let mut users: Edn = Edn::Nil;

        for (key, value) in record.pairs.iter() {
          match key.arc_str().as_ref() {
            "package" => {
              if let Edn::Str(pkg_str) = value {
                package = pkg_str.to_string();
              }
            }
            "configs" => {
              configs = Some(value.to_owned());
            }
            "entries" => {
              entries = value.to_owned();
            }
            "files" => {
              if let Edn::Map(files_map) = value {
                for (k, v) in files_map.0.iter() {
                  if let (Edn::Str(key_str), Ok(file)) = (k, v.to_owned().try_into()) {
                    files.insert(key_str.to_string(), file);
                  }
                }
              }
            }
            "users" => {
              users = value.to_owned();
            }
            _ => {}
          }
        }

        let configs = configs.ok_or("Missing configs field")?;
        Ok(DetailedSnapshot {
          package,
          configs,
          entries,
          files,
          users,
        })
      }
      _ => {
        let data = data.view_map()?;

        let files = data
          .get_or_nil("files")
          .view_map()
          .map(|map| {
            let mut result = HashMap::new();
            for (k, v) in map.0.iter() {
              if let (Edn::Str(key), Ok(file)) = (k, v.to_owned().try_into()) {
                result.insert(key.to_string(), file);
              }
            }
            result
          })
          .unwrap_or_default();

        Ok(DetailedSnapshot {
          package: data.get_or_nil("package").try_into()?,
          configs: data.get_or_nil("configs"),
          entries: data.get_or_nil("entries"),
          files,
          users: data.get_or_nil("users"),
        })
      }
    }
  }
}

/// Load detailed snapshot data from EDN
pub fn load_detailed_snapshot_data(data: &Edn, path: &str) -> Result<DetailedSnapshot, String> {
  let data = data.view_map()?;
  let pkg: Arc<str> = data.get_or_nil("package").try_into()?;

  let files_edn = data.get_or_nil("files");
  let mut files: HashMap<String, DetailedFileInSnapshot> = files_edn
    .view_map()
    .map(|map| {
      let mut result = HashMap::new();
      for (k, v) in map.0.iter() {
        if let (Edn::Str(key), Ok(file)) = (k, v.to_owned().try_into()) {
          result.insert(key.to_string(), file);
        }
      }
      result
    })
    .unwrap_or_default();

  let meta_ns = format!("{pkg}.$meta");
  files.insert(meta_ns.to_owned(), gen_meta_ns(&meta_ns, path).into());

  let s = DetailedSnapshot {
    package: pkg.to_string(),
    configs: data.get_or_nil("configs"),
    entries: data.get_or_nil("entries"),
    files,
    users: data.get_or_nil("users"),
  };
  Ok(s)
}
