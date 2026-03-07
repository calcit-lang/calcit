use cirru_edn::{Edn, EdnRecordView, from_edn};
use cirru_parser::Cirru;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotConfigs {
  #[serde(rename = "init-fn")]
  pub init_fn: String,
  #[serde(rename = "reload-fn")]
  pub reload_fn: String,
  #[serde(default)]
  pub modules: Vec<String>,
  #[serde(default)]
  pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeEntry {
  pub doc: String,
  #[serde(default)]
  pub examples: Vec<Cirru>,
  pub code: Cirru,
  #[serde(default)]
  pub schema: Option<Edn>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileInSnapShot {
  pub ns: CodeEntry,
  pub defs: HashMap<String, CodeEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
  pub package: String,
  pub about: Option<String>,
  pub configs: SnapshotConfigs,
  pub entries: HashMap<String, SnapshotConfigs>,
  pub files: HashMap<String, FileInSnapShot>,
}

/// Convert a schema Edn value (either old Quote-wrapped or new direct map) into Edn map form.
fn parse_schema_from_edn(value: &Edn) -> Result<Edn, String> {
  // Old format: Edn::Quote wrapping Cirru — convert to direct map Edn
  if let Ok(cirru) = from_edn::<Cirru>(value.clone()) {
    let text = cirru_parser::format(&[cirru], true.into()).map_err(|e| format!("schema format error: {e}"))?;
    let parsed = cirru_edn::parse(&text).map_err(|e| format!("schema parse error: {e}"))?;
    validate_schema_edn_no_legacy_quotes(&parsed)?;
    return Ok(parsed);
  }
  // New format: already a direct Edn map
  validate_schema_edn_no_legacy_quotes(value)?;
  Ok(value.clone())
}

fn validate_schema_edn_no_legacy_quotes(value: &Edn) -> Result<(), String> {
  match value {
    Edn::Symbol(s) => {
      if s.starts_with('\'') {
        let inner = s.trim_start_matches('\'');
        return Err(format!(
          "Legacy schema generic symbol `{s}` is invalid. Use single-quoted source syntax like `'{inner}`, which should be stored as plain EDN symbol `{inner}`."
        ));
      }
      Ok(())
    }
    Edn::List(xs) => {
      for item in &xs.0 {
        validate_schema_edn_no_legacy_quotes(item)?;
      }
      Ok(())
    }
    Edn::Map(map) => {
      for (_, v) in &map.0 {
        validate_schema_edn_no_legacy_quotes(v)?;
      }
      Ok(())
    }
    Edn::Tuple(view) => {
      validate_schema_edn_no_legacy_quotes(view.tag.as_ref())?;
      for item in &view.extra {
        validate_schema_edn_no_legacy_quotes(item)?;
      }
      Ok(())
    }
    Edn::Set(set) => {
      for item in &set.0 {
        validate_schema_edn_no_legacy_quotes(item)?;
      }
      Ok(())
    }
    Edn::Record(_) => Ok(()),
    _ => Ok(()),
  }
}

fn parse_code_entry(edn: Edn) -> Result<CodeEntry, String> {
  let record: EdnRecordView = match edn {
    Edn::Record(r) => r,
    other => return Err(format!("CodeEntry: expected record, got {other:?}")),
  };
  let mut doc = String::new();
  let mut examples: Vec<Cirru> = vec![];
  let mut code: Option<Cirru> = None;
  let mut schema: Option<Edn> = None;
  for (key, value) in &record.pairs {
    match key.arc_str().as_ref() {
      "doc" => doc = from_edn(value.clone()).map_err(|e| format!("doc: {e}"))?,
      "examples" => examples = from_edn(value.clone()).map_err(|e| format!("examples: {e}"))?,
      "code" => code = Some(from_edn(value.clone()).map_err(|e| format!("code: {e}"))?),
      "schema" => {
        if !matches!(value, Edn::Nil) {
          schema = Some(parse_schema_from_edn(value).map_err(|e| format!("schema: {e}"))?);
        }
      }
      _ => {}
    }
  }
  Ok(CodeEntry {
    doc,
    examples,
    code: code.ok_or("CodeEntry: missing code field")?,
    schema,
  })
}

fn parse_file_in_snapshot(edn: Edn) -> Result<FileInSnapShot, String> {
  let record: EdnRecordView = match edn {
    Edn::Record(r) => r,
    other => return Err(format!("FileInSnapShot: expected record, got {other:?}")),
  };
  let mut ns: Option<CodeEntry> = None;
  let mut defs: HashMap<String, CodeEntry> = HashMap::new();
  for (key, value) in &record.pairs {
    match key.arc_str().as_ref() {
      "ns" => ns = Some(parse_code_entry(value.clone())?),
      "defs" => {
        let map = match value {
          Edn::Map(m) => m,
          other => return Err(format!("FileInSnapShot.defs: expected map, got {other:?}")),
        };
        for (def_key, def_value) in &map.0 {
          let name: String = from_edn(def_key.clone()).map_err(|e| format!("def key: {e}"))?;
          defs.insert(name, parse_code_entry(def_value.clone())?);
        }
      }
      _ => {}
    }
  }
  Ok(FileInSnapShot {
    ns: ns.ok_or("FileInSnapShot: missing ns field")?,
    defs,
  })
}

fn parse_files(edn: Edn) -> Result<HashMap<String, FileInSnapShot>, String> {
  match edn {
    Edn::Map(map) => {
      let mut result = HashMap::with_capacity(map.0.len());
      for (key, value) in map.0 {
        let name: String = from_edn(key).map_err(|e| format!("file key: {e}"))?;
        result.insert(name, parse_file_in_snapshot(value)?);
      }
      Ok(result)
    }
    other => Err(format!("files: expected map, got {other:?}")),
  }
}

fn main() {
  println!("cargo:rerun-if-changed=src/cirru/calcit-core.cirru");

  let out_dir = env::var_os("OUT_DIR").unwrap();
  let dest_path = Path::new(&out_dir).join("calcit-core.rmp");

  let core_content = fs::read_to_string("src/cirru/calcit-core.cirru").expect("read core");
  let core_data = cirru_edn::parse(&core_content).expect("parse core");

  // Minimal logic to convert Edn to Snapshot as in src/snapshot.rs
  let data = core_data.view_map().expect("map");
  let pkg: String = from_edn(data.get_or_nil("package")).expect("pkg");
  let about = match data.get_or_nil("about") {
    Edn::Nil => None,
    value => Some(from_edn::<String>(value).expect("about")),
  };

  let files = parse_files(data.get_or_nil("files")).expect("files");

  let snapshot = Snapshot {
    package: pkg,
    about,
    configs: from_edn(data.get_or_nil("configs")).expect("configs"),
    entries: from_edn(data.get_or_nil("entries")).expect("entries"),
    files,
  };

  let mut buf = Vec::new();
  snapshot.serialize(&mut rmp_serde::Serializer::new(&mut buf)).expect("serialize");
  fs::write(dest_path, buf).expect("write");
}
