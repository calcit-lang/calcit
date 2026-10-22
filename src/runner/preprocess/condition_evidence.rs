//! Definite predicate evidence in expanded, value-returning conditions.
//! Analyze existing core control flow; do not rewrite/evaluate source or trust
//! macro names, user Bool contracts, or only one arm of a disjunction.

use super::*;

pub(super) struct ConditionEvidence {
  pub when_true: ScopeTypes,
  pub when_false: ScopeTypes,
}

#[derive(Clone)]
struct Outcomes {
  // None is an unreachable outcome; an empty map is reachable without proof.
  truthy: Option<ScopeTypes>,
  falsey: Option<ScopeTypes>,
  preserves_locals: bool,
}

impl Outcomes {
  fn unknown(preserves_locals: bool) -> Self {
    Self {
      truthy: Some(ScopeTypes::new()),
      falsey: Some(ScopeTypes::new()),
      preserves_locals,
    }
  }

  fn forget(&mut self, name: &str) {
    for facts in [&mut self.truthy, &mut self.falsey].into_iter().flatten() {
      facts.remove(name);
    }
  }
}

fn common(a: Option<ScopeTypes>, b: Option<ScopeTypes>) -> Option<ScopeTypes> {
  match (a, b) {
    (None, other) | (other, None) => other,
    (Some(mut a), Some(b)) => {
      a.retain(|name, annotation| b.get(name) == Some(annotation));
      Some(a)
    }
  }
}

fn following(prefix: &ScopeTypes, suffix: Option<ScopeTypes>, preserves_locals: bool) -> Option<ScopeTypes> {
  suffix.map(|suffix| {
    let mut facts = if preserves_locals { prefix.clone() } else { ScopeTypes::new() };
    facts.extend(suffix);
    facts
  })
}

pub(super) fn infer(expr: &Calcit, scope: &ScopeTypes) -> ConditionEvidence {
  let outcomes = visit::<false>(expr, scope, &HashMap::new(), &mut 4096, 0);
  ConditionEvidence {
    when_true: outcomes.truthy.unwrap_or_default(),
    when_false: outcomes.falsey.unwrap_or_default(),
  }
}

/// A sequence continues only through normal exits, regardless of their truthiness.
pub(super) fn establish_normal_exit(expr: &Calcit, scope: &mut ScopeTypes) {
  let outcomes = visit::<false>(expr, scope, &HashMap::new(), &mut 4096, 0);
  if let Some(facts) = common(outcomes.truthy, outcomes.falsey) {
    for (name, annotation) in facts {
      scope.refine(name, annotation);
    }
  }
}

pub(super) fn preserves_locals(expr: &Calcit, scope: &ScopeTypes) -> bool {
  visit::<true>(expr, scope, &HashMap::new(), &mut 4096, 0).preserves_locals
}

fn visit_sequence<'a, const EFFECTS_ONLY: bool>(
  forms: impl Iterator<Item = &'a Calcit>,
  scope: &ScopeTypes,
  aliases: &HashMap<Arc<str>, Outcomes>,
  remaining: &mut usize,
  depth: usize,
) -> Outcomes {
  let mut result = Outcomes::unknown(true);
  let mut body_aliases = aliases.clone();
  for form in forms {
    let Some(prefix) = common(result.truthy.take(), result.falsey.take()) else {
      return result;
    };
    let mut body_scope = scope.clone();
    body_scope.extend(prefix.clone());
    let current = visit::<EFFECTS_ONLY>(form, &body_scope, &body_aliases, remaining, depth + 1);
    if !current.preserves_locals {
      body_aliases.clear();
    }
    result = Outcomes {
      truthy: following(&prefix, current.truthy, current.preserves_locals),
      falsey: following(&prefix, current.falsey, current.preserves_locals),
      preserves_locals: result.preserves_locals && current.preserves_locals,
    };
  }
  result
}

fn has_js_implementation(ns: &str, name: &str) -> bool {
  program::lookup_def_ffi(ns, name).is_some_and(|ffi| crate::snapshot::ffi_metadata_value(&ffi, "js").is_some())
}

fn known_import_call_preserves_locals(ns: &str, name: &str, remaining: &mut usize, depth: usize) -> bool {
  // A registered backend implementation can replace the visible Calcit body.
  if has_js_implementation(ns, name) {
    return false;
  }
  if let Some(compiled) = program::lookup_compiled_def(ns, name) {
    known_call_preserves_locals(&compiled.preprocessed_code, remaining, depth + 1)
  } else {
    program::lookup_runtime_ready(ns, name).is_some_and(|value| known_call_preserves_locals(&value, remaining, depth + 1))
  }
}

fn known_call_preserves_locals(head: &Calcit, remaining: &mut usize, depth: usize) -> bool {
  if *remaining == 0 || depth > 64 {
    return false;
  }
  *remaining -= 1;
  // Analyze actual code, never a Bool/result contract or the caller's aliases.
  let scope = ScopeTypes::new();
  let aliases = HashMap::new();
  match head {
    Calcit::Fn { info, .. } => {
      !has_js_implementation(&info.def_ns, &info.name)
        && info
          .def_ref
          .as_ref()
          .is_none_or(|owner| !has_js_implementation(&owner.def_ns, &owner.def_name))
        && visit_sequence::<true>(info.body.iter(), &scope, &aliases, remaining, depth).preserves_locals
    }
    Calcit::List(items) if matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::Defn, _))) => {
      visit_sequence::<true>(items.iter().skip(3), &scope, &aliases, remaining, depth).preserves_locals
    }
    Calcit::EnumDef(_) | Calcit::Struct(_) => true,
    Calcit::Proc(proc) => non_invoking_value_proc(*proc),
    Calcit::Import(import) => known_import_call_preserves_locals(&import.ns, &import.def, remaining, depth),
    _ => false,
  }
}

fn quoted_preserves_locals(
  expr: &Calcit,
  scope: &ScopeTypes,
  aliases: &HashMap<Arc<str>, Outcomes>,
  remaining: &mut usize,
  depth: usize,
) -> bool {
  if *remaining == 0 || depth > 64 {
    return false;
  }
  *remaining -= 1;
  let Calcit::List(items) = expr else { return true };
  if matches!(
    items.first(),
    Some(Calcit::Syntax(
      CalcitSyntax::MacroInterpolate | CalcitSyntax::MacroInterpolateSpread,
      _
    ))
  ) {
    return items
      .get(1)
      .is_some_and(|value| visit::<true>(value, scope, aliases, remaining, depth + 1).preserves_locals);
  }
  items
    .iter()
    .all(|item| quoted_preserves_locals(item, scope, aliases, remaining, depth + 1))
}

fn visit<const EFFECTS_ONLY: bool>(
  expr: &Calcit,
  scope: &ScopeTypes,
  aliases: &HashMap<Arc<str>, Outcomes>,
  remaining: &mut usize,
  depth: usize,
) -> Outcomes {
  if *remaining == 0 || depth > 64 {
    return Outcomes::unknown(false);
  }
  *remaining -= 1;
  if is_definition_syntax(expr) {
    return Outcomes::unknown(true);
  }
  if expression_definitely_diverges(expr) {
    return Outcomes {
      truthy: None,
      falsey: None,
      preserves_locals: true,
    };
  }
  match expr {
    Calcit::Nil | Calcit::Unit | Calcit::Bool(false) => {
      return Outcomes {
        truthy: None,
        falsey: Some(ScopeTypes::new()),
        preserves_locals: true,
      };
    }
    Calcit::Bool(true) => {
      return Outcomes {
        truthy: Some(ScopeTypes::new()),
        falsey: None,
        preserves_locals: true,
      };
    }
    Calcit::Local(local) => {
      // Effect inspection may ignore writes to bindings owned by this
      // expression. Their old predicate aliases must not prune later branches.
      return if EFFECTS_ONLY {
        Outcomes::unknown(true)
      } else {
        aliases.get(&local.sym).cloned().unwrap_or_else(|| Outcomes::unknown(true))
      };
    }
    Calcit::Import(import) => {
      if program::lookup_runtime_ready(&import.ns, &import.def).is_some() {
        return Outcomes::unknown(true);
      }
      // A value read can force a lazy initializer. Prove its actual code in
      // its own lexical scope, including reads nested in constructor inputs.
      return Outcomes::unknown(if let Some(compiled) = program::lookup_compiled_def(&import.ns, &import.def) {
        visit::<true>(
          &compiled.preprocessed_code,
          &ScopeTypes::new(),
          &HashMap::new(),
          remaining,
          depth + 1,
        )
        .preserves_locals
      } else {
        false
      });
    }
    _ => {}
  }

  let direct = extract_predicate_bindings(expr, scope);
  if !EFFECTS_ONLY && (direct.true_binding.is_some() || direct.false_binding.is_some()) {
    return Outcomes {
      truthy: Some(direct.true_binding.into_iter().collect()),
      falsey: Some(direct.false_binding.into_iter().collect()),
      preserves_locals: true,
    };
  }

  let Calcit::List(items) = expr else {
    // Reading a global may force an initializer. Raw host expressions and
    // unresolved names also carry no proof that caller locals stay unchanged.
    return Outcomes::unknown(!matches!(
      expr,
      Calcit::Import(_) | Calcit::Symbol { .. } | Calcit::Thunk(_) | Calcit::RawCode(_, _)
    ));
  };
  match items.first() {
    Some(Calcit::Syntax(CalcitSyntax::HintFn | CalcitSyntax::Quote, _)) => Outcomes::unknown(true),
    Some(Calcit::Syntax(CalcitSyntax::UnsafeCoerce, _)) => items
      .get(1)
      .map(|value| visit::<EFFECTS_ONLY>(value, scope, aliases, remaining, depth + 1))
      .unwrap_or_else(|| Outcomes::unknown(false)),
    Some(Calcit::Syntax(CalcitSyntax::Quasiquote, _)) => Outcomes::unknown(
      items
        .iter()
        .skip(1)
        .all(|item| quoted_preserves_locals(item, scope, aliases, remaining, depth + 1)),
    ),
    Some(Calcit::Syntax(CalcitSyntax::DecodeMapAs, _)) => Outcomes::unknown(
      items
        .get(1)
        .is_some_and(|input| visit::<EFFECTS_ONLY>(input, scope, aliases, remaining, depth + 1).preserves_locals),
    ),
    Some(Calcit::Syntax(CalcitSyntax::Defref, _)) => Outcomes::unknown(
      items
        .get(2)
        .is_some_and(|initial| visit::<EFFECTS_ONLY>(initial, scope, aliases, remaining, depth + 1).preserves_locals),
    ),
    Some(Calcit::Syntax(CalcitSyntax::TryDecodeMapAs, _)) => Outcomes::unknown(
      items
        .get(1)
        .is_some_and(|input| visit::<EFFECTS_ONLY>(input, scope, aliases, remaining, depth + 1).preserves_locals)
        // The native decoder wraps both outcomes through these actual core
        // constructors. Do not infer effects from their Result signatures.
        && ["%ok", "%err"]
          .iter()
          .all(|name| known_import_call_preserves_locals(calcit::CORE_NS, name, remaining, depth + 1)),
    ),
    Some(Calcit::Syntax(CalcitSyntax::Match, _)) => {
      let Some(subject) = items.get(1) else {
        return Outcomes::unknown(false);
      };
      if !visit::<EFFECTS_ONLY>(subject, scope, aliases, remaining, depth + 1).preserves_locals {
        return Outcomes::unknown(false);
      }
      let Some(branches) = type_inference::preprocessed_match_branches(items) else {
        return Outcomes::unknown(false);
      };
      Outcomes::unknown(branches.into_iter().all(|(pattern, body)| {
        let mut bindings = ScopeTypes::new();
        type_inference::bind_pattern_scope(pattern, &mut bindings);
        let mut branch_scope = scope.clone();
        let mut branch_aliases = aliases.clone();
        for (name, annotation) in bindings {
          branch_scope.insert(name.clone(), annotation);
          branch_aliases.remove(&name);
          for evidence in branch_aliases.values_mut() {
            evidence.forget(&name);
          }
          if EFFECTS_ONLY {
            branch_aliases.insert(name, Outcomes::unknown(true));
          }
        }
        visit::<EFFECTS_ONLY>(body, &branch_scope, &branch_aliases, remaining, depth + 1).preserves_locals
      }))
    }
    Some(Calcit::Proc(CalcitProc::Not)) if items.len() == 2 => {
      let inner = visit::<EFFECTS_ONLY>(items.get(1).unwrap(), scope, aliases, remaining, depth + 1);
      Outcomes {
        truthy: inner.falsey,
        falsey: inner.truthy,
        preserves_locals: inner.preserves_locals,
      }
    }
    Some(Calcit::Syntax(CalcitSyntax::If, _)) if matches!(items.len(), 3 | 4) => {
      let condition = visit::<EFFECTS_ONLY>(items.get(1).unwrap(), scope, aliases, remaining, depth + 1);
      let mut result = Outcomes {
        truthy: None,
        falsey: None,
        preserves_locals: condition.preserves_locals,
      };
      for (path, branch) in [
        (condition.truthy, items.get(2).unwrap()),
        (condition.falsey, items.get(3).unwrap_or(&Calcit::Nil)),
      ] {
        if let Some(path) = path {
          let mut branch_scope = scope.clone();
          branch_scope.extend(path.clone());
          let empty_aliases = HashMap::new();
          let branch_aliases = if condition.preserves_locals { aliases } else { &empty_aliases };
          let branch = visit::<EFFECTS_ONLY>(branch, &branch_scope, branch_aliases, remaining, depth + 1);
          result.truthy = common(result.truthy, following(&path, branch.truthy, branch.preserves_locals));
          result.falsey = common(result.falsey, following(&path, branch.falsey, branch.preserves_locals));
          result.preserves_locals &= branch.preserves_locals;
        }
      }
      result
    }
    Some(Calcit::Syntax(CalcitSyntax::CoreLet, _)) if items.len() >= 3 => {
      let Some(Calcit::List(pair)) = items.get(1) else {
        return Outcomes::unknown(false);
      };
      if pair.is_empty() {
        return visit_sequence::<EFFECTS_ONLY>(items.iter().skip(2), scope, aliases, remaining, depth);
      }
      let (Some(Calcit::Local(local)), Some(value)) = (pair.first(), pair.get(1)) else {
        return Outcomes::unknown(false);
      };
      let mut bound = visit::<EFFECTS_ONLY>(value, scope, aliases, remaining, depth + 1);
      let mut body_aliases = aliases.clone();
      if !bound.preserves_locals {
        body_aliases.clear();
      }
      // Same-spelling locals have different lexical owners. Never let an
      // outer proof alias narrow a shadowing local, or escape its scope.
      for evidence in body_aliases.values_mut() {
        evidence.forget(&local.sym);
      }
      bound.forget(&local.sym);
      let preserves_initializer = bound.preserves_locals;
      bound.preserves_locals = true;
      body_aliases.insert(local.sym.clone(), bound);
      let mut body_scope = scope.clone();
      body_scope.insert(local.sym.clone(), local.type_info.clone());
      let mut body = visit_sequence::<EFFECTS_ONLY>(items.iter().skip(2), &body_scope, &body_aliases, remaining, depth);
      body.forget(&local.sym);
      body.preserves_locals &= preserves_initializer;
      body
    }
    Some(head)
      if EFFECTS_ONLY
        && is_local_assignment(head)
        && matches!(items.get(1), Some(Calcit::Local(local)) if aliases.contains_key(&local.sym)) =>
    {
      // A shadowing binding belongs to this expression, not its caller.
      // Still inspect the assigned value, which can invoke a captured writer.
      Outcomes::unknown(
        items
          .iter()
          .skip(2)
          .all(|item| visit::<true>(item, scope, aliases, remaining, depth + 1).preserves_locals),
      )
    }
    // These value operations never invoke user callbacks. Merely declaring
    // a Bool/Number result is not an effect or predicate proof for other calls.
    Some(Calcit::Proc(proc)) if non_invoking_value_proc(*proc) => Outcomes::unknown(
      items
        .iter()
        .skip(1)
        .all(|item| visit::<EFFECTS_ONLY>(item, scope, aliases, remaining, depth + 1).preserves_locals),
    ),
    Some(Calcit::Import(import))
      if import.ns.as_ref() == calcit::CORE_NS
        && program::has_bundled_core_source()
        && (matches!(import.def.as_ref(), "=" | "&>=" | "&<=" | ">=" | "<=" | ">" | "<")
          || (matches!(import.def.as_ref(), "count" | "empty?")
            && items
              .get(1)
              .is_some_and(|value| matches!(infer_type_from_expr(value, scope).as_deref(), Some(CalcitTypeAnnotation::List(_)))))) =>
    {
      // Bundled comparisons and the proven List branch do not invoke user
      // code. Custom Countable implementations remain an effect barrier.
      Outcomes::unknown(
        items
          .iter()
          .skip(1)
          .all(|item| visit::<EFFECTS_ONLY>(item, scope, aliases, remaining, depth + 1).preserves_locals),
      )
    }
    Some(head) => Outcomes::unknown(
      items
        .iter()
        .skip(1)
        .all(|item| visit::<EFFECTS_ONLY>(item, scope, aliases, remaining, depth + 1).preserves_locals)
        && known_call_preserves_locals(head, remaining, depth + 1),
    ),
    None => Outcomes::unknown(true),
  }
}

fn non_invoking_value_proc(proc: CalcitProc) -> bool {
  matches!(
    proc,
    CalcitProc::List
      | CalcitProc::Recur
      | CalcitProc::NativeMap
      | CalcitProc::Set
      | CalcitProc::Floor
      | CalcitProc::Ceil
      | CalcitProc::Round
      | CalcitProc::NativeAdd
      | CalcitProc::NativeMinus
      | CalcitProc::NativeMultiply
      | CalcitProc::NativeDivide
      | CalcitProc::NativeEquals
      | CalcitProc::Identical
      | CalcitProc::NativeLessThan
      | CalcitProc::NativeGreaterThan
      | CalcitProc::Not
      | CalcitProc::TypeOf
      | CalcitProc::NativeListCount
      | CalcitProc::NativeMapCount
      | CalcitProc::NativeSetCount
      | CalcitProc::NativeEnumCount
      | CalcitProc::NativeStructCount
      | CalcitProc::NativeStrCount
      | CalcitProc::NativeStrUtf8ByteCount
      | CalcitProc::NativeStrEmpty
      | CalcitProc::NativeStrContains
      | CalcitProc::NativeStrNth
      | CalcitProc::NativeStrFirst
      | CalcitProc::NativeStrRest
      | CalcitProc::NativeStrSlice
      | CalcitProc::NativeListEmpty
      | CalcitProc::NativeListContains
      | CalcitProc::NativeListNth
      | CalcitProc::NativeListFirst
      | CalcitProc::NativeListLast
      | CalcitProc::NativeListRest
      | CalcitProc::NativeListSlice
      | CalcitProc::NativeMapGet
      | CalcitProc::NativeEnumNth
      | CalcitProc::NativeEnumNew
      | CalcitProc::NativeStructNew
      | CalcitProc::NativeNamedEnumNew
      | CalcitProc::NativeEnum
      | CalcitProc::NativeStruct
      | CalcitProc::NativeStructDefinition
      | CalcitProc::NativeEnumDefinition
      | CalcitProc::NativeStructGet
      | CalcitProc::NativeStructContains
      | CalcitProc::NativeStructToMap
      | CalcitProc::TurnTag
      | CalcitProc::AtomDeref
      | CalcitProc::ListQuestion
      | CalcitProc::TagQuestion
      | CalcitProc::SymbolQuestion
      | CalcitProc::NilQuestion
      | CalcitProc::StringQuestion
      | CalcitProc::MapQuestion
      | CalcitProc::NumberQuestion
      | CalcitProc::BoolQuestion
      | CalcitProc::SetQuestion
      | CalcitProc::EnumQuestion
      | CalcitProc::StructQuestion
      | CalcitProc::FnQuestion
      | CalcitProc::NativeCompare
      | CalcitProc::NativeHash
      | CalcitProc::Sin
      | CalcitProc::Cos
      | CalcitProc::Pow
      | CalcitProc::Sqrt
      | CalcitProc::IsInteger
      | CalcitProc::NativeNumberFract
      | CalcitProc::NativeNumberRem
      | CalcitProc::NativeNumberFormat
      | CalcitProc::NativeNumberFits
      | CalcitProc::BitShl
      | CalcitProc::BitShr
      | CalcitProc::BitAnd
      | CalcitProc::BitOr
      | CalcitProc::BitXor
      | CalcitProc::BitNot
      | CalcitProc::NativeStrConcat
      | CalcitProc::Trim
      | CalcitProc::TurnString
      | CalcitProc::Split
      | CalcitProc::SplitLines
      | CalcitProc::StartsWith
      | CalcitProc::EndsWith
      | CalcitProc::GetCharCode
      | CalcitProc::CharFromCode
      | CalcitProc::ParseFloat
      | CalcitProc::IsBlank
      | CalcitProc::NativeStrCompare
      | CalcitProc::NativeStrReplace
      | CalcitProc::NativeStrFindIndex
      | CalcitProc::NativeStrEscape
      | CalcitProc::NativeStrIncludes
      | CalcitProc::NativeStrPadLeft
      | CalcitProc::NativeStrPadRight
      | CalcitProc::NativeListReverse
      | CalcitProc::NativeListConcat
      | CalcitProc::NativeListAssocBefore
      | CalcitProc::NativeListAssocAfter
      | CalcitProc::NativeListIncludes
      | CalcitProc::NativeListAssoc
      | CalcitProc::NativeListDissoc
      | CalcitProc::NativeListToSet
      | CalcitProc::NativeListDistinct
      | CalcitProc::NativeListAppend
      | CalcitProc::NativeListPrepend
      | CalcitProc::NativeListButlast
      | CalcitProc::NativeListRange
      | CalcitProc::Append
      | CalcitProc::Prepend
      | CalcitProc::Butlast
      | CalcitProc::Range
      | CalcitProc::NativeMerge
      | CalcitProc::NativeMergeNonNil
      | CalcitProc::ToPairs
      | CalcitProc::NativeMapDissoc
      | CalcitProc::NativeMapToList
      | CalcitProc::NativeMapEmpty
      | CalcitProc::NativeMapContains
      | CalcitProc::NativeMapIncludes
      | CalcitProc::NativeMapAssoc
      | CalcitProc::NativeMapDiffNew
      | CalcitProc::NativeMapDiffKeys
      | CalcitProc::NativeMapCommonKeys
      | CalcitProc::NativeMapKeys
      | CalcitProc::NativeMapVals
      | CalcitProc::NativeInclude
      | CalcitProc::NativeExclude
      | CalcitProc::NativeDifference
      | CalcitProc::NativeUnion
      | CalcitProc::NativeSetIntersection
      | CalcitProc::NativeSetToList
      | CalcitProc::NativeSetEmpty
      | CalcitProc::NativeSetIncludes
      | CalcitProc::NativeStructAssoc
      | CalcitProc::NativeStructNth
      | CalcitProc::NativeStructGetName
  )
}
