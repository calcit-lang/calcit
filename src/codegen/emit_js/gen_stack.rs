use std::sync::RwLock;

use im_ternary_tree::TernaryTreeList;

use crate::call_stack::{CalcitStack, CallStackVec, StackKind};
use crate::primes::{Calcit, CalcitItems};

lazy_static! {
  static ref CALL_STACK: RwLock<TernaryTreeList<CalcitStack>> = RwLock::new(TernaryTreeList::Empty);
}

pub fn push_call_stack(ns: &str, def: &str, kind: StackKind, code: Calcit, args: &CalcitItems) {
  let mut stack = CALL_STACK.write().unwrap();
  *stack = stack.push(CalcitStack {
    ns: ns.to_owned(),
    def: def.to_owned(),
    code,
    args: args.to_owned(),
    kind,
  })
}

pub fn pop_call_stack() {
  let mut stack = CALL_STACK.write().unwrap();
  if !stack.is_empty() {
    match stack.butlast() {
      Ok(v) => *stack = v,
      Err(e) => {
        println!("stack problem, {}", e)
      }
    }
  }
}

pub fn clear_stack() {
  let mut stack = CALL_STACK.write().unwrap();
  *stack = TernaryTreeList::Empty;
}

pub fn get_gen_stack() -> CallStackVec {
  let stack = CALL_STACK.read().unwrap();
  stack.to_owned()
}
