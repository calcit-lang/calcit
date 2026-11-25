use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

use crate::calcit::{
  Calcit, CalcitTypeAnnotation, NullableOrigin, SchemaKind, resolve_type_slot, type_annotation::ASYNC_INVOCATION_FEATURE,
  with_type_annotation_warning_context,
};

const MAX_TYPE_ID_DEPTH: usize = 128;

/// Compile-time metadata handle attached by preprocessing to lowered `any:box`
/// and `any:downcast` forms. Cannot be constructed from user Cirru source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledAnyTypeId {
  pub type_key: Arc<str>,
  pub type_hash: u64,
}

impl CompiledAnyTypeId {
  pub fn new(type_key: impl Into<Arc<str>>) -> Self {
    let type_key: Arc<str> = type_key.into();
    let type_hash = canonical_type_hash(&type_key);
    Self { type_key, type_hash }
  }

  pub fn into_calcit_handle(self) -> Calcit {
    Calcit::AnyRef(cirru_edn::EdnAnyRef::new(Arc::new(self)))
  }

  pub fn from_calcit_handle(value: &Calcit) -> Option<Arc<Self>> {
    let Calcit::AnyRef(reference) = value else {
      return None;
    };
    let guard = reference.0.read().ok()?;
    guard.as_any().downcast_ref::<Arc<Self>>().cloned()
  }
}

/// Error returned when a type cannot yield a closed, monomorphic canonical `TypeId`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalTypeIdError {
  pub code: &'static str,
  pub message: String,
}

impl CanonicalTypeIdError {
  pub(crate) fn open_type(message: impl Into<String>) -> Self {
    Self {
      code: "E_ANY_OPEN_TYPE",
      message: message.into(),
    }
  }
}

impl fmt::Display for CanonicalTypeIdError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "[{}] {}", self.code, self.message)
  }
}

impl std::error::Error for CanonicalTypeIdError {}

/// Deterministic 64-bit FNV-1a hash of a canonical `type_key`.
pub fn canonical_type_hash(type_key: &str) -> u64 {
  let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
  for byte in type_key.as_bytes() {
    hash ^= u64::from(*byte);
    hash = hash.wrapping_mul(0x0100_0000_01b3);
  }
  hash
}

/// Compute the canonical, backend-independent `type_key` for a closed,
/// monomorphic Calcit type annotation.
///
/// Rejects open or non-monomorphic types (`Dynamic`, `TypeVar`, `DynFn`, `AnyFn`,
/// `AnyStruct`, `AnyEnum`, `AnonymousEnum`, unresolved `TypeSlot`, bare non-external
/// `Trait`/`TraitSet`, `Never` including `List Never` / `Option Never`, etc.)
/// with `E_ANY_OPEN_TYPE`.
pub fn canonical_type_key(ty: &CalcitTypeAnnotation, default_ns: Option<&str>) -> Result<String, CanonicalTypeIdError> {
  let run = || {
    ty.validate_applied_type_args().map_err(CanonicalTypeIdError::open_type)?;
    let mut visiting = HashSet::new();
    let mut visited_aliases = Vec::new();
    canonical_type_key_inner(ty, &mut visiting, &mut visited_aliases, 0)
  };
  match default_ns {
    Some(ns) => with_type_annotation_warning_context(format!("{ns}/any-id"), run),
    None => run(),
  }
}

fn nominal_label(def_ref: Option<&str>, resolved_pair: Option<&(std::sync::Arc<str>, std::sync::Arc<str>)>, fallback: &str) -> String {
  if let Some(reference) = def_ref {
    reference.trim_start_matches('\'').to_owned()
  } else if let Some((ns, def)) = resolved_pair {
    format!("{ns}/{def}")
  } else {
    fallback.trim_start_matches('\'').to_owned()
  }
}

fn canonical_nominal_args(
  kind: &str,
  name: &str,
  expected_arity: usize,
  args: &[std::sync::Arc<CalcitTypeAnnotation>],
  visiting: &mut HashSet<String>,
  visited_aliases: &mut Vec<CalcitTypeAnnotation>,
  depth: usize,
) -> Result<String, CanonicalTypeIdError> {
  if args.len() != expected_arity {
    return Err(CanonicalTypeIdError::open_type(format!(
      "nominal `{name}` requires {expected_arity} concrete type argument(s), got {}; `Any` requires a fully monomorphic type",
      args.len()
    )));
  }
  let mut encoded_args = Vec::with_capacity(args.len());
  for arg in args {
    encoded_args.push(canonical_type_key_inner(arg.as_ref(), visiting, visited_aliases, depth + 1)?);
  }
  Ok(format!("{kind}:{name}[{}]", encoded_args.join(",")))
}

fn canonical_type_key_inner(
  ty: &CalcitTypeAnnotation,
  visiting: &mut HashSet<String>,
  visited_aliases: &mut Vec<CalcitTypeAnnotation>,
  depth: usize,
) -> Result<String, CanonicalTypeIdError> {
  if depth >= MAX_TYPE_ID_DEPTH {
    return Err(CanonicalTypeIdError::open_type(format!(
      "type `{}` exceeded recursion limit while computing canonical Any TypeId",
      ty.to_brief_string()
    )));
  }

  if let Some(open_name) = ty.open_family_name() {
    return Err(CanonicalTypeIdError::open_type(format!(
      "`{open_name}` is an open type family and cannot be boxed into or downcast from `Any`; use a concrete type"
    )));
  }

  match ty {
    CalcitTypeAnnotation::Nil => Ok(String::from("nil")),
    CalcitTypeAnnotation::Unit => Ok(String::from("unit")),
    CalcitTypeAnnotation::Bool => Ok(String::from("bool")),
    CalcitTypeAnnotation::Number => Ok(String::from("number")),
    CalcitTypeAnnotation::Numeric(kind) => Ok(format!("numeric:{}", kind.tag_name())),
    CalcitTypeAnnotation::String => Ok(String::from("string")),
    CalcitTypeAnnotation::Symbol => Ok(String::from("symbol")),
    CalcitTypeAnnotation::Tag => Ok(String::from("tag")),
    CalcitTypeAnnotation::Buffer => Ok(String::from("buffer")),
    CalcitTypeAnnotation::BufList => Ok(String::from("buf-list")),
    CalcitTypeAnnotation::CirruQuote => Ok(String::from("cirru-quote")),
    CalcitTypeAnnotation::Any => Ok(String::from("any")),

    CalcitTypeAnnotation::List(elem) => {
      let inner = canonical_type_key_inner(elem.as_ref(), visiting, visited_aliases, depth + 1)?;
      Ok(format!("list<{inner}>"))
    }
    CalcitTypeAnnotation::Set(elem) => {
      let inner = canonical_type_key_inner(elem.as_ref(), visiting, visited_aliases, depth + 1)?;
      Ok(format!("set<{inner}>"))
    }
    CalcitTypeAnnotation::Ref(inner) => {
      let inner_key = canonical_type_key_inner(inner.as_ref(), visiting, visited_aliases, depth + 1)?;
      Ok(format!("ref<{inner_key}>"))
    }
    CalcitTypeAnnotation::Tuple(items) => {
      let mut parts = Vec::with_capacity(items.len());
      for item in items.iter() {
        parts.push(canonical_type_key_inner(item.as_ref(), visiting, visited_aliases, depth + 1)?);
      }
      Ok(format!("tuple<{}>", parts.join(",")))
    }
    CalcitTypeAnnotation::Map(k, v) => {
      let k_key = canonical_type_key_inner(k.as_ref(), visiting, visited_aliases, depth + 1)?;
      let v_key = canonical_type_key_inner(v.as_ref(), visiting, visited_aliases, depth + 1)?;
      Ok(format!("map<{k_key},{v_key}>"))
    }
    CalcitTypeAnnotation::Nullable(inner, NullableOrigin::JsHost) => {
      let inner_key = canonical_type_key_inner(inner.as_ref(), visiting, visited_aliases, depth + 1)?;
      Ok(format!("js-nullish<{inner_key}>"))
    }
    CalcitTypeAnnotation::Nullable(inner, NullableOrigin::Inferred) => Err(CanonicalTypeIdError::open_type(format!(
      "inferred nullable type `{}` is open and cannot be boxed into or downcast from `Any`; handle nil or use `Option`",
      inner.to_brief_string()
    ))),

    CalcitTypeAnnotation::Fn(sig) => {
      if sig.fn_kind != SchemaKind::Fn {
        return Err(CanonicalTypeIdError::open_type(
          "macro values are compile-time constructs and cannot be used with `Any`",
        ));
      }
      if !sig.generics.is_empty() || !sig.cases.is_empty() {
        return Err(CanonicalTypeIdError::open_type(format!(
          "polymorphic or multi-case function type `{}` is not monomorphic and cannot be used with `Any`",
          ty.to_brief_string()
        )));
      }
      let mut params = Vec::with_capacity(sig.arg_types.len() + usize::from(sig.rest_type.is_some()));
      for arg in &sig.arg_types {
        params.push(canonical_type_key_inner(arg.as_ref(), visiting, visited_aliases, depth + 1)?);
      }
      if let Some(rest) = &sig.rest_type {
        let rest_key = canonical_type_key_inner(rest.as_ref(), visiting, visited_aliases, depth + 1)?;
        params.push(format!("...{rest_key}"));
      }
      let ret_key = canonical_type_key_inner(sig.return_type.as_ref(), visiting, visited_aliases, depth + 1)?;
      let mut features: Vec<&str> = sig
        .features
        .iter()
        .map(|tag| tag.ref_str())
        .filter(|name| *name != ASYNC_INVOCATION_FEATURE)
        .collect();
      features.sort_unstable();
      features.dedup();
      let arity = sig.call_arity();
      let arity_suffix = match arity.max {
        Some(max) => format!(":arity[{}..{max}]", arity.min),
        None => format!(":arity[{}..*]", arity.min),
      };
      let async_suffix = if sig.is_async_invocation() { ":async" } else { "" };
      let feature_suffix = if features.is_empty() {
        String::new()
      } else {
        format!(":features[{}]", features.join(","))
      };
      Ok(format!(
        "fn<({})->{ret_key}{arity_suffix}{async_suffix}{feature_suffix}>",
        params.join(",")
      ))
    }

    CalcitTypeAnnotation::Struct(struct_def, args) => {
      let name = nominal_label(struct_def.definition_ref.as_deref(), None, struct_def.name.ref_str());
      canonical_nominal_args(
        "struct",
        &name,
        struct_def.generics.len(),
        args.as_slice(),
        visiting,
        visited_aliases,
        depth,
      )
    }
    CalcitTypeAnnotation::StructValue(struct_def) => {
      let name = nominal_label(struct_def.definition_ref.as_deref(), None, struct_def.name.ref_str());
      canonical_nominal_args("struct", &name, struct_def.generics.len(), &[], visiting, visited_aliases, depth)
    }
    CalcitTypeAnnotation::Enum(enum_def, args) => {
      let name = nominal_label(enum_def.definition_ref().map(AsRef::as_ref), None, enum_def.name().ref_str());
      canonical_nominal_args(
        "enum",
        &name,
        enum_def.generics().len(),
        args.as_slice(),
        visiting,
        visited_aliases,
        depth,
      )
    }
    CalcitTypeAnnotation::EnumValue(enum_def) => {
      let name = nominal_label(enum_def.definition_ref().map(AsRef::as_ref), None, enum_def.name().ref_str());
      canonical_nominal_args("enum", &name, enum_def.generics().len(), &[], visiting, visited_aliases, depth)
    }

    CalcitTypeAnnotation::TypeSlot(slot_name) => {
      let visit_key = format!("slot:{slot_name}");
      if !visiting.insert(visit_key.clone()) {
        return Err(CanonicalTypeIdError::open_type(format!(
          "cyclic type slot `'{slot_name}` cannot yield a canonical `Any` TypeId"
        )));
      }
      let result = match resolve_type_slot(slot_name) {
        Some(bound) => canonical_type_key_inner(bound.as_ref(), visiting, visited_aliases, depth + 1),
        None => Err(CanonicalTypeIdError::open_type(format!(
          "unresolved type slot `'{slot_name}` is open and cannot be used with `Any`"
        ))),
      };
      visiting.remove(&visit_key);
      result
    }

    CalcitTypeAnnotation::TypeRef(raw_name, args) => {
      if let Some((struct_def, ref_pair)) = ty.resolve_to_struct_with_ref() {
        let name = nominal_label(struct_def.definition_ref.as_deref(), ref_pair.as_ref(), struct_def.name.ref_str());
        return canonical_nominal_args(
          "struct",
          &name,
          struct_def.generics.len(),
          args.as_slice(),
          visiting,
          visited_aliases,
          depth,
        );
      }
      if let Some((enum_def, ref_pair)) = ty.resolve_to_enum_with_ref() {
        let name = nominal_label(
          enum_def.definition_ref().map(AsRef::as_ref),
          ref_pair.as_ref(),
          enum_def.name().ref_str(),
        );
        return canonical_nominal_args(
          "enum",
          &name,
          enum_def.generics().len(),
          args.as_slice(),
          visiting,
          visited_aliases,
          depth,
        );
      }
      if let Some(union_def) = ty.resolve_to_union() {
        let name = union_def
          .definition_ref()
          .map(|r| r.trim_start_matches('\'').to_owned())
          .unwrap_or_else(|| union_def.name.ref_str().to_owned());
        return Ok(format!("union:{name}"));
      }
      if let Some((_trait_def, (ns, def))) = ty.resolve_to_external_object_trait_with_ref() {
        return Ok(format!("external:{ns}/{def}"));
      }
      if let Some(aliased) = ty.expand_single_type_alias() {
        if visited_aliases.contains(ty) {
          return Err(CanonicalTypeIdError::open_type(format!(
            "cyclic type alias `{raw_name}` cannot yield a canonical `Any` TypeId"
          )));
        }
        visited_aliases.push(ty.clone());
        let result = canonical_type_key_inner(aliased.as_ref(), visiting, visited_aliases, depth + 1);
        visited_aliases.pop();
        return result;
      }
      if let Some((trait_def, (ns, def))) = ty.resolve_to_source_trait_with_ref() {
        return Err(CanonicalTypeIdError::open_type(format!(
          "bare trait `{ns}/{def}` ({}) is an open existential boundary and cannot be boxed into or downcast from `Any`",
          trait_def.name
        )));
      }
      Err(CanonicalTypeIdError::open_type(format!(
        "unresolved or open type `{}` cannot be boxed into or downcast from `Any`",
        ty.to_brief_string()
      )))
    }

    CalcitTypeAnnotation::Trait(_) => {
      if let Some((_trait_def, (ns, def))) = ty.resolve_to_external_object_trait_with_ref() {
        Ok(format!("external:{ns}/{def}"))
      } else {
        Err(CanonicalTypeIdError::open_type(format!(
          "bare trait `{}` is an open existential boundary and cannot be boxed into or downcast from `Any`",
          ty.to_brief_string()
        )))
      }
    }
    CalcitTypeAnnotation::TraitSet(_) => Err(CanonicalTypeIdError::open_type(format!(
      "trait bound set `{}` is an open existential boundary and cannot be used with `Any`",
      ty.to_brief_string()
    ))),

    CalcitTypeAnnotation::Dynamic => Err(CanonicalTypeIdError::open_type(
      "`Dynamic` is an open type and cannot be boxed into or downcast from `Any`; add a concrete type annotation first",
    )),
    CalcitTypeAnnotation::TypeVar(name) => Err(CanonicalTypeIdError::open_type(format!(
      "unbound type variable `'{name}` is non-monomorphic and cannot be boxed into or downcast from `Any`"
    ))),
    CalcitTypeAnnotation::DynFn => Err(CanonicalTypeIdError::open_type(
      "`DynFn` / unannotated `Fn` signature is open and cannot be boxed into or downcast from `Any`; annotate concrete parameter and return types",
    )),
    CalcitTypeAnnotation::AnonymousEnum => Err(CanonicalTypeIdError::open_type(
      "anonymous `Enum` is an open type and cannot be boxed into or downcast from `Any`; declare a named `defenum`",
    )),
    CalcitTypeAnnotation::Never => Err(CanonicalTypeIdError::open_type(
      "`Never` (such as an empty list `[]` or unannotated `None`) has no concrete element type and cannot be boxed into or downcast from `Any`; annotate it explicitly",
    )),
    CalcitTypeAnnotation::JsObject => Err(CanonicalTypeIdError::open_type(
      "`JsObject` is an untyped host boundary and cannot be boxed into or downcast from `Any`; declare an external-object trait or decode it",
    )),
    CalcitTypeAnnotation::FfiResource => Err(CanonicalTypeIdError::open_type(
      "`FfiResource` is a native-only host handle and cannot be boxed into portable `Any`",
    )),
    CalcitTypeAnnotation::Method => Err(CanonicalTypeIdError::open_type(
      "`Method` shorthand cannot be boxed into or downcast from `Any`",
    )),
    CalcitTypeAnnotation::Variadic(_) => Err(CanonicalTypeIdError::open_type(format!(
      "variadic type `{}` is not a concrete runtime value type for `Any`",
      ty.to_brief_string()
    ))),
    CalcitTypeAnnotation::StructDef(_)
    | CalcitTypeAnnotation::EnumDef(_)
    | CalcitTypeAnnotation::Macro(_)
    | CalcitTypeAnnotation::Syntax(_)
    | CalcitTypeAnnotation::Custom(_) => Err(CanonicalTypeIdError::open_type(format!(
      "type `{}` is open or not a closed runtime value type and cannot be used with `Any`",
      ty.to_brief_string()
    ))),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::{CalcitEnumDef, CalcitList, CalcitStructDef, CalcitStructValue, CalcitTrait};
  use cirru_edn::EdnTag;
  use std::sync::Arc;

  #[test]
  fn canonical_type_keys_cover_closed_types_and_distinguish_numeric() {
    assert_eq!(canonical_type_key(&CalcitTypeAnnotation::Bool, None).unwrap(), "bool");
    assert_eq!(canonical_type_key(&CalcitTypeAnnotation::Number, None).unwrap(), "number");
    assert_eq!(
      canonical_type_key(&CalcitTypeAnnotation::Numeric(crate::calcit::CalcitNumericRefinement::Int32), None).unwrap(),
      "numeric:int32"
    );
    assert_ne!(
      canonical_type_key(&CalcitTypeAnnotation::Number, None).unwrap(),
      canonical_type_key(&CalcitTypeAnnotation::Numeric(crate::calcit::CalcitNumericRefinement::Int32), None).unwrap()
    );
    assert_eq!(canonical_type_key(&CalcitTypeAnnotation::Any, None).unwrap(), "any");
    assert_eq!(
      canonical_type_key(
        &CalcitTypeAnnotation::Map(Arc::new(CalcitTypeAnnotation::String), Arc::new(CalcitTypeAnnotation::Any)),
        None
      )
      .unwrap(),
      "map<string,any>"
    );
    assert_eq!(
      canonical_type_key(&CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Number)), None).unwrap(),
      "list<number>"
    );
    let fn_ty =
      CalcitTypeAnnotation::from_function_parts(vec![Arc::new(CalcitTypeAnnotation::Number)], Arc::new(CalcitTypeAnnotation::String));
    assert_eq!(canonical_type_key(&fn_ty, None).unwrap(), "fn<(number)->string:arity[1..1]>");
  }

  #[test]
  fn struct_value_matches_struct_annotation_key() {
    let def = CalcitStructDef::from_fields(EdnTag::new("Counter"), vec![EdnTag::new("n")]).with_definition_ref("app.main", "Counter");
    let def_arc = Arc::new(def);
    let val_key = canonical_type_key(&CalcitTypeAnnotation::StructValue(def_arc.clone()), None).unwrap();
    let ann_key = canonical_type_key(&CalcitTypeAnnotation::Struct(def_arc, Arc::new(vec![])), None).unwrap();
    assert_eq!(val_key, "struct:app.main/Counter[]");
    assert_eq!(val_key, ann_key);
    assert_eq!(canonical_type_hash(&val_key), canonical_type_hash(&ann_key));
  }

  #[test]
  fn enum_value_matches_enum_annotation_key_and_rejects_unapplied_generic() {
    let mut base_struct =
      CalcitStructDef::from_fields(EdnTag::new("Opt"), vec![EdnTag::new("none")]).with_definition_ref("app.main", "Opt");
    base_struct.generics = Arc::new(vec![Arc::from("T")]);
    let generic_def = Arc::new(
      CalcitEnumDef::from_struct(CalcitStructValue {
        struct_ref: Arc::new(base_struct),
        values: Arc::new(vec![Calcit::from(CalcitList::default())]),
      })
      .expect("enum prototype"),
    );
    let err = canonical_type_key(&CalcitTypeAnnotation::EnumValue(generic_def.clone()), None).unwrap_err();
    assert_eq!(err.code, "E_ANY_OPEN_TYPE");

    let concrete = canonical_type_key(
      &CalcitTypeAnnotation::Enum(generic_def, Arc::new(vec![Arc::new(CalcitTypeAnnotation::String)])),
      None,
    )
    .unwrap();
    assert_eq!(concrete, "enum:app.main/Opt[string]");
  }

  #[test]
  fn canonical_type_key_rejects_open_types_recursively() {
    let open_cases: Vec<CalcitTypeAnnotation> = vec![
      CalcitTypeAnnotation::Dynamic,
      CalcitTypeAnnotation::TypeVar(Arc::from("T")),
      CalcitTypeAnnotation::DynFn,
      CalcitTypeAnnotation::AnonymousEnum,
      CalcitTypeAnnotation::Never,
      CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Never)),
      CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Dynamic)),
      CalcitTypeAnnotation::Map(
        Arc::new(CalcitTypeAnnotation::String),
        Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("V"))),
      ),
      CalcitTypeAnnotation::TypeSlot(Arc::from("MissingSlot")),
      CalcitTypeAnnotation::JsObject,
      CalcitTypeAnnotation::FfiResource,
      CalcitTypeAnnotation::Trait(Arc::new(CalcitTrait::new_reference("Show"))),
      CalcitTypeAnnotation::Nullable(Arc::new(CalcitTypeAnnotation::Number), NullableOrigin::Inferred),
      crate::calcit::type_annotation::CalcitTypeAnnotation::open_family_type("AnyStruct")
        .unwrap()
        .as_ref()
        .clone(),
    ];
    for case in open_cases {
      let err = canonical_type_key(&case, None).expect_err(&format!("expected E_ANY_OPEN_TYPE for {case}"));
      assert_eq!(err.code, "E_ANY_OPEN_TYPE", "unexpected code for {case}: {err}");
    }
  }

  #[test]
  fn fn_type_key_distinguishes_async_and_sorted_effect_features() {
    let base = match CalcitTypeAnnotation::from_function_parts(
      vec![Arc::new(CalcitTypeAnnotation::Number)],
      Arc::new(CalcitTypeAnnotation::Number),
    ) {
      CalcitTypeAnnotation::Fn(sig) => sig.as_ref().clone(),
      other => panic!("expected Fn, got {other}"),
    };

    let sync_pure = CalcitTypeAnnotation::Fn(Arc::new(base.clone()));
    let async_pure = CalcitTypeAnnotation::Fn(Arc::new(base.with_async_invocation()));
    let mut io_state_a_sig = base.clone();
    io_state_a_sig.features = Arc::new([EdnTag::new("io"), EdnTag::new("state")].into_iter().collect());
    let io_state_a = CalcitTypeAnnotation::Fn(Arc::new(io_state_a_sig));
    let mut io_state_b_sig = base.clone();
    io_state_b_sig.features = Arc::new([EdnTag::new("state"), EdnTag::new("io")].into_iter().collect());
    let io_state_b = CalcitTypeAnnotation::Fn(Arc::new(io_state_b_sig));
    let mut io_only_sig = base;
    io_only_sig.features = Arc::new([EdnTag::new("io")].into_iter().collect());
    let io_only = CalcitTypeAnnotation::Fn(Arc::new(io_only_sig));

    let k_sync = canonical_type_key(&sync_pure, None).unwrap();
    let k_async = canonical_type_key(&async_pure, None).unwrap();
    let k_io_state_a = canonical_type_key(&io_state_a, None).unwrap();
    let k_io_state_b = canonical_type_key(&io_state_b, None).unwrap();
    let k_io_only = canonical_type_key(&io_only, None).unwrap();

    assert_eq!(k_sync, "fn<(number)->number:arity[1..1]>");
    assert_eq!(k_async, "fn<(number)->number:arity[1..1]:async>");
    assert_eq!(k_io_state_a, "fn<(number)->number:arity[1..1]:features[io,state]>");
    assert_eq!(k_io_state_a, k_io_state_b);
    assert_eq!(k_io_only, "fn<(number)->number:arity[1..1]:features[io]>");
    assert_ne!(k_sync, k_async);
    assert_ne!(k_sync, k_io_only);
    assert_ne!(k_io_only, k_io_state_a);
  }

  #[test]
  fn fn_type_key_distinguishes_required_option_from_optional_call_arity() {
    use crate::calcit::ProcArity;

    let opt_num = Arc::new(CalcitTypeAnnotation::TypeRef(
      Arc::from("calcit.core/Option"),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]),
    ));
    let opt_param_sig = match CalcitTypeAnnotation::from_function_parts(vec![opt_num], Arc::new(CalcitTypeAnnotation::Number)) {
      CalcitTypeAnnotation::Fn(sig) => sig.as_ref().clone(),
      other => panic!("expected Fn, got {other}"),
    };
    assert_eq!(opt_param_sig.call_arity(), ProcArity { min: 0, max: Some(1) });
    let mut required_opt_sig = opt_param_sig;
    required_opt_sig.runtime_arity = Some(ProcArity { min: 1, max: Some(1) });
    assert_eq!(required_opt_sig.call_arity(), ProcArity { min: 1, max: Some(1) });

    let base_sig = match CalcitTypeAnnotation::from_function_parts(
      vec![Arc::new(CalcitTypeAnnotation::Number)],
      Arc::new(CalcitTypeAnnotation::Number),
    ) {
      CalcitTypeAnnotation::Fn(sig) => sig.as_ref().clone(),
      other => panic!("expected Fn, got {other}"),
    };
    let fixed_inferred = CalcitTypeAnnotation::Fn(Arc::new(base_sig.clone()));
    let mut fixed_explicit_sig = base_sig.clone();
    fixed_explicit_sig.runtime_arity = Some(ProcArity { min: 1, max: Some(1) });
    let fixed_explicit = CalcitTypeAnnotation::Fn(Arc::new(fixed_explicit_sig));

    let mut optional_sig = base_sig.clone();
    optional_sig.runtime_arity = Some(ProcArity { min: 0, max: Some(1) });
    let optional_fn = CalcitTypeAnnotation::Fn(Arc::new(optional_sig));

    let mut variadic_sig = base_sig;
    variadic_sig.rest_type = Some(Arc::new(CalcitTypeAnnotation::String));
    variadic_sig.runtime_arity = Some(ProcArity { min: 1, max: None });
    let variadic_fn = CalcitTypeAnnotation::Fn(Arc::new(variadic_sig));

    let k_fixed_inferred = canonical_type_key(&fixed_inferred, None).unwrap();
    let k_fixed_explicit = canonical_type_key(&fixed_explicit, None).unwrap();
    let k_optional = canonical_type_key(&optional_fn, None).unwrap();
    let k_variadic = canonical_type_key(&variadic_fn, None).unwrap();

    assert_eq!(k_fixed_inferred, k_fixed_explicit);
    assert_eq!(k_fixed_inferred, "fn<(number)->number:arity[1..1]>");
    assert_eq!(k_optional, "fn<(number)->number:arity[0..1]>");
    assert_eq!(k_variadic, "fn<(number,...string)->number:arity[1..*]>");
    assert_ne!(k_fixed_explicit, k_optional);
    assert_ne!(k_fixed_explicit, k_variadic);
  }
}
