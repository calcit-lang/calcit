mod effect;
mod math;
mod syntax;

use crate::primes::{CalcitData, CalcitScope};

pub fn is_proc_name(s: &str) -> bool {
  matches!(s, "echo" | "&+")
}

pub fn is_syntax_name(s: &str) -> bool {
  matches!(
    s,
    "defn"
      | "defmacro"
      | "if"
      | "&let"
      | ";"
      | "quote"
      | "quasiquote"
      | "eval"
      | "macroexpand"
      | "macroexpand-1"
      | "macroexpand-all"
  )
}

pub fn handle_proc(name: &str, args: im::Vector<CalcitData>) -> Result<CalcitData, String> {
  match name {
    "echo" => effect::echo(args),
    "&+" => math::binary_add(args),
    a => Err(format!("TODO proc: {}", a)),
  }
}

pub fn handle_syntax(
  name: &str,
  nodes: im::Vector<CalcitData>,
  scope: CalcitScope,
) -> Result<CalcitData, String> {
  match name {
    "defn" => syntax::defn(nodes, scope),
    a => Err(format!("TODO syntax: {}", a)),
  }
}
