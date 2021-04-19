use crate::primes::{CalcitData, CalcitItems};
use std::sync::Mutex;

#[derive(Debug, PartialEq)]
pub struct CalcitStack {
  pub ns: String,
  pub def: String,
  pub code: Option<CalcitData>, // built in functions may not contain code
  pub args: CalcitItems,
}

// TODO impl fmt

lazy_static! {
  static ref CALL_STACK: Mutex<Vec<CalcitStack>> = Mutex::new(vec![]);
}

pub fn push_call_stack(ns: &str, def: &str, code: &Option<CalcitData>, args: &CalcitItems) {
  let stack = &mut CALL_STACK.lock().unwrap();
  stack.push(CalcitStack {
    ns: ns.to_string(),
    def: def.to_string(),
    code: code.clone(),
    args: args.clone(),
  })
}

pub fn pop_call_stack() {
  let stack = &mut CALL_STACK.lock().unwrap();
  stack.pop();
}

pub fn display_stack() {
  let stack: &Vec<CalcitStack> = &mut CALL_STACK.lock().unwrap();
  for s in stack {
    println!("stack: {:?}", s);
  }
}
