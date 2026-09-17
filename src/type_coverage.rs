//! Type coverage and weak-type analysis for `calcit analyze`.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::sync::Arc;

use calcit::calcit::{
  CalcitProc, CalcitSyntax, CalcitTypeAnnotation, MacroExpansionType, MacroSyntaxType, ParamShape, ParamShapeToken, ProcTypeSignature,
  SchemaKind, SyntaxTypeSignature, compare_param_shapes, resolve_type_slot,
};
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
  if let CalcitTypeAnnotation::Macro(signature) = entry.schema.as_ref() {
    return (
      signature.generics.iter().map(|name| format!("'{name}")).collect(),
      signature.where_bounds.iter().map(|bound| bound.to_brief_string()).collect(),
    );
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
  UnresolvedTypeSlot,
  CodeDynamic,
  CodeNil,
  UnsafeCoerce,
}

impl WeakTypeKind {
  pub fn as_str(&self) -> &'static str {
    match self {
      Self::SchemaDynamic => "schema-dynamic",
      Self::UnresolvedTypeSlot => "unresolved-type-slot",
      Self::CodeDynamic => "code-dynamic",
      Self::CodeNil => "code-nil",
      Self::UnsafeCoerce => "unsafe-coerce",
    }
  }

  pub fn all() -> BTreeSet<Self> {
    BTreeSet::from([
      Self::SchemaDynamic,
      Self::UnresolvedTypeSlot,
      Self::CodeDynamic,
      Self::CodeNil,
      Self::UnsafeCoerce,
    ])
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WeakTypeIntent {
  Unresolved,
  IntentionalJsFfi,
  IntentionalMacroSyntax,
  IntentionalTypeSlotDynamic,
  ExplicitUnsafe,
  DeclaredUnit,
  DeclaredOptional,
}

impl WeakTypeIntent {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Unresolved => "unresolved",
      Self::IntentionalJsFfi => "intentional-js-ffi",
      Self::IntentionalMacroSyntax => "intentional-macro-syntax",
      Self::IntentionalTypeSlotDynamic => "intentional-type-slot-dynamic",
      Self::ExplicitUnsafe => "explicit-unsafe",
      Self::DeclaredUnit => "declared-unit",
      Self::DeclaredOptional => "declared-optional",
    }
  }

  pub fn all() -> BTreeSet<Self> {
    BTreeSet::from([
      Self::Unresolved,
      Self::IntentionalJsFfi,
      Self::IntentionalMacroSyntax,
      Self::IntentionalTypeSlotDynamic,
      Self::ExplicitUnsafe,
      Self::DeclaredUnit,
      Self::DeclaredOptional,
    ])
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeakTypeOccurrence {
  pub kind: WeakTypeKind,
  pub intent: WeakTypeIntent,
  pub detail: String,
  pub path: String,
  pub unsafe_evidence: Option<UnsafeCoerceEvidence>,
}

/// Static evidence attached only to an explicit `unsafe-coerce` boundary.
/// It intentionally describes source *form*, not an inferred host value type:
/// `analyze weak-types` reads a Snapshot and never executes the program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsafeCoerceEvidence {
  pub source_form: &'static str,
  pub target_schema: String,
  pub js_ffi_feature: bool,
  pub raw_adapter_namespace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeakTypeRow {
  pub ns: String,
  pub def: String,
  pub occurrences: Vec<WeakTypeOccurrence>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FfiBoundaryOperation {
  pub kind: String,
  pub classification: String,
  pub source: String,
  pub member: Option<String>,
  pub path: String,
  pub nullable_evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FfiHelperCandidate {
  pub definition: String,
  pub origin: String,
  pub compatibility: String,
  pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FfiTraitCandidate {
  pub receiver: String,
  pub suggested_name: String,
  pub fields: Vec<String>,
  pub methods: Vec<String>,
  pub contract_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FfiAdapterCandidate {
  pub source: String,
  pub suggested_definition: String,
  pub schema_cirru_edn: String,
  pub contract_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FfiBoundaryEvidence {
  pub diagnostic_code: String,
  pub definition: String,
  pub classification: String,
  pub target: String,
  pub js_ffi_feature: bool,
  pub raw_adapter_namespace: bool,
  pub operations: Vec<FfiBoundaryOperation>,
  pub unsafe_paths: Vec<String>,
  pub callers: Vec<String>,
  pub helper_candidates: Vec<FfiHelperCandidate>,
  pub trait_candidates: Vec<FfiTraitCandidate>,
  pub adapter_candidates: Vec<FfiAdapterCandidate>,
  pub provenance: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FfiImportBinding {
  source: String,
  default_import: bool,
}

fn schema_has_js_ffi(schema: &CalcitTypeAnnotation) -> bool {
  match schema {
    CalcitTypeAnnotation::Fn(annotation) => annotation.features.iter().any(|feature| feature.ref_str() == "js-ffi"),
    CalcitTypeAnnotation::Macro(annotation) => annotation.features.iter().any(|feature| feature.ref_str() == "js-ffi"),
    _ => false,
  }
}

fn ffi_import_bindings(code: &Cirru) -> BTreeMap<String, FfiImportBinding> {
  let mut bindings = BTreeMap::new();
  let Cirru::List(items) = code else {
    return bindings;
  };
  let Some(Cirru::List(require)) = items
    .iter()
    .skip(2)
    .find(|item| matches!(item, Cirru::List(parts) if parts.first().is_some_and(|head| head.eq_leaf(":require"))))
  else {
    return bindings;
  };
  for rule in require.iter().skip(1) {
    let Cirru::List(parts) = rule else {
      continue;
    };
    let parts = if parts.first().is_some_and(|item| item.eq_leaf("[]")) {
      &parts[1..]
    } else {
      parts.as_slice()
    };
    if parts.len() != 3 {
      continue;
    }
    let (Cirru::Leaf(source), Cirru::Leaf(kind), local) = (&parts[0], &parts[1], &parts[2]) else {
      continue;
    };
    match (kind.as_ref(), local) {
      (":as", Cirru::Leaf(alias)) | (":default", Cirru::Leaf(alias)) => {
        bindings.insert(
          alias.to_string(),
          FfiImportBinding {
            source: source.trim_start_matches('|').to_owned(),
            default_import: kind.as_ref() == ":default",
          },
        );
      }
      (":refer", Cirru::List(names)) => {
        for name in names {
          if let Cirru::Leaf(name) = name
            && name.as_ref() != "[]"
          {
            bindings.insert(
              name.to_string(),
              FfiImportBinding {
                source: source.trim_start_matches('|').to_owned(),
                default_import: false,
              },
            );
          }
        }
      }
      _ => {}
    }
  }
  bindings
}

fn ffi_member_operation(head: &str) -> Option<(&'static str, &'static str, bool)> {
  if head.starts_with(".?!") {
    Some(("optional-method-call", ".?!", true))
  } else if head.starts_with(".?-") {
    Some(("optional-field-read", ".?-", true))
  } else if head.starts_with(".!") {
    Some(("method-call", ".!", false))
  } else if head.starts_with(".-") {
    Some(("field-read", ".-", false))
  } else {
    None
  }
}

fn ffi_postfix_operation(head: &str) -> Option<(String, String, &'static str, bool)> {
  for (marker, kind, optional) in [
    (".?!", "optional-method-call", true),
    (".?-", "optional-field-read", true),
    (".!", "method-call", false),
    (".-", "field-read", false),
  ] {
    if let Some((receiver, member)) = head.split_once(marker)
      && !receiver.is_empty()
      && !member.is_empty()
    {
      return Some((receiver.to_owned(), member.to_owned(), kind, optional));
    }
  }
  None
}

fn ffi_classification(source: &str, import_source: Option<&str>, target: Option<snapshot::SnapshotTarget>) -> &'static str {
  let lowered = format!("{} {}", source, import_source.unwrap_or_default()).to_ascii_lowercase();
  if lowered.contains("webgpu")
    || lowered.contains("navigator.gpu")
    || lowered
      .split(|character: char| !character.is_ascii_alphanumeric())
      .any(|part| part == "gpu")
  {
    "webgpu"
  } else if import_source.is_some() {
    let package = import_source.unwrap_or_default().trim_start_matches("node:");
    if matches!(
      package,
      "assert"
        | "buffer"
        | "child_process"
        | "crypto"
        | "events"
        | "fs"
        | "http"
        | "https"
        | "os"
        | "path"
        | "process"
        | "stream"
        | "url"
        | "util"
    ) {
      "node"
    } else {
      "npm-import"
    }
  } else if [
    "js/window",
    "js/document",
    "js/navigator",
    "js/location",
    "js/localstorage",
    "js/sessionstorage",
  ]
  .iter()
  .any(|prefix| lowered.contains(prefix))
  {
    "browser"
  } else if ["js/process", "js/buffer", "js/global", "js/__dirname", "js/__filename"]
    .iter()
    .any(|prefix| lowered.contains(prefix))
  {
    "node"
  } else {
    match target {
      Some(snapshot::SnapshotTarget::Browser) => "browser",
      Some(snapshot::SnapshotTarget::Node) => "node",
      _ => "unknown-host",
    }
  }
}

fn push_ffi_operation(
  operations: &mut Vec<FfiBoundaryOperation>,
  kind: &str,
  source: String,
  member: Option<String>,
  path: &[usize],
  optional: bool,
) {
  operations.push(FfiBoundaryOperation {
    kind: kind.to_owned(),
    classification: "unknown-host".to_owned(),
    source,
    member,
    path: format_cirru_path("code", path),
    nullable_evidence: if optional { "optional-access" } else { "not-proven" }.to_owned(),
  });
}

fn scan_ffi_operations(
  node: &Cirru,
  path: &mut Vec<usize>,
  quote_context: QuoteContext,
  imports: &BTreeMap<String, FfiImportBinding>,
  operations: &mut Vec<FfiBoundaryOperation>,
) {
  if quote_context.is_quoted() {
    return;
  }
  let Cirru::List(items) = node else {
    return;
  };
  let head = items.first().and_then(|item| match item {
    Cirru::Leaf(value) => Some(value.as_ref()),
    _ => None,
  });
  if let Some(head) = head {
    if head.starts_with("js/") {
      push_ffi_operation(operations, "raw-js-call", head.to_owned(), None, path, false);
    } else if head == "unsafe-coerce" {
      let source = items
        .get(1)
        .map(render_cirru_inline)
        .unwrap_or_else(|| "<missing-value>".to_owned());
      push_ffi_operation(operations, "unsafe-coerce", source, None, path, false);
    } else if let Some((kind, marker, optional)) = ffi_member_operation(head) {
      let receiver = items
        .get(1)
        .map(render_cirru_inline)
        .unwrap_or_else(|| "<missing-receiver>".to_owned());
      push_ffi_operation(
        operations,
        kind,
        receiver,
        Some(head.trim_start_matches(marker).to_owned()),
        path,
        optional,
      );
    } else if let Some((receiver, member, kind, optional)) = ffi_postfix_operation(head) {
      push_ffi_operation(operations, kind, receiver, Some(member), path, optional);
    } else if matches!(head, "aget" | "aset" | "js-get" | "js-set") {
      let receiver = items
        .get(1)
        .map(render_cirru_inline)
        .unwrap_or_else(|| "<missing-receiver>".to_owned());
      let member = items.get(2).map(render_cirru_inline);
      push_ffi_operation(operations, head, receiver, member, path, false);
    } else if let Some((binding, export)) = head.split_once('/')
      && imports.contains_key(binding)
    {
      push_ffi_operation(
        operations,
        "module-call",
        format!("{binding}/{export}"),
        Some(export.to_owned()),
        path,
        false,
      );
    } else if imports.get(head).is_some_and(|binding| binding.default_import) {
      push_ffi_operation(operations, "module-call", head.to_owned(), Some("default".to_owned()), path, false);
    }
  }
  for (index, child) in items.iter().enumerate() {
    path.push(index);
    scan_ffi_operations(child, path, quote_context.for_child(head, index), imports, operations);
    path.pop();
  }
}

fn code_calls_definition(code: &Cirru, caller_namespace: &str, namespace: &str, definition: &str) -> bool {
  fn visit(node: &Cirru, caller_namespace: &str, namespace: &str, definition: &str, quote_context: QuoteContext) -> bool {
    if quote_context.is_quoted() {
      return false;
    }
    let Cirru::List(items) = node else {
      return false;
    };
    let head = items.first().and_then(|item| match item {
      Cirru::Leaf(value) => Some(value.as_ref()),
      _ => None,
    });
    if head
      .is_some_and(|head| (caller_namespace == namespace && head == definition) || head == format!("{namespace}/{definition}").as_str())
    {
      return true;
    }
    items
      .iter()
      .enumerate()
      .any(|(index, item)| visit(item, caller_namespace, namespace, definition, quote_context.for_child(head, index)))
  }

  visit(code, caller_namespace, namespace, definition, QuoteContext::default())
}

fn ffi_target_name(entry: &snapshot::CodeEntry, fallback: Option<snapshot::SnapshotTarget>) -> Result<String, String> {
  let target = entry
    .ffi
    .as_ref()
    .map(snapshot::parse_ffi_target)
    .transpose()?
    .flatten()
    .or(fallback);
  Ok(
    target
      .map(|target| target.as_str().to_owned())
      .unwrap_or_else(|| "unspecified".to_owned()),
  )
}

fn schema_cirru_edn(schema: &CalcitTypeAnnotation) -> Result<String, String> {
  cirru_edn::format(&snapshot::schema_annotation_to_edn(schema), true)
    .map_err(|error| format!("Failed to render FFI adapter schema: {error}"))
}

fn suggested_trait_name(definition: &str) -> String {
  let mut out = String::new();
  let mut uppercase = true;
  for character in definition.chars() {
    if character.is_ascii_alphanumeric() {
      if uppercase {
        out.extend(character.to_uppercase());
        uppercase = false;
      } else {
        out.push(character);
      }
    } else {
      uppercase = true;
    }
  }
  if out.is_empty() {
    "HostBoundary".to_owned()
  } else if out.ends_with("Host") {
    out
  } else {
    format!("{out}Host")
  }
}

pub fn parse_weak_type_kinds(raw: &str) -> Result<BTreeSet<WeakTypeKind>, String> {
  let mut selected = BTreeSet::new();

  for item in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
    let kind = match item {
      "schema-dynamic" => WeakTypeKind::SchemaDynamic,
      "unresolved-type-slot" => WeakTypeKind::UnresolvedTypeSlot,
      "code-dynamic" => WeakTypeKind::CodeDynamic,
      "code-nil" => WeakTypeKind::CodeNil,
      "unsafe-coerce" => WeakTypeKind::UnsafeCoerce,
      other => {
        return Err(format!(
          "Unknown weak-type filter `{other}`. Expected comma-separated values from: schema-dynamic, unresolved-type-slot, code-dynamic, code-nil, unsafe-coerce"
        ));
      }
    };
    selected.insert(kind);
  }

  if selected.is_empty() {
    return Err(
      "Weak-type filter cannot be empty. Use comma-separated values from: schema-dynamic, unresolved-type-slot, code-dynamic, code-nil, unsafe-coerce"
        .to_owned(),
    );
  }

  Ok(selected)
}

pub fn parse_weak_type_intents(raw: &str) -> Result<BTreeSet<WeakTypeIntent>, String> {
  let mut selected = BTreeSet::new();

  for item in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
    let intent = match item {
      "unresolved" => WeakTypeIntent::Unresolved,
      "intentional-js-ffi" => WeakTypeIntent::IntentionalJsFfi,
      "intentional-macro-syntax" => WeakTypeIntent::IntentionalMacroSyntax,
      "intentional-type-slot-dynamic" => WeakTypeIntent::IntentionalTypeSlotDynamic,
      "explicit-unsafe" => WeakTypeIntent::ExplicitUnsafe,
      "declared-unit" => WeakTypeIntent::DeclaredUnit,
      "declared-optional" => WeakTypeIntent::DeclaredOptional,
      other => {
        return Err(format!(
          "Unknown weak-type intent `{other}`. Expected comma-separated values from: unresolved, intentional-js-ffi, intentional-macro-syntax, intentional-type-slot-dynamic, explicit-unsafe, declared-unit, declared-optional"
        ));
      }
    };
    selected.insert(intent);
  }

  if selected.is_empty() {
    return Err(
      "Weak-type intent filter cannot be empty. Use comma-separated values from: unresolved, intentional-js-ffi, intentional-macro-syntax, intentional-type-slot-dynamic, explicit-unsafe, declared-unit, declared-optional"
        .to_owned(),
    );
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
    unsafe_evidence: None,
  });
}

fn scan_intentional_macro_expr(annotation: &CalcitTypeAnnotation, path: &str, detail: &str, occurrences: &mut Vec<WeakTypeOccurrence>) {
  let start = occurrences.len();
  scan_schema_dynamic_annotation(annotation, path, detail, occurrences);
  for occurrence in &mut occurrences[start..] {
    if occurrence.kind == WeakTypeKind::SchemaDynamic && occurrence.intent == WeakTypeIntent::Unresolved {
      occurrence.intent = WeakTypeIntent::IntentionalMacroSyntax;
    }
  }
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
    CalcitTypeAnnotation::TypeSlot(name) => match resolve_type_slot(name) {
      Some(resolved) => {
        let start = occurrences.len();
        scan_schema_dynamic_annotation(&resolved, path, detail, occurrences);
        for occurrence in &mut occurrences[start..] {
          if occurrence.intent == WeakTypeIntent::Unresolved && occurrence.kind != WeakTypeKind::UnresolvedTypeSlot {
            occurrence.intent = WeakTypeIntent::IntentionalTypeSlotDynamic;
          }
        }
      }
      None => push_weak_type_occurrence(
        occurrences,
        WeakTypeKind::UnresolvedTypeSlot,
        weak_type_detail(WeakTypeKind::UnresolvedTypeSlot, &format!("{detail}:{name}")),
        path.to_owned(),
      ),
    },
    CalcitTypeAnnotation::List(inner)
    | CalcitTypeAnnotation::Set(inner)
    | CalcitTypeAnnotation::Ref(inner)
    | CalcitTypeAnnotation::Variadic(inner)
    | CalcitTypeAnnotation::Optional(inner)
    | CalcitTypeAnnotation::JsNullish(inner) => {
      let segment = match annotation {
        CalcitTypeAnnotation::List(_) => "list-item",
        CalcitTypeAnnotation::Set(_) => "set-item",
        CalcitTypeAnnotation::Ref(_) => "ref-item",
        CalcitTypeAnnotation::Variadic(_) => "variadic-item",
        CalcitTypeAnnotation::Optional(_) => "optional-item",
        CalcitTypeAnnotation::JsNullish(_) => "js-nullish-item",
        _ => unreachable!("composite item annotation should be covered by the match arm"),
      };
      let nested_detail = extend_schema_dynamic_detail(detail, segment);
      let occurrence_start = occurrences.len();
      scan_schema_dynamic_annotation(inner, &format!("{path}.item"), &nested_detail, occurrences);
      if matches!(annotation, CalcitTypeAnnotation::JsNullish(_)) {
        for occurrence in &mut occurrences[occurrence_start..] {
          if occurrence.kind == WeakTypeKind::SchemaDynamic && occurrence.intent == WeakTypeIntent::Unresolved {
            occurrence.intent = WeakTypeIntent::IntentionalJsFfi;
          }
        }
      }
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
    CalcitTypeAnnotation::Macro(signature) => {
      for (idx, contract) in signature.required_inputs.iter().chain(signature.optional_inputs.iter()).enumerate() {
        if let MacroSyntaxType::Expr(semantic) = contract {
          scan_intentional_macro_expr(semantic, &format!("{path}.inputs.{idx}.expr"), detail, occurrences);
        }
      }
      if let Some(MacroSyntaxType::Expr(semantic)) = &signature.rest_input {
        scan_intentional_macro_expr(semantic, &format!("{path}.rest.expr"), detail, occurrences);
      }
      match &signature.expansion {
        MacroExpansionType::Expr(semantic) => {
          scan_intentional_macro_expr(semantic, &format!("{path}.expansion"), detail, occurrences);
        }
        MacroExpansionType::Definition(semantic) => {
          scan_schema_dynamic_annotation(semantic, &format!("{path}.expansion"), detail, occurrences);
        }
        MacroExpansionType::Dynamic => {
          scan_schema_dynamic_annotation(&CalcitTypeAnnotation::Dynamic, &format!("{path}.expansion"), detail, occurrences)
        }
        MacroExpansionType::Declarations => {}
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
  if occurrence.kind == WeakTypeKind::UnsafeCoerce {
    if occurrence.unsafe_evidence.as_ref().is_some_and(|evidence| !evidence.js_ffi_feature) {
      return "Declare `:features $ #{} :js-ffi` on this adapter first, then keep the assertion narrow, validate or normalize untrusted runtime data before application code consumes it, and add positive and negative runtime-contract tests.";
    }
    return "Keep this assertion at a small trusted boundary, validate or normalize untrusted runtime data into Option, Result, a struct, or an enum before it reaches application code, and add positive and negative runtime-contract tests.";
  }
  if occurrence.intent == WeakTypeIntent::IntentionalJsFfi {
    return "Keep the dynamic value isolated at the declared JS FFI boundary and validate or convert it before typed code consumes it.";
  }
  if occurrence.intent == WeakTypeIntent::IntentionalMacroSyntax {
    return "This Dynamic is an explicit open expression boundary inside a strict MacroSignature. Narrow the Expr semantic type when the macro contract guarantees more, but do not replace the phase-aware signature with whole-Dynamic.";
  }
  if occurrence.intent == WeakTypeIntent::IntentionalTypeSlotDynamic {
    return "This slot intentionally resolves to Dynamic for the selected entry. Keep that boundary documented and narrow or validate the value before typed code relies on it.";
  }

  if occurrence.kind == WeakTypeKind::CodeNil {
    return match occurrence.intent {
      WeakTypeIntent::DeclaredUnit if occurrence.detail.contains("nil-macro") => {
        "The legacy `;nil` form returns Nil, not Unit. Replace it with `&unit`, or end the body with an effect that already returns Unit."
      }
      WeakTypeIntent::DeclaredUnit => {
        "The declared return is Unit, so replace this nil with `&unit`, or end the body with an effect that already returns Unit."
      }
      WeakTypeIntent::DeclaredOptional => {
        "Keep Optional only at a compatibility/FFI boundary; migrate application-level absence to Option and failures with details to Result."
      }
      _ => {
        "Declare Unit for a no-value result and remove explicit nil/`;nil` when possible; use Option/Result for application absence or failure."
      }
    };
  }
  if occurrence.kind == WeakTypeKind::UnresolvedTypeSlot {
    return "Bind this slot in the selected entry with `calcit config set-type-slot <slot> <namespace/definition>`, or explicitly select `:dynamic` only when this is a documented open boundary.";
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

fn entry_schema_issues(ns: &str, def_name: &str, code: &Cirru, schema: &Arc<CalcitTypeAnnotation>) -> Vec<String> {
  let annotation = schema.as_ref();
  let mut issues = validate_def_vs_schema(ns, def_name, code, annotation);
  let has_js_ffi_feature = match annotation {
    CalcitTypeAnnotation::Fn(fn_annot) => fn_annot.features.iter().any(|feature| feature.ref_str() == "js-ffi"),
    CalcitTypeAnnotation::Macro(signature) => signature.features.iter().any(|feature| feature.ref_str() == "js-ffi"),
    _ => false,
  };
  if matches!(annotation, CalcitTypeAnnotation::Dynamic) {
    if matches!(code, Cirru::List(items) if matches!(items.first(), Some(Cirru::Leaf(head)) if head.as_ref() == "defmacro")) {
      let (warning, detail) = if snapshot::schema_annotation_is_missing(schema) {
        ("W_SCHEMA_MISSING", "has no declared macro schema")
      } else {
        (
          "W_MACRO_SCHEMA_DYNAMIC",
          "declares an intentionally untyped whole-Dynamic macro schema",
        )
      };
      issues.push(format!("[{warning}] {ns}/{def_name} {detail}"));
    }
    return issues;
  }

  let mut occurrences = vec![];
  scan_schema_dynamic_annotation(annotation, "schema", "root", &mut occurrences);
  for occurrence in occurrences {
    if (has_js_ffi_feature && occurrence.kind == WeakTypeKind::SchemaDynamic)
      || occurrence.intent == WeakTypeIntent::IntentionalMacroSyntax
    {
      continue;
    }
    let warning = if occurrence.kind == WeakTypeKind::UnresolvedTypeSlot {
      "W_UNRESOLVED_TYPE_SLOT"
    } else {
      "W_SCHEMA_DYNAMIC"
    };
    issues.push(format!(
      "[{warning}] {} is unresolved ({}). Fix: {}",
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

fn nil_return_intent(annotation: &CalcitTypeAnnotation) -> WeakTypeIntent {
  let return_type = match annotation {
    CalcitTypeAnnotation::Fn(fn_annotation) => fn_annotation.return_type.as_ref(),
    CalcitTypeAnnotation::Macro(signature) => match &signature.expansion {
      MacroExpansionType::Expr(semantic) | MacroExpansionType::Definition(semantic) => semantic.as_ref(),
      MacroExpansionType::Dynamic | MacroExpansionType::Declarations => annotation,
    },
    other => other,
  };
  match return_type {
    CalcitTypeAnnotation::Unit => WeakTypeIntent::DeclaredUnit,
    CalcitTypeAnnotation::Optional(_) => WeakTypeIntent::DeclaredOptional,
    CalcitTypeAnnotation::JsNullish(_) => WeakTypeIntent::IntentionalJsFfi,
    _ => WeakTypeIntent::Unresolved,
  }
}

fn child_is_return_position(
  is_code_root: bool,
  parent_is_return_position: bool,
  head: Option<&str>,
  child_index: usize,
  child_count: usize,
) -> bool {
  if !parent_is_return_position {
    return false;
  }
  if is_code_root {
    return match head {
      Some("defn" | "defmacro") => child_index >= 3 && child_index + 1 == child_count,
      Some("fn" | "macro") => child_index >= 2 && child_index + 1 == child_count,
      Some("def") => child_index == 2,
      _ => false,
    };
  }
  match head {
    Some("if") => matches!(child_index, 2 | 3),
    Some("let" | "&let") => child_index >= 2 && child_index + 1 == child_count,
    Some("do") => child_index >= 1 && child_index + 1 == child_count,
    _ => false,
  }
}

fn code_declares_embedded_type_schema(code: &Cirru) -> bool {
  matches!(
    code,
    Cirru::List(items)
      if matches!(items.first(), Some(Cirru::Leaf(head)) if matches!(head.as_ref(), "defstruct" | "defenum" | "deftrait" | "defimpl"))
  )
}

fn classify_unsafe_coerce_source(items: &[Cirru]) -> &'static str {
  let Some(source) = items.get(1) else {
    return "missing-value";
  };
  match source {
    Cirru::Leaf(name) if name.starts_with("js/") => "raw-js-value",
    Cirru::List(parts) => match parts.first() {
      Some(Cirru::Leaf(head)) if head.starts_with("js/") => "raw-js-operation",
      Some(Cirru::Leaf(head))
        if matches!(head.as_ref(), "aget" | "js-get")
          || head.starts_with(".-")
          || head.starts_with(".!")
          || head.starts_with(".?-")
          || head.starts_with(".?!") =>
      {
        "host-member-operation"
      }
      _ => "expression",
    },
    Cirru::Leaf(_) => "value",
  }
}

fn is_raw_adapter_namespace(ns: &str) -> bool {
  ns == "js-ffi.raw" || ns.starts_with("js-ffi.raw.") || ns.contains(".js-ffi.raw.")
}

#[derive(Debug, Clone, Copy, Default)]
struct QuoteContext {
  quoted_depth: usize,
  quasiquoted_depth: usize,
}

impl QuoteContext {
  fn is_quoted(self) -> bool {
    self.quoted_depth > 0 || self.quasiquoted_depth > 0
  }

  fn for_child(self, head: Option<&str>, child_index: usize) -> Self {
    if child_index == 0 {
      return self;
    }
    match head {
      Some("quote") => Self {
        quoted_depth: self.quoted_depth + 1,
        ..self
      },
      Some("quasiquote") => Self {
        quasiquoted_depth: self.quasiquoted_depth + 1,
        ..self
      },
      Some("~" | "~@") if self.quoted_depth == 0 => Self {
        quasiquoted_depth: self.quasiquoted_depth.saturating_sub(1),
        ..self
      },
      _ => self,
    }
  }
}

#[derive(Debug, Clone, Copy, Default)]
struct WeakScanState {
  nil_return_intent: Option<WeakTypeIntent>,
  quote_context: QuoteContext,
}

fn scan_cirru_weak_types(
  node: &Cirru,
  root: &str,
  path: &mut Vec<usize>,
  parent: Option<&WeakCodeParent>,
  state: WeakScanState,
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
      } else if text.as_ref() == "nil"
        && root == "code"
        && selected.contains(&WeakTypeKind::CodeNil)
        && !state.quote_context.is_quoted()
      {
        push_weak_type_occurrence(
          occurrences,
          WeakTypeKind::CodeNil,
          weak_type_detail(WeakTypeKind::CodeNil, classify_code_nil(parent)),
          format_cirru_path(root, path),
        );
        if let (Some(intent), Some(occurrence)) = (state.nil_return_intent, occurrences.last_mut()) {
          occurrence.intent = intent;
        }
      }
    }
    Cirru::List(items) => {
      let head = items.first().and_then(|item| match item {
        Cirru::Leaf(text) => Some(text.to_string()),
        _ => None,
      });
      if root == "code"
        && selected.contains(&WeakTypeKind::CodeNil)
        && matches!(head.as_deref(), Some(";nil"))
        && !state.quote_context.is_quoted()
      {
        push_weak_type_occurrence(
          occurrences,
          WeakTypeKind::CodeNil,
          weak_type_detail(WeakTypeKind::CodeNil, &format!("nil-macro:{}", classify_code_nil(parent))),
          format_cirru_path(root, path),
        );
        if let (Some(intent), Some(occurrence)) = (state.nil_return_intent, occurrences.last_mut()) {
          // `;nil` is a legacy spelling of Nil. It inherits a declared return
          // intent only when structural analysis proves that it is returned.
          occurrence.intent = intent;
        }
      }
      if root == "code"
        && selected.contains(&WeakTypeKind::UnsafeCoerce)
        && matches!(head.as_deref(), Some("unsafe-coerce"))
        && !state.quote_context.is_quoted()
      {
        let target = items
          .get(2)
          .map(render_cirru_inline)
          .unwrap_or_else(|| "<missing-target>".to_owned());
        occurrences.push(WeakTypeOccurrence {
          kind: WeakTypeKind::UnsafeCoerce,
          intent: WeakTypeIntent::ExplicitUnsafe,
          detail: format!("unsafe-coerce:target={target}"),
          path: format_cirru_path(root, path),
          unsafe_evidence: Some(UnsafeCoerceEvidence {
            source_form: classify_unsafe_coerce_source(items),
            target_schema: target,
            js_ffi_feature: false,
            raw_adapter_namespace: false,
          }),
        });
      }
      for (idx, item) in items.iter().enumerate() {
        path.push(idx);
        let next_parent = WeakCodeParent {
          head: head.clone(),
          child_index: idx,
        };
        let child_return_position = child_is_return_position(
          root == "code" && path.len() == 1,
          state.nil_return_intent.is_some(),
          head.as_deref(),
          idx,
          items.len(),
        );
        let child_state = WeakScanState {
          nil_return_intent: state.nil_return_intent.filter(|_| child_return_position),
          quote_context: state.quote_context.for_child(head.as_deref(), idx),
        };
        scan_cirru_weak_types(item, root, path, Some(&next_parent), child_state, selected, occurrences);
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

  let selected_schema_annotations =
    selected.contains(&WeakTypeKind::SchemaDynamic) || selected.contains(&WeakTypeKind::UnresolvedTypeSlot);
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
    if selected_schema_annotations {
      for (idx, arg) in fn_annot.arg_types.iter().enumerate() {
        scan_schema_dynamic_annotation(arg, &format!("schema.args.{idx}"), "arg", &mut occurrences);
      }
      scan_schema_dynamic_annotation(&fn_annot.return_type, "schema.return", "return", &mut occurrences);
      if let Some(rest) = &fn_annot.rest_type {
        scan_schema_dynamic_annotation(rest, "schema.rest", "rest", &mut occurrences);
      }
    }
  } else if selected_schema_annotations {
    let before = occurrences.len();
    scan_schema_dynamic_annotation(entry.schema.as_ref(), "schema", "root", &mut occurrences);

    if occurrences.len() == before
      && selected.contains(&WeakTypeKind::SchemaDynamic)
      && let Ok(schema_cirru) = snapshot::schema_edn_to_cirru(&entry.schema.to_type_edn())
    {
      let mut path = vec![];
      scan_cirru_weak_types(
        &schema_cirru,
        "schema",
        &mut path,
        None,
        WeakScanState::default(),
        selected,
        &mut occurrences,
      );
    }
  }

  let mut code_path = vec![];
  scan_cirru_weak_types(
    &entry.code,
    "code",
    &mut code_path,
    None,
    WeakScanState {
      nil_return_intent: Some(nil_return_intent(entry.schema.as_ref())),
      quote_context: QuoteContext::default(),
    },
    selected,
    &mut occurrences,
  );

  occurrences.retain(|occurrence| selected.contains(&occurrence.kind));

  let has_js_ffi_feature = match entry.schema.as_ref() {
    CalcitTypeAnnotation::Fn(fn_annot) => fn_annot.features.iter().any(|feature| feature.ref_str() == "js-ffi"),
    CalcitTypeAnnotation::Macro(signature) => signature.features.iter().any(|feature| feature.ref_str() == "js-ffi"),
    _ => false,
  };
  if has_js_ffi_feature {
    for occurrence in &mut occurrences {
      if matches!(occurrence.kind, WeakTypeKind::SchemaDynamic | WeakTypeKind::CodeDynamic) {
        occurrence.intent = WeakTypeIntent::IntentionalJsFfi;
      }
    }
  }
  for occurrence in &mut occurrences {
    if let Some(evidence) = &mut occurrence.unsafe_evidence {
      evidence.js_ffi_feature = has_js_ffi_feature;
      evidence.raw_adapter_namespace = is_raw_adapter_namespace(ns);
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

pub fn collect_ffi_boundary_evidence(
  options: &WeakTypesCommand,
  snapshot: &snapshot::Snapshot,
) -> Result<Vec<FfiBoundaryEvidence>, String> {
  let active_target = snapshot.active_entry().ok().and_then(|entry| entry.target);
  let package_prefix = format!("{}.", snapshot.package);
  let mut boundaries = Vec::new();
  let mut target_error = None;

  visit_scoped_definitions(
    snapshot,
    AnalysisScope {
      namespace: options.ns.as_deref(),
      namespace_prefix: options.ns_prefix.as_deref(),
      include_dependencies: options.deps,
    },
    |namespace, definition, entry| {
      if target_error.is_some() {
        return;
      }
      let imports = snapshot
        .files
        .get(namespace)
        .map(|file| ffi_import_bindings(&file.ns.code))
        .unwrap_or_default();
      let mut operations = Vec::new();
      scan_ffi_operations(&entry.code, &mut Vec::new(), QuoteContext::default(), &imports, &mut operations);
      operations.sort_by(|left, right| left.path.cmp(&right.path).then(left.kind.cmp(&right.kind)));
      operations.dedup();

      let js_ffi_feature = schema_has_js_ffi(entry.schema.as_ref());
      if operations.is_empty() && !js_ffi_feature && entry.ffi.is_none() {
        return;
      }

      let target = match ffi_target_name(entry, active_target) {
        Ok(target) => target,
        Err(error) => {
          target_error = Some(format!("Invalid FFI metadata for {namespace}/{definition}: {error}"));
          return;
        }
      };

      let mut classifications = BTreeSet::new();
      for operation in &mut operations {
        let import_source = operation
          .source
          .split_once('/')
          .and_then(|(binding, _)| imports.get(binding))
          .or_else(|| imports.get(&operation.source))
          .map(|binding| binding.source.as_str());
        let classification = ffi_classification(&operation.source, import_source, active_target);
        operation.classification = classification.to_owned();
        classifications.insert(classification);
      }
      if classifications.is_empty() {
        classifications.insert(ffi_classification("", None, active_target));
      }
      let classification = if classifications.len() == 1 {
        classifications.iter().next().copied().unwrap_or("unknown-host").to_owned()
      } else {
        "mixed".to_owned()
      };

      let unsafe_paths = operations
        .iter()
        .filter(|operation| operation.kind == "unsafe-coerce")
        .map(|operation| operation.path.clone())
        .collect::<Vec<_>>();

      let mut callers = Vec::new();
      for (caller_ns, file) in &snapshot.files {
        for (caller_def, caller_entry) in &file.defs {
          if caller_ns == namespace && caller_def == definition {
            continue;
          }
          if code_calls_definition(&caller_entry.code, caller_ns, namespace, definition) {
            callers.push(format!("{caller_ns}/{caller_def}"));
          }
        }
      }
      callers.sort();
      callers.dedup();

      let schema_is_concrete = {
        let mut findings = Vec::new();
        scan_schema_dynamic_annotation(entry.schema.as_ref(), "schema", "root", &mut findings);
        findings.is_empty() && !matches!(entry.schema.as_ref(), CalcitTypeAnnotation::Dynamic)
      };
      let mut helper_candidates = Vec::new();
      if schema_is_concrete {
        for (candidate_ns, file) in &snapshot.files {
          for (candidate_def, candidate) in &file.defs {
            if candidate_ns == namespace && candidate_def == definition {
              continue;
            }
            if candidate.schema != entry.schema || !schema_has_js_ffi(candidate.schema.as_ref()) {
              continue;
            }
            let candidate_target = ffi_target_name(candidate, active_target).unwrap_or_else(|_| "invalid".to_owned());
            if target != "unspecified" && candidate_target != "unspecified" && candidate_target != target {
              continue;
            }
            let project_candidate = candidate_ns == &snapshot.package || candidate_ns.starts_with(&package_prefix);
            helper_candidates.push(FfiHelperCandidate {
              definition: format!("{candidate_ns}/{candidate_def}"),
              origin: if project_candidate { "project" } else { "dependency" }.to_owned(),
              compatibility: "exact-schema".to_owned(),
              target: candidate_target,
            });
          }
        }
      }
      helper_candidates.sort_by(|left, right| {
        let left_rank = if left.origin == "dependency" { 0 } else { 1 };
        let right_rank = if right.origin == "dependency" { 0 } else { 1 };
        left_rank.cmp(&right_rank).then(left.definition.cmp(&right.definition))
      });

      let mut member_groups = BTreeMap::<String, (BTreeSet<String>, BTreeSet<String>)>::new();
      for operation in &operations {
        if operation.kind == "module-call" {
          continue;
        }
        let Some(member) = &operation.member else {
          continue;
        };
        let group = member_groups.entry(operation.source.clone()).or_default();
        if operation.kind.contains("method") {
          group.1.insert(member.clone());
        } else if matches!(
          operation.kind.as_str(),
          "field-read" | "optional-field-read" | "aget" | "aset" | "js-get" | "js-set"
        ) {
          group.0.insert(member.clone());
        }
      }
      let trait_candidates = member_groups
        .into_iter()
        .filter(|(_, (fields, methods))| !fields.is_empty() || !methods.is_empty())
        .map(|(receiver, (fields, methods))| FfiTraitCandidate {
          receiver,
          suggested_name: suggested_trait_name(definition),
          fields: fields.into_iter().collect(),
          methods: methods.into_iter().collect(),
          contract_status: "review-required".to_owned(),
        })
        .collect::<Vec<_>>();

      let schema = schema_cirru_edn(entry.schema.as_ref()).unwrap_or_else(|error| format!("<unavailable: {error}>"));
      let mut adapter_sources = BTreeSet::new();
      for operation in &operations {
        if operation.kind != "module-call" {
          continue;
        }
        if let Some((binding, export)) = operation.source.split_once('/')
          && let Some(import) = imports.get(binding)
        {
          adapter_sources.insert(format!("{}/{}", import.source, export));
        } else if let Some(import) = imports.get(&operation.source) {
          adapter_sources.insert(format!("{}/default", import.source));
        }
      }
      let adapter_candidates = adapter_sources
        .into_iter()
        .map(|source| FfiAdapterCandidate {
          source,
          suggested_definition: format!("{definition}-adapter"),
          schema_cirru_edn: schema.clone(),
          contract_status: "review-required".to_owned(),
        })
        .collect();

      boundaries.push(FfiBoundaryEvidence {
        diagnostic_code: "I_FFI_BOUNDARY_EVIDENCE".to_owned(),
        definition: format!("{namespace}/{definition}"),
        classification,
        target,
        js_ffi_feature,
        raw_adapter_namespace: is_raw_adapter_namespace(namespace),
        operations,
        unsafe_paths,
        callers,
        helper_candidates,
        trait_candidates,
        adapter_candidates,
        provenance: vec![
          "snapshot-source".to_owned(),
          "declared-schema".to_owned(),
          "namespace-imports".to_owned(),
          "no-runtime-trust-inference".to_owned(),
        ],
      });
    },
  )?;

  if let Some(error) = target_error {
    return Err(error);
  }

  boundaries.sort_by(|left, right| left.definition.cmp(&right.definition));
  Ok(boundaries)
}

pub fn run_weak_types_report(options: &WeakTypesCommand, snapshot: &snapshot::Snapshot, out: &mut String) -> Result<(), String> {
  let rows = collect_weak_type_rows(options, snapshot)?;
  let ffi_boundaries = if options.ffi_evidence {
    collect_ffi_boundary_evidence(options, snapshot)?
  } else {
    Vec::new()
  };

  if rows.is_empty() && ffi_boundaries.is_empty() {
    let _ = writeln!(out, "No weak type usage found in selected namespace scope.");
    return Ok(());
  }

  let mut kind_count: BTreeMap<&'static str, usize> = BTreeMap::new();
  let mut intent_count: BTreeMap<&'static str, usize> = BTreeMap::new();
  let mut detail_count: BTreeMap<&'static str, BTreeMap<String, usize>> = BTreeMap::new();
  let mut ns_set: BTreeSet<&str> = BTreeSet::new();
  let mut def_count = 0usize;

  for row in &rows {
    def_count += 1;
    ns_set.insert(row.ns.as_str());
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
    "- hits: schema-dynamic={} unresolved-type-slot={} code-dynamic={} code-nil={} unsafe-coerce={}",
    kind_count.get("schema-dynamic").copied().unwrap_or(0),
    kind_count.get("unresolved-type-slot").copied().unwrap_or(0),
    kind_count.get("code-dynamic").copied().unwrap_or(0),
    kind_count.get("code-nil").copied().unwrap_or(0),
    kind_count.get("unsafe-coerce").copied().unwrap_or(0)
  );
  let _ = writeln!(
    out,
    "- intents: unresolved={} intentional-js-ffi={} intentional-macro-syntax={} intentional-type-slot-dynamic={} explicit-unsafe={} declared-unit={} declared-optional={}",
    intent_count.get("unresolved").copied().unwrap_or(0),
    intent_count.get("intentional-js-ffi").copied().unwrap_or(0),
    intent_count.get("intentional-macro-syntax").copied().unwrap_or(0),
    intent_count.get("intentional-type-slot-dynamic").copied().unwrap_or(0),
    intent_count.get("explicit-unsafe").copied().unwrap_or(0),
    intent_count.get("declared-unit").copied().unwrap_or(0),
    intent_count.get("declared-optional").copied().unwrap_or(0)
  );
  if options.ffi_evidence {
    let mut classifications = BTreeMap::<&str, usize>::new();
    for boundary in &ffi_boundaries {
      *classifications.entry(boundary.classification.as_str()).or_insert(0) += 1;
    }
    let _ = writeln!(out, "- FFI boundaries: {}", ffi_boundaries.len());
    let _ = writeln!(
      out,
      "- FFI classifications: {}",
      classifications
        .iter()
        .map(|(name, count)| format!("{name}={count}"))
        .collect::<Vec<_>>()
        .join(" ")
    );
  }
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
  let unresolved_type_slots = rows
    .iter()
    .flat_map(|row| row.occurrences.iter())
    .filter(|occurrence| occurrence.intent == WeakTypeIntent::Unresolved && occurrence.kind == WeakTypeKind::UnresolvedTypeSlot)
    .count();
  if unresolved_type_slots > 0 {
    let _ = writeln!(
      out,
      "- agent-note: {unresolved_type_slots} type slot(s) are not bound for the selected entry and currently behave like Dynamic."
    );
    let _ = writeln!(
      out,
      "- next: bind each slot with `calcit config set-type-slot`, or choose `:dynamic` explicitly for a documented open boundary."
    );
  }
  let nil_debt = rows
    .iter()
    .flat_map(|row| row.occurrences.iter())
    .filter(|occurrence| {
      occurrence.kind == WeakTypeKind::CodeNil
        && matches!(
          occurrence.intent,
          WeakTypeIntent::Unresolved | WeakTypeIntent::DeclaredUnit | WeakTypeIntent::DeclaredOptional
        )
    })
    .count();
  if nil_debt > 0 {
    let _ = writeln!(
      out,
      "- agent-note: {nil_debt} nil occurrence(s) are unresolved, violate Unit returns, or are covered only by Optional compatibility contracts."
    );
    let _ = writeln!(
      out,
      "- next: filter `--only code-nil --intent unresolved,declared-unit,declared-optional`; use `&unit` for no-value returns, then prefer Option/Result for application absence or failure."
    );
  }
  let unsafe_coercions = rows
    .iter()
    .flat_map(|row| row.occurrences.iter())
    .filter(|occurrence| occurrence.kind == WeakTypeKind::UnsafeCoerce)
    .count();
  if unsafe_coercions > 0 {
    let _ = writeln!(
      out,
      "- agent-note: {unsafe_coercions} explicit unsafe coercion(s) bypass static compatibility and need a documented runtime contract."
    );
    let _ = writeln!(
      out,
      "- next: rerun with `--only unsafe-coerce`; keep each assertion in a minimal adapter and add positive and negative boundary-contract tests."
    );
  }
  if options.summary_only {
    return Ok(());
  }
  if options.ffi_evidence && !ffi_boundaries.is_empty() {
    let _ = writeln!(out, "\n## FFI boundary evidence");
    let _ = writeln!(
      out,
      "\nStatic migration evidence only: every candidate remains review-required and does not assert runtime trust."
    );
    for boundary in &ffi_boundaries {
      let _ = writeln!(out, "\n### `{}`", boundary.definition);
      let _ = writeln!(out, "\n- classification: `{}`", boundary.classification);
      let _ = writeln!(out, "- target: `{}`", boundary.target);
      let _ = writeln!(out, "- js-ffi feature: `{}`", boundary.js_ffi_feature);
      let _ = writeln!(out, "- callers: `{}`", boundary.callers.join(", "));
      for operation in &boundary.operations {
        let _ = writeln!(
          out,
          "  - `{}` classification=`{}` source=`{}` member=`{}` path=`{}` nullable=`{}`",
          operation.kind,
          operation.classification,
          operation.source,
          operation.member.as_deref().unwrap_or("-"),
          operation.path,
          operation.nullable_evidence
        );
      }
      for helper in &boundary.helper_candidates {
        let _ = writeln!(
          out,
          "  - helper candidate: `{}` origin=`{}` compatibility=`{}`",
          helper.definition, helper.origin, helper.compatibility
        );
      }
      for candidate in &boundary.trait_candidates {
        let _ = writeln!(
          out,
          "  - trait manifest: `{}` receiver=`{}` fields=`{}` methods=`{}` status=`{}`",
          candidate.suggested_name,
          candidate.receiver,
          candidate.fields.join(","),
          candidate.methods.join(","),
          candidate.contract_status
        );
      }
      for candidate in &boundary.adapter_candidates {
        let _ = writeln!(
          out,
          "  - adapter manifest: `{}` source=`{}` status=`{}`",
          candidate.suggested_definition, candidate.source, candidate.contract_status
        );
      }
    }
  }
  let _ = writeln!(out, "- detail:");
  for kind in [
    "schema-dynamic",
    "unresolved-type-slot",
    "code-dynamic",
    "code-nil",
    "unsafe-coerce",
  ] {
    let _ = writeln!(out, "  - {kind}");
    if let Some(items) = detail_count.get(kind) {
      for (detail, count) in items {
        let _ = writeln!(out, "    - {detail}={count}");
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
      if let Some(evidence) = &occurrence.unsafe_evidence {
        let _ = writeln!(
          out,
          "    evidence: source-form={} target={} js-ffi-feature={} raw-adapter-namespace={}",
          evidence.source_form, evidence.target_schema, evidence.js_ffi_feature, evidence.raw_adapter_namespace
        );
      }
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
      "- next: `calcit analyze weak-types --only schema-dynamic,unresolved-type-slot,code-dynamic --intent unresolved --summary-only`, then scope the reported namespaces and rerun without `--summary-only`."
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

  if let CalcitTypeAnnotation::Macro(signature) = schema {
    let Cirru::List(xs) = code else { return vec![] };
    if !matches!(xs.first(), Some(Cirru::Leaf(head)) if head.as_ref() == "defmacro") {
      let code_kind = xs
        .first()
        .and_then(|head| match head {
          Cirru::Leaf(head) => Some(head.as_ref()),
          _ => None,
        })
        .unwrap_or("non-macro definition");
      return vec![format!(
        "[E_SCHEMA_KIND] {ns}/{def_name}: schema :kind is :macro but code uses {code_kind}"
      )];
    }
    let code_shape = analyze_param_shape(xs.get(2));
    let schema_shape = ParamShape {
      required: signature.required_inputs.len(),
      optional: signature.optional_inputs.len(),
      has_rest: signature.rest_input.is_some(),
      errors: vec![],
    };
    return compare_param_shapes(&format!("{ns}/{def_name}"), &code_shape, &schema_shape);
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
      issues.push(format!(
        "[E_SCHEMA_KIND] {ns}/{def_name}: schema :kind is :fn but code uses defmacro"
      ));
    }
    (SchemaKind::Macro, "defn") => {
      issues.push(format!(
        "[E_SCHEMA_KIND] {ns}/{def_name}: schema :kind is :macro but code uses defn"
      ));
    }
    _ => {}
  }

  let mut code_shape = analyze_param_shape(xs.get(2));
  let mut schema_shape = ParamShape::from_schema(&fn_annot.arg_types, fn_annot.rest_type.is_some());
  if code_kind != "defmacro" {
    code_shape = code_shape.as_fixed_arity();
    schema_shape = schema_shape.as_fixed_arity();
  }
  issues.extend(compare_param_shapes(&format!("{ns}/{def_name}"), &code_shape, &schema_shape));

  issues
}

/// Count required params and detect rest param from a defn/defmacro args form.
pub fn analyze_param_arity(args: Option<&Cirru>) -> (usize, bool) {
  let shape = analyze_param_shape(args);
  (shape.required, shape.has_rest)
}

fn analyze_param_shape(args: Option<&Cirru>) -> ParamShape {
  let Some(Cirru::List(xs)) = args else {
    return ParamShape::from_tokens([]);
  };
  ParamShape::from_tokens(xs.iter().filter_map(|item| match item {
    Cirru::Leaf(s) if matches!(s.as_ref(), "[]" | ",") => None,
    Cirru::Leaf(s) if s.as_ref() == "?" => Some(ParamShapeToken::OptionalMark),
    Cirru::Leaf(s) if s.as_ref() == "&" => Some(ParamShapeToken::RestMark),
    _ => Some(ParamShapeToken::Binding),
  }))
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

  if let CalcitTypeAnnotation::Macro(signature) = entry.schema.as_ref() {
    let contract_label = |contract: &MacroSyntaxType| match contract {
      MacroSyntaxType::Syntax => "Syntax".to_owned(),
      MacroSyntaxType::SyntaxSymbol => "SyntaxSymbol".to_owned(),
      MacroSyntaxType::SyntaxList => "SyntaxList".to_owned(),
      MacroSyntaxType::Expr(semantic) => format!("Expr<{}>", semantic.to_brief_string()),
    };
    let mut params = vec![];
    let mut param_annotations = BTreeMap::new();
    for (idx, contract) in signature.required_inputs.iter().enumerate() {
      let name = format!("arg-{}", idx + 1);
      params.push(name.clone());
      param_annotations.insert(name, vec![contract_label(contract)]);
    }
    for (idx, contract) in signature.optional_inputs.iter().enumerate() {
      let name = format!("optional-{}", idx + 1);
      params.push(name.clone());
      param_annotations.insert(name, vec![contract_label(contract)]);
    }
    if let Some(contract) = &signature.rest_input {
      params.push("rest".to_owned());
      param_annotations.insert("rest".to_owned(), vec![contract_label(contract)]);
    }
    let expansion = match &signature.expansion {
      MacroExpansionType::Dynamic => "Dynamic".to_owned(),
      MacroExpansionType::Expr(semantic) => format!("Expr<{}>", semantic.to_brief_string()),
      MacroExpansionType::Definition(semantic) => format!("Definition<{}>", semantic.to_brief_string()),
      MacroExpansionType::Declarations => "Declarations".to_owned(),
    };
    let level = if !matches!(signature.expansion, MacroExpansionType::Dynamic) {
      CoverageLevel::Full
    } else {
      CoverageLevel::Partial
    };
    return TypeCoverageRow {
      ns: ns.to_owned(),
      def: def_name.to_owned(),
      kind: DefKind::Macro,
      level: downgrade_coverage_for_dynamic_annotation(level, entry.schema.as_ref()),
      params,
      param_annotations,
      return_type_hints: vec![expansion],
      generics: signature.generics.iter().map(|name| format!("'{name}")).collect(),
      where_bounds: signature.where_bounds.iter().map(|bound| bound.to_brief_string()).collect(),
      data_type: None,
      schema_issues: entry_schema_issues(ns, def_name, &entry.code, &entry.schema),
    };
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
        let typed_count = count_typed_params(&params, &param_annotations);
        let level = if typed_count > 0 {
          CoverageLevel::Partial
        } else {
          CoverageLevel::None
        };
        (DefKind::Macro, params, param_annotations, Vec::new(), None, level)
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

fn read_schema_param_wrapped(item: &Cirru, default_name: &str, wrap_rest_as_list: bool) -> Option<(String, String)> {
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
      if let Some((name, ty)) = read_schema_param_wrapped(item, &format!("arg{idx}"), false) {
        params.push(name.clone());
        param_annotations.entry(name).or_default().push(ty);
      }
    }
  }

  if let Some(rest_node) = schema.get(":rest")
    && let Some((name, ty)) = read_schema_param_wrapped(rest_node, "rest", true)
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

pub(crate) fn analysis_revision(snapshot: &snapshot::Snapshot, definitions: &[(String, String)]) -> Result<String, String> {
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
  Ok(format!("md5:{}", hex::encode(hasher.finalize())))
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
  let mut diagnostics = vec![];
  if coverage_gaps > 0 {
    diagnostics.push(serde_json::json!({
      "code": "W_TYPE_COVERAGE_GAPS",
      "phase": "analysis",
      "severity": "warning",
      "message": format!("{coverage_gaps} definition(s) lack full static coverage; unresolved dynamic slots can hide generic relations and force runtime method dispatch."),
      "suggestion": "Run `calcit analyze weak-types --only schema-dynamic,unresolved-type-slot,code-dynamic --intent unresolved --format json`; bind required type slots, then prefer concrete types, declared type variables, or trait `:where` bounds.",
    }));
  }
  let ids = rows.iter().map(|row| (row.ns.clone(), row.def.clone())).collect::<Vec<_>>();
  let envelope = serde_json::json!({
    "schema_version": 2,
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
  let ffi_boundaries = if options.ffi_evidence {
    collect_ffi_boundary_evidence(options, snapshot)?
  } else {
    Vec::new()
  };
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
  for kind in [
    "schema-dynamic",
    "unresolved-type-slot",
    "code-dynamic",
    "code-nil",
    "unsafe-coerce",
  ] {
    kinds.entry(kind).or_insert(0);
  }
  for intent in [
    "unresolved",
    "intentional-js-ffi",
    "intentional-macro-syntax",
    "intentional-type-slot-dynamic",
    "explicit-unsafe",
    "declared-unit",
    "declared-optional",
  ] {
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
        "occurrences": row.occurrences.iter().map(|occurrence| {
          let evidence = occurrence.unsafe_evidence.as_ref().map(|evidence| serde_json::json!({
            "source_form": evidence.source_form,
            "target_schema": evidence.target_schema,
            "js_ffi_feature": evidence.js_ffi_feature,
            "raw_adapter_namespace": evidence.raw_adapter_namespace,
          }));
          serde_json::json!({
            "kind": occurrence.kind.as_str(),
            "intent": occurrence.intent.as_str(),
            "detail": occurrence.detail,
            "path": occurrence.path,
            "evidence": evidence,
            "suggestion": weak_type_suggestion(occurrence),
          })
        }).collect::<Vec<_>>(),
      })
    })
    .collect::<Vec<_>>();
  let mut ids = rows.iter().map(|row| (row.ns.clone(), row.def.clone())).collect::<Vec<_>>();
  for boundary in &ffi_boundaries {
    if let Some((namespace, definition)) = boundary.definition.split_once('/') {
      ids.push((namespace.to_owned(), definition.to_owned()));
    }
  }
  ids.sort();
  ids.dedup();
  let hit_count = rows.iter().map(|row| row.occurrences.len()).sum::<usize>();
  let unresolved_dynamic = rows
    .iter()
    .flat_map(|row| row.occurrences.iter())
    .filter(|occurrence| {
      occurrence.intent == WeakTypeIntent::Unresolved
        && matches!(occurrence.kind, WeakTypeKind::SchemaDynamic | WeakTypeKind::CodeDynamic)
    })
    .count();
  let unresolved_type_slots = rows
    .iter()
    .flat_map(|row| row.occurrences.iter())
    .filter(|occurrence| occurrence.intent == WeakTypeIntent::Unresolved && occurrence.kind == WeakTypeKind::UnresolvedTypeSlot)
    .count();
  let nil_debt = rows
    .iter()
    .flat_map(|row| row.occurrences.iter())
    .filter(|occurrence| {
      occurrence.kind == WeakTypeKind::CodeNil
        && matches!(
          occurrence.intent,
          WeakTypeIntent::Unresolved | WeakTypeIntent::DeclaredUnit | WeakTypeIntent::DeclaredOptional
        )
    })
    .count();
  let unsafe_coercions = rows
    .iter()
    .flat_map(|row| row.occurrences.iter())
    .filter(|occurrence| occurrence.kind == WeakTypeKind::UnsafeCoerce)
    .count();
  let mut diagnostics = vec![];
  if unresolved_dynamic > 0 {
    diagnostics.push(serde_json::json!({
      "code": "W_DYNAMIC_TYPE_DEBT",
      "phase": "analysis",
      "severity": "warning",
      "message": format!("{unresolved_dynamic} unresolved dynamic slot(s) erase static relationships used by generic binding, callback checking, and method specialization."),
      "suggestion": "Prefer concrete types; use `:generics` when positions share a type and trait `:where` bounds when only capabilities are required. Keep `:dynamic` only at documented boundaries.",
    }));
  }
  if unresolved_type_slots > 0 {
    diagnostics.push(serde_json::json!({
      "code": "W_UNRESOLVED_TYPE_SLOT",
      "phase": "analysis",
      "severity": "warning",
      "message": format!("{unresolved_type_slots} type slot(s) are unbound for the selected entry and therefore accept every value as though they were Dynamic."),
      "suggestion": "Bind each slot with `calcit config set-type-slot <slot> <namespace/definition>`, or explicitly select `:dynamic` only for a documented open boundary.",
    }));
  }
  if nil_debt > 0 {
    diagnostics.push(serde_json::json!({
      "code": "W_NIL_TYPE_DEBT",
      "phase": "analysis",
      "severity": "warning",
      "message": format!("{nil_debt} nil occurrence(s) are unresolved, violate Unit returns, or are covered only by Optional compatibility contracts."),
      "suggestion": "Use `&unit` for no-value returns. Prefer Option for application absence and Result when failure details matter; keep Optional only at compatibility or FFI boundaries.",
    }));
  }
  if unsafe_coercions > 0 {
    diagnostics.push(serde_json::json!({
      "code": "W_JS_FFI_UNCHECKED_COERCE",
      "phase": "analysis",
      "severity": "warning",
      "message": format!("{unsafe_coercions} explicit unsafe coercion(s) bypass static compatibility and need an audited runtime contract when they cross an untrusted boundary."),
      "suggestion": "Keep each assertion at a minimal trusted boundary, validate or normalize untrusted runtime data before application code consumes it, and add positive and negative runtime-contract tests.",
    }));
  }
  let ffi_boundary_count = ffi_boundaries.len();
  let ffi_boundary_rows = if options.summary_only { Vec::new() } else { ffi_boundaries };
  let envelope = serde_json::json!({
    "schema_version": 7,
    "command": "analyze.weak-types",
    "revision": analysis_revision(snapshot, &ids)?,
    "data": {
      "filters": {
        "namespace": options.ns,
        "namespace_prefix": options.ns_prefix,
        "only": options.only,
        "intent": options.intent,
        "include_dependencies": options.deps,
        "ffi_evidence": options.ffi_evidence,
        "summary_only": options.summary_only,
      },
      "summary": {
        "namespaces": namespaces.len(),
        "definitions": rows.len(),
        "hits": hit_count,
        "ffi_boundaries": ffi_boundary_count,
        "kinds": kinds,
        "intents": intents,
      },
      "definitions": definitions,
      "evidence": {
        "ffi_boundaries": ffi_boundary_rows,
        "contract_status": "review-required",
        "runtime_trust_inferred": false,
      },
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

#[cfg(test)]
mod tests {
  use std::collections::HashSet;
  use std::sync::Arc;

  use calcit::calcit::{
    CalcitTypeAnnotation, MacroExpansionType, MacroSignature, MacroSyntaxType, clear_type_slots, push_type_slot_override,
  };
  use cirru_parser::Cirru;

  use super::{
    WeakTypeIntent, WeakTypeKind, classify_unsafe_coerce_source, entry_schema_issues, is_raw_adapter_namespace,
    scan_schema_dynamic_annotation,
  };

  fn leaf(value: &str) -> Cirru {
    Cirru::Leaf(Arc::from(value))
  }

  fn list(items: Vec<Cirru>) -> Cirru {
    Cirru::List(items)
  }

  fn scan(annotation: CalcitTypeAnnotation) -> Vec<super::WeakTypeOccurrence> {
    let mut occurrences = vec![];
    scan_schema_dynamic_annotation(&annotation, "schema", "root", &mut occurrences);
    occurrences
  }

  fn macro_schema(
    required_inputs: Vec<MacroSyntaxType>,
    rest_input: Option<MacroSyntaxType>,
    expansion: MacroExpansionType,
  ) -> CalcitTypeAnnotation {
    CalcitTypeAnnotation::Macro(Arc::new(MacroSignature {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      required_inputs: Arc::new(required_inputs),
      optional_inputs: Arc::new(vec![]),
      rest_input,
      expansion,
      capabilities: Arc::new(HashSet::new()),
      features: Arc::new(HashSet::new()),
    }))
  }

  #[test]
  fn open_expr_positions_in_strict_macros_have_intentional_syntax_intent() {
    let annotation = macro_schema(
      vec![MacroSyntaxType::Expr(Arc::new(CalcitTypeAnnotation::Dynamic))],
      Some(MacroSyntaxType::Expr(Arc::new(CalcitTypeAnnotation::Dynamic))),
      MacroExpansionType::Expr(Arc::new(CalcitTypeAnnotation::Dynamic)),
    );

    let occurrences = scan(annotation.clone());

    assert_eq!(occurrences.len(), 3);
    assert!(
      occurrences
        .iter()
        .all(|occurrence| occurrence.intent == WeakTypeIntent::IntentionalMacroSyntax)
    );
    let issues = entry_schema_issues(
      "app.main",
      "open-macro",
      &list(vec![
        leaf("defmacro"),
        leaf("open-macro"),
        list(vec![leaf("x"), leaf("&"), leaf("xs")]),
        leaf("body"),
      ]),
      &Arc::new(annotation),
    );
    assert!(
      issues.is_empty(),
      "reviewed Expr<Dynamic> boundaries should not be unresolved: {issues:?}"
    );
  }

  #[test]
  fn whole_dynamic_macro_expansions_and_definitions_remain_unresolved() {
    let dynamic_expansion = scan(macro_schema(vec![], None, MacroExpansionType::Dynamic));
    let dynamic_definition = scan(macro_schema(
      vec![],
      None,
      MacroExpansionType::Definition(Arc::new(CalcitTypeAnnotation::Dynamic)),
    ));

    assert_eq!(dynamic_expansion.len(), 1);
    assert_eq!(dynamic_expansion[0].intent, WeakTypeIntent::Unresolved);
    assert_eq!(dynamic_definition.len(), 1);
    assert_eq!(dynamic_definition[0].intent, WeakTypeIntent::Unresolved);
  }

  #[test]
  fn nested_unresolved_slots_keep_unresolved_intent() {
    clear_type_slots();
    push_type_slot_override(
      Arc::from("outer"),
      Arc::new(CalcitTypeAnnotation::JsNullish(Arc::new(CalcitTypeAnnotation::TypeSlot(
        Arc::from("inner"),
      )))),
    );

    let occurrences = scan(CalcitTypeAnnotation::TypeSlot(Arc::from("outer")));

    assert_eq!(occurrences.len(), 1);
    assert_eq!(occurrences[0].kind, WeakTypeKind::UnresolvedTypeSlot);
    assert_eq!(occurrences[0].intent, WeakTypeIntent::Unresolved);
    clear_type_slots();
  }

  #[test]
  fn nested_js_nullish_dynamic_keeps_ffi_intent() {
    clear_type_slots();
    push_type_slot_override(
      Arc::from("outer"),
      Arc::new(CalcitTypeAnnotation::JsNullish(Arc::new(CalcitTypeAnnotation::Dynamic))),
    );

    let occurrences = scan(CalcitTypeAnnotation::TypeSlot(Arc::from("outer")));

    assert_eq!(occurrences.len(), 1);
    assert_eq!(occurrences[0].kind, WeakTypeKind::SchemaDynamic);
    assert_eq!(occurrences[0].intent, WeakTypeIntent::IntentionalJsFfi);
    clear_type_slots();
  }

  #[test]
  fn resolved_dynamic_slots_receive_slot_intent() {
    clear_type_slots();
    push_type_slot_override(Arc::from("slot"), Arc::new(CalcitTypeAnnotation::Dynamic));

    let occurrences = scan(CalcitTypeAnnotation::TypeSlot(Arc::from("slot")));

    assert_eq!(occurrences.len(), 1);
    assert_eq!(occurrences[0].kind, WeakTypeKind::SchemaDynamic);
    assert_eq!(occurrences[0].intent, WeakTypeIntent::IntentionalTypeSlotDynamic);
    clear_type_slots();
  }

  #[test]
  fn unsafe_coerce_source_forms_and_adapter_names_are_static_and_distinct() {
    assert_eq!(
      classify_unsafe_coerce_source(&[Cirru::leaf("unsafe-coerce"), Cirru::leaf("js/window"), Cirru::leaf("Number")]),
      "raw-js-value"
    );
    assert_eq!(
      classify_unsafe_coerce_source(&[
        Cirru::leaf("unsafe-coerce"),
        Cirru::List(vec![Cirru::leaf("js/fetch"), Cirru::leaf("url")]),
        Cirru::leaf("Response"),
      ]),
      "raw-js-operation"
    );
    assert_eq!(
      classify_unsafe_coerce_source(&[
        Cirru::leaf("unsafe-coerce"),
        Cirru::List(vec![Cirru::leaf("aget"), Cirru::leaf("host"), Cirru::leaf("key")]),
        Cirru::leaf("String"),
      ]),
      "host-member-operation"
    );
    assert_eq!(
      classify_unsafe_coerce_source(&[Cirru::leaf("unsafe-coerce"), Cirru::leaf("value"), Cirru::leaf("String")]),
      "value"
    );
    assert!(is_raw_adapter_namespace("js-ffi.raw.node"));
    assert!(is_raw_adapter_namespace("app.js-ffi.raw.browser"));
    assert!(!is_raw_adapter_namespace("app.js-ffi.host.browser"));
  }
}
