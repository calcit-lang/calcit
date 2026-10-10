//! Post-lowering invariant validation (#1553).
//!
//! Preprocess resolves, infers, checks and rewrites in one walk. This module
//! re-examines the *final* tree of every definition after all rewrites, so a
//! construct that lost type evidence or skipped a check during lowering is
//! reported once, in one place, instead of per construct.
//!
//! The pass is a compiler-internal debug aid: it only runs when the
//! `CALCIT_LINT_CORE` environment variable is `1` (CI sets it for the core
//! checks) and reports violations as internal compiler errors that carry the
//! rewrite origin chain. It never changes the tree and adds no user-facing
//! diagnostic code.
//!
//! Invariants enforced today:
//! - (b) a lowering that is recorded at a rewrite point (method inlining,
//!   typed optional access, trait-bound method lowering, struct field-index
//!   lowering) must produce a node
//!   whose inferred type still proves the pre-rewrite type;
//! - (c) every `Proc` call, every call to a compiled user function and every
//!   `recur` in the final tree passes the same argument checks as the source
//!   form; a check that fires on the final tree
//!   but was never reported during preprocess means the lowered node skipped
//!   a check. When a recorded rewrite produced the call node, the violation
//!   also names that rewrite, so the origin chain points at the lowering.
//!   Such a call was checked in its source shape (for example as a method
//!   call), whose warning text differs from the lowered call's; a warning
//!   preprocess reported at the same call location therefore counts as the
//!   check having run.
//!
//! Invariant (a), "every checked node has a type or an explicit Unknown", is
//! documented but not enforced yet: it needs per-node type slots.
use super::*;
use crate::calcit::CalcitErrProvenance;
use std::sync::OnceLock;

/// The rewrite point that produced a lowered node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RewriteOrigin {
  /// `recv .method args` resolved to a direct callable.
  MethodInline,
  /// `get` / `nth` / `first` / `last` lowered to guarded `%some` / `%none`.
  TypedAccess,
  /// A trait-bounded method call lowered to `&trait-call`.
  TraitCall,
  /// `&struct:get` / `&struct:assoc` / `&struct:with` on a known struct
  /// lowered to the field-index procedures.
  StructFieldIndex,
}

impl RewriteOrigin {
  fn label(self) -> &'static str {
    match self {
      Self::MethodInline => "method-inline",
      Self::TypedAccess => "typed-access",
      Self::TraitCall => "trait-call",
      Self::StructFieldIndex => "struct-field-index",
    }
  }
}

/// Pre-rewrite evidence captured at a rewrite point.
#[derive(Debug, Clone)]
pub(super) struct RewriteEvidence {
  origin: RewriteOrigin,
  before_form: Calcit,
  before_type: Arc<CalcitTypeAnnotation>,
  after_form: Calcit,
  after_type: Arc<CalcitTypeAnnotation>,
  location: Option<NodeLocation>,
}

thread_local! {
  static FRAMES: RefCell<Vec<Vec<RewriteEvidence>>> = const { RefCell::new(Vec::new()) };
}

/// Whether the validation pass is switched on.
pub(super) fn enabled() -> bool {
  static ON: OnceLock<bool> = OnceLock::new();
  *ON.get_or_init(|| std::env::var("CALCIT_LINT_CORE").is_ok_and(|value| value == "1"))
}

/// Evidence frame of one definition; nested definitions compiled on demand get
/// their own frame.
pub(super) struct Scope {
  active: bool,
}

impl Scope {
  pub(super) fn enter() -> Self {
    let active = enabled();
    if active {
      FRAMES.with(|frames| frames.borrow_mut().push(Vec::new()));
    }
    Self { active }
  }

  /// Take the evidence recorded for this definition.
  pub(super) fn finish(mut self) -> Vec<RewriteEvidence> {
    if !self.active {
      return Vec::new();
    }
    self.active = false;
    FRAMES.with(|frames| frames.borrow_mut().pop().unwrap_or_default())
  }
}

impl Drop for Scope {
  fn drop(&mut self) {
    if self.active {
      FRAMES.with(|frames| {
        frames.borrow_mut().pop();
      });
    }
  }
}

/// Record the pre-rewrite type of `before` and the type of the lowered `after`.
/// `before` is only built when the pass is enabled.
pub(super) fn record_rewrite(origin: RewriteOrigin, before: impl FnOnce() -> Calcit, after: &Calcit, scope_types: &ScopeTypes) {
  if !enabled() {
    return;
  }
  let before = before();
  let Some(before_type) = infer_type_from_expr(&before, scope_types) else {
    return;
  };
  let after_type = infer_type_from_expr(after, scope_types).unwrap_or_else(|| calcit::DYNAMIC_TYPE.clone());
  let evidence = RewriteEvidence {
    origin,
    before_form: before.clone(),
    before_type,
    after_form: after.clone(),
    after_type,
    location: find_calcit_location_matching(&before, |_| true),
  };
  FRAMES.with(|frames| {
    if let Some(frame) = frames.borrow_mut().last_mut() {
      frame.push(evidence);
    }
  });
}

struct Violation {
  invariant: &'static str,
  detail: String,
  location: Option<NodeLocation>,
  chain: Vec<CalcitErrProvenance>,
}

fn chain_item(kind: &str, operation: String, location: Option<&NodeLocation>, type_text: String) -> CalcitErrProvenance {
  CalcitErrProvenance {
    kind: kind.to_owned(),
    operation,
    definition: location.map(|location| format!("{}/{}", location.ns, location.def)),
    path: location.map(|location| {
      if location.coord.is_empty() {
        "code".to_owned()
      } else {
        format!("code@{}", location.coord.iter().map(u16::to_string).collect::<Vec<_>>().join("."))
      }
    }),
    r#type: type_text.clone(),
    output_type: type_text,
    flow: "post-lowering".to_owned(),
    migration: "Compiler-internal: preserve the pre-rewrite evidence at the rewrite point; do not exempt the definition.".to_owned(),
  }
}

/// The source form and the lowered form of one recorded rewrite.
fn rewrite_chain(item: &RewriteEvidence) -> [CalcitErrProvenance; 2] {
  [
    chain_item(
      "source",
      item.before_form.to_string(),
      item.location.as_ref(),
      item.before_type.to_brief_string(),
    ),
    chain_item(
      "lowering",
      format!("{} => {}", item.origin.label(), item.after_form),
      item.location.as_ref(),
      item.after_type.to_brief_string(),
    ),
  ]
}

/// Origin chain of a call node: the call itself, then every recorded rewrite
/// that produced it.
fn call_chain(operation: String, location: Option<&NodeLocation>, rewrites: &[&RewriteEvidence]) -> Vec<CalcitErrProvenance> {
  let mut chain = vec![chain_item("call", operation, location, String::new())];
  chain.extend(rewrites.iter().flat_map(|item| rewrite_chain(item)));
  chain
}

/// Visit every list node of a lowered tree, skipping quoted data.
fn visit_lists(expr: &Calcit, visit: &mut impl FnMut(&Calcit, &CalcitList)) {
  let Calcit::List(items) = expr else {
    return;
  };
  if let Some(Calcit::Syntax(CalcitSyntax::Quote | CalcitSyntax::Quasiquote, _)) = items.first() {
    return;
  }
  visit(expr, items);
  for item in items.iter() {
    visit_lists(item, visit);
  }
}

/// Warning text with the enclosing definition name erased: the source check
/// sometimes cannot name the definition (`ns/??`) while the final tree can.
fn normalized_message(message: &str, ns: &str, def: &str) -> String {
  message
    .replace(&format!("{ns}/{def}"), "<def>")
    .replace(&format!("{ns}/??"), "<def>")
}

fn reported_messages(reported: &[LocatedWarning], ns: &str, def: &str) -> HashSet<String> {
  reported
    .iter()
    .map(|warning| normalized_message(warning.message(), ns, def))
    .collect()
}

/// Validate the final tree of one definition. Returns an internal compiler
/// error that lists every violation with its origin chain.
pub(super) fn validate_definition(
  ns: &str,
  def: &str,
  resolved: &Calcit,
  reported: &[LocatedWarning],
  evidence: &[RewriteEvidence],
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  // The reported set only matters when a re-check finds something, so build it lazily.
  let known_set = std::cell::OnceCell::new();
  let known = |message: &str| {
    known_set
      .get_or_init(|| reported_messages(reported, ns, def))
      .contains(&normalized_message(message, ns, def))
  };
  // A rewritten call was checked in its source shape, so its warning text can
  // differ from the lowered call's; the shared call location identifies it.
  let reported_at = |location: Option<&NodeLocation>, rewritten: bool| {
    rewritten && location.is_some_and(|location| reported.iter().any(|warning| warning.location() == location))
  };
  let mut violations: Vec<Violation> = vec![];
  let mut live: Vec<&RewriteEvidence> = vec![];

  visit_lists(resolved, &mut |node, items| {
    let produced_by = evidence.iter().filter(|item| item.after_form == *node).collect::<Vec<_>>();
    live.extend(produced_by.iter().copied());
    match items.first() {
      Some(Calcit::Proc(CalcitProc::Recur)) => {}
      Some(Calcit::Proc(proc)) => {
        let recheck = RefCell::new(vec![]);
        let location = find_calcit_location_matching(node, |location| location.def.as_ref() != GENERATED_DEF);
        check_proc_arg_types(proc, &items.drop_left(), &ScopeTypes::new(), ns, def, location.clone(), &recheck);
        let already_checked = reported_at(location.as_ref(), !produced_by.is_empty());
        for warning in recheck
          .borrow()
          .iter()
          .filter(|warning| !already_checked && !known(warning.message()))
        {
          violations.push(Violation {
            invariant: "(c) every call node is checked",
            detail: format!(
              "the final tree fails a Proc argument check that preprocess never reported: {}",
              warning.message()
            ),
            location: location.clone(),
            chain: call_chain(proc.as_ref().to_owned(), location.as_ref(), &produced_by),
          });
        }
      }
      Some(Calcit::Syntax(CalcitSyntax::Defn, _)) => check_recur_in_function(ns, def, items, &known, &mut violations),
      Some(head @ (Calcit::Import(_) | Calcit::Fn { .. })) => {
        let Some(info) = resolved_user_fn(head, call_stack) else {
          return;
        };
        let recheck = RefCell::new(vec![]);
        let location = find_calcit_location_matching(node, |location| location.def.as_ref() != GENERATED_DEF);
        let call_info = CallTypeCheckInfo {
          file_ns: ns,
          def_name: def,
          call_location: location.clone(),
        };
        check_user_fn_arg_types(&info, head, &items.drop_left(), &ScopeTypes::new(), &call_info, &recheck);
        let already_checked = reported_at(location.as_ref(), !produced_by.is_empty());
        for warning in recheck
          .borrow()
          .iter()
          .filter(|warning| !already_checked && !known(warning.message()))
        {
          violations.push(Violation {
            invariant: "(c) every call node is checked",
            detail: format!(
              "the final tree fails a function argument check that preprocess never reported: {}",
              warning.message()
            ),
            location: location.clone(),
            chain: call_chain(format!("{}/{}", info.def_ns, info.name), location.as_ref(), &produced_by),
          });
        }
      }
      _ => {}
    }
  });

  for item in live {
    if matches!(item.before_type.as_ref(), CalcitTypeAnnotation::Dynamic) {
      continue;
    }
    if item
      .after_type
      .prove_with_bindings(&item.before_type, &mut HashMap::new())
      .is_proven()
    {
      continue;
    }
    violations.push(Violation {
      invariant: "(b) rewriting never lowers type precision",
      detail: format!(
        "`{}` lowering turned type `{}` into `{}`",
        item.origin.label(),
        item.before_type.to_brief_string(),
        item.after_type.to_brief_string()
      ),
      location: item.location.clone(),
      chain: rewrite_chain(item).to_vec(),
    });
  }

  if violations.is_empty() {
    return Ok(());
  }
  let mut message = format!(
    "internal compiler error: post-lowering validation (CALCIT_LINT_CORE=1) found {} violation(s) in {ns}/{def}",
    violations.len()
  );
  for violation in &violations {
    message.push_str(&format!("\n  invariant {}: {}", violation.invariant, violation.detail));
    for link in &violation.chain {
      message.push_str(&format!(
        "\n    origin {} {} @ {}{}",
        link.kind,
        link.operation,
        link.path.as_deref().unwrap_or("-"),
        if link.r#type.is_empty() {
          String::new()
        } else {
          format!(" : {}", link.r#type)
        }
      ));
    }
  }
  let location = violations.iter().find_map(|violation| violation.location.clone());
  let mut error = CalcitErr::use_msg_stack_location(CalcitErrKind::Unexpected, message, call_stack, location);
  *error.provenance = violations.into_iter().flat_map(|violation| violation.chain).collect();
  Err(error)
}

/// The user function a final-tree call head refers to. Only already compiled
/// definitions are looked up: validation never compiles or evaluates code, and
/// a definition still being compiled (a self call) has no checked contract yet.
fn resolved_user_fn(head: &Calcit, call_stack: &CallStackList) -> Option<Arc<CalcitFn>> {
  let value = match head {
    Calcit::Fn { info, .. } => return Some(info.clone()),
    Calcit::Import(CalcitImport { ns, def, .. }) => program::resolve_compiled_executable_def(ns, def, call_stack).ok().flatten(),
    _ => None,
  };
  match value {
    Some(Calcit::Fn { info, .. }) => Some(info),
    _ => None,
  }
}

/// Re-run the `recur` arity and argument checks against the lowered function.
/// Lowering may introduce `recur` after the source-level check ran.
fn check_recur_in_function(ns: &str, def: &str, items: &CalcitList, known: &dyn Fn(&str) -> bool, violations: &mut Vec<Violation>) {
  let Some(Calcit::List(params)) = items.get(2) else {
    return;
  };
  let mut types: Vec<Arc<CalcitTypeAnnotation>> = vec![];
  let mut scope = ScopeTypes::new();
  for param in params.iter() {
    match param {
      Calcit::Local(local) => {
        scope.insert(local.sym.clone(), local.type_info.clone());
        types.push(local.type_info.clone());
      }
      // Marked parameters follow their own arity rules, same as the source check.
      _ => return,
    }
  }
  let recheck = RefCell::new(vec![]);
  for body in items.iter().skip(3) {
    check_recur_args_in_expr(body, types.len(), &types, &scope, ns, def, &recheck);
  }
  for warning in recheck.borrow().iter().filter(|warning| !known(warning.message())) {
    violations.push(Violation {
      invariant: "(c) every call node is checked",
      detail: format!(
        "the final tree fails a recur check that preprocess never reported: {}",
        warning.message()
      ),
      location: Some(warning.location().clone()),
      chain: vec![chain_item("recur", "recur".to_owned(), Some(warning.location()), String::new())],
    });
  }
}

/// Regression set for the post-lowering validator (#1553). Each test builds the
/// final tree a past bug used to leave behind and asserts that the pass reports
/// it with an origin chain; the matching valid programs run under
/// `CALCIT_LINT_CORE=1` in `tests/post_lowering_cli.rs` and in CI.
#[cfg(test)]
mod tests {
  use super::*;

  fn local(name: &str, type_info: Arc<CalcitTypeAnnotation>) -> Calcit {
    Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from(name)),
      sym: Arc::from(name),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.post-lowering"),
        at_def: Arc::from("main"),
      }),
      location: None,
      type_info,
    })
  }

  fn number() -> Arc<CalcitTypeAnnotation> {
    Arc::new(CalcitTypeAnnotation::Number)
  }

  fn validate(tree: &Calcit, reported: &[LocatedWarning], evidence: &[RewriteEvidence]) -> Result<(), CalcitErr> {
    validate_definition("tests.post-lowering", "main", tree, reported, evidence, &CallStackList::default())
  }

  /// #1428: `recur` arguments were never checked against the function type.
  #[test]
  fn recur_with_wrong_argument_type_is_reported_with_origin() {
    let tree = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::Defn, Arc::from("calcit.core")),
      Calcit::Symbol {
        sym: Arc::from("count-up"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.post-lowering"),
          at_def: Arc::from("main"),
        }),
        location: None,
      },
      Calcit::from(vec![local("n", number())]),
      Calcit::from(vec![Calcit::Proc(CalcitProc::Recur), Calcit::new_str("oops")]),
    ]);
    let error = validate(&tree, &[], &[]).expect_err("an unchecked recur argument must be reported");
    assert!(
      error.msg.contains("internal compiler error: post-lowering validation"),
      "{}",
      error.msg
    );
    assert!(error.msg.contains("recur"), "{}", error.msg);
    assert!(error.msg.contains("origin recur"), "{}", error.msg);
    assert!(!error.provenance.is_empty());
  }

  #[test]
  fn recur_reported_during_preprocess_is_not_a_violation() {
    let recur_call = Calcit::from(vec![Calcit::Proc(CalcitProc::Recur), Calcit::new_str("oops")]);
    let tree = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::Defn, Arc::from("calcit.core")),
      Calcit::Symbol {
        sym: Arc::from("count-up"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.post-lowering"),
          at_def: Arc::from("main"),
        }),
        location: None,
      },
      Calcit::from(vec![local("n", number())]),
      recur_call.clone(),
    ]);
    let recheck = RefCell::new(vec![]);
    check_recur_args_in_expr(
      &recur_call,
      1,
      &[number()],
      &ScopeTypes::new(),
      "tests.post-lowering",
      "main",
      &recheck,
    );
    assert!(!recheck.borrow().is_empty(), "the source check must reject the argument");
    let reported = recheck.into_inner();
    validate(&tree, &reported, &[]).expect("a reported finding is not a lowering violation");
  }

  /// #1494: an inlined Proc method bypassed the argument type check.
  #[test]
  fn proc_call_that_skipped_argument_checks_is_reported() {
    let map = Arc::new(CalcitTypeAnnotation::Map(
      Arc::new(CalcitTypeAnnotation::Tag),
      Arc::new(CalcitTypeAnnotation::Number),
    ));
    let call = Calcit::from(vec![
      Calcit::Proc(CalcitProc::NativeMapAssoc),
      local("m", map),
      Calcit::Tag(cirru_edn::EdnTag::from("b")),
      Calcit::new_str("oops"),
    ]);
    let error = validate(&call, &[], &[]).expect_err("the lowered call must be checked");
    assert!(error.msg.contains("(c) every call node is checked"), "{}", error.msg);
    assert!(error.msg.contains("&map:assoc"), "{}", error.msg);
  }

  fn expects_number_fn() -> Calcit {
    Calcit::Fn {
      id: Arc::from("tests.post-lowering/expects-number"),
      info: Arc::new(CalcitFn {
        name: Arc::from("expects-number"),
        def_ns: Arc::from("tests.post-lowering"),
        def_ref: None,
        usage: crate::calcit::CalcitFnUsageMeta::default(),
        scope: Arc::new(CalcitScope::default()),
        args: Arc::new(CalcitFnArgs::Args(vec![0])),
        call_shape: crate::calcit::CalcitFnCallShape::fixed(1),
        body: vec![Calcit::Nil],
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types: vec![number()],
        return_type: number(),
        rest_type: None,
      }),
    }
  }

  /// A lowered call to a user function must pass the same argument checks as
  /// the source call; rewrites that build user calls (method inlining, macro
  /// expansion) would otherwise skip them.
  #[test]
  fn user_fn_call_that_skipped_argument_checks_is_reported() {
    let call = Calcit::from(vec![expects_number_fn(), Calcit::new_str("oops")]);
    let error = validate(&call, &[], &[]).expect_err("the lowered user call must be checked");
    assert!(error.msg.contains("(c) every call node is checked"), "{}", error.msg);
    assert!(error.msg.contains("function argument check"), "{}", error.msg);
    assert!(
      error.msg.contains("origin call tests.post-lowering/expects-number"),
      "{}",
      error.msg
    );

    let valid = Calcit::from(vec![expects_number_fn(), Calcit::Number(1.0)]);
    validate(&valid, &[], &[]).expect("a matching argument is valid");
  }

  /// A call produced by a recorded method inlining names that rewrite in its
  /// origin chain, so the violation points at the lowering, not only the call.
  #[test]
  fn skipped_check_on_an_inlined_call_names_the_rewrite() {
    let map = Arc::new(CalcitTypeAnnotation::Map(
      Arc::new(CalcitTypeAnnotation::Tag),
      Arc::new(CalcitTypeAnnotation::Number),
    ));
    let call = Calcit::from(vec![
      Calcit::Proc(CalcitProc::NativeMapAssoc),
      local("m", map.clone()),
      Calcit::Tag(cirru_edn::EdnTag::from("b")),
      Calcit::new_str("oops"),
    ]);
    let evidence = RewriteEvidence {
      origin: RewriteOrigin::MethodInline,
      before_form: Calcit::from(vec![Calcit::new_str(".assoc"), Calcit::new_str("m")]),
      before_type: map.clone(),
      after_form: call.clone(),
      after_type: map,
      location: None,
    };
    let error = validate(&call, &[], &[evidence]).expect_err("the lowered call must be checked");
    assert!(error.msg.contains("(c) every call node is checked"), "{}", error.msg);
    assert!(error.msg.contains("origin call &map:assoc"), "{}", error.msg);
    assert!(error.msg.contains("origin lowering method-inline"), "{}", error.msg);
    assert!(
      error.provenance.iter().any(|link| link.kind == "lowering"),
      "{:?}",
      error.provenance
    );
  }

  /// A rewritten call was checked in its source shape: preprocess reported a
  /// method warning at the call location, with different text than the
  /// lowered call's check. That is not a skipped check.
  #[test]
  fn rewritten_call_reported_at_its_location_is_not_a_violation() {
    let at = NodeLocation::new(Arc::from("tests.post-lowering"), Arc::from("main"), Arc::new(vec![3, 2, 0]));
    let map = Arc::new(CalcitTypeAnnotation::Map(
      Arc::new(CalcitTypeAnnotation::Tag),
      Arc::new(CalcitTypeAnnotation::Number),
    ));
    let Calcit::Local(mut receiver) = local("m", map.clone()) else {
      unreachable!()
    };
    receiver.location = Some(at.coord.clone());
    let call = Calcit::from(vec![
      Calcit::Proc(CalcitProc::NativeMapAssoc),
      Calcit::Local(receiver),
      Calcit::Tag(cirru_edn::EdnTag::from("b")),
      Calcit::new_str("oops"),
    ]);
    let evidence = RewriteEvidence {
      origin: RewriteOrigin::MethodInline,
      before_form: Calcit::Nil,
      before_type: map.clone(),
      after_form: call.clone(),
      after_type: map,
      location: None,
    };
    let source_warning = LocatedWarning::new("[Warn] Method `.assoc` arg 3 expects type `number`".to_owned(), at);
    validate(&call, &[source_warning], std::slice::from_ref(&evidence)).expect("the source-shaped check ran at this call");
    validate(&call, &[], &[evidence]).expect_err("without a reported warning the skipped check is still a violation");
  }

  /// #1378: `%none` was re-inferred as `Option<Dynamic>` and erased the payload.
  #[test]
  fn lowering_that_erases_a_payload_type_is_reported_with_both_forms() {
    let node = Calcit::from(vec![local("lowered", calcit::DYNAMIC_TYPE.clone()), Calcit::Number(1.0)]);
    let evidence = RewriteEvidence {
      origin: RewriteOrigin::TypedAccess,
      before_form: Calcit::from(vec![Calcit::new_str("get"), Calcit::new_str("m")]),
      before_type: Arc::new(CalcitTypeAnnotation::Optional(number())),
      after_form: node.clone(),
      after_type: Arc::new(CalcitTypeAnnotation::Optional(calcit::DYNAMIC_TYPE.clone())),
      location: None,
    };
    let error = validate(&node, &[], std::slice::from_ref(&evidence)).expect_err("precision loss must be reported");
    assert!(error.msg.contains("(b) rewriting never lowers type precision"), "{}", error.msg);
    assert!(error.msg.contains("typed-access"), "{}", error.msg);
    assert!(error.msg.contains("origin source"), "{}", error.msg);
    assert!(error.msg.contains("origin lowering"), "{}", error.msg);

    // A rewrite that no longer appears in the final tree is not live evidence.
    let unrelated = Calcit::Number(1.0);
    validate(&unrelated, &[], &[evidence]).expect("only nodes that reach the final tree are validated");
  }

  #[test]
  fn struct_field_index_lowering_is_named_in_the_violation() {
    let node = Calcit::from(vec![Calcit::Proc(CalcitProc::NativeStructNth), Calcit::Number(0.0)]);
    let evidence = RewriteEvidence {
      origin: RewriteOrigin::StructFieldIndex,
      before_form: Calcit::Nil,
      before_type: number(),
      after_form: node.clone(),
      after_type: calcit::DYNAMIC_TYPE.clone(),
      location: None,
    };
    let error = validate(&node, &[], &[evidence]).expect_err("precision loss must be reported");
    assert!(error.msg.contains("`struct-field-index` lowering"), "{}", error.msg);
  }

  #[test]
  fn lowering_that_keeps_its_type_passes() {
    let node = Calcit::from(vec![local("lowered", number()), Calcit::Number(1.0)]);
    let evidence = RewriteEvidence {
      origin: RewriteOrigin::MethodInline,
      before_form: Calcit::Nil,
      before_type: number(),
      after_form: node.clone(),
      after_type: number(),
      location: None,
    };
    validate(&node, &[], &[evidence]).expect("equal precision is valid");
  }
}
