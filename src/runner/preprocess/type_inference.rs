//! Type inference / synthesis module.
//!
//! Pure bottom-up type inference: given an expression, synthesize its type
//! without any "expected type" context flowing in. This corresponds to the
//! **synthesis** direction in bidirectional type checking.
//!
//! Key entry points:
//! - `resolve_type_value` — resolve the type of an already-preprocessed expression
//! - `infer_type_from_expr` — synthesize a type for an arbitrary expression
//! - `resolve_enum_value` / `resolve_record_value` — resolve data definitions

use std::sync::Arc;
use std::{
  cell::RefCell,
  collections::{HashMap, HashSet},
};

use crate::{
  calcit::{
    self, Calcit, CalcitEnum, CalcitFnTypeAnnotation, CalcitImport, CalcitList, CalcitProc, CalcitRecord, CalcitStruct, CalcitSyntax,
    CalcitTypeAnnotation, SchemaKind, resolve_type_slot,
  },
  call_stack::CallStackList,
  program, runner,
};

use super::{ScopeTypes, tag_annotation};

// ---------------------------------------------------------------------------
// Core resolution
// ---------------------------------------------------------------------------

pub(crate) fn resolve_type_value(target: &Calcit, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  match target {
    Calcit::Local(local) => {
      // First check if the local has inline type_info, then fall back to scope_types
      if matches!(*local.type_info, CalcitTypeAnnotation::Dynamic) {
        let scoped = scope_types.get(&local.sym).cloned();
        scoped.map(normalize_variadic_as_list)
      } else {
        Some(normalize_variadic_as_list(local.type_info.clone()))
      }
    }
    Calcit::Symbol { sym, .. } => scope_types
      .get(sym)
      .cloned()
      .map(normalize_variadic_as_list)
      .or_else(|| infer_type_from_expr(target, scope_types)),
    _ => infer_type_from_expr(target, scope_types),
  }
}

/// Treat variadic locals as list values when resolving expression types.
///
/// This is distinct from `collect_arg_type_hints_from_body`: that function extracts parameter
/// annotations, while this function only normalizes the inferred type for internal list operations
/// like `&list:count` and `&list:first`.
fn normalize_variadic_as_list(value: Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  match value.as_ref() {
    CalcitTypeAnnotation::Variadic(inner) => Arc::new(CalcitTypeAnnotation::List(inner.clone())),
    _ => value,
  }
}

// ---------------------------------------------------------------------------
// If-branch type merging
// ---------------------------------------------------------------------------

fn merge_if_branch_types(
  true_type: Arc<CalcitTypeAnnotation>,
  false_type: Arc<CalcitTypeAnnotation>,
) -> Option<Arc<CalcitTypeAnnotation>> {
  if true_type.as_ref().matches_annotation(false_type.as_ref()) {
    Some(true_type)
  } else if false_type.as_ref().matches_annotation(true_type.as_ref()) {
    Some(false_type)
  } else {
    None
  }
}

pub(crate) fn infer_if_return_type(xs: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  if xs.len() < 3 {
    return None;
  }

  let true_expr = xs.get(2)?;
  let true_type = resolve_type_value(true_expr, scope_types)?;

  if let Some(false_expr) = xs.get(3) {
    let false_type = resolve_type_value(false_expr, scope_types)?;
    merge_if_branch_types(true_type, false_type)
  } else {
    Some(Arc::new(CalcitTypeAnnotation::Optional(true_type)))
  }
}

// ---------------------------------------------------------------------------
// Generic return type resolution
// ---------------------------------------------------------------------------

/// Given a function's type info and the actual call arguments, resolve a generic return type.
/// If the return type contains TypeVars, match actual arg types against declared arg types
/// to build bindings, then substitute TypeVars in the return type.
/// Returns `None` if the return type has no TypeVars or resolution fails.
pub(crate) fn resolve_generic_return_type<'a>(
  fn_info: &crate::calcit::CalcitFn,
  call_args: impl Iterator<Item = &'a Calcit>,
  scope_types: &ScopeTypes,
) -> Option<Arc<CalcitTypeAnnotation>> {
  // Only attempt resolution when there are generics and the return type contains TypeVars
  if fn_info.generics.is_empty() || !fn_info.return_type.contains_type_var() {
    return None;
  }

  let mut bindings: HashMap<Arc<str>, Arc<CalcitTypeAnnotation>> = HashMap::new();

  // Match each actual argument against the declared argument type to build bindings
  for (arg, expected_type) in call_args.zip(fn_info.arg_types.iter()) {
    if matches!(**expected_type, CalcitTypeAnnotation::Dynamic) {
      continue;
    }
    if let Some(actual_type) = resolve_type_value(arg, scope_types) {
      // Use matches_with_bindings to populate TypeVar bindings
      actual_type.as_ref().matches_with_bindings(expected_type.as_ref(), &mut bindings);
    }
  }

  if bindings.is_empty() {
    return None;
  }

  let resolved = fn_info.return_type.substitute_type_vars(&bindings);
  // Only return if we actually resolved something (no remaining TypeVars)
  if resolved.contains_type_var() { None } else { Some(resolved) }
}

pub(crate) fn infer_return_type_from_compiled_callable(
  ns: &str,
  def: &str,
  call_expr: &CalcitList,
  scope_types: &ScopeTypes,
) -> Option<Arc<CalcitTypeAnnotation>> {
  if ns == calcit::CORE_NS
    && def == "get"
    && let Some(inferred) = infer_core_get_return_type(call_expr, scope_types)
  {
    return Some(inferred);
  }
  if ns == calcit::CORE_NS
    && def == "get-in"
    && let Some(inferred) = infer_core_get_in_return_type(call_expr, scope_types)
  {
    return Some(inferred);
  }

  // Prefer compiled callable metadata. The four core enum constructors are
  // also needed while the core is bootstrapping, before their payloads are
  // compiled; their declared schemas unlock receiver-first method calls.
  if let Some(compiled) = program::lookup_compiled_def(ns, def) {
    // Avoid evaluating compiled payloads during preprocess type inference.
    // Evaluating function code here can recurse back into preprocess and overflow stack.
    match compiled.preprocessed_code {
      Calcit::Fn { info, .. } => {
        if let Some(resolved) = resolve_generic_return_type(&info, call_expr.iter().skip(1), scope_types) {
          return Some(resolved);
        }
        return Some(info.return_type.clone());
      }
      Calcit::Proc(proc) => return proc.get_type_signature().map(|type_sig| type_sig.return_type.clone()),
      _ => {}
    }
  }

  if ns != calcit::CORE_NS || !matches!(def, "%some" | "%none" | "%ok" | "%err") {
    return None;
  }

  let schema = program::lookup_def_schema(ns, def);
  let CalcitTypeAnnotation::Fn(info) = schema.as_ref() else {
    return None;
  };
  if info.generics.is_empty() || !info.return_type.contains_type_var() {
    return Some(info.return_type.clone());
  }

  let mut bindings: HashMap<Arc<str>, Arc<CalcitTypeAnnotation>> = HashMap::new();
  for (arg, expected_type) in call_expr.iter().skip(1).zip(info.arg_types.iter()) {
    if !matches!(expected_type.as_ref(), CalcitTypeAnnotation::Dynamic)
      && let Some(actual_type) = resolve_type_value(arg, scope_types)
    {
      actual_type.as_ref().matches_with_bindings(expected_type.as_ref(), &mut bindings);
    }
  }
  // A constructor such as `%none` has no payload from which to infer one or
  // more generic arguments. Preserve its nominal enum identity with Dynamic
  // type arguments, but do not synthesize partial types for ordinary functions.
  for generic in info.generics.iter() {
    bindings.entry(generic.clone()).or_insert_with(|| calcit::DYNAMIC_TYPE.clone());
  }
  let resolved = info.return_type.substitute_type_vars(&bindings);
  if resolved.contains_type_var() {
    return None;
  }
  (resolved.resolve_to_struct().is_some() || resolved.resolve_to_enum().is_some()).then_some(resolved)
}

fn infer_core_get_return_type(call_expr: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  let base_arg = call_expr.get(1)?;
  let base_type = resolve_type_value(base_arg, scope_types)?;
  let key_arg = call_expr.get(2);
  infer_get_return_type_from_type(base_type.as_ref(), key_arg)
}

fn infer_core_get_in_return_type(call_expr: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  let base_arg = call_expr.get(1)?;
  let path_arg = call_expr.get(2)?;
  let path_items = extract_literal_list_items(path_arg)?;
  let mut current_type = resolve_type_value(base_arg, scope_types)?;

  if path_items.is_empty() {
    return Some(current_type);
  }

  for key in path_items {
    current_type = infer_get_return_type_from_type(current_type.as_ref(), Some(key))?;
  }

  Some(current_type)
}

fn infer_get_return_type_from_type(base_type: &CalcitTypeAnnotation, key_arg: Option<&Calcit>) -> Option<Arc<CalcitTypeAnnotation>> {
  match base_type {
    CalcitTypeAnnotation::Optional(inner) => infer_get_return_type_from_type(inner.as_ref(), key_arg),
    CalcitTypeAnnotation::List(element_type) => Some(wrap_optional_type(element_type.clone())),
    CalcitTypeAnnotation::Map(_, value_type) => Some(wrap_optional_type(value_type.clone())),
    CalcitTypeAnnotation::String => Some(wrap_optional_type(tag_annotation("string"))),
    CalcitTypeAnnotation::Record(_) | CalcitTypeAnnotation::Struct(_, _) | CalcitTypeAnnotation::TypeRef(_, _) => {
      if let Some(field_name) = key_arg.and_then(extract_field_name)
        && let Some(field_type) = resolve_struct_field_type(base_type, field_name)
      {
        return Some(wrap_optional_type(field_type));
      }
      Some(wrap_optional_type(calcit::DYNAMIC_TYPE.clone()))
    }
    _ => None,
  }
}

fn wrap_optional_type(inner: Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  match inner.as_ref() {
    CalcitTypeAnnotation::Optional(_) => inner,
    _ => Arc::new(CalcitTypeAnnotation::Optional(inner)),
  }
}

fn extract_literal_list_items(form: &Calcit) -> Option<Vec<&Calcit>> {
  let Calcit::List(items) = form else {
    return None;
  };

  let head = items.first()?;
  let is_list_literal = matches!(head, Calcit::Proc(CalcitProc::List))
    || matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "[]")
    || matches!(head, Calcit::Import(CalcitImport { ns, def, .. }) if &**ns == calcit::CORE_NS && &**def == "[]");

  if !is_list_literal {
    return None;
  }

  Some(items.iter().skip(1).collect())
}

// ---------------------------------------------------------------------------
// Main synthesis: infer_type_from_expr
// ---------------------------------------------------------------------------

/// Infer type from an expression (for &let bindings)
/// Supports:
/// - Literals (number, string, bool, nil)
/// - Proc calls with known return types
/// - Function calls with return-type annotations
/// - Nested &let expressions (returns type of final expression)
/// - Local variables (reads from type_info field)
pub(crate) fn infer_type_from_expr(expr: &Calcit, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  match expr {
    // Literal types
    Calcit::Number(_) => Some(tag_annotation("number")),
    Calcit::Str(_) => Some(tag_annotation("string")),
    Calcit::Bool(_) => Some(tag_annotation("bool")),
    Calcit::Nil => Some(tag_annotation("nil")),
    Calcit::Tag(_) => Some(tag_annotation("tag")),
    Calcit::Map(values) => Some(Arc::new(CalcitTypeAnnotation::Map(
      infer_homogeneous_type(values.iter().map(|(key, _)| key), scope_types),
      infer_homogeneous_type(values.iter().map(|(_, value)| value), scope_types),
    ))),
    Calcit::Set(values) => Some(Arc::new(CalcitTypeAnnotation::Set(infer_homogeneous_type(
      values.iter(),
      scope_types,
    )))),
    Calcit::Tuple(tuple) => match &tuple.sum_type {
      Some(enum_def) => Some(Arc::new(CalcitTypeAnnotation::Tuple(enum_def.clone()))),
      None => Some(Arc::new(CalcitTypeAnnotation::DynTuple)),
    },
    Calcit::Record(record) => {
      if record.struct_ref.generics.is_empty() {
        Some(Arc::new(CalcitTypeAnnotation::Record(record.struct_ref.clone())))
      } else {
        let applied_args = infer_struct_applied_args(record.struct_ref.as_ref(), record.values.iter(), scope_types);
        Some(Arc::new(CalcitTypeAnnotation::Struct(
          record.struct_ref.clone(),
          Arc::new(applied_args),
        )))
      }
    }
    Calcit::Struct(struct_def) => Some(Arc::new(CalcitTypeAnnotation::Struct(
      Arc::new(struct_def.to_owned()),
      Arc::new(vec![]),
    ))),
    Calcit::Enum(enum_def) => Some(Arc::new(CalcitTypeAnnotation::Enum(
      Arc::new(enum_def.to_owned()),
      Arc::new(vec![]),
    ))),
    Calcit::Ref(..) => Some(Arc::new(CalcitTypeAnnotation::Ref(calcit::DYNAMIC_TYPE.clone()))),
    Calcit::Buffer(_) => Some(Arc::new(CalcitTypeAnnotation::Buffer)),
    Calcit::CirruQuote(_) => Some(Arc::new(CalcitTypeAnnotation::CirruQuote)),
    Calcit::Fn { info, .. } => Some(Arc::new(CalcitTypeAnnotation::from_calcit_fn(info))),
    Calcit::Proc(proc) => proc
      .get_type_signature()
      .map(|signature| {
        Arc::new(CalcitTypeAnnotation::from_function_parts(
          signature.arg_types.clone(),
          signature.return_type.clone(),
        ))
      })
      .or_else(|| Some(tag_annotation("fn"))),

    Calcit::Import(CalcitImport { ns, def, .. }) => infer_definition_value_type(ns, def),
    Calcit::Symbol { sym, info, .. } => infer_definition_value_type(&info.at_ns, sym),

    // Local variable: read type_info
    Calcit::Local(local) => Some(local.type_info.clone()),

    // List/vector literal or expressions
    Calcit::List(xs) if xs.is_empty() => Some(tag_annotation("list")),

    // Function call or Proc call or special forms
    Calcit::List(xs) => {
      let head = xs.first()?;
      match head {
        // &let expression: infer from final expression (last element)
        Calcit::Syntax(CalcitSyntax::CoreLet, _) => {
          // &let has format: (&let (binding) body...)
          // The last element is the return value
          if xs.len() > 1 {
            infer_type_from_expr(&xs[xs.len() - 1], scope_types)
          } else {
            None
          }
        }
        Calcit::Syntax(CalcitSyntax::If, _) => infer_if_return_type(xs, scope_types),

        // A preprocessed function remains a syntax list until runtime construction. Preserve an
        // explicit body `hint-fn` as its static value type; without a schema we only know that the
        // value is callable and deliberately keep its argument/return details dynamic.
        Calcit::Syntax(CalcitSyntax::Defn | CalcitSyntax::Defmacro, _) => Some(infer_preprocessed_function_type(xs)),

        Calcit::Syntax(CalcitSyntax::UnsafeCoerce, _) => xs.get(2).map(CalcitTypeAnnotation::parse_type_annotation_form),

        // Local variable as head (function call)
        // If it's a function type, return its return type
        Calcit::Local(local) => {
          let type_ann = &local.type_info;
          match type_ann.as_ref() {
            CalcitTypeAnnotation::Fn(fn_type) => Some(fn_type.return_type.clone()),
            CalcitTypeAnnotation::DynFn => Some(calcit::DYNAMIC_TYPE.clone()),
            _ => Some(type_ann.clone()),
          }
        }

        // Proc call: check if proc has return_type
        Calcit::Proc(proc) => infer_proc_call_return_type(proc, xs, scope_types),

        // Import: could be a function, try to get its return type
        Calcit::Import(CalcitImport { ns, def, .. }) => {
          if &**ns == calcit::CORE_NS
            && (&**def == "record-get" || &**def == "&record:get")
            && let Some(field_type) = infer_record_get_type(xs, scope_types)
          {
            return Some(field_type);
          }
          infer_return_type_from_compiled_callable(ns, def, xs, scope_types)
        }

        // Symbol: might be a function reference before preprocessing
        // Try to resolve it and get the return type
        Calcit::Symbol { sym, info, .. } => {
          if let Some(inferred) = infer_return_type_from_compiled_callable(&info.at_ns, sym, xs, scope_types) {
            return Some(inferred);
          }

          if let Some(code) = program::lookup_def_code(&info.at_ns, sym)
            && let Calcit::List(xs) = code
            && let Some(Calcit::Symbol { sym, .. }) = xs.first()
            && sym.as_ref() == "defn"
            && let Some(ret_type) = xs.get(3)
            && matches!(ret_type, Calcit::Tag(_))
          {
            return Some(CalcitTypeAnnotation::parse_type_annotation_form(ret_type));
          }
          None
        }

        // Direct Fn call: return the function's return type
        Calcit::Fn { info, .. } => {
          if info.return_type.contains_type_var()
            && let Some(resolved) = resolve_generic_return_type(info, xs.iter().skip(1), scope_types)
          {
            return Some(resolved);
          }
          Some(info.return_type.clone())
        }

        // Method access: infer record field type when available
        Calcit::Method(field_name, calcit::MethodKind::Access | calcit::MethodKind::TagAccess) => {
          if let Some(receiver) = xs.get(1)
            && let Some(field_type) = infer_record_field_type(receiver, field_name.as_ref(), scope_types)
          {
            return Some(field_type);
          }
          None
        }

        // Nested List call: the head is a function call expression
        // First infer what type the head returns, then if it's a function, get its return type
        Calcit::List(_) => {
          if let Some(head_type) = infer_type_from_expr(head, scope_types) {
            match head_type.as_ref() {
              CalcitTypeAnnotation::Fn(fn_type) => Some(fn_type.return_type.clone()),
              CalcitTypeAnnotation::DynFn => Some(calcit::DYNAMIC_TYPE.clone()),
              // If head returns a non-function type, the call will fail at runtime
              // Return the non-callable type so caller can detect this issue
              _ => Some(head_type),
            }
          } else {
            None
          }
        }

        _ => None,
      }
    }

    _ => None,
  }
}

fn infer_preprocessed_function_type(xs: &CalcitList) -> Arc<CalcitTypeAnnotation> {
  let hinted = xs
    .iter()
    .skip(3)
    .find_map(CalcitTypeAnnotation::extract_fn_annotation_from_hint_form);
  let Some(hinted) = hinted else {
    return Arc::new(CalcitTypeAnnotation::DynFn);
  };
  let CalcitTypeAnnotation::Fn(fn_annotation) = hinted.as_ref() else {
    return hinted;
  };

  let (parameter_types, inferred_rest_type) = infer_preprocessed_function_parameters(xs.get(2));
  let mut arg_types = fn_annotation.arg_types.clone();
  for parameter_type in parameter_types.iter().skip(arg_types.len()) {
    arg_types.push(parameter_type.clone());
  }

  let fn_kind = match xs.first() {
    Some(Calcit::Syntax(CalcitSyntax::Defmacro, _)) => SchemaKind::Macro,
    _ => fn_annotation.fn_kind,
  };

  Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
    generics: fn_annotation.generics.clone(),
    where_bounds: fn_annotation.where_bounds.clone(),
    arg_types,
    return_type: fn_annotation.return_type.clone(),
    fn_kind,
    rest_type: fn_annotation.rest_type.clone().or(inferred_rest_type),
    features: fn_annotation.features.clone(),
  })))
}

fn infer_preprocessed_function_parameters(
  params: Option<&Calcit>,
) -> (Vec<Arc<CalcitTypeAnnotation>>, Option<Arc<CalcitTypeAnnotation>>) {
  let Some(Calcit::List(params)) = params else {
    return (vec![], None);
  };

  let mut fixed = vec![];
  let mut expects_rest_binding = false;
  let mut rest_type = None;
  for param in params.iter() {
    match param {
      Calcit::Syntax(CalcitSyntax::ArgSpread, _) => expects_rest_binding = true,
      Calcit::Local(local) if expects_rest_binding => {
        rest_type = Some(match local.type_info.as_ref() {
          CalcitTypeAnnotation::Variadic(inner) => inner.clone(),
          CalcitTypeAnnotation::Dynamic => calcit::DYNAMIC_TYPE.clone(),
          other => Arc::new(other.to_owned()),
        });
        expects_rest_binding = false;
      }
      Calcit::Local(local) => fixed.push(local.type_info.clone()),
      _ => {}
    }
  }
  if expects_rest_binding && rest_type.is_none() {
    rest_type = Some(calcit::DYNAMIC_TYPE.clone());
  }
  (fixed, rest_type)
}

/// Infer the return type of a built-in proc call expression.
///
/// Extracted from the large `Calcit::Proc` arm of `infer_type_from_expr` for clarity.
fn infer_proc_call_return_type(proc: &CalcitProc, xs: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  if matches!(proc, CalcitProc::List) {
    return Some(Arc::new(CalcitTypeAnnotation::List(infer_homogeneous_type(
      xs.iter().skip(1),
      scope_types,
    ))));
  }
  if matches!(proc, CalcitProc::Set) {
    return Some(Arc::new(CalcitTypeAnnotation::Set(infer_homogeneous_type(
      xs.iter().skip(1),
      scope_types,
    ))));
  }
  if matches!(proc, CalcitProc::NativeMap) {
    let args = xs.iter().skip(1).collect::<Vec<_>>();
    if args.len() % 2 != 0 {
      return Some(Arc::new(CalcitTypeAnnotation::Map(
        calcit::DYNAMIC_TYPE.clone(),
        calcit::DYNAMIC_TYPE.clone(),
      )));
    }
    return Some(Arc::new(CalcitTypeAnnotation::Map(
      infer_homogeneous_type(args.iter().step_by(2).copied(), scope_types),
      infer_homogeneous_type(args.iter().skip(1).step_by(2).copied(), scope_types),
    )));
  }
  if matches!(proc, CalcitProc::Atom)
    && let Some(initial_value) = xs.get(1)
    && let Some(initial_type) = resolve_type_value(initial_value, scope_types)
  {
    return Some(Arc::new(CalcitTypeAnnotation::Ref(initial_type)));
  }
  if matches!(proc, CalcitProc::NativeListNth | CalcitProc::NativeListFirst)
    && let Some(first_arg) = xs.get(1)
    && let Some(type_value) = resolve_type_value(first_arg, scope_types)
    && let CalcitTypeAnnotation::List(element_type) = type_value.as_ref()
  {
    return Some(element_type.clone());
  }
  if matches!(
    proc,
    CalcitProc::NativeListRest
      | CalcitProc::NativeListSlice
      | CalcitProc::NativeListReverse
      | CalcitProc::NativeListDistinct
      | CalcitProc::NativeListConcat
      | CalcitProc::Append
      | CalcitProc::Prepend
      | CalcitProc::Butlast
      | CalcitProc::Sort
      | CalcitProc::NativeListAssoc
      | CalcitProc::NativeListAssocBefore
      | CalcitProc::NativeListAssocAfter
      | CalcitProc::NativeListDissoc
  ) && let Some(first_arg) = xs.get(1)
    && let Some(type_value) = resolve_type_value(first_arg, scope_types)
    && let CalcitTypeAnnotation::List(_) = type_value.as_ref()
  {
    return Some(type_value.clone());
  }
  // Range always returns List(Number)
  if matches!(proc, CalcitProc::Range) {
    return Some(Arc::new(CalcitTypeAnnotation::List(tag_annotation("number"))));
  }
  // Split/SplitLines always return List(String)
  if matches!(proc, CalcitProc::Split | CalcitProc::SplitLines) {
    return Some(Arc::new(CalcitTypeAnnotation::List(tag_annotation("string"))));
  }
  if matches!(proc, CalcitProc::NativeMapGet)
    && let Some(first_arg) = xs.get(1)
    && let Some(type_value) = resolve_type_value(first_arg, scope_types)
    && let CalcitTypeAnnotation::Map(_key_type, val_type) = type_value.as_ref()
  {
    return Some(val_type.clone());
  }
  if matches!(
    proc,
    CalcitProc::NativeMapAssoc
      | CalcitProc::NativeMapDissoc
      | CalcitProc::NativeMerge
      | CalcitProc::NativeMergeNonNil
      | CalcitProc::NativeMapDiffNew
  ) && let Some(first_arg) = xs.get(1)
    && let Some(type_value) = resolve_type_value(first_arg, scope_types)
    && let CalcitTypeAnnotation::Map(_, _) = type_value.as_ref()
  {
    return Some(type_value.clone());
  }
  // MapToList converts Map(K, V) → List(Dynamic)
  if matches!(proc, CalcitProc::NativeMapToList) {
    return Some(tag_annotation("list"));
  }
  if matches!(proc, CalcitProc::NativeSetToList)
    && let Some(first_arg) = xs.get(1)
    && let Some(type_value) = resolve_type_value(first_arg, scope_types)
    && let CalcitTypeAnnotation::Set(element_type) = type_value.as_ref()
  {
    return Some(Arc::new(CalcitTypeAnnotation::List(element_type.clone())));
  }
  if matches!(
    proc,
    CalcitProc::NativeInclude
      | CalcitProc::NativeExclude
      | CalcitProc::NativeDifference
      | CalcitProc::NativeUnion
      | CalcitProc::NativeSetIntersection
  ) && let Some(first_arg) = xs.get(1)
    && let Some(type_value) = resolve_type_value(first_arg, scope_types)
    && let CalcitTypeAnnotation::Set(_) = type_value.as_ref()
  {
    return Some(type_value.clone());
  }
  if matches!(proc, CalcitProc::AtomDeref)
    && let Some(first_arg) = xs.get(1)
    && let Some(type_value) = resolve_type_value(first_arg, scope_types)
    && let CalcitTypeAnnotation::Ref(element_type) = type_value.as_ref()
  {
    return Some(element_type.clone());
  }
  if matches!(proc, CalcitProc::NativeListToSet)
    && let Some(first_arg) = xs.get(1)
    && let Some(type_value) = resolve_type_value(first_arg, scope_types)
    && let CalcitTypeAnnotation::List(element_type) = type_value.as_ref()
  {
    return Some(Arc::new(CalcitTypeAnnotation::Set(element_type.clone())));
  }
  if matches!(proc, CalcitProc::NativeEnumTupleNew)
    && let Some(tuple_type) = infer_enum_tuple_annotation(proc, xs, scope_types)
  {
    return Some(tuple_type);
  }
  if matches!(proc, CalcitProc::NativeStructNew)
    && let Some(struct_type) = infer_struct_literal_type(xs)
  {
    return Some(struct_type);
  }
  if matches!(proc, CalcitProc::NativeRecord | CalcitProc::NativeRecordPartial)
    && let Some(record_type) = infer_record_literal_type(xs, scope_types)
  {
    return Some(record_type);
  }
  if matches!(proc, CalcitProc::NativeLooseRecord) {
    return Some(tag_annotation("record"));
  }
  if matches!(proc, CalcitProc::NativeRecordGet)
    && let Some(field_type) = infer_record_get_type(xs, scope_types)
  {
    return Some(field_type);
  }
  if matches!(proc, CalcitProc::NativeRecordNth)
    && let Some(field_type) = infer_record_nth_type(xs, scope_types)
  {
    return Some(field_type);
  }
  proc.get_type_signature().map(|type_sig| type_sig.return_type.clone())
}

fn infer_homogeneous_type<'a>(values: impl Iterator<Item = &'a Calcit>, scope_types: &ScopeTypes) -> Arc<CalcitTypeAnnotation> {
  let mut inferred: Option<Arc<CalcitTypeAnnotation>> = None;
  for value in values {
    let Some(next) = resolve_type_value(value, scope_types) else {
      return calcit::DYNAMIC_TYPE.clone();
    };
    if matches!(next.as_ref(), CalcitTypeAnnotation::Dynamic) {
      return calcit::DYNAMIC_TYPE.clone();
    }
    match &inferred {
      Some(current) if !current.as_ref().matches_annotation(next.as_ref()) || !next.as_ref().matches_annotation(current.as_ref()) => {
        return calcit::DYNAMIC_TYPE.clone();
      }
      Some(_) => {}
      None => inferred = Some(next),
    }
  }
  inferred.unwrap_or_else(|| calcit::DYNAMIC_TYPE.clone())
}

thread_local! {
  static INFERRED_DEFINITIONS: RefCell<HashSet<(String, String)>> = RefCell::new(HashSet::new());
}

fn infer_definition_value_type(ns: &str, def: &str) -> Option<Arc<CalcitTypeAnnotation>> {
  let key = (ns.to_owned(), def.to_owned());
  let entered = INFERRED_DEFINITIONS.with(|definitions| definitions.borrow_mut().insert(key.clone()));
  if !entered {
    return Some(calcit::DYNAMIC_TYPE.clone());
  }

  let inferred = infer_definition_value_type_inner(ns, def);
  INFERRED_DEFINITIONS.with(|definitions| {
    definitions.borrow_mut().remove(&key);
  });
  inferred
}

fn infer_definition_value_type_inner(ns: &str, def: &str) -> Option<Arc<CalcitTypeAnnotation>> {
  let schema = program::lookup_def_schema(ns, def);
  if !matches!(schema.as_ref(), CalcitTypeAnnotation::Dynamic) {
    return Some(schema);
  }

  // Data definitions often keep `:dynamic` as their value schema because their concrete field
  // shape lives in the source form. Preserve a named TypeRef here instead of mistaking the
  // synthetic record prototype used during preprocessing for a runtime record instance.
  let named_type = Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from(format!("{ns}/{def}")), Arc::new(vec![])));
  if named_type.resolve_to_struct().is_some() || named_type.resolve_to_enum().is_some() {
    return Some(named_type);
  }

  let compiled = program::lookup_compiled_def(ns, def)?;
  match compiled.preprocessed_code {
    Calcit::Fn { info, .. } => Some(Arc::new(CalcitTypeAnnotation::from_calcit_fn(&info))),
    Calcit::Proc(proc) => proc.get_type_signature().map(|signature| {
      Arc::new(CalcitTypeAnnotation::from_function_parts(
        signature.arg_types.clone(),
        signature.return_type.clone(),
      ))
    }),
    value => infer_type_from_expr(&value, &ScopeTypes::new()),
  }
}

/// Infer an expression type from already-preprocessed code without executing it.
/// Local nodes retain their lexical type information, while imports resolve from compiled metadata.
pub fn infer_static_type_from_expr(expr: &Calcit) -> Option<Arc<CalcitTypeAnnotation>> {
  infer_type_from_expr(expr, &ScopeTypes::new())
}

// ---------------------------------------------------------------------------
// Specialised inference helpers
// ---------------------------------------------------------------------------

fn infer_enum_tuple_annotation(proc: &CalcitProc, xs: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  let (enum_proto, tag_arg) = match proc {
    CalcitProc::NativeEnumTupleNew => {
      if xs.len() < 3 {
        return None;
      }
      let enum_arg = xs.get(1)?;
      let tag_arg = xs.get(2);
      let enum_proto = resolve_enum_value(enum_arg, scope_types)?;
      (enum_proto, tag_arg)
    }
    _ => return None,
  };

  if enum_proto.generics().is_empty() {
    return Some(Arc::new(CalcitTypeAnnotation::Tuple(Arc::new(enum_proto))));
  }

  let applied_args = infer_enum_tuple_applied_args(&enum_proto, tag_arg, xs.iter().skip(3), scope_types).unwrap_or_else(|| {
    enum_proto
      .generics()
      .iter()
      .map(|_| calcit::DYNAMIC_TYPE.clone())
      .collect::<Vec<_>>()
  });

  Some(Arc::new(CalcitTypeAnnotation::Enum(Arc::new(enum_proto), Arc::new(applied_args))))
}

fn infer_enum_tuple_applied_args<'a>(
  enum_proto: &CalcitEnum,
  tag_arg: Option<&Calcit>,
  payload_args: impl Iterator<Item = &'a Calcit>,
  scope_types: &ScopeTypes,
) -> Option<Vec<Arc<CalcitTypeAnnotation>>> {
  if enum_proto.generics().is_empty() {
    return Some(vec![]);
  }

  let tag_name = match tag_arg? {
    Calcit::Tag(tag) => tag.ref_str(),
    _ => return None,
  };
  let variant = enum_proto.find_variant_by_name(tag_name)?;
  let mut bindings: HashMap<Arc<str>, Arc<CalcitTypeAnnotation>> = HashMap::new();

  for (payload, expected_type) in payload_args.zip(variant.payload_types().iter()) {
    let actual_type = resolve_type_value(payload, scope_types).unwrap_or_else(|| calcit::DYNAMIC_TYPE.clone());
    actual_type.as_ref().matches_with_bindings(expected_type.as_ref(), &mut bindings);
  }

  Some(
    enum_proto
      .generics()
      .iter()
      .map(|name| bindings.get(name).cloned().unwrap_or_else(|| calcit::DYNAMIC_TYPE.clone()))
      .collect(),
  )
}

fn infer_record_get_type(xs: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  if xs.len() < 3 {
    return None;
  }
  let record_arg = xs.get(1)?;
  let field_arg = xs.get(2)?;
  let field_name = extract_field_name(field_arg)?;
  infer_record_field_type(record_arg, field_name, scope_types)
}

/// Infer the return type of `&record:nth record idx` by looking up the field type at the given index.
fn infer_record_nth_type(xs: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  if xs.len() < 3 {
    return None;
  }
  let record_arg = xs.get(1)?;
  let idx_arg = xs.get(2)?;
  let idx = match idx_arg {
    Calcit::Number(n) => *n as usize,
    _ => return None,
  };
  let type_info = resolve_type_value(record_arg, scope_types)?;
  resolve_struct_field_type_by_index(type_info.as_ref(), idx)
}

fn infer_record_literal_type(xs: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  if xs.len() < 2 {
    return None;
  }
  let proto_arg = xs.get(1)?;
  let record = resolve_record_value(proto_arg, scope_types)?;
  if record.struct_ref.generics.is_empty() {
    return Some(Arc::new(CalcitTypeAnnotation::Record(record.struct_ref.clone())));
  }

  let field_values = collect_record_literal_values(xs, &record)?;
  let applied_args = infer_struct_applied_args(record.struct_ref.as_ref(), field_values.iter(), scope_types);
  Some(Arc::new(CalcitTypeAnnotation::Struct(
    record.struct_ref.clone(),
    Arc::new(applied_args),
  )))
}

fn infer_struct_literal_type(xs: &CalcitList) -> Option<Arc<CalcitTypeAnnotation>> {
  if xs.len() < 2 {
    return None;
  }

  let name = parse_struct_name(xs.get(1)?)?;
  let mut fields: Vec<(cirru_edn::EdnTag, Arc<CalcitTypeAnnotation>)> = Vec::new();

  for item in xs.iter().skip(2) {
    let (field_name, field_type) = parse_struct_field_entry(item)?;
    fields.push((field_name, field_type));
  }

  fields.sort_by(|a, b| a.0.ref_str().cmp(b.0.ref_str()));
  for idx in 1..fields.len() {
    if fields[idx - 1].0 == fields[idx].0 {
      return None;
    }
  }

  let field_names: Vec<cirru_edn::EdnTag> = fields.iter().map(|(name, _)| name.to_owned()).collect();
  let field_types: Vec<Arc<CalcitTypeAnnotation>> = fields.iter().map(|(_, t)| t.to_owned()).collect();

  let struct_def = CalcitStruct {
    name,
    fields: Arc::new(field_names.clone()),
    field_types: Arc::new(field_types),
    generics: Arc::new(vec![]),
    where_bounds: Arc::new(vec![]),
    impls: vec![],
  };

  Some(Arc::new(CalcitTypeAnnotation::Record(Arc::new(struct_def))))
}

fn parse_struct_name(form: &Calcit) -> Option<cirru_edn::EdnTag> {
  match form {
    Calcit::Symbol { sym, .. } | Calcit::Str(sym) => Some(cirru_edn::EdnTag::from(sym.as_ref())),
    Calcit::Tag(tag) => Some(tag.to_owned()),
    _ => None,
  }
}

fn parse_struct_field_entry(form: &Calcit) -> Option<(cirru_edn::EdnTag, Arc<CalcitTypeAnnotation>)> {
  let Calcit::List(list) = form else {
    return None;
  };
  let head = list.first()?;
  let is_list_literal = matches!(head, Calcit::Proc(CalcitProc::List))
    || matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "[]")
    || matches!(head, Calcit::Import(CalcitImport { ns, def, .. }) if &**ns == calcit::CORE_NS && &**def == "[]");

  if !is_list_literal {
    return None;
  }

  let field_name_form = list.get(1)?;
  let field_type_form = list.get(2)?;
  if list.len() != 3 {
    return None;
  }

  let field_name = match field_name_form {
    Calcit::Symbol { sym, .. } | Calcit::Str(sym) => cirru_edn::EdnTag::from(sym.as_ref()),
    Calcit::Tag(tag) => tag.to_owned(),
    _ => return None,
  };

  let field_type = CalcitTypeAnnotation::parse_type_annotation_form(field_type_form);
  Some((field_name, field_type))
}

pub(crate) fn infer_record_field_type(
  receiver: &Calcit,
  field_name: &str,
  scope_types: &ScopeTypes,
) -> Option<Arc<CalcitTypeAnnotation>> {
  let type_info = resolve_type_value(receiver, scope_types)?;
  resolve_struct_field_type(type_info.as_ref(), field_name)
}

fn resolve_struct_field_type(type_info: &CalcitTypeAnnotation, field_name: &str) -> Option<Arc<CalcitTypeAnnotation>> {
  let idx = type_info.resolve_to_struct()?.index_of(field_name)?;
  resolve_struct_field_type_by_index(type_info, idx)
}

fn resolve_struct_field_type_by_index(type_info: &CalcitTypeAnnotation, idx: usize) -> Option<Arc<CalcitTypeAnnotation>> {
  match type_info {
    CalcitTypeAnnotation::Optional(inner) => resolve_struct_field_type_by_index(inner.as_ref(), idx),
    CalcitTypeAnnotation::Record(struct_def) => struct_def.field_types.get(idx).cloned(),
    CalcitTypeAnnotation::Struct(struct_def, args) => {
      let field_type = struct_def.field_types.get(idx)?.clone();
      Some(substitute_declared_generics(
        struct_def.generics.as_ref(),
        args.as_ref(),
        field_type.as_ref(),
      ))
    }
    CalcitTypeAnnotation::TypeRef(_, args) => {
      let struct_def = type_info.resolve_to_struct()?;
      let field_type = struct_def.field_types.get(idx)?.clone();
      Some(substitute_declared_generics(
        struct_def.generics.as_ref(),
        args.as_ref(),
        field_type.as_ref(),
      ))
    }
    _ => None,
  }
}

fn substitute_declared_generics(
  declared_generics: &[Arc<str>],
  applied_args: &[Arc<CalcitTypeAnnotation>],
  field_type: &CalcitTypeAnnotation,
) -> Arc<CalcitTypeAnnotation> {
  if declared_generics.is_empty() || applied_args.is_empty() {
    return Arc::new(field_type.to_owned());
  }

  let mut bindings: HashMap<Arc<str>, Arc<CalcitTypeAnnotation>> = HashMap::new();
  for (name, arg) in declared_generics.iter().zip(applied_args.iter()) {
    bindings.insert(name.to_owned(), arg.to_owned());
  }
  field_type.substitute_type_vars(&bindings)
}

fn infer_struct_applied_args<'a>(
  struct_def: &CalcitStruct,
  values: impl Iterator<Item = &'a Calcit>,
  scope_types: &ScopeTypes,
) -> Vec<Arc<CalcitTypeAnnotation>> {
  if struct_def.generics.is_empty() {
    return vec![];
  }

  let mut bindings: HashMap<Arc<str>, Arc<CalcitTypeAnnotation>> = HashMap::new();
  for (value, expected_type) in values.zip(struct_def.field_types.iter()) {
    let actual_type = resolve_type_value(value, scope_types).unwrap_or_else(|| calcit::DYNAMIC_TYPE.clone());
    actual_type.as_ref().matches_with_bindings(expected_type.as_ref(), &mut bindings);
  }

  struct_def
    .generics
    .iter()
    .map(|name| bindings.get(name).cloned().unwrap_or_else(|| calcit::DYNAMIC_TYPE.clone()))
    .collect()
}

fn collect_record_literal_values(xs: &CalcitList, record: &CalcitRecord) -> Option<Vec<Calcit>> {
  if xs.len() < 2 {
    return None;
  }

  let mut values = vec![Calcit::Nil; record.struct_ref.fields.len()];
  let pair_count = (xs.len().saturating_sub(2)) / 2;
  for idx in 0..pair_count {
    let k_idx = idx * 2 + 2;
    let v_idx = k_idx + 1;
    let key = xs.get(k_idx)?;
    let value = xs.get(v_idx)?;
    let field_name = match key {
      Calcit::Tag(tag) => tag.ref_str(),
      Calcit::Symbol { sym, .. } | Calcit::Str(sym) => sym,
      _ => continue,
    };
    if let Some(pos) = record.index_of(field_name) {
      values[pos] = value.to_owned();
    }
  }

  Some(values)
}

pub(crate) fn extract_field_name(field_arg: &Calcit) -> Option<&str> {
  match field_arg {
    Calcit::Tag(tag) => Some(tag.ref_str()),
    Calcit::Str(s) => Some(s.as_ref()),
    Calcit::Symbol { sym, .. } => Some(sym.as_ref()),
    _ => None,
  }
}

// ---------------------------------------------------------------------------
// Value resolution helpers
// ---------------------------------------------------------------------------

pub(crate) fn resolve_program_value_for_preprocess(ns: &str, def: &str, def_id: Option<u32>) -> Option<Calcit> {
  let call_stack = CallStackList::default();
  runner::evaluate_symbol_from_program(def, ns, def_id, &call_stack).ok()
}

pub(crate) fn resolve_enum_value(target: &Calcit, scope_types: &ScopeTypes) -> Option<CalcitEnum> {
  match target {
    Calcit::Enum(enum_def) => Some(enum_def.to_owned()),
    Calcit::Record(record) => CalcitEnum::from_record(record.to_owned()).ok(),
    Calcit::Symbol { sym, info, .. } => match resolve_program_value_for_preprocess(&info.at_ns, sym, None) {
      Some(Calcit::Enum(enum_def)) => Some(enum_def),
      Some(Calcit::Record(record)) => CalcitEnum::from_record(record).ok(),
      _ => None,
    },
    Calcit::Import(CalcitImport { ns, def, def_id, .. }) => match resolve_program_value_for_preprocess(ns, def, *def_id) {
      Some(Calcit::Enum(enum_def)) => Some(enum_def),
      Some(Calcit::Record(record)) => CalcitEnum::from_record(record).ok(),
      _ => None,
    },
    _ => resolve_type_value(target, scope_types)
      .and_then(|t| match t.as_ref() {
        CalcitTypeAnnotation::TypeSlot(name) => resolve_type_slot(name),
        _ => Some(t),
      })
      .and_then(|t| t.resolve_to_struct())
      .and_then(|struct_def| {
        let len = struct_def.fields.len();
        let record = CalcitRecord {
          struct_ref: Arc::new(struct_def),
          values: Arc::new(vec![Calcit::Nil; len]),
        };
        CalcitEnum::from_record(record).ok()
      }),
  }
}

pub(crate) fn resolve_record_value(target: &Calcit, scope_types: &ScopeTypes) -> Option<CalcitRecord> {
  match target {
    Calcit::Record(record) => Some(record.to_owned()),
    Calcit::Enum(enum_def) => Some(enum_def.to_record_prototype()),
    Calcit::Struct(struct_def) => {
      let values = vec![Calcit::Nil; struct_def.fields.len()];
      Some(CalcitRecord {
        struct_ref: Arc::new(struct_def.to_owned()),
        values: Arc::new(values),
      })
    }
    Calcit::Symbol { sym, info, .. } => match resolve_program_value_for_preprocess(&info.at_ns, sym, None) {
      Some(Calcit::Record(record)) => Some(record),
      Some(Calcit::Enum(enum_def)) => Some(enum_def.to_record_prototype()),
      Some(Calcit::Struct(struct_def)) => {
        let values = vec![Calcit::Nil; struct_def.fields.len()];
        Some(CalcitRecord {
          struct_ref: Arc::new(struct_def.to_owned()),
          values: Arc::new(values),
        })
      }
      _ => None,
    },
    Calcit::Import(CalcitImport { ns, def, def_id, .. }) => {
      let runtime_value = resolve_program_value_for_preprocess(ns, def, *def_id);
      match runtime_value {
        Some(Calcit::Record(record)) => Some(record),
        Some(Calcit::Enum(enum_def)) => Some(enum_def.to_record_prototype()),
        Some(Calcit::Struct(struct_def)) => {
          let values = vec![Calcit::Nil; struct_def.fields.len()];
          Some(CalcitRecord {
            struct_ref: Arc::new(struct_def.to_owned()),
            values: Arc::new(values),
          })
        }
        _ => None,
      }
    }
    _ => resolve_type_value(target, scope_types).and_then(|t| {
      t.resolve_to_struct().map(|struct_def| CalcitRecord {
        struct_ref: Arc::new(struct_def.clone()),
        values: Arc::new(vec![Calcit::Nil; struct_def.fields.len()]),
      })
    }),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn proc_call(proc: CalcitProc, args: Vec<Calcit>) -> Calcit {
    let mut items = Vec::with_capacity(args.len() + 1);
    items.push(Calcit::Proc(proc));
    items.extend(args);
    Calcit::from(items)
  }

  #[test]
  fn infers_homogeneous_collection_literal_types() {
    let list = proc_call(CalcitProc::List, vec![Calcit::Number(1.0), Calcit::Number(2.0)]);
    let set = proc_call(CalcitProc::Set, vec![Calcit::Str(Arc::from("a")), Calcit::Str(Arc::from("b"))]);
    let map = proc_call(
      CalcitProc::NativeMap,
      vec![
        Calcit::Tag(cirru_edn::EdnTag::new("a")),
        Calcit::Number(1.0),
        Calcit::Tag(cirru_edn::EdnTag::new("b")),
        Calcit::Number(2.0),
      ],
    );

    assert!(matches!(
      infer_static_type_from_expr(&list).as_deref(),
      Some(CalcitTypeAnnotation::List(inner)) if matches!(inner.as_ref(), CalcitTypeAnnotation::Number)
    ));
    assert!(matches!(
      infer_static_type_from_expr(&set).as_deref(),
      Some(CalcitTypeAnnotation::Set(inner)) if matches!(inner.as_ref(), CalcitTypeAnnotation::String)
    ));
    assert!(matches!(
      infer_static_type_from_expr(&map).as_deref(),
      Some(CalcitTypeAnnotation::Map(key, value))
        if matches!(key.as_ref(), CalcitTypeAnnotation::Tag) && matches!(value.as_ref(), CalcitTypeAnnotation::Number)
    ));
  }

  #[test]
  fn heterogeneous_collection_and_atom_inference_keep_safe_boundaries() {
    let mixed = proc_call(CalcitProc::List, vec![Calcit::Number(1.0), Calcit::Str(Arc::from("x"))]);
    let atom = proc_call(CalcitProc::Atom, vec![Calcit::Number(1.0)]);

    assert!(matches!(
      infer_static_type_from_expr(&mixed).as_deref(),
      Some(CalcitTypeAnnotation::List(inner)) if matches!(inner.as_ref(), CalcitTypeAnnotation::Dynamic)
    ));
    assert!(matches!(
      infer_static_type_from_expr(&atom).as_deref(),
      Some(CalcitTypeAnnotation::Ref(inner)) if matches!(inner.as_ref(), CalcitTypeAnnotation::Number)
    ));
  }
}
