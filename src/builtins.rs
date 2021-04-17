mod effects;
mod lists;
mod maps;
mod math;
mod sets;
mod syntax;

use crate::primes::{CalcitData, CalcitItems, CalcitScope};
use crate::program::ProgramCodeData;

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
      | "quote"
      | "quasiquote"
      | "eval"
      | "macroexpand"
      | "macroexpand-1"
      | "macroexpand-all"
  )
}

pub fn handle_proc(name: &str, args: &CalcitItems) -> Result<CalcitData, String> {
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
  nodes: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program: &ProgramCodeData,
) -> Result<CalcitData, String> {
  match name {
    "defn" => syntax::defn(nodes, scope, file_ns, program),
    "eval" => syntax::eval(nodes, scope, file_ns, program),
    "defmacro" => syntax::defmacro(nodes, scope, file_ns, program),
    "quote" => syntax::quote(nodes, scope, file_ns, program),
    "if" => syntax::syntax_if(nodes, scope, file_ns, program),
    "&let" => syntax::syntax_let(nodes, scope, file_ns, program),
    a => Err(format!("TODO syntax: {}", a)),
  }
}
