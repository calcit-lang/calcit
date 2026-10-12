//! Lexical record of locals whose open type was chosen in source (#1767).
//!
//! `Dynamic` is also the fallback for evidence inference cannot express yet
//! (heterogeneous Map entries, anonymous enum payloads, contextual callback
//! inputs). Only a declared parameter contract, or a `&let` alias computed
//! from such a parameter, is treated as an explicitly open value that must be
//! proved before it reaches a concrete parameter.
//! The same lexical record retains the initial write contract independently
//! of predicate refinements on references to the binding.
use super::*;

type BindingKey = (Arc<str>, Arc<str>, Arc<str>);

#[derive(Clone)]
struct LocalContract {
  explicitly_open: bool,
  annotation: Arc<CalcitTypeAnnotation>,
}

thread_local! {
  static EXPLICIT_OPEN_LOCALS: RefCell<HashMap<BindingKey, LocalContract>> = RefCell::new(HashMap::new());
}

fn binding_key(local: &CalcitLocal) -> BindingKey {
  (local.info.at_ns.clone(), local.info.at_def.clone(), local.sym.clone())
}

/// Restore lexical marks on success, rejection and unwinding. Every binding is
/// recorded, so an inner binding with inferred evidence shadows an outer
/// explicitly open one.
pub(super) struct ExplicitOpenScope(Vec<(BindingKey, Option<LocalContract>)>);

impl ExplicitOpenScope {
  pub(super) fn new() -> Self {
    Self(Vec::new())
  }

  pub(super) fn bind(&mut self, local: &Calcit, explicitly_open: bool) {
    let Calcit::Local(local) = local else {
      return;
    };
    let key = binding_key(local);
    let contract = LocalContract {
      explicitly_open,
      annotation: local.type_info.clone(),
    };
    let previous = EXPLICIT_OPEN_LOCALS.with(|marks| marks.borrow_mut().insert(key.clone(), contract));
    self.0.push((key, previous));
  }
}

/// The original lexical contract, before branch evidence refines a reference.
/// Reuse the binding record so shadowing and unwinding restore both together.
pub(super) fn binding_contract(local: &CalcitLocal) -> Option<Arc<CalcitTypeAnnotation>> {
  EXPLICIT_OPEN_LOCALS.with(|marks| marks.borrow().get(&binding_key(local)).map(|contract| contract.annotation.clone()))
}

impl Drop for ExplicitOpenScope {
  fn drop(&mut self) {
    EXPLICIT_OPEN_LOCALS.with(|marks| {
      let mut marks = marks.borrow_mut();
      for (key, previous) in self.0.drain(..).rev() {
        match previous {
          Some(previous) => {
            marks.insert(key, previous);
          }
          None => {
            marks.remove(&key);
          }
        }
      }
    });
  }
}

/// Whether an expression reads a local marked as explicitly open. Nested
/// function bodies are skipped: they bind their own parameters and produce a
/// callable rather than the open value itself.
pub(super) fn reads_explicitly_open_local(value: &Calcit) -> bool {
  let any_marked = EXPLICIT_OPEN_LOCALS.with(|marks| marks.borrow().values().any(|contract| contract.explicitly_open));
  any_marked && reads_marked_local(value)
}

fn reads_marked_local(value: &Calcit) -> bool {
  match value {
    Calcit::Local(local) => EXPLICIT_OPEN_LOCALS.with(|marks| {
      marks
        .borrow()
        .get(&binding_key(local))
        .is_some_and(|contract| contract.explicitly_open)
    }),
    Calcit::List(items) => {
      if matches!(
        items.first(),
        Some(Calcit::Syntax(CalcitSyntax::Defn | CalcitSyntax::Quote | CalcitSyntax::HintFn, _))
      ) {
        return false;
      }
      items.iter().any(reads_marked_local)
    }
    _ => false,
  }
}
