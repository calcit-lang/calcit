use std::collections::{BTreeMap, BTreeSet, VecDeque};

use md5::{Digest, Md5};
use serde::Serialize;

use crate::calcit::{Calcit, CalcitEnumDef, CalcitStructDef, CalcitTypeAnnotation, SchemaKind};
use crate::data::cirru::code_to_calcit;
use crate::snapshot::{CodeEntry, Snapshot};
use cirru_edn::Edn;
use cirru_parser::Cirru;

pub const FFI_INTERFACE_IR_VERSION: u32 = 2;
pub const FFI_INTERFACE_IR_SCHEMA_ID: &str = "https://calcit-lang.org/schemas/ffi-interface-ir-v2.schema.json";
pub const FFI_INTERFACE_IR_SCHEMA: &str = include_str!("../schemas/ffi-interface-ir-v2.schema.json");

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FfiInterfaceDocument {
  pub version: u32,
  pub package: String,
  pub package_version: String,
  pub declarations: Vec<FfiTypeDeclarationIr>,
  pub definitions: Vec<FfiDefinitionIr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum FfiTypeDeclarationIr {
  Struct {
    id: String,
    namespace: String,
    name: String,
    type_parameters: Vec<String>,
    fields: Vec<FfiStructFieldIr>,
  },
  Enum {
    id: String,
    namespace: String,
    name: String,
    type_parameters: Vec<String>,
    variants: Vec<FfiEnumVariantIr>,
  },
}

impl FfiTypeDeclarationIr {
  fn id(&self) -> &str {
    match self {
      Self::Struct { id, .. } | Self::Enum { id, .. } => id,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FfiStructFieldIr {
  pub name: String,
  #[serde(rename = "type")]
  pub type_ir: FfiTypeIr,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FfiEnumVariantIr {
  pub name: String,
  pub payload: Vec<FfiTypeIr>,
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
  Option { item: Box<FfiTypeIr> },
  Result { ok: Box<FfiTypeIr>, error: Box<FfiTypeIr> },
  Struct { id: String, arguments: Vec<FfiTypeIr> },
  Enum { id: String, arguments: Vec<FfiTypeIr> },
  TypeParameter { name: String },
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

#[derive(Debug, Clone)]
enum LocalTypeDeclaration {
  Struct {
    id: String,
    source_id: String,
    namespace: String,
    nominal: CalcitStructDef,
  },
  Enum {
    id: String,
    source_id: String,
    namespace: String,
    nominal: CalcitEnumDef,
  },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalTypeDeclarationKind {
  Struct,
  Enum,
}

impl LocalTypeDeclaration {
  fn id(&self) -> &str {
    match self {
      Self::Struct { id, .. } | Self::Enum { id, .. } => id,
    }
  }

  fn namespace(&self) -> &str {
    match self {
      Self::Struct { namespace, .. } | Self::Enum { namespace, .. } => namespace,
    }
  }

  fn source_id(&self) -> &str {
    match self {
      Self::Struct { source_id, .. } | Self::Enum { source_id, .. } => source_id,
    }
  }

  fn kind(&self) -> LocalTypeDeclarationKind {
    match self {
      Self::Struct { .. } => LocalTypeDeclarationKind::Struct,
      Self::Enum { .. } => LocalTypeDeclarationKind::Enum,
    }
  }

  fn type_parameters(&self) -> &[std::sync::Arc<str>] {
    match self {
      Self::Struct { nominal, .. } => nominal.generics.as_ref(),
      Self::Enum { nominal, .. } => nominal.generics(),
    }
  }
}

type LocalTypeDeclarations = BTreeMap<String, Vec<LocalTypeDeclaration>>;

struct TypeConversionContext<'a> {
  declarations: &'a LocalTypeDeclarations,
  required: &'a mut BTreeSet<String>,
  current_namespace: &'a str,
  type_parameters: &'a BTreeSet<String>,
}

fn normalized_type_name(name: &str) -> &str {
  name.trim_start_matches('\'')
}

fn explicit_builtin_type(name: &str) -> Option<&str> {
  match normalized_type_name(name) {
    "Option" | "calcit.core/Option" => Some("Option"),
    "Result" | "calcit.core/Result" => Some("Result"),
    _ => None,
  }
}

fn core_host_managed_type(name: &str) -> Option<&str> {
  match normalized_type_name(name) {
    "FfiTask" | "calcit.core/FfiTask" => Some("FfiTask"),
    "FfiResponse" | "calcit.core/FfiResponse" => Some("FfiResponse"),
    _ => None,
  }
}

fn convert_type_arguments(
  arguments: &[std::sync::Arc<CalcitTypeAnnotation>],
  definition: &str,
  path: &str,
  context: &mut TypeConversionContext<'_>,
) -> Result<Vec<FfiTypeIr>, Box<FfiInterfaceDiagnostic>> {
  arguments
    .iter()
    .enumerate()
    .map(|(index, argument)| convert_type(argument, definition, &format!("{path}.arguments.{index}"), context))
    .collect()
}

fn resolve_local_declaration<'a>(
  name: &str,
  expected_kind: Option<LocalTypeDeclarationKind>,
  definition: &str,
  path: &str,
  context: &'a TypeConversionContext<'_>,
) -> Result<&'a LocalTypeDeclaration, Box<FfiInterfaceDiagnostic>> {
  let name = normalized_type_name(name);
  let name_is_qualified = name.contains('/');
  let mut candidates = Vec::new();
  if name_is_qualified {
    if let Some(found) = context.declarations.get(name) {
      candidates.extend(found);
    }
  } else {
    let local_id = format!("{}/{name}", context.current_namespace);
    if let Some(found) = context.declarations.get(&local_id) {
      candidates.extend(found);
    }
  }
  if let Some(kind) = expected_kind {
    candidates.retain(|candidate| candidate.kind() == kind);
  }
  candidates.sort_unstable_by_key(|candidate| candidate.source_id());

  match candidates.as_slice() {
    [declaration] => Ok(*declaration),
    [] if let Some(capability) = core_host_managed_type(name) => Err(Box::new(diagnostic(
      definition,
      path,
      "E_FFI_IR_HOST_MANAGED_TYPE",
      format!(
        "FFI Interface IR v{FFI_INTERFACE_IR_VERSION} deliberately does not represent host-managed core capability `{capability}` at `{path}`."
      ),
      "Keep this capability boundary handwritten: wrap opaque native task/response tokens with `ffi:task` or `ffi:response` inside the Calcit adapter, and do not expose their representation to generated bindings.",
    ))),
    [] => Err(Box::new(diagnostic(
      definition,
      path,
      "E_FFI_IR_DECLARATION_MISSING",
      format!("FFI Interface IR v{FFI_INTERFACE_IR_VERSION} cannot resolve local type declaration `{name}` at `{path}`."),
      "Use a namespace-qualified local defstruct/defenum reference, or keep dependency/resource/host types behind a handwritten adapter until declarations can be included explicitly.",
    ))),
    many => {
      let suggestion = if name_is_qualified {
        "Keep exactly one local defstruct/defenum source for this namespace-qualified nominal type."
      } else {
        "Use a namespace-qualified declaration ID; if it remains ambiguous, keep exactly one local defstruct/defenum source for that nominal type."
      };
      Err(Box::new(diagnostic(
        definition,
        path,
        "E_FFI_IR_DECLARATION_AMBIGUOUS",
        format!(
          "FFI Interface IR v{FFI_INTERFACE_IR_VERSION} found multiple local declarations for `{name}` at `{path}`: {}.",
          many.iter().map(|candidate| candidate.source_id()).collect::<Vec<_>>().join(", ")
        ),
        suggestion,
      )))
    }
  }
}

fn nominal_type(
  name: &str,
  expected_kind: Option<LocalTypeDeclarationKind>,
  arguments: &[std::sync::Arc<CalcitTypeAnnotation>],
  definition: &str,
  path: &str,
  context: &mut TypeConversionContext<'_>,
) -> Result<FfiTypeIr, Box<FfiInterfaceDiagnostic>> {
  if let Some(builtin) = explicit_builtin_type(name) {
    let converted = convert_type_arguments(arguments, definition, path, context)?;
    return match (builtin, converted.as_slice()) {
      ("Option", [item]) => Ok(FfiTypeIr::Option {
        item: Box::new(item.clone()),
      }),
      ("Result", [ok, error]) => Ok(FfiTypeIr::Result {
        ok: Box::new(ok.clone()),
        error: Box::new(error.clone()),
      }),
      ("Option", _) => Err(Box::new(diagnostic(
        definition,
        path,
        "E_FFI_IR_TYPE_ARGUMENT_ARITY",
        format!("Option expects 1 type argument at `{path}`, but received {}.", converted.len()),
        "Use Option<T> with exactly one generator-safe item type.",
      ))),
      ("Result", _) => Err(Box::new(diagnostic(
        definition,
        path,
        "E_FFI_IR_TYPE_ARGUMENT_ARITY",
        format!("Result expects 2 type arguments at `{path}`, but received {}.", converted.len()),
        "Use Result<T, E> with exactly one success type and one error type.",
      ))),
      _ => unreachable!("explicit_builtin_type returns a known built-in"),
    };
  }

  let declaration = resolve_local_declaration(name, expected_kind, definition, path, context)?;
  let expected_arity = declaration.type_parameters().len();
  if arguments.len() != expected_arity {
    return Err(Box::new(diagnostic(
      definition,
      path,
      "E_FFI_IR_TYPE_ARGUMENT_ARITY",
      format!(
        "Type declaration `{}` expects {expected_arity} argument(s) at `{path}`, but received {}.",
        declaration.id(),
        arguments.len()
      ),
      "Apply every declared type parameter exactly once; callable signatures remain monomorphic in Interface IR v2.",
    )));
  }
  let id = declaration.id().to_owned();
  let kind = declaration.kind();
  let converted = convert_type_arguments(arguments, definition, path, context)?;
  context.required.insert(id.clone());
  Ok(match kind {
    LocalTypeDeclarationKind::Struct => FfiTypeIr::Struct { id, arguments: converted },
    LocalTypeDeclarationKind::Enum => FfiTypeIr::Enum { id, arguments: converted },
  })
}

fn convert_type(
  annotation: &CalcitTypeAnnotation,
  definition: &str,
  path: &str,
  context: &mut TypeConversionContext<'_>,
) -> Result<FfiTypeIr, Box<FfiInterfaceDiagnostic>> {
  match annotation {
    CalcitTypeAnnotation::Unit => Ok(FfiTypeIr::Unit),
    CalcitTypeAnnotation::Bool => Ok(FfiTypeIr::Bool),
    CalcitTypeAnnotation::Number => Ok(FfiTypeIr::Number),
    CalcitTypeAnnotation::String => Ok(FfiTypeIr::String),
    CalcitTypeAnnotation::Buffer => Ok(FfiTypeIr::Buffer),
    CalcitTypeAnnotation::List(item) => Ok(FfiTypeIr::List {
      item: Box::new(convert_type(item, definition, &format!("{path}.item"), context)?),
    }),
    CalcitTypeAnnotation::TypeRef(name, arguments) => nominal_type(name, None, arguments, definition, path, context),
    CalcitTypeAnnotation::Struct(struct_def, arguments) => nominal_type(
      struct_def.name.ref_str(),
      Some(LocalTypeDeclarationKind::Struct),
      arguments,
      definition,
      path,
      context,
    ),
    CalcitTypeAnnotation::Enum(enum_def, arguments) => nominal_type(
      enum_def.name().ref_str(),
      Some(LocalTypeDeclarationKind::Enum),
      arguments,
      definition,
      path,
      context,
    ),
    CalcitTypeAnnotation::StructValue(struct_def) => nominal_type(
      struct_def.name.ref_str(),
      Some(LocalTypeDeclarationKind::Struct),
      &[],
      definition,
      path,
      context,
    ),
    CalcitTypeAnnotation::EnumValue(enum_def) => nominal_type(
      enum_def.name().ref_str(),
      Some(LocalTypeDeclarationKind::Enum),
      &[],
      definition,
      path,
      context,
    ),
    CalcitTypeAnnotation::TypeVar(name) if context.type_parameters.contains(name.as_ref()) => Ok(FfiTypeIr::TypeParameter {
      name: name.trim_start_matches('\'').to_owned(),
    }),
    CalcitTypeAnnotation::Dynamic => Err(Box::new(diagnostic(
      definition,
      path,
      "E_FFI_IR_DYNAMIC_TYPE",
      format!("FFI Interface IR v{FFI_INTERFACE_IR_VERSION} cannot generate an open Dynamic type at `{path}`."),
      "Replace Dynamic at the raw binding with a concrete generator-safe type or a declared local Struct/Enum. If the payload is intentionally open, keep it behind a handwritten adapter and validate or decode it before entering typed business code.",
    ))),
    CalcitTypeAnnotation::Fn(_) | CalcitTypeAnnotation::DynFn => Err(Box::new(diagnostic(
      definition,
      path,
      "E_FFI_IR_CALLBACK_TYPE",
      format!(
        "FFI Interface IR v{FFI_INTERFACE_IR_VERSION} does not model callback type `{}` at `{path}`.",
        annotation.to_brief_string()
      ),
      "Keep callback ownership, thread affinity, and lifetime orchestration in a handwritten adapter. Expose only generator-safe request and result values across generated raw bindings.",
    ))),
    unsupported => Err(Box::new(diagnostic(
      definition,
      path,
      "E_FFI_IR_UNSUPPORTED_TYPE",
      format!(
        "FFI Interface IR v{FFI_INTERFACE_IR_VERSION} cannot represent Calcit type `{}` at `{path}`.",
        unsupported.to_brief_string()
      ),
      "Use Unit, Bool, Number, String, Buffer, List, Option, Result, or an explicitly declared local Struct/Enum; keep Map/Set, Ref, resources, host objects, and other non-portable values behind a handwritten adapter.",
    ))),
  }
}

fn convert_signature(
  entry: &CodeEntry,
  definition: &str,
  namespace: &str,
  declarations: &LocalTypeDeclarations,
  required: &mut BTreeSet<String>,
) -> Result<FfiFunctionSignatureIr, Vec<FfiInterfaceDiagnostic>> {
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
      "Generic and trait-bounded FFI call signatures are not part of Interface IR v2.",
      "Expose a monomorphic raw binding and keep generic normalization in handwritten Calcit code.",
    ));
  }
  if signature.rest_type.is_some() {
    diagnostics.push(diagnostic(
      definition,
      "logical_schema.rest",
      "E_FFI_IR_UNSUPPORTED_REST",
      "Variadic FFI call signatures are not part of Interface IR v2.",
      "Expose a fixed-arity raw binding, using a typed List or Tuple when the host needs multiple values.",
    ));
  }

  let type_parameters = BTreeSet::new();
  let mut context = TypeConversionContext {
    declarations,
    required,
    current_namespace: namespace,
    type_parameters: &type_parameters,
  };
  let mut parameters = Vec::with_capacity(signature.arg_types.len());
  for (position, annotation) in signature.arg_types.iter().enumerate() {
    match convert_type(
      annotation,
      definition,
      &format!("signature.parameters.{position}.type"),
      &mut context,
    ) {
      Ok(type_ir) => parameters.push(FfiParameterIr { position, type_ir }),
      Err(error) => diagnostics.push(*error),
    }
  }
  let result = match convert_type(&signature.return_type, definition, "signature.result", &mut context) {
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

fn cirru_may_define_nominal_type(code: &Cirru) -> bool {
  let Cirru::List(items) = code else {
    return false;
  };
  let Some(Cirru::Leaf(head)) = items.first() else {
    return false;
  };
  match head.rsplit('/').next().unwrap_or(head) {
    "defstruct" | "defenum" | "&struct-def:new" | "&enum-def:new" => true,
    "def" => items.get(2).is_some_and(cirru_may_define_nominal_type),
    "impl-traits" => items.get(1).is_some_and(cirru_may_define_nominal_type),
    "quote" => items.get(1).is_some_and(cirru_may_define_nominal_type),
    _ => false,
  }
}

fn local_nominal_id(namespace: &str, name: &str) -> String {
  let name = normalized_type_name(name);
  if name.contains('/') {
    name.to_owned()
  } else {
    format!("{namespace}/{name}")
  }
}

fn collect_local_type_declarations(snapshot: &Snapshot) -> LocalTypeDeclarations {
  let mut declarations = BTreeMap::new();
  for (namespace, file) in &snapshot.files {
    for (definition, entry) in &file.defs {
      if !cirru_may_define_nominal_type(&entry.code) {
        continue;
      }
      let Ok(code) = code_to_calcit(&entry.code, namespace, definition, vec![]) else {
        continue;
      };
      let Some(declaration) = crate::calcit::type_annotation::resolve_type_def_from_code(&code) else {
        continue;
      };
      let source_id = format!("{namespace}/{definition}");
      let source = match declaration {
        Calcit::StructDef(nominal) => {
          let id = local_nominal_id(namespace, nominal.name.ref_str());
          LocalTypeDeclaration::Struct {
            id,
            source_id,
            namespace: namespace.clone(),
            nominal,
          }
        }
        Calcit::EnumDef(nominal) => {
          let id = local_nominal_id(namespace, nominal.name().ref_str());
          LocalTypeDeclaration::Enum {
            id,
            source_id,
            namespace: namespace.clone(),
            nominal,
          }
        }
        _ => continue,
      };
      declarations.entry(source.id().to_owned()).or_insert_with(Vec::new).push(source);
    }
  }
  for candidates in declarations.values_mut() {
    candidates.sort_unstable_by(|left, right| left.source_id().cmp(right.source_id()));
  }
  declarations
}

fn declaration_type_parameters(source: &LocalTypeDeclaration) -> Vec<String> {
  source
    .type_parameters()
    .iter()
    .map(|parameter| parameter.trim_start_matches('\'').to_owned())
    .collect()
}

fn convert_declaration(
  source: &LocalTypeDeclaration,
  owner_definition: &str,
  declarations: &LocalTypeDeclarations,
  required: &mut BTreeSet<String>,
) -> Result<FfiTypeDeclarationIr, Vec<FfiInterfaceDiagnostic>> {
  let mut diagnostics = Vec::new();
  let parameters = declaration_type_parameters(source);
  let type_parameters = parameters.iter().cloned().collect::<BTreeSet<_>>();
  let mut context = TypeConversionContext {
    declarations,
    required,
    current_namespace: source.namespace(),
    type_parameters: &type_parameters,
  };

  match source {
    LocalTypeDeclaration::Struct {
      id, namespace, nominal, ..
    } => {
      if !nominal.where_bounds.is_empty() {
        diagnostics.push(diagnostic(
          owner_definition,
          format!("declarations.{id}.type_parameters"),
          "E_FFI_IR_DECLARATION_BOUNDS",
          format!("Struct declaration `{id}` has trait-bounded type parameters, which Interface IR v2 cannot lower portably."),
          "Expose an unbounded transport struct or keep trait-constrained normalization in handwritten Calcit code.",
        ));
      }
      let mut fields = Vec::with_capacity(nominal.fields.len());
      for (index, (name, annotation)) in nominal.fields.iter().zip(nominal.field_types.iter()).enumerate() {
        match convert_type(
          annotation,
          owner_definition,
          &format!("declarations.{id}.fields.{index}.type"),
          &mut context,
        ) {
          Ok(type_ir) => fields.push(FfiStructFieldIr {
            name: name.ref_str().to_owned(),
            type_ir,
          }),
          Err(error) => diagnostics.push(*error),
        }
      }
      if diagnostics.is_empty() {
        Ok(FfiTypeDeclarationIr::Struct {
          id: id.clone(),
          namespace: namespace.clone(),
          name: nominal.name.ref_str().to_owned(),
          type_parameters: parameters,
          fields,
        })
      } else {
        Err(diagnostics)
      }
    }
    LocalTypeDeclaration::Enum {
      id, namespace, nominal, ..
    } => {
      if !nominal.where_bounds().is_empty() {
        diagnostics.push(diagnostic(
          owner_definition,
          format!("declarations.{id}.type_parameters"),
          "E_FFI_IR_DECLARATION_BOUNDS",
          format!("Enum declaration `{id}` has trait-bounded type parameters, which Interface IR v2 cannot lower portably."),
          "Expose an unbounded transport enum or keep trait-constrained normalization in handwritten Calcit code.",
        ));
      }
      let mut variants = Vec::with_capacity(nominal.variants().len());
      for (variant_index, variant) in nominal.variants().iter().enumerate() {
        let mut payload = Vec::with_capacity(variant.payload_types().len());
        for (payload_index, annotation) in variant.payload_types().iter().enumerate() {
          match convert_type(
            annotation,
            owner_definition,
            &format!("declarations.{id}.variants.{variant_index}.payload.{payload_index}"),
            &mut context,
          ) {
            Ok(type_ir) => payload.push(type_ir),
            Err(error) => diagnostics.push(*error),
          }
        }
        variants.push(FfiEnumVariantIr {
          name: variant.tag.ref_str().to_owned(),
          payload,
        });
      }
      if diagnostics.is_empty() {
        Ok(FfiTypeDeclarationIr::Enum {
          id: id.clone(),
          namespace: namespace.clone(),
          name: nominal.name().ref_str().to_owned(),
          type_parameters: parameters,
          variants,
        })
      } else {
        Err(diagnostics)
      }
    }
  }
}

fn convert_reachable_declarations(
  owner_definition: &str,
  declarations: &LocalTypeDeclarations,
  required: &mut BTreeSet<String>,
) -> (Vec<FfiTypeDeclarationIr>, Vec<FfiInterfaceDiagnostic>) {
  let mut pending = required.iter().cloned().collect::<VecDeque<_>>();
  let mut visited = BTreeSet::new();
  let mut converted = BTreeMap::new();
  let mut diagnostics = Vec::new();

  while let Some(id) = pending.pop_front() {
    if !visited.insert(id.clone()) {
      continue;
    }
    let Some(candidates) = declarations.get(&id) else {
      diagnostics.push(diagnostic(
        owner_definition,
        format!("declarations.{id}"),
        "E_FFI_IR_DECLARATION_MISSING",
        format!("Required declaration `{id}` disappeared while building Interface IR v{FFI_INTERFACE_IR_VERSION}."),
        "Keep the namespace-qualified local declaration in the same snapshot as its FFI signature.",
      ));
      continue;
    };
    let [source] = candidates.as_slice() else {
      diagnostics.push(diagnostic(
        owner_definition,
        format!("declarations.{id}"),
        "E_FFI_IR_DECLARATION_AMBIGUOUS",
        format!(
          "Required declaration `{id}` resolves to multiple snapshot definitions: {}.",
          candidates
            .iter()
            .map(LocalTypeDeclaration::source_id)
            .collect::<Vec<_>>()
            .join(", ")
        ),
        "Keep one local defstruct/defenum source for each namespace-qualified nominal type.",
      ));
      continue;
    };
    match convert_declaration(source, owner_definition, declarations, required) {
      Ok(declaration) => {
        converted.insert(declaration.id().to_owned(), declaration);
      }
      Err(errors) => diagnostics.extend(errors),
    }
    for nested in required.iter() {
      if !visited.contains(nested) && !pending.contains(nested) {
        pending.push_back(nested.clone());
      }
    }
  }

  (converted.into_values().collect(), diagnostics)
}

fn metadata_value<'a>(metadata: &'a Edn, key: &str) -> Option<&'a Edn> {
  match metadata {
    Edn::Struct(value) => value.pairs.iter().find(|(field, _)| field.ref_str() == key).map(|(_, value)| value),
    Edn::Map(value) => value.get(&Edn::tag(key)),
    _ => None,
  }
}

fn canonical_edn_display(value: &Edn) -> String {
  match value {
    Edn::List(items) => {
      let values = items.iter().map(canonical_edn_display).collect::<Vec<_>>().join(" ");
      if values.is_empty() {
        "([])".to_owned()
      } else {
        format!("([] {values})")
      }
    }
    Edn::Set(items) => {
      let mut values = items.0.iter().map(canonical_edn_display).collect::<Vec<_>>();
      values.sort_unstable();
      if values.is_empty() {
        "(#{})".to_owned()
      } else {
        format!("(#{{}} {})", values.join(" "))
      }
    }
    Edn::Map(items) => {
      let mut pairs = items
        .0
        .iter()
        .map(|(key, value)| (canonical_edn_display(key), canonical_edn_display(value)))
        .collect::<Vec<_>>();
      pairs.sort_unstable();
      let entries = pairs
        .into_iter()
        .map(|(key, value)| format!("({key} {value})"))
        .collect::<Vec<_>>()
        .join(" ");
      if entries.is_empty() {
        "({})".to_owned()
      } else {
        format!("({{}} {entries})")
      }
    }
    Edn::Struct(value) => {
      let mut pairs = value
        .pairs
        .iter()
        .map(|(field, value)| (field.ref_str(), canonical_edn_display(value)))
        .collect::<Vec<_>>();
      pairs.sort_unstable();
      let entries = pairs
        .into_iter()
        .map(|(field, value)| format!("(:{field} {value})"))
        .collect::<Vec<_>>()
        .join(" ");
      if entries.is_empty() {
        format!("(%{{}} '{})", value.name)
      } else {
        format!("(%{{}} '{} {entries})", value.name)
      }
    }
    Edn::Enum(value) => {
      let extra = value.extra.iter().map(canonical_edn_display).collect::<Vec<_>>().join(" ");
      let prefix = match &value.type_name {
        Some(type_name) => format!("(%:: '{type_name} '{}", value.variant),
        None => format!("(:: '{}", value.variant),
      };
      if extra.is_empty() {
        format!("{prefix})")
      } else {
        format!("{prefix} {extra})")
      }
    }
    Edn::Atom(value) => format!("(atom {})", canonical_edn_display(value)),
    _ => value.to_string(),
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

fn is_portable_c_identifier(value: &str) -> bool {
  let mut chars = value.chars();
  let Some(first) = chars.next() else {
    return false;
  };
  (first == '_' || first.is_ascii_alphabetic())
    && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
    && !["_calcit_ffi_v1", "_calcit_ffi_async_v1", "_calcit_ffi_blocking_v1"]
      .iter()
      .any(|suffix| value.ends_with(suffix))
}

fn validate_lowering_contract(definition: &str, lowering: &FfiLoweringIr, diagnostics: &mut Vec<FfiInterfaceDiagnostic>) {
  match lowering.backend.as_deref() {
    None => {}
    Some("native") => {
      if let Some(target) = lowering.target.as_deref()
        && target != "native"
      {
        diagnostics.push(diagnostic(
          definition,
          "lowering.target",
          "E_FFI_IR_TARGET_INVALID",
          format!("Native FFI lowering cannot target `{target}`."),
          "Omit `:target` for the current native host or use `:target :native`; browser/node targets belong to the JS backend.",
        ));
      }

      match lowering.symbol.as_deref() {
        None => diagnostics.push(diagnostic(
          definition,
          "lowering.symbol",
          "E_FFI_IR_SYMBOL_REQUIRED",
          "Native FFI lowering does not declare a base symbol.",
          "Declare a portable C identifier such as `:symbol |read_file`; Calcit derives the versioned protocol suffix.",
        )),
        Some(symbol) if !is_portable_c_identifier(symbol) => diagnostics.push(diagnostic(
          definition,
          "lowering.symbol",
          "E_FFI_IR_SYMBOL_INVALID",
          format!("Native FFI base symbol `{symbol}` is not a portable C identifier."),
          "Use ASCII letters, digits, and underscores, beginning with a letter or underscore; do not include a protocol suffix.",
        )),
        Some(_) => {}
      }

      let invoke_known = match lowering.invoke.as_deref() {
        None => {
          diagnostics.push(diagnostic(
            definition,
            "lowering.invoke",
            "E_FFI_IR_INVOKE_REQUIRED",
            "Native FFI lowering does not declare an invocation mode.",
            "Declare `:invoke :sync`, `:invoke :async`, or `:invoke :blocking-callback`.",
          ));
          false
        }
        Some("sync" | "async" | "blocking-callback") => true,
        Some(invoke) => {
          diagnostics.push(diagnostic(
            definition,
            "lowering.invoke",
            "E_FFI_IR_INVOKE_UNKNOWN",
            format!("Native FFI invocation mode `{invoke}` is not supported."),
            "Use one of the published native invocation modes: sync, async, or blocking-callback.",
          ));
          false
        }
      };

      let transport_known = match lowering.transport.as_deref() {
        None => {
          diagnostics.push(diagnostic(
            definition,
            "lowering.transport",
            "E_FFI_IR_TRANSPORT_REQUIRED",
            "Native FFI lowering does not declare a transport.",
            "Declare `:transport :edn-buffer-v1`, `:transport :async-task-v1`, or `:transport :blocking-host-v1`.",
          ));
          false
        }
        Some("edn-buffer-v1" | "async-task-v1" | "blocking-host-v1") => true,
        Some(transport) => {
          diagnostics.push(diagnostic(
            definition,
            "lowering.transport",
            "E_FFI_IR_TRANSPORT_UNKNOWN",
            format!("Native FFI transport `{transport}` is not supported."),
            "Use one of the published versioned native transports instead of an unversioned or Rust-layout ABI.",
          ));
          false
        }
      };

      if invoke_known && transport_known {
        let pair = (lowering.invoke.as_deref(), lowering.transport.as_deref());
        if !matches!(
          pair,
          (Some("sync"), Some("edn-buffer-v1"))
            | (Some("async"), Some("async-task-v1"))
            | (Some("blocking-callback"), Some("blocking-host-v1"))
        ) {
          diagnostics.push(diagnostic(
            definition,
            "lowering.transport",
            "E_FFI_IR_TRANSPORT_MISMATCH",
            format!(
              "Native FFI invocation mode `{}` is incompatible with transport `{}`.",
              lowering.invoke.as_deref().expect("known invocation mode"),
              lowering.transport.as_deref().expect("known transport")
            ),
            "Use sync + edn-buffer-v1, async + async-task-v1, or blocking-callback + blocking-host-v1.",
          ));
        }
      }
    }
    Some("js") => {
      if let Some(target) = lowering.target.as_deref()
        && !matches!(target, "browser" | "node")
      {
        diagnostics.push(diagnostic(
          definition,
          "lowering.target",
          "E_FFI_IR_TARGET_INVALID",
          format!("JS FFI target `{target}` is not supported."),
          "Use `:target :browser`, `:target :node`, or omit the target for a shared JS boundary.",
        ));
      }
    }
    Some(backend) => diagnostics.push(diagnostic(
      definition,
      "lowering.backend",
      "E_FFI_IR_BACKEND_UNKNOWN",
      format!("FFI backend `{backend}` is not part of Interface IR v{FFI_INTERFACE_IR_VERSION}."),
      "Use the native or JS backend, or keep the boundary behind a handwritten adapter until that backend has a versioned contract.",
    )),
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
  let lowering = FfiLoweringIr {
    backend,
    target: scalar_metadata(metadata, "target", definition, &mut diagnostics),
    kind: scalar_metadata(metadata, "kind", definition, &mut diagnostics),
    symbol: scalar_metadata(metadata, "symbol", definition, &mut diagnostics),
    invoke: scalar_metadata(metadata, "invoke", definition, &mut diagnostics),
    transport: scalar_metadata(metadata, "transport", definition, &mut diagnostics),
    raw: canonical_edn_display(metadata),
  };
  validate_lowering_contract(definition, &lowering, &mut diagnostics);
  Ok((lowering, diagnostics))
}

pub fn export_snapshot(snapshot: &Snapshot, namespace: Option<&str>) -> Result<FfiExportReport, String> {
  let local_declarations = collect_local_type_declarations(snapshot);
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
  let mut interface_declarations = BTreeMap::new();
  let mut diagnostics = Vec::new();
  for (namespace, name, entry, metadata) in candidates {
    let id = format!("{namespace}/{name}");
    let definition_diagnostic_start = diagnostics.len();
    let mut required = BTreeSet::new();
    let mut signature = match convert_signature(entry, &id, namespace, &local_declarations, &mut required) {
      Ok(signature) => Some(signature),
      Err(errors) => {
        diagnostics.extend(errors);
        None
      }
    };
    if signature.is_some() {
      let (declarations, declaration_diagnostics) = convert_reachable_declarations(&id, &local_declarations, &mut required);
      if declaration_diagnostics.is_empty() {
        for declaration in declarations {
          interface_declarations.insert(declaration.id().to_owned(), declaration);
        }
      } else {
        diagnostics.extend(declaration_diagnostics);
        signature = None;
      }
    }
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
      logical_schema: canonical_edn_display(&entry.schema.to_type_edn()),
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
    declarations: interface_declarations.into_values().collect(),
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
    "Calcit FFI Interface IR v{}\n- package: {} {}\n- revision: {}\n- declarations: {}\n- definitions: {} ({} supported, {} unsupported)\n",
    report.interface.version,
    report.interface.package,
    report.interface.package_version,
    report.revision,
    report.interface.declarations.len(),
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

  fn data_entry(code: &str) -> CodeEntry {
    let parsed = cirru_parser::parse(code).expect("parse data declaration fixture");
    CodeEntry {
      doc: "test declaration".to_owned(),
      examples: vec![],
      tests: vec![],
      tags: HashSet::new(),
      code: parsed.into_iter().next().expect("one data declaration fixture"),
      schema: DYNAMIC_TYPE.clone(),
      ffi: None,
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
    Edn::map_from_iter([
      (Edn::tag("backend"), Edn::tag("native")),
      (Edn::tag("invoke"), Edn::tag("sync")),
      (Edn::tag("kind"), Edn::tag("dylib-method")),
      (Edn::tag("symbol"), Edn::str(symbol)),
      (Edn::tag("transport"), Edn::tag("edn-buffer-v1")),
    ])
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
  fn exports_reachable_struct_enum_option_and_result_declarations() {
    let person = Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from("test.ffi/Person"), Arc::new(vec![])));
    let outcome = Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from("test.ffi/Outcome"), Arc::new(vec![])));
    let result = Arc::new(CalcitTypeAnnotation::TypeRef(
      Arc::from("Result"),
      Arc::new(vec![outcome, Arc::new(CalcitTypeAnnotation::String)]),
    ));
    let report = export_snapshot(
      &snapshot(vec![
        (
          "Person",
          data_entry("defstruct Person (:name 'String) (:nickname (:: 'Option 'String))"),
        ),
        ("Outcome", data_entry("defenum Outcome (:ok Person) (:err 'String)")),
        ("roundtrip", function_entry(vec![person], result, native_metadata("roundtrip"))),
      ]),
      None,
    )
    .expect("export reachable composite declarations");

    assert_eq!(report.summary.supported, 1);
    assert!(report.diagnostics.is_empty());
    assert_eq!(
      report
        .interface
        .declarations
        .iter()
        .map(FfiTypeDeclarationIr::id)
        .collect::<Vec<_>>(),
      ["test.ffi/Outcome", "test.ffi/Person"]
    );
    assert!(matches!(
      report.interface.definitions[0].signature.as_ref().expect("supported signature").parameters[0].type_ir,
      FfiTypeIr::Struct { ref id, ref arguments } if id == "test.ffi/Person" && arguments.is_empty()
    ));
    assert!(matches!(
      report.interface.definitions[0].signature.as_ref().expect("supported signature").result,
      FfiTypeIr::Result { ref ok, ref error }
        if matches!(ok.as_ref(), FfiTypeIr::Enum { id, arguments } if id == "test.ffi/Outcome" && arguments.is_empty())
          && matches!(error.as_ref(), FfiTypeIr::String)
    ));
    let person_declaration = report
      .interface
      .declarations
      .iter()
      .find(|declaration| declaration.id() == "test.ffi/Person")
      .expect("person declaration");
    assert!(matches!(
      person_declaration,
      FfiTypeDeclarationIr::Struct { fields, .. }
        if matches!(fields[1].type_ir, FfiTypeIr::Option { ref item } if matches!(item.as_ref(), FfiTypeIr::String))
    ));
  }

  #[test]
  fn indexes_local_declarations_by_nominal_name_instead_of_binding_name() {
    let person = Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from("test.ffi/Person"), Arc::new(vec![])));
    let report = export_snapshot(
      &snapshot(vec![
        ("PersonShape", data_entry("defstruct Person (:name 'String)")),
        (
          "read-person",
          function_entry(vec![person], Arc::new(CalcitTypeAnnotation::String), native_metadata("read_person")),
        ),
      ]),
      None,
    )
    .expect("export nominal declaration stored behind a differently named binding");

    assert_eq!(report.summary.supported, 1);
    assert!(report.diagnostics.is_empty());
    assert!(matches!(
      report.interface.declarations.as_slice(),
      [FfiTypeDeclarationIr::Struct { id, name, .. }] if id == "test.ffi/Person" && name == "Person"
    ));
    assert!(matches!(
      report.interface.definitions[0].signature.as_ref().expect("supported signature").parameters[0].type_ir,
      FfiTypeIr::Struct { ref id, ref arguments } if id == "test.ffi/Person" && arguments.is_empty()
    ));
  }

  #[test]
  fn duplicate_nominal_declaration_bindings_are_ambiguous_and_deterministic() {
    let export = |type_name: &str| {
      export_snapshot(
        &snapshot(vec![
          ("PersonText", data_entry("defstruct Person (:value 'String)")),
          ("PersonNumber", data_entry("defstruct Person (:value 'Number)")),
          (
            "read-person",
            function_entry(
              vec![Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from(type_name), Arc::new(vec![])))],
              Arc::new(CalcitTypeAnnotation::Unit),
              native_metadata("read_person"),
            ),
          ),
        ]),
        None,
      )
      .expect("inventory duplicate nominal declarations")
    };
    let report = export("test.ffi/Person");

    assert_eq!(report.summary.unsupported, 1);
    assert!(report.interface.definitions[0].signature.is_none());
    let ambiguity = report
      .diagnostics
      .iter()
      .find(|diagnostic| diagnostic.code == "E_FFI_IR_DECLARATION_AMBIGUOUS")
      .expect("duplicate nominal declarations must be rejected");
    assert!(ambiguity.message.contains("test.ffi/PersonNumber, test.ffi/PersonText"));
    assert_eq!(
      ambiguity.suggestion,
      "Keep exactly one local defstruct/defenum source for this namespace-qualified nominal type."
    );
    assert_eq!(
      report,
      export("test.ffi/Person"),
      "duplicate declaration diagnostics must be deterministic"
    );

    let unqualified = export("Person");
    let unqualified_ambiguity = unqualified
      .diagnostics
      .iter()
      .find(|diagnostic| diagnostic.code == "E_FFI_IR_DECLARATION_AMBIGUOUS")
      .expect("unqualified duplicate nominal declarations must be rejected");
    assert_eq!(
      unqualified_ambiguity.suggestion,
      "Use a namespace-qualified declaration ID; if it remains ambiguous, keep exactly one local defstruct/defenum source for that nominal type."
    );
  }

  #[test]
  fn unresolved_named_type_fails_before_generation() {
    let export = || {
      export_snapshot(
        &snapshot(vec![(
          "read",
          function_entry(
            vec![Arc::new(CalcitTypeAnnotation::TypeRef(
              Arc::from("missing/Thing"),
              Arc::new(vec![]),
            ))],
            Arc::new(CalcitTypeAnnotation::Unit),
            native_metadata("read"),
          ),
        )]),
        None,
      )
      .expect("inventory unresolved named type")
    };
    let report = export();

    assert_eq!(report.summary.unsupported, 1);
    assert!(report.interface.definitions[0].signature.is_none());
    assert!(diagnostic_codes(&report).contains("E_FFI_IR_DECLARATION_MISSING"));
    assert_eq!(report, export(), "unresolved declaration diagnostics must be deterministic");
  }

  #[test]
  fn classifies_core_host_managed_capabilities_separately_from_missing_declarations() {
    let export = || {
      export_snapshot(
        &snapshot(vec![(
          "serve",
          function_entry(
            vec![Arc::new(CalcitTypeAnnotation::TypeRef(
              Arc::from("calcit.core/FfiResponse"),
              Arc::new(vec![]),
            ))],
            Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from("FfiTask"), Arc::new(vec![]))),
            native_metadata("serve"),
          ),
        )]),
        None,
      )
      .expect("inventory host-managed core capabilities")
    };
    let report = export();

    assert_eq!(report.summary.unsupported, 1);
    assert_eq!(report.summary.diagnostics, 2);
    assert!(report.interface.definitions[0].signature.is_none());
    assert!(
      report
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == "E_FFI_IR_HOST_MANAGED_TYPE")
    );
    assert_eq!(
      report
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.path.as_str())
        .collect::<Vec<_>>(),
      ["signature.parameters.0.type", "signature.result"]
    );
    assert_eq!(report, export(), "host-managed capability diagnostics must be deterministic");
  }

  #[test]
  fn prefers_a_local_declaration_over_the_unqualified_core_capability_name() {
    let report = export_snapshot(
      &snapshot(vec![
        ("FfiTask", data_entry("defstruct FfiTask (:id 'String)")),
        (
          "read-task",
          function_entry(
            vec![],
            Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from("FfiTask"), Arc::new(vec![]))),
            native_metadata("read_task"),
          ),
        ),
      ]),
      None,
    )
    .expect("export local declaration shadowing a core capability name");

    assert_eq!(report.summary.supported, 1);
    assert!(report.diagnostics.is_empty());
    assert!(matches!(
      report.interface.definitions[0].signature.as_ref().expect("supported signature").result,
      FfiTypeIr::Struct { ref id, ref arguments } if id == "test.ffi/FfiTask" && arguments.is_empty()
    ));
  }

  #[test]
  fn rejects_declaration_type_argument_arity_mismatch() {
    let report = export_snapshot(
      &snapshot(vec![
        ("Box", data_entry("defstruct Box ('T) (:value 'T)")),
        (
          "read-box",
          function_entry(
            vec![Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from("test.ffi/Box"), Arc::new(vec![])))],
            Arc::new(CalcitTypeAnnotation::Unit),
            native_metadata("read_box"),
          ),
        ),
      ]),
      None,
    )
    .expect("inventory declaration with wrong type argument arity");

    assert_eq!(report.summary.unsupported, 1);
    assert!(report.interface.definitions[0].signature.is_none());
    assert!(diagnostic_codes(&report).contains("E_FFI_IR_TYPE_ARGUMENT_ARITY"));
  }

  #[test]
  fn preserves_generic_declaration_parameters_with_monomorphic_application() {
    let report = export_snapshot(
      &snapshot(vec![
        ("Box", data_entry("defstruct Box ('T) (:value 'T)")),
        (
          "read-box",
          function_entry(
            vec![Arc::new(CalcitTypeAnnotation::TypeRef(
              Arc::from("test.ffi/Box"),
              Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]),
            ))],
            Arc::new(CalcitTypeAnnotation::Unit),
            native_metadata("read_box"),
          ),
        ),
      ]),
      None,
    )
    .expect("export generic declaration with monomorphic call site");

    assert_eq!(report.summary.supported, 1);
    assert!(matches!(
      &report.interface.declarations[0],
      FfiTypeDeclarationIr::Struct { type_parameters, fields, .. }
        if type_parameters == &["T"]
          && matches!(fields[0].type_ir, FfiTypeIr::TypeParameter { ref name } if name == "T")
    ));
    assert!(matches!(
      report.interface.definitions[0].signature.as_ref().expect("supported signature").parameters[0].type_ir,
      FfiTypeIr::Struct { ref arguments, .. } if matches!(arguments.as_slice(), [FfiTypeIr::Number])
    ));
  }

  #[test]
  fn namespace_qualified_declaration_ids_do_not_collide() {
    let file = |definitions: Vec<(&str, CodeEntry)>| FileInSnapShot {
      ns: NsEntry {
        doc: String::new(),
        code: Cirru::List(vec![]),
      },
      defs: definitions.into_iter().map(|(name, entry)| (name.to_owned(), entry)).collect(),
    };
    let source = Snapshot {
      package: "test-package".to_owned(),
      about: None,
      version: "0.1.0".to_owned(),
      entries: HashMap::new(),
      files: HashMap::from([
        (
          "alpha.ffi".to_owned(),
          file(vec![
            ("Person", data_entry("defstruct Person (:name 'String)")),
            (
              "read",
              function_entry(
                vec![Arc::new(CalcitTypeAnnotation::TypeRef(
                  Arc::from("alpha.ffi/Person"),
                  Arc::new(vec![]),
                ))],
                Arc::new(CalcitTypeAnnotation::Unit),
                native_metadata("alpha_read"),
              ),
            ),
          ]),
        ),
        (
          "beta.ffi".to_owned(),
          file(vec![
            ("Person", data_entry("defstruct Person (:name 'String)")),
            (
              "read",
              function_entry(
                vec![Arc::new(CalcitTypeAnnotation::TypeRef(
                  Arc::from("beta.ffi/Person"),
                  Arc::new(vec![]),
                ))],
                Arc::new(CalcitTypeAnnotation::Unit),
                native_metadata("beta_read"),
              ),
            ),
          ]),
        ),
      ]),
      active_entry: "default".to_owned(),
    };
    let report = export_snapshot(&source, None).expect("export same-name declarations from two namespaces");

    assert_eq!(report.summary.supported, 2);
    assert_eq!(
      report
        .interface
        .declarations
        .iter()
        .map(FfiTypeDeclarationIr::id)
        .collect::<Vec<_>>(),
      ["alpha.ffi/Person", "beta.ffi/Person"]
    );
  }

  #[test]
  fn unrelated_declarations_do_not_change_interface_revision() {
    let binding = || {
      function_entry(
        vec![Arc::new(CalcitTypeAnnotation::String)],
        Arc::new(CalcitTypeAnnotation::String),
        native_metadata("read"),
      )
    };
    let without_declaration = export_snapshot(&snapshot(vec![("read", binding())]), None).expect("baseline export");
    let with_declaration = export_snapshot(
      &snapshot(vec![
        ("Unused", data_entry("defstruct Unused (:value 'Dynamic)")),
        ("read", binding()),
      ]),
      None,
    )
    .expect("export with unrelated declaration");

    assert!(with_declaration.interface.declarations.is_empty());
    assert_eq!(without_declaration.revision, with_declaration.revision);
  }

  #[test]
  fn emits_deterministic_diagnostics_without_dynamic_fallback() {
    let report = export_snapshot(
      &snapshot(vec![(
        "dynamic-call",
        function_entry(
          vec![DYNAMIC_TYPE.clone(), Arc::new(CalcitTypeAnnotation::List(DYNAMIC_TYPE.clone()))],
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
    assert!(codes.contains("E_FFI_IR_DYNAMIC_TYPE"));
    assert!(codes.contains("E_FFI_IR_BACKEND_REQUIRED"));
    assert_eq!(
      report
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "E_FFI_IR_DYNAMIC_TYPE")
        .map(|diagnostic| diagnostic.path.as_str())
        .collect::<Vec<_>>(),
      [
        "signature.parameters.0.type",
        "signature.parameters.1.type.item",
        "signature.result"
      ]
    );
  }

  #[test]
  fn classifies_typed_and_untyped_callbacks_with_stable_paths() {
    let callback = Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![Arc::new(CalcitTypeAnnotation::String)],
      return_type: Arc::new(CalcitTypeAnnotation::Unit),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(HashSet::new()),
    })));
    let export = || {
      export_snapshot(
        &snapshot(vec![(
          "watch",
          function_entry(
            vec![callback.clone(), Arc::new(CalcitTypeAnnotation::DynFn)],
            Arc::new(CalcitTypeAnnotation::Unit),
            native_metadata("watch"),
          ),
        )]),
        None,
      )
      .expect("inventory callback boundaries")
    };
    let report = export();

    assert_eq!(report.summary.supported, 0);
    assert_eq!(report.summary.unsupported, 1);
    assert_eq!(report.summary.diagnostics, 2);
    assert!(
      report
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == "E_FFI_IR_CALLBACK_TYPE")
    );
    assert_eq!(
      report
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.path.as_str())
        .collect::<Vec<_>>(),
      ["signature.parameters.0.type", "signature.parameters.1.type"]
    );
    assert_eq!(report, export(), "callback diagnostics must be deterministic");
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
  fn rejects_incomplete_native_lowering_before_generation() {
    let report = export_snapshot(
      &snapshot(vec![(
        "incomplete",
        function_entry(
          vec![Arc::new(CalcitTypeAnnotation::String)],
          Arc::new(CalcitTypeAnnotation::String),
          Edn::map_from_iter([(Edn::tag("backend"), Edn::tag("native"))]),
        ),
      )]),
      None,
    )
    .expect("inventory incomplete native metadata");

    let codes = diagnostic_codes(&report);
    assert_eq!(report.summary.unsupported, 1);
    assert!(codes.contains("E_FFI_IR_SYMBOL_REQUIRED"));
    assert!(codes.contains("E_FFI_IR_INVOKE_REQUIRED"));
    assert!(codes.contains("E_FFI_IR_TRANSPORT_REQUIRED"));
    assert_eq!(
      report.diagnostics.iter().map(|item| item.path.as_str()).collect::<Vec<_>>(),
      ["lowering.symbol", "lowering.invoke", "lowering.transport"]
    );
  }

  #[test]
  fn rejects_invalid_native_symbol_and_protocol_pair() {
    let metadata = Edn::map_from_iter([
      (Edn::tag("backend"), Edn::tag("native")),
      (Edn::tag("invoke"), Edn::tag("async")),
      (Edn::tag("symbol"), Edn::str("not-portable!")),
      (Edn::tag("transport"), Edn::tag("edn-buffer-v1")),
    ]);
    let report = export_snapshot(
      &snapshot(vec![(
        "invalid",
        function_entry(
          vec![Arc::new(CalcitTypeAnnotation::String)],
          Arc::new(CalcitTypeAnnotation::String),
          metadata,
        ),
      )]),
      None,
    )
    .expect("inventory invalid native metadata");

    let codes = diagnostic_codes(&report);
    assert!(codes.contains("E_FFI_IR_SYMBOL_INVALID"));
    assert!(codes.contains("E_FFI_IR_TRANSPORT_MISMATCH"));
  }

  #[test]
  fn rejects_protocol_suffixed_native_base_symbols() {
    let definitions = [
      ("sync", "edn-buffer-v1", "read_calcit_ffi_v1"),
      ("async", "async-task-v1", "watch_calcit_ffi_async_v1"),
      ("blocking-callback", "blocking-host-v1", "read_lines_calcit_ffi_blocking_v1"),
    ]
    .into_iter()
    .map(|(invoke, transport, symbol)| {
      (
        invoke,
        function_entry(
          vec![],
          Arc::new(CalcitTypeAnnotation::Unit),
          Edn::map_from_iter([
            (Edn::tag("backend"), Edn::tag("native")),
            (Edn::tag("invoke"), Edn::tag(invoke)),
            (Edn::tag("symbol"), Edn::str(symbol)),
            (Edn::tag("transport"), Edn::tag(transport)),
          ]),
        ),
      )
    })
    .collect();
    let report = export_snapshot(&snapshot(definitions), None).expect("inventory protocol-suffixed symbols");

    assert_eq!(report.summary.supported, 0);
    assert_eq!(report.summary.unsupported, 3);
    assert!(diagnostic_codes(&report).contains("E_FFI_IR_SYMBOL_INVALID"));
  }

  #[test]
  fn accepts_each_published_native_protocol_pair() {
    let definitions = [
      ("sync", "edn-buffer-v1"),
      ("async", "async-task-v1"),
      ("blocking-callback", "blocking-host-v1"),
    ]
    .into_iter()
    .map(|(invoke, transport)| {
      (
        invoke,
        function_entry(
          vec![],
          Arc::new(CalcitTypeAnnotation::Unit),
          Edn::map_from_iter([
            (Edn::tag("backend"), Edn::tag("native")),
            (Edn::tag("invoke"), Edn::tag(invoke)),
            (Edn::tag("symbol"), Edn::str(invoke.replace('-', "_"))),
            (Edn::tag("transport"), Edn::tag(transport)),
          ]),
        ),
      )
    })
    .collect();
    let report = export_snapshot(&snapshot(definitions), None).expect("export published native protocol pairs");

    assert_eq!(report.summary.supported, 3);
    assert_eq!(report.summary.unsupported, 0);
    assert!(report.diagnostics.is_empty());
  }

  #[test]
  fn rejects_unknown_lowering_values_and_backend_targets() {
    let definitions = vec![
      (
        "native-target",
        function_entry(
          vec![],
          Arc::new(CalcitTypeAnnotation::Unit),
          Edn::map_from_iter([
            (Edn::tag("backend"), Edn::tag("native")),
            (Edn::tag("invoke"), Edn::tag("future")),
            (Edn::tag("symbol"), Edn::str("native_target")),
            (Edn::tag("target"), Edn::tag("browser")),
            (Edn::tag("transport"), Edn::tag("rust-abi")),
          ]),
        ),
      ),
      (
        "js-target",
        function_entry(
          vec![],
          Arc::new(CalcitTypeAnnotation::Unit),
          Edn::map_from_iter([(Edn::tag("backend"), Edn::tag("js")), (Edn::tag("target"), Edn::tag("worker"))]),
        ),
      ),
      (
        "unknown-backend",
        function_entry(
          vec![],
          Arc::new(CalcitTypeAnnotation::Unit),
          Edn::map_from_iter([(Edn::tag("backend"), Edn::tag("python"))]),
        ),
      ),
    ];
    let report = export_snapshot(&snapshot(definitions), None).expect("inventory unknown lowering values");

    let codes = diagnostic_codes(&report);
    assert_eq!(report.summary.unsupported, 3);
    assert!(codes.contains("E_FFI_IR_TARGET_INVALID"));
    assert!(codes.contains("E_FFI_IR_INVOKE_UNKNOWN"));
    assert!(codes.contains("E_FFI_IR_TRANSPORT_UNKNOWN"));
    assert!(codes.contains("E_FFI_IR_BACKEND_UNKNOWN"));
  }

  #[test]
  fn invalid_lowering_diagnostics_and_revision_are_repeatable() {
    let metadata = || {
      Edn::map_from_iter([
        (Edn::tag("transport"), Edn::tag("edn-buffer-v1")),
        (Edn::tag("backend"), Edn::tag("native")),
        (Edn::tag("symbol"), Edn::str("not-portable!")),
        (Edn::tag("invoke"), Edn::tag("async")),
      ])
    };
    let make_report = || {
      export_snapshot(
        &snapshot(vec![(
          "invalid",
          function_entry(vec![], Arc::new(CalcitTypeAnnotation::Unit), metadata()),
        )]),
        None,
      )
      .expect("export repeatable invalid lowering")
    };

    assert_eq!(make_report(), make_report());
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
  fn metadata_map_order_does_not_change_raw_output_or_revision() {
    let metadata_a = Edn::map_from_iter([
      (Edn::tag("backend"), Edn::tag("native")),
      (Edn::tag("symbol"), Edn::str("read")),
      (Edn::tag("target"), Edn::tag("native")),
    ]);
    let metadata_b = Edn::map_from_iter([
      (Edn::tag("target"), Edn::tag("native")),
      (Edn::tag("symbol"), Edn::str("read")),
      (Edn::tag("backend"), Edn::tag("native")),
    ]);
    let make_report = |metadata| {
      export_snapshot(
        &snapshot(vec![(
          "read",
          function_entry(
            vec![Arc::new(CalcitTypeAnnotation::String)],
            Arc::new(CalcitTypeAnnotation::String),
            metadata,
          ),
        )]),
        None,
      )
      .expect("export canonically ordered metadata")
    };

    let first = make_report(metadata_a);
    let second = make_report(metadata_b);
    assert_eq!(
      first.interface.definitions[0].lowering.raw,
      "({} (:backend :native) (:symbol |read) (:target :native))"
    );
    assert_eq!(
      first.interface.definitions[0].lowering.raw,
      second.interface.definitions[0].lowering.raw
    );
    assert_eq!(first.revision, second.revision);
  }

  #[test]
  fn bundled_json_schema_matches_interface_version() {
    let schema: serde_json::Value = serde_json::from_str(FFI_INTERFACE_IR_SCHEMA).expect("parse bundled FFI Interface IR schema");

    assert_eq!(schema["$id"], FFI_INTERFACE_IR_SCHEMA_ID);
    assert_eq!(schema["properties"]["version"]["const"], FFI_INTERFACE_IR_VERSION);
    assert!(
      schema["required"]
        .as_array()
        .expect("top-level required properties")
        .iter()
        .any(|property| property == "declarations")
    );
    assert!(!FFI_INTERFACE_IR_SCHEMA.contains("\"const\": \"named\""));
  }
}
