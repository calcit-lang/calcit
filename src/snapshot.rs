use cirru_edn::{Edn, EdnMapView, EdnRecordView, EdnSetView, EdnTag, from_edn};
use cirru_parser::Cirru;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;
use std::path::Path;
use std::sync::Arc;

use crate::calcit::{CalcitFnTypeAnnotation, CalcitTypeAnnotation, DYNAMIC_TYPE, SchemaKind, with_type_annotation_warning_context};

const SNAPSHOT_ABOUT_MESSAGE: &str = "file is generated - never edit directly; learn cr edit/tree workflows before changing";

fn default_version() -> String {
  "0.0.0".to_owned()
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

fn normalize_schema_map(map: &EdnMapView) -> EdnMapView {
  let mut normalized = EdnMapView::default();

  for (key, value) in map.0.iter() {
    let normalized_key = match key {
      Edn::Tag(tag) => Edn::tag(tag.ref_str()),
      Edn::Str(text) => canonical_schema_field_name(text).map(Edn::tag).unwrap_or_else(|| key.clone()),
      Edn::Symbol(text) => canonical_schema_field_name(text).map(Edn::tag).unwrap_or_else(|| key.clone()),
      _ => key.clone(),
    };

    let normalized_value = match (&normalized_key, value) {
      (Edn::Tag(tag), Edn::Str(text)) | (Edn::Tag(tag), Edn::Symbol(text)) if tag.ref_str() == "kind" => {
        canonical_schema_kind_name(text).map(Edn::tag).unwrap_or_else(|| value.clone())
      }
      _ => value.clone(),
    };

    normalized.insert(normalized_key, normalized_value);
  }

  normalized
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
struct RawCodeEntry {
  pub doc: String,
  #[serde(default)]
  pub examples: Vec<Cirru>,
  pub code: Cirru,
  #[serde(default)]
  pub schema: Option<Edn>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RawFileInSnapShot {
  pub ns: NsEntry,
  pub defs: HashMap<String, RawCodeEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RawSnapshot {
  pub package: String,
  pub about: Option<String>,
  pub configs: SnapshotConfigs,
  pub entries: HashMap<String, SnapshotConfigs>,
  pub files: HashMap<String, RawFileInSnapShot>,
}

impl RawCodeEntry {
  fn into_code_entry(self, owner: &str) -> Result<CodeEntry, String> {
    let schema = match self.schema {
      None | Some(Edn::Nil) => DYNAMIC_TYPE.clone(),
      Some(value) => with_type_annotation_warning_context(owner.to_owned(), || parse_loaded_schema_annotation(&value, owner))?,
    };

    Ok(CodeEntry {
      doc: self.doc,
      examples: self.examples,
      code: self.code,
      schema,
    })
  }
}

pub fn decode_binary_snapshot(bytes: &[u8]) -> Result<Snapshot, String> {
  let raw: RawSnapshot = rmp_serde::from_slice(bytes).map_err(|e| e.to_string())?;
  let mut files: HashMap<String, FileInSnapShot> = HashMap::with_capacity(raw.files.len());

  for (file_name, raw_file) in raw.files {
    let ns = raw_file.ns;
    let mut defs: HashMap<String, CodeEntry> = HashMap::with_capacity(raw_file.defs.len());

    for (def_name, raw_entry) in raw_file.defs {
      let owner = format!("{file_name}/{def_name}");
      defs.insert(def_name, raw_entry.into_code_entry(&owner)?);
    }

    files.insert(file_name, FileInSnapShot { ns, defs });
  }

  Ok(Snapshot {
    package: raw.package,
    about: raw.about,
    configs: raw.configs,
    entries: raw.entries,
    files,
  })
}

impl From<&FileInSnapShot> for Edn {
  fn from(data: &FileInSnapShot) -> Edn {
    let mut defs_map = EdnMapView::default();
    for (k, v) in &data.defs {
      defs_map.insert(Edn::str(k.as_str()), Edn::from(v));
    }
    Edn::Record(EdnRecordView {
      tag: EdnTag::new("FileEntry"),
      pairs: vec![("defs".into(), Edn::from(defs_map)), ("ns".into(), Edn::from(&data.ns))], // TODO
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
    let mut defs_map = EdnMapView::default();
    for (k, v) in data.defs {
      defs_map.insert(Edn::str(k.as_str()), Edn::from(v));
    }
    Edn::map_from_iter([("defs".into(), Edn::from(defs_map)), ("ns".into(), data.ns.into())])
  }
}

impl TryFrom<Edn> for NsEntry {
  type Error = String;
  fn try_from(data: Edn) -> Result<Self, String> {
    let mut doc = String::new();
    let mut code: Option<Cirru> = None;

    match data {
      Edn::Record(record) => {
        for (key, value) in &record.pairs {
          match key.arc_str().as_ref() {
            "doc" => {
              doc = from_edn(value.to_owned()).map_err(|e| format!("failed to parse NsEntry.doc: {e}"))?;
            }
            "code" => {
              code = Some(from_edn(value.to_owned()).map_err(|e| format!("failed to parse NsEntry.code: {e}"))?);
            }
            _ => {}
          }
        }
      }
      Edn::Map(map) => {
        if let Some(value) = map.get(&Edn::Tag(EdnTag::new("doc"))) {
          doc = from_edn(value.to_owned()).map_err(|e| format!("failed to parse NsEntry.doc: {e}"))?;
        }
        if let Some(value) = map.get(&Edn::Tag(EdnTag::new("code"))) {
          code = Some(from_edn(value.to_owned()).map_err(|e| format!("failed to parse NsEntry.code: {e}"))?);
        }
      }
      other => {
        return Err(format!("failed to parse NsEntry: expected record/map, got: {other:?}"));
      }
    }

    Ok(NsEntry {
      doc,
      code: code.ok_or_else(|| "failed to parse NsEntry: missing code field".to_owned())?,
    })
  }
}

impl From<NsEntry> for Edn {
  fn from(data: NsEntry) -> Self {
    Edn::record_from_pairs(
      "NsEntry".into(),
      &[("doc".into(), data.doc.into()), ("code".into(), data.code.into())],
    )
  }
}

impl From<&NsEntry> for Edn {
  fn from(data: &NsEntry) -> Self {
    Edn::record_from_pairs(
      "NsEntry".into(),
      &[
        ("doc".into(), data.doc.to_owned().into()),
        ("code".into(), data.code.to_owned().into()),
      ],
    )
  }
}

/// Custom serde for `CodeEntry::schema`.
/// The binary RMP format stores schemas as `Option<Edn>` (compatible with `build.rs`);
/// at runtime we keep a parsed `Arc<CalcitTypeAnnotation>` for direct use.
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
      Some(v) => parse_loaded_schema_annotation(&v, "CodeEntry.schema").map_err(serde::de::Error::custom)?,
    })
  }
}

fn parse_loaded_schema_annotation(value: &Edn, owner: &str) -> Result<Arc<CalcitTypeAnnotation>, String> {
  if matches!(value, Edn::Nil) {
    return Ok(DYNAMIC_TYPE.clone());
  }

  // Primitive type tag stored as a plain EDN tag (e.g. :string, :number).
  if let Edn::Tag(tag) = value {
    let tag_name = tag.ref_str();
    if PRIMITIVE_SCHEMA_TAGS.contains(&tag_name) {
      return Ok(Arc::new(CalcitTypeAnnotation::from_tag_name(tag_name)));
    }
    return Err(format!(
      "unknown primitive schema tag `:{tag_name}` in {owner}; valid tags: {}",
      PRIMITIVE_SCHEMA_TAGS.join(", ")
    ));
  }

  let normalized =
    normalize_schema_edn(value).map_err(|e| format!("failed to normalize {owner}: {e}; schema={}", format_edn_preview(value)))?;
  let schema_cirru = parse_schema_cirru_from_edn(&normalized).map_err(|e| {
    format!(
      "failed to convert {owner} into Cirru: {e}; schema={}",
      format_edn_preview(&normalized)
    )
  })?;
  parse_schema_data(&schema_cirru)
    .map_err(|e| format!("failed to validate {owner}: {e}; schema={}", format_edn_preview(&normalized)))?;

  CalcitTypeAnnotation::parse_fn_schema_from_edn(&normalized)
    .map(|s| Arc::new(CalcitTypeAnnotation::Fn(Arc::new(s))))
    .ok_or_else(|| {
      format!(
        "failed to parse {owner} as function schema after normalization; schema={}",
        format_edn_preview(&normalized)
      )
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeEntry {
  pub doc: String,
  #[serde(default)]
  pub examples: Vec<Cirru>,
  pub code: Cirru,
  #[serde(default = "schema_serde::default_schema", with = "schema_serde")]
  pub schema: Arc<CalcitTypeAnnotation>,
}

impl TryFrom<Edn> for CodeEntry {
  type Error = String;
  fn try_from(data: Edn) -> Result<Self, String> {
    let mut doc = String::new();
    let mut examples: Vec<Cirru> = vec![];
    let mut code: Option<Cirru> = None;
    let mut schema: Arc<CalcitTypeAnnotation> = DYNAMIC_TYPE.clone();

    match data {
      Edn::Record(record) => {
        for (key, value) in &record.pairs {
          match key.arc_str().as_ref() {
            "doc" => {
              doc = from_edn(value.to_owned()).map_err(|e| format!("failed to parse CodeEntry.doc: {e}"))?;
            }
            "examples" => {
              examples = from_edn(value.to_owned()).map_err(|e| format!("failed to parse CodeEntry.examples: {e}"))?;
            }
            "code" => {
              code = Some(from_edn(value.to_owned()).map_err(|e| format!("failed to parse CodeEntry.code: {e}"))?);
            }
            "schema" => {
              if !matches!(value, Edn::Nil) {
                schema = parse_loaded_schema_annotation(value, "CodeEntry.schema")?;
              }
            }
            _ => {}
          }
        }
      }
      Edn::Map(map) => {
        if let Some(value) = map.get(&Edn::Tag(EdnTag::new("doc"))) {
          doc = from_edn(value.to_owned()).map_err(|e| format!("failed to parse CodeEntry.doc: {e}"))?;
        }
        if let Some(value) = map.get(&Edn::Tag(EdnTag::new("examples"))) {
          examples = from_edn(value.to_owned()).map_err(|e| format!("failed to parse CodeEntry.examples: {e}"))?;
        }
        if let Some(value) = map.get(&Edn::Tag(EdnTag::new("code"))) {
          code = Some(from_edn(value.to_owned()).map_err(|e| format!("failed to parse CodeEntry.code: {e}"))?);
        }
        if let Some(value) = map.get(&Edn::Tag(EdnTag::new("schema")))
          && !matches!(value, Edn::Nil)
        {
          schema = parse_loaded_schema_annotation(value, "CodeEntry.schema")?;
        }
      }
      other => {
        return Err(format!("failed to parse CodeEntry: expected record/map, got: {other:?}"));
      }
    }

    let code = code.ok_or_else(|| "failed to parse CodeEntry: missing code field".to_owned())?;
    let schema = normalize_schema_for_code(&code, &schema);

    Ok(CodeEntry {
      doc,
      examples,
      code,
      schema,
    })
  }
}

/// Normalize a schema Edn value.
/// Wrapped `(:: :fn ({} ...))` / `(:: :macro ({} ...))` forms are converted to a direct map Edn.
/// Direct map format is returned as-is.
fn normalize_schema_edn(value: &Edn) -> Result<Edn, String> {
  if matches!(value, Edn::Map(_)) {
    let Edn::Map(map) = value else { unreachable!() };
    let normalized = Edn::Map(normalize_schema_map(map));
    validate_schema_edn_no_legacy_quotes(&normalized)?;
    return Ok(normalized);
  }

  if let Edn::Tuple(view) = value
    && matches!(view.tag.as_ref(), Edn::Tag(tag) if matches!(tag.ref_str(), "fn" | "macro"))
    && let Some(Edn::Map(map)) = view.extra.first()
  {
    let mut normalized_map = normalize_schema_map(map);
    if normalized_map.tag_get("kind").is_none() && matches!(view.tag.as_ref(), Edn::Tag(tag) if tag.ref_str() == "macro") {
      normalized_map.insert_key("kind", Edn::tag("macro"));
    }
    let normalized = Edn::Map(normalized_map);
    validate_schema_edn_no_legacy_quotes(&normalized)?;
    return Ok(normalized);
  }

  Err(format!(
    "invalid schema format: expected wrapped `(:: :fn ({{}} ...))` / `(:: :macro ({{}} ...))` or a normalized schema map, got {}",
    format_edn_preview(value)
  ))
}

fn validate_schema_edn_no_legacy_quotes(value: &Edn) -> Result<(), String> {
  fn walk(value: &Edn, path: &mut Vec<String>) -> Result<(), String> {
    match value {
      Edn::Symbol(s) => {
        if s.starts_with('\'') {
          let inner = s.trim_start_matches('\'');
          return Err(format!(
            "invalid schema generic symbol `{s}` at {}. Use source syntax like `'{inner}`, but store it as plain EDN symbol `{inner}`.",
            schema_path_label(path)
          ));
        }
        Ok(())
      }
      Edn::List(xs) => {
        for (idx, item) in xs.0.iter().enumerate() {
          path.push(format!("[{idx}]"));
          walk(item, path)?;
          path.pop();
        }
        Ok(())
      }
      Edn::Map(map) => {
        for (k, v) in map.0.iter() {
          path.push(map_key_path_segment(k));
          walk(v, path)?;
          path.pop();
        }
        Ok(())
      }
      Edn::Tuple(view) => {
        path.push(".tag".to_owned());
        walk(view.tag.as_ref(), path)?;
        path.pop();
        for (idx, item) in view.extra.iter().enumerate() {
          path.push(format!("[{idx}]"));
          walk(item, path)?;
          path.pop();
        }
        Ok(())
      }
      Edn::Set(set) => {
        for (idx, item) in set.0.iter().enumerate() {
          path.push(format!("[#{idx}]"));
          walk(item, path)?;
          path.pop();
        }
        Ok(())
      }
      Edn::Record(record) => {
        let _ = record;
        Ok(())
      }
      _ => Ok(()),
    }
  }

  let mut path = vec![];
  walk(value, &mut path)
}

/// Convert a schema Edn value to Cirru for operations that require Cirru (validation, runtime).
/// Handles both old Quote-wrapped format and new direct map format.
pub fn schema_edn_to_cirru(value: &Edn) -> Result<Cirru, String> {
  parse_schema_cirru_from_edn(value)
}

fn parse_schema_cirru_from_edn(value: &Edn) -> Result<Cirru, String> {
  // Do not use `from_edn::<Cirru>` here: EDN symbols such as `Edn::Symbol("T")`
  // would become Cirru leaves like `'T`, while valid schema source should round-trip
  // through the parser into `(quote T)` / `'T` syntax without embedding quote
  // characters inside leaf names.
  let schema_text = cirru_edn::format(value, true).map_err(|e| format!("Failed to format schema EDN to Cirru: {e}"))?;
  let schema_nodes = cirru_parser::parse(&schema_text).map_err(|e| format!("Failed to parse schema Cirru from EDN text: {e}"))?;

  if schema_nodes.len() != 1 {
    return Err(format!(
      "Schema EDN should convert to exactly 1 Cirru expression, got {}",
      schema_nodes.len()
    ));
  }
  Ok(schema_nodes[0].to_owned())
}

pub fn parse_schema_data(schema: &Cirru) -> Result<(), String> {
  if let Cirru::List(items) = schema {
    if let Some(Cirru::Leaf(head)) = items.first() {
      if &**head == ":optional" {
        if items.len() != 2 {
          return Err("schema `:optional` expects exactly one payload".to_owned());
        }
        return parse_schema_data(&items[1]);
      }
      if &**head == "::" && items.len() == 3 && matches!(items.get(1), Some(Cirru::Leaf(tag)) if &**tag == ":optional") {
        return parse_schema_data(&items[2]);
      }
    }
  }

  let schema_text =
    cirru_parser::format(std::slice::from_ref(schema), true.into()).map_err(|e| format!("Failed to format schema to Cirru: {e}"))?;

  cirru_edn::parse(&schema_text).map_err(|e| format!("Failed to parse schema as Cirru EDN: {e}"))?;

  Ok(())
}

/// Convert a Cirru schema tree to a direct Edn value (not Quote-wrapped).
/// Used when serializing CodeEntry to file: the schema is stored as a native
/// EDN map instead of a quoted Cirru expression.
/// `cr edit format` normalises old quote-wrapped schemas to this format.
/// Returns `Edn::Nil` if conversion fails (should not happen for valid schemas).
pub fn schema_cirru_to_edn(schema: Cirru) -> Edn {
  let text = match cirru_parser::format(&[schema], true.into()) {
    Ok(t) => t,
    Err(_) => return Edn::Nil,
  };
  match cirru_edn::parse(&text) {
    Ok(edn) => edn,
    Err(_) => Edn::Nil,
  }
}

fn validate_schema_for_snapshot_write(owner: &str, schema: &Arc<CalcitTypeAnnotation>) -> Result<(), String> {
  let CalcitTypeAnnotation::Fn(fn_annot) = schema.as_ref() else {
    return Ok(());
  };

  let schema_edn = fn_annot.to_wrapped_schema_edn();
  let schema_text =
    cirru_edn::format(&schema_edn, true).map_err(|e| format!("{owner}: failed to format `:schema` for snapshot write: {e}"))?;
  let schema_nodes = cirru_parser::parse(&schema_text)
    .map_err(|e| format!("{owner}: failed to parse serialized `:schema` during snapshot write validation: {e}"))?;

  if schema_nodes.len() != 1 {
    return Err(format!(
      "{owner}: serialized `:schema` should produce exactly 1 Cirru expression, got {}",
      schema_nodes.len()
    ));
  }

  validate_schema_for_write(&schema_nodes[0])
    .map_err(|e| format!("{owner}: serialized `:schema` becomes invalid during snapshot write: {e}; schema={schema_text}"))
}

fn validate_snapshot_schemas_for_write(snapshot: &Snapshot) -> Result<(), String> {
  for (ns_name, file_data) in &snapshot.files {
    if ns_name.ends_with(".$meta") {
      continue;
    }

    for (def_name, code_entry) in &file_data.defs {
      validate_schema_for_snapshot_write(&format!("{ns_name}/{def_name}"), &code_entry.schema)?;
    }
  }

  Ok(())
}

fn validate_serialized_snapshot_content(content: &str) -> Result<(), String> {
  fn walk(node: &Cirru, path: &mut Vec<usize>) -> Result<(), String> {
    if let Cirru::List(items) = node {
      if let Some(Cirru::Leaf(head)) = items.first()
        && &**head == ":schema"
        && let Some(schema_node) = items.get(1)
      {
        if matches!(schema_node, Cirru::Leaf(s) if s.as_ref() == "nil") {
          return Ok(());
        }
        return validate_schema_for_write(schema_node)
          .map_err(|e| format!("serialized snapshot has invalid `:schema` at {path:?}: {e}"));
      }

      for (idx, item) in items.iter().enumerate() {
        path.push(idx);
        walk(item, path)?;
        path.pop();
      }
    }
    Ok(())
  }

  let nodes = cirru_parser::parse(content).map_err(|e| format!("Failed to parse serialized snapshot content: {e}"))?;
  let mut path = vec![];
  for (idx, node) in nodes.iter().enumerate() {
    path.push(idx);
    walk(node, &mut path)?;
    path.pop();
  }
  Ok(())
}

/// Valid top-level field names accepted in a schema map.
pub const VALID_SCHEMA_FIELDS: &[&str] = &[":kind", ":args", ":return", ":rest", ":generics"];

/// Recursively check a Cirru schema tree for deprecated `:nil` type annotations.
fn check_no_nil_type(node: &Cirru) -> Result<(), String> {
  match node {
    Cirru::Leaf(s) if s.as_ref() == ":nil" => Err(
      "`:nil` is no longer a valid schema type. Use `:unit` for functions returning nil/unit, or `:dynamic` for unknown types."
        .to_owned(),
    ),
    Cirru::List(items) => {
      for item in items.iter() {
        check_no_nil_type(item)?;
      }
      Ok(())
    }
    _ => Ok(()),
  }
}

/// Recursively check for symbols with excess leading single-quotes.
/// In schema source, a valid generic type variable is written as `'T`, so a single
/// leading quote in a leaf is valid, but `''T` and deeper are malformed.
fn check_no_excess_quotes(node: &Cirru) -> Result<(), String> {
  match node {
    Cirru::Leaf(s) => {
      // A leaf with one leading quote is valid schema source syntax for an EDN symbol.
      // More than one means the underlying symbol name itself also contains quote chars.
      let name = s.as_ref();
      if name.starts_with('\'') && !name.trim_start_matches('\'').is_empty() {
        let inner = name.trim_start_matches('\'');
        if name.chars().filter(|c| *c == '\'').count() > 1 {
          return Err(format!(
            "Type variable `{name}` has excess leading quotes. Use a single-quoted uppercase symbol like `'{inner}`."
          ));
        }
      }
      Ok(())
    }
    Cirru::List(items) => {
      for item in items.iter() {
        check_no_excess_quotes(item)?;
      }
      Ok(())
    }
  }
}

/// Recursively collect all type-variable names from a Cirru node.
/// A type variable is represented as `(quote Name)` in the Cirru AST,
/// i.e. the source form `'T` parses to `(quote T)`.
fn collect_type_vars(node: &Cirru, out: &mut HashSet<String>) {
  if let Cirru::List(items) = node {
    if items.len() == 2 {
      if let (Some(Cirru::Leaf(head)), Some(Cirru::Leaf(name))) = (items.first(), items.get(1)) {
        if head.as_ref() == "quote" {
          out.insert(name.to_string());
          return;
        }
      }
    }
    for item in items.iter() {
      collect_type_vars(item, out);
    }
  }
}

/// Extract the list of declared generic type-variable names from a `:generics` value node.
/// Accepts `([] 'T 'U ...)` — each `(quote X)` child is one variable.
fn parse_generics_vars(node: &Cirru) -> HashSet<String> {
  let mut vars = HashSet::new();
  if let Cirru::List(items) = node {
    // skip leading `[]` head if present
    let start = match items.first() {
      Some(Cirru::Leaf(s)) if s.as_ref() == "[]" => 1,
      _ => 0,
    };
    for item in items.iter().skip(start) {
      collect_type_vars(item, &mut vars);
    }
  }
  vars
}

/// Allowed primitive tag types usable as a bare leaf schema (e.g. `:string`, `:number`).
pub const PRIMITIVE_SCHEMA_TAGS: &[&str] = &[
  "bool", "number", "string", "symbol", "tag", "list", "map", "set", "fn", "tuple", "ref", "buffer", "dynamic", "unit",
];

/// Strict validation for schemas submitted via `cr edit schema`.
/// Ensures the schema is a `{}` map or wrapped `(:: :fn ({} ...))` form, has a recognised `:kind`, and contains
/// only permitted fields.  Loading (read-only) only requires the weaker
/// `parse_schema_data` check.
pub fn validate_schema_for_write(schema: &Cirru) -> Result<(), String> {
  let mut wrapped_kind: Option<&str> = None;
  let raw_items = match schema {
    Cirru::List(items) => items,
    Cirru::Leaf(s) => {
      // Allow known primitive type tags as a bare leaf schema (e.g. :string, :number).
      let tag_name = s.trim_start_matches(':');
      if PRIMITIVE_SCHEMA_TAGS.contains(&tag_name) {
        return Ok(());
      }
      return Err(format!(
        "Schema must be a `{{}}` map or `(:: :fn ({{}} ...))` / `(:: :macro ({{}} ...))`, got leaf: `{s}`. \
         Primitive type tags (e.g. `:string`, `:number`, `:bool`) are accepted."
      ));
    }
  };

  let items: &[Cirru] = if matches!(raw_items.first(), Some(Cirru::Leaf(head)) if head.as_ref() == "::") {
    if raw_items.len() != 3 {
      return Err("Wrapped schema `(:: :fn schema-map)` or `(:: :macro schema-map)` expects exactly 3 items".to_owned());
    }
    match (&raw_items[1], &raw_items[2]) {
      (Cirru::Leaf(tag), Cirru::List(inner_items)) if tag.as_ref() == ":fn" || tag.as_ref() == ":macro" => {
        wrapped_kind = Some(tag);
        inner_items
      }
      (Cirru::Leaf(tag), _) => {
        return Err(format!(
          "Wrapped schema tag must be `:fn` or `:macro`, got: `{tag}`. Example: `(:: :fn ({{}} (:args ([] :string)) (:return :bool)))`"
        ));
      }
      _ => return Err("Wrapped schema second item must be `:fn` or `:macro` and third item must be a `{}` map".to_owned()),
    }
  } else {
    raw_items
  };

  let Some(Cirru::Leaf(head)) = items.first() else {
    return Err("Schema must be a non-empty list starting with `{}`".to_owned());
  };

  if head.as_ref() != "{}" {
    return Err(format!(
      "Schema top-level must start with `{{}}` or be wrapped as `(:: :fn ({{}} ...))` / `(:: :macro ({{}} ...))`, got: `{head}`. \
       Example: `(:: :fn ({{}} (:args ([] :string)) (:return :bool)))`"
    ));
  }

  // EDN-level validity
  parse_schema_data(schema)?;

  // Reject deprecated :nil type annotation
  check_no_nil_type(schema)?;

  // Reject excess-quoted type variables like ''T.
  check_no_excess_quotes(schema)?;

  // Field-level validation
  let mut has_kind = wrapped_kind.is_some();
  for pair in items.iter().skip(1) {
    let Cirru::List(xs) = pair else {
      let text = cirru_parser::format(&[pair.clone()], true.into()).unwrap_or_else(|_| format!("{pair:?}"));
      return Err(format!("Each schema field must be a `(:key val)` pair list, got: {text}"));
    };

    if xs.len() < 2 {
      return Err(format!(
        "Schema field pair must have exactly 2 elements, got {} in: {xs:?}",
        xs.len()
      ));
    }

    let Some(Cirru::Leaf(key)) = xs.first() else {
      return Err(format!("Schema field key must be a leaf tag, got: {:?}", xs.first()));
    };

    if !VALID_SCHEMA_FIELDS.contains(&key.as_ref()) {
      return Err(format!(
        "Unknown schema field: `{key}`. Valid fields: {}",
        VALID_SCHEMA_FIELDS.join(", ")
      ));
    }

    if key.as_ref() == ":kind" {
      has_kind = true;
      match xs.get(1) {
        Some(Cirru::Leaf(val)) if val.as_ref() == ":fn" || val.as_ref() == ":macro" => {}
        Some(Cirru::Leaf(val)) => {
          return Err(format!("Schema `:kind` must be `:fn` or `:macro`, got: `{val}`"));
        }
        _ => return Err("Schema `:kind` value must be a leaf tag (`:fn` or `:macro`)".to_owned()),
      }

      if let (Some(wrapped), Some(Cirru::Leaf(val))) = (wrapped_kind, xs.get(1))
        && wrapped != val.as_ref()
      {
        return Err(format!(
          "Wrapped schema tag `{wrapped}` conflicts with inner `:kind {val}`; keep only one kind or make them match"
        ));
      }
    }
  }

  if !has_kind {
    return Err("Schema must have a `:kind` field (`:fn` or `:macro`), unless the wrapped tag already provides it".to_owned());
  }

  // --- Type-variable consistency check ---
  // Collect declared generics, args, and return from the schema pairs.
  let mut generics_node: Option<&Cirru> = None;
  let mut args_node: Option<&Cirru> = None;
  let mut return_node: Option<&Cirru> = None;
  let mut rest_node: Option<&Cirru> = None;

  for pair in items.iter().skip(1) {
    if let Cirru::List(xs) = pair {
      if let (Some(Cirru::Leaf(key)), Some(val)) = (xs.first(), xs.get(1)) {
        match key.as_ref() {
          ":generics" => generics_node = Some(val),
          ":args" => args_node = Some(val),
          ":return" => return_node = Some(val),
          ":rest" => rest_node = Some(val),
          _ => {}
        }
      }
    }
  }

  if let Some(gen_node) = generics_node {
    let declared: HashSet<String> = parse_generics_vars(gen_node);

    // Collect used type vars from :args, :return, :rest
    let mut used: HashSet<String> = HashSet::new();
    if let Some(node) = args_node {
      collect_type_vars(node, &mut used);
    }
    if let Some(node) = return_node {
      collect_type_vars(node, &mut used);
    }
    if let Some(node) = rest_node {
      collect_type_vars(node, &mut used);
    }

    // Every declared var must be used at least once
    for var in &declared {
      if !used.contains(var) {
        return Err(format!(
          "Generic type variable `'{var}` is declared in `:generics` but never used in `:args`, `:rest`, or `:return`."
        ));
      }
    }

    // Every used var must be declared in :generics
    for var in &used {
      if !declared.contains(var) {
        return Err(format!(
          "Type variable `'{var}` is used in `:args`/`:rest`/`:return` but not declared in `:generics`."
        ));
      }
    }
  } else {
    // No :generics — any type var usage is an error
    let mut used: HashSet<String> = HashSet::new();
    if let Some(node) = args_node {
      collect_type_vars(node, &mut used);
    }
    if let Some(node) = return_node {
      collect_type_vars(node, &mut used);
    }
    if let Some(node) = rest_node {
      collect_type_vars(node, &mut used);
    }
    if let Some(var) = used.iter().next() {
      return Err(format!("Type variable `'{var}` is used but no `:generics` field is declared."));
    }
  }

  Ok(())
}

impl From<CodeEntry> for Edn {
  fn from(data: CodeEntry) -> Self {
    let schema = normalize_schema_for_code(&data.code, &data.schema);
    let schema_edn: Edn = match schema.as_ref() {
      CalcitTypeAnnotation::Dynamic => Edn::Nil,
      CalcitTypeAnnotation::Fn(fn_annot) => fn_annot.to_wrapped_schema_edn(),
      other => {
        // Primitive type tag schema (e.g. :string, :number) — serialize as plain EDN tag.
        other.builtin_tag_name().map(Edn::tag).unwrap_or(Edn::Nil)
      }
    };
    Edn::record_from_pairs(
      "CodeEntry".into(),
      &[
        ("doc".into(), data.doc.into()),
        ("examples".into(), data.examples.into()),
        ("code".into(), data.code.into()),
        ("schema".into(), schema_edn),
      ],
    )
  }
}

impl From<&CodeEntry> for Edn {
  fn from(data: &CodeEntry) -> Self {
    let schema = normalize_schema_for_code(&data.code, &data.schema);
    let schema_edn: Edn = match schema.as_ref() {
      CalcitTypeAnnotation::Dynamic => Edn::Nil,
      CalcitTypeAnnotation::Fn(fn_annot) => fn_annot.to_wrapped_schema_edn(),
      other => {
        // Primitive type tag schema (e.g. :string, :number) — serialize as plain EDN tag.
        other.builtin_tag_name().map(Edn::tag).unwrap_or(Edn::Nil)
      }
    };
    Edn::record_from_pairs(
      "CodeEntry".into(),
      &[
        ("doc".into(), data.doc.to_owned().into()),
        ("examples".into(), data.examples.to_owned().into()),
        ("code".into(), data.code.to_owned().into()),
        ("schema".into(), schema_edn),
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
      schema: DYNAMIC_TYPE.clone(),
    }
  }
}

fn code_declares_macro(code: &Cirru) -> bool {
  matches!(code, Cirru::List(items) if matches!(items.first(), Some(Cirru::Leaf(head)) if head.as_ref() == "defmacro"))
}

fn normalize_schema_for_code(code: &Cirru, schema: &Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  let CalcitTypeAnnotation::Fn(fn_annot) = schema.as_ref() else {
    return schema.clone();
  };

  if !code_declares_macro(code) || matches!(fn_annot.fn_kind, SchemaKind::Macro) {
    return schema.clone();
  }

  Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
    generics: fn_annot.generics.clone(),
    arg_types: fn_annot.arg_types.clone(),
    return_type: fn_annot.return_type.clone(),
    fn_kind: SchemaKind::Macro,
    rest_type: fn_annot.rest_type.clone(),
  })))
}

/// structure of `compact.cirru` file
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
  pub package: String,
  pub about: Option<String>,
  pub configs: SnapshotConfigs,
  pub entries: HashMap<String, SnapshotConfigs>,
  pub files: HashMap<String, FileInSnapShot>,
}

impl TryFrom<Edn> for SnapshotConfigs {
  type Error = String;
  fn try_from(data: Edn) -> Result<SnapshotConfigs, String> {
    parse_snapshot_configs_with_context(data, "configs")
  }
}

fn parse_snapshot_config_string_field(data: &EdnMapView, key: &str, owner: &str) -> Result<String, String> {
  let value = data.get(&Edn::tag(key)).ok_or_else(|| format!("{owner}: missing `:{key}` field"))?;

  let text: Arc<str> = value
    .to_owned()
    .try_into()
    .map_err(|e| format!("{owner}.{key}: {e}; got {}", format_edn_preview(value)))?;

  if key == "version" && (text.trim().is_empty() || text.as_ref() == "|") {
    return Err(format!(
      "{owner}.version cannot be empty; check `:configs (:version ...)`; got {}",
      format_edn_preview(value)
    ));
  }

  Ok(text.to_string())
}

fn parse_snapshot_configs_with_context(data: Edn, owner: &str) -> Result<SnapshotConfigs, String> {
  let data = data
    .view_map()
    .map_err(|e| format!("{owner}: failed to parse config map: {e}; got {}", format_edn_preview(&data)))?;

  let init_fn = parse_snapshot_config_string_field(&data, "init-fn", owner)?;
  let reload_fn = parse_snapshot_config_string_field(&data, "reload-fn", owner)?;

  let version = match data.get(&Edn::tag("version")) {
    Some(_) => parse_snapshot_config_string_field(&data, "version", owner)?,
    None => default_version(),
  };

  let modules = match data.get(&Edn::tag("modules")) {
    Some(value) => from_edn(value.to_owned()).map_err(|e| format!("{owner}.modules: {e}; got {}", format_edn_preview(value)))?,
    None => Vec::new(),
  };

  Ok(SnapshotConfigs {
    init_fn,
    reload_fn,
    modules,
    version,
  })
}

fn parse_entries_with_context(data: &Edn) -> Result<HashMap<String, SnapshotConfigs>, String> {
  let entries_map = data
    .view_map()
    .map_err(|e| format!("entries: failed to parse entries map: {e}; got {}", format_edn_preview(data)))?;

  let mut entries = HashMap::with_capacity(entries_map.0.len());
  for (entry_key, entry_value) in entries_map.0.iter() {
    let entry_name: String = from_edn(entry_key.to_owned())
      .map_err(|e| format!("entries: failed to parse entry name: {e}; got {}", format_edn_preview(entry_key)))?;
    let owner = format!("entries.{entry_name}");
    let entry = parse_snapshot_configs_with_context(entry_value.to_owned(), &owner)?;
    entries.insert(entry_name, entry);
  }

  Ok(entries)
}

/// parse snapshot
pub fn load_snapshot_data(data: &Edn, path: &str) -> Result<Snapshot, String> {
  let data = data.view_map()?;
  let pkg: Arc<str> = data.get_or_nil("package").try_into()?;
  let mut files: HashMap<String, FileInSnapShot> = parse_files_with_context(&data.get_or_nil("files"))?;
  let about = match data.get_or_nil("about") {
    Edn::Nil => None,
    value => {
      let s: Arc<str> = value.try_into()?;
      Some(s.to_string())
    }
  };
  let meta_ns = format!("{pkg}.$meta");
  files.insert(meta_ns.to_owned(), gen_meta_ns(&meta_ns, path));
  let s = Snapshot {
    package: pkg.to_string(),
    about,
    configs: parse_snapshot_configs_with_context(data.get_or_nil("configs"), "configs")?,
    entries: parse_entries_with_context(&data.get_or_nil("entries"))?,
    files,
  };
  Ok(s)
}

fn parse_code_entry_with_context(data: Edn, owner: &str) -> Result<CodeEntry, String> {
  with_type_annotation_warning_context(owner.to_owned(), || data.try_into()).map_err(|e| format!("{owner}: {e}"))
}

fn parse_file_in_snapshot_with_context(data: Edn, file_name: &str) -> Result<FileInSnapShot, String> {
  match data {
    Edn::Map(map) => {
      let ns_value = map
        .get(&Edn::tag("ns"))
        .ok_or_else(|| format!("{file_name}: missing `:ns` field in FileEntry"))?;
      let defs_value = map
        .get(&Edn::tag("defs"))
        .ok_or_else(|| format!("{file_name}: missing `:defs` field in FileEntry"))?;

      let ns: NsEntry = ns_value
        .to_owned()
        .try_into()
        .map_err(|e: String| format!("{file_name}/:ns: {e}"))?;
      let defs_map = defs_value.view_map().map_err(|e| {
        format!(
          "{file_name}: failed to parse `:defs` as map: {e}; got {}",
          format_edn_preview(defs_value)
        )
      })?;

      let mut defs = HashMap::with_capacity(defs_map.0.len());
      for (def_key, def_value) in defs_map.0.iter() {
        let def_name: String = from_edn(def_key.to_owned())
          .map_err(|e| format!("{file_name}: failed to parse def name: {e}; got {}", format_edn_preview(def_key)))?;
        let owner = format!("{file_name}/{def_name}");
        defs.insert(def_name, parse_code_entry_with_context(def_value.to_owned(), &owner)?);
      }

      Ok(FileInSnapShot { ns, defs })
    }
    Edn::Record(record) => {
      let mut ns: Option<NsEntry> = None;
      let mut defs = HashMap::new();

      for (key, value) in record.pairs.iter() {
        match key.arc_str().as_ref() {
          "ns" => {
            ns = Some(value.to_owned().try_into().map_err(|e: String| format!("{file_name}/:ns: {e}"))?);
          }
          "defs" => {
            let defs_map = value.view_map().map_err(|e| {
              format!(
                "{file_name}: failed to parse `:defs` as map: {e}; got {}",
                format_edn_preview(value)
              )
            })?;
            for (def_key, def_value) in defs_map.0.iter() {
              let def_name: String = from_edn(def_key.to_owned())
                .map_err(|e| format!("{file_name}: failed to parse def name: {e}; got {}", format_edn_preview(def_key)))?;
              let owner = format!("{file_name}/{def_name}");
              defs.insert(def_name, parse_code_entry_with_context(def_value.to_owned(), &owner)?);
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
    other => Err(format!(
      "{file_name}: expected FileEntry map/record, got {}",
      format_edn_preview(&other)
    )),
  }
}

fn parse_files_with_context(data: &Edn) -> Result<HashMap<String, FileInSnapShot>, String> {
  let files_map = data
    .view_map()
    .map_err(|e| format!("failed to parse snapshot `:files` as map: {e}; got {}", format_edn_preview(data)))?;
  let mut files = HashMap::with_capacity(files_map.0.len());
  for (file_key, file_value) in files_map.0.iter() {
    let file_name: String = from_edn(file_key.to_owned())
      .map_err(|e| format!("failed to parse snapshot file key: {e}; got {}", format_edn_preview(file_key)))?;
    files.insert(
      file_name.clone(),
      parse_file_in_snapshot_with_context(file_value.to_owned(), &file_name)?,
    );
  }
  Ok(files)
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
    ns: NsEntry {
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
      about: Some(SNAPSHOT_ABOUT_MESSAGE.to_string()),
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
      let mut ns_code: Cirru = vec!["ns", "app.main"].into();
      let mut body_start = 0;
      if let Some(Cirru::List(items)) = lines.first()
        && let Some(Cirru::Leaf(head)) = items.first()
        && &**head == "ns"
      {
        if items.len() < 2 {
          return Err("Invalid `ns` expression in snippet: expected namespace after `ns`".to_string());
        }
        let mut merged_ns = vec![Cirru::leaf("ns"), Cirru::leaf("app.main")];
        merged_ns.extend(items.iter().skip(2).cloned());
        ns_code = Cirru::List(merged_ns);
        body_start = 1;
      }

      let mut def_dict: HashMap<String, CodeEntry> = HashMap::with_capacity(2);
      let mut func_code = vec![Cirru::leaf("defn"), "main!".into(), Cirru::List(vec![])];
      for line in lines.into_iter().skip(body_start) {
        func_code.push(line.to_owned());
      }
      def_dict.insert("main!".into(), CodeEntry::from_code(Cirru::List(func_code)));
      def_dict.insert(
        "reload!".into(),
        CodeEntry::from_code(vec![Cirru::leaf("defn"), "reload!".into(), Cirru::List(vec![])].into()),
      );
      Ok(FileInSnapShot {
        ns: NsEntry {
          doc: "".to_owned(),
          code: ns_code,
        },
        defs: def_dict,
      })
    }
    Err(e) => {
      eprintln!("\nFailed to parse code snippet:");
      eprintln!("{}", e.format_detailed(Some(raw)));
      Err("Failed to parse code snippet".to_string())
    }
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
    let mut map = EdnMapView::default();
    map.insert_key("added", x.added.into());
    map.insert_key("changed", x.changed.into());
    map.insert_key("removed", x.removed.into());
    Ok(Edn::Map(map))
  }
}

/// Save snapshot to compact.cirru file
/// This is a shared utility function used by CLI edit commands
pub fn render_snapshot_content(snapshot: &Snapshot) -> Result<String, String> {
  validate_snapshot_schemas_for_write(snapshot)?;

  // Build root level Edn mapping
  let mut edn_map = EdnMapView::default();

  // Build package
  edn_map.insert_key("package", Edn::Str(snapshot.package.as_str().into()));

  // Insert about message (always enforce canonical hint)
  edn_map.insert_key("about", Edn::Str(SNAPSHOT_ABOUT_MESSAGE.into()));

  // Build configs
  let mut configs_map = EdnMapView::default();
  configs_map.insert_key("init-fn", Edn::Str(snapshot.configs.init_fn.as_str().into()));
  configs_map.insert_key("reload-fn", Edn::Str(snapshot.configs.reload_fn.as_str().into()));
  configs_map.insert_key("version", Edn::Str(snapshot.configs.version.as_str().into()));
  configs_map.insert_key(
    "modules",
    Edn::from(
      snapshot
        .configs
        .modules
        .iter()
        .map(|s| Edn::Str(s.as_str().into()))
        .collect::<Vec<_>>(),
    ),
  );
  edn_map.insert_key("configs", configs_map.into());

  // Build entries
  let mut entries_map = EdnMapView::default();
  for (k, v) in &snapshot.entries {
    let mut entry_map = EdnMapView::default();
    entry_map.insert_key("init-fn", Edn::Str(v.init_fn.as_str().into()));
    entry_map.insert_key("reload-fn", Edn::Str(v.reload_fn.as_str().into()));
    entry_map.insert_key("version", Edn::Str(v.version.as_str().into()));
    entry_map.insert_key(
      "modules",
      Edn::from(v.modules.iter().map(|s| Edn::Str(s.as_str().into())).collect::<Vec<_>>()),
    );
    entries_map.insert_key(k.as_str(), entry_map.into());
  }
  edn_map.insert_key("entries", entries_map.into());

  // Build files
  let mut files_map = EdnMapView::default();
  for (k, v) in &snapshot.files {
    // Skip $meta namespaces as they are special and should not be serialized to file
    if k.ends_with(".$meta") {
      continue;
    }
    files_map.insert(Edn::str(k.as_str()), Edn::from(v));
  }
  edn_map.insert_key("files", files_map.into());

  let edn_data = Edn::from(edn_map);

  // Normalize on AST directly, avoiding parse-after-format roundtrip.
  let normalized = normalize_pipe_prefixed_leaf(edn_data.cirru());
  let content = cirru_parser::format(std::slice::from_ref(&normalized), true.into())
    .map_err(|e| format!("Failed to format snapshot as Cirru: {e}"))?;

  validate_serialized_snapshot_content(&content)?;

  Ok(content)
}

fn normalize_pipe_prefixed_leaf(node: Cirru) -> Cirru {
  match node {
    Cirru::Leaf(token) => {
      if let Some(rest) = token.strip_prefix('"') {
        Cirru::leaf(format!("|{rest}"))
      } else {
        Cirru::Leaf(token)
      }
    }
    Cirru::List(items) => Cirru::List(items.into_iter().map(normalize_pipe_prefixed_leaf).collect()),
  }
}

/// Save snapshot to compact.cirru file
/// This is a shared utility function used by CLI edit commands
pub fn save_snapshot_to_file<P: AsRef<Path>>(compact_cirru_path: P, snapshot: &Snapshot) -> Result<(), String> {
  let content = render_snapshot_content(snapshot)?;

  // Write to file
  std::fs::write(compact_cirru_path, content).map_err(|e| format!("Failed to write compact.cirru: {e}"))?;

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::CalcitFnTypeAnnotation;
  use cirru_edn::EdnListView;

  use std::fs;

  #[test]
  fn normalizes_simple_quoted_tokens_to_pipe_prefix() {
    let input = "{} (:a \"|&\") (:b \"|56px\") (:c \"|hello-world\")";
    let nodes = cirru_parser::parse(input).expect("input should parse");
    let output_node = normalize_pipe_prefixed_leaf(nodes[0].to_owned());
    let output = cirru_parser::format(std::slice::from_ref(&output_node), true.into()).expect("output should format");
    assert_eq!(output.trim(), "{} (:a |&) (:b |56px) (:c |hello-world)");
  }

  #[test]
  fn normalizes_all_quote_prefixed_leaves_from_ast() {
    let input = "{} (:a \"|hello world\") (:b \"|line\\nfeed\") (:c \"|x(y)\")";
    let nodes = cirru_parser::parse(input).expect("input should parse");
    let output_node = normalize_pipe_prefixed_leaf(nodes[0].to_owned());
    let output = cirru_parser::format(std::slice::from_ref(&output_node), true.into()).expect("output should format");

    let nodes = cirru_parser::parse(&output).expect("normalized output should still be parseable");
    let Cirru::List(root_items) = &nodes[0] else {
      panic!("expected one root list");
    };

    for pair in root_items.iter().skip(1) {
      let Cirru::List(pair_items) = pair else {
        continue;
      };
      if pair_items.len() < 2 {
        continue;
      }
      let Cirru::Leaf(value) = &pair_items[1] else {
        continue;
      };
      assert!(
        value.starts_with('|'),
        "expected string leaf to be normalized to pipe-prefix in AST, got: {value}"
      );
    }
  }

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
      ("*", 6),
      ("/", 2),
      ("map", 2),
      ("filter", 2),
      ("first", 3),
      ("count", 3),
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
      schema: {
        let schema_edn = schema_cirru_to_edn(Cirru::List(vec![
          Cirru::leaf("{}"),
          Cirru::List(vec![Cirru::leaf(":kind"), Cirru::leaf(":fn")]),
          Cirru::List(vec![Cirru::leaf(":name"), Cirru::leaf("'add")]),
          Cirru::List(vec![Cirru::leaf(":args"), Cirru::List(vec![Cirru::leaf("[]")])]),
          Cirru::List(vec![Cirru::leaf(":return"), Cirru::leaf(":number")]),
        ]));
        CalcitTypeAnnotation::parse_fn_schema_from_edn(&schema_edn)
          .map(|s| std::sync::Arc::new(CalcitTypeAnnotation::Fn(std::sync::Arc::new(s))))
          .unwrap_or_else(|| DYNAMIC_TYPE.clone())
      },
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

  #[test]
  fn test_parse_schema_data_valid_and_invalid() {
    let valid = Cirru::List(vec![
      Cirru::leaf("{}"),
      Cirru::List(vec![Cirru::leaf(":kind"), Cirru::leaf(":fn")]),
      Cirru::List(vec![Cirru::leaf(":name"), Cirru::leaf("'demo")]),
      Cirru::List(vec![Cirru::leaf(":args"), Cirru::List(vec![Cirru::leaf("[]")])]),
      Cirru::List(vec![Cirru::leaf(":return"), Cirru::leaf(":dynamic")]),
    ]);
    assert!(parse_schema_data(&valid).is_ok());

    let missing_return = Cirru::List(vec![
      Cirru::leaf("{}"),
      Cirru::List(vec![Cirru::leaf(":kind"), Cirru::leaf(":fn")]),
      Cirru::List(vec![Cirru::leaf(":name"), Cirru::leaf("'demo")]),
      Cirru::List(vec![Cirru::leaf(":args"), Cirru::List(vec![Cirru::leaf("[]")])]),
    ]);
    assert!(parse_schema_data(&missing_return).is_ok());

    let optional_wrapped = Cirru::List(vec![Cirru::leaf(":optional"), valid.clone()]);
    assert!(parse_schema_data(&optional_wrapped).is_ok());

    let optional_wrapped_by_tuple = Cirru::List(vec![Cirru::leaf("::"), Cirru::leaf(":optional"), valid]);
    assert!(parse_schema_data(&optional_wrapped_by_tuple).is_ok());

    let invalid_edn = Cirru::List(vec![Cirru::leaf("~"), Cirru::leaf("x")]);
    assert!(parse_schema_data(&invalid_edn).is_err());
  }

  #[test]
  fn test_validate_schema_for_write() {
    let valid = Cirru::List(vec![
      Cirru::leaf("{}"),
      Cirru::List(vec![Cirru::leaf(":kind"), Cirru::leaf(":fn")]),
      Cirru::List(vec![
        Cirru::leaf(":args"),
        Cirru::List(vec![Cirru::leaf("[]"), Cirru::leaf(":string")]),
      ]),
      Cirru::List(vec![Cirru::leaf(":return"), Cirru::leaf(":bool")]),
    ]);
    assert!(validate_schema_for_write(&valid).is_ok(), "valid schema should pass");

    let wrapped_macro = Cirru::List(vec![
      Cirru::leaf("::"),
      Cirru::leaf(":macro"),
      Cirru::List(vec![
        Cirru::leaf("{}"),
        Cirru::List(vec![
          Cirru::leaf(":args"),
          Cirru::List(vec![Cirru::leaf("[]"), Cirru::leaf(":dynamic")]),
        ]),
        Cirru::List(vec![Cirru::leaf(":return"), Cirru::leaf(":dynamic")]),
      ]),
    ]);
    assert!(
      validate_schema_for_write(&wrapped_macro).is_ok(),
      "wrapped macro schema should pass"
    );

    // Missing :kind
    let no_kind = Cirru::List(vec![
      Cirru::leaf("{}"),
      Cirru::List(vec![Cirru::leaf(":args"), Cirru::List(vec![Cirru::leaf("[]")])]),
    ]);
    assert!(validate_schema_for_write(&no_kind).is_err(), "missing :kind should fail");

    // Unknown field
    let unknown_field = Cirru::List(vec![
      Cirru::leaf("{}"),
      Cirru::List(vec![Cirru::leaf(":kind"), Cirru::leaf(":fn")]),
      Cirru::List(vec![Cirru::leaf(":foobar"), Cirru::leaf(":dynamic")]),
    ]);
    assert!(validate_schema_for_write(&unknown_field).is_err(), "unknown field should fail");

    // Bad :kind value
    let bad_kind = Cirru::List(vec![
      Cirru::leaf("{}"),
      Cirru::List(vec![Cirru::leaf(":kind"), Cirru::leaf(":something-else")]),
    ]);
    assert!(validate_schema_for_write(&bad_kind).is_err(), "bad :kind value should fail");

    // Primitive type tag leaves are now accepted.
    let leaf_string = Cirru::Leaf(Arc::from(":string"));
    assert!(validate_schema_for_write(&leaf_string).is_ok(), ":string leaf should pass");
    let leaf_fn = Cirru::Leaf(Arc::from(":fn"));
    assert!(validate_schema_for_write(&leaf_fn).is_ok(), ":fn leaf should pass");
    let leaf_number = Cirru::Leaf(Arc::from(":number"));
    assert!(validate_schema_for_write(&leaf_number).is_ok(), ":number leaf should pass");

    // Unknown leaf (not a known primitive type) must still fail.
    let leaf_unknown = Cirru::Leaf(Arc::from(":not-a-type"));
    assert!(validate_schema_for_write(&leaf_unknown).is_err(), "unknown leaf should fail");

    // Wrong head (still quote-wrapped - must be unwrapped by caller first)
    let quote_wrapped = Cirru::List(vec![
      Cirru::leaf("quote"),
      Cirru::List(vec![Cirru::leaf("{}"), Cirru::List(vec![Cirru::leaf(":kind"), Cirru::leaf(":fn")])]),
    ]);
    assert!(
      validate_schema_for_write(&quote_wrapped).is_err(),
      "quote-wrapped should fail (caller must unwrap)"
    );
  }

  #[test]
  fn test_typevar_consistency_validation() {
    // Helper: make a (quote X) node representing 'X type var
    fn quote(name: &str) -> Cirru {
      Cirru::List(vec![Cirru::leaf("quote"), Cirru::leaf(name)])
    }

    // Valid: 'T declared and used in both args and return
    let valid_generic = Cirru::List(vec![
      Cirru::leaf("{}"),
      Cirru::List(vec![Cirru::leaf(":kind"), Cirru::leaf(":fn")]),
      Cirru::List(vec![Cirru::leaf(":generics"), Cirru::List(vec![Cirru::leaf("[]"), quote("T")])]),
      Cirru::List(vec![
        Cirru::leaf(":args"),
        Cirru::List(vec![
          Cirru::leaf("[]"),
          Cirru::List(vec![Cirru::leaf("::"), Cirru::leaf(":list"), quote("T")]),
        ]),
      ]),
      Cirru::List(vec![Cirru::leaf(":return"), quote("T")]),
    ]);
    assert!(validate_schema_for_write(&valid_generic).is_ok(), "valid generics should pass");

    // Invalid: 'K used in :return but not declared in :generics
    let undeclared = Cirru::List(vec![
      Cirru::leaf("{}"),
      Cirru::List(vec![Cirru::leaf(":kind"), Cirru::leaf(":fn")]),
      Cirru::List(vec![Cirru::leaf(":generics"), Cirru::List(vec![Cirru::leaf("[]"), quote("T")])]),
      Cirru::List(vec![
        Cirru::leaf(":args"),
        Cirru::List(vec![
          Cirru::leaf("[]"),
          Cirru::List(vec![Cirru::leaf("::"), Cirru::leaf(":list"), quote("T")]),
        ]),
      ]),
      Cirru::List(vec![Cirru::leaf(":return"), quote("K")]),
    ]);
    assert!(
      validate_schema_for_write(&undeclared).is_err(),
      "undeclared type var 'K should fail"
    );

    // Invalid: 'U declared but never used
    let unused_declared = Cirru::List(vec![
      Cirru::leaf("{}"),
      Cirru::List(vec![Cirru::leaf(":kind"), Cirru::leaf(":fn")]),
      Cirru::List(vec![
        Cirru::leaf(":generics"),
        Cirru::List(vec![Cirru::leaf("[]"), quote("T"), quote("U")]),
      ]),
      Cirru::List(vec![
        Cirru::leaf(":args"),
        Cirru::List(vec![
          Cirru::leaf("[]"),
          Cirru::List(vec![Cirru::leaf("::"), Cirru::leaf(":list"), quote("T")]),
        ]),
      ]),
      Cirru::List(vec![Cirru::leaf(":return"), quote("T")]),
    ]);
    assert!(
      validate_schema_for_write(&unused_declared).is_err(),
      "unused declared 'U should fail"
    );

    // Invalid: type var used without any :generics
    let typevar_no_generics = Cirru::List(vec![
      Cirru::leaf("{}"),
      Cirru::List(vec![Cirru::leaf(":kind"), Cirru::leaf(":fn")]),
      Cirru::List(vec![Cirru::leaf(":args"), Cirru::List(vec![Cirru::leaf("[]"), quote("T")])]),
      Cirru::List(vec![Cirru::leaf(":return"), quote("T")]),
    ]);
    assert!(
      validate_schema_for_write(&typevar_no_generics).is_err(),
      "type var without :generics should fail"
    );
  }

  #[test]
  fn test_schema_cirru_to_edn_no_quote_wrapper() {
    let schema = Cirru::List(vec![
      Cirru::leaf("{}"),
      Cirru::List(vec![Cirru::leaf(":kind"), Cirru::leaf(":fn")]),
      Cirru::List(vec![Cirru::leaf(":return"), Cirru::leaf(":string")]),
    ]);
    let edn = schema_cirru_to_edn(schema);
    assert!(!matches!(edn, Edn::Nil), "should not produce Nil for valid schema");
    assert!(
      !matches!(edn, Edn::Quote(_)),
      "output must NOT be Quote-wrapped (new direct-map format)"
    );
  }

  #[test]
  fn test_schema_generics_round_trip_uses_single_quote_source_syntax() {
    let schema_text = "{} (:kind :fn) (:args ([] :number)) (:generics ([] 'T)) (:return :number)";
    let schema_cirru = cirru_parser::parse(schema_text)
      .expect("should parse")
      .into_iter()
      .next()
      .expect("should have one node");

    let schema_edn = schema_cirru_to_edn(schema_cirru);
    let fn_schema = CalcitTypeAnnotation::parse_fn_schema_from_edn(&schema_edn).expect("must parse generic schema");
    assert_eq!(fn_schema.generics.as_ref(), &[Arc::from("T")]);

    let saved_edn = fn_schema.to_schema_edn();
    let Edn::Map(saved_map) = &saved_edn else {
      panic!("saved schema must be a map, got {saved_edn:?}");
    };
    let Some(Edn::List(generics)) = saved_map.tag_get("generics") else {
      panic!("saved schema must contain :generics, got {saved_edn:?}");
    };
    assert_eq!(generics.0, vec![Edn::Symbol(Arc::from("T"))]);

    let saved_cirru = schema_edn_to_cirru(&saved_edn).expect("schema edn to cirru");
    validate_schema_for_write(&saved_cirru).expect("saved schema should still be writable");
    let saved_text = cirru_parser::format(&[saved_cirru], true.into()).expect("format schema");
    assert!(
      saved_text.contains(":generics $ [] 'T"),
      "saved schema should use single-quoted source syntax: {saved_text}"
    );
    assert!(
      !saved_text.contains("''T"),
      "saved schema must not contain double-leading-quote generics: {saved_text}"
    );
  }

  #[test]
  fn test_schema_named_type_refs_round_trip_without_becoming_type_vars() {
    let schema_text = "{} (:kind :fn) (:generics ([] 'T 'E)) (:args ([] 'T)) (:return (:: 'Result 'T 'E))";
    let schema_cirru = cirru_parser::parse(schema_text)
      .expect("should parse")
      .into_iter()
      .next()
      .expect("should have one node");

    let schema_edn = schema_cirru_to_edn(schema_cirru);
    let fn_schema = CalcitTypeAnnotation::parse_fn_schema_from_edn(&schema_edn).expect("must parse named ref schema");

    assert!(
      matches!(fn_schema.arg_types.first().map(|t| t.as_ref()), Some(CalcitTypeAnnotation::TypeVar(name)) if name.as_ref() == "T")
    );
    assert!(
      matches!(fn_schema.return_type.as_ref(), CalcitTypeAnnotation::TypeRef(name, args) if name.as_ref() == "Result" && args.len() == 2)
    );

    let saved_text = cirru_parser::format(
      &[schema_edn_to_cirru(&fn_schema.to_schema_edn()).expect("schema edn to cirru")],
      true.into(),
    )
    .expect("format schema");
    assert!(
      saved_text.contains(":return $ :: 'Result 'T 'E"),
      "saved schema should keep named type reference syntax: {saved_text}"
    );
  }

  #[test]
  fn test_normalize_schema_rejects_legacy_quoted_generic_symbol() {
    let schema = Edn::Map(EdnMapView::from(HashMap::from([
      (Edn::tag("kind"), Edn::tag("fn")),
      (Edn::tag("args"), Edn::List(cirru_edn::EdnListView(vec![Edn::tag("number")]))),
      (
        Edn::tag("generics"),
        Edn::List(cirru_edn::EdnListView(vec![Edn::Symbol(Arc::from("'T"))])),
      ),
      (Edn::tag("return"), Edn::tag("number")),
    ])));

    let err = normalize_schema_edn(&schema).expect_err("legacy quoted generic symbol should fail on load");
    assert!(err.contains("invalid schema generic symbol"), "unexpected error: {err}");
  }

  #[test]
  fn test_schema_write_rejects_double_quoted_generics() {
    let schema_text = "{} (:kind :fn) (:args ([] :number)) (:generics ([] ''T)) (:return :number)";
    let schema_cirru = cirru_parser::parse(schema_text)
      .expect("should parse")
      .into_iter()
      .next()
      .expect("should have one node");

    let err = validate_schema_for_write(&schema_cirru).expect_err("double-quoted generic should be rejected");
    assert!(err.contains("excess leading quotes"), "unexpected error: {err}");
  }

  #[test]
  fn test_normalize_schema_rejects_quoted_singleton_list() {
    let quoted = Edn::Quote(Cirru::List(vec![
      Cirru::leaf("[]"),
      Cirru::List(vec![
        Cirru::leaf("{}"),
        Cirru::List(vec![Cirru::leaf(":kind"), Cirru::leaf(":fn")]),
        Cirru::List(vec![Cirru::leaf(":args"), Cirru::List(vec![Cirru::leaf("[]")])]),
        Cirru::List(vec![Cirru::leaf(":return"), Cirru::leaf(":dynamic")]),
      ]),
    ]));

    let err = normalize_schema_edn(&quoted).expect_err("legacy quoted schema should be rejected");
    assert!(err.contains("invalid schema"), "unexpected error: {err}");
  }

  #[test]
  fn test_normalize_schema_unwraps_wrapped_fn_tuple() {
    let wrapped = Edn::tuple(
      Edn::tag("fn"),
      vec![Edn::Map(EdnMapView::from(HashMap::from([
        (Edn::tag("kind"), Edn::tag("fn")),
        (Edn::tag("args"), Edn::List(EdnListView(vec![]))),
        (Edn::tag("return"), Edn::tag("dynamic")),
      ])))],
    );

    let normalized = normalize_schema_edn(&wrapped).expect("wrapped schema should normalize");
    let Edn::Map(map) = normalized else {
      panic!("normalized schema should be a map");
    };
    assert!(matches!(map.tag_get("kind"), Some(Edn::Tag(tag)) if tag.ref_str() == "fn"));
  }

  #[test]
  fn test_normalize_schema_unwraps_wrapped_macro_tuple() {
    let wrapped = Edn::tuple(
      Edn::tag("macro"),
      vec![Edn::Map(EdnMapView::from(HashMap::from([
        (Edn::tag("args"), Edn::List(EdnListView(vec![]))),
        (Edn::tag("return"), Edn::tag("dynamic")),
      ])))],
    );

    let normalized = normalize_schema_edn(&wrapped).expect("wrapped macro schema should normalize");
    let Edn::Map(map) = normalized else {
      panic!("normalized schema should be a map");
    };
    assert!(matches!(map.tag_get("kind"), Some(Edn::Tag(tag)) if tag.ref_str() == "macro"));
  }

  #[test]
  fn test_normalize_schema_canonicalizes_string_keys_and_kind_values() {
    let wrapped = Edn::tuple(
      Edn::tag("fn"),
      vec![Edn::Map(EdnMapView::from(HashMap::from([
        (Edn::Str(Arc::from(":args")), Edn::List(EdnListView(vec![Edn::tag("set")]))),
        (Edn::Str(Arc::from(":return")), Edn::tag("bool")),
        (Edn::Str(Arc::from(":kind")), Edn::Str(Arc::from(":fn"))),
      ])))],
    );

    let normalized = normalize_schema_edn(&wrapped).expect("string-key schema should normalize");
    let Edn::Map(map) = normalized else {
      panic!("normalized schema should be a map");
    };

    assert!(matches!(map.tag_get("args"), Some(Edn::List(_))));
    assert!(matches!(map.tag_get("return"), Some(Edn::Tag(tag)) if tag.ref_str() == "bool"));
    assert!(matches!(map.tag_get("kind"), Some(Edn::Tag(tag)) if tag.ref_str() == "fn"));
    assert!(CalcitTypeAnnotation::parse_fn_schema_from_edn(&Edn::Map(map)).is_some());
  }

  #[test]
  fn test_macro_schema_full_file_round_trip() {
    use crate::calcit::SchemaKind;
    // Simulate saving + loading via the actual file format:
    // 1. Write a CodeEntry with :kind :macro schema to Edn (as done by save_snapshot_to_file)
    // 2. Format it to Cirru string (via cirru_edn::format)
    // 3. Parse it back (via cirru_edn::parse)
    // 4. TryFrom<Edn> for CodeEntry
    // 5. Check entry.schema is Fn with fn_kind: Macro

    let schema_text = "{} (:kind :macro) (:return :bool) (:args ([] :number :number))";
    let schema_cirru = cirru_parser::parse(schema_text)
      .expect("should parse")
      .into_iter()
      .next()
      .expect("should have one node");
    let schema_edn = schema_cirru_to_edn(schema_cirru);

    let fn_schema = CalcitTypeAnnotation::parse_fn_schema_from_edn(&schema_edn).expect("must parse");
    assert_eq!(fn_schema.fn_kind, SchemaKind::Macro);

    // Build a minimal CodeEntry with this schema
    let entry = CodeEntry {
      doc: "test fn".to_owned(),
      examples: vec![],
      code: vec!["defmacro", "test-fn", "(a b)", "nil"].into(),
      schema: std::sync::Arc::new(CalcitTypeAnnotation::Fn(std::sync::Arc::new(fn_schema))),
    };

    // Serialize to Edn (as From<&CodeEntry> for Edn does)
    let entry_edn: Edn = Edn::from(&entry);

    // Format to Cirru string and parse back (as save_snapshot + load_snapshot do)
    let cirru_text = cirru_edn::format(&entry_edn, true).expect("format should succeed");
    assert!(
      cirru_text.contains(":schema $ :: :macro"),
      "macro schema should use wrapped :macro tag: {cirru_text}"
    );
    assert!(
      !cirru_text.contains(":return"),
      "macro schema should omit redundant return field during serialization: {cirru_text}"
    );
    let parsed_edn = cirru_edn::parse(&cirru_text).expect("parse should succeed");

    // Deserialize back to CodeEntry
    let reloaded: CodeEntry = parsed_edn.try_into().expect("TryFrom<Edn> should succeed");

    // Check the schema was preserved
    match reloaded.schema.as_ref() {
      CalcitTypeAnnotation::Fn(fn_annot) => {
        assert_eq!(
          fn_annot.fn_kind,
          SchemaKind::Macro,
          "fn_kind must survive round-trip; cirru_text: {cirru_text:?}"
        );
        assert_eq!(fn_annot.arg_types.len(), 2, "arg_types must survive round-trip");
      }
      other => panic!("schema must be Fn after round-trip, got {other:?}; cirru_text: {cirru_text:?}"),
    }
  }

  #[test]
  fn test_code_entry_serializes_schema_as_wrapped_fn() {
    use crate::calcit::SchemaKind;

    let entry = CodeEntry {
      doc: "wrapped schema".to_owned(),
      examples: vec![],
      code: vec!["defn", "wrapped", "()", "nil"].into(),
      schema: std::sync::Arc::new(CalcitTypeAnnotation::Fn(std::sync::Arc::new(CalcitFnTypeAnnotation {
        generics: std::sync::Arc::new(vec![]),
        arg_types: vec![],
        return_type: crate::calcit::DYNAMIC_TYPE.clone(),
        fn_kind: SchemaKind::Fn,
        rest_type: None,
      }))),
    };

    let entry_edn: Edn = Edn::from(&entry);
    let schema = match entry_edn {
      Edn::Record(record) => record
        .pairs
        .iter()
        .find(|(k, _)| k.arc_str().as_ref() == "schema")
        .map(|(_, v)| v.to_owned())
        .expect("schema field should exist"),
      _ => panic!("expected record edn"),
    };

    let Edn::Tuple(view) = schema else {
      panic!("top-level schema should serialize as wrapped fn tuple");
    };
    assert!(matches!(view.tag.as_ref(), Edn::Tag(tag) if tag.ref_str() == "fn"));
    let Some(Edn::Map(map)) = view.extra.first() else {
      panic!("wrapped schema payload should be a map");
    };
    assert!(
      map.tag_get("kind").is_none(),
      "wrapped plain fn schema should omit redundant :kind :fn"
    );
  }

  #[test]
  fn test_code_entry_serializes_macro_rest_schema_without_losing_rest() {
    use crate::calcit::SchemaKind;

    let entry = CodeEntry {
      doc: "wrapped macro schema".to_owned(),
      examples: vec![],
      code: vec!["defmacro", "wrapped", "(& body)", "nil"].into(),
      schema: std::sync::Arc::new(CalcitTypeAnnotation::Fn(std::sync::Arc::new(CalcitFnTypeAnnotation {
        generics: std::sync::Arc::new(vec![]),
        arg_types: vec![crate::calcit::DYNAMIC_TYPE.clone()],
        return_type: crate::calcit::DYNAMIC_TYPE.clone(),
        fn_kind: SchemaKind::Macro,
        rest_type: Some(crate::calcit::DYNAMIC_TYPE.clone()),
      }))),
    };

    let entry_edn: Edn = Edn::from(&entry);
    let schema = match entry_edn {
      Edn::Record(record) => record
        .pairs
        .iter()
        .find(|(k, _)| k.arc_str().as_ref() == "schema")
        .map(|(_, v)| v.to_owned())
        .expect("schema field should exist"),
      _ => panic!("expected record edn"),
    };

    let Edn::Tuple(view) = schema else {
      panic!("top-level schema should serialize as wrapped macro tuple");
    };
    assert!(matches!(view.tag.as_ref(), Edn::Tag(tag) if tag.ref_str() == "macro"));
    let Some(Edn::Map(map)) = view.extra.first() else {
      panic!("wrapped schema payload should be a map");
    };
    assert!(
      map.tag_get("kind").is_none(),
      "wrapped macro schema should omit redundant inner :kind"
    );
    assert!(map.tag_get("return").is_none(), "wrapped macro schema should omit redundant return");
    assert!(matches!(map.tag_get("rest"), Some(Edn::Tag(tag)) if tag.ref_str() == "dynamic"));
  }

  #[test]
  fn test_defmacro_code_normalizes_fn_schema_kind_on_load() {
    let code = cirru_parser::parse("defmacro demo (x) x")
      .expect("should parse code")
      .into_iter()
      .next()
      .expect("should have one node");
    let schema = Edn::tuple(
      Edn::tag("fn"),
      vec![Edn::Map(EdnMapView::from(HashMap::from([
        (Edn::tag("args"), Edn::List(EdnListView(vec![Edn::tag("dynamic")]))),
        (Edn::tag("return"), Edn::tag("dynamic")),
      ])))],
    );

    let entry = Edn::record_from_pairs(
      "CodeEntry".into(),
      &[
        ("doc".into(), Edn::Str(Arc::from(""))),
        ("examples".into(), Edn::List(EdnListView(vec![]))),
        ("code".into(), code.into()),
        ("schema".into(), schema),
      ],
    );

    let entry: CodeEntry = entry.try_into().expect("code entry should parse");
    let CalcitTypeAnnotation::Fn(fn_annot) = entry.schema.as_ref() else {
      panic!("schema should be fn-like");
    };
    assert_eq!(fn_annot.fn_kind, SchemaKind::Macro);
  }

  #[test]
  fn test_macro_schema_round_trip() {
    use crate::calcit::SchemaKind;
    // Simulate writing a :kind :macro schema and reading it back
    let schema_text = "{} (:kind :macro) (:return :bool) (:args ([] :number :number))";
    let schema_cirru = cirru_parser::parse(schema_text)
      .expect("should parse")
      .into_iter()
      .next()
      .expect("should have one node");

    // Convert to EDN (as done by handle_schema)
    let schema_edn = schema_cirru_to_edn(schema_cirru);
    assert!(!matches!(schema_edn, Edn::Nil), "schema_edn must not be Nil: {schema_edn:?}");

    // Parse the schema (as done when reading back)
    let fn_schema = CalcitTypeAnnotation::parse_fn_schema_from_edn(&schema_edn);
    assert!(
      fn_schema.is_some(),
      "parse_fn_schema_from_edn must return Some for macro schema; schema_edn={schema_edn:?}"
    );
    let fn_schema = fn_schema.unwrap();
    assert_eq!(fn_schema.fn_kind, SchemaKind::Macro, "fn_kind must be Macro");
    assert_eq!(fn_schema.arg_types.len(), 2, "must have 2 arg types");

    // Simulate a save (to_schema_edn) + reload
    let saved_edn = fn_schema.to_schema_edn();
    let fn_schema2 = CalcitTypeAnnotation::parse_fn_schema_from_edn(&saved_edn);
    assert!(
      fn_schema2.is_some(),
      "reload: parse_fn_schema_from_edn must return Some; saved_edn={saved_edn:?}"
    );
    let fn_schema2 = fn_schema2.unwrap();
    assert_eq!(fn_schema2.fn_kind, SchemaKind::Macro, "reload: fn_kind must be Macro");
    assert_eq!(fn_schema2.arg_types.len(), 2, "reload: must have 2 arg types");

    // Simulate normalize_schema_edn path (as used in TryFrom<Edn> for CodeEntry)
    let normalized = normalize_schema_edn(&saved_edn).expect("normalize must succeed");
    let fn_schema3 = CalcitTypeAnnotation::parse_fn_schema_from_edn(&normalized);
    assert!(
      fn_schema3.is_some(),
      "normalized: parse_fn_schema_from_edn must return Some; normalized={normalized:?}"
    );
    let fn_schema3 = fn_schema3.unwrap();
    assert_eq!(fn_schema3.fn_kind, SchemaKind::Macro, "normalized: fn_kind must be Macro");
  }

  #[test]
  fn test_load_snapshot_preserves_selected_real_world_schemas() {
    let core_file_content = fs::read_to_string("src/cirru/calcit-core.cirru").expect("Failed to read calcit-core.cirru");
    let edn_data = cirru_edn::parse(&core_file_content).expect("Failed to parse cirru content as EDN");
    let snapshot = load_snapshot_data(&edn_data, "src/cirru/calcit-core.cirru").expect("Failed to parse snapshot");

    let core_file = snapshot.files.get("calcit.core").expect("calcit.core file should exist");

    for def_name in [
      "&+",
      "%{}",
      "deftrait",
      "[,]",
      "not",
      "not=",
      "noted",
      "nth",
      "number?",
      "option:map",
      "optionally",
    ] {
      let entry = core_file.defs.get(def_name).unwrap_or_else(|| panic!("missing def: {def_name}"));
      assert!(
        matches!(entry.schema.as_ref(), CalcitTypeAnnotation::Fn(_)),
        "schema for {def_name} should stay fn-like after load, got {:?}",
        entry.schema
      );
    }
  }

  #[test]
  fn test_save_snapshot_round_trip_keeps_real_world_schema_markers() {
    let core_file_content = fs::read_to_string("src/cirru/calcit-core.cirru").expect("Failed to read calcit-core.cirru");
    let edn_data = cirru_edn::parse(&core_file_content).expect("Failed to parse cirru content as EDN");
    let snapshot = load_snapshot_data(&edn_data, "src/cirru/calcit-core.cirru").expect("Failed to parse snapshot");

    let temp_path = std::env::temp_dir().join(format!("calcit-schema-roundtrip-{}.cirru", std::process::id()));

    save_snapshot_to_file(&temp_path, &snapshot).expect("round-trip save should succeed");
    let saved = fs::read_to_string(&temp_path).expect("should read saved snapshot");
    let saved_edn = cirru_edn::parse(&saved).expect("saved snapshot should remain valid EDN");
    let saved_snapshot =
      load_snapshot_data(&saved_edn, temp_path.to_str().expect("temp path should be utf-8")).expect("saved snapshot should load again");

    let source_core_file = snapshot.files.get("calcit.core").expect("source calcit.core file should exist");
    let saved_core_file = saved_snapshot
      .files
      .get("calcit.core")
      .expect("saved calcit.core file should exist");

    for def_name in ["&+", "%{}", "not", "not=", "noted", "nth", "number?", "option:map", "optionally"] {
      let source_entry = source_core_file
        .defs
        .get(def_name)
        .unwrap_or_else(|| panic!("missing source def: {def_name}"));
      let saved_entry = saved_core_file
        .defs
        .get(def_name)
        .unwrap_or_else(|| panic!("missing saved def: {def_name}"));
      assert_eq!(saved_entry.schema, source_entry.schema, "schema should round-trip for {def_name}");
    }

    let _ = fs::remove_file(&temp_path);

    assert!(
      saved.contains("|&+ $ %{} :CodeEntry") && saved.contains(":schema $ :: :fn"),
      "saved snapshot should retain wrapped fn schemas"
    );
    assert!(
      saved.contains("|%{} $ %{} :CodeEntry") && saved.contains(":schema $ :: :macro"),
      "saved snapshot should retain wrapped macro schemas"
    );
  }

  #[test]
  fn test_validate_serialized_snapshot_content_rejects_double_quoted_generics() {
    let content = r#"{} (:package |mini)
  :configs $ {} (:init-fn |mini/main!) (:reload-fn |mini/main!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |mini $ %{} :FileEntry
      :ns $ %{} :CodeEntry (:doc |) (:code $ quote (ns mini)) (:examples $ []) (:schema nil)
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote (defn main! (x) x)
          :examples $ []
          :schema $ {} (:kind :fn) (:args $ [] :dynamic) (:generics $ [] ''T) (:return :dynamic)
"#;

    let err = validate_serialized_snapshot_content(content).expect_err("serialized snapshot should reject double-quoted generics");
    assert!(
      err.contains("serialized snapshot has invalid `:schema`") && err.contains("excess leading quotes"),
      "unexpected error: {err}"
    );
  }

  #[test]
  fn test_load_snapshot_reports_empty_configs_version_with_field_context() {
    let content = r#"{} (:package |mini)
  :configs $ {} (:init-fn |mini/main!) (:reload-fn |mini/main!) (:version ||)
    :modules $ []
  :entries $ {}
  :files $ {}
    |mini $ %{} :FileEntry
      :ns $ %{} :CodeEntry (:doc |) (:code $ quote (ns mini)) (:examples $ []) (:schema nil)
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote (defn main! () nil)
          :examples $ []
          :schema nil
"#;

    let edn_data = cirru_edn::parse(content).expect("snapshot text should parse as EDN");
    let err = load_snapshot_data(&edn_data, "mini.cirru").expect_err("empty configs.version should fail on load");

    assert!(err.contains("configs.version cannot be empty"), "unexpected error: {err}");
    assert!(
      err.contains(":configs (:version ...)") || err.contains("got ||"),
      "unexpected error: {err}"
    );
  }
}
