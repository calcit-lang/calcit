use crate::data::cirru;
use crate::data::edn;
use crate::primes::{Calcit, CalcitItems};
use cirru_edn::Edn;
use std::collections::HashMap;
use std::fs;

#[derive(Debug, PartialEq, Clone)]
pub struct CalcitStack {
  pub ns: String,
  pub def: String,
  pub code: Calcit, // built in functions may not contain code
  pub args: CalcitItems,
  pub kind: StackKind,
}

#[derive(Debug, PartialEq, Clone)]
pub enum StackKind {
  Fn,
  Proc,
  Macro,
  Syntax,  // rarely used
  Codegen, // track preprocessing
}

pub type CallStackVec = rpds::VectorSync<CalcitStack>;

// TODO impl fmt

/// create new entry to the tree
pub fn extend_call_stack(stack: &CallStackVec, ns: &str, def: &str, kind: StackKind, code: Calcit, args: &CalcitItems) -> CallStackVec {
  let mut s2 = stack.to_owned();
  s2.push_back_mut(CalcitStack {
    ns: ns.to_owned(),
    def: def.to_owned(),
    code,
    args: args.to_owned(),
    kind,
  });
  s2
}

// show simplified version of stack
pub fn show_stack(stack: &CallStackVec) {
  println!("\ncall stack:");
  for idx in 0..stack.len() {
    let s = &stack[stack.len() - idx - 1];
    let is_macro = s.kind == StackKind::Macro;
    println!("  {}/{}{}", s.ns, s.def, if is_macro { "\t ~macro" } else { "" });
  }
}

pub fn display_stack(failure: &str, stack: &CallStackVec) -> Result<(), String> {
  println!("\ncall stack:");

  for idx in 0..stack.len() {
    let s = &stack[stack.len() - idx - 1];
    let is_macro = s.kind == StackKind::Macro;
    println!("  {}/{}{}", s.ns, s.def, if is_macro { "\t ~macro" } else { "" });
  }

  let mut stack_list: Vec<Edn> = Vec::with_capacity(stack.len());
  for idx in 0..stack.len() {
    let s = &stack[stack.len() - idx - 1];
    let mut info: HashMap<Edn, Edn> = HashMap::with_capacity(4);
    info.insert(Edn::kwd("def"), Edn::str(format!("{}/{}", s.ns, s.def)));
    info.insert(Edn::kwd("code"), Edn::Quote(cirru::calcit_to_cirru(&s.code)?));
    let mut args: Vec<Edn> = Vec::with_capacity(s.args.len());
    for a in &s.args {
      args.push(edn::calcit_to_edn(a)?);
    }
    info.insert(Edn::kwd("args"), Edn::List(args));
    info.insert(Edn::kwd("kind"), Edn::kwd(&name_kind(&s.kind)));

    stack_list.push(Edn::Map(info))
  }

  let mut data: HashMap<Edn, Edn> = HashMap::with_capacity(2);
  data.insert(Edn::kwd("message"), Edn::str(failure));
  data.insert(Edn::kwd("stack"), Edn::List(stack_list));
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
