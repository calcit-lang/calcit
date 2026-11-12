//! The type a position expects from the form written there (#2637, #2733).
//!
//! A declared result, a parameter or a collection member offers its type to
//! the form in that position. The offer passes through `&let` bodies, `if`
//! branches and macro expansions such as `let`, `do` and `fn`, and is used by
//! the forms that cannot be typed bottom-up: a lambda takes its parameter
//! types, a bare tag names a zero-payload enum variant, a collection literal
//! hands the member type to its items. The context is addressed to one
//! preprocessing depth, so a form bound or passed elsewhere inside the offered
//! one (always deeper) never receives it.

use std::cell::{Cell, RefCell};
use std::sync::Arc;

use crate::calcit::CalcitTypeAnnotation;

/// An expected type and how to name its position in a diagnostic.
#[derive(Clone)]
pub(super) struct Expected {
  pub(super) annotation: Arc<CalcitTypeAnnotation>,
  pub(super) position: &'static str,
  /// Where to report a form rejected for this position, when it has no
  /// location of its own (a tag literal).
  pub(super) location: Option<crate::calcit::NodeLocation>,
}

thread_local! {
  static DEPTH: Cell<usize> = const { Cell::new(0) };
  static TAIL: RefCell<Option<(usize, Expected)>> = const { RefCell::new(None) };
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

/// Offers `expected` to the form preprocessed next by the current handler,
/// until the guard is dropped.
pub(super) struct TailScope {
  previous: Option<(usize, Expected)>,
}

impl TailScope {
  pub(super) fn offer(expected: Option<Expected>) -> Self {
    let next = expected.map(|expected| (current_depth() + 1, expected));
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

/// The type offered to the form handled at the current depth, if any.
pub(super) fn take_for_current() -> Option<Expected> {
  let depth = current_depth();
  TAIL.with(|cell| {
    let mut cell = cell.borrow_mut();
    if cell.as_ref().is_some_and(|(target, _)| *target == depth) {
      cell.take().map(|(_, expected)| expected)
    } else {
      None
    }
  })
}

/// The offer for the form handled at the current depth, left in place.
pub(super) fn peek_for_current() -> Option<Expected> {
  let depth = current_depth();
  TAIL.with(|cell| {
    cell
      .borrow()
      .as_ref()
      .filter(|(target, _)| *target == depth)
      .map(|(_, expected)| expected.clone())
  })
}

/// Whether the form handled at the current depth has an offer satisfying `test`.
pub(super) fn current_offer_is(test: impl FnOnce(&CalcitTypeAnnotation) -> bool) -> bool {
  let depth = current_depth();
  TAIL.with(|cell| {
    cell
      .borrow()
      .as_ref()
      .is_some_and(|(target, expected)| *target == depth && test(&expected.annotation))
  })
}

/// Hide any pending offer while compiling another definition on demand.
pub(super) struct SuspendScope {
  previous: Option<(usize, Expected)>,
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
