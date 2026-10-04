//! Retain evaluated source during lexical function constraint retries.
//! This plan is scoped to one compilation, not a cache of local type evidence.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use crate::calcit::{Calcit, CalcitList, CalcitMacro};

#[derive(Clone)]
pub(super) struct Expansion {
  pub definition: Arc<CalcitMacro>,
  pub code: Calcit,
  pub recur_inputs: Vec<Vec<Calcit>>,
  pub native_lowered: bool,
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum SiteKind {
  Function,
  Macro,
  Assertion,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct SourceSite {
  parent_macro: Option<usize>,
  source: usize,
  kind: SiteKind,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct Occurrence {
  site: SourceSite,
  ordinal: usize,
}

struct MacroEntry {
  // Keep source identities alive; an allocator must not recycle a memo key.
  _source: Arc<CalcitList>,
  macro_id: Arc<str>,
  definition: Arc<CalcitMacro>,
  id: usize,
  expansion: Option<Expansion>,
}

#[derive(Default)]
struct FunctionFrame {
  _parameters: Option<Arc<CalcitList>>,
  visits: HashMap<SourceSite, usize>,
  children: HashMap<Occurrence, usize>,
  macros: HashMap<Occurrence, MacroEntry>,
  assertions: HashMap<Occurrence, (Arc<CalcitList>, bool)>,
}

impl FunctionFrame {
  fn next_occurrence(&mut self, site: SourceSite) -> Occurrence {
    let ordinal = self.visits.entry(site.clone()).or_default();
    let occurrence = Occurrence { site, ordinal: *ordinal };
    *ordinal += 1;
    occurrence
  }
}

#[derive(Default)]
struct Plan {
  frames: Vec<FunctionFrame>,
  next_macro: usize,
}

#[derive(Clone)]
struct Active {
  plan: Rc<RefCell<Plan>>,
  frame: usize,
  parent_macro: Option<usize>,
  restart: bool,
}

thread_local! {
  static ACTIVE: RefCell<Option<Active>> = const { RefCell::new(None) };
}

/// Restore the enclosing lexical compilation even on error or unwinding.
pub(super) struct Scope {
  previous: Option<Active>,
  macro_entry: Option<(Rc<RefCell<Plan>>, usize, Occurrence)>,
}

impl Drop for Scope {
  fn drop(&mut self) {
    ACTIVE.with(|active| *active.borrow_mut() = self.previous.take());
  }
}

/// Retain an existing assertion obligation, never its old local type evidence.
pub(super) struct Assertion {
  site: Option<(Rc<RefCell<Plan>>, usize, Occurrence)>,
  pub required: bool,
}

impl Assertion {
  pub fn at_source(source: &Arc<CalcitList>) -> Self {
    let Some(active) = ACTIVE.with(|active| active.borrow().clone()) else {
      return Self {
        site: None,
        required: false,
      };
    };
    let mut plan = active.plan.borrow_mut();
    let frame = &mut plan.frames[active.frame];
    let occurrence = frame.next_occurrence(SourceSite {
      parent_macro: active.parent_macro,
      source: Arc::as_ptr(source) as usize,
      kind: SiteKind::Assertion,
    });
    let required = frame
      .assertions
      .entry(occurrence.clone())
      .or_insert_with(|| (source.clone(), false))
      .1;
    Self {
      site: Some((active.plan.clone(), active.frame, occurrence)),
      required,
    }
  }

  pub fn retain_required(&self) {
    if let Some((plan, frame, occurrence)) = &self.site {
      plan.borrow_mut().frames[*frame]
        .assertions
        .get_mut(occurrence)
        .expect("entered assertion source")
        .1 = true;
    }
  }
}

impl Scope {
  pub fn pause_definition() -> Self {
    Self {
      previous: ACTIVE.with(|active| active.borrow_mut().take()),
      macro_entry: None,
    }
  }

  pub fn enter_function(parameters: &Arc<CalcitList>, needs_constraints: bool) -> Self {
    let previous = ACTIVE.with(|active| active.borrow().clone());
    let next = if let Some(enclosing) = previous.as_ref() {
      let mut plan = enclosing.plan.borrow_mut();
      let frame = if enclosing.restart {
        enclosing.frame
      } else {
        // Interpolated source may share an Arc at multiple logical sites.
        // A parent expansion and an occurrence ordinal keep those distinct.
        let occurrence = plan.frames[enclosing.frame].next_occurrence(SourceSite {
          parent_macro: enclosing.parent_macro,
          source: Arc::as_ptr(parameters) as usize,
          kind: SiteKind::Function,
        });
        if let Some(frame) = plan.frames[enclosing.frame].children.get(&occurrence) {
          *frame
        } else {
          let frame = plan.frames.len();
          plan.frames.push(FunctionFrame {
            _parameters: Some(parameters.clone()),
            ..FunctionFrame::default()
          });
          plan.frames[enclosing.frame].children.insert(occurrence, frame);
          frame
        }
      };
      plan.frames[frame].visits.clear();
      Some(Active {
        plan: enclosing.plan.clone(),
        frame,
        parent_macro: enclosing.parent_macro,
        restart: false,
      })
    } else if needs_constraints {
      Some(Active {
        plan: Rc::new(RefCell::new(Plan {
          frames: vec![FunctionFrame {
            _parameters: Some(parameters.clone()),
            ..FunctionFrame::default()
          }],
          ..Plan::default()
        })),
        frame: 0,
        parent_macro: None,
        restart: false,
      })
    } else {
      None
    };
    ACTIVE.with(|active| *active.borrow_mut() = next);
    Self {
      previous,
      macro_entry: None,
    }
  }

  pub fn restart_function() -> Self {
    let previous = ACTIVE.with(|active| active.borrow().clone());
    if let Some(enclosing) = previous.as_ref() {
      let mut next = enclosing.clone();
      next.restart = true;
      ACTIVE.with(|active| *active.borrow_mut() = Some(next));
    }
    Self {
      previous,
      macro_entry: None,
    }
  }

  /// Reuse a resolved static callee, not a new value with a fresh runtime ID.
  /// Expression heads still preprocess normally to recheck their obligations.
  pub fn retained_static_callee(source: &Arc<CalcitList>) -> Option<(Arc<str>, Arc<CalcitMacro>)> {
    let active = ACTIVE.with(|active| active.borrow().clone())?;
    let plan = active.plan.borrow();
    let frame = &plan.frames[active.frame];
    let site = SourceSite {
      parent_macro: active.parent_macro,
      source: Arc::as_ptr(source) as usize,
      kind: SiteKind::Macro,
    };
    let occurrence = Occurrence {
      ordinal: frame.visits.get(&site).copied().unwrap_or_default(),
      site,
    };
    let entry = frame.macros.get(&occurrence)?;
    entry.expansion.as_ref()?;
    Some((entry.macro_id.clone(), entry.definition.clone()))
  }

  pub fn enter_macro(source: &Arc<CalcitList>, macro_id: &Arc<str>, definition: &Arc<CalcitMacro>) -> (Self, Option<Expansion>) {
    let previous = ACTIVE.with(|active| active.borrow().clone());
    let Some(enclosing) = previous.as_ref() else {
      return (
        Self {
          previous,
          macro_entry: None,
        },
        None,
      );
    };
    let mut plan = enclosing.plan.borrow_mut();
    // Resolving one source definition can allocate a fresh runtime macro ID.
    // A lexical retry instead reuses its already resolved callee and source.
    let occurrence = plan.frames[enclosing.frame].next_occurrence(SourceSite {
      parent_macro: enclosing.parent_macro,
      source: Arc::as_ptr(source) as usize,
      kind: SiteKind::Macro,
    });
    if !plan.frames[enclosing.frame].macros.contains_key(&occurrence) {
      let id = plan.next_macro;
      plan.next_macro += 1;
      plan.frames[enclosing.frame].macros.insert(
        occurrence.clone(),
        MacroEntry {
          _source: source.clone(),
          macro_id: macro_id.clone(),
          definition: definition.clone(),
          id,
          expansion: None,
        },
      );
    }
    let entry = &plan.frames[enclosing.frame].macros[&occurrence];
    let expansion = entry.expansion.clone();
    let next = Active {
      plan: enclosing.plan.clone(),
      frame: enclosing.frame,
      parent_macro: Some(entry.id),
      restart: false,
    };
    let macro_entry = Some((enclosing.plan.clone(), enclosing.frame, occurrence));
    drop(plan);
    ACTIVE.with(|active| *active.borrow_mut() = Some(next));
    (Self { previous, macro_entry }, expansion)
  }

  pub fn retain_expansion(&self, code: &Calcit, recur_inputs: &[Vec<Calcit>]) {
    self.retain(code, recur_inputs, false);
  }

  pub fn retain_native_expansion(&self, code: &Calcit) {
    self.retain(code, &[], true);
  }

  fn retain(&self, code: &Calcit, recur_inputs: &[Vec<Calcit>], native_lowered: bool) {
    if let Some((plan, frame, occurrence)) = &self.macro_entry {
      let mut plan = plan.borrow_mut();
      let entry = plan.frames[*frame].macros.get_mut(occurrence).expect("entered macro occurrence");
      entry.expansion = Some(Expansion {
        definition: entry.definition.clone(),
        code: code.clone(),
        recur_inputs: recur_inputs.to_vec(),
        native_lowered,
      });
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::panic::{AssertUnwindSafe, catch_unwind};

  #[test]
  fn restores_lexical_plan_after_definition_pause_retry_and_unwind() {
    assert!(ACTIVE.with(|active| active.borrow().is_none()));
    let parameters = Arc::new(CalcitList::default());
    let root = Scope::enter_function(&parameters, true);
    let initial = ACTIVE.with(|active| active.borrow().clone()).expect("root plan");
    {
      let _paused = Scope::pause_definition();
      assert!(ACTIVE.with(|active| active.borrow().is_none()));
      let _unrelated = Scope::enter_function(&parameters, false);
      assert!(ACTIVE.with(|active| active.borrow().is_none()));
    }
    assert!(ACTIVE.with(|active| Rc::ptr_eq(&active.borrow().as_ref().unwrap().plan, &initial.plan)));
    {
      let _restart = Scope::restart_function();
      let _retry = Scope::enter_function(&parameters, false);
      let retry = ACTIVE.with(|active| active.borrow().clone()).expect("retry plan");
      assert!(Rc::ptr_eq(&retry.plan, &initial.plan));
      assert_eq!(retry.frame, initial.frame);
      assert!(!retry.restart, "restart applies to only the retried function");
    }
    let unwound = catch_unwind(AssertUnwindSafe(|| {
      let _nested = Scope::enter_function(&parameters, false);
      assert_ne!(ACTIVE.with(|active| active.borrow().as_ref().unwrap().frame), initial.frame);
      panic!("abort nested compilation");
    }));
    assert!(unwound.is_err());
    let restored = ACTIVE.with(|active| active.borrow().clone()).expect("enclosing plan after unwind");
    assert!(Rc::ptr_eq(&restored.plan, &initial.plan));
    assert_eq!(restored.frame, initial.frame);
    assert!(!restored.restart);
    drop(root);
    assert!(
      ACTIVE.with(|active| active.borrow().is_none()),
      "compilations must not share a stale plan"
    );
  }
}
