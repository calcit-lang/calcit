//! Capability and boundary validation for JavaScript host operations.
//!
//! This module owns the `:js-ffi` feature gate, host-target validation,
//! external-object trait field checks, and the nullable/untyped access
//! diagnostics. It reads shared preprocess state (current function features and
//! program FFI metadata) through `super` but does not change language semantics.

use super::*;

/// Capability validation for operations that lower directly to JavaScript.
///
/// This deliberately runs after ordinary resolution and does not alter type
/// annotations, trait lookup, or generic bindings. A function's `:features`
/// declares what its implementation body may do; callers do not inherit it.
pub(super) fn require_js_ffi_feature(
  operation: &str,
  location: Option<NodeLocation>,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  if !codegen::codegen_mode() {
    return Ok(());
  }
  // Core runtime initialization uses internal lowering forms that are not
  // project-level FFI boundaries and do not carry public function schemas.
  if file_ns == calcit::CORE_NS {
    return Ok(());
  }
  validate_js_ffi_definition_target(operation, location.clone(), file_ns, def_name, call_stack)?;
  let policy = program::active_feature_policy("js-ffi");
  if matches!(policy, crate::snapshot::FeaturePolicy::Allow) {
    return Ok(());
  }

  let has_ffi = CURRENT_FN_FEATURES.with(|cell| {
    cell
      .borrow()
      .as_ref()
      .is_some_and(|features| features.contains(&EdnTag::new("js-ffi")))
  }) || program::lookup_def_schema(file_ns, def_name)
    .as_ref()
    .as_fn()
    .is_some_and(|fn_annot| fn_annot.features.contains(&EdnTag::new("js-ffi")));

  if has_ffi {
    return Ok(());
  }

  let message = format!(
    "[Warn] {operation} used in {file_ns}/{def_name} without `:js-ffi` feature in schema — isolate host operations in a binding function with `:features $ #{{}} :js-ffi`; read `calcit docs read js-interop.md --full` for the adapter and capability policy"
  );
  if matches!(policy, crate::snapshot::FeaturePolicy::Error) {
    return Err(CalcitErr::use_msg_stack_location_with_code(
      CalcitErrKind::Type,
      message.replacen("[Warn]", "[Error]", 1),
      "E_JS_FFI_FEATURE_REQUIRED",
      call_stack,
      location,
    ));
  }
  if let Some(location) = location {
    gen_check_warning_with_location_code(message, "W_JS_FFI_FEATURE_REQUIRED", location, check_warnings);
  } else {
    gen_check_warning_code(message, "W_JS_FFI_FEATURE_REQUIRED", file_ns, check_warnings);
  }
  Ok(())
}

pub(super) fn ffi_metadata_target(ffi: &cirru_edn::Edn) -> Option<crate::snapshot::SnapshotTarget> {
  crate::snapshot::parse_ffi_target(ffi).ok().flatten()
}

pub(super) fn ffi_metadata_value<'a>(ffi: &'a cirru_edn::Edn, key: &str) -> Option<&'a cirru_edn::Edn> {
  crate::snapshot::ffi_metadata_value(ffi, key)
}

pub(super) fn validate_js_ffi_target(
  expected: crate::snapshot::SnapshotTarget,
  operation: &str,
  location: Option<NodeLocation>,
  file_ns: &str,
  def_name: &str,
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  let Some(active) = program::active_entry_target() else {
    return Ok(());
  };
  if active == expected {
    return Ok(());
  }
  let message = format!(
    "[Error] {operation} requires `{}` target, but the selected entry targets `{}` in {file_ns}/{def_name}; read `calcit docs read js-interop.md --full` for target-specific bindings",
    expected.as_str(),
    active.as_str()
  );
  Err(CalcitErr::use_msg_stack_location_with_code(
    CalcitErrKind::Type,
    message,
    "E_JS_FFI_TARGET_MISMATCH",
    call_stack,
    location,
  ))
}

pub(super) fn validate_js_ffi_definition_target(
  operation: &str,
  location: Option<NodeLocation>,
  file_ns: &str,
  def_name: &str,
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  let Some(ffi) = program::lookup_def_ffi(file_ns, def_name) else {
    return Ok(());
  };
  let Some(expected) = ffi_metadata_target(&ffi) else {
    return Ok(());
  };
  validate_js_ffi_target(expected, operation, location, file_ns, def_name, call_stack)
}

pub(super) fn js_ffi_operation_name(head: &Calcit) -> Option<&'static str> {
  match head {
    Calcit::Symbol { sym, .. } if matches!(sym.as_ref(), "js-get" | "aget") => Some("JavaScript field read"),
    Calcit::Symbol { sym, .. } if matches!(sym.as_ref(), "js-set" | "aset") => Some("JavaScript field write"),
    Calcit::Method(_, calcit::MethodKind::InvokeNative | calcit::MethodKind::InvokeNativeOptional) => {
      Some("native JavaScript method call")
    }
    Calcit::Method(_, calcit::MethodKind::Access | calcit::MethodKind::AccessOptional) => Some("native JavaScript property access"),
    Calcit::Method(_, calcit::MethodKind::ExternalAccess(_)) => Some("external-object field access"),
    Calcit::Method(_, calcit::MethodKind::ExternalGet(_)) => Some("external-object field read"),
    Calcit::Method(_, calcit::MethodKind::ExternalSet(_)) => Some("external-object field write"),
    Calcit::Method(_, calcit::MethodKind::ExternalInvoke(_)) => Some("external-object method call"),
    Calcit::Import(CalcitImport { ns, def, .. }) if ns.as_ref() == calcit::CORE_NS && def.as_ref() == "unsafe-coerce" => {
      Some("unsafe host assertion `unsafe-coerce`")
    }
    _ => None,
  }
}

pub(super) fn require_js_ffi_feature_for_operation(
  head: &Calcit,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  if let Some(operation) = js_ffi_operation_name(head) {
    if let Some(receiver_type) = match head {
      Calcit::Method(_, calcit::MethodKind::ExternalAccess(value))
      | Calcit::Method(_, calcit::MethodKind::ExternalGet(value))
      | Calcit::Method(_, calcit::MethodKind::ExternalSet(value))
      | Calcit::Method(_, calcit::MethodKind::ExternalInvoke(value)) => Some(value.as_ref()),
      _ => None,
    } && let Some(traits) = trait_list_from_type(receiver_type)
    {
      for trait_def in traits {
        if let Some(ffi) = trait_def
          .definition_ref
          .as_deref()
          .and_then(|definition| definition.rsplit_once('/'))
          .and_then(|(ns, def)| program::lookup_def_ffi(ns, def))
          && let Some(expected) = ffi_metadata_target(&ffi)
        {
          validate_js_ffi_target(expected, operation, head.get_location(), file_ns, def_name, call_stack)?;
        }
      }
    }
    require_js_ffi_feature(operation, head.get_location(), file_ns, def_name, check_warnings, call_stack)?;
  }
  Ok(())
}

pub(super) fn reject_or_warn_on_nullable_js_ffi_dereference(
  head: &Calcit,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  if file_ns == calcit::CORE_NS {
    return Ok(());
  }

  let is_raw_dereference = matches!(
    head,
    Calcit::Method(_, calcit::MethodKind::Access | calcit::MethodKind::InvokeNative)
  ) || matches!(head, Calcit::Symbol { sym, .. } if matches!(sym.as_ref(), "aget" | "js-get"));
  if !is_raw_dereference {
    return Ok(());
  }

  let Some(receiver) = args.first() else {
    return Ok(());
  };
  let Some(receiver_type) = resolve_type_value(receiver, scope_types) else {
    return Ok(());
  };
  let CalcitTypeAnnotation::JsNullish(inner) = receiver_type.as_ref() else {
    return Ok(());
  };
  if !matches!(inner.as_ref(), CalcitTypeAnnotation::JsObject) {
    return Ok(());
  }

  let operation = match head {
    Calcit::Method(name, calcit::MethodKind::Access) => format!(".-{name}"),
    Calcit::Method(name, calcit::MethodKind::InvokeNative) => format!(".!{name}"),
    Calcit::Method(name, _) => format!(".{name}"),
    Calcit::Symbol { sym, .. } => sym.to_string(),
    _ => "JS FFI access".to_owned(),
  };
  let message = format!(
    "[Warn] JsNullish FFI value is dereferenced by `{operation}` in {file_ns}/{def_name}; use optional access, narrow with `js-present?`/`js-nullish?`, then validate or explicitly `unsafe-coerce` the opaque JsObject value"
  );

  let location = head.get_location().or_else(|| receiver.get_location());
  if strict_types_enabled() && should_emit_project_source_lint(file_ns) {
    return Err(CalcitErr::use_msg_stack_location_with_code(
      CalcitErrKind::Type,
      message.replacen("[Warn]", "[Error]", 1),
      "E_JS_FFI_NULLABLE_DEREF",
      call_stack,
      location,
    ));
  }

  if let Some(location) = location {
    gen_check_warning_with_location_code(message, "W_JS_FFI_NULLABLE_DEREF", location, check_warnings);
  } else {
    gen_check_warning_code(message, "W_JS_FFI_NULLABLE_DEREF", file_ns, check_warnings);
  }
  Ok(())
}

pub(super) fn static_js_field_name(form: &Calcit) -> Option<&str> {
  match form {
    Calcit::Tag(tag) => Some(tag.ref_str()),
    Calcit::Str(name) => Some(name.as_ref()),
    _ => None,
  }
}

pub(super) fn rewrite_typed_js_field_operation(head: &Calcit, args: &CalcitList, scope_types: &ScopeTypes) -> Option<Calcit> {
  let operation = match head {
    Calcit::Symbol { sym, .. } if sym.as_ref() == "js-get" => "get",
    Calcit::Symbol { sym, .. } if sym.as_ref() == "js-set" => "set",
    _ => return None,
  };
  let receiver = args.first()?;
  let field_name = static_js_field_name(args.get(1)?)?;
  let receiver_type = resolve_type_value(receiver, scope_types)?;
  let traits = trait_list_from_type(receiver_type.as_ref())?;
  let (trait_def, _) = find_trait_field_type(&traits, field_name)?;
  if !trait_is_external_object(trait_def) {
    return None;
  }
  let kind = if operation == "get" {
    calcit::MethodKind::ExternalGet(receiver_type)
  } else {
    calcit::MethodKind::ExternalSet(receiver_type)
  };
  let mut rewritten = vec![Calcit::Method(Arc::from(field_name), kind), receiver.clone()];
  if operation == "set" {
    rewritten.push(args.get(2)?.clone());
  }
  Some(Calcit::from(rewritten))
}

pub(super) fn check_typed_js_field_operation(
  head: &Calcit,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  let operation = match head {
    Calcit::Symbol { sym, .. } if sym.as_ref() == "js-get" => "js-get",
    Calcit::Symbol { sym, .. } if sym.as_ref() == "js-set" => "js-set",
    _ => return Ok(()),
  };
  let (Some(receiver), Some(key)) = (args.first(), args.get(1)) else {
    return Ok(());
  };
  let Some(field_name) = static_js_field_name(key) else {
    return Ok(());
  };
  let Some(receiver_type) = resolve_type_value(receiver, scope_types) else {
    return Ok(());
  };
  let Some(traits) = trait_list_from_type(receiver_type.as_ref()) else {
    return Ok(());
  };
  let external_traits = traits
    .iter()
    .filter(|trait_def| trait_is_external_object(trait_def.as_ref()))
    .cloned()
    .collect::<Vec<_>>();
  if external_traits.is_empty() {
    return Ok(());
  }
  let Some((field_trait, field_type)) = find_trait_field_type(&external_traits, field_name) else {
    let message = format!(
      "[Warn] `{operation}` cannot access undeclared external-object field `:{field_name}` in {file_ns}/{def_name}; declare the field on the external trait, use a dynamic key, or use raw `aget`/`aset`"
    );
    gen_check_warning_code_at(
      message,
      "W_JS_FFI_UNKNOWN_FIELD",
      file_ns,
      key.get_location().or_else(|| head.get_location()),
      check_warnings,
    );
    return Ok(());
  };
  let policy = program::active_feature_policy("js-ffi");
  if operation == "js-set"
    && !matches!(policy, crate::snapshot::FeaturePolicy::Allow)
    && !external_trait_field_is_writable(field_trait, field_name)
  {
    let message = format!(
      "[Warn] `js-set` cannot write read-only external-object field `:{field_name}` in {file_ns}/{def_name}; add `:writable $ #{{}} :{field_name}` to that trait's `:ffi` metadata or expose a mutating method instead; read `calcit docs read js-interop.md --full` for external-object contracts"
    );
    let location = key.get_location().or_else(|| head.get_location());
    if matches!(policy, crate::snapshot::FeaturePolicy::Error) {
      return Err(CalcitErr::use_msg_stack_location_with_code(
        CalcitErrKind::Type,
        message.replacen("[Warn]", "[Error]", 1),
        "E_JS_FFI_FIELD_READONLY",
        call_stack,
        location,
      ));
    }
    gen_check_warning_code_at(message, "W_JS_FFI_FIELD_READONLY", file_ns, location, check_warnings);
  }
  if operation == "js-set"
    && let Some(value) = args.get(2)
    && let Some(value_type) = resolve_type_value(value, scope_types)
    && !value_type.as_ref().is_compatible_with(field_type.as_ref())
  {
    let message = format!(
      "[Warn] `js-set` field `:{field_name}` expects {}, got {} in {file_ns}/{def_name}",
      field_type.to_brief_string(),
      value_type.to_brief_string()
    );
    gen_check_warning_code_at(
      message,
      "W_JS_FFI_FIELD_TYPE_MISMATCH",
      file_ns,
      value.get_location().or_else(|| head.get_location()),
      check_warnings,
    );
  }
  Ok(())
}

/// Raw `.-`/`.!`/`.?-`/`.?!`/`aget`/`aset`/`js-get`/`js-set` on a bare `JsObject` receiver
/// (no external-object trait attached) has nothing left to check statically.
/// Strict project source must attach an external-object trait when the key is
/// statically known. Compatibility mode keeps the opt-in inventory warning;
/// dynamic keys retain explicit raw JavaScript semantics.
pub(super) fn reject_or_warn_on_untyped_js_ffi_field_access(
  head: &Calcit,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  if file_ns == calcit::CORE_NS {
    return Ok(());
  }

  if !strict_types_enabled() && !warn_dyn_method_enabled() {
    return Ok(());
  }

  let (operation, field_name) = match head {
    Calcit::Method(name, calcit::MethodKind::Access) => (format!(".-{name}"), name.to_string()),
    Calcit::Method(name, calcit::MethodKind::AccessOptional) => (format!(".?-{name}"), name.to_string()),
    Calcit::Method(name, calcit::MethodKind::InvokeNative) => (format!(".!{name}"), name.to_string()),
    Calcit::Method(name, calcit::MethodKind::InvokeNativeOptional) => (format!(".?!{name}"), name.to_string()),
    Calcit::Symbol { sym, .. } if matches!(sym.as_ref(), "aget" | "js-get" | "aset" | "js-set") => {
      let Some(field_name) = args.get(1).and_then(static_js_field_name) else {
        // A dynamic (non-literal) key cannot be described by a trait field
        // either, so there is no actionable next step to suggest here.
        return Ok(());
      };
      (sym.to_string(), field_name.to_owned())
    }
    _ => return Ok(()),
  };

  let Some(receiver) = args.first() else {
    return Ok(());
  };
  let Some(receiver_type) = resolve_type_value(receiver, scope_types) else {
    return Ok(());
  };
  // Only the bare, non-nullable `JsObject` case is targeted here: it already
  // proves the value is a raw host object, and the key is a literal the
  // developer already knows, so declaring a trait is directly actionable.
  // Nullable receivers and declared traits are covered by other diagnostics.
  if !matches!(receiver_type.as_ref(), CalcitTypeAnnotation::JsObject) {
    return Ok(());
  }

  let message = format!(
    "[Warn] `{operation}` accesses untyped JS field `:{field_name}` on inferred `JsObject` in {file_ns}/{def_name}; declare an external-object trait (`deftrait ... :ffi {{:kind :external-object ...}}`) containing that field or method, then coerce the host value to the trait inside the lexical `:js-ffi` adapter; use a dynamic key only when raw lookup semantics are intentional"
  );

  let location = head.get_location().or_else(|| receiver.get_location());
  if strict_types_enabled() && should_emit_project_source_lint(file_ns) {
    return Err(CalcitErr::use_msg_stack_location_with_code(
      CalcitErrKind::Type,
      message.replacen("[Warn]", "[Error]", 1),
      "E_UNTYPED_JS_OBJECT_ACCESS",
      call_stack,
      location,
    ));
  }

  if warn_dyn_method_enabled() {
    gen_check_warning_code_at(message, "W_JS_FFI_UNTYPED_ACCESS", file_ns, location, check_warnings);
  }
  Ok(())
}

pub(super) fn reject_or_warn_on_legacy_js_nullish_predicate(
  head: &Calcit,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  if file_ns == calcit::CORE_NS {
    return Ok(());
  }
  let Some(operation) = canonical_absence_operation_name(head) else {
    return Ok(());
  };
  if !matches!(operation, "nil?" | "some?") {
    return Ok(());
  }
  let Some(value) = args.first() else {
    return Ok(());
  };
  let Some(value_type) = resolve_type_value(value, scope_types) else {
    return Ok(());
  };
  if !matches!(value_type.as_ref(), CalcitTypeAnnotation::JsNullish(_)) {
    return Ok(());
  }

  let message = format!(
    "[Warn] `{operation}` consumes a JsNullish FFI value in {file_ns}/{def_name}; use `js-nullish?` or `js-present?` so host nullability stays explicit"
  );
  let location = head.get_location().or_else(|| value.get_location());
  if strict_types_enabled() && should_emit_project_source_lint(file_ns) {
    return Err(CalcitErr::use_msg_stack_location_with_code(
      CalcitErrKind::Type,
      message.replacen("[Warn]", "[Error]", 1),
      "E_JS_FFI_NULLABLE_PREDICATE",
      call_stack,
      location,
    ));
  }

  if let Some(location) = location {
    gen_check_warning_with_location_code(message, "W_JS_FFI_NULLABLE_PREDICATE", location, check_warnings);
  } else {
    gen_check_warning_code(message, "W_JS_FFI_NULLABLE_PREDICATE", file_ns, check_warnings);
  }
  Ok(())
}

/// Checks whether a type exposes a declared field through an external-object trait.
pub(super) fn is_external_trait_field(type_value: &CalcitTypeAnnotation, field_name: &str) -> bool {
  trait_list_from_type(type_value)
    .is_some_and(|traits| find_trait_field_type(&traits, field_name).is_some_and(|(trait_def, _)| trait_is_external_object(trait_def)))
}

pub(crate) fn trait_is_external_object(trait_def: &CalcitTrait) -> bool {
  let Some(def_ref) = trait_def.definition_ref.as_deref() else {
    return false;
  };
  let Some((ns, def)) = def_ref.rsplit_once('/') else { return false };
  let Some(ffi) = program::lookup_def_ffi(ns, def) else {
    return false;
  };
  match ffi {
    cirru_edn::Edn::Struct(value) => value
      .pairs
      .iter()
      .find(|(key, _)| key.ref_str() == "kind")
      .is_some_and(|(_, value)| matches!(value, cirru_edn::Edn::Tag(tag) if tag.ref_str() == "external-object")),
    cirru_edn::Edn::Map(value) => value
      .get(&cirru_edn::Edn::Tag(EdnTag::new("kind")))
      .is_some_and(|value| matches!(value, cirru_edn::Edn::Tag(tag) if tag.ref_str() == "external-object")),
    _ => false,
  }
}

pub(super) fn external_trait_field_is_writable(trait_def: &CalcitTrait, field_name: &str) -> bool {
  let Some(def_ref) = trait_def.definition_ref.as_deref() else {
    return false;
  };
  let Some((ns, def)) = def_ref.rsplit_once('/') else {
    return false;
  };
  let Some(ffi) = program::lookup_def_ffi(ns, def) else {
    return false;
  };
  let Some(cirru_edn::Edn::Set(values)) = ffi_metadata_value(&ffi, "writable") else {
    return false;
  };
  values.0.iter().any(|value| match value {
    cirru_edn::Edn::Tag(tag) => tag.ref_str() == field_name,
    cirru_edn::Edn::Str(name) | cirru_edn::Edn::Symbol(name) => name.as_ref().trim_start_matches(':') == field_name,
    _ => false,
  })
}

pub(super) fn current_function_has_js_ffi_feature() -> bool {
  CURRENT_FN_FEATURES.with(|cell| {
    cell
      .borrow()
      .as_ref()
      .is_some_and(|features| features.iter().any(|feature| feature.ref_str() == "js-ffi"))
  })
}
