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
  let outcomes = visit(expr, scope, &HashMap::new(), &mut 4096, 0);
  ConditionEvidence {
    when_true: outcomes.truthy.unwrap_or_default(),
    when_false: outcomes.falsey.unwrap_or_default(),
  }
}

/// A sequence continues only through normal exits, regardless of their truthiness.
pub(super) fn establish_normal_exit(expr: &Calcit, scope: &mut ScopeTypes) {
  let outcomes = visit(expr, scope, &HashMap::new(), &mut 4096, 0);
  if let Some(facts) = common(outcomes.truthy, outcomes.falsey) {
    for (name, annotation) in facts {
      scope.refine(name, annotation);
    }
  }
}

pub(super) fn preserves_locals(expr: &Calcit, scope: &ScopeTypes) -> bool {
  if let Calcit::Import(import) = expr
    && let Some(compiled) = program::lookup_compiled_def(&import.ns, &import.def)
  {
    // Constructing a known function does not execute its body. Other lazy
    // initializers owe the same effect proof as an ordinary expression.
    return visit(&compiled.preprocessed_code, scope, &HashMap::new(), &mut 4096, 0).preserves_locals;
  }
  visit(expr, scope, &HashMap::new(), &mut 4096, 0).preserves_locals
}

fn visit_sequence<'a>(
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
    let current = visit(form, &body_scope, &body_aliases, remaining, depth + 1);
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

fn visit(expr: &Calcit, scope: &ScopeTypes, aliases: &HashMap<Arc<str>, Outcomes>, remaining: &mut usize, depth: usize) -> Outcomes {
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
    Calcit::Local(local) => return aliases.get(&local.sym).cloned().unwrap_or_else(|| Outcomes::unknown(true)),
    _ => {}
  }

  let direct = extract_predicate_bindings(expr, scope);
  if direct.true_binding.is_some() || direct.false_binding.is_some() {
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
    Some(Calcit::Proc(CalcitProc::Not)) if items.len() == 2 => {
      let inner = visit(items.get(1).unwrap(), scope, aliases, remaining, depth + 1);
      Outcomes {
        truthy: inner.falsey,
        falsey: inner.truthy,
        preserves_locals: inner.preserves_locals,
      }
    }
    Some(Calcit::Syntax(CalcitSyntax::If, _)) if matches!(items.len(), 3 | 4) => {
      let condition = visit(items.get(1).unwrap(), scope, aliases, remaining, depth + 1);
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
          let branch = visit(branch, &branch_scope, branch_aliases, remaining, depth + 1);
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
        return visit_sequence(items.iter().skip(2), scope, aliases, remaining, depth);
      }
      let (Some(Calcit::Local(local)), Some(value)) = (pair.first(), pair.get(1)) else {
        return Outcomes::unknown(false);
      };
      let mut bound = visit(value, scope, aliases, remaining, depth + 1);
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
      let mut body = visit_sequence(items.iter().skip(2), &body_scope, &body_aliases, remaining, depth);
      body.forget(&local.sym);
      body.preserves_locals &= preserves_initializer;
      body
    }
    // These value operations never invoke user callbacks. Merely declaring
    // a Bool/Number result is not an effect or predicate proof for other calls.
    Some(Calcit::Proc(proc)) if non_invoking_value_proc(*proc) => Outcomes::unknown(
      items
        .iter()
        .skip(1)
        .all(|item| visit(item, scope, aliases, remaining, depth + 1).preserves_locals),
    ),
    Some(Calcit::Import(import))
      if import.ns.as_ref() == calcit::CORE_NS
        && program::has_bundled_core_source()
        && (matches!(import.def.as_ref(), "=" | "&>=" | "&<=")
          || (import.def.as_ref() == "count"
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
          .all(|item| visit(item, scope, aliases, remaining, depth + 1).preserves_locals),
      )
    }
    _ => Outcomes::unknown(false),
  }
}

fn non_invoking_value_proc(proc: CalcitProc) -> bool {
  matches!(
    proc,
    CalcitProc::List
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
  )
}
