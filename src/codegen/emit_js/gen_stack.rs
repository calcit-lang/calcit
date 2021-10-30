use std::sync::RwLock;

use crate::call_stack::{CalcitStack, CallStackVec, StackKind};
use crate::primes::{Calcit, CalcitItems};

lazy_static! {
  static ref CALL_STACK: RwLock<rpds::VectorSync<CalcitStack>> = RwLock::new(rpds::vector_sync![]);
}

pub fn push_call_stack(ns: &str, def: &str, kind: StackKind, code: Calcit, args: &CalcitItems) {
  let mut stack = CALL_STACK.write().unwrap();
  (*stack).push_back_mut(CalcitStack {
    ns: ns.to_owned(),
    def: def.to_owned(),
    code,
    args: args.to_owned(),
    kind,
  })
}

pub fn pop_call_stack() {
  let stack = &mut CALL_STACK.write().unwrap();
  (*stack).drop_last_mut();
}

pub fn clear_stack() {
  let stack = &mut CALL_STACK.write().unwrap();
  loop {
    if stack.is_empty() {
      break;
    }
    (*stack).drop_last_mut();
  }
}

pub fn get_gen_stack() -> CallStackVec {
  let stack = CALL_STACK.read().unwrap();
  stack.to_owned()
}
