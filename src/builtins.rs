mod effect;
mod math;
mod syntax;

use crate::primes::CalcitData::*;
use crate::primes::{CalcitData, CalcitScope};

pub fn is_proc_name(s: &str) -> bool {
  match s {
    "echo" => true,
    "&+" => true,
    _ => false,
  }
}

pub fn is_syntax_name(s: &str) -> bool {
  match s {
    "defn" => true,
    "defmacro" => true,
    "if" => true,
    "&let" => true,
    ";" => true,
    "quote" => true,
    "quasiquote" => true,
    "eval" => true,
    "macroexpand" => true,
    "macroexpand-1" => true,
    "macroexpand-all" => true,
    _ => false,
  }
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
