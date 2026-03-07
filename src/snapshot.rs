use cirru_edn::{Edn, EdnMapView, EdnRecordView, EdnSetView, EdnTag, from_edn};
use cirru_parser::Cirru;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;
use std::path::Path;
use std::sync::Arc;

use crate::calcit::{CalcitTypeAnnotation, DYNAMIC_TYPE};

const SNAPSHOT_ABOUT_MESSAGE: &str = "file is generated - never edit directly; learn cr edit/tree workflows before changing";

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
      Some(v) => CalcitTypeAnnotation::parse_fn_schema_from_edn(&v)
        .map(|s| Arc::new(CalcitTypeAnnotation::Fn(Arc::new(s))))
        .unwrap_or_else(|| DYNAMIC_TYPE.clone()),
    })
  }
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
                let normalized = normalize_schema_edn(value)?;
                let schema_cirru = parse_schema_cirru_from_edn(&normalized)?;
                parse_schema_data(&schema_cirru)?;
                schema = CalcitTypeAnnotation::parse_fn_schema_from_edn(&normalized)
                  .map(|s| Arc::new(CalcitTypeAnnotation::Fn(Arc::new(s))))
                  .unwrap_or_else(|| DYNAMIC_TYPE.clone());
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
          let normalized = normalize_schema_edn(value)?;
          let schema_cirru = parse_schema_cirru_from_edn(&normalized)?;
          parse_schema_data(&schema_cirru)?;
          schema = CalcitTypeAnnotation::parse_fn_schema_from_edn(&normalized)
            .map(|s| Arc::new(CalcitTypeAnnotation::Fn(Arc::new(s))))
            .unwrap_or_else(|| DYNAMIC_TYPE.clone());
        }
      }
      other => {
        return Err(format!("failed to parse CodeEntry: expected record/map, got: {other:?}"));
      }
    }

    Ok(CodeEntry {
      doc,
      examples,
      code: code.ok_or_else(|| "failed to parse CodeEntry: missing code field".to_owned())?,
      schema,
    })
  }
}

/// Normalize a schema Edn value: old Quote-wrapped format is converted to direct map Edn.
/// New direct map format is returned as-is.
fn normalize_schema_edn(value: &Edn) -> Result<Edn, String> {
  // Old format stored as Edn::Quote — convert via Cirru → Edn map
  if let Ok(cirru) = from_edn::<Cirru>(value.to_owned()) {
    return Ok(schema_cirru_to_edn(cirru));
  }
  Ok(value.clone())
}

/// Convert a schema Edn value to Cirru for operations that require Cirru (validation, runtime).
/// Handles both old Quote-wrapped format and new direct map format.
pub fn schema_edn_to_cirru(value: &Edn) -> Result<Cirru, String> {
  parse_schema_cirru_from_edn(value)
}

fn parse_schema_cirru_from_edn(value: &Edn) -> Result<Cirru, String> {
  if let Ok(schema) = from_edn::<Cirru>(value.to_owned()) {
    return Ok(schema);
  }

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

/// Recursively check for symbols with excess leading single-quotes (e.g. `''''T`).
/// A valid generic type variable is a quoted uppercase symbol like `'T`, which after
/// Cirru parsing becomes a `(quote T)` list — the symbol name itself has no quotes.
fn check_no_excess_quotes(node: &Cirru) -> Result<(), String> {
  match node {
    Cirru::Leaf(s) => {
      // A leaf that starts with ' means the Cirru serializer emitted a quoted symbol name,
      // which indicates the underlying symbol still contains leading quote characters.
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

/// Strict validation for schemas submitted via `cr edit schema`.
/// Ensures the schema is a `{}` map, has a recognised `:kind`, and contains
/// only permitted fields.  Loading (read-only) only requires the weaker
/// `parse_schema_data` check.
pub fn validate_schema_for_write(schema: &Cirru) -> Result<(), String> {
  let Cirru::List(items) = schema else {
    let leaf = if let Cirru::Leaf(s) = schema {
      s.to_string()
    } else {
      "(unexpected)".to_owned()
    };
    return Err(format!("Schema must be a `{{}}` map expression, got leaf: `{leaf}`"));
  };

  let Some(Cirru::Leaf(head)) = items.first() else {
    return Err("Schema must be a non-empty list starting with `{}`".to_owned());
  };

  if head.as_ref() != "{}" {
    return Err(format!(
      "Schema top-level must start with `{{}}`, got: `{head}`. \
       Example: `{{}} (:kind :fn) (:args ([] :string)) (:return :bool)`"
    ));
  }

  // EDN-level validity
  parse_schema_data(schema)?;

  // Reject deprecated :nil type annotation
  check_no_nil_type(schema)?;

  // Reject excess-quoted type variables like ''''T
  check_no_excess_quotes(schema)?;

  // Field-level validation
  let mut has_kind = false;
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
    }
  }

  if !has_kind {
    return Err("Schema must have a `:kind` field (`:fn` or `:macro`)".to_owned());
  }

  Ok(())
}

impl From<CodeEntry> for Edn {
  fn from(data: CodeEntry) -> Self {
    let schema_edn: Edn = match data.schema.as_ref() {
      CalcitTypeAnnotation::Dynamic => Edn::Nil,
      CalcitTypeAnnotation::Fn(fn_annot) => fn_annot.to_schema_edn(),
      _ => Edn::Nil,
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
    let schema_edn: Edn = match data.schema.as_ref() {
      CalcitTypeAnnotation::Dynamic => Edn::Nil,
      CalcitTypeAnnotation::Fn(fn_annot) => fn_annot.to_schema_edn(),
      _ => Edn::Nil,
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
    from_edn(data)
  }
}

/// parse snapshot
pub fn load_snapshot_data(data: &Edn, path: &str) -> Result<Snapshot, String> {
  let data = data.view_map()?;
  let pkg: Arc<str> = data.get_or_nil("package").try_into()?;
  let mut files: HashMap<String, FileInSnapShot> = data.get_or_nil("files").try_into()?;
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
      schema: DYNAMIC_TYPE.clone(),
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
        ns: CodeEntry::from_code(ns_code),
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
pub fn save_snapshot_to_file<P: AsRef<Path>>(compact_cirru_path: P, snapshot: &Snapshot) -> Result<(), String> {
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

  // Format Edn as Cirru string
  let content = cirru_edn::format(&edn_data, true).map_err(|e| format!("Failed to format snapshot as Cirru: {e}"))?;

  // Write to file
  std::fs::write(compact_cirru_path, content).map_err(|e| format!("Failed to write compact.cirru: {e}"))?;

  Ok(())
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

    // Leaf (not a map form)
    let leaf = Cirru::Leaf(Arc::from(":fn"));
    assert!(validate_schema_for_write(&leaf).is_err(), "leaf should fail");

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
}
