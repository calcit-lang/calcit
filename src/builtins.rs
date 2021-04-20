mod effects;
mod lists;
mod logics;
mod maps;
mod math;
mod meta;
mod sets;
mod strings;
mod syntax;

use crate::primes::{CalcitData, CalcitItems, CalcitScope};
use crate::program::ProgramCodeData;

pub fn is_proc_name(s: &str) -> bool {
  matches!(
    s,
    // meta
    "type-of"
      | "recur"
      | "format-to-lisp"
      | "gensym"
      | "&reset-gensym-index!"
      | "&get-calcit-running-mode"
      | "generate-id!"
      | "display-stack"
      | "parse-cirru"
      | "write-cirru"
      | "parse-cirru-edn"
      | "write-cirru-edn"
      // effects
      | "echo"
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
      // strings
      | "&str-concat"
      | "trim"
      // lists
      | "[]"
      | "empty?"
      | "count"
      | "nth"
      | "slice"
      | "append"
      | "prepend"
      | "rest"
      | "butlast"
      | "concat"
      // maps
      | "&{}"
      | "assoc"
      | "&get"
      | "contains?"
      // sets
      | "#{}"
  )
}

pub fn handle_proc(name: &str, args: &CalcitItems) -> Result<CalcitData, String> {
  match name {
    // meta
    "type-of" => meta::type_of(args),
    "recur" => meta::recur(args),
    "format-to-lisp" => meta::format_to_lisp(args),
    "gensym" => meta::gensym(args),
    "&reset-gensym-index!" => meta::reset_gensym_index(args),
    "&get-calcit-running-mode" => meta::get_calcit_running_mode(args),
    "generate-id!" => meta::generate_id(args),
    "display-stack" => meta::display_stack(args),
    "parse-cirru" => meta::parse_cirru(args),
    "write-cirru" => meta::write_cirru(args),
    "parse-cirru-edn" => meta::parse_cirru_edn(args),
    "write-cirru-edn" => meta::write_cirru_edn(args),
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
    // strings
    "&str-concat" => strings::binary_str_concat(args),
    "trim" => strings::trim(args),
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
    "concat" => lists::concat(args),
    // maps
    "&{}" => maps::call_new_map(args),
    "assoc" => maps::assoc(args),
    "&get" => maps::map_get(args),
    "contains?" => maps::contains_ques(args),
    // sets
    "#{}" => sets::new_set(args),
    a => Err(format!("TODO proc: {}", a)),
  }
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
      | "quote-replace" // alias for quasiquote
      | "eval"
      | "macroexpand"
      | "macroexpand-1"
      | "macroexpand-all"
      | "foldl" // for performance
  )
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
    "quasiquote" => syntax::quasiquote(nodes, scope, file_ns, program),
    "quote-replace" => syntax::quasiquote(nodes, scope, file_ns, program), // alias
    "if" => syntax::syntax_if(nodes, scope, file_ns, program),
    "&let" => syntax::syntax_let(nodes, scope, file_ns, program),
    "foldl" => syntax::foldl(nodes, scope, file_ns, program),
    "macroexpand" => syntax::macroexpand(nodes, scope, file_ns, program),
    "macroexpand-1" => syntax::macroexpand_1(nodes, scope, file_ns, program),
    a => Err(format!("TODO syntax: {}", a)),
  }
}
