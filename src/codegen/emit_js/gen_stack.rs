use std::sync::Mutex;

use crate::calcit::Calcit;
use crate::call_stack::{CalcitStack, CallStackList, StackKind};

lazy_static! {
  static ref CALL_STACK: Mutex<rpds::ListSync<CalcitStack>> = Mutex::new(rpds::List::new_sync());
}

pub fn push_call_stack(ns: &str, def: &str, kind: StackKind, code: Calcit, args: &[Calcit]) {
  let mut stack = CALL_STACK.lock().expect("open call stack");
  stack.push_front_mut(CalcitStack {
    ns: ns.into(),
    def: def.into(),
    code,
    args: args.to_owned(),
    kind,
  })
}

pub fn pop_call_stack() {
  let mut stack = CALL_STACK.lock().expect("open call stack");
  if !stack.is_empty() {
    let xs = stack.drop_first();
    match xs {
      Some(v) => *stack = v,
      None => {
        eprintln!("empty stack, nothing to pop")
      }
    }
  }
}

pub fn clear_stack() {
  let mut stack = CALL_STACK.lock().expect("open call stack");
  *stack = rpds::List::new_sync();
}

pub fn get_gen_stack() -> CallStackList {
  let stack = CALL_STACK.lock().expect("read call stack");
  CallStackList(stack.to_owned())
}
