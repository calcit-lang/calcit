//! Type coverage and weak-type analysis for `cr analyze`.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use calcit::calcit::{CalcitProc, CalcitSyntax, CalcitTypeAnnotation, ProcTypeSignature, SchemaKind, SyntaxTypeSignature};
use calcit::cli_args::{CheckTypesCommand, WeakTypesCommand};
use calcit::snapshot;
use cirru_parser::Cirru;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefKind {
  Data,
  Fn,
  Macro,
  Proc,
  Syntax,
  Other,
}

impl DefKind {
  fn as_str(self) -> &'static str {
    match self {
      DefKind::Data => "data",
      DefKind::Fn => "fn",
      DefKind::Macro => "macro",
      DefKind::Proc => "proc",
      DefKind::Syntax => "syntax",
      DefKind::Other => "other",
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CoverageLevel {
  None,
  Partial,
  Full,
}

impl CoverageLevel {
  fn as_str(self) -> &'static str {
    match self {
      CoverageLevel::None => "none",
      CoverageLevel::Partial => "partial",
      CoverageLevel::Full => "full",
    }
  }
}

#[derive(Debug, Clone)]
pub struct TypeCoverageRow {
  pub ns: String,
  pub def: String,
  pub kind: DefKind,
  pub level: CoverageLevel,
  pub params: Vec<String>,
  pub param_annotations: BTreeMap<String, Vec<String>>,
  pub return_type_hints: Vec<String>,
  pub data_type: Option<String>,
  pub schema_issues: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WeakTypeKind {
  SchemaDynamic,
  CodeDynamic,
  CodeNil,
}

impl WeakTypeKind {
  fn as_str(&self) -> &'static str {
    match self {
      Self::SchemaDynamic => "schema-dynamic",
      Self::CodeDynamic => "code-dynamic",
      Self::CodeNil => "code-nil",
    }
  }

  pub fn all() -> BTreeSet<Self> {
    BTreeSet::from([Self::SchemaDynamic, Self::CodeDynamic, Self::CodeNil])
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeakTypeOccurrence {
  pub kind: WeakTypeKind,
  pub detail: String,
  pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeakTypeRow {
  pub ns: String,
  pub def: String,
  pub occurrences: Vec<WeakTypeOccurrence>,
}

pub fn parse_weak_type_kinds(raw: &str) -> Result<BTreeSet<WeakTypeKind>, String> {
  let mut selected = BTreeSet::new();

  for item in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
    let kind = match item {
      "schema-dynamic" => WeakTypeKind::SchemaDynamic,
      "code-dynamic" => WeakTypeKind::CodeDynamic,
      "code-nil" => WeakTypeKind::CodeNil,
      other => {
        return Err(format!(
          "Unknown weak-type filter `{other}`. Expected comma-separated values from: schema-dynamic, code-dynamic, code-nil"
        ));
      }
    };
    selected.insert(kind);
  }

  if selected.is_empty() {
    return Err("Weak-type filter cannot be empty. Use comma-separated values from: schema-dynamic, code-dynamic, code-nil".to_owned());
  }

  Ok(selected)
}

fn format_cirru_path(root: &str, path: &[usize]) -> String {
  let mut rendered = root.to_owned();
  for idx in path {
    rendered.push('.');
    rendered.push_str(&idx.to_string());
  }
  rendered
}

fn weak_type_detail(kind: WeakTypeKind, detail: &str) -> String {
  format!("{}:{}", kind.as_str(), detail)
}

pub fn extract_schema_dynamic_position(detail: &str) -> Option<String> {
  let mut parts = detail.split(':');
  match (parts.next(), parts.next()) {
    (Some("schema-dynamic"), Some(position)) => Some(position.to_owned()),
    _ => None,
  }
}

pub fn extract_schema_dynamic_shape(detail: &str) -> Option<String> {
  let mut parts = detail.split(':');
  match (parts.next(), parts.next(), parts.next()) {
    (Some("schema-dynamic"), Some(_position), Some(shape)) => {
      let mut result = shape.to_owned();
      for part in parts {
        result.push(':');
        result.push_str(part);
      }
      Some(result)
    }
    _ => None,
  }
}

pub fn extract_schema_dynamic_family(detail: &str) -> Option<String> {
  let shape = extract_schema_dynamic_shape(detail)?;
  let family = shape.split(':').next()?;
  let family = family.split('-').next()?;
  Some(family.to_owned())
}

fn extend_schema_dynamic_detail(detail: &str, segment: &str) -> String {
  format!("{detail}:{segment}")
}

fn push_weak_type_occurrence(
  occurrences: &mut Vec<WeakTypeOccurrence>,
  kind: WeakTypeKind,
  detail: impl Into<String>,
  path: impl Into<String>,
) {
  occurrences.push(WeakTypeOccurrence {
    kind,
    detail: detail.into(),
    path: path.into(),
  });
}

fn scan_schema_dynamic_annotation(
  annotation: &CalcitTypeAnnotation,
  path: &str,
  detail: &str,
  occurrences: &mut Vec<WeakTypeOccurrence>,
) {
  match annotation {
    CalcitTypeAnnotation::Dynamic => {
      push_weak_type_occurrence(
        occurrences,
        WeakTypeKind::SchemaDynamic,
        weak_type_detail(WeakTypeKind::SchemaDynamic, detail),
        path.to_owned(),
      );
    }
    CalcitTypeAnnotation::List(inner)
    | CalcitTypeAnnotation::Set(inner)
    | CalcitTypeAnnotation::Ref(inner)
    | CalcitTypeAnnotation::Variadic(inner)
    | CalcitTypeAnnotation::Optional(inner) => {
      let segment = match annotation {
        CalcitTypeAnnotation::List(_) => "list-item",
        CalcitTypeAnnotation::Set(_) => "set-item",
        CalcitTypeAnnotation::Ref(_) => "ref-item",
        CalcitTypeAnnotation::Variadic(_) => "variadic-item",
        CalcitTypeAnnotation::Optional(_) => "optional-item",
        _ => unreachable!("composite item annotation should be covered by the match arm"),
      };
      let nested_detail = extend_schema_dynamic_detail(detail, segment);
      scan_schema_dynamic_annotation(inner, &format!("{path}.item"), &nested_detail, occurrences);
    }
    CalcitTypeAnnotation::Map(key, value) => {
      let key_detail = extend_schema_dynamic_detail(detail, "map-key");
      let value_detail = extend_schema_dynamic_detail(detail, "map-value");
      scan_schema_dynamic_annotation(key, &format!("{path}.key"), &key_detail, occurrences);
      scan_schema_dynamic_annotation(value, &format!("{path}.value"), &value_detail, occurrences);
    }
    CalcitTypeAnnotation::Fn(fn_annot) => {
      for (idx, arg) in fn_annot.arg_types.iter().enumerate() {
        let arg_detail = extend_schema_dynamic_detail(detail, "fn-arg");
        scan_schema_dynamic_annotation(arg, &format!("{path}.args.{idx}"), &arg_detail, occurrences);
      }
      let return_detail = extend_schema_dynamic_detail(detail, "fn-return");
      scan_schema_dynamic_annotation(&fn_annot.return_type, &format!("{path}.return"), &return_detail, occurrences);
      if let Some(rest) = &fn_annot.rest_type {
        let rest_detail = extend_schema_dynamic_detail(detail, "fn-rest");
        scan_schema_dynamic_annotation(rest, &format!("{path}.rest"), &rest_detail, occurrences);
      }
    }
    CalcitTypeAnnotation::Struct(_, args) | CalcitTypeAnnotation::Enum(_, args) | CalcitTypeAnnotation::TypeRef(_, args) => {
      for (idx, arg) in args.iter().enumerate() {
        let type_arg_detail = extend_schema_dynamic_detail(detail, "type-arg");
        scan_schema_dynamic_annotation(arg, &format!("{path}.type-arg.{idx}"), &type_arg_detail, occurrences);
      }
    }
    _ => {}
  }
}

#[derive(Debug, Clone)]
struct WeakCodeParent {
  head: Option<String>,
  child_index: usize,
}

fn classify_code_dynamic(parent: Option<&WeakCodeParent>) -> &'static str {
  match parent.and_then(|it| it.head.as_deref()) {
    Some("assert-type") => "assert-type",
    Some("hint-fn") => "hint-fn",
    Some("::") => "schema-tag",
    Some("defstruct") => "defstruct",
    Some("defenum") => "defenum",
    Some("deftrait") => "deftrait",
    Some("quote") | Some("quasiquote") => "quoted",
    Some("[]") => "list-item",
    _ => "literal",
  }
}

fn classify_code_nil(parent: Option<&WeakCodeParent>) -> &'static str {
  match parent.and_then(|it| it.head.as_deref()) {
    Some("if") if parent.is_some_and(|it| it.child_index == 2) => "if-then",
    Some("if") if parent.is_some_and(|it| it.child_index == 3) => "if-else",
    Some("let") | Some("&let") => "let-binding",
    Some("do") => "do-step",
    Some("[]") => "list-item",
    Some("{}") | Some("&{}") | Some("#{}") => "collection-item",
    Some("cond") | Some("case") | Some("case-default") => "branch",
    _ => "literal",
  }
}

fn scan_cirru_weak_types(
  node: &Cirru,
  root: &str,
  path: &mut Vec<usize>,
  parent: Option<&WeakCodeParent>,
  selected: &BTreeSet<WeakTypeKind>,
  occurrences: &mut Vec<WeakTypeOccurrence>,
) {
  match node {
    Cirru::Leaf(text) => {
      if text.as_ref() == ":dynamic" && root == "schema" && selected.contains(&WeakTypeKind::SchemaDynamic) {
        push_weak_type_occurrence(
          occurrences,
          WeakTypeKind::SchemaDynamic,
          weak_type_detail(WeakTypeKind::SchemaDynamic, "raw-schema"),
          format_cirru_path(root, path),
        );
      } else if text.as_ref() == ":dynamic" && root == "code" && selected.contains(&WeakTypeKind::CodeDynamic) {
        push_weak_type_occurrence(
          occurrences,
          WeakTypeKind::CodeDynamic,
          weak_type_detail(WeakTypeKind::CodeDynamic, classify_code_dynamic(parent)),
          format_cirru_path(root, path),
        );
      } else if text.as_ref() == "nil" && root == "code" && selected.contains(&WeakTypeKind::CodeNil) {
        push_weak_type_occurrence(
          occurrences,
          WeakTypeKind::CodeNil,
          weak_type_detail(WeakTypeKind::CodeNil, classify_code_nil(parent)),
          format_cirru_path(root, path),
        );
      }
    }
    Cirru::List(items) => {
      let head = items.first().and_then(|item| match item {
        Cirru::Leaf(text) => Some(text.to_string()),
        _ => None,
      });
      for (idx, item) in items.iter().enumerate() {
        path.push(idx);
        let next_parent = WeakCodeParent {
          head: head.clone(),
          child_index: idx,
        };
        scan_cirru_weak_types(item, root, path, Some(&next_parent), selected, occurrences);
        path.pop();
      }
    }
  }
}

pub fn analyze_weak_types_entry(
  ns: &str,
  def_name: &str,
  entry: &snapshot::CodeEntry,
  selected: &BTreeSet<WeakTypeKind>,
) -> Option<WeakTypeRow> {
  let mut occurrences: Vec<WeakTypeOccurrence> = vec![];

  if matches!(entry.schema.as_ref(), CalcitTypeAnnotation::Dynamic) && selected.contains(&WeakTypeKind::SchemaDynamic) {
    push_weak_type_occurrence(
      &mut occurrences,
      WeakTypeKind::SchemaDynamic,
      weak_type_detail(WeakTypeKind::SchemaDynamic, "root"),
      "schema".to_owned(),
    );
  } else if let CalcitTypeAnnotation::Fn(fn_annot) = entry.schema.as_ref() {
    if selected.contains(&WeakTypeKind::SchemaDynamic) {
      for (idx, arg) in fn_annot.arg_types.iter().enumerate() {
        scan_schema_dynamic_annotation(arg, &format!("schema.args.{idx}"), "arg", &mut occurrences);
      }
      scan_schema_dynamic_annotation(&fn_annot.return_type, "schema.return", "return", &mut occurrences);
      if let Some(rest) = &fn_annot.rest_type {
        scan_schema_dynamic_annotation(rest, "schema.rest", "rest", &mut occurrences);
      }
    }
  } else if selected.contains(&WeakTypeKind::SchemaDynamic) {
    let before = occurrences.len();
    scan_schema_dynamic_annotation(entry.schema.as_ref(), "schema", "root", &mut occurrences);

    if occurrences.len() == before
      && let Ok(schema_cirru) = snapshot::schema_edn_to_cirru(&entry.schema.to_type_edn())
    {
      let mut path = vec![];
      scan_cirru_weak_types(&schema_cirru, "schema", &mut path, None, selected, &mut occurrences);
    }
  }

  let mut code_path = vec![];
  scan_cirru_weak_types(&entry.code, "code", &mut code_path, None, selected, &mut occurrences);

  if occurrences.is_empty() {
    None
  } else {
    Some(WeakTypeRow {
      ns: ns.to_owned(),
      def: def_name.to_owned(),
      occurrences,
    })
  }
}

pub fn run_weak_types_report(options: &WeakTypesCommand, snapshot: &snapshot::Snapshot, out: &mut String) -> Result<(), String> {
  if let Some(ns) = &options.ns
    && !snapshot.files.contains_key(ns)
  {
    return Err(format!("Namespace not found: {ns}"));
  }

  let selected = options
    .only
    .as_deref()
    .map(parse_weak_type_kinds)
    .transpose()?
    .unwrap_or_else(WeakTypeKind::all);

  let pkg = snapshot.package.as_str();
  let explicit_scope = options.ns.is_some() || options.ns_prefix.is_some();
  let mut rows: Vec<WeakTypeRow> = vec![];

  for (ns, file) in &snapshot.files {
    if let Some(exact) = &options.ns
      && ns != exact
    {
      continue;
    }
    if let Some(prefix) = &options.ns_prefix
      && !ns.starts_with(prefix)
    {
      continue;
    }
    if !(options.deps || explicit_scope || ns == pkg || ns.starts_with(&format!("{pkg}."))) {
      continue;
    }

    for (def_name, entry) in &file.defs {
      if let Some(row) = analyze_weak_types_entry(ns, def_name, entry, &selected) {
        rows.push(row);
      }
    }
  }

  rows.sort_by(|a, b| a.ns.cmp(&b.ns).then(a.def.cmp(&b.def)));

  if rows.is_empty() {
    let _ = writeln!(out, "No weak type usage found in selected namespace scope.");
    return Ok(());
  }

  let mut kind_count: BTreeMap<&'static str, usize> = BTreeMap::new();
  let mut detail_count: BTreeMap<&'static str, BTreeMap<String, usize>> = BTreeMap::new();
  let mut schema_shape_count: BTreeMap<String, usize> = BTreeMap::new();
  let mut schema_shape_positions: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
  let mut schema_shape_defs: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
  let mut schema_shape_position_defs: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
  let mut schema_family_count: BTreeMap<String, usize> = BTreeMap::new();
  let mut schema_family_positions: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
  let mut schema_family_defs: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
  let mut ns_set: BTreeSet<&str> = BTreeSet::new();
  let mut def_count = 0usize;

  for row in &rows {
    def_count += 1;
    ns_set.insert(row.ns.as_str());
    let def_label = format!("{}/{}", row.ns, row.def);
    for occurrence in &row.occurrences {
      *kind_count.entry(occurrence.kind.as_str()).or_insert(0) += 1;
      *detail_count
        .entry(occurrence.kind.as_str())
        .or_default()
        .entry(occurrence.detail.clone())
        .or_insert(0) += 1;
      let schema_position = if occurrence.kind == WeakTypeKind::SchemaDynamic {
        extract_schema_dynamic_position(&occurrence.detail)
      } else {
        None
      };
      if occurrence.kind == WeakTypeKind::SchemaDynamic
        && let Some(shape) = extract_schema_dynamic_shape(&occurrence.detail)
      {
        *schema_shape_count.entry(shape.clone()).or_insert(0) += 1;
        if let Some(position) = &schema_position {
          *schema_shape_positions
            .entry(shape.clone())
            .or_default()
            .entry(position.clone())
            .or_insert(0) += 1;
          *schema_shape_position_defs
            .entry(format!("{shape}@{position}"))
            .or_default()
            .entry(def_label.clone())
            .or_insert(0) += 1;
        }
        *schema_shape_defs.entry(shape).or_default().entry(def_label.clone()).or_insert(0) += 1;
      }
      if occurrence.kind == WeakTypeKind::SchemaDynamic
        && let Some(family) = extract_schema_dynamic_family(&occurrence.detail)
      {
        *schema_family_count.entry(family.clone()).or_insert(0) += 1;
        if let Some(position) = &schema_position {
          *schema_family_positions
            .entry(family.clone())
            .or_default()
            .entry(position.clone())
            .or_insert(0) += 1;
        }
        *schema_family_defs.entry(family).or_default().entry(def_label.clone()).or_insert(0) += 1;
      }
    }
  }

  let _ = writeln!(out, "Weak type usage check");
  let _ = writeln!(out, "- namespaces: {}", ns_set.len());
  let _ = writeln!(out, "- defs with hits: {def_count}");
  if let Some(raw) = &options.only {
    let _ = writeln!(out, "- only: {raw}");
  }
  let _ = writeln!(
    out,
    "- hits: schema-dynamic={} code-dynamic={} code-nil={}",
    kind_count.get("schema-dynamic").copied().unwrap_or(0),
    kind_count.get("code-dynamic").copied().unwrap_or(0),
    kind_count.get("code-nil").copied().unwrap_or(0)
  );
  let _ = writeln!(out, "- detail:");
  for kind in ["schema-dynamic", "code-dynamic", "code-nil"] {
    let _ = writeln!(out, "  - {kind}");
    if let Some(items) = detail_count.get(kind) {
      for (detail, count) in items {
        let _ = writeln!(out, "    - {detail}={count}");
      }
    }
  }
  if !schema_shape_count.is_empty() {
    let _ = writeln!(out, "- schema-shapes:");
    for (shape, count) in &schema_shape_count {
      let _ = writeln!(out, "  - {shape}={count}");
    }
    let _ = writeln!(out, "- schema-shape-positions:");
    for (shape, positions) in &schema_shape_positions {
      let _ = writeln!(out, "  - {shape}");
      for (position, count) in positions {
        let _ = writeln!(out, "    - {position}={count}");
      }
    }
    let _ = writeln!(out, "- schema-shape-defs:");
    for (shape, defs) in &schema_shape_defs {
      let _ = writeln!(out, "  - {shape}");
      let mut items = defs.iter().collect::<Vec<_>>();
      items.sort_by(|(a_name, a_count), (b_name, b_count)| b_count.cmp(a_count).then(a_name.cmp(b_name)));
      for (def_name, count) in items {
        let _ = writeln!(out, "    - {def_name}={count}");
      }
    }
    let _ = writeln!(out, "- schema-shape-position-defs:");
    for (shape_position, defs) in &schema_shape_position_defs {
      let _ = writeln!(out, "  - {shape_position}");
      let mut items = defs.iter().collect::<Vec<_>>();
      items.sort_by(|(a_name, a_count), (b_name, b_count)| b_count.cmp(a_count).then(a_name.cmp(b_name)));
      for (def_name, count) in items {
        let _ = writeln!(out, "    - {def_name}={count}");
      }
    }
  }
  if !schema_family_count.is_empty() {
    let _ = writeln!(out, "- schema-families:");
    for (family, count) in &schema_family_count {
      let _ = writeln!(out, "  - {family}={count}");
    }
    let _ = writeln!(out, "- schema-family-positions:");
    for (family, positions) in &schema_family_positions {
      let _ = writeln!(out, "  - {family}");
      for (position, count) in positions {
        let _ = writeln!(out, "    - {position}={count}");
      }
    }
    let _ = writeln!(out, "- schema-family-defs:");
    for (family, defs) in &schema_family_defs {
      let _ = writeln!(out, "  - {family}");
      let mut items = defs.iter().collect::<Vec<_>>();
      items.sort_by(|(a_name, a_count), (b_name, b_count)| b_count.cmp(a_count).then(a_name.cmp(b_name)));
      for (def_name, count) in items {
        let _ = writeln!(out, "    - {def_name}={count}");
      }
    }
  }
  let _ = writeln!(out,);

  let mut current_ns: Option<&str> = None;
  for row in &rows {
    if current_ns != Some(row.ns.as_str()) {
      let _ = writeln!(out, "namespace: {}", row.ns);
      current_ns = Some(row.ns.as_str());
    }

    let _ = writeln!(out, "- def: {}", row.def);
    for occurrence in &row.occurrences {
      let _ = writeln!(
        out,
        "  - {} ({}) @ {}",
        occurrence.kind.as_str(),
        occurrence.detail,
        occurrence.path
      );
    }
    let _ = writeln!(out,);
  }

  Ok(())
}

pub fn run_check_types_report(options: &CheckTypesCommand, snapshot: &snapshot::Snapshot, out: &mut String) -> Result<(), String> {
  if let Some(ns) = &options.ns
    && !snapshot.files.contains_key(ns)
  {
    return Err(format!("Namespace not found: {ns}"));
  }

  let mut rows: Vec<TypeCoverageRow> = Vec::new();
  let pkg = snapshot.package.as_str();
  let explicit_scope = options.ns.is_some() || options.ns_prefix.is_some();

  for (ns, file) in &snapshot.files {
    if let Some(exact) = &options.ns
      && ns != exact
    {
      continue;
    }
    if let Some(prefix) = &options.ns_prefix
      && !ns.starts_with(prefix)
    {
      continue;
    }
    if !(options.deps || explicit_scope || ns == pkg || ns.starts_with(&format!("{pkg}."))) {
      continue;
    }

    for (def_name, entry) in &file.defs {
      rows.push(analyze_code_entry(ns, def_name, entry));
    }
  }

  if let Some(raw) = &options.only {
    let selected = parse_coverage_levels(raw)?;
    rows.retain(|row| selected.contains(&row.level));
  }

  rows.sort_by(|a, b| {
    a.ns
      .cmp(&b.ns)
      .then(a.level.cmp(&b.level))
      .then(a.kind.as_str().cmp(b.kind.as_str()))
      .then(a.def.cmp(&b.def))
  });

  if rows.is_empty() {
    let _ = writeln!(out, "No definitions found in selected namespace scope.");
    return Ok(());
  }

  let mut level_count: BTreeMap<&'static str, usize> = BTreeMap::new();
  let mut kind_count: BTreeMap<&'static str, usize> = BTreeMap::new();
  let mut ns_set: BTreeSet<String> = BTreeSet::new();

  for row in &rows {
    *level_count.entry(row.level.as_str()).or_insert(0) += 1;
    *kind_count.entry(row.kind.as_str()).or_insert(0) += 1;
    ns_set.insert(row.ns.clone());
  }

  let _ = writeln!(out, "Type coverage check");
  let _ = writeln!(out, "- namespaces: {}", ns_set.len());
  let _ = writeln!(out, "- defs: {}", rows.len());
  if let Some(raw) = &options.only {
    let _ = writeln!(out, "- only: {raw}");
  }
  let _ = writeln!(
    out,
    "- levels: full={} partial={} none={}",
    level_count.get("full").copied().unwrap_or(0),
    level_count.get("partial").copied().unwrap_or(0),
    level_count.get("none").copied().unwrap_or(0)
  );
  let _ = writeln!(
    out,
    "- kinds: fn={} macro={} proc={} syntax={} data={} other={}",
    kind_count.get("fn").copied().unwrap_or(0),
    kind_count.get("macro").copied().unwrap_or(0),
    kind_count.get("proc").copied().unwrap_or(0),
    kind_count.get("syntax").copied().unwrap_or(0),
    kind_count.get("data").copied().unwrap_or(0),
    kind_count.get("other").copied().unwrap_or(0)
  );
  let _ = writeln!(out,);

  let mut current_ns: Option<&str> = None;

  for row in &rows {
    let typed_params = count_typed_params(&row.params, &row.param_annotations);
    let total_params = row.params.len();

    if current_ns != Some(row.ns.as_str()) {
      let _ = writeln!(out, "namespace: {}", row.ns);
      current_ns = Some(row.ns.as_str());
    }

    let _ = writeln!(out, "- def: {}", row.def);
    let _ = writeln!(out, "  kind: {}", row.kind.as_str());
    let _ = writeln!(out, "  coverage: {}", row.level.as_str());

    match row.kind {
      DefKind::Data => {
        let _ = writeln!(
          out,
          "  data-type: {}",
          row.data_type.clone().unwrap_or_else(|| "unknown".to_string())
        );
      }
      DefKind::Fn => {
        if row.return_type_hints.is_empty() {
          let _ = writeln!(out, "  return: (no hint)");
        } else {
          let _ = writeln!(out, "  return:");
          for item in &row.return_type_hints {
            let _ = writeln!(out, "    - {item}");
          }
        }

        let _ = writeln!(out, "  params ({typed_params}/{total_params}):");
        if total_params == 0 {
          let _ = writeln!(out, "    - (no params)");
        } else {
          for name in &row.params {
            match row.param_annotations.get(name) {
              Some(types) if !types.is_empty() => {
                let _ = writeln!(out, "    - {} => {}", name, types.join(" | "));
              }
              _ => {
                let _ = writeln!(out, "    - {name} => (no assert-type)");
              }
            }
          }
        }
      }
      DefKind::Macro => {
        let _ = writeln!(out, "  params ({typed_params}/{total_params}):");
        if total_params == 0 {
          let _ = writeln!(out, "    - (no params)");
        } else {
          for name in &row.params {
            match row.param_annotations.get(name) {
              Some(types) if !types.is_empty() => {
                let _ = writeln!(out, "    - {} => {}", name, types.join(" | "));
              }
              _ => {
                let _ = writeln!(out, "    - {name} => (no assert-type)");
              }
            }
          }
        }
      }
      DefKind::Proc => {
        if row.return_type_hints.is_empty() {
          let _ = writeln!(out, "  return: (no hint)");
        } else {
          let _ = writeln!(out, "  return:");
          for item in &row.return_type_hints {
            let _ = writeln!(out, "    - {item}");
          }
        }

        let _ = writeln!(out, "  params ({typed_params}/{total_params}):");
        if total_params == 0 {
          let _ = writeln!(out, "    - (no params)");
        } else {
          for name in &row.params {
            match row.param_annotations.get(name) {
              Some(types) if !types.is_empty() => {
                let _ = writeln!(out, "    - {} => {}", name, types.join(" | "));
              }
              _ => {
                let _ = writeln!(out, "    - {name} => (no assert-type)");
              }
            }
          }
        }
      }
      DefKind::Syntax => {
        if row.return_type_hints.is_empty() {
          let _ = writeln!(out, "  return: (no hint)");
        } else {
          let _ = writeln!(out, "  return:");
          for item in &row.return_type_hints {
            let _ = writeln!(out, "    - {item}");
          }
        }

        let _ = writeln!(out, "  params ({typed_params}/{total_params}):");
        if total_params == 0 {
          let _ = writeln!(out, "    - (no params)");
        } else {
          for name in &row.params {
            match row.param_annotations.get(name) {
              Some(types) if !types.is_empty() => {
                let _ = writeln!(out, "    - {} => {}", name, types.join(" | "));
              }
              _ => {
                let _ = writeln!(out, "    - {name} => (no assert-type)");
              }
            }
          }
        }
      }
      DefKind::Other => {
        let _ = writeln!(out, "  details: no type pattern recognized");
      }
    }

    if !row.schema_issues.is_empty() {
      let _ = writeln!(out, "  schema-issues:");
      for issue in &row.schema_issues {
        let _ = writeln!(out, "    - {issue}");
      }
    }

    let _ = writeln!(out,);
  }

  Ok(())
}

fn analyze_builtin_syntax(def_name: &str, sig: &SyntaxTypeSignature) -> TypeCoverageRow {
  let params: Vec<String> = sig.param_names.iter().map(|s| s.to_string()).collect();

  let param_annotations: BTreeMap<String, Vec<String>> = sig
    .param_types
    .iter()
    .zip(sig.param_names.iter())
    .map(|(t, name)| {
      let type_str = t.describe();
      (name.to_string(), vec![type_str])
    })
    .collect();

  let return_type_hints = vec![sig.return_type.describe()];

  let typed_count = param_annotations.values().filter(|v| !v.is_empty()).count();
  let level = if params.is_empty() || typed_count == params.len() {
    CoverageLevel::Full
  } else if typed_count > 0 {
    CoverageLevel::Partial
  } else {
    CoverageLevel::None
  };

  TypeCoverageRow {
    ns: calcit::calcit::CORE_NS.to_owned(),
    def: def_name.to_owned(),
    kind: DefKind::Syntax,
    level,
    params,
    param_annotations,
    return_type_hints,
    data_type: None,
    schema_issues: vec![],
  }
}

fn analyze_builtin_proc(def_name: &str, sig: &ProcTypeSignature) -> TypeCoverageRow {
  let params: Vec<String> = sig.arg_types.iter().enumerate().map(|(i, _)| format!("arg{i}")).collect();

  let param_annotations: BTreeMap<String, Vec<String>> = sig
    .arg_types
    .iter()
    .enumerate()
    .map(|(i, t)| {
      let name = format!("arg{i}");
      let type_str = t.describe();
      (name, vec![type_str])
    })
    .collect();

  let return_type_hints = vec![sig.return_type.describe()];

  let typed_count = param_annotations.values().filter(|v| !v.is_empty()).count();
  let level = if params.is_empty() || typed_count == params.len() {
    CoverageLevel::Full
  } else if typed_count > 0 {
    CoverageLevel::Partial
  } else {
    CoverageLevel::None
  };

  TypeCoverageRow {
    ns: calcit::calcit::CORE_NS.to_owned(),
    def: def_name.to_owned(),
    kind: DefKind::Proc,
    level,
    params,
    param_annotations,
    return_type_hints,
    data_type: None,
    schema_issues: vec![],
  }
}

/// Validate that a code entry matches its schema (kind, arity, rest param presence).
/// Returns a list of warning/error messages. Empty means no issues.
/// - `&runtime-implementation` = builtin proc/syntax → always skipped.
/// - Schema `:kind :fn`   → code must use `defn`.
/// - Schema `:kind :macro` → code must use `defmacro`.
/// - Schema `:args` length must match required param count in code.
/// - Schema `:rest` presence must match `&` rest param in code.
pub fn validate_def_vs_schema(ns: &str, def_name: &str, code: &Cirru, schema: &CalcitTypeAnnotation) -> Vec<String> {
  // builtin proc/syntax — skip structural checks
  if matches!(code, Cirru::Leaf(s) if s.as_ref() == "&runtime-implementation") {
    return vec![];
  }

  let CalcitTypeAnnotation::Fn(fn_annot) = schema else {
    // Non-Fn schema (Dynamic, etc.) has no structural constraints
    return vec![];
  };

  let Cirru::List(xs) = code else {
    return vec![];
  };

  let code_kind = match xs.first() {
    Some(Cirru::Leaf(s)) if s.as_ref() == "defn" => "defn",
    Some(Cirru::Leaf(s)) if s.as_ref() == "defmacro" => "defmacro",
    _ => return vec![], // not a defn/defmacro form — skip
  };

  let mut issues: Vec<String> = vec![];

  // Kind mismatch
  match (fn_annot.fn_kind, code_kind) {
    (SchemaKind::Fn, "defmacro") => {
      issues.push(format!("{ns}/{def_name}: schema :kind is :fn but code uses defmacro"));
    }
    (SchemaKind::Macro, "defn") => {
      issues.push(format!("{ns}/{def_name}: schema :kind is :macro but code uses defn"));
    }
    _ => {}
  }

  if code_kind == "defmacro" {
    return issues;
  }

  // Arity check
  let (required_count, has_rest) = analyze_param_arity(xs.get(2));
  let schema_required = fn_annot.arg_types.len();
  let schema_has_rest = fn_annot.rest_type.is_some();

  if required_count != schema_required {
    issues.push(format!(
      "{ns}/{def_name}: schema has {schema_required} required arg(s) but code has {required_count}"
    ));
  }
  if has_rest != schema_has_rest {
    if has_rest {
      issues.push(format!("{ns}/{def_name}: code has & rest param but schema has no :rest"));
    } else {
      issues.push(format!("{ns}/{def_name}: schema has :rest but code has no & param"));
    }
  }

  issues
}

/// Count required params and detect rest param from a defn/defmacro args form.
pub fn analyze_param_arity(args: Option<&Cirru>) -> (usize, bool) {
  let Some(Cirru::List(xs)) = args else {
    return (0, false);
  };
  let mut required = 0usize;
  let mut has_rest = false;
  let mut after_amp = false;
  for item in xs.iter() {
    match item {
      Cirru::Leaf(s) => {
        let s = s.as_ref();
        if s == "&" {
          after_amp = true;
        } else if s == "[]" || s == "," || s == "?" {
          // skip structural markers
        } else if after_amp {
          has_rest = true;
        } else if !s.starts_with(':') && !s.starts_with('|') && !s.chars().all(|c| c.is_ascii_digit()) {
          required += 1;
        }
      }
      Cirru::List(_) => {
        if !after_amp {
          required += 1;
        }
      }
    }
  }
  (required, has_rest)
}

pub fn analyze_code_entry(ns: &str, def_name: &str, entry: &snapshot::CodeEntry) -> TypeCoverageRow {
  // First check if this is a builtin proc in calcit.core
  if ns == calcit::calcit::CORE_NS {
    if let Ok(proc) = (*def_name).parse::<CalcitProc>() {
      if let Some(sig) = proc.get_type_signature() {
        return analyze_builtin_proc(def_name, sig);
      }
    }
    // Then check if this is a builtin syntax
    if let Ok(syntax) = (*def_name).parse::<CalcitSyntax>() {
      if let Some(sig) = syntax.get_type_signature() {
        return analyze_builtin_syntax(def_name, &sig);
      }
    }
  }

  let (kind, params, param_annotations, return_type_hints, data_type, level) = match &entry.code {
    Cirru::List(xs) => match xs.first() {
      Some(Cirru::Leaf(head)) if &**head == "defn" => {
        if let CalcitTypeAnnotation::Fn(fn_annot) = entry.schema.as_ref()
          && let Ok(schema) = snapshot::schema_edn_to_cirru(&fn_annot.to_schema_edn())
          && let Some((params, param_annotations, return_type_hints, level)) = extract_fn_schema_hints(&schema)
        {
          return TypeCoverageRow {
            ns: ns.to_owned(),
            def: def_name.to_owned(),
            kind: DefKind::Fn,
            level,
            params,
            param_annotations,
            return_type_hints,
            data_type: None,
            schema_issues: validate_def_vs_schema(ns, def_name, &entry.code, &entry.schema),
          };
        }
        if std::env::var("CR_DEBUG_SCHEMA").is_ok() {
          let schema_kind = match entry.schema.as_ref() {
            CalcitTypeAnnotation::Fn(fn_annot) => match snapshot::schema_edn_to_cirru(&fn_annot.to_schema_edn()) {
              Ok(schema) => match extract_fn_schema_hints(&schema) {
                Some(_) => "Fn/schema-hints-ok".to_owned(),
                None => "Fn/schema-hints-none".to_owned(),
              },
              Err(e) => format!("Fn/edn-to-cirru-err:{e}"),
            },
            other => format!("non-fn:{other:?}"),
          };
          eprintln!("[debug] {ns}/{def_name}: schema={schema_kind}");
        }

        let args = xs.get(2);
        let body = &xs[3..];
        let params = extract_param_symbols(args);
        let param_annotations = extract_assert_type_annotations(body);
        let return_type_hints = extract_return_type_hints(body);
        let typed_count = count_typed_params(&params, &param_annotations);
        let ret_typed = !return_type_hints.is_empty();
        let level = if ret_typed && (params.is_empty() || typed_count == params.len()) {
          CoverageLevel::Full
        } else if ret_typed || typed_count > 0 {
          CoverageLevel::Partial
        } else {
          CoverageLevel::None
        };
        (DefKind::Fn, params, param_annotations, return_type_hints, None, level)
      }
      Some(Cirru::Leaf(head)) if &**head == "defmacro" => {
        let args = xs.get(2);
        let body = &xs[3..];
        let params = extract_param_symbols(args);
        let param_annotations = extract_assert_type_annotations(body);
        (DefKind::Macro, params, param_annotations, Vec::new(), None, CoverageLevel::Full)
      }
      Some(Cirru::Leaf(head)) if &**head == "def" => {
        let inferred = xs.get(2).and_then(infer_data_type);
        let level = CoverageLevel::Full;
        (DefKind::Data, Vec::new(), BTreeMap::new(), Vec::new(), inferred, level)
      }
      _ => (DefKind::Other, Vec::new(), BTreeMap::new(), Vec::new(), None, CoverageLevel::Full),
    },
    _ => (DefKind::Other, Vec::new(), BTreeMap::new(), Vec::new(), None, CoverageLevel::Full),
  };

  TypeCoverageRow {
    ns: ns.to_owned(),
    def: def_name.to_owned(),
    kind,
    level,
    params,
    param_annotations,
    return_type_hints,
    data_type,
    schema_issues: validate_def_vs_schema(ns, def_name, &entry.code, &entry.schema),
  }
}

fn unwrap_optional_schema(schema: &Cirru) -> &Cirru {
  match schema {
    Cirru::List(items) => {
      if let Some(Cirru::Leaf(head)) = items.first() {
        if &**head == ":optional" && items.len() == 2 {
          return &items[1];
        }
        if &**head == "::" && items.len() == 3 && matches!(items.get(1), Some(Cirru::Leaf(tag)) if &**tag == ":optional") {
          return &items[2];
        }
      }
      schema
    }
    _ => schema,
  }
}

fn schema_to_map(schema: &Cirru) -> Option<BTreeMap<&str, &Cirru>> {
  let schema = unwrap_optional_schema(schema);
  let Cirru::List(items) = schema else {
    return None;
  };
  let Some(Cirru::Leaf(head)) = items.first() else {
    return None;
  };

  let mut data = BTreeMap::new();
  match &**head {
    "&{}" => {
      if (items.len() - 1) % 2 != 0 {
        return None;
      }
      for idx in (1..items.len()).step_by(2) {
        let key = match &items[idx] {
          Cirru::Leaf(s) if s.starts_with(':') => s.as_ref(),
          _ => return None,
        };
        data.insert(key, &items[idx + 1]);
      }
    }
    "{}" => {
      for pair in items.iter().skip(1) {
        let Cirru::List(xs) = pair else {
          return None;
        };
        if xs.len() != 2 {
          return None;
        }
        let key = match &xs[0] {
          Cirru::Leaf(s) if s.starts_with(':') => s.as_ref(),
          _ => return None,
        };
        data.insert(key, &xs[1]);
      }
    }
    _ => return None,
  }
  Some(data)
}

fn is_schema_list_annotation(node: &Cirru) -> bool {
  match node {
    Cirru::Leaf(s) => s.as_ref() == ":list",
    Cirru::List(xs) => {
      matches!(xs.first(), Some(Cirru::Leaf(head)) if &**head == "::")
        && matches!(xs.get(1), Some(Cirru::Leaf(tag)) if &**tag == ":list")
    }
  }
}

fn render_schema_param_type(ty_node: Option<&Cirru>, wrap_rest_as_list: bool) -> String {
  let Some(ty_node) = ty_node else {
    return ":dynamic".to_owned();
  };

  let rendered = render_cirru_inline(ty_node);
  if !wrap_rest_as_list || rendered == ":dynamic" || is_schema_list_annotation(ty_node) {
    rendered
  } else {
    format!(":: :list {rendered}")
  }
}

fn read_schema_param_tuple(item: &Cirru, default_name: &str, wrap_rest_as_list: bool) -> Option<(String, String)> {
  match item {
    Cirru::Leaf(_) => Some((default_name.to_owned(), render_schema_param_type(Some(item), wrap_rest_as_list))),
    Cirru::List(xs) => {
      let Some(Cirru::Leaf(head)) = xs.first() else {
        return None;
      };
      if &**head != "[]" && &**head != "::" {
        return None;
      }

      match xs.len() {
        2 => {
          let ty = render_schema_param_type(xs.get(1), wrap_rest_as_list);
          Some((default_name.to_owned(), ty))
        }
        3 => {
          let ty_node = match xs.get(1) {
            Some(Cirru::Leaf(name)) if name.starts_with('\'') => xs.get(2),
            _ => Some(item),
          };
          let ty = render_schema_param_type(ty_node, wrap_rest_as_list);
          Some((default_name.to_owned(), ty))
        }
        _ => None,
      }
    }
  }
}

type FnSchemaHints = (Vec<String>, BTreeMap<String, Vec<String>>, Vec<String>, CoverageLevel);

pub fn extract_fn_schema_hints(schema: &Cirru) -> Option<FnSchemaHints> {
  let schema = schema_to_map(schema)?;

  let mut params: Vec<String> = Vec::new();
  let mut param_annotations: BTreeMap<String, Vec<String>> = BTreeMap::new();

  if let Some(args_node) = schema.get(":args")
    && let Cirru::List(items) = args_node
    && matches!(items.first(), Some(Cirru::Leaf(head)) if &**head == "[]")
  {
    for (idx, item) in items.iter().skip(1).enumerate() {
      if let Some((name, ty)) = read_schema_param_tuple(item, &format!("arg{idx}"), false) {
        params.push(name.clone());
        param_annotations.entry(name).or_default().push(ty);
      }
    }
  }

  if let Some(rest_node) = schema.get(":rest")
    && let Some((name, ty)) = read_schema_param_tuple(rest_node, "rest", true)
  {
    params.push(name.clone());
    param_annotations.entry(name).or_default().push(ty);
  }

  let return_type_hints = vec![
    schema
      .get(":return")
      .map_or_else(|| ":dynamic".to_owned(), |v| render_cirru_inline(v)),
  ];

  let typed_count = params
    .iter()
    .filter(|name| {
      param_annotations
        .get(*name)
        .is_some_and(|hints| hints.iter().any(|hint| hint != ":dynamic"))
    })
    .count();

  let ret_typed = return_type_hints.iter().any(|hint| hint != ":dynamic");
  let level = if ret_typed && (params.is_empty() || typed_count == params.len()) {
    CoverageLevel::Full
  } else if ret_typed || typed_count > 0 {
    CoverageLevel::Partial
  } else {
    CoverageLevel::None
  };

  Some((params, param_annotations, return_type_hints, level))
}

fn extract_param_symbols(args: Option<&Cirru>) -> Vec<String> {
  let mut out: Vec<String> = vec![];
  if let Some(node) = args {
    collect_param_symbols(node, &mut out);
  }
  dedup_keep_order(out)
}

fn collect_param_symbols(node: &Cirru, out: &mut Vec<String>) {
  match node {
    Cirru::Leaf(s) => {
      let name = s.as_ref();
      if name == "&" || name == "?" || name == "[]" || name == "," {
        return;
      }
      if name.starts_with('|') || name.starts_with(':') || name.chars().all(|c| c.is_ascii_digit()) {
        return;
      }
      out.push(name.to_string());
    }
    Cirru::List(xs) => {
      for x in xs {
        collect_param_symbols(x, out);
      }
    }
  }
}

fn extract_assert_type_annotations(nodes: &[Cirru]) -> BTreeMap<String, Vec<String>> {
  let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
  for node in nodes {
    collect_assert_type_annotations(node, &mut out);
  }

  for items in out.values_mut() {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    items.retain(|v| seen.insert(v.to_owned()));
  }

  out
}

fn collect_assert_type_annotations(node: &Cirru, out: &mut BTreeMap<String, Vec<String>>) {
  match node {
    Cirru::Leaf(_) => {}
    Cirru::List(xs) => {
      if let Some(Cirru::Leaf(head)) = xs.first()
        && &**head == "assert-type"
        && let Some(Cirru::Leaf(symbol)) = xs.get(1)
        && let Some(ty_node) = xs.get(2)
      {
        out.entry(symbol.to_string()).or_default().push(render_cirru_inline(ty_node));
      }

      for x in xs {
        collect_assert_type_annotations(x, out);
      }
    }
  }
}

fn extract_return_type_hints(nodes: &[Cirru]) -> Vec<String> {
  let mut out: Vec<String> = Vec::new();
  for node in nodes {
    collect_return_type_hints(node, &mut out);
  }

  let mut seen: BTreeSet<String> = BTreeSet::new();
  out.retain(|v| seen.insert(v.to_owned()));
  out
}

fn collect_return_type_hints(node: &Cirru, out: &mut Vec<String>) {
  match node {
    Cirru::Leaf(_) => {}
    Cirru::List(xs) => {
      if let Some(Cirru::Leaf(head)) = xs.first()
        && &**head == "return-type"
        && let Some(ty_node) = xs.get(1)
      {
        out.push(render_cirru_inline(ty_node));
      }

      for x in xs {
        collect_return_type_hints(x, out);
      }
    }
  }
}

pub fn count_typed_params(params: &[String], annotations: &BTreeMap<String, Vec<String>>) -> usize {
  params
    .iter()
    .filter(|name| annotations.get(*name).is_some_and(|items| !items.is_empty()))
    .count()
}

fn dedup_keep_order(items: Vec<String>) -> Vec<String> {
  let mut seen: BTreeSet<String> = BTreeSet::new();
  let mut out: Vec<String> = Vec::new();
  for item in items {
    if seen.insert(item.to_owned()) {
      out.push(item);
    }
  }
  out
}

fn render_cirru_inline(node: &Cirru) -> String {
  match node {
    Cirru::Leaf(s) => s.to_string(),
    Cirru::List(xs) => {
      let parts = xs.iter().map(render_cirru_inline).collect::<Vec<_>>().join(" ");
      format!("({parts})")
    }
  }
}

pub fn parse_coverage_levels(raw: &str) -> Result<BTreeSet<CoverageLevel>, String> {
  let mut selected: BTreeSet<CoverageLevel> = BTreeSet::new();

  for part in raw.split(',') {
    let token = part.trim().to_ascii_lowercase();
    if token.is_empty() {
      continue;
    }

    match token.as_str() {
      "none" => {
        selected.insert(CoverageLevel::None);
      }
      "partial" => {
        selected.insert(CoverageLevel::Partial);
      }
      "full" => {
        selected.insert(CoverageLevel::Full);
      }
      _ => {
        return Err(format!(
          "Unknown coverage level `{token}` in --only. Expected comma-separated values from: none,partial,full"
        ));
      }
    }
  }

  if selected.is_empty() {
    return Err("`--only` is empty. Use one or more of: none,partial,full".to_string());
  }

  Ok(selected)
}

fn infer_data_type(node: &Cirru) -> Option<String> {
  match node {
    Cirru::Leaf(s) => {
      let raw = s.as_ref();
      if raw == "nil" {
        Some("nil".to_string())
      } else if raw == "true" || raw == "false" {
        Some("bool".to_string())
      } else if raw.starts_with('|') {
        Some("string".to_string())
      } else if raw.starts_with(':') {
        Some("tag".to_string())
      } else if raw.parse::<f64>().is_ok() {
        Some("number".to_string())
      } else {
        None
      }
    }
    Cirru::List(xs) => match xs.first() {
      Some(Cirru::Leaf(head)) if &**head == "[]" => Some("list".to_string()),
      Some(Cirru::Leaf(head)) if &**head == "{}" || &**head == "&{}" => Some("map".to_string()),
      Some(Cirru::Leaf(head)) if &**head == "#{}" => Some("set".to_string()),
      Some(Cirru::Leaf(head)) if &**head == "::" => Some("tuple".to_string()),
      Some(Cirru::Leaf(head)) if &**head == "defn" || &**head == "fn" => Some("fn".to_string()),
      Some(Cirru::Leaf(head)) if &**head == "defmacro" => Some("macro".to_string()),
      _ => None,
    },
  }
}

pub fn format_check_types(options: &CheckTypesCommand, snapshot: &snapshot::Snapshot) -> Result<String, String> {
  let mut out = String::new();
  run_check_types_report(options, snapshot, &mut out)?;
  Ok(out)
}

pub fn format_weak_types(options: &WeakTypesCommand, snapshot: &snapshot::Snapshot) -> Result<String, String> {
  let mut out = String::new();
  run_weak_types_report(options, snapshot, &mut out)?;
  Ok(out)
}
