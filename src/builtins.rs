mod effects;
mod lists;
mod maps;
mod math;
mod sets;
mod syntax;

use crate::primes::{CalcitData, CalcitScope};

pub fn is_proc_name(s: &str) -> bool {
  matches!(s, "echo" | "echo-values" | "[]" | "&{}" | "#{}" | "&+")
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
    "echo" => effects::echo(args),
    "echo-values" => effects::echo_values(args),
    "&+" => math::binary_add(args),
    "[]" => lists::new_list(args),
    "&{}" => maps::call_new_map(args),
    "#{}" => sets::new_set(args),
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
    "defmacro" => syntax::defmacro(nodes, scope),
    a => Err(format!("TODO syntax: {}", a)),
  }
}
