pub mod effects;
mod lists;
mod logics;
mod maps;
mod math;
pub mod meta;
mod records;
mod refs;
mod sets;
mod strings;
pub mod syntax;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::call_stack::CallStackList;
use crate::primes::{Calcit, CalcitErr, CalcitItems, CalcitScope, CalcitSyntax};

pub type FnType = fn(xs: &CalcitItems, call_stack: &CallStackList) -> Result<Calcit, CalcitErr>;
pub type SyntaxType = fn(expr: &CalcitItems, scope: &CalcitScope, file_ns: &str) -> Result<Calcit, CalcitErr>;

lazy_static! {
  static ref IMPORTED_PROCS: RwLock<HashMap<Arc<str>, FnType>> = RwLock::new(HashMap::new());
}

pub fn is_proc_name(s: &str) -> bool {
  let builtin = matches!(
    s,
    // meta
    "type-of"
      | "recur"
      | "format-to-lisp"
      | "format-to-cirru"
      | "gensym"
      | "&reset-gensym-index!"
      | "&get-calcit-running-mode"
      | "generate-id!"
      | "turn-symbol"
      | "turn-keyword"
      | "&compare"
      | "&get-os"
      | "&format-ternary-tree"
      | "&buffer"
      // tuples
      | "::" // unstable
      | "&tuple:nth"
      | "&tuple:assoc"
      // effects
      | "&display-stack"
      | "raise"
      | "quit!"
      | "get-env"
      | "&get-calcit-backend"
      | "read-file"
      | "write-file"
      // external format
      | "parse-cirru"
      | "format-cirru"
      | "parse-cirru-edn"
      | "format-cirru-edn"
      // time
      | "cpu-time"
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
      | "floor"
      | "sin"
      | "cos"
      | "pow"
      | "ceil"
      | "sqrt"
      | "round?"
      | "&number:fract"
      | "&number:rem"
      | "&number:format"
      | "bit-shl"
      | "bit-shr"
      // strings
      | "&str:concat"
      | "trim"
      | "&str"
      | "turn-string"
      | "split"
      | "split-lines"
      | "starts-with?"
      | "ends-with?"
      | "get-char-code"
      | "char-from-code"
      | "pr-str"
      | "parse-float"
      | "blank?"
      | "&str:compare"
      | "&str:replace"
      | "&str:slice"
      | "&str:find-index"
      | "&str:escape"
      | "&str:count"
      | "&str:empty?"
      | "&str:contains?"
      | "&str:includes?"
      | "&str:nth"
      | "&str:first"
      | "&str:rest"
      | "&str:pad-left"
      | "&str:pad-right"
      // lists
      | "[]"
      | "'" // used as an alias for `[]`, experimental
      | "append"
      | "prepend"
      | "butlast"
      | "range"
      | "sort"
      | "foldl"
      | "foldl-shortcut"
      | "foldr-shortcut"
      | "&list:reverse"
      | "&list:concat"
      | "&list:count"
      | "&list:empty?"
      | "&list:slice"
      | "&list:assoc-before"
      | "&list:assoc-after"
      | "&list:contains?"
      | "&list:includes?"
      | "&list:nth"
      | "&list:first"
      | "&list:rest"
      | "&list:assoc"
      | "&list:dissoc"
      | "&list:to-set"
      | "&list:distinct"
      // maps
      | "&{}"
      | "&merge"
      | "to-pairs"
      | "&merge-non-nil"
      | "&map:get"
      | "&map:dissoc"
      | "&map:to-list"
      | "&map:count"
      | "&map:empty?"
      | "&map:contains?"
      | "&map:includes?"
      | "&map:first"
      | "&map:rest"
      | "&map:assoc"
      | "&map:diff-new"
      | "&map:diff-keys"
      | "&map:common-keys"
      // sets
      | "#{}"
      | "&include"
      | "&exclude"
      | "&difference"
      | "&union"
      | "&set:intersection"
      | "&set:to-list"
      | "&set:count"
      | "&set:empty?"
      | "&set:includes?"
      | "&set:first"
      | "&set:rest"
      | "&set:assoc"
      // refs
      | "deref"
      | "add-watch"
      | "remove-watch"
      // records
      | "new-record"
      | "&%{}"
      | "&record:matches?"
      | "&record:from-map"
      | "&record:get-name"
      | "&record:to-map"
      | "&record:count"
      | "&record:contains?"
      | "&record:get"
      | "&record:assoc"
      | "&record:extend-as"
  );
  if builtin {
    true
  } else {
    let ps = IMPORTED_PROCS.read().unwrap();
    ps.contains_key(s)
  }
}

/// make sure that stack information attached in errors from procs
pub fn handle_proc(name: &str, args: &CalcitItems, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  handle_proc_internal(name, args, call_stack).map_err(|e| {
    if e.stack.is_empty() {
      let mut e2 = e;
      e2.stack = call_stack.to_owned();
      e2
    } else {
      e
    }
  })
}

fn handle_proc_internal(name: &str, args: &CalcitItems, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  match name {
    // meta
    "type-of" => meta::type_of(args),
    "recur" => meta::recur(args),
    "format-to-lisp" => meta::format_to_lisp(args),
    "format-to-cirru" => meta::format_to_cirru(args),
    "gensym" => meta::gensym(args),
    "&reset-gensym-index!" => meta::reset_gensym_index(args),
    "&get-calcit-running-mode" => effects::calcit_running_mode(args),
    "generate-id!" => meta::generate_id(args),
    "turn-symbol" => meta::turn_symbol(args),
    "turn-keyword" => meta::turn_keyword(args),
    "&compare" => meta::native_compare(args),
    "&get-os" => meta::get_os(args),
    "&format-ternary-tree" => meta::format_ternary_tree(args),
    "&buffer" => meta::buffer(args),
    // tuple
    "::" => meta::new_tuple(args), // unstable solution for the name
    "&tuple:nth" => meta::tuple_nth(args),
    "&tuple:assoc" => meta::assoc(args),
    // effects
    "&display-stack" => meta::display_stack(args, call_stack),
    "raise" => effects::raise(args),
    "quit!" => effects::quit(args),
    "get-env" => effects::get_env(args),
    "&get-calcit-backend" => effects::call_get_calcit_backend(args),
    "read-file" => effects::read_file(args),
    "write-file" => effects::write_file(args),
    // external data format
    "parse-cirru" => meta::parse_cirru(args),
    "format-cirru" => meta::format_cirru(args),
    "parse-cirru-edn" => meta::parse_cirru_edn(args),
    "format-cirru-edn" => meta::format_cirru_edn(args),
    // time
    "cpu-time" => effects::cpu_time(args),
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
    "floor" => math::floor(args),
    "sin" => math::sin(args),
    "cos" => math::cos(args),
    "pow" => math::pow(args),
    "ceil" => math::ceil(args),
    "sqrt" => math::sqrt(args),
    "round" => math::round(args),
    "round?" => math::round_ques(args),
    "&number:rem" => math::rem(args),
    "&number:fract" => math::fractional(args),
    "&number:format" => strings::format_number(args),
    "bit-shr" => math::bit_shr(args),
    "bit-shl" => math::bit_shl(args),
    // strings
    "trim" => strings::trim(args),
    "&str" => strings::call_str(args),
    "turn-string" => strings::turn_string(args),
    "split" => strings::split(args),
    "split-lines" => strings::split_lines(args),
    "starts-with?" => strings::starts_with_ques(args),
    "ends-with?" => strings::ends_with_ques(args),
    "get-char-code" => strings::get_char_code(args),
    "char-from-code" => strings::char_from_code(args),
    "parse-float" => strings::parse_float(args),
    "pr-str" => strings::pr_str(args),
    "blank?" => strings::blank_ques(args),
    "&str:concat" => strings::binary_str_concat(args),
    "&str:slice" => strings::str_slice(args),
    "&str:compare" => strings::compare_string(args),
    "&str:find-index" => strings::find_index(args),
    "&str:replace" => strings::replace(args),
    "&str:escape" => strings::escape(args),
    "&str:count" => strings::count(args),
    "&str:empty?" => strings::empty_ques(args),
    "&str:contains?" => strings::contains_ques(args),
    "&str:includes?" => strings::includes_ques(args),
    "&str:nth" => strings::nth(args),
    "&str:first" => strings::first(args),
    "&str:rest" => strings::rest(args),
    "&str:pad-left" => strings::pad_left(args),
    "&str:pad-right" => strings::pad_right(args),
    // lists
    "[]" => lists::new_list(args),
    "'" => lists::new_list(args), // alias
    "append" => lists::append(args),
    "prepend" => lists::prepend(args),
    "butlast" => lists::butlast(args),
    "&list:concat" => lists::concat(args),
    "range" => lists::range(args),
    "sort" => lists::sort(args, call_stack),
    "foldl" => lists::foldl(args, call_stack),
    "foldl-shortcut" => lists::foldl_shortcut(args, call_stack),
    "foldr-shortcut" => lists::foldr_shortcut(args, call_stack),
    "&list:reverse" => lists::reverse(args),
    "&list:slice" => lists::slice(args),
    "&list:assoc-before" => lists::assoc_before(args),
    "&list:assoc-after" => lists::assoc_after(args),
    "&list:count" => lists::count(args),
    "&list:empty?" => lists::empty_ques(args),
    "&list:contains?" => lists::contains_ques(args),
    "&list:includes?" => lists::includes_ques(args),
    "&list:nth" => lists::nth(args),
    "&list:first" => lists::first(args),
    "&list:rest" => lists::rest(args),
    "&list:assoc" => lists::assoc(args),
    "&list:dissoc" => lists::dissoc(args),
    "&list:to-set" => lists::list_to_set(args),
    "&list:distinct" => lists::distinct(args),
    // maps
    "&{}" => maps::call_new_map(args),
    "&merge" => maps::call_merge(args),
    "to-pairs" => maps::to_pairs(args),
    "&merge-non-nil" => maps::call_merge_non_nil(args),
    "&map:to-list" => maps::to_list(args),
    "&map:count" => maps::count(args),
    "&map:empty?" => maps::empty_ques(args),
    "&map:contains?" => maps::contains_ques(args),
    "&map:includes?" => maps::includes_ques(args),
    "&map:first" => maps::first(args),
    "&map:rest" => maps::rest(args),
    "&map:get" => maps::get(args),
    "&map:assoc" => maps::assoc(args),
    "&map:dissoc" => maps::dissoc(args),
    "&map:diff-new" => maps::diff_new(args),
    "&map:diff-keys" => maps::diff_keys(args),
    "&map:common-keys" => maps::common_keys(args),
    // sets
    "#{}" => sets::new_set(args),
    "&include" => sets::call_include(args),
    "&exclude" => sets::call_exclude(args),
    "&difference" => sets::call_difference(args),
    "&union" => sets::call_union(args),
    "&set:intersection" => sets::call_intersection(args),
    "&set:to-list" => sets::set_to_list(args),
    "&set:count" => sets::count(args),
    "&set:empty?" => sets::empty_ques(args),
    "&set:includes?" => sets::includes_ques(args),
    "&set:first" => sets::first(args),
    "&set:rest" => sets::rest(args),
    // refs
    "deref" => refs::deref(args),
    "add-watch" => refs::add_watch(args),
    "remove-watch" => refs::remove_watch(args),
    // records
    "new-record" => records::new_record(args),
    "&%{}" => records::call_record(args),
    "&record:from-map" => records::record_from_map(args),
    "&record:get-name" => records::get_record_name(args),
    "&record:to-map" => records::turn_map(args),
    "&record:matches?" => records::matches(args),
    "&record:count" => records::count(args),
    "&record:contains?" => records::contains_ques(args),
    "&record:get" => records::get(args),
    "&record:assoc" => records::assoc(args),
    "&record:extend-as" => records::extend_as(args),
    a => {
      let ps = IMPORTED_PROCS.read().unwrap();
      if ps.contains_key(name) {
        let f = ps[name];
        f(args, call_stack)
      } else {
        Err(CalcitErr::use_msg_stack(format!("No such proc: {}", a), call_stack))
      }
    }
  }
}

/// inject into procs
pub fn register_import_proc(name: &str, f: FnType) {
  let mut ps = IMPORTED_PROCS.write().unwrap();
  (*ps).insert(name.to_owned().into(), f);
}

pub fn handle_syntax(
  name: &CalcitSyntax,
  nodes: &CalcitItems,
  scope: &CalcitScope,
  file_ns: Arc<str>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  match name {
    CalcitSyntax::Defn => syntax::defn(nodes, scope, file_ns),
    CalcitSyntax::Eval => syntax::eval(nodes, scope, file_ns, call_stack),
    CalcitSyntax::Defmacro => syntax::defmacro(nodes, scope, file_ns),
    CalcitSyntax::Quote => syntax::quote(nodes, scope, file_ns),
    CalcitSyntax::Quasiquote => syntax::quasiquote(nodes, scope, file_ns, call_stack),
    CalcitSyntax::If => syntax::syntax_if(nodes, scope, file_ns, call_stack),
    CalcitSyntax::CoreLet => syntax::syntax_let(nodes, scope, file_ns, call_stack),
    CalcitSyntax::Macroexpand => syntax::macroexpand(nodes, scope, file_ns, call_stack),
    CalcitSyntax::Macroexpand1 => syntax::macroexpand_1(nodes, scope, file_ns, call_stack),
    CalcitSyntax::MacroexpandAll => syntax::macroexpand_all(nodes, scope, file_ns, call_stack),
    CalcitSyntax::Try => syntax::call_try(nodes, scope, file_ns, call_stack),
    // "define reference" although it uses a confusing name "atom"
    CalcitSyntax::Defatom => refs::defatom(nodes, scope, file_ns, call_stack),
    CalcitSyntax::Reset => refs::reset_bang(nodes, scope, file_ns, call_stack),
    // different behavoirs, in Rust interpreter it's nil, in js codegen it's nothing
    CalcitSyntax::HintFn => meta::no_op(),
  }
}

// detects extra javascript things in js mode,
// mostly internal procs, also some syntaxs
pub fn is_js_syntax_procs(s: &str) -> bool {
  matches!(
    s,
    "aget"
      | "aset"
      | "exists?"
      | "extract-cirru-edn"
      | "foldl"
      | "instance?"
      | "&js-object"
      | "js-array"
      | "js-await"
      | "load-console-formatter!"
      | "printable"
      | "new"
      | "set!"
      | "timeout-call"
      | "to-calcit-data"
      | "to-cirru-edn"
      | "to-js-data"
      | "invoke-method" // dynamically
  )
}
