pub mod effects;
mod lists;
mod logics;
mod maps;
mod math;
pub mod meta;
mod records;
mod refs;
mod regexes;
mod sets;
mod strings;
mod syntax;

use crate::data::json;
use crate::primes::{Calcit, CalcitItems, CalcitScope};
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
      | "turn-symbol"
      | "turn-keyword"
      // effects
      | "echo"
      | "println" // alias for echo
      | "echo-values"
      | "raise"
      | "cpu-time"
      | "quit"
      | "get-env"
      | "&get-calcit-backend"
      | "read-file"
      | "write-file"
      | "format-time"
      | "parse-time"
      | "now!"
      // logics
      | "&="
      | "&<"
      | "&>"
      | "not"
      | "identical?"
      // math
      | "&+"
      | "&-"
      | "&*"
      | "&/"
      | "round"
      | "fractional" // TODO
      | "rand"
      | "rand-int"
      | "floor"
      | "rem"
      | "sin"
      | "cos"
      | "pow"
      | "ceil"
      | "sqrt"
      | "integer?"
      // strings
      | "&str-concat"
      | "trim"
      | "&str"
      | "turn-string"
      | "split"
      | "format-number"
      | "replace"
      | "split-lines"
      | "substr"
      | "compare-string"
      | "str-find"
      | "starts-with?"
      | "ends-with?"
      | "get-char-code"
      | "re-matches"
      | "re-find"
      | "parse-float"
      | "pr-str"
      | "re-find-index"
      | "re-find-all"
      | "blank?"
      | "escape"
      // lists
      | "[]"
      | "'" // used as an alias for `[]`, experimental
      | "empty?"
      | "count"
      | "nth"
      | "slice"
      | "append"
      | "prepend"
      | "rest"
      | "butlast"
      | "concat"
      | "range"
      | "reverse"
      | "first"
      | "assoc-before"
      | "assoc-after"
      // maps
      | "&{}"
      | "assoc"
      | "&get"
      | "contains?"
      | "dissoc"
      | "&merge"
      | "includes?"
      | "to-pairs"
      | "&merge-non-nil"
      // sets
      | "#{}"
      | "&include"
      | "&exclude"
      | "&difference"
      | "&union"
      | "&intersection"
      | "set->list"
      // json
      | "parse-json"
      | "stringify-json"
      // refs
      | "deref"
      | "add-watch"
      | "remove-watch"
      // records
      | "new-record"
      | "&%{}"
      | "make-record" // TODO switch to (into-record xs r) ?
      | "get-record-name"
      | "turn-map"
      | "relevant-record?" // regexs
  )
}

pub fn handle_proc(name: &str, args: &CalcitItems) -> Result<Calcit, String> {
  match name {
    // meta
    "type-of" => meta::type_of(args),
    "recur" => meta::recur(args),
    "format-to-lisp" => meta::format_to_lisp(args),
    "gensym" => meta::gensym(args),
    "&reset-gensym-index!" => meta::reset_gensym_index(args),
    "&get-calcit-running-mode" => effects::calcit_running_mode(args),
    "generate-id!" => meta::generate_id(args),
    "display-stack" => meta::display_stack(args),
    "parse-cirru" => meta::parse_cirru(args),
    "write-cirru" => meta::write_cirru(args),
    "parse-cirru-edn" => meta::parse_cirru_edn(args),
    "write-cirru-edn" => meta::write_cirru_edn(args),
    "turn-symbol" => meta::turn_symbol(args),
    "turn-keyword" => meta::turn_keyword(args),
    // effects
    "echo" => effects::echo(args),
    "println" => effects::echo(args), // alias
    "echo-values" => effects::echo_values(args),
    "raise" => effects::raise(args),
    "cpu-time" => effects::cpu_time(args),
    "quit" => effects::quit(args),
    "get-env" => effects::get_env(args),
    "&get-calcit-backend" => effects::call_get_calcit_backend(args),
    "read-file" => effects::read_file(args),
    "write-file" => effects::write_file(args),
    "parse-time" => effects::parse_time(args),
    "format-time" => effects::format_time(args),
    "now!" => effects::now_bang(args),
    // logics
    "&=" => logics::binary_equal(args),
    "&<" => logics::binary_less(args),
    "&>" => logics::binary_greater(args),
    "not" => logics::not(args),
    // in Rust, no real pointer `identical?`, fallback to value equal
    "identical?" => logics::binary_equal(args),
    // math
    "&+" => math::binary_add(args),
    "&-" => math::binary_minus(args),
    "&*" => math::binary_multiply(args),
    "&/" => math::binary_divide(args),
    "rand" => math::rand(args),
    "rand-int" => math::rand_int(args),
    "floor" => math::floor(args),
    "rem" => math::rem(args),
    "sin" => math::sin(args),
    "cos" => math::cos(args),
    "pow" => math::pow(args),
    "ceil" => math::ceil(args),
    "sqrt" => math::sqrt(args),
    "round" => math::round(args),
    "fractional" => math::fractional(args),
    "integer?" => math::integer_ques(args),
    // strings
    "&str-concat" => strings::binary_str_concat(args),
    "trim" => strings::trim(args),
    "&str" => strings::call_str(args),
    "turn-string" => strings::turn_string(args),
    "split" => strings::split(args),
    "format-number" => strings::format_number(args),
    "replace" => strings::replace(args),
    "split-lines" => strings::split_lines(args),
    "substr" => strings::substr(args),
    "compare-string" => strings::compare_string(args),
    "str-find" => strings::str_find(args),
    "starts-with?" => strings::starts_with_ques(args),
    "ends-with?" => strings::ends_with_ques(args),
    "get-char-code" => strings::get_char_code(args),
    "parse-float" => strings::parse_float(args),
    "pr-str" => strings::pr_str(args),
    "blank?" => strings::blank_ques(args),
    "escape" => strings::escape(args),
    // regex
    "re-matches" => regexes::re_matches(args),
    "re-find" => regexes::re_find(args),
    "re-find-index" => regexes::re_find_index(args),
    "re-find-all" => regexes::re_find_all(args),
    // lists
    "[]" => lists::new_list(args),
    "'" => lists::new_list(args), // alias
    "empty?" => lists::empty_ques(args),
    "count" => lists::count(args),
    "nth" => lists::nth(args),
    "slice" => lists::slice(args),
    "append" => lists::append(args),
    "prepend" => lists::prepend(args),
    "rest" => lists::rest(args),
    "butlast" => lists::butlast(args),
    "concat" => lists::concat(args),
    "range" => lists::range(args),
    "reverse" => lists::reverse(args),
    "first" => lists::first(args),
    "assoc-before" => lists::assoc_before(args),
    "assoc-after" => lists::assoc_after(args),
    // maps
    "&{}" => maps::call_new_map(args),
    "assoc" => maps::assoc(args),
    "&get" => maps::map_get(args),
    "contains?" => maps::contains_ques(args),
    "dissoc" => maps::dissoc(args),
    "&merge" => maps::call_merge(args),
    "includes?" => maps::includes_ques(args),
    "to-pairs" => maps::to_pairs(args),
    "&merge-non-nil" => maps::call_merge_non_nil(args),
    // sets
    "#{}" => sets::new_set(args),
    "&include" => sets::call_include(args),
    "&exclude" => sets::call_exclude(args),
    "&difference" => sets::call_difference(args),
    "&union" => sets::call_union(args),
    "&intersection" => sets::call_intersection(args),
    "set->list" => sets::set_to_list(args),
    // json
    "parse-json" => json::parse_json(args),
    "stringify-json" => json::stringify_json(args),
    // refs
    "deref" => refs::deref(args),
    "add-watch" => refs::add_watch(args),
    "remove-watch" => refs::remove_watch(args),
    // records
    "new-record" => records::new_record(args),
    "&%{}" => records::call_record(args),
    "make-record" => records::record_from_map(args), // TODO switch to (into-record xs r) ?
    "get-record-name" => records::get_record_name(args),
    "turn-map" => records::turn_map(args),
    "relevant-record?" => records::relevant_record_ques(args),
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
      | "foldl-shortcut" // for performance
      | "try"
      | "sort" // TODO need better solution
      | "defatom"
      | "reset!"
  )
}

pub fn handle_syntax(
  name: &str,
  nodes: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program: &ProgramCodeData,
) -> Result<Calcit, String> {
  match name {
    "defn" => syntax::defn(nodes, scope, file_ns, program),
    "eval" => syntax::eval(nodes, scope, file_ns, program),
    "defmacro" => syntax::defmacro(nodes, scope, file_ns, program),
    "quote" => syntax::quote(nodes, scope, file_ns, program),
    "quasiquote" => syntax::quasiquote(nodes, scope, file_ns, program),
    "quote-replace" => syntax::quasiquote(nodes, scope, file_ns, program), // alias
    "if" => syntax::syntax_if(nodes, scope, file_ns, program),
    "&let" => syntax::syntax_let(nodes, scope, file_ns, program),
    "foldl" => lists::foldl(nodes, scope, file_ns, program),
    "foldl-shortcut" => lists::foldl_shortcut(nodes, scope, file_ns, program),
    "macroexpand" => syntax::macroexpand(nodes, scope, file_ns, program),
    "macroexpand-1" => syntax::macroexpand_1(nodes, scope, file_ns, program),
    "macroexpand-all" => syntax::macroexpand_all(nodes, scope, file_ns, program),
    "try" => syntax::call_try(nodes, scope, file_ns, program),
    "sort" => lists::sort(nodes, scope, file_ns, program),
    // "define reference" although it uses a confusing name "atom"
    "defatom" => refs::defatom(nodes, scope, file_ns, program),
    "reset!" => refs::reset_bang(nodes, scope, file_ns, program),
    a => Err(format!("TODO syntax: {}", a)),
  }
}

// detects extra javascript things in js mode
pub fn is_js_syntax_procs(s: &str) -> bool {
  matches!(
    s,
    "aget"
      | "aset"
      | "new"
      | "set!"
      | "exists?"
      | "instance?"
      | "to-calcit-data"
      | "to-js-data"
      | "to-cirru-edn"
      | "extract-cirru-edn"
      | "timeout-call"
      | "load-console-formatter!"
  )
}
