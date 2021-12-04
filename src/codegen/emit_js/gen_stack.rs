use std::sync::RwLock;

use crate::call_stack::{CalcitStack, CallStackList, StackKind};
use crate::primes::{Calcit, CalcitItems};

lazy_static! {
  static ref CALL_STACK: RwLock<rpds::ListSync<CalcitStack>> = RwLock::new(rpds::List::new_sync());
}

pub fn push_call_stack(ns: &str, def: &str, kind: StackKind, code: Calcit, args: &CalcitItems) {
  let mut stack = CALL_STACK.write().unwrap();
  stack.push_front_mut(CalcitStack {
    ns: ns.into(),
    def: def.into(),
    code,
    args: Box::new(args.to_owned()),
    kind,
  })
}

pub fn pop_call_stack() {
  let mut stack = CALL_STACK.write().unwrap();
  if !stack.is_empty() {
    match stack.drop_first() {
      Some(v) => *stack = v,
      None => {
        println!("empty stack")
      }
    }
  }
}

pub fn clear_stack() {
  let mut stack = CALL_STACK.write().unwrap();
  *stack = rpds::List::new_sync();
}

pub fn get_gen_stack() -> CallStackList {
  let stack = CALL_STACK.read().unwrap();
  stack.to_owned()
}
