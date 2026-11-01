//! Restricted unions declared with `defunion`.
//!
//! A union value carries no wrapper, so the only way to use one is a `match`
//! with type patterns such as `(String s)`. The match lowers to a chain of
//! runtime kind tests, one per member, the same way `match (data-view x)`
//! does; each branch binds the matched value with the member's type.

use std::sync::Arc;

use super::{
  PreprocessContext, core_import, gen_check_warning, generated_call, generated_if, generated_let, generated_path_symbol,
  preprocess_expr,
};
use crate::calcit::{
  self, Calcit, CalcitErr, CalcitErrKind, CalcitImport, CalcitList, CalcitLocal, CalcitProc, CalcitSymbolInfo, CalcitTypeAnnotation,
  CalcitUnionDef, ImportInfo,
  union_def::{UnionMember, UnionMemberKind},
};
use crate::program;

/// The checked union behind a type, when it names a `defunion`.
pub(super) fn union_of_type(annotation: &CalcitTypeAnnotation) -> Option<Arc<CalcitUnionDef>> {
  annotation.resolve_to_union()
}

fn union_error(message: String, code: &str, ctx: &PreprocessContext, location: Option<&Calcit>) -> CalcitErr {
  CalcitErr::use_msg_stack_location_with_code(
    CalcitErrKind::Type,
    message,
    code,
    ctx.call_stack,
    location.and_then(|node| node.get_location()),
  )
}

/// Check a `defunion` declaration where it is written, so an invalid union is
/// rejected even when no code uses its type yet.
pub(super) fn check_union_declaration(
  head: &Calcit,
  args: &CalcitList,
  file_ns: &str,
  call_stack: &crate::call_stack::CallStackList,
) -> Result<(), CalcitErr> {
  if !matches!(head, Calcit::Proc(CalcitProc::NativeUnionNew)) {
    return Ok(());
  }
  fn unquote(form: &Calcit) -> &Calcit {
    match form {
      Calcit::List(items) if matches!(items.first(), Some(Calcit::Syntax(calcit::CalcitSyntax::Quote, _))) => {
        items.get(1).unwrap_or(form)
      }
      _ => form,
    }
  }
  // The macro passes `(turn-tag name)` or a literal tag, then quoted member forms.
  let name = match args.first().map(unquote) {
    Some(Calcit::Tag(tag)) => tag.ref_str().to_owned(),
    Some(Calcit::Symbol { sym, .. }) => sym.to_string(),
    Some(Calcit::List(items)) => match items.get(1).map(unquote) {
      Some(Calcit::Tag(tag)) => tag.ref_str().to_owned(),
      Some(Calcit::Symbol { sym, .. }) => sym.to_string(),
      _ => return Ok(()),
    },
    _ => return Ok(()),
  };
  let members = args.iter().skip(1).map(|form| unquote(form).to_owned()).collect();
  let union_def = CalcitUnionDef::new(cirru_edn::EdnTag::from(name.as_str()), members).with_definition_ref(file_ns, &name);
  union_def.checked_members().map(|_| ()).map_err(|message| {
    CalcitErr::use_msg_stack_location_with_code(CalcitErrKind::Type, message, "E_UNION_MEMBER_INVALID", call_stack, None)
  })
}

/// Whether any branch uses a type pattern `(Name binding)`, i.e. a pattern
/// list whose head is a symbol. Enum patterns always start with a tag.
pub(super) fn has_type_pattern(args: &CalcitList) -> bool {
  args.iter().skip(1).any(|branch| {
    matches!(branch, Calcit::List(pair)
      if matches!(pair.first(), Some(Calcit::List(pattern)) if matches!(pattern.first(), Some(Calcit::Symbol { .. }))))
  })
}

fn definition_ref_node(ns: &Arc<str>, def: &Arc<str>, file_ns: &str, def_name: &str) -> Calcit {
  let info = if ns.as_ref() == file_ns {
    ImportInfo::SameFile {
      at_def: Arc::from(def_name),
    }
  } else {
    ImportInfo::NsReferDef {
      at_ns: Arc::from(file_ns),
      at_def: Arc::from(def_name),
    }
  };
  Calcit::Import(CalcitImport {
    ns: ns.to_owned(),
    def: def.to_owned(),
    info: Arc::new(info),
    def_id: Some(program::ensure_def_id(ns, def).0),
  })
}

/// The runtime test selecting one member. `value` is a Dynamic-typed local,
/// so the tests never observe the union type itself.
fn member_test(member: &UnionMember, value: &Calcit, file_ns: &str, def_name: &str) -> Calcit {
  let proc_test = |proc: CalcitProc| generated_call(vec![Calcit::Proc(proc), value.to_owned()]);
  match &member.kind {
    UnionMemberKind::Bool => proc_test(CalcitProc::BoolQuestion),
    UnionMemberKind::Number => proc_test(CalcitProc::NumberQuestion),
    UnionMemberKind::String => proc_test(CalcitProc::StringQuestion),
    UnionMemberKind::Tag => proc_test(CalcitProc::TagQuestion),
    UnionMemberKind::Symbol => proc_test(CalcitProc::SymbolQuestion),
    UnionMemberKind::List => proc_test(CalcitProc::ListQuestion),
    UnionMemberKind::Map => proc_test(CalcitProc::MapQuestion),
    UnionMemberKind::Set => proc_test(CalcitProc::SetQuestion),
    UnionMemberKind::Fn => proc_test(CalcitProc::FnQuestion),
    UnionMemberKind::Ref => generated_call(vec![core_import("ref?", file_ns), value.to_owned()]),
    UnionMemberKind::Buffer => generated_call(vec![core_import("buffer?", file_ns), value.to_owned()]),
    UnionMemberKind::CirruQuote => generated_call(vec![core_import("cirru-quote?", file_ns), value.to_owned()]),
    UnionMemberKind::Struct(_) => {
      let (ns, def) = member.definition.as_ref().expect("checked struct member has a definition");
      generated_if(
        proc_test(CalcitProc::StructQuestion),
        generated_call(vec![
          Calcit::Proc(CalcitProc::NativeStructMatches),
          value.to_owned(),
          definition_ref_node(ns, def, file_ns, def_name),
        ]),
        Calcit::Bool(false),
        file_ns,
      )
    }
    UnionMemberKind::Enum(_) => {
      let (ns, def) = member.definition.as_ref().expect("checked enum member has a definition");
      generated_if(
        proc_test(CalcitProc::EnumQuestion),
        generated_call(vec![
          Calcit::Proc(CalcitProc::NativeEquals),
          generated_call(vec![Calcit::Proc(CalcitProc::NativeEnumDefinition), value.to_owned()]),
          generated_call(vec![
            core_import("Option", file_ns),
            Calcit::tag("some"),
            definition_ref_node(ns, def, file_ns, def_name),
          ]),
        ]),
        Calcit::Bool(false),
        file_ns,
      )
    }
  }
}

fn typed_local(
  sym: &Arc<str>,
  info: &Arc<CalcitSymbolInfo>,
  location: &Option<Arc<Vec<u16>>>,
  type_info: Arc<CalcitTypeAnnotation>,
) -> Calcit {
  Calcit::Local(CalcitLocal {
    idx: CalcitLocal::track_sym(sym),
    sym: sym.to_owned(),
    info: info.to_owned(),
    location: location.to_owned(),
    type_info,
  })
}

fn nominal_pattern_type(name: &str, file_ns: &str) -> CalcitTypeAnnotation {
  let (ns, def) = program::resolve_type_name_in_ns(file_ns, name).unwrap_or_else(|| (Arc::from(calcit::CORE_NS), Arc::from(name)));
  CalcitTypeAnnotation::TypeRef(Arc::from(format!("{ns}/{def}")), Arc::new(vec![]))
}

fn same_nominal_pattern(member: &UnionMember, pattern: &CalcitTypeAnnotation) -> bool {
  match member.kind {
    UnionMemberKind::Struct(_) => pattern
      .resolve_to_struct()
      .zip(member.annotation.resolve_to_struct())
      .is_some_and(|(actual, expected)| actual.same_nominal_definition(&expected)),
    UnionMemberKind::Enum(_) => pattern
      .resolve_to_enum()
      .zip(member.annotation.resolve_to_enum())
      .is_some_and(|(actual, expected)| actual.same_nominal_definition(&expected)),
    _ => false,
  }
}

/// Lower `match` on a union value. `value_form` is already preprocessed.
pub(super) fn lower_union_match(
  union_def: &CalcitUnionDef,
  value_form: &Calcit,
  args: &CalcitList,
  ctx: &mut PreprocessContext,
) -> Result<Calcit, CalcitErr> {
  let file_ns = ctx.file_ns;
  let def_name: Arc<str> = ctx
    .call_stack
    .0
    .first()
    .map(|frame| frame.def.to_owned())
    .unwrap_or_else(|| Arc::from(calcit::GENERATED_DEF));
  let union_name = union_def
    .definition_ref()
    .map(|reference| reference.to_string())
    .unwrap_or_else(|| union_def.name.ref_str().to_owned());
  let members = union_def
    .checked_members()
    .map_err(|message| union_error(message, "E_UNION_MEMBER_INVALID", ctx, args.first()))?;

  // (member index, binding, body)
  let mut arms: Vec<(usize, Option<Calcit>, &Calcit)> = vec![];
  let mut wildcard: Option<&Calcit> = None;
  for (idx, branch) in args.iter().enumerate().skip(1) {
    let Calcit::List(pair) = branch else {
      return Err(union_error(
        format!("match branch expected a 2-element list (pattern body), got: {branch}"),
        "E_UNION_MATCH_PATTERN",
        ctx,
        Some(branch),
      ));
    };
    if pair.len() != 2 {
      return Err(union_error(
        format!("match branch expected a 2-element list (pattern body), got: {branch}"),
        "E_UNION_MATCH_PATTERN",
        ctx,
        Some(branch),
      ));
    }
    match &pair[0] {
      Calcit::Symbol { sym, .. } if sym.as_ref() == "_" => {
        if idx + 1 != args.len() {
          return Err(union_error(
            "match wildcard `_` must be the last branch".to_owned(),
            "E_UNION_MATCH_PATTERN",
            ctx,
            Some(&pair[0]),
          ));
        }
        wildcard = Some(&pair[1]);
      }
      Calcit::List(pattern) => {
        let Some(Calcit::Symbol { sym: type_name, .. }) = pattern.first() else {
          return Err(union_error(
            format!(
              "match on union {union_name} takes type patterns such as `(String s)`; tag patterns are for enums, got: {}",
              pair[0]
            ),
            "E_UNION_MATCH_PATTERN",
            ctx,
            Some(&pair[0]),
          ));
        };
        // Canonical member names remain valid labels. Other spellings must
        // resolve to the same nominal definition, including constructor aliases.
        let Some(member_index) = members
          .iter()
          .position(|member| member.pattern_name.as_ref() == type_name.as_ref())
          .or_else(|| {
            let pattern_type = nominal_pattern_type(type_name, file_ns);
            members.iter().position(|member| same_nominal_pattern(member, &pattern_type))
          })
        else {
          let names: Vec<&str> = members.iter().map(|member| member.pattern_name.as_ref()).collect();
          return Err(union_error(
            format!(
              "`{type_name}` is not a member of union {union_name}; members are [{}]",
              names.join(", ")
            ),
            "E_UNION_MATCH_PATTERN",
            ctx,
            Some(&pair[0]),
          ));
        };
        if arms.iter().any(|(seen, _, _)| *seen == member_index) {
          return Err(union_error(
            format!("duplicate branch for member `{type_name}` of union {union_name}"),
            "E_UNION_MATCH_PATTERN",
            ctx,
            Some(&pair[0]),
          ));
        }
        if pattern.len() > 2 {
          return Err(union_error(
            format!("type pattern `{}` binds at most one name", pair[0]),
            "E_UNION_MATCH_PATTERN",
            ctx,
            Some(&pair[0]),
          ));
        }
        let binding = match pattern.get(1) {
          None => None,
          Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "_" => None,
          Some(Calcit::Symbol { sym, info, location }) => {
            Some(typed_local(sym, info, location, members[member_index].annotation.to_owned()))
          }
          Some(other) => {
            return Err(union_error(
              format!("type pattern binding must be a symbol, got: {other}"),
              "E_UNION_MATCH_PATTERN",
              ctx,
              Some(other),
            ));
          }
        };
        arms.push((member_index, binding, &pair[1]));
      }
      other => {
        return Err(union_error(
          format!("match on union {union_name} takes type patterns such as `(String s)`, got: {other}"),
          "E_UNION_MATCH_PATTERN",
          ctx,
          Some(other),
        ));
      }
    }
  }

  let covers_all = arms.len() == members.len();
  if covers_all && wildcard.is_some() {
    gen_check_warning(
      format!("[Warn] match on union {union_name} names every member, so the `_` branch is unreachable, at {file_ns}/{def_name}"),
      file_ns,
      ctx.check_warnings,
    );
  }

  let value_sym_symbol = generated_path_symbol("union_v", file_ns, ctx.call_stack)?;
  let Calcit::Symbol { sym, info, location } = &value_sym_symbol else {
    unreachable!("gensym returns a symbol");
  };
  // The tested value is held as Dynamic so the generated kind tests stay
  // ordinary predicate calls; each branch rebinds it with the member type.
  let value_local = typed_local(sym, info, location, calcit::DYNAMIC_TYPE.clone());

  let mut chain = match wildcard {
    Some(body) => body.to_owned(),
    None if covers_all => generated_call(vec![
      Calcit::Proc(CalcitProc::Raise),
      Calcit::Str(Arc::from(format!("match: value is not a member of union {union_name}"))),
    ]),
    None => {
      let missing: Vec<&str> = members
        .iter()
        .enumerate()
        .filter(|(index, _)| !arms.iter().any(|(seen, _, _)| seen == index))
        .map(|(_, member)| member.pattern_name.as_ref())
        .collect();
      return Err(union_error(
        format!(
          "match on union {union_name} is not exhaustive; add branches for [{}] or a `_` branch",
          missing.join(", ")
        ),
        "E_UNION_MATCH_NOT_EXHAUSTIVE",
        ctx,
        args.first(),
      ));
    }
  };
  for (member_index, binding, body) in arms.iter().rev() {
    let body = match binding {
      Some(local) => generated_let(local.to_owned(), value_local.to_owned(), (*body).to_owned(), file_ns),
      None => (*body).to_owned(),
    };
    let test = member_test(&members[*member_index], &value_local, file_ns, &def_name);
    chain = generated_if(test, body, chain, file_ns);
  }

  let lowered = generated_let(value_local, value_form.to_owned(), chain, file_ns);
  preprocess_expr(
    &lowered,
    ctx.scope_defs,
    ctx.scope_types,
    file_ns,
    ctx.check_warnings,
    ctx.call_stack,
  )
}

fn named_head(head: &Calcit) -> Option<&str> {
  match head {
    Calcit::Proc(proc) => Some(proc.as_ref()),
    Calcit::Import(CalcitImport { ns, def, .. }) if ns.as_ref() == calcit::CORE_NS => Some(def.as_ref()),
    Calcit::Fn { info, .. } if info.def_ns.as_ref() == calcit::CORE_NS => Some(info.name.as_ref()),
    _ => None,
  }
}

fn is_type_predicate(name: &str) -> bool {
  matches!(
    name,
    "nil?"
      | "some?"
      | "bool?"
      | "number?"
      | "string?"
      | "tag?"
      | "symbol?"
      | "list?"
      | "map?"
      | "set?"
      | "fn?"
      | "enum?"
      | "struct?"
      | "ref?"
      | "buffer?"
      | "cirru-quote?"
      | "record?"
      | "tuple?"
  )
}

fn is_literal(node: &Calcit) -> bool {
  matches!(
    node,
    Calcit::Nil | Calcit::Bool(_) | Calcit::Number(_) | Calcit::Str(_) | Calcit::Tag(_)
  )
}

/// Reject the uses a union value does not support: type predicates, equality
/// against literals and implicit stringification. None of these narrow a
/// union, so each points to `match` instead.
pub(super) fn reject_union_value_use(
  head: &Calcit,
  args: &CalcitList,
  scope_types: &super::ScopeTypes,
  call_stack: &crate::call_stack::CallStackList,
) -> Result<(), CalcitErr> {
  let Some(name) = named_head(head) else {
    return Ok(());
  };
  let union_arg = || {
    args.iter().find_map(|arg| {
      let annotation = super::resolve_type_value(arg, scope_types)?;
      let union_def = annotation.resolve_to_union()?;
      Some((arg, union_def))
    })
  };
  let error = |arg: &Calcit, message: String, code: &str| {
    Err(CalcitErr::use_msg_stack_location_with_code(
      CalcitErrKind::Type,
      message,
      code,
      call_stack,
      arg.get_location().or_else(|| head.get_location()),
    ))
  };
  if is_type_predicate(name) {
    if let Some((arg, union_def)) = union_arg() {
      return error(
        arg,
        format!(
          "`{name}` on a value of union {} does not narrow it; take the union apart with `match` type patterns such as `(String s)`",
          union_def.name
        ),
        "E_UNION_PREDICATE",
      );
    }
  } else if matches!(name, "=" | "&=" | "not=" | "identical?") {
    if args.iter().any(is_literal)
      && let Some((arg, union_def)) = union_arg()
    {
      return error(
        arg,
        format!(
          "`{name}` compares a value of union {} with a literal; take the union apart with `match` first and compare the member value",
          union_def.name
        ),
        "E_UNION_LITERAL_EQ",
      );
    }
  } else if matches!(name, "str" | "&turn-string" | "turn-string")
    && let Some((arg, union_def)) = union_arg()
  {
    return error(
      arg,
      format!(
        "`{name}` does not accept a value of union {}; take it apart with `match` and convert the member value",
        union_def.name
      ),
      "E_UNION_REQUIRES_MATCH",
    );
  }
  Ok(())
}

/// A union has no methods or fields of its own; only its members do.
pub(super) fn reject_union_method_receiver(
  method_name: &str,
  receiver: &Calcit,
  scope_types: &super::ScopeTypes,
  call_stack: &crate::call_stack::CallStackList,
) -> Result<(), CalcitErr> {
  let Some(union_def) = super::resolve_type_value(receiver, scope_types).and_then(|annotation| annotation.resolve_to_union()) else {
    return Ok(());
  };
  Err(CalcitErr::use_msg_stack_location_with_code(
    CalcitErrKind::Type,
    format!(
      "`.{method_name}` is called on a value of union {}, which has no methods or fields; take it apart with `match` type patterns first",
      union_def.name
    ),
    "E_UNION_REQUIRES_MATCH",
    call_stack,
    receiver.get_location(),
  ))
}

fn find_inline_union(annotation: &CalcitTypeAnnotation) -> bool {
  use CalcitTypeAnnotation as T;
  match annotation {
    T::TypeRef(name, args) => {
      let name = name.trim_start_matches('\'');
      matches!(name, "Union" | "calcit.core/Union") || args.iter().any(|arg| find_inline_union(arg))
    }
    T::List(inner) | T::Set(inner) | T::Ref(inner) | T::Nullable(inner, _) | T::Variadic(inner) => find_inline_union(inner),
    T::Map(key, value) => find_inline_union(key) || find_inline_union(value),
    T::Struct(_, args) | T::Enum(_, args) | T::Tuple(args) => args.iter().any(|arg| find_inline_union(arg)),
    T::Fn(signature) => {
      signature.arg_types.iter().any(|arg| find_inline_union(arg))
        || signature.rest_type.as_ref().is_some_and(|rest| find_inline_union(rest))
        || find_inline_union(&signature.return_type)
    }
    _ => false,
  }
}

/// Unions are only written as named `defunion` definitions.
pub(super) fn reject_inline_union_schema(
  ns: &str,
  def_name: &str,
  schema: &CalcitTypeAnnotation,
  call_stack: &crate::call_stack::CallStackList,
  location: Option<calcit::NodeLocation>,
) -> Result<(), CalcitErr> {
  if !find_inline_union(schema) {
    return Ok(());
  }
  Err(CalcitErr::use_msg_stack_location_with_code(
    CalcitErrKind::Type,
    format!(
      "{ns}/{def_name} writes an inline `Union` in its schema; declare the union with `defunion Name MemberA MemberB` and refer to it by name"
    ),
    "E_UNION_INLINE",
    call_stack,
    location,
  ))
}
