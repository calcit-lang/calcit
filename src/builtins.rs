mod effects;
mod lists;
mod logics;
mod maps;
mod math;
mod sets;
mod syntax;

use crate::primes::{CalcitData, CalcitItems, CalcitScope};
use crate::program::ProgramCodeData;

pub fn is_proc_name(s: &str) -> bool {
  matches!(
    s,
    // effecrs
    "echo"
      | "echo-values"
      | "raise"
      // logics
      | "&="
      | "&<"
      | "&>"
      | "not"
      // math
      | "&+"
      | "&-"
      | "&*"
      | "&/"
      | "round"
      | "fractional" // logics
      // lists
      | "[]"
      | "&{}"
      | "#{}"
      | "empty?"
      | "count"
      | "nth"
      | "slice"
      | "append"
      | "prepend"
      | "rest"
      | "butlast"
  )
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
    // effects
    "echo" => effects::echo(args),
    "echo-values" => effects::echo_values(args),
    "raise" => effects::raise(args),
    // logics
    "&=" => logics::binary_equal(args),
    "&<" => logics::binary_less(args),
    "&>" => logics::binary_greater(args),
    "not" => logics::not(args),
    // math
    "&+" => math::binary_add(args),
    "&-" => math::binary_minus(args),
    "&*" => math::binary_multiply(args),
    "&/" => math::binary_divide(args),
    // lists
    "[]" => lists::new_list(args),
    "empty?" => lists::empty_ques(args),
    "count" => lists::count(args),
    "nth" => lists::nth(args),
    "slice" => lists::slice(args),
    "append" => lists::append(args),
    "prepend" => lists::prepend(args),
    "rest" => lists::rest(args),
    "butlast" => lists::butlast(args),
    // maps
    "&{}" => maps::call_new_map(args),
    // sets
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
