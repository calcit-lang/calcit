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

use crate::calcit::{
  self, Calcit, CalcitFn, CalcitList, CalcitLocal, CalcitProc, CalcitSyntax, CalcitTypeAnnotation, LocatedWarning, NodeLocation,
};

use super::{
  ScopeTypes, check_enum_tuple_construction, check_tuple_nth_bounds, gen_check_warning, gen_check_warning_code,
  gen_check_warning_code_at, resolve_type_value, tag_annotation,
};

// ---------------------------------------------------------------------------
// Unified arg-type checking loop
// ---------------------------------------------------------------------------

struct CheckContext<'a> {
  head_form: &'a Calcit,
  args: &'a CalcitList,
  expected_types: &'a [Arc<CalcitTypeAnnotation>],
  scope_types: &'a ScopeTypes,
  file_ns: &'a str,
  call_location: Option<NodeLocation>,
  warning_code: &'static str,
  check_warnings: &'a RefCell<Vec<LocatedWarning>>,
}

pub(crate) struct CallTypeCheckInfo<'a> {
  pub file_ns: &'a str,
  pub def_name: &'a str,
  pub call_location: Option<NodeLocation>,
}

impl<'a> CheckContext<'a> {
  fn emit_warning(
    &self,
    arg_idx: usize,
    expected_str: &str,
    actual_str: &str,
    make_warning: impl Fn(usize, &str, &str, String) -> String,
  ) {
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
      make_warning(arg_idx, expected_str, actual_str, expr_str),
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
        if let Some(actual_type) = resolve_type_value(rest_arg, ctx.scope_types) {
          if !actual_type.as_ref().matches_with_bindings(inner_type.as_ref(), &mut bindings) {
            let expected_str = inner_type.as_ref().to_brief_string();
            let actual_str = actual_type.as_ref().to_brief_string();
            ctx.emit_warning(idx + rest_idx + 1, &expected_str, &actual_str, &make_warning);
          }
        }
      }
      return; // Done after variadic
    }

    if let Some(actual_type) = resolve_type_value(arg, ctx.scope_types) {
      if !actual_type.as_ref().matches_with_bindings(expected_type.as_ref(), &mut bindings) {
        let expected_str = expected_type.as_ref().to_brief_string();
        let actual_str = actual_type.as_ref().to_brief_string();
        ctx.emit_warning(idx + 1, &expected_str, &actual_str, &make_warning);
      }
    }
  }
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
  let arity = signature.arity();
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
    CalcitProc::NativeRecord
      | CalcitProc::NativeRecordPartial
      | CalcitProc::NativeRecordGet
      | CalcitProc::NativeRecordNth
      | CalcitProc::NativeLooseRecord
  ) {
    return;
  }

  if matches!(proc, CalcitProc::NativeEnumTupleNew) {
    check_enum_tuple_construction(args, scope_types, file_ns, def_name, check_warnings);
    return;
  }

  if matches!(proc, CalcitProc::NativeTupleNth) {
    check_tuple_nth_bounds(args, scope_types, file_ns, def_name, check_warnings);
    return;
  }

  // Use the unified checker for Proc arg types. Proc's arg_types may have
  // Optional wrappers and variadic markers handled inside the loop.
  let mut bindings: HashMap<Arc<str>, Arc<CalcitTypeAnnotation>> = HashMap::new();

  for (idx, (arg, expected_type)) in args.iter().zip(signature.arg_types.iter()).enumerate() {
    if matches!(expected_type.as_ref(), CalcitTypeAnnotation::Variadic(_)) {
      break;
    }

    let base_type = match expected_type.as_ref() {
      CalcitTypeAnnotation::Optional(inner) => inner,
      _ => expected_type,
    };

    if matches!(base_type.as_ref(), CalcitTypeAnnotation::Dynamic) {
      continue;
    }

    if let Some(actual_type) = resolve_type_value(arg, scope_types) {
      if !actual_type.as_ref().matches_with_bindings(base_type.as_ref(), &mut bindings) {
        let expected_str = base_type.as_ref().to_brief_string();
        let actual_str = actual_type.as_ref().to_brief_string();
        let warning_location = arg.get_location().or_else(|| call_location.clone());
        gen_check_warning_code_at(
          format!(
            "[Warn] Proc `{}` arg {} expects type `{expected_str}`, but got `{actual_str}` in call at {file_ns}/{def_name}",
            proc.as_ref(),
            idx + 1
          ),
          "W_PROC_ARG_TYPE_MISMATCH",
          file_ns,
          warning_location,
          check_warnings,
        );
      }
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
    if let Some(actual_type) = resolve_type_value(arg, scope_types) {
      if !actual_type.as_ref().matches_annotation(expected_type.as_ref()) {
        let actual_str = actual_type.as_ref().to_brief_string();
        let warning_location = arg.get_location().or_else(|| call_location.clone());
        gen_check_warning_code_at(
          format!(
            "[Warn] Function `calcit.core/{}` arg {} expects type `:number`, but got `{actual_str}` in call at {file_ns}/{def_name}",
            fn_info.name,
            idx + 1
          ),
          "W_CORE_FN_ARG_TYPE_MISMATCH",
          file_ns,
          warning_location,
          check_warnings,
        );
      }
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
  if fn_info.arg_types.is_empty() || fn_info.arg_types.iter().all(|t| matches!(**t, CalcitTypeAnnotation::Dynamic)) {
    return;
  }

  let fn_def_ns = fn_info.def_ns.clone();
  let fn_name = fn_info.name.clone();
  let def_name = call_info.def_name.to_owned();
  let file_ns_owned = call_info.file_ns.to_owned();
  let ctx = CheckContext {
    head_form,
    args,
    expected_types: &fn_info.arg_types,
    scope_types,
    file_ns: call_info.file_ns,
    call_location: call_info.call_location.clone(),
    warning_code: "W_FN_ARG_TYPE_MISMATCH",
    check_warnings,
  };
  check_arg_types_loop(ctx, |arg_idx, expected_str, actual_str, expr_str| {
    format!(
      "[Warn] Function `{fn_def_ns}/{fn_name}` arg {arg_idx} expects type `{expected_str}`, but got `{actual_str}` in call at {file_ns_owned}/{def_name}\n  Expression: {expr_str}"
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

  if declared_return_type.contains_type_var() {
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

  if !actual_type.as_ref().matches_annotation(declared_return_type.as_ref()) {
    let expected_str = declared_return_type.as_ref().to_brief_string();
    let actual_str = actual_type.as_ref().to_brief_string();
    gen_check_warning_code(
      format!("[Warn] Function `{file_ns}/{def_name}` declares return type `{expected_str}`, but body returns `{actual_str}`"),
      "W_FN_RETURN_TYPE_MISMATCH",
      file_ns,
      check_warnings,
    );
  }
}
