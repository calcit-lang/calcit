use crate::data::cirru;
use crate::data::edn;
use crate::primes::{CalcitData, CalcitItems};
use cirru_edn::{write_cirru_edn, CirruEdn, CirruEdn::*};
use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;

#[derive(Debug, PartialEq)]
pub struct CalcitStack {
  pub ns: String,
  pub def: String,
  pub code: Option<CalcitData>, // built in functions may not contain code
  pub args: CalcitItems,
  pub kind: StackKind,
}

#[derive(Debug, PartialEq)]
pub enum StackKind {
  Fn,
  Proc,
  Macro,
  // Syntax, // rarely used
}

// TODO impl fmt

lazy_static! {
  static ref CALL_STACK: Mutex<Vec<CalcitStack>> = Mutex::new(vec![]);
}

pub fn push_call_stack(
  ns: &str,
  def: &str,
  kind: StackKind,
  code: &Option<CalcitData>,
  args: &CalcitItems,
) {
  let stack = &mut CALL_STACK.lock().unwrap();
  stack.push(CalcitStack {
    ns: ns.to_string(),
    def: def.to_string(),
    code: code.clone(),
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
    println!(
      "  {}/{}{}",
      s.ns,
      s.def,
      if is_macro { "\t ~macro" } else { "" }
    );
  }
}

pub fn display_stack(failure: &str) {
  let stack: &Vec<CalcitStack> = &mut CALL_STACK.lock().unwrap();
  println!("\ncall stack:");

  for idx in 0..stack.len() {
    let s = &stack[stack.len() - idx - 1];
    let is_macro = s.kind == StackKind::Macro;
    println!(
      "  {}/{}{}",
      s.ns,
      s.def,
      if is_macro { "\t ~macro" } else { "" }
    );
  }

  let mut stack_list: Vec<CirruEdn> = vec![];
  for idx in 0..stack.len() {
    let s = &stack[stack.len() - idx - 1];
    let mut info: HashMap<CirruEdn, CirruEdn> = HashMap::new();
    info.insert(
      CirruEdnKeyword(String::from("def")),
      CirruEdnString(format!("{}/{}", s.ns, s.def)),
    );
    info.insert(
      CirruEdnKeyword(String::from("code")),
      match &s.code {
        Some(code) => CirruEdnQuote(cirru::calcit_to_cirru(code)),
        None => CirruEdnNil,
      },
    );
    let mut args: Vec<CirruEdn> = vec![];
    for a in &s.args {
      args.push(edn::calcit_to_edn(a));
    }
    info.insert(CirruEdnKeyword(String::from("args")), CirruEdnList(args));
    info.insert(
      CirruEdnKeyword(String::from("kind")),
      CirruEdnKeyword(name_kind(&s.kind)),
    );

    stack_list.push(CirruEdnMap(info))
  }

  let mut data: HashMap<CirruEdn, CirruEdn> = HashMap::new();
  data.insert(
    CirruEdnKeyword(String::from("message")),
    CirruEdnString(failure.to_string()),
  );
  data.insert(
    CirruEdnKeyword(String::from("stack")),
    CirruEdnList(stack_list),
  );
  let content = write_cirru_edn(CirruEdnMap(data));
  let _ = fs::write(ERROR_SNAPSHOT, content);
  println!("\nrun `cat {}` to read stack details.", ERROR_SNAPSHOT);
}

const ERROR_SNAPSHOT: &str = ".calcit-error.cirru";

fn name_kind(k: &StackKind) -> String {
  match k {
    StackKind::Fn => String::from("fn"),
    StackKind::Proc => String::from("proc"),
    StackKind::Macro => String::from("macro"),
  }
}
