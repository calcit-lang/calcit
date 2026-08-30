use md5::{Digest, Md5};
use serde::Serialize;

use crate::calcit::{CalcitTypeAnnotation, SchemaKind};
use crate::data::edn::format_edn_display;
use crate::snapshot::{CodeEntry, Snapshot};
use cirru_edn::Edn;

pub const FFI_INTERFACE_IR_VERSION: u32 = 1;
pub const FFI_INTERFACE_IR_SCHEMA_ID: &str = "https://calcit-lang.org/schemas/ffi-interface-ir-v1.schema.json";
pub const FFI_INTERFACE_IR_SCHEMA: &str = include_str!("../schemas/ffi-interface-ir-v1.schema.json");

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FfiInterfaceDocument {
  pub version: u32,
  pub package: String,
  pub package_version: String,
  pub definitions: Vec<FfiDefinitionIr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FfiDefinitionIr {
  pub id: String,
  pub namespace: String,
  pub name: String,
  pub doc: String,
  pub logical_schema: String,
  pub signature: Option<FfiFunctionSignatureIr>,
  pub lowering: FfiLoweringIr,
  pub status: FfiDefinitionStatus,
  pub diagnostic_codes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FfiDefinitionStatus {
  Supported,
  Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FfiFunctionSignatureIr {
  pub parameters: Vec<FfiParameterIr>,
  pub result: FfiTypeIr,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FfiParameterIr {
  pub position: usize,
  #[serde(rename = "type")]
  pub type_ir: FfiTypeIr,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum FfiTypeIr {
  Unit,
  Bool,
  Number,
  String,
  Buffer,
  List { item: Box<FfiTypeIr> },
  Named { name: String, arguments: Vec<FfiTypeIr> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FfiLoweringIr {
  pub backend: Option<String>,
  pub target: Option<String>,
  pub kind: Option<String>,
  pub symbol: Option<String>,
  pub invoke: Option<String>,
  pub transport: Option<String>,
  pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FfiInterfaceDiagnostic {
  pub code: String,
  pub phase: String,
  pub severity: String,
  pub definition: String,
  pub path: String,
  pub message: String,
  pub suggestion: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FfiExportSummary {
  pub definitions: usize,
  pub supported: usize,
  pub unsupported: usize,
  pub diagnostics: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FfiExportReport {
  pub revision: String,
  pub interface: FfiInterfaceDocument,
  pub summary: FfiExportSummary,
  pub diagnostics: Vec<FfiInterfaceDiagnostic>,
}

fn diagnostic(
  definition: &str,
  path: impl Into<String>,
  code: &str,
  message: impl Into<String>,
  suggestion: &str,
) -> FfiInterfaceDiagnostic {
  FfiInterfaceDiagnostic {
    code: code.to_owned(),
    phase: "ffi-interface-ir".to_owned(),
    severity: "error".to_owned(),
    definition: definition.to_owned(),
    path: path.into(),
    message: message.into(),
    suggestion: suggestion.to_owned(),
  }
}

fn named_type(
  name: &str,
  arguments: &[std::sync::Arc<CalcitTypeAnnotation>],
  definition: &str,
  path: &str,
) -> Result<FfiTypeIr, Box<FfiInterfaceDiagnostic>> {
  let mut converted = Vec::with_capacity(arguments.len());
  for (index, argument) in arguments.iter().enumerate() {
    converted.push(convert_type(argument, definition, &format!("{path}.arguments.{index}"))?);
  }
  Ok(FfiTypeIr::Named {
    name: name.trim_start_matches('\'').to_owned(),
    arguments: converted,
  })
}

fn convert_type(annotation: &CalcitTypeAnnotation, definition: &str, path: &str) -> Result<FfiTypeIr, Box<FfiInterfaceDiagnostic>> {
  match annotation {
    CalcitTypeAnnotation::Unit => Ok(FfiTypeIr::Unit),
    CalcitTypeAnnotation::Bool => Ok(FfiTypeIr::Bool),
    CalcitTypeAnnotation::Number => Ok(FfiTypeIr::Number),
    CalcitTypeAnnotation::String => Ok(FfiTypeIr::String),
    CalcitTypeAnnotation::Buffer => Ok(FfiTypeIr::Buffer),
    CalcitTypeAnnotation::List(item) => Ok(FfiTypeIr::List {
      item: Box::new(convert_type(item, definition, &format!("{path}.item"))?),
    }),
    CalcitTypeAnnotation::TypeRef(name, arguments) => named_type(name, arguments, definition, path),
    CalcitTypeAnnotation::Struct(struct_def, arguments) => named_type(struct_def.name.ref_str(), arguments, definition, path),
    CalcitTypeAnnotation::Enum(enum_def, arguments) => named_type(enum_def.name().ref_str(), arguments, definition, path),
    CalcitTypeAnnotation::StructValue(struct_def) => Ok(FfiTypeIr::Named {
      name: struct_def.name.ref_str().to_owned(),
      arguments: vec![],
    }),
    CalcitTypeAnnotation::EnumValue(enum_def) => Ok(FfiTypeIr::Named {
      name: enum_def.name().ref_str().to_owned(),
      arguments: vec![],
    }),
    unsupported => Err(Box::new(diagnostic(
      definition,
      path,
      "E_FFI_IR_UNSUPPORTED_TYPE",
      format!(
        "FFI Interface IR v{FFI_INTERFACE_IR_VERSION} cannot represent Calcit type `{}` at `{path}`.",
        unsupported.to_brief_string()
      ),
      "Use Unit, Bool, Number, String, Buffer, List, Struct, Enum, Option, Result, or another nominal named type; keep Dynamic, callbacks, Map/Set, Ref, and host objects behind a handwritten adapter.",
    ))),
  }
}

fn convert_signature(entry: &CodeEntry, definition: &str) -> Result<FfiFunctionSignatureIr, Vec<FfiInterfaceDiagnostic>> {
  let CalcitTypeAnnotation::Fn(signature) = entry.schema.as_ref() else {
    return Err(vec![diagnostic(
      definition,
      "logical_schema",
      "E_FFI_IR_UNSUPPORTED_SCHEMA",
      format!(
        "FFI Interface IR v{FFI_INTERFACE_IR_VERSION} currently exports function schemas, but `{definition}` has `{}`.",
        entry.schema.to_brief_string()
      ),
      "Expose a typed function at the raw binding boundary; keep macros, traits, and data declarations outside the generated callable inventory.",
    )]);
  };

  let mut diagnostics = Vec::new();
  if signature.fn_kind != SchemaKind::Fn {
    diagnostics.push(diagnostic(
      definition,
      "logical_schema.kind",
      "E_FFI_IR_UNSUPPORTED_SCHEMA",
      "Macro-kind function schemas cannot be exported as runtime FFI calls.",
      "Use a runtime Fn schema for the raw FFI binding.",
    ));
  }
  if !signature.generics.is_empty() || !signature.where_bounds.is_empty() {
    diagnostics.push(diagnostic(
      definition,
      "logical_schema.generics",
      "E_FFI_IR_UNSUPPORTED_GENERIC",
      "Generic and trait-bounded FFI call signatures are not part of Interface IR v1.",
      "Expose a monomorphic raw binding and keep generic normalization in handwritten Calcit code.",
    ));
  }
  if signature.rest_type.is_some() {
    diagnostics.push(diagnostic(
      definition,
      "logical_schema.rest",
      "E_FFI_IR_UNSUPPORTED_REST",
      "Variadic FFI call signatures are not part of Interface IR v1.",
      "Expose a fixed-arity raw binding, using a typed List or Tuple when the host needs multiple values.",
    ));
  }

  let mut parameters = Vec::with_capacity(signature.arg_types.len());
  for (position, annotation) in signature.arg_types.iter().enumerate() {
    match convert_type(annotation, definition, &format!("signature.parameters.{position}.type")) {
      Ok(type_ir) => parameters.push(FfiParameterIr { position, type_ir }),
      Err(error) => diagnostics.push(*error),
    }
  }
  let result = match convert_type(&signature.return_type, definition, "signature.result") {
    Ok(result) => Some(result),
    Err(error) => {
      diagnostics.push(*error);
      None
    }
  };

  if diagnostics.is_empty() {
    Ok(FfiFunctionSignatureIr {
      parameters,
      result: result.expect("successful signature conversion has a result"),
    })
  } else {
    Err(diagnostics)
  }
}

fn metadata_value<'a>(metadata: &'a Edn, key: &str) -> Option<&'a Edn> {
  match metadata {
    Edn::Struct(value) => value.pairs.iter().find(|(field, _)| field.ref_str() == key).map(|(_, value)| value),
    Edn::Map(value) => value.get(&Edn::tag(key)),
    _ => None,
  }
}

fn is_ffi_boundary_candidate(metadata: &Edn) -> bool {
  match metadata {
    Edn::Map(_) | Edn::Struct(_) => ["backend", "target", "kind", "symbol", "invoke", "transport"]
      .iter()
      .any(|key| metadata_value(metadata, key).is_some()),
    _ => true,
  }
}

fn scalar_metadata(metadata: &Edn, key: &str, definition: &str, diagnostics: &mut Vec<FfiInterfaceDiagnostic>) -> Option<String> {
  let value = metadata_value(metadata, key)?;
  match value {
    Edn::Tag(value) => Some(value.ref_str().to_owned()),
    Edn::Str(value) | Edn::Symbol(value) => Some(value.trim_start_matches(':').to_owned()),
    _ => {
      diagnostics.push(diagnostic(
        definition,
        format!("lowering.{key}"),
        "E_FFI_IR_METADATA_TYPE",
        format!("FFI lowering field `{key}` must be a tag, symbol, or string."),
        "Use a stable scalar value in `:ffi` metadata so generators can compare it deterministically.",
      ));
      None
    }
  }
}

fn convert_lowering(metadata: &Edn, definition: &str) -> Result<(FfiLoweringIr, Vec<FfiInterfaceDiagnostic>), String> {
  let mut diagnostics = Vec::new();
  if !matches!(metadata, Edn::Struct(_) | Edn::Map(_)) {
    diagnostics.push(diagnostic(
      definition,
      "lowering",
      "E_FFI_IR_METADATA_SHAPE",
      "FFI lowering metadata must be a map or struct.",
      "Store backend lowering as `{} (:backend :native) (:symbol |name)` or the equivalent typed metadata struct.",
    ));
  }
  let backend = scalar_metadata(metadata, "backend", definition, &mut diagnostics);
  if backend.is_none() {
    diagnostics.push(diagnostic(
      definition,
      "lowering.backend",
      "E_FFI_IR_BACKEND_REQUIRED",
      "FFI lowering metadata does not declare a backend.",
      "Declare `:backend :native`, `:backend :js`, or another explicit backend before generation.",
    ));
  }
  Ok((
    FfiLoweringIr {
      backend,
      target: scalar_metadata(metadata, "target", definition, &mut diagnostics),
      kind: scalar_metadata(metadata, "kind", definition, &mut diagnostics),
      symbol: scalar_metadata(metadata, "symbol", definition, &mut diagnostics),
      invoke: scalar_metadata(metadata, "invoke", definition, &mut diagnostics),
      transport: scalar_metadata(metadata, "transport", definition, &mut diagnostics),
      raw: format_edn_display(metadata),
    },
    diagnostics,
  ))
}

pub fn export_snapshot(snapshot: &Snapshot, namespace: Option<&str>) -> Result<FfiExportReport, String> {
  let mut candidates = snapshot
    .files
    .iter()
    .filter(|(ns, _)| namespace.is_none_or(|filter| ns.as_str() == filter))
    .flat_map(|(ns, file)| {
      file.defs.iter().filter_map(move |(name, entry)| {
        entry
          .ffi
          .as_ref()
          .filter(|ffi| is_ffi_boundary_candidate(ffi))
          .map(|ffi| (ns.as_str(), name.as_str(), entry, ffi))
      })
    })
    .collect::<Vec<_>>();
  candidates.sort_unstable_by(|left, right| (left.0, left.1).cmp(&(right.0, right.1)));

  let mut definitions = Vec::with_capacity(candidates.len());
  let mut diagnostics = Vec::new();
  for (namespace, name, entry, metadata) in candidates {
    let id = format!("{namespace}/{name}");
    let definition_diagnostic_start = diagnostics.len();
    let signature = match convert_signature(entry, &id) {
      Ok(signature) => Some(signature),
      Err(errors) => {
        diagnostics.extend(errors);
        None
      }
    };
    let (lowering, lowering_diagnostics) = convert_lowering(metadata, &id)?;
    diagnostics.extend(lowering_diagnostics);
    let definition_diagnostics = &diagnostics[definition_diagnostic_start..];
    let mut diagnostic_codes = definition_diagnostics.iter().map(|item| item.code.clone()).collect::<Vec<_>>();
    diagnostic_codes.sort_unstable();
    diagnostic_codes.dedup();
    let status = if diagnostic_codes.is_empty() {
      FfiDefinitionStatus::Supported
    } else {
      FfiDefinitionStatus::Unsupported
    };
    definitions.push(FfiDefinitionIr {
      id,
      namespace: namespace.to_owned(),
      name: name.to_owned(),
      doc: entry.doc.clone(),
      logical_schema: format_edn_display(&entry.schema.to_type_edn()),
      signature,
      lowering,
      status,
      diagnostic_codes,
    });
  }

  let supported = definitions
    .iter()
    .filter(|definition| definition.status == FfiDefinitionStatus::Supported)
    .count();
  let interface = FfiInterfaceDocument {
    version: FFI_INTERFACE_IR_VERSION,
    package: snapshot.package.clone(),
    package_version: snapshot.version.clone(),
    definitions,
  };
  let summary = FfiExportSummary {
    definitions: interface.definitions.len(),
    supported,
    unsupported: interface.definitions.len() - supported,
    diagnostics: diagnostics.len(),
  };
  let revision_payload = serde_json::to_vec(&(&interface, &diagnostics))
    .map_err(|error| format!("Failed to encode FFI Interface IR revision input: {error}"))?;
  let mut hasher = Md5::new();
  hasher.update(revision_payload);
  let revision = format!("md5:{}", hex::encode(hasher.finalize()));
  Ok(FfiExportReport {
    revision,
    interface,
    summary,
    diagnostics,
  })
}

pub fn format_human_report(report: &FfiExportReport) -> String {
  let mut output = format!(
    "Calcit FFI Interface IR v{}\n- package: {} {}\n- revision: {}\n- definitions: {} ({} supported, {} unsupported)\n",
    report.interface.version,
    report.interface.package,
    report.interface.package_version,
    report.revision,
    report.summary.definitions,
    report.summary.supported,
    report.summary.unsupported,
  );
  for definition in &report.interface.definitions {
    output.push_str(&format!(
      "- {} [{}] backend={} symbol={}\n",
      definition.id,
      match definition.status {
        FfiDefinitionStatus::Supported => "supported",
        FfiDefinitionStatus::Unsupported => "unsupported",
      },
      definition.lowering.backend.as_deref().unwrap_or("<missing>"),
      definition.lowering.symbol.as_deref().unwrap_or("<none>"),
    ));
  }
  if !report.diagnostics.is_empty() {
    output.push_str("Diagnostics:\n");
    for item in &report.diagnostics {
      output.push_str(&format!("- {} {} {}: {}\n", item.code, item.definition, item.path, item.message));
    }
  }
  output
}

#[cfg(test)]
mod tests {
  use std::collections::{HashMap, HashSet};
  use std::sync::Arc;

  use cirru_edn::Edn;
  use cirru_parser::Cirru;

  use super::*;
  use crate::calcit::{CalcitFnTypeAnnotation, DYNAMIC_TYPE};
  use crate::snapshot::{CodeEntry, FileInSnapShot, NsEntry};

  fn function_entry(args: Vec<Arc<CalcitTypeAnnotation>>, result: Arc<CalcitTypeAnnotation>, ffi: Edn) -> CodeEntry {
    CodeEntry {
      doc: "test binding".to_owned(),
      examples: vec![],
      tests: vec![],
      tags: HashSet::new(),
      code: Cirru::List(vec![]),
      schema: Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types: args,
        return_type: result,
        fn_kind: SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(HashSet::new()),
      }))),
      ffi: Some(ffi),
    }
  }

  fn snapshot(definitions: Vec<(&str, CodeEntry)>) -> Snapshot {
    Snapshot {
      package: "test-package".to_owned(),
      about: None,
      version: "0.1.0".to_owned(),
      entries: HashMap::new(),
      files: HashMap::from([(
        "test.ffi".to_owned(),
        FileInSnapShot {
          ns: NsEntry {
            doc: String::new(),
            code: Cirru::List(vec![]),
          },
          defs: definitions.into_iter().map(|(name, entry)| (name.to_owned(), entry)).collect(),
        },
      )]),
      active_entry: "default".to_owned(),
    }
  }

  fn native_metadata(symbol: &str) -> Edn {
    Edn::map_from_iter([(Edn::tag("backend"), Edn::tag("native")), (Edn::tag("symbol"), Edn::str(symbol))])
  }

  fn diagnostic_codes(report: &FfiExportReport) -> HashSet<&str> {
    report.diagnostics.iter().map(|diagnostic| diagnostic.code.as_str()).collect()
  }

  #[test]
  fn exports_supported_functions_in_definition_order() {
    let report = export_snapshot(
      &snapshot(vec![
        (
          "z-last",
          function_entry(
            vec![Arc::new(CalcitTypeAnnotation::String)],
            Arc::new(CalcitTypeAnnotation::Bool),
            native_metadata("z_last"),
          ),
        ),
        (
          "a-first",
          function_entry(
            vec![Arc::new(CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Number)))],
            Arc::new(CalcitTypeAnnotation::Unit),
            native_metadata("a_first"),
          ),
        ),
      ]),
      None,
    )
    .expect("export supported FFI definitions");

    assert_eq!(report.interface.version, FFI_INTERFACE_IR_VERSION);
    assert_eq!(report.interface.definitions[0].id, "test.ffi/a-first");
    assert_eq!(report.interface.definitions[1].id, "test.ffi/z-last");
    assert_eq!(report.summary.supported, 2);
    assert_eq!(report.summary.unsupported, 0);
    assert!(report.diagnostics.is_empty());
    assert!(report.revision.starts_with("md5:"));
  }

  #[test]
  fn emits_deterministic_diagnostics_without_dynamic_fallback() {
    let report = export_snapshot(
      &snapshot(vec![(
        "dynamic-call",
        function_entry(
          vec![DYNAMIC_TYPE.clone()],
          DYNAMIC_TYPE.clone(),
          Edn::map_from_iter([(Edn::tag("symbol"), Edn::str("dynamic_call"))]),
        ),
      )]),
      None,
    )
    .expect("export unsupported FFI definition inventory");

    assert_eq!(report.summary.supported, 0);
    assert_eq!(report.summary.unsupported, 1);
    assert!(report.interface.definitions[0].signature.is_none());
    let codes = diagnostic_codes(&report);
    assert!(codes.contains("E_FFI_IR_UNSUPPORTED_TYPE"));
    assert!(codes.contains("E_FFI_IR_BACKEND_REQUIRED"));
  }

  #[test]
  fn ignores_snapshot_placeholders_and_capability_only_metadata() {
    let report = export_snapshot(
      &snapshot(vec![
        (
          "empty-wrapper",
          function_entry(
            vec![Arc::new(CalcitTypeAnnotation::String)],
            Arc::new(CalcitTypeAnnotation::String),
            Edn::map_from_iter(Vec::<(Edn, Edn)>::new()),
          ),
        ),
        (
          "capability-wrapper",
          function_entry(
            vec![Arc::new(CalcitTypeAnnotation::String)],
            Arc::new(CalcitTypeAnnotation::String),
            Edn::map_from_iter([(Edn::tag("features"), Edn::tag("js-ffi"))]),
          ),
        ),
      ]),
      None,
    )
    .expect("ignore non-lowering FFI metadata");

    assert_eq!(report.summary.definitions, 0);
    assert!(report.diagnostics.is_empty());
  }

  #[test]
  fn reports_malformed_non_container_metadata() {
    let report = export_snapshot(
      &snapshot(vec![(
        "malformed",
        function_entry(
          vec![Arc::new(CalcitTypeAnnotation::String)],
          Arc::new(CalcitTypeAnnotation::String),
          Edn::str("native"),
        ),
      )]),
      None,
    )
    .expect("inventory malformed FFI metadata");

    let codes = diagnostic_codes(&report);
    assert_eq!(report.summary.unsupported, 1);
    assert!(codes.contains("E_FFI_IR_METADATA_SHAPE"));
    assert!(codes.contains("E_FFI_IR_BACKEND_REQUIRED"));
  }

  #[test]
  fn namespace_filter_is_exact_and_revision_is_stable() {
    let source = snapshot(vec![(
      "read",
      function_entry(
        vec![Arc::new(CalcitTypeAnnotation::String)],
        Arc::new(CalcitTypeAnnotation::String),
        native_metadata("read"),
      ),
    )]);
    let first = export_snapshot(&source, Some("test.ffi")).expect("first export");
    let second = export_snapshot(&source, Some("test.ffi")).expect("second export");
    let empty = export_snapshot(&source, Some("test.other")).expect("filtered export");

    assert_eq!(first, second);
    assert_eq!(empty.summary.definitions, 0);
  }

  #[test]
  fn bundled_json_schema_matches_interface_version() {
    let schema: serde_json::Value = serde_json::from_str(FFI_INTERFACE_IR_SCHEMA).expect("parse bundled FFI Interface IR schema");

    assert_eq!(schema["$id"], FFI_INTERFACE_IR_SCHEMA_ID);
    assert_eq!(schema["properties"]["version"]["const"], FFI_INTERFACE_IR_VERSION);
  }
}
