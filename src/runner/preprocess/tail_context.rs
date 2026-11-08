//! Expected callable types for lambdas in return position (#2637).
//!
//! A function whose declared result is a `Fn` gives that signature to a lambda
//! written as its last body form, also through `&let` bodies, `if` branches and
//! macro expansions such as `let`, `do` and `fn`. The context is addressed to
//! one preprocessing depth, so a lambda bound or passed elsewhere inside the
//! tail form (always deeper) never receives it.

use std::cell::{Cell, RefCell};
use std::sync::Arc;

use crate::calcit::CalcitFnTypeAnnotation;

thread_local! {
  static DEPTH: Cell<usize> = const { Cell::new(0) };
  static TAIL: RefCell<Option<(usize, Arc<CalcitFnTypeAnnotation>)>> = const { RefCell::new(None) };
}

/// Marks one `preprocess_expr` level for the duration of the guard.
pub(super) struct DepthGuard;

impl DepthGuard {
  pub(super) fn enter() -> Self {
    DEPTH.with(|depth| depth.set(depth.get() + 1));
    Self
  }
}

impl Drop for DepthGuard {
  fn drop(&mut self) {
    DEPTH.with(|depth| depth.set(depth.get() - 1));
  }
}

fn current_depth() -> usize {
  DEPTH.with(Cell::get)
}

/// Offers `signature` to the form preprocessed next by the current handler,
/// until the guard is dropped.
pub(super) struct TailScope {
  previous: Option<(usize, Arc<CalcitFnTypeAnnotation>)>,
}

impl TailScope {
  pub(super) fn offer(signature: Option<Arc<CalcitFnTypeAnnotation>>) -> Self {
    let next = signature.map(|signature| (current_depth() + 1, signature));
    Self {
      previous: TAIL.with(|cell| cell.replace(next)),
    }
  }
}

impl Drop for TailScope {
  fn drop(&mut self) {
    TAIL.with(|cell| *cell.borrow_mut() = self.previous.take());
  }
}

/// The signature offered to the form handled at the current depth, if any.
pub(super) fn take_for_current() -> Option<Arc<CalcitFnTypeAnnotation>> {
  let depth = current_depth();
  TAIL.with(|cell| {
    let mut cell = cell.borrow_mut();
    if cell.as_ref().is_some_and(|(target, _)| *target == depth) {
      cell.take().map(|(_, signature)| signature)
    } else {
      None
    }
  })
}

/// Hide any pending offer while compiling another definition on demand.
pub(super) struct SuspendScope {
  previous: Option<(usize, Arc<CalcitFnTypeAnnotation>)>,
}

impl SuspendScope {
  pub(super) fn enter() -> Self {
    Self {
      previous: TAIL.with(|cell| cell.borrow_mut().take()),
    }
  }
}

impl Drop for SuspendScope {
  fn drop(&mut self) {
    TAIL.with(|cell| *cell.borrow_mut() = self.previous.take());
  }
}
