use bisection_key::LexiconKey;
use cirru_edn::{Edn, EdnTag};
use cirru_parser::Cirru;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::calcit::{CalcitTypeAnnotation, DYNAMIC_TYPE};
use crate::snapshot::{CodeEntry, FileInSnapShot, NsEntry, TestEntry, gen_meta_ns, validate_test_names};

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
      Edn::Struct(struct_value) => {
        let mut at = 0u64;
        let mut by = String::new();
        let mut text = None;
        let mut data_map = HashMap::new();

        for (key, value) in struct_value.pairs.iter() {
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
      _ => Err("Expected struct for DetailCirru".to_string()),
    }
  }
}

mod schema_serde {
  use super::*;

  pub fn default_schema() -> Arc<CalcitTypeAnnotation> {
    DYNAMIC_TYPE.clone()
  }

  pub fn serialize<S>(schema: &Arc<CalcitTypeAnnotation>, s: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    let edn: Option<Edn> = match schema.as_ref() {
      CalcitTypeAnnotation::Dynamic => None,
      CalcitTypeAnnotation::Fn(fn_annot) => Some(fn_annot.to_schema_edn()),
      CalcitTypeAnnotation::Macro(signature) => Some(signature.to_wrapped_schema_edn()),
      _ => None,
    };
    edn.serialize(s)
  }

  pub fn deserialize<'de, D>(d: D) -> Result<Arc<CalcitTypeAnnotation>, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    let opt = Option::<Edn>::deserialize(d)?;
    Ok(match opt {
      None | Some(Edn::Nil) => DYNAMIC_TYPE.clone(),
      Some(v) => CalcitTypeAnnotation::parse_macro_signature_from_edn(&v)
        .map(|s| Arc::new(CalcitTypeAnnotation::Macro(Arc::new(s))))
        .or_else(|| CalcitTypeAnnotation::parse_fn_schema_from_edn(&v).map(|s| Arc::new(CalcitTypeAnnotation::Fn(Arc::new(s)))))
        .unwrap_or_else(|| DYNAMIC_TYPE.clone()),
    })
  }
}

/// Detailed code entry with metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailedTestEntry {
  pub name: String,
  pub code: DetailCirru,
  #[serde(default)]
  pub tags: Vec<String>,
}

impl From<TestEntry> for DetailedTestEntry {
  fn from(test: TestEntry) -> Self {
    let mut tags = test.tags.iter().map(|tag| format!(":{}", tag.ref_str())).collect::<Vec<_>>();
    tags.sort();
    DetailedTestEntry {
      name: test.name,
      code: test.code.into(),
      tags,
    }
  }
}

impl From<DetailedTestEntry> for TestEntry {
  fn from(test: DetailedTestEntry) -> Self {
    TestEntry {
      name: test.name,
      code: test.code.into(),
      tags: test.tags.iter().map(|tag| EdnTag::new(tag.trim_start_matches(':'))).collect(),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailedCodeEntry {
  pub doc: String,
  #[serde(default)]
  pub examples: Vec<DetailCirru>,
  #[serde(default)]
  pub tests: Vec<DetailedTestEntry>,
  #[serde(default)]
  pub tags: Vec<String>,
  pub code: DetailCirru,
  #[serde(default = "schema_serde::default_schema", with = "schema_serde")]
  pub schema: Arc<CalcitTypeAnnotation>,
  #[serde(default)]
  pub ffi: Option<Edn>,
}

impl From<CodeEntry> for DetailedCodeEntry {
  fn from(entry: CodeEntry) -> Self {
    DetailedCodeEntry {
      doc: entry.doc,
      examples: entry.examples.into_iter().map(|e| e.into()).collect(),
      tests: entry.tests.into_iter().map(Into::into).collect(),
      tags: entry.tags.iter().map(|tag| format!(":{}", tag.ref_str())).collect(),
      code: entry.code.into(),
      schema: entry.schema,
      ffi: entry.ffi,
    }
  }
}

impl From<DetailedCodeEntry> for CodeEntry {
  fn from(detailed: DetailedCodeEntry) -> Self {
    CodeEntry {
      doc: detailed.doc,
      examples: detailed.examples.into_iter().map(|e| e.into()).collect(),
      tests: detailed.tests.into_iter().map(Into::into).collect(),
      tags: detailed.tags.iter().map(|tag| EdnTag::new(tag.trim_start_matches(':'))).collect(),
      code: detailed.code.into(),
      schema: detailed.schema,
      ffi: detailed.ffi,
    }
  }
}

impl TryFrom<Edn> for DetailedCodeEntry {
  type Error = String;
  fn try_from(data: Edn) -> Result<Self, String> {
    match data {
      Edn::Struct(struct_value) => {
        let mut doc = String::new();
        let mut examples = Vec::new();
        let mut tests = Vec::new();
        let mut tags = Vec::new();
        let mut code = None;
        let mut schema = None;
        let mut ffi = None;

        for (key, value) in struct_value.pairs.iter() {
          match key.arc_str().as_ref() {
            "doc" => {
              if let Edn::Str(doc_str) = value {
                doc = doc_str.to_string();
              }
            }
            "examples" => {
              if let Edn::List(list) = value {
                for item in list.iter() {
                  examples.push(item.to_owned().try_into()?);
                }
              }
            }
            "tests" => {
              if let Edn::List(list) = value {
                for item in list.iter() {
                  tests.push(parse_detailed_test_entry(item)?);
                }
              } else {
                return Err(format!("DetailedCodeEntry.tests expects a list, got: {value}"));
              }
            }
            "tags" => {
              if let Edn::Set(set) = value {
                for item in &set.0 {
                  if let Edn::Tag(tag) = item {
                    tags.push(format!(":{}", tag.ref_str()));
                  } else {
                    return Err(format!("DetailedCodeEntry.tags expects tag items, got: {item}"));
                  }
                }
                tags.sort();
                tags.dedup();
              } else {
                return Err(format!("DetailedCodeEntry.tags expects a hashset, got: {value}"));
              }
            }
            "code" => {
              code = Some(value.to_owned().try_into()?);
            }
            "schema" if !matches!(value, Edn::Nil) => {
              schema = Some(value.to_owned());
            }
            "ffi" if !matches!(value, Edn::Nil) => {
              ffi = Some(value.to_owned());
            }
            _ => {}
          }
        }

        let code = code.ok_or("Missing code field")?;
        validate_test_names(tests.iter().map(|test| test.name.as_str()), "DetailedCodeEntry.tests")?;
        let schema_parsed: Arc<CalcitTypeAnnotation> = match schema {
          None | Some(Edn::Nil) => DYNAMIC_TYPE.clone(),
          Some(v) => CalcitTypeAnnotation::parse_macro_signature_from_edn(&v)
            .map(|s| Arc::new(CalcitTypeAnnotation::Macro(Arc::new(s))))
            .or_else(|| CalcitTypeAnnotation::parse_fn_schema_from_edn(&v).map(|s| Arc::new(CalcitTypeAnnotation::Fn(Arc::new(s)))))
            .unwrap_or_else(|| DYNAMIC_TYPE.clone()),
        };
        Ok(DetailedCodeEntry {
          doc,
          examples,
          tests,
          tags,
          code,
          schema: schema_parsed,
          ffi,
        })
      }
      _ => Err("Expected struct for DetailedCodeEntry".to_string()),
    }
  }
}

fn parse_detailed_test_entry(value: &Edn) -> Result<DetailedTestEntry, String> {
  let Edn::Struct(test) = value else {
    return Err(format!("DetailedCodeEntry.tests expects TestEntry structs, got: {value}"));
  };
  let mut name = None;
  let mut code = None;
  let mut tags = Vec::new();
  for (key, value) in &test.pairs {
    match key.ref_str() {
      "name" => match value {
        Edn::Str(value) => name = Some(value.to_string()),
        _ => return Err(format!("Detailed TestEntry.name expects a string, got: {value}")),
      },
      "code" => code = Some(value.clone().try_into()?),
      "tags" => match value {
        Edn::Set(items) => {
          for item in &items.0 {
            let Edn::Tag(tag) = item else {
              return Err(format!("Detailed TestEntry.tags expects tag items, got: {item}"));
            };
            tags.push(format!(":{}", tag.ref_str()));
          }
          tags.sort();
          tags.dedup();
        }
        _ => return Err(format!("Detailed TestEntry.tags expects a hashset, got: {value}")),
      },
      _ => {}
    }
  }
  let name = name.ok_or_else(|| "Detailed TestEntry is missing name".to_owned())?;
  validate_test_names([name.as_str()], "Detailed TestEntry")?;
  let code = code.ok_or_else(|| "Detailed TestEntry is missing code".to_owned())?;
  Ok(DetailedTestEntry { name, code, tags })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailedNsEntry {
  pub doc: String,
  pub code: DetailCirru,
}

impl From<NsEntry> for DetailedNsEntry {
  fn from(entry: NsEntry) -> Self {
    DetailedNsEntry {
      doc: entry.doc,
      code: entry.code.into(),
    }
  }
}

impl From<DetailedNsEntry> for NsEntry {
  fn from(detailed: DetailedNsEntry) -> Self {
    NsEntry {
      doc: detailed.doc,
      code: detailed.code.into(),
    }
  }
}

impl TryFrom<Edn> for DetailedNsEntry {
  type Error = String;
  fn try_from(data: Edn) -> Result<Self, String> {
    match data {
      Edn::Struct(struct_value) => {
        let mut doc = String::new();
        let mut code = None;

        for (key, value) in struct_value.pairs.iter() {
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

        Ok(DetailedNsEntry {
          doc,
          code: code.ok_or("Missing code field")?,
        })
      }
      _ => Err("Expected struct for DetailedNsEntry".to_string()),
    }
  }
}

/// Detailed file in snapshot with metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailedFileInSnapshot {
  pub ns: DetailedNsEntry,
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
      Edn::Struct(struct_value) => {
        let mut ns = None;
        let mut defs = HashMap::new();

        for (key, value) in struct_value.pairs.iter() {
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
      _ => Err("Expected struct for DetailedFileInSnapshot".to_string()),
    }
  }
}

/// Detailed snapshot structure for calcit.cirru format with additional metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailedSnapshot {
  pub package: String,
  pub version: String,
  pub entries: Edn,
  pub files: HashMap<String, DetailedFileInSnapshot>,
  /// Additional metadata for detailed snapshot
  pub users: Edn,
}

impl TryFrom<Edn> for DetailedSnapshot {
  type Error = String;
  fn try_from(data: Edn) -> Result<Self, String> {
    match data {
      Edn::Struct(struct_value) => {
        let mut package = String::new();
        let mut version = String::new();
        let mut entries: Edn = Edn::Nil;
        let mut files = HashMap::new();
        let mut users: Edn = Edn::Nil;

        for (key, value) in struct_value.pairs.iter() {
          match key.arc_str().as_ref() {
            "package" => {
              if let Edn::Str(pkg_str) = value {
                package = pkg_str.to_string();
              }
            }
            "version" => {
              if let Edn::Str(text) = value {
                version = text.to_string();
              }
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

        Ok(DetailedSnapshot {
          package,
          version,
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
          version: data.get_or_nil("version").try_into()?,
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
    version: data.get_or_nil("version").try_into()?,
    entries: data.get_or_nil("entries"),
    files,
    users: data.get_or_nil("users"),
  };
  Ok(s)
}

#[cfg(test)]
mod tests {
  use super::{DetailedCodeEntry, parse_detailed_test_entry};
  use crate::calcit::DYNAMIC_TYPE;
  use crate::snapshot::{CodeEntry, TestEntry};
  use cirru_edn::{Edn, EdnTag};
  use cirru_parser::Cirru;
  use std::collections::HashSet;

  #[test]
  fn detailed_code_entry_preserves_attached_tests() {
    let entry = CodeEntry {
      doc: "demo".to_owned(),
      examples: vec![],
      tests: vec![TestEntry {
        name: "returns-value".to_owned(),
        code: Cirru::List(vec![Cirru::leaf("assert="), Cirru::leaf("1"), Cirru::leaf("1")]),
        tags: HashSet::from([EdnTag::new("unit")]),
      }],
      tags: HashSet::new(),
      code: Cirru::leaf("nil"),
      schema: DYNAMIC_TYPE.clone(),
      ffi: None,
    };

    let restored: CodeEntry = DetailedCodeEntry::from(entry.clone()).into();
    assert_eq!(restored.tests, entry.tests);
  }

  #[test]
  fn detailed_test_entry_rejects_surrounding_whitespace() {
    let test = Edn::struct_from_pairs("TestEntry", &[(EdnTag::new("name"), Edn::Str(" test-name ".into()))]);
    let error = parse_detailed_test_entry(&test).expect_err("whitespace must be rejected");
    assert!(error.contains("leading or trailing whitespace"), "unexpected error: {error}");
  }
}
