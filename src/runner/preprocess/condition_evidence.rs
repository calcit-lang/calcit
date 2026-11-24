//! Definite predicate evidence in expanded, value-returning conditions.
//! Analyze existing core control flow; do not rewrite/evaluate source or trust
//! macro names, user Bool contracts, or only one arm of a disjunction.

use super::*;

pub(super) struct ConditionEvidence {
  pub when_true: ScopeTypes,
  pub when_false: ScopeTypes,
  pub true_reachable: bool,
  pub false_reachable: bool,
}

#[derive(Clone)]
struct Path {
  facts: ScopeTypes,
  preserves_locals: bool,
}

impl Path {
  fn empty(preserves_locals: bool) -> Self {
    Self {
      facts: ScopeTypes::new(),
      preserves_locals,
    }
  }
}

#[derive(Clone, Default)]
struct Outcomes {
  // None is an unreachable outcome; an empty fact map is reachable without proof.
  truthy: Option<Path>,
  falsey: Option<Path>,
}

impl Outcomes {
  fn unknown(preserves_locals: bool) -> Self {
    Self {
      truthy: Some(Path::empty(preserves_locals)),
      falsey: Some(Path::empty(preserves_locals)),
    }
  }

  fn forget(&mut self, name: &str) {
    for path in [&mut self.truthy, &mut self.falsey].into_iter().flatten() {
      path.facts.remove(name);
    }
  }

  fn preserves_locals(&self) -> bool {
    self.truthy.iter().chain(&self.falsey).all(|path| path.preserves_locals)
  }

  fn mark_local_effect(&mut self) {
    for path in [&mut self.truthy, &mut self.falsey].into_iter().flatten() {
      path.preserves_locals = false;
    }
  }
}

fn common(a: Option<Path>, b: Option<Path>) -> Option<Path> {
  match (a, b) {
    (None, other) | (other, None) => other,
    (Some(mut a), Some(b)) => {
      a.facts.retain(|name, annotation| b.facts.get(name) == Some(annotation));
      a.preserves_locals &= b.preserves_locals;
      Some(a)
    }
  }
}

fn following(prefix: &Path, suffix: Option<Path>) -> Option<Path> {
  suffix.map(|mut suffix| {
    if suffix.preserves_locals {
      let mut facts = prefix.facts.clone();
      facts.extend(suffix.facts);
      suffix.facts = facts;
    }
    suffix.preserves_locals &= prefix.preserves_locals;
    suffix
  })
}

pub(super) fn infer(expr: &Calcit, scope: &ScopeTypes) -> ConditionEvidence {
  let outcomes = visit::<false, false>(expr, scope, &HashMap::new(), &mut 4096, 0);
  ConditionEvidence {
    true_reachable: outcomes.truthy.is_some(),
    false_reachable: outcomes.falsey.is_some(),
    when_true: outcomes.truthy.map(|path| path.facts).unwrap_or_default(),
    when_false: outcomes.falsey.map(|path| path.facts).unwrap_or_default(),
  }
}

/// Only evidence about the incoming parameter establishes a declared guard.
pub(super) fn proves_guard<'a>(
  body: impl Iterator<Item = &'a Calcit>,
  parameter: &calcit::CalcitLocal,
  target: &CalcitTypeAnnotation,
) -> bool {
  let mut scope = ScopeTypes::new();
  scope.insert(parameter.sym.clone(), parameter.type_info.clone());
  let outcomes = visit_sequence::<false, true>(body, &scope, &HashMap::new(), &mut 4096, 0);
  outcomes
    .truthy
    .is_none_or(|path| path.preserves_locals && path.facts.get(&parameter.sym).unwrap_or(&parameter.type_info).is_proven_for(target))
}

/// A sequence continues only through normal exits, regardless of their truthiness.
pub(super) fn establish_normal_exit(expr: &Calcit, scope: &mut ScopeTypes) {
  let outcomes = visit::<false, false>(expr, scope, &HashMap::new(), &mut 4096, 0);
  if let Some(path) = common(outcomes.truthy, outcomes.falsey) {
    for (name, annotation) in path.facts {
      scope.refine(name, annotation);
    }
  }
}

pub(super) fn preserves_locals(expr: &Calcit, scope: &ScopeTypes) -> bool {
  visit::<true, false>(expr, scope, &HashMap::new(), &mut 4096, 0).preserves_locals()
}

fn visit_sequence<'a, const EFFECTS_ONLY: bool, const PORTABLE: bool>(
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
    body_scope.extend(prefix.facts.clone());
    let current = visit::<EFFECTS_ONLY, PORTABLE>(form, &body_scope, &body_aliases, remaining, depth + 1);
    if !current.preserves_locals() {
      body_aliases.clear();
    }
    result = Outcomes {
      truthy: following(&prefix, current.truthy),
      falsey: following(&prefix, current.falsey),
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
        && visit_sequence::<true, false>(info.body.iter(), &scope, &aliases, remaining, depth).preserves_locals()
    }
    Calcit::List(items) if matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::Defn, _))) => {
      visit_sequence::<true, false>(items.iter().skip(3), &scope, &aliases, remaining, depth).preserves_locals()
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
      .is_some_and(|value| visit::<true, false>(value, scope, aliases, remaining, depth + 1).preserves_locals());
  }
  items
    .iter()
    .all(|item| quoted_preserves_locals(item, scope, aliases, remaining, depth + 1))
}

fn visit<const EFFECTS_ONLY: bool, const PORTABLE: bool>(
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
    return Outcomes::default();
  }
  match expr {
    Calcit::Nil | Calcit::Unit | Calcit::Bool(false) => {
      return Outcomes {
        truthy: None,
        falsey: Some(Path::empty(true)),
      };
    }
    Calcit::Bool(true) => {
      return Outcomes {
        truthy: Some(Path::empty(true)),
        falsey: None,
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
        visit::<true, false>(
          &compiled.preprocessed_code,
          &ScopeTypes::new(),
          &HashMap::new(),
          remaining,
          depth + 1,
        )
        .preserves_locals()
      } else {
        false
      });
    }
    _ => {}
  }

  // Open WASM scalars can share their numeric representation. Equality
  // with a literal cannot establish portable kind evidence for callers.
  let direct = extract_predicate_bindings_with_literal_equality(expr, scope, !PORTABLE);
  if !EFFECTS_ONLY {
    if let Some(static_truth) = static_type_predicate_truth(expr, scope) {
      return if static_truth {
        Outcomes {
          truthy: Some(Path {
            facts: direct.true_binding.into_iter().collect(),
            preserves_locals: true,
          }),
          falsey: None,
        }
      } else {
        Outcomes {
          truthy: None,
          falsey: Some(Path {
            facts: direct.false_binding.into_iter().collect(),
            preserves_locals: true,
          }),
        }
      };
    }
    if direct.true_binding.is_some() || direct.false_binding.is_some() {
      return Outcomes {
        truthy: Some(Path {
          facts: direct.true_binding.into_iter().collect(),
          preserves_locals: true,
        }),
        falsey: Some(Path {
          facts: direct.false_binding.into_iter().collect(),
          preserves_locals: true,
        }),
      };
    }
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
    Some(Calcit::Import(import))
      if !EFFECTS_ONLY
        && let Some(Calcit::Local(local)) = items.get(1)
        && let Some(target) = program::lookup_proven_guard(&import.ns, &import.def)
        && items
          .iter()
          .skip(2)
          .all(|item| visit::<true, false>(item, scope, aliases, remaining, depth + 1).preserves_locals()) =>
    {
      let current = scope.get(&local.sym).unwrap_or(&local.type_info);
      let narrowed = if current.is_proven_for(&target) { current.clone() } else { target };
      Outcomes {
        truthy: Some(Path {
          facts: [(local.sym.clone(), narrowed)].into_iter().collect(),
          preserves_locals: true,
        }),
        // The proof says nothing about writes on a false return.
        falsey: Some(Path::empty(false)),
      }
    }
    Some(Calcit::Syntax(CalcitSyntax::HintFn | CalcitSyntax::Quote, _)) => Outcomes::unknown(true),
    Some(Calcit::Syntax(CalcitSyntax::UnsafeCoerce, _)) => items
      .get(1)
      .map(|value| visit::<EFFECTS_ONLY, PORTABLE>(value, scope, aliases, remaining, depth + 1))
      .unwrap_or_else(|| Outcomes::unknown(false)),
    Some(Calcit::Syntax(CalcitSyntax::Quasiquote, _)) => Outcomes::unknown(
      items
        .iter()
        .skip(1)
        .all(|item| quoted_preserves_locals(item, scope, aliases, remaining, depth + 1)),
    ),
    Some(Calcit::Syntax(CalcitSyntax::DecodeMapAs | CalcitSyntax::EncodeMapAs, _)) => Outcomes::unknown(
      items
        .get(1)
        .is_some_and(|input| visit::<EFFECTS_ONLY, PORTABLE>(input, scope, aliases, remaining, depth + 1).preserves_locals()),
    ),
    Some(Calcit::Syntax(CalcitSyntax::Defref, _)) => Outcomes::unknown(
      items
        .get(2)
        .is_some_and(|initial| visit::<EFFECTS_ONLY, PORTABLE>(initial, scope, aliases, remaining, depth + 1).preserves_locals()),
    ),
    Some(Calcit::Syntax(CalcitSyntax::TryDecodeMapAs, _)) => Outcomes::unknown(
      // The decoder builds both Result outcomes directly; only its input can run code.
      items
        .get(1)
        .is_some_and(|input| visit::<EFFECTS_ONLY, PORTABLE>(input, scope, aliases, remaining, depth + 1).preserves_locals()),
    ),
    Some(Calcit::Syntax(CalcitSyntax::Match, _)) => {
      let Some(subject) = items.get(1) else {
        return Outcomes::unknown(false);
      };
      if !visit::<EFFECTS_ONLY, PORTABLE>(subject, scope, aliases, remaining, depth + 1).preserves_locals() {
        return Outcomes::unknown(false);
      }
      let Some(branches) = type_inference::preprocessed_match_branches(items) else {
        return Outcomes::unknown(false);
      };
      if branches.is_empty() {
        return Outcomes::unknown(false);
      }
      let mut result = Outcomes::default();
      for (pattern, body) in branches {
        let mut bindings = ScopeTypes::new();
        type_inference::bind_pattern_scope(pattern, &mut bindings);
        let mut branch_scope = scope.clone();
        let mut branch_aliases = aliases.clone();
        for (name, annotation) in &bindings {
          branch_scope.insert(name.clone(), annotation.clone());
          branch_aliases.remove(name);
          for evidence in branch_aliases.values_mut() {
            evidence.forget(name);
          }
          if EFFECTS_ONLY {
            branch_aliases.insert(name.clone(), Outcomes::unknown(true));
          }
        }
        let mut branch = visit::<EFFECTS_ONLY, PORTABLE>(body, &branch_scope, &branch_aliases, remaining, depth + 1);
        // Pattern-local evidence cannot justify narrowing an outer binding.
        for (name, _) in bindings {
          branch.forget(&name);
        }
        result.truthy = common(result.truthy, branch.truthy);
        result.falsey = common(result.falsey, branch.falsey);
      }
      result
    }
    Some(Calcit::Proc(CalcitProc::Not)) if items.len() == 2 => {
      let inner = visit::<EFFECTS_ONLY, PORTABLE>(items.get(1).unwrap(), scope, aliases, remaining, depth + 1);
      Outcomes {
        truthy: inner.falsey,
        falsey: inner.truthy,
      }
    }
    Some(Calcit::Import(import))
      if import.ns.as_ref() == calcit::CORE_NS
        && import.def.as_ref() == "not"
        && program::has_bundled_core_source()
        && items.len() == 2 =>
    {
      let inner = visit::<EFFECTS_ONLY, PORTABLE>(items.get(1).unwrap(), scope, aliases, remaining, depth + 1);
      Outcomes {
        truthy: inner.falsey,
        falsey: inner.truthy,
      }
    }
    Some(Calcit::Syntax(CalcitSyntax::If, _)) if matches!(items.len(), 3 | 4) => {
      let condition = visit::<EFFECTS_ONLY, PORTABLE>(items.get(1).unwrap(), scope, aliases, remaining, depth + 1);
      let mut result = Outcomes::default();
      for (path, branch) in [
        (condition.truthy, items.get(2).unwrap()),
        (condition.falsey, items.get(3).unwrap_or(&Calcit::Unit)),
      ] {
        if let Some(path) = path {
          let mut branch_scope = scope.clone();
          branch_scope.extend(path.facts.clone());
          let empty_aliases = HashMap::new();
          let branch_aliases = if path.preserves_locals { aliases } else { &empty_aliases };
          let branch = visit::<EFFECTS_ONLY, PORTABLE>(branch, &branch_scope, branch_aliases, remaining, depth + 1);
          result.truthy = common(result.truthy, following(&path, branch.truthy));
          result.falsey = common(result.falsey, following(&path, branch.falsey));
        }
      }
      result
    }
    Some(Calcit::Syntax(CalcitSyntax::CoreLet, _)) if items.len() >= 3 => {
      let Some(Calcit::List(pair)) = items.get(1) else {
        return Outcomes::unknown(false);
      };
      if pair.is_empty() {
        return visit_sequence::<EFFECTS_ONLY, PORTABLE>(items.iter().skip(2), scope, aliases, remaining, depth);
      }
      let (Some(Calcit::Local(local)), Some(value)) = (pair.first(), pair.get(1)) else {
        return Outcomes::unknown(false);
      };
      let mut bound = visit::<EFFECTS_ONLY, PORTABLE>(value, scope, aliases, remaining, depth + 1);
      let mut body_aliases = aliases.clone();
      if !bound.preserves_locals() {
        body_aliases.clear();
      }
      // Same-spelling locals have different lexical owners. Never let an
      // outer proof alias narrow a shadowing local, or escape its scope.
      for evidence in body_aliases.values_mut() {
        evidence.forget(&local.sym);
      }
      bound.forget(&local.sym);
      let preserves_initializer = bound.preserves_locals();
      for path in [&mut bound.truthy, &mut bound.falsey].into_iter().flatten() {
        path.preserves_locals = true;
      }
      body_aliases.insert(local.sym.clone(), bound);
      let mut body_scope = scope.clone();
      body_scope.insert(local.sym.clone(), local.type_info.clone());
      let mut body = visit_sequence::<EFFECTS_ONLY, PORTABLE>(items.iter().skip(2), &body_scope, &body_aliases, remaining, depth);
      body.forget(&local.sym);
      if !preserves_initializer {
        body.mark_local_effect();
      }
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
          .all(|item| visit::<true, false>(item, scope, aliases, remaining, depth + 1).preserves_locals()),
      )
    }
    // These value operations never invoke user callbacks. Merely declaring
    // a Bool/Number result is not an effect or predicate proof for other calls.
    Some(Calcit::Proc(proc)) if non_invoking_value_proc(*proc) => Outcomes::unknown(
      items
        .iter()
        .skip(1)
        .all(|item| visit::<EFFECTS_ONLY, PORTABLE>(item, scope, aliases, remaining, depth + 1).preserves_locals()),
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
          .all(|item| visit::<EFFECTS_ONLY, PORTABLE>(item, scope, aliases, remaining, depth + 1).preserves_locals()),
      )
    }
    Some(head) => Outcomes::unknown(
      items
        .iter()
        .skip(1)
        .all(|item| visit::<EFFECTS_ONLY, PORTABLE>(item, scope, aliases, remaining, depth + 1).preserves_locals())
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
