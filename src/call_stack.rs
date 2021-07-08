use crate::data::cirru;
use crate::data::edn;
use crate::primes::{Calcit, CalcitItems};
use cirru_edn::Edn;
use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;

#[derive(Debug, PartialEq)]
pub struct CalcitStack {
  pub ns: String,
  pub def: String,
  pub code: Calcit, // built in functions may not contain code
  pub args: CalcitItems,
  pub kind: StackKind,
}

#[derive(Debug, PartialEq)]
pub enum StackKind {
  Fn,
  Proc,
  Macro,
  Syntax,  // rarely used
  Codegen, // track preprocessing
}

// TODO impl fmt

lazy_static! {
  static ref CALL_STACK: Mutex<Vec<CalcitStack>> = Mutex::new(vec![]);
}

pub fn push_call_stack(ns: &str, def: &str, kind: StackKind, code: Calcit, args: &CalcitItems) {
  let stack = &mut CALL_STACK.lock().unwrap();
  stack.push(CalcitStack {
    ns: ns.to_owned(),
    def: def.to_owned(),
    code,
    args: args.clone(),
    kind,
  })
}

pub fn pop_call_stack() {
  let stack = &mut CALL_STACK.lock().unwrap();
  stack.pop();
}

// show simplified version of stack
pub fn show_stack() {
  let stack: &Vec<CalcitStack> = &mut CALL_STACK.lock().unwrap();
  println!("\ncall stack:");
  for idx in 0..stack.len() {
    let s = &stack[stack.len() - idx - 1];
    let is_macro = s.kind == StackKind::Macro;
    println!("  {}/{}{}", s.ns, s.def, if is_macro { "\t ~macro" } else { "" });
  }
}

pub fn clear_stack() {
  let stack = &mut CALL_STACK.lock().unwrap();
  stack.clear();
}

pub fn display_stack(failure: &str) -> Result<(), String> {
  let stack: &Vec<CalcitStack> = &mut CALL_STACK.lock().unwrap();
  println!("\ncall stack:");

  for idx in 0..stack.len() {
    let s = &stack[stack.len() - idx - 1];
    let is_macro = s.kind == StackKind::Macro;
    println!("  {}/{}{}", s.ns, s.def, if is_macro { "\t ~macro" } else { "" });
  }

  let mut stack_list: Vec<Edn> = vec![];
  for idx in 0..stack.len() {
    let s = &stack[stack.len() - idx - 1];
    let mut info: HashMap<Edn, Edn> = HashMap::new();
    info.insert(
      Edn::Keyword(String::from("def")),
      Edn::Str(format!("{}/{}", s.ns, s.def)),
    );
    info.insert(
      Edn::Keyword(String::from("code")),
      Edn::Quote(cirru::calcit_to_cirru(&s.code)),
    );
    let mut args: Vec<Edn> = vec![];
    for a in &s.args {
      args.push(edn::calcit_to_edn(a)?);
    }
    info.insert(Edn::Keyword(String::from("args")), Edn::List(args));
    info.insert(Edn::Keyword(String::from("kind")), Edn::Keyword(name_kind(&s.kind)));

    stack_list.push(Edn::Map(info))
  }

  let mut data: HashMap<Edn, Edn> = HashMap::new();
  data.insert(Edn::Keyword(String::from("message")), Edn::Str(failure.to_owned()));
  data.insert(Edn::Keyword(String::from("stack")), Edn::List(stack_list));
  let content = cirru_edn::format(&Edn::Map(data), true)?;
  let _ = fs::write(ERROR_SNAPSHOT, content);
  println!("\nrun `cat {}` to read stack details.", ERROR_SNAPSHOT);
  Ok(())
}

const ERROR_SNAPSHOT: &str = ".calcit-error.cirru";

fn name_kind(k: &StackKind) -> String {
  match k {
    StackKind::Fn => String::from("fn"),
    StackKind::Proc => String::from("proc"),
    StackKind::Macro => String::from("macro"),
    StackKind::Syntax => String::from("syntax"),
    StackKind::Codegen => String::from("codegen"),
  }
}
