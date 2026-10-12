//! Resolve and expand lexical transfer inputs before static call lowering.
//! The prepared tree is inference-only; ordinary checking consumes source again.

use super::*;

const PENDING_PREFIX: &str = "@calcit:recursive-input/";

fn pending_variable(identity: usize) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from(format!("{PENDING_PREFIX}{identity}"))))
}

fn has_pending_variable(annotation: &CalcitTypeAnnotation) -> bool {
  crate::calcit::type_annotation::free_type_variable_names(&[Arc::new(annotation.clone())])
    .iter()
    .any(|name| name.starts_with(PENDING_PREFIX))
}

pub(super) fn prepare_expression(expr: &Calcit, ctx: PreprocessContext, constraints: &mut Constraints) -> Result<Calcit, CalcitErr> {
  runner::with_stack_guard_suspended(|| prepare(expr, ctx, constraints))
}

fn prepare(expr: &Calcit, mut ctx: PreprocessContext, constraints: &mut Constraints) -> Result<Calcit, CalcitErr> {
  let Calcit::List(items) = expr else {
    return preprocess_expr(
      expr,
      ctx.scope_defs,
      ctx.scope_types,
      ctx.file_ns,
      ctx.check_warnings,
      ctx.call_stack,
    );
  };
  let Some(source_head) = items.first() else {
    return Ok(expr.clone());
  };
  ctx.call_location = derive_list_call_expr_location(items);
  if matches!(source_head, Calcit::Symbol { .. } | Calcit::Import(_))
    && let Some((id, definition)) = source_replay::Scope::retained_static_callee(items)
  {
    return process_macro_source(
      &definition,
      &id,
      items,
      &items.drop_left(),
      grab_def_name(source_head).as_ref(),
      ctx,
      MacroSourcePhase::Prepare(constraints),
    );
  }
  let head = prepare_expression(
    source_head,
    PreprocessContext::new(ctx.scope_defs, ctx.scope_types, ctx.file_ns, ctx.check_warnings, ctx.call_stack),
    constraints,
  )?;
  if let Calcit::Import(import) = &head
    && let Some(Calcit::Macro { id, info }) =
      lookup_callable_ns_def_for_preprocess(&import.ns, &import.def, ctx.check_warnings, ctx.call_stack)?
  {
    return process_macro_source(
      &info,
      &id,
      items,
      &items.drop_left(),
      grab_def_name(source_head).as_ref(),
      ctx,
      MacroSourcePhase::Prepare(constraints),
    );
  }
  match &head {
    Calcit::Syntax(
      CalcitSyntax::Quote | CalcitSyntax::Eval | CalcitSyntax::HintFn | CalcitSyntax::Quasiquote | CalcitSyntax::Defmacro,
      _,
    ) => Ok(expr.clone()),
    Calcit::Syntax(CalcitSyntax::Defn | CalcitSyntax::DefWasmExport | CalcitSyntax::DefWasmImport, _) => {
      prepare_function(&head, &items.drop_left(), ctx, constraints)
    }
    Calcit::Syntax(CalcitSyntax::CoreLet, _) => prepare_binding(&head, &items.drop_left(), ctx, constraints),
    Calcit::Syntax(CalcitSyntax::If, _) => prepare_branches(&head, &items.drop_left(), ctx, constraints),
    Calcit::Syntax(CalcitSyntax::Match, namespace) => {
      let mut ctx = ctx;
      process_match(
        &CalcitSyntax::Match,
        namespace,
        &items.drop_left(),
        &mut ctx,
        MacroSourcePhase::Prepare(constraints),
      )
    }
    Calcit::Syntax(CalcitSyntax::AssertType, _) => {
      let obligation = source_replay::Assertion::at_source(items);
      let Some(target) = items.get(1) else { return Ok(expr.clone()) };
      let prepared = prepare_expression(
        target,
        PreprocessContext::new(ctx.scope_defs, ctx.scope_types, ctx.file_ns, ctx.check_warnings, ctx.call_stack),
        constraints,
      )?;
      let evidence = resolve_type_value(&prepared, ctx.scope_types);
      let needs_independent_input = obligation.required || evidence.as_ref().is_none_or(|annotation| has_pending_variable(annotation));
      if needs_independent_input
        || evidence
          .as_ref()
          .is_some_and(|annotation| type_inference::has_inferred_empty_slot(annotation))
      {
        obligation.retain_required();
      }
      // A declaration cannot supply its own independent transfer evidence.
      // The source assertion is checked after constraints have converged.
      if needs_independent_input {
        Ok(prepared)
      } else {
        // An independent boundary keeps its existing declared context. The
        // ordinary checker, not preparation, authorizes that narrowing.
        let mut assertion = items.to_vec();
        assertion[1] = prepared;
        Ok(Calcit::from(assertion))
      }
    }
    _ => {
      let mut arguments = Vec::new();
      for argument in items.iter().skip(1) {
        arguments.push(prepare_expression(
          argument,
          PreprocessContext::new(ctx.scope_defs, ctx.scope_types, ctx.file_ns, ctx.check_warnings, ctx.call_stack),
          constraints,
        )?);
      }
      let args = CalcitList::from(arguments.as_slice());
      // Normalize receiver-first syntax before collecting the same call
      // constraints as prefix syntax; this does not emit an implementation.
      let (head, args) = if matches!(args.first(), Some(Calcit::Method(_, calcit::MethodKind::Invoke(_)))) {
        (args[0].clone(), args.drop_left().push_left(head))
      } else {
        (head, args)
      };
      let signature = match &head {
        Calcit::Method(name, calcit::MethodKind::Invoke(_)) => args
          .first()
          .and_then(|receiver| resolve_type_value(receiver, ctx.scope_types))
          .and_then(|receiver| {
            if let Some(traits) = trait_list_from_type(&receiver) {
              selected_trait_method(&traits, name).map(|candidate| candidate.method_type)
            } else {
              let impls = get_impls_from_type(&receiver)?;
              resolve_type_value(find_method_entry_for_type(&receiver, &impls, name)?, ctx.scope_types)
            }
          }),
        _ => resolve_type_value(&head, ctx.scope_types),
      }
      .and_then(|annotation| annotation.resolve_to_nonoptional_fn());
      if let Some(signature) = signature {
        let actual = args.iter().map(|arg| resolve_type_value(arg, ctx.scope_types)).collect::<Vec<_>>();
        if actual.iter().flatten().any(|annotation| has_pending_variable(annotation)) {
          let callable = match &head {
            Calcit::Fn { info, .. } => Some(info.clone()),
            Calcit::Import(import) => {
              match lookup_callable_ns_def_for_preprocess(&import.ns, &import.def, ctx.check_warnings, ctx.call_stack)? {
                Some(Calcit::Fn { info, .. }) => Some(info),
                _ => None,
              }
            }
            _ => None,
          };
          // Named collection calls and methods must contribute the same
          // receiver-specialized input equations during preparation.
          let specialized = callable
            .as_ref()
            .and_then(|info| type_checking::specialize_core_expected_types(info, &args, ctx.scope_types, &signature.arg_types));
          let parameters = specialized.as_deref().unwrap_or(&signature.arg_types);
          let mut proof = crate::calcit::type_annotation::CallTypeProof::new(
            &signature.generics,
            parameters,
            &actual.iter().flatten().cloned().collect::<Vec<_>>(),
          );
          for (index, annotation) in actual.into_iter().enumerate() {
            let Some(annotation) = annotation else { continue };
            let Some(expected) = parameters.get(index).or(signature.rest_type.as_ref()) else {
              break;
            };
            let expected = if let CalcitTypeAnnotation::Variadic(inner) = expected.as_ref() {
              inner
            } else {
              expected
            };
            // Repeated callee slots connect independently supplied arguments;
            // only compiler-owned pending inputs may be solved by that relation.
            if let Some(instantiated) = proof.result(expected) {
              constraints.join_branches(vec![Some(annotation.clone()), Some(instantiated)], &ctx)?;
            }
            proof.prove(&annotation, expected);
          }
        }
      }
      // Preparation only projects constructor types. Ordinary source checking
      // reports skipped rewrites once, after recursive inputs have converged.
      let prepare_warnings = RefCell::new(Vec::new());
      if let Some(constructor) = try_rewrite_struct_enum_constructor_head_call(
        &head,
        &args,
        ctx.scope_types,
        ctx.file_ns,
        grab_def_name(source_head).as_ref(),
        &prepare_warnings,
        ctx.call_stack,
      )? {
        return Ok(constructor);
      }
      Ok(Calcit::from(args.push_left(head)))
    }
  }
}

fn prepare_binding(
  head: &Calcit,
  args: &CalcitList,
  ctx: PreprocessContext,
  constraints: &mut Constraints,
) -> Result<Calcit, CalcitErr> {
  let mut definitions = ctx.scope_defs.clone();
  let mut types = ctx.scope_types.clone();
  let Some(Calcit::List(pair)) = args.first() else {
    return Ok(Calcit::from(args.push_left(head.clone())));
  };
  let binding = if pair.is_empty() {
    Calcit::from(CalcitList::default())
  } else if let (Some(name), Some(value), None) = (pair.first(), pair.get(1), pair.get(2)) {
    let (symbol, info, location) = match name {
      Calcit::Symbol { sym, info, location } => (sym, info, location),
      Calcit::Local(local) => (&local.sym, &local.info, &local.location),
      _ => return Ok(Calcit::from(args.push_left(head.clone()))),
    };
    let value = prepare_expression(
      value,
      PreprocessContext::new(&definitions, &mut types, ctx.file_ns, ctx.check_warnings, ctx.call_stack),
      constraints,
    )?;
    let annotation =
      constraints.resolve(&resolve_type_value(&value, &types).unwrap_or_else(|| pending_variable(std::ptr::from_ref(name) as usize)));
    definitions.insert(symbol.clone());
    types.insert(symbol.clone(), annotation.clone());
    let local = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(symbol),
      sym: symbol.clone(),
      info: info.clone(),
      location: location.clone(),
      type_info: annotation,
    });
    Calcit::from(vec![local, value])
  } else {
    return Ok(Calcit::from(args.push_left(head.clone())));
  };
  let mut forms = vec![head.clone(), binding];
  for form in args.iter().skip(1) {
    forms.push(prepare_expression(
      form,
      PreprocessContext::new(&definitions, &mut types, ctx.file_ns, ctx.check_warnings, ctx.call_stack),
      constraints,
    )?);
  }
  Ok(Calcit::from(forms))
}

fn prepare_branches(
  head: &Calcit,
  args: &CalcitList,
  ctx: PreprocessContext,
  constraints: &mut Constraints,
) -> Result<Calcit, CalcitErr> {
  let Some(condition) = args.first() else {
    return Ok(Calcit::from(args.push_left(head.clone())));
  };
  let condition = prepare_expression(
    condition,
    PreprocessContext::new(ctx.scope_defs, ctx.scope_types, ctx.file_ns, ctx.check_warnings, ctx.call_stack),
    constraints,
  )?;
  let narrowing = extract_predicate_bindings(&condition, ctx.scope_types);
  let mut forms = vec![head.clone(), condition];
  let mut branch_types = Vec::new();
  for (index, branch) in args.iter().skip(1).enumerate() {
    let mut types = ctx.scope_types.clone();
    let binding = if index == 0 {
      &narrowing.true_binding
    } else {
      &narrowing.false_binding
    };
    if let Some((name, annotation)) = binding {
      types.insert(name.clone(), annotation.clone());
    }
    let prepared = prepare_expression(
      branch,
      PreprocessContext::new(ctx.scope_defs, &mut types, ctx.file_ns, ctx.check_warnings, ctx.call_stack),
      constraints,
    )?;
    if !type_inference::expression_definitely_diverges(&prepared) {
      branch_types.push(resolve_type_value(&prepared, &types));
    }
    forms.push(prepared);
  }
  constraints.join_branches(branch_types, &ctx)?;
  // Retain both lexical transfers before ordinary constant folding.
  Ok(Calcit::from(forms))
}

fn prepare_function(
  head: &Calcit,
  args: &CalcitList,
  ctx: PreprocessContext,
  constraints: &mut Constraints,
) -> Result<Calcit, CalcitErr> {
  let (Some(name), Some(Calcit::List(parameters))) = (args.first(), args.get(1)) else {
    return Ok(Calcit::from(args.push_left(head.clone())));
  };
  let lexical_generics =
    crate::calcit::type_annotation::free_type_variable_names(&ctx.scope_types.values().cloned().collect::<Vec<_>>());
  let signature = args
    .iter()
    .skip(2)
    .find_map(|form| CalcitTypeAnnotation::extract_surrounding_fn_annotation_from_hint_form_in_scope(form, &lexical_generics))
    .and_then(|annotation| annotation.resolve_to_fn());
  let mut definitions = ctx.scope_defs.clone();
  let mut types = ctx.scope_types.clone();
  let mut prepared_parameters = Vec::new();
  for (index, parameter) in parameters.iter().enumerate() {
    let (symbol, info, location) = match parameter {
      Calcit::Symbol { sym, info, location } => (sym, info, location),
      Calcit::Local(local) => (&local.sym, &local.info, &local.location),
      _ => {
        prepared_parameters.push(parameter.clone());
        continue;
      }
    };
    let annotation = signature
      .as_ref()
      .and_then(|signature| signature.arg_types.get(index))
      .cloned()
      .unwrap_or_else(|| pending_variable(std::ptr::from_ref(parameter) as usize));
    definitions.insert(symbol.clone());
    types.insert(symbol.clone(), annotation.clone());
    prepared_parameters.push(Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(symbol),
      sym: symbol.clone(),
      info: info.clone(),
      location: location.clone(),
      type_info: annotation,
    }));
  }
  let previous = CURRENT_FN_FEATURES.with(|features| {
    let mut features = features.borrow_mut();
    let previous = features.clone();
    let own = signature.as_ref().map(|signature| signature.features.clone());
    *features = match (own, previous.as_ref().filter(|_| call_stack_contains_macro(ctx.call_stack))) {
      (Some(own), Some(parent)) => Some(Arc::new(own.union(parent.as_ref()).cloned().collect())),
      (None, Some(parent)) => Some((*parent).clone()),
      (own, None) => own,
    }
    .map(lexical_function_features);
    previous
  });
  let _features = FunctionFeaturesScope { previous };
  let _frame = source_replay::Scope::enter_function(parameters, false);
  let mut forms = vec![head.clone(), name.clone(), Calcit::from(prepared_parameters)];
  for form in args.iter().skip(2) {
    forms.push(prepare_expression(
      form,
      PreprocessContext::new(&definitions, &mut types, ctx.file_ns, ctx.check_warnings, ctx.call_stack),
      constraints,
    )?);
  }
  Ok(Calcit::from(forms))
}

fn prepare_body(args: &CalcitList, ctx: PreprocessContext, constraints: &mut Constraints) -> Result<Vec<Calcit>, CalcitErr> {
  let mut body = Vec::new();
  for form in args.iter().skip(2) {
    body.push(prepare_expression(
      form,
      PreprocessContext::new(ctx.scope_defs, ctx.scope_types, ctx.file_ns, ctx.check_warnings, ctx.call_stack),
      constraints,
    )?);
  }
  Ok(body)
}

#[derive(Default)]
pub(super) struct Constraints {
  bindings: HashMap<Arc<str>, Arc<CalcitTypeAnnotation>>,
}

impl Constraints {
  /// Branch joins contribute equations for compiler-owned unresolved slots.
  /// User-declared generic variables and Dynamic are never refined here.
  pub(super) fn join_branches(
    &mut self,
    branches: Vec<Option<Arc<CalcitTypeAnnotation>>>,
    ctx: &PreprocessContext,
  ) -> Result<(), CalcitErr> {
    let mut previous = None;
    for branch in branches {
      let Some(branch) = branch else { continue };
      if let Some(previous) = &previous {
        for (actual, expected) in [(previous, &branch), (&branch, previous)] {
          let actual = self.resolve(actual);
          let expected = self.resolve(expected);
          let mut substitutions = HashMap::new();
          if actual.prove_with_bindings(&expected, &mut substitutions).is_proven() {
            for (name, value) in substitutions {
              if name.starts_with(PENDING_PREFIX) && self.bind(&name, &value).is_err() {
                return Err(constraint_error(ctx));
              }
            }
          }
        }
      }
      previous = Some(branch);
    }
    Ok(())
  }

  fn resolve(&self, annotation: &Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
    let mut current = annotation.clone();
    loop {
      let next = current.substitute_type_vars(&self.bindings);
      if next == current {
        return current;
      }
      current = next;
    }
  }

  fn bind(&mut self, name: &Arc<str>, value: &Arc<CalcitTypeAnnotation>) -> Result<(), ()> {
    let value = self.resolve(value);
    if matches!(value.as_ref(), CalcitTypeAnnotation::Never)
      || matches!(value.as_ref(), CalcitTypeAnnotation::TypeVar(other) if other == name)
    {
      return Ok(());
    }
    if value.contains_type_var_named(name) {
      return Err(());
    }
    let joined = if let Some(previous) = self.bindings.get(name) {
      let previous = self.resolve(previous);
      if let CalcitTypeAnnotation::TypeVar(other) = previous.as_ref()
        && other.starts_with(PENDING_PREFIX)
      {
        return self.bind(other, &value);
      }
      if let CalcitTypeAnnotation::TypeVar(other) = value.as_ref()
        && other.starts_with(PENDING_PREFIX)
      {
        return self.bind(other, &previous);
      }
      type_inference::join_return_types(previous, value).ok_or(())?
    } else {
      value
    };
    self.bindings.insert(name.clone(), joined);
    Ok(())
  }
}

/// Replace only covariant, inferred empty slots. Ref and function contracts are
/// intentionally opaque: their inputs cannot widen after initialization.
fn empty_slot_template(
  annotation: &Arc<CalcitTypeAnnotation>,
  prefix: &str,
  roots: &mut HashSet<Arc<str>>,
) -> Arc<CalcitTypeAnnotation> {
  use CalcitTypeAnnotation as T;
  let rewritten = match annotation.as_ref() {
    T::Never => {
      let name = Arc::<str>::from(format!("{prefix}/{}", roots.len()));
      roots.insert(name.clone());
      T::TypeVar(name)
    }
    T::List(member) => T::List(empty_slot_template(member, prefix, roots)),
    T::Set(member) => T::Set(empty_slot_template(member, prefix, roots)),
    T::Map(key, value) => T::Map(empty_slot_template(key, prefix, roots), empty_slot_template(value, prefix, roots)),
    T::Enum(definition, arguments) => T::Enum(
      definition.clone(),
      Arc::new(arguments.iter().map(|arg| empty_slot_template(arg, prefix, roots)).collect()),
    ),
    T::TypeRef(name, arguments) if annotation.resolve_to_enum().is_some() => T::TypeRef(
      name.clone(),
      Arc::new(arguments.iter().map(|arg| empty_slot_template(arg, prefix, roots)).collect()),
    ),
    _ => return annotation.clone(),
  };
  Arc::new(rewritten)
}

/// Solve symbolic payload slots rather than treating provisional locals as Dynamic.
/// The ordinary proof relation extracts nominal substitutions; occurs-checks keep
/// direct and indirect recursive payload equations finite.
pub(super) fn solve(
  args: &CalcitList,
  symbols: &[Arc<str>],
  initial: &[Arc<CalcitTypeAnnotation>],
  ctx: PreprocessContext,
) -> Result<Vec<Arc<CalcitTypeAnnotation>>, CalcitErr> {
  let mut roots = HashSet::new();
  let templates = initial
    .iter()
    .enumerate()
    .map(|(index, annotation)| {
      let prefix = format!("{PENDING_PREFIX}slot/{}/{index}", std::ptr::from_ref(args) as usize);
      empty_slot_template(annotation, &prefix, &mut roots)
    })
    .collect::<Vec<_>>();
  let mut constraints = Constraints::default();
  let mut current = templates.clone();
  loop {
    let mut types = ctx.scope_types.clone();
    for (symbol, annotation) in symbols.iter().zip(&current) {
      types.insert(symbol.clone(), annotation.clone());
    }
    let prepared = prepare_body(
      args,
      PreprocessContext::new(ctx.scope_defs, &mut types, ctx.file_ns, ctx.check_warnings, ctx.call_stack),
      &mut constraints,
    )?;
    source_replay::Scope::rewind_function();
    for transfer in type_inference::lexical_recur_inputs(&prepared, &types, initial.len()) {
      for (index, actual) in transfer.iter().enumerate() {
        if !type_inference::has_inferred_empty_slot(&initial[index]) {
          continue;
        }
        let Some(actual) = actual else { continue };
        let actual = constraints.resolve(actual);
        let mut substitutions = HashMap::new();
        // Incompatible nominal transfers are diagnosed by ordinary source
        // checking; they cannot supply substitutions for this declaration.
        match actual.prove_with_bindings(&templates[index], &mut substitutions) {
          crate::calcit::type_annotation::TypeProof::Proven => {}
          crate::calcit::type_annotation::TypeProof::NeedsBoundary(
            crate::calcit::type_annotation::TypeBoundaryReason::RecursiveTypeVariable,
          ) => return Err(constraint_error(&ctx)),
          _ => continue,
        }
        for (name, value) in substitutions {
          if roots.contains(&name) && constraints.bind(&name, &value).is_err() {
            return Err(constraint_error(&ctx));
          }
        }
      }
    }
    let next = templates
      .iter()
      .map(|annotation| constraints.resolve(annotation))
      .collect::<Vec<_>>();
    if next == current {
      let unresolved = crate::calcit::type_annotation::free_type_variable_names(&current);
      if unresolved
        .iter()
        .any(|name| name.starts_with(PENDING_PREFIX) && !roots.contains(name))
      {
        return Err(CalcitErr::use_msg_stack_location_with_code(
          CalcitErrKind::Type,
          "recur argument expects independently inferred payload evidence; a local input remains unresolved",
          "W_RECUR_ARG_TYPE_MISMATCH",
          ctx.call_stack,
          ctx.call_location.clone().or_else(|| args.first().and_then(Calcit::get_location)),
        ));
      }
      let absent = roots
        .into_iter()
        .map(|name| (name, crate::calcit::type_annotation::NEVER_TYPE.clone()))
        .collect();
      return Ok(current.iter().map(|annotation| annotation.substitute_type_vars(&absent)).collect());
    }
    current = next;
  }
}

fn constraint_error(ctx: &PreprocessContext) -> CalcitErr {
  CalcitErr::use_msg_stack_location_with_code(
    CalcitErrKind::Type,
    "recur argument expects a finite consistent payload type; its constraints contain an incompatible or recursive substitution",
    "W_RECUR_ARG_TYPE_MISMATCH",
    ctx.call_stack,
    ctx.call_location.clone(),
  )
}
