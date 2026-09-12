//! Receiver-specialized contracts shared by checking, inference, rewriting,
//! and lowering for polymorphic collection calls.

use std::{collections::HashSet, sync::Arc};

use crate::calcit::{self, Calcit, CalcitList, CalcitTypeAnnotation};

use super::{ScopeTypes, type_inference::resolve_type_value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CheckedCallLowering {
  CoreDef(&'static str),
  TypedOptionalAccess,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CheckedCallContract {
  /// Bound checking evidence. Syntax-member collections intentionally keep
  /// this open while still carrying their proven lowering target.
  pub expected_types: Option<Vec<Arc<CalcitTypeAnnotation>>>,
  pub return_type: Arc<CalcitTypeAnnotation>,
  pub lowering: Option<CheckedCallLowering>,
}

fn core_type_ref(name: &str, args: Vec<Arc<CalcitTypeAnnotation>>) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::TypeRef(
    Arc::from(format!("{}/{name}", calcit::CORE_NS)),
    Arc::new(args),
  ))
}

fn fn_type(args: Vec<Arc<CalcitTypeAnnotation>>, return_type: Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::from_function_parts(args, return_type))
}

fn callback_return_type(callback: &Calcit, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  let embedded_schema = || {
    let Calcit::List(items) = callback else { return None };
    items
      .iter()
      .skip(3)
      .find_map(CalcitTypeAnnotation::extract_fn_annotation_from_hint_form)
  };
  let callback_type = embedded_schema().or_else(|| resolve_type_value(callback, scope_types))?;
  match callback_type.as_ref() {
    CalcitTypeAnnotation::Fn(signature) if !matches!(signature.return_type.as_ref(), CalcitTypeAnnotation::Dynamic) => {
      Some(signature.return_type.clone())
    }
    _ => None,
  }
}

fn core_result_type_args(type_value: &CalcitTypeAnnotation) -> Option<&[Arc<CalcitTypeAnnotation>]> {
  match type_value {
    CalcitTypeAnnotation::TypeRef(name, args)
      if matches!(
        name.trim_start_matches('\'').trim_start_matches(':'),
        "Result" | "calcit.core/Result"
      ) && args.len() == 2 =>
    {
      Some(args.as_ref())
    }
    CalcitTypeAnnotation::Enum(enum_def, args)
      if enum_def.definition_ref().is_some_and(|name| name.as_ref() == "calcit.core/Result") && args.len() == 2 =>
    {
      Some(args.as_ref())
    }
    _ => None,
  }
}

pub(crate) fn checked_call_contract_arity(fn_ns: &str, fn_def: &str) -> Option<usize> {
  if fn_ns != calcit::CORE_NS {
    return None;
  }
  match fn_def {
    "any?" | "each" | "every?" | "get" | "filter" | "map" | "map-indexed" | "map-list-kv" | "result:map" => Some(2),
    "option:fold" | "update" => Some(3),
    _ => None,
  }
}

/// Follow concrete type-slot bindings while leaving unresolved or cyclic slots
/// open for compatibility handling by the caller.
pub(crate) fn resolve_bound_type_slot_chain(mut type_value: Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  let mut resolving_slots = HashSet::new();
  while let CalcitTypeAnnotation::TypeSlot(name) = type_value.as_ref() {
    if !resolving_slots.insert(name.clone()) {
      break;
    }
    let Some(bound) = calcit::resolve_type_slot(name) else {
      break;
    };
    type_value = bound;
  }
  type_value
}

/// Resolve one concrete collection contract from the receiver and current call
/// scope. Returning `None` keeps Dynamic/open receivers on the compatibility
/// path and never authorizes static lowering.
pub(crate) fn resolve_checked_call_contract(
  fn_ns: &str,
  fn_def: &str,
  args: &CalcitList,
  scope_types: &ScopeTypes,
) -> Option<CheckedCallContract> {
  use CalcitTypeAnnotation as T;

  if fn_def == "option:fold" && !super::strict_types_enabled() {
    return None;
  }
  let required_arity = checked_call_contract_arity(fn_ns, fn_def)?;
  if args.len() != required_arity {
    return None;
  }

  let receiver_type = resolve_type_value(args.first()?, scope_types)
    .map(resolve_bound_type_slot_chain)
    .or_else(|| {
      if fn_def != "option:fold" {
        return None;
      }
      let Calcit::List(items) = args.first()? else { return None };
      let is_some_constructor = matches!(items.first(), Some(Calcit::Import(import)) if import.ns.as_ref() == calcit::CORE_NS && import.def.as_ref() == "%some")
        || matches!(items.first(), Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "%some");
      if !is_some_constructor {
        return None;
      }
      let payload = resolve_type_value(items.get(1)?, scope_types)?;
      Some(core_type_ref("Option", vec![payload]))
    })?;
  if fn_def == "result:map" {
    let type_args = core_result_type_args(receiver_type.as_ref())?;
    let input_type = type_args[0].clone();
    let error_type = type_args[1].clone();
    let output_type =
      callback_return_type(args.get(1)?, scope_types).unwrap_or_else(|| Arc::new(T::TypeVar(Arc::from("ResultMapOutput"))));
    return Some(CheckedCallContract {
      expected_types: Some(vec![receiver_type.clone(), fn_type(vec![input_type], output_type.clone())]),
      return_type: core_type_ref("Result", vec![output_type, error_type]),
      lowering: None,
    });
  }
  match (fn_def, receiver_type.as_ref()) {
    ("any?" | "every?", T::List(item_type) | T::Set(item_type)) => {
      let callback_type = if matches!(item_type.as_ref(), T::Syntax(_)) {
        Arc::new(T::DynFn)
      } else {
        fn_type(vec![item_type.clone()], Arc::new(T::Bool))
      };
      Some(CheckedCallContract {
        expected_types: Some(vec![receiver_type.clone(), callback_type]),
        return_type: Arc::new(T::Bool),
        lowering: None,
      })
    }
    ("any?" | "every?", T::Map(_, _)) => Some(CheckedCallContract {
      expected_types: Some(vec![
        receiver_type.clone(),
        fn_type(vec![Arc::new(T::List(crate::calcit::DYNAMIC_TYPE.clone()))], Arc::new(T::Bool)),
      ]),
      return_type: Arc::new(T::Bool),
      lowering: None,
    }),
    ("each", T::List(item_type) | T::Set(item_type)) => {
      let callback_type = if matches!(item_type.as_ref(), T::Syntax(_)) {
        Arc::new(T::DynFn)
      } else {
        fn_type(vec![item_type.clone()], crate::calcit::DYNAMIC_TYPE.clone())
      };
      Some(CheckedCallContract {
        expected_types: Some(vec![receiver_type.clone(), callback_type]),
        return_type: Arc::new(T::Unit),
        lowering: None,
      })
    }
    ("each", T::Map(_, _)) => Some(CheckedCallContract {
      expected_types: Some(vec![
        receiver_type.clone(),
        fn_type(
          vec![Arc::new(T::List(crate::calcit::DYNAMIC_TYPE.clone()))],
          crate::calcit::DYNAMIC_TYPE.clone(),
        ),
      ]),
      return_type: Arc::new(T::Unit),
      lowering: None,
    }),
    ("option:fold", T::TypeRef(_, type_args) | T::Enum(_, type_args)) if receiver_type.is_option_type() => {
      let input_type = type_args.first()?.clone();
      let output_type = callback_return_type(args.get(1)?, scope_types)
        .or_else(|| callback_return_type(args.get(2)?, scope_types))
        .unwrap_or_else(|| Arc::new(T::TypeVar(Arc::from("OptionFoldOutput"))));
      Some(CheckedCallContract {
        expected_types: Some(vec![
          receiver_type.clone(),
          fn_type(vec![], output_type.clone()),
          fn_type(vec![input_type], output_type.clone()),
        ]),
        return_type: output_type,
        lowering: None,
      })
    }
    ("get", T::Map(key_type, value_type)) => Some(CheckedCallContract {
      expected_types: Some(vec![receiver_type.clone(), key_type.clone()]),
      return_type: core_type_ref("Option", vec![value_type.clone()]),
      lowering: Some(CheckedCallLowering::TypedOptionalAccess),
    }),
    ("get", T::List(item_type)) => Some(CheckedCallContract {
      expected_types: Some(vec![receiver_type.clone(), Arc::new(T::Number)]),
      return_type: core_type_ref("Option", vec![item_type.clone()]),
      lowering: Some(CheckedCallLowering::TypedOptionalAccess),
    }),
    ("get", T::String) => Some(CheckedCallContract {
      expected_types: Some(vec![receiver_type.clone(), Arc::new(T::Number)]),
      return_type: core_type_ref("Option", vec![Arc::new(T::String)]),
      lowering: Some(CheckedCallLowering::TypedOptionalAccess),
    }),
    ("get", value) if matches!(value, T::EnumValue(_) | T::AnonymousEnum) || value.resolve_to_enum().is_some() => {
      Some(CheckedCallContract {
        expected_types: Some(vec![receiver_type, Arc::new(T::Number)]),
        return_type: core_type_ref("Option", vec![crate::calcit::DYNAMIC_TYPE.clone()]),
        lowering: Some(CheckedCallLowering::TypedOptionalAccess),
      })
    }
    ("update", T::List(item_type)) => Some(CheckedCallContract {
      expected_types: Some(vec![
        receiver_type.clone(),
        Arc::new(T::Number),
        fn_type(vec![item_type.clone()], item_type.clone()),
      ]),
      return_type: receiver_type,
      lowering: None,
    }),
    ("update", T::Map(key_type, value_type)) => Some(CheckedCallContract {
      expected_types: Some(vec![
        receiver_type.clone(),
        key_type.clone(),
        fn_type(vec![value_type.clone()], value_type.clone()),
      ]),
      return_type: receiver_type,
      lowering: None,
    }),
    ("filter", T::List(item_type)) | ("filter", T::Set(item_type)) if !matches!(item_type.as_ref(), T::Syntax(_)) => {
      let target = if matches!(receiver_type.as_ref(), T::List(_)) {
        "&list:filter"
      } else {
        "&set:filter"
      };
      Some(CheckedCallContract {
        expected_types: Some(vec![receiver_type.clone(), fn_type(vec![item_type.clone()], Arc::new(T::Bool))]),
        return_type: receiver_type,
        lowering: Some(CheckedCallLowering::CoreDef(target)),
      })
    }
    ("filter", T::Map(_, _)) => Some(CheckedCallContract {
      expected_types: Some(vec![
        receiver_type.clone(),
        fn_type(vec![Arc::new(T::List(crate::calcit::DYNAMIC_TYPE.clone()))], Arc::new(T::Bool)),
      ]),
      return_type: receiver_type,
      lowering: Some(CheckedCallLowering::CoreDef("&map:filter")),
    }),
    ("map", T::List(item_type)) | ("map", T::Set(item_type)) if !matches!(item_type.as_ref(), T::Syntax(_)) => {
      let output_type = callback_return_type(args.get(1)?, scope_types).unwrap_or_else(|| Arc::new(T::TypeVar(Arc::from("MapOutput"))));
      let (return_type, target) = if matches!(receiver_type.as_ref(), T::List(_)) {
        (Arc::new(T::List(output_type.clone())), "&list:map")
      } else {
        (Arc::new(T::Set(output_type.clone())), "&set:map")
      };
      Some(CheckedCallContract {
        expected_types: Some(vec![receiver_type.clone(), fn_type(vec![item_type.clone()], output_type)]),
        return_type,
        lowering: Some(CheckedCallLowering::CoreDef(target)),
      })
    }
    ("map-indexed", T::List(item_type)) if !matches!(item_type.as_ref(), T::Syntax(_)) => {
      let output_type =
        callback_return_type(args.get(1)?, scope_types).unwrap_or_else(|| Arc::new(T::TypeVar(Arc::from("MapIndexedOutput"))));
      Some(CheckedCallContract {
        expected_types: Some(vec![
          receiver_type.clone(),
          fn_type(vec![Arc::new(T::Number), item_type.clone()], output_type.clone()),
        ]),
        return_type: Arc::new(T::List(output_type)),
        lowering: None,
      })
    }
    ("map", T::Map(_, _)) => {
      let pair_type = Arc::new(T::List(crate::calcit::DYNAMIC_TYPE.clone()));
      Some(CheckedCallContract {
        expected_types: Some(vec![receiver_type, fn_type(vec![pair_type.clone()], pair_type)]),
        return_type: Arc::new(T::Map(crate::calcit::DYNAMIC_TYPE.clone(), crate::calcit::DYNAMIC_TYPE.clone())),
        lowering: Some(CheckedCallLowering::CoreDef("&map:map")),
      })
    }
    ("map-list-kv", T::Map(key_type, value_type)) => {
      let output_type =
        callback_return_type(args.get(1)?, scope_types).unwrap_or_else(|| Arc::new(T::TypeVar(Arc::from("MapListOutput"))));
      Some(CheckedCallContract {
        expected_types: Some(vec![
          receiver_type.clone(),
          fn_type(vec![key_type.clone(), value_type.clone()], output_type.clone()),
        ]),
        return_type: Arc::new(T::List(output_type)),
        lowering: None,
      })
    }
    ("filter", T::List(item_type)) | ("filter", T::Set(item_type)) if matches!(item_type.as_ref(), T::Syntax(_)) => {
      let target = if matches!(receiver_type.as_ref(), T::List(_)) {
        "&list:filter"
      } else {
        "&set:filter"
      };
      Some(CheckedCallContract {
        expected_types: None,
        return_type: receiver_type,
        lowering: Some(CheckedCallLowering::CoreDef(target)),
      })
    }
    ("map", T::List(item_type)) | ("map", T::Set(item_type)) if matches!(item_type.as_ref(), T::Syntax(_)) => {
      let output_type = Arc::new(T::TypeVar(Arc::from("MapOutput")));
      let (return_type, target) = if matches!(receiver_type.as_ref(), T::List(_)) {
        (Arc::new(T::List(output_type)), "&list:map")
      } else {
        (Arc::new(T::Set(output_type)), "&set:map")
      };
      Some(CheckedCallContract {
        expected_types: None,
        return_type,
        lowering: Some(CheckedCallLowering::CoreDef(target)),
      })
    }
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::{CalcitEnumDef, CalcitLocal, CalcitStructDef, CalcitStructValue, CalcitSymbolInfo};
  use cirru_edn::EdnTag;

  fn symbol(name: &str) -> Calcit {
    Calcit::Symbol {
      sym: Arc::from(name),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests"),
        at_def: Arc::from("checked_call_contract"),
      }),
      location: None,
    }
  }

  fn result_enum(namespace: &str) -> Arc<CalcitEnumDef> {
    Arc::new(
      CalcitEnumDef::from_struct(CalcitStructValue {
        struct_ref: Arc::new(CalcitStructDef {
          definition_ref: None,
          name: EdnTag::new("Result"),
          fields: Arc::new(vec![EdnTag::new("err"), EdnTag::new("ok")]),
          field_types: Arc::new(vec![crate::calcit::DYNAMIC_TYPE.clone(); 2]),
          generics: Arc::new(vec![Arc::from("T"), Arc::from("E")]),
          where_bounds: Arc::new(vec![]),
          impls: vec![],
        }),
        values: Arc::new(vec![
          Calcit::List(Arc::new(CalcitList::Vector(vec![symbol("E")]))),
          Calcit::List(Arc::new(CalcitList::Vector(vec![symbol("T")]))),
        ]),
      })
      .expect("valid Result enum")
      .with_definition_ref(namespace, "Result"),
    )
  }

  fn local(name: &str, type_info: Arc<CalcitTypeAnnotation>) -> Calcit {
    Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from(name)),
      sym: Arc::from(name),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.checked-call"),
        at_def: Arc::from("demo"),
      }),
      location: None,
      type_info,
    })
  }

  #[test]
  fn collection_contract_binds_checking_inference_and_lowering_evidence() {
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let string = Arc::new(CalcitTypeAnnotation::String);
    let receiver = Arc::new(CalcitTypeAnnotation::List(number.clone()));
    let mapper = Arc::new(CalcitTypeAnnotation::from_function_parts(vec![number.clone()], string.clone()));
    let args = CalcitList::from(&[local("items", receiver.clone()), local("render", mapper)] as &[Calcit]);

    let contract = resolve_checked_call_contract(calcit::CORE_NS, "map", &args, &ScopeTypes::new())
      .expect("typed list map should have one checked contract");
    let expected_types = contract.expected_types.as_ref().expect("typed map should bind checking evidence");
    assert_eq!(expected_types[0], receiver);
    let CalcitTypeAnnotation::Fn(callback) = expected_types[1].as_ref() else {
      panic!("map callback should be checked as a function");
    };
    assert_eq!(callback.arg_types.as_slice(), &[number]);
    assert_eq!(callback.return_type, string.clone());
    assert_eq!(contract.return_type, Arc::new(CalcitTypeAnnotation::List(string)));
    assert_eq!(contract.lowering, Some(CheckedCallLowering::CoreDef("&list:map")));
  }

  #[test]
  fn map_list_kv_contract_preserves_both_map_members_and_callback_output() {
    let key = Arc::new(CalcitTypeAnnotation::Tag);
    let value = Arc::new(CalcitTypeAnnotation::String);
    let output = Arc::new(CalcitTypeAnnotation::Number);
    let receiver = Arc::new(CalcitTypeAnnotation::Map(key.clone(), value.clone()));
    let mapper = Arc::new(CalcitTypeAnnotation::from_function_parts(
      vec![key.clone(), value.clone()],
      output.clone(),
    ));
    let args = CalcitList::from(&[local("options", receiver.clone()), local("measure", mapper)] as &[Calcit]);

    let contract = resolve_checked_call_contract(calcit::CORE_NS, "map-list-kv", &args, &ScopeTypes::new())
      .expect("typed map-list-kv should have a checked contract");
    let expected_types = contract.expected_types.as_ref().expect("map-list-kv should bind checking evidence");
    assert_eq!(expected_types[0], receiver);
    let CalcitTypeAnnotation::Fn(callback) = expected_types[1].as_ref() else {
      panic!("map-list-kv callback should be checked as a function");
    };
    assert_eq!(callback.arg_types.as_slice(), &[key, value]);
    assert_eq!(callback.return_type, output.clone());
    assert_eq!(contract.return_type, Arc::new(CalcitTypeAnnotation::List(output)));
    assert_eq!(contract.lowering, None);
  }

  #[test]
  fn map_indexed_contract_preserves_open_members_without_narrowing_them() {
    let dynamic = crate::calcit::DYNAMIC_TYPE.clone();
    let output = Arc::new(CalcitTypeAnnotation::List(dynamic.clone()));
    let receiver = Arc::new(CalcitTypeAnnotation::List(dynamic.clone()));
    let mapper = Arc::new(CalcitTypeAnnotation::from_function_parts(
      vec![Arc::new(CalcitTypeAnnotation::Number), dynamic.clone()],
      output.clone(),
    ));
    let args = CalcitList::from(&[local("items", receiver.clone()), local("with-index", mapper)] as &[Calcit]);

    let contract = resolve_checked_call_contract(calcit::CORE_NS, "map-indexed", &args, &ScopeTypes::new())
      .expect("typed map-indexed should derive one receiver-bound contract");
    let expected_types = contract.expected_types.as_ref().expect("map-indexed should bind callback evidence");
    assert_eq!(expected_types[0], receiver);
    let CalcitTypeAnnotation::Fn(callback) = expected_types[1].as_ref() else {
      panic!("map-indexed callback should be checked as a function");
    };
    assert_eq!(callback.arg_types.as_slice(), &[Arc::new(CalcitTypeAnnotation::Number), dynamic]);
    assert_eq!(callback.return_type, output.clone());
    assert_eq!(contract.return_type, Arc::new(CalcitTypeAnnotation::List(output)));
    assert_eq!(contract.lowering, None);
  }

  #[test]
  fn result_map_contract_preserves_ok_error_and_callback_output_types() {
    let input = Arc::new(CalcitTypeAnnotation::Tag);
    let error = Arc::new(CalcitTypeAnnotation::String);
    let output = Arc::new(CalcitTypeAnnotation::Number);
    let receiver = core_type_ref("Result", vec![input.clone(), error.clone()]);
    let mapper = Arc::new(CalcitTypeAnnotation::from_function_parts(vec![input.clone()], output.clone()));
    let args = CalcitList::from(&[local("result", receiver.clone()), local("measure", mapper)] as &[Calcit]);

    let contract = resolve_checked_call_contract(calcit::CORE_NS, "result:map", &args, &ScopeTypes::new())
      .expect("typed result:map should have a checked contract");
    let expected_types = contract.expected_types.as_ref().expect("result:map should bind checking evidence");
    assert_eq!(expected_types[0], receiver);
    let CalcitTypeAnnotation::Fn(callback) = expected_types[1].as_ref() else {
      panic!("result:map callback should be checked as a function");
    };
    assert_eq!(callback.arg_types.as_slice(), &[input]);
    assert_eq!(callback.return_type, output.clone());
    assert_eq!(contract.return_type, core_type_ref("Result", vec![output, error]));
    assert_eq!(contract.lowering, None);
  }

  #[test]
  fn result_map_contract_checks_constructor_produced_core_result_locals() {
    let input = Arc::new(CalcitTypeAnnotation::String);
    let error = Arc::new(CalcitTypeAnnotation::Tag);
    let output = Arc::new(CalcitTypeAnnotation::Number);
    let receiver = Arc::new(CalcitTypeAnnotation::Enum(
      result_enum(calcit::CORE_NS),
      Arc::new(vec![input.clone(), error.clone()]),
    ));
    let mapper = Arc::new(CalcitTypeAnnotation::from_function_parts(vec![input.clone()], output.clone()));
    let args = CalcitList::from(&[local("constructed", receiver.clone()), local("measure", mapper)] as &[Calcit]);

    let contract = resolve_checked_call_contract(calcit::CORE_NS, "result:map", &args, &ScopeTypes::new())
      .expect("constructor-produced core Result should retain its checked map contract");
    let expected_types = contract.expected_types.as_ref().expect("result:map should bind checking evidence");
    assert_eq!(expected_types[0], receiver);
    let CalcitTypeAnnotation::Fn(callback) = expected_types[1].as_ref() else {
      panic!("result:map callback should be checked as a function");
    };
    assert_eq!(callback.arg_types.as_slice(), &[input]);
    assert_eq!(callback.return_type, output.clone());
    assert_eq!(contract.return_type, core_type_ref("Result", vec![output, error]));

    let user_receiver = Arc::new(CalcitTypeAnnotation::Enum(
      result_enum("app.models"),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::String), Arc::new(CalcitTypeAnnotation::Tag)]),
    ));
    let user_args = CalcitList::from(&[
      local("user-result", user_receiver),
      local(
        "measure",
        Arc::new(CalcitTypeAnnotation::from_function_parts(
          vec![Arc::new(CalcitTypeAnnotation::String)],
          Arc::new(CalcitTypeAnnotation::Number),
        )),
      ),
    ] as &[Calcit]);
    assert!(
      resolve_checked_call_contract(calcit::CORE_NS, "result:map", &user_args, &ScopeTypes::new()).is_none(),
      "a user enum with the same short name must not acquire the core Result contract"
    );

    let user_ref = Arc::new(CalcitTypeAnnotation::TypeRef(
      Arc::from("app.models/Result"),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::String), Arc::new(CalcitTypeAnnotation::Tag)]),
    ));
    let args = CalcitList::from(&[
      local("user-result-ref", user_ref),
      local(
        "measure",
        Arc::new(CalcitTypeAnnotation::from_function_parts(
          vec![Arc::new(CalcitTypeAnnotation::String)],
          Arc::new(CalcitTypeAnnotation::Number),
        )),
      ),
    ] as &[Calcit]);
    assert!(
      resolve_checked_call_contract(calcit::CORE_NS, "result:map", &args, &ScopeTypes::new()).is_none(),
      "an explicitly user-qualified Result reference must not acquire the core contract"
    );
  }

  #[test]
  fn iteration_contracts_bind_receiver_members_before_callback_preprocessing() {
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let list_type = Arc::new(CalcitTypeAnnotation::List(number.clone()));
    let predicate = Arc::new(CalcitTypeAnnotation::DynFn);

    for fn_name in ["any?", "every?"] {
      let args = CalcitList::from(&[local("items", list_type.clone()), local("predicate", predicate.clone())] as &[Calcit]);
      let contract = resolve_checked_call_contract(calcit::CORE_NS, fn_name, &args, &ScopeTypes::new())
        .expect("typed collection predicates should have one shared contract");
      let expected = contract.expected_types.expect("predicate contract should carry callback evidence");
      let CalcitTypeAnnotation::Fn(callback) = expected[1].as_ref() else {
        panic!("predicate should receive a concrete callback type");
      };
      assert_eq!(callback.arg_types.as_slice(), std::slice::from_ref(&number));
      assert!(matches!(callback.return_type.as_ref(), CalcitTypeAnnotation::Bool));
      assert!(matches!(contract.return_type.as_ref(), CalcitTypeAnnotation::Bool));
    }

    let each_args = CalcitList::from(&[local("items", list_type), local("effect", predicate)] as &[Calcit]);
    let each_contract = resolve_checked_call_contract(calcit::CORE_NS, "each", &each_args, &ScopeTypes::new())
      .expect("typed each should share the receiver-bound callback contract");
    let expected = each_contract.expected_types.expect("each should carry callback evidence");
    let CalcitTypeAnnotation::Fn(callback) = expected[1].as_ref() else {
      panic!("each should receive a concrete callback type");
    };
    assert_eq!(callback.arg_types.as_slice(), std::slice::from_ref(&number));
    assert!(matches!(callback.return_type.as_ref(), CalcitTypeAnnotation::Dynamic));
    assert!(matches!(each_contract.return_type.as_ref(), CalcitTypeAnnotation::Unit));
  }

  #[test]
  fn iteration_contracts_keep_syntax_members_open_and_map_pairs_shaped() {
    let syntax = Arc::new(CalcitTypeAnnotation::Syntax(Arc::new(calcit::MacroSyntaxType::Syntax)));
    let syntax_args = CalcitList::from(&[
      local("forms", Arc::new(CalcitTypeAnnotation::List(syntax))),
      local("predicate", Arc::new(CalcitTypeAnnotation::DynFn)),
    ] as &[Calcit]);
    let syntax_contract = resolve_checked_call_contract(calcit::CORE_NS, "every?", &syntax_args, &ScopeTypes::new())
      .expect("syntax iteration should retain a non-generic open callback contract");
    assert!(matches!(
      syntax_contract.expected_types.as_ref().unwrap()[1].as_ref(),
      CalcitTypeAnnotation::DynFn
    ));

    let map_type = Arc::new(CalcitTypeAnnotation::Map(
      Arc::new(CalcitTypeAnnotation::Tag),
      Arc::new(CalcitTypeAnnotation::Number),
    ));
    let map_args = CalcitList::from(&[
      local("entries", map_type),
      local("predicate", Arc::new(CalcitTypeAnnotation::DynFn)),
    ] as &[Calcit]);
    let map_contract = resolve_checked_call_contract(calcit::CORE_NS, "any?", &map_args, &ScopeTypes::new())
      .expect("map iteration should preserve its pair shape");
    let CalcitTypeAnnotation::Fn(callback) = map_contract.expected_types.as_ref().unwrap()[1].as_ref() else {
      panic!("map predicate should be checked as a function");
    };
    assert!(matches!(
      callback.arg_types.as_slice(),
      [pair] if matches!(pair.as_ref(), CalcitTypeAnnotation::List(inner) if matches!(inner.as_ref(), CalcitTypeAnnotation::Dynamic))
    ));
  }

  #[test]
  fn get_update_and_filter_share_receiver_bound_types() {
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let string = Arc::new(CalcitTypeAnnotation::String);
    let map_type = Arc::new(CalcitTypeAnnotation::Map(string.clone(), number.clone()));
    let get_args = CalcitList::from(&[local("counts", map_type), Calcit::Str(Arc::from("a"))] as &[Calcit]);
    let get_contract = resolve_checked_call_contract(calcit::CORE_NS, "get", &get_args, &ScopeTypes::new())
      .expect("typed map get should have one checked contract");
    assert_eq!(get_contract.expected_types.as_ref().unwrap()[1], string);
    assert_eq!(get_contract.return_type, core_type_ref("Option", vec![number.clone()]));
    assert_eq!(get_contract.lowering, Some(CheckedCallLowering::TypedOptionalAccess));

    let list_type = Arc::new(CalcitTypeAnnotation::List(number.clone()));
    let updater = Arc::new(CalcitTypeAnnotation::from_function_parts(vec![number.clone()], number.clone()));
    let update_args =
      CalcitList::from(&[local("items", list_type.clone()), Calcit::Number(0.0), local("increment", updater)] as &[Calcit]);
    let update_contract = resolve_checked_call_contract(calcit::CORE_NS, "update", &update_args, &ScopeTypes::new())
      .expect("typed list update should have one checked contract");
    assert_eq!(update_contract.return_type, list_type);
    let CalcitTypeAnnotation::Fn(callback) = update_contract.expected_types.as_ref().unwrap()[2].as_ref() else {
      panic!("update callback should be checked as a function");
    };
    assert_eq!(callback.arg_types.as_slice(), std::slice::from_ref(&number));
    assert_eq!(callback.return_type, number.clone());

    let set_type = Arc::new(CalcitTypeAnnotation::Set(number));
    let predicate = Arc::new(CalcitTypeAnnotation::from_function_parts(
      vec![Arc::new(CalcitTypeAnnotation::Number)],
      Arc::new(CalcitTypeAnnotation::Bool),
    ));
    let filter_args = CalcitList::from(&[local("ids", set_type.clone()), local("positive?", predicate)] as &[Calcit]);
    let filter_contract = resolve_checked_call_contract(calcit::CORE_NS, "filter", &filter_args, &ScopeTypes::new())
      .expect("typed set filter should have one checked contract");
    assert_eq!(filter_contract.return_type, set_type);
    assert_eq!(filter_contract.lowering, Some(CheckedCallLowering::CoreDef("&set:filter")));
  }

  #[test]
  fn open_receivers_keep_the_compatibility_path() {
    let args = CalcitList::from(&[
      local("items", crate::calcit::DYNAMIC_TYPE.clone()),
      local("mapper", Arc::new(CalcitTypeAnnotation::DynFn)),
    ] as &[Calcit]);
    assert!(resolve_checked_call_contract(calcit::CORE_NS, "map", &args, &ScopeTypes::new()).is_none());

    let predicate_args = CalcitList::from(&[
      local("items", crate::calcit::DYNAMIC_TYPE.clone()),
      local("predicate", Arc::new(CalcitTypeAnnotation::DynFn)),
    ] as &[Calcit]);
    assert!(
      resolve_checked_call_contract(calcit::CORE_NS, "every?", &predicate_args, &ScopeTypes::new()).is_none(),
      "an open receiver must not invent a callback member type"
    );

    let slot_args = CalcitList::from(&[
      local("items", Arc::new(CalcitTypeAnnotation::TypeSlot(Arc::from("unbound-items")))),
      local("mapper", Arc::new(CalcitTypeAnnotation::DynFn)),
    ] as &[Calcit]);
    assert!(resolve_checked_call_contract(calcit::CORE_NS, "map", &slot_args, &ScopeTypes::new()).is_none());

    let syntax_args = CalcitList::from(&[
      local(
        "forms",
        Arc::new(CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Syntax(Arc::new(
          calcit::MacroSyntaxType::Syntax,
        ))))),
      ),
      local("transform", Arc::new(CalcitTypeAnnotation::DynFn)),
    ] as &[Calcit]);
    let syntax_contract = resolve_checked_call_contract(calcit::CORE_NS, "map", &syntax_args, &ScopeTypes::new())
      .expect("a proven Syntax list should retain its existing lowering");
    assert!(syntax_contract.expected_types.is_none());
    assert_eq!(syntax_contract.lowering, Some(CheckedCallLowering::CoreDef("&list:map")));
  }

  #[test]
  fn map_lookup_preserves_cross_namespace_keys_and_nested_option_payloads() {
    let account = Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from("accounts/User"), Arc::new(vec![])));
    let optional_number = core_type_ref("Option", vec![Arc::new(CalcitTypeAnnotation::Number)]);
    let receiver = Arc::new(CalcitTypeAnnotation::Map(account.clone(), optional_number.clone()));
    let args = CalcitList::from(&[local("scores", receiver), local("account", account.clone())] as &[Calcit]);

    let contract = resolve_checked_call_contract(calcit::CORE_NS, "get", &args, &ScopeTypes::new())
      .expect("a cross-namespace nominal key remains a valid typed map lookup");
    assert_eq!(contract.expected_types.as_ref().unwrap()[1], account);
    assert_eq!(
      contract.return_type,
      core_type_ref("Option", vec![optional_number]),
      "lookup absence must not flatten a field payload that is already Option"
    );
  }
}
