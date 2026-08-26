use cirru_edn::{Edn, EdnMapView, EdnStructView, from_edn};
use cirru_parser::Cirru;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SnapshotRunMode {
  #[default]
  Native,
  Js,
}

fn deserialize_run_mode<'de, D>(deserializer: D) -> Result<SnapshotRunMode, D::Error>
where
  D: serde::Deserializer<'de>,
{
  let value = Edn::deserialize(deserializer)?;
  let mode = match value {
    Edn::Tag(tag) => tag.ref_str().to_owned(),
    Edn::Str(text) | Edn::Symbol(text) => text.trim_start_matches(':').to_owned(),
    other => return Err(serde::de::Error::custom(format!("expected :native or :js, got {other:?}"))),
  };
  match mode.as_str() {
    "native" => Ok(SnapshotRunMode::Native),
    "js" => Ok(SnapshotRunMode::Js),
    _ => Err(serde::de::Error::custom(format!("expected :native or :js, got {mode}"))),
  }
}

fn deserialize_ns_def<'de, D>(deserializer: D) -> Result<String, D::Error>
where
  D: serde::Deserializer<'de>,
{
  match Edn::deserialize(deserializer)? {
    Edn::Str(text) | Edn::Symbol(text) => Ok(text.to_string()),
    other => Err(serde::de::Error::custom(format!(
      "expected namespace/definition string or symbol, got {other:?}"
    ))),
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotEntry {
  #[serde(default, deserialize_with = "deserialize_run_mode")]
  pub mode: SnapshotRunMode,
  #[serde(rename = "init-fn", deserialize_with = "deserialize_ns_def")]
  pub init_fn: String,
  #[serde(rename = "reload-fn", deserialize_with = "deserialize_ns_def")]
  pub reload_fn: String,
  #[serde(default)]
  pub description: String,
  #[serde(default)]
  pub modules: Vec<String>,
  #[serde(default, rename = "type-slots")]
  pub type_slots: HashMap<String, String>,
  #[serde(default, rename = "feature-policy")]
  pub feature_policy: HashMap<String, String>,
  #[serde(default)]
  pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacySnapshotConfigs {
  #[serde(rename = "init-fn", deserialize_with = "deserialize_ns_def")]
  pub init_fn: String,
  #[serde(rename = "reload-fn", deserialize_with = "deserialize_ns_def")]
  pub reload_fn: String,
  #[serde(default)]
  pub modules: Vec<String>,
  #[serde(default)]
  pub version: String,
  #[serde(default, rename = "type-slots")]
  pub type_slots: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeEntry {
  pub doc: String,
  #[serde(default)]
  pub examples: Vec<Cirru>,
  #[serde(default)]
  pub tests: Vec<TestEntry>,
  #[serde(default)]
  pub tags: Vec<String>,
  pub code: Cirru,
  #[serde(default)]
  pub schema: Option<Edn>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestEntry {
  pub name: String,
  pub code: Cirru,
  #[serde(default)]
  pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NsEntry {
  pub doc: String,
  pub code: Cirru,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileInSnapShot {
  pub ns: NsEntry,
  pub defs: HashMap<String, CodeEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
  pub package: String,
  pub about: Option<String>,
  #[serde(default)]
  pub version: String,
  pub entries: HashMap<String, SnapshotEntry>,
  pub files: HashMap<String, FileInSnapShot>,
}

fn parse_snapshot_entry(data: Edn) -> Result<SnapshotEntry, String> {
  let map = data.view_map().map_err(|e| format!("entry must be a map: {e}"))?;
  let ns_def = |key: &str| match map.get_or_nil(key) {
    Edn::Str(text) | Edn::Symbol(text) => Ok(text.to_string()),
    other => Err(format!(
      "entry `:{key}` must be a namespace/definition string or symbol, got {other:?}"
    )),
  };
  let mode = match map.get_or_nil("mode") {
    Edn::Tag(tag) if tag.ref_str() == "native" => SnapshotRunMode::Native,
    Edn::Tag(tag) if tag.ref_str() == "js" => SnapshotRunMode::Js,
    Edn::Str(text) | Edn::Symbol(text) if text.trim_start_matches(':') == "native" => SnapshotRunMode::Native,
    Edn::Str(text) | Edn::Symbol(text) if text.trim_start_matches(':') == "js" => SnapshotRunMode::Js,
    other => return Err(format!("entry `:mode` must be :native or :js, got {other:?}")),
  };
  Ok(SnapshotEntry {
    mode,
    init_fn: ns_def("init-fn")?,
    reload_fn: ns_def("reload-fn")?,
    description: from_edn(map.get_or_nil("description")).map_err(|e| format!("entry `:description`: {e}"))?,
    modules: from_edn(map.get_or_nil("modules")).map_err(|e| format!("entry `:modules`: {e}"))?,
    type_slots: from_edn(map.get_or_nil("type-slots")).map_err(|e| format!("entry `:type-slots`: {e}"))?,
    feature_policy: match map.get_or_nil("feature-policy") {
      Edn::Nil => HashMap::new(),
      value => parse_feature_policy(value)?,
    },
    target: match map.get_or_nil("target") {
      Edn::Nil => None,
      value => Some(from_edn(value).map_err(|e| format!("entry `:target`: {e}"))?),
    },
  })
}

fn parse_feature_policy(data: Edn) -> Result<HashMap<String, String>, String> {
  let map = data.view_map().map_err(|e| format!("entry `:feature-policy` must be a map: {e}"))?;
  let text = |value: &Edn| match value {
    Edn::Tag(tag) => Ok(tag.ref_str().to_owned()),
    Edn::Str(value) | Edn::Symbol(value) => Ok(value.to_string().trim_start_matches(':').to_owned()),
    other => Err(format!("entry `:feature-policy` expects tag/string/symbol values, got {other:?}")),
  };
  map.0.iter().map(|(key, value)| Ok((text(key)?, text(value)?))).collect()
}

fn parse_entries(data: Edn) -> Result<HashMap<String, SnapshotEntry>, String> {
  let map = data.view_map().map_err(|e| format!("entries must be a map: {e}"))?;
  let mut entries = HashMap::with_capacity(map.0.len());
  for (name, value) in map.0 {
    let name: String = from_edn(name).map_err(|e| format!("invalid entry name: {e}"))?;
    entries.insert(name, parse_snapshot_entry(value)?);
  }
  Ok(entries)
}

fn format_edn_preview(value: &Edn) -> String {
  let raw = cirru_edn::format(value, true).unwrap_or_else(|_| format!("{value:?}"));
  const LIMIT: usize = 220;
  if raw.chars().count() > LIMIT {
    let truncated = raw.chars().take(LIMIT).collect::<String>();
    format!("{truncated}…")
  } else {
    raw
  }
}

fn truncate_preview(raw: &str, limit: usize) -> String {
  if raw.chars().count() > limit {
    let truncated = raw.chars().take(limit).collect::<String>();
    format!("{truncated}…")
  } else {
    raw.to_owned()
  }
}

fn truncate_edn_error_nodes(message: &str) -> String {
  const NODE_LIMIT: usize = 200;

  message
    .lines()
    .map(|line| {
      if let Some((prefix, preview)) = line.split_once("Node: ") {
        format!("{prefix}Node: {}", truncate_preview(preview, NODE_LIMIT))
      } else {
        line.to_owned()
      }
    })
    .collect::<Vec<_>>()
    .join("\n")
}

fn format_edn_error<E: std::fmt::Display>(error: E) -> String {
  truncate_edn_error_nodes(&error.to_string())
}

fn schema_path_label(path: &[String]) -> String {
  if path.is_empty() { "<root>".to_owned() } else { path.join("") }
}

fn map_key_path_segment(key: &Edn) -> String {
  match key {
    Edn::Tag(tag) => format!(".{}", tag.ref_str()),
    Edn::Str(text) => format!(".{text}"),
    Edn::Symbol(text) => format!(".{text}"),
    _ => ".<key>".to_owned(),
  }
}

fn canonical_schema_field_name(text: &str) -> Option<&'static str> {
  match text.trim_start_matches(':') {
    "kind" => Some("kind"),
    "args" => Some("args"),
    "return" => Some("return"),
    "rest" => Some("rest"),
    "generics" => Some("generics"),
    "where" => Some("where"),
    _ => None,
  }
}

fn canonical_schema_kind_name(text: &str) -> Option<&'static str> {
  match text.trim_start_matches(':') {
    "fn" => Some("fn"),
    "macro" => Some("macro"),
    _ => None,
  }
}

fn normalize_schema_map(map: &EdnMapView) -> Edn {
  let mut normalized = EdnMapView::default();

  for (key, value) in &map.0 {
    let normalized_key = match key {
      Edn::Tag(tag) => Edn::tag(tag.ref_str()),
      Edn::Str(text) => canonical_schema_field_name(text.as_ref())
        .map(Edn::tag)
        .unwrap_or_else(|| key.clone()),
      Edn::Symbol(text) => canonical_schema_field_name(text.as_ref())
        .map(Edn::tag)
        .unwrap_or_else(|| key.clone()),
      _ => key.clone(),
    };

    let normalized_value = match (&normalized_key, value) {
      (Edn::Tag(tag), Edn::Str(text)) | (Edn::Tag(tag), Edn::Symbol(text)) if tag.ref_str() == "kind" => {
        canonical_schema_kind_name(text.as_ref())
          .map(Edn::tag)
          .unwrap_or_else(|| value.clone())
      }
      _ => value.clone(),
    };

    normalized.insert(normalized_key, normalized_value);
  }

  Edn::Map(normalized)
}

/// Convert a schema Edn value (either old Quote-wrapped or new direct map) into Edn map form.
fn parse_schema_from_edn(value: &Edn, owner: &str) -> Result<Edn, String> {
  // Simple scalar schemas (e.g. :dynamic, :nil) are valid as-is — skip Cirru path
  match value {
    Edn::Tag(_) | Edn::Nil => {
      validate_schema_edn_no_legacy_quotes(value, owner)?;
      return Ok(value.clone());
    }
    Edn::Map(map) => {
      let normalized = normalize_schema_map(map);
      validate_schema_edn_no_legacy_quotes(&normalized, owner)?;
      return Ok(normalized);
    }
    Edn::Enum(view) if matches!(view.variant.as_ref(), "fn" | "macro") && matches!(view.extra.first(), Some(Edn::Map(_))) => {
      let Some(Edn::Map(map)) = view.extra.first() else {
        unreachable!();
      };
      let mut normalized = match normalize_schema_map(map) {
        Edn::Map(map) => map,
        _ => unreachable!(),
      };
      if normalized.tag_get("kind").is_none() && view.variant.as_ref() == "macro" {
        normalized.insert_key("kind", Edn::tag("macro"));
      }
      let normalized = Edn::Map(normalized);
      validate_schema_edn_no_legacy_quotes(&normalized, owner)?;
      return Ok(normalized);
    }
    _ => {}
  }
  // Old format: Edn::Quote wrapping Cirru — convert to direct map Edn
  if let Ok(cirru) = from_edn::<Cirru>(value.clone()) {
    let text = cirru_parser::format(&[cirru], true.into())
      .map_err(|e| format!("{owner}: failed to format quoted schema before validation: {}", format_edn_error(e)))?;
    let parsed = cirru_edn::parse(&text).map_err(|e| {
      format!(
        "{owner}: failed to parse quoted schema after formatting: {}; schema={}",
        format_edn_error(e),
        truncate_preview(&text, 200)
      )
    })?;
    validate_schema_edn_no_legacy_quotes(&parsed, owner)?;
    return Ok(parsed);
  }
  // New format: already a direct Edn map
  validate_schema_edn_no_legacy_quotes(value, owner)?;
  Ok(value.clone())
}

fn validate_schema_edn_no_legacy_quotes(value: &Edn, owner: &str) -> Result<(), String> {
  fn walk(value: &Edn, owner: &str, path: &mut Vec<String>) -> Result<(), String> {
    match value {
      Edn::Symbol(s) => {
        if s.starts_with('\'') {
          let inner = s.trim_start_matches('\'');
          return Err(format!(
            "{owner}: invalid schema generic symbol `{s}` at {}. Use source syntax like `'{inner}`, but store it as plain EDN symbol `{inner}`.",
            schema_path_label(path)
          ));
        }
        Ok(())
      }
      Edn::List(xs) => {
        for (idx, item) in xs.0.iter().enumerate() {
          path.push(format!("[{idx}]"));
          walk(item, owner, path)?;
          path.pop();
        }
        Ok(())
      }
      Edn::Map(map) => {
        for (k, v) in &map.0 {
          path.push(map_key_path_segment(k));
          walk(v, owner, path)?;
          path.pop();
        }
        Ok(())
      }
      Edn::Enum(view) => {
        if view.variant.starts_with('\'') {
          return Err(format!(
            "{owner}: invalid schema enum symbol `{}` at {}",
            view.variant,
            schema_path_label(path)
          ));
        }
        for (idx, item) in view.extra.iter().enumerate() {
          path.push(format!("[{idx}]"));
          walk(item, owner, path)?;
          path.pop();
        }
        Ok(())
      }
      Edn::Set(set) => {
        for (idx, item) in set.0.iter().enumerate() {
          path.push(format!("[#{idx}]"));
          walk(item, owner, path)?;
          path.pop();
        }
        Ok(())
      }
      Edn::Struct(_) => Ok(()),
      _ => Ok(()),
    }
  }

  let mut path = vec![];
  walk(value, owner, &mut path)
}

fn parse_tags_from_edn(value: &Edn, owner: &str, field: &str) -> Result<Vec<String>, String> {
  match value {
    Edn::Set(set) => {
      let mut tags = Vec::with_capacity(set.0.len());
      for item in &set.0 {
        match item {
          Edn::Tag(tag) => tags.push(format!(":{}", tag.ref_str())),
          other => {
            return Err(format!("{owner}: {field} expects tag items, got {}", format_edn_preview(other)));
          }
        }
      }
      tags.sort();
      tags.dedup();
      Ok(tags)
    }
    other => Err(format!("{owner}: {field} expects a hashset, got {}", format_edn_preview(other))),
  }
}

fn parse_code_entry(edn: Edn, owner: &str) -> Result<CodeEntry, String> {
  let struct_value: EdnStructView = match edn {
    Edn::Struct(r) => r,
    other => return Err(format!("{owner}: expected CodeEntry struct, got {}", format_edn_preview(&other))),
  };
  let mut doc = String::new();
  let mut examples: Vec<Cirru> = vec![];
  let mut tests: Vec<TestEntry> = vec![];
  let mut tags: Vec<String> = Vec::new();
  let mut code: Option<Cirru> = None;
  let mut schema: Option<Edn> = None;
  for (key, value) in &struct_value.pairs {
    match key.arc_str().as_ref() {
      "doc" => doc = from_edn(value.clone()).map_err(|e| format!("{owner}: invalid `:doc`: {e}"))?,
      "examples" => examples = from_edn(value.clone()).map_err(|e| format!("{owner}: invalid `:examples`: {e}"))?,
      "tests" => {
        let Edn::List(items) = value else {
          return Err(format!("{owner}: `:tests` expects a list, got {}", format_edn_preview(value)));
        };
        tests = items
          .0
          .iter()
          .cloned()
          .map(|item| parse_test_entry(item, owner))
          .collect::<Result<Vec<_>, _>>()?;
        let mut names = std::collections::HashSet::new();
        for test in &tests {
          if !names.insert(test.name.as_str()) {
            return Err(format!("{owner}: duplicate test name `{}`", test.name));
          }
        }
      }
      "tags" => tags = parse_tags_from_edn(value, owner, "CodeEntry.tags")?,
      "code" => code = Some(from_edn(value.clone()).map_err(|e| format!("{owner}: invalid `:code`: {e}"))?),
      "schema" if !matches!(value, Edn::Nil) => {
        schema = Some(parse_schema_from_edn(value, owner).map_err(|e| format!("{owner}: invalid `:schema`: {e}"))?);
      }
      _ => {}
    }
  }
  Ok(CodeEntry {
    doc,
    examples,
    tests,
    tags,
    code: code.ok_or_else(|| format!("{owner}: missing `:code` field in CodeEntry"))?,
    schema,
  })
}

fn parse_test_entry(edn: Edn, owner: &str) -> Result<TestEntry, String> {
  let struct_value = match edn {
    Edn::Struct(value) => value,
    other => return Err(format!("{owner}: expected TestEntry struct, got {}", format_edn_preview(&other))),
  };
  let mut name = None;
  let mut code = None;
  let mut tags = Vec::new();
  for (key, value) in &struct_value.pairs {
    match key.ref_str() {
      "name" => name = Some(from_edn(value.clone()).map_err(|error| format!("{owner}: invalid test `:name`: {error}"))?),
      "code" => code = Some(from_edn(value.clone()).map_err(|error| format!("{owner}: invalid test `:code`: {error}"))?),
      "tags" => tags = parse_tags_from_edn(value, owner, "TestEntry.tags")?,
      _ => {}
    }
  }
  let name: String = name.ok_or_else(|| format!("{owner}: test is missing `:name`"))?;
  if name.trim().is_empty() {
    return Err(format!("{owner}: test name must not be empty"));
  }
  Ok(TestEntry {
    name,
    code: code.ok_or_else(|| format!("{owner}: test is missing `:code`"))?,
    tags,
  })
}

fn parse_ns_entry(edn: Edn, owner: &str) -> Result<NsEntry, String> {
  let struct_value: EdnStructView = match edn {
    Edn::Struct(r) => r,
    other => {
      return Err(format!(
        "{owner}: expected NsEntry/CodeEntry struct, got {}",
        format_edn_preview(&other)
      ));
    }
  };
  let mut doc = String::new();
  let mut code: Option<Cirru> = None;
  for (key, value) in &struct_value.pairs {
    match key.arc_str().as_ref() {
      "doc" => doc = from_edn(value.clone()).map_err(|e| format!("{owner}: invalid `:doc`: {e}"))?,
      "code" => code = Some(from_edn(value.clone()).map_err(|e| format!("{owner}: invalid `:code`: {e}"))?),
      _ => {}
    }
  }
  Ok(NsEntry {
    doc,
    code: code.ok_or_else(|| format!("{owner}: missing `:code` field in NsEntry"))?,
  })
}

fn parse_file_in_snapshot(edn: Edn, file_name: &str) -> Result<FileInSnapShot, String> {
  let struct_value: EdnStructView = match edn {
    Edn::Struct(r) => r,
    other => {
      return Err(format!(
        "{file_name}: expected FileEntry struct, got {}",
        format_edn_preview(&other)
      ));
    }
  };
  let mut ns: Option<NsEntry> = None;
  let mut defs: HashMap<String, CodeEntry> = HashMap::new();
  for (key, value) in &struct_value.pairs {
    match key.arc_str().as_ref() {
      "ns" => ns = Some(parse_ns_entry(value.clone(), &format!("{file_name}/:ns"))?),
      "defs" => {
        let map = match value {
          Edn::Map(m) => m,
          other => return Err(format!("{file_name}: expected `:defs` map, got {}", format_edn_preview(other))),
        };
        for (def_key, def_value) in &map.0 {
          let name: String = from_edn(def_key.clone()).map_err(|e| format!("{file_name}: invalid def key: {e}"))?;
          let owner = format!("{file_name}/{name}");
          defs.insert(name, parse_code_entry(def_value.clone(), &owner)?);
        }
      }
      _ => {}
    }
  }
  Ok(FileInSnapShot {
    ns: ns.ok_or_else(|| format!("{file_name}: missing `:ns` field in FileEntry"))?,
    defs,
  })
}

fn parse_files(edn: Edn) -> Result<HashMap<String, FileInSnapShot>, String> {
  match edn {
    Edn::Map(map) => {
      let mut result = HashMap::with_capacity(map.0.len());
      for (key, value) in map.0 {
        let name: String = from_edn(key).map_err(|e| format!("invalid file key: {e}"))?;
        result.insert(name.clone(), parse_file_in_snapshot(value, &name)?);
      }
      Ok(result)
    }
    other => Err(format!("snapshot `:files` must be a map, got {}", format_edn_preview(&other))),
  }
}

fn rustc_verbose_field<'a>(output: &'a str, name: &str) -> &'a str {
  output
    .lines()
    .find_map(|line| line.strip_prefix(name).map(str::trim))
    .unwrap_or_else(|| panic!("`rustc --version --verbose` did not report `{name}`"))
}

fn ffi_build_id() -> String {
  let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
  let output = Command::new(&rustc)
    .args(["--version", "--verbose"])
    .output()
    .unwrap_or_else(|error| panic!("failed to run `{rustc} --version --verbose`: {error}"));
  if !output.status.success() {
    panic!("`{rustc} --version --verbose` failed with {}", output.status);
  }
  let verbose = String::from_utf8(output.stdout).expect("rustc verbose version must be UTF-8");
  let release = rustc_verbose_field(&verbose, "release:");
  let commit = rustc_verbose_field(&verbose, "commit-hash:");
  let target = env::var("TARGET").expect("Cargo must provide TARGET to build scripts");
  let debug_assertions = env::var_os("CARGO_CFG_DEBUG_ASSERTIONS").is_some();
  let panic_strategy = env::var("CARGO_CFG_PANIC").expect("Cargo must provide CARGO_CFG_PANIC to build scripts");

  format!("rustc={release}:{commit};target={target};debug-assertions={debug_assertions};panic={panic_strategy}")
}

fn main() {
  println!("cargo:rerun-if-changed=src/cirru/calcit-core.cirru");
  println!("cargo:rerun-if-env-changed=RUSTC");
  println!("cargo:rerun-if-env-changed=TARGET");
  println!("cargo:rerun-if-env-changed=CARGO_CFG_DEBUG_ASSERTIONS");
  println!("cargo:rerun-if-env-changed=CARGO_CFG_PANIC");
  println!("cargo:rustc-env=CALCIT_FFI_BUILD_ID={}", ffi_build_id());

  let out_dir = env::var_os("OUT_DIR").unwrap();
  let dest_path = Path::new(&out_dir).join("calcit-core.rmp");

  let core_content =
    fs::read_to_string("src/cirru/calcit-core.cirru").unwrap_or_else(|e| panic!("failed to read src/cirru/calcit-core.cirru: {e}"));
  let core_data = cirru_edn::parse(&core_content)
    .unwrap_or_else(|e| panic!("failed to parse src/cirru/calcit-core.cirru as Cirru EDN: {}", format_edn_error(e)));

  // Minimal logic to convert Edn to Snapshot as in src/snapshot.rs
  let data = core_data
    .view_map()
    .unwrap_or_else(|e| panic!("calcit-core snapshot root must be a map: {e}"));
  let pkg: String = from_edn(data.get_or_nil("package")).unwrap_or_else(|e| panic!("failed to parse calcit-core `:package`: {e}"));
  let about = match data.get_or_nil("about") {
    Edn::Nil => None,
    value => Some(from_edn::<String>(value).unwrap_or_else(|e| panic!("failed to parse calcit-core `:about`: {e}"))),
  };

  let files = parse_files(data.get_or_nil("files")).unwrap_or_else(|e| panic!("failed to parse calcit-core `:files`: {e}"));

  let legacy_configs = match data.get_or_nil("configs") {
    Edn::Nil => None,
    value => {
      Some(from_edn::<LegacySnapshotConfigs>(value).unwrap_or_else(|e| panic!("failed to parse calcit-core legacy `:configs`: {e}")))
    }
  };
  let mut entries = parse_entries(data.get_or_nil("entries")).unwrap_or_else(|e| panic!("failed to parse calcit-core `:entries`: {e}"));
  if let Some(configs) = &legacy_configs {
    entries.insert(
      "default".to_owned(),
      SnapshotEntry {
        mode: SnapshotRunMode::Native,
        init_fn: configs.init_fn.clone(),
        reload_fn: configs.reload_fn.clone(),
        description: String::new(),
        modules: configs.modules.clone(),
        type_slots: configs.type_slots.clone(),
        feature_policy: HashMap::new(),
        target: None,
      },
    );
  }
  let version = match data.get_or_nil("version") {
    Edn::Nil => legacy_configs.map(|configs| configs.version).unwrap_or_default(),
    value => from_edn(value).unwrap_or_else(|e| panic!("failed to parse calcit-core `:version`: {e}")),
  };

  let snapshot = Snapshot {
    package: pkg,
    about,
    version,
    entries,
    files,
  };

  let mut buf = Vec::new();
  snapshot
    .serialize(&mut rmp_serde::Serializer::new(&mut buf))
    .unwrap_or_else(|e| panic!("failed to serialize embedded calcit-core snapshot: {e}"));
  fs::write(dest_path, buf).unwrap_or_else(|e| panic!("failed to write embedded calcit-core snapshot: {e}"));
}
