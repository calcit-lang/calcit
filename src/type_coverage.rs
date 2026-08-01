//! Type coverage and weak-type analysis for `cr analyze`.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use calcit::calcit::{CalcitProc, CalcitSyntax, CalcitTypeAnnotation, ProcTypeSignature, SchemaKind, SyntaxTypeSignature};
use calcit::cli_args::{CheckTypesCommand, WeakTypesCommand};
use calcit::snapshot;
use cirru_parser::Cirru;
use md5::{Digest, Md5};

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
  pub fn as_str(self) -> &'static str {
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
  pub fn as_str(self) -> &'static str {
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
  pub generics: Vec<String>,
  pub where_bounds: Vec<String>,
  pub data_type: Option<String>,
  pub schema_issues: Vec<String>,
}

fn fn_polymorphism(fn_annot: &calcit::calcit::CalcitFnTypeAnnotation) -> (Vec<String>, Vec<String>) {
  let generics = fn_annot.generics.iter().map(|name| format!("'{name}")).collect();
  let where_bounds = fn_annot.where_bounds.iter().map(|bound| bound.to_brief_string()).collect();
  (generics, where_bounds)
}

fn unwrap_singleton_group(mut node: &Cirru) -> &Cirru {
  while let Cirru::List(items) = node
    && items.len() == 1
    && matches!(items.first(), Some(Cirru::List(_)))
  {
    node = &items[0];
  }
  node
}

fn entry_polymorphism(entry: &snapshot::CodeEntry) -> (Vec<String>, Vec<String>) {
  if let CalcitTypeAnnotation::Fn(fn_annot) = entry.schema.as_ref() {
    return fn_polymorphism(fn_annot);
  }

  let Cirru::List(items) = &entry.code else {
    return (vec![], vec![]);
  };
  if !matches!(items.first(), Some(Cirru::Leaf(head)) if matches!(head.as_ref(), "defstruct" | "defenum")) {
    return (vec![], vec![]);
  }

  let generics = match items.get(2) {
    Some(Cirru::List(vars)) if vars.iter().all(|item| matches!(item, Cirru::Leaf(name) if name.starts_with('\''))) => vars
      .iter()
      .filter_map(|item| match item {
        Cirru::Leaf(name) => Some(name.to_string()),
        _ => None,
      })
      .collect(),
    _ => vec![],
  };
  let where_bounds = items
    .iter()
    .skip(3)
    .find_map(|item| match unwrap_singleton_group(item) {
      Cirru::List(parts) if matches!(parts.first(), Some(Cirru::Leaf(head)) if head.as_ref() == "{}") => {
        Some(parts.iter().skip(1).map(render_cirru_inline).collect::<Vec<_>>())
      }
      _ => None,
    })
    .unwrap_or_default();
  (generics, where_bounds)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WeakTypeKind {
  SchemaDynamic,
  CodeDynamic,
  CodeNil,
}

impl WeakTypeKind {
  pub fn as_str(&self) -> &'static str {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WeakTypeIntent {
  Unresolved,
  IntentionalJsFfi,
}

impl WeakTypeIntent {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Unresolved => "unresolved",
      Self::IntentionalJsFfi => "intentional-js-ffi",
    }
  }

  pub fn all() -> BTreeSet<Self> {
    BTreeSet::from([Self::Unresolved, Self::IntentionalJsFfi])
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeakTypeOccurrence {
  pub kind: WeakTypeKind,
  pub intent: WeakTypeIntent,
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

pub fn parse_weak_type_intents(raw: &str) -> Result<BTreeSet<WeakTypeIntent>, String> {
  let mut selected = BTreeSet::new();

  for item in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
    let intent = match item {
      "unresolved" => WeakTypeIntent::Unresolved,
      "intentional-js-ffi" => WeakTypeIntent::IntentionalJsFfi,
      other => {
        return Err(format!(
          "Unknown weak-type intent `{other}`. Expected comma-separated values from: unresolved, intentional-js-ffi"
        ));
      }
    };
    selected.insert(intent);
  }

  if selected.is_empty() {
    return Err("Weak-type intent filter cannot be empty. Use comma-separated values from: unresolved, intentional-js-ffi".to_owned());
  }

  Ok(selected)
}

fn format_cirru_path(root: &str, path: &[usize]) -> String {
  if path.is_empty() {
    root.to_owned()
  } else {
    let mut rendered = format!("{root}@");
    for (i, idx) in path.iter().enumerate() {
      if i > 0 {
        rendered.push('.');
      }
      rendered.push_str(&idx.to_string());
    }
    rendered
  }
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
    intent: WeakTypeIntent::Unresolved,
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

fn weak_type_suggestion(occurrence: &WeakTypeOccurrence) -> &'static str {
  if occurrence.intent == WeakTypeIntent::IntentionalJsFfi {
    return "Keep the dynamic value isolated at the declared JS FFI boundary and validate or convert it before typed code consumes it.";
  }

  if occurrence.kind == WeakTypeKind::CodeNil {
    return "Use `:: :optional <type>` when nil is part of the contract; otherwise remove the nil-producing branch.";
  }
  if occurrence.detail.contains("legacy-any") {
    return "Migrate legacy `:any` to canonical `:dynamic`, then narrow it with a concrete type, a declared type variable, or a named enum when the value participates in typed code.";
  }
  if occurrence.kind == WeakTypeKind::CodeDynamic {
    return "Replace this `:dynamic` slot with a concrete type; use a declared type variable when input and output types are related, or a trait plus `:where` when only a capability is required. Keep dynamic only at a documented boundary.";
  }
  if occurrence.detail.contains("ref-item") {
    return "Replace bare `:ref` with `:: :ref <value-type>`; use a declared type variable for a polymorphic ref, or a named enum for intentionally heterogeneous state.";
  }
  if occurrence.detail.contains("list-item") {
    return "Replace bare `:list` with `:: :list <item-type>`; use a declared type variable for a homogeneous polymorphic list, or a named enum for heterogeneous items.";
  }
  if occurrence.detail.contains("set-item") {
    return "Replace bare `:set` with `:: :set <item-type>` and choose a concrete type or a declared type variable.";
  }
  if occurrence.detail.contains("map-key") || occurrence.detail.contains("map-value") {
    return "Use `:: :map <key-type> <value-type>` and replace each unresolved slot with a concrete type or a declared type variable.";
  }
  "Replace `:dynamic` with a concrete schema, a declared type variable, or a trait-bounded variable; mark `:features $ #{} :js-ffi` only for an intentional JS FFI boundary."
}

fn weak_type_impact(occurrence: &WeakTypeOccurrence) -> &'static str {
  if occurrence.intent == WeakTypeIntent::IntentionalJsFfi {
    return "The value stays dynamic at an explicit boundary; typed callers must validate or convert it before relying on methods or generic relations.";
  }
  if occurrence.kind == WeakTypeKind::CodeNil {
    return "Implicit nil weakens branch and return inference unless the surrounding contract is explicitly optional.";
  }
  if occurrence.detail.contains("fn-arg") || occurrence.detail.contains("fn-return") {
    return "The callback contract loses parameter/return checking and prevents reliable variance or generic substitution.";
  }
  if occurrence.detail.contains("list-item")
    || occurrence.detail.contains("set-item")
    || occurrence.detail.contains("map-key")
    || occurrence.detail.contains("map-value")
    || occurrence.detail.contains("ref-item")
    || occurrence.detail.contains("type-arg")
  {
    return "The container or applied type loses its element relationship, so downstream generic inference and method specialization may fall back to runtime dispatch.";
  }
  "The dynamic slot erases type relations at this boundary, reducing call checking, generic binding, and compile-time method specialization."
}

fn entry_schema_issues(ns: &str, def_name: &str, code: &Cirru, annotation: &CalcitTypeAnnotation) -> Vec<String> {
  let mut issues = validate_def_vs_schema(ns, def_name, code, annotation);
  let has_js_ffi_feature = matches!(
    annotation,
    CalcitTypeAnnotation::Fn(fn_annot) if fn_annot.features.iter().any(|feature| feature.ref_str() == "js-ffi")
  );
  if has_js_ffi_feature || matches!(annotation, CalcitTypeAnnotation::Dynamic) {
    return issues;
  }

  let mut occurrences = vec![];
  scan_schema_dynamic_annotation(annotation, "schema", "root", &mut occurrences);
  for occurrence in occurrences {
    issues.push(format!(
      "[W_SCHEMA_DYNAMIC] {} is unresolved ({}). Fix: {}",
      occurrence.path,
      occurrence.detail,
      weak_type_suggestion(&occurrence)
    ));
  }
  issues
}

fn downgrade_coverage_for_dynamic_annotation(level: CoverageLevel, annotation: &CalcitTypeAnnotation) -> CoverageLevel {
  if level != CoverageLevel::Full {
    return level;
  }
  let mut occurrences = vec![];
  scan_schema_dynamic_annotation(annotation, "schema", "root", &mut occurrences);
  if occurrences.is_empty() {
    CoverageLevel::Full
  } else {
    CoverageLevel::Partial
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

fn code_declares_embedded_type_schema(code: &Cirru) -> bool {
  matches!(
    code,
    Cirru::List(items)
      if matches!(items.first(), Some(Cirru::Leaf(head)) if matches!(head.as_ref(), "defstruct" | "defenum" | "deftrait" | "defimpl"))
  )
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
      let is_dynamic = matches!(text.as_ref(), ":dynamic" | ":any");
      let detail_prefix = if text.as_ref() == ":any" { "legacy-any" } else { "raw-schema" };
      if is_dynamic && root == "schema" && selected.contains(&WeakTypeKind::SchemaDynamic) {
        push_weak_type_occurrence(
          occurrences,
          WeakTypeKind::SchemaDynamic,
          weak_type_detail(WeakTypeKind::SchemaDynamic, detail_prefix),
          format_cirru_path(root, path),
        );
      } else if is_dynamic && root == "code" && selected.contains(&WeakTypeKind::CodeDynamic) {
        let detail = if text.as_ref() == ":any" {
          format!("legacy-any:{}", classify_code_dynamic(parent))
        } else {
          classify_code_dynamic(parent).to_owned()
        };
        push_weak_type_occurrence(
          occurrences,
          WeakTypeKind::CodeDynamic,
          weak_type_detail(WeakTypeKind::CodeDynamic, &detail),
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

  if matches!(entry.schema.as_ref(), CalcitTypeAnnotation::Dynamic) {
    if selected.contains(&WeakTypeKind::SchemaDynamic) && !code_declares_embedded_type_schema(&entry.code) {
      push_weak_type_occurrence(
        &mut occurrences,
        WeakTypeKind::SchemaDynamic,
        weak_type_detail(WeakTypeKind::SchemaDynamic, "root"),
        "schema".to_owned(),
      );
    }
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

  let has_js_ffi_feature = matches!(
    entry.schema.as_ref(),
    CalcitTypeAnnotation::Fn(fn_annot) if fn_annot.features.iter().any(|feature| feature.ref_str() == "js-ffi")
  );
  if has_js_ffi_feature {
    for occurrence in &mut occurrences {
      if matches!(occurrence.kind, WeakTypeKind::SchemaDynamic | WeakTypeKind::CodeDynamic) {
        occurrence.intent = WeakTypeIntent::IntentionalJsFfi;
      }
    }
  }

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

#[derive(Debug, Clone, Copy)]
struct AnalysisScope<'a> {
  namespace: Option<&'a str>,
  namespace_prefix: Option<&'a str>,
  include_dependencies: bool,
}

fn visit_scoped_definitions<F>(snapshot: &snapshot::Snapshot, scope: AnalysisScope<'_>, mut visit: F) -> Result<(), String>
where
  F: FnMut(&str, &str, &snapshot::CodeEntry),
{
  if let Some(namespace) = scope.namespace
    && !snapshot.files.contains_key(namespace)
  {
    return Err(format!("Namespace not found: {namespace}"));
  }

  let package = snapshot.package.as_str();
  let package_prefix = format!("{package}.");
  let explicit_scope = scope.namespace.is_some() || scope.namespace_prefix.is_some();
  for (namespace, file) in &snapshot.files {
    if !explicit_scope && namespace.ends_with(".$meta") {
      continue;
    }
    if scope.namespace.is_some_and(|exact| namespace != exact) {
      continue;
    }
    if scope.namespace_prefix.is_some_and(|prefix| !namespace.starts_with(prefix)) {
      continue;
    }
    if !(scope.include_dependencies || explicit_scope || namespace == package || namespace.starts_with(&package_prefix)) {
      continue;
    }
    for (definition, entry) in &file.defs {
      visit(namespace, definition, entry);
    }
  }
  Ok(())
}

pub fn collect_weak_type_rows(options: &WeakTypesCommand, snapshot: &snapshot::Snapshot) -> Result<Vec<WeakTypeRow>, String> {
  let selected = options
    .only
    .as_deref()
    .map(parse_weak_type_kinds)
    .transpose()?
    .unwrap_or_else(WeakTypeKind::all);
  let selected_intents = options
    .intent
    .as_deref()
    .map(parse_weak_type_intents)
    .transpose()?
    .unwrap_or_else(WeakTypeIntent::all);

  let mut rows: Vec<WeakTypeRow> = vec![];

  visit_scoped_definitions(
    snapshot,
    AnalysisScope {
      namespace: options.ns.as_deref(),
      namespace_prefix: options.ns_prefix.as_deref(),
      include_dependencies: options.deps,
    },
    |namespace, definition, entry| {
      if let Some(mut row) = analyze_weak_types_entry(namespace, definition, entry, &selected) {
        row.occurrences.retain(|occurrence| selected_intents.contains(&occurrence.intent));
        if !row.occurrences.is_empty() {
          rows.push(row);
        }
      }
    },
  )?;

  rows.sort_by(|a, b| a.ns.cmp(&b.ns).then(a.def.cmp(&b.def)));
  Ok(rows)
}

pub fn run_weak_types_report(options: &WeakTypesCommand, snapshot: &snapshot::Snapshot, out: &mut String) -> Result<(), String> {
  let rows = collect_weak_type_rows(options, snapshot)?;

  if rows.is_empty() {
    let _ = writeln!(out, "No weak type usage found in selected namespace scope.");
    return Ok(());
  }

  let mut kind_count: BTreeMap<&'static str, usize> = BTreeMap::new();
  let mut intent_count: BTreeMap<&'static str, usize> = BTreeMap::new();
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
      *intent_count.entry(occurrence.intent.as_str()).or_insert(0) += 1;
      if options.summary_only {
        continue;
      }
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
  if let Some(raw) = &options.intent {
    let _ = writeln!(out, "- intent-filter: {raw}");
  }
  let _ = writeln!(
    out,
    "- hits: schema-dynamic={} code-dynamic={} code-nil={}",
    kind_count.get("schema-dynamic").copied().unwrap_or(0),
    kind_count.get("code-dynamic").copied().unwrap_or(0),
    kind_count.get("code-nil").copied().unwrap_or(0)
  );
  let _ = writeln!(
    out,
    "- intents: unresolved={} intentional-js-ffi={}",
    intent_count.get("unresolved").copied().unwrap_or(0),
    intent_count.get("intentional-js-ffi").copied().unwrap_or(0)
  );
  let unresolved_dynamic = rows
    .iter()
    .flat_map(|row| row.occurrences.iter())
    .filter(|occurrence| {
      occurrence.intent == WeakTypeIntent::Unresolved
        && matches!(occurrence.kind, WeakTypeKind::SchemaDynamic | WeakTypeKind::CodeDynamic)
    })
    .count();
  if unresolved_dynamic > 0 {
    let _ = writeln!(
      out,
      "- agent-note: {unresolved_dynamic} unresolved dynamic slot(s) can erase generic relations, callback checks, and compile-time method specialization."
    );
    let _ = writeln!(
      out,
      "- next: rerun without `--summary-only`; prefer concrete types, `:generics` type variables, or trait `:where` bounds before retaining a documented dynamic boundary."
    );
  }
  if options.summary_only {
    return Ok(());
  }
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
        "  - {} [{}] ({}) @ {}",
        occurrence.kind.as_str(),
        occurrence.intent.as_str(),
        occurrence.detail,
        occurrence.path
      );
      let _ = writeln!(out, "    impact: {}", weak_type_impact(occurrence));
      let _ = writeln!(out, "    fix: {}", weak_type_suggestion(occurrence));
    }
    let _ = writeln!(out,);
  }

  Ok(())
}

pub fn collect_type_coverage_rows(options: &CheckTypesCommand, snapshot: &snapshot::Snapshot) -> Result<Vec<TypeCoverageRow>, String> {
  let mut rows: Vec<TypeCoverageRow> = Vec::new();
  visit_scoped_definitions(
    snapshot,
    AnalysisScope {
      namespace: options.ns.as_deref(),
      namespace_prefix: options.ns_prefix.as_deref(),
      include_dependencies: options.deps,
    },
    |namespace, definition, entry| rows.push(analyze_code_entry(namespace, definition, entry)),
  )?;

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
  Ok(rows)
}

pub fn run_check_types_report(options: &CheckTypesCommand, snapshot: &snapshot::Snapshot, out: &mut String) -> Result<(), String> {
  let rows = collect_type_coverage_rows(options, snapshot)?;

  if rows.is_empty() {
    let _ = writeln!(out, "No definitions found in selected namespace scope.");
    return Ok(());
  }

  let mut level_count: BTreeMap<&'static str, usize> = BTreeMap::new();
  let mut kind_count: BTreeMap<&'static str, usize> = BTreeMap::new();
  let mut ns_set: BTreeSet<String> = BTreeSet::new();
  let mut polymorphic_defs = 0usize;
  let mut bounded_polymorphic_defs = 0usize;

  for row in &rows {
    *level_count.entry(row.level.as_str()).or_insert(0) += 1;
    *kind_count.entry(row.kind.as_str()).or_insert(0) += 1;
    ns_set.insert(row.ns.clone());
    if !row.generics.is_empty() {
      polymorphic_defs += 1;
      if !row.where_bounds.is_empty() {
        bounded_polymorphic_defs += 1;
      }
    }
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
  let _ = writeln!(
    out,
    "- polymorphism: generic={polymorphic_defs} trait-bounded={bounded_polymorphic_defs}"
  );
  let coverage_gaps = level_count.get("partial").copied().unwrap_or(0) + level_count.get("none").copied().unwrap_or(0);
  if coverage_gaps > 0 {
    let _ = writeln!(
      out,
      "- agent-note: {coverage_gaps} definition(s) lack full static coverage; unresolved dynamic slots can hide parametric relationships and force runtime method dispatch."
    );
    let _ = writeln!(
      out,
      "- next: `cr analyze weak-types --only schema-dynamic,code-dynamic --intent unresolved --summary-only`, then scope the reported namespaces and rerun without `--summary-only`."
    );
  }
  if options.summary_only {
    return Ok(());
  }
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
    if !row.generics.is_empty() {
      let _ = writeln!(out, "  generics: {}", row.generics.join(", "));
    }
    if !row.where_bounds.is_empty() {
      let _ = writeln!(out, "  where:");
      for bound in &row.where_bounds {
        let _ = writeln!(out, "    - {bound}");
      }
    }

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
    generics: vec![],
    where_bounds: vec![],
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
    generics: vec![],
    where_bounds: vec![],
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
    if let Ok(proc) = (*def_name).parse::<CalcitProc>()
      && let Some(sig) = proc.get_type_signature()
    {
      return analyze_builtin_proc(def_name, sig);
    }
    // Then check if this is a builtin syntax
    if let Ok(syntax) = (*def_name).parse::<CalcitSyntax>()
      && let Some(sig) = syntax.get_type_signature()
    {
      return analyze_builtin_syntax(def_name, &sig);
    }
  }

  // Function schemas are the canonical source for top-level callable coverage.
  // Definition payloads may use `fn` values instead of `defn`, so relying on
  // the source head alone incorrectly classifies typed functions as `other`.
  if let CalcitTypeAnnotation::Fn(fn_annot) = entry.schema.as_ref()
    && let Ok(schema) = snapshot::schema_edn_to_cirru(&fn_annot.to_schema_edn())
    && let Some((params, param_annotations, return_type_hints, level)) = extract_fn_schema_hints(&schema)
  {
    let level = downgrade_coverage_for_dynamic_annotation(level, entry.schema.as_ref());
    let (generics, where_bounds) = fn_polymorphism(fn_annot);
    return TypeCoverageRow {
      ns: ns.to_owned(),
      def: def_name.to_owned(),
      kind: match fn_annot.fn_kind {
        SchemaKind::Fn => DefKind::Fn,
        SchemaKind::Macro => DefKind::Macro,
      },
      level,
      params,
      param_annotations,
      return_type_hints,
      generics,
      where_bounds,
      data_type: None,
      schema_issues: entry_schema_issues(ns, def_name, &entry.code, &entry.schema),
    };
  }

  fn type_form_contains_dynamic(form: &Cirru) -> bool {
    match form {
      Cirru::Leaf(value) => matches!(value.as_ref(), ":dynamic" | ":any"),
      Cirru::List(items) => items.iter().any(type_form_contains_dynamic),
    }
  }

  fn embedded_type_declaration_coverage(head: &str, items: &[Cirru]) -> CoverageLevel {
    let mut typed_slots = 0usize;
    let mut dynamic_slots = 0usize;

    fn entry_parts(node: &Cirru, prefix: char) -> Option<&[Cirru]> {
      match unwrap_singleton_group(node) {
        Cirru::List(parts) if matches!(parts.first(), Some(Cirru::Leaf(name)) if name.starts_with(prefix)) => Some(parts.as_slice()),
        _ => None,
      }
    }

    match head {
      "defstruct" => {
        for field in items.iter().skip(2) {
          let Some(parts) = entry_parts(field, ':') else {
            continue;
          };
          let field_type = parts.get(1);
          match field_type {
            Some(form) if !type_form_contains_dynamic(form) => typed_slots += 1,
            _ => dynamic_slots += 1,
          }
        }
      }
      "defenum" => {
        for variant in items.iter().skip(2) {
          let Some(parts) = entry_parts(variant, ':') else {
            continue;
          };
          for payload_type in parts.iter().skip(1) {
            if type_form_contains_dynamic(payload_type) {
              dynamic_slots += 1;
            } else {
              typed_slots += 1;
            }
          }
        }
      }
      "deftrait" => {
        for method in items.iter().skip(2) {
          if entry_parts(method, '.').is_none() {
            continue;
          }
          if type_form_contains_dynamic(method) {
            dynamic_slots += 1;
          } else {
            typed_slots += 1;
          }
        }
      }
      "defimpl" => {
        for method in items.iter().skip(3) {
          if entry_parts(method, '.').is_none() {
            continue;
          }
          if type_form_contains_dynamic(method) {
            dynamic_slots += 1;
          } else {
            typed_slots += 1;
          }
        }
      }
      _ => {}
    }

    if dynamic_slots == 0 {
      CoverageLevel::Full
    } else if typed_slots > 0 || matches!(head, "defstruct" | "defenum") {
      CoverageLevel::Partial
    } else {
      CoverageLevel::None
    }
  }

  fn explicit_data_schema(annotation: &CalcitTypeAnnotation) -> Option<(String, CoverageLevel)> {
    if matches!(annotation, CalcitTypeAnnotation::Dynamic) {
      return None;
    }
    let description = annotation.describe();
    if matches!(description.as_str(), "dynamic" | "unknown") {
      return None;
    }
    let mut dynamic_parts = vec![];
    scan_schema_dynamic_annotation(annotation, "schema", "root", &mut dynamic_parts);
    let level = if dynamic_parts.is_empty() {
      CoverageLevel::Full
    } else {
      CoverageLevel::Partial
    };
    Some((description, level))
  }

  let (kind, params, param_annotations, return_type_hints, data_type, level) = match &entry.code {
    Cirru::List(xs) => match xs.first() {
      Some(Cirru::Leaf(head)) if matches!(head.as_ref(), "defstruct" | "defenum" | "deftrait" | "defimpl") => (
        DefKind::Data,
        Vec::new(),
        BTreeMap::new(),
        Vec::new(),
        Some(head.trim_start_matches("def").to_owned()),
        embedded_type_declaration_coverage(head, xs),
      ),
      Some(Cirru::Leaf(head)) if &**head == "defn" => {
        if let CalcitTypeAnnotation::Fn(fn_annot) = entry.schema.as_ref()
          && let Ok(schema) = snapshot::schema_edn_to_cirru(&fn_annot.to_schema_edn())
          && let Some((params, param_annotations, return_type_hints, level)) = extract_fn_schema_hints(&schema)
        {
          let level = downgrade_coverage_for_dynamic_annotation(level, entry.schema.as_ref());
          let (generics, where_bounds) = fn_polymorphism(fn_annot);
          return TypeCoverageRow {
            ns: ns.to_owned(),
            def: def_name.to_owned(),
            kind: DefKind::Fn,
            level,
            params,
            param_annotations,
            return_type_hints,
            generics,
            where_bounds,
            data_type: None,
            schema_issues: entry_schema_issues(ns, def_name, &entry.code, &entry.schema),
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
        let explicit = explicit_data_schema(entry.schema.as_ref());
        let data_type = inferred.or_else(|| explicit.as_ref().map(|(data_type, _)| data_type.clone()));
        let level = explicit.map(|(_, level)| level).unwrap_or(if data_type.is_some() {
          CoverageLevel::Full
        } else {
          CoverageLevel::None
        });
        (DefKind::Data, Vec::new(), BTreeMap::new(), Vec::new(), data_type, level)
      }
      Some(Cirru::Leaf(head)) if head.as_ref() == "defatom" => {
        let explicit = explicit_data_schema(entry.schema.as_ref());
        let (data_type, level) = explicit.unwrap_or_else(|| ("ref<dynamic>".to_owned(), CoverageLevel::Partial));
        (DefKind::Data, Vec::new(), BTreeMap::new(), Vec::new(), Some(data_type), level)
      }
      _ => match explicit_data_schema(entry.schema.as_ref()) {
        Some((data_type, level)) => (DefKind::Data, Vec::new(), BTreeMap::new(), Vec::new(), Some(data_type), level),
        None => (DefKind::Other, Vec::new(), BTreeMap::new(), Vec::new(), None, CoverageLevel::None),
      },
    },
    _ => match explicit_data_schema(entry.schema.as_ref()) {
      Some((data_type, level)) => (DefKind::Data, Vec::new(), BTreeMap::new(), Vec::new(), Some(data_type), level),
      None => (DefKind::Other, Vec::new(), BTreeMap::new(), Vec::new(), None, CoverageLevel::None),
    },
  };
  let (generics, where_bounds) = entry_polymorphism(entry);

  TypeCoverageRow {
    ns: ns.to_owned(),
    def: def_name.to_owned(),
    kind,
    level,
    params,
    param_annotations,
    return_type_hints,
    generics,
    where_bounds,
    data_type,
    schema_issues: entry_schema_issues(ns, def_name, &entry.code, &entry.schema),
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

fn analysis_revision(snapshot: &snapshot::Snapshot, definitions: &[(String, String)]) -> Result<String, String> {
  let mut ids = definitions.to_vec();
  ids.sort();
  ids.dedup();
  let mut hasher = Md5::new();
  for (namespace, definition) in ids {
    let entry = snapshot
      .files
      .get(&namespace)
      .and_then(|file| file.defs.get(&definition))
      .ok_or_else(|| format!("Definition disappeared while computing analysis revision: {namespace}/{definition}"))?;
    let revision = snapshot::definition_revision(entry)?;
    let id = format!("{namespace}/{definition}");
    hasher.update((id.len() as u64).to_le_bytes());
    hasher.update(id.as_bytes());
    hasher.update(revision.as_bytes());
  }
  Ok(format!("md5:{:x}", hasher.finalize()))
}

pub fn format_check_types_json(options: &CheckTypesCommand, snapshot: &snapshot::Snapshot) -> Result<String, String> {
  let rows = collect_type_coverage_rows(options, snapshot)?;
  let mut levels = BTreeMap::<&str, usize>::new();
  let mut kinds = BTreeMap::<&str, usize>::new();
  let mut namespaces = BTreeSet::<&str>::new();
  let mut polymorphic_defs = 0usize;
  let mut bounded_polymorphic_defs = 0usize;
  for row in &rows {
    *levels.entry(row.level.as_str()).or_insert(0) += 1;
    *kinds.entry(row.kind.as_str()).or_insert(0) += 1;
    namespaces.insert(row.ns.as_str());
    if !row.generics.is_empty() {
      polymorphic_defs += 1;
      if !row.where_bounds.is_empty() {
        bounded_polymorphic_defs += 1;
      }
    }
  }
  for level in ["none", "partial", "full"] {
    levels.entry(level).or_insert(0);
  }
  for kind in ["fn", "macro", "proc", "syntax", "data", "other"] {
    kinds.entry(kind).or_insert(0);
  }

  let definitions = rows
    .iter()
    .filter(|_| !options.summary_only)
    .map(|row| {
      let parameters = row
        .params
        .iter()
        .map(|name| {
          serde_json::json!({
            "name": name,
            "types": row.param_annotations.get(name).cloned().unwrap_or_default(),
          })
        })
        .collect::<Vec<_>>();
      serde_json::json!({
        "id": format!("{}/{}", row.ns, row.def),
        "namespace": row.ns,
        "name": row.def,
        "kind": row.kind.as_str(),
        "coverage": row.level.as_str(),
        "parameters": parameters,
        "return_types": row.return_type_hints,
        "generics": row.generics,
        "where_bounds": row.where_bounds,
        "data_type": row.data_type,
        "schema_issues": row.schema_issues,
      })
    })
    .collect::<Vec<_>>();
  let coverage_gaps = levels.get("partial").copied().unwrap_or(0) + levels.get("none").copied().unwrap_or(0);
  let diagnostics = if coverage_gaps == 0 {
    vec![]
  } else {
    vec![serde_json::json!({
      "code": "W_TYPE_COVERAGE_GAPS",
      "phase": "analysis",
      "severity": "warning",
      "message": format!("{coverage_gaps} definition(s) lack full static coverage; unresolved dynamic slots can hide generic relations and force runtime method dispatch."),
      "suggestion": "Run `cr analyze weak-types --only schema-dynamic,code-dynamic --intent unresolved --format json`; prefer concrete types, declared type variables, or trait `:where` bounds.",
    })]
  };
  let ids = rows.iter().map(|row| (row.ns.clone(), row.def.clone())).collect::<Vec<_>>();
  let envelope = serde_json::json!({
    "schema_version": 1,
    "command": "analyze.check-types",
    "revision": analysis_revision(snapshot, &ids)?,
    "data": {
      "filters": {
        "namespace": options.ns,
        "namespace_prefix": options.ns_prefix,
        "only": options.only,
        "include_dependencies": options.deps,
        "summary_only": options.summary_only,
      },
      "summary": {
        "namespaces": namespaces.len(),
        "definitions": rows.len(),
        "levels": levels,
        "kinds": kinds,
        "polymorphism": {
          "generic_definitions": polymorphic_defs,
          "trait_bounded_definitions": bounded_polymorphic_defs,
        },
      },
      "definitions": definitions,
    },
    "diagnostics": diagnostics,
  });
  serde_json::to_string_pretty(&envelope).map_err(|error| format!("Failed to encode type coverage JSON: {error}"))
}

pub fn format_weak_types_json(options: &WeakTypesCommand, snapshot: &snapshot::Snapshot) -> Result<String, String> {
  let rows = collect_weak_type_rows(options, snapshot)?;
  let mut kinds = BTreeMap::<&str, usize>::new();
  let mut intents = BTreeMap::<&str, usize>::new();
  let mut namespaces = BTreeSet::<&str>::new();
  for row in &rows {
    namespaces.insert(row.ns.as_str());
    for occurrence in &row.occurrences {
      *kinds.entry(occurrence.kind.as_str()).or_insert(0) += 1;
      *intents.entry(occurrence.intent.as_str()).or_insert(0) += 1;
    }
  }
  for kind in ["schema-dynamic", "code-dynamic", "code-nil"] {
    kinds.entry(kind).or_insert(0);
  }
  for intent in ["unresolved", "intentional-js-ffi"] {
    intents.entry(intent).or_insert(0);
  }

  let definitions = rows
    .iter()
    .filter(|_| !options.summary_only)
    .map(|row| {
      serde_json::json!({
        "id": format!("{}/{}", row.ns, row.def),
        "namespace": row.ns,
        "name": row.def,
        "occurrences": row.occurrences.iter().map(|occurrence| serde_json::json!({
          "kind": occurrence.kind.as_str(),
          "intent": occurrence.intent.as_str(),
          "detail": occurrence.detail,
          "path": occurrence.path,
          "impact": weak_type_impact(occurrence),
          "suggestion": weak_type_suggestion(occurrence),
        })).collect::<Vec<_>>(),
      })
    })
    .collect::<Vec<_>>();
  let ids = rows.iter().map(|row| (row.ns.clone(), row.def.clone())).collect::<Vec<_>>();
  let hit_count = rows.iter().map(|row| row.occurrences.len()).sum::<usize>();
  let unresolved_dynamic = rows
    .iter()
    .flat_map(|row| row.occurrences.iter())
    .filter(|occurrence| {
      occurrence.intent == WeakTypeIntent::Unresolved
        && matches!(occurrence.kind, WeakTypeKind::SchemaDynamic | WeakTypeKind::CodeDynamic)
    })
    .count();
  let diagnostics = if unresolved_dynamic == 0 {
    vec![]
  } else {
    vec![serde_json::json!({
      "code": "W_DYNAMIC_TYPE_DEBT",
      "phase": "analysis",
      "severity": "warning",
      "message": format!("{unresolved_dynamic} unresolved dynamic slot(s) erase static relationships used by generic binding, callback checking, and method specialization."),
      "suggestion": "Prefer concrete types; use `:generics` when positions share a type and trait `:where` bounds when only capabilities are required. Keep `:dynamic` only at documented boundaries.",
    })]
  };
  let envelope = serde_json::json!({
    "schema_version": 1,
    "command": "analyze.weak-types",
    "revision": analysis_revision(snapshot, &ids)?,
    "data": {
      "filters": {
        "namespace": options.ns,
        "namespace_prefix": options.ns_prefix,
        "only": options.only,
        "intent": options.intent,
        "include_dependencies": options.deps,
        "summary_only": options.summary_only,
      },
      "summary": {
        "namespaces": namespaces.len(),
        "definitions": rows.len(),
        "hits": hit_count,
        "kinds": kinds,
        "intents": intents,
      },
      "definitions": definitions,
    },
    "diagnostics": diagnostics,
  });
  serde_json::to_string_pretty(&envelope).map_err(|error| format!("Failed to encode weak type JSON: {error}"))
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
