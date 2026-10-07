//! Type inference / synthesis module.
//!
//! Pure bottom-up type inference: given an expression, synthesize its type
//! without any "expected type" context flowing in. This corresponds to the
//! **synthesis** direction in bidirectional type checking.
//!
//! Key entry points:
//! - `resolve_type_value` — resolve the type of an already-preprocessed expression
//! - `infer_type_from_expr` — synthesize a type for an arbitrary expression
//! - `resolve_enum_value` / `resolve_struct_value` — resolve data definitions

use std::sync::Arc;
use std::{
  cell::RefCell,
  collections::{HashMap, HashSet},
};

use crate::calcit::type_annotation::code_resolves_to_nominal_type_def;
use crate::{
  builtins,
  calcit::{
    self, Calcit, CalcitEnumDef, CalcitFnTypeAnnotation, CalcitImpl, CalcitImport, CalcitList, CalcitProc, CalcitStructDef,
    CalcitStructValue, CalcitSyntax, CalcitTrait, CalcitTypeAnnotation, ImportInfo, SchemaKind, resolve_type_slot,
  },
  call_stack::CallStackList,
  program, runner,
};
use cirru_edn::EdnTag;

use super::{
  ScopeTypes, checked_call_contract::resolve_checked_call_contract, find_method_entry_for_type, find_trait_field_type,
  get_impls_from_type, lookup_source_backed_trait_def, reachable_dispatch_traits, resolve_local_type_refs_for_body,
  resolve_namespace_type_refs_for_body, resolve_program_trait_refs_for_body, selected_trait_method, tag_annotation,
  trait_is_external_object, trait_list_from_type,
};

// ---------------------------------------------------------------------------
// Core resolution
// ---------------------------------------------------------------------------

const ASYNC_INVOCATION_VALUE_TYPE: &str = "calcit.core/$AsyncInvocation";

fn pending_async_value(logical_return: Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::TypeRef(
    Arc::from(ASYNC_INVOCATION_VALUE_TYPE),
    Arc::new(vec![logical_return]),
  ))
}

pub(crate) fn is_pending_async_value(value: &CalcitTypeAnnotation) -> bool {
  matches!(value, CalcitTypeAnnotation::TypeRef(name, args) if name.as_ref() == ASYNC_INVOCATION_VALUE_TYPE && args.len() == 1)
}

fn awaited_async_value(value: Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  async_invocation_result(value.as_ref()).unwrap_or_else(|| calcit::DYNAMIC_TYPE.clone())
}

/// Recover the logical result only at an await or an async function return boundary.
pub(crate) fn async_invocation_result(value: &CalcitTypeAnnotation) -> Option<Arc<CalcitTypeAnnotation>> {
  match value {
    CalcitTypeAnnotation::TypeRef(name, args) if name.as_ref() == ASYNC_INVOCATION_VALUE_TYPE && args.len() == 1 => {
      Some(args[0].clone())
    }
    _ => None,
  }
}

fn definition_marks_async(ns: &str, def: &str) -> bool {
  program::lookup_def_code(ns, def).is_some_and(|code| CalcitTypeAnnotation::function_form_marks_async(&code))
}

fn invocation_return_type(
  signature: &CalcitFnTypeAnnotation,
  logical_return: Arc<CalcitTypeAnnotation>,
  definition_async: bool,
) -> Arc<CalcitTypeAnnotation> {
  if signature.is_async_invocation() || definition_async {
    pending_async_value(logical_return)
  } else {
    logical_return
  }
}

fn definition_value_schema(ns: &str, def: &str, schema: Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  let schema = resolve_namespace_type_refs_for_body(schema, ns);
  if !definition_marks_async(ns, def) {
    return schema;
  }
  match schema.as_ref() {
    CalcitTypeAnnotation::Fn(signature) => Arc::new(CalcitTypeAnnotation::Fn(Arc::new(signature.with_async_invocation()))),
    _ => schema,
  }
}

fn mark_async_callable(annotation: Arc<CalcitTypeAnnotation>, is_async: bool) -> Arc<CalcitTypeAnnotation> {
  if !is_async {
    return annotation;
  }
  match annotation.as_ref() {
    CalcitTypeAnnotation::Fn(signature) => Arc::new(CalcitTypeAnnotation::Fn(Arc::new(signature.with_async_invocation()))),
    _ => annotation,
  }
}

/// Resolves expression type evidence and normalizes trait references for method inference.
pub(crate) fn resolve_type_value(target: &Calcit, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  match target {
    Calcit::Local(local) => {
      // First check if the local has inline type_info, then fall back to scope_types
      if matches!(*local.type_info, CalcitTypeAnnotation::Dynamic) {
        let scoped = scope_types.get(&local.sym).cloned();
        scoped.map(resolve_trait_type_ref)
      } else {
        Some(resolve_trait_type_ref(local.type_info.clone()))
      }
    }
    Calcit::Symbol { sym, .. } => scope_types
      .get(sym)
      .cloned()
      .map(resolve_trait_type_ref)
      .or_else(|| infer_type_from_expr(target, scope_types).map(resolve_trait_type_ref)),
    _ => infer_type_from_expr(target, scope_types).map(resolve_trait_type_ref),
  }
}

fn resolve_trait_type_ref(value: Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  let CalcitTypeAnnotation::TypeRef(name, args) = value.as_ref() else {
    return normalize_variadic_as_list(value);
  };
  if !args.is_empty() {
    return value;
  }
  let Some((ns, def)) = name.rsplit_once('/') else {
    return value;
  };
  if let Some(trait_def) = lookup_source_backed_trait_def(ns, def) {
    return Arc::new(CalcitTypeAnnotation::Trait(Arc::new(trait_def.with_definition_ref(ns, def))));
  }
  if let Some(resolved) = infer_definition_value_type(ns, def)
    && matches!(
      resolved.as_ref(),
      CalcitTypeAnnotation::Trait(_) | CalcitTypeAnnotation::TraitSet(_)
    )
  {
    return resolved;
  }
  value
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

fn wrap_option_like(template: &CalcitTypeAnnotation, inner: Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  match template {
    CalcitTypeAnnotation::Optional(_) => Arc::new(CalcitTypeAnnotation::Optional(inner)),
    CalcitTypeAnnotation::TypeRef(name, _) => Arc::new(CalcitTypeAnnotation::TypeRef(name.clone(), Arc::new(vec![inner]))),
    _ => inner,
  }
}

fn compatible_if_join(
  true_type: &Arc<CalcitTypeAnnotation>,
  false_type: &Arc<CalcitTypeAnnotation>,
) -> Option<Arc<CalcitTypeAnnotation>> {
  let true_accepts_false = true_type.as_ref().is_compatible_with(false_type.as_ref());
  let false_accepts_true = false_type.as_ref().is_compatible_with(true_type.as_ref());
  let true_weight = super::annotation_dynamic_weight(true_type.as_ref());
  let false_weight = super::annotation_dynamic_weight(false_type.as_ref());

  // A candidate branch is a valid join only when the *other* branch is
  // acceptable where it is required. The option wrappers are asymmetric on
  // purpose (`Number` is accepted where `Option<Number>` is required, but not
  // the reverse), so checking the wrong direction would silently drop the
  // option branch and erase the possibility of absence. When both branches are
  // valid, prefer the more Dynamic one so evidence is never narrowed.
  if false_accepts_true && true_weight >= false_weight {
    Some(true_type.clone())
  } else if true_accepts_false && false_weight >= true_weight {
    Some(false_type.clone())
  } else {
    None
  }
}

/// Payload of a schema-level `Option<T>` (`TypeRef`) only. The internal
/// `Optional` marker used for control-flow joins keeps its historical
/// compatibility path so core bootstrap macros are unaffected.
fn schema_option_payload(type_value: &CalcitTypeAnnotation) -> Option<Arc<CalcitTypeAnnotation>> {
  match type_value {
    CalcitTypeAnnotation::TypeRef(_, _) => type_value.option_payload(),
    _ => None,
  }
}

fn merge_if_branch_types(
  true_type: Arc<CalcitTypeAnnotation>,
  false_type: Arc<CalcitTypeAnnotation>,
) -> Option<Arc<CalcitTypeAnnotation>> {
  if matches!(true_type.as_ref(), CalcitTypeAnnotation::Never) {
    return Some(false_type);
  }
  if matches!(false_type.as_ref(), CalcitTypeAnnotation::Never) {
    return Some(true_type);
  }
  // Root Dynamic admits every value shape. A container with several open
  // slots cannot outrank it merely by accumulating a larger heuristic score.
  if matches!(true_type.as_ref(), CalcitTypeAnnotation::Dynamic) || matches!(false_type.as_ref(), CalcitTypeAnnotation::Dynamic) {
    return Some(calcit::DYNAMIC_TYPE.clone());
  }
  if let Some(joined) = compatible_if_join(&true_type, &false_type) {
    return Some(joined);
  }

  // A live value joined with nil keeps its evidence and the possibility of
  // absence. Existing nullable types were handled by compatibility above;
  // this is not a nominal Option constructor or evidence for an open value.
  if matches!(true_type.as_ref(), CalcitTypeAnnotation::Nil) {
    return Some(Arc::new(CalcitTypeAnnotation::Optional(false_type)));
  }
  if matches!(false_type.as_ref(), CalcitTypeAnnotation::Nil) {
    return Some(Arc::new(CalcitTypeAnnotation::Optional(true_type)));
  }

  // The branches share no compatible annotation. When one side is a schema
  // `Option<T>`, widen the other branch back into the wrapper so the
  // possibility of absence is not erased; otherwise an unchecked operation
  // could observe `none` as `NaN`.
  match (
    schema_option_payload(true_type.as_ref()),
    schema_option_payload(false_type.as_ref()),
  ) {
    (Some(true_inner), Some(false_inner)) => {
      let merged = merge_if_branch_types(true_inner, false_inner)?;
      Some(wrap_option_like(true_type.as_ref(), merged))
    }
    (Some(true_inner), None) => {
      let merged = merge_if_branch_types(true_inner, false_type.clone())?;
      Some(wrap_option_like(true_type.as_ref(), merged))
    }
    (None, Some(false_inner)) => {
      let merged = merge_if_branch_types(true_type.clone(), false_inner)?;
      Some(wrap_option_like(false_type.as_ref(), merged))
    }
    (None, None) => None,
  }
}

fn nominal_constructor_variant(expr: &Calcit) -> Option<&'static str> {
  let Calcit::List(items) = expr else { return None };
  match items.first()? {
    Calcit::Import(CalcitImport { ns, def, .. }) if ns.as_ref() == calcit::CORE_NS => match def.as_ref() {
      "%ok" => Some("ok"),
      "%err" => Some("err"),
      "%none" if items.len() == 1 => Some("none"),
      _ => None,
    },
    Calcit::Registered(name) => match name.as_ref() {
      "%ok" | "calcit.core/%ok" => Some("ok"),
      "%err" | "calcit.core/%err" => Some("err"),
      "%none" | "calcit.core/%none" if items.len() == 1 => Some("none"),
      _ => None,
    },
    _ => None,
  }
}

// An empty core Option carries no T value. Let a later concrete argument bind T,
// but never grant this exception to an open Option that could contain a payload.
fn core_option_none_without_payload(expr: &Calcit, actual_type: &CalcitTypeAnnotation, expected_type: &CalcitTypeAnnotation) -> bool {
  let Calcit::List(items) = expr else { return false };
  let is_core_none = match (items.len(), items.first(), items.get(1)) {
    (1, Some(Calcit::Import(CalcitImport { ns, def, .. })), _) => ns.as_ref() == calcit::CORE_NS && def.as_ref() == "%none",
    (1, Some(Calcit::Fn { info, .. }), _) => info.def_ns.as_ref() == calcit::CORE_NS && info.name.as_ref() == "%none",
    (1, Some(Calcit::Registered(name)), _) => name.as_ref() == "calcit.core/%none",
    (3, Some(Calcit::Proc(CalcitProc::NativeNamedEnumNew)), Some(Calcit::Import(import))) => {
      import.ns.as_ref() == "calcit.core"
        && import.def.as_ref() == "Option"
        && matches!(items.get(2), Some(Calcit::Tag(variant)) if variant.ref_str() == "none")
    }
    _ => false,
  };
  is_core_none
    && match actual_type {
      CalcitTypeAnnotation::TypeRef(name, args) => {
        name.as_ref() == "calcit.core/Option"
          && matches!(args.as_slice(), [payload] if matches!(payload.as_ref(), CalcitTypeAnnotation::Dynamic))
      }
      CalcitTypeAnnotation::Enum(def, args) => {
        def.definition_ref().is_some_and(|name| name.as_ref() == "calcit.core/Option")
          && matches!(args.as_slice(), [payload] if matches!(payload.as_ref(), CalcitTypeAnnotation::Dynamic))
      }
      _ => false,
    }
    && matches!(expected_type, CalcitTypeAnnotation::TypeRef(name, args)
      if matches!(name.as_ref(), "Option" | "calcit.core/Option") && matches!(args.as_slice(), [payload] if matches!(payload.as_ref(), CalcitTypeAnnotation::TypeVar(_))))
}

// A known error has no success payload. Bind E from the actual error while
// leaving T for a later fallback; an open Result is not proof of an error.
#[cfg(test)]
fn bind_core_result_err_without_ok_payload(
  expr: &Calcit,
  actual_type: &CalcitTypeAnnotation,
  expected_type: &CalcitTypeAnnotation,
  bindings: &mut HashMap<Arc<str>, Arc<CalcitTypeAnnotation>>,
) -> Option<bool> {
  let (actual_error, expected_error) = core_result_err_payload_types(expr, actual_type, expected_type)?;
  Some(!actual_error.prove_available_bindings(expected_error, bindings).is_mismatch())
}

fn core_result_err_payload_types<'a>(
  expr: &Calcit,
  actual_type: &'a CalcitTypeAnnotation,
  expected_type: &'a CalcitTypeAnnotation,
) -> Option<(&'a CalcitTypeAnnotation, &'a CalcitTypeAnnotation)> {
  let Calcit::List(items) = expr else { return None };
  let is_core_err = match (items.len(), items.first(), items.get(1), items.get(2)) {
    (2, Some(Calcit::Import(CalcitImport { ns, def, .. })), _, _) => ns.as_ref() == calcit::CORE_NS && def.as_ref() == "%err",
    (2, Some(Calcit::Fn { info, .. }), _, _) => info.def_ns.as_ref() == calcit::CORE_NS && info.name.as_ref() == "%err",
    (2, Some(Calcit::Registered(name)), _, _) => name.as_ref() == "calcit.core/%err",
    (4, Some(Calcit::Proc(CalcitProc::NativeNamedEnumNew)), Some(Calcit::Import(import)), Some(Calcit::Tag(variant))) => {
      import.ns.as_ref() == calcit::CORE_NS && import.def.as_ref() == "Result" && variant.ref_str() == "err"
    }
    _ => false,
  };
  if !is_core_err {
    return None;
  }
  let actual_args = match actual_type {
    CalcitTypeAnnotation::TypeRef(name, args) if name.as_ref() == "calcit.core/Result" => args,
    CalcitTypeAnnotation::Enum(def, args) if def.definition_ref().is_some_and(|name| name.as_ref() == "calcit.core/Result") => args,
    _ => return None,
  };
  let CalcitTypeAnnotation::TypeRef(expected_name, expected_args) = expected_type else {
    return None;
  };
  if !matches!(expected_name.as_ref(), "Result" | "calcit.core/Result") {
    return None;
  }
  let ([actual_ok, actual_error], [expected_ok, expected_error]) = (actual_args.as_slice(), expected_args.as_slice()) else {
    return None;
  };
  if !matches!(actual_ok.as_ref(), CalcitTypeAnnotation::Dynamic) || !matches!(expected_ok.as_ref(), CalcitTypeAnnotation::TypeVar(_)) {
    return None;
  }
  Some((actual_error.as_ref(), expected_error.as_ref()))
}

/// Check payload evidence without treating unused constructor slots as open values.
pub(crate) fn constructor_payload_is_proven(actual: &CalcitTypeAnnotation, expected: &CalcitTypeAnnotation) -> bool {
  if actual.is_proven_for(expected) {
    return true;
  }
  // Legacy nullable fields store either nil or the inner value directly.
  // Check their payload rather than treating this runtime shape as narrowing.
  if let CalcitTypeAnnotation::Optional(expected_inner) | CalcitTypeAnnotation::JsNullish(expected_inner) = expected {
    return match actual {
      CalcitTypeAnnotation::Nil => true,
      CalcitTypeAnnotation::Optional(actual_inner) | CalcitTypeAnnotation::JsNullish(actual_inner) => {
        constructor_payload_is_proven(actual_inner, expected_inner)
      }
      _ => constructor_payload_is_proven(actual, expected_inner),
    };
  }
  false
}

fn enum_constructor_type_participation(expr: &Calcit, definition: &CalcitEnumDef, scope: &ScopeTypes) -> Option<Vec<Arc<str>>> {
  let Calcit::List(items) = expr else { return None };
  // A compiled constructor wrapper also fixes the variant. Inspect its
  // single tail constructor, never a schema-only promise or an arbitrary name.
  let owned;
  let items = if let Some(Calcit::Import(import)) = items.first() {
    owned = program::lookup_compiled_def(&import.ns, &import.def)?.preprocessed_code;
    let Calcit::List(body) = &owned else { return None };
    if !matches!(body.first(), Some(Calcit::Syntax(CalcitSyntax::Defn, _))) {
      return None;
    }
    let Calcit::List(tail) = body
      .iter()
      .skip(3)
      .filter(|form| !crate::builtins::syntax::is_function_metadata_hint(form))
      .last()?
    else {
      return None;
    };
    tail
  } else {
    items
  };
  if !matches!(
    items.first(),
    Some(Calcit::Proc(CalcitProc::NativeEnumNew | CalcitProc::NativeNamedEnumNew))
  ) {
    return None;
  }
  let Calcit::Tag(tag) = items.get(2)? else { return None };
  let prototype = resolve_enum_value(items.get(1)?, scope)?;
  if &prototype != definition && !prototype.same_nominal_definition(definition) {
    return None;
  }
  let variant = definition.find_variant_by_name(tag.ref_str())?;
  (items.len() == variant.arity() + 3).then(|| crate::calcit::type_annotation::free_type_variable_names(variant.payload_types()))
}

/// Join nominal enum slots from inferred types, independently of source shape.
/// Never contributes no value; Dynamic remains real open payload evidence.
fn merge_nominal_enum_branches<'a>(
  branches: impl IntoIterator<Item = &'a Arc<CalcitTypeAnnotation>>,
) -> Option<Arc<CalcitTypeAnnotation>> {
  let mut prototype: Option<Arc<CalcitEnumDef>> = None;
  let mut slots: Vec<Option<Arc<CalcitTypeAnnotation>>> = Vec::new();
  let mut missing_evidence = false;
  for annotation in branches {
    let (definition, arguments) = match annotation.as_ref() {
      CalcitTypeAnnotation::Enum(definition, arguments) => (definition.clone(), arguments.clone()),
      CalcitTypeAnnotation::TypeRef(_, arguments) => (Arc::new(annotation.resolve_to_enum()?), arguments.clone()),
      _ => return None,
    };
    if definition.generics().len() != arguments.len() {
      return None;
    }
    if let Some(previous) = &prototype {
      if previous != &definition && !previous.same_nominal_definition(&definition) {
        return None;
      }
    } else {
      prototype = Some(definition.clone());
      slots.resize(arguments.len(), None);
    }
    for (index, argument) in arguments.iter().enumerate() {
      if matches!(argument.as_ref(), CalcitTypeAnnotation::Never) {
        missing_evidence = true;
        continue;
      }
      slots[index] = Some(match slots[index].take() {
        Some(previous) => merge_if_branch_types(previous, argument.clone())?,
        None => argument.clone(),
      });
    }
  }
  if !missing_evidence {
    return None;
  }
  let prototype = prototype?;
  let arguments = Arc::new(
    slots
      .into_iter()
      .map(|slot| slot.unwrap_or_else(|| crate::calcit::type_annotation::NEVER_TYPE.clone()))
      .collect(),
  );
  Some(Arc::new(if let Some(reference) = prototype.definition_ref() {
    CalcitTypeAnnotation::TypeRef(reference.clone(), arguments)
  } else {
    CalcitTypeAnnotation::Enum(prototype, arguments)
  }))
}

/// Reuse branch joining where several callbacks share one returned value contract.
pub(crate) fn join_return_types(
  left: Arc<CalcitTypeAnnotation>,
  right: Arc<CalcitTypeAnnotation>,
) -> Option<Arc<CalcitTypeAnnotation>> {
  merge_nominal_enum_branches([&left, &right]).or_else(|| merge_if_branch_types(left, right))
}

/// Whether an enum retains direct absence evidence that payload inference may refine.
pub(super) fn has_payload_free_enum_slot(annotation: &CalcitTypeAnnotation) -> bool {
  matches!(annotation, CalcitTypeAnnotation::Enum(_, args) | CalcitTypeAnnotation::TypeRef(_, args)
    if args.iter().any(|arg| matches!(arg.as_ref(), CalcitTypeAnnotation::Never)) && annotation.resolve_to_enum().is_some())
}

/// Collect lexical tail-transfer evidence without completing unresolved inputs.
/// Nested functions and quoted data do not belong to the current transfer scope.
pub(super) fn lexical_recur_inputs(
  body: &[Calcit],
  scope_types: &ScopeTypes,
  arity: usize,
) -> Vec<Vec<Option<Arc<CalcitTypeAnnotation>>>> {
  let mut transfers = Vec::new();
  let Some(tail) = body.iter().rev().find(|form| !builtins::syntax::is_function_metadata_hint(form)) else {
    return transfers;
  };
  let mut pending = vec![(tail, scope_types.clone())];
  while let Some((expr, scope)) = pending.pop() {
    let arguments = match expr {
      Calcit::Recur(arguments) => arguments.iter().collect::<Vec<_>>(),
      Calcit::List(items) => {
        let Some(head) = items.first() else {
          continue;
        };
        match head {
          Calcit::Proc(CalcitProc::Recur) => items.iter().skip(1).collect(),
          Calcit::Syntax(CalcitSyntax::CoreLet, _) => {
            if let Some(tail) = items.iter().last() {
              pending.push((tail, core_let_scope(items, &scope)));
            }
            continue;
          }
          Calcit::Syntax(CalcitSyntax::If, _) => {
            pending.extend(items.iter().skip(2).map(|branch| (branch, scope.clone())));
            continue;
          }
          Calcit::Syntax(CalcitSyntax::Match, _) => {
            let Some(branches) = preprocessed_match_branches(items) else {
              continue;
            };
            for (pattern, branch) in branches {
              let mut branch_scope = scope.clone();
              bind_pattern_scope(pattern, &mut branch_scope);
              pending.push((branch, branch_scope));
            }
            continue;
          }
          // A function-valued expression or a quote is not a lexical transfer.
          _ => continue,
        }
      }
      _ => continue,
    };
    if arguments.len() != arity {
      continue;
    }
    transfers.push(arguments.into_iter().map(|argument| resolve_type_value(argument, &scope)).collect());
  }
  transfers
}

fn merge_result_constructor_branches(
  true_expr: &Calcit,
  true_type: &CalcitTypeAnnotation,
  false_expr: &Calcit,
  false_type: &CalcitTypeAnnotation,
) -> Option<Arc<CalcitTypeAnnotation>> {
  let (ok_type, err_type) = match (nominal_constructor_variant(true_expr), nominal_constructor_variant(false_expr)) {
    (Some("ok"), Some("err")) => (true_type, false_type),
    (Some("err"), Some("ok")) => (false_type, true_type),
    _ => return None,
  };
  let (CalcitTypeAnnotation::TypeRef(ok_name, ok_args), CalcitTypeAnnotation::TypeRef(err_name, err_args)) = (ok_type, err_type) else {
    return None;
  };
  if ok_name != err_name || !matches!(ok_name.as_ref(), "Result" | "calcit.core/Result") || ok_args.len() != 2 || err_args.len() != 2 {
    return None;
  }
  if !matches!(ok_args[1].as_ref(), CalcitTypeAnnotation::Dynamic) || !matches!(err_args[0].as_ref(), CalcitTypeAnnotation::Dynamic) {
    return None;
  }
  Some(Arc::new(CalcitTypeAnnotation::TypeRef(
    ok_name.clone(),
    Arc::new(vec![ok_args[0].clone(), err_args[1].clone()]),
  )))
}

fn merge_option_absence_branch(
  true_expr: &Calcit,
  true_type: &Arc<CalcitTypeAnnotation>,
  false_expr: &Calcit,
  false_type: &Arc<CalcitTypeAnnotation>,
) -> Option<Arc<CalcitTypeAnnotation>> {
  let (absent_type, other_type) = if nominal_constructor_variant(true_expr) == Some("none") {
    (true_type, false_type)
  } else if nominal_constructor_variant(false_expr) == Some("none") {
    (false_type, true_type)
  } else {
    return None;
  };
  let (CalcitTypeAnnotation::TypeRef(absent_name, absent_args), CalcitTypeAnnotation::TypeRef(other_name, other_args)) =
    (absent_type.as_ref(), other_type.as_ref())
  else {
    return None;
  };
  // Only a known empty constructor has no payload evidence. An arbitrary
  // Option<Dynamic> branch must still weaken the join, never narrow it.
  (absent_name == other_name
    && matches!(absent_name.as_ref(), "Option" | "calcit.core/Option")
    && absent_args.len() == 1
    && other_args.len() == 1
    && matches!(absent_args[0].as_ref(), CalcitTypeAnnotation::Dynamic))
  .then(|| other_type.clone())
}

pub(crate) fn infer_if_return_type(xs: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  if xs.len() < 3 {
    return None;
  }

  let true_expr = xs.get(2)?;
  if let Some(false_expr) = xs.get(3) {
    // A direct raise has no value to join with the live branch. Keep an
    // ordinary Dynamic branch in the join; only proven divergence is bottom.
    if expression_definitely_diverges(true_expr) {
      return resolve_type_value(false_expr, scope_types);
    }
    if expression_definitely_diverges(false_expr) {
      return resolve_type_value(true_expr, scope_types);
    }
    let true_type = resolve_type_value(true_expr, scope_types)?;
    let false_type = resolve_type_value(false_expr, scope_types)?;
    merge_nominal_enum_branches([&true_type, &false_type])
      .or_else(|| merge_result_constructor_branches(true_expr, true_type.as_ref(), false_expr, false_type.as_ref()))
      .or_else(|| merge_option_absence_branch(true_expr, &true_type, false_expr, &false_type))
      .or_else(|| merge_if_branch_types(true_type, false_type))
  } else {
    let true_type = resolve_type_value(true_expr, scope_types)?;
    Some(Arc::new(CalcitTypeAnnotation::Optional(true_type)))
  }
}

/// Join normal and caught values without evaluating the lazy handler. Its
/// invocation consumes the runtime's String error, not the enclosing return
/// declaration. Unknown or incompatible handler inputs retain missing proof.
fn infer_try_return_type(xs: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  if xs.len() != 3 {
    return None;
  }
  let body = xs.get(1)?;
  let handler = xs.get(2)?;
  if expression_definitely_diverges(handler) {
    return if expression_definitely_diverges(body) {
      Some(crate::calcit::type_annotation::NEVER_TYPE.clone())
    } else {
      resolve_type_value(body, scope_types)
    };
  }
  let signature = resolve_type_value(handler, scope_types)?.resolve_to_nonoptional_fn()?;
  // Match ordinary calls: only fixed functions fill omitted trailing Options.
  // Variadic functions still require every fixed argument before their rest.
  let trailing_options = if signature.rest_type.is_some() {
    0
  } else {
    calcit::trailing_option_arg_count(&signature.arg_types, signature.arg_types.len())
  };
  if signature.fn_kind != SchemaKind::Fn
    || signature.arg_types.len() - trailing_options > 1
    || signature.arg_types.is_empty() && signature.rest_type.is_none()
  {
    return None;
  }
  let argument = Calcit::Str(Arc::from(""));
  let expected = signature.arg_types.first().or(signature.rest_type.as_ref())?;
  let mut bindings = HashMap::new();
  if !CalcitTypeAnnotation::String
    .prove_with_bindings(expected, &mut bindings)
    .is_proven()
  {
    return None;
  }
  let call = Calcit::from(vec![handler.clone(), argument]);
  let caught = resolve_type_value(&call, scope_types)?;
  if expression_definitely_diverges(body) {
    return Some(caught);
  }
  let normal = resolve_type_value(body, scope_types)?;
  merge_nominal_enum_branches([&normal, &caught]).or_else(|| merge_if_branch_types(normal, caught))
}

/// Infer independent function exits without assigning a value type to tail recur.
/// A transfer is valid only against the current lexical parameter contract.
/// Ordinary expression inference intentionally continues to treat recur as unknown.
pub(crate) fn infer_function_exit_type(
  expr: &Calcit,
  scope_types: &ScopeTypes,
  parameters: &[Arc<CalcitTypeAnnotation>],
) -> Option<Arc<CalcitTypeAnnotation>> {
  enum Exit {
    Value(Arc<CalcitTypeAnnotation>),
    Transfer,
  }

  fn infer(expr: &Calcit, scope: &ScopeTypes, parameters: &[Arc<CalcitTypeAnnotation>]) -> Option<Exit> {
    let Calcit::List(items) = expr else {
      return resolve_type_value(expr, scope).map(Exit::Value);
    };
    match items.first()? {
      Calcit::Proc(CalcitProc::Recur) => {
        if items.len() != parameters.len() + 1 {
          return None;
        }
        let mut bindings = HashMap::new();
        for (argument, expected) in items.iter().skip(1).zip(parameters) {
          if !matches!(
            resolve_type_value(argument, scope)?.prove_with_bindings(expected, &mut bindings),
            crate::calcit::type_annotation::TypeProof::Proven
          ) || !bindings.is_empty()
          {
            // Recur preserves the current instantiation. Unlike a new call,
            // it cannot infer fresh substitutions for lexical type variables.
            return None;
          }
        }
        Some(Exit::Transfer)
      }
      Calcit::Syntax(CalcitSyntax::CoreLet, _) => {
        let tail = items.get(items.len().checked_sub(1)?)?;
        infer(tail, &core_let_scope(items, scope), parameters)
      }
      Calcit::Syntax(CalcitSyntax::If, _) if items.len() == 4 => {
        let left = items.get(2)?;
        let right = items.get(3)?;
        let left_exit = if expression_definitely_diverges(left) {
          Exit::Transfer
        } else {
          infer(left, scope, parameters)?
        };
        let right_exit = if expression_definitely_diverges(right) {
          Exit::Transfer
        } else {
          infer(right, scope, parameters)?
        };
        match (left_exit, right_exit) {
          (Exit::Transfer, other) | (other, Exit::Transfer) => Some(other),
          (Exit::Value(left_type), Exit::Value(right_type)) => merge_nominal_enum_branches([&left_type, &right_type])
            .or_else(|| merge_result_constructor_branches(left, &left_type, right, &right_type))
            .or_else(|| merge_option_absence_branch(left, &left_type, right, &right_type))
            .or_else(|| merge_if_branch_types(left_type, right_type))
            .map(Exit::Value),
        }
      }
      _ => resolve_type_value(expr, scope).map(Exit::Value),
    }
  }

  match infer(expr, scope_types, parameters)? {
    Exit::Value(value) => Some(value),
    // A recursive cycle alone cannot independently establish a return contract.
    Exit::Transfer => None,
  }
}

/// Read pattern/body pairs from pair-based or indexed preprocessed matches.
/// Absent indexed branches are skipped; malformed branch pairs reject inference.
pub(super) fn preprocessed_match_branches(xs: &CalcitList) -> Option<Vec<(&Calcit, &Calcit)>> {
  if xs.len() < 3 {
    return None;
  }

  let indexed_table = matches!(xs.get(2), Some(Calcit::EnumDef(_)));
  let branches = if indexed_table {
    let Calcit::List(table) = xs.get(3)? else { return None };
    table.iter().collect::<Vec<_>>()
  } else {
    xs.iter().skip(2).collect::<Vec<_>>()
  };
  let mut bodies = Vec::with_capacity(branches.len());
  for branch in branches {
    if indexed_table && matches!(branch, Calcit::Nil) {
      continue;
    }
    let Calcit::List(pair) = branch else { return None };
    if pair.len() != 2 {
      return None;
    }
    bodies.push((pair.first()?, pair.get(1)?));
  }
  Some(bodies)
}

/// Project validated match branches when only their bodies are needed.
fn preprocessed_match_bodies(xs: &CalcitList) -> Option<Vec<&Calcit>> {
  Some(preprocessed_match_branches(xs)?.into_iter().map(|(_, body)| body).collect())
}

/// A Dynamic binder must shadow same-named outer evidence, too.
pub(super) fn bind_pattern_scope(pattern: &Calcit, scope: &mut ScopeTypes) {
  match pattern {
    Calcit::Local(local) => {
      scope.insert(local.sym.clone(), local.type_info.clone());
    }
    Calcit::List(items) => {
      for item in items.iter() {
        bind_pattern_scope(item, scope);
      }
    }
    _ => {}
  }
}

/// Infer a core-let initializer in its parent scope, then bind its local name.
/// Missing evidence becomes Dynamic and still shadows same-named outer evidence.
fn core_let_scope(items: &CalcitList, scope_types: &ScopeTypes) -> ScopeTypes {
  let mut scope = scope_types.clone();
  if let Some(Calcit::List(pair)) = items.get(1)
    && let (Some(Calcit::Local(local)), Some(value)) = (pair.first(), pair.get(1))
  {
    let evidence = resolve_type_value(value, scope_types).unwrap_or_else(|| calcit::DYNAMIC_TYPE.clone());
    scope.insert(local.sym.clone(), evidence);
  }
  scope
}

/// Project each reachable match branch's result in its lexical payload scope.
pub(super) fn match_branch_types(xs: &CalcitList, scope_types: &ScopeTypes) -> Vec<Option<Arc<CalcitTypeAnnotation>>> {
  let mut branches = Vec::new();
  let Some(pairs) = preprocessed_match_branches(xs) else {
    return branches;
  };
  for (pattern, branch_expr) in pairs {
    if expression_definitely_diverges(branch_expr) {
      continue;
    }
    let mut branch_scope = scope_types.clone();
    bind_pattern_scope(pattern, &mut branch_scope);
    branches.push(resolve_type_value(branch_expr, &branch_scope));
  }
  branches
}

/// Merge the body types of a preprocessed `match` expression.
fn infer_match_return_type(xs: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  let branches = match_branch_types(xs, scope_types).into_iter().collect::<Option<Vec<_>>>()?;
  if let Some(joined) = merge_nominal_enum_branches(branches.iter()) {
    return Some(joined);
  }
  let mut inferred: Option<Arc<CalcitTypeAnnotation>> = None;
  for branch_type in branches {
    inferred = Some(match inferred {
      Some(previous) => merge_if_branch_types(previous, branch_type)?,
      None => branch_type,
    });
  }
  inferred
}

// ---------------------------------------------------------------------------
// Generic return type resolution
// ---------------------------------------------------------------------------

/// Given a function's type info and the actual call arguments, resolve a generic return type.
/// If the return type contains TypeVars, match actual arg types against declared arg types
/// to build bindings, then substitute TypeVars in the return type.
/// Unbound generic payloads fall back to Dynamic while preserving the return
/// type's outer structure (for example Option<T> becomes Option<Dynamic>).
pub(crate) fn resolve_generic_return_type<'a>(
  fn_info: &crate::calcit::CalcitFn,
  call_args: impl Iterator<Item = &'a Calcit>,
  scope_types: &ScopeTypes,
) -> Option<Arc<CalcitTypeAnnotation>> {
  resolve_generic_return_type_parts(
    fn_info.generics.as_ref(),
    &fn_info.arg_types,
    fn_info.rest_type.as_ref(),
    &fn_info.return_type,
    call_args,
    scope_types,
  )
}

fn resolve_generic_return_type_parts<'a>(
  generics: &[Arc<str>],
  arg_types: &[Arc<CalcitTypeAnnotation>],
  rest_type: Option<&Arc<CalcitTypeAnnotation>>,
  return_type: &Arc<CalcitTypeAnnotation>,
  call_args: impl Iterator<Item = &'a Calcit>,
  scope_types: &ScopeTypes,
) -> Option<Arc<CalcitTypeAnnotation>> {
  // Only attempt resolution when there are generics and the return type contains TypeVars
  if generics.is_empty() || !return_type.contains_type_var() {
    return None;
  }

  let call_args = call_args.collect::<Vec<_>>();
  let actual_types = call_args
    .iter()
    .filter_map(|arg| resolve_type_value(arg, scope_types))
    .collect::<Vec<_>>();
  let mut expected_types = arg_types.to_vec();
  expected_types.extend(rest_type.cloned());
  expected_types.push(return_type.clone());
  let mut call_proof = crate::calcit::type_annotation::CallTypeProof::new(generics, &expected_types, &actual_types);

  // Match fixed and variadic actual arguments against the declared types.
  // Rest-only generic functions otherwise lose their payload type before a
  // typed receiver method has a chance to specialize.
  for (idx, arg) in call_args.into_iter().enumerate() {
    let Some(expected_type) = arg_types.get(idx).or(rest_type) else {
      break;
    };
    if matches!(**expected_type, CalcitTypeAnnotation::Dynamic) {
      continue;
    }
    if super::empty_container_has_no_type_evidence(arg, expected_type) {
      continue;
    }
    let actual_type = resolve_type_value(arg, scope_types).or_else(|| match arg {
      Calcit::Local(local)
        if matches!(expected_type.as_ref(), CalcitTypeAnnotation::TypeVar(_))
          && matches!(local.type_info.as_ref(), CalcitTypeAnnotation::Dynamic) =>
      {
        Some(local.type_info.clone())
      }
      _ => None,
    })?;
    if core_option_none_without_payload(arg, actual_type.as_ref(), expected_type.as_ref()) {
      continue;
    }
    if let Some((actual_error, expected_error)) = core_result_err_payload_types(arg, actual_type.as_ref(), expected_type.as_ref()) {
      if call_proof.prove(actual_error, expected_error).is_mismatch() {
        return None;
      }
      continue;
    }
    if call_proof.prove(actual_type.as_ref(), expected_type.as_ref()).is_mismatch() {
      return None;
    }
  }

  Some(call_proof.result_or_open(return_type))
}

pub(crate) fn infer_return_type_from_compiled_callable(
  ns: &str,
  def: &str,
  call_expr: &CalcitList,
  scope_types: &ScopeTypes,
) -> Option<Arc<CalcitTypeAnnotation>> {
  if ns == calcit::CORE_NS {
    let args = call_expr.drop_left();
    if let Some(contract) = resolve_checked_call_contract(ns, def, &args, scope_types)
      && !contract.return_type.contains_type_var()
    {
      return Some(contract.return_type);
    }
  }
  if ns == calcit::CORE_NS
    && def == "apply"
    && let Some(inferred) = infer_core_apply_return_type(call_expr, scope_types)
  {
    return Some(inferred);
  }
  if ns == calcit::CORE_NS
    && let Some(inferred) = infer_core_nominal_absence_return_type(def, call_expr, scope_types)
  {
    return Some(inferred);
  }
  if ns == calcit::CORE_NS
    && def == "get-in"
    && let Some(inferred) = infer_core_get_in_return_type(call_expr, scope_types)
  {
    return Some(inferred);
  }
  if ns == calcit::CORE_NS
    && def == "update"
    && let Some(receiver_type) = call_expr.get(1).and_then(|receiver| resolve_type_value(receiver, scope_types))
    && receiver_type.resolve_to_struct().is_some()
  {
    // Struct update is outside the collection contract but still preserves
    // the nominal receiver for checked field access after the call.
    return Some(receiver_type);
  }
  if ns == calcit::CORE_NS
    && def == "%::"
    && let Some(inferred) = infer_enum_annotation(call_expr, scope_types)
  {
    return Some(inferred);
  }
  if ns == calcit::CORE_NS
    && def == "impl-traits"
    && let Some(inferred) = infer_impl_attachment_type(call_expr, scope_types)
  {
    return Some(inferred);
  }

  let is_core_enum_constructor = ns == calcit::CORE_NS && matches!(def, "%some" | "%none" | "%ok" | "%err");

  // A definition schema is the public contract and is stronger evidence than
  // a Dynamic return inferred from its implementation body. Read it before
  // compiled metadata so generic collection/ref shapes survive call sites.
  // Resolve the declaration before substituting caller argument types. Resolving
  // the result afterward would reinterpret a caller-owned generic payload in ns.
  let declared_schema = resolve_namespace_type_refs_for_body(program::lookup_def_schema(ns, def), ns);
  let open_return = matches!(declared_schema.as_ref(), CalcitTypeAnnotation::Fn(info) if matches!(info.return_type.as_ref(), CalcitTypeAnnotation::Dynamic));
  if super::REQUIRE_ASSERTION_PROOF.with(std::cell::Cell::get)
    && (crate::snapshot::schema_annotation_is_missing(&declared_schema) || open_return)
    && let Some(returned) = with_callable_return_inference(ns, def, || {
      // Open metadata is not evidence either way. During an audit, recover
      // the source implementation's actual return without changing its public
      // contract or using a declared return as proof of that implementation.
      let implementation = infer_compiled_definition_implementation_type(ns, def)?;
      let CalcitTypeAnnotation::Fn(implementation) = implementation.as_ref() else {
        return None;
      };
      let signature = match declared_schema.as_ref() {
        CalcitTypeAnnotation::Fn(signature) => signature,
        _ => implementation,
      };
      let returned = resolve_namespace_type_refs_for_body(implementation.return_type.clone(), ns);
      let resolved = resolve_generic_return_type_parts(
        signature.generics.as_ref(),
        &signature.arg_types,
        signature.rest_type.as_ref(),
        &returned,
        call_expr.iter().skip(1),
        scope_types,
      )
      .or_else(|| (!returned.contains_type_var()).then(|| returned.clone()))?;
      Some(invocation_return_type(signature, resolved, definition_marks_async(ns, def)))
    })
  {
    return Some(returned);
  }
  if let CalcitTypeAnnotation::Fn(info) = declared_schema.as_ref() {
    let declared_return = resolve_generic_return_type_parts(
      info.generics.as_ref(),
      &info.arg_types,
      info.rest_type.as_ref(),
      &info.return_type,
      call_expr.iter().skip(1),
      scope_types,
    )
    // An explicit Dynamic result is a known open contract, not missing
    // inference. Do not let the caller's concrete context fill that gap.
    .or_else(|| (!info.return_type.contains_type_var()).then(|| info.return_type.clone()));
    if let Some(declared_return) = declared_return {
      // A public schema remains authoritative for the nominal shape and type
      // arguments, but an implementation can construct that same nominal
      // type through a top-level `impl-traits` alias. Preserve the attached
      // method table when compiled inference has that stronger evidence.
      if declared_return.resolve_to_struct().is_some()
        && let Some(compiled) = program::lookup_compiled_def(ns, def)
        && let Some(inferred_body_return) = infer_compiled_callable_body_return(ns, def, &compiled.preprocessed_code)
        && let Some(enriched) =
          enrich_declared_struct_return_with_impls(&declared_return, &resolve_namespace_type_refs_for_body(inferred_body_return, ns))
      {
        return Some(invocation_return_type(info, enriched, definition_marks_async(ns, def)));
      }
      return Some(invocation_return_type(info, declared_return, definition_marks_async(ns, def)));
    }
  }

  // Prefer compiled callable metadata. The four core enum constructors are
  // also needed while the core is bootstrapping, before their payloads are
  // compiled; their declared schemas unlock receiver-first method calls.
  if let Some(compiled) = program::lookup_compiled_def(ns, def) {
    // Avoid evaluating compiled payloads during preprocess type inference.
    // Evaluating function code here can recurse back into preprocess and overflow stack.
    match compiled.preprocessed_code {
      Calcit::List(ref forms) if crate::snapshot::schema_annotation_is_missing(&declared_schema) => {
        // Source-backed definitions retain a preprocessed defn rather than a
        // runtime Fn. Reuse its checked contract, without evaluating the body
        // or replacing an explicit public Dynamic declaration.
        if let Some(CalcitTypeAnnotation::Fn(info)) = forms
          .iter()
          .skip(3)
          .find_map(CalcitTypeAnnotation::extract_surrounding_fn_annotation_from_hint_form)
          .as_deref()
        {
          let qualified = resolve_namespace_type_refs_for_body(Arc::new(CalcitTypeAnnotation::Fn(info.clone())), ns);
          let CalcitTypeAnnotation::Fn(info) = qualified.as_ref() else {
            unreachable!("a qualified function annotation remains a function annotation")
          };
          let returned = resolve_generic_return_type_parts(
            info.generics.as_ref(),
            &info.arg_types,
            info.rest_type.as_ref(),
            &info.return_type,
            call_expr.iter().skip(1),
            scope_types,
          )
          .or_else(|| (!info.return_type.contains_type_var()).then(|| info.return_type.clone()))?;
          return Some(invocation_return_type(info, returned, definition_marks_async(ns, def)));
        }
      }
      Calcit::Fn { info, .. } => {
        let qualified = resolve_namespace_type_refs_for_body(Arc::new(CalcitTypeAnnotation::from_calcit_fn(&info)), ns);
        let CalcitTypeAnnotation::Fn(signature) = qualified.as_ref() else {
          unreachable!("a qualified function annotation remains a function annotation")
        };
        if let Some(resolved) = resolve_generic_return_type_parts(
          signature.generics.as_ref(),
          &signature.arg_types,
          signature.rest_type.as_ref(),
          &signature.return_type,
          call_expr.iter().skip(1),
          scope_types,
        ) {
          return Some(if definition_marks_async(ns, def) {
            pending_async_value(resolved)
          } else {
            resolved
          });
        }
        if !is_core_enum_constructor || !signature.return_type.contains_type_var() {
          return Some(if definition_marks_async(ns, def) {
            pending_async_value(signature.return_type.clone())
          } else {
            signature.return_type.clone()
          });
        }
      }
      Calcit::Proc(proc) => {
        if let Some(type_sig) = proc.get_type_signature()
          && (!is_core_enum_constructor || !type_sig.return_type.contains_type_var())
        {
          return Some(resolve_namespace_type_refs_for_body(type_sig.return_type.clone(), ns));
        }
      }
      _ => {}
    }
  }

  if !is_core_enum_constructor {
    return None;
  }

  let CalcitTypeAnnotation::Fn(info) = declared_schema.as_ref() else {
    return None;
  };
  if info.generics.is_empty() || !info.return_type.contains_type_var() {
    return Some(invocation_return_type(
      info,
      info.return_type.clone(),
      definition_marks_async(ns, def),
    ));
  }

  let mut bindings: HashMap<Arc<str>, Arc<CalcitTypeAnnotation>> = HashMap::new();
  for (arg, expected_type) in call_expr.iter().skip(1).zip(info.arg_types.iter()) {
    if !matches!(expected_type.as_ref(), CalcitTypeAnnotation::Dynamic)
      && let Some(actual_type) = resolve_type_value(arg, scope_types)
      && !actual_type
        .as_ref()
        .prove_with_bindings(expected_type.as_ref(), &mut bindings)
        .is_proven()
    {
      return None;
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
  (resolved.resolve_to_struct().is_some() || resolved.resolve_to_enum().is_some())
    .then(|| invocation_return_type(info, resolved, definition_marks_async(ns, def)))
}

thread_local! {
  static INFERRED_CALLABLE_IMPL_RETURNS: RefCell<HashSet<(String, String)>> = RefCell::new(HashSet::new());
}

fn with_callable_return_inference<R>(ns: &str, def: &str, infer: impl FnOnce() -> Option<R>) -> Option<R> {
  let key = (ns.to_owned(), def.to_owned());
  let entered = INFERRED_CALLABLE_IMPL_RETURNS.with(|definitions| definitions.borrow_mut().insert(key.clone()));
  if !entered {
    return None;
  }
  struct Restore((String, String));
  impl Drop for Restore {
    fn drop(&mut self) {
      INFERRED_CALLABLE_IMPL_RETURNS.with(|definitions| {
        definitions.borrow_mut().remove(&self.0);
      });
    }
  }
  let _restore = Restore(key);
  infer()
}

fn infer_compiled_callable_body_return(ns: &str, def: &str, code: &Calcit) -> Option<Arc<CalcitTypeAnnotation>> {
  with_callable_return_inference(ns, def, || {
    let body = match code {
      Calcit::Fn { info, .. } => info.body.last()?,
      Calcit::List(items)
        if matches!(
          items.first(),
          Some(Calcit::Syntax(
            CalcitSyntax::Defn | CalcitSyntax::DefWasmExport | CalcitSyntax::DefWasmImport,
            _
          ))
        ) =>
      {
        items.get(items.len().checked_sub(1)?)?
      }
      _ => return None,
    };
    infer_guaranteed_nominal_impl_return(body)
  })
}

fn infer_guaranteed_nominal_impl_return(expr: &Calcit) -> Option<Arc<CalcitTypeAnnotation>> {
  if let Calcit::List(items) = expr
    && let Some(head) = items.first()
  {
    match head {
      Calcit::Syntax(CalcitSyntax::CoreLet, _) => {
        return items
          .get(items.len().checked_sub(1)?)
          .and_then(infer_guaranteed_nominal_impl_return);
      }
      Calcit::Syntax(CalcitSyntax::If, _) => {
        let true_type = infer_guaranteed_nominal_impl_return(items.get(2)?);
        let false_expr = items.get(3)?;
        let false_type = infer_guaranteed_nominal_impl_return(false_expr);
        return merge_guaranteed_impl_returns(items.get(2)?, true_type, false_expr, false_type);
      }
      Calcit::Syntax(CalcitSyntax::Match, _) => {
        let mut merged = None;
        for branch_expr in preprocessed_match_bodies(items)? {
          if expression_definitely_diverges(branch_expr) {
            continue;
          }
          let branch_type = infer_guaranteed_nominal_impl_return(branch_expr)?;
          if let Some(previous) = &merged
            && previous != &branch_type
          {
            return None;
          }
          merged = Some(branch_type);
        }
        return merged;
      }
      _ => {}
    }
  }

  infer_type_from_expr(expr, &ScopeTypes::new()).filter(|annotation| match annotation.as_ref() {
    CalcitTypeAnnotation::Struct(struct_def, _) | CalcitTypeAnnotation::StructValue(struct_def) => !struct_def.impls.is_empty(),
    _ => false,
  })
}

fn merge_guaranteed_impl_returns(
  true_expr: &Calcit,
  true_type: Option<Arc<CalcitTypeAnnotation>>,
  false_expr: &Calcit,
  false_type: Option<Arc<CalcitTypeAnnotation>>,
) -> Option<Arc<CalcitTypeAnnotation>> {
  match (true_type, false_type) {
    (Some(a), Some(b)) if a == b => Some(a),
    (Some(a), None) if expression_definitely_diverges(false_expr) => Some(a),
    (None, Some(b)) if expression_definitely_diverges(true_expr) => Some(b),
    _ => None,
  }
}

pub(crate) fn expression_definitely_diverges(expr: &Calcit) -> bool {
  // Inspect only guaranteed exits, not conditional initializers or recur.
  // An iterative walk also handles deeply expanded lexical wrappers.
  let mut pending = vec![expr];
  while let Some(expr) = pending.pop() {
    let Calcit::List(items) = expr else { return false };
    match items.first() {
      Some(Calcit::Proc(CalcitProc::Raise)) => {}
      Some(Calcit::Import(CalcitImport { ns, def, .. })) if ns.as_ref() == calcit::CORE_NS && def.as_ref() == "raise" => {}
      Some(Calcit::Syntax(CalcitSyntax::CoreLet, _)) if items.len() >= 3 => {
        pending.push(items.get(items.len() - 1).expect("validated lexical tail"));
      }
      Some(Calcit::Syntax(CalcitSyntax::If, _)) if items.len() == 4 => {
        pending.extend(items.iter().skip(2));
      }
      Some(Calcit::Syntax(CalcitSyntax::Match, _)) => {
        let Some(branches) = preprocessed_match_bodies(items) else {
          return false;
        };
        if branches.is_empty() {
          return false;
        }
        pending.extend(branches);
      }
      _ => return false,
    }
  }
  true
}

fn enrich_declared_struct_return_with_impls(
  declared: &Arc<CalcitTypeAnnotation>,
  inferred: &Arc<CalcitTypeAnnotation>,
) -> Option<Arc<CalcitTypeAnnotation>> {
  let inferred_struct = match inferred.as_ref() {
    CalcitTypeAnnotation::Struct(struct_def, _) | CalcitTypeAnnotation::StructValue(struct_def) => struct_def,
    _ => return None,
  };
  if inferred_struct.impls.is_empty() {
    return None;
  }

  let mut declared_struct = declared.resolve_to_struct()?;
  if declared_struct.name != inferred_struct.name
    || declared_struct.fields != inferred_struct.fields
    || declared_struct.field_types != inferred_struct.field_types
    || declared_struct.generics != inferred_struct.generics
    || declared_struct.where_bounds != inferred_struct.where_bounds
  {
    return None;
  }
  declared_struct.impls = inferred_struct.impls.clone();

  match declared.as_ref() {
    CalcitTypeAnnotation::TypeRef(_, args) | CalcitTypeAnnotation::Struct(_, args) => {
      Some(Arc::new(CalcitTypeAnnotation::Struct(Arc::new(declared_struct), args.clone())))
    }
    CalcitTypeAnnotation::StructValue(_) => Some(Arc::new(CalcitTypeAnnotation::StructValue(Arc::new(declared_struct)))),
    _ => None,
  }
}

/// Recover the result of `apply f args` from the concrete callable contract.
///
/// A homogeneous `List<T>` can satisfy the spread only when `T` matches every
/// fixed/rest input in the callable. If that proof is unavailable, keep the
/// public compatibility schema's Dynamic return instead of guessing.
fn infer_core_apply_return_type(call_expr: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  let callable_type = call_expr.get(1).and_then(|callable| resolve_type_value(callable, scope_types))?;
  let arguments = call_expr.get(2)?;
  let arguments_type = resolve_type_value(arguments, scope_types)?;
  let signature = callable_type.resolve_to_nonoptional_fn()?;
  let CalcitTypeAnnotation::List(item_type) = arguments_type.as_ref() else {
    return None;
  };
  if !signature.where_bounds.is_empty() || matches!(item_type.as_ref(), CalcitTypeAnnotation::Dynamic) {
    return None;
  }

  let known_length = match arguments {
    Calcit::List(items) if items.first().is_some_and(is_list_constructor) => Some(items.len().saturating_sub(1)),
    _ => None,
  };
  match (&signature.rest_type, known_length) {
    (None, Some(length)) if length == signature.arg_types.len() => {}
    (None, _) => return None,
    (Some(_), Some(length)) if length >= signature.arg_types.len() => {}
    (Some(_), None) if signature.arg_types.is_empty() => {}
    (Some(_), _) => return None,
  }

  let mut bindings = HashMap::new();
  for expected in signature.arg_types.iter().chain(signature.rest_type.iter()) {
    let mut candidate = bindings.clone();
    if !item_type
      .as_ref()
      .prove_with_bindings(expected.as_ref(), &mut candidate)
      .is_proven()
    {
      return None;
    }
    bindings = candidate;
  }
  for generic in signature.generics.iter() {
    bindings.entry(generic.clone()).or_insert_with(|| calcit::DYNAMIC_TYPE.clone());
  }
  let resolved = signature.return_type.substitute_type_vars(&bindings);
  (!resolved.contains_type_var()).then(|| invocation_return_type(signature.as_ref(), resolved, false))
}

fn core_type_ref(name: &str, args: Vec<Arc<CalcitTypeAnnotation>>) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::TypeRef(
    Arc::from(format!("{}/{name}", calcit::CORE_NS)),
    Arc::new(args),
  ))
}

fn infer_core_nominal_absence_return_type(
  def: &str,
  call_expr: &CalcitList,
  scope_types: &ScopeTypes,
) -> Option<Arc<CalcitTypeAnnotation>> {
  match def {
    "find" => {
      let element_type = call_expr
        .get(1)
        .and_then(|value| resolve_type_value(value, scope_types))
        .and_then(|value_type| match value_type.as_ref() {
          CalcitTypeAnnotation::List(inner) => Some(inner.clone()),
          _ => None,
        })
        .unwrap_or_else(|| calcit::DYNAMIC_TYPE.clone());
      Some(core_type_ref("Option", vec![element_type]))
    }
    "find-index" | "index-of" => Some(core_type_ref("Option", vec![tag_annotation("number")])),
    "parse-float" => Some(core_type_ref("Result", vec![tag_annotation("number"), tag_annotation("string")])),
    "get-env" => Some(core_type_ref("Option", vec![tag_annotation("string")])),
    _ => None,
  }
}

fn infer_core_get_in_return_type(call_expr: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  let base_arg = call_expr.get(1)?;
  let path_arg = call_expr.get(2)?;
  let path_items = extract_literal_list_items(path_arg)?;
  let mut current_type = resolve_type_value(base_arg, scope_types)?;

  if path_items.is_empty() {
    return Some(core_type_ref("Option", vec![current_type]));
  }

  for key in path_items {
    // A path API must not become an alternate, Option-producing Struct field
    // accessor. Stop inference at the same boundary that preprocessing and the
    // runtime reject, so adding/removing type evidence never changes get-in's
    // contract.
    if is_struct_lookup_boundary(current_type.as_ref()) {
      return None;
    }
    current_type = infer_lookup_payload_type_from_type(current_type.as_ref(), Some(key))?;
  }

  Some(core_type_ref("Option", vec![current_type]))
}

fn is_struct_lookup_boundary(type_info: &CalcitTypeAnnotation) -> bool {
  type_info.resolve_to_struct().is_some()
    || matches!(
      type_info,
      CalcitTypeAnnotation::Custom(value)
        if matches!(value.as_ref(), Calcit::Tag(tag) if matches!(tag.ref_str().trim_start_matches(':'), "record" | "struct"))
    )
}

/// Return the first literal path segment that would enter a Struct. Public
/// path APIs use this to stop before nominal fields and keep required field
/// access visible in source.
pub(super) fn find_struct_lookup_in_literal_path(base_type: &CalcitTypeAnnotation, path_arg: &Calcit) -> Option<(usize, Calcit)> {
  let path_items = extract_literal_list_items(path_arg)?;
  let mut current_type = Arc::new(base_type.to_owned());

  for (index, key) in path_items.into_iter().enumerate() {
    if is_struct_lookup_boundary(current_type.as_ref()) {
      return Some((index, key.to_owned()));
    }
    current_type = infer_lookup_payload_type_from_type(current_type.as_ref(), Some(key))?;
  }
  None
}

/// Return a non-empty literal lookup path when every receiver needed for a
/// further hop remains statically known. The final payload may be Dynamic
/// because it is returned rather than traversed. A Struct boundary always
/// stays on the public fallback.
pub(super) fn fully_typed_literal_lookup_path(base_type: &CalcitTypeAnnotation, path_arg: &Calcit) -> Option<Vec<Calcit>> {
  let path_items = extract_literal_list_items(path_arg)?;
  if path_items.is_empty() || matches!(base_type, CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::DynFn) {
    return None;
  }

  let mut current_type = Arc::new(base_type.to_owned());
  for key in &path_items {
    if is_struct_lookup_boundary(current_type.as_ref())
      || matches!(current_type.as_ref(), CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::DynFn)
    {
      return None;
    }
    current_type = infer_lookup_payload_type_from_type(current_type.as_ref(), Some(key))?;
  }

  Some(path_items.into_iter().cloned().collect())
}

/// The first expansion phase limits `assoc-in` to map-only chains. Missing map
/// entries are also represented by fresh maps, which means direct map
/// primitives preserve the public construction behavior. The final payload
/// must also be non-Dynamic; lists, strings and enum payloads need a
/// mixed-container lowering and stay on the fallback.
pub(super) fn fully_typed_literal_assoc_path(base_type: &CalcitTypeAnnotation, path_arg: &Calcit) -> Option<Vec<Calcit>> {
  let path_items = extract_literal_list_items(path_arg)?;
  if path_items.is_empty() {
    return None;
  }

  let mut current_type = Arc::new(base_type.to_owned());
  for key in &path_items {
    if !matches!(current_type.as_ref(), CalcitTypeAnnotation::Map(_, _)) {
      return None;
    }
    current_type = infer_lookup_payload_type_from_type(current_type.as_ref(), Some(key))?;
  }

  (!matches!(current_type.as_ref(), CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::DynFn))
    .then(|| path_items.into_iter().cloned().collect())
}

fn infer_lookup_payload_type_from_type(
  base_type: &CalcitTypeAnnotation,
  key_arg: Option<&Calcit>,
) -> Option<Arc<CalcitTypeAnnotation>> {
  match base_type {
    CalcitTypeAnnotation::List(element_type) => Some(element_type.clone()),
    CalcitTypeAnnotation::Map(_, value_type) => Some(value_type.clone()),
    CalcitTypeAnnotation::String => Some(tag_annotation("string")),
    CalcitTypeAnnotation::EnumValue(_) | CalcitTypeAnnotation::AnonymousEnum => Some(calcit::DYNAMIC_TYPE.clone()),
    CalcitTypeAnnotation::StructValue(_) | CalcitTypeAnnotation::Struct(_, _) | CalcitTypeAnnotation::TypeRef(_, _) => {
      if let Some(field_name) = key_arg.and_then(extract_field_name)
        && let Some(field_type) = resolve_struct_field_type(base_type, field_name)
      {
        // A statically known struct field is always present. Preserve its
        // declared type instead of applying the conservative map lookup rule.
        return Some(field_type);
      }
      Some(calcit::DYNAMIC_TYPE.clone())
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

fn js_host_value_type() -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::JsObject)
}

fn js_nullish_host_value_type() -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::JsNullish(js_host_value_type()))
}

fn infer_external_field_type(base_type: &CalcitTypeAnnotation, key_arg: Option<&Calcit>) -> Option<Arc<CalcitTypeAnnotation>> {
  let field_name = key_arg.and_then(extract_field_name)?;
  let traits = trait_list_from_type(base_type)?;
  let (trait_def, field_type) = find_trait_field_type(&traits, field_name)?;
  trait_is_external_object(trait_def).then(|| field_type.clone())
}

fn infer_js_ffi_call_return_type(name: &str, call_expr: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  match name {
    // JavaScript's `typeof` operator always produces one of the specified
    // string tags. Unlike arbitrary host calls, its result is neither opaque
    // nor nullish, even when the inspected value is null or undefined.
    "js/typeof" => Some(tag_annotation("string")),
    // JavaScript indexed/property reads may yield `undefined` or `null`. Keep
    // the raw host value opaque as well as nullable so a nil check alone does
    // not silently prove that it is a Calcit Number/String/etc.
    "js-get" => {
      let field_type = call_expr
        .get(1)
        .and_then(|base| resolve_type_value(base, scope_types))
        .and_then(|base_type| infer_external_field_type(base_type.as_ref(), call_expr.get(2)));
      Some(
        field_type
          .map(|field_type| Arc::new(CalcitTypeAnnotation::JsNullish(field_type)))
          .unwrap_or_else(js_nullish_host_value_type),
      )
    }
    "aget" => Some(js_nullish_host_value_type()),
    // Assignment expressions evaluate to the assigned value in JavaScript.
    "aset" | "js-set" => call_expr
      .get(3)
      .and_then(|value| resolve_type_value(value, scope_types))
      .or_else(|| Some(js_host_value_type())),
    "js-delete" | "exists?" | "instance?" => Some(tag_annotation("bool")),
    "&js-object" | "js-array" | "new" => Some(js_host_value_type()),
    _ if name.starts_with("js/") => Some(js_nullish_host_value_type()),
    _ => None,
  }
}

pub(super) fn extract_literal_list_items(form: &Calcit) -> Option<Vec<&Calcit>> {
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

/// Expand only source-owned literal spreads without evaluating any expression.
pub(super) fn expand_literal_call_arguments(args: &CalcitList) -> Result<CalcitList, Option<&Calcit>> {
  let mut expanded = Vec::new();
  let mut arguments = args.iter();
  while let Some(argument) = arguments.next() {
    if !matches!(argument, Calcit::Syntax(CalcitSyntax::ArgSpread, _)) {
      expanded.push(argument.clone());
      continue;
    }
    let operand = arguments.next();
    let items = operand.and_then(extract_literal_list_items).ok_or(operand)?;
    if items.iter().any(|item| matches!(item, Calcit::Syntax(CalcitSyntax::ArgSpread, _))) {
      return Err(operand);
    }
    expanded.extend(items.into_iter().cloned());
  }
  Ok(CalcitList::from(expanded.as_slice()))
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
  let annotation = infer_expression_type(expr, scope_types)?;
  let arguments = match annotation.as_ref() {
    CalcitTypeAnnotation::Enum(_, args) | CalcitTypeAnnotation::TypeRef(_, args)
      if args.iter().any(|arg| matches!(arg.as_ref(), CalcitTypeAnnotation::Dynamic)) =>
    {
      args
    }
    _ => return Some(annotation),
  };
  let Some(definition) = annotation.resolve_to_enum() else {
    return Some(annotation);
  };
  let Some(used) = enum_constructor_type_participation(expr, &definition, scope_types) else {
    return Some(annotation);
  };
  if definition.generics().len() != arguments.len() {
    return Some(annotation);
  }
  let completed = arguments
    .iter()
    .zip(definition.generics())
    .map(|(actual, name)| {
      if matches!(actual.as_ref(), CalcitTypeAnnotation::Dynamic) && !used.contains(name) {
        crate::calcit::type_annotation::NEVER_TYPE.clone()
      } else {
        actual.clone()
      }
    })
    .collect();
  Some(Arc::new(match annotation.as_ref() {
    CalcitTypeAnnotation::Enum(definition, _) => CalcitTypeAnnotation::Enum(definition.clone(), Arc::new(completed)),
    CalcitTypeAnnotation::TypeRef(name, _) => CalcitTypeAnnotation::TypeRef(name.clone(), Arc::new(completed)),
    _ => unreachable!(),
  }))
}

fn infer_expression_type(expr: &Calcit, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  match expr {
    // Literal types
    Calcit::Number(_) => Some(tag_annotation("number")),
    Calcit::Str(_) => Some(tag_annotation("string")),
    Calcit::Bool(_) => Some(tag_annotation("bool")),
    Calcit::Nil => Some(tag_annotation("nil")),
    Calcit::Unit => Some(tag_annotation("unit")),
    Calcit::Tag(_) => Some(tag_annotation("tag")),
    Calcit::Map(values) => Some(Arc::new(CalcitTypeAnnotation::Map(
      infer_homogeneous_type(values.iter().map(|(key, _)| key), scope_types),
      infer_homogeneous_type(values.iter().map(|(_, value)| value), scope_types),
    ))),
    Calcit::Set(values) => Some(Arc::new(CalcitTypeAnnotation::Set(infer_homogeneous_type(
      values.iter(),
      scope_types,
    )))),
    Calcit::Enum(enum_value) => match &enum_value.sum_type {
      Some(enum_def) => Some(Arc::new(CalcitTypeAnnotation::EnumValue(enum_def.clone()))),
      None => Some(Arc::new(CalcitTypeAnnotation::AnonymousEnum)),
    },
    Calcit::Struct(struct_value) => Some(infer_struct_value_annotation(struct_value, scope_types)),
    Calcit::StructDef(struct_def) => Some(Arc::new(CalcitTypeAnnotation::StructDef(Arc::new(struct_def.to_owned())))),
    Calcit::EnumDef(enum_def) => Some(Arc::new(CalcitTypeAnnotation::EnumDef(Arc::new(enum_def.to_owned())))),
    Calcit::Trait(trait_def) => Some(Arc::new(CalcitTypeAnnotation::Trait(Arc::new(trait_def.to_owned())))),
    // Impl values carry useful compile-time method metadata. `Custom` is the
    // existing annotation representation for first-class runtime metadata;
    // keeping the concrete value here is strictly more precise than `:impl`.
    Calcit::Impl(impl_def) => Some(Arc::new(CalcitTypeAnnotation::Custom(Arc::new(Calcit::Impl(impl_def.to_owned()))))),
    Calcit::Ref(..) => Some(Arc::new(CalcitTypeAnnotation::Ref(calcit::DYNAMIC_TYPE.clone()))),
    Calcit::Buffer(_) => Some(Arc::new(CalcitTypeAnnotation::Buffer)),
    Calcit::CirruQuote(_) => Some(Arc::new(CalcitTypeAnnotation::CirruQuote)),
    Calcit::Fn { info, .. } => Some(resolve_namespace_type_refs_for_body(
      Arc::new(CalcitTypeAnnotation::from_calcit_fn(info)),
      &info.def_ns,
    )),
    Calcit::Proc(proc) => proc
      .get_type_signature()
      .map(|signature| {
        Arc::new(CalcitTypeAnnotation::from_proc_parts(
          signature.arg_types.clone(),
          signature.return_type.clone(),
        ))
      })
      .or_else(|| Some(tag_annotation("fn"))),

    Calcit::Import(CalcitImport { info, .. }) if matches!(info.as_ref(), ImportInfo::JsDefault { .. }) => Some(js_host_value_type()),
    Calcit::Import(CalcitImport { ns, def, .. }) => infer_definition_value_type(ns, def),
    Calcit::Symbol { sym, .. } if sym.starts_with("js/") => Some(js_nullish_host_value_type()),
    Calcit::Symbol { sym, info, .. } => scope_types
      .get(sym)
      .cloned()
      .or_else(|| infer_definition_value_type(&info.at_ns, sym)),
    Calcit::RawCode(..) => Some(js_nullish_host_value_type()),

    // Local variable: read type_info
    Calcit::Local(local) => Some(local.type_info.clone()),

    // List/vector literal or expressions
    Calcit::List(xs) if xs.is_empty() => Some(tag_annotation("list")),

    // Function call or Proc call or special forms
    Calcit::List(xs) => {
      let head = xs.first()?;
      match head {
        Calcit::Syntax(CalcitSyntax::CallSpread, _) => {
          let callable = xs.get(1)?;
          let arguments = xs.drop_left().drop_left();
          if let Ok(expanded) = expand_literal_call_arguments(&arguments) {
            return infer_type_from_expr(&Calcit::from(expanded.push_left(callable.clone())), scope_types);
          }
          let annotation = resolve_type_value(callable, scope_types)?;
          let CalcitTypeAnnotation::Fn(signature) = annotation.as_ref() else {
            return None;
          };
          let (projected, contract) = super::typed_rest_spread_contract(signature, &arguments)?;
          let actual_types = projected
            .iter()
            .map(|argument| resolve_type_value(argument, scope_types))
            .collect::<Option<Vec<_>>>()?;
          let mut expected_types = contract.arg_types.clone();
          expected_types.push(contract.return_type.clone());
          let mut call_proof = crate::calcit::type_annotation::CallTypeProof::new(&contract.generics, &expected_types, &actual_types);
          for (argument, expected) in projected.iter().zip(&contract.arg_types) {
            if !call_proof
              .prove(resolve_type_value(argument, scope_types)?.as_ref(), expected)
              .is_proven()
            {
              return None;
            }
          }
          let logical_return = call_proof.result(&contract.return_type)?;
          Some(invocation_return_type(&contract, logical_return, false))
        }
        // Hints refine the surrounding function or a local binding, but the
        // hint expression itself returns Nil; it never wraps a function value.
        Calcit::Syntax(CalcitSyntax::HintFn, _) => Some(tag_annotation("nil")),

        // &let expression: infer from final expression (last element)
        Calcit::Syntax(CalcitSyntax::CoreLet, _) => {
          // &let has format: (&let (binding) body...)
          // The last element is the return value
          if xs.len() > 1 {
            resolve_type_value(&xs[xs.len() - 1], &core_let_scope(xs, scope_types))
          } else {
            None
          }
        }
        Calcit::Syntax(CalcitSyntax::If, _) => infer_if_return_type(xs, scope_types),
        Calcit::Syntax(CalcitSyntax::Try, _) => infer_try_return_type(xs, scope_types),
        Calcit::Syntax(CalcitSyntax::Match, _) => infer_match_return_type(xs, scope_types),

        // A preprocessed function remains a syntax list until runtime construction. Preserve an
        // explicit body `hint-fn` as its static value type; without a schema we only know that the
        // value is callable and deliberately keep its argument/return details dynamic.
        Calcit::Syntax(CalcitSyntax::Defn | CalcitSyntax::Defmacro | CalcitSyntax::DefWasmExport | CalcitSyntax::DefWasmImport, _) => {
          Some(infer_preprocessed_function_type(xs))
        }

        // A `defatom` expression evaluates to the reference that it defines.
        // Preserve the initializer type so imported atoms and receiver-first
        // `.deref` calls do not lose `Ref<T>` at the definition boundary.
        Calcit::Syntax(CalcitSyntax::Defref, _) => xs
          .get(2)
          .and_then(|initial_value| infer_type_from_expr(initial_value, scope_types))
          .map(|initial_type| Arc::new(CalcitTypeAnnotation::Ref(initial_type))),

        // reset! evaluates the target first and returns the assigned value,
        // not Unit or the target's declared payload. Resolve the input through
        // the normal evidence path so an unsafe cast cannot prove itself.
        Calcit::Syntax(CalcitSyntax::Reset, _) if xs.len() == 3 => {
          if expression_definitely_diverges(&xs[1]) || expression_definitely_diverges(&xs[2]) {
            return Some(crate::calcit::type_annotation::NEVER_TYPE.clone());
          }
          if let Some(target_type) = resolve_type_value(&xs[1], scope_types)
            && matches!(target_type.as_ref(), CalcitTypeAnnotation::Never)
          {
            return Some(target_type);
          }
          resolve_type_value(&xs[2], scope_types)
        }

        // Assertion and coercion type expressions have no local generics, so names are concrete refs.
        // `assert-type` is erased for local bindings during preprocessing, but when it wraps an
        // arbitrary expression (notably on the right-hand side of `let`) the expression must keep
        // its declared type for the new local binding and later field/method checks.
        // A trusted coercion changes the ordinary static contract, but it is
        // not independent evidence in a proof audit. Retain the input evidence
        // so locals and producer returns cannot lend the cast its own proof.
        Calcit::Syntax(CalcitSyntax::UnsafeCoerce, _) if super::REQUIRE_ASSERTION_PROOF.with(std::cell::Cell::get) => {
          xs.get(1).and_then(|input| resolve_type_value(input, scope_types))
        }
        Calcit::Syntax(CalcitSyntax::AssertType | CalcitSyntax::UnsafeCoerce | CalcitSyntax::JsCast, _) => xs
          .get(2)
          .map(|form| CalcitTypeAnnotation::parse_type_annotation_form_with_generics(form, &[]))
          .map(resolve_program_trait_refs_for_body),

        Calcit::Syntax(CalcitSyntax::ParseCirruEdnAs | CalcitSyntax::DecodeMapAs, _) => xs
          .get(2)
          .map(|form| CalcitTypeAnnotation::parse_type_annotation_form_with_generics(form, &[])),

        Calcit::Syntax(CalcitSyntax::TryParseCirruEdnAs | CalcitSyntax::TryDecodeMapAs, _) => xs.get(2).map(|form| {
          let target = CalcitTypeAnnotation::parse_type_annotation_form_with_generics(form, &[]);
          Arc::new(CalcitTypeAnnotation::TypeRef(
            Arc::from("calcit.core/Result"),
            Arc::new(vec![target, Arc::new(CalcitTypeAnnotation::String)]),
          ))
        }),

        // Local variable as head (function call)
        // If it's a function type, return its return type
        Calcit::Local(local) => {
          let type_ann = &local.type_info;
          if let Some(fn_type) = type_ann.resolve_to_nonoptional_fn() {
            let returned = resolve_generic_return_type_parts(
              &fn_type.generics,
              &fn_type.arg_types,
              fn_type.rest_type.as_ref(),
              &fn_type.return_type,
              xs.iter().skip(1),
              scope_types,
            )
            .or_else(|| {
              (fn_type.generics.is_empty() || !fn_type.return_type.contains_type_var()).then(|| fn_type.return_type.clone())
            })?;
            Some(invocation_return_type(&fn_type, returned, false))
          } else {
            match type_ann.as_ref() {
              CalcitTypeAnnotation::DynFn => Some(calcit::DYNAMIC_TYPE.clone()),
              _ => Some(type_ann.clone()),
            }
          }
        }

        // Proc call: check if proc has return_type
        Calcit::Proc(proc) => infer_proc_call_return_type(proc, xs, scope_types),

        // Import: could be a function, try to get its return type
        Calcit::Import(CalcitImport { ns, def, .. }) => {
          if ns.as_ref() == calcit::CORE_NS && def.as_ref() == "js-await" {
            return xs
              .get(1)
              .and_then(|value| resolve_type_value(value, scope_types))
              .map(awaited_async_value);
          }
          if &**ns == calcit::CORE_NS
            && (&**def == "record-get" || &**def == "&struct:get")
            && let Some(field_type) = infer_struct_get_type(xs, scope_types)
          {
            return Some(field_type);
          }
          infer_return_type_from_compiled_callable(ns, def, xs, scope_types)
        }

        // Symbol: might be a function reference before preprocessing
        // Try to resolve it and get the return type
        Calcit::Symbol { sym, info, .. } => {
          if sym.as_ref() == "js-await" {
            return xs
              .get(1)
              .and_then(|value| resolve_type_value(value, scope_types))
              .map(awaited_async_value);
          }
          if let Some(inferred) = infer_js_ffi_call_return_type(sym, xs, scope_types) {
            return Some(inferred);
          }

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

        Calcit::RawCode(calcit::RawCodeType::Js, code) if code.as_ref() == "typeof" => Some(tag_annotation("string")),
        Calcit::RawCode(..) => Some(js_nullish_host_value_type()),

        // Direct Fn call: return the function's return type
        Calcit::Fn { info, .. } => {
          if info.def_ns.as_ref() == calcit::CORE_NS
            && let Some(contract) = resolve_checked_call_contract(&info.def_ns, &info.name, &xs.drop_left(), scope_types)
            && !contract.return_type.contains_type_var()
          {
            return Some(contract.return_type);
          }
          if info.def_ns.as_ref() == calcit::CORE_NS
            && let Some(inferred) = infer_core_nominal_absence_return_type(info.name.as_ref(), xs, scope_types)
          {
            return Some(inferred);
          }
          if info.return_type.contains_type_var()
            && let Some(resolved) = resolve_generic_return_type(info, xs.iter().skip(1), scope_types)
          {
            return Some(if definition_marks_async(&info.def_ns, &info.name) {
              pending_async_value(resolved)
            } else {
              resolved
            });
          }
          Some(if definition_marks_async(&info.def_ns, &info.name) {
            pending_async_value(info.return_type.clone())
          } else {
            info.return_type.clone()
          })
        }

        // Typed method invocation: resolve the selected trait signature and
        // substitute receiver/argument types into its generic return type.
        Calcit::Method(method_name, calcit::MethodKind::Invoke(receiver_hint) | calcit::MethodKind::ExternalInvoke(receiver_hint)) => {
          let receiver_type = xs
            .get(1)
            .and_then(|receiver| resolve_type_value(receiver, scope_types))
            .unwrap_or_else(|| receiver_hint.clone());
          let method_type = if let Some(traits) = trait_list_from_type(receiver_type.as_ref()) {
            selected_trait_method(&traits, method_name).map(|candidate| candidate.method_type)?
          } else {
            let impls = get_impls_from_type(receiver_type.as_ref())?;
            let method = find_method_entry_for_type(receiver_type.as_ref(), &impls, method_name)?;
            infer_type_from_expr(method, scope_types)?
          };
          let CalcitTypeAnnotation::Fn(info) = method_type.as_ref() else {
            return Some(calcit::DYNAMIC_TYPE.clone());
          };
          Some(infer_typed_method_result(info, xs.iter().skip(1), scope_types))
        }

        // Method access: infer struct field type when available
        Calcit::Method(
          field_name,
          calcit::MethodKind::Access
          | calcit::MethodKind::TagAccess
          | calcit::MethodKind::ExternalAccess(_)
          | calcit::MethodKind::ExternalGet(_),
        ) => {
          if let Some(receiver) = xs.get(1)
            && let Some(field_type) = infer_struct_field_type(receiver, field_name.as_ref(), scope_types)
          {
            return Some(field_type);
          }
          if let Some(receiver) = xs.get(1)
            && let Some(receiver_type) = resolve_type_value(receiver, scope_types)
            && let Some(traits) = trait_list_from_type(receiver_type.as_ref())
            && let Some((trait_def, field_type)) = find_trait_field_type(&traits, field_name.as_ref())
            && trait_is_external_object(trait_def)
          {
            return Some(if matches!(head, Calcit::Method(_, calcit::MethodKind::ExternalGet(_))) {
              Arc::new(CalcitTypeAnnotation::JsNullish(field_type.clone()))
            } else {
              field_type.clone()
            });
          }
          match head {
            Calcit::Method(_, calcit::MethodKind::Access) => Some(js_nullish_host_value_type()),
            _ => None,
          }
        }
        Calcit::Method(_, calcit::MethodKind::ExternalSet(_)) => xs.get(2).and_then(|value| resolve_type_value(value, scope_types)),

        // Native JavaScript access/calls remain explicit JsNullish boundary values,
        // not legacy Optional or nominal Option values. The opaque JsObject payload
        // prevents an unchecked host value from matching strong Calcit types.
        Calcit::Method(field_name, calcit::MethodKind::AccessOptional) => {
          if let Some(receiver) = xs.get(1)
            && let Some(field_type) = infer_struct_field_type(receiver, field_name.as_ref(), scope_types)
          {
            return Some(wrap_optional_type(field_type));
          }
          Some(js_nullish_host_value_type())
        }
        Calcit::Method(_, calcit::MethodKind::InvokeNative | calcit::MethodKind::InvokeNativeOptional) => {
          Some(js_nullish_host_value_type())
        }

        // Nested List call: the head is a function call expression
        // First infer what type the head returns, then if it's a function, get its return type
        Calcit::List(_) => {
          if let Some(head_type) = infer_type_from_expr(head, scope_types) {
            if let Some(fn_type) = head_type.resolve_to_nonoptional_fn() {
              let returned = resolve_generic_return_type_parts(
                &fn_type.generics,
                &fn_type.arg_types,
                fn_type.rest_type.as_ref(),
                &fn_type.return_type,
                xs.iter().skip(1),
                scope_types,
              )
              .or_else(|| {
                (fn_type.generics.is_empty() || !fn_type.return_type.contains_type_var()).then(|| fn_type.return_type.clone())
              })?;
              Some(invocation_return_type(&fn_type, returned, false))
            } else {
              match head_type.as_ref() {
                CalcitTypeAnnotation::DynFn => Some(calcit::DYNAMIC_TYPE.clone()),
                // If head returns a non-function type, the call will fail at runtime
                // Return the non-callable type so caller can detect this issue
                _ => Some(head_type),
              }
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
  let is_async = xs.iter().skip(3).any(CalcitTypeAnnotation::hint_form_marks_async);
  let (parameter_types, inferred_rest_type) = infer_preprocessed_function_parameters(xs.get(2));
  let lexical_generics = crate::calcit::type_annotation::lexical_type_variables_in_forms(xs.iter().skip(2));
  let hinted = xs
    .iter()
    .skip(3)
    .find_map(|form| CalcitTypeAnnotation::extract_surrounding_fn_annotation_from_hint_form_in_scope(form, &lexical_generics));
  let Some(hinted) = hinted else {
    // Preserve independently inferred output when every lexical input already
    // carries evidence, including call-site parameter contexts. Open inputs
    // remain open; an expected return declaration is never output evidence.
    let Some(inferred) = infer_unhinted_callback_signature(xs, &ScopeTypes::new()) else {
      return Arc::new(CalcitTypeAnnotation::DynFn);
    };
    let CalcitTypeAnnotation::Fn(signature) = inferred.as_ref() else {
      return Arc::new(CalcitTypeAnnotation::DynFn);
    };
    if signature.rest_type.is_some()
      || signature
        .arg_types
        .iter()
        .any(|parameter| super::contains_dynamic_type(parameter.as_ref()))
    {
      return Arc::new(CalcitTypeAnnotation::DynFn);
    }
    return mark_async_callable(inferred, is_async);
  };
  let CalcitTypeAnnotation::Fn(fn_annotation) = hinted.as_ref() else {
    if is_async && let Some(inferred) = infer_unhinted_callback_signature(xs, &ScopeTypes::new()) {
      return mark_async_callable(inferred, true);
    }
    return mark_async_callable(hinted, is_async);
  };

  // Contextual metadata cannot invent lexical parameters. In particular, a
  // zero-argument callback does not become unary because its callee expects
  // a String input. Keep the actual fixed call shape alongside its evidence.
  let mut arg_types = fn_annotation
    .arg_types
    .iter()
    .take(parameter_types.len())
    .cloned()
    .collect::<Vec<_>>();
  for parameter_type in parameter_types.iter().skip(arg_types.len()) {
    arg_types.push(parameter_type.clone());
  }

  let fn_kind = match xs.first() {
    Some(Calcit::Syntax(CalcitSyntax::Defmacro, _)) => SchemaKind::Macro,
    _ => fn_annotation.fn_kind,
  };

  mark_async_callable(
    Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      generics: fn_annotation.generics.clone(),
      where_bounds: fn_annotation.where_bounds.clone(),
      arg_types,
      return_type: fn_annotation.return_type.clone(),
      fn_kind,
      rest_type: fn_annotation.rest_type.clone().or(inferred_rest_type),
      features: fn_annotation.features.clone(),
    }))),
    is_async,
  )
}

/// Recover a concrete signature from an already preprocessed anonymous
/// callback when its body has a statically known final expression. Callers
/// choose whether using the recovered signature is appropriate for their
/// contract; ordinary unhinted functions remain dynamic by default.
pub(crate) fn infer_unhinted_callback_signature(xs: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  let fn_kind = match xs.first()? {
    Calcit::Syntax(CalcitSyntax::Defmacro, _) => SchemaKind::Macro,
    Calcit::Syntax(CalcitSyntax::Defn | CalcitSyntax::DefWasmExport | CalcitSyntax::DefWasmImport, _) => SchemaKind::Fn,
    _ => return None,
  };
  if xs.len() <= 3
    || xs.iter().skip(3).any(|form| {
      matches!(
        CalcitTypeAnnotation::extract_surrounding_fn_annotation_from_hint_form(form).as_deref(),
        Some(CalcitTypeAnnotation::Fn(_))
      )
    })
  {
    return None;
  }
  if let Some(Calcit::List(params)) = xs.get(2)
    && params
      .iter()
      .any(|param| matches!(param, Calcit::Syntax(CalcitSyntax::ArgOptional | CalcitSyntax::ArgSpread, _)))
  {
    // Function annotations do not retain optional-parameter call shapes.
    // Recovering these callbacks as fixed arity would reject valid shorthand
    // lambdas that accept an optional second argument.
    return None;
  }
  let (arg_types, rest_type) = infer_preprocessed_function_parameters(xs.get(2));
  let return_type = infer_type_from_expr(xs.get(xs.len() - 1)?, scope_types)
    .filter(|annotation| !matches!(annotation.as_ref(), CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::DynFn))?;
  Some(Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
    generics: Arc::new(vec![]),
    where_bounds: Arc::new(vec![]),
    arg_types,
    return_type,
    fn_kind,
    rest_type,
    features: Arc::new(std::collections::HashSet::new()),
  }))))
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

fn resolve_impl_origin(value: &Calcit, scope_types: &ScopeTypes) -> Option<(EdnTag, Option<Arc<CalcitTrait>>)> {
  let resolved_trait = match value {
    Calcit::Trait(trait_def) => Some(Arc::new(trait_def.to_owned())),
    Calcit::Import(import) => match resolve_program_value_for_preprocess(&import.ns, &import.def, import.def_id) {
      Some(Calcit::Trait(trait_def)) => Some(Arc::new(trait_def)),
      _ => None,
    },
    Calcit::Symbol { sym, info, .. } => match resolve_program_value_for_preprocess(&info.at_ns, sym, None) {
      Some(Calcit::Trait(trait_def)) => Some(Arc::new(trait_def)),
      _ => None,
    },
    Calcit::Local(_) => match resolve_type_value(value, scope_types).as_deref() {
      Some(CalcitTypeAnnotation::Trait(trait_def)) => Some(trait_def.clone()),
      _ => None,
    },
    _ => None,
  };
  if let Some(trait_def) = resolved_trait {
    return Some((trait_def.name.to_owned(), Some(trait_def)));
  }

  match value {
    Calcit::Tag(name) => Some((name.to_owned(), None)),
    Calcit::Str(name) | Calcit::Symbol { sym: name, .. } => Some((EdnTag(name.to_owned()), None)),
    _ => None,
  }
}

fn is_enum_constructor(value: &Calcit) -> bool {
  matches!(value, Calcit::Proc(CalcitProc::NativeEnum))
    || matches!(value, Calcit::Import(CalcitImport { ns, def, .. }) if ns.as_ref() == calcit::CORE_NS && def.as_ref() == "::")
    || matches!(value, Calcit::Symbol { sym, .. } if sym.as_ref() == "::")
}

fn is_list_constructor(value: &Calcit) -> bool {
  matches!(value, Calcit::Proc(CalcitProc::List))
    || matches!(value, Calcit::Import(CalcitImport { ns, def, .. }) if ns.as_ref() == calcit::CORE_NS && def.as_ref() == "[]")
    || matches!(value, Calcit::Symbol { sym, .. } if sym.as_ref() == "[]")
}

fn extract_impl_pair(value: &Calcit) -> Option<(&Calcit, &Calcit)> {
  match value {
    Calcit::Enum(enum_value) if enum_value.extra.len() == 1 => Some((enum_value.tag.as_ref(), enum_value.extra.first()?)),
    Calcit::List(items) if items.len() == 2 => Some((items.first()?, items.get(1)?)),
    Calcit::List(items)
      if items.len() == 3
        && items
          .first()
          .is_some_and(|head| is_enum_constructor(head) || is_list_constructor(head)) =>
    {
      Some((items.get(1)?, items.get(2)?))
    }
    _ => None,
  }
}

fn extract_impl_field_name(value: &Calcit) -> Option<EdnTag> {
  match value {
    Calcit::Method(name, _) | Calcit::Str(name) | Calcit::Symbol { sym: name, .. } => Some(EdnTag(name.to_owned())),
    Calcit::Tag(name) => Some(name.to_owned()),
    _ => None,
  }
}

fn normalize_static_metadata_form(value: &Calcit) -> Calcit {
  match value {
    Calcit::List(items) if matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::Quote, _))) && items.len() == 2 => {
      normalize_static_metadata_form(items.get(1).expect("quoted metadata value"))
    }
    Calcit::List(items) => {
      let skip_head = items.first().is_some_and(|head| {
        matches!(head, Calcit::Proc(CalcitProc::List))
          || matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "[]")
          || matches!(head, Calcit::Import(CalcitImport { ns, def, .. }) if ns.as_ref() == calcit::CORE_NS && def.as_ref() == "[]")
      });
      let normalized = items
        .iter()
        .skip(usize::from(skip_head))
        .map(normalize_static_metadata_form)
        .collect::<Vec<_>>();
      Calcit::List(Arc::new(CalcitList::from(normalized.as_slice())))
    }
    _ => value.to_owned(),
  }
}

fn infer_trait_value(xs: &CalcitList) -> Option<CalcitTrait> {
  let mut args = xs.iter().skip(1).map(normalize_static_metadata_form).collect::<Vec<_>>();
  // Parents are evaluated trait values at runtime; statically, resolve each
  // imported reference to its source-backed trait with a definition origin.
  if let Some(Calcit::List(parents)) = xs.get(3) {
    let resolved = parents
      .iter()
      .filter(|item| !matches!(item, Calcit::Proc(CalcitProc::List)))
      .map(|item| match item {
        Calcit::Import(import) => lookup_source_backed_trait_def(&import.ns, &import.def)
          .map(|trait_def| Calcit::Trait(trait_def.with_definition_ref(&import.ns, &import.def))),
        Calcit::Trait(trait_def) => Some(Calcit::Trait(trait_def.to_owned())),
        _ => None,
      })
      .collect::<Option<Vec<_>>>()?;
    args[2] = Calcit::from(CalcitList::from(resolved.as_slice()));
  }
  match builtins::meta::trait_new(&args).ok()? {
    Calcit::Trait(trait_def) => Some(trait_def),
    _ => None,
  }
}

/// Static declaration check for `&trait::new name members parents`: every
/// parent resolves to a trait of the same kind (external-object or ordinary),
/// the reachable set has no cycle and member names stay unique.
pub(crate) fn check_trait_requires_declaration(args: &CalcitList, file_ns: &str, def_name: &str) -> Option<String> {
  let Some(Calcit::List(parents)) = args.get(2) else {
    return None;
  };
  let mut resolved = vec![];
  for parent in parents.iter().filter(|item| !matches!(item, Calcit::Proc(CalcitProc::List))) {
    let trait_def = match parent {
      Calcit::Trait(trait_def) => Some(trait_def.to_owned()),
      Calcit::Import(import) => {
        lookup_source_backed_trait_def(&import.ns, &import.def).map(|trait_def| trait_def.with_definition_ref(&import.ns, &import.def))
      }
      _ => None,
    };
    match trait_def {
      Some(trait_def) => resolved.push(Arc::new(trait_def)),
      None => {
        return Some(format!(
          "trait {file_ns}/{def_name} requires `{parent}`, which is not a trait definition"
        ));
      }
    }
  }
  // Source resolution also covers external-object field members, which the
  // runtime constructor does not evaluate outside JS.
  let mut child = lookup_source_backed_trait_def(file_ns, def_name)?.with_definition_ref(file_ns, def_name);
  child.runtime_id = None;
  let child_external = trait_is_external_object(&child);
  for parent in resolved.iter() {
    if trait_is_external_object(parent) != child_external {
      let kind = |external: bool| {
        if external {
          "an external-object trait"
        } else {
          "an ordinary trait"
        }
      };
      return Some(format!(
        "trait {file_ns}/{def_name} is {} but requires {}, which is {}",
        kind(child_external),
        parent.origin_label(),
        kind(!child_external)
      ));
    }
  }
  child.requires = std::sync::Arc::new(vec![]);
  child.with_requires(resolved).err()
}

/// Static check for `impl-traits`: every attached impl needs impls of all
/// traits its origin requires on the same type.
pub(crate) fn check_impl_attachment_requires(args: &CalcitList, scope_types: &ScopeTypes) -> Option<String> {
  let base = resolve_type_value(args.first()?, scope_types)?;
  let mut impls: Vec<Arc<CalcitImpl>> = match base.as_ref() {
    CalcitTypeAnnotation::StructDef(definition)
    | CalcitTypeAnnotation::Struct(definition, _)
    | CalcitTypeAnnotation::StructValue(definition) => definition.impls.to_vec(),
    CalcitTypeAnnotation::EnumDef(definition) | CalcitTypeAnnotation::Enum(definition, _) => definition.impls().to_vec(),
    _ => return None,
  };
  for value in args.iter().skip(1) {
    impls.push(resolve_impl_annotation(value, scope_types)?);
  }
  for imp in impls.iter() {
    let origin = imp.origin()?;
    let reachable = origin.normalized_reachable_traits().ok()?;
    for required in reachable.iter().filter(|required| !required.has_same_origin(origin)) {
      let attached = impls.iter().any(|candidate| {
        candidate
          .origin()
          .is_some_and(|candidate_origin| candidate_origin.has_same_origin(required))
      });
      if !attached {
        return Some(format!(
          "an impl of trait {} requires an impl of trait {} on the same type; attach one before or with it",
          origin.origin_label(),
          required.origin_label()
        ));
      }
    }
  }
  None
}

/// Synthesize the concrete metadata type produced by `&impl::new` without
/// executing method bodies. This lets local impl bindings participate in
/// subsequent `impl-traits` inference just like top-level `defimpl` values.
fn infer_impl_value(xs: &CalcitList, scope_types: &ScopeTypes) -> Option<CalcitImpl> {
  let (name, origin) = resolve_impl_origin(xs.get(1)?, scope_types)?;
  let mut entries = Vec::with_capacity(xs.len().saturating_sub(2));
  for item in xs.iter().skip(2) {
    let (field, method) = extract_impl_pair(item)?;
    entries.push((extract_impl_field_name(field)?, method.to_owned()));
  }
  entries.sort_by(|a, b| a.0.ref_str().cmp(b.0.ref_str()));
  if entries.windows(2).any(|pair| pair[0].0 == pair[1].0) {
    return None;
  }
  let fields = entries.iter().map(|(field, _)| field.to_owned()).collect();
  let values = entries.into_iter().map(|(_, method)| method).collect();
  Some(CalcitImpl {
    name,
    origin,
    fields: Arc::new(fields),
    values: Arc::new(values),
  })
}

fn resolve_impl_annotation(value: &Calcit, scope_types: &ScopeTypes) -> Option<Arc<CalcitImpl>> {
  let inferred = resolve_type_value(value, scope_types)?;
  match inferred.as_ref() {
    CalcitTypeAnnotation::Custom(inner) => match inner.as_ref() {
      Calcit::Impl(impl_def) => Some(Arc::new(impl_def.to_owned())),
      _ => None,
    },
    _ => None,
  }
}

/// Preserve the nominal data type of `impl-traits` while merging every impl
/// whose metadata is statically known. If any attachment is opaque, return
/// `None` so the builtin signature provides a conservative open type instead
/// of a nominal type with an incorrectly complete method table.
fn infer_impl_attachment_type(xs: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  let base_expr = xs.get(1)?;
  let base = resolve_type_value(base_expr, scope_types)?;
  let impl_values = xs
    .iter()
    .skip(2)
    .map(|value| resolve_impl_annotation(value, scope_types))
    .collect::<Option<Vec<_>>>()?;
  if impl_values.is_empty() {
    return None;
  }

  match base.as_ref() {
    CalcitTypeAnnotation::StructDef(struct_def) => {
      let mut attached = struct_def.as_ref().to_owned();
      attached.impls.extend(impl_values);
      Some(Arc::new(CalcitTypeAnnotation::StructDef(Arc::new(attached))))
    }
    CalcitTypeAnnotation::EnumDef(enum_def) => {
      let mut attached = enum_def.as_ref().to_owned();
      let mut impls = attached.impls().to_vec();
      impls.extend(impl_values);
      attached.set_impls(impls);
      Some(Arc::new(CalcitTypeAnnotation::EnumDef(Arc::new(attached))))
    }
    CalcitTypeAnnotation::Struct(struct_def, args) => {
      let mut attached = struct_def.as_ref().to_owned();
      attached.impls.extend(impl_values);
      Some(Arc::new(CalcitTypeAnnotation::Struct(Arc::new(attached), args.clone())))
    }
    CalcitTypeAnnotation::StructValue(struct_def) => {
      let mut attached = struct_def.as_ref().to_owned();
      attached.impls.extend(impl_values);
      Some(Arc::new(CalcitTypeAnnotation::StructValue(Arc::new(attached))))
    }
    CalcitTypeAnnotation::Enum(enum_def, args) => {
      let mut attached = enum_def.as_ref().to_owned();
      let mut impls = attached.impls().to_vec();
      impls.extend(impl_values);
      attached.set_impls(impls);
      Some(Arc::new(CalcitTypeAnnotation::Enum(Arc::new(attached), args.clone())))
    }
    CalcitTypeAnnotation::EnumValue(enum_def) => {
      let mut attached = enum_def.as_ref().to_owned();
      let mut impls = attached.impls().to_vec();
      impls.extend(impl_values);
      attached.set_impls(impls);
      Some(Arc::new(CalcitTypeAnnotation::EnumValue(Arc::new(attached))))
    }
    CalcitTypeAnnotation::TypeRef(_, args) => {
      // The expression's source distinguishes a definition value from an
      // instance whose schema refers to that same nominal definition.
      let source_definition = match base_expr {
        Calcit::Import(CalcitImport { ns, def, .. }) => program::lookup_def_code(ns, def),
        Calcit::Symbol { sym, info, .. } => program::lookup_def_code(&info.at_ns, sym),
        _ => None,
      };
      let is_definition_value = source_definition.as_ref().is_some_and(code_resolves_to_nominal_type_def);
      if let Some(mut attached) = base.resolve_to_struct() {
        attached.impls.extend(impl_values);
        if is_definition_value {
          Some(Arc::new(CalcitTypeAnnotation::StructDef(Arc::new(attached))))
        } else {
          Some(Arc::new(CalcitTypeAnnotation::Struct(Arc::new(attached), args.clone())))
        }
      } else if let Some(mut attached) = base.resolve_to_enum() {
        let mut impls = attached.impls().to_vec();
        impls.extend(impl_values);
        attached.set_impls(impls);
        if is_definition_value {
          Some(Arc::new(CalcitTypeAnnotation::EnumDef(Arc::new(attached))))
        } else {
          Some(Arc::new(CalcitTypeAnnotation::Enum(Arc::new(attached), args.clone())))
        }
      } else {
        None
      }
    }
    _ => None,
  }
}

/// Preserve a nominal method contract through origin-qualified trait lowering.
fn lowered_trait_method_signature(xs: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitFnTypeAnnotation>> {
  let trait_def = match xs.get(1)? {
    Calcit::Trait(definition) => definition.clone(),
    Calcit::Import(import) => lookup_source_backed_trait_def(&import.ns, &import.def)?.with_definition_ref(&import.ns, &import.def),
    local @ Calcit::Local(_) => match resolve_type_value(local, scope_types)?.as_ref() {
      CalcitTypeAnnotation::Trait(definition) => definition.as_ref().clone(),
      _ => return None,
    },
    _ => return None,
  };
  let method_name = match xs.get(2)? {
    Calcit::Tag(name) => name.ref_str(),
    Calcit::Symbol { sym, .. } | Calcit::Str(sym) => sym.as_ref(),
    _ => return None,
  };
  let receiver_type = resolve_type_value(xs.get(3)?, scope_types)?;
  let matching_count = if let Some(traits) = trait_list_from_type(receiver_type.as_ref()) {
    // A listed bound counts as is, so duplicates stay ambiguous. A trait that is
    // only required by a listed bound is also proven, as in surface method lookup.
    let direct = traits.iter().filter(|candidate| candidate.has_same_origin(&trait_def)).count();
    if direct > 0 {
      direct
    } else {
      let reachable = reachable_dispatch_traits(&traits).ok()?;
      reachable.iter().filter(|candidate| candidate.has_same_origin(&trait_def)).count()
    }
  } else {
    get_impls_from_type(receiver_type.as_ref())?
      .iter()
      .filter(|candidate| candidate.implements_trait(&trait_def))
      .count()
  };
  if matching_count != 1 {
    return None;
  }
  trait_def
    .method_index(method_name)
    .and_then(|index| trait_def.method_types.get(index))?
    .resolve_to_nonoptional_fn()
}

/// Surface and lowered methods share argument proof and result substitution.
fn infer_typed_method_result<'a>(
  signature: &CalcitFnTypeAnnotation,
  arguments: impl Iterator<Item = &'a Calcit>,
  scope_types: &ScopeTypes,
) -> Arc<CalcitTypeAnnotation> {
  let arguments = arguments.collect::<Vec<_>>();
  let required = signature.arg_types.len() - calcit::trailing_option_arg_count(&signature.arg_types, signature.arg_types.len());
  if arguments.len() < required || signature.rest_type.is_none() && arguments.len() > signature.arg_types.len() {
    return calcit::DYNAMIC_TYPE.clone();
  }
  let Some(actual_types) = arguments
    .iter()
    .map(|argument| resolve_type_value(argument, scope_types))
    .collect::<Option<Vec<_>>>()
  else {
    return calcit::DYNAMIC_TYPE.clone();
  };
  let mut expected_types = signature.arg_types.clone();
  expected_types.extend(signature.rest_type.iter().cloned());
  expected_types.push(signature.return_type.clone());
  let mut proof = crate::calcit::type_annotation::CallTypeProof::new(&signature.generics, &expected_types, &actual_types);
  for (index, (argument, actual)) in arguments.iter().zip(&actual_types).enumerate() {
    let Some(expected) = signature.arg_types.get(index).or(signature.rest_type.as_ref()) else {
      return calcit::DYNAMIC_TYPE.clone();
    };
    if core_option_none_without_payload(argument, actual.as_ref(), expected.as_ref()) {
      continue;
    }
    let proven =
      if let Some((actual_error, expected_error)) = core_result_err_payload_types(argument, actual.as_ref(), expected.as_ref()) {
        proof.prove(actual_error, expected_error)
      } else {
        proof.prove(actual.as_ref(), expected.as_ref())
      };
    if !proven.is_proven() {
      return calcit::DYNAMIC_TYPE.clone();
    }
  }
  for bound in signature.where_bounds.iter() {
    let variable = CalcitTypeAnnotation::TypeVar(bound.name.clone());
    if !proof
      .result(&variable)
      .is_some_and(|actual| actual.is_proven_for(&bound.as_type_annotation()))
    {
      return calcit::DYNAMIC_TYPE.clone();
    }
  }
  invocation_return_type(signature, proof.result_or_open(&signature.return_type), false)
}

/// Infer the return type of a built-in proc call expression.
///
/// Extracted from the large `Calcit::Proc` arm of `infer_type_from_expr` for clarity.
fn infer_proc_call_return_type(proc: &CalcitProc, xs: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  if matches!(proc, CalcitProc::GetEnv)
    && matches!(xs.len(), 2 | 3)
    && let Some(name_type) = xs.get(1).and_then(|name| resolve_type_value(name, scope_types))
    && name_type.is_proven_for(&CalcitTypeAnnotation::String)
  {
    // The raw one-argument operation can only return text or nil. Its broad
    // first-class signature also admits legacy arbitrary fallbacks, whose
    // independent evidence must participate in the actual call's result.
    let text = Arc::new(CalcitTypeAnnotation::String);
    return Some(match xs.get(2) {
      None => wrap_optional_type(text),
      Some(fallback) => match resolve_type_value(fallback, scope_types) {
        // Arguments evaluate before the environment lookup, not lazily in
        // its missing-value branch.
        Some(value) if matches!(value.as_ref(), CalcitTypeAnnotation::Never) => value,
        Some(value) => merge_if_branch_types(text.clone(), value.clone())
          // A compatibility join must not fabricate a nominal wrapper for
          // the actual String branch. Both producers must prove the result.
          .filter(|joined| text.is_proven_for(joined) && value.is_proven_for(joined))
          .unwrap_or_else(|| calcit::DYNAMIC_TYPE.clone()),
        None => calcit::DYNAMIC_TYPE.clone(),
      },
    });
  }
  if matches!(proc, CalcitProc::NativeTraitCall)
    && let Some(signature) = lowered_trait_method_signature(xs, scope_types)
  {
    return Some(infer_typed_method_result(&signature, xs.iter().skip(3), scope_types));
  }
  if matches!(proc, CalcitProc::NativeEnumDefinition)
    && let Some(receiver) = xs.get(1).and_then(|value| resolve_type_value(value, scope_types))
    && receiver.is_proven_for(&CalcitTypeAnnotation::AnonymousEnum)
    && let Some(definition) = receiver.resolve_to_enum()
  {
    // A proven nominal value always carries its definition. Anonymous or
    // nullable inputs retain the proc's open, nullable result contract.
    return Some(Arc::new(CalcitTypeAnnotation::EnumDef(Arc::new(definition))));
  }
  if matches!(proc, CalcitProc::NativeTraitNew)
    && let Some(trait_def) = infer_trait_value(xs)
  {
    return Some(Arc::new(CalcitTypeAnnotation::Trait(Arc::new(trait_def))));
  }
  if matches!(proc, CalcitProc::NativeImplNew)
    && let Some(impl_def) = infer_impl_value(xs, scope_types)
  {
    return Some(Arc::new(CalcitTypeAnnotation::Custom(Arc::new(Calcit::Impl(impl_def)))));
  }
  if matches!(
    proc,
    CalcitProc::NativeStructValueImplTraits
      | CalcitProc::NativeEnumValueImplTraits
      | CalcitProc::NativeStructImplTraits
      | CalcitProc::NativeEnumImplTraits
  ) && let Some(inferred) = infer_impl_attachment_type(xs, scope_types)
  {
    return Some(inferred);
  }
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
  if matches!(proc, CalcitProc::Ref)
    && let Some(initial_value) = xs.get(1)
    && let Some(initial_type) = resolve_type_value(initial_value, scope_types)
  {
    return Some(Arc::new(CalcitTypeAnnotation::Ref(initial_type)));
  }
  if matches!(
    proc,
    CalcitProc::FoldlShortcut | CalcitProc::FoldrShortcut | CalcitProc::NativeListFoldlShortcut
  ) && let Some(return_type) = infer_shortcut_fold_return(xs, scope_types)
  {
    return Some(return_type);
  }
  if matches!(proc, CalcitProc::Foldl | CalcitProc::NativeListFoldl | CalcitProc::NativeMapFoldKv)
    && let Some(reducer) = xs.get(3)
    && let Some(reducer_type) = resolve_type_value(reducer, scope_types)
  {
    // Return proof and callback preprocessing share the same input contract.
    if let Some(expected) = super::type_checking::specialize_collection_fold_expected_types(
      &xs.drop_left(),
      scope_types,
      &proc.get_type_signature()?.arg_types,
    ) && matches!(reducer_type.as_ref(), CalcitTypeAnnotation::Fn(_))
      && reducer_type.as_ref().is_proven_for(&expected[2])
    {
      return Some(expected[1].clone());
    }
  }
  // `&list:nth` retains its unchecked payload type for guarded core macro
  // expansion. Unlike it, `&list:first` has a nullable Core schema and must
  // expose that absence to callers.
  if matches!(proc, CalcitProc::NativeListNth | CalcitProc::NativeListFirst)
    && let Some(first_arg) = xs.get(1)
    && let Some(type_value) = resolve_type_value(first_arg, scope_types)
    && let CalcitTypeAnnotation::List(element_type) = type_value.as_ref()
  {
    return Some(if matches!(proc, CalcitProc::NativeListFirst) {
      wrap_optional_type(element_type.clone())
    } else {
      element_type.clone()
    });
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
      | CalcitProc::NativeListSort
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
  // MapToList exposes each heterogeneous key/value pair as an open two-item
  // list, while preserving the outer collection shape.
  if matches!(proc, CalcitProc::NativeMapToList) {
    return Some(Arc::new(CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::List(
      calcit::DYNAMIC_TYPE.clone(),
    )))));
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
  if matches!(proc, CalcitProc::NativeNamedEnumNew)
    && let Some(tuple_type) = infer_enum_annotation(xs, scope_types)
  {
    return Some(tuple_type);
  }
  if matches!(proc, CalcitProc::NativeStructNew)
    && let Some(struct_type) = infer_struct_literal_type(xs)
  {
    return Some(struct_type);
  }
  if matches!(proc, CalcitProc::NativeEnumNew)
    && let Some(enum_type) = infer_enum_literal_type(xs)
  {
    return Some(enum_type);
  }
  if matches!(proc, CalcitProc::NativeStruct)
    && let Some(struct_type) = infer_struct_value_literal_type(xs, scope_types)
  {
    return Some(struct_type);
  }
  if matches!(proc, CalcitProc::NativeStructFromMap)
    && let Some(struct_value) = xs.get(1).and_then(|definition| resolve_struct_value(definition, scope_types))
  {
    return Some(Arc::new(CalcitTypeAnnotation::StructValue(struct_value.struct_ref)));
  }
  if matches!(proc, CalcitProc::NativeLooseStruct) {
    return Some(tag_annotation("struct"));
  }
  if matches!(proc, CalcitProc::NativeStructGet)
    && let Some(field_type) = infer_struct_get_type(xs, scope_types)
  {
    return Some(field_type);
  }
  if matches!(proc, CalcitProc::NativeStructNth)
    && let Some(field_type) = infer_struct_nth_type(xs, scope_types)
  {
    return Some(field_type);
  }
  if matches!(
    proc,
    CalcitProc::NativeStructAssoc | CalcitProc::NativeStructAssocAt | CalcitProc::NativeStructWith | CalcitProc::NativeStructWithAt
  ) && let Some(record_arg) = xs.get(1)
    && let Some(record_type) = resolve_type_value(record_arg, scope_types)
    && record_type.resolve_to_struct().is_some()
  {
    return Some(record_type);
  }
  // Substitute the type variables that the argument evidence proves, so a
  // generic result such as `Set<K>` keeps the key type of its receiver.
  proc.get_type_signature().map(|type_sig| {
    if !type_sig.return_type.contains_type_var() {
      return type_sig.return_type.clone();
    }
    let mut bindings = HashMap::new();
    for (argument, expected) in xs.iter().skip(1).zip(type_sig.arg_types.iter()) {
      if let Some(actual) = resolve_type_value(argument, scope_types) {
        actual.prove_with_bindings(expected, &mut bindings);
      }
    }
    if bindings.is_empty() {
      type_sig.return_type.clone()
    } else {
      type_sig.return_type.substitute_type_vars(&bindings)
    }
  })
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
      Some(current)
        if matches!(current.as_ref(), CalcitTypeAnnotation::Fn(_)) && matches!(next.as_ref(), CalcitTypeAnnotation::Fn(_)) =>
      {
        // Function inputs are contravariant: retain a common callable contract,
        // not an invariant equality requirement on every parameter annotation.
        if current.is_proven_for(next.as_ref()) {
          inferred = Some(next);
        } else if !next.is_proven_for(current.as_ref()) {
          return calcit::DYNAMIC_TYPE.clone();
        }
      }
      Some(current) if !current.as_ref().is_proven_for(next.as_ref()) || !next.as_ref().is_proven_for(current.as_ref()) => {
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

  // Data definitions keep `:dynamic` as their value schema because their
  // concrete shape is declared in the source form. Resolve the nominal name
  // before looking at compiled metadata: preprocessing a `defstruct` can
  // temporarily expose an implementation helper, which must not replace the
  // source-owned definition object or conflate it with an instance type.
  let schema_is_data_definition_marker = match schema.as_ref() {
    CalcitTypeAnnotation::Dynamic => true,
    CalcitTypeAnnotation::Custom(value) => {
      matches!(value.as_ref(), Calcit::Tag(tag) if matches!(tag.ref_str(), "struct-def" | "enum-def"))
    }
    _ => false,
  };
  if schema_is_data_definition_marker {
    let named_type = Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from(format!("{ns}/{def}")), Arc::new(vec![])));
    let source_is_nominal = program::lookup_def_code(ns, def).is_some_and(|code| code_resolves_to_nominal_type_def(&code));
    if source_is_nominal {
      if let Some(definition) = named_type.resolve_to_enum() {
        return Some(Arc::new(CalcitTypeAnnotation::EnumDef(Arc::new(definition))));
      }
      if let Some(definition) = named_type.resolve_to_struct() {
        return Some(Arc::new(CalcitTypeAnnotation::StructDef(Arc::new(definition))));
      }
      return Some(named_type);
    }
  }

  // Static data/trait/impl declarations carry richer metadata in their
  // preprocessed value than in the intentionally broad top-level schema.
  // Prefer that concrete metadata so definition values stay distinct from
  // their instance types and imported `impl-traits` calls retain their method table.
  // imported `impl-traits` calls keep their nominal type and method table.
  if let Some(compiled) = program::lookup_compiled_def(ns, def)
    && let Some(inferred) = infer_type_from_expr(&compiled.preprocessed_code, &ScopeTypes::new())
  {
    let is_concrete_metadata = matches!(
      inferred.as_ref(),
      CalcitTypeAnnotation::Struct(..)
        | CalcitTypeAnnotation::Enum(..)
        | CalcitTypeAnnotation::StructDef(..)
        | CalcitTypeAnnotation::EnumDef(..)
        | CalcitTypeAnnotation::Trait(..)
    ) || matches!(inferred.as_ref(), CalcitTypeAnnotation::Custom(value) if matches!(value.as_ref(), Calcit::Impl(_)));
    if is_concrete_metadata {
      return Some(inferred);
    }
  }

  if !matches!(schema.as_ref(), CalcitTypeAnnotation::Dynamic) {
    return Some(definition_value_schema(ns, def, schema));
  }

  // Data definitions often keep `:dynamic` as their value schema because their concrete field
  // shape lives in the source form. Preserve a named TypeRef here instead of mistaking the
  // synthetic struct prototype used during preprocessing for a runtime struct instance.
  let named_type = Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from(format!("{ns}/{def}")), Arc::new(vec![])));
  if named_type.resolve_to_struct().is_some() || named_type.resolve_to_enum().is_some() {
    return Some(named_type);
  }

  let compiled = program::lookup_compiled_def(ns, def)?;
  match compiled.preprocessed_code {
    Calcit::Fn { info, .. } => Some(definition_value_schema(
      ns,
      def,
      Arc::new(CalcitTypeAnnotation::from_calcit_fn(&info)),
    )),
    Calcit::Proc(proc) => proc.get_type_signature().map(|signature| {
      Arc::new(CalcitTypeAnnotation::from_proc_parts(
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

/// Prove a fixed call from already-processed expressions, without executing them.
/// This uses the compiler's ordinary proof relation, not compatibility matching.
pub fn fixed_call_arguments_are_proven(callable: &Calcit, arguments: &[Calcit]) -> bool {
  let Some(signature) = infer_static_type_from_expr(callable).and_then(|annotation| annotation.resolve_to_nonoptional_fn()) else {
    return false;
  };
  if signature.fn_kind != SchemaKind::Fn
    || signature.rest_type.is_some()
    || !signature.where_bounds.is_empty()
    || signature.arg_types.len() != arguments.len()
    || calcit::trailing_option_arg_count(&signature.arg_types, signature.arg_types.len()) > 0
    || signature
      .arg_types
      .iter()
      .any(|annotation| matches!(annotation.as_ref(), CalcitTypeAnnotation::Optional(_)))
  {
    return false;
  }
  let mut bindings = HashMap::new();
  arguments.iter().zip(&signature.arg_types).all(|(argument, expected)| {
    infer_static_type_from_expr(argument).is_some_and(|actual| actual.prove_with_bindings(expected, &mut bindings).is_proven())
  })
}

/// Resolve a declarative core nominal definition through the same value path
/// used by method preprocessing, preserving attached implementations.
pub fn resolve_core_nominal_instance_type(ns: &str, def: &str) -> Option<Arc<CalcitTypeAnnotation>> {
  if ns != calcit::CORE_NS || !program::lookup_def_code(ns, def).is_some_and(|code| code_resolves_to_nominal_type_def(&code)) {
    return None;
  }
  match resolve_program_value_for_preprocess(ns, def, None)? {
    Calcit::StructDef(struct_def) => Some(Arc::new(CalcitTypeAnnotation::Struct(Arc::new(struct_def), Arc::new(vec![])))),
    Calcit::EnumDef(enum_def) => Some(Arc::new(CalcitTypeAnnotation::Enum(Arc::new(enum_def), Arc::new(vec![])))),
    _ => None,
  }
}

/// Recover the implementation type already present in a compiled definition.
///
/// This deliberately reuses the normal preprocessor output and bottom-up
/// inference. It does not use the source schema as a fallback, so callers can
/// compare the implementation evidence with an incomplete declaration without
/// accidentally treating that declaration as inferred proof.
pub fn infer_compiled_definition_implementation_type(ns: &str, def: &str) -> Option<Arc<CalcitTypeAnnotation>> {
  let compiled = program::lookup_compiled_def(ns, def)?;
  let Calcit::List(items) = &compiled.preprocessed_code else {
    return infer_type_from_expr(&compiled.preprocessed_code, &ScopeTypes::new());
  };

  match items.first() {
    Some(Calcit::Syntax(CalcitSyntax::DefWasmImport, _)) => None,
    Some(Calcit::Syntax(CalcitSyntax::Defn | CalcitSyntax::DefWasmExport, _)) => {
      let Calcit::List(params) = items.get(2)? else {
        return None;
      };
      let mut arg_types = Vec::new();
      let mut rest_type = None;
      let mut spread = false;
      for param in params.iter() {
        match param {
          Calcit::Syntax(CalcitSyntax::ArgSpread, _) => spread = true,
          Calcit::Local(local) if spread => {
            rest_type = Some(local.type_info.clone());
            spread = false;
          }
          Calcit::Local(local) => arg_types.push(local.type_info.clone()),
          _ => return None,
        }
      }
      if spread {
        return None;
      }
      let mut returned = None;
      for form in items.iter().skip(3) {
        if !crate::builtins::syntax::is_function_metadata_hint(form) {
          returned = Some(form);
        }
      }
      // Reconstruct lexical parameter evidence from the compiled definition,
      // including open inputs. Missing scope is not an explicit Dynamic input.
      let mut parameter_scope = ScopeTypes::new();
      bind_pattern_scope(items.get(2)?, &mut parameter_scope);
      let return_type = returned.and_then(|body| resolve_type_value(body, &parameter_scope))?;
      let mut signature = CalcitFnTypeAnnotation {
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types,
        return_type,
        fn_kind: SchemaKind::Fn,
        rest_type,
        features: Arc::new(HashSet::new()),
      };
      if definition_marks_async(ns, def) {
        signature = signature.with_async_invocation();
      }
      Some(Arc::new(CalcitTypeAnnotation::Fn(Arc::new(signature))))
    }
    Some(Calcit::Syntax(CalcitSyntax::Defref, _)) => items
      .get(2)
      .and_then(|initializer| resolve_type_value(initializer, &ScopeTypes::new()))
      .map(|inner| Arc::new(CalcitTypeAnnotation::Ref(inner))),
    _ => items
      .get(items.len().checked_sub(1)?)
      .and_then(|value| resolve_type_value(value, &ScopeTypes::new())),
  }
}

// ---------------------------------------------------------------------------
// Specialised inference helpers
// ---------------------------------------------------------------------------

/// Anonymous enum schemas do not describe their control/payload slots. Inspect
/// the actual reducer implementation instead of trusting a declared Enum return.
fn shortcut_payload_is_proven(expr: &Calcit, accumulator: &CalcitTypeAnnotation, scope_types: &ScopeTypes) -> bool {
  let Calcit::List(items) = expr else { return false };
  match items.first() {
    Some(Calcit::Proc(CalcitProc::NativeEnum)) if items.len() == 3 => {
      items
        .get(1)
        .and_then(|control| resolve_type_value(control, scope_types))
        .is_some_and(|control| control.is_proven_for(&CalcitTypeAnnotation::Bool))
        && items
          .get(2)
          .and_then(|payload| resolve_type_value(payload, scope_types))
          .is_some_and(|payload| payload.is_proven_for(accumulator))
    }
    Some(Calcit::Syntax(CalcitSyntax::CoreLet, _)) => items
      .get(items.len().saturating_sub(1))
      .is_some_and(|tail| shortcut_payload_is_proven(tail, accumulator, &core_let_scope(items, scope_types))),
    Some(Calcit::Syntax(CalcitSyntax::If, _)) if items.len() == 4 => [items.get(2), items.get(3)].into_iter().all(|branch| {
      branch
        .is_some_and(|branch| expression_definitely_diverges(branch) || shortcut_payload_is_proven(branch, accumulator, scope_types))
    }),
    Some(Calcit::Syntax(CalcitSyntax::Match, _)) => preprocessed_match_branches(items).is_some_and(|branches| {
      !branches.is_empty()
        && branches.into_iter().all(|(pattern, branch)| {
          let mut branch_scope = scope_types.clone();
          bind_pattern_scope(pattern, &mut branch_scope);
          expression_definitely_diverges(branch) || shortcut_payload_is_proven(branch, accumulator, &branch_scope)
        })
    }),
    _ => false,
  }
}

/// Prove a shortcut fold result from its concrete accumulator, default and reducer.
/// Every reachable reducer result must have a Bool control and a proven payload;
/// unavailable source or open evidence leaves the result unproven, not narrowed.
fn infer_shortcut_fold_return(xs: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  if xs.len() != 5 {
    return None;
  }
  let accumulator = resolve_type_value(xs.get(2)?, scope_types)?;
  let default = resolve_type_value(xs.get(3)?, scope_types)?;
  if !default.is_proven_for(&accumulator) || super::contains_dynamic_type(&accumulator) {
    return None;
  }
  let args = xs.drop_left();
  let expected =
    super::type_checking::specialize_collection_fold_expected_types(&args, scope_types, &vec![calcit::DYNAMIC_TYPE.clone(); 4])?;
  let reducer = xs.get(4)?;
  // Use compiled source only: resolving a runtime thunk here could re-enter
  // preprocessing, and open callable metadata cannot prove enum slot values.
  let owned;
  let mut reducer_scope = scope_types.clone();
  let reducer = if let Calcit::Import(import) = reducer {
    reducer_scope.clear();
    owned = program::lookup_compiled_def(&import.ns, &import.def)?.preprocessed_code;
    &owned
  } else {
    reducer
  };
  let Calcit::List(body) = reducer else { return None };
  if !matches!(body.first(), Some(Calcit::Syntax(CalcitSyntax::Defn, _))) {
    return None;
  }
  let (arg_types, rest) = infer_preprocessed_function_parameters(body.get(2));
  if rest.is_some() || arg_types.len() != 2 {
    return None;
  }
  bind_pattern_scope(body.get(2)?, &mut reducer_scope);
  let signature = CalcitTypeAnnotation::from_function_parts(arg_types, Arc::new(CalcitTypeAnnotation::AnonymousEnum));
  if !signature.is_proven_for(expected.get(3)?) {
    return None;
  }
  let annotation = infer_preprocessed_function_type(body);
  if annotation.resolve_to_fn().is_some_and(|signature| signature.is_async_invocation()) {
    return None;
  }
  let tail = body
    .iter()
    .skip(3)
    .filter(|form| !crate::builtins::syntax::is_function_metadata_hint(form))
    .last()?;
  shortcut_payload_is_proven(tail, &accumulator, &reducer_scope).then_some(accumulator)
}

fn infer_enum_annotation(xs: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  if xs.len() < 3 {
    return None;
  }
  let enum_arg = xs.get(1)?;
  let tag_arg = xs.get(2);
  let enum_proto = resolve_enum_value(enum_arg, scope_types)?;

  if enum_proto.generics().is_empty() {
    return Some(Arc::new(CalcitTypeAnnotation::EnumValue(Arc::new(enum_proto))));
  }

  let applied_args = infer_enum_applied_args(&enum_proto, tag_arg, xs.iter().skip(3), scope_types).unwrap_or_else(|| {
    enum_proto
      .generics()
      .iter()
      .map(|_| calcit::DYNAMIC_TYPE.clone())
      .collect::<Vec<_>>()
  });
  Some(Arc::new(CalcitTypeAnnotation::Enum(Arc::new(enum_proto), Arc::new(applied_args))))
}

fn infer_enum_applied_args<'a>(
  enum_proto: &CalcitEnumDef,
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
  let payload_args = payload_args.collect::<Vec<_>>();
  if payload_args.len() != variant.arity() {
    return None;
  }
  let mut bindings: HashMap<Arc<str>, Arc<CalcitTypeAnnotation>> = HashMap::new();

  for (payload, expected_type) in payload_args.into_iter().zip(variant.payload_types().iter()) {
    let actual_type = resolve_type_value(payload, scope_types).unwrap_or_else(|| calcit::DYNAMIC_TYPE.clone());
    let resolved_expected = resolve_local_type_refs_for_body(expected_type.clone(), scope_types);
    if !actual_type
      .as_ref()
      .prove_with_bindings(resolved_expected.as_ref(), &mut bindings)
      .is_proven()
    {
      return None;
    }
  }

  let used = crate::calcit::type_annotation::free_type_variable_names(variant.payload_types());
  Some(
    enum_proto
      .generics()
      .iter()
      .map(|name| {
        bindings.get(name).cloned().unwrap_or_else(|| {
          if used.contains(name) {
            calcit::DYNAMIC_TYPE.clone()
          } else {
            crate::calcit::type_annotation::NEVER_TYPE.clone()
          }
        })
      })
      .collect(),
  )
}

fn infer_struct_get_type(xs: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  if xs.len() < 3 {
    return None;
  }
  let struct_arg = xs.get(1)?;
  let field_arg = xs.get(2)?;
  let field_name = extract_field_name(field_arg)?;
  infer_struct_field_type(struct_arg, field_name, scope_types)
}

/// Infer the return type of `&struct:nth struct_value idx` by looking up the field type at the given index.
fn infer_struct_nth_type(xs: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  if xs.len() < 3 {
    return None;
  }
  let struct_arg = xs.get(1)?;
  let idx_arg = xs.get(2)?;
  let idx = match idx_arg {
    Calcit::Number(n) => *n as usize,
    _ => return None,
  };
  let type_info = resolve_type_value(struct_arg, scope_types)?;
  resolve_struct_field_type_by_index(type_info.as_ref(), idx)
}

fn infer_struct_value_literal_type(xs: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  if xs.len() < 2 {
    return None;
  }
  let proto_arg = xs.get(1)?;
  let struct_value = resolve_struct_value(proto_arg, scope_types)?;
  if struct_value.struct_ref.generics.is_empty() {
    return Some(Arc::new(CalcitTypeAnnotation::StructValue(struct_value.struct_ref.clone())));
  }

  let field_values = collect_struct_literal_values(xs, &struct_value)?;
  let applied_args = infer_struct_applied_args(struct_value.struct_ref.as_ref(), field_values.iter(), scope_types);
  Some(Arc::new(CalcitTypeAnnotation::Struct(
    struct_value.struct_ref.clone(),
    Arc::new(applied_args),
  )))
}

fn infer_struct_literal_type(xs: &CalcitList) -> Option<Arc<CalcitTypeAnnotation>> {
  let args = xs.iter().skip(1).map(normalize_static_metadata_form).collect::<Vec<_>>();
  match builtins::structs::new_struct(&args).ok()? {
    Calcit::StructDef(struct_def) => Some(Arc::new(CalcitTypeAnnotation::StructDef(Arc::new(struct_def)))),
    _ => None,
  }
}

fn infer_enum_literal_type(xs: &CalcitList) -> Option<Arc<CalcitTypeAnnotation>> {
  let args = xs.iter().skip(1).map(normalize_static_metadata_form).collect::<Vec<_>>();
  match builtins::structs::new_enum(&args).ok()? {
    Calcit::EnumDef(enum_def) => Some(Arc::new(CalcitTypeAnnotation::EnumDef(Arc::new(enum_def)))),
    _ => None,
  }
}

pub(crate) fn infer_struct_field_type(
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

pub(crate) fn resolve_struct_field_type_by_index(type_info: &CalcitTypeAnnotation, idx: usize) -> Option<Arc<CalcitTypeAnnotation>> {
  if let CalcitTypeAnnotation::Optional(inner) = type_info {
    return resolve_struct_field_type_by_index(inner, idx);
  }
  let (struct_def, definition_ref) = type_info.resolve_to_struct_with_ref()?;
  let field_type = struct_def.field_types.get(idx)?.clone();
  // Resolve declaration-owned names before substituting caller-owned generic
  // arguments, and use the same contract for reads, writes and construction.
  let field_type = match definition_ref {
    Some((declaring_ns, _)) => resolve_namespace_type_refs_for_body(field_type, &declaring_ns),
    None => field_type,
  };
  let args = match type_info {
    CalcitTypeAnnotation::Struct(_, args) | CalcitTypeAnnotation::TypeRef(_, args) => args.as_slice(),
    _ => &[],
  };
  Some(substitute_declared_generics(&struct_def.generics, args, &field_type))
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
  struct_def: &CalcitStructDef,
  values: impl Iterator<Item = &'a Calcit>,
  scope_types: &ScopeTypes,
) -> Vec<Arc<CalcitTypeAnnotation>> {
  if struct_def.generics.is_empty() {
    return vec![];
  }

  let prototype = CalcitTypeAnnotation::StructValue(Arc::new(struct_def.clone()));
  let fields = values
    .enumerate()
    .filter_map(|(index, value)| {
      let expected = resolve_struct_field_type_by_index(&prototype, index)?;
      // A field with no declared generic cannot contribute to its bindings.
      // Its ordinary constructor check still validates the field independently.
      struct_def
        .generics
        .iter()
        .any(|name| expected.contains_type_var_named(name))
        .then(|| {
          (
            expected,
            resolve_type_value(value, scope_types).unwrap_or_else(|| calcit::DYNAMIC_TYPE.clone()),
          )
        })
    })
    .collect::<Vec<_>>();
  let expected_types = fields.iter().map(|(expected, _)| expected.clone()).collect::<Vec<_>>();
  let actual_types = fields.iter().map(|(_, actual)| actual.clone()).collect::<Vec<_>>();
  let mut proof = crate::calcit::type_annotation::CallTypeProof::new(&struct_def.generics, &expected_types, &actual_types);
  for (expected, actual) in fields {
    if proof.prove(&actual, &expected).is_mismatch() {
      return struct_def.generics.iter().map(|_| calcit::DYNAMIC_TYPE.clone()).collect();
    }
  }

  struct_def
    .generics
    .iter()
    .map(|name| proof.result_or_open(&CalcitTypeAnnotation::TypeVar(name.clone())))
    .collect()
}

pub(super) fn infer_struct_value_annotation(struct_value: &CalcitStructValue, scope_types: &ScopeTypes) -> Arc<CalcitTypeAnnotation> {
  if struct_value.struct_ref.generics.is_empty() {
    Arc::new(CalcitTypeAnnotation::StructValue(struct_value.struct_ref.clone()))
  } else {
    let applied_args = infer_struct_applied_args(struct_value.struct_ref.as_ref(), struct_value.values.iter(), scope_types);
    Arc::new(CalcitTypeAnnotation::Struct(
      struct_value.struct_ref.clone(),
      Arc::new(applied_args),
    ))
  }
}

fn collect_struct_literal_values(xs: &CalcitList, struct_value: &CalcitStructValue) -> Option<Vec<Calcit>> {
  if xs.len() < 2 {
    return None;
  }

  let mut values = vec![Calcit::Nil; struct_value.struct_ref.fields.len()];
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
    if let Some(pos) = struct_value.index_of(field_name) {
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

pub(crate) fn resolve_enum_value(target: &Calcit, scope_types: &ScopeTypes) -> Option<CalcitEnumDef> {
  fn resolve_named_enum(ns: &str, def: &str, def_id: Option<u32>) -> Option<CalcitEnumDef> {
    match resolve_program_value_for_preprocess(ns, def, def_id) {
      Some(Calcit::EnumDef(enum_def)) => Some(enum_def),
      Some(Calcit::Struct(struct_value)) => CalcitEnumDef::from_struct(struct_value).ok(),
      _ => {
        // A trait method can request its own enum while its impl attachment
        // is still compiling. Recover only the declared nominal shape; do not
        // execute the attachment or fabricate its unfinished method table.
        CalcitTypeAnnotation::TypeRef(Arc::from(format!("{ns}/{def}")), Arc::new(vec![])).resolve_to_enum()
      }
    }
  }

  match target {
    Calcit::EnumDef(enum_def) => Some(enum_def.to_owned()),
    Calcit::Struct(struct_value) => CalcitEnumDef::from_struct(struct_value.to_owned()).ok(),
    Calcit::Symbol { sym, info, .. } => resolve_named_enum(&info.at_ns, sym, None),
    Calcit::Import(CalcitImport { ns, def, def_id, .. }) => resolve_named_enum(ns, def, *def_id),
    _ => resolve_type_value(target, scope_types)
      .and_then(|t| match t.as_ref() {
        CalcitTypeAnnotation::TypeSlot(name) => resolve_type_slot(name),
        _ => Some(t),
      })
      .and_then(|t| match t.as_ref() {
        // Definition values stay distinct from enum instances in ordinary type
        // matching. Constructor inference is the one context that deliberately
        // unwraps the definition metadata into its resulting instance type.
        CalcitTypeAnnotation::EnumDef(enum_def) => Some(enum_def.as_ref().to_owned()),
        _ => t.resolve_to_enum(),
      }),
  }
}

pub(crate) fn resolve_struct_value(target: &Calcit, scope_types: &ScopeTypes) -> Option<CalcitStructValue> {
  match target {
    Calcit::Struct(struct_value) => Some(struct_value.to_owned()),
    Calcit::EnumDef(enum_def) => Some(enum_def.to_struct_prototype()),
    Calcit::StructDef(struct_def) => {
      let values = vec![Calcit::Nil; struct_def.fields.len()];
      Some(CalcitStructValue {
        struct_ref: Arc::new(struct_def.to_owned()),
        values: Arc::new(values),
      })
    }
    Calcit::Symbol { sym, info, .. } => match resolve_program_value_for_preprocess(&info.at_ns, sym, None) {
      Some(Calcit::Struct(struct_value)) => Some(struct_value),
      Some(Calcit::EnumDef(enum_def)) => Some(enum_def.to_struct_prototype()),
      Some(Calcit::StructDef(struct_def)) => {
        let values = vec![Calcit::Nil; struct_def.fields.len()];
        Some(CalcitStructValue {
          struct_ref: Arc::new(struct_def.to_owned()),
          values: Arc::new(values),
        })
      }
      _ => None,
    },
    Calcit::Import(CalcitImport { ns, def, def_id, .. }) => {
      let runtime_value = resolve_program_value_for_preprocess(ns, def, *def_id);
      match runtime_value {
        Some(Calcit::Struct(struct_value)) => Some(struct_value),
        Some(Calcit::EnumDef(enum_def)) => Some(enum_def.to_struct_prototype()),
        Some(Calcit::StructDef(struct_def)) => {
          let values = vec![Calcit::Nil; struct_def.fields.len()];
          Some(CalcitStructValue {
            struct_ref: Arc::new(struct_def.to_owned()),
            values: Arc::new(values),
          })
        }
        _ => None,
      }
    }
    _ => resolve_type_value(target, scope_types).and_then(|t| {
      let struct_def = match t.as_ref() {
        // As above, only the explicit struct-construction path unwraps a
        // StructDef into the nominal metadata of the instance being created.
        CalcitTypeAnnotation::StructDef(struct_def) => struct_def.as_ref().to_owned(),
        _ => t.resolve_to_struct()?,
      };
      Some(CalcitStructValue {
        struct_ref: Arc::new(struct_def.clone()),
        values: Arc::new(vec![Calcit::Nil; struct_def.fields.len()]),
      })
    }),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::{CalcitFn, CalcitFnArgs, CalcitFnUsageMeta, CalcitLocal, CalcitScope, CalcitSymbolInfo};

  #[test]
  fn callable_return_inference_restores_recursion_guard_on_success_failure_and_unwind() {
    for outcome in 0..3 {
      let result = std::panic::catch_unwind(|| {
        with_callable_return_inference("tests.return-guard", "root", || {
          assert_eq!(with_callable_return_inference("tests.return-guard", "root", || Some(9)), None);
          assert_eq!(with_callable_return_inference("tests.return-guard", "other", || Some(7)), Some(7));
          match outcome {
            0 => Some(3),
            1 => None,
            _ => panic!("return inference cleanup probe"),
          }
        })
      });
      assert_eq!(result.is_err(), outcome == 2);
      if let Ok(value) = result {
        assert_eq!(value, if outcome == 0 { Some(3) } else { None });
      }
      assert_eq!(with_callable_return_inference("tests.return-guard", "root", || Some(5)), Some(5));
    }
  }

  fn proc_call(proc: CalcitProc, args: Vec<Calcit>) -> Calcit {
    let mut items = Vec::with_capacity(args.len() + 1);
    items.push(Calcit::Proc(proc));
    items.extend(args);
    Calcit::from(items)
  }

  fn typed_local(name: &str, type_info: Arc<CalcitTypeAnnotation>) -> Calcit {
    let sym: Arc<str> = Arc::from(name);
    Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&sym),
      sym,
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.async"),
        at_def: Arc::from("main!"),
      }),
      location: None,
      type_info,
    })
  }

  #[test]
  fn lowered_trait_contract_requires_unique_nominal_origin_and_proven_arguments() {
    let generic = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")));
    let signature = Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![generic, Arc::new(CalcitTypeAnnotation::String)],
      return_type: Arc::new(CalcitTypeAnnotation::Number),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(HashSet::new()),
    })));
    let origin = CalcitTrait::new_runtime(EdnTag::from("Render"), vec![EdnTag::from("render")], vec![signature.clone()]);
    let foreign = CalcitTrait::new_runtime(EdnTag::from("Render"), vec![EdnTag::from("render")], vec![signature]);
    let receiver = |traits| typed_local("receiver", Arc::new(CalcitTypeAnnotation::TraitSet(Arc::new(traits))));
    let call = |receiver, argument| {
      proc_call(
        CalcitProc::NativeTraitCall,
        vec![
          Calcit::Trait(origin.clone()),
          Calcit::Tag(EdnTag::from("render")),
          receiver,
          argument,
        ],
      )
    };
    let matching = receiver(vec![Arc::new(origin.clone())]);
    assert_eq!(
      infer_type_from_expr(&call(matching.clone(), Calcit::Str(Arc::from("prefix"))), &ScopeTypes::new()),
      Some(Arc::new(CalcitTypeAnnotation::Number))
    );
    for invalid in [
      call(receiver(vec![Arc::new(foreign)]), Calcit::Str(Arc::from("prefix"))),
      call(
        receiver(vec![Arc::new(origin.clone()), Arc::new(origin.clone())]),
        Calcit::Str(Arc::from("prefix")),
      ),
      call(matching.clone(), Calcit::Number(1.0)),
      call(matching, typed_local("open", calcit::DYNAMIC_TYPE.clone())),
    ] {
      assert_eq!(
        infer_type_from_expr(&invalid, &ScopeTypes::new()),
        Some(calcit::DYNAMIC_TYPE.clone())
      );
    }
  }

  #[test]
  fn async_callable_alias_requires_await_for_its_logical_value() {
    let signature = CalcitFnTypeAnnotation {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![],
      return_type: Arc::new(CalcitTypeAnnotation::String),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(HashSet::new()),
    };
    let async_fn = Arc::new(CalcitTypeAnnotation::Fn(Arc::new(signature.with_async_invocation())));
    let invocation = Calcit::from(vec![typed_local("load-text", async_fn)]);

    let pending = infer_type_from_expr(&invocation, &ScopeTypes::new()).expect("async invocation type");
    assert!(is_pending_async_value(pending.as_ref()));
    assert!(!pending.is_compatible_with(&CalcitTypeAnnotation::String));

    let awaited = Calcit::from(vec![symbol("js-await"), invocation]);
    assert!(matches!(
      infer_type_from_expr(&awaited, &ScopeTypes::new()).as_deref(),
      Some(CalcitTypeAnnotation::String)
    ));
  }

  #[test]
  fn sync_callable_control_keeps_its_logical_return() {
    let sync_fn = Arc::new(CalcitTypeAnnotation::from_function_parts(
      vec![],
      Arc::new(CalcitTypeAnnotation::String),
    ));
    let invocation = Calcit::from(vec![typed_local("read-text", sync_fn)]);

    assert!(matches!(
      infer_type_from_expr(&invocation, &ScopeTypes::new()).as_deref(),
      Some(CalcitTypeAnnotation::String)
    ));
  }

  fn symbol(name: &str) -> Calcit {
    Calcit::Symbol {
      sym: Arc::from(name),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.js-ffi"),
        at_def: Arc::from("demo"),
      }),
      location: None,
    }
  }

  fn local(name: &str, type_info: Arc<CalcitTypeAnnotation>) -> Calcit {
    let sym: Arc<str> = Arc::from(name);
    Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&sym),
      sym,
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.js-ffi"),
        at_def: Arc::from("demo"),
      }),
      location: None,
      type_info,
    })
  }

  fn assert_js_nullish_host_value(value: Option<Arc<CalcitTypeAnnotation>>) {
    assert!(matches!(
      value.as_deref(),
      Some(CalcitTypeAnnotation::JsNullish(inner)) if matches!(inner.as_ref(), CalcitTypeAnnotation::JsObject)
    ));
  }

  #[test]
  fn function_exit_evidence_keeps_tail_transfers_out_of_expression_types() {
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let recur = |arguments: Vec<Calcit>| {
      let mut call = vec![Calcit::Proc(CalcitProc::Recur)];
      call.extend(arguments);
      Calcit::from(call)
    };
    let branch = |left, right| {
      Calcit::from(vec![
        Calcit::Syntax(CalcitSyntax::If, "tests.exits".into()),
        Calcit::Bool(true),
        left,
        right,
      ])
    };
    let parameters = [number.clone()];
    let mut scope = ScopeTypes::new();
    scope.insert(Arc::from("open"), calcit::DYNAMIC_TYPE.clone());
    let transfer = recur(vec![Calcit::Number(2.0)]);
    let live = branch(Calcit::Number(1.0), transfer.clone());
    assert_eq!(infer_function_exit_type(&live, &scope, &parameters), Some(number.clone()));
    assert!(
      resolve_type_value(&live, &scope).is_none(),
      "ordinary expressions do not acquire recur authority"
    );
    assert!(
      infer_function_exit_type(&transfer, &scope, &parameters).is_none(),
      "a cycle is not independent return evidence"
    );
    for invalid in [
      recur(vec![]),
      recur(vec![Calcit::Str("bad".into())]),
      recur(vec![local("open", calcit::DYNAMIC_TYPE.clone())]),
    ] {
      assert!(infer_function_exit_type(&branch(Calcit::Number(1.0), invalid), &scope, &parameters).is_none());
    }
    let open = local("open", calcit::DYNAMIC_TYPE.clone());
    assert_eq!(
      infer_function_exit_type(&branch(open, transfer), &scope, &parameters),
      Some(calcit::DYNAMIC_TYPE.clone())
    );
    assert!(infer_function_exit_type(&branch(Calcit::Number(1.0), Calcit::Str("bad".into())), &scope, &parameters).is_none());
    let generic = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")));
    scope.insert(Arc::from("item"), generic.clone());
    let generic_parameters = [generic.clone()];
    let same_instantiation = branch(Calcit::Number(1.0), recur(vec![local("item", generic)]));
    assert_eq!(
      infer_function_exit_type(&same_instantiation, &scope, &generic_parameters),
      Some(number)
    );
    let changed_instantiation = branch(Calcit::Number(1.0), recur(vec![Calcit::Number(2.0)]));
    assert!(infer_function_exit_type(&changed_instantiation, &scope, &generic_parameters).is_none());
  }

  #[test]
  fn foldl_returns_the_initial_accumulator_type() {
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let expression = proc_call(
      CalcitProc::Foldl,
      vec![
        local("items", Arc::new(CalcitTypeAnnotation::Set(Arc::new(CalcitTypeAnnotation::String)))),
        Calcit::Number(0.0),
        local(
          "reducer",
          Arc::new(CalcitTypeAnnotation::from_function_parts(
            vec![number.clone(), Arc::new(CalcitTypeAnnotation::String)],
            number,
          )),
        ),
      ],
    );

    assert!(matches!(
      infer_type_from_expr(&expression, &ScopeTypes::new()).as_deref(),
      Some(CalcitTypeAnnotation::Number)
    ));

    let dynamic_reducer = proc_call(
      CalcitProc::Foldl,
      vec![
        local("items", Arc::new(CalcitTypeAnnotation::Set(Arc::new(CalcitTypeAnnotation::String)))),
        Calcit::Number(0.0),
        local("reducer", Arc::new(CalcitTypeAnnotation::DynFn)),
      ],
    );
    assert!(matches!(
      infer_type_from_expr(&dynamic_reducer, &ScopeTypes::new()).as_deref(),
      Some(CalcitTypeAnnotation::Dynamic)
    ));
  }

  #[test]
  fn dynamic_if_branch_does_not_authorize_a_concrete_merge() {
    assert!(matches!(
      merge_if_branch_types(Arc::new(CalcitTypeAnnotation::Number), calcit::DYNAMIC_TYPE.clone()).as_deref(),
      Some(CalcitTypeAnnotation::Dynamic)
    ));
    assert!(matches!(
      merge_if_branch_types(calcit::DYNAMIC_TYPE.clone(), Arc::new(CalcitTypeAnnotation::Number)).as_deref(),
      Some(CalcitTypeAnnotation::Dynamic)
    ));
    let open_map = Arc::new(CalcitTypeAnnotation::Map(
      calcit::DYNAMIC_TYPE.clone(),
      calcit::DYNAMIC_TYPE.clone(),
    ));
    for (left, right) in [
      (open_map.clone(), calcit::DYNAMIC_TYPE.clone()),
      (calcit::DYNAMIC_TYPE.clone(), open_map),
    ] {
      assert!(matches!(
        merge_if_branch_types(left, right).as_deref(),
        Some(CalcitTypeAnnotation::Dynamic)
      ));
    }
  }

  #[test]
  fn direct_raise_if_branch_preserves_the_live_type() {
    let option_number = Arc::new(CalcitTypeAnnotation::TypeRef(
      Arc::from("calcit.core/Option"),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]),
    ));
    let live = local("value", option_number.clone());
    let raise = proc_call(CalcitProc::Raise, vec![Calcit::Str(Arc::from("missing"))]);
    let if_expr = |true_branch: Calcit, false_branch: Calcit| {
      CalcitList::from(&[
        Calcit::Syntax(CalcitSyntax::If, Arc::from("tests.raise")),
        Calcit::Bool(true),
        true_branch,
        false_branch,
      ] as &[Calcit])
    };

    assert_eq!(
      infer_if_return_type(&if_expr(live.clone(), raise.clone()), &ScopeTypes::new()),
      Some(option_number.clone())
    );
    assert_eq!(
      infer_if_return_type(&if_expr(raise.clone(), live.clone()), &ScopeTypes::new()),
      Some(option_number.clone())
    );
    let wrapped = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::CoreLet, Arc::from("tests.raise")),
      Calcit::from(Vec::<Calcit>::new()),
      Calcit::Nil,
      raise.clone(),
    ]);
    assert_eq!(
      infer_function_exit_type(
        &Calcit::List(if_expr(wrapped.clone(), live.clone()).into()),
        &ScopeTypes::new(),
        &[]
      ),
      Some(option_number.clone())
    );
    assert!(expression_definitely_diverges(&Calcit::List(
      if_expr(wrapped.clone(), raise).into()
    )));
    assert!(!expression_definitely_diverges(&Calcit::List(
      if_expr(wrapped, live.clone()).into()
    )));
    assert!(!expression_definitely_diverges(&proc_call(CalcitProc::Recur, vec![])));
    let local_raise = Calcit::from(vec![
      local("raise", calcit::DYNAMIC_TYPE.clone()),
      Calcit::Str(Arc::from("returns")),
    ]);
    assert!(!expression_definitely_diverges(&local_raise));
    assert_ne!(
      infer_if_return_type(&if_expr(live, local("open", calcit::DYNAMIC_TYPE.clone())), &ScopeTypes::new()),
      Some(option_number)
    );
  }

  #[test]
  fn option_join_only_ignores_a_known_empty_constructor() {
    let option = |payload| {
      Arc::new(CalcitTypeAnnotation::TypeRef(
        Arc::from("calcit.core/Option"),
        Arc::new(vec![payload]),
      ))
    };
    let concrete = option(Arc::new(CalcitTypeAnnotation::Number));
    let open = option(calcit::DYNAMIC_TYPE.clone());
    let value = Calcit::Registered(Arc::from("unknown-option"));
    let none = Calcit::from(vec![Calcit::Registered(Arc::from("calcit.core/%none"))]);
    assert_eq!(merge_option_absence_branch(&none, &open, &value, &concrete), Some(concrete.clone()));
    assert_eq!(merge_option_absence_branch(&value, &concrete, &none, &open), Some(concrete.clone()));
    assert!(merge_option_absence_branch(&value, &concrete, &value, &open).is_none());
    assert_eq!(merge_if_branch_types(concrete, open.clone()), Some(open));
  }

  #[test]
  fn empty_option_fallback_does_not_narrow_an_open_payload() {
    let open = CalcitTypeAnnotation::TypeRef(Arc::from("calcit.core/Option"), Arc::new(vec![calcit::DYNAMIC_TYPE.clone()]));
    let expected = CalcitTypeAnnotation::TypeRef(
      Arc::from("calcit.core/Option"),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")))]),
    );
    let none = Calcit::from(vec![Calcit::Registered(Arc::from("calcit.core/%none"))]);
    let some = Calcit::from(vec![Calcit::Registered(Arc::from("calcit.core/%some")), Calcit::Number(1.0)]);
    let shadowed = Calcit::from(vec![Calcit::Registered(Arc::from("app/%none"))]);
    assert!(core_option_none_without_payload(&none, &open, &expected));
    assert!(!core_option_none_without_payload(&some, &open, &expected));
    assert!(!core_option_none_without_payload(&shadowed, &open, &expected));
    assert!(!core_option_none_without_payload(&none, &CalcitTypeAnnotation::Dynamic, &expected));
  }

  #[test]
  fn known_result_error_binds_only_error_payload() {
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let actual = CalcitTypeAnnotation::TypeRef(
      Arc::from("calcit.core/Result"),
      Arc::new(vec![calcit::DYNAMIC_TYPE.clone(), number.clone()]),
    );
    let expected = CalcitTypeAnnotation::TypeRef(
      Arc::from("calcit.core/Result"),
      Arc::new(vec![
        Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T"))),
        Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("E"))),
      ]),
    );
    let err = Calcit::from(vec![Calcit::Registered(Arc::from("calcit.core/%err")), Calcit::Number(3.0)]);
    let ok = Calcit::from(vec![Calcit::Registered(Arc::from("calcit.core/%ok")), Calcit::Number(3.0)]);
    let shadowed = Calcit::from(vec![Calcit::Registered(Arc::from("app/%err")), Calcit::Number(3.0)]);
    let mut bindings = HashMap::new();
    assert_eq!(
      bind_core_result_err_without_ok_payload(&err, &actual, &expected, &mut bindings),
      Some(true)
    );
    assert_eq!(bindings.get("E"), Some(&number));
    assert!(!bindings.contains_key("T"));
    assert_eq!(
      bind_core_result_err_without_ok_payload(&ok, &actual, &expected, &mut HashMap::new()),
      None
    );
    assert_eq!(
      bind_core_result_err_without_ok_payload(&shadowed, &actual, &expected, &mut HashMap::new()),
      None
    );
    assert_eq!(
      bind_core_result_err_without_ok_payload(&err, &CalcitTypeAnnotation::Dynamic, &expected, &mut HashMap::new()),
      None
    );
  }

  #[test]
  fn if_join_widens_payload_branches_back_into_the_option_wrapper() {
    let option = |payload| {
      Arc::new(CalcitTypeAnnotation::TypeRef(
        Arc::from("calcit.core/Option"),
        Arc::new(vec![payload]),
      ))
    };
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let option_number = option(number.clone());

    // A plain payload branch joined with its Option form keeps the Option so
    // absence is not erased before arithmetic checks run.
    assert_eq!(
      merge_if_branch_types(number.clone(), option_number.clone()),
      Some(option_number.clone())
    );
    assert_eq!(
      merge_if_branch_types(option_number.clone(), number.clone()),
      Some(option_number.clone())
    );

    // An internal Optional wrapper is kept by the corrected compatibility
    // direction instead of being dropped for the plain payload.
    let optional_number = Arc::new(CalcitTypeAnnotation::Optional(number.clone()));
    assert_eq!(
      merge_if_branch_types(number.clone(), optional_number.clone()),
      Some(optional_number)
    );

    // Open payloads stay open instead of narrowing to one concrete branch.
    let option_dynamic = option(calcit::DYNAMIC_TYPE.clone());
    assert_eq!(merge_if_branch_types(number.clone(), option_dynamic.clone()), Some(option_dynamic));

    // Unrelated payloads must not fabricate a shared Option.
    let option_string = option(Arc::new(CalcitTypeAnnotation::String));
    assert!(merge_if_branch_types(number, option_string).is_none());
  }

  #[test]
  fn nominal_branch_participation_preserves_used_dynamic_and_open_values() {
    let t = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")));
    let e = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("E")));
    let mut schema = CalcitStructDef::from_fields(
      EdnTag::from("Choice"),
      vec![EdnTag::from("left"), EdnTag::from("right"), EdnTag::from("empty")],
    );
    schema.generics = Arc::new(vec![Arc::from("T"), Arc::from("E")]);
    let definition = CalcitEnumDef::from_struct(CalcitStructValue {
      struct_ref: Arc::new(schema),
      values: Arc::new(vec![
        Calcit::from(vec![t.to_calcit()]),
        Calcit::from(vec![e.to_calcit()]),
        Calcit::from(Vec::<Calcit>::new()),
      ]),
    })
    .unwrap();
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let string = Arc::new(CalcitTypeAnnotation::String);
    let annotation = |left, right| {
      Arc::new(CalcitTypeAnnotation::Enum(
        Arc::new(definition.clone()),
        Arc::new(vec![left, right]),
      ))
    };
    let left_type = annotation(number.clone(), crate::calcit::type_annotation::NEVER_TYPE.clone());
    let right_type = annotation(crate::calcit::type_annotation::NEVER_TYPE.clone(), string.clone());
    let empty_type = annotation(
      crate::calcit::type_annotation::NEVER_TYPE.clone(),
      crate::calcit::type_annotation::NEVER_TYPE.clone(),
    );
    let open_type = annotation(calcit::DYNAMIC_TYPE.clone(), calcit::DYNAMIC_TYPE.clone());
    assert_eq!(
      merge_nominal_enum_branches([&empty_type, &left_type, &right_type]),
      Some(annotation(number, string.clone()))
    );
    assert_eq!(merge_nominal_enum_branches([&open_type, &right_type]), Some(open_type.clone()));
    let dynamic_left_type = annotation(calcit::DYNAMIC_TYPE.clone(), crate::calcit::type_annotation::NEVER_TYPE.clone());
    assert_eq!(
      merge_nominal_enum_branches([&dynamic_left_type, &right_type]),
      Some(annotation(calcit::DYNAMIC_TYPE.clone(), string))
    );
  }

  #[test]
  fn opposite_result_constructors_complete_only_their_missing_generic_slots() {
    let result = |ok: Arc<CalcitTypeAnnotation>, err: Arc<CalcitTypeAnnotation>| {
      Arc::new(CalcitTypeAnnotation::TypeRef(
        Arc::from("calcit.core/Result"),
        Arc::new(vec![ok, err]),
      ))
    };
    let ok_expr = Calcit::from(vec![Calcit::Registered(Arc::from("%ok")), Calcit::Number(1.0)]);
    let err_expr = Calcit::from(vec![Calcit::Registered(Arc::from("%err")), Calcit::Str(Arc::from("failed"))]);
    let ok_type = result(Arc::new(CalcitTypeAnnotation::Number), calcit::DYNAMIC_TYPE.clone());
    let err_type = result(calcit::DYNAMIC_TYPE.clone(), Arc::new(CalcitTypeAnnotation::String));
    let expected = result(Arc::new(CalcitTypeAnnotation::Number), Arc::new(CalcitTypeAnnotation::String));
    assert_eq!(
      merge_result_constructor_branches(&ok_expr, ok_type.as_ref(), &err_expr, err_type.as_ref()),
      Some(expected.clone())
    );
    assert_eq!(
      merge_result_constructor_branches(&err_expr, err_type.as_ref(), &ok_expr, ok_type.as_ref()),
      Some(expected)
    );
    assert!(merge_result_constructor_branches(&ok_expr, ok_type.as_ref(), &ok_expr, ok_type.as_ref()).is_none());
  }

  #[test]
  fn sort_preserves_the_concrete_list_type() {
    let string_list = Arc::new(CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::String)));
    for proc in [CalcitProc::Sort, CalcitProc::NativeListSort] {
      let expression = proc_call(proc, vec![local("items", string_list.clone())]);
      assert_eq!(infer_type_from_expr(&expression, &ScopeTypes::new()), Some(string_list.clone()));
    }
  }

  #[test]
  fn unhinted_callback_retains_a_concrete_literal_return_type() {
    let callback = CalcitList::from(
      &[
        Calcit::Syntax(CalcitSyntax::Defn, Arc::from("tests.callback")),
        symbol("callback"),
        Calcit::from(Vec::<Calcit>::new()),
        Calcit::Str(Arc::from("fallback")),
      ][..],
    );

    let inferred = infer_preprocessed_function_type(&callback);
    let CalcitTypeAnnotation::Fn(signature) = inferred.as_ref() else {
      panic!("an unhinted callback with a literal body should retain a function signature: {inferred:?}");
    };
    assert!(matches!(signature.return_type.as_ref(), CalcitTypeAnnotation::String));
  }

  #[test]
  fn unhinted_zero_arg_macro_retains_macro_kind() {
    let macro_form = CalcitList::from(
      &[
        Calcit::Syntax(CalcitSyntax::Defmacro, Arc::from("tests.callback")),
        symbol("callback"),
        Calcit::from(Vec::<Calcit>::new()),
        Calcit::Str(Arc::from("fallback")),
      ][..],
    );

    let inferred = infer_preprocessed_function_type(&macro_form);
    let CalcitTypeAnnotation::Fn(signature) = inferred.as_ref() else {
      panic!("an unhinted zero-argument macro should retain a macro signature: {inferred:?}");
    };
    assert_eq!(signature.fn_kind, SchemaKind::Macro);
  }

  #[test]
  fn unhinted_empty_function_remains_dynamic() {
    let empty_callback = CalcitList::from(
      &[
        Calcit::Syntax(CalcitSyntax::Defn, Arc::from("tests.callback")),
        symbol("callback"),
        Calcit::from(Vec::<Calcit>::new()),
      ][..],
    );

    assert!(matches!(
      infer_preprocessed_function_type(&empty_callback).as_ref(),
      CalcitTypeAnnotation::DynFn
    ));
  }

  #[test]
  fn unresolved_generic_payload_does_not_authorize_a_static_return() {
    let type_var: Arc<CalcitTypeAnnotation> = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")));
    let fn_info = CalcitFn {
      name: Arc::from("find"),
      def_ns: Arc::from(calcit::CORE_NS),
      def_ref: None,
      usage: CalcitFnUsageMeta::default(),
      scope: Arc::new(CalcitScope::default()),
      args: Arc::new(CalcitFnArgs::Args(vec![0])),
      call_shape: crate::calcit::CalcitFnCallShape::fixed(1),
      body: vec![],
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(vec![]),
      return_type: Arc::new(CalcitTypeAnnotation::TypeRef(
        Arc::from("calcit.core/Option"),
        Arc::new(vec![type_var.clone()]),
      )),
      arg_types: vec![Arc::new(CalcitTypeAnnotation::List(type_var))],
      rest_type: None,
    };
    let unknown_list = local("xs", calcit::DYNAMIC_TYPE.clone());
    assert!(resolve_generic_return_type(&fn_info, std::iter::once(&unknown_list), &ScopeTypes::new()).is_none());
  }

  #[test]
  fn direct_generic_argument_preserves_a_dynamic_local_boundary() {
    let payload = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")));
    let return_type = Arc::new(CalcitTypeAnnotation::TypeRef(
      Arc::from("calcit.core/Result"),
      Arc::new(vec![payload.clone(), Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("E")))]),
    ));
    let dynamic_value = local("value", calcit::DYNAMIC_TYPE.clone());
    let resolved = resolve_generic_return_type_parts(
      &[Arc::from("T"), Arc::from("E")],
      &[payload],
      None,
      &return_type,
      std::iter::once(&dynamic_value),
      &ScopeTypes::new(),
    )
    .expect("a direct generic argument should retain its Dynamic boundary");
    assert!(matches!(resolved.as_ref(),
      CalcitTypeAnnotation::TypeRef(name, args)
        if name.as_ref() == "calcit.core/Result"
          && matches!(args.as_slice(), [ok, err]
            if matches!(ok.as_ref(), CalcitTypeAnnotation::Dynamic)
              && matches!(err.as_ref(), CalcitTypeAnnotation::Dynamic))));
  }

  #[test]
  fn empty_literal_does_not_erase_a_generic_return_anchor() {
    let payload = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("U")));
    let concrete = Arc::new(CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::String)));
    let anchor = local("anchor", concrete.clone());
    let empty = Calcit::from(vec![Calcit::Proc(CalcitProc::List)]);
    let inferred = resolve_generic_return_type_parts(
      &[Arc::from("U")],
      &[payload.clone(), payload.clone()],
      None,
      &payload,
      [&empty, &anchor].into_iter(),
      &ScopeTypes::new(),
    )
    .expect("an empty literal has no member evidence to bind U");
    assert_eq!(inferred, concrete);
    let open = local("open", calcit::DYNAMIC_TYPE.clone());
    let inferred = resolve_generic_return_type_parts(
      &[Arc::from("U")],
      &[payload.clone(), payload.clone()],
      None,
      &payload,
      [&open, &anchor].into_iter(),
      &ScopeTypes::new(),
    )
    .expect("an explicit Dynamic is evidence, unlike an empty literal");
    assert_eq!(inferred, calcit::DYNAMIC_TYPE.clone());
  }

  #[test]
  fn later_dynamic_argument_widens_an_existing_generic_binding() {
    let payload = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")));
    let concrete = Calcit::Number(7.0);
    let dynamic_value = local("value", calcit::DYNAMIC_TYPE.clone());
    for arguments in [vec![&concrete, &dynamic_value], vec![&dynamic_value, &concrete]] {
      let inferred = resolve_generic_return_type_parts(
        &[Arc::from("T")],
        &[payload.clone(), payload.clone()],
        None,
        &payload,
        arguments.into_iter(),
        &ScopeTypes::new(),
      )
      .expect("a Dynamic argument must not retain a concrete generic binding");
      assert!(matches!(inferred.as_ref(), CalcitTypeAnnotation::Dynamic));
    }
  }

  #[test]
  fn variadic_generic_schema_preserves_collection_payloads() {
    let key_var = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("K")));
    let value_var = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("V")));
    let generic_map = Arc::new(CalcitTypeAnnotation::Map(key_var, value_var));
    let concrete_map = Arc::new(CalcitTypeAnnotation::Map(
      Arc::new(CalcitTypeAnnotation::String),
      Arc::new(CalcitTypeAnnotation::Number),
    ));
    let first = local("first", concrete_map.clone());
    let second = local("second", concrete_map);

    let inferred = resolve_generic_return_type_parts(
      &[Arc::from("K"), Arc::from("V")],
      std::slice::from_ref(&generic_map),
      Some(&generic_map),
      &generic_map,
      [&first, &second].into_iter(),
      &ScopeTypes::new(),
    )
    .expect("generic map payloads should survive fixed and rest arguments");

    assert!(matches!(
      inferred.as_ref(),
      CalcitTypeAnnotation::Map(key, value)
        if matches!(key.as_ref(), CalcitTypeAnnotation::String)
          && matches!(value.as_ref(), CalcitTypeAnnotation::Number)
    ));
  }

  #[test]
  fn generic_return_preserves_a_callers_symbolic_payload() {
    let type_var = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")));
    let generic_list = Arc::new(CalcitTypeAnnotation::List(type_var.clone()));
    let items = local("items", generic_list.clone());

    let inferred = resolve_generic_return_type_parts(
      &[Arc::from("T")],
      std::slice::from_ref(&generic_list),
      None,
      &generic_list,
      std::iter::once(&items),
      &ScopeTypes::new(),
    )
    .expect("a generic helper should preserve the caller's symbolic payload");

    assert!(matches!(
      inferred.as_ref(),
      CalcitTypeAnnotation::List(inner) if matches!(inner.as_ref(), CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "T")
    ));
  }

  #[test]
  fn apply_recovers_a_compatible_callable_return_type() {
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let callable = local(
      "combine",
      Arc::new(CalcitTypeAnnotation::from_function_parts(
        vec![number.clone(), number.clone()],
        Arc::new(CalcitTypeAnnotation::String),
      )),
    );
    let arguments = proc_call(CalcitProc::List, vec![Calcit::Number(1.0), Calcit::Number(2.0)]);
    let call = CalcitList::from(&[symbol("apply"), callable, arguments][..]);

    assert!(matches!(
      infer_core_apply_return_type(&call, &ScopeTypes::new()).as_deref(),
      Some(CalcitTypeAnnotation::String)
    ));
  }

  #[test]
  fn apply_does_not_infer_a_return_for_an_optional_callable() {
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let signature = Arc::new(CalcitTypeAnnotation::from_function_parts(
      vec![number],
      Arc::new(CalcitTypeAnnotation::String),
    ));
    let callable = local("maybe-format", Arc::new(CalcitTypeAnnotation::Optional(signature)));
    let arguments = proc_call(CalcitProc::List, vec![Calcit::Number(1.0)]);
    let call = CalcitList::from(&[symbol("apply"), callable, arguments][..]);

    assert!(infer_core_apply_return_type(&call, &ScopeTypes::new()).is_none());
  }

  #[test]
  fn optional_callable_heads_keep_their_absence_in_return_inference() {
    let signature = Arc::new(CalcitTypeAnnotation::from_function_parts(
      vec![Arc::new(CalcitTypeAnnotation::Number)],
      Arc::new(CalcitTypeAnnotation::String),
    ));
    let optional_callable = Arc::new(CalcitTypeAnnotation::Optional(signature));
    let direct_call = Calcit::from(vec![local("maybe-format", optional_callable.clone()), Calcit::Number(1.0)]);
    assert_eq!(
      infer_type_from_expr(&direct_call, &ScopeTypes::new()),
      Some(optional_callable.clone())
    );

    let provider = local(
      "provider",
      Arc::new(CalcitTypeAnnotation::from_function_parts(vec![], optional_callable.clone())),
    );
    let nested_call = Calcit::from(vec![Calcit::from(vec![provider]), Calcit::Number(1.0)]);
    assert_eq!(
      infer_type_from_expr(&nested_call, &ScopeTypes::new()),
      Some(optional_callable.clone())
    );

    let slot_name: Arc<str> = Arc::from("optional-callable-regression");
    calcit::push_type_slot_override(slot_name.clone(), optional_callable);
    let alias = Arc::new(CalcitTypeAnnotation::TypeSlot(slot_name.clone()));
    let alias_call = Calcit::from(vec![local("maybe-aliased", alias.clone()), Calcit::Number(1.0)]);
    assert_eq!(infer_type_from_expr(&alias_call, &ScopeTypes::new()), Some(alias));
    calcit::pop_type_slot_override(&slot_name);
  }

  #[test]
  fn apply_preserves_an_async_callable_pending_result() {
    let signature = CalcitFnTypeAnnotation {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![Arc::new(CalcitTypeAnnotation::String)],
      return_type: Arc::new(CalcitTypeAnnotation::String),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(HashSet::new()),
    }
    .with_async_invocation();
    let callable = local("load-text", Arc::new(CalcitTypeAnnotation::Fn(Arc::new(signature))));
    let arguments = proc_call(CalcitProc::List, vec![Calcit::Str(Arc::from("notes.txt"))]);
    let call = CalcitList::from(&[symbol("apply"), callable, arguments][..]);

    let inferred = infer_core_apply_return_type(&call, &ScopeTypes::new()).expect("async apply result");
    assert!(is_pending_async_value(inferred.as_ref()));
  }

  #[test]
  fn apply_binds_a_generic_callable_return_from_the_list_member() {
    let type_var = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")));
    let callable = local(
      "identity",
      Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
        generics: Arc::new(vec![Arc::from("T")]),
        where_bounds: Arc::new(vec![]),
        arg_types: vec![type_var.clone()],
        return_type: type_var,
        fn_kind: SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(HashSet::new()),
      }))),
    );
    let arguments = proc_call(CalcitProc::List, vec![Calcit::Number(1.0)]);
    let call = CalcitList::from(&[symbol("apply"), callable, arguments][..]);

    assert!(matches!(
      infer_core_apply_return_type(&call, &ScopeTypes::new()).as_deref(),
      Some(CalcitTypeAnnotation::Number)
    ));
  }

  #[test]
  fn apply_keeps_dynamic_when_a_homogeneous_list_cannot_satisfy_every_parameter() {
    let callable = local(
      "mixed",
      Arc::new(CalcitTypeAnnotation::from_function_parts(
        vec![Arc::new(CalcitTypeAnnotation::Number), Arc::new(CalcitTypeAnnotation::String)],
        Arc::new(CalcitTypeAnnotation::Bool),
      )),
    );
    let arguments = proc_call(CalcitProc::List, vec![Calcit::Number(1.0), Calcit::Number(2.0)]);
    let call = CalcitList::from(&[symbol("apply"), callable, arguments][..]);

    assert!(infer_core_apply_return_type(&call, &ScopeTypes::new()).is_none());
  }

  #[test]
  fn apply_keeps_dynamic_for_a_list_without_concrete_member_evidence() {
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let callable = local(
      "combine",
      Arc::new(CalcitTypeAnnotation::from_function_parts(
        vec![number.clone(), number],
        Arc::new(CalcitTypeAnnotation::String),
      )),
    );
    let arguments = proc_call(CalcitProc::List, vec![Calcit::Number(1.0), Calcit::Str(Arc::from("two"))]);
    let call = CalcitList::from(&[symbol("apply"), callable, arguments][..]);

    assert!(infer_core_apply_return_type(&call, &ScopeTypes::new()).is_none());
  }

  #[test]
  fn apply_keeps_dynamic_when_fixed_arity_cannot_be_proven() {
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let callable = local(
      "combine",
      Arc::new(CalcitTypeAnnotation::from_function_parts(
        vec![number.clone(), number.clone()],
        Arc::new(CalcitTypeAnnotation::String),
      )),
    );
    let unknown_length = local("arguments", Arc::new(CalcitTypeAnnotation::List(number.clone())));
    let too_short = proc_call(CalcitProc::List, vec![Calcit::Number(1.0)]);

    for arguments in [unknown_length, too_short] {
      let call = CalcitList::from(&[symbol("apply"), callable.clone(), arguments][..]);
      assert!(infer_core_apply_return_type(&call, &ScopeTypes::new()).is_none());
    }
  }

  #[test]
  fn apply_allows_unknown_length_for_a_rest_only_callable() {
    let type_var = Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")));
    let callable = local(
      "collect",
      Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
        generics: Arc::new(vec![Arc::from("T")]),
        where_bounds: Arc::new(vec![]),
        arg_types: vec![],
        return_type: Arc::new(CalcitTypeAnnotation::List(type_var.clone())),
        fn_kind: SchemaKind::Fn,
        rest_type: Some(type_var),
        features: Arc::new(HashSet::new()),
      }))),
    );
    let arguments = local(
      "arguments",
      Arc::new(CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Number))),
    );
    let call = CalcitList::from(&[symbol("apply"), callable, arguments][..]);

    assert!(matches!(
      infer_core_apply_return_type(&call, &ScopeTypes::new()).as_deref(),
      Some(CalcitTypeAnnotation::List(item)) if matches!(item.as_ref(), CalcitTypeAnnotation::Number)
    ));
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
  fn list_first_inference_preserves_nullable_result() {
    let list = proc_call(CalcitProc::List, vec![Calcit::Number(1.0), Calcit::Number(2.0)]);
    let first = proc_call(CalcitProc::NativeListFirst, vec![list]);

    assert!(matches!(
      infer_static_type_from_expr(&first).as_deref(),
      Some(CalcitTypeAnnotation::Optional(inner)) if matches!(inner.as_ref(), CalcitTypeAnnotation::Number)
    ));

    let empty_list = proc_call(CalcitProc::List, vec![]);
    let first_empty = proc_call(CalcitProc::NativeListFirst, vec![empty_list]);
    assert!(matches!(
      infer_static_type_from_expr(&first_empty).as_deref(),
      Some(CalcitTypeAnnotation::Optional(inner)) if matches!(inner.as_ref(), CalcitTypeAnnotation::Dynamic)
    ));
  }

  #[test]
  fn typed_decode_expression_has_declared_target_type() {
    let target = Calcit::Enum(calcit::CalcitEnumValue {
      tag: Arc::new(Calcit::tag("list")),
      extra: vec![Calcit::tag("number")],
      sum_type: None,
    });
    for syntax in [CalcitSyntax::ParseCirruEdnAs, CalcitSyntax::DecodeMapAs] {
      let expression = Calcit::from(vec![
        Calcit::Syntax(syntax, Arc::from(calcit::CORE_NS)),
        Calcit::Str(Arc::from("[] 1 2")),
        target.clone(),
      ]);

      assert!(matches!(
        infer_static_type_from_expr(&expression).as_deref(),
        Some(CalcitTypeAnnotation::List(inner)) if matches!(inner.as_ref(), CalcitTypeAnnotation::Number)
      ));
    }
  }

  #[test]
  fn safe_typed_decode_expression_has_result_target_type() {
    let target = Calcit::Enum(calcit::CalcitEnumValue {
      tag: Arc::new(Calcit::tag("list")),
      extra: vec![Calcit::tag("number")],
      sum_type: None,
    });
    for syntax in [CalcitSyntax::TryParseCirruEdnAs, CalcitSyntax::TryDecodeMapAs] {
      let expression = Calcit::from(vec![
        Calcit::Syntax(syntax, Arc::from(calcit::CORE_NS)),
        Calcit::Str(Arc::from("[] 1 2")),
        target.clone(),
      ]);

      let inferred = infer_static_type_from_expr(&expression).expect("safe decoder type");
      assert!(matches!(
        inferred.as_ref(),
        CalcitTypeAnnotation::TypeRef(name, args)
          if name.as_ref() == "calcit.core/Result"
            && matches!(args.first().map(AsRef::as_ref), Some(CalcitTypeAnnotation::List(inner)) if matches!(inner.as_ref(), CalcitTypeAnnotation::Number))
            && matches!(args.get(1).map(AsRef::as_ref), Some(CalcitTypeAnnotation::String))
      ));
    }
  }

  #[test]
  fn assert_type_expression_has_declared_target_type() {
    let target = Calcit::Enum(calcit::CalcitEnumValue {
      tag: Arc::new(Calcit::tag("list")),
      extra: vec![Calcit::tag("number")],
      sum_type: None,
    });
    let expression = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::AssertType, Arc::from(calcit::CORE_NS)),
      Calcit::Nil,
      target,
    ]);

    assert!(matches!(
      infer_static_type_from_expr(&expression).as_deref(),
      Some(CalcitTypeAnnotation::List(inner)) if matches!(inner.as_ref(), CalcitTypeAnnotation::Number)
    ));
  }

  #[test]
  fn heterogeneous_collection_and_atom_inference_keep_safe_boundaries() {
    let mixed = proc_call(CalcitProc::List, vec![Calcit::Number(1.0), Calcit::Str(Arc::from("x"))]);
    let atom = proc_call(CalcitProc::Ref, vec![Calcit::Number(1.0)]);

    assert!(matches!(
      infer_static_type_from_expr(&mixed).as_deref(),
      Some(CalcitTypeAnnotation::List(inner)) if matches!(inner.as_ref(), CalcitTypeAnnotation::Dynamic)
    ));
    assert!(matches!(
      infer_static_type_from_expr(&atom).as_deref(),
      Some(CalcitTypeAnnotation::Ref(inner)) if matches!(inner.as_ref(), CalcitTypeAnnotation::Number)
    ));
  }

  #[test]
  fn defatom_expression_preserves_initializer_type() {
    let expression = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::Defref, Arc::from(calcit::CORE_NS)),
      symbol("*counter"),
      Calcit::Number(0.0),
    ]);

    assert!(matches!(
      infer_static_type_from_expr(&expression).as_deref(),
      Some(CalcitTypeAnnotation::Ref(inner)) if matches!(inner.as_ref(), CalcitTypeAnnotation::Number)
    ));
  }

  #[test]
  fn js_ffi_reads_and_native_calls_keep_js_nullish_opaque_types() {
    let receiver = local("host", Arc::new(CalcitTypeAnnotation::JsObject));
    let access = Calcit::from(vec![
      Calcit::Method(Arc::from("value"), calcit::MethodKind::Access),
      receiver.clone(),
    ]);
    let optional_access = Calcit::from(vec![
      Calcit::Method(Arc::from("value"), calcit::MethodKind::AccessOptional),
      receiver.clone(),
    ]);
    let native_call = Calcit::from(vec![
      Calcit::Method(Arc::from("read"), calcit::MethodKind::InvokeNative),
      receiver.clone(),
    ]);
    let indexed_read = Calcit::from(vec![symbol("aget"), receiver, Calcit::Str(Arc::from("value"))]);
    let global_call = Calcit::from(vec![symbol("js/Date.now")]);
    let raw_global = Calcit::RawCode(calcit::RawCodeType::Js, Arc::from("Date.now"));
    let raw_global_call = Calcit::from(vec![raw_global.clone()]);

    for expression in [
      access,
      optional_access,
      native_call,
      indexed_read,
      global_call,
      raw_global,
      raw_global_call,
    ] {
      assert_js_nullish_host_value(infer_static_type_from_expr(&expression));
    }

    let opaque = CalcitTypeAnnotation::JsObject;
    assert!(opaque.is_compatible_with(&CalcitTypeAnnotation::JsObject));
    assert!(!opaque.is_compatible_with(&CalcitTypeAnnotation::String));
    assert!(!opaque.is_compatible_with(&CalcitTypeAnnotation::Number));
  }

  #[test]
  fn js_typeof_has_a_concrete_string_result() {
    let value = local("host", js_nullish_host_value_type());
    let typeof_call = Calcit::from(vec![symbol("js/typeof"), value]);
    let processed_typeof_call = Calcit::from(vec![
      Calcit::RawCode(calcit::RawCodeType::Js, Arc::from("typeof")),
      local("host", js_nullish_host_value_type()),
    ]);

    for expression in [typeof_call, processed_typeof_call] {
      assert!(matches!(
        infer_static_type_from_expr(&expression).as_deref(),
        Some(CalcitTypeAnnotation::String)
      ));
    }
  }

  #[test]
  fn common_get_does_not_specialize_struct_fields() {
    let mut struct_def = CalcitStructDef::from_fields(EdnTag::from("User"), vec![EdnTag::from("name")]);
    struct_def.field_types = Arc::new(vec![Arc::new(CalcitTypeAnnotation::String)]);
    let user_type = CalcitTypeAnnotation::Struct(Arc::new(struct_def), Arc::new(vec![]));
    let key = Calcit::Tag(EdnTag::from("name"));

    let user_args = CalcitList::from(&[local("user", Arc::new(user_type.clone())), key.clone()] as &[Calcit]);
    assert!(resolve_checked_call_contract(calcit::CORE_NS, "get", &user_args, &ScopeTypes::new()).is_none());
    let optional_args = CalcitList::from(&[
      local("maybe-user", Arc::new(CalcitTypeAnnotation::Optional(Arc::new(user_type)))),
      key,
    ] as &[Calcit]);
    assert!(
      resolve_checked_call_contract(calcit::CORE_NS, "get", &optional_args, &ScopeTypes::new()).is_none(),
      "legacy Optional receivers must be narrowed or converted before nominal lookup"
    );
  }

  #[test]
  fn typed_collection_calls_infer_from_the_checked_contract() {
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let string = Arc::new(CalcitTypeAnnotation::String);
    let list_type = Arc::new(CalcitTypeAnnotation::List(number.clone()));
    let mapper_type = Arc::new(CalcitTypeAnnotation::from_function_parts(vec![number.clone()], string.clone()));
    let map_call = CalcitList::from(&[symbol("map"), local("items", list_type), local("render", mapper_type)] as &[Calcit]);
    assert_eq!(
      infer_return_type_from_compiled_callable(calcit::CORE_NS, "map", &map_call, &ScopeTypes::new()),
      Some(Arc::new(CalcitTypeAnnotation::List(string.clone())))
    );

    let map_type = Arc::new(CalcitTypeAnnotation::Map(string.clone(), number.clone()));
    let get_call = CalcitList::from(&[symbol("get"), local("counts", map_type.clone()), Calcit::Str(Arc::from("a"))] as &[Calcit]);
    assert_eq!(
      infer_return_type_from_compiled_callable(calcit::CORE_NS, "get", &get_call, &ScopeTypes::new()),
      Some(core_type_ref("Option", vec![number.clone()]))
    );

    let predicate_type = Arc::new(CalcitTypeAnnotation::from_function_parts(
      vec![number.clone()],
      Arc::new(CalcitTypeAnnotation::Bool),
    ));
    let set_type = Arc::new(CalcitTypeAnnotation::Set(number));
    let filter_call =
      CalcitList::from(&[symbol("filter"), local("ids", set_type.clone()), local("positive?", predicate_type)] as &[Calcit]);
    assert_eq!(
      infer_return_type_from_compiled_callable(calcit::CORE_NS, "filter", &filter_call, &ScopeTypes::new()),
      Some(set_type)
    );

    let updater_type = Arc::new(CalcitTypeAnnotation::from_function_parts(
      vec![Arc::new(CalcitTypeAnnotation::Number)],
      Arc::new(CalcitTypeAnnotation::Number),
    ));
    let update_call = CalcitList::from(&[
      symbol("update"),
      local("counts", map_type.clone()),
      Calcit::Str(Arc::from("a")),
      local("increment", updater_type),
    ] as &[Calcit]);
    assert_eq!(
      infer_return_type_from_compiled_callable(calcit::CORE_NS, "update", &update_call, &ScopeTypes::new()),
      Some(map_type)
    );
  }

  #[test]
  fn get_in_stops_before_struct_boundaries() {
    let mut struct_def = CalcitStructDef::from_fields(EdnTag::from("User"), vec![EdnTag::from("name")]);
    struct_def.field_types = Arc::new(vec![Arc::new(CalcitTypeAnnotation::String)]);
    let user_type = Arc::new(CalcitTypeAnnotation::Struct(Arc::new(struct_def), Arc::new(vec![])));
    let users_type = Arc::new(CalcitTypeAnnotation::Map(tag_annotation("tag"), user_type.clone()));
    let path = proc_call(
      CalcitProc::List,
      vec![Calcit::Tag(EdnTag::from("user")), Calcit::Tag(EdnTag::from("name"))],
    );
    let call = CalcitList::from(&[symbol("get-in"), local("users", users_type.clone()), path.clone()] as &[Calcit]);

    assert!(
      infer_core_get_in_return_type(&call, &ScopeTypes::new()).is_none(),
      "get-in must not infer through a Struct field"
    );
    assert!(matches!(
      find_struct_lookup_in_literal_path(users_type.as_ref(), &path),
      Some((1, Calcit::Tag(field))) if field.ref_str() == "name"
    ));
    assert!(fully_typed_literal_lookup_path(users_type.as_ref(), &path).is_none());
    assert!(fully_typed_literal_assoc_path(users_type.as_ref(), &path).is_none());

    let empty_path = proc_call(CalcitProc::List, vec![]);
    let empty_call = CalcitList::from(&[symbol("get-in"), local("user", user_type.clone()), empty_path.clone()] as &[Calcit]);
    assert_eq!(
      infer_core_get_in_return_type(&empty_call, &ScopeTypes::new()),
      Some(core_type_ref("Option", vec![user_type.clone()])),
      "an empty path does not access a Struct field"
    );
    assert!(fully_typed_literal_lookup_path(user_type.as_ref(), &empty_path).is_none());
    assert!(fully_typed_literal_assoc_path(user_type.as_ref(), &empty_path).is_none());
  }

  #[test]
  fn get_in_recovers_literal_map_path_payload() {
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let nested_map = Arc::new(CalcitTypeAnnotation::Map(
      tag_annotation("tag"),
      Arc::new(CalcitTypeAnnotation::Map(tag_annotation("tag"), number.clone())),
    ));
    let path = proc_call(
      CalcitProc::List,
      vec![Calcit::Tag(EdnTag::from("session")), Calcit::Tag(EdnTag::from("revision"))],
    );
    let call = CalcitList::from(&[symbol("get-in"), local("store", nested_map), path] as &[Calcit]);

    assert_eq!(
      infer_core_get_in_return_type(&call, &ScopeTypes::new()),
      Some(core_type_ref("Option", vec![number])),
      "a fully typed literal path must preserve its final payload in Option"
    );
  }

  #[test]
  fn typed_literal_paths_require_static_non_struct_hops() {
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let nested_map = Arc::new(CalcitTypeAnnotation::Map(
      tag_annotation("tag"),
      Arc::new(CalcitTypeAnnotation::Map(tag_annotation("tag"), number)),
    ));
    let path = proc_call(
      CalcitProc::List,
      vec![Calcit::Tag(EdnTag::from("user")), Calcit::Tag(EdnTag::from("score"))],
    );

    assert_eq!(
      fully_typed_literal_lookup_path(nested_map.as_ref(), &path).map(|xs| xs.len()),
      Some(2)
    );
    assert_eq!(
      fully_typed_literal_assoc_path(nested_map.as_ref(), &path).map(|xs| xs.len()),
      Some(2)
    );

    let dynamic_map = CalcitTypeAnnotation::Map(tag_annotation("tag"), calcit::DYNAMIC_TYPE.clone());
    assert!(fully_typed_literal_lookup_path(&dynamic_map, &path).is_none());
    assert!(fully_typed_literal_assoc_path(&dynamic_map, &path).is_none());
    let one_step_path = proc_call(CalcitProc::List, vec![Calcit::Tag(EdnTag::from("user"))]);
    assert!(fully_typed_literal_lookup_path(&dynamic_map, &one_step_path).is_some());
    assert!(fully_typed_literal_assoc_path(&dynamic_map, &one_step_path).is_none());

    let string_path = proc_call(CalcitProc::List, vec![Calcit::Number(0.0)]);
    assert!(fully_typed_literal_lookup_path(&CalcitTypeAnnotation::String, &string_path).is_some());
    assert!(fully_typed_literal_assoc_path(&CalcitTypeAnnotation::String, &string_path).is_none());
  }

  #[test]
  fn core_let_tail_uses_initializer_evidence_in_lexical_scope() {
    // This constructs the internal lowered representation: a source let usually
    // annotates its local before this inference entrypoint sees it.
    let mut outer = ScopeTypes::new();
    outer.insert(Arc::from("x"), Arc::new(CalcitTypeAnnotation::Number));
    for (initializer, expected) in [
      (Calcit::Number(1.0), Arc::new(CalcitTypeAnnotation::Number)),
      (Calcit::Str(Arc::from("text")), Arc::new(CalcitTypeAnnotation::String)),
      (local("unknown", calcit::DYNAMIC_TYPE.clone()), calcit::DYNAMIC_TYPE.clone()),
    ] {
      let binder = local("x", calcit::DYNAMIC_TYPE.clone());
      let expression = Calcit::from(vec![
        Calcit::Syntax(CalcitSyntax::CoreLet, Arc::from("tests.let-scope")),
        Calcit::from(vec![binder.clone(), initializer]),
        binder,
      ]);
      assert_eq!(infer_type_from_expr(&expression, &outer), Some(expected));
    }
  }

  #[test]
  fn preprocessed_match_preserves_merged_branch_type() {
    let option_number = core_type_ref("Option", vec![Arc::new(CalcitTypeAnnotation::Number)]);
    let branch_value = |name: &str| local(name, option_number.clone());
    let match_expr = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::Match, Arc::from("tests.match-type")),
      Calcit::Tag(EdnTag::from("some")),
      Calcit::from(vec![Calcit::Tag(EdnTag::from("some")), branch_value("some-result")]),
      Calcit::from(vec![Calcit::Tag(EdnTag::from("none")), branch_value("none-result")]),
    ]);

    assert_eq!(infer_type_from_expr(&match_expr, &ScopeTypes::new()), Some(option_number));
  }

  #[test]
  fn preprocessed_match_skips_only_proven_raise_branches() {
    let option_number = core_type_ref("Option", vec![Arc::new(CalcitTypeAnnotation::Number)]);
    let live = local("value", option_number.clone());
    let raise = proc_call(CalcitProc::Raise, vec![Calcit::Str(Arc::from("missing"))]);
    let match_expr = |first: Calcit, second: Calcit| {
      Calcit::from(vec![
        Calcit::Syntax(CalcitSyntax::Match, Arc::from("tests.match-raise")),
        Calcit::Tag(EdnTag::from("some")),
        Calcit::from(vec![Calcit::Tag(EdnTag::from("some")), first]),
        Calcit::from(vec![Calcit::Tag(EdnTag::from("none")), second]),
      ])
    };

    assert_eq!(
      infer_type_from_expr(&match_expr(live.clone(), raise.clone()), &ScopeTypes::new()),
      Some(option_number.clone())
    );
    assert_eq!(
      infer_type_from_expr(&match_expr(raise, live.clone()), &ScopeTypes::new()),
      Some(option_number.clone())
    );
    assert_ne!(
      infer_type_from_expr(&match_expr(live, local("open", calcit::DYNAMIC_TYPE.clone())), &ScopeTypes::new()),
      Some(option_number)
    );
  }

  #[test]
  fn indexed_match_preserves_live_type_without_narrowing_dynamic() {
    let enum_def = CalcitEnumDef::from_struct(CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef::from_fields(
        EdnTag::from("Status"),
        vec![EdnTag::from("ok"), EdnTag::from("err")],
      )),
      values: Arc::new(vec![Calcit::from(vec![]), Calcit::from(vec![])]),
    })
    .expect("valid enum");
    let option_number = core_type_ref("Option", vec![Arc::new(CalcitTypeAnnotation::Number)]);
    let live = local("value", option_number.clone());
    let raise = proc_call(CalcitProc::Raise, vec![Calcit::Str(Arc::from("missing"))]);
    let branch = |tag: &str, value: Calcit| Calcit::from(vec![Calcit::from(vec![Calcit::Tag(EdnTag::from(tag))]), value]);
    let indexed = |other: Calcit| {
      Calcit::from(vec![
        Calcit::Syntax(CalcitSyntax::Match, Arc::from("tests.match-raise")),
        Calcit::Tag(EdnTag::from("ok")),
        Calcit::EnumDef(enum_def.clone()),
        Calcit::from(vec![branch("ok", live.clone()), branch("err", other), Calcit::Nil]),
      ])
    };

    assert_eq!(
      infer_type_from_expr(&indexed(raise), &ScopeTypes::new()),
      Some(option_number.clone())
    );
    assert_ne!(
      infer_type_from_expr(&indexed(local("open", calcit::DYNAMIC_TYPE.clone())), &ScopeTypes::new()),
      Some(option_number)
    );
  }

  #[test]
  fn struct_preserving_operations_keep_nominal_type_for_required_field_access() {
    let mut struct_def = CalcitStructDef::from_fields(EdnTag::from("User"), vec![EdnTag::from("name")]);
    struct_def.field_types = Arc::new(vec![Arc::new(CalcitTypeAnnotation::String)]);
    let struct_def = Arc::new(struct_def);
    let user_type = Arc::new(CalcitTypeAnnotation::StructValue(struct_def.clone()));
    let user = local("user", user_type.clone());
    let updater = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from("updater")),
      sym: Arc::from("updater"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.struct"),
        at_def: Arc::from("demo"),
      }),
      location: None,
      type_info: Arc::new(CalcitTypeAnnotation::from_function_parts(
        vec![Arc::new(CalcitTypeAnnotation::String)],
        Arc::new(CalcitTypeAnnotation::String),
      )),
    });
    let update_call = CalcitList::from(&[symbol("update"), user, Calcit::Tag(EdnTag::from("name")), updater] as &[Calcit]);

    assert_eq!(
      infer_return_type_from_compiled_callable(calcit::CORE_NS, "update", &update_call, &ScopeTypes::new()),
      Some(user_type.clone())
    );

    let from_map_call = proc_call(
      CalcitProc::NativeStructFromMap,
      vec![Calcit::StructDef(struct_def.as_ref().clone()), Calcit::Map(Default::default())],
    );
    assert_eq!(infer_type_from_expr(&from_map_call, &ScopeTypes::new()), Some(user_type));
  }

  #[test]
  fn typed_struct_updates_preserve_the_receiver_type() {
    let generic: Arc<str> = Arc::from("T");
    let mut struct_def = CalcitStructDef::from_fields(EdnTag::from("Box"), vec![EdnTag::from("value")]);
    struct_def.generics = Arc::new(vec![generic.clone()]);
    struct_def.field_types = Arc::new(vec![Arc::new(CalcitTypeAnnotation::TypeVar(generic))]);
    let receiver_type = Arc::new(CalcitTypeAnnotation::Struct(
      Arc::new(struct_def),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::String)]),
    ));
    let receiver = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from("box")),
      sym: Arc::from("box"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.struct"),
        at_def: Arc::from("demo"),
      }),
      location: None,
      type_info: receiver_type.clone(),
    });

    for call in [
      proc_call(
        CalcitProc::NativeStructAssoc,
        vec![receiver.clone(), Calcit::Tag(EdnTag::from("value")), Calcit::Str(Arc::from("next"))],
      ),
      proc_call(
        CalcitProc::NativeStructAssocAt,
        vec![
          receiver.clone(),
          Calcit::Number(0.0),
          Calcit::Tag(EdnTag::from("value")),
          Calcit::Str(Arc::from("next")),
        ],
      ),
      proc_call(
        CalcitProc::NativeStructWith,
        vec![receiver.clone(), Calcit::Tag(EdnTag::from("value")), Calcit::Str(Arc::from("next"))],
      ),
      proc_call(
        CalcitProc::NativeStructWithAt,
        vec![
          receiver,
          Calcit::Number(0.0),
          Calcit::Tag(EdnTag::from("value")),
          Calcit::Str(Arc::from("next")),
        ],
      ),
    ] {
      assert_eq!(infer_static_type_from_expr(&call).as_deref(), Some(receiver_type.as_ref()));
    }
  }

  #[test]
  fn enum_constructor_keeps_nominal_type_without_payload_evidence() {
    let generic: Arc<str> = Arc::from("T");
    let struct_def = CalcitStructDef {
      definition_ref: None,
      name: EdnTag::from("MaybeX"),
      fields: Arc::new(vec![EdnTag::from("none"), EdnTag::from("some")]),
      field_types: Arc::new(vec![calcit::DYNAMIC_TYPE.clone(), calcit::DYNAMIC_TYPE.clone()]),
      generics: Arc::new(vec![generic.clone()]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    };
    let enum_def = CalcitEnumDef::from_struct(CalcitStructValue {
      struct_ref: Arc::new(struct_def),
      values: Arc::new(vec![
        Calcit::from(CalcitList::default()),
        Calcit::from(vec![CalcitTypeAnnotation::TypeVar(generic).to_calcit()]),
      ]),
    })
    .expect("valid generic enum fixture");

    let nominal = Arc::new(CalcitTypeAnnotation::Enum(
      Arc::new(enum_def.clone()),
      Arc::new(vec![calcit::DYNAMIC_TYPE.clone()]),
    ));
    let expression = proc_call(CalcitProc::NativeEnumDefinition, vec![local("value", nominal.clone())]);
    assert!(matches!(infer_type_from_expr(&expression, &ScopeTypes::new()).as_deref(),
      Some(CalcitTypeAnnotation::EnumDef(definition)) if definition.name() == enum_def.name()));
    for open in [
      Arc::new(CalcitTypeAnnotation::Optional(nominal)),
      Arc::new(CalcitTypeAnnotation::AnonymousEnum),
    ] {
      let expression = proc_call(CalcitProc::NativeEnumDefinition, vec![local("value", open)]);
      assert!(
        matches!(
          infer_type_from_expr(&expression, &ScopeTypes::new()).as_deref(),
          Some(CalcitTypeAnnotation::Optional(_))
        ),
        "nullable or anonymous evidence must not prove a nominal definition"
      );
    }

    let none_call = CalcitList::from(&[
      Calcit::Proc(CalcitProc::NativeNamedEnumNew),
      Calcit::EnumDef(enum_def.clone()),
      Calcit::Tag(EdnTag::from("none")),
    ] as &[Calcit]);
    assert!(matches!(
      infer_enum_annotation(&none_call, &ScopeTypes::new()).as_deref(),
      Some(CalcitTypeAnnotation::Enum(inferred, args))
        if inferred.name() == enum_def.name()
          && matches!(args.first().map(AsRef::as_ref), Some(CalcitTypeAnnotation::Never))
    ));

    let some_call = CalcitList::from(&[
      Calcit::Proc(CalcitProc::NativeNamedEnumNew),
      Calcit::EnumDef(enum_def.clone()),
      Calcit::Tag(EdnTag::from("some")),
      Calcit::Number(1.0),
    ] as &[Calcit]);
    assert!(matches!(
      infer_enum_annotation(&some_call, &ScopeTypes::new()).as_deref(),
      Some(CalcitTypeAnnotation::Enum(inferred, args))
        if inferred.name() == enum_def.name()
          && matches!(args.first().map(AsRef::as_ref), Some(CalcitTypeAnnotation::Number))
    ));

    let some_list_call = CalcitList::from(&[
      Calcit::Proc(CalcitProc::NativeNamedEnumNew),
      Calcit::EnumDef(enum_def.clone()),
      Calcit::Tag(EdnTag::from("some")),
      Calcit::from(vec![Calcit::Proc(CalcitProc::List), Calcit::Number(1.0), Calcit::Number(2.0)]),
    ] as &[Calcit]);
    assert!(matches!(
      infer_enum_annotation(&some_list_call, &ScopeTypes::new()).as_deref(),
      Some(CalcitTypeAnnotation::Enum(inferred, args))
        if inferred.name() == enum_def.name()
          && matches!(
            args.first().map(AsRef::as_ref),
            Some(CalcitTypeAnnotation::List(item)) if matches!(item.as_ref(), CalcitTypeAnnotation::Number)
          )
    ));
  }

  #[test]
  fn enum_constructor_resolves_local_generic_struct_payloads() {
    let generic: Arc<str> = Arc::from("T");
    let mut box_def = CalcitStructDef::from_fields(EdnTag::from("Box"), vec![EdnTag::from("value")]);
    box_def.generics = Arc::new(vec![generic.clone()]);
    box_def.field_types = Arc::new(vec![Arc::new(CalcitTypeAnnotation::TypeVar(generic.clone()))]);
    let box_def = Arc::new(box_def);

    let enum_struct = CalcitStructDef {
      definition_ref: None,
      name: EdnTag::from("MaybeBox"),
      fields: Arc::new(vec![EdnTag::from("empty"), EdnTag::from("box")]),
      field_types: Arc::new(vec![calcit::DYNAMIC_TYPE.clone(), calcit::DYNAMIC_TYPE.clone()]),
      generics: Arc::new(vec![generic.clone()]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    };
    let enum_def = CalcitEnumDef::from_struct(CalcitStructValue {
      struct_ref: Arc::new(enum_struct),
      values: Arc::new(vec![
        Calcit::from(CalcitList::default()),
        Calcit::from(vec![
          CalcitTypeAnnotation::TypeRef(Arc::from("Box"), Arc::new(vec![Arc::new(CalcitTypeAnnotation::TypeVar(generic))])).to_calcit(),
        ]),
      ]),
    })
    .expect("valid generic enum fixture");

    let number = Arc::new(CalcitTypeAnnotation::Number);
    let boxed_number = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from("boxed")),
      sym: Arc::from("boxed"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.enum"),
        at_def: Arc::from("demo"),
      }),
      location: None,
      type_info: Arc::new(CalcitTypeAnnotation::Struct(box_def.clone(), Arc::new(vec![number.clone()]))),
    });
    let payloads = [boxed_number];
    let mut scope_types = ScopeTypes::new();
    scope_types.insert(Arc::from("Box"), Arc::new(CalcitTypeAnnotation::StructDef(box_def)));

    assert_eq!(
      infer_enum_applied_args(&enum_def, Some(&Calcit::Tag(EdnTag::from("box"))), payloads.iter(), &scope_types,),
      Some(vec![number]),
      "a local Box<T> payload should preserve T for the enclosing enum"
    );
  }

  #[test]
  fn impl_attachment_procs_preserve_nominal_data_types() {
    let method_impl = CalcitImpl {
      name: EdnTag::from("BirdMethods"),
      origin: None,
      fields: Arc::new(vec![EdnTag::from("show")]),
      values: Arc::new(vec![Calcit::Nil]),
    };
    let struct_def = CalcitStructDef::from_fields(cirru_edn::EdnTag::from("Bird"), vec![cirru_edn::EdnTag::from("name")]);
    let struct_call = proc_call(
      CalcitProc::NativeStructImplTraits,
      vec![Calcit::StructDef(struct_def.clone()), Calcit::Impl(method_impl.clone())],
    );
    assert!(matches!(
      infer_static_type_from_expr(&struct_call).as_deref(),
      Some(CalcitTypeAnnotation::StructDef(inferred))
        if inferred.name == struct_def.name && inferred.impls.iter().any(|item| item.get("show").is_some())
    ));

    let enum_struct = CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef::from_fields(
        cirru_edn::EdnTag::from("Outcome"),
        vec![cirru_edn::EdnTag::from("ok")],
      )),
      values: Arc::new(vec![Calcit::List(Arc::new(CalcitList::default()))]),
    };
    let enum_def = CalcitEnumDef::from_struct(enum_struct).expect("valid enum fixture");
    let enum_call = proc_call(
      CalcitProc::NativeEnumImplTraits,
      vec![Calcit::EnumDef(enum_def.clone()), Calcit::Impl(method_impl)],
    );
    assert!(matches!(
      infer_static_type_from_expr(&enum_call).as_deref(),
      Some(CalcitTypeAnnotation::EnumDef(inferred))
        if inferred.name() == enum_def.name() && inferred.impls().iter().any(|item| item.get("show").is_some())
    ));
  }

  #[test]
  fn declared_struct_return_keeps_impl_evidence_from_compiled_body() {
    let method_impl = Arc::new(CalcitImpl {
      name: EdnTag::from("DateImpl"),
      origin: None,
      fields: Arc::new(vec![EdnTag::from("format")]),
      values: Arc::new(vec![Calcit::Nil]),
    });
    let mut declared_struct = CalcitStructDef::from_fields(EdnTag::from("Date0"), vec![EdnTag::from("date")]);
    declared_struct.field_types = Arc::new(vec![calcit::DYNAMIC_TYPE.clone()]);
    let mut attached_struct = declared_struct.clone();
    attached_struct.impls = vec![method_impl];
    let constructed = Calcit::Struct(CalcitStructValue {
      struct_ref: Arc::new(attached_struct),
      values: Arc::new(vec![Calcit::Number(0.0)]),
    });
    let compiled_defn = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::Defn, Arc::from("tests.date")),
      symbol("from-ywd"),
      Calcit::from(CalcitList::default()),
      constructed.clone(),
    ]);

    let inferred_body =
      infer_compiled_callable_body_return("tests.date", "from-ywd", &compiled_defn).expect("compiled body return type");
    let declared = Arc::new(CalcitTypeAnnotation::Struct(Arc::new(declared_struct.clone()), Arc::new(vec![])));
    let enriched = enrich_declared_struct_return_with_impls(&declared, &inferred_body).expect("matching nominal impl evidence");
    assert!(matches!(
      enriched.as_ref(),
      CalcitTypeAnnotation::Struct(struct_def, args)
        if args.is_empty()
          && struct_def.fields == declared_struct.fields
          && struct_def.impls.iter().any(|item| item.get("format").is_some())
    ));

    let other = Arc::new(CalcitTypeAnnotation::Struct(
      Arc::new(CalcitStructDef::from_fields(EdnTag::from("Other"), vec![EdnTag::from("date")])),
      Arc::new(vec![]),
    ));
    assert!(
      enrich_declared_struct_return_with_impls(&other, &inferred_body).is_none(),
      "impl evidence must not cross nominal types"
    );

    let diverging_match = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::Match, Arc::from("tests.date")),
      Calcit::Tag(EdnTag::from("single")),
      Calcit::from(vec![Calcit::Tag(EdnTag::from("single")), constructed.clone()]),
      Calcit::from(vec![
        Calcit::Tag(EdnTag::from("none")),
        proc_call(CalcitProc::Raise, vec![Calcit::Str(Arc::from("cannot construct"))]),
      ]),
    ]);
    assert_eq!(infer_guaranteed_nominal_impl_return(&diverging_match), Some(inferred_body.clone()));

    let mut incompatible_struct = declared_struct;
    incompatible_struct.impls = vec![Arc::new(CalcitImpl {
      name: EdnTag::from("OtherImpl"),
      origin: None,
      fields: Arc::new(vec![EdnTag::from("other")]),
      values: Arc::new(vec![Calcit::Nil]),
    })];
    let incompatible = Calcit::Struct(CalcitStructValue {
      struct_ref: Arc::new(incompatible_struct),
      values: Arc::new(vec![Calcit::Number(0.0)]),
    });
    let ambiguous_match = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::Match, Arc::from("tests.date")),
      Calcit::Tag(EdnTag::from("single")),
      Calcit::from(vec![Calcit::Tag(EdnTag::from("left")), constructed]),
      Calcit::from(vec![Calcit::Tag(EdnTag::from("right")), incompatible]),
    ]);
    assert!(
      infer_guaranteed_nominal_impl_return(&ambiguous_match).is_none(),
      "different alias attachments across live branches are not guaranteed"
    );
  }
}
