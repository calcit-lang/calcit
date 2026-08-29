use cirru_edn::{Edn, EdnListView, EdnMapView, EdnSetView, EdnStructView, EdnTag, from_edn};
use cirru_parser::Cirru;
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;
use std::path::Path;
use std::sync::Arc;

use crate::calcit::{Calcit, CalcitTypeAnnotation, DYNAMIC_TYPE, SchemaKind, with_type_annotation_warning_context};
use crate::data::edn::{format_deserialize_error, format_edn_display};

const SNAPSHOT_ABOUT_MESSAGE: &str = "Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --full` first. Manual edits must follow format and schema conventions, then run `calcit edit format`.";

fn default_version() -> String {
  "0.0.0".to_owned()
}

pub const DEFAULT_ENTRY_NAME: &str = "default";

fn default_active_entry() -> String {
  DEFAULT_ENTRY_NAME.to_owned()
}

fn format_edn_preview(value: &Edn) -> String {
  format_edn_display(value)
}

pub(crate) fn parse_snapshot_identifier_key(value: &Edn, owner: &str) -> Result<String, String> {
  match value {
    Edn::Str(text) | Edn::Symbol(text) if !text.is_empty() => Ok(text.to_string()),
    Edn::Str(_) | Edn::Symbol(_) => Err(format!("{owner}: snapshot identifier key cannot be empty")),
    other => Err(format!(
      "{owner}: snapshot identifier key must be a String or Symbol, got {}",
      format_edn_preview(other)
    )),
  }
}

pub(crate) fn insert_snapshot_identifier<T>(
  target: &mut HashMap<String, T>,
  name: String,
  value: T,
  owner: &str,
) -> Result<(), String> {
  if target.insert(name.clone(), value).is_some() {
    return Err(format!(
      "{owner}: duplicate snapshot identifier `{name}` after normalizing String/Symbol keys"
    ));
  }
  Ok(())
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
    "required" => Some("required"),
    "optional" => Some("optional"),
    "expansion" => Some("expansion"),
    "rest" => Some("rest"),
    "generics" => Some("generics"),
    "where" => Some("where"),
    "features" => Some("features"),
    "capabilities" => Some("capabilities"),
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

fn is_callable_schema_wrapper_variant(value: &str) -> bool {
  matches!(value, "fn" | "macro" | "Fn" | "Macro")
}

fn is_macro_schema_wrapper_variant(value: &str) -> bool {
  matches!(value, "macro" | "Macro")
}

fn normalize_schema_map(map: &EdnMapView) -> EdnMapView {
  let mut normalized = EdnMapView::default();

  for (key, value) in map.0.iter() {
    let normalized_key = match key {
      Edn::Tag(tag) => Edn::tag(tag.ref_str()),
      Edn::Str(text) | Edn::Symbol(text) => canonical_schema_field_name(text)
        .map(Edn::tag)
        .or_else(|| text.strip_prefix('\'').map(|name| Edn::Symbol(Arc::from(name))))
        .unwrap_or_else(|| key.clone()),
      _ => key.clone(),
    };

    let normalized_value = match (&normalized_key, value) {
      (Edn::Tag(tag), Edn::Str(text)) | (Edn::Tag(tag), Edn::Symbol(text)) if tag.ref_str() == "kind" => {
        canonical_schema_kind_name(text).map(Edn::tag).unwrap_or_else(|| value.clone())
      }
      _ => normalize_schema_value(value),
    };

    normalized.insert(normalized_key, normalized_value);
  }

  normalized
}

fn normalize_schema_value(value: &Edn) -> Edn {
  match value {
    Edn::Map(map) => Edn::Map(normalize_schema_map(map)),
    Edn::List(items) => Edn::List(EdnListView(items.0.iter().map(normalize_schema_value).collect())),
    Edn::Enum(view) => {
      let mut normalized = view.clone();
      normalized.extra = view.extra.iter().map(normalize_schema_value).collect();
      Edn::Enum(normalized)
    }
    _ => value.clone(),
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SnapshotRunMode {
  Native,
  Js,
}

impl SnapshotRunMode {
  pub fn as_str(self) -> &'static str {
    match self {
      SnapshotRunMode::Native => "native",
      SnapshotRunMode::Js => "js",
    }
  }
}

impl std::fmt::Display for SnapshotRunMode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.as_str())
  }
}

/// Host target selected by an entry. This stays separate from the execution
/// mode because Node.js code is emitted by the JavaScript backend too.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SnapshotTarget {
  Browser,
  Node,
  Native,
  Wasm,
}

impl SnapshotTarget {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Browser => "browser",
      Self::Node => "node",
      Self::Native => "native",
      Self::Wasm => "wasm",
    }
  }
}

/// Per-entry capability policy. Features remain implementation metadata; this
/// policy only controls the diagnostics emitted when a body uses a capability
/// without declaring it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FeaturePolicy {
  #[default]
  Allow,
  Warn,
  Error,
}

impl FeaturePolicy {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Allow => "allow",
      Self::Warn => "warn",
      Self::Error => "error",
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotEntry {
  pub mode: SnapshotRunMode,
  #[serde(rename = "init-fn")]
  pub init_fn: String,
  #[serde(rename = "reload-fn")]
  pub reload_fn: String,
  /// Human-oriented semantic context for this entry.
  #[serde(default)]
  pub description: String,
  #[serde(default)]
  pub modules: Vec<String>,
  #[serde(default, rename = "type-slots")]
  pub type_slots: HashMap<String, String>,
  #[serde(default, rename = "feature-policy")]
  pub feature_policy: HashMap<String, FeaturePolicy>,
  /// Optional host target. Omitting it preserves old projects and disables
  /// target-specific FFI validation for that entry.
  #[serde(default)]
  pub target: Option<SnapshotTarget>,
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
  #[serde(default)]
  pub tests: Vec<RawTestEntry>,
  #[serde(default)]
  pub tags: Vec<String>,
  pub code: Cirru,
  #[serde(default)]
  pub schema: Option<Edn>,
  #[serde(default)]
  pub ffi: Option<Edn>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RawTestEntry {
  pub name: String,
  pub code: Cirru,
  #[serde(default)]
  pub tags: Vec<String>,
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
  #[serde(default = "default_version")]
  pub version: String,
  pub entries: HashMap<String, SnapshotEntry>,
  pub files: HashMap<String, RawFileInSnapShot>,
}

impl RawCodeEntry {
  fn into_code_entry(self, owner: &str) -> Result<CodeEntry, String> {
    let schema = match self.schema {
      None | Some(Edn::Nil) => DYNAMIC_TYPE.clone(),
      Some(value) => with_type_annotation_warning_context(owner.to_owned(), || parse_loaded_schema_annotation(&value, owner))?,
    };

    let tests = self
      .tests
      .into_iter()
      .map(|test| TestEntry {
        name: test.name,
        code: test.code,
        tags: tags_vec_to_set(test.tags),
      })
      .collect::<Vec<_>>();
    validate_test_entries(&tests, owner)?;

    Ok(CodeEntry {
      doc: self.doc,
      examples: self.examples,
      tests,
      tags: tags_vec_to_set(self.tags),
      code: self.code,
      schema,
      ffi: self.ffi,
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
    version: raw.version,
    entries: raw.entries,
    files,
    active_entry: default_active_entry(),
  })
}

impl From<&FileInSnapShot> for Edn {
  fn from(data: &FileInSnapShot) -> Edn {
    let mut defs_map = EdnMapView::default();
    for (k, v) in &data.defs {
      defs_map.insert(Edn::Symbol(k.as_str().into()), Edn::from(v));
    }
    Edn::Struct(EdnStructView {
      name: Arc::from("FileEntry"),
      pairs: vec![("defs".into(), Edn::from(defs_map)), ("ns".into(), Edn::from(&data.ns))], // TODO
    })
  }
}

impl TryFrom<Edn> for FileInSnapShot {
  type Error = String;
  fn try_from(data: Edn) -> Result<Self, String> {
    parse_file_in_snapshot_with_context(data, "<file>")
  }
}

impl From<FileInSnapShot> for Edn {
  fn from(data: FileInSnapShot) -> Edn {
    let mut defs_map = EdnMapView::default();
    for (k, v) in data.defs {
      defs_map.insert(Edn::Symbol(k.as_str().into()), Edn::from(v));
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
      Edn::Struct(struct_value) => {
        for (key, value) in &struct_value.pairs {
          match key.arc_str().as_ref() {
            "doc" => {
              doc = from_edn(value.to_owned())
                .map_err(|e| format!("failed to parse NsEntry.doc: {}", format_deserialize_error(&e, value)))?;
            }
            "code" => {
              code = Some(
                from_edn(value.to_owned())
                  .map_err(|e| format!("failed to parse NsEntry.code: {}", format_deserialize_error(&e, value)))?,
              );
            }
            _ => {}
          }
        }
      }
      Edn::Map(map) => {
        if let Some(value) = map.get(&Edn::Tag(EdnTag::new("doc"))) {
          doc =
            from_edn(value.to_owned()).map_err(|e| format!("failed to parse NsEntry.doc: {}", format_deserialize_error(&e, value)))?;
        }
        if let Some(value) = map.get(&Edn::Tag(EdnTag::new("code"))) {
          code = Some(
            from_edn(value.to_owned()).map_err(|e| format!("failed to parse NsEntry.code: {}", format_deserialize_error(&e, value)))?,
          );
        }
      }
      other => {
        return Err(format!(
          "failed to parse NsEntry: expected struct/map, got: {}",
          format_edn_display(&other)
        ));
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
    Edn::struct_from_pairs("NsEntry", &[("doc".into(), data.doc.into()), ("code".into(), data.code.into())])
  }
}

impl From<&NsEntry> for Edn {
  fn from(data: &NsEntry) -> Self {
    Edn::struct_from_pairs(
      "NsEntry",
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
      CalcitTypeAnnotation::Dynamic if schema_annotation_is_missing(schema) => None,
      CalcitTypeAnnotation::Dynamic => Some(schema_annotation_to_edn(schema.as_ref())),
      // Keep the binary snapshot representation of function schemas stable:
      // build.rs and older runtimes expect the direct map form here. Value
      // annotations use their ordinary type-expression representation.
      CalcitTypeAnnotation::Fn(fn_annot) => Some(fn_annot.to_schema_edn()),
      annotation => Some(schema_annotation_to_edn(annotation)),
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

/// Missing schemas use the shared Dynamic singleton. An explicitly declared
/// `:: Dynamic` is parsed into its own Arc so analysis and binary snapshots can
/// preserve the difference between omitted and intentionally untyped schemas.
pub fn schema_annotation_is_missing(schema: &Arc<CalcitTypeAnnotation>) -> bool {
  matches!(schema.as_ref(), CalcitTypeAnnotation::Dynamic) && Arc::ptr_eq(schema, &DYNAMIC_TYPE)
}

mod tags_serde {
  use super::*;

  pub fn serialize<S>(tags: &HashSet<EdnTag>, s: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    tags_set_to_vec(tags).serialize(s)
  }

  pub fn deserialize<'de, D>(d: D) -> Result<HashSet<EdnTag>, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    let tags = Vec::<String>::deserialize(d)?;
    Ok(tags_vec_to_set(tags))
  }
}

fn parse_loaded_schema_annotation(value: &Edn, owner: &str) -> Result<Arc<CalcitTypeAnnotation>, String> {
  if matches!(value, Edn::Nil) {
    return Ok(DYNAMIC_TYPE.clone());
  }

  // A top-level quoted symbol is rendered by Cirru EDN as `'String` (rather
  // than as a list context such as `[] 'String`) and parses back as `Quote`.
  // Treat that one-node quote as the canonical nominal type spelling while
  // keeping all older tag spellings accepted below.
  if let Edn::Quote(Cirru::Leaf(symbol)) = value {
    let annotation = CalcitTypeAnnotation::parse_type_annotation_from_edn(&Edn::Symbol(symbol.clone()));
    if CalcitTypeAnnotation::canonical_type_symbol_name(symbol).is_some() {
      return Ok(annotation);
    }
  }

  // Snapshot writers wrap standalone type symbols as zero-payload EDN enums
  // (`:: 'String`, `:: 'StructDef`, ...). Decode those wrappers back through
  // the canonical symbol parser. Otherwise a second unrelated Snapshot write
  // would interpret definition-kind markers as anonymous Enum values and
  // silently serialize `StructDef`/`EnumDef` as `Enum`.
  if let Some(annotation) = parse_zero_payload_schema_wrapper(value) {
    return Ok(annotation);
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

  if let Ok(normalized) = normalize_schema_edn(value) {
    let schema_cirru = parse_schema_cirru_from_edn(&normalized).map_err(|e| {
      format!(
        "failed to convert {owner} into Cirru: {e}; schema={}",
        format_edn_preview(&normalized)
      )
    })?;
    parse_schema_data(&schema_cirru)
      .map_err(|e| format!("failed to validate {owner}: {e}; schema={}", format_edn_preview(&normalized)))?;

    if let Some(signature) = CalcitTypeAnnotation::parse_macro_signature_from_edn(&normalized) {
      return Ok(Arc::new(CalcitTypeAnnotation::Macro(Arc::new(signature))));
    }
    return CalcitTypeAnnotation::parse_fn_schema_from_edn(&normalized)
      .map(|s| Arc::new(CalcitTypeAnnotation::Fn(Arc::new(s))))
      .ok_or_else(|| {
        format!(
          "failed to parse {owner} as function schema after normalization; schema={}",
          format_edn_preview(&normalized)
        )
      });
  }

  let schema_cirru = parse_schema_cirru_from_edn(value)
    .map_err(|e| format!("failed to convert {owner} into Cirru: {e}; schema={}", format_edn_preview(value)))?;
  parse_schema_data(&schema_cirru).map_err(|e| format!("failed to validate {owner}: {e}; schema={}", format_edn_preview(value)))?;

  let annotation = CalcitTypeAnnotation::parse_type_annotation_from_edn(value);
  if matches!(annotation.as_ref(), CalcitTypeAnnotation::Dynamic) {
    return Err(format!(
      "failed to parse {owner} as a standalone type annotation; schema={}",
      format_edn_preview(value)
    ));
  }
  Ok(annotation)
}

fn parse_zero_payload_schema_wrapper(value: &Edn) -> Option<Arc<CalcitTypeAnnotation>> {
  let Edn::Enum(view) = value else { return None };
  if !view.extra.is_empty() {
    return None;
  }
  let canonical = CalcitTypeAnnotation::canonical_type_symbol_name(&view.variant)?;
  if matches!(canonical, "Optional" | "JsNullish" | "Variadic") {
    return None;
  }
  Some(CalcitTypeAnnotation::parse_type_annotation_from_edn(&Edn::Symbol(Arc::from(
    canonical,
  ))))
}

fn tags_vec_to_set(tags: Vec<String>) -> HashSet<EdnTag> {
  tags.into_iter().map(|tag| EdnTag::new(tag.trim_start_matches(':'))).collect()
}

fn tags_set_to_vec(tags: &HashSet<EdnTag>) -> Vec<String> {
  let mut items: Vec<String> = tags.iter().map(|tag| format!(":{}", tag.ref_str())).collect();
  items.sort();
  items
}

pub fn parse_code_entry_tags_from_edn(value: &Edn) -> Result<HashSet<EdnTag>, String> {
  match value {
    Edn::Set(set) => {
      let mut tags = HashSet::with_capacity(set.0.len());
      for item in &set.0 {
        match item {
          Edn::Tag(tag) => {
            tags.insert(tag.clone());
          }
          other => {
            return Err(format!("CodeEntry.tags expects tag items, got: {}", format_edn_preview(other)));
          }
        }
      }
      Ok(tags)
    }
    other => Err(format!("CodeEntry.tags expects a hashset, got: {}", format_edn_preview(other))),
  }
}

fn tags_to_edn(tags: &HashSet<EdnTag>) -> Edn {
  #[allow(clippy::mutable_key_type)]
  let items: HashSet<Edn> = tags.iter().map(|tag| Edn::Tag(tag.clone())).collect();
  Edn::Set(EdnSetView(items))
}

/// Convert a loaded definition schema annotation into snapshot-style EDN.
pub fn schema_annotation_to_edn(schema: &CalcitTypeAnnotation) -> Edn {
  let expression = match schema {
    CalcitTypeAnnotation::Dynamic => Edn::Symbol(Arc::from("Dynamic")),
    CalcitTypeAnnotation::Fn(fn_annot) => fn_annot.to_wrapped_schema_edn(),
    CalcitTypeAnnotation::Macro(signature) => signature.to_wrapped_schema_edn(),
    // Runtime-resolved nominal types are intentionally persisted as their
    // broad schema kinds. Their concrete definitions belong to source code,
    // and serializing only a local name would lose namespace identity.
    CalcitTypeAnnotation::Custom(value) => match value.as_ref() {
      crate::calcit::Calcit::Tag(tag) => CalcitTypeAnnotation::canonical_type_symbol_name(tag.ref_str())
        .map(|name| Edn::Symbol(Arc::from(name)))
        .unwrap_or_else(|| Edn::Symbol(Arc::from("Dynamic"))),
      _ => Edn::Symbol(Arc::from("Dynamic")),
    },
    CalcitTypeAnnotation::StructValue(_) => Edn::Symbol(Arc::from("Struct")),
    CalcitTypeAnnotation::Struct(..) => Edn::Symbol(Arc::from("Struct")),
    CalcitTypeAnnotation::Enum(..) => Edn::Symbol(Arc::from("Enum")),
    CalcitTypeAnnotation::EnumValue(_) => Edn::Symbol(Arc::from("Enum")),
    CalcitTypeAnnotation::Trait(_) => Edn::Symbol(Arc::from("Trait")),
    other => other.to_type_edn(),
  };
  // A lone EDN symbol in a struct field is parsed as a Cirru quote. Wrap it
  // as a zero-argument type expression so Snapshot serialization remains
  // structurally unambiguous while rendering source-level `:: 'String`.
  match expression {
    Edn::Symbol(name) => Edn::enum_value(name, vec![]),
    other => other,
  }
}

fn code_entry_edn_pairs(data: &CodeEntry) -> Vec<(EdnTag, Edn)> {
  let schema = normalize_schema_for_code(&data.code, &data.schema);
  let schema_edn = schema_annotation_to_edn(schema.as_ref());
  let mut pairs = vec![
    ("doc".into(), data.doc.to_owned().into()),
    ("examples".into(), data.examples.to_owned().into()),
    ("code".into(), data.code.to_owned().into()),
    ("schema".into(), schema_edn),
  ];
  if !data.tests.is_empty() {
    pairs.insert(
      2,
      ("tests".into(), Edn::List(EdnListView(data.tests.iter().map(Edn::from).collect()))),
    );
  }
  if !data.tags.is_empty() {
    pairs.insert(2, ("tags".into(), tags_to_edn(&data.tags)));
  }
  if let Some(ffi) = &data.ffi {
    pairs.push(("ffi".into(), ffi.clone()));
  }
  pairs
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestEntry {
  pub name: String,
  pub code: Cirru,
  #[serde(default, with = "tags_serde")]
  pub tags: HashSet<EdnTag>,
}

pub fn validate_test_names<'a>(names: impl IntoIterator<Item = &'a str>, owner: &str) -> Result<(), String> {
  let mut seen = HashSet::new();
  for name in names {
    if name.trim().is_empty() {
      return Err(format!("{owner}: test name must not be empty"));
    }
    if name != name.trim() {
      return Err(format!("{owner}: test name must not have leading or trailing whitespace: `{name}`"));
    }
    if !seen.insert(name) {
      return Err(format!("{owner}: duplicate test name `{name}`"));
    }
  }
  Ok(())
}

fn validate_test_entries(tests: &[TestEntry], owner: &str) -> Result<(), String> {
  validate_test_names(tests.iter().map(|test| test.name.as_str()), owner)
}

impl TryFrom<Edn> for TestEntry {
  type Error = String;

  fn try_from(data: Edn) -> Result<Self, Self::Error> {
    let mut name = None;
    let mut code = None;
    let mut tags = HashSet::new();
    let pairs = match data {
      Edn::Struct(value) => value.pairs,
      Edn::Map(value) => value
        .0
        .into_iter()
        .map(|(key, value)| match key {
          Edn::Tag(key) => Ok((key, value)),
          other => Err(format!("TestEntry field must use a tag key, got: {other}")),
        })
        .collect::<Result<Vec<_>, _>>()?,
      other => return Err(format!("failed to parse TestEntry: expected struct/map, got: {other}")),
    };

    for (key, value) in pairs {
      match key.ref_str() {
        "name" => {
          name = Some(
            from_edn(value.to_owned())
              .map_err(|error| format!("failed to parse TestEntry.name: {}", format_deserialize_error(&error, &value)))?,
          );
        }
        "code" => {
          code = Some(
            from_edn(value.to_owned())
              .map_err(|error| format!("failed to parse TestEntry.code: {}", format_deserialize_error(&error, &value)))?,
          );
        }
        "tags" => tags = parse_code_entry_tags_from_edn(&value)?,
        _ => {}
      }
    }

    let name: String = name.ok_or_else(|| "failed to parse TestEntry: missing name field".to_owned())?;
    validate_test_names([name.as_str()], "TestEntry").map_err(|error| format!("failed to parse {error}"))?;
    let code = code.ok_or_else(|| "failed to parse TestEntry: missing code field".to_owned())?;
    Ok(TestEntry { name, code, tags })
  }
}

impl From<&TestEntry> for Edn {
  fn from(data: &TestEntry) -> Self {
    let mut pairs = vec![
      (EdnTag::new("name"), Edn::Str(data.name.clone().into())),
      (EdnTag::new("code"), data.code.clone().into()),
    ];
    if !data.tags.is_empty() {
      pairs.push((EdnTag::new("tags"), tags_to_edn(&data.tags)));
    }
    Edn::struct_from_pairs("TestEntry", &pairs)
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeEntry {
  pub doc: String,
  #[serde(default)]
  pub examples: Vec<Cirru>,
  #[serde(default)]
  pub tests: Vec<TestEntry>,
  #[serde(default, with = "tags_serde")]
  pub tags: HashSet<EdnTag>,
  pub code: Cirru,
  #[serde(default = "schema_serde::default_schema", with = "schema_serde")]
  pub schema: Arc<CalcitTypeAnnotation>,
  #[serde(default)]
  pub ffi: Option<Edn>,
}

/// Return an opaque, deterministic revision for one definition.
///
/// The revision covers every persisted `CodeEntry` field and deliberately
/// sorts set-like metadata before hashing. It can therefore be used as a
/// read-only identity or as a future stale-edit precondition without depending
/// on map iteration order, file timestamps, or the definition's position in a
/// snapshot.
pub fn definition_revision(entry: &CodeEntry) -> Result<String, String> {
  fn update_part(hasher: &mut Md5, label: &str, content: &[u8]) {
    hasher.update(label.as_bytes());
    hasher.update([0]);
    hasher.update((content.len() as u64).to_le_bytes());
    hasher.update(content);
  }

  fn render_cirru_node_for_revision(node: &Cirru, label: &str) -> Result<Vec<u8>, String> {
    match node {
      // `cirru_parser::format` accepts top-level expressions, not a standalone
      // leaf. Examples and definition-attached tests intentionally allow both.
      Cirru::Leaf(value) => {
        let mut rendered = b"leaf\0".to_vec();
        rendered.extend_from_slice(value.as_bytes());
        Ok(rendered)
      }
      Cirru::List(_) => cirru_parser::format(std::slice::from_ref(node), true.into())
        .map(String::into_bytes)
        .map_err(|error| format!("Failed to format definition {label} for revision: {error}")),
    }
  }

  let mut hasher = Md5::new();
  update_part(&mut hasher, "doc", entry.doc.as_bytes());

  let mut tags = entry.tags.iter().map(|tag| tag.ref_str()).collect::<Vec<_>>();
  tags.sort_unstable();
  for tag in tags {
    update_part(&mut hasher, "tag", tag.as_bytes());
  }

  let schema = cirru_edn::format(&schema_annotation_to_edn(entry.schema.as_ref()), true)
    .map_err(|error| format!("Failed to format definition schema for revision: {error}"))?;
  update_part(&mut hasher, "schema", schema.as_bytes());

  let code = render_cirru_node_for_revision(&entry.code, "code")?;
  update_part(&mut hasher, "code", &code);

  for example in &entry.examples {
    let rendered = render_cirru_node_for_revision(example, "example")?;
    update_part(&mut hasher, "example", &rendered);
  }

  for test in &entry.tests {
    update_part(&mut hasher, "test-name", test.name.as_bytes());
    let mut tags = test.tags.iter().map(|tag| tag.ref_str()).collect::<Vec<_>>();
    tags.sort_unstable();
    for tag in tags {
      update_part(&mut hasher, "test-tag", tag.as_bytes());
    }
    let rendered = render_cirru_node_for_revision(&test.code, "test")?;
    update_part(&mut hasher, "test-code", &rendered);
  }

  if let Some(ffi) = &entry.ffi {
    let rendered =
      cirru_edn::format(ffi, true).map_err(|error| format!("Failed to format definition FFI metadata for revision: {error}"))?;
    update_part(&mut hasher, "ffi", rendered.as_bytes());
  }

  Ok(format!("md5:{}", hex::encode(hasher.finalize())))
}

impl TryFrom<Edn> for CodeEntry {
  type Error = String;
  fn try_from(data: Edn) -> Result<Self, String> {
    let mut doc = String::new();
    let mut examples: Vec<Cirru> = vec![];
    let mut tests: Vec<TestEntry> = vec![];
    let mut tags: HashSet<EdnTag> = HashSet::new();
    let mut code: Option<Cirru> = None;
    let mut schema: Arc<CalcitTypeAnnotation> = DYNAMIC_TYPE.clone();
    let mut ffi: Option<Edn> = None;

    match data {
      Edn::Struct(struct_value) => {
        for (key, value) in &struct_value.pairs {
          match key.arc_str().as_ref() {
            "doc" => {
              doc = from_edn(value.to_owned())
                .map_err(|e| format!("failed to parse CodeEntry.doc: {}", format_deserialize_error(&e, value)))?;
            }
            "examples" => {
              examples = from_edn(value.to_owned())
                .map_err(|e| format!("failed to parse CodeEntry.examples: {}", format_deserialize_error(&e, value)))?;
            }
            "tests" => {
              let Edn::List(items) = value else {
                return Err(format!("failed to parse CodeEntry.tests: expected list, got: {value}"));
              };
              tests = items.0.iter().cloned().map(TestEntry::try_from).collect::<Result<Vec<_>, _>>()?;
            }
            "tags" => {
              tags = parse_code_entry_tags_from_edn(value)?;
            }
            "code" => {
              code = Some(
                from_edn(value.to_owned())
                  .map_err(|e| format!("failed to parse CodeEntry.code: {}", format_deserialize_error(&e, value)))?,
              );
            }
            "schema" if !matches!(value, Edn::Nil) => {
              schema = parse_loaded_schema_annotation(value, "CodeEntry.schema")?;
            }
            "ffi" if !matches!(value, Edn::Nil) => {
              ffi = Some(value.to_owned());
            }
            _ => {}
          }
        }
      }
      Edn::Map(map) => {
        if let Some(value) = map.get(&Edn::Tag(EdnTag::new("doc"))) {
          doc = from_edn(value.to_owned())
            .map_err(|e| format!("failed to parse CodeEntry.doc: {}", format_deserialize_error(&e, value)))?;
        }
        if let Some(value) = map.get(&Edn::Tag(EdnTag::new("examples"))) {
          examples = from_edn(value.to_owned())
            .map_err(|e| format!("failed to parse CodeEntry.examples: {}", format_deserialize_error(&e, value)))?;
        }
        if let Some(value) = map.get(&Edn::Tag(EdnTag::new("tests"))) {
          let Edn::List(items) = value else {
            return Err(format!("failed to parse CodeEntry.tests: expected list, got: {value}"));
          };
          tests = items.0.iter().cloned().map(TestEntry::try_from).collect::<Result<Vec<_>, _>>()?;
        }
        if let Some(value) = map.get(&Edn::Tag(EdnTag::new("tags"))) {
          tags = parse_code_entry_tags_from_edn(value)?;
        }
        if let Some(value) = map.get(&Edn::Tag(EdnTag::new("code"))) {
          code = Some(
            from_edn(value.to_owned())
              .map_err(|e| format!("failed to parse CodeEntry.code: {}", format_deserialize_error(&e, value)))?,
          );
        }
        if let Some(value) = map.get(&Edn::Tag(EdnTag::new("schema")))
          && !matches!(value, Edn::Nil)
        {
          schema = parse_loaded_schema_annotation(value, "CodeEntry.schema")?;
        }
        if let Some(value) = map.get(&Edn::Tag(EdnTag::new("ffi")))
          && !matches!(value, Edn::Nil)
        {
          ffi = Some(value.to_owned());
        }
      }
      other => {
        return Err(format!(
          "failed to parse CodeEntry: expected struct/map, got: {}",
          format_edn_display(&other)
        ));
      }
    }

    let code = code.ok_or_else(|| "failed to parse CodeEntry: missing code field".to_owned())?;
    validate_test_entries(&tests, "CodeEntry.tests")?;
    let schema = normalize_schema_for_code(&code, &schema);

    Ok(CodeEntry {
      doc,
      examples,
      tests,
      tags,
      code,
      schema,
      ffi,
    })
  }
}

/// Normalize a schema Edn value.
/// Wrapped `(:: 'Fn ({} ...))` / `(:: 'Macro ({} ...))` forms are converted to a direct map Edn.
/// Legacy `:fn` / `:macro` tags remain accepted while loading.
/// Direct map format is returned as-is.
fn normalize_schema_edn(value: &Edn) -> Result<Edn, String> {
  if matches!(value, Edn::Map(_)) {
    let Edn::Map(map) = value else { unreachable!() };
    let normalized = Edn::Map(normalize_schema_map(map));
    validate_schema_edn_no_legacy_quotes(&normalized)?;
    return Ok(normalized);
  }

  if let Edn::Enum(view) = value
    && is_callable_schema_wrapper_variant(view.variant.as_ref())
    && let Some(Edn::Map(map)) = view.extra.first()
  {
    let mut normalized_map = normalize_schema_map(map);
    if normalized_map.tag_get("kind").is_none() && is_macro_schema_wrapper_variant(view.variant.as_ref()) {
      normalized_map.insert_key("kind", Edn::tag("macro"));
    }
    let normalized = Edn::Map(normalized_map);
    validate_schema_edn_no_legacy_quotes(&normalized)?;
    return Ok(normalized);
  }

  Err(format!(
    "invalid schema format: expected wrapped `(:: 'Fn ({{}} ...))` / `(:: 'Macro ({{}} ...))` or a normalized schema map, got {}",
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
      Edn::Enum(view) => {
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
      Edn::Struct(struct_value) => {
        let _ = struct_value;
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
  if let Cirru::List(items) = schema
    && let Some(Cirru::Leaf(head)) = items.first()
  {
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

  let schema_text =
    cirru_parser::format(std::slice::from_ref(schema), true.into()).map_err(|e| format!("Failed to format schema to Cirru: {e}"))?;

  cirru_edn::parse(&schema_text).map_err(|e| format!("Failed to parse schema as Cirru EDN: {e}"))?;

  Ok(())
}

/// Convert a Cirru schema tree to a direct Edn value (not Quote-wrapped).
/// Used when serializing CodeEntry to file: the schema is stored as a native
/// EDN map instead of a quoted Cirru expression.
/// `calcit edit format` normalises old quote-wrapped schemas to this format.
/// Returns `Edn::Nil` if conversion fails (should not happen for valid schemas).
pub fn schema_cirru_to_edn(schema: Cirru) -> Edn {
  fn cirru_schema_to_edn(node: &Cirru) -> Option<Edn> {
    match node {
      Cirru::Leaf(text) => {
        let value = text.as_ref();
        if let Some(stripped) = value.strip_prefix(':') {
          Some(Edn::Tag(EdnTag::new(stripped)))
        } else if let Some(stripped) = value.strip_prefix('\'') {
          Some(Edn::Symbol(Arc::from(stripped)))
        } else if let Some(stripped) = value.strip_prefix('|') {
          Some(Edn::str(stripped))
        } else {
          Some(Edn::Symbol(text.clone()))
        }
      }
      Cirru::List(items) => match items.first() {
        Some(Cirru::Leaf(head)) if head.as_ref() == "quote" && items.len() == 2 => match items.get(1) {
          Some(Cirru::Leaf(name)) => Some(Edn::Symbol(name.clone())),
          _ => None,
        },
        Some(Cirru::Leaf(head)) if head.as_ref() == "{}" => {
          let mut map = EdnMapView::default();
          for pair in items.iter().skip(1) {
            let Cirru::List(xs) = pair else {
              return None;
            };
            if xs.len() < 2 {
              return None;
            }
            let key = cirru_schema_to_edn(&xs[0])?;
            let value_node = if xs.len() == 2 {
              xs[1].clone()
            } else {
              Cirru::List(xs.iter().skip(1).cloned().collect())
            };
            let value = cirru_schema_to_edn(&value_node)?;
            map.insert(key, value);
          }
          Some(Edn::Map(map))
        }
        Some(Cirru::Leaf(head)) if head.as_ref() == "[]" => {
          let values: Option<Vec<Edn>> = items.iter().skip(1).map(cirru_schema_to_edn).collect();
          values.map(|xs| Edn::List(cirru_edn::EdnListView(xs)))
        }
        Some(Cirru::Leaf(head)) if head.as_ref() == "#{}" => {
          let values: Option<Vec<Edn>> = items.iter().skip(1).map(cirru_schema_to_edn).collect();
          values.map(|xs| {
            let mut set = EdnSetView::default();
            for item in xs {
              set.insert(item);
            }
            Edn::Set(set)
          })
        }
        Some(Cirru::Leaf(head)) if head.as_ref() == "::" && items.len() >= 2 => {
          let tag = cirru_schema_to_edn(&items[1])?;
          let variant = match tag {
            Edn::Tag(tag) => tag.arc_str(),
            Edn::Symbol(symbol) => symbol,
            _ => return None,
          };
          let extra: Option<Vec<Edn>> = items.iter().skip(2).map(cirru_schema_to_edn).collect();
          extra.map(|xs| Edn::enum_value(variant, xs))
        }
        _ => {
          let values: Option<Vec<Edn>> = items.iter().map(cirru_schema_to_edn).collect();
          values.map(|xs| Edn::List(cirru_edn::EdnListView(xs)))
        }
      },
    }
  }

  cirru_schema_to_edn(&schema).unwrap_or(Edn::Nil)
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
  fn validate_serialized_schema(schema: &Cirru) -> Result<(), String> {
    if let Cirru::Leaf(tag) = schema {
      let tag_name = tag.trim_start_matches(':');
      if PRIMITIVE_SCHEMA_TAGS.contains(&tag_name) {
        return Ok(());
      }
    }
    validate_schema_for_write(schema)
  }

  fn walk(node: &Cirru, path: &mut Vec<usize>) -> Result<(), String> {
    if let Cirru::List(items) = node {
      if let Some(Cirru::Leaf(head)) = items.first()
        && &**head == ":schema"
        && let Some(schema_node) = items.get(1)
      {
        if matches!(schema_node, Cirru::Leaf(s) if s.as_ref() == "nil") {
          return Ok(());
        }
        return validate_serialized_schema(schema_node)
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
pub const VALID_SCHEMA_FIELDS: &[&str] = &[
  ":kind",
  ":args",
  ":return",
  ":required",
  ":optional",
  ":expansion",
  ":capabilities",
  ":rest",
  ":generics",
  ":where",
  ":features",
  ":legacy-origin",
];

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
  match node {
    Cirru::Leaf(value) => {
      if let Some(name) = value.strip_prefix('\'')
        && !name.is_empty()
      {
        out.insert(name.to_owned());
      }
    }
    Cirru::List(items) => {
      if items.len() == 2
        && let (Some(Cirru::Leaf(head)), Some(Cirru::Leaf(name))) = (items.first(), items.get(1))
        && head.as_ref() == "quote"
      {
        out.insert(name.to_string());
        return;
      }
      for item in items.iter() {
        collect_type_vars(item, out);
      }
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

fn looks_like_undeclared_type_var(name: &str) -> bool {
  name.len() == 1 && name.as_bytes()[0].is_ascii_uppercase()
}

/// Allowed primitive tag types usable as a bare leaf schema (e.g. `:string`, `:number`).
pub const PRIMITIVE_SCHEMA_TAGS: &[&str] = &[
  "any",
  "bool",
  "number",
  "string",
  "symbol",
  "tag",
  "list",
  "map",
  "set",
  "fn",
  "tuple",
  "ref",
  "buffer",
  "dynamic",
  "unit",
  "record",
  "struct",
  "enum",
  "struct-def",
  "enum-def",
  "trait",
  "impl",
];

const PARAMETERIZED_SCHEMA_TAGS: &[&str] = &["list", "map", "set", "fn", "ref"];

fn canonical_schema_symbol_from_cirru(node: &Cirru) -> Option<&'static str> {
  let Cirru::Leaf(value) = node else {
    return None;
  };
  CalcitTypeAnnotation::canonical_type_symbol_name(value.trim_start_matches('\''))
}

fn is_qualified_nominal_schema_ref(value: &str) -> bool {
  let Some(name) = value.strip_prefix('\'') else {
    return false;
  };
  let Some((namespace, definition)) = name.rsplit_once('/') else {
    return false;
  };
  !namespace.is_empty() && !definition.is_empty()
}

fn check_no_legacy_data_type_names(schema: &Cirru) -> Result<(), String> {
  match schema {
    Cirru::Leaf(value) => {
      let name = value.trim_start_matches(['\'', ':']);
      let replacement = match name {
        "record" | "Record" => Some("Struct"),
        "tuple" | "Tuple" => Some("Enum"),
        _ => None,
      };
      if let Some(replacement) = replacement {
        return Err(format!(
          "Legacy type name `{name}` was removed by the struct/enum data-model migration; use `'{replacement}`."
        ));
      }
      Ok(())
    }
    Cirru::List(items) => {
      for item in items {
        check_no_legacy_data_type_names(item)?;
      }
      Ok(())
    }
  }
}

fn validate_standalone_type_schema(schema: &Cirru) -> Result<(), String> {
  parse_schema_data(schema)?;
  check_no_nil_type(schema)?;
  check_no_excess_quotes(schema)?;

  if let Cirru::List(items) = schema
    && matches!(items.first(), Some(Cirru::Leaf(head)) if head.as_ref() == "::")
    && let Some(Cirru::Leaf(type_name)) = items.get(1)
    && canonical_schema_symbol_from_cirru(&items[1]).is_none()
    && !is_qualified_nominal_schema_ref(type_name)
  {
    return Err(format!(
      "Unknown standalone type `{type_name}`. Use a built-in type name or a fully qualified nominal type such as `'app.schema/Store`."
    ));
  }

  let schema_edn = schema_cirru_to_edn(schema.clone());
  if matches!(schema_edn, Edn::Nil) {
    return Err("Failed to convert standalone type schema into EDN".to_owned());
  }
  let annotation = CalcitTypeAnnotation::parse_type_annotation_from_edn(&schema_edn);
  if matches!(annotation.as_ref(), CalcitTypeAnnotation::Dynamic)
    && matches!(schema, Cirru::List(items) if items.len() == 2 && matches!(items.first(), Some(Cirru::Leaf(marker)) if marker.as_ref() == "::") && items.get(1).and_then(canonical_schema_symbol_from_cirru) == Some("Dynamic"))
  {
    return Ok(());
  }
  if matches!(annotation.as_ref(), CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::Tag) {
    return Err(format!(
      "Unsupported standalone type schema: {}",
      cirru_parser::format(std::slice::from_ref(schema), true.into()).unwrap_or_else(|_| format!("{schema:?}"))
    ));
  }
  Ok(())
}

/// Strict validation for schemas submitted via `calcit edit schema`.
/// New writes use one canonical form: direct value types or wrapped
/// `(:: :fn ({} ...))` / `(:: :macro ({} ...))` callable schemas. Loading
/// existing snapshots remains deliberately more permissive.
pub fn validate_schema_for_write(schema: &Cirru) -> Result<(), String> {
  check_no_legacy_data_type_names(schema)?;
  let raw_items = match schema {
    Cirru::List(items) => items,
    Cirru::Leaf(s) => {
      let tag_name = s.trim_start_matches(':');
      if let Some(canonical) = canonical_schema_symbol_from_cirru(schema) {
        let parameterized = matches!(canonical, "List" | "Map" | "Set" | "Fn" | "Ref");
        if !parameterized {
          return Ok(());
        }
        return Err(format!(
          "Bare `'{canonical}` leaves its nested type dynamic. Use an explicit type expression such as `:: '{canonical} 'Bool`; write `'Dynamic` as a nested type only when the boundary is intentionally dynamic."
        ));
      }
      if is_qualified_nominal_schema_ref(s) {
        check_no_excess_quotes(schema)?;
        let schema_edn = schema_cirru_to_edn(schema.clone());
        let annotation = CalcitTypeAnnotation::parse_type_annotation_from_edn(&schema_edn);
        if matches!(
          annotation.as_ref(),
          CalcitTypeAnnotation::TypeRef(name, args)
            if name.as_ref() == s.trim_start_matches('\'') && args.is_empty()
        ) {
          return Ok(());
        }
        return Err(format!("Failed to parse fully qualified nominal value schema `{s}`"));
      }
      if PARAMETERIZED_SCHEMA_TAGS.contains(&tag_name) {
        let example = match tag_name {
          "map" => ":: :map :tag :bool",
          "fn" => ":: :fn $ {} (:args $ []) (:return :unit)",
          other => {
            return Err(format!(
              "Bare `:{other}` leaves its nested type dynamic. Use an explicit type expression such as `:: :{other} :bool`; write `:dynamic` as the nested type only when the boundary is intentionally dynamic."
            ));
          }
        };
        return Err(format!(
          "Bare `:{tag_name}` leaves its nested type dynamic. Use an explicit type expression such as `{example}`; write `:dynamic` as a nested type only when the boundary is intentionally dynamic."
        ));
      }
      if PRIMITIVE_SCHEMA_TAGS.contains(&tag_name) {
        return Ok(());
      }
      return Err(format!(
        "Unknown value schema `{s}`. Use a direct type such as `'String`, a fully qualified nominal type such as `'app.schema/Store`, a parameterized value type such as `:: 'Ref 'Bool`, or a callable schema such as `:: 'Fn $ {{}} (:args $ []) (:return 'Unit)`."
      ));
    }
  };

  let items: &[Cirru] = if matches!(raw_items.first(), Some(Cirru::Leaf(head)) if head.as_ref() == "::") {
    let is_function_schema = raw_items
      .get(1)
      .and_then(canonical_schema_symbol_from_cirru)
      .is_some_and(|name| matches!(name, "Fn" | "Macro"));
    if !is_function_schema {
      return validate_standalone_type_schema(schema);
    }
    if raw_items.len() != 3 {
      return Err("Wrapped schema `(:: :fn schema-map)` or `(:: :macro schema-map)` expects exactly 3 items".to_owned());
    }
    match (&raw_items[1], &raw_items[2]) {
      (tag, Cirru::List(inner_items)) if canonical_schema_symbol_from_cirru(tag).is_some_and(|name| matches!(name, "Fn" | "Macro")) => {
        inner_items
      }
      (Cirru::Leaf(tag), _) => {
        return Err(format!(
          "Wrapped schema type must be `'Fn` or `'Macro`, got: `{tag}`. Example: `(:: 'Fn ({{}} (:args ([] 'String)) (:return 'Bool)))`"
        ));
      }
      _ => return Err("Wrapped schema second item must be `:fn` or `:macro` and third item must be a `{}` map".to_owned()),
    }
  } else if matches!(raw_items.first(), Some(Cirru::Leaf(head)) if head.as_ref() == "{}") {
    return Err(
      "Legacy unwrapped callable schema maps are not accepted by `calcit edit schema`. Use the canonical wrapped form `:: :fn $ {} ...` or `:: :macro $ {} ...`."
        .to_owned(),
    );
  } else {
    return validate_standalone_type_schema(schema);
  };

  for pair in items.iter().skip(1) {
    if matches!(pair, Cirru::List(xs) if matches!(xs.first(), Some(Cirru::Leaf(key)) if key.as_ref() == ":kind")) {
      return Err(
        "Wrapped callable schemas must not repeat `:kind`. Keep the outer `:: :fn` or `:: :macro` tag and remove the inner `(:kind ...)` field."
          .to_owned(),
      );
    }
  }

  let Some(Cirru::Leaf(head)) = items.first() else {
    return Err("Schema must be a non-empty list starting with `{}`".to_owned());
  };

  if head.as_ref() != "{}" {
    return Err(format!(
      "Schema top-level must start with `{{}}` or be wrapped as `(:: :fn ({{}} ...))` / `(:: :macro ({{}} ...))`, got: `{head}`. \
       Example: `(:: :fn ({{}} (:args ([] :string)) (:return :bool)))`"
    ));
  }

  // Reject deprecated :nil type annotation
  check_no_nil_type(schema)?;

  // Reject excess-quoted type variables like ''T.
  check_no_excess_quotes(schema)?;

  // Field-level validation
  for pair in items.iter().skip(1) {
    let Cirru::List(xs) = pair else {
      let text = cirru_parser::format(std::slice::from_ref(pair), true.into()).unwrap_or_else(|_| format!("{pair:?}"));
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
  }

  // --- Type-variable consistency check ---
  // Collect declared generics, args, and return from the schema pairs.
  let mut generics_node: Option<&Cirru> = None;
  let mut args_node: Option<&Cirru> = None;
  let mut return_node: Option<&Cirru> = None;
  let mut rest_node: Option<&Cirru> = None;
  let mut required_node: Option<&Cirru> = None;
  let mut optional_node: Option<&Cirru> = None;
  let mut expansion_node: Option<&Cirru> = None;
  let mut capabilities_node: Option<&Cirru> = None;
  let mut legacy_origin_node: Option<&Cirru> = None;
  let mut where_node: Option<&Cirru> = None;
  let mut features_node: Option<&Cirru> = None;

  for pair in items.iter().skip(1) {
    if let Cirru::List(xs) = pair
      && let (Some(Cirru::Leaf(key)), Some(val)) = (xs.first(), xs.get(1))
    {
      match key.as_ref() {
        ":generics" => generics_node = Some(val),
        ":args" => args_node = Some(val),
        ":return" => return_node = Some(val),
        ":required" => required_node = Some(val),
        ":optional" => optional_node = Some(val),
        ":expansion" => expansion_node = Some(val),
        ":capabilities" => capabilities_node = Some(val),
        ":legacy-origin" => legacy_origin_node = Some(val),
        ":rest" => rest_node = Some(val),
        ":where" => where_node = Some(val),
        ":features" => features_node = Some(val),
        _ => {}
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
    for node in [required_node, optional_node, expansion_node].into_iter().flatten() {
      collect_type_vars(node, &mut used);
    }
    if let Some(node) = where_node {
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
      if !declared.contains(var) && looks_like_undeclared_type_var(var) {
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
    for node in [required_node, optional_node, expansion_node].into_iter().flatten() {
      collect_type_vars(node, &mut used);
    }
    if let Some(node) = where_node {
      collect_type_vars(node, &mut used);
    }
    if let Some(var) = used.iter().find(|name| looks_like_undeclared_type_var(name)) {
      return Err(format!("Type variable `'{var}` is used but no `:generics` field is declared."));
    }
  }

  if let Some(capabilities_val) = capabilities_node {
    let Cirru::List(items) = capabilities_val else {
      return Err("`:capabilities` must be a hashset like `(#{} :env-read :fs-read)`".to_owned());
    };
    if !matches!(items.first(), Some(Cirru::Leaf(first)) if first.as_ref() == "#{}") {
      return Err("`:capabilities` must be a hashset like `(#{} :env-read :fs-read)`".to_owned());
    }
    for item in items.iter().skip(1) {
      let Cirru::Leaf(name) = item else {
        return Err("`:capabilities` hashset items must be simple leaf tags".to_owned());
      };
      if !name.starts_with(':') {
        return Err(format!("Macro capability `{name}` must be a colon-prefixed tag"));
      }
      if crate::calcit::MacroCapability::parse(name).is_none() {
        return Err(format!(
          "Unknown macro capability `{name}`. Expected one of: :env-read, :fs-read, :platform-read, :clock-read, :log, :mutable-state, :dynamic-eval, :fs-write, :process, :host-ffi"
        ));
      }
    }
  }

  if let Some(legacy_origin) = legacy_origin_node
    && !matches!(legacy_origin, Cirru::Leaf(value) if matches!(value.as_ref(), ":fn" | ":dynamic"))
  {
    return Err("`:legacy-origin` must be `:fn` or `:dynamic`".to_owned());
  }

  // Validate :features value — must be a hashset of tags
  if let Some(features_val) = features_node {
    match features_val {
      Cirru::List(items) => {
        // Check it's a hashset: `(#{} tag1 tag2 ...)`
        let Some(Cirru::Leaf(first)) = items.first() else {
          return Err("`:features` must be a hashset like `(#{} :tag1 :tag2)`".to_owned());
        };
        if first.as_ref() != "#{}" {
          return Err("`:features` must be a hashset like `(#{} :tag1 :tag2)`".to_owned());
        }
        for item in items.iter().skip(1) {
          if !matches!(item, Cirru::Leaf(_)) {
            return Err("`:features` hashset items must be simple leaf tags".to_owned());
          }
        }
      }
      _ => {
        return Err("`:features` must be a hashset like `(#{} :tag1 :tag2)`".to_owned());
      }
    }
  }

  // Run the general EDN parser after field-specific checks so malformed
  // capability metadata receives the stable schema diagnostic above.
  parse_schema_data(schema)?;

  Ok(())
}

impl From<CodeEntry> for Edn {
  fn from(data: CodeEntry) -> Self {
    Edn::struct_from_pairs("CodeEntry", &code_entry_edn_pairs(&data))
  }
}

/// Validate and parse one schema submitted through `calcit edit schema`.
/// Leaf schemas cannot be round-tripped through `parse_schema_data`, whose
/// Cirru formatter requires a top-level expression, so they are converted
/// directly into EDN before type parsing.
pub fn parse_schema_annotation_for_write(schema: &Cirru) -> Result<Arc<CalcitTypeAnnotation>, String> {
  validate_schema_for_write(schema)?;
  if !matches!(schema, Cirru::Leaf(_)) {
    parse_schema_data(schema)?;
  }
  let schema_edn = schema_cirru_to_edn(schema.clone());
  if let Some(annotation) = parse_zero_payload_schema_wrapper(&schema_edn) {
    return Ok(annotation);
  }
  if let Some(signature) = CalcitTypeAnnotation::parse_macro_signature_from_edn(&schema_edn) {
    return Ok(Arc::new(CalcitTypeAnnotation::Macro(Arc::new(signature))));
  }
  if let Some(signature) = CalcitTypeAnnotation::parse_fn_schema_from_edn(&schema_edn) {
    if matches!(signature.fn_kind, SchemaKind::Macro) {
      return Err(
        "legacy `:kind :macro` function schemas are no longer writable; declare a strict `Macro` contract with :required/:optional/:rest, :expansion, and :capabilities instead"
          .to_owned(),
      );
    }
    return Ok(Arc::new(CalcitTypeAnnotation::Fn(Arc::new(signature))));
  }
  Ok(CalcitTypeAnnotation::parse_type_annotation_from_edn(&schema_edn))
}

impl From<&CodeEntry> for Edn {
  fn from(data: &CodeEntry) -> Self {
    Edn::struct_from_pairs("CodeEntry", &code_entry_edn_pairs(data))
  }
}

impl CodeEntry {
  pub fn from_code(code: Cirru) -> Self {
    CodeEntry {
      doc: "".to_owned(),
      examples: vec![],
      tests: vec![],
      tags: HashSet::new(),
      code,
      schema: DYNAMIC_TYPE.clone(),
      ffi: None,
    }
  }
}

fn code_declares_macro(code: &Cirru) -> bool {
  matches!(code, Cirru::List(items) if matches!(items.first(), Some(Cirru::Leaf(head)) if head.as_ref() == "defmacro"))
}

fn normalize_schema_for_code(code: &Cirru, schema: &Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  // Data declarations are definition values, not untyped application data.
  // Older snapshots stored their root schema as Dynamic because the concrete
  // fields/variants/methods live in the source form. Keep that compatibility
  // on load, but immediately canonicalize it to the existing definition-kind
  // markers so Dynamic metrics only describe genuinely unknown slots.
  if matches!(schema.as_ref(), CalcitTypeAnnotation::Dynamic)
    && let Cirru::List(items) = code
    && let Some(Cirru::Leaf(head)) = items.first()
  {
    let marker = match head.as_ref() {
      "defstruct" => Some("struct-def"),
      "defenum" => Some("enum-def"),
      "deftrait" => Some("trait"),
      "defimpl" => Some("impl"),
      _ => None,
    };
    if let Some(marker) = marker {
      return Arc::new(CalcitTypeAnnotation::Custom(Arc::new(Calcit::tag(marker))));
    }
  }

  schema.clone()
}

/// structure of canonical runtime snapshot files such as `calcit.cirru`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
  pub package: String,
  pub about: Option<String>,
  pub version: String,
  pub entries: HashMap<String, SnapshotEntry>,
  pub files: HashMap<String, FileInSnapShot>,
  #[serde(skip, default = "default_active_entry")]
  #[doc(hidden)]
  pub active_entry: String,
}

/// One-shot legacy conversions performed exclusively by `calcit edit format`.
/// Runtime loading stays strict so canonical snapshots cannot depend on these
/// compatibility branches.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SnapshotFormatMigration {
  pub direct_quote_namespaces: usize,
  pub direct_quote_definitions: usize,
  pub legacy_configs: bool,
}

impl SnapshotFormatMigration {
  pub fn happened(self) -> bool {
    self.legacy_configs || self.direct_quote_namespaces > 0 || self.direct_quote_definitions > 0
  }
}

impl Snapshot {
  pub fn active_entry_name(&self) -> &str {
    &self.active_entry
  }

  pub fn active_entry(&self) -> Result<&SnapshotEntry, String> {
    self
      .entries
      .get(&self.active_entry)
      .ok_or_else(|| format!("Snapshot is missing active entry '{}'", self.active_entry))
  }

  pub fn select_entry(&mut self, entry: Option<&str>) -> Result<(), String> {
    let name = entry.unwrap_or(DEFAULT_ENTRY_NAME);
    if self.entries.contains_key(name) {
      self.active_entry = name.to_owned();
      Ok(())
    } else {
      let mut available = self.entries.keys().cloned().collect::<Vec<_>>();
      available.sort();
      Err(format!("Unknown entry `{name}`. Available entries: {}", available.join(", ")))
    }
  }
}

impl TryFrom<Edn> for SnapshotEntry {
  type Error = String;
  fn try_from(data: Edn) -> Result<SnapshotEntry, String> {
    parse_snapshot_entry_with_context(data, "entry", true)
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
      "{owner}.version cannot be empty; check the project `:version`; got {}",
      format_edn_preview(value)
    ));
  }

  Ok(text.to_string())
}

/// Entry functions identify Calcit definitions, not text values. Both strings
/// and symbols remain readable for compatibility, while writers use symbols.
fn parse_snapshot_ns_def_field(data: &EdnMapView, key: &str, owner: &str) -> Result<String, String> {
  let value = data.get(&Edn::tag(key)).ok_or_else(|| format!("{owner}: missing `:{key}` field"))?;
  match value {
    Edn::Str(text) | Edn::Symbol(text) => Ok(text.to_string()),
    _ => Err(format!(
      "{owner}.{key}: expected a namespace/definition string or symbol; got {}",
      format_edn_preview(value)
    )),
  }
}

fn parse_optional_snapshot_config_string_field(data: &EdnMapView, key: &str, owner: &str) -> Result<String, String> {
  match data.get(&Edn::tag(key)) {
    Some(_) => parse_snapshot_config_string_field(data, key, owner),
    None => Ok(String::new()),
  }
}

fn parse_snapshot_run_mode(data: &EdnMapView, owner: &str, require_mode: bool) -> Result<SnapshotRunMode, String> {
  let Some(value) = data.get(&Edn::tag("mode")) else {
    return if require_mode {
      Err(format!("{owner}: missing `:mode` field; expected `:native` or `:js`"))
    } else {
      Ok(SnapshotRunMode::Native)
    };
  };
  let mode = match value {
    Edn::Tag(tag) => tag.ref_str(),
    Edn::Str(text) | Edn::Symbol(text) => text.trim_start_matches(':'),
    _ => {
      return Err(format!(
        "{owner}.mode: expected `:native` or `:js`, got {}",
        format_edn_preview(value)
      ));
    }
  };
  match mode {
    "native" => Ok(SnapshotRunMode::Native),
    "js" => Ok(SnapshotRunMode::Js),
    _ => Err(format!("{owner}.mode: expected `:native` or `:js`, got `{mode}`")),
  }
}

fn parse_snapshot_entry_with_context(data: Edn, owner: &str, require_mode: bool) -> Result<SnapshotEntry, String> {
  let data = data
    .view_map()
    .map_err(|e| format!("{owner}: failed to parse entry map: {e}; got {}", format_edn_preview(&data)))?;

  let mode = parse_snapshot_run_mode(&data, owner, require_mode)?;
  let init_fn = parse_snapshot_ns_def_field(&data, "init-fn", owner)?;
  let reload_fn = parse_snapshot_ns_def_field(&data, "reload-fn", owner)?;
  let description = parse_optional_snapshot_config_string_field(&data, "description", owner)?;

  let modules = match data.get(&Edn::tag("modules")) {
    Some(value) => from_edn(value.to_owned()).map_err(|e| format!("{owner}.modules: {e}; got {}", format_edn_preview(value)))?,
    None => Vec::new(),
  };

  let type_slots = match data.get(&Edn::tag("type-slots")) {
    Some(value) => parse_snapshot_type_slots(value, owner)?,
    None => HashMap::new(),
  };
  let feature_policy = match data.get(&Edn::tag("feature-policy")) {
    Some(value) => parse_snapshot_feature_policy(value, owner)?,
    None => HashMap::new(),
  };
  let target = match data.get(&Edn::tag("target")) {
    Some(value) => Some(parse_snapshot_target(value, owner)?),
    None => None,
  };

  Ok(SnapshotEntry {
    mode,
    init_fn,
    reload_fn,
    description,
    modules,
    type_slots,
    feature_policy,
    target,
  })
}

fn parse_snapshot_target(value: &Edn, owner: &str) -> Result<SnapshotTarget, String> {
  let target = match value {
    Edn::Tag(tag) => tag.ref_str(),
    Edn::Str(text) | Edn::Symbol(text) => text.trim_start_matches(':'),
    _ => {
      return Err(format!(
        "{owner}.target: expected :browser, :node, :native, or :wasm, got {}",
        format_edn_preview(value)
      ));
    }
  };
  match target {
    "browser" => Ok(SnapshotTarget::Browser),
    "node" => Ok(SnapshotTarget::Node),
    "native" => Ok(SnapshotTarget::Native),
    "wasm" => Ok(SnapshotTarget::Wasm),
    _ => Err(format!(
      "{owner}.target: expected :browser, :node, :native, or :wasm, got `{target}`"
    )),
  }
}

fn parse_snapshot_feature_policy(data: &Edn, owner: &str) -> Result<HashMap<String, FeaturePolicy>, String> {
  let policies = data
    .view_map()
    .map_err(|e| format!("{owner}.feature-policy: expected a map: {e}; got {}", format_edn_preview(data)))?;
  let mut result = HashMap::with_capacity(policies.0.len());
  for (raw_feature, raw_policy) in policies.0.iter() {
    let feature = match raw_feature {
      Edn::Tag(tag) => tag.ref_str().to_owned(),
      Edn::Str(text) | Edn::Symbol(text) => text.trim_start_matches(':').to_owned(),
      _ => {
        return Err(format!(
          "{owner}.feature-policy: feature name must be a tag, string, or symbol; got {}",
          format_edn_preview(raw_feature)
        ));
      }
    };
    if feature.trim().is_empty() {
      return Err(format!("{owner}.feature-policy: feature name cannot be empty"));
    }
    let policy_name = match raw_policy {
      Edn::Tag(tag) => tag.ref_str(),
      Edn::Str(text) | Edn::Symbol(text) => text.trim_start_matches(':'),
      _ => {
        return Err(format!(
          "{owner}.feature-policy.{feature}: expected :allow, :warn, or :error; got {}",
          format_edn_preview(raw_policy)
        ));
      }
    };
    let policy = match policy_name {
      "allow" => FeaturePolicy::Allow,
      "warn" => FeaturePolicy::Warn,
      "error" => FeaturePolicy::Error,
      _ => {
        return Err(format!(
          "{owner}.feature-policy.{feature}: expected :allow, :warn, or :error, got `{policy_name}`"
        ));
      }
    };
    if result.insert(feature.clone(), policy).is_some() {
      return Err(format!("{owner}.feature-policy: duplicate feature `:{feature}`"));
    }
  }
  Ok(result)
}

fn parse_snapshot_type_slots(data: &Edn, owner: &str) -> Result<HashMap<String, String>, String> {
  let slots = data
    .view_map()
    .map_err(|e| format!("{owner}.type-slots: expected a map: {e}; got {}", format_edn_preview(data)))?;
  let mut result = HashMap::with_capacity(slots.0.len());

  for (raw_slot, raw_type) in slots.0.iter() {
    let slot = match raw_slot {
      Edn::Tag(tag) => tag.ref_str().to_owned(),
      Edn::Str(text) | Edn::Symbol(text) => text.trim_start_matches(':').to_owned(),
      _ => {
        return Err(format!(
          "{owner}.type-slots: slot name must be a tag, string, or symbol; got {}",
          format_edn_preview(raw_slot)
        ));
      }
    };
    if slot.is_empty() {
      return Err(format!("{owner}.type-slots: slot name cannot be empty"));
    }

    let type_path = match raw_type {
      Edn::Str(text) | Edn::Symbol(text) if text.as_ref() == "Dynamic" => ":dynamic".to_owned(),
      Edn::Str(text) | Edn::Symbol(text) => text.to_string(),
      Edn::Tag(tag) if tag.ref_str() == "dynamic" => ":dynamic".to_owned(),
      _ => {
        return Err(format!(
          "{owner}.type-slots.{slot}: type must be a full `namespace/definition` string or `:dynamic`; got {}",
          format_edn_preview(raw_type)
        ));
      }
    };
    if result.insert(slot.clone(), type_path).is_some() {
      return Err(format!("{owner}.type-slots: duplicate slot name `:{slot}`"));
    }
  }

  Ok(result)
}

fn parse_entries_with_context(data: &Edn, require_mode: bool) -> Result<HashMap<String, SnapshotEntry>, String> {
  let entries_map = data
    .view_map()
    .map_err(|e| format!("entries: failed to parse entries map: {e}; got {}", format_edn_preview(data)))?;

  let mut entries = HashMap::with_capacity(entries_map.0.len());
  for (entry_key, entry_value) in entries_map.0.iter() {
    let entry_name: String = from_edn(entry_key.to_owned())
      .map_err(|e| format!("entries: failed to parse entry name: {e}; got {}", format_edn_preview(entry_key)))?;
    let owner = format!("entries.{entry_name}");
    let entry = parse_snapshot_entry_with_context(entry_value.to_owned(), &owner, require_mode)?;
    entries.insert(entry_name, entry);
  }

  Ok(entries)
}

fn legacy_snapshot_recovery_hint(path: &str) -> Option<String> {
  let snapshot_path = Path::new(path);
  let compact_path = snapshot_path.parent()?.join("compact.cirru");
  if snapshot_path.file_name()?.to_str()? == "calcit.cirru" && compact_path.is_file() {
    Some(format!(
      "A sibling `{}` exists. If it is the last runnable compact Snapshot, back up this `calcit.cirru`, copy `compact.cirru` over it, then run `calcit calcit.cirru edit format` before `calcit calcit.cirru --check-only`.",
      compact_path.display()
    ))
  } else {
    None
  }
}

/// Build migration guidance when `path` uses the retired snapshot filename.
pub fn retired_snapshot_migration_error(path: &Path) -> Option<String> {
  if path.file_name().and_then(|name| name.to_str()) != Some(crate::LEGACY_SNAPSHOT_FILE) {
    return None;
  }

  let canonical_path = path.with_file_name(crate::DEFAULT_SNAPSHOT_FILE);
  Some(format!(
    "Snapshot filename `{}` is retired. Copy or rename the last runnable snapshot to `{}`, then run `calcit {} edit format` and `calcit {} --check-only`. The published Calcit 0.13.48 release is the final release that accepts the old filename.",
    crate::LEGACY_SNAPSHOT_FILE,
    canonical_path.display(),
    canonical_path.display(),
    canonical_path.display()
  ))
}

/// Build migration guidance for the retired top-level `:configs` shape.
pub fn retired_snapshot_configs_error(path: &str) -> String {
  let path_arg = format!("'{}'", path.replace('\'', "'\"'\"'"));
  format!(
    "Top-level `:configs` is retired in Snapshot `{path}`. Run `calcit {path_arg} edit format` with the current Calcit to perform the isolated one-way migration, review the generated `:entries.default` with `calcit {path_arg} config show`, then retry `calcit {path_arg} --check-only`. Runtime loading remains strict outside `edit format`."
  )
}

/// Parse a Snapshot while preserving the source path in deserialization errors.
pub fn load_snapshot_data(data: &Edn, path: &str) -> Result<Snapshot, String> {
  if let Some(error) = retired_snapshot_migration_error(Path::new(path)) {
    return Err(error);
  }
  load_snapshot_data_inner(data, path).map_err(|error| {
    let mut message = format!("Failed to load Snapshot `{path}`: {error}");
    if let Some(hint) = legacy_snapshot_recovery_hint(path) {
      message.push_str("\nLegacy Snapshot recovery: ");
      message.push_str(&hint);
    }
    message
  })
}

/// Parse a Snapshot for canonical formatting, including the constrained
/// direct-quote/configs shape used by early compact snapshots.
///
/// This is intentionally separate from [`load_snapshot_data`]: callers other
/// than `edit format` must not gain an implicit legacy runtime path.
pub fn load_snapshot_data_for_format(data: &Edn, path: &str) -> Result<(Snapshot, SnapshotFormatMigration), String> {
  if let Some(error) = retired_snapshot_migration_error(Path::new(path)) {
    return Err(error);
  }
  load_snapshot_data_for_format_inner(data, path).map_err(|error| format!("Failed to load Snapshot `{path}` for formatting: {error}"))
}

fn load_snapshot_data_for_format_inner(data: &Edn, path: &str) -> Result<(Snapshot, SnapshotFormatMigration), String> {
  let data = data.view_map()?;
  let mut migration = SnapshotFormatMigration::default();
  let pkg: Arc<str> = data.get_or_nil("package").try_into()?;
  let mut files = parse_files_for_format_with_context(&data.get_or_nil("files"), &mut migration)?;

  // A direct-quote macro has no schema to validate yet. Formatting writes an
  // explicit Dynamic schema, after which the existing strict-loader guidance
  // points users at the final schema-migration release.
  if migration.direct_quote_definitions == 0 {
    validate_strict_macro_schemas(&files, path)?;
  }

  let about = match data.get_or_nil("about") {
    Edn::Nil => None,
    value => {
      let s: Arc<str> = value.try_into()?;
      Some(s.to_string())
    }
  };
  let meta_ns = format!("{pkg}.$meta");
  files.insert(meta_ns.to_owned(), gen_meta_ns(&meta_ns, path));

  let entries_value = data.get_or_nil("entries");
  let mut entries = if matches!(entries_value, Edn::Nil) {
    HashMap::new()
  } else {
    parse_entries_with_context(&entries_value, true)?
  };
  let mut legacy_version = None;
  if let Some(configs) = data.get(&Edn::tag("configs")).or_else(|| data.get(&Edn::str("configs"))) {
    if entries.contains_key(DEFAULT_ENTRY_NAME) {
      return Err("legacy `:configs` conflicts with existing `:entries.default`; remove the ambiguity before formatting".to_owned());
    }
    let (entry, version) = parse_legacy_configs_for_format(configs)?;
    entries.insert(DEFAULT_ENTRY_NAME.to_owned(), entry);
    legacy_version = version;
    migration.legacy_configs = true;
  }

  if !entries.contains_key(DEFAULT_ENTRY_NAME) {
    return Err("Snapshot `:entries` must contain a `:default` entry".to_owned());
  }
  let version = match data.get(&Edn::tag("version")).or_else(|| data.get(&Edn::str("version"))) {
    Some(_) => parse_snapshot_config_string_field(&data, "version", "snapshot")?,
    None => legacy_version.unwrap_or_else(default_version),
  };

  Ok((
    Snapshot {
      package: pkg.to_string(),
      about,
      version,
      entries,
      files,
      active_entry: default_active_entry(),
    },
    migration,
  ))
}

fn parse_legacy_configs_for_format(data: &Edn) -> Result<(SnapshotEntry, Option<String>), String> {
  let configs = data
    .view_map()
    .map_err(|e| format!("legacy configs: expected a map: {e}; got {}", format_edn_preview(data)))?;
  for key in configs.0.keys() {
    let name = match key {
      Edn::Tag(tag) => tag.ref_str(),
      Edn::Str(text) | Edn::Symbol(text) => text.trim_start_matches(':'),
      _ => {
        return Err(format!(
          "legacy configs: field name must be a tag, string, or symbol; got {}",
          format_edn_preview(key)
        ));
      }
    };
    if !matches!(name, "init-fn" | "reload-fn" | "modules" | "version" | "mode") {
      return Err(format!(
        "legacy configs: unknown field `:{name}`; migrate it explicitly before formatting"
      ));
    }
  }
  let entry = parse_snapshot_entry_with_context(data.to_owned(), "legacy configs", false)?;
  let version = match configs.get(&Edn::tag("version")).or_else(|| configs.get(&Edn::str("version"))) {
    Some(_) => Some(parse_snapshot_config_string_field(&configs, "version", "legacy configs")?),
    None => None,
  };
  Ok((entry, version))
}

fn load_snapshot_data_inner(data: &Edn, path: &str) -> Result<Snapshot, String> {
  let data = data.view_map()?;
  if data.contains_key("configs") {
    return Err(retired_snapshot_configs_error(path));
  }
  let pkg: Arc<str> = data.get_or_nil("package").try_into()?;
  let mut files: HashMap<String, FileInSnapShot> = parse_files_with_context(&data.get_or_nil("files"))?;
  validate_strict_macro_schemas(&files, path)?;
  let about = match data.get_or_nil("about") {
    Edn::Nil => None,
    value => {
      let s: Arc<str> = value.try_into()?;
      Some(s.to_string())
    }
  };
  let meta_ns = format!("{pkg}.$meta");
  files.insert(meta_ns.to_owned(), gen_meta_ns(&meta_ns, path));
  let entries = parse_entries_with_context(&data.get_or_nil("entries"), true)?;
  let version = match data.get(&Edn::tag("version")) {
    Some(_) => parse_snapshot_config_string_field(&data, "version", "snapshot")?,
    None => default_version(),
  };

  if !entries.contains_key(DEFAULT_ENTRY_NAME) {
    return Err("Snapshot `:entries` must contain a `:default` entry".to_owned());
  }

  let s = Snapshot {
    package: pkg.to_string(),
    about,
    version,
    entries,
    files,
    active_entry: default_active_entry(),
  };
  Ok(s)
}

fn validate_strict_macro_schemas(files: &HashMap<String, FileInSnapShot>, path: &str) -> Result<(), String> {
  let mut namespaces = files.keys().collect::<Vec<_>>();
  namespaces.sort_unstable();
  for ns in namespaces {
    let file = &files[ns];
    let mut definitions = file.defs.keys().collect::<Vec<_>>();
    definitions.sort_unstable();
    for def_name in definitions {
      let entry = &file.defs[def_name];
      if !code_declares_macro(&entry.code) || matches!(entry.schema.as_ref(), CalcitTypeAnnotation::Macro(_)) {
        continue;
      }
      let found = match entry.schema.as_ref() {
        CalcitTypeAnnotation::Fn(_) => "a runtime Fn schema",
        CalcitTypeAnnotation::Dynamic => "a Dynamic schema",
        _ => "a non-Macro schema",
      };
      return Err(format!(
        "legacy macro schema at snapshot.files[{ns:?}].defs[{def_name:?}].schema: `defmacro` requires a strict `Macro` contract with :required/:optional/:rest, :expansion, and :capabilities, but found {found}. Migrate this definition with the final compatible Calcit 0.13.51 release, then retry `calcit '{path}' --check-only`."
      ));
    }
  }
  Ok(())
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
        let def_name = parse_snapshot_identifier_key(def_key, &format!("{file_name}/:defs"))?;
        let owner = format!("{file_name}/{def_name}");
        let entry = parse_code_entry_with_context(def_value.to_owned(), &owner)?;
        insert_snapshot_identifier(&mut defs, def_name, entry, &format!("{file_name}/:defs"))?;
      }

      Ok(FileInSnapShot { ns, defs })
    }
    Edn::Struct(struct_value) => {
      let mut ns: Option<NsEntry> = None;
      let mut defs = HashMap::new();

      for (key, value) in struct_value.pairs.iter() {
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
              let def_name = parse_snapshot_identifier_key(def_key, &format!("{file_name}/:defs"))?;
              let owner = format!("{file_name}/{def_name}");
              let entry = parse_code_entry_with_context(def_value.to_owned(), &owner)?;
              insert_snapshot_identifier(&mut defs, def_name, entry, &format!("{file_name}/:defs"))?;
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
      "{file_name}: expected FileEntry map/struct, got {}",
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
    let file_name = parse_snapshot_identifier_key(file_key, "snapshot/:files")?;
    let file = parse_file_in_snapshot_with_context(file_value.to_owned(), &file_name)?;
    insert_snapshot_identifier(&mut files, file_name, file, "snapshot/:files")?;
  }
  Ok(files)
}

fn parse_file_for_format_with_context(
  data: Edn,
  file_name: &str,
  migration: &mut SnapshotFormatMigration,
) -> Result<FileInSnapShot, String> {
  let (ns_value, defs_value) = match &data {
    Edn::Map(map) => (
      map.get(&Edn::tag("ns")).or_else(|| map.get(&Edn::str("ns"))),
      map.get(&Edn::tag("defs")).or_else(|| map.get(&Edn::str("defs"))),
    ),
    Edn::Struct(struct_value) => (
      struct_value
        .pairs
        .iter()
        .find(|(key, _)| key.ref_str() == "ns")
        .map(|(_, value)| value),
      struct_value
        .pairs
        .iter()
        .find(|(key, _)| key.ref_str() == "defs")
        .map(|(_, value)| value),
    ),
    other => {
      return Err(format!(
        "{file_name}: expected FileEntry map/struct, got {}",
        format_edn_preview(other)
      ));
    }
  };
  let ns_value = ns_value.ok_or_else(|| format!("{file_name}: missing `:ns` field in FileEntry"))?;
  let defs_value = defs_value.ok_or_else(|| format!("{file_name}: missing `:defs` field in FileEntry"))?;
  let ns = match ns_value {
    Edn::Quote(code) => {
      migration.direct_quote_namespaces += 1;
      NsEntry {
        doc: String::new(),
        code: code.clone(),
      }
    }
    modern => modern.to_owned().try_into().map_err(|e: String| format!("{file_name}/:ns: {e}"))?,
  };

  let defs_map = defs_value.view_map().map_err(|e| {
    format!(
      "{file_name}: failed to parse `:defs` as map: {e}; got {}",
      format_edn_preview(defs_value)
    )
  })?;
  let mut defs = HashMap::with_capacity(defs_map.0.len());
  for (def_key, def_value) in defs_map.0.iter() {
    let def_name = parse_snapshot_identifier_key(def_key, &format!("{file_name}/:defs"))?;
    let owner = format!("{file_name}/{def_name}");
    let entry = match def_value {
      Edn::Quote(code) => {
        migration.direct_quote_definitions += 1;
        CodeEntry::from_code(code.clone())
      }
      modern => parse_code_entry_with_context(modern.to_owned(), &owner)?,
    };
    insert_snapshot_identifier(&mut defs, def_name, entry, &format!("{file_name}/:defs"))?;
  }
  Ok(FileInSnapShot { ns, defs })
}

fn parse_files_for_format_with_context(
  data: &Edn,
  migration: &mut SnapshotFormatMigration,
) -> Result<HashMap<String, FileInSnapShot>, String> {
  let files_map = data
    .view_map()
    .map_err(|e| format!("failed to parse snapshot `:files` as map: {e}; got {}", format_edn_preview(data)))?;
  let mut files = HashMap::with_capacity(files_map.0.len());
  for (file_key, file_value) in files_map.0.iter() {
    let file_name = parse_snapshot_identifier_key(file_key, "snapshot/:files")?;
    let file = parse_file_for_format_with_context(file_value.to_owned(), &file_name, migration)?;
    insert_snapshot_identifier(&mut files, file_name, file, "snapshot/:files")?;
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
    let default_entry = SnapshotEntry {
      mode: SnapshotRunMode::Native,
      init_fn: "app.main/main!".into(),
      reload_fn: "app.main/reload!".into(),
      description: String::new(),
      modules: vec![],
      type_slots: HashMap::new(),
      feature_policy: HashMap::new(),
      target: None,
    };
    Snapshot {
      package: "app".into(),
      about: Some(SNAPSHOT_ABOUT_MESSAGE.to_string()),
      version: default_version(),
      entries: HashMap::from([(DEFAULT_ENTRY_NAME.to_owned(), default_entry)]),
      files: HashMap::new(),
      active_entry: default_active_entry(),
    }
  }
}

/// Keywords that introduce a named top-level definition in Calcit.
/// When a snippet contains multiple such forms, each is extracted as its own
/// `CodeEntry` so the type-checker can inspect them individually (no-run mode).
const TOP_LEVEL_DEF_HEADS: &[&str] = &[
  "def",
  "defn",
  "defwasm-export",
  "defwasm-import",
  "defcomp",
  "defeffect",
  "defatom",
  "defstruct",
  "defenum",
  "defmacro",
  "defrecord",
];

/// Extract the binding name from a top-level definition form.
/// Returns `Some(name)` for recognised `(def name ...)` / `(defn name args ...)` etc.
fn extract_def_name(items: &[Cirru]) -> Option<&str> {
  match (items.first(), items.get(1)) {
    (Some(Cirru::Leaf(head)), Some(Cirru::Leaf(name))) if TOP_LEVEL_DEF_HEADS.contains(&head.as_ref()) => Some(name.as_ref()),
    _ => None,
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

      let body_lines: Vec<Cirru> = lines.into_iter().skip(body_start).collect();

      // If every body line is a top-level definition (def/defn/defcomp/…), promote
      // each to its own CodeEntry.  This lets the type-checker handle multi-def
      // snippets that appear in documentation (no-run mode).
      let all_top_level = !body_lines.is_empty()
        && body_lines.iter().all(|line| {
          if let Cirru::List(items) = line {
            extract_def_name(items).is_some()
          } else {
            false
          }
        });

      let mut def_dict: HashMap<String, CodeEntry> = HashMap::with_capacity(body_lines.len() + 2);

      if all_top_level {
        for line in &body_lines {
          if let Cirru::List(items) = line
            && let Some(name) = extract_def_name(items)
          {
            def_dict.insert(name.to_owned(), CodeEntry::from_code(line.clone()));
          }
        }
        // Each def is registered as its own CodeEntry so the type-checker can
        // analyse multi-def snippets individually.  A no-op main! is still
        // required so run_eval_in_process (Run mode) can find the entry point.
      } else {
        let mut func_code = vec![Cirru::leaf("defn"), "main!".into(), Cirru::List(vec![])];
        for line in body_lines {
          func_code.push(line);
        }
        def_dict.insert("main!".into(), CodeEntry::from_code(Cirru::List(func_code)));
      }

      def_dict
        .entry("main!".to_string())
        .or_insert_with(|| CodeEntry::from_code(vec![Cirru::leaf("defn"), "main!".into(), Cirru::List(vec![])].into()));
      def_dict
        .entry("reload!".to_string())
        .or_insert_with(|| CodeEntry::from_code(vec![Cirru::leaf("defn"), "reload!".into(), Cirru::List(vec![])].into()));

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

fn type_slots_to_edn(type_slots: &HashMap<String, String>) -> Edn {
  let mut slots_map = EdnMapView::default();
  let mut slots: Vec<(&String, &String)> = type_slots.iter().collect();
  slots.sort_by_key(|(slot, _)| *slot);
  for (slot, type_path) in slots {
    let value = if type_path == ":dynamic" {
      Edn::Symbol(Arc::from("Dynamic"))
    } else {
      Edn::Str(type_path.as_str().into())
    };
    slots_map.insert_key(slot.as_str(), value);
  }
  slots_map.into()
}

fn feature_policy_to_edn(feature_policy: &HashMap<String, FeaturePolicy>) -> Edn {
  let mut policies = EdnMapView::default();
  let mut items = feature_policy.iter().collect::<Vec<_>>();
  items.sort_by_key(|(feature, _)| *feature);
  for (feature, policy) in items {
    policies.insert_key(feature.as_str(), Edn::tag(policy.as_str()));
  }
  policies.into()
}

fn canonicalize_legacy_type_leaf(node: &Cirru) -> Option<Cirru> {
  let Cirru::Leaf(value) = node else {
    return None;
  };
  let legacy_name = value.strip_prefix(':')?;
  let canonical = CalcitTypeAnnotation::canonical_type_symbol_name(legacy_name)?;
  Some(Cirru::leaf(format!("'{canonical}")))
}

fn canonicalize_type_expression(node: &Cirru) -> (Cirru, usize) {
  if let Some(canonical) = canonicalize_legacy_type_leaf(node) {
    return (canonical, 1);
  }
  match node {
    Cirru::Leaf(_) => (node.clone(), 0),
    Cirru::List(items) => {
      let implicit_constructor = items.first().and_then(canonical_schema_symbol_from_cirru).is_some()
        && !matches!(items.first(), Some(Cirru::Leaf(head)) if head.as_ref() == "::");
      let mut rewritten = Vec::with_capacity(items.len());
      let mut changed = 0;
      if implicit_constructor {
        rewritten.push(Cirru::leaf("::"));
      }
      for (index, item) in items.iter().enumerate() {
        let (next, count) = canonicalize_type_expression(item);
        rewritten.push(next);
        changed += count;
        if implicit_constructor && index == 0 && canonicalize_legacy_type_leaf(item).is_none() {
          changed += 1;
        }
      }
      (Cirru::List(rewritten), changed)
    }
  }
}

fn canonicalize_schema_map_types(node: &Cirru) -> (Cirru, usize) {
  let Cirru::List(items) = node else {
    return (node.clone(), 0);
  };
  if !matches!(items.first(), Some(Cirru::Leaf(head)) if head.as_ref() == "{}") {
    return canonicalize_type_expression(node);
  }

  let mut rewritten = Vec::with_capacity(items.len());
  let mut changed = 0;
  rewritten.push(items[0].clone());
  for pair in items.iter().skip(1) {
    let (next, count) = match pair {
      Cirru::List(pair_items)
        if matches!(pair_items.first(), Some(Cirru::Leaf(key)) if matches!(key.as_ref(), ":args" | ":return" | ":rest" | ":where"))
          && pair_items.len() >= 2 =>
      {
        let mut next_pair = pair_items.clone();
        let (value, count) = canonicalize_type_expression(&pair_items[1]);
        next_pair[1] = value;
        (Cirru::List(next_pair), count)
      }
      _ => (pair.clone(), 0),
    };
    rewritten.push(next);
    changed += count;
  }
  (Cirru::List(rewritten), changed)
}

fn canonicalize_code_type_syntax(node: &Cirru) -> (Cirru, usize) {
  let Cirru::List(items) = node else {
    return (node.clone(), 0);
  };
  let mut rewritten = Vec::with_capacity(items.len());
  let mut changed = 0;
  for item in items {
    let (next, count) = canonicalize_code_type_syntax(item);
    rewritten.push(next);
    changed += count;
  }

  let head = items.first().and_then(|item| match item {
    Cirru::Leaf(value) => Some(value.as_ref()),
    _ => None,
  });
  match head {
    Some("assert-type" | "unsafe-coerce") if items.len() >= 3 => {
      let (next, count) = canonicalize_type_expression(&items[2]);
      rewritten[2] = next;
      changed += count;
    }
    Some("defstruct" | "defrecord" | "defenum") if items.len() >= 3 => {
      for index in 2..items.len() {
        let Cirru::List(field) = &items[index] else {
          continue;
        };
        if field.len() < 2 {
          continue;
        }
        let mut next_field = field.clone();
        for type_index in 1..field.len() {
          let (next, count) = canonicalize_type_expression(&field[type_index]);
          next_field[type_index] = next;
          changed += count;
        }
        rewritten[index] = Cirru::List(next_field);
      }
    }
    Some("hint-fn") => {
      for index in 1..items.len() {
        let (next, count) = canonicalize_schema_map_types(&items[index]);
        rewritten[index] = next;
        changed += count;
      }
    }
    Some("fn" | "defn" | "defmacro" | "defcomp" | "defeffect") => {
      let args_index = if head == Some("fn") { 1 } else { 2 };
      let type_index = args_index + 1;
      if let Some(type_form) = items.get(type_index)
        && (canonicalize_legacy_type_leaf(type_form).is_some()
          || matches!(type_form, Cirru::List(inner) if matches!(inner.first(), Some(Cirru::Leaf(marker)) if marker.as_ref() == "::")))
      {
        let (next, count) = canonicalize_type_expression(type_form);
        rewritten[type_index] = next;
        changed += count;
      }
    }
    _ => {}
  }
  (Cirru::List(rewritten), changed)
}

/// Rewrite legacy tag-based type syntax in code type positions. This is intentionally
/// called by `calcit edit format`, not by unrelated structural edits: old snapshots stay
/// compatible until users explicitly request canonical formatting.
pub fn canonicalize_snapshot_type_syntax(snapshot: &mut Snapshot) -> usize {
  let mut changed = 0;
  for file in snapshot.files.values_mut() {
    let (ns_code, count) = canonicalize_code_type_syntax(&file.ns.code);
    file.ns.code = ns_code;
    changed += count;
    for entry in file.defs.values_mut() {
      let (code, count) = canonicalize_code_type_syntax(&entry.code);
      entry.code = code;
      changed += count;
      let mut rewritten_examples = Vec::with_capacity(entry.examples.len());
      for example in &entry.examples {
        let (code, count) = canonicalize_code_type_syntax(example);
        rewritten_examples.push(code);
        changed += count;
      }
      entry.examples = rewritten_examples;
    }
  }
  changed
}

/// Render snapshot content for runtime snapshot files such as `calcit.cirru`
/// This is a shared utility function used by CLI edit commands
pub fn render_snapshot_content(snapshot: &Snapshot) -> Result<String, String> {
  validate_snapshot_schemas_for_write(snapshot)?;

  // Build root level Edn mapping
  let mut edn_map = EdnMapView::default();

  // Build package
  edn_map.insert_key("package", Edn::Str(snapshot.package.as_str().into()));

  // Insert about message (always enforce canonical hint)
  edn_map.insert_key("about", Edn::Str(SNAPSHOT_ABOUT_MESSAGE.into()));

  // Build entries
  let mut entries_map = EdnMapView::default();
  for (k, v) in &snapshot.entries {
    let mut entry_map = EdnMapView::default();
    entry_map.insert_key("mode", Edn::tag(v.mode.as_str()));
    entry_map.insert_key("init-fn", Edn::Symbol(v.init_fn.as_str().into()));
    entry_map.insert_key("reload-fn", Edn::Symbol(v.reload_fn.as_str().into()));
    entry_map.insert_key("description", Edn::Str(v.description.as_str().into()));
    entry_map.insert_key(
      "modules",
      Edn::from(v.modules.iter().map(|s| Edn::Str(s.as_str().into())).collect::<Vec<_>>()),
    );
    entry_map.insert_key("type-slots", type_slots_to_edn(&v.type_slots));
    entry_map.insert_key("feature-policy", feature_policy_to_edn(&v.feature_policy));
    if let Some(target) = v.target {
      entry_map.insert_key("target", Edn::tag(target.as_str()));
    }
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
    files_map.insert(Edn::Symbol(k.as_str().into()), Edn::from(v));
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

/// Save snapshot to a runtime snapshot file such as `calcit.cirru`
/// This is a shared utility function used by CLI edit commands
pub fn save_snapshot_to_file<P: AsRef<Path>>(snapshot_path: P, snapshot: &Snapshot) -> Result<(), String> {
  let content = render_snapshot_content(snapshot)?;

  // Write to file
  std::fs::write(&snapshot_path, content)
    .map_err(|e| format!("Failed to write snapshot file {}: {e}", snapshot_path.as_ref().display()))?;

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::{CalcitFnTypeAnnotation, SchemaKind};
  use cirru_edn::EdnListView;

  fn parse_one(source: &str) -> Cirru {
    cirru_parser::parse(source)
      .unwrap_or_else(|error| panic!("failed to parse test Cirru `{source}`: {error}"))
      .into_iter()
      .next()
      .expect("test Cirru should contain one expression")
  }

  #[test]
  fn snapshot_load_error_names_source_and_compact_recovery_path() {
    let root = std::env::temp_dir().join(format!("calcit-legacy-snapshot-recovery-{}", std::process::id()));
    fs::create_dir_all(&root).expect("create legacy snapshot fixture directory");
    let snapshot_path = root.join("calcit.cirru");
    let compact_path = root.join("compact.cirru");
    fs::write(&snapshot_path, "legacy full snapshot").expect("write full snapshot marker");
    fs::write(&compact_path, "compact snapshot").expect("write compact snapshot marker");

    let error = load_snapshot_data(&Edn::Nil, snapshot_path.to_str().expect("utf-8 temp path"))
      .expect_err("invalid legacy snapshot data should fail with recovery guidance");

    assert!(error.contains(snapshot_path.to_str().unwrap()), "error: {error}");
    assert!(error.contains(compact_path.to_str().unwrap()), "error: {error}");
    assert!(error.contains("calcit calcit.cirru edit format"), "error: {error}");
    assert!(error.contains("calcit calcit.cirru --check-only"), "error: {error}");

    fs::remove_dir_all(root).expect("remove legacy snapshot fixture directory");
  }

  #[test]
  fn compact_snapshot_filename_is_rejected_with_migration_commands() {
    let error = load_snapshot_data(&Edn::Nil, "compact.cirru").expect_err("retired snapshot filename should fail before parsing");

    assert!(error.contains("filename `compact.cirru` is retired"), "error: {error}");
    assert!(error.contains("calcit.cirru"), "error: {error}");
    assert!(error.contains("calcit calcit.cirru edit format"), "error: {error}");
    assert!(
      error.contains("published Calcit 0.13.48 release is the final release"),
      "error: {error}"
    );
  }

  fn revision_test_entry(tags: &[&str]) -> CodeEntry {
    CodeEntry {
      doc: "revision test".to_owned(),
      examples: vec![Cirru::List(vec![Cirru::leaf("inc"), Cirru::leaf("1")])],
      tests: vec![TestEntry {
        name: "returns-answer".to_owned(),
        code: Cirru::List(vec![Cirru::leaf("assert="), Cirru::leaf("42"), Cirru::leaf("answer")]),
        tags: [EdnTag::new("unit")].into_iter().collect(),
      }],
      tags: tags.iter().map(|tag| EdnTag::new(*tag)).collect(),
      code: Cirru::List(vec![Cirru::leaf("def"), Cirru::leaf("answer"), Cirru::leaf("42")]),
      schema: Arc::new(CalcitTypeAnnotation::Number),
      ffi: None,
    }
  }

  #[test]
  fn definition_revision_is_stable_and_covers_persisted_fields() {
    let entry = revision_test_entry(&["public", "demo"]);
    let reordered_tags = revision_test_entry(&["demo", "public"]);
    let revision = definition_revision(&entry).expect("revision should render");

    assert_eq!(
      revision,
      definition_revision(&reordered_tags).expect("tag order should not affect revision")
    );
    assert!(revision.starts_with("md5:"));

    let mut changed = entry.clone();
    changed.doc.push('!');
    assert_ne!(revision, definition_revision(&changed).expect("changed revision should render"));

    let mut changed = entry.clone();
    changed.code = Cirru::List(vec![Cirru::leaf("def"), Cirru::leaf("answer"), Cirru::leaf("43")]);
    assert_ne!(
      revision,
      definition_revision(&changed).expect("changed code revision should render")
    );

    let mut changed = entry.clone();
    changed.tests[0].code = Cirru::List(vec![Cirru::leaf("assert="), Cirru::leaf("43"), Cirru::leaf("answer")]);
    assert_ne!(
      revision,
      definition_revision(&changed).expect("changed test revision should render")
    );
  }

  #[test]
  fn definition_revision_supports_leaf_examples_and_tests() {
    let mut entry = revision_test_entry(&["public"]);
    entry.examples = vec![Cirru::leaf("literal-example")];
    entry.tests[0].code = Cirru::leaf("run-test");

    let revision = definition_revision(&entry).expect("leaf code entries should have a revision");

    assert!(revision.starts_with("md5:"));
    entry.tests[0].code = Cirru::leaf("run-other-test");
    assert_ne!(
      revision,
      definition_revision(&entry).expect("changed leaf test should have a revision")
    );
  }

  #[test]
  fn code_entry_tests_round_trip_through_edn() {
    let entry = revision_test_entry(&["public"]);
    let edn = Edn::from(&entry);
    let decoded = CodeEntry::try_from(edn).expect("CodeEntry tests should deserialize");
    assert_eq!(decoded.tests, entry.tests);
  }

  #[test]
  fn code_entry_rejects_duplicate_test_names() {
    let test = TestEntry {
      name: "duplicate".to_owned(),
      code: Cirru::leaf("nil"),
      tags: HashSet::new(),
    };
    let edn = Edn::struct_from_pairs(
      "CodeEntry",
      &[
        (EdnTag::new("doc"), Edn::Str(Arc::from(""))),
        (EdnTag::new("examples"), Edn::List(EdnListView(vec![]))),
        (
          EdnTag::new("tests"),
          Edn::List(EdnListView(vec![Edn::from(&test), Edn::from(&test)])),
        ),
        (EdnTag::new("code"), Cirru::leaf("nil").into()),
      ],
    );
    let error = CodeEntry::try_from(edn).expect_err("duplicate test names should be rejected");
    assert!(error.contains("duplicate test name `duplicate`"), "unexpected error: {error}");
  }

  #[test]
  fn test_names_reject_surrounding_whitespace() {
    let error = validate_test_names([" stable-name "], "CodeEntry.tests").expect_err("whitespace must be rejected");
    assert!(error.contains("leading or trailing whitespace"), "unexpected error: {error}");
  }

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
      tests: vec![],
      tags: HashSet::new(),
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
      ffi: None,
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
  fn test_code_entry_tags_field_defaults_and_round_trip() {
    let entry_edn = Edn::struct_from_pairs(
      "CodeEntry",
      &[
        ("doc".into(), Edn::str("tagged def")),
        ("examples".into(), Edn::List(EdnListView(vec![]))),
        ("code".into(), Cirru::leaf("x").into()),
        ("schema".into(), Edn::tag("dynamic")),
      ],
    );
    let parsed: CodeEntry = entry_edn.try_into().expect("missing tags should default to empty set");
    assert!(parsed.tags.is_empty());

    let mut tagged = parsed.clone();
    tagged.tags.insert(EdnTag::new("smoke"));
    tagged.tags.insert(EdnTag::new("doc"));

    let serialized = Edn::from(&tagged);
    let Edn::Struct(struct_value) = &serialized else {
      panic!("expected CodeEntry struct");
    };
    assert!(struct_value.pairs.iter().any(|(k, _)| k.ref_str() == "tags"));

    let reloaded: CodeEntry = serialized.try_into().expect("tags should round-trip");
    assert_eq!(reloaded.tags, tagged.tags);

    let mut external = tagged.clone();
    external.ffi = Some(Edn::map_from_iter([
      (Edn::tag("backend"), Edn::tag("js")),
      (Edn::tag("kind"), Edn::tag("external-object")),
    ]));
    let external_serialized = Edn::from(&external);
    let external_reloaded: CodeEntry = external_serialized.try_into().expect("ffi metadata should round-trip");
    assert_eq!(external_reloaded.ffi, external.ffi);

    let empty_serialized = Edn::from(&parsed);
    let Edn::Struct(empty_struct) = &empty_serialized else {
      panic!("expected CodeEntry struct");
    };
    assert!(!empty_struct.pairs.iter().any(|(k, _)| k.ref_str() == "tags"));
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

    let optional_wrapped_by_enum = Cirru::List(vec![Cirru::leaf("::"), Cirru::leaf(":optional"), valid]);
    assert!(parse_schema_data(&optional_wrapped_by_enum).is_ok());

    let invalid_edn = Cirru::List(vec![Cirru::leaf("~"), Cirru::leaf("x")]);
    assert!(parse_schema_data(&invalid_edn).is_err());
  }

  #[test]
  fn test_validate_schema_for_write() {
    let valid = parse_one(":: :fn $ {} (:args ([] :string)) (:return :bool)");
    assert!(validate_schema_for_write(&valid).is_ok(), "valid schema should pass");

    let valid_with_where = parse_one(":: :fn $ {} (:generics ([] 'T)) (:args ([] 'T)) (:where {} ('T Show)) (:return :string)");
    assert!(
      validate_schema_for_write(&valid_with_where).is_ok(),
      "schema with :where should pass"
    );

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

    let ref_bool = Cirru::List(vec![Cirru::leaf("::"), Cirru::leaf(":ref"), Cirru::leaf(":bool")]);
    assert!(
      validate_schema_for_write(&ref_bool).is_ok(),
      "standalone parameterized value schema should pass"
    );

    let qualified_struct = Cirru::Leaf(Arc::from("'app.schema/Store"));
    let qualified_result = validate_schema_for_write(&qualified_struct);
    assert!(
      qualified_result.is_ok(),
      "fully qualified nominal value schema should pass: {qualified_result:?}"
    );
    let qualified_annotation =
      parse_schema_annotation_for_write(&qualified_struct).expect("fully qualified nominal value schema should parse");
    assert!(matches!(
      qualified_annotation.as_ref(),
      CalcitTypeAnnotation::TypeRef(name, args) if name.as_ref() == "app.schema/Store" && args.is_empty()
    ));
    let unqualified_struct = Cirru::Leaf(Arc::from("'Store"));
    let error = validate_schema_for_write(&unqualified_struct).expect_err("unqualified nominal value schema should fail");
    assert!(error.contains("fully qualified nominal type"), "error: {error}");

    // Legacy unwrapped callable maps are rejected even when they carry :kind.
    let legacy_unwrapped = parse_one("{} (:kind :fn) (:args ([] :string)) (:return :bool)");
    let error = validate_schema_for_write(&legacy_unwrapped).expect_err("legacy map should fail");
    assert!(error.contains("Legacy unwrapped callable schema"), "error: {error}");

    // Missing callable wrapper
    let no_kind = Cirru::List(vec![
      Cirru::leaf("{}"),
      Cirru::List(vec![Cirru::leaf(":args"), Cirru::List(vec![Cirru::leaf("[]")])]),
    ]);
    assert!(validate_schema_for_write(&no_kind).is_err(), "missing :kind should fail");

    // Unknown field
    let unknown_field = parse_one(":: :fn $ {} (:foobar :dynamic)");
    assert!(validate_schema_for_write(&unknown_field).is_err(), "unknown field should fail");

    // Bad outer callable kind.
    let bad_kind = parse_one(":: :something-else $ {}");
    assert!(validate_schema_for_write(&bad_kind).is_err(), "bad :kind value should fail");

    let repeated_kind = parse_one(":: :fn $ {} (:kind :fn) (:return :unit)");
    let error = validate_schema_for_write(&repeated_kind).expect_err("redundant inner kind should fail");
    assert!(error.contains("must not repeat `:kind`"), "error: {error}");

    // Primitive type tag leaves are now accepted.
    let leaf_string = Cirru::Leaf(Arc::from(":string"));
    assert!(validate_schema_for_write(&leaf_string).is_ok(), ":string leaf should pass");
    let parsed_leaf_string = parse_schema_annotation_for_write(&leaf_string).expect(":string leaf should parse");
    assert!(matches!(parsed_leaf_string.as_ref(), CalcitTypeAnnotation::String));
    let quoted_string = Cirru::Leaf(Arc::from("'String"));
    let parsed_quoted_string = parse_schema_annotation_for_write(&quoted_string).expect("'String leaf should parse");
    assert!(matches!(parsed_quoted_string.as_ref(), CalcitTypeAnnotation::String));

    for (legacy, replacement) in [
      ("'Record", "'Struct"),
      ("'Tuple", "'Enum"),
      (":record", "'Struct"),
      (":tuple", "'Enum"),
    ] {
      let error =
        validate_schema_for_write(&Cirru::Leaf(Arc::from(legacy))).expect_err("legacy data type names must be rejected on write");
      assert!(error.contains(replacement), "error should point to {replacement}: {error}");
    }

    let nested_legacy = parse_one(":: 'List 'Record");
    let error = validate_schema_for_write(&nested_legacy).expect_err("nested legacy data type names must be rejected on write");
    assert!(error.contains("'Struct"), "nested error should point to 'Struct: {error}");
    let leaf_fn = Cirru::Leaf(Arc::from(":fn"));
    assert!(validate_schema_for_write(&leaf_fn).is_err(), "bare :fn should require a signature");
    let leaf_ref = Cirru::Leaf(Arc::from(":ref"));
    let error = validate_schema_for_write(&leaf_ref).expect_err("bare :ref should require an inner type");
    assert!(error.contains("leaves its nested type dynamic"), "error: {error}");
    let leaf_number = Cirru::Leaf(Arc::from(":number"));
    assert!(validate_schema_for_write(&leaf_number).is_ok(), ":number leaf should pass");
    let leaf_any = Cirru::Leaf(Arc::from(":any"));
    assert!(validate_schema_for_write(&leaf_any).is_ok(), ":any leaf should pass");
    let leaf_trait = Cirru::Leaf(Arc::from(":trait"));
    assert!(validate_schema_for_write(&leaf_trait).is_ok(), ":trait leaf should pass");
    let leaf_enum = Cirru::Leaf(Arc::from(":enum"));
    assert!(validate_schema_for_write(&leaf_enum).is_ok(), ":enum leaf should pass");
    let leaf_struct = Cirru::Leaf(Arc::from(":struct"));
    assert!(validate_schema_for_write(&leaf_struct).is_ok(), ":struct leaf should pass");
    let leaf_impl = Cirru::Leaf(Arc::from(":impl"));
    assert!(validate_schema_for_write(&leaf_impl).is_ok(), ":impl leaf should pass");
    for kind in ["struct", "enum", "trait", "impl"] {
      let schema = Cirru::Leaf(Arc::from(format!(":{kind}")));
      let annotation = parse_schema_annotation_for_write(&schema).unwrap_or_else(|error| panic!(":{kind} should parse: {error}"));
      assert!(
        matches!(annotation.as_ref(), CalcitTypeAnnotation::Custom(value) if matches!(value.as_ref(), crate::calcit::Calcit::Tag(tag) if tag.ref_str() == kind)),
        ":{kind} should keep its broad schema kind, got {annotation}"
      );
    }

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
  fn standalone_value_schema_round_trips_without_becoming_dynamic() {
    let schema_edn = Edn::enum_value("ref", vec![Edn::tag("bool")]);
    let annotation = parse_loaded_schema_annotation(&schema_edn, "tests/*flag").expect("ref<bool> should load");

    assert!(matches!(
      annotation.as_ref(),
      CalcitTypeAnnotation::Ref(inner) if matches!(inner.as_ref(), CalcitTypeAnnotation::Bool)
    ));
    assert_eq!(
      schema_annotation_to_edn(annotation.as_ref()),
      Edn::enum_value("Ref", vec![Edn::Symbol(Arc::from("Bool"))])
    );

    let mut entry = CodeEntry::from_code(Cirru::leaf("nil"));
    entry.schema = annotation;
    let encoded = rmp_serde::to_vec(&entry).expect("value schema should serialize into binary snapshot data");
    let decoded: CodeEntry = rmp_serde::from_slice(&encoded).expect("value schema should deserialize from binary snapshot data");
    assert!(matches!(
      decoded.schema.as_ref(),
      CalcitTypeAnnotation::Ref(inner) if matches!(inner.as_ref(), CalcitTypeAnnotation::Bool)
    ));

    let nominal_edn = Edn::Symbol(Arc::from("app.schema/Store"));
    let nominal = parse_loaded_schema_annotation(&nominal_edn, "app.schema/store").expect("qualified nominal schema should load");
    assert!(matches!(
      nominal.as_ref(),
      CalcitTypeAnnotation::TypeRef(name, args) if name.as_ref() == "app.schema/Store" && args.is_empty()
    ));
    let stored_nominal = schema_annotation_to_edn(nominal.as_ref());
    assert_eq!(stored_nominal, Edn::enum_value("app.schema/Store", vec![]));
    let reloaded = parse_loaded_schema_annotation(&stored_nominal, "app.schema/store").expect("stored nominal schema should reload");
    assert!(matches!(
      reloaded.as_ref(),
      CalcitTypeAnnotation::TypeRef(name, args) if name.as_ref() == "app.schema/Store" && args.is_empty()
    ));
  }

  #[test]
  fn format_canonicalizes_legacy_type_tags_only_in_type_positions() {
    let typed = parse_one(
      "defn example (value) :string\n  hint-fn $ {} (:args $ [] :number) (:return $ :: :list :string)\n  assert-type value :string\n  unsafe-coerce value $ :: :ref :bool",
    );
    let enum_decl = parse_one("defenum Result (:ok :string) (:err :tag)");
    let ordinary_data = parse_one("def config $ {} (:kind :string)");

    let (typed, typed_count) = canonicalize_code_type_syntax(&typed);
    let (enum_decl, enum_count) = canonicalize_code_type_syntax(&enum_decl);
    let (ordinary_data, data_count) = canonicalize_code_type_syntax(&ordinary_data);

    let typed_text = cirru_parser::format(&[typed], true.into()).expect("typed code should render");
    let enum_text = cirru_parser::format(&[enum_decl], true.into()).expect("enum should render");
    let data_text = cirru_parser::format(&[ordinary_data], true.into()).expect("data should render");
    assert_eq!(typed_count, 7, "typed text: {typed_text}");
    assert_eq!(enum_count, 2, "enum text: {enum_text}");
    assert_eq!(data_count, 0, "data text: {data_text}");
    assert!(typed_text.contains("'String") && typed_text.contains(":: 'List 'String") && typed_text.contains(":: 'Ref 'Bool"));
    assert!(enum_text.contains("(:ok 'String)") && enum_text.contains("(:err 'Tag)"));
    assert!(
      data_text.contains("(:kind :string)"),
      "ordinary tag data must not be rewritten: {data_text}"
    );
  }

  #[test]
  fn test_typevar_consistency_validation() {
    // Valid: 'T declared and used in both args and return
    let valid_generic = parse_one(":: :fn $ {} (:generics ([] 'T)) (:args ([] (:: :list 'T))) (:return 'T)");
    assert!(validate_schema_for_write(&valid_generic).is_ok(), "valid generics should pass");

    // Invalid: 'K used in :return but not declared in :generics
    let undeclared = parse_one(":: :fn $ {} (:generics ([] 'T)) (:args ([] (:: :list 'T))) (:return 'K)");
    assert!(
      validate_schema_for_write(&undeclared).is_err(),
      "undeclared type var 'K should fail"
    );

    // Invalid: 'U declared but never used
    let unused_declared = parse_one(":: :fn $ {} (:generics ([] 'T 'U)) (:args ([] (:: :list 'T))) (:return 'T)");
    assert!(
      validate_schema_for_write(&unused_declared).is_err(),
      "unused declared 'U should fail"
    );

    // Invalid: type var used without any :generics
    let typevar_no_generics = parse_one(":: :fn $ {} (:args ([] 'T)) (:return 'T)");
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
    let schema_text = "{} (:kind :fn) (:args ([] 'T)) (:generics ([] 'T)) (:return 'T)";
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

    let saved_cirru = schema_edn_to_cirru(&fn_schema.to_wrapped_schema_edn()).expect("schema edn to cirru");
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
  fn test_schema_where_round_trip_is_preserved() {
    let schema_text = ":: :fn $ {} (:generics ([] 'T)) (:args ([] 'T)) (:where {} ('T Show)) (:return :string)";
    let schema_cirru = cirru_parser::parse(schema_text)
      .expect("should parse")
      .into_iter()
      .next()
      .expect("should have one node");

    validate_schema_for_write(&schema_cirru).expect("schema with where should be writable");

    let schema_edn = schema_cirru_to_edn(schema_cirru);
    let fn_schema =
      CalcitTypeAnnotation::parse_fn_schema_from_edn(&schema_edn).unwrap_or_else(|| panic!("must parse where schema: {schema_edn:?}"));
    assert_eq!(fn_schema.where_bounds.len(), 1, "schema_edn={schema_edn:?}");
    assert_eq!(fn_schema.where_bounds[0].name.as_ref(), "T");
    assert_eq!(fn_schema.where_bounds[0].traits[0].name.ref_str(), "Show");

    let saved_cirru = schema_edn_to_cirru(&fn_schema.to_wrapped_schema_edn()).expect("schema edn to cirru");
    validate_schema_for_write(&saved_cirru).expect("saved where schema should still be writable");
    let saved_text = cirru_parser::format(&[saved_cirru], true.into()).expect("format schema");
    assert!(saved_text.contains(":where"), "saved schema should keep :where: {saved_text}");
    assert!(
      saved_text.contains("Show"),
      "saved schema should keep trait bound payload: {saved_text}"
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
    let schema_text = ":: :fn $ {} (:args ([] :number)) (:generics ([] ''T)) (:return :number)";
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
  fn test_normalize_schema_unwraps_wrapped_fn_enum() {
    let wrapped = Edn::enum_value(
      "fn",
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
  fn test_normalize_schema_unwraps_wrapped_macro_enum() {
    let wrapped = Edn::enum_value(
      "macro",
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
    let wrapped = Edn::enum_value(
      "fn",
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
  fn data_definition_schema_uses_definition_kind_marker() {
    for (head, marker) in [
      ("defstruct", "struct-def"),
      ("defenum", "enum-def"),
      ("deftrait", "trait"),
      ("defimpl", "impl"),
    ] {
      let code = Cirru::List(vec![Cirru::leaf(head)]);
      let normalized = normalize_schema_for_code(&code, &DYNAMIC_TYPE);
      assert!(
        matches!(normalized.as_ref(), CalcitTypeAnnotation::Custom(value) if matches!(value.as_ref(), Calcit::Tag(tag) if tag.ref_str() == marker)),
        "{head} should normalize Dynamic to {marker}, got {normalized}"
      );
      assert!(
        !matches!(normalized.as_ref(), CalcitTypeAnnotation::Dynamic),
        "{head} definition marker must not remain Dynamic"
      );
    }
  }

  #[test]
  fn explicit_data_definition_schema_is_not_overwritten() {
    let code = Cirru::List(vec![Cirru::leaf("defstruct")]);
    let explicit = Arc::new(CalcitTypeAnnotation::Custom(Arc::new(Calcit::tag("struct"))));
    assert_eq!(normalize_schema_for_code(&code, &explicit), explicit);
  }

  #[test]
  fn binary_schema_round_trip_distinguishes_missing_and_explicit_dynamic() {
    let mut explicit = CodeEntry::from_code(Cirru::leaf("nil"));
    explicit.schema = Arc::new(CalcitTypeAnnotation::Dynamic);
    let bytes = rmp_serde::to_vec(&explicit).expect("explicit Dynamic entry should encode");
    let decoded: CodeEntry = rmp_serde::from_slice(&bytes).expect("explicit Dynamic entry should decode");
    assert!(!schema_annotation_is_missing(&decoded.schema));

    let missing = CodeEntry::from_code(Cirru::leaf("nil"));
    let bytes = rmp_serde::to_vec(&missing).expect("missing schema entry should encode");
    let decoded: CodeEntry = rmp_serde::from_slice(&bytes).expect("missing schema entry should decode");
    assert!(schema_annotation_is_missing(&decoded.schema));
  }

  #[test]
  fn strict_loader_rejects_legacy_macro_schemas_with_snapshot_path() {
    let macro_code = Cirru::List(vec![Cirru::leaf("defmacro"), Cirru::leaf("legacy"), Cirru::List(vec![])]);
    for schema in [
      DYNAMIC_TYPE.clone(),
      Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types: vec![],
        return_type: DYNAMIC_TYPE.clone(),
        fn_kind: SchemaKind::Macro,
        rest_type: None,
        features: Arc::new(HashSet::new()),
      }))),
    ] {
      let files = HashMap::from([(
        "app.main".to_owned(),
        FileInSnapShot {
          ns: NsEntry {
            doc: String::new(),
            code: Cirru::List(vec![Cirru::leaf("ns"), Cirru::leaf("app.main")]),
          },
          defs: HashMap::from([(
            "legacy".to_owned(),
            CodeEntry {
              doc: String::new(),
              examples: vec![],
              tests: vec![],
              tags: HashSet::new(),
              code: macro_code.clone(),
              schema,
              ffi: None,
            },
          )]),
        },
      )]);
      let error = validate_strict_macro_schemas(&files, "fixtures/legacy.cirru").expect_err("legacy macro must be rejected");
      assert!(error.contains("snapshot.files[\"app.main\"].defs[\"legacy\"].schema"), "{error}");
      assert!(error.contains("Calcit 0.13.51"), "{error}");
      assert!(error.contains("fixtures/legacy.cirru"), "{error}");
    }
  }

  #[test]
  fn test_code_entry_serializes_schema_as_wrapped_fn() {
    let entry = CodeEntry {
      doc: "wrapped schema".to_owned(),
      examples: vec![],
      tests: vec![],
      tags: HashSet::new(),
      code: vec!["defn", "wrapped", "()", "nil"].into(),
      schema: std::sync::Arc::new(CalcitTypeAnnotation::Fn(std::sync::Arc::new(CalcitFnTypeAnnotation {
        generics: std::sync::Arc::new(vec![]),
        where_bounds: std::sync::Arc::new(vec![]),
        arg_types: vec![],
        return_type: crate::calcit::DYNAMIC_TYPE.clone(),
        fn_kind: SchemaKind::Fn,
        rest_type: None,
        features: std::sync::Arc::new(std::collections::HashSet::new()),
      }))),
      ffi: None,
    };

    let entry_edn: Edn = Edn::from(&entry);
    let schema = match entry_edn {
      Edn::Struct(struct_value) => struct_value
        .pairs
        .iter()
        .find(|(k, _)| k.arc_str().as_ref() == "schema")
        .map(|(_, v)| v.to_owned())
        .expect("schema field should exist"),
      _ => panic!("expected struct edn"),
    };

    let Edn::Enum(view) = schema else {
      panic!("top-level schema should serialize as wrapped fn tuple");
    };
    assert_eq!(view.variant.as_ref(), "Fn");
    let Some(Edn::Map(map)) = view.extra.first() else {
      panic!("wrapped schema payload should be a map");
    };
    assert!(
      map.tag_get("kind").is_none(),
      "wrapped plain fn schema should omit redundant :kind :fn"
    );
  }

  #[test]
  fn code_entry_keeps_legacy_fn_schema_for_snapshot_level_rejection() {
    let code = cirru_parser::parse("defmacro demo (x) x")
      .expect("should parse code")
      .into_iter()
      .next()
      .expect("should have one node");
    let schema = Edn::enum_value(
      "fn",
      vec![Edn::Map(EdnMapView::from(HashMap::from([
        (Edn::tag("args"), Edn::List(EdnListView(vec![Edn::tag("dynamic")]))),
        (Edn::tag("return"), Edn::tag("dynamic")),
      ])))],
    );

    let entry = Edn::struct_from_pairs(
      "CodeEntry",
      &[
        ("doc".into(), Edn::Str(Arc::from(""))),
        ("examples".into(), Edn::List(EdnListView(vec![]))),
        ("code".into(), code.into()),
        ("schema".into(), schema),
      ],
    );

    let entry: CodeEntry = entry.try_into().expect("code entry should parse");
    assert!(matches!(entry.schema.as_ref(), CalcitTypeAnnotation::Fn(_)));
  }

  #[test]
  fn defmacro_dynamic_schema_is_not_normalized_to_a_macro_contract() {
    let code = Cirru::List(vec![Cirru::leaf("defmacro"), Cirru::leaf("demo"), Cirru::List(vec![])]);
    let normalized = normalize_schema_for_code(&code, &DYNAMIC_TYPE);
    assert!(matches!(normalized.as_ref(), CalcitTypeAnnotation::Dynamic));
  }

  #[test]
  fn strict_macro_schema_round_trips_without_legacy_origin_metadata() {
    let entry = CodeEntry {
      doc: String::new(),
      examples: vec![],
      tests: vec![],
      tags: HashSet::new(),
      code: Cirru::List(vec![Cirru::leaf("defmacro"), Cirru::leaf("demo"), Cirru::List(vec![])]),
      schema: parse_schema_annotation_for_write(
        &cirru_parser::parse(":: 'Macro\n  {} (:required $ [])\n    :expansion $ :: 'Expr 'Dynamic\n    :capabilities $ #{}")
          .expect("strict schema syntax")
          .into_iter()
          .next()
          .expect("strict schema node"),
      )
      .expect("strict schema"),
      ffi: None,
    };
    let encoded = Edn::from(&entry);
    let text = cirru_edn::format(&encoded, true).expect("strict macro CodeEntry should format");
    assert!(
      !text.contains(":legacy-origin"),
      "strict schema must not serialize legacy metadata: {text}"
    );
    let parsed = cirru_edn::parse(&text).expect("legacy macro CodeEntry should parse");
    let reloaded: CodeEntry = parsed.try_into().expect("strict macro CodeEntry should reload");
    let CalcitTypeAnnotation::Macro(_signature) = reloaded.schema.as_ref() else {
      panic!("reloaded schema should remain a macro");
    };
  }

  #[test]
  fn legacy_macro_fn_schema_is_not_parsed_as_a_macro_signature() {
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

    assert!(CalcitTypeAnnotation::parse_macro_signature_from_edn(&schema_edn).is_none());
  }

  #[test]
  fn schema_writer_rejects_legacy_macro_function_schema() {
    let schema = cirru_parser::parse(":: 'Macro\n  {} (:return 'Bool)\n    :args $ [] 'Number")
      .expect("legacy schema syntax should parse")
      .into_iter()
      .next()
      .expect("legacy schema node");
    let error = parse_schema_annotation_for_write(&schema).expect_err("legacy macro function schema must be rejected");
    assert!(error.contains("no longer writable"), "unexpected error: {error}");
  }

  #[test]
  fn phase_aware_macro_schema_writes_and_loads_as_macro_signature() {
    let schema = cirru_parser::parse(
      ":: 'Macro\n  {} (:generics $ [] 'T)\n    :required $ [] 'SyntaxSymbol (:: 'Expr 'T)\n    :optional $ [] 'SyntaxList\n    :rest 'Syntax\n    :expansion $ :: 'Expr 'T\n    :capabilities $ #{} :env-read :fs-read",
    )
    .expect("strict macro schema should parse")
    .into_iter()
    .next()
    .expect("schema node");
    let annotation = parse_schema_annotation_for_write(&schema).expect("strict macro schema should validate");
    let CalcitTypeAnnotation::Macro(signature) = annotation.as_ref() else {
      panic!("strict macro schema must not become Fn: {annotation:?}")
    };
    assert!(signature.is_strict());
    assert_eq!(signature.required_inputs.len(), 2);
    assert_eq!(signature.optional_inputs.len(), 1);
    assert!(signature.rest_input.is_some());
    assert!(signature.capabilities.contains(&crate::calcit::MacroCapability::EnvRead));
    assert!(signature.capabilities.contains(&crate::calcit::MacroCapability::FsRead));

    let saved = schema_annotation_to_edn(annotation.as_ref());
    let loaded = parse_loaded_schema_annotation(&saved, "test macro schema").expect("saved strict signature should reload");
    assert_eq!(loaded, annotation);
  }

  #[test]
  fn macro_schema_rejects_unknown_compile_time_capabilities() {
    let schema = cirru_parser::parse(
      ":: 'Macro\n  {} (:required $ [])\n    :expansion $ :: 'Expr 'String\n    :capabilities $ #{} :network-everything",
    )
    .expect("schema syntax")
    .into_iter()
    .next()
    .expect("schema node");
    let error = parse_schema_annotation_for_write(&schema).expect_err("unknown capabilities must not silently become pure");
    assert!(error.contains("Unknown macro capability"), "unexpected error: {error}");
  }

  #[test]
  fn macro_schema_rejects_non_tag_compile_time_capabilities() {
    let schema =
      cirru_parser::parse(":: 'Macro\n  {} (:required $ [])\n    :expansion $ :: 'Expr 'String\n    :capabilities $ #{} env-read")
        .expect("schema syntax")
        .into_iter()
        .next()
        .expect("schema node");
    let error = parse_schema_annotation_for_write(&schema).expect_err("capability symbols must not pass as tags");
    assert!(error.contains("colon-prefixed tag"), "unexpected error: {error}");
  }

  #[test]
  fn bundled_core_defmacros_require_phase_aware_contracts() {
    let core_file_content = fs::read_to_string("src/cirru/calcit-core.cirru").expect("Failed to read calcit-core.cirru");
    let edn_data = cirru_edn::parse(&core_file_content).expect("Failed to parse cirru content as EDN");
    let snapshot = load_snapshot_data(&edn_data, "src/cirru/calcit-core.cirru").expect("Failed to parse snapshot");
    let mut macro_count = 0;
    let mut legacy_macros = vec![];

    for (ns_name, file) in &snapshot.files {
      for (def_name, entry) in &file.defs {
        if !code_declares_macro(&entry.code) {
          continue;
        }
        macro_count += 1;
        let CalcitTypeAnnotation::Macro(signature) = entry.schema.as_ref() else {
          panic!("{ns_name}/{def_name} should load as MacroSignature");
        };
        if !signature.is_strict() {
          legacy_macros.push(format!("{ns_name}/{def_name}"));
        }
      }
    }

    legacy_macros.sort();
    assert!(
      legacy_macros.is_empty(),
      "bundled macros must declare phase-aware contracts instead of legacy whole-Dynamic schemas: {legacy_macros:?}"
    );
    assert_eq!(
      macro_count, 63,
      "update the audited bundled macro inventory when core macros change"
    );
  }

  #[test]
  fn bundled_tag_match_is_deprecated_in_favor_of_native_match() {
    let core_file_content = fs::read_to_string("src/cirru/calcit-core.cirru").expect("Failed to read calcit-core.cirru");
    let edn_data = cirru_edn::parse(&core_file_content).expect("Failed to parse cirru content as EDN");
    let snapshot = load_snapshot_data(&edn_data, "src/cirru/calcit-core.cirru").expect("Failed to parse snapshot");
    let entry = snapshot.files["calcit.core"]
      .defs
      .get("tag-match")
      .expect("calcit.core/tag-match should exist");

    assert!(entry.tags.iter().any(|tag| tag.ref_str() == "deprecated"));
    assert!(entry.doc.contains("use `match`"), "migration guidance: {}", entry.doc);
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
      "not",
      "not=",
      "noted",
      "nth",
      "number?",
      "option:map",
      "optionally",
    ] {
      let entry = core_file.defs.get(def_name).unwrap_or_else(|| panic!("missing def: {def_name}"));
      if matches!(def_name, "%{}" | "deftrait" | "noted") {
        assert!(
          matches!(entry.schema.as_ref(), CalcitTypeAnnotation::Macro(_)),
          "{def_name} should load as MacroSignature"
        );
      } else {
        assert!(
          matches!(entry.schema.as_ref(), CalcitTypeAnnotation::Fn(_)),
          "schema for {def_name} should stay fn-like"
        );
      }
    }

    let CalcitTypeAnnotation::Macro(js_object) = core_file.defs["js-object"].schema.as_ref() else {
      panic!("js-object should load as MacroSignature");
    };
    assert!(js_object.is_strict());
    assert!(matches!(js_object.rest_input, Some(crate::calcit::MacroSyntaxType::SyntaxList)));
    assert!(matches!(
      js_object.expansion,
      crate::calcit::MacroExpansionType::Expr(ref inner)
        if matches!(inner.as_ref(), CalcitTypeAnnotation::JsObject)
    ));
    assert!(
      js_object.features.iter().any(|feature| feature.ref_str() == "js-ffi"),
      "js-object should retain its js-ffi backend feature"
    );

    for def_name in ["let", "fn", "and", "cond", "do"] {
      let entry = core_file.defs.get(def_name).unwrap_or_else(|| panic!("missing def: {def_name}"));
      let CalcitTypeAnnotation::Macro(signature) = entry.schema.as_ref() else {
        panic!("{def_name} should load as MacroSignature");
      };
      assert!(signature.is_strict(), "{def_name} should use a phase-aware contract");
      assert!(signature.capabilities.is_empty(), "{def_name} should be compile-time pure");
      assert!(
        matches!(signature.expansion, crate::calcit::MacroExpansionType::Expr(ref inner) if matches!(inner.as_ref(), CalcitTypeAnnotation::Dynamic)),
        "{def_name} should explicitly retain a dynamic expression result"
      );
    }

    let CalcitTypeAnnotation::Macro(def_signature) = core_file.defs["def"].schema.as_ref() else {
      panic!("def should load as MacroSignature");
    };
    assert!(def_signature.is_strict());
    assert!(def_signature.capabilities.is_empty());
    assert!(matches!(
      def_signature.required_inputs.as_slice(),
      [
        crate::calcit::MacroSyntaxType::SyntaxSymbol,
        crate::calcit::MacroSyntaxType::Expr(value)
      ] if matches!(value.as_ref(), CalcitTypeAnnotation::Dynamic)
    ));
    assert!(def_signature.optional_inputs.is_empty());
    assert!(def_signature.rest_input.is_none());
    assert!(matches!(
      def_signature.expansion,
      crate::calcit::MacroExpansionType::Expr(ref value)
        if matches!(value.as_ref(), CalcitTypeAnnotation::Dynamic)
    ));

    for (def_name, expected_output, rest_is_list) in [
      ("deftrait", "trait", true),
      ("defstruct", "struct-def", true),
      ("defimpl", "impl", false),
      ("defenum", "enum-def", true),
    ] {
      let CalcitTypeAnnotation::Macro(signature) = core_file.defs[def_name].schema.as_ref() else {
        panic!("{def_name} should load as MacroSignature");
      };
      assert!(signature.is_strict(), "{def_name} should use a phase-aware contract");
      assert!(signature.capabilities.is_empty(), "{def_name} should be compile-time pure");
      assert!(signature.optional_inputs.is_empty(), "{def_name} should not have optional inputs");
      match def_name {
        "defimpl" => assert!(matches!(
          signature.required_inputs.as_slice(),
          [crate::calcit::MacroSyntaxType::Syntax, crate::calcit::MacroSyntaxType::Syntax]
        )),
        _ => assert!(matches!(
          signature.required_inputs.as_slice(),
          [crate::calcit::MacroSyntaxType::Syntax]
        )),
      }
      if rest_is_list {
        assert!(matches!(signature.rest_input, Some(crate::calcit::MacroSyntaxType::SyntaxList)));
      } else {
        assert!(matches!(signature.rest_input, Some(crate::calcit::MacroSyntaxType::Syntax)));
      }
      assert!(matches!(
        signature.expansion,
        crate::calcit::MacroExpansionType::Expr(ref output)
          if matches!(output.as_ref(), CalcitTypeAnnotation::Custom(value) if value.as_ref() == &Calcit::tag(expected_output))
      ));
    }

    for def_name in ["->", "->%", "apply-args", "flipped", "\\"] {
      let CalcitTypeAnnotation::Macro(signature) = core_file.defs[def_name].schema.as_ref() else {
        panic!("{def_name} should load as MacroSignature");
      };
      assert!(signature.is_strict(), "{def_name} should use a phase-aware contract");
      assert!(signature.capabilities.is_empty(), "{def_name} should be compile-time pure");
      assert!(signature.optional_inputs.is_empty(), "{def_name} should not have optional inputs");
      match def_name {
        "->" => {
          assert!(matches!(
            signature.required_inputs.as_slice(),
            [crate::calcit::MacroSyntaxType::Expr(value)]
              if matches!(value.as_ref(), CalcitTypeAnnotation::Dynamic)
          ));
          assert!(matches!(signature.rest_input, Some(crate::calcit::MacroSyntaxType::Syntax)));
        }
        "->%" => {
          assert!(matches!(
            signature.required_inputs.as_slice(),
            [crate::calcit::MacroSyntaxType::Expr(value)]
              if matches!(value.as_ref(), CalcitTypeAnnotation::Dynamic)
          ));
          assert!(matches!(
            signature.rest_input,
            Some(crate::calcit::MacroSyntaxType::Expr(ref value))
              if matches!(value.as_ref(), CalcitTypeAnnotation::Dynamic)
          ));
        }
        "apply-args" => {
          assert!(matches!(
            signature.required_inputs.as_slice(),
            [
              crate::calcit::MacroSyntaxType::SyntaxList,
              crate::calcit::MacroSyntaxType::Expr(value)
            ] if matches!(value.as_ref(), CalcitTypeAnnotation::DynFn)
          ));
          assert!(signature.rest_input.is_none());
        }
        "flipped" => {
          assert!(matches!(
            signature.required_inputs.as_slice(),
            [crate::calcit::MacroSyntaxType::Expr(value)]
              if matches!(value.as_ref(), CalcitTypeAnnotation::Dynamic)
          ));
          assert!(matches!(
            signature.rest_input,
            Some(crate::calcit::MacroSyntaxType::Expr(ref value))
              if matches!(value.as_ref(), CalcitTypeAnnotation::Dynamic)
          ));
        }
        "\\" => {
          assert!(signature.required_inputs.is_empty());
          assert!(matches!(signature.rest_input, Some(crate::calcit::MacroSyntaxType::Syntax)));
        }
        _ => unreachable!(),
      }
      if def_name == "\\" {
        assert!(matches!(
          signature.expansion,
          crate::calcit::MacroExpansionType::Expr(ref output)
            if matches!(output.as_ref(), CalcitTypeAnnotation::DynFn)
        ));
      } else {
        assert!(matches!(
          signature.expansion,
          crate::calcit::MacroExpansionType::Expr(ref output)
            if matches!(output.as_ref(), CalcitTypeAnnotation::Dynamic)
        ));
      }
    }

    for def_name in [
      "let-destruct",
      "let-sugar",
      "let[]",
      "let{}",
      "loop",
      "struct-with",
      "swap!",
      "&doseq",
    ] {
      let CalcitTypeAnnotation::Macro(signature) = core_file.defs[def_name].schema.as_ref() else {
        panic!("{def_name} should load as MacroSignature");
      };
      assert!(signature.is_strict(), "{def_name} should use a phase-aware contract");
      assert!(signature.capabilities.is_empty(), "{def_name} should be compile-time pure");
      assert!(signature.optional_inputs.is_empty(), "{def_name} should not have optional inputs");
      match def_name {
        "let-destruct" => assert!(matches!(
          signature.required_inputs.as_slice(),
          [crate::calcit::MacroSyntaxType::Syntax, crate::calcit::MacroSyntaxType::Expr(value)]
            if matches!(value.as_ref(), CalcitTypeAnnotation::Dynamic)
        )),
        "let-sugar" | "loop" | "&doseq" => assert!(matches!(
          signature.required_inputs.as_slice(),
          [crate::calcit::MacroSyntaxType::SyntaxList]
        )),
        "let[]" | "let{}" => assert!(matches!(
          signature.required_inputs.as_slice(),
          [crate::calcit::MacroSyntaxType::SyntaxList, crate::calcit::MacroSyntaxType::Expr(value)]
            if matches!(value.as_ref(), CalcitTypeAnnotation::Dynamic)
        )),
        "struct-with" => assert!(matches!(
          signature.required_inputs.as_slice(),
          [crate::calcit::MacroSyntaxType::Expr(value)]
            if matches!(value.as_ref(), CalcitTypeAnnotation::Custom(kind) if kind.as_ref() == &Calcit::tag("struct"))
        )),
        "swap!" => assert!(matches!(
          signature.required_inputs.as_slice(),
          [crate::calcit::MacroSyntaxType::Expr(reference), crate::calcit::MacroSyntaxType::Expr(function)]
            if matches!(reference.as_ref(), CalcitTypeAnnotation::Ref(value) if matches!(value.as_ref(), CalcitTypeAnnotation::Dynamic))
              && matches!(function.as_ref(), CalcitTypeAnnotation::DynFn)
        )),
        _ => unreachable!(),
      }
      if def_name == "struct-with" {
        assert!(matches!(signature.rest_input, Some(crate::calcit::MacroSyntaxType::SyntaxList)));
        assert!(matches!(
          signature.expansion,
          crate::calcit::MacroExpansionType::Expr(ref output)
            if matches!(output.as_ref(), CalcitTypeAnnotation::Custom(kind) if kind.as_ref() == &Calcit::tag("struct"))
        ));
      } else {
        assert!(matches!(
          signature.rest_input,
          Some(crate::calcit::MacroSyntaxType::Expr(ref value))
            if matches!(value.as_ref(), CalcitTypeAnnotation::Dynamic)
        ));
        let expected_unit = matches!(def_name, "swap!" | "&doseq");
        assert!(matches!(
          signature.expansion,
          crate::calcit::MacroExpansionType::Expr(ref output)
            if (expected_unit && matches!(output.as_ref(), CalcitTypeAnnotation::Unit))
              || (!expected_unit && matches!(output.as_ref(), CalcitTypeAnnotation::Dynamic))
        ));
      }
    }

    for def_name in ["let", "fn"] {
      let CalcitTypeAnnotation::Macro(signature) = core_file.defs[def_name].schema.as_ref() else {
        unreachable!()
      };
      assert!(matches!(
        signature.required_inputs.as_slice(),
        [crate::calcit::MacroSyntaxType::SyntaxList]
      ));
      assert!(matches!(signature.rest_input, Some(crate::calcit::MacroSyntaxType::Syntax)));
    }

    let CalcitTypeAnnotation::Macro(cond_signature) = core_file.defs["cond"].schema.as_ref() else {
      unreachable!()
    };
    assert!(cond_signature.required_inputs.is_empty());
    assert!(matches!(
      cond_signature.rest_input,
      Some(crate::calcit::MacroSyntaxType::SyntaxList)
    ));

    for def_name in ["assert", "assert-detect", "assert="] {
      let CalcitTypeAnnotation::Macro(signature) = core_file.defs[def_name].schema.as_ref() else {
        panic!("{def_name} should load as MacroSignature");
      };
      assert!(signature.is_strict(), "{def_name} should use a phase-aware contract");
      assert!(signature.capabilities.is_empty(), "{def_name} should be compile-time pure");
      assert_eq!(signature.required_inputs.len(), 2);
      assert!(signature.required_inputs.iter().all(
        |input| matches!(input, crate::calcit::MacroSyntaxType::Expr(inner) if matches!(inner.as_ref(), CalcitTypeAnnotation::Dynamic))
      ));
      assert!(matches!(
        signature.expansion,
        crate::calcit::MacroExpansionType::Expr(ref inner) if matches!(inner.as_ref(), CalcitTypeAnnotation::Unit)
      ));
    }

    for (def_name, required, optional, has_rest) in [
      ("or", 1, 0, true),
      ("either", 0, 0, true),
      ("when", 1, 0, true),
      ("when-not", 1, 0, true),
      ("if-not", 0, 0, true),
      ("if-let", 2, 1, false),
      ("when-let", 1, 0, true),
    ] {
      let CalcitTypeAnnotation::Macro(signature) = core_file.defs[def_name].schema.as_ref() else {
        panic!("{def_name} should load as MacroSignature");
      };
      assert!(signature.is_strict(), "{def_name} should use a phase-aware contract");
      assert!(signature.capabilities.is_empty(), "{def_name} should be compile-time pure");
      assert_eq!(signature.required_inputs.len(), required, "{def_name} required inputs");
      assert_eq!(signature.optional_inputs.len(), optional, "{def_name} optional inputs");
      assert_eq!(signature.rest_input.is_some(), has_rest, "{def_name} rest input");
      match def_name {
        "or" | "when" | "when-not" => {
          assert!(matches!(
            signature.required_inputs.as_slice(),
            [crate::calcit::MacroSyntaxType::Expr(inner)]
              if matches!(inner.as_ref(), CalcitTypeAnnotation::Dynamic)
          ));
          assert!(matches!(
            signature.rest_input,
            Some(crate::calcit::MacroSyntaxType::Expr(ref inner))
              if matches!(inner.as_ref(), CalcitTypeAnnotation::Dynamic)
          ));
        }
        "either" | "if-not" => {
          assert!(signature.required_inputs.is_empty());
          assert!(matches!(
            signature.rest_input,
            Some(crate::calcit::MacroSyntaxType::Expr(ref inner))
              if matches!(inner.as_ref(), CalcitTypeAnnotation::Dynamic)
          ));
        }
        "if-let" => {
          assert!(matches!(
            signature.required_inputs.as_slice(),
            [
              crate::calcit::MacroSyntaxType::SyntaxList,
              crate::calcit::MacroSyntaxType::Expr(inner)
            ] if matches!(inner.as_ref(), CalcitTypeAnnotation::Dynamic)
          ));
          assert!(matches!(
            signature.optional_inputs.as_slice(),
            [crate::calcit::MacroSyntaxType::Expr(inner)]
              if matches!(inner.as_ref(), CalcitTypeAnnotation::Dynamic)
          ));
          assert!(signature.rest_input.is_none());
        }
        "when-let" => {
          assert!(matches!(
            signature.required_inputs.as_slice(),
            [crate::calcit::MacroSyntaxType::SyntaxList]
          ));
          assert!(matches!(
            signature.rest_input,
            Some(crate::calcit::MacroSyntaxType::Expr(ref inner))
              if matches!(inner.as_ref(), CalcitTypeAnnotation::Dynamic)
          ));
        }
        _ => unreachable!(),
      }
      if def_name == "when-let" {
        assert!(matches!(
          signature.expansion,
          crate::calcit::MacroExpansionType::Expr(ref inner)
            if matches!(
              inner.as_ref(),
              CalcitTypeAnnotation::TypeRef(name, args)
                if name.as_ref() == "Option"
                  && matches!(args.as_slice(), [item] if matches!(item.as_ref(), CalcitTypeAnnotation::Dynamic))
            )
        ));
      } else {
        assert!(matches!(
          signature.expansion,
          crate::calcit::MacroExpansionType::Expr(ref inner)
            if matches!(inner.as_ref(), CalcitTypeAnnotation::Dynamic)
        ));
      }
    }

    for def_name in [
      "tag-match",
      "list-match",
      "&list-match-internal",
      "struct-match",
      "&struct-match-internal",
      "case",
      "&case",
    ] {
      let CalcitTypeAnnotation::Macro(signature) = core_file.defs[def_name].schema.as_ref() else {
        panic!("{def_name} should load as MacroSignature");
      };
      assert!(signature.is_strict(), "{def_name} should use a phase-aware contract");
      assert!(signature.capabilities.is_empty(), "{def_name} should be compile-time pure");
      assert!(signature.optional_inputs.is_empty(), "{def_name} should not have optional inputs");
      assert!(matches!(
        signature.expansion,
        crate::calcit::MacroExpansionType::Expr(ref inner)
          if matches!(inner.as_ref(), CalcitTypeAnnotation::Dynamic)
      ));
      match def_name {
        "tag-match" | "struct-match" | "&struct-match-internal" | "case" => {
          assert!(matches!(
            signature.required_inputs.as_slice(),
            [crate::calcit::MacroSyntaxType::Expr(inner)]
              if matches!(inner.as_ref(), CalcitTypeAnnotation::Dynamic)
          ));
          assert!(matches!(signature.rest_input, Some(crate::calcit::MacroSyntaxType::SyntaxList)));
        }
        "list-match" => {
          assert!(signature.required_inputs.is_empty());
          assert!(matches!(signature.rest_input, Some(crate::calcit::MacroSyntaxType::Syntax)));
        }
        "&list-match-internal" => {
          assert!(matches!(
            signature.required_inputs.as_slice(),
            [
              crate::calcit::MacroSyntaxType::Expr(inner),
              crate::calcit::MacroSyntaxType::SyntaxList,
              crate::calcit::MacroSyntaxType::SyntaxList,
              crate::calcit::MacroSyntaxType::SyntaxList
            ] if matches!(inner.as_ref(), CalcitTypeAnnotation::Dynamic)
          ));
          assert!(signature.rest_input.is_none());
        }
        "&case" => {
          assert!(matches!(
            signature.required_inputs.as_slice(),
            [
              crate::calcit::MacroSyntaxType::Expr(item),
              crate::calcit::MacroSyntaxType::Expr(default),
              crate::calcit::MacroSyntaxType::SyntaxList
            ] if matches!(item.as_ref(), CalcitTypeAnnotation::Dynamic)
              && matches!(default.as_ref(), CalcitTypeAnnotation::Dynamic)
          ));
          assert!(matches!(signature.rest_input, Some(crate::calcit::MacroSyntaxType::SyntaxList)));
        }
        _ => unreachable!(),
      }
    }

    let CalcitTypeAnnotation::Macro(case_default) = core_file.defs["case-default"].schema.as_ref() else {
      panic!("case-default should load as MacroSignature");
    };
    assert!(case_default.is_strict());
    assert!(case_default.capabilities.is_empty());
    assert!(matches!(
      case_default.required_inputs.as_slice(),
      [
        crate::calcit::MacroSyntaxType::Expr(item),
        crate::calcit::MacroSyntaxType::Expr(default)
      ] if matches!(item.as_ref(), CalcitTypeAnnotation::Dynamic)
        && matches!(default.as_ref(), CalcitTypeAnnotation::Dynamic)
    ));
    assert!(matches!(case_default.rest_input, Some(crate::calcit::MacroSyntaxType::SyntaxList)));
    assert!(matches!(
      case_default.expansion,
      crate::calcit::MacroExpansionType::Expr(ref inner)
        if matches!(inner.as_ref(), CalcitTypeAnnotation::Dynamic)
    ));

    let internal_file = snapshot.files.get("calcit.internal").expect("calcit.internal file should exist");
    let CalcitTypeAnnotation::Macro(tag_internal) = internal_file.defs["&tag-match-internal"].schema.as_ref() else {
      panic!("&tag-match-internal should load as MacroSignature");
    };
    assert!(tag_internal.is_strict());
    assert!(tag_internal.capabilities.is_empty());
    assert!(tag_internal.optional_inputs.is_empty());
    assert!(matches!(
      tag_internal.required_inputs.as_slice(),
      [
        crate::calcit::MacroSyntaxType::Expr(value),
        crate::calcit::MacroSyntaxType::Expr(tag)
      ] if matches!(value.as_ref(), CalcitTypeAnnotation::Dynamic)
        && matches!(tag.as_ref(), CalcitTypeAnnotation::Tag)
    ));
    assert!(matches!(tag_internal.rest_input, Some(crate::calcit::MacroSyntaxType::SyntaxList)));
    assert!(matches!(
      tag_internal.expansion,
      crate::calcit::MacroExpansionType::Expr(ref inner)
        if matches!(inner.as_ref(), CalcitTypeAnnotation::Dynamic)
    ));

    for def_name in ["{}", "%{}", "{,}"] {
      let CalcitTypeAnnotation::Macro(signature) = core_file.defs[def_name].schema.as_ref() else {
        panic!("{def_name} should load as MacroSignature");
      };
      assert!(signature.is_strict(), "{def_name} should use a phase-aware contract");
      assert!(signature.capabilities.is_empty(), "{def_name} should be compile-time pure");
      assert!(signature.optional_inputs.is_empty(), "{def_name} should not have optional inputs");
      match def_name {
        "{}" => {
          assert!(signature.required_inputs.is_empty());
          assert!(matches!(signature.rest_input, Some(crate::calcit::MacroSyntaxType::SyntaxList)));
        }
        "%{}" => {
          assert!(matches!(
            signature.required_inputs.as_slice(),
            [crate::calcit::MacroSyntaxType::Expr(inner)]
              if matches!(inner.as_ref(), CalcitTypeAnnotation::Dynamic)
          ));
          assert!(matches!(signature.rest_input, Some(crate::calcit::MacroSyntaxType::SyntaxList)));
        }
        "{,}" => {
          assert!(signature.required_inputs.is_empty());
          assert!(matches!(signature.rest_input, Some(crate::calcit::MacroSyntaxType::Syntax)));
        }
        _ => unreachable!(),
      }
      if def_name == "%{}" {
        assert!(matches!(
          signature.expansion,
          crate::calcit::MacroExpansionType::Expr(ref inner)
            if inner.as_ref()
              == &CalcitTypeAnnotation::Custom(Arc::new(Calcit::tag("struct")))
        ));
      } else {
        assert!(matches!(
          signature.expansion,
          crate::calcit::MacroExpansionType::Expr(ref inner)
            if matches!(
              inner.as_ref(),
              CalcitTypeAnnotation::Map(key, value)
                if matches!(key.as_ref(), CalcitTypeAnnotation::Dynamic)
                  && matches!(value.as_ref(), CalcitTypeAnnotation::Dynamic)
            )
        ));
      }
    }

    let test_file = snapshot.files.get("calcit.test").expect("calcit.test file should exist");
    for def_name in ["is", "is-not=", "is-throws", "is=", "throws?"] {
      let CalcitTypeAnnotation::Macro(signature) = test_file.defs[def_name].schema.as_ref() else {
        panic!("calcit.test/{def_name} should load as MacroSignature");
      };
      let expected_input_count = match def_name {
        "is" | "is-throws" | "throws?" => 1,
        "is-not=" | "is=" => 2,
        _ => unreachable!(),
      };
      assert!(signature.is_strict(), "calcit.test/{def_name} should use a phase-aware contract");
      assert!(
        signature.capabilities.is_empty(),
        "calcit.test/{def_name} should be compile-time pure"
      );
      assert_eq!(signature.required_inputs.len(), expected_input_count);
      assert!(signature.required_inputs.iter().all(
        |input| matches!(input, crate::calcit::MacroSyntaxType::Expr(inner) if matches!(inner.as_ref(), CalcitTypeAnnotation::Dynamic))
      ));
      let expected = if def_name == "throws?" {
        &CalcitTypeAnnotation::Bool
      } else {
        &CalcitTypeAnnotation::Unit
      };
      assert!(matches!(
        signature.expansion,
        crate::calcit::MacroExpansionType::Expr(ref inner) if inner.as_ref() == expected
      ));
    }
  }

  #[test]
  fn optionally_schema_bridges_nullable_values_to_nominal_option() {
    let core_file_content = fs::read_to_string("src/cirru/calcit-core.cirru").expect("Failed to read calcit-core.cirru");
    let edn_data = cirru_edn::parse(&core_file_content).expect("Failed to parse cirru content as EDN");
    let snapshot = load_snapshot_data(&edn_data, "src/cirru/calcit-core.cirru").expect("Failed to parse snapshot");
    let entry = snapshot
      .files
      .get("calcit.core")
      .and_then(|file| file.defs.get("optionally"))
      .expect("calcit.core/optionally should exist");
    let CalcitTypeAnnotation::Fn(schema) = entry.schema.as_ref() else {
      panic!("optionally should have a function schema");
    };

    let input_var = match schema.arg_types.as_slice() {
      [arg] => match arg.as_ref() {
        CalcitTypeAnnotation::Optional(inner) => match inner.as_ref() {
          CalcitTypeAnnotation::TypeVar(name) => name,
          other => panic!("optionally Optional input should contain a type variable, got {other:?}"),
        },
        other => panic!("optionally should accept Optional<T>, got {other:?}"),
      },
      args => panic!("optionally should accept exactly one argument, got {args:?}"),
    };
    let output_var = match schema.return_type.as_ref() {
      CalcitTypeAnnotation::TypeRef(name, args) if name.as_ref() == "Option" => match args.as_slice() {
        [arg] => match arg.as_ref() {
          CalcitTypeAnnotation::TypeVar(name) => name,
          other => panic!("optionally Option output should contain a type variable, got {other:?}"),
        },
        args => panic!("optionally Option output should have one type argument, got {args:?}"),
      },
      other => panic!("optionally should return Option<T>, got {other:?}"),
    };
    assert_eq!(input_var, output_var, "optionally must preserve its input type variable");
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
      // Parallel tests may populate the core registry between the two loads,
      // causing the latter parser to qualify `Option` as `calcit.core/Option`.
      // Those references are nominally equivalent; the round-trip contract is
      // semantic type equality, while the assertions below separately protect
      // the serialized Fn/Macro markers.
      assert!(
        saved_entry.schema.matches_annotation(source_entry.schema.as_ref())
          && source_entry.schema.matches_annotation(saved_entry.schema.as_ref()),
        "schema should round-trip for {def_name}: source={:?}, saved={:?}",
        source_entry.schema,
        saved_entry.schema
      );
    }

    let _ = fs::remove_file(&temp_path);

    assert!(
      saved.contains("&+ $ %{} 'CodeEntry") && saved.contains(":schema $ :: 'Fn"),
      "saved snapshot should retain wrapped fn schemas"
    );
    assert!(
      saved.contains("%{} $ %{} 'CodeEntry") && saved.contains(":schema $ :: 'Macro"),
      "saved snapshot should retain wrapped macro schemas"
    );
  }

  #[test]
  fn test_custom_kind_schema_tags_round_trip_instead_of_degrading_to_dynamic() {
    // Regression test: `:struct`/`:enum`/`:trait`/`:impl`/`:record` shorthand tags load
    // into `CalcitTypeAnnotation::Custom(Arc<Calcit>)` (see `from_tag_name`). Any file
    // save previously coerced these back through `builtin_tag_name`, which doesn't know
    // about `Custom` and silently fell back to `:dynamic`, destroying the original kind
    // on every unrelated `calcit edit`/`calcit tree` write to the containing file.
    for kind in ["struct", "enum", "trait", "impl", "record"] {
      let schema = CalcitTypeAnnotation::from_tag_name(kind);
      let edn = schema_annotation_to_edn(&schema);
      assert_eq!(
        edn,
        Edn::enum_value(CalcitTypeAnnotation::canonical_type_symbol_name(kind).expect("known kind"), vec![]),
        "schema kind `:{kind}` must round-trip as a canonical symbol, not degrade to dynamic"
      );
    }
  }

  #[test]
  fn test_zero_payload_canonical_schema_wrappers_survive_repeated_round_trips() {
    for symbol in [
      "Dynamic",
      "Unit",
      "Bool",
      "Number",
      "String",
      "Symbol",
      "Tag",
      "List",
      "Map",
      "Set",
      "Fn",
      "Enum",
      "Ref",
      "Buffer",
      "CirruQuote",
      "JsObject",
      "Struct",
      "StructDef",
      "EnumDef",
      "Trait",
      "Impl",
    ] {
      let serialized = Edn::enum_value(symbol, vec![]);
      let first = parse_loaded_schema_annotation(&serialized, "test/schema").unwrap_or_else(|error| panic!("{symbol}: {error}"));
      let first_saved = schema_annotation_to_edn(first.as_ref());
      assert_eq!(first_saved, serialized, "{symbol} should survive the first save");

      let second = parse_loaded_schema_annotation(&first_saved, "test/schema").unwrap_or_else(|error| panic!("{symbol}: {error}"));
      assert_eq!(
        schema_annotation_to_edn(second.as_ref()),
        serialized,
        "{symbol} should not degrade on the second save"
      );
    }
  }

  #[test]
  fn test_zero_payload_macro_schema_wrapper_canonicalizes_to_dyn_fn() {
    let parsed = parse_loaded_schema_annotation(&Edn::enum_value("Macro", vec![]), "test/schema").unwrap();
    assert!(matches!(parsed.as_ref(), CalcitTypeAnnotation::DynFn));

    let canonical = Edn::enum_value("Fn", vec![]);
    let first_saved = schema_annotation_to_edn(parsed.as_ref());
    assert_eq!(first_saved, canonical);

    let second = parse_loaded_schema_annotation(&first_saved, "test/schema").unwrap();
    assert!(matches!(second.as_ref(), CalcitTypeAnnotation::DynFn));
    assert_eq!(schema_annotation_to_edn(second.as_ref()), canonical);
  }

  #[test]
  fn test_validate_serialized_snapshot_content_rejects_double_quoted_generics() {
    let content = r#"{} (:package |mini)
  :version |0.0.0
  :entries $ {}
    :default $ {} (:mode :native) (:init-fn |mini/main!) (:reload-fn |mini/main!)
      :modules $ []
  :files $ {}
    |mini $ %{} :FileEntry
      :ns $ %{} :CodeEntry (:doc |) (:code $ quote (ns mini)) (:examples $ []) (:schema nil)
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote (defn main! (x) x)
          :examples $ []
          :schema $ :: :fn
            {} (:args $ [] :dynamic) (:generics $ [] ''T) (:return :dynamic)
"#;

    let err = validate_serialized_snapshot_content(content).expect_err("serialized snapshot should reject double-quoted generics");
    assert!(
      err.contains("serialized snapshot has invalid `:schema`") && err.contains("excess leading quotes"),
      "unexpected error: {err}"
    );
  }

  #[test]
  fn test_load_snapshot_reports_empty_top_level_version_with_field_context() {
    let content = r#"{} (:package |mini)
  :version ||
  :entries $ {}
    :default $ {} (:mode :native) (:init-fn |mini/main!) (:reload-fn |mini/main!)
      :modules $ []
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
    let err = load_snapshot_data(&edn_data, "mini.cirru").expect_err("empty top-level version should fail on load");

    assert!(err.contains("snapshot.version cannot be empty"), "unexpected error: {err}");
    assert!(err.contains("||"), "unexpected error: {err}");
  }

  #[test]
  fn test_entry_type_slots_and_feature_policy_round_trip_for_default_and_named_entries() {
    let content = r#"{} (:package |mini)
  :version |0.0.0
  :entries $ {}
    :default $ {} (:mode :js) (:init-fn |mini/main!) (:reload-fn 'mini/reload!)
      :description "|Browser client entry"
      :target :browser
      :modules $ []
      :type-slots $ {} (:dispatch-op |mini.schema/ClientOp)
      :feature-policy $ {} (:js-ffi :error)
    :server $ {} (:mode :native) (:init-fn 'mini/server-main!) (:reload-fn 'mini/reload!)
      :description "|HTTP server entry"
      :target :node
      :modules $ []
      :type-slots $ {} (:dispatch-op |mini.schema/ServerOp) (:optional-op :dynamic)
      :feature-policy $ {} (:js-ffi :warn)
  :files $ {}
    |mini $ %{} :FileEntry
      :ns $ %{} :CodeEntry (:doc |) (:code $ quote (ns mini)) (:examples $ []) (:schema nil)
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |) (:code $ quote (defn main! () nil)) (:examples $ []) (:schema nil)
        |reload! $ %{} :CodeEntry (:doc |) (:code $ quote (defn reload! () nil)) (:examples $ []) (:schema nil)
"#;

    let edn_data = cirru_edn::parse(content).expect("snapshot text should parse as EDN");
    let snapshot = load_snapshot_data(&edn_data, "mini.cirru").expect("snapshot should load");
    assert_eq!(
      snapshot.entries[DEFAULT_ENTRY_NAME]
        .type_slots
        .get("dispatch-op")
        .map(String::as_str),
      Some("mini.schema/ClientOp")
    );
    assert_eq!(snapshot.entries[DEFAULT_ENTRY_NAME].mode, SnapshotRunMode::Js);
    assert_eq!(snapshot.entries[DEFAULT_ENTRY_NAME].description, "Browser client entry");
    assert_eq!(snapshot.entries[DEFAULT_ENTRY_NAME].target, Some(SnapshotTarget::Browser));
    assert_eq!(
      snapshot.entries[DEFAULT_ENTRY_NAME].feature_policy.get("js-ffi"),
      Some(&FeaturePolicy::Error)
    );
    let server = snapshot.entries.get("server").expect("server entry");
    assert_eq!(server.description, "HTTP server entry");
    assert_eq!(server.target, Some(SnapshotTarget::Node));
    assert_eq!(
      server.type_slots.get("dispatch-op").map(String::as_str),
      Some("mini.schema/ServerOp")
    );
    assert_eq!(server.type_slots.get("optional-op").map(String::as_str), Some(":dynamic"));
    assert_eq!(server.feature_policy.get("js-ffi"), Some(&FeaturePolicy::Warn));

    let rendered = render_snapshot_content(&snapshot).expect("snapshot should render");
    assert!(
      !rendered.contains(":version"),
      "snapshot version must stay in deps.cirru, not calcit.cirru: {rendered}"
    );
    assert!(
      rendered.contains(":init-fn 'mini/main!"),
      "entry function should be stored as a symbol: {rendered}"
    );
    assert!(
      rendered.contains(":reload-fn 'mini/reload!"),
      "entry function should be stored as a symbol: {rendered}"
    );
    let rendered_edn = cirru_edn::parse(&rendered).expect("rendered snapshot should parse");
    let restored = load_snapshot_data(&rendered_edn, "mini.cirru").expect("rendered snapshot should load");
    assert_eq!(
      restored.entries[DEFAULT_ENTRY_NAME].type_slots,
      snapshot.entries[DEFAULT_ENTRY_NAME].type_slots
    );
    assert_eq!(restored.entries["server"].type_slots, server.type_slots);
    assert_eq!(
      restored.entries[DEFAULT_ENTRY_NAME].target,
      snapshot.entries[DEFAULT_ENTRY_NAME].target
    );
    assert_eq!(restored.entries["server"].target, server.target);
    assert_eq!(
      restored.entries[DEFAULT_ENTRY_NAME].feature_policy,
      snapshot.entries[DEFAULT_ENTRY_NAME].feature_policy
    );
    assert_eq!(restored.entries["server"].feature_policy, server.feature_policy);
  }

  #[test]
  fn legacy_configs_are_rejected_with_current_format_migration_guidance() {
    let content = r#"{} (:package |mini)
  :configs $ {} (:init-fn |mini/main!) (:reload-fn |mini/reload!) (:version |1.2.3)
    :modules $ [] |legacy/
  :entries $ {}
  :files $ {}
"#;
    let edn_data = cirru_edn::parse(content).expect("legacy snapshot text should parse");
    let error = load_snapshot_data(&edn_data, "fixtures/mini.cirru").expect_err("legacy configs must be rejected");
    assert!(error.contains("Top-level `:configs` is retired"), "error: {error}");
    assert!(error.contains("current Calcit"), "error: {error}");
    assert!(error.contains("Runtime loading remains strict"), "error: {error}");
    assert!(error.contains("calcit 'fixtures/mini.cirru' edit format"), "error: {error}");
    assert!(error.contains("calcit 'fixtures/mini.cirru' --check-only"), "error: {error}");
  }

  fn legacy_direct_quote_snapshot(config_overrides: impl IntoIterator<Item = (&'static str, Edn)>) -> Edn {
    let mut configs = EdnMapView::default();
    configs.insert_key("init-fn", Edn::str("mini/main!"));
    configs.insert_key("reload-fn", Edn::str("mini/reload!"));
    configs.insert_key("version", Edn::str("1.2.3"));
    configs.insert_key("modules", Edn::List(EdnListView(vec![Edn::str("legacy/")])));
    for (key, value) in config_overrides {
      configs.insert_key(key, value);
    }

    let modern = CodeEntry::from_code(parse_one("defn modern () nil"));
    let mut defs = EdnMapView::default();
    defs.insert(Edn::str("main!"), Edn::Quote(parse_one("defn main! () nil")));
    defs.insert(Edn::str("modern"), Edn::from(&modern));
    let file = Edn::map_from_iter([
      (Edn::tag("ns"), Edn::Quote(vec!["ns", "mini.core"].into())),
      (Edn::tag("defs"), Edn::Map(defs)),
    ]);
    Edn::map_from_iter([
      (Edn::tag("package"), Edn::str("mini")),
      (Edn::tag("configs"), Edn::Map(configs)),
      (Edn::tag("entries"), Edn::Map(EdnMapView::default())),
      (Edn::tag("files"), Edn::map_from_iter([(Edn::str("mini.core"), file)])),
    ])
  }

  #[test]
  fn format_loader_migrates_direct_quotes_and_configs_without_weakening_strict_loader() {
    let legacy = legacy_direct_quote_snapshot([]);
    let strict_error = load_snapshot_data(&legacy, "calcit.cirru").expect_err("strict loader must reject legacy configs");
    assert!(strict_error.contains("Top-level `:configs` is retired"), "error: {strict_error}");

    let mut direct_quotes_only = legacy.clone();
    let Edn::Map(root) = &mut direct_quotes_only else {
      panic!("legacy fixture root map")
    };
    root.0.remove(&Edn::tag("configs"));
    let default_entry = Edn::map_from_iter([
      (Edn::tag("mode"), Edn::tag("native")),
      (Edn::tag("init-fn"), Edn::Symbol("mini/main!".into())),
      (Edn::tag("reload-fn"), Edn::Symbol("mini/reload!".into())),
    ]);
    root.insert_key("entries", Edn::map_from_iter([(Edn::str(DEFAULT_ENTRY_NAME), default_entry)]));
    let strict_error =
      load_snapshot_data(&direct_quotes_only, "calcit.cirru").expect_err("strict loader must also reject direct-quote code");
    assert!(strict_error.contains("mini.core/:ns"), "error: {strict_error}");
    assert!(strict_error.contains("expected struct/map"), "error: {strict_error}");

    let (snapshot, migration) =
      load_snapshot_data_for_format(&legacy, "calcit.cirru").expect("format loader should migrate the constrained legacy shape");
    assert_eq!(
      migration,
      SnapshotFormatMigration {
        direct_quote_namespaces: 1,
        direct_quote_definitions: 1,
        legacy_configs: true,
      }
    );
    assert_eq!(snapshot.version, "1.2.3");
    assert_eq!(snapshot.entries[DEFAULT_ENTRY_NAME].mode, SnapshotRunMode::Native);
    assert_eq!(snapshot.entries[DEFAULT_ENTRY_NAME].modules, vec!["legacy/"]);
    assert_eq!(snapshot.files["mini.core"].defs["main!"].schema, DYNAMIC_TYPE.clone());
    assert!(snapshot.files["mini.core"].defs.contains_key("modern"));

    let rendered = render_snapshot_content(&snapshot).expect("migrated snapshot should render canonically");
    assert!(!rendered.contains(":configs"), "rendered: {rendered}");
    assert!(rendered.contains(":entries"), "rendered: {rendered}");
    let canonical = cirru_edn::parse(&rendered).expect("canonical output should parse");
    load_snapshot_data(&canonical, "calcit.cirru").expect("canonical output should pass the strict loader");
  }

  #[test]
  fn format_loader_rejects_unknown_legacy_configs_fields() {
    let legacy = legacy_direct_quote_snapshot([("custom", Edn::Bool(true))]);
    let error = load_snapshot_data_for_format(&legacy, "calcit.cirru").expect_err("unknown legacy config must not be discarded");
    assert!(error.contains("legacy configs: unknown field `:custom`"), "error: {error}");
  }

  #[test]
  fn format_loader_accepts_fileentry_struct_written_by_schema_migration_release() {
    let legacy = legacy_direct_quote_snapshot([]);
    let (snapshot, _) = load_snapshot_data_for_format(&legacy, "calcit.cirru").expect("legacy fixture should migrate");
    let rendered = render_snapshot_content(&snapshot).expect("migrated snapshot should render");
    let canonical = cirru_edn::parse(&rendered).expect("rendered snapshot should parse");
    let Edn::Map(root) = &canonical else { panic!("snapshot root map") };
    let files = root.get(&Edn::tag("files")).expect("files");
    let Edn::Map(files) = files else { panic!("files map") };
    let file = files.get(&Edn::Symbol("mini.core".into())).expect("canonical Symbol file key");
    let Edn::Struct(file) = file else { panic!("FileEntry struct") };
    let defs = file
      .pairs
      .iter()
      .find(|(key, _)| key.ref_str() == "defs")
      .map(|(_, value)| value)
      .expect("defs");
    let Edn::Map(defs) = defs else { panic!("defs map") };
    assert!(defs.get(&Edn::Symbol("main!".into())).is_some(), "canonical Symbol def key");

    load_snapshot_data(&canonical, "calcit.cirru").expect("strict loader already accepts FileEntry structs");
    let (_, migration) =
      load_snapshot_data_for_format(&canonical, "calcit.cirru").expect("format loader should accept FileEntry structs too");
    assert_eq!(migration, SnapshotFormatMigration::default());
  }

  #[test]
  fn snapshot_loader_rejects_string_symbol_identifier_collision() {
    let mut legacy = legacy_direct_quote_snapshot([]);
    let Edn::Map(root) = &mut legacy else {
      panic!("legacy fixture root map")
    };
    let files = root.0.get_mut(&Edn::tag("files")).expect("files");
    let Edn::Map(files) = files else { panic!("files map") };
    let duplicate = files.get(&Edn::str("mini.core")).expect("legacy String key").clone();
    files.insert(Edn::Symbol("mini.core".into()), duplicate);

    let error = load_snapshot_data_for_format(&legacy, "calcit.cirru").expect_err("normalized collision must fail");
    assert!(error.contains("duplicate snapshot identifier `mini.core`"), "error: {error}");
  }

  #[test]
  fn format_loader_reports_malformed_legacy_definition_with_owner() {
    let mut legacy = legacy_direct_quote_snapshot([]);
    let Edn::Map(root) = &mut legacy else {
      panic!("legacy fixture root map")
    };
    let files = root.0.get_mut(&Edn::tag("files")).expect("files");
    let Edn::Map(files) = files else { panic!("files map") };
    let file = files.0.get_mut(&Edn::str("mini.core")).expect("mini.core");
    let Edn::Map(file) = file else { panic!("file map") };
    let defs = file.0.get_mut(&Edn::tag("defs")).expect("defs");
    let Edn::Map(defs) = defs else { panic!("defs map") };
    defs.insert(Edn::str("broken"), Edn::Number(1.0));

    let error = load_snapshot_data_for_format(&legacy, "calcit.cirru").expect_err("malformed legacy definition must fail");
    assert!(error.contains("mini.core/broken"), "error: {error}");
    assert!(error.contains("expected struct/map"), "error: {error}");
  }

  #[test]
  fn test_entry_type_slots_reject_duplicate_normalized_names() {
    let slots = Edn::Map(EdnMapView(HashMap::from([
      (Edn::tag("dispatch-op"), Edn::str("mini.schema/ClientOp")),
      (Edn::str(":dispatch-op"), Edn::str("mini.schema/ServerOp")),
    ])));
    let err = parse_snapshot_type_slots(&slots, "configs").expect_err("duplicate normalized slot names should fail");
    assert!(err.contains("duplicate slot name `:dispatch-op`"), "unexpected error: {err}");
  }

  #[test]
  fn test_feature_policy_rejects_empty_feature_name() {
    let policies = Edn::map_from_iter([(Edn::str(""), Edn::tag("error"))]);
    let err = parse_snapshot_feature_policy(&policies, "entry").expect_err("empty feature names should be rejected");
    assert!(err.contains("feature name cannot be empty"), "unexpected error: {err}");
  }

  #[test]
  fn create_file_from_snippet_promotes_top_level_defs() {
    let raw = r#"ns app.demo
  :require
    respo.core :refer $ div

def style-space $ {}
  :width "|1px"

defn compute (w h)
  + w h

defcomp comp-space (w h)
  div $ {}
"#;
    let file = create_file_from_snippet(raw).expect("snippet should parse");
    assert!(file.defs.contains_key("style-space"));
    assert!(file.defs.contains_key("compute"));
    assert!(file.defs.contains_key("comp-space"));
    // main! and reload! are always injected as no-op entry points
    assert!(file.defs.contains_key("main!"));
    assert!(file.defs.contains_key("reload!"));
  }
}
