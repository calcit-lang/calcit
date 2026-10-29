use cirru_edn::{Edn, EdnListView, EdnMapView, EdnSetView, EdnStructView, EdnTag, from_edn};
use cirru_parser::Cirru;
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;
use std::path::Path;
use std::sync::Arc;

use crate::calcit::{
  Calcit, CalcitFnTypeAnnotation, CalcitTypeAnnotation, DYNAMIC_TYPE, MacroExpansionType, MacroSignature, MacroSyntaxType, ParamShape,
  ParamShapeToken, SchemaKind, with_type_annotation_warning_context,
};
use crate::data::edn::{format_deserialize_error, format_edn_display};

/// Named declaration heads accepted by the structured definition editor.
pub const DEFINITION_HEADS: &[&str] = &[
  "def",
  "defn",
  "defmacro",
  "defstruct",
  "defenum",
  "deftrait",
  "defimpl",
  "defref",
  "defexternal",
  "defwasm-export",
  "defwasm-import",
];

/// Reject a named declaration used as the operator of a Snapshot root call.
/// Do not inspect bodies, quoted data, anonymous functions or ordinary calls.
pub fn validate_definition_root(code: &Cirru, owner: &str) -> Result<(), String> {
  let Cirru::List(items) = code else { return Ok(()) };
  let mut operator = items.first();
  while let Some(Cirru::List(nested)) = operator {
    if let Some(Cirru::Leaf(head)) = nested.first()
      && DEFINITION_HEADS.contains(&head.as_ref())
    {
      return Err(format!(
        "{owner}: wrapped top-level definition `{head}` would be called during initialization; put the named declaration directly at the definition root, without an extra list"
      ));
    }
    operator = nested.first();
  }
  Ok(())
}

const SNAPSHOT_ABOUT_MESSAGE: &str = "Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`.";

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
    "async" => Some("async"),
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

/// Read one field from definition-level `:ffi` metadata.
pub fn ffi_metadata_value<'a>(ffi: &'a Edn, key: &str) -> Option<&'a Edn> {
  match ffi {
    Edn::Struct(value) => value.pairs.iter().find(|(field, _)| field.ref_str() == key).map(|(_, value)| value),
    Edn::Map(value) => value.get(&Edn::tag(key)),
    _ => None,
  }
}

/// Read the optional host target from definition-level `:ffi` metadata.
///
/// Definitions without a target are shared across entry targets. Callers that
/// implement fail-closed checks should surface the returned error instead of
/// treating malformed target metadata as shared.
pub fn parse_ffi_target(ffi: &Edn) -> Result<Option<SnapshotTarget>, String> {
  match ffi {
    Edn::Struct(_) => {}
    Edn::Map(value) => {
      if let Some(key) = value.0.keys().find(|key| !matches!(key, Edn::Tag(_))) {
        return Err(format!("expected definition :ffi map keys to be tags, got `{key}`"));
      }
    }
    _ => return Err(format!("expected definition :ffi metadata to be a map or struct, got `{ffi}`")),
  }
  let value = ffi_metadata_value(ffi, "target");
  let Some(value) = value else {
    return Ok(None);
  };
  let name = match value {
    Edn::Tag(tag) => tag.ref_str(),
    Edn::Str(text) | Edn::Symbol(text) => text.trim_start_matches(':'),
    _ => return Err(format!("expected :ffi :target to be a tag, string, or symbol, got `{value}`")),
  };
  match name {
    "browser" => Ok(Some(SnapshotTarget::Browser)),
    "node" => Ok(Some(SnapshotTarget::Node)),
    "native" => Ok(Some(SnapshotTarget::Native)),
    "wasm" => Ok(Some(SnapshotTarget::Wasm)),
    _ => Err(format!("unknown :ffi :target `{name}`; expected browser, node, native, or wasm")),
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

pub const VERIFICATION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VerificationCheckKind {
  Strict,
}

impl VerificationCheckKind {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Strict => "strict",
    }
  }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerificationFailurePolicy {
  #[default]
  Stop,
  Continue,
}

impl VerificationFailurePolicy {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Stop => "stop",
      Self::Continue => "continue",
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationProfile {
  pub entries: Vec<String>,
  pub checks: Vec<VerificationCheckKind>,
  #[serde(default, rename = "on-failure")]
  pub on_failure: VerificationFailurePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationConfig {
  #[serde(rename = "schema-version")]
  pub schema_version: u32,
  pub profiles: HashMap<String, VerificationProfile>,
  #[serde(default, rename = "host-requirements")]
  pub host_requirements: HashMap<String, String>,
  #[serde(default, rename = "external-gates")]
  pub external_gates: Vec<String>,
}

impl Default for VerificationConfig {
  fn default() -> Self {
    Self {
      schema_version: VERIFICATION_SCHEMA_VERSION,
      profiles: HashMap::new(),
      host_requirements: HashMap::new(),
      external_gates: Vec::new(),
    }
  }
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
  #[serde(default)]
  pub verification: VerificationConfig,
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
    let (code, ffi) = normalize_entry_external(self.code, self.ffi, owner)?;
    // Match the text loader: a definition-kind schema stored as Dynamic must
    // become the canonical marker, including for `defexternal`-lowered
    // `deftrait` entries.
    let schema = normalize_schema_for_code(&code, &schema);

    Ok(CodeEntry {
      doc: self.doc,
      examples: self.examples,
      tests,
      tags: tags_vec_to_set(self.tags),
      code,
      schema,
      ffi,
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
    verification: raw.verification,
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

/// `Tuple` used to name Enum; it is now the positional type and needs element types.
const BARE_TUPLE_SCHEMA_ERROR: &str = "`Tuple` needs its element types, such as `Tuple Number String`.";

fn parse_loaded_schema_annotation(value: &Edn, owner: &str) -> Result<Arc<CalcitTypeAnnotation>, String> {
  if matches!(value, Edn::Nil) {
    return Ok(DYNAMIC_TYPE.clone());
  }

  // A top-level quoted symbol is rendered by Cirru EDN as `'String` (rather
  // than as a list context such as `[] 'String`) and parses back as `Quote`.
  // Treat that one-node quote as the canonical nominal type spelling while
  // keeping all older tag spellings accepted below.
  // Compact storage `quote $ Fn Number (-> String)` goes through the same
  // desugaring as `edit schema` input.
  if let Edn::Quote(payload @ Cirru::List(_)) = value {
    if !crate::calcit::type_expr::is_compact_payload(payload) {
      return Err(format!(
        "{owner} quotes a non-compact schema; write it as `quote $ Fn ...` or `quote $ Macro ...`"
      ));
    }
    return parse_schema_annotation_for_write(payload).map_err(|e| format!("failed to parse {owner}: {e}"));
  }

  if let Edn::Quote(leaf @ Cirru::Leaf(symbol)) = value {
    if symbol.as_ref() == "Tuple" {
      return Err(format!("{owner}: {BARE_TUPLE_SCHEMA_ERROR}"));
    }
    let annotation = CalcitTypeAnnotation::parse_type_annotation_from_edn(&Edn::Symbol(symbol.clone()));
    if CalcitTypeAnnotation::canonical_type_symbol_name(symbol).is_some() {
      return Ok(annotation);
    }
    // `quote app.main/Person` names a nominal type in compact storage.
    if let Ok(quoted) = crate::calcit::type_expr::desugar_type_expr(leaf) {
      return parse_schema_annotation_for_write(&quoted).map_err(|e| format!("failed to parse {owner}: {e}"));
    }
  }

  // Snapshot writers wrap standalone type symbols as zero-payload EDN enums
  // (`:: 'String`, `:: 'StructDef`, ...). Decode those wrappers back through
  // the canonical symbol parser. Otherwise a second unrelated Snapshot write
  // would interpret definition-kind markers as anonymous Enum values and
  // silently serialize `StructDef`/`EnumDef` as `Enum`.
  if let Edn::Enum(view) = value
    && view.extra.is_empty()
    && view.variant.as_ref() == "Tuple"
  {
    return Err(format!("{owner}: {BARE_TUPLE_SCHEMA_ERROR}"));
  }
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

/// Snapshot spelling of a definition schema: the compact `quote $ Fn ...`
/// form when it round-trips exactly, otherwise the canonical EDN.
pub fn schema_annotation_to_storage_edn(schema: &CalcitTypeAnnotation) -> Edn {
  let canonical = schema_annotation_to_edn(schema);
  match compact_type_spelling(&canonical) {
    Some(compact) => Edn::Quote(compact),
    None => canonical,
  }
}

/// Compact spelling (the payload of `quote $ ...`) of a definition schema when
/// it round-trips exactly.
pub fn compact_type_spelling(canonical: &Edn) -> Option<Cirru> {
  let compact = match canonical {
    Edn::Enum(view) if view.type_name.is_none() && (!view.extra.is_empty() || matches!(view.variant.as_ref(), "Fn" | "Macro")) => {
      crate::calcit::type_expr::print_type_compact(canonical).ok()
    }
    Edn::Enum(view) if view.type_name.is_none() && view.extra.is_empty() => Some(Cirru::Leaf(view.variant.clone())),
    _ => None,
  }?;
  round_trip_compact(compact, canonical)
}

/// Compact spelling of any type EDN for display (`query type`, `query schema`),
/// including bare names and `::` type lists; `None` when no exact spelling exists.
pub fn compact_type_display(canonical: &Edn) -> Option<Cirru> {
  match canonical {
    Edn::Symbol(_) | Edn::List(_) => {
      let compact = crate::calcit::type_expr::print_type_compact(canonical).ok()?;
      round_trip_compact(compact, canonical)
    }
    _ => compact_type_spelling(canonical),
  }
}

fn round_trip_compact(compact: Cirru, canonical: &Edn) -> Option<Cirru> {
  if let Cirru::Leaf(name) = &compact
    && CalcitTypeAnnotation::canonical_type_symbol_name(name) == Some(name.as_ref())
  {
    return Some(compact);
  }
  // Compare spellings structurally rather than by re-parsing: parsing resolves
  // nominal names against whatever the program registry holds at this moment,
  // which made the stored form depend on load order.
  match crate::calcit::type_expr::desugar_type_expr(&compact) {
    Ok(back) if normalize_type_edn(&schema_cirru_to_edn(back.clone())) == normalize_type_edn(canonical) => Some(compact),
    _ => None,
  }
}

/// Folds equivalent encodings of one type: a zero-payload `:: 'Name` is the
/// symbol `Name`, and a type-ref vector `[] ':: 'Name args…` is `:: 'Name args…`.
fn normalize_type_edn(edn: &Edn) -> Edn {
  match edn {
    Edn::Enum(view) if view.type_name.is_none() && view.extra.is_empty() => Edn::Symbol(view.variant.clone()),
    Edn::Enum(view) => Edn::Enum(cirru_edn::EdnEnumView {
      variant: view.variant.clone(),
      type_name: view.type_name.clone(),
      extra: view.extra.iter().map(normalize_type_edn).collect(),
    }),
    Edn::List(EdnListView(items)) => match items.as_slice() {
      [Edn::Symbol(head), Edn::Symbol(name), rest @ ..] if head.as_ref() == "::" => {
        normalize_type_edn(&Edn::enum_value(name.clone(), rest.to_vec()))
      }
      _ => Edn::List(EdnListView(items.iter().map(normalize_type_edn).collect())),
    },
    Edn::Map(EdnMapView(map)) => Edn::Map(EdnMapView(map.iter().map(|(k, v)| (k.clone(), normalize_type_edn(v))).collect())),
    other => other.clone(),
  }
}

fn code_entry_edn_pairs(data: &CodeEntry) -> Vec<(EdnTag, Edn)> {
  let schema = normalize_schema_for_code(&data.code, &data.schema);
  let mut pairs = vec![
    ("doc".into(), data.doc.to_owned().into()),
    ("examples".into(), data.examples.to_owned().into()),
    ("code".into(), data.code.to_owned().into()),
  ];
  // Absence is an inference hole, not an explicit open boundary. Preserve
  // that distinction through every structural edit and format round trip.
  if !schema_annotation_is_missing(&schema) {
    pairs.push(("schema".into(), schema_annotation_to_storage_edn(schema.as_ref())));
  }
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
  if schema_annotation_is_missing(&entry.schema) {
    update_part(&mut hasher, "schema-presence", b"missing");
  }

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
              parse_ffi_target(value).map_err(|e| format!("failed to parse CodeEntry.ffi: {e}"))?;
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
          parse_ffi_target(value).map_err(|e| format!("failed to parse CodeEntry.ffi: {e}"))?;
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
    let (code, ffi) = normalize_entry_external(code, ffi, "CodeEntry")?;
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
          match value {
            "true" => Some(Edn::Bool(true)),
            "false" => Some(Edn::Bool(false)),
            "nil" => Some(Edn::Nil),
            _ => Some(Edn::Symbol(text.clone())),
          }
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

/// Edit inputs may still spell code types in the retired `::`/`{}` form; they
/// are written in the compact spelling, the same rewrite `edit format` does.
/// Returns `None` when nothing needs rewriting.
fn migrate_snapshot_code_for_write(snapshot: &Snapshot) -> Option<Snapshot> {
  let needs_rewrite = |entry: &CodeEntry| {
    migrate_code_type_syntax(&entry.code).1 > 0
      || entry.examples.iter().any(|example| migrate_code_type_syntax(example).1 > 0)
      || entry.tests.iter().any(|test| migrate_code_type_syntax(&test.code).1 > 0)
  };
  if !snapshot.files.values().any(|file| file.defs.values().any(needs_rewrite)) {
    return None;
  }
  let mut migrated = snapshot.clone();
  for file in migrated.files.values_mut() {
    for entry in file.defs.values_mut() {
      if !needs_rewrite(entry) {
        continue;
      }
      entry.code = migrate_code_type_syntax(&entry.code).0;
      entry.examples = entry.examples.iter().map(|example| migrate_code_type_syntax(example).0).collect();
      for test in &mut entry.tests {
        test.code = migrate_code_type_syntax(&test.code).0;
      }
    }
  }
  Some(migrated)
}

fn validate_snapshot_contracts_for_write(snapshot: &Snapshot) -> Result<(), String> {
  check_snapshot_code_type_syntax(&snapshot.files, "<snapshot>")?;
  for (ns_name, file_data) in &snapshot.files {
    if ns_name.ends_with(".$meta") {
      continue;
    }

    for (def_name, code_entry) in &file_data.defs {
      validate_schema_for_snapshot_write(&format!("{ns_name}/{def_name}"), &code_entry.schema)?;
      crate::js_ffi_source::validate_js_definition(code_entry).map_err(|error| format!("{ns_name}/{def_name}: {error}"))?;
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
    if let Cirru::List(items) = schema
      && let [Cirru::Leaf(head), payload] = items.as_slice()
      && head.as_ref() == "quote"
      && crate::calcit::type_expr::is_compact_payload(payload)
    {
      return parse_schema_annotation_for_write(payload).map(|_| ());
    }
    if let Cirru::List(items) = schema
      && let [Cirru::Leaf(head), Cirru::Leaf(name)] = items.as_slice()
      && head.as_ref() == "quote"
    {
      if name.as_ref() == "Tuple" {
        return Err(BARE_TUPLE_SCHEMA_ERROR.to_owned());
      }
      return match CalcitTypeAnnotation::canonical_type_symbol_name(name) {
        Some(canonical) if canonical == name.as_ref() => Ok(()),
        _ if let Ok(quoted) = crate::calcit::type_expr::desugar_type_expr(&items[1]) => {
          parse_schema_annotation_for_write(&quoted).map(|_| ())
        }
        _ => Err(format!("unknown type name `{name}` in quoted schema")),
      };
    }
    validate_schema_for_write(schema)
  }

  fn pairs(node: &Cirru) -> impl Iterator<Item = (usize, &[Cirru])> {
    let items = match node {
      Cirru::List(items) => items.as_slice(),
      _ => &[],
    };
    items.iter().enumerate().filter_map(|(index, item)| match item {
      Cirru::List(pair) if pair.len() == 2 => Some((index, pair.as_slice())),
      _ => None,
    })
  }

  fn field<'a>(node: &'a Cirru, name: &str) -> Option<(usize, &'a Cirru)> {
    pairs(node).find_map(|(index, pair)| matches!(&pair[0], Cirru::Leaf(key) if key.as_ref() == name).then_some((index, &pair[1])))
  }

  fn validate_entry(node: &Cirru, mut path: Vec<usize>) -> Result<(), String> {
    let Some((index, schema)) = field(node, ":schema") else {
      return Ok(());
    };
    if matches!(schema, Cirru::Leaf(s) if s.as_ref() == "nil") {
      return Ok(());
    }
    path.push(index);
    validate_serialized_schema(schema).map_err(|e| format!("serialized snapshot has invalid `:schema` at {path:?}: {e}"))
  }

  let nodes = cirru_parser::parse(content).map_err(|e| format!("Failed to parse serialized snapshot content: {e}"))?;
  // Follow Snapshot metadata, never recursively interpreting quoted source,
  // examples, tests or arbitrary FFI data as additional schema declarations.
  for (root_index, node) in nodes.iter().enumerate() {
    let Some((files_index, files)) = field(node, ":files") else {
      continue;
    };
    for (file_index, file_pair) in pairs(files) {
      let Some((defs_index, defs)) = field(&file_pair[1], ":defs") else {
        continue;
      };
      for (def_index, def_pair) in pairs(defs) {
        validate_entry(
          &def_pair[1],
          vec![root_index, files_index, 1, file_index, 1, defs_index, 1, def_index, 1],
        )?;
      }
    }
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
  ":async",
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

fn core_nominal_schema_short_ref_arity(value: &str) -> Option<usize> {
  match value.trim_start_matches('\'') {
    "Option" => Some(1),
    "Result" => Some(2),
    _ => None,
  }
}

fn check_no_legacy_data_type_names(schema: &Cirru) -> Result<(), String> {
  match schema {
    Cirru::Leaf(value) => {
      let name = value.trim_start_matches(['\'', ':']);
      let replacement = match name {
        "record" | "Record" => Some("Struct"),
        "tuple" => Some("Enum"),
        _ => None,
      };
      if let Some(replacement) = replacement {
        return Err(format!(
          "Legacy type name `{name}` was removed by the struct/enum data-model migration; use `'{replacement}`."
        ));
      }
      if value.trim_start_matches('\'') == "Tuple" {
        return Err(BARE_TUPLE_SCHEMA_ERROR.to_owned());
      }
      Ok(())
    }
    Cirru::List(items) => {
      // `:: 'Tuple A B` is the applied positional type; only its head may name it.
      let applied_tuple = matches!(items.first(), Some(Cirru::Leaf(head)) if head.as_ref() == "::")
        && matches!(items.get(1), Some(Cirru::Leaf(name)) if name.trim_start_matches('\'') == "Tuple");
      for (idx, item) in items.iter().enumerate() {
        if applied_tuple && idx == 1 {
          if items.len() < 3 {
            return Err("`Tuple` needs at least one element type, such as `Tuple Number String`.".to_owned());
          }
          continue;
        }
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
    && core_nominal_schema_short_ref_arity(type_name).is_none()
  {
    return Err(format!(
      "Unknown standalone type `{type_name}`. Use a built-in type name or a fully qualified nominal type such as `app.schema/Store`."
    ));
  }

  if let Cirru::List(items) = schema
    && matches!(items.first(), Some(Cirru::Leaf(head)) if head.as_ref() == "::")
    && let Some(Cirru::Leaf(type_name)) = items.get(1)
    && let Some(expected_args) = core_nominal_schema_short_ref_arity(type_name)
    && items.len() != expected_args + 2
  {
    return Err(format!(
      "Core nominal type `{type_name}` expects {expected_args} type argument{}, got {}.",
      if expected_args == 1 { "" } else { "s" },
      items.len().saturating_sub(2)
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
  if matches!(annotation.as_ref(), CalcitTypeAnnotation::Dynamic) {
    return Err(format!(
      "Unsupported standalone type schema: {}",
      cirru_parser::format(std::slice::from_ref(schema), true.into()).unwrap_or_else(|_| format!("{schema:?}"))
    ));
  }
  Ok(())
}

/// A filled-in compact example for a type that takes type arguments.
fn parameterized_type_example(name: &str) -> &'static str {
  match name {
    "Map" => "Map Tag Bool",
    "Set" => "Set Bool",
    "Fn" => "Fn (-> Unit)",
    "Ref" => "Ref Bool",
    _ => "List Bool",
  }
}

/// Strict validation for schemas submitted via `calcit edit schema`.
/// Runs on the canonical form that compact input desugars to, so its messages
/// suggest the compact spelling.
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
          "Bare `{canonical}` leaves its nested type dynamic. Use an explicit type such as `{}`; write `Dynamic` as a nested type only when the boundary is intentionally dynamic.",
          parameterized_type_example(canonical)
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
        let example = CalcitTypeAnnotation::canonical_type_symbol_name(tag_name).map_or("List Bool", parameterized_type_example);
        return Err(format!(
          "Bare `:{tag_name}` leaves its nested type dynamic. Use an explicit type such as `{example}`; write `Dynamic` as a nested type only when the boundary is intentionally dynamic."
        ));
      }
      if PRIMITIVE_SCHEMA_TAGS.contains(&tag_name) {
        return Ok(());
      }
      return Err(format!(
        "Unknown value schema `{s}`. Use a type name such as `String`, a fully qualified nominal type such as `app.schema/Store`, a type application such as `Ref Bool`, or a function type such as `Fn (-> Unit)`; a Snapshot stores them as `quote String` or `quote $ Fn (-> Unit)`."
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
      return Err(
        "Retired wrapped schema `:: 'Fn $ {} …` expects exactly 3 items; write the compact `Fn Number (-> String)` instead".to_owned(),
      );
    }
    match (&raw_items[1], &raw_items[2]) {
      (tag, Cirru::List(inner_items)) if canonical_schema_symbol_from_cirru(tag).is_some_and(|name| matches!(name, "Fn" | "Macro")) => {
        inner_items
      }
      (Cirru::Leaf(tag), _) => {
        return Err(format!(
          "Wrapped schema type must be `Fn` or `Macro`, got: `{tag}`. Write the compact form such as `Fn String (-> Bool)`"
        ));
      }
      _ => {
        return Err(
          "Retired wrapped schema expects `Fn` or `Macro` followed by a `{}` map; write the compact form such as `Fn String (-> Bool)`"
            .to_owned(),
        );
      }
    }
  } else if matches!(raw_items.first(), Some(Cirru::Leaf(head)) if head.as_ref() == "{}") {
    return Err(
      "Legacy unwrapped callable schema maps are not accepted by `calcit edit schema`. Write a function type such as `Fn String (-> Bool)`, or a macro contract such as `Macro (Expr Dynamic) (-> (Expr Dynamic))`."
        .to_owned(),
    );
  } else {
    return validate_standalone_type_schema(schema);
  };

  for pair in items.iter().skip(1) {
    if matches!(pair, Cirru::List(xs) if matches!(xs.first(), Some(Cirru::Leaf(key)) if key.as_ref() == ":kind")) {
      return Err(
        "Wrapped callable schemas must not repeat `:kind`. Write the compact `Fn …` or `Macro …` form, which carries the kind in its head."
          .to_owned(),
      );
    }
  }

  let Some(Cirru::Leaf(head)) = items.first() else {
    return Err("Schema must be a non-empty list starting with `{}`".to_owned());
  };

  if head.as_ref() != "{}" {
    return Err(format!(
      "Schema must be a type such as `String`, `List Number` or `Fn String (-> Bool)`, got a list headed by `{head}`."
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
  let mut async_node: Option<&Cirru> = None;

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
        ":async" => async_node = Some(val),
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

  if let Some(async_value) = async_node
    && !matches!(async_value, Cirru::Leaf(value) if matches!(value.as_ref(), "true" | "false"))
  {
    return Err("`:async` must be `true` or `false`".to_owned());
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

/// Reject schema input written in the canonical `:: 'Fn $ {} …` form when a
/// compact `quote $ Fn …` spelling exists, pointing at that spelling.
pub fn reject_legacy_schema_input(schema: &Cirru) -> Result<(), String> {
  if crate::calcit::type_expr::is_compact_payload(schema) {
    return Ok(());
  }
  let Cirru::List(items) = schema else { return Ok(()) };
  if !matches!(items.first(), Some(Cirru::Leaf(head)) if matches!(head.as_ref(), "::" | "{}")) {
    return Ok(());
  }
  let Ok(annotation) = parse_schema_annotation_for_write(schema) else {
    return Ok(());
  };
  let suggestion = match schema_annotation_to_storage_edn(annotation.as_ref()) {
    Edn::Quote(Cirru::Leaf(name)) => format!("quote {name}"),
    Edn::Quote(compact) => format!("quote $ {}", render_cirru_inline(&compact)),
    _ => return Ok(()),
  };
  Err(format!(
    "[{LEGACY_TYPE_SYNTAX_CODE}] the `::`/`{{}}` schema syntax `{}` is retired; write `{suggestion}` instead.",
    render_cirru_inline(schema)
  ))
}

/// Validate and parse one schema submitted through `calcit edit schema`.
/// Leaf schemas cannot be round-tripped through `parse_schema_data`, whose
/// Cirru formatter requires a top-level expression, so they are converted
/// directly into EDN before type parsing.
pub fn parse_schema_annotation_for_write(schema: &Cirru) -> Result<Arc<CalcitTypeAnnotation>, String> {
  // Compact `Fn Number (-> String)` input is surface syntax; storage keeps the canonical form.
  if crate::calcit::type_expr::is_compact_payload(schema) {
    let canonical = crate::calcit::type_expr::desugar_type_expr(schema).map_err(|e| format!("invalid compact type expression: {e}"))?;
    return parse_schema_annotation_for_write(&canonical);
  }
  // A bare compact name such as `app.main/Person` (from `quote app.main/Person`).
  if let Cirru::Leaf(name) = schema
    && !name.starts_with(['\'', ':', '|'])
    && let Ok(quoted @ Cirru::Leaf(_)) = crate::calcit::type_expr::desugar_type_expr(schema)
    && quoted != *schema
  {
    return parse_schema_annotation_for_write(&quoted);
  }
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
  if declares_strict_macro_contract(&schema_edn) {
    return Err(
      "invalid strict `Macro` contract: :required/:optional/:rest must list syntax contracts such as 'Syntax or 'Expr, :expansion must be a contract such as `:: 'Expr 'Dynamic`, and :capabilities must be a set of known capability tags"
        .to_owned(),
    );
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

/// A `Macro` wrapper (or `:kind :macro` map) that carries strict contract keys
/// is a strict macro declaration; when it fails strict parsing it must not fall
/// through to an ordinary type, because the loader would then reject the entry.
fn declares_strict_macro_contract(schema: &Edn) -> bool {
  let map = match schema {
    Edn::Enum(view) if matches!(view.variant.as_ref(), "macro" | "Macro") => match view.extra.first() {
      Some(Edn::Map(map)) => map,
      _ => return true,
    },
    Edn::Map(map) if matches!(map.tag_get("kind"), Some(Edn::Tag(tag)) if tag.ref_str() == "macro") => map,
    _ => return false,
  };
  ["required", "optional", "rest", "expansion", "capabilities"]
    .iter()
    .any(|key| map.tag_get(key).is_some())
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

/// Build the narrowest safe macro contract recoverable from source syntax.
/// Parameter shape is structural evidence, while semantic input categories and
/// expansion types are not, so callers get Syntax inputs, an `Expr<Dynamic>`
/// expansion, and no compile-time capabilities. Projects can refine that
/// conservative contract after the Snapshot becomes readable.
pub fn conservative_macro_schema(code: &Cirru, owner: &str) -> Result<Option<Arc<CalcitTypeAnnotation>>, String> {
  if !code_declares_macro(code) {
    return Ok(None);
  }
  let Cirru::List(items) = code else {
    unreachable!("code_declares_macro only accepts list forms")
  };
  let Some(Cirru::List(params)) = items.get(2) else {
    return Err(format!(
      "{owner}: cannot derive a strict `defmacro` schema: expected a parameter list"
    ));
  };
  let shape = ParamShape::from_tokens(params.iter().filter_map(|item| match item {
    Cirru::Leaf(token) if matches!(token.as_ref(), "[]" | ",") => None,
    Cirru::Leaf(token) if token.as_ref() == "?" => Some(ParamShapeToken::OptionalMark),
    Cirru::Leaf(token) if token.as_ref() == "&" => Some(ParamShapeToken::RestMark),
    _ => Some(ParamShapeToken::Binding),
  }));
  if !shape.errors.is_empty() {
    return Err(format!(
      "{owner}: cannot derive a strict `defmacro` schema: malformed parameter list: {}",
      shape.errors.join("; ")
    ));
  }
  Ok(Some(Arc::new(CalcitTypeAnnotation::Macro(Arc::new(MacroSignature {
    generics: Arc::new(vec![]),
    where_bounds: Arc::new(vec![]),
    required_inputs: Arc::new(vec![MacroSyntaxType::Syntax; shape.required]),
    optional_inputs: Arc::new(vec![MacroSyntaxType::Syntax; shape.optional]),
    rest_input: shape.has_rest.then_some(MacroSyntaxType::Syntax),
    expansion: MacroExpansionType::Expr(DYNAMIC_TYPE.clone()),
    capabilities: Arc::new(HashSet::new()),
    features: Arc::new(HashSet::new()),
  })))))
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
      "deftrait" | "defexternal" => Some("trait"),
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
  #[serde(default)]
  pub verification: VerificationConfig,
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

  let mut unknown_entries = Vec::new();
  if !require_mode {
    for (entry_key, entry_value) in &entries_map.0 {
      let entry_name: String = from_edn(entry_key.to_owned())
        .map_err(|e| format!("entries: failed to parse entry name: {e}; got {}", format_edn_preview(entry_key)))?;
      let owner = format!("entries.{entry_name}");
      let entry_map = entry_value
        .view_map()
        .map_err(|e| format!("{owner}: expected a map: {e}; got {}", format_edn_preview(entry_value)))?;
      let unknown = unknown_format_config_keys(&entry_map, &owner, SNAPSHOT_ENTRY_FIELDS)?;
      if !unknown.is_empty() {
        unknown_entries.push((entry_name, unknown));
      }
    }
    if !unknown_entries.is_empty() {
      unknown_entries.sort();
      let diagnostics = unknown_entries
        .iter()
        .map(|(entry, fields)| format!("entries.{entry}: unknown field {}", format_unknown_config_fields(fields)))
        .collect::<Vec<_>>()
        .join("; ");
      let flags = unknown_entries
        .iter()
        .flat_map(|(entry, fields)| fields.iter().map(move |field| format!("--drop-legacy-entry-field {entry}.{field}")))
        .collect::<Vec<_>>()
        .join(" ");
      return Err(format!(
        "{diagnostics}; review the values, then rerun `calcit <snapshot> edit format {flags}`"
      ));
    }
  }

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

fn parse_verification_name(value: &Edn, owner: &str) -> Result<String, String> {
  let name = match value {
    Edn::Tag(tag) => tag.ref_str().to_owned(),
    Edn::Str(text) | Edn::Symbol(text) => text.trim_start_matches(':').to_owned(),
    _ => {
      return Err(format!(
        "{owner}: expected a tag, string, or symbol; got {}",
        format_edn_preview(value)
      ));
    }
  };
  if name.is_empty() {
    Err(format!("{owner}: name cannot be empty"))
  } else {
    Ok(name)
  }
}

fn reject_unknown_verification_fields(map: &EdnMapView, owner: &str, allowed: &[&str]) -> Result<(), String> {
  for key in map.0.keys() {
    let name = parse_verification_name(key, owner)?;
    if !allowed.contains(&name.as_str()) {
      return Err(format!("{owner}: unknown field `:{name}`"));
    }
  }
  Ok(())
}

fn parse_verification(data: &Edn, entries: &HashMap<String, SnapshotEntry>) -> Result<VerificationConfig, String> {
  if matches!(data, Edn::Nil) {
    return Ok(VerificationConfig::default());
  }
  let map = data
    .view_map()
    .map_err(|e| format!("verification: expected a map: {e}; got {}", format_edn_preview(data)))?;
  reject_unknown_verification_fields(
    &map,
    "verification",
    &["schema-version", "host-requirements", "external-gates", "profiles"],
  )?;
  let raw_version = map
    .get(&Edn::tag("schema-version"))
    .ok_or_else(|| "verification: missing `:schema-version` field".to_owned())?;
  let schema_version = match raw_version {
    Edn::Number(value) if *value == VERIFICATION_SCHEMA_VERSION as f64 => VERIFICATION_SCHEMA_VERSION,
    Edn::Number(value) => {
      return Err(format!(
        "verification.schema-version: unsupported version {value}; expected {VERIFICATION_SCHEMA_VERSION}"
      ));
    }
    _ => {
      return Err(format!(
        "verification.schema-version: expected number {VERIFICATION_SCHEMA_VERSION}; got {}",
        format_edn_preview(raw_version)
      ));
    }
  };
  let raw_profiles = map
    .get(&Edn::tag("profiles"))
    .ok_or_else(|| "verification: missing `:profiles` field".to_owned())?;
  let profiles_map = raw_profiles.view_map().map_err(|e| {
    format!(
      "verification.profiles: expected a map: {e}; got {}",
      format_edn_preview(raw_profiles)
    )
  })?;
  let mut host_requirements = HashMap::new();
  if let Some(raw_requirements) = map.get(&Edn::tag("host-requirements")) {
    let requirements = raw_requirements.view_map().map_err(|e| {
      format!(
        "verification.host-requirements: expected a map: {e}; got {}",
        format_edn_preview(raw_requirements)
      )
    })?;
    for (raw_tool, raw_requirement) in &requirements.0 {
      let tool = parse_verification_name(raw_tool, "verification.host-requirements")?;
      if !matches!(tool.as_str(), "node" | "yarn" | "rustc" | "caps") {
        return Err(format!(
          "verification.host-requirements: unsupported tool `{tool}`; expected node, yarn, rustc, or caps"
        ));
      }
      let requirement = match raw_requirement {
        Edn::Str(value) | Edn::Symbol(value) => value.to_string(),
        _ => {
          return Err(format!(
            "verification.host-requirements.{tool}: expected a version requirement string; got {}",
            format_edn_preview(raw_requirement)
          ));
        }
      };
      semver::VersionReq::parse(&requirement).map_err(|error| {
        format!("verification.host-requirements.{tool}: invalid semantic version requirement `{requirement}`: {error}")
      })?;
      if host_requirements.insert(tool.clone(), requirement).is_some() {
        return Err(format!("verification.host-requirements: duplicate tool `{tool}`"));
      }
    }
  }
  let mut external_gates = Vec::new();
  if let Some(raw_gates) = map.get(&Edn::tag("external-gates")) {
    let gates = raw_gates.view_list().map_err(|e| {
      format!(
        "verification.external-gates: expected a list: {e}; got {}",
        format_edn_preview(raw_gates)
      )
    })?;
    for raw_gate in &gates.0 {
      let gate = match raw_gate {
        Edn::Str(value) | Edn::Symbol(value) => value.to_string(),
        _ => {
          return Err(format!(
            "verification.external-gates: expected strings; got {}",
            format_edn_preview(raw_gate)
          ));
        }
      };
      if gate.trim().is_empty() {
        return Err("verification.external-gates: gate description cannot be empty".to_owned());
      }
      external_gates.push(gate);
    }
  }
  let mut profiles = HashMap::with_capacity(profiles_map.0.len());
  for (raw_name, raw_profile) in &profiles_map.0 {
    let name = parse_verification_name(raw_name, "verification.profiles")?;
    let owner = format!("verification.profiles.{name}");
    let profile_map = raw_profile
      .view_map()
      .map_err(|e| format!("{owner}: expected a map: {e}; got {}", format_edn_preview(raw_profile)))?;
    reject_unknown_verification_fields(&profile_map, &owner, &["entries", "checks", "on-failure"])?;
    let raw_entries = profile_map
      .get(&Edn::tag("entries"))
      .ok_or_else(|| format!("{owner}: missing `:entries` field"))?;
    let entry_list = raw_entries
      .view_list()
      .map_err(|e| format!("{owner}.entries: expected a list: {e}; got {}", format_edn_preview(raw_entries)))?;
    let mut selected_entries = Vec::with_capacity(entry_list.0.len());
    let mut seen_entries = HashSet::new();
    for raw_entry in &entry_list.0 {
      let entry = parse_verification_name(raw_entry, &format!("{owner}.entries"))?;
      if !entries.contains_key(&entry) {
        return Err(format!("{owner}.entries: unknown entry `{entry}`"));
      }
      if !seen_entries.insert(entry.clone()) {
        return Err(format!("{owner}.entries: duplicate entry `{entry}`"));
      }
      selected_entries.push(entry);
    }
    if selected_entries.is_empty() {
      return Err(format!("{owner}.entries: at least one entry is required"));
    }

    let raw_checks = profile_map
      .get(&Edn::tag("checks"))
      .ok_or_else(|| format!("{owner}: missing `:checks` field"))?;
    let check_list = raw_checks
      .view_list()
      .map_err(|e| format!("{owner}.checks: expected a list: {e}; got {}", format_edn_preview(raw_checks)))?;
    let mut checks = Vec::with_capacity(check_list.0.len());
    let mut seen_checks = HashSet::new();
    for raw_check in &check_list.0 {
      let check_name = parse_verification_name(raw_check, &format!("{owner}.checks"))?;
      let check = match check_name.as_str() {
        "strict" => VerificationCheckKind::Strict,
        "quality" => {
          return Err(format!(
            "{owner}.checks: `quality` was removed; replace it with `strict` and behavior tests; use analyze quality only for an existing migration baseline"
          ));
        }
        "dynamic-methods" => {
          return Err(format!(
            "{owner}.checks: `dynamic-methods` was removed; replace it with `strict` and behavior tests; use `analyze weak-types --only dynamic-method` for a read-only migration report"
          ));
        }
        _ => return Err(format!("{owner}.checks: unknown check `{check_name}`; expected strict")),
      };
      if !seen_checks.insert(check) {
        return Err(format!("{owner}.checks: duplicate check `{check_name}`"));
      }
      checks.push(check);
    }
    if checks.is_empty() {
      return Err(format!("{owner}.checks: at least one check is required"));
    }

    let on_failure = match profile_map.get(&Edn::tag("on-failure")) {
      None => VerificationFailurePolicy::Stop,
      Some(raw_policy) => match parse_verification_name(raw_policy, &format!("{owner}.on-failure"))?.as_str() {
        "stop" => VerificationFailurePolicy::Stop,
        "continue" => VerificationFailurePolicy::Continue,
        policy => return Err(format!("{owner}.on-failure: unknown policy `{policy}`; expected stop or continue")),
      },
    };
    if profiles
      .insert(
        name.clone(),
        VerificationProfile {
          entries: selected_entries,
          checks,
          on_failure,
        },
      )
      .is_some()
    {
      return Err(format!("verification.profiles: duplicate profile `{name}`"));
    }
  }

  Ok(VerificationConfig {
    schema_version,
    host_requirements,
    external_gates,
    profiles,
  })
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
  load_snapshot_data_inner(data, path, true).map_err(|error| {
    let mut message = format!("Failed to load Snapshot `{path}`: {error}");
    if let Some(hint) = legacy_snapshot_recovery_hint(path) {
      message.push_str("\nLegacy Snapshot recovery: ");
      message.push_str(&hint);
    }
    message
  })
}

/// Inspect canonical source for an explicit package/schema repair, not execution.
/// Existing macro contracts may need repair, but the selected schema must be
/// valid after its edit. Structural parsing and entry validation remain strict.
pub fn load_snapshot_data_for_metadata(data: &Edn, path: &str, edited_schema: Option<(&str, &str)>) -> Result<Snapshot, String> {
  if let Some(error) = retired_snapshot_migration_error(Path::new(path)) {
    return Err(error);
  }
  let snapshot = load_snapshot_data_inner(data, path, false)?;
  if let Some((namespace, definition)) = edited_schema {
    let entry = snapshot
      .files
      .get(namespace)
      .and_then(|file| file.defs.get(definition))
      .ok_or_else(|| format!("Definition '{namespace}/{definition}' not found"))?;
    validate_strict_macro_schema(entry, namespace, definition, path)?;
  }
  Ok(snapshot)
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

  // Direct-quote macros receive a conservative strict contract while they are
  // decoded. Existing modern entries remain subject to the ordinary strict
  // loader policy even when the same Snapshot also contains legacy quotes.
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

  let legacy_configs = data.get(&Edn::tag("configs")).or_else(|| data.get(&Edn::str("configs")));
  let entries_value = data.get_or_nil("entries");
  let mut entries = if matches!(entries_value, Edn::Nil) {
    HashMap::new()
  } else {
    // Historical named entries shared the legacy configs' native default.
    // Only the isolated format loader restores it; modern entries stay strict.
    parse_entries_with_context(&entries_value, legacy_configs.is_none())?
  };
  let mut legacy_version = None;
  if let Some(configs) = legacy_configs {
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
  let verification = parse_verification(&data.get_or_nil("verification"), &entries)?;

  Ok((
    Snapshot {
      package: pkg.to_string(),
      about,
      version,
      entries,
      verification,
      files,
      active_entry: default_active_entry(),
    },
    migration,
  ))
}

/// Fields a Snapshot entry may declare.
const SNAPSHOT_ENTRY_FIELDS: &[&str] = &[
  "mode",
  "init-fn",
  "reload-fn",
  "description",
  "modules",
  "type-slots",
  "feature-policy",
  "target",
];

/// Remove explicitly named unknown legacy fields (`<entry>.<field>`) from `:entries`
/// before the isolated format loader runs. Each target must exist and must not be a
/// current entry field, so nothing is dropped implicitly. Returns each removed field
/// with a preview of its value for the migration report.
pub fn drop_legacy_entry_fields(data: &mut Edn, targets: &[String]) -> Result<Vec<(String, String)>, String> {
  let mut dropped = Vec::with_capacity(targets.len());
  if targets.is_empty() {
    return Ok(dropped);
  }
  let Edn::Map(root) = data else {
    return Err("Snapshot root must be a map".to_owned());
  };
  let entries = [Edn::tag("entries"), Edn::str("entries")]
    .into_iter()
    .find(|key| root.0.contains_key(key))
    .and_then(|key| root.0.get_mut(&key))
    .ok_or_else(|| "Snapshot has no `:entries` map to migrate".to_owned())?;
  let Edn::Map(entries) = entries else {
    return Err("Snapshot `:entries` must be a map".to_owned());
  };
  for target in targets {
    let (entry_name, field) = target
      .rsplit_once('.')
      .filter(|(entry, field)| !entry.is_empty() && !field.trim_start_matches(':').is_empty())
      .ok_or_else(|| {
        format!("`--drop-legacy-entry-field {target}` must be `<entry>.<field>` (split at the last dot; neither part may be empty)")
      })?;
    let field = field.trim_start_matches(':');
    if SNAPSHOT_ENTRY_FIELDS.contains(&field) {
      return Err(format!(
        "`--drop-legacy-entry-field {target}`: `:{field}` is a current entry field; edit it with `calcit config` instead"
      ));
    }
    let entry_key = [Edn::str(entry_name), Edn::tag(entry_name)]
      .into_iter()
      .find(|key| entries.0.contains_key(key))
      .ok_or_else(|| format!("`--drop-legacy-entry-field {target}`: entry `{entry_name}` does not exist"))?;
    let Some(Edn::Map(entry)) = entries.0.get_mut(&entry_key) else {
      return Err(format!("`--drop-legacy-entry-field {target}`: entry `{entry_name}` is not a map"));
    };
    let mut keys = entry
      .0
      .keys()
      .filter(|key| format_config_key_name(key) == Some(field))
      .cloned()
      .collect::<Vec<_>>();
    if keys.is_empty() {
      return Err(format!(
        "`--drop-legacy-entry-field {target}`: entry `{entry_name}` has no `:{field}` field"
      ));
    }
    // All aliases denote the explicitly reviewed field; report every removed value.
    keys.sort_by_key(format_edn_preview);
    for key in keys {
      let value = entry.0.remove(&key).expect("collected field key exists");
      dropped.push((format!("{entry_name}.{field}"), format_edn_preview(&value)));
    }
  }
  Ok(dropped)
}

/// Normalize field names identically for format validation and explicit removal.
fn format_config_key_name(key: &Edn) -> Option<&str> {
  match key {
    Edn::Tag(tag) => Some(tag.ref_str()),
    Edn::Str(text) | Edn::Symbol(text) => Some(text.trim_start_matches(':')),
    _ => None,
  }
}

/// Collect unknown fields without stopping at the first named entry.
fn unknown_format_config_keys(data: &EdnMapView, owner: &str, allowed: &[&str]) -> Result<Vec<String>, String> {
  let mut unknown = vec![];
  for key in data.0.keys() {
    let name = format_config_key_name(key).ok_or_else(|| {
      format!(
        "{owner}: field name must be a tag, string, or symbol; got {}",
        format_edn_preview(key)
      )
    })?;
    if !allowed.contains(&name) {
      unknown.push(name.to_owned());
    }
  }
  unknown.sort();
  unknown.dedup();
  Ok(unknown)
}

/// Render normalized field names in deterministic diagnostics.
fn format_unknown_config_fields(fields: &[String]) -> String {
  fields.iter().map(|name| format!("`:{name}`")).collect::<Vec<_>>().join(", ")
}

fn validate_format_config_keys(data: &EdnMapView, owner: &str, allowed: &[&str]) -> Result<(), String> {
  let unknown = unknown_format_config_keys(data, owner, allowed)?;
  if unknown.is_empty() {
    Ok(())
  } else {
    Err(format!(
      "{owner}: unknown field {}; migrate it explicitly before formatting",
      format_unknown_config_fields(&unknown)
    ))
  }
}

fn parse_legacy_configs_for_format(data: &Edn) -> Result<(SnapshotEntry, Option<String>), String> {
  let configs = data
    .view_map()
    .map_err(|e| format!("legacy configs: expected a map: {e}; got {}", format_edn_preview(data)))?;
  validate_format_config_keys(&configs, "legacy configs", &["init-fn", "reload-fn", "modules", "version", "mode"])?;
  let entry = parse_snapshot_entry_with_context(data.to_owned(), "legacy configs", false)?;
  let version = match configs.get(&Edn::tag("version")).or_else(|| configs.get(&Edn::str("version"))) {
    Some(Edn::Nil) => None,
    Some(_) => Some(parse_snapshot_config_string_field(&configs, "version", "legacy configs")?),
    None => None,
  };
  Ok((entry, version))
}

/// Error code for type syntax that `calcit edit format` rewrites into the compact quote form.
pub const LEGACY_TYPE_SYNTAX_WARNING: &str = "W_LEGACY_TYPE_SYNTAX";
pub const LEGACY_TYPE_SYNTAX_CODE: &str = "E_LEGACY_TYPE_SYNTAX";

/// One-line Cirru for diagnostics: nested lists in parentheses, a trailing
/// non-clause list after `$`.
fn render_cirru_inline(node: &Cirru) -> String {
  fn item(node: &Cirru) -> String {
    match node {
      Cirru::Leaf(s) => s.to_string(),
      Cirru::List(xs) => format!("({})", line(xs)),
    }
  }
  fn line(xs: &[Cirru]) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(xs.len());
    for (index, x) in xs.iter().enumerate() {
      match x {
        Cirru::List(ys)
          if index + 1 == xs.len()
            && index > 0
            && !ys.is_empty()
            && !matches!(&ys[0], Cirru::Leaf(head) if head.as_ref() == "->" || head.starts_with(':')) =>
        {
          parts.push(format!("$ {}", line(ys)))
        }
        _ => parts.push(item(x)),
      }
    }
    parts.join(" ")
  }
  match node {
    Cirru::Leaf(s) => s.to_string(),
    Cirru::List(xs) => line(xs),
  }
}

/// The first subtree where `before` and `after` differ, as (written, migrated).
fn first_rewritten_form<'a>(before: &'a Cirru, after: &'a Cirru) -> Option<(&'a Cirru, &'a Cirru)> {
  match (before, after) {
    (Cirru::List(xs), Cirru::List(ys)) if xs.len() == ys.len() => xs.iter().zip(ys).find_map(|(x, y)| first_rewritten_form(x, y)),
    _ if before == after => None,
    _ => Some((before, after)),
  }
}

fn legacy_type_syntax_error(owner: &str, path: &str, written: &Cirru, migrated: &Cirru) -> String {
  format!(
    "[{LEGACY_TYPE_SYNTAX_CODE}] {owner} uses the retired `::`/`{{}}` type syntax `{}`; write `{}` instead, or run `calcit {path} edit format` to migrate the Snapshot.",
    render_cirru_inline(written),
    render_cirru_inline(migrated)
  )
}

/// Reject canonical type forms in code type positions that have a compact
/// spelling. Value references and partial hints have none and stay accepted.
pub fn check_code_type_syntax(code: &Cirru, owner: &str, path: &str) -> Result<(), String> {
  let (migrated, count) = migrate_code_type_syntax(code);
  if count == 0 {
    return Ok(());
  }
  match first_rewritten_form(code, &migrated) {
    Some((written, next)) => Err(legacy_type_syntax_error(owner, path, written, next)),
    None => Ok(()),
  }
}

/// Reject `:schema` values stored in the canonical EDN form when the compact
/// `quote $ …` form expresses the same type.
fn check_raw_schema_syntax(files: &Edn, path: &str) -> Result<(), String> {
  let Ok(files) = files.view_map() else { return Ok(()) };
  for (file_key, file) in files.0.iter() {
    let Some(Edn::Map(defs)) = ffi_metadata_value(file, "defs") else {
      continue;
    };
    for (def_key, entry) in defs.0.iter() {
      let Some(schema) = ffi_metadata_value(entry, "schema") else {
        continue;
      };
      if matches!(schema, Edn::Nil | Edn::Quote(_)) {
        continue;
      }
      let owner = format!("{}/{} :schema", edn_key_text(file_key), edn_key_text(def_key));
      let Ok(annotation) = parse_loaded_schema_annotation(schema, &owner) else {
        continue;
      };
      if let Edn::Quote(compact) = schema_annotation_to_storage_edn(annotation.as_ref()) {
        let compact = match compact {
          Cirru::Leaf(name) => format!("quote {name}"),
          list => format!("quote $ {}", render_cirru_inline(&list)),
        };
        let written = parse_schema_cirru_from_edn(schema)
          .map(|node| render_cirru_inline(&node))
          .unwrap_or_else(|_| schema.to_string());
        return Err(format!(
          "[{LEGACY_TYPE_SYNTAX_CODE}] {owner} uses the retired EDN type syntax `{written}`; store it as `{compact}`, or run `calcit {path} edit format` to migrate the Snapshot."
        ));
      }
    }
  }
  Ok(())
}

fn edn_key_text(key: &Edn) -> String {
  match key {
    Edn::Symbol(name) | Edn::Str(name) => name.to_string(),
    Edn::Tag(tag) => tag.ref_str().to_owned(),
    other => other.to_string(),
  }
}

fn check_snapshot_code_type_syntax(files: &HashMap<String, FileInSnapShot>, path: &str) -> Result<(), String> {
  let mut names: Vec<&String> = files.keys().collect();
  names.sort();
  for ns in names {
    let file = &files[ns];
    let mut defs: Vec<&String> = file.defs.keys().collect();
    defs.sort();
    for def in defs {
      let entry = &file.defs[def];
      let owner = format!("{ns}/{def}");
      check_code_type_syntax(&entry.code, &owner, path)?;
      for example in &entry.examples {
        check_code_type_syntax(example, &format!("{owner} example"), path)?;
      }
      for test in &entry.tests {
        check_code_type_syntax(&test.code, &format!("{owner} test `{}`", test.name), path)?;
      }
    }
  }
  Ok(())
}

fn load_snapshot_data_inner(data: &Edn, path: &str, strict_macros: bool) -> Result<Snapshot, String> {
  let data = data.view_map()?;
  if data.contains_key("configs") {
    return Err(retired_snapshot_configs_error(path));
  }
  let pkg: Arc<str> = data.get_or_nil("package").try_into()?;
  check_raw_schema_syntax(&data.get_or_nil("files"), path)?;
  let mut files: HashMap<String, FileInSnapShot> = parse_files_with_context(&data.get_or_nil("files"))?;
  check_snapshot_code_type_syntax(&files, path)?;
  if strict_macros {
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
  let entries = parse_entries_with_context(&data.get_or_nil("entries"), true)?;
  let verification = parse_verification(&data.get_or_nil("verification"), &entries)?;
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
    verification,
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
      validate_strict_macro_schema(&file.defs[def_name], ns, def_name, path)?;
    }
  }
  Ok(())
}

fn validate_strict_macro_schema(entry: &CodeEntry, ns: &str, def_name: &str, path: &str) -> Result<(), String> {
  if !code_declares_macro(&entry.code) || matches!(entry.schema.as_ref(), CalcitTypeAnnotation::Macro(_)) {
    return Ok(());
  }
  let found = match entry.schema.as_ref() {
    CalcitTypeAnnotation::Fn(_) => "a runtime Fn schema",
    CalcitTypeAnnotation::Dynamic => "a Dynamic schema",
    _ => "a non-Macro schema",
  };
  Err(format!(
    "legacy macro schema at snapshot.files[{ns:?}].defs[{def_name:?}].schema: `defmacro` requires a strict `Macro` contract with :required/:optional/:rest, :expansion, and :capabilities, but found {found}. Migrate this definition with the final compatible Calcit 0.13.51 release, then retry `calcit '{path}' --check-only`."
  ))
}

/// Expand a `defexternal` authoring shorthand into the canonical `deftrait` code
/// plus external-object `:ffi` metadata. This keeps preprocess/type/codegen/query
/// unchanged: `defexternal` is only a declaration surface.
fn normalize_defexternal_code(code: Cirru, owner: &str) -> Result<(Cirru, Edn), String> {
  let Cirru::List(items) = &code else {
    return Err(format!("{owner}: expected `defexternal` form to be a list"));
  };
  if items.len() < 2 {
    return Err(format!("{owner}: `defexternal` requires a trait name and members"));
  }
  let name = match items.get(1) {
    Some(Cirru::Leaf(name)) if !name.is_empty() => name.clone(),
    other => {
      return Err(format!(
        "{owner}: `defexternal` expects a trait name symbol, got {}",
        format_cirru_preview(other)
      ));
    }
  };

  let mut target: Option<Edn> = None;
  let mut names_pairs: Vec<(Edn, Edn)> = vec![];
  let mut writable: Vec<Edn> = vec![];
  let mut members: Vec<Cirru> = vec![];

  for item in &items[2..] {
    let Cirru::List(parts) = item else {
      return Err(format!(
        "{owner}: `defexternal` expects option/member entries, got {}",
        format_cirru_preview(Some(item))
      ));
    };
    let Some(Cirru::Leaf(head)) = parts.first() else {
      return Err(format!(
        "{owner}: `defexternal` expects option/member heads, got {}",
        format_cirru_preview(Some(item))
      ));
    };
    // Each outer option or member is a head plus exactly one value. Plural
    // contents stay inside that single value (`:names` map / `:writable` set).
    if parts.len() != 2 {
      return Err(format!(
        "{owner}: `defexternal` entry `{head}` expects exactly one value, got {} part(s)",
        parts.len()
      ));
    }
    match head.as_ref() {
      ":target" => {
        if target.is_some() {
          return Err(format!("{owner}: `defexternal` declares `:target` more than once"));
        }
        let value = parts
          .get(1)
          .ok_or_else(|| format!("{owner}: `defexternal` `:target` is missing a value"))?;
        target = Some(parse_defexternal_target(value, owner)?);
      }
      ":names" => {
        let value = parts
          .get(1)
          .ok_or_else(|| format!("{owner}: `defexternal` `:names` is missing a map"))?;
        names_pairs.extend(parse_defexternal_names(value, owner)?);
      }
      ":writable" => {
        let value = parts
          .get(1)
          .ok_or_else(|| format!("{owner}: `defexternal` `:writable` is missing a set"))?;
        writable.extend(parse_defexternal_writable(value, owner)?);
      }
      "requires" | "'requires" => {
        members.push(Cirru::List(vec![parts[0].clone(), parts[1].clone()]));
      }
      ":backend" | ":kind" => {
        return Err(format!(
          "{owner}: `defexternal` does not accept `{head}`; it always produces `:backend :js` and `:kind :external-object`"
        ));
      }
      _ if head.starts_with(':') || head.starts_with('.') => {
        let value = parts
          .get(1)
          .ok_or_else(|| format!("{owner}: `defexternal` member `{head}` is missing a schema/value"))?;
        members.push(Cirru::List(vec![parts[0].clone(), value.clone()]));
      }
      other => {
        return Err(format!(
          "{owner}: `defexternal` expects field (`:field Type`) or method (`.method Schema`) members, got `{other}`"
        ));
      }
    }
  }

  if members.is_empty() {
    return Err(format!("{owner}: `defexternal` requires at least one field or method member"));
  }

  let mut trait_items: Vec<Cirru> = vec![Cirru::Leaf(Arc::from("deftrait")), Cirru::Leaf(name.clone())];
  trait_items.extend(members.iter().cloned());
  let trait_code = Cirru::List(trait_items);

  let mut ffi_pairs: Vec<(Edn, Edn)> = vec![
    (Edn::tag("backend"), Edn::tag("js")),
    (Edn::tag("kind"), Edn::tag("external-object")),
  ];
  if let Some(target) = target {
    ffi_pairs.push((Edn::tag("target"), target));
  }
  if !names_pairs.is_empty() {
    ffi_pairs.push((Edn::tag("names"), Edn::Map(EdnMapView(HashMap::from_iter(names_pairs)))));
  }
  if !writable.is_empty() {
    #[allow(clippy::mutable_key_type)]
    let set: HashSet<Edn> = writable.into_iter().collect();
    ffi_pairs.push((Edn::tag("writable"), Edn::Set(EdnSetView(set))));
  }

  Ok((trait_code, Edn::Map(EdnMapView(HashMap::from_iter(ffi_pairs)))))
}

/// Parse a `defexternal` `:target` value into a canonical target tag.
fn parse_defexternal_target(value: &Cirru, owner: &str) -> Result<Edn, String> {
  let Cirru::Leaf(value) = value else {
    return Err(format!(
      "{owner}: `defexternal` `:target` expects a tag, got {}",
      format_cirru_preview(Some(value))
    ));
  };
  let name = value.trim_start_matches(':');
  match name {
    "browser" | "node" | "native" | "wasm" => Ok(Edn::tag(name)),
    _ => Err(format!(
      "{owner}: unknown `defexternal` `:target` `{name}`; expected browser, node, native, or wasm"
    )),
  }
}

/// Parse a `defexternal` `:names` map into Calcit-name/host-name tag/string pairs.
fn parse_defexternal_names(value: &Cirru, owner: &str) -> Result<Vec<(Edn, Edn)>, String> {
  let Cirru::List(items) = value else {
    return Err(format!(
      "{owner}: `defexternal` `:names` expects a map, got {}",
      format_cirru_preview(Some(value))
    ));
  };
  let Some(Cirru::Leaf(head)) = items.first() else {
    return Err(format!("{owner}: `defexternal` `:names` expects a `{{}}` map head"));
  };
  if head.as_ref() != "{}" {
    return Err(format!("{owner}: `defexternal` `:names` expects a map, got `{head}`"));
  }
  let pairs = &items[1..];
  let mut result = vec![];
  for pair in pairs {
    let Cirru::List(pair) = pair else {
      return Err(format!(
        "{owner}: `defexternal` `:names` expects `(key value)` pairs, got {}",
        format_cirru_preview(Some(pair))
      ));
    };
    if pair.len() != 2 {
      return Err(format!("{owner}: `defexternal` `:names` expects `(key value)` pairs"));
    }
    let key = match &pair[0] {
      Cirru::Leaf(key) if key.starts_with(':') => Edn::tag(key.trim_start_matches(':')),
      other => {
        return Err(format!(
          "{owner}: `defexternal` `:names` keys must be tags, got {}",
          format_cirru_preview(Some(other))
        ));
      }
    };
    let host = match &pair[1] {
      Cirru::Leaf(host) if host.starts_with('|') => Edn::str(host.trim_start_matches('|')),
      other => {
        return Err(format!(
          "{owner}: `defexternal` `:names` values must be strings, got {}",
          format_cirru_preview(Some(other))
        ));
      }
    };
    result.push((key, host));
  }
  Ok(result)
}

/// Parse a `defexternal` `:writable` set into field tags.
fn parse_defexternal_writable(value: &Cirru, owner: &str) -> Result<Vec<Edn>, String> {
  let Cirru::List(items) = value else {
    return Err(format!(
      "{owner}: `defexternal` `:writable` expects a set, got {}",
      format_cirru_preview(Some(value))
    ));
  };
  let Some(Cirru::Leaf(head)) = items.first() else {
    return Err(format!("{owner}: `defexternal` `:writable` expects a `#{{}}` set head"));
  };
  if head.as_ref() != "#{}" {
    return Err(format!("{owner}: `defexternal` `:writable` expects a set, got `{head}`"));
  }
  let mut result = vec![];
  for item in &items[1..] {
    match item {
      Cirru::Leaf(field) if field.starts_with(':') => result.push(Edn::tag(field.trim_start_matches(':'))),
      other => {
        return Err(format!(
          "{owner}: `defexternal` `:writable` entries must be field tags, got {}",
          format_cirru_preview(Some(other))
        ));
      }
    }
  }
  Ok(result)
}

/// Render a Cirru node for diagnostics, or `<missing>` when absent.
fn format_cirru_preview(node: Option<&Cirru>) -> String {
  match node {
    Some(node) => format!("{node:?}"),
    None => "<missing>".to_owned(),
  }
}

/// Whether a definition body uses the `defexternal` shorthand head.
pub fn code_declares_defexternal(code: &Cirru) -> bool {
  matches!(code, Cirru::List(items) if matches!(items.first(), Some(Cirru::Leaf(head)) if head.as_ref() == "defexternal"))
}

/// Apply the `defexternal` shorthand, if present, to a parsed entry. Explicit
/// `:ffi` metadata wins over the shorthand's generated metadata.
fn normalize_entry_external(code: Cirru, ffi: Option<Edn>, owner: &str) -> Result<(Cirru, Option<Edn>), String> {
  if !code_declares_defexternal(&code) {
    return Ok((code, ffi));
  }
  let (trait_code, generated_ffi) = normalize_defexternal_code(code, owner)?;
  if ffi.is_some() {
    return Err(format!(
      "{owner}: `defexternal` already declares external-object metadata; remove the explicit `:ffi` or use `deftrait` instead"
    ));
  }
  Ok((trait_code, Some(generated_ffi)))
}

/// Validate a `defexternal` shorthand body before it is written to a Snapshot,
/// so malformed declarations fail at the edit boundary instead of at the next
/// load. Non-shorthand code is left untouched.
pub fn validate_defexternal_shorthand(code: &Cirru, owner: &str) -> Result<(), String> {
  if code_declares_defexternal(code) {
    normalize_defexternal_code(code.clone(), owner)?;
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
        let mut entry = CodeEntry::from_code(code.clone());
        if let Some(schema) = conservative_macro_schema(code, &owner)? {
          entry.schema = schema;
        }
        entry
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
      verification: VerificationConfig::default(),
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
  "defref",
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

/// Synthetic snippet entry points are reviewed open boundaries: their
/// argument shape is fixed even though an ad-hoc expression may return any
/// value. Keeping that distinction explicit lets default strict diagnostics
/// validate `eval` bodies without rejecting the wrapper itself.
fn snippet_runtime_entry(code: Cirru) -> CodeEntry {
  let mut entry = CodeEntry::from_code(code);
  entry.schema = Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
    runtime_arity: None,
    generics: Arc::new(vec![]),
    where_bounds: Arc::new(vec![]),
    arg_types: vec![],
    return_type: DYNAMIC_TYPE.clone(),
    fn_kind: SchemaKind::Fn,
    rest_type: None,
    features: Arc::new(HashSet::new()),
  })));
  entry
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
        def_dict.insert("main!".into(), snippet_runtime_entry(Cirru::List(func_code)));
      }

      def_dict
        .entry("main!".to_string())
        .or_insert_with(|| snippet_runtime_entry(vec![Cirru::leaf("defn"), "main!".into(), Cirru::List(vec![])].into()));
      def_dict
        .entry("reload!".to_string())
        .or_insert_with(|| snippet_runtime_entry(vec![Cirru::leaf("defn"), "reload!".into(), Cirru::List(vec![])].into()));

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

/// One definition entry inside `added-defs` / `changed-defs` of an
/// incremental change file (`.compact-inc.cirru`).
///
/// Two payload shapes are accepted:
///
/// - `Entry`: a full `%{} CodeEntry` record (or `{} (:code ..) ..` map), parsed by
///   the same loader as the full Snapshot. Applying it replaces code *and*
///   every runtime metadata field (`:schema`, `:doc`, `:examples`, `:ffi`,
///   `:tags`), so the incremental state matches a cold start of the same entry.
///   An entry whose code is unchanged expresses a metadata-only change.
/// - `Code`: the legacy bare quoted code. It only carries code, so applying it
///   keeps the definition's current in-memory metadata for `changed-defs` and
///   starts an `added-defs` definition with empty metadata (Dynamic schema, no
///   doc/examples/ffi/tags). The watcher never writes the full Snapshot, so the
///   metadata stored there is never dropped or overwritten by either shape.
#[derive(Debug, PartialEq, Clone, Eq)]
pub enum DefinitionChange {
  Code(Cirru),
  Entry(CodeEntry),
}

impl DefinitionChange {
  pub fn code(&self) -> &Cirru {
    match self {
      DefinitionChange::Code(code) => code,
      DefinitionChange::Entry(entry) => &entry.code,
    }
  }

  pub fn is_code_only(&self) -> bool {
    matches!(self, DefinitionChange::Code(_))
  }
}

impl From<Cirru> for DefinitionChange {
  fn from(code: Cirru) -> Self {
    DefinitionChange::Code(code)
  }
}

impl From<CodeEntry> for DefinitionChange {
  fn from(entry: CodeEntry) -> Self {
    DefinitionChange::Entry(entry)
  }
}

impl From<&DefinitionChange> for Edn {
  fn from(data: &DefinitionChange) -> Edn {
    match data {
      DefinitionChange::Code(code) => Edn::Quote(code.to_owned()),
      DefinitionChange::Entry(entry) => Edn::from(entry),
    }
  }
}

fn parse_definition_change(value: Edn, owner: &str) -> Result<DefinitionChange, String> {
  match value {
    Edn::Quote(code) => Ok(DefinitionChange::Code(code)),
    value @ (Edn::Struct(_) | Edn::Map(_)) => parse_code_entry_with_context(value, owner).map(DefinitionChange::Entry),
    other => Err(format!(
      "{owner}: expected quoted code or a CodeEntry record, got: {}",
      format_edn_display(&other)
    )),
  }
}

fn parse_definition_changes(value: Edn, field: &str) -> Result<HashMap<String, DefinitionChange>, String> {
  match value {
    Edn::Nil => Ok(HashMap::new()),
    Edn::Map(map) => {
      let mut defs = HashMap::with_capacity(map.0.len());
      for (key, value) in map.0 {
        // Same identifier rules as `:defs` in a full Snapshot.
        let name = parse_snapshot_identifier_key(&key, field)?;
        let change = parse_definition_change(value, &format!("{field}/{name}"))?;
        insert_snapshot_identifier(&mut defs, name, change, field)?;
      }
      Ok(defs)
    }
    other => Err(format!("{field}: expected a map, got: {}", format_edn_display(&other))),
  }
}

/// Apply the cold-start strict `defmacro` schema check to every full
/// `CodeEntry` carried by an incremental change, so a definition the next cold
/// start would reject is not accepted by `cr watch` either. Legacy code-only
/// entries carry no schema and are not checked here.
pub fn validate_incremental_macro_schemas(changes: &ChangesDict) -> Result<(), String> {
  const PATH: &str = ".compact-inc.cirru";
  let mut added = changes.added.keys().collect::<Vec<_>>();
  added.sort_unstable();
  for ns in added {
    let file = &changes.added[ns];
    let mut defs = file.defs.keys().collect::<Vec<_>>();
    defs.sort_unstable();
    for def in defs {
      validate_strict_macro_schema(&file.defs[def], ns, def, PATH)?;
    }
  }
  let mut namespaces = changes.changed.keys().collect::<Vec<_>>();
  namespaces.sort_unstable();
  for ns in namespaces {
    let info = &changes.changed[ns];
    let mut defs = info.added_defs.iter().chain(info.changed_defs.iter()).collect::<Vec<_>>();
    defs.sort_unstable_by(|a, b| a.0.cmp(b.0));
    for (def, change) in defs {
      if let DefinitionChange::Entry(entry) = change {
        validate_strict_macro_schema(entry, ns, def, PATH)?;
      }
    }
  }
  Ok(())
}

fn definition_changes_to_edn(defs: &HashMap<String, DefinitionChange>) -> Edn {
  Edn::Map(EdnMapView(
    defs.iter().map(|(name, def)| (Edn::str(&**name), Edn::from(def))).collect(),
  ))
}

#[derive(Debug, PartialEq, Clone, Eq)]
pub struct FileChangeInfo {
  pub ns: Option<Cirru>,
  pub added_defs: HashMap<String, DefinitionChange>,
  pub removed_defs: HashSet<String>,
  pub changed_defs: HashMap<String, DefinitionChange>,
}

impl FileChangeInfo {
  /// Number of `added-defs` / `changed-defs` entries that only carry legacy bare code.
  pub fn code_only_def_count(&self) -> usize {
    self
      .added_defs
      .values()
      .chain(self.changed_defs.values())
      .filter(|change| change.is_code_only())
      .count()
  }
}

impl From<&FileChangeInfo> for Edn {
  fn from(data: &FileChangeInfo) -> Edn {
    let mut map = EdnMapView::default();
    if let Some(ns) = &data.ns {
      map.insert_key("ns", Edn::Quote(ns.to_owned()));
    }

    if !data.added_defs.is_empty() {
      map.insert_key("added-defs", definition_changes_to_edn(&data.added_defs));
    }
    if !data.removed_defs.is_empty() {
      map.insert_key(
        "removed-defs",
        Edn::Set(EdnSetView(data.removed_defs.iter().map(|s| Edn::str(&**s)).collect())),
      );
    }
    if !data.changed_defs.is_empty() {
      map.insert_key("changed-defs", definition_changes_to_edn(&data.changed_defs));
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
      added_defs: parse_definition_changes(data.get_or_nil("added-defs"), "added-defs")?,
      removed_defs: data.get_or_nil("removed-defs").try_into()?,
      changed_defs: parse_definition_changes(data.get_or_nil("changed-defs"), "changed-defs")?,
    })
  }
}

/// Incremental change dictionary read from `.compact-inc.cirru` by `cr watch`.
/// See [`DefinitionChange`] for the accepted per-definition payloads.
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
  // `quote $ Fn …` is already the compact source syntax; it has no legacy tags to rewrite.
  if let Cirru::List(items) = node
    && let [Cirru::Leaf(head), payload] = items.as_slice()
    && head.as_ref() == "quote"
    && crate::calcit::type_expr::is_compact_payload(payload)
  {
    return (node.clone(), 0);
  }
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

fn is_tag_leaf(node: Option<&Cirru>) -> bool {
  matches!(node, Some(Cirru::Leaf(leaf)) if leaf.starts_with(':'))
}

/// Canonicalize the type of a `(:field type default?)` entry, leaving the default as data.
fn canonicalize_struct_field_type(entry: &[Cirru]) -> (Cirru, usize) {
  let mut next = entry.to_vec();
  let mut changed = 0;
  if let Some(type_form) = entry.get(1) {
    let (type_form, count) = canonicalize_type_expression(type_form);
    next[1] = type_form;
    changed += count;
  }
  (Cirru::List(next), changed)
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
        let map_headed = matches!(field.first(), Some(Cirru::Leaf(brace)) if brace.as_ref() == "{}");
        for type_index in 1..field.len() {
          // A struct field's third item is a literal default, not a type; this
          // also holds for fields wrapped in a `{}` form.
          let (next, count) = match (&field[type_index], head) {
            (Cirru::List(entry), Some("defstruct")) if map_headed && is_tag_leaf(entry.first()) => {
              canonicalize_struct_field_type(entry)
            }
            (_, Some("defstruct")) if !map_headed && is_tag_leaf(field.first()) => {
              if type_index > 1 {
                continue;
              }
              canonicalize_type_expression(&field[type_index])
            }
            (item, _) => canonicalize_type_expression(item),
          };
          next_field[type_index] = next;
          changed += count;
        }
        rewritten[index] = Cirru::List(next_field);
      }
    }
    Some("deftrait") if items.len() >= 3 => {
      for index in 2..items.len() {
        let Cirru::List(entry) = &items[index] else { continue };
        let type_index = match entry.as_slice() {
          [Cirru::Leaf(bracket), _, _] if bracket.as_ref() == "[]" => 2,
          [_, _] => 1,
          _ => continue,
        };
        let (next, count) = canonicalize_type_expression(&entry[type_index]);
        if count > 0 {
          let mut next_entry = entry.clone();
          next_entry[type_index] = next;
          rewritten[index] = Cirru::List(next_entry);
          changed += count;
        }
      }
    }
    Some("hint-fn") => {
      for index in 1..items.len() {
        if is_bare_hint_fn_type(&items[index]) {
          continue;
        }
        let (next, count) = canonicalize_schema_map_types(&items[index]);
        rewritten[index] = next;
        changed += count;
      }
    }
    Some("fn" | "defn" | "defmacro" | "defcomp" | "defeffect") => {
      let args_index = if head == Some("fn") { 1 } else { 2 };
      let type_index = args_index + 1;
      // A lone form after the parameters is the body's value, not a return label.
      if items.len() > type_index + 1
        && let Some(type_form) = items.get(type_index)
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

/// Leaves of a canonical type form that the compact syntax can spell. A bare
/// leaf other than a built-in type name, such as `Person` in `:: 'List Person`,
/// is a value reference,
/// which only `assert-type` keeps in the canonical form.
fn canonical_type_form_is_spellable(node: &Cirru) -> bool {
  match node {
    Cirru::Leaf(text) => {
      matches!(text.as_ref(), "::" | "{}" | "[]" | "#{}" | "true")
        || text.starts_with(['\'', ':', '*'])
        || CalcitTypeAnnotation::canonical_type_symbol_name(text) == Some(text.as_ref())
    }
    Cirru::List(items) => items.iter().all(canonical_type_form_is_spellable),
  }
}

/// `quote $ <compact>` for a canonical `:: ...` type form when the compact
/// spelling desugars back to the same EDN.
fn compact_type_form(node: &Cirru) -> Option<Cirru> {
  let Cirru::List(items) = node else { return None };
  if !matches!(items.first(), Some(Cirru::Leaf(head)) if head.as_ref() == "::") || items.len() < 3 {
    return None;
  }
  if !canonical_type_form_is_spellable(node) {
    return None;
  }
  let edn = schema_cirru_to_edn(node.clone());
  let compact = crate::calcit::type_expr::print_type_compact(&edn).ok()?;
  let back = crate::calcit::type_expr::desugar_type_expr(&compact).ok()?;
  (schema_cirru_to_edn(back) == edn).then(|| Cirru::List(vec![Cirru::leaf("quote"), compact]))
}

/// `quote $ Fn ...` or `quote $ Async ...` for a `hint-fn` schema map.
fn compact_hint_fn_map(node: &Cirru) -> Option<Cirru> {
  let Cirru::List(items) = node else { return None };
  if !matches!(items.first(), Some(Cirru::Leaf(head)) if head.as_ref() == "{}") || !canonical_type_form_is_spellable(node) {
    return None;
  }
  let edn = schema_cirru_to_edn(node.clone());
  let Edn::Map(map) = &edn else { return None };
  let keys: Vec<&str> = map
    .0
    .keys()
    .filter_map(|key| match key {
      Edn::Tag(tag) => Some(tag.ref_str()),
      _ => None,
    })
    .collect();
  let compact = if keys.iter().all(|key| matches!(*key, "async" | "features")) {
    if map.0.get(&Edn::tag("async")) != Some(&Edn::Bool(true)) {
      return None;
    }
    let mut marker = vec![Cirru::leaf("Async")];
    if let Some(Edn::Set(features)) = map.0.get(&Edn::tag("features")) {
      let mut clause = vec![Cirru::leaf(":features")];
      let mut tags: Vec<_> = features.0.iter().map(|tag| Cirru::Leaf(tag.to_string().into())).collect();
      tags.sort();
      clause.extend(tags);
      marker.push(Cirru::List(clause));
    }
    if marker.len() == 1 {
      Cirru::leaf("Async")
    } else {
      Cirru::List(marker)
    }
  } else {
    crate::calcit::type_expr::print_type_compact(&Edn::enum_value("Fn", vec![edn.clone()])).ok()?
  };
  let back = match crate::calcit::type_expr::desugar_async_hint(&compact) {
    Some(result) => result.ok()?,
    None => match crate::calcit::type_expr::desugar_type_expr(&compact).ok()? {
      Cirru::List(mut parts) if parts.len() == 3 => parts.remove(2),
      _ => return None,
    },
  };
  (schema_cirru_to_edn(back) == edn).then_some(compact)
}

/// `hint-fn $ Fn …`, `hint-fn Async` or `hint-fn (Async …)`.
fn is_bare_hint_fn_type(node: &Cirru) -> bool {
  match node {
    Cirru::Leaf(name) => name.as_ref() == "Async",
    Cirru::List(items) => matches!(items.first(), Some(Cirru::Leaf(head)) if matches!(head.as_ref(), "Fn" | "Async")),
  }
}

/// `hint-fn $ quote $ Fn …` from before `hint-fn` took a bare type.
fn unquote_hint_fn_type(node: &Cirru) -> Option<Cirru> {
  let Cirru::List(quoted) = node else { return None };
  let [Cirru::Leaf(head), payload] = quoted.as_slice() else {
    return None;
  };
  (head.as_ref() == "quote" && is_bare_hint_fn_type(payload)).then(|| payload.to_owned())
}

/// Rewrite canonical `::` / `{}` type forms in code type positions into the
/// compact `quote $ ...` syntax. Run after `canonicalize_code_type_syntax`.
fn compact_code_type_syntax(node: &Cirru) -> (Cirru, usize) {
  let Cirru::List(items) = node else {
    return (node.clone(), 0);
  };
  let head = items.first().and_then(|item| match item {
    Cirru::Leaf(value) => Some(value.as_ref()),
    _ => None,
  });
  // Quoted data is not code; leave it as written.
  if head == Some("quote") {
    return (node.clone(), 0);
  }
  let mut rewritten = Vec::with_capacity(items.len());
  let mut changed = 0;
  for item in items {
    let (next, count) = compact_code_type_syntax(item);
    rewritten.push(next);
    changed += count;
  }
  let single = match head {
    Some("assert-type" | "unsafe-coerce") if items.len() >= 3 => compact_type_form(&rewritten[2]).map(|compact| (2, compact)),
    // The payload is last: `hint-fn schema` or the legacy `hint-fn target schema`.
    Some("hint-fn") if matches!(items.len(), 2 | 3) => {
      let index = items.len() - 1;
      compact_hint_fn_map(&rewritten[index])
        .or_else(|| unquote_hint_fn_type(&rewritten[index]))
        .map(|compact| (index, compact))
    }
    _ => None,
  };
  if let Some((index, compact)) = single {
    rewritten[index] = compact;
    changed += 1;
  }
  match head {
    Some(kind @ ("defstruct" | "defenum" | "deftrait")) if items.len() >= 3 => {
      for slot in rewritten.iter_mut().skip(2) {
        let Cirru::List(entry) = &*slot else { continue };
        let type_indexes: Vec<usize> = match (kind, entry.as_slice()) {
          ("defstruct", [_, _] | [_, _, _]) => vec![1],
          ("deftrait", [Cirru::Leaf(bracket), _, _]) if bracket.as_ref() == "[]" => vec![2],
          ("deftrait", [_, _]) => vec![1],
          ("defenum", [Cirru::Leaf(tag), ..]) if tag.starts_with(':') => (1..entry.len()).collect(),
          _ => vec![],
        };
        let mut next_entry = entry.clone();
        for type_index in type_indexes {
          if let Some(compact) = compact_type_form(&entry[type_index]) {
            next_entry[type_index] = compact;
            changed += 1;
          }
        }
        *slot = Cirru::List(next_entry);
      }
    }
    _ => {}
  }
  (Cirru::List(rewritten), changed)
}

fn migrate_code_type_syntax(node: &Cirru) -> (Cirru, usize) {
  let (canonical, legacy) = canonicalize_code_type_syntax(node);
  let (compact, count) = compact_code_type_syntax(&canonical);
  (compact, legacy + count)
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
      let (code, count) = migrate_code_type_syntax(&entry.code);
      entry.code = code;
      changed += count;
      let mut rewritten_examples = Vec::with_capacity(entry.examples.len());
      for example in &entry.examples {
        let (code, count) = migrate_code_type_syntax(example);
        rewritten_examples.push(code);
        changed += count;
      }
      entry.examples = rewritten_examples;
      for test in entry.tests.iter_mut() {
        let (code, count) = migrate_code_type_syntax(&test.code);
        test.code = code;
        changed += count;
      }
    }
  }
  changed
}

/// Render snapshot content for runtime snapshot files such as `calcit.cirru`
/// This is a shared utility function used by CLI edit commands
pub fn render_snapshot_content(snapshot: &Snapshot) -> Result<String, String> {
  let migrated = migrate_snapshot_code_for_write(snapshot);
  let snapshot = migrated.as_ref().unwrap_or(snapshot);
  validate_snapshot_contracts_for_write(snapshot)?;

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

  if !snapshot.verification.profiles.is_empty()
    || !snapshot.verification.host_requirements.is_empty()
    || !snapshot.verification.external_gates.is_empty()
  {
    let mut profiles_map = EdnMapView::default();
    for (name, profile) in &snapshot.verification.profiles {
      let mut profile_map = EdnMapView::default();
      profile_map.insert_key(
        "entries",
        Edn::from(profile.entries.iter().map(|entry| Edn::tag(entry.as_str())).collect::<Vec<_>>()),
      );
      profile_map.insert_key(
        "checks",
        Edn::from(profile.checks.iter().map(|check| Edn::tag(check.as_str())).collect::<Vec<_>>()),
      );
      profile_map.insert_key("on-failure", Edn::tag(profile.on_failure.as_str()));
      profiles_map.insert_key(name.as_str(), profile_map.into());
    }
    let mut verification = EdnMapView::default();
    verification.insert_key("schema-version", Edn::from(snapshot.verification.schema_version));
    if !snapshot.verification.host_requirements.is_empty() {
      verification.insert_key(
        "host-requirements",
        Edn::map_from_iter(
          snapshot
            .verification
            .host_requirements
            .iter()
            .map(|(tool, requirement)| (Edn::tag(tool.as_str()), Edn::str(requirement.as_str()))),
        ),
      );
    }
    if !snapshot.verification.external_gates.is_empty() {
      verification.insert_key(
        "external-gates",
        Edn::from(
          snapshot
            .verification
            .external_gates
            .iter()
            .map(|gate| Edn::str(gate.as_str()))
            .collect::<Vec<_>>(),
        ),
      );
    }
    verification.insert_key("profiles", profiles_map.into());
    edn_map.insert_key("verification", verification.into());
  }

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

/// Render one namespace's `FileEntry` value with the same serializer and Cirru
/// formatting `render_snapshot_content` uses for each `:files` entry.
pub fn render_file_entry_content(file: &FileInSnapShot) -> Result<String, String> {
  let normalized = normalize_pipe_prefixed_leaf(Edn::from(file).cirru());
  cirru_parser::format(std::slice::from_ref(&normalized), true.into()).map_err(|e| format!("Failed to format FileEntry as Cirru: {e}"))
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

  crate::util::atomic_write::write_snapshot(snapshot_path, content.as_bytes())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::{CalcitFnTypeAnnotation, CalcitGenericBound, CalcitTrait, SchemaKind};
  use cirru_edn::EdnListView;

  fn parse_one(source: &str) -> Cirru {
    cirru_parser::parse(source)
      .unwrap_or_else(|error| panic!("failed to parse test Cirru `{source}`: {error}"))
      .into_iter()
      .next()
      .expect("test Cirru should contain one expression")
  }

  #[test]
  fn definition_root_rejects_wrapped_declarations_without_inspecting_expression_bodies() {
    for head in DEFINITION_HEADS {
      let declaration = parse_one(&format!("{head} sample (n) n"));
      validate_definition_root(&declaration, "demo/sample").expect("an unwrapped declaration root");
      let wrapped = Cirru::List(vec![declaration]);
      for code in [&wrapped, &Cirru::List(vec![wrapped.clone()])] {
        let error = validate_definition_root(code, "demo/sample").expect_err("wrapped declaration root");
        assert!(error.starts_with("demo/sample: wrapped top-level definition"), "{error}");
        assert!(error.contains(&format!("`{head}`")), "{error}");
      }
    }
    for source in [
      "fn (n) (&+ n 1)",
      "quote (defn sample (n) n)",
      "def factory (fn (n) n)",
      "defn sample (n) ((fn (x) x) n)",
    ] {
      validate_definition_root(&parse_one(source), "demo/sample").expect("only declaration operators at the root are rejected");
    }
    let anonymous_call = Cirru::List(vec![parse_one("fn (n) (&+ n 1)"), Cirru::leaf("4")]);
    validate_definition_root(&anonymous_call, "demo/result").expect("an anonymous function call is an expression, not a declaration");
  }

  #[test]
  fn defexternal_expands_to_deftrait_and_external_ffi_metadata() {
    let source = r#"defexternal QueryHost
  :target :browser
  :names $ {} (:query |querySelector)
  :writable $ #{} :text-content
  :length 'Number
  .query $ :: 'Fn $ {} (:args $ [] 'QueryHost 'String) (:return 'QueryHost)"#;
    let (code, ffi) = normalize_defexternal_code(parse_one(source), "demo/QueryHost").expect("defexternal should expand");

    let Cirru::List(items) = &code else {
      panic!("expanded code should be a list");
    };
    assert!(matches!(items.first(), Some(Cirru::Leaf(head)) if head.as_ref() == "deftrait"));
    assert!(matches!(items.get(1), Some(Cirru::Leaf(name)) if name.as_ref() == "QueryHost"));
    // members preserve declaration order: :length, .query
    assert!(
      matches!(items.get(2), Some(Cirru::List(field)) if matches!(field.first(), Some(Cirru::Leaf(head)) if head.as_ref() == ":length"))
    );
    assert!(
      matches!(items.get(3), Some(Cirru::List(method)) if matches!(method.first(), Some(Cirru::Leaf(head)) if head.as_ref() == ".query"))
    );
    assert_eq!(items.len(), 4);

    let ffi = ffi.view_map().expect("ffi should be a map");
    assert_eq!(ffi.get(&Edn::tag("backend")), Some(&Edn::tag("js")));
    assert_eq!(ffi.get(&Edn::tag("kind")), Some(&Edn::tag("external-object")));
    assert_eq!(ffi.get(&Edn::tag("target")), Some(&Edn::tag("browser")));
    assert_eq!(
      ffi.get(&Edn::tag("writable")),
      Some(&Edn::Set(EdnSetView(HashSet::from([Edn::tag("text-content")]))))
    );
    let names = ffi.get(&Edn::tag("names")).expect("names should exist");
    let names = names.view_map().expect("names should be a map");
    assert_eq!(names.get(&Edn::tag("query")), Some(&Edn::str("querySelector")));
  }

  #[test]
  fn defexternal_keeps_bare_and_quoted_requires_clause() {
    for head in ["requires", "'requires"] {
      let source = format!("defexternal KeyEvent\n  :target :browser\n  {head} EventHost\n  :key 'String");
      let (code, _) = normalize_defexternal_code(parse_one(&source), "demo/KeyEvent").expect("defexternal should expand");
      let Cirru::List(items) = &code else {
        panic!("expanded code should be a list");
      };
      assert!(
        items.iter().any(|item| matches!(
          item,
          Cirru::List(clause)
            if matches!(clause.first(), Some(Cirru::Leaf(leaf)) if leaf.as_ref() == head)
              && matches!(clause.get(1), Some(Cirru::Leaf(parent)) if parent.as_ref() == "EventHost")
        )),
        "{head} clause should be kept: {code:?}"
      );
    }
  }

  #[test]
  fn defexternal_omits_empty_optional_metadata() {
    let source = "defexternal Bare\n  :value 'String";
    let (code, ffi) = normalize_defexternal_code(parse_one(source), "demo/Bare").expect("defexternal should expand");
    let Cirru::List(items) = &code else {
      panic!("expanded code should be a list");
    };
    assert_eq!(items.len(), 3);
    let ffi = ffi.view_map().expect("ffi should be a map");
    assert!(ffi.get(&Edn::tag("target")).is_none());
    assert!(ffi.get(&Edn::tag("names")).is_none());
    assert!(ffi.get(&Edn::tag("writable")).is_none());
  }

  #[test]
  fn defexternal_rejects_unknown_target_and_backend() {
    let target_error = normalize_defexternal_code(parse_one("defexternal T\n  :target :deno\n  :value 'String"), "demo/T")
      .expect_err("unknown target should fail");
    assert!(target_error.contains("unknown `defexternal` `:target`"), "error: {target_error}");

    let backend_error = normalize_defexternal_code(parse_one("defexternal T\n  :backend :native\n  :value 'String"), "demo/T")
      .expect_err("explicit backend should fail");
    assert!(backend_error.contains("does not accept `:backend`"), "error: {backend_error}");
  }

  #[test]
  fn defexternal_rejects_missing_members_and_duplicate_target() {
    let empty = normalize_defexternal_code(parse_one("defexternal T\n  :target :node"), "demo/T").expect_err("members required");
    assert!(empty.contains("at least one field or method"), "error: {empty}");

    let duplicate = normalize_defexternal_code(
      parse_one("defexternal T\n  :target :node\n  :target :browser\n  :value 'String"),
      "demo/T",
    )
    .expect_err("duplicate target should fail");
    assert!(duplicate.contains("more than once"), "error: {duplicate}");
  }

  #[test]
  fn defexternal_rejects_surplus_option_or_member_values() {
    let target = normalize_defexternal_code(parse_one("defexternal T\n  :target :browser :node\n  :value 'String"), "demo/T")
      .expect_err("surplus target values should fail");
    assert!(target.contains("expects exactly one value"), "error: {target}");

    let member = normalize_defexternal_code(parse_one("defexternal T\n  :value 'String 'Unexpected"), "demo/T")
      .expect_err("surplus member values should fail");
    assert!(member.contains("expects exactly one value"), "error: {member}");
  }

  #[test]
  fn defexternal_conflicts_with_explicit_ffi_metadata() {
    let ffi = Edn::Map(EdnMapView(HashMap::from_iter([(Edn::tag("backend"), Edn::tag("js"))])));
    let error = normalize_entry_external(parse_one("defexternal T\n  :value 'String"), Some(ffi), "demo/T")
      .expect_err("explicit ffi should conflict");
    assert!(error.contains("remove the explicit `:ffi`"), "error: {error}");
  }

  #[test]
  fn binary_code_entry_normalizes_defexternal_schema_and_ffi() {
    let raw = RawCodeEntry {
      doc: String::new(),
      examples: vec![],
      tests: vec![],
      tags: vec![],
      code: parse_one("defexternal QueryHost\n  :target :browser\n  :value 'String"),
      schema: None,
      ffi: None,
    };
    let entry = raw.into_code_entry("demo/QueryHost").expect("binary entry should normalize");
    assert!(
      matches!(entry.code, Cirru::List(ref items) if matches!(items.first(), Some(Cirru::Leaf(head)) if head.as_ref() == "deftrait")),
      "binary code should expand to deftrait"
    );
    assert!(
      matches!(entry.schema.as_ref(), CalcitTypeAnnotation::Custom(marker) if marker.as_ref() == &Calcit::tag("trait")),
      "binary schema should canonicalize to the trait marker, got {:?}",
      entry.schema
    );
    assert!(entry.ffi.is_some(), "binary ffi metadata should be generated");
  }

  #[test]
  fn defexternal_canonicalizes_write_schema_to_trait_marker() {
    let code = parse_one("defexternal T\n  :value 'String");
    let schema = normalize_schema_for_code(&code, &DYNAMIC_TYPE);
    assert!(
      matches!(schema.as_ref(), CalcitTypeAnnotation::Custom(marker) if marker.as_ref() == &Calcit::tag("trait")),
      "defexternal should canonicalize to the trait marker"
    );
  }

  #[test]
  fn validate_defexternal_shorthand_accepts_valid_and_rejects_invalid() {
    validate_defexternal_shorthand(&parse_one("defexternal T\n  :value 'String"), "demo/T").expect("valid shorthand should pass");
    validate_defexternal_shorthand(&parse_one("deftrait T (.x :fn)"), "demo/T")
      .expect("non-shorthand code should be ignored without validation");
    let error = validate_defexternal_shorthand(&parse_one("defexternal T\n  :target :deno\n  :value 'String"), "demo/T")
      .expect_err("invalid target should fail at the edit boundary");
    assert!(error.contains("unknown `defexternal` `:target`"), "error: {error}");
  }

  #[test]
  fn code_entry_json_round_trip_preserves_nested_function_schema() {
    let fn_schema = Arc::new(CalcitFnTypeAnnotation {
      runtime_arity: None,
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(vec![CalcitGenericBound {
        name: Arc::from("T"),
        traits: Arc::new(vec![Arc::new(CalcitTrait::new_reference("Show"))]),
      }]),
      arg_types: vec![Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")))],
      return_type: Arc::new(CalcitTypeAnnotation::Unit),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(HashSet::new()),
    });
    let entry = CodeEntry {
      doc: String::new(),
      examples: vec![],
      tests: vec![],
      tags: HashSet::new(),
      code: Cirru::leaf("nil"),
      schema: Arc::new(CalcitTypeAnnotation::Ref(Arc::new(CalcitTypeAnnotation::Fn(fn_schema)))),
      ffi: None,
    };

    let encoded = serde_json::to_string(&entry).expect("code entry should serialize");
    let restored: CodeEntry = serde_json::from_str(&encoded).expect("code entry should deserialize");

    assert_eq!(
      schema_annotation_to_edn(restored.schema.as_ref()),
      schema_annotation_to_edn(entry.schema.as_ref())
    );
  }

  #[test]
  fn verification_profiles_round_trip_with_declared_order() {
    let content = fs::read_to_string("calcit/add.cirru").expect("fixture should load");
    let data = cirru_edn::parse(&content).expect("fixture should parse");
    let mut snapshot = load_snapshot_data(&data, "calcit/add.cirru").expect("fixture snapshot should load");
    snapshot.verification.profiles.insert(
      "release".to_owned(),
      VerificationProfile {
        entries: vec!["default".to_owned()],
        checks: vec![VerificationCheckKind::Strict],
        on_failure: VerificationFailurePolicy::Continue,
      },
    );
    snapshot
      .verification
      .host_requirements
      .insert("node".to_owned(), ">=24.0.0".to_owned());
    snapshot.verification.external_gates.push("yarn build".to_owned());

    let rendered = render_snapshot_content(&snapshot).expect("verification profile should render");
    assert!(rendered.contains(":verification"));
    assert!(rendered.contains(":schema-version 1"));
    assert!(rendered.contains(":host-requirements"));
    assert!(rendered.contains(":node |>=24.0.0"));
    assert!(rendered.contains(":external-gates"));
    let parsed = cirru_edn::parse(&rendered).expect("rendered snapshot should parse");
    let restored = load_snapshot_data(&parsed, "calcit/add.cirru").expect("rendered snapshot should load");
    assert_eq!(restored.verification, snapshot.verification);
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
  fn code_entry_rejects_malformed_ffi_metadata() {
    let make_entry = |ffi| {
      Edn::struct_from_pairs(
        "CodeEntry",
        &[
          (EdnTag::new("doc"), Edn::Str(Arc::from(""))),
          (EdnTag::new("examples"), Edn::List(EdnListView(vec![]))),
          (EdnTag::new("code"), Cirru::leaf("nil").into()),
          (EdnTag::new("ffi"), ffi),
        ],
      )
    };

    let error = CodeEntry::try_from(make_entry(Edn::str("browser"))).expect_err("scalar ffi metadata should be rejected");
    assert!(error.contains("ffi metadata to be a map or struct"), "unexpected error: {error}");

    let malformed_map = Edn::map_from_iter([(Edn::str("target"), Edn::tag("browser"))]);
    let error = CodeEntry::try_from(make_entry(malformed_map)).expect_err("non-tag ffi keys should be rejected");
    assert!(error.contains("ffi map keys to be tags"), "unexpected error: {error}");
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
    // Schema parsing must not observe another test's temporary core registry.
    let _guard = crate::program::lock_program_test_state();
    let valid = parse_one(":: :fn $ {} (:args ([] :string)) (:return :bool)");
    assert!(validate_schema_for_write(&valid).is_ok(), "valid schema should pass");

    let valid_async = parse_one(":: :fn $ {} (:args ([])) (:return :string) (:async true)");
    assert!(validate_schema_for_write(&valid_async).is_ok(), "async schema should pass");
    let invalid_async = parse_one(":: :fn $ {} (:args ([])) (:return :string) (:async :yes)");
    assert_eq!(
      validate_schema_for_write(&invalid_async).expect_err("invalid async marker should fail"),
      "`:async` must be `true` or `false`"
    );

    let parsed_async = parse_schema_annotation_for_write(&valid_async).expect("async schema should parse");
    assert!(
      matches!(parsed_async.as_ref(), CalcitTypeAnnotation::Fn(signature) if signature.is_async_invocation()),
      "schema writer must preserve explicit async invocation"
    );

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

    for (source, name, arg_count) in [(":: 'Option 'Dynamic", "Option", 1), (":: 'Result 'String 'Tag", "Result", 2)] {
      let schema = parse_one(source);
      let annotation = parse_schema_annotation_for_write(&schema)
        .unwrap_or_else(|error| panic!("core nominal short name should parse: {source}: {error}"));
      assert!(
        matches!(annotation.as_ref(), CalcitTypeAnnotation::TypeRef(parsed_name, args) if parsed_name.as_ref() == name && args.len() == arg_count),
        "unexpected annotation for {source}: {annotation}"
      );
      let queried = schema_annotation_to_edn(annotation.as_ref());
      let reparsed = parse_loaded_schema_annotation(&queried, "tests/core-nominal")
        .unwrap_or_else(|error| panic!("query-style schema should reload: {source}: {error}"));
      assert_eq!(schema_annotation_to_edn(reparsed.as_ref()), queried, "{source} should round-trip");
    }

    for source in [":: 'Option", ":: 'Option 'String 'Tag", ":: 'Result 'String"] {
      let error = validate_schema_for_write(&parse_one(source)).expect_err("wrong core nominal arity should fail");
      assert!(error.contains("expects"), "error for {source}: {error}");
    }

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
    let quoted_tag = Cirru::Leaf(Arc::from("'Tag"));
    let parsed_quoted_tag = parse_schema_annotation_for_write(&quoted_tag).expect("'Tag leaf should parse");
    assert!(matches!(parsed_quoted_tag.as_ref(), CalcitTypeAnnotation::Tag));
    let serialized_tag = parse_one(":: 'Tag");
    let parsed_serialized_tag = parse_schema_annotation_for_write(&serialized_tag).expect("serialized Tag schema should parse");
    assert!(matches!(parsed_serialized_tag.as_ref(), CalcitTypeAnnotation::Tag));

    for (legacy, replacement) in [
      ("'Record", "'Struct"),
      (":record", "'Struct"),
      (":tuple", "'Enum"),
    ] {
      let error =
        validate_schema_for_write(&Cirru::Leaf(Arc::from(legacy))).expect_err("legacy data type names must be rejected on write");
      assert!(error.contains(replacement), "error should point to {replacement}: {error}");
    }

    // `Tuple` is the positional type now; it needs its element types.
    let bare_tuple = validate_schema_for_write(&Cirru::Leaf(Arc::from("'Tuple"))).expect_err("bare Tuple must be rejected");
    assert!(bare_tuple.contains("element types"), "error: {bare_tuple}");
    let tuple = parse_one(":: 'Tuple 'Number 'String");
    validate_schema_for_write(&tuple).expect("applied Tuple should validate");
    assert!(matches!(
      parse_schema_annotation_for_write(&tuple).expect("Tuple schema should parse").as_ref(),
      CalcitTypeAnnotation::Tuple(items) if items.len() == 2
    ));
    // Loading rejects a bare `Tuple` too, instead of reading it as an unresolved name.
    for loaded in [
      Edn::Quote(Cirru::Leaf(Arc::from("Tuple"))),
      Edn::enum_value("Tuple", vec![]),
    ] {
      let error = parse_loaded_schema_annotation(&loaded, "tests/pair").expect_err("bare Tuple must not load");
      assert!(error.contains("element types"), "error: {error}");
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
      data_text.contains(":kind :string"),
      "ordinary tag data must not be rewritten: {data_text}"
    );

    // A struct field's third item is a literal default, so a tag there is data.
    let struct_decl = parse_one("defstruct Mode (:kind :tag :string)");
    let (struct_decl, struct_count) = canonicalize_code_type_syntax(&struct_decl);
    let struct_text = cirru_parser::format(&[struct_decl], true.into()).expect("struct should render");
    assert_eq!(struct_count, 1, "struct text: {struct_text}");
    assert!(struct_text.contains(":kind 'Tag :string"), "struct text: {struct_text}");
    let wrapped_struct = parse_one("defstruct Mode $ {} (:kind :tag :string) (:size :number)");
    let (wrapped_struct, wrapped_count) = canonicalize_code_type_syntax(&wrapped_struct);
    let wrapped_text = cirru_parser::format(&[wrapped_struct], true.into()).expect("struct should render");
    assert_eq!(wrapped_count, 2, "wrapped struct text: {wrapped_text}");
    assert!(
      wrapped_text.contains(":kind 'Tag :string") && wrapped_text.contains(":size 'Number"),
      "wrapped struct text: {wrapped_text}"
    );

    // With nothing after it, the form is the body's value: a tag, not a return type.
    let tag_body = parse_one("defn kind-name () :string");
    assert_eq!(canonicalize_code_type_syntax(&tag_body), (tag_body.clone(), 0));

    let compact = parse_one("assert-type f $ quote $ Fn T (-> T) (:generics T)");
    let (formatted, compact_count) = canonicalize_code_type_syntax(&compact);
    assert_eq!(compact_count, 0);
    assert_eq!(formatted, compact, "compact quote type expressions are already canonical source");
  }

  #[test]
  fn format_migrates_canonical_type_positions_to_compact_quotes() {
    let cases = [
      ("assert-type xs $ :: 'List 'Number", "assert-type xs $ quote $ List Number"),
      (
        "hint-fn $ {} (:args $ [] 'T) (:return 'T) (:generics $ [] 'T)",
        "hint-fn $ Fn T (-> T) (:generics T)",
      ),
      ("hint-fn $ {} (:async true)", "hint-fn Async"),
      ("hint-fn $ quote $ Fn T (-> T) (:generics T)", "hint-fn $ Fn T (-> T) (:generics T)"),
      ("hint-fn f $ quote $ Fn Number (-> Number)", "hint-fn f $ Fn Number (-> Number)"),
      (
        "hint-fn f $ {} (:args $ [] 'Number) (:return 'Number)",
        "hint-fn f $ Fn Number (-> Number)",
      ),
      (
        "defstruct Box (:items $ :: 'List 'Number)",
        "defstruct Box $ :items $ quote $ List Number",
      ),
      (
        "defenum Shape (:some 'T $ :: 'List 'T) (:none)",
        "defenum Shape (:some 'T $ quote $ List T) (:none)",
      ),
      (
        "deftrait Show $ .show $ :: 'Fn $ {} (:args $ [] 'T) (:return 'String) (:generics $ [] 'T)",
        "deftrait Show $ .show $ quote $ Fn T (-> String) (:generics T)",
      ),
    ];
    for (source, expected) in cases {
      let (migrated, count) = migrate_code_type_syntax(&parse_one(source));
      assert_eq!(migrated, parse_one(expected), "{source}");
      assert!(count > 0, "{source}");
    }
    // Value references, partial hints and quoted data keep their written shape.
    for source in [
      "assert-type xs $ :: 'List Person",
      "hint-fn $ {} $ :args ([] 'A)",
      "hint-fn $ {} (:async false)",
      "quote $ assert-type xs $ :: 'List 'Number",
    ] {
      let node = parse_one(source);
      assert_eq!(migrate_code_type_syntax(&node), (node.clone(), 0), "{source}");
    }
  }

  fn mini_snapshot_with_def(schema: &str, code: &str) -> Edn {
    let content = format!(
      r#"{{}} (:package |mini)
  :version |0.0.0
  :entries $ {{}}
    :default $ {{}} (:mode :native) (:init-fn |mini/main!) (:reload-fn |mini/main!)
      :modules $ []
  :files $ {{}}
    |mini $ %{{}} :FileEntry
      :ns $ %{{}} :CodeEntry (:doc |) (:code $ quote (ns mini)) (:examples $ []) (:schema nil)
      :defs $ {{}}
        |main! $ %{{}} :CodeEntry (:doc |)
          :code $ quote {code}
          :examples $ []
          :schema {schema}
"#
    );
    cirru_edn::parse(&content).expect("snapshot text should parse as EDN")
  }

  #[test]
  fn strict_load_rejects_retired_type_syntax_and_points_to_format() {
    let cases = [
      (
        "$ :: 'Fn $ {} (:args $ [] 'Number) (:return 'String)",
        "(defn main! (x) (str x))",
        "quote $ Fn Number (-> String)",
      ),
      ("$ :: 'Number", "(def main! 1)", "quote Number"),
      ("$ :: 'mini/Person", "(def main! 1)", "quote mini/Person"),
      (":number", "(def main! 1)", "quote Number"),
      (
        "$ quote $ Fn Dynamic (-> Dynamic)",
        "(defn main! (xs) (assert-type xs $ :: 'List 'Number))",
        "write `quote $ List Number`",
      ),
    ];
    for (schema, code, suggestion) in cases {
      let data = mini_snapshot_with_def(schema, code);
      let err = load_snapshot_data(&data, "mini.cirru").expect_err("retired type syntax should fail strict load");
      assert!(err.contains(LEGACY_TYPE_SYNTAX_CODE), "{schema}: {err}");
      assert!(err.contains(suggestion), "{schema}: expected `{suggestion}` in {err}");
      assert!(err.contains("edit format"), "{schema}: {err}");
      load_snapshot_data_for_format(&data, "mini.cirru").expect("edit format still reads retired syntax");
    }
    for (schema, code) in [
      ("$ quote $ Fn Number (-> String)", "(defn main! (x) (str x))"),
      ("$ quote Number", "(def main! 1)"),
      (
        "$ quote $ Fn Dynamic (-> Dynamic)",
        "(defn main! (xs) (assert-type xs $ :: 'List Person))",
      ),
      ("nil", "(defn main! (x) (hint-fn $ {} (:async false)) x)"),
    ] {
      load_snapshot_data(&mini_snapshot_with_def(schema, code), "mini.cirru").unwrap_or_else(|err| panic!("{schema} {code}: {err}"));
    }
  }

  #[test]
  fn snapshot_write_stores_code_types_in_the_compact_spelling() {
    let data = mini_snapshot_with_def("$ quote $ Fn Dynamic (-> Dynamic)", "(defn main! (xs) xs)");
    let mut snapshot = load_snapshot_data(&data, "mini.cirru").expect("compact snapshot loads");
    let entry = snapshot.files.get_mut("mini").unwrap().defs.get_mut("main!").unwrap();
    entry.code = parse_one("defn main! (xs) (assert-type xs $ :: 'List 'Number) xs");
    let rendered = render_snapshot_content(&snapshot).expect("retired code input is rewritten on write");
    assert!(rendered.contains("assert-type xs $ quote $ List Number"), "{rendered}");
    let reloaded = cirru_edn::parse(rendered.trim_start_matches("#! /usr/bin/env calcit\n")).expect("rendered snapshot parses");
    load_snapshot_data(&reloaded, "mini.cirru").expect("rewritten snapshot loads strictly");
  }

  #[test]
  fn schema_input_rejects_retired_spellings_with_their_compact_form() {
    for (input, suggestion) in [
      (
        "quote $ :: 'Fn $ {} (:args $ [] 'T) (:return 'T) (:generics $ [] 'T)",
        "quote $ Fn T (-> T) (:generics T)",
      ),
      ("quote $ :: 'Trait", "quote Trait"),
    ] {
      let Cirru::List(items) = parse_one(input) else { panic!("{input}") };
      let err = reject_legacy_schema_input(&items[1]).expect_err("retired schema input should be rejected");
      assert!(err.contains(LEGACY_TYPE_SYNTAX_CODE) && err.contains(suggestion), "{input}: {err}");
    }
    for input in ["quote $ Fn T (-> T) (:generics T)", "quote Trait"] {
      let Cirru::List(items) = parse_one(input) else { panic!("{input}") };
      reject_legacy_schema_input(&items[1]).unwrap_or_else(|err| panic!("{input}: {err}"));
    }
  }

  #[test]
  fn storage_spells_type_slots_and_nominal_names_as_quotes() {
    for (input, stored) in [
      ("Fn *payload (-> *payload)", "quote $ Fn *payload (-> *payload)"),
      ("app.values/LoginState", "quote app.values/LoginState"),
    ] {
      let node = if input.contains(' ') {
        parse_one(input)
      } else {
        Cirru::leaf(input)
      };
      let annotation = parse_schema_annotation_for_write(&node).expect(input);
      let storage = schema_annotation_to_storage_edn(annotation.as_ref());
      assert_eq!(storage, cirru_edn::parse(stored).expect(stored), "{input}");
      let reloaded = parse_loaded_schema_annotation(&storage, "schema").expect(input);
      assert_eq!(
        schema_annotation_to_edn(reloaded.as_ref()),
        schema_annotation_to_edn(annotation.as_ref()),
        "{input}"
      );
    }
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
    let _guard = crate::program::lock_program_test_state();
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
    assert!(matches!(
      fn_schema.return_type.as_ref(),
      CalcitTypeAnnotation::TypeRef(name, args)
        if matches!(name.as_ref(), "Result" | "calcit.core/Result") && args.len() == 2
    ));

    let saved_text = cirru_parser::format(
      &[schema_edn_to_cirru(&fn_schema.to_schema_edn()).expect("schema edn to cirru")],
      true.into(),
    )
    .expect("format schema");
    assert!(
      saved_text.contains(":return $ :: 'Result 'T 'E") || saved_text.contains(":return $ :: 'calcit.core/Result 'T 'E"),
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
  fn test_normalize_schema_canonicalizes_string_keys_kind_and_async_values() {
    let wrapped = Edn::enum_value(
      "fn",
      vec![Edn::Map(EdnMapView::from(HashMap::from([
        (Edn::Str(Arc::from(":args")), Edn::List(EdnListView(vec![Edn::tag("set")]))),
        (Edn::Str(Arc::from(":return")), Edn::tag("bool")),
        (Edn::Str(Arc::from(":kind")), Edn::Str(Arc::from(":fn"))),
        (Edn::Str(Arc::from(":async")), Edn::Bool(true)),
      ])))],
    );

    let normalized = normalize_schema_edn(&wrapped).expect("string-key schema should normalize");
    let Edn::Map(map) = normalized else {
      panic!("normalized schema should be a map");
    };

    assert!(matches!(map.tag_get("args"), Some(Edn::List(_))));
    assert!(matches!(map.tag_get("return"), Some(Edn::Tag(tag)) if tag.ref_str() == "bool"));
    assert!(matches!(map.tag_get("kind"), Some(Edn::Tag(tag)) if tag.ref_str() == "fn"));
    assert!(matches!(map.tag_get("async"), Some(Edn::Bool(true))));
    let parsed = CalcitTypeAnnotation::parse_fn_schema_from_edn(&Edn::Map(map)).expect("normalized async fn schema");
    assert!(parsed.is_async_invocation());
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
  fn edn_schema_round_trip_preserves_absence_and_explicit_dynamic() {
    let missing = CodeEntry::from_code(Cirru::List(vec![
      Cirru::leaf("defn"),
      Cirru::leaf("helper"),
      Cirru::List(vec![]),
      Cirru::leaf("1"),
    ]));
    let mut explicit = missing.clone();
    explicit.schema = Arc::new(CalcitTypeAnnotation::Dynamic);
    assert_ne!(definition_revision(&missing).unwrap(), definition_revision(&explicit).unwrap());

    for (entry, expected_missing) in [(missing, true), (explicit, false)] {
      let mut decoded = entry;
      for _ in 0..2 {
        let rendered = cirru_edn::format(&Edn::from(&decoded), true).unwrap();
        assert_eq!(rendered.contains(":schema"), !expected_missing, "{rendered}");
        decoded = CodeEntry::try_from(cirru_edn::parse(&rendered).unwrap()).unwrap();
        assert_eq!(schema_annotation_is_missing(&decoded.schema), expected_missing);
      }
    }
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
        runtime_arity: None,
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
        runtime_arity: None,
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

    let Edn::Quote(compact) = schema else {
      panic!("top-level schema should serialize as a compact quote");
    };
    assert_eq!(
      cirru_parser::format(&[compact], true.into()).expect("format compact").trim(),
      "Fn $ -> Dynamic"
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
      macro_count, 50,
      "update the audited bundled macro inventory when core macros change"
    );
  }

  #[test]
  fn test_load_snapshot_preserves_selected_real_world_schemas() {
    let _guard = crate::program::lock_program_test_state();
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
      "nil->option",
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

    for def_name in ["->", "->%", "\\"] {
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

    for def_name in ["let[]", "loop", "struct-with", "swap!", "&doseq"] {
      let CalcitTypeAnnotation::Macro(signature) = core_file.defs[def_name].schema.as_ref() else {
        panic!("{def_name} should load as MacroSignature");
      };
      assert!(signature.is_strict(), "{def_name} should use a phase-aware contract");
      assert!(signature.capabilities.is_empty(), "{def_name} should be compile-time pure");
      assert!(signature.optional_inputs.is_empty(), "{def_name} should not have optional inputs");
      match def_name {
        "loop" | "&doseq" => assert!(matches!(
          signature.required_inputs.as_slice(),
          [crate::calcit::MacroSyntaxType::SyntaxList]
        )),
        "let[]" => assert!(matches!(
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
        let expected_unit = def_name == "&doseq";
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
                if matches!(name.as_ref(), "Option" | "calcit.core/Option")
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

    for def_name in ["&list-match-internal", "struct-match", "&struct-match-internal", "case", "&case"] {
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
        "struct-match" | "&struct-match-internal" | "case" => {
          assert!(matches!(
            signature.required_inputs.as_slice(),
            [crate::calcit::MacroSyntaxType::Expr(inner)]
              if matches!(inner.as_ref(), CalcitTypeAnnotation::Dynamic)
          ));
          assert!(matches!(signature.rest_input, Some(crate::calcit::MacroSyntaxType::SyntaxList)));
        }
        "&list-match-internal" => {
          assert!(matches!(
            signature.required_inputs.as_slice(),
            [
              crate::calcit::MacroSyntaxType::Expr(inner),
              crate::calcit::MacroSyntaxType::Expr(empty_branch),
              crate::calcit::MacroSyntaxType::SyntaxList
            ] if matches!(inner.as_ref(), CalcitTypeAnnotation::Dynamic)
              && matches!(empty_branch.as_ref(), CalcitTypeAnnotation::Dynamic)
          ));
          assert!(matches!(
            signature.rest_input,
            Some(crate::calcit::MacroSyntaxType::Expr(ref body))
              if matches!(body.as_ref(), CalcitTypeAnnotation::Dynamic)
          ));
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
  fn nil_to_option_schema_bridges_nullable_values_to_nominal_option() {
    let _guard = crate::program::lock_program_test_state();
    let core_file_content = fs::read_to_string("src/cirru/calcit-core.cirru").expect("Failed to read calcit-core.cirru");
    let edn_data = cirru_edn::parse(&core_file_content).expect("Failed to parse cirru content as EDN");
    let snapshot = load_snapshot_data(&edn_data, "src/cirru/calcit-core.cirru").expect("Failed to parse snapshot");
    let entry = snapshot
      .files
      .get("calcit.core")
      .and_then(|file| file.defs.get("nil->option"))
      .expect("calcit.core/nil->option should exist");
    let CalcitTypeAnnotation::Fn(schema) = entry.schema.as_ref() else {
      panic!("nil->option should have a function schema");
    };

    let input_var = match schema.arg_types.as_slice() {
      [arg] => match arg.as_ref() {
        CalcitTypeAnnotation::Nullable(inner, crate::calcit::NullableOrigin::Calcit) => match inner.as_ref() {
          CalcitTypeAnnotation::TypeVar(name) => name,
          other => panic!("nil->option Optional input should contain a type variable, got {other:?}"),
        },
        other => panic!("nil->option should accept Optional<T>, got {other:?}"),
      },
      args => panic!("nil->option should accept exactly one argument, got {args:?}"),
    };
    let output_var = match schema.return_type.as_ref() {
      CalcitTypeAnnotation::TypeRef(name, args) if matches!(name.as_ref(), "Option" | "calcit.core/Option") => match args.as_slice() {
        [arg] => match arg.as_ref() {
          CalcitTypeAnnotation::TypeVar(name) => name,
          other => panic!("nil->option Option output should contain a type variable, got {other:?}"),
        },
        args => panic!("nil->option Option output should have one type argument, got {args:?}"),
      },
      other => panic!("nil->option should return Option<T>, got {other:?}"),
    };
    assert_eq!(input_var, output_var, "nil->option must preserve its input type variable");
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

    for def_name in ["&+", "%{}", "not", "not=", "noted", "nth", "number?", "option:map", "nil->option"] {
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
        saved_entry.schema.is_compatible_with(source_entry.schema.as_ref())
          && source_entry.schema.is_compatible_with(saved_entry.schema.as_ref()),
        "schema should round-trip for {def_name}: source={:?}, saved={:?}",
        source_entry.schema,
        saved_entry.schema
      );
    }

    let _ = fs::remove_file(&temp_path);

    assert!(
      saved.contains("&+ $ %{} 'CodeEntry") && saved.contains(":schema $ quote $ Fn"),
      "saved snapshot should retain wrapped fn schemas"
    );
    assert!(
      saved.contains("%{} $ %{} 'CodeEntry") && saved.contains(":schema $ quote $ Macro"),
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

    for content in [
      content.to_owned(),
      content.replace("%{} :FileEntry", "{}").replace("%{} :CodeEntry", "{}"),
    ] {
      let err = validate_serialized_snapshot_content(&content).expect_err("serialized snapshot should reject double-quoted generics");
      assert!(
        err.contains("serialized snapshot has invalid `:schema`") && err.contains("excess leading quotes"),
        "unexpected error: {err}"
      );
    }
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
  fn format_loader_migrates_legacy_named_entries_without_weakening_modern_entries() {
    let mut legacy = legacy_direct_quote_snapshot([]);
    let Edn::Map(root) = &mut legacy else {
      panic!("legacy fixture root map")
    };
    let server = Edn::map_from_iter([
      (Edn::tag("init-fn"), Edn::str("mini.server/main!")),
      (Edn::tag("reload-fn"), Edn::str("mini.server/reload!")),
      (Edn::tag("modules"), Edn::List(EdnListView(vec![Edn::str("server-lib/")]))),
    ]);
    let mut node = server.clone();
    let Edn::Map(node_map) = &mut node else { panic!("entry map") };
    node_map.insert_key("mode", Edn::tag("js"));
    node_map.insert_key("target", Edn::tag("node"));
    root.insert_key(
      "entries",
      Edn::map_from_iter([(Edn::str("server"), server.clone()), (Edn::str("node"), node)]),
    );

    assert!(load_snapshot_data(&legacy, "calcit.cirru").is_err());
    let (snapshot, migration) = load_snapshot_data_for_format(&legacy, "calcit.cirru").expect("legacy named entries should migrate");
    assert!(migration.legacy_configs);
    assert_eq!(snapshot.entries["server"].mode, SnapshotRunMode::Native);
    assert_eq!(snapshot.entries["server"].init_fn, "mini.server/main!");
    assert_eq!(snapshot.entries["server"].reload_fn, "mini.server/reload!");
    assert_eq!(snapshot.entries["server"].modules, vec!["server-lib/"]);
    assert_eq!(snapshot.entries["node"].mode, SnapshotRunMode::Js);
    assert_eq!(snapshot.entries["node"].target, Some(SnapshotTarget::Node));

    let mut misspelled = legacy.clone();
    let Edn::Map(root) = &mut misspelled else {
      panic!("legacy root map")
    };
    let mut misspelled_server = server.clone();
    let Edn::Map(fields) = &mut misspelled_server else {
      panic!("entry map")
    };
    fields.insert_key("targte", Edn::tag("native"));
    root.insert_key("entries", Edn::map_from_iter([(Edn::str("server"), misspelled_server)]));
    let error =
      load_snapshot_data_for_format(&misspelled, "calcit.cirru").expect_err("unknown legacy entry fields must not be discarded");
    assert!(error.contains("entries.server: unknown field `:targte`"), "error: {error}");

    let rendered = render_snapshot_content(&snapshot).expect("migrated snapshot should render");
    let mut canonical = cirru_edn::parse(&rendered).expect("canonical output should parse");
    load_snapshot_data(&canonical, "calcit.cirru").expect("canonical entries must load strictly");
    let Edn::Map(root) = &mut canonical else {
      panic!("canonical root map")
    };
    let Edn::Map(entries) = root.0.get_mut(&Edn::tag("entries")).expect("entries") else {
      panic!("entries map")
    };
    entries.insert(Edn::str("server"), server);
    let error = load_snapshot_data_for_format(&canonical, "calcit.cirru").expect_err("modern named entries still require mode");
    assert!(error.contains("entries.server: missing `:mode`"), "error: {error}");

    let Edn::Map(root) = &mut legacy else { panic!("legacy root map") };
    let Edn::Map(entries) = root.0.get_mut(&Edn::tag("entries")).expect("entries") else {
      panic!("entries map")
    };
    let Edn::Map(server) = entries.0.get_mut(&Edn::str("server")).expect("server") else {
      panic!("entry map")
    };
    server.insert_key("mode", Edn::tag("invalid"));
    let error = load_snapshot_data_for_format(&legacy, "calcit.cirru").expect_err("explicit invalid modes must not be guessed");
    assert!(error.contains("entries.server.mode"), "error: {error}");

    let Edn::Map(root) = &mut legacy else { panic!("legacy root map") };
    root.insert_key(
      "entries",
      Edn::map_from_iter([(
        Edn::str(DEFAULT_ENTRY_NAME),
        Edn::map_from_iter([
          (Edn::tag("mode"), Edn::tag("native")),
          (Edn::tag("init-fn"), Edn::str("mini/main!")),
          (Edn::tag("reload-fn"), Edn::str("mini/reload!")),
        ]),
      )]),
    );
    let error = load_snapshot_data_for_format(&legacy, "calcit.cirru").expect_err("ambiguous defaults must still fail");
    assert!(error.contains("conflicts with existing `:entries.default`"), "error: {error}");
  }

  #[test]
  fn format_loader_rejects_unknown_legacy_configs_fields() {
    let legacy = legacy_direct_quote_snapshot([("custom", Edn::Bool(true))]);
    let error = load_snapshot_data_for_format(&legacy, "calcit.cirru").expect_err("unknown legacy config must not be discarded");
    assert!(error.contains("legacy configs: unknown field `:custom`"), "error: {error}");
  }

  #[test]
  fn format_loader_drops_only_named_legacy_entry_fields() {
    // #2052: historical editor metadata such as `:storage-key` blocks recovery until it is reviewed.
    let mut legacy = legacy_direct_quote_snapshot([]);
    let Edn::Map(root) = &mut legacy else { panic!("legacy root map") };
    root.insert_key(
      "entries",
      Edn::map_from_iter([(
        Edn::tag("server"),
        Edn::map_from_iter([
          (Edn::tag("init-fn"), Edn::str("mini/main!")),
          (Edn::tag("reload-fn"), Edn::str("mini/reload!")),
          (Edn::tag("storage-key"), Edn::str("calcit.cirru")),
          (Edn::tag("port"), Edn::Number(6001.0)),
        ]),
      )]),
    );
    let error = load_snapshot_data_for_format(&legacy, "calcit.cirru").expect_err("unknown legacy fields must not be discarded");
    assert!(error.contains("--drop-legacy-entry-field server."), "error: {error}");

    for (targets, expected) in [
      (vec!["server.mode"], "is a current entry field"),
      (vec!["client.port"], "entry `client` does not exist"),
      (vec!["server.missing"], "has no `:missing` field"),
      (vec!["server"], "must be `<entry>.<field>`"),
      (vec![".port"], "neither part may be empty"),
      (vec!["server."], "neither part may be empty"),
      (vec!["server.:"], "neither part may be empty"),
      (vec!["server.:mode"], "is a current entry field"),
    ] {
      let targets = targets.into_iter().map(str::to_owned).collect::<Vec<_>>();
      let error = drop_legacy_entry_fields(&mut legacy.clone(), &targets).expect_err("invalid drop must fail");
      assert!(error.contains(expected), "{targets:?}: {error}");
    }

    let mut partial = legacy.clone();
    drop_legacy_entry_fields(&mut partial, &["server.storage-key".to_owned()]).expect("drop storage-key");
    let error = load_snapshot_data_for_format(&partial, "calcit.cirru").expect_err("unnamed fields stay rejected");
    assert!(error.contains("unknown field `:port`"), "error: {error}");

    let dropped = drop_legacy_entry_fields(&mut legacy, &["server.storage-key".to_owned(), "server.:port".to_owned()])
      .expect("drop reviewed fields");
    assert_eq!(
      dropped,
      vec![
        ("server.storage-key".to_owned(), "do |calcit.cirru".to_owned()),
        ("server.port".to_owned(), "do 6001".to_owned())
      ]
    );
    let (snapshot, migration) = load_snapshot_data_for_format(&legacy, "calcit.cirru").expect("reviewed legacy entry should migrate");
    assert!(migration.legacy_configs);
    assert_eq!(snapshot.entries["server"].init_fn, "mini/main!");
  }

  #[test]
  fn format_loader_reports_all_entries_and_removes_normalized_field_aliases() {
    let mut legacy = legacy_direct_quote_snapshot([]);
    let Edn::Map(root) = &mut legacy else { panic!("legacy root map") };
    let entry = |fields: Vec<(Edn, Edn)>| {
      Edn::map_from_iter(
        [
          (Edn::tag("init-fn"), Edn::str("mini/main!")),
          (Edn::tag("reload-fn"), Edn::str("mini/reload!")),
        ]
        .into_iter()
        .chain(fields),
      )
    };
    root.insert_key(
      "entries",
      Edn::map_from_iter([
        (
          Edn::str("server.prod"),
          entry(vec![
            (Edn::Symbol("port".into()), Edn::Number(6001.0)),
            (Edn::str(":port"), Edn::Number(6002.0)),
          ]),
        ),
        (Edn::str("worker"), entry(vec![(Edn::tag("storage-key"), Edn::str("worker.cirru"))])),
      ]),
    );
    let error = load_snapshot_data_for_format(&legacy, "calcit.cirru").expect_err("report every entry before migration");
    assert!(error.contains("entries.server.prod: unknown field `:port`"), "{error}");
    assert!(error.contains("entries.worker: unknown field `:storage-key`"), "{error}");
    assert!(
      error.contains("edit format --drop-legacy-entry-field server.prod.port --drop-legacy-entry-field worker.storage-key"),
      "{error}"
    );
    assert_eq!(error.matches("--drop-legacy-entry-field server.prod.port").count(), 1);

    let mut partial = legacy.clone();
    let removed = drop_legacy_entry_fields(&mut partial, &["server.prod.port".to_owned()]).expect("remove both normalized aliases");
    assert_eq!(removed.len(), 2);
    assert!(removed.iter().all(|(target, _)| target == "server.prod.port"));
    for value in ["do 6001", "do 6002"] {
      assert!(removed.iter().any(|(_, preview)| preview == value), "report original value {value}");
    }
    let error = load_snapshot_data_for_format(&partial, "calcit.cirru").expect_err("other entries remain protected");
    assert!(error.contains("entries.worker: unknown field `:storage-key`"), "{error}");
    assert!(!error.contains("server.prod.port"), "{error}");

    let removed = drop_legacy_entry_fields(&mut legacy, &["server.prod.port".to_owned(), "worker.storage-key".to_owned()])
      .expect("one reviewed command can remove every named field");
    assert_eq!(removed.len(), 3);
    let (snapshot, _) = load_snapshot_data_for_format(&legacy, "calcit.cirru").expect("all legacy entries migrate");
    for name in ["server.prod", "worker"] {
      assert_eq!(snapshot.entries[name].init_fn, "mini/main!");
      assert_eq!(snapshot.entries[name].mode, SnapshotRunMode::Native);
    }
    let rendered = render_snapshot_content(&snapshot).expect("canonical output");
    let canonical = cirru_edn::parse(&rendered).expect("parse canonical output");
    load_snapshot_data(&canonical, "calcit.cirru").expect("migrated entries load with the strict runtime loader");
  }

  #[test]
  fn format_loader_treats_legacy_nil_version_as_absent() {
    let legacy = legacy_direct_quote_snapshot([("version", Edn::Nil)]);
    let (snapshot, migration) = load_snapshot_data_for_format(&legacy, "calcit.cirru").expect("legacy nil version should migrate");
    assert!(migration.legacy_configs);
    assert_eq!(snapshot.version, default_version());

    let rendered = render_snapshot_content(&snapshot).expect("migrated snapshot should render");
    let canonical = cirru_edn::parse(&rendered).expect("canonical output should parse");
    load_snapshot_data(&canonical, "calcit.cirru").expect("canonical output should pass the strict loader");
  }

  #[test]
  fn format_loader_rejects_non_nil_legacy_version_with_wrong_type() {
    let legacy = legacy_direct_quote_snapshot([("version", Edn::Bool(true))]);
    let error = load_snapshot_data_for_format(&legacy, "calcit.cirru").expect_err("non-string version must fail");
    assert!(error.contains("legacy configs.version"), "error: {error}");
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
  fn format_loader_synthesizes_a_conservative_contract_for_direct_quote_macros() {
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
    defs.insert(
      Edn::str("legacy-macro"),
      Edn::Quote(parse_one("defmacro legacy-macro (required ? optional & rest) required")),
    );

    let (snapshot, migration) =
      load_snapshot_data_for_format(&legacy, "calcit.cirru").expect("direct-quote macro should migrate in one pass");
    assert_eq!(migration.direct_quote_definitions, 2);
    let entry = &snapshot.files["mini.core"].defs["legacy-macro"];
    let CalcitTypeAnnotation::Macro(signature) = entry.schema.as_ref() else {
      panic!("migrated direct-quote macro should have a strict Macro contract")
    };
    assert!(matches!(signature.required_inputs.as_slice(), [MacroSyntaxType::Syntax]));
    assert!(matches!(signature.optional_inputs.as_slice(), [MacroSyntaxType::Syntax]));
    assert!(matches!(signature.rest_input, Some(MacroSyntaxType::Syntax)));
    assert!(
      matches!(signature.expansion, MacroExpansionType::Expr(ref semantic) if matches!(semantic.as_ref(), CalcitTypeAnnotation::Dynamic))
    );
    assert!(signature.capabilities.is_empty());

    let rendered = render_snapshot_content(&snapshot).expect("migrated snapshot should render canonically");
    let canonical = cirru_edn::parse(&rendered).expect("canonical output should parse");
    load_snapshot_data(&canonical, "calcit.cirru").expect("one-pass output should pass the current strict loader");
  }

  #[test]
  fn format_loader_does_not_hide_modern_dynamic_macros_beside_direct_quotes() {
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
    defs.insert(
      Edn::str("modern-dynamic-macro"),
      Edn::from(&CodeEntry::from_code(parse_one("defmacro modern-dynamic-macro (x) x"))),
    );

    let error = load_snapshot_data_for_format(&legacy, "calcit.cirru")
      .expect_err("an existing modern Dynamic macro must remain rejected even beside direct quotes");
    assert!(error.contains("mini.core"), "error: {error}");
    assert!(error.contains("modern-dynamic-macro"), "error: {error}");
    assert!(error.contains("a Dynamic schema"), "error: {error}");
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
    assert!(matches!(file.defs["main!"].schema.as_ref(), CalcitTypeAnnotation::Fn(_)));
    assert!(matches!(file.defs["reload!"].schema.as_ref(), CalcitTypeAnnotation::Fn(_)));
  }

  #[test]
  fn create_file_from_expression_uses_a_structured_open_entry_schema() {
    let file = create_file_from_snippet("+ 1 2").expect("expression snippet should parse");
    let CalcitTypeAnnotation::Fn(schema) = file.defs["main!"].schema.as_ref() else {
      panic!("synthetic expression entry should use a structured Fn schema");
    };
    assert!(schema.arg_types.is_empty());
    assert!(matches!(schema.return_type.as_ref(), CalcitTypeAnnotation::Dynamic));
  }
}
