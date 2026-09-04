//! Type checking module.
//!
//! Implements the **checking direction** of bidirectional type checking:
//! given expected parameter types (from function/proc signatures),
//! validate that actual argument types conform.
//!
//! Key entry points:
//! - `check_proc_arg_types` — check built-in proc call arguments
//! - `check_core_fn_arg_types` — check core arithmetic fn arguments
//! - `check_local_fn_call_arg_types` — check local fn variable call arguments
//! - `check_user_fn_arg_types` — check user-defined fn call arguments
//! - `check_function_return_type` — check fn body return type vs declaration
//! - `detect_return_type_hint_from_processed_body` — extract hint-fn return type

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use super::type_inference::{infer_struct_field_type, infer_unhinted_callback_signature};
use crate::calcit::{
  self, Calcit, CalcitFn, CalcitGenericBound, CalcitList, CalcitLocal, CalcitProc, CalcitSyntax, CalcitTypeAnnotation, LocatedWarning,
  NodeLocation,
};
use crate::program;

use super::{
  ScopeTypes, check_enum_construction, check_enum_nth_bounds, gen_check_warning, gen_check_warning_code, gen_check_warning_code_at,
  resolve_type_value, tag_annotation,
};

// ---------------------------------------------------------------------------
// Unified arg-type checking loop
// ---------------------------------------------------------------------------

struct CheckContext<'a> {
  head_form: &'a Calcit,
  args: &'a CalcitList,
  expected_types: &'a [Arc<CalcitTypeAnnotation>],
  where_bounds: &'a [CalcitGenericBound],
  scope_types: &'a ScopeTypes,
  file_ns: &'a str,
  call_location: Option<NodeLocation>,
  warning_code: &'static str,
  check_warnings: &'a RefCell<Vec<LocatedWarning>>,
}

fn append_js_ffi_type_hint(mut message: String, actual_type: &str) -> String {
  if actual_type.contains(":js-object") {
    message.push_str(
      "; JS FFI values stay opaque after JsNullish checks, so validate/convert the value or use `unsafe-coerce` only at a trusted boundary",
    );
  }
  message
}

fn append_option_migration_hint(
  mut message: String,
  expected_type: &CalcitTypeAnnotation,
  actual_type: &CalcitTypeAnnotation,
) -> String {
  if actual_type.is_option_type() && !expected_type.is_option_type() {
    message.push_str(&format!(
      "; inferred type `{}` is an Option rather than its payload; use a matching typed `*-or` query helper when available or `.unwrap-or` for a safe default, or native `match` (legacy `tag-match`) to handle both variants before passing it here",
      actual_type.to_brief_string()
    ));
  }
  message
}

fn check_generic_trait_bounds(ctx: &CheckContext<'_>, bindings: &HashMap<Arc<str>, Arc<CalcitTypeAnnotation>>) {
  if ctx.where_bounds.is_empty() {
    return;
  }

  let expr_str = format!(
    "{} {}",
    ctx.head_form,
    ctx.args.iter().map(|a| format!("{a}")).collect::<Vec<_>>().join(" ")
  );

  for bound in ctx.where_bounds {
    let Some(actual_type) = bindings.get(&bound.name) else {
      continue;
    };
    if matches!(actual_type.as_ref(), CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::DynFn) {
      continue;
    }

    let required = bound.as_type_annotation();
    if actual_type.as_ref().matches_annotation(required.as_ref()) {
      continue;
    }

    let warning_location = ctx
      .call_location
      .clone()
      .or_else(|| ctx.args.first().and_then(Calcit::get_location));
    gen_check_warning_code_at(
      format!(
        "[Warn] call binds generic `'{}' to `{}`, but it does not satisfy trait bound `{}` at {}/{}\n  Expression: `{}`",
        bound.name,
        actual_type.to_brief_string(),
        required.to_brief_string(),
        ctx.file_ns,
        match ctx.head_form {
          Calcit::Import(import) => import.def.as_ref(),
          Calcit::Symbol { sym, .. } => sym.as_ref(),
          Calcit::Local(local) => local.sym.as_ref(),
          _ => "<call>",
        },
        expr_str
      ),
      "W_GENERIC_WHERE_BOUND_MISMATCH",
      ctx.file_ns,
      warning_location,
      ctx.check_warnings,
    );
  }
}

pub(crate) struct CallTypeCheckInfo<'a> {
  pub file_ns: &'a str,
  pub def_name: &'a str,
  pub call_location: Option<NodeLocation>,
}

fn specialize_core_expected_types(
  fn_info: &CalcitFn,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  expected_types: &[Arc<CalcitTypeAnnotation>],
) -> Option<Vec<Arc<CalcitTypeAnnotation>>> {
  if fn_info.def_ns.as_ref() != calcit::CORE_NS {
    return None;
  }
  if !matches!(fn_info.name.as_ref(), "get" | "includes?" | "update") {
    return None;
  }
  let receiver = args.first()?;
  let receiver_type = resolve_type_value(receiver, scope_types)?;
  let mut specialized = expected_types.to_vec();

  match fn_info.name.as_ref() {
    "get" if expected_types.len() >= 2 && args.len() >= 2 => {
      specialized[1] = match receiver_type.as_ref() {
        CalcitTypeAnnotation::Map(key_type, _) => key_type.clone(),
        CalcitTypeAnnotation::List(_) | CalcitTypeAnnotation::String => Arc::new(CalcitTypeAnnotation::Number),
        CalcitTypeAnnotation::EnumValue(_) | CalcitTypeAnnotation::AnonymousEnum => Arc::new(CalcitTypeAnnotation::Number),
        value if value.resolve_to_enum().is_some() => Arc::new(CalcitTypeAnnotation::Number),
        _ => return None,
      };
      Some(specialized)
    }
    "includes?" if expected_types.len() >= 2 && args.len() >= 2 => {
      specialized[1] = match receiver_type.as_ref() {
        CalcitTypeAnnotation::Map(_, value_type) => value_type.clone(),
        CalcitTypeAnnotation::List(item_type) | CalcitTypeAnnotation::Set(item_type) => item_type.clone(),
        CalcitTypeAnnotation::String => Arc::new(CalcitTypeAnnotation::String),
        _ => return None,
      };
      Some(specialized)
    }
    "update" => specialize_update_expected_types(args, receiver_type.as_ref(), scope_types, expected_types),
    _ => None,
  }
}

fn specialize_update_expected_types(
  args: &CalcitList,
  receiver_type: &CalcitTypeAnnotation,
  scope_types: &ScopeTypes,
  expected_types: &[Arc<CalcitTypeAnnotation>],
) -> Option<Vec<Arc<CalcitTypeAnnotation>>> {
  if expected_types.len() < 3 || args.len() < 3 {
    return None;
  }

  let receiver = args.first()?;
  let mut specialized = expected_types.to_vec();
  let value_type = match receiver_type {
    CalcitTypeAnnotation::List(item_type) => {
      specialized[1] = Arc::new(CalcitTypeAnnotation::Number);
      item_type.clone()
    }
    CalcitTypeAnnotation::Map(key_type, value_type) => {
      specialized[1] = key_type.clone();
      value_type.clone()
    }
    CalcitTypeAnnotation::Struct(_, _) | CalcitTypeAnnotation::StructValue(_) => {
      let field_name = match args.get(1)? {
        Calcit::Tag(tag) => tag.ref_str(),
        Calcit::Str(text) => text.as_ref(),
        Calcit::Symbol { sym, .. } => sym.as_ref(),
        _ => return None,
      };
      infer_struct_field_type(receiver, field_name, scope_types)?
    }
    _ => return None,
  };
  specialized[2] = Arc::new(CalcitTypeAnnotation::from_function_parts(vec![value_type.clone()], value_type));
  Some(specialized)
}

impl<'a> CheckContext<'a> {
  fn emit_warning(
    &self,
    arg_idx: usize,
    expected_type: &CalcitTypeAnnotation,
    actual_type: &CalcitTypeAnnotation,
    make_warning: impl Fn(usize, &str, &str, String) -> String,
  ) {
    let expected_str = expected_type.to_brief_string();
    let actual_str = actual_type.to_brief_string();
    let expr_str = format!(
      "{} {}",
      self.head_form,
      self.args.iter().map(|a| format!("{a}")).collect::<Vec<_>>().join(" ")
    );
    let warning_location = self
      .args
      .get(arg_idx - 1)
      .and_then(Calcit::get_location)
      .or_else(|| self.call_location.clone());
    gen_check_warning_code_at(
      append_js_ffi_type_hint(
        append_option_migration_hint(
          make_warning(arg_idx, &expected_str, &actual_str, expr_str),
          expected_type,
          actual_type,
        ),
        &actual_str,
      ),
      self.warning_code,
      self.file_ns,
      warning_location,
      self.check_warnings,
    );
  }
}

/// Core loop shared by `check_user_fn_arg_types`, `check_local_fn_call_arg_types`,
/// and the proc checking path. Walks `(arg, expected_type)` pairs, resolves
/// actual types from `scope_types`, and emits warnings on mismatch.
///
/// `make_warning` is called with `(arg_index_1based, expected_brief, actual_brief, expr_str)`
/// and should return the full warning message string.
fn check_arg_types_loop<F>(ctx: CheckContext<'_>, make_warning: F)
where
  F: Fn(usize, &str, &str, String) -> String,
{
  // Check if we have spreading args — can't check with spread
  for arg in ctx.args.iter() {
    if matches!(arg, Calcit::Syntax(CalcitSyntax::ArgSpread, _)) {
      return;
    }
  }

  let mut bindings: HashMap<Arc<str>, Arc<CalcitTypeAnnotation>> = HashMap::new();
  for (idx, (arg, expected_type)) in ctx.args.iter().zip(ctx.expected_types.iter()).enumerate() {
    if matches!(expected_type.as_ref(), CalcitTypeAnnotation::Dynamic) {
      continue;
    }

    // Handle variadic argument type
    if let CalcitTypeAnnotation::Variadic(inner_type) = expected_type.as_ref() {
      for (rest_idx, rest_arg) in ctx.args.iter().skip(idx).enumerate() {
        if let Some(actual_type) = resolve_type_value(rest_arg, ctx.scope_types)
          && !actual_type.as_ref().matches_with_bindings(inner_type.as_ref(), &mut bindings)
        {
          ctx.emit_warning(idx + rest_idx + 1, inner_type.as_ref(), actual_type.as_ref(), &make_warning);
        }
      }
      return; // Done after variadic
    }

    if let Some(actual_type) = resolve_type_value(arg, ctx.scope_types) {
      let callback_needs_inference = matches!(actual_type.as_ref(), CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::DynFn)
        || matches!(actual_type.as_ref(), CalcitTypeAnnotation::Fn(signature) if matches!(signature.return_type.as_ref(), CalcitTypeAnnotation::Dynamic) || signature.return_type.contains_type_var());
      let actual_type = if callback_needs_inference
        && matches!(expected_type.as_ref(), CalcitTypeAnnotation::Fn(_))
        && let Calcit::List(callback) = arg
      {
        infer_unhinted_callback_signature(callback, ctx.scope_types).unwrap_or(actual_type)
      } else {
        actual_type
      };
      if !actual_type.as_ref().matches_with_bindings(expected_type.as_ref(), &mut bindings) {
        ctx.emit_warning(idx + 1, expected_type.as_ref(), actual_type.as_ref(), &make_warning);
      }
    }
  }

  check_generic_trait_bounds(&ctx, &bindings);
}

// ---------------------------------------------------------------------------
// Public check functions
// ---------------------------------------------------------------------------

/// Check Proc argument types against type signature
pub(crate) fn check_proc_arg_types(
  proc: &CalcitProc,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  call_location: Option<NodeLocation>,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  // Get type signature for this proc
  let Some(signature) = proc.get_type_signature() else {
    return;
  };

  // Check if we have spreading args
  for arg in args.iter() {
    if matches!(arg, Calcit::Syntax(CalcitSyntax::ArgSpread, _)) {
      return;
    }
  }

  // Check argument count
  let Some(arity) = proc.arity() else {
    return;
  };
  let min_count = arity.min;
  let max_count = arity.max.unwrap_or(usize::MAX);
  let has_variadic = arity.max.is_none();
  let actual_count = args.len();

  if !has_variadic {
    if actual_count < min_count || actual_count > max_count {
      let expected_str = if min_count == max_count {
        format!("{min_count}")
      } else {
        format!("{min_count}~{max_count}")
      };
      gen_check_warning(
        format!(
          "[Warn] Proc `{}` expects {} args, got {} in call `({} {})`, at {file_ns}/{def_name}",
          proc.as_ref(),
          expected_str,
          actual_count,
          proc.as_ref(),
          args.iter().map(|a| format!("{a}")).collect::<Vec<_>>().join(" ")
        ),
        file_ns,
        check_warnings,
      );
    }
  } else if actual_count < min_count {
    gen_check_warning(
      format!(
        "[Warn] Proc `{}` expects at least {} args, got {} in call `({} {})`, at {file_ns}/{def_name}",
        proc.as_ref(),
        min_count,
        actual_count,
        proc.as_ref(),
        args.iter().map(|a| format!("{a}")).collect::<Vec<_>>().join(" ")
      ),
      file_ns,
      check_warnings,
    );
  }

  if matches!(
    proc,
    CalcitProc::NativeStruct
      | CalcitProc::NativeStructPartial
      | CalcitProc::NativeStructGet
      | CalcitProc::NativeStructNth
      | CalcitProc::NativeLooseStruct
  ) {
    return;
  }

  if matches!(proc, CalcitProc::NativeNamedEnumNew) {
    check_enum_construction(args, scope_types, file_ns, def_name, check_warnings);
    return;
  }

  if matches!(proc, CalcitProc::NativeEnumNth) {
    check_enum_nth_bounds(args, scope_types, file_ns, def_name, check_warnings);
    return;
  }

  // Parameter omission is represented by Proc arity metadata. Optional<T>
  // remains a value type here and must accept either T or nil.
  let mut bindings: HashMap<Arc<str>, Arc<CalcitTypeAnnotation>> = HashMap::new();

  for (idx, (arg, expected_type)) in args.iter().zip(signature.arg_types.iter()).enumerate() {
    if matches!(expected_type.as_ref(), CalcitTypeAnnotation::Variadic(_)) {
      break;
    }

    if matches!(expected_type.as_ref(), CalcitTypeAnnotation::Dynamic) {
      continue;
    }

    if let Some(actual_type) = resolve_type_value(arg, scope_types)
      && !actual_type.as_ref().matches_with_bindings(expected_type.as_ref(), &mut bindings)
    {
      let expected_str = expected_type.as_ref().to_brief_string();
      let actual_str = actual_type.as_ref().to_brief_string();
      let warning_location = arg.get_location().or_else(|| call_location.clone());
      gen_check_warning_code_at(
        append_option_migration_hint(
          format!(
            "[Warn] Proc `{}` arg {} expects type `{expected_str}`, but got `{actual_str}` in call at {file_ns}/{def_name}",
            proc.as_ref(),
            idx + 1
          ),
          expected_type.as_ref(),
          actual_type.as_ref(),
        ),
        "W_PROC_ARG_TYPE_MISMATCH",
        file_ns,
        warning_location,
        check_warnings,
      );
    }
  }
}

/// Check core arithmetic fn arguments (hardcoded :number for +, -, *, /).
pub(crate) fn check_core_fn_arg_types(
  fn_info: &CalcitFn,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  call_location: Option<NodeLocation>,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  if fn_info.def_ns.as_ref() != calcit::CORE_NS {
    return;
  }

  if !fn_info.arg_types.is_empty()
    && fn_info
      .arg_types
      .iter()
      .any(|t| !matches!(t.as_ref(), CalcitTypeAnnotation::Dynamic))
  {
    return;
  }

  let needs_number_args = matches!(fn_info.name.as_ref(), "+" | "-" | "*" | "/");
  if !needs_number_args {
    return;
  }

  let expected_type = tag_annotation("number");

  for (idx, arg) in args.iter().enumerate() {
    if let Some(actual_type) = resolve_type_value(arg, scope_types)
      && !actual_type.as_ref().matches_annotation(expected_type.as_ref())
    {
      let actual_str = actual_type.as_ref().to_brief_string();
      let warning_location = arg.get_location().or_else(|| call_location.clone());
      gen_check_warning_code_at(
        append_option_migration_hint(
          format!(
            "[Warn] Function `calcit.core/{}` arg {} expects type `:number`, but got `{actual_str}` in call at {file_ns}/{def_name}",
            fn_info.name,
            idx + 1
          ),
          expected_type.as_ref(),
          actual_type.as_ref(),
        ),
        "W_CORE_FN_ARG_TYPE_MISMATCH",
        file_ns,
        warning_location,
        check_warnings,
      );
    }
  }
}

/// Check argument types when calling a local variable with a known Fn type.
pub(crate) fn check_local_fn_call_arg_types(
  head_form: &Calcit,
  local: &CalcitLocal,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  call_info: &CallTypeCheckInfo<'_>,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  let local_type = if matches!(*local.type_info, CalcitTypeAnnotation::Dynamic) {
    match scope_types.get(&local.sym) {
      Some(t) => t.as_ref(),
      None => return,
    }
  } else {
    &*local.type_info
  };

  let CalcitTypeAnnotation::Fn(fn_annot) = local_type else {
    return;
  };

  if fn_annot.arg_types.is_empty()
    || fn_annot
      .arg_types
      .iter()
      .all(|t| matches!(t.as_ref(), CalcitTypeAnnotation::Dynamic))
  {
    return;
  }

  let local_sym = local.sym.clone();
  let def_name = call_info.def_name.to_owned();
  let file_ns_owned = call_info.file_ns.to_owned();
  let ctx = CheckContext {
    head_form,
    args,
    expected_types: &fn_annot.arg_types,
    where_bounds: fn_annot.where_bounds.as_ref(),
    scope_types,
    file_ns: call_info.file_ns,
    call_location: call_info.call_location.clone(),
    warning_code: "W_LOCAL_FN_ARG_TYPE_MISMATCH",
    check_warnings,
  };
  check_arg_types_loop(ctx, |arg_idx, expected_str, actual_str, expr_str| {
    format!(
      "[Warn] calling `{local_sym}` arg {arg_idx} expects type `{expected_str}`, but got `{actual_str}` at {file_ns_owned}/{def_name}\n  Expression: ({local_sym} {expr_str})"
    )
  });
}

/// Check user-defined function argument types against type annotations.
pub(crate) fn check_user_fn_arg_types(
  fn_info: &CalcitFn,
  head_form: &Calcit,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  call_info: &CallTypeCheckInfo<'_>,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  let effective_schema = program::lookup_def_schema(&fn_info.def_ns, &fn_info.name);
  let schema_fn_annot = match effective_schema.as_ref() {
    CalcitTypeAnnotation::Fn(fn_annot) => Some(fn_annot.as_ref()),
    _ => None,
  };

  let expected_types = if fn_info.arg_types.is_empty() || fn_info.arg_types.iter().all(|t| matches!(**t, CalcitTypeAnnotation::Dynamic))
  {
    let Some(fn_annot) = schema_fn_annot else {
      return;
    };
    fn_annot.arg_types.as_slice()
  } else {
    fn_info.arg_types.as_slice()
  };

  let where_bounds = if fn_info.where_bounds.is_empty() {
    schema_fn_annot
      .map(|fn_annot| fn_annot.where_bounds.as_slice())
      .unwrap_or(fn_info.where_bounds.as_ref())
  } else {
    fn_info.where_bounds.as_ref()
  };

  let specialized_expected_types = specialize_core_expected_types(fn_info, args, scope_types, expected_types);
  if specialized_expected_types.is_none()
    && (expected_types.is_empty()
      || expected_types
        .iter()
        .all(|expected| matches!(expected.as_ref(), CalcitTypeAnnotation::Dynamic)))
  {
    return;
  }
  let expected_types = specialized_expected_types.as_deref().unwrap_or(expected_types);

  let fn_def_ns = fn_info.def_ns.clone();
  let fn_name = fn_info.name.clone();
  let equality_migration = fn_def_ns.as_ref() == crate::calcit::CORE_NS && matches!(fn_name.as_ref(), "=" | "not=" | "/=");
  let def_name = call_info.def_name.to_owned();
  let file_ns_owned = call_info.file_ns.to_owned();
  let ctx = CheckContext {
    head_form,
    args,
    expected_types,
    where_bounds,
    scope_types,
    file_ns: call_info.file_ns,
    call_location: call_info.call_location.clone(),
    warning_code: "W_FN_ARG_TYPE_MISMATCH",
    check_warnings,
  };
  check_arg_types_loop(ctx, |arg_idx, expected_str, actual_str, expr_str| {
    let migration = if equality_migration {
      "\n  Migration: public equality now requires operands of one static type. Normalize both values, narrow Dynamic/FFI values with a typed adapter, validator, or assert-type, use a type predicate for category checks, or pattern-match nominal values before comparing."
    } else {
      ""
    };
    format!(
      "[Warn] Function `{fn_def_ns}/{fn_name}` arg {arg_idx} expects type `{expected_str}`, but got `{actual_str}` in call at {file_ns_owned}/{def_name}\n  Expression: `{expr_str}`{migration}"
    )
  });
}

// ---------------------------------------------------------------------------
// Return-type checking
// ---------------------------------------------------------------------------

/// Extract return type hint from processed function body.
pub(crate) fn detect_return_type_hint_from_processed_body(processed_body: &[Calcit]) -> Arc<CalcitTypeAnnotation> {
  for form in processed_body {
    if let Some(hint) = CalcitTypeAnnotation::extract_return_type_from_hint_form(form) {
      return hint;
    }
  }
  crate::calcit::DYNAMIC_TYPE.clone()
}

/// Check function return type matches declared return_type.
pub(crate) fn check_function_return_type(
  fn_body: &[Calcit],
  declared_return_type: &Arc<CalcitTypeAnnotation>,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  if matches!(**declared_return_type, CalcitTypeAnnotation::Dynamic) {
    return;
  }

  if fn_body.is_empty() {
    return;
  }

  let last_expr = &fn_body[fn_body.len() - 1];

  let Some(actual_type) = resolve_type_value(last_expr, scope_types) else {
    return;
  };

  if matches!(actual_type.as_ref(), CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::DynFn) {
    return;
  }

  let mut bindings = HashMap::new();
  if !actual_type
    .as_ref()
    .matches_with_bindings(declared_return_type.as_ref(), &mut bindings)
  {
    let expected_str = declared_return_type.as_ref().to_brief_string();
    let actual_str = actual_type.as_ref().to_brief_string();
    gen_check_warning_code(
      append_js_ffi_type_hint(
        format!("[Warn] Function `{file_ns}/{def_name}` declares return type `{expected_str}`, but body returns `{actual_str}`"),
        &actual_str,
      ),
      "W_FN_RETURN_TYPE_MISMATCH",
      file_ns,
      check_warnings,
    );
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::{CalcitLocal, CalcitStructDef, CalcitSymbolInfo};
  use cirru_edn::EdnTag;

  fn make_local(name: &str, type_info: Arc<CalcitTypeAnnotation>) -> Calcit {
    let sym: Arc<str> = Arc::from(name);
    Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&sym),
      sym: sym.clone(),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.update"),
        at_def: Arc::from("demo"),
      }),
      location: None,
      type_info,
    })
  }

  fn make_core_fn(name: &str, arg_types: Vec<Arc<CalcitTypeAnnotation>>) -> CalcitFn {
    CalcitFn {
      name: Arc::from(name),
      def_ns: Arc::from(calcit::CORE_NS),
      def_ref: None,
      usage: Default::default(),
      scope: Arc::new(Default::default()),
      args: Arc::new(crate::calcit::CalcitFnArgs::Args(vec![])),
      call_shape: crate::calcit::CalcitFnCallShape::fixed(0),
      body: vec![],
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      return_type: crate::calcit::DYNAMIC_TYPE.clone(),
      arg_types,
      rest_type: None,
    }
  }

  #[test]
  fn js_ffi_mismatches_include_boundary_guidance() {
    let message = append_js_ffi_type_hint("type mismatch".to_owned(), "js-nullish<:js-object>");
    assert!(message.contains("stay opaque after JsNullish checks"));
    assert!(message.contains("unsafe-coerce"));
  }

  #[test]
  fn generic_return_contract_still_checks_outer_shape() {
    let body = vec![Calcit::Number(1.0)];
    let expected = Arc::new(CalcitTypeAnnotation::TypeRef(
      Arc::from("calcit.core/Result"),
      Arc::new(vec![
        Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T"))),
        Arc::new(CalcitTypeAnnotation::String),
      ]),
    ));
    let warnings = RefCell::new(vec![]);

    check_function_return_type(&body, &expected, &ScopeTypes::new(), "tests.return", "callback", &warnings);

    let warnings = warnings.borrow();
    assert_eq!(warnings.len(), 1, "a generic payload must not erase the Result wrapper contract");
    assert_eq!(warnings[0].code(), Some("W_FN_RETURN_TYPE_MISMATCH"));
  }

  #[test]
  fn generic_return_contract_accepts_matching_wrapper() {
    let generic = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")));
    let expected = Arc::new(CalcitTypeAnnotation::TypeRef(
      Arc::from("calcit.core/Option"),
      Arc::new(vec![generic]),
    ));
    let actual = Arc::new(CalcitTypeAnnotation::TypeRef(
      Arc::from("calcit.core/Option"),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]),
    ));
    let local = CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from("value")),
      sym: Arc::from("value"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.return"),
        at_def: Arc::from("callback"),
      }),
      location: None,
      type_info: actual,
    };
    let mut scope_types = ScopeTypes::new();
    scope_types.insert(Arc::from("value"), local.type_info.clone());
    let warnings = RefCell::new(vec![]);

    check_function_return_type(
      &[Calcit::Local(local)],
      &expected,
      &scope_types,
      "tests.return",
      "callback",
      &warnings,
    );

    assert!(warnings.borrow().is_empty(), "matching generic wrappers should remain valid");
  }

  #[test]
  fn specialize_update_uses_struct_field_type_for_callback() {
    let task_struct = Arc::new(CalcitStructDef {
      name: EdnTag::new("Task"),
      fields: Arc::new(vec![EdnTag::new("done?")]),
      field_types: Arc::new(vec![tag_annotation("bool")]),
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    });

    let fn_info = CalcitFn {
      name: Arc::from("update"),
      def_ns: Arc::from(calcit::CORE_NS),
      def_ref: None,
      usage: Default::default(),
      scope: Arc::new(Default::default()),
      args: Arc::new(crate::calcit::CalcitFnArgs::Args(vec![])),
      call_shape: crate::calcit::CalcitFnCallShape::fixed(0),
      body: vec![],
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(vec![]),
      return_type: crate::calcit::DYNAMIC_TYPE.clone(),
      arg_types: vec![
        Arc::new(CalcitTypeAnnotation::StructValue(task_struct.clone())),
        crate::calcit::DYNAMIC_TYPE.clone(),
        Arc::new(CalcitTypeAnnotation::from_function_parts(
          vec![Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")))],
          Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T"))),
        )),
      ],
      rest_type: None,
    };

    let args = CalcitList::from(&[
      make_local("task", Arc::new(CalcitTypeAnnotation::StructValue(task_struct))),
      Calcit::Tag(EdnTag::new("done?")),
      make_local(
        "not",
        Arc::new(CalcitTypeAnnotation::from_function_parts(
          vec![tag_annotation("bool")],
          tag_annotation("bool"),
        )),
      ),
    ]);

    let scope_types = ScopeTypes::new();
    let specialized = specialize_core_expected_types(&fn_info, &args, &scope_types, fn_info.arg_types.as_slice())
      .expect("update callback type should specialize from struct field");

    let CalcitTypeAnnotation::Fn(callback) = specialized[2].as_ref() else {
      panic!("specialized callback should be fn type");
    };
    assert!(matches!(callback.arg_types.as_slice(), [arg] if matches!(arg.as_ref(), CalcitTypeAnnotation::Bool)));
    assert!(matches!(callback.return_type.as_ref(), CalcitTypeAnnotation::Bool));
  }

  #[test]
  fn specialize_update_uses_collection_key_and_value_types() {
    let generic = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")));
    let fn_info = CalcitFn {
      name: Arc::from("update"),
      def_ns: Arc::from(calcit::CORE_NS),
      def_ref: None,
      usage: Default::default(),
      scope: Arc::new(Default::default()),
      args: Arc::new(crate::calcit::CalcitFnArgs::Args(vec![])),
      call_shape: crate::calcit::CalcitFnCallShape::fixed(0),
      body: vec![],
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(vec![]),
      return_type: crate::calcit::DYNAMIC_TYPE.clone(),
      arg_types: vec![
        crate::calcit::DYNAMIC_TYPE.clone(),
        crate::calcit::DYNAMIC_TYPE.clone(),
        Arc::new(CalcitTypeAnnotation::from_function_parts(vec![generic.clone()], generic)),
      ],
      rest_type: None,
    };
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let string = Arc::new(CalcitTypeAnnotation::String);
    let callback = make_local(
      "identity",
      Arc::new(CalcitTypeAnnotation::from_function_parts(vec![number.clone()], number.clone())),
    );

    let list_args = CalcitList::from(&[
      make_local("items", Arc::new(CalcitTypeAnnotation::List(string.clone()))),
      Calcit::Number(0.0),
      callback.clone(),
    ]);
    let list_specialized = specialize_core_expected_types(&fn_info, &list_args, &ScopeTypes::new(), &fn_info.arg_types)
      .expect("list update should specialize");
    assert!(matches!(list_specialized[1].as_ref(), CalcitTypeAnnotation::Number));
    let CalcitTypeAnnotation::Fn(list_callback) = list_specialized[2].as_ref() else {
      panic!("list updater should be a function");
    };
    assert!(matches!(list_callback.arg_types.as_slice(), [arg] if arg == &string));
    assert_eq!(list_callback.return_type, string);

    let map_args = CalcitList::from(&[
      make_local("counts", Arc::new(CalcitTypeAnnotation::Map(string.clone(), number.clone()))),
      Calcit::Str(Arc::from("a")),
      callback,
    ]);
    let map_specialized = specialize_core_expected_types(&fn_info, &map_args, &ScopeTypes::new(), &fn_info.arg_types)
      .expect("map update should specialize");
    assert_eq!(map_specialized[1], string);
    let CalcitTypeAnnotation::Fn(map_callback) = map_specialized[2].as_ref() else {
      panic!("map updater should be a function");
    };
    assert!(matches!(map_callback.arg_types.as_slice(), [arg] if arg == &number));
    assert_eq!(map_callback.return_type, number);
  }

  #[test]
  fn specialize_get_uses_collection_key_type() {
    let fn_info = make_core_fn(
      "get",
      vec![crate::calcit::DYNAMIC_TYPE.clone(), crate::calcit::DYNAMIC_TYPE.clone()],
    );
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let string = Arc::new(CalcitTypeAnnotation::String);

    let list_args = CalcitList::from(&[
      make_local("items", Arc::new(CalcitTypeAnnotation::List(string.clone()))),
      Calcit::Tag(EdnTag::new("bad")),
    ]);
    let list_specialized =
      specialize_core_expected_types(&fn_info, &list_args, &ScopeTypes::new(), &fn_info.arg_types).expect("list get should specialize");
    assert_eq!(list_specialized[1], number);

    let map_args = CalcitList::from(&[
      make_local("counts", Arc::new(CalcitTypeAnnotation::Map(string.clone(), number))),
      Calcit::Number(0.0),
    ]);
    let map_specialized =
      specialize_core_expected_types(&fn_info, &map_args, &ScopeTypes::new(), &fn_info.arg_types).expect("map get should specialize");
    assert_eq!(map_specialized[1], string);
  }

  #[test]
  fn specialize_includes_uses_collection_member_type() {
    let fn_info = make_core_fn(
      "includes?",
      vec![crate::calcit::DYNAMIC_TYPE.clone(), crate::calcit::DYNAMIC_TYPE.clone()],
    );
    let number = Arc::new(CalcitTypeAnnotation::Number);

    let set_args = CalcitList::from(&[
      make_local("ids", Arc::new(CalcitTypeAnnotation::Set(number.clone()))),
      Calcit::Tag(EdnTag::new("bad")),
    ]);
    let set_specialized = specialize_core_expected_types(&fn_info, &set_args, &ScopeTypes::new(), &fn_info.arg_types)
      .expect("set membership should specialize");
    assert_eq!(set_specialized[1], number);

    let string_args = CalcitList::from(&[Calcit::Str(Arc::from("abc")), Calcit::Number(1.0)]);
    let string_specialized = specialize_core_expected_types(&fn_info, &string_args, &ScopeTypes::new(), &fn_info.arg_types)
      .expect("string membership should specialize");
    assert!(matches!(string_specialized[1].as_ref(), CalcitTypeAnnotation::String));

    let map_args = CalcitList::from(&[
      make_local(
        "counts",
        Arc::new(CalcitTypeAnnotation::Map(
          Arc::new(CalcitTypeAnnotation::Tag),
          Arc::new(CalcitTypeAnnotation::Number),
        )),
      ),
      Calcit::Tag(EdnTag::new("bad")),
    ]);
    let map_specialized = specialize_core_expected_types(&fn_info, &map_args, &ScopeTypes::new(), &fn_info.arg_types)
      .expect("map value membership should specialize");
    assert_eq!(map_specialized[1], number);
  }
}
