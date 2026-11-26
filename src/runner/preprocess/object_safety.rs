//! Object-safety validation for `(Dyn Trait)` and compile-time checks for `(dyn:box expr Trait)`.

use std::sync::Arc;

use super::*;
use crate::calcit::{
  self, Calcit, CalcitErr, CalcitErrKind, CalcitTrait, CalcitTraitMemberKind, CalcitTypeAnnotation, ImportInfo, NodeLocation,
};
use crate::call_stack::CallStackList;
use crate::program;

pub(crate) const TRAIT_NOT_OBJECT_SAFE_ERROR: &str = "E_TRAIT_NOT_OBJECT_SAFE";
pub(crate) const DYN_BOX_TYPE_ERROR: &str = "E_DYN_BOX_TYPE";

fn annotation_mentions_self(annotation: &CalcitTypeAnnotation) -> bool {
  annotation.any_component(|node| {
    matches!(node, CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "Self")
      || matches!(node, CalcitTypeAnnotation::TypeRef(name, _) if name.trim_start_matches('\'') == "Self")
  })
}

fn first_non_self_type_var(annotation: &CalcitTypeAnnotation) -> Option<Arc<str>> {
  let mut found = None;
  annotation.walk_components(&mut |node| {
    if found.is_some() {
      return false;
    }
    if let CalcitTypeAnnotation::TypeVar(name) = node
      && name.as_ref() != "Self"
    {
      found = Some(name.clone());
      return false;
    }
    true
  });
  found
}

/// Verify that `trait_def` and all of its transitive `requires` parents satisfy
/// the 6 object-safety rules for `(Dyn Trait)`.
pub(crate) fn check_trait_object_safe(trait_def: &CalcitTrait) -> Result<(), String> {
  if let Err(reason) = trait_def.validate_reachable_method_schemas() {
    return Err(format!("trait {} is not object-safe: {reason}", trait_def.origin_label()));
  }
  let reachable = trait_def
    .normalized_reachable_traits()
    .map_err(|reason| format!("trait {} is not object-safe: {reason}", trait_def.origin_label()))?;

  for required in reachable {
    if super::js_ffi::trait_is_external_object(required.as_ref()) {
      return Err(format!(
        "trait {} is not object-safe: {} is an `:ffi` external-object trait; pass it directly instead of `(Dyn ...)`",
        trait_def.origin_label(),
        required.origin_label()
      ));
    }

    for (idx, (member, kind)) in required.methods.iter().zip(required.member_kinds.iter()).enumerate() {
      if *kind == CalcitTraitMemberKind::Field {
        return Err(format!(
          "trait {} is not object-safe: {}.{} is a `:field` member, and only fixed-arity methods are allowed",
          trait_def.origin_label(),
          required.origin_label(),
          member.ref_str()
        ));
      }

      let Some(method_type) = required.method_types.get(idx) else {
        return Err(format!(
          "trait {} is not object-safe: `.{}` in {} has no method schema",
          trait_def.origin_label(),
          member.ref_str(),
          required.origin_label()
        ));
      };
      let CalcitTypeAnnotation::Fn(signature) = method_type.as_ref() else {
        return Err(format!(
          "trait {} is not object-safe: `.{}` requires an explicit fixed-arity `Fn` schema, got `{}`",
          trait_def.origin_label(),
          member.ref_str(),
          method_type.to_brief_string()
        ));
      };
      if !signature.cases.is_empty() {
        return Err(format!(
          "trait {} is not object-safe: `.{}` uses `:cases` overloaded signatures, which cannot use static vtable dispatch",
          trait_def.origin_label(),
          member.ref_str()
        ));
      }
      if signature.rest_type.is_some() {
        return Err(format!(
          "trait {} is not object-safe: `.{}` uses variadic `:rest` arguments, which cannot be boxed in `(Dyn ...)`",
          trait_def.origin_label(),
          member.ref_str()
        ));
      }
      if let Some(extra_generic) = signature.generics.iter().find(|name| name.as_ref() != "Self") {
        return Err(format!(
          "trait {} is not object-safe: `.{}` declares generic parameter `{extra_generic}`, and object-safe methods may only quantify `Self`",
          trait_def.origin_label(),
          member.ref_str()
        ));
      }
      if !signature.where_bounds.is_empty() {
        return Err(format!(
          "trait {} is not object-safe: `.{}` declares method `:where` bounds, which cannot be erased in `(Dyn ...)`",
          trait_def.origin_label(),
          member.ref_str()
        ));
      }

      let Some(receiver_ty) = signature.arg_types.first() else {
        return Err(format!(
          "trait {} is not object-safe: `.{}` must take bare `Self` as its first parameter",
          trait_def.origin_label(),
          member.ref_str()
        ));
      };
      if !matches!(receiver_ty.as_ref(), CalcitTypeAnnotation::TypeVar(name) if name.as_ref() == "Self") {
        return Err(format!(
          "trait {} is not object-safe: `.{}` must take bare `Self` as its first parameter, got `{}`",
          trait_def.origin_label(),
          member.ref_str(),
          receiver_ty.to_brief_string()
        ));
      }

      for (arg_idx, arg_ty) in signature.arg_types.iter().enumerate().skip(1) {
        if annotation_mentions_self(arg_ty.as_ref()) {
          return Err(format!(
            "trait {} is not object-safe: `.{}` mentions `Self` in parameter {} (`{}`)",
            trait_def.origin_label(),
            member.ref_str(),
            arg_idx + 1,
            arg_ty.to_brief_string()
          ));
        }
        if let Some(var) = first_non_self_type_var(arg_ty.as_ref()) {
          return Err(format!(
            "trait {} is not object-safe: `.{}` uses type variable `{var}` in parameter {}",
            trait_def.origin_label(),
            member.ref_str(),
            arg_idx + 1
          ));
        }
      }

      if annotation_mentions_self(signature.return_type.as_ref()) {
        return Err(format!(
          "trait {} is not object-safe: `.{}` mentions `Self` in return type (`{}`)",
          trait_def.origin_label(),
          member.ref_str(),
          signature.return_type.to_brief_string()
        ));
      }
      if let Some(var) = first_non_self_type_var(signature.return_type.as_ref()) {
        return Err(format!(
          "trait {} is not object-safe: `.{}` uses type variable `{var}` in return type",
          trait_def.origin_label(),
          member.ref_str()
        ));
      }
      if let Some(guard) = &signature.guard {
        if annotation_mentions_self(guard.as_ref()) {
          return Err(format!(
            "trait {} is not object-safe: `.{}` mentions `Self` in return guard",
            trait_def.origin_label(),
            member.ref_str()
          ));
        }
        if let Some(var) = first_non_self_type_var(guard.as_ref()) {
          return Err(format!(
            "trait {} is not object-safe: `.{}` uses type variable `{var}` in return guard",
            trait_def.origin_label(),
            member.ref_str()
          ));
        }
      }
    }
  }
  Ok(())
}

/// Resolve a `DynTrait` trait reference to a full `CalcitTrait` definition if possible.
/// Returns `Ok(None)` only when the target belongs to an unloaded module namespace.
pub(crate) fn resolve_dyn_trait_definition(
  trait_def: &CalcitTrait,
  file_ns: &str,
  scope_types: Option<&ScopeTypes>,
) -> Result<Option<Arc<CalcitTrait>>, String> {
  let has_def_ref = trait_def.definition_ref.is_some();
  let is_unresolved_ref = trait_def.runtime_id.is_none() && trait_def.methods.is_empty() && trait_def.requires.is_empty();

  if !is_unresolved_ref && (has_def_ref || trait_def.runtime_id.is_some()) {
    return Ok(Some(Arc::new(trait_def.clone())));
  }

  let raw_name = trait_def.definition_ref.as_deref().unwrap_or_else(|| trait_def.name.ref_str());

  if let Some(scope) = scope_types
    && !raw_name.contains('/')
    && let Some(local) = scope.type_reference(raw_name)
  {
    match local.as_ref() {
      CalcitTypeAnnotation::Trait(resolved) => return Ok(Some(resolved.clone())),
      _ => {
        if let Some(resolved) = local.as_trait_definition() {
          return Ok(Some(Arc::new(resolved.clone())));
        }
      }
    }
  }

  if let Some((ns, def)) = program::resolve_type_name_in_ns(file_ns, raw_name) {
    let (origin_ns, origin_def) = program::impl_attach::resolve_trait_origin_in_program(&ns, &def);
    if let Some(resolved) = super::lookup_source_backed_trait_def(&origin_ns, &origin_def) {
      return Ok(Some(Arc::new(resolved.with_definition_ref(&origin_ns, &origin_def))));
    }
    if let Some(Calcit::Trait(runtime_trait)) = super::resolve_program_value_for_preprocess(&origin_ns, &origin_def, None) {
      let resolved = if runtime_trait.definition_ref.is_some() {
        runtime_trait
      } else {
        Arc::new((*runtime_trait).clone().with_definition_ref(&origin_ns, &origin_def))
      };
      return Ok(Some(resolved));
    }
    if !program::has_ns_code(&origin_ns)
      && (super::unknown_type_names::imports_unloaded_ns(file_ns, raw_name, &origin_ns) || (origin_ns != ns && origin_ns.contains('.')))
    {
      return Ok(None);
    }
    if program::has_def_code(&origin_ns, &origin_def) {
      return Err(format!(
        "`(Dyn {raw_name})` requires an object-safe trait, but `{origin_ns}/{origin_def}` is not a trait definition"
      ));
    }
    if program::has_def_code(&ns, &def) {
      return Err(format!(
        "`(Dyn {raw_name})` requires an object-safe trait, but `{ns}/{def}` is not a trait definition"
      ));
    }
    return Err(format!("`(Dyn {raw_name})` refers to unknown trait `{origin_ns}/{origin_def}`"));
  }

  if trait_def.runtime_id.is_some() || !trait_def.methods.is_empty() || !trait_def.requires.is_empty() {
    return Ok(Some(Arc::new(trait_def.clone())));
  }
  Err(format!("`(Dyn {raw_name})` refers to unknown trait `{raw_name}` in `{file_ns}`"))
}

/// Reject any `(Dyn ...)` type annotation in `annotation` whose target is not
/// an object-safe trait.
pub(super) fn reject_non_object_safe_dyn_traits(
  owner: &str,
  position: &str,
  annotation: &Arc<CalcitTypeAnnotation>,
  file_ns: &str,
  scope_types: Option<&ScopeTypes>,
  call_stack: &CallStackList,
  location: Option<NodeLocation>,
) -> Result<(), CalcitErr> {
  if let Some(reason) = find_dyn_object_safety_error(annotation, position, file_ns, scope_types) {
    return Err(CalcitErr::use_msg_stack_location_with_code(
      CalcitErrKind::Type,
      format!("{owner} uses invalid `(Dyn ...)` type in {position}: {reason}"),
      TRAIT_NOT_OBJECT_SAFE_ERROR,
      call_stack,
      location,
    ));
  }
  Ok(())
}

fn find_dyn_object_safety_error(
  annotation: &Arc<CalcitTypeAnnotation>,
  path: &str,
  file_ns: &str,
  scope_types: Option<&ScopeTypes>,
) -> Option<String> {
  let mut visited_aliases = std::collections::HashSet::new();
  find_dyn_object_safety_error_inner(annotation, path, file_ns, scope_types, &mut visited_aliases)
}

fn find_dyn_object_safety_error_inner(
  annotation: &Arc<CalcitTypeAnnotation>,
  path: &str,
  file_ns: &str,
  scope_types: Option<&ScopeTypes>,
  visited_aliases: &mut std::collections::HashSet<String>,
) -> Option<String> {
  if let Some(found) = super::find_schema_annotation(annotation, path, &mut |candidate, candidate_path| match candidate.as_ref() {
    CalcitTypeAnnotation::TypeRef(name, args) if name.trim_start_matches('\'') == "Dyn" => {
      let detail = match args.as_slice() {
        [single] => format!("got `{}` instead of a trait", single.to_brief_string()),
        _ => format!("expected 1 trait argument, got {}", args.len()),
      };
      Some(format!("at {candidate_path}: `(Dyn ...)` expects an object-safe trait, {detail}"))
    }
    CalcitTypeAnnotation::DynTrait(trait_def) => match resolve_dyn_trait_definition(trait_def, file_ns, scope_types) {
      Ok(Some(resolved)) => check_trait_object_safe(resolved.as_ref())
        .err()
        .map(|err| format!("at {candidate_path}: {err}")),
      Ok(None) => None,
      Err(err) => Some(format!("at {candidate_path}: {err}")),
    },
    CalcitTypeAnnotation::TypeRef(name, args) => {
      let (alias_ns, alias_def) = program::resolve_type_name_in_ns(file_ns, name)?;
      let alias = crate::calcit::type_annotation::type_alias_declaration(&format!("{alias_ns}/{alias_def}"))?;
      if visited_aliases.len() >= 128 {
        return None;
      }
      let key = format!(
        "{alias_ns}/{alias_def}<{}>",
        args.iter().map(|arg| arg.to_brief_string()).collect::<Vec<_>>().join(",")
      );
      if !visited_aliases.insert(key.clone()) {
        return None;
      }
      let expanded = alias.expand(&alias_ns, args);
      let result = find_dyn_object_safety_error_inner(&expanded, candidate_path, &alias_ns, scope_types, visited_aliases);
      visited_aliases.remove(&key);
      result
    }
    CalcitTypeAnnotation::Fn(info) => info.cases.iter().enumerate().find_map(|(idx, case_sig)| {
      find_dyn_object_safety_error_inner(
        &Arc::new(CalcitTypeAnnotation::Fn(case_sig.clone())),
        &format!("{candidate_path}.cases.{idx}"),
        file_ns,
        scope_types,
        visited_aliases,
      )
    }),
    _ => None,
  }) {
    return Some(found);
  }
  None
}

/// Validate any `(Dyn ...)` references declared inside a `deftype` body.
pub(super) fn reject_non_object_safe_dyn_traits_in_deftype(
  ns: &str,
  def: &str,
  source_code: &Calcit,
  scope_types: &ScopeTypes,
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  let Some(alias) = crate::calcit::type_annotation::type_alias_from_code(source_code) else {
    return Ok(());
  };
  let generic_vars: Vec<Arc<CalcitTypeAnnotation>> = alias
    .generics
    .iter()
    .map(|g| Arc::new(CalcitTypeAnnotation::TypeVar(g.clone())))
    .collect();
  let expanded = alias.expand(ns, &generic_vars);
  let location = alias
    .body
    .get_location()
    .or_else(|| Some(NodeLocation::new(Arc::from(ns), Arc::from(def), Arc::new(vec![]))));
  reject_non_object_safe_dyn_traits(
    &format!("{ns}/{def}"),
    "deftype",
    &expanded,
    ns,
    Some(scope_types),
    call_stack,
    location,
  )
}

/// Validate `(Dyn ...)` references in `deftrait` method signatures.
pub(super) fn reject_non_object_safe_dyn_traits_in_deftrait(
  owner: &str,
  args: &CalcitList,
  file_ns: &str,
  scope_types: &ScopeTypes,
  call_stack: &CallStackList,
  fallback_location: Option<&NodeLocation>,
) -> Result<(), CalcitErr> {
  let Some(Calcit::List(methods)) = args.get(1) else {
    return Ok(());
  };
  for entry in methods.iter().skip(1) {
    let Calcit::List(pair) = entry else {
      continue;
    };
    let start = if matches!(pair.first(), Some(Calcit::Proc(CalcitProc::List)))
      || matches!(pair.first(), Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "[]")
    {
      1
    } else {
      0
    };
    let (Some(name_node), Some(type_form)) = (pair.get(start), pair.get(start + 1)) else {
      continue;
    };
    let method_name = match name_node {
      Calcit::Tag(tag) => tag.ref_str(),
      Calcit::Symbol { sym, .. } => sym.trim_start_matches(':').trim_start_matches('.'),
      _ => continue,
    };
    let inner_form = match type_form {
      Calcit::List(quoted) if quoted.len() == 2 && matches!(quoted.first(), Some(Calcit::Syntax(CalcitSyntax::Quote, _))) => &quoted[1],
      other => other,
    };
    if let Ok(method_annotation) = crate::builtins::meta::parse_trait_method_signature(method_name, inner_form) {
      reject_non_object_safe_dyn_traits(
        owner,
        &format!("deftrait method `.{method_name}`"),
        &method_annotation,
        file_ns,
        Some(scope_types),
        call_stack,
        type_form.get_location().or_else(|| fallback_location.cloned()),
      )?;
    }
  }
  Ok(())
}

/// Validate `(Dyn ...)` references in `defenum` variant payload types (`&enum-def:new`).
pub(super) fn reject_non_object_safe_dyn_traits_in_defenum(args: &CalcitList, ctx: &PreprocessContext) -> Result<(), CalcitErr> {
  let mut idx = 1;
  let mut generics: Vec<Arc<str>> = ctx.scope_types.generic_names();
  if let Some(Calcit::List(items)) = args.get(1) {
    let start = if matches!(items.first(), Some(Calcit::Proc(CalcitProc::List)))
      || matches!(items.first(), Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "[]")
      || matches!(items.first(), Some(Calcit::Import(CalcitImport { ns, def, .. })) if ns.as_ref() == calcit::CORE_NS && def.as_ref() == "[]")
    {
      1
    } else {
      0
    };
    if !matches!(items.get(start), Some(Calcit::Tag(_))) {
      for item in items.iter().skip(start) {
        if let Some(name) = match item {
          Calcit::Symbol { sym, .. } => Some(sym.trim_start_matches('\'')),
          Calcit::List(q) if q.len() == 2 && matches!(q.first(), Some(Calcit::Syntax(CalcitSyntax::Quote, _))) => match q.get(1) {
            Some(Calcit::Symbol { sym, .. }) => Some(sym.trim_start_matches('\'')),
            _ => None,
          },
          _ => None,
        } && !name.is_empty()
        {
          generics.push(Arc::from(name));
        }
      }
      idx = 2;
    }
  }
  if matches!(args.get(idx), Some(Calcit::Map(_)))
    || matches!(args.get(idx), Some(Calcit::List(m)) if matches!(m.first(), Some(Calcit::Proc(CalcitProc::NativeMap))) || matches!(m.first(), Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "{}"))
  {
    idx += 1;
  }

  for variant in args.iter().skip(idx) {
    let Calcit::List(entry) = variant else {
      continue;
    };
    let start = if matches!(entry.first(), Some(Calcit::Proc(CalcitProc::List)))
      || matches!(entry.first(), Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "[]")
      || matches!(entry.first(), Some(Calcit::Import(CalcitImport { ns, def, .. })) if ns.as_ref() == calcit::CORE_NS && def.as_ref() == "[]")
    {
      1
    } else {
      0
    };
    if !matches!(entry.get(start), Some(Calcit::Tag(_))) {
      continue;
    }
    for payload in entry.iter().skip(start + 1) {
      let unwrapped = match payload {
        Calcit::List(quoted)
          if quoted.len() == 2
            && (matches!(quoted.first(), Some(Calcit::Syntax(CalcitSyntax::Quote, _)))
              || matches!(quoted.first(), Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "quote")) =>
        {
          &quoted[1]
        }
        other => other,
      };
      let desugared = super::desugar_compact_type_form(unwrapped, ctx)?;
      let payload_type = super::resolve_local_type_refs_for_body(
        CalcitTypeAnnotation::parse_type_annotation_form_with_generics(&desugared, &generics),
        ctx.scope_types,
      );
      reject_non_object_safe_dyn_traits(
        ctx.file_ns,
        "defenum payload",
        &payload_type,
        ctx.file_ns,
        Some(ctx.scope_types),
        ctx.call_stack,
        payload.get_location().or_else(|| ctx.call_location.clone()),
      )?;
    }
  }
  Ok(())
}

fn resolve_dyn_box_trait_operand(trait_raw: &Calcit, ctx: &mut PreprocessContext) -> Result<(Arc<CalcitTrait>, Calcit), CalcitErr> {
  let location = trait_raw.get_location();
  let fail = |msg: String| {
    CalcitErr::use_msg_stack_location_with_code(CalcitErrKind::Type, msg, DYN_BOX_TYPE_ERROR, ctx.call_stack, location.clone())
  };

  let quoted_symbol = match trait_raw {
    Calcit::List(items) if items.len() == 2 && matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::Quote, _))) => items.get(1),
    _ => None,
  };

  if let Some(Calcit::Trait(trait_def)) = quoted_symbol.or(Some(trait_raw)) {
    return Ok((trait_def.clone(), Calcit::Trait(trait_def.clone())));
  }

  let preprocessed_trait = if quoted_symbol.is_none() {
    super::preprocess_expr(
      trait_raw,
      ctx.scope_defs,
      ctx.scope_types,
      ctx.file_ns,
      ctx.check_warnings,
      ctx.call_stack,
    )?
  } else {
    trait_raw.clone()
  };

  match &preprocessed_trait {
    Calcit::Trait(trait_def) => Ok((trait_def.clone(), preprocessed_trait)),
    Calcit::Import(import) => {
      if let Some(Calcit::Trait(trait_def)) = super::resolve_program_value_for_preprocess(&import.ns, &import.def, import.def_id) {
        let resolved = if trait_def.definition_ref.is_some() {
          trait_def
        } else {
          Arc::new((*trait_def).clone().with_definition_ref(&import.ns, &import.def))
        };
        return Ok((resolved, preprocessed_trait));
      }
      if let Some(resolved) = super::lookup_trait_ns_def_for_preprocess(&import.ns, &import.def, ctx.check_warnings, ctx.call_stack)? {
        return Ok((resolved, preprocessed_trait));
      }
      Err(fail(format!(
        "dyn:box expects a trait as its second argument, but `{}/{}` is not a trait definition",
        import.ns, import.def
      )))
    }
    Calcit::Local(local) => {
      if let Some(ann) = ctx.scope_types.type_reference(&local.sym) {
        if let CalcitTypeAnnotation::Trait(trait_def) = ann.as_ref() {
          return Ok((trait_def.clone(), preprocessed_trait));
        }
        if let Some(trait_def) = ann.as_trait_definition() {
          return Ok((Arc::new(trait_def.clone()), preprocessed_trait));
        }
      }
      Err(fail(format!(
        "dyn:box expects a statically resolved trait as its second argument, got local `{}`",
        local.sym
      )))
    }
    _ => {
      let symbol_candidate = quoted_symbol.unwrap_or(trait_raw);
      let Some((ns, def)) = super::resolve_trait_reference_target(symbol_candidate, ctx.file_ns) else {
        return Err(fail(format!("dyn:box expects a trait as its second argument, got `{trait_raw}`")));
      };
      let Some(resolved) = super::lookup_trait_ns_def_for_preprocess(&ns, &def, ctx.check_warnings, ctx.call_stack)? else {
        return Err(fail(format!(
          "dyn:box expects a trait as its second argument, but `{ns}/{def}` is not a trait definition"
        )));
      };
      let info = if ns.as_ref() == calcit::CORE_NS {
        ImportInfo::Core {
          at_ns: Arc::from(ctx.file_ns),
        }
      } else {
        ImportInfo::NsReferDef {
          at_ns: Arc::from(ctx.file_ns),
          at_def: def.clone(),
        }
      };
      let import_expr = Calcit::Import(crate::calcit::CalcitImport {
        ns: ns.clone(),
        def: def.clone(),
        info: Arc::new(info),
        def_id: Some(program::ensure_def_id(&ns, &def).0),
      });
      Ok((resolved, import_expr))
    }
  }
}

fn dyn_box_open_type_reason(annotation: &CalcitTypeAnnotation) -> Option<&'static str> {
  match annotation {
    CalcitTypeAnnotation::Never => Some("`Never` bottom type"),
    CalcitTypeAnnotation::DynTrait(_) => Some("existing `(Dyn ...)` trait object"),
    _ => dyn_box_nested_open_type_reason(annotation),
  }
}

fn dyn_box_nested_open_type_reason(annotation: &CalcitTypeAnnotation) -> Option<&'static str> {
  match annotation {
    CalcitTypeAnnotation::Dynamic => Some("open `Dynamic` type"),
    CalcitTypeAnnotation::TypeVar(_) => Some("unbound generic type variable"),
    CalcitTypeAnnotation::TypeSlot(_) => Some("unbound type slot"),
    CalcitTypeAnnotation::Custom(_) => Some("untyped custom type"),
    CalcitTypeAnnotation::Nullable(_, _) => Some("nullable type"),
    CalcitTypeAnnotation::DynFn => Some("open `DynFn` function type"),
    CalcitTypeAnnotation::AnonymousEnum => Some("open anonymous `Enum` type"),
    CalcitTypeAnnotation::Trait(_) | CalcitTypeAnnotation::TraitSet(_) => Some("bare trait constraint"),
    CalcitTypeAnnotation::JsObject | CalcitTypeAnnotation::FfiResource => match annotation.host_target() {
      Some(crate::ffi_boundary::HostTarget::Js) => Some("opaque `JsObject` host value"),
      Some(crate::ffi_boundary::HostTarget::Native) => Some("opaque `FfiResource` handle"),
      Some(crate::ffi_boundary::HostTarget::Wasm) | None => unreachable!("host-target partition covers JsObject and FfiResource"),
    },
    CalcitTypeAnnotation::Macro(_) | CalcitTypeAnnotation::Syntax(_) => Some("compile-time macro/syntax value"),
    CalcitTypeAnnotation::Variadic(_) => Some("variadic parameter constraint"),
    CalcitTypeAnnotation::StructDef(_) | CalcitTypeAnnotation::EnumDef(_) => Some("type definition value"),
    CalcitTypeAnnotation::Fn(sig) => {
      if sig.fn_kind != crate::calcit::SchemaKind::Fn {
        return Some("compile-time macro/syntax value");
      }
      if !sig.generics.is_empty() || !sig.cases.is_empty() {
        return Some("polymorphic or multi-case function type");
      }
      sig
        .arg_types
        .iter()
        .find_map(|arg| dyn_box_nested_open_type_reason(arg))
        .or_else(|| sig.rest_type.as_deref().and_then(dyn_box_nested_open_type_reason))
        .or_else(|| dyn_box_nested_open_type_reason(&sig.return_type))
    }
    CalcitTypeAnnotation::List(inner) | CalcitTypeAnnotation::Set(inner) | CalcitTypeAnnotation::Ref(inner) => {
      dyn_box_nested_open_type_reason(inner)
    }
    CalcitTypeAnnotation::Map(k, v) => dyn_box_nested_open_type_reason(k).or_else(|| dyn_box_nested_open_type_reason(v)),
    CalcitTypeAnnotation::Tuple(items) => items.iter().find_map(|item| dyn_box_nested_open_type_reason(item)),
    CalcitTypeAnnotation::TypeRef(name, args) => {
      if CalcitTypeAnnotation::open_family_type(name).is_some() {
        return Some("open family type");
      }
      args.iter().find_map(|arg| dyn_box_nested_open_type_reason(arg))
    }
    CalcitTypeAnnotation::Struct(_, args) | CalcitTypeAnnotation::Enum(_, args) => {
      args.iter().find_map(|arg| dyn_box_nested_open_type_reason(arg))
    }
    _ => None,
  }
}

/// Preprocess `(dyn:box expr Trait)`: check object-safety of `Trait` and verify
/// `expr` has a concrete non-open type implementing `Trait` (and all its parents).
pub(super) fn preprocess_dyn_box(
  head: &CalcitSyntax,
  head_ns: &str,
  args: &CalcitList,
  ctx: &mut PreprocessContext,
) -> Result<Calcit, CalcitErr> {
  let location = args
    .first()
    .and_then(Calcit::get_location)
    .or_else(|| args.get(1).and_then(Calcit::get_location));
  if args.len() != 2 {
    return Err(CalcitErr::use_msg_stack_location_with_code(
      CalcitErrKind::Arity,
      format!("dyn:box expected an expression and a trait, got {}", args.len()),
      DYN_BOX_TYPE_ERROR,
      ctx.call_stack,
      location,
    ));
  }

  let expr_form = super::preprocess_expr(
    args.get(0).expect("checked 2 args"),
    ctx.scope_defs,
    ctx.scope_types,
    ctx.file_ns,
    ctx.check_warnings,
    ctx.call_stack,
  )?;
  super::check_value_position_nil_branch(
    &expr_form,
    "dyn:box value",
    true,
    ctx.scope_types,
    ctx.file_ns,
    ctx.call_stack,
    location.as_ref(),
  )?;

  let (trait_def, runtime_trait_expr) = resolve_dyn_box_trait_operand(args.get(1).expect("checked 2 args"), ctx)?;
  check_trait_object_safe(trait_def.as_ref()).map_err(|reason| {
    CalcitErr::use_msg_stack_location_with_code(
      CalcitErrKind::Type,
      reason,
      TRAIT_NOT_OBJECT_SAFE_ERROR,
      ctx.call_stack,
      args.get(1).and_then(Calcit::get_location).or_else(|| location.clone()),
    )
  })?;

  let Some(raw_expr_type) = super::type_inference::resolve_type_value(&expr_form, ctx.scope_types) else {
    return Err(CalcitErr::use_msg_stack_location_with_code(
      CalcitErrKind::Type,
      format!(
        "dyn:box cannot box value into `(Dyn {})`: value type is unknown; annotate the value concretely before boxing",
        trait_def.name
      ),
      DYN_BOX_TYPE_ERROR,
      ctx.call_stack,
      location,
    ));
  };

  let expanded_expr_type = CalcitTypeAnnotation::expand_value_type_aliases(&raw_expr_type);
  let resolved_expr_type = match expanded_expr_type.as_ref() {
    CalcitTypeAnnotation::TypeRef(_, type_args) => {
      if let Some(struct_def) = expanded_expr_type.resolve_to_struct() {
        Arc::new(CalcitTypeAnnotation::Struct(Arc::new(struct_def), type_args.clone()))
      } else if let Some(enum_def) = expanded_expr_type.resolve_to_enum() {
        Arc::new(CalcitTypeAnnotation::Enum(Arc::new(enum_def), type_args.clone()))
      } else {
        expanded_expr_type.clone()
      }
    }
    _ => expanded_expr_type.clone(),
  };

  if resolved_expr_type.resolve_to_union().is_some() {
    return Err(CalcitErr::use_msg_stack_location_with_code(
      CalcitErrKind::Type,
      format!(
        "dyn:box cannot box union value `{}` directly into `(Dyn {})`; match its variant first so each concrete branch boxes unambiguously",
        resolved_expr_type.describe(),
        trait_def.name
      ),
      DYN_BOX_TYPE_ERROR,
      ctx.call_stack,
      location,
    ));
  }

  if let Some(open_reason) = dyn_box_open_type_reason(resolved_expr_type.as_ref()) {
    return Err(CalcitErr::use_msg_stack_location_with_code(
      CalcitErrKind::Type,
      format!(
        "dyn:box cannot box value of {open_reason} (`{}`) into `(Dyn {})`",
        resolved_expr_type.describe(),
        trait_def.name
      ),
      DYN_BOX_TYPE_ERROR,
      ctx.call_stack,
      location,
    ));
  }

  if let Ok(reachable) = trait_def.normalized_reachable_traits() {
    for required in reachable {
      if !resolved_expr_type.satisfies_trait_bound(required.as_ref()) {
        return Err(CalcitErr::use_msg_stack_location_with_code(
          CalcitErrKind::Type,
          format!(
            "dyn:box value of type `{}` does not implement trait {}",
            resolved_expr_type.describe(),
            required.origin_label()
          ),
          DYN_BOX_TYPE_ERROR,
          ctx.call_stack,
          location,
        ));
      }
      if let Some(impls) = super::get_impls_from_type(resolved_expr_type.as_ref()) {
        let matches = impls.iter().filter(|imp| imp.implements_trait(required.as_ref())).count();
        if matches > 1 {
          return Err(CalcitErr::use_msg_stack_location_with_code(
            CalcitErrKind::Type,
            format!(
              "dyn:box value of type `{}` has {matches} implementations of trait {}; cannot select unique vtable",
              resolved_expr_type.describe(),
              required.origin_label()
            ),
            "E_DUPLICATE_TRAIT_IMPL",
            ctx.call_stack,
            location,
          ));
        }
      }
    }
  } else if !resolved_expr_type.satisfies_trait_bound(trait_def.as_ref()) {
    return Err(CalcitErr::use_msg_stack_location_with_code(
      CalcitErrKind::Type,
      format!(
        "dyn:box value of type `{}` does not implement trait {}",
        resolved_expr_type.describe(),
        trait_def.origin_label()
      ),
      DYN_BOX_TYPE_ERROR,
      ctx.call_stack,
      location,
    ));
  }

  Ok(Calcit::from(vec![
    Calcit::Syntax(head.to_owned(), Arc::from(head_ns)),
    expr_form,
    runtime_trait_expr,
  ]))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::{CalcitFnTypeAnnotation, CalcitGenericBound, CalcitTraitMemberKind, FnOpenReasonNote, SchemaKind};
  use std::collections::HashSet;

  fn fn_sig(
    generics: Vec<&str>,
    where_bounds: Vec<CalcitGenericBound>,
    arg_types: Vec<Arc<CalcitTypeAnnotation>>,
    return_type: Arc<CalcitTypeAnnotation>,
    rest_type: Option<Arc<CalcitTypeAnnotation>>,
    cases: Vec<Arc<CalcitFnTypeAnnotation>>,
  ) -> Arc<CalcitTypeAnnotation> {
    Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      generics: Arc::new(generics.into_iter().map(Arc::from).collect()),
      where_bounds: Arc::new(where_bounds),
      arg_types,
      return_type,
      fn_kind: SchemaKind::Fn,
      rest_type,
      features: Arc::new(HashSet::new()),
      runtime_arity: None,
      open_reason: FnOpenReasonNote(None),
      guard: None,
      cases: Arc::new(cases),
    })))
  }

  fn self_var() -> Arc<CalcitTypeAnnotation> {
    Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("Self")))
  }

  #[test]
  fn object_safety_checks_all_six_rules() {
    let str_t = Arc::new(CalcitTypeAnnotation::String);
    let num_t = Arc::new(CalcitTypeAnnotation::Number);

    // Valid object-safe trait
    let ok = CalcitTrait::new(
      EdnTag::new("SafeShow"),
      vec![EdnTag::new("show")],
      vec![fn_sig(vec![], vec![], vec![self_var()], str_t.clone(), None, vec![])],
    );
    check_trait_object_safe(&ok).expect("Show-like trait must be object-safe");

    // Rule 2b: Field member is rejected
    let field_trait = CalcitTrait::new_with_member_kinds(
      EdnTag::new("WithField"),
      vec![EdnTag::new("size")],
      vec![num_t.clone()],
      Some(vec![CalcitTraitMemberKind::Field]),
    );
    assert!(check_trait_object_safe(&field_trait).is_err());

    // Rule 3a: DynFn method is rejected
    let dyn_fn_trait = CalcitTrait::new(
      EdnTag::new("DynFnMethod"),
      vec![EdnTag::new("m")],
      vec![Arc::new(CalcitTypeAnnotation::DynFn)],
    );
    assert!(check_trait_object_safe(&dyn_fn_trait).is_err());

    // Rule 3b: :rest method is rejected
    let rest_trait = CalcitTrait::new(
      EdnTag::new("RestMethod"),
      vec![EdnTag::new("m")],
      vec![fn_sig(vec![], vec![], vec![self_var()], num_t.clone(), Some(num_t.clone()), vec![])],
    );
    assert!(check_trait_object_safe(&rest_trait).is_err());

    // Rule 3c: :cases method is rejected
    let case_sig = Arc::new(CalcitFnTypeAnnotation {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![self_var()],
      return_type: str_t.clone(),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(HashSet::new()),
      runtime_arity: None,
      open_reason: FnOpenReasonNote(None),
      guard: None,
      cases: Arc::new(vec![]),
    });
    let cases_trait = CalcitTrait::new(
      EdnTag::new("CasesMethod"),
      vec![EdnTag::new("m")],
      vec![fn_sig(vec![], vec![], vec![self_var()], str_t.clone(), None, vec![case_sig])],
    );
    assert!(check_trait_object_safe(&cases_trait).is_err());

    // Rule 4: arg 0 must be bare Self
    let non_self_arg0 = CalcitTrait::new(
      EdnTag::new("NonSelfArg0"),
      vec![EdnTag::new("m")],
      vec![fn_sig(
        vec![],
        vec![],
        vec![Arc::new(CalcitTypeAnnotation::List(self_var()))],
        str_t.clone(),
        None,
        vec![],
      )],
    );
    assert!(check_trait_object_safe(&non_self_arg0).is_err());

    // Rule 5a: Self in remaining arg_types is rejected
    let self_in_arg1 = CalcitTrait::new(
      EdnTag::new("EqLike"),
      vec![EdnTag::new("eq?")],
      vec![fn_sig(
        vec![],
        vec![],
        vec![self_var(), self_var()],
        Arc::new(CalcitTypeAnnotation::Bool),
        None,
        vec![],
      )],
    );
    assert!(check_trait_object_safe(&self_in_arg1).is_err());

    // Rule 5b: Self in return_type is rejected
    let self_in_ret = CalcitTrait::new(
      EdnTag::new("CloneLike"),
      vec![EdnTag::new("clone")],
      vec![fn_sig(vec![], vec![], vec![self_var()], self_var(), None, vec![])],
    );
    assert!(check_trait_object_safe(&self_in_ret).is_err());

    // Rule 6: non-Self :generics on method is rejected
    let extra_generic = CalcitTrait::new(
      EdnTag::new("ExtraGeneric"),
      vec![EdnTag::new("map")],
      vec![fn_sig(
        vec!["U"],
        vec![],
        vec![self_var(), Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("U")))],
        num_t,
        None,
        vec![],
      )],
    );
    assert!(check_trait_object_safe(&extra_generic).is_err());

    // Rule 1 / transitive requires: child of non-object-safe parent is rejected
    let child_bad = CalcitTrait::new(
      EdnTag::new("ChildOfBad"),
      vec![EdnTag::new("ok")],
      vec![fn_sig(vec![], vec![], vec![self_var()], str_t, None, vec![])],
    )
    .with_requires(vec![Arc::new(self_in_ret)])
    .expect("valid requires");
    assert!(check_trait_object_safe(&child_bad).is_err());
  }
}
