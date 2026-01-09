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
use std::sync::{Arc, LazyLock, RwLock};

use crate::calcit::{Calcit, CalcitErr, CalcitErrKind, CalcitList, CalcitProc, CalcitScope, CalcitSyntax};
use crate::call_stack::{CallStackList, using_stack};

use im_ternary_tree::TernaryTreeList;
pub(crate) use refs::{ValueAndListeners, quick_build_atom};

pub type FnType = fn(xs: Vec<Calcit>, call_stack: &CallStackList) -> Result<Calcit, CalcitErr>;
pub type SyntaxType = fn(expr: &TernaryTreeList<Calcit>, scope: &CalcitScope, file_ns: &str) -> Result<Calcit, CalcitErr>;

pub(crate) static IMPORTED_PROCS: LazyLock<RwLock<HashMap<Arc<str>, FnType>>> = LazyLock::new(|| RwLock::new(HashMap::new()));

pub fn is_proc_name(s: &str) -> bool {
  let builtin = s.parse::<CalcitProc>();
  if builtin.is_ok() { true } else { is_registered_proc(s) }
}

pub fn is_registered_proc(s: &str) -> bool {
  let ps = IMPORTED_PROCS.read().expect("read procs");
  ps.contains_key(s)
}

/// make sure that stack information attached in errors from procs
pub fn handle_proc(name: CalcitProc, args: &[Calcit], call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if using_stack() {
    handle_proc_internal(name, args, call_stack).map_err(|e| {
      if e.stack.is_empty() {
        let mut e2 = e;
        call_stack.clone_into(&mut e2.stack);
        e2
      } else {
        e
      }
    })
  } else {
    handle_proc_internal(name, args, call_stack)
  }
}

fn handle_proc_internal(name: CalcitProc, args: &[Calcit], call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  use CalcitProc::*;

  match name {
    // meta
    TypeOf => meta::type_of(args),
    Recur => meta::recur(args),
    FormatToLisp => meta::format_to_lisp(args),
    FormatToCirru => meta::format_to_cirru(args),
    NativeResetGenSymIndex => meta::reset_gensym_index(args),
    NativeGetCalcitRunningMode => effects::calcit_running_mode(args),
    GenerateId => meta::generate_id(args),
    TurnSymbol => meta::turn_symbol(args),
    TurnTag => meta::turn_tag(args),
    NativeCompare => meta::native_compare(args),
    NativeGetOs => meta::get_os(args),
    NativeFormatTernaryTree => meta::format_ternary_tree(args),
    NativeBuffer => meta::buffer(args),
    NativeHash => meta::hash(args),
    NativeExtractCodeIntoEdn => meta::extract_code_into_edn(args),
    NativeDataToCode => meta::data_to_code(args),
    NativeCirruNth => meta::cirru_nth(args),
    NativeCirruType => meta::cirru_type(args),
    IsSpreadingMark => meta::is_spreading_mark(args),
    // tuple
    NativeTuple => meta::new_tuple(args), // unstable solution for the name
    NativeClassTuple => meta::new_class_tuple(args),
    NativeEnumTuple => meta::new_enum_tuple(args),
    NativeTupleNth => meta::tuple_nth(args),
    NativeTupleAssoc => meta::assoc(args),
    NativeTupleCount => meta::tuple_count(args),
    NativeTupleClass => meta::tuple_class(args),
    NativeTupleParams => meta::tuple_params(args),
    NativeTupleWithClass => meta::tuple_with_class(args),
    // effects
    NativeDisplayStack => meta::display_stack(args, call_stack),
    Raise => effects::raise(args),
    Quit => effects::quit(args),
    GetEnv => effects::get_env(args),
    NativeGetCalcitBackend => effects::call_get_calcit_backend(args),
    ReadFile => effects::read_file(args),
    WriteFile => effects::write_file(args),
    // external data format
    ParseCirru => meta::parse_cirru(args),
    ParseCirruList => meta::parse_cirru_list(args),
    FormatCirru => meta::format_cirru(args),
    ParseCirruEdn => meta::parse_cirru_edn(args),
    FormatCirruEdn => meta::format_cirru_edn(args),
    NativeCirruQuoteToList => meta::cirru_quote_to_list(args),
    // time
    CpuTime => effects::cpu_time(args),
    // logics
    NativeEquals => logics::binary_equal(args),
    NativeLessThan => logics::binary_less(args),
    NativeGreaterThan => logics::binary_greater(args),
    Not => logics::not(args),
    // in Rust, no real pointer `identical?`, fallback to value equal
    Identical => logics::binary_equal(args),
    // math
    NativeAdd => math::binary_add(args),
    NativeMinus => math::binary_minus(args),
    NativeMultiply => math::binary_multiply(args),
    NativeDivide => math::binary_divide(args),
    Floor => math::floor(args),
    Sin => math::sin(args),
    Cos => math::cos(args),
    Pow => math::pow(args),
    Ceil => math::ceil(args),
    Sqrt => math::sqrt(args),
    Round => math::round(args),
    IsRound => math::round_ques(args),
    NativeNumberRem => math::rem(args),
    NativeNumberFract => math::fractional(args),
    NativeNumberFormat => strings::format_number(args),
    NativeNumberDisplayBy => strings::display_number_by(args),
    BitShr => math::bit_shr(args),
    BitShl => math::bit_shl(args),
    BitAnd => math::bit_and(args),
    BitOr => math::bit_or(args),
    BitXor => math::bit_xor(args),
    BitNot => math::bit_not(args),
    // strings
    Trim => strings::trim(args),
    NativeStr => strings::call_str(args),
    TurnString => strings::turn_string(args),
    Split => strings::split(args),
    SplitLines => strings::split_lines(args),
    StartsWith => strings::starts_with_ques(args),
    EndsWith => strings::ends_with_ques(args),
    GetCharCode => strings::get_char_code(args),
    CharFromCode => strings::char_from_code(args),
    ParseFloat => strings::parse_float(args),
    PrStr => strings::lispy_string(args),
    IsBlank => strings::blank_ques(args),
    NativeStrConcat => strings::binary_str_concat(args),
    NativeStrSlice => strings::str_slice(args),
    NativeStrCompare => strings::compare_string(args),
    NativeStrFindIndex => strings::find_index(args),
    NativeStrReplace => strings::replace(args),
    NativeStrEscape => strings::escape(args),
    NativeStrCount => strings::count(args),
    NativeStrEmpty => strings::empty_ques(args),
    NativeStrContains => strings::contains_ques(args),
    NativeStrIncludes => strings::includes_ques(args),
    NativeStrNth => strings::nth(args),
    NativeStrFirst => strings::first(args),
    NativeStrRest => strings::rest(args),
    NativeStrPadLeft => strings::pad_left(args),
    NativeStrPadRight => strings::pad_right(args),
    // lists
    List => lists::new_list(args),
    Append => lists::append(args),
    Prepend => lists::prepend(args),
    Butlast => lists::butlast(args),
    NativeListConcat => lists::concat(args),
    Range => lists::range(args),
    Sort => lists::sort(args, call_stack),
    Foldl => lists::foldl(args, call_stack),
    FoldlShortcut => lists::foldl_shortcut(args, call_stack),
    FoldrShortcut => lists::foldr_shortcut(args, call_stack),
    NativeListReverse => lists::reverse(args),
    NativeListSlice => lists::slice(args),
    NativeListAssocBefore => lists::assoc_before(args),
    NativeListAssocAfter => lists::assoc_after(args),
    NativeListCount => lists::count(args),
    NativeListEmpty => lists::empty_ques(args),
    NativeListContains => lists::contains_ques(args),
    NativeListIncludes => lists::includes_ques(args),
    NativeListNth => lists::nth(args),
    NativeListFirst => lists::first(args),
    NativeListRest => lists::rest(args),
    NativeListAssoc => lists::assoc(args),
    NativeListDissoc => lists::dissoc(args),
    NativeListToSet => lists::list_to_set(args),
    NativeListDistinct => lists::distinct(args),
    // maps
    NativeMap => maps::call_new_map(args),
    NativeMerge => maps::call_merge(args),
    ToPairs => maps::to_pairs(args),
    NativeMergeNonNil => maps::call_merge_non_nil(args),
    NativeMapToList => maps::to_list(args),
    NativeMapCount => maps::count(args),
    NativeMapEmpty => maps::empty_ques(args),
    NativeMapContains => maps::contains_ques(args),
    NativeMapIncludes => maps::includes_ques(args),
    NativeMapDestruct => maps::destruct(args),
    NativeMapGet => maps::get(args),
    NativeMapAssoc => maps::assoc(args),
    NativeMapDissoc => maps::dissoc(args),
    NativeMapDiffNew => maps::diff_new(args),
    NativeMapDiffKeys => maps::diff_keys(args),
    NativeMapCommonKeys => maps::common_keys(args),
    // sets
    Set => sets::new_set(args),
    NativeInclude => sets::call_include(args),
    NativeExclude => sets::call_exclude(args),
    NativeDifference => sets::call_difference(args),
    NativeUnion => sets::call_union(args),
    NativeSetIntersection => sets::call_intersection(args),
    NativeSetToList => sets::set_to_list(args),
    NativeSetCount => sets::count(args),
    NativeSetEmpty => sets::empty_ques(args),
    NativeSetIncludes => sets::includes_ques(args),
    NativeSetDestruct => sets::destruct(args),
    // refs
    Atom => refs::atom(args),
    AtomDeref => refs::atom_deref(args),
    AddWatch => refs::add_watch(args),
    RemoveWatch => refs::remove_watch(args),
    // records
    NewRecord => records::new_record(args),
    NewClassRecord => records::new_class_record(args),
    NativeRecord => records::call_record(args),
    NativeRecordWith => records::record_with(args),
    NativeRecordClass => records::get_class(args),
    NativeRecordWithClass => records::with_class(args),
    NativeRecordFromMap => records::record_from_map(args),
    NativeRecordGetName => records::get_record_name(args),
    NativeRecordToMap => records::turn_map(args),
    NativeRecordMatches => records::matches(args),
    NativeRecordCount => records::count(args),
    NativeRecordContains => records::contains_ques(args),
    NativeRecordGet => records::get(args),
    NativeRecordAssoc => records::assoc(args),
    NativeRecordExtendAs => records::extend_as(args),
  }
}

/// inject into procs
pub fn register_import_proc(name: &str, f: FnType) {
  let mut ps = IMPORTED_PROCS.write().expect("open procs");
  (*ps).insert(Arc::from(name), f);
}

pub fn handle_syntax(
  name: &CalcitSyntax,
  nodes: &CalcitList,
  scope: &CalcitScope,
  file_ns: &str,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  use CalcitSyntax::*;
  match name {
    Defn => syntax::defn(nodes, scope, file_ns),
    Eval => syntax::eval(nodes, scope, file_ns, call_stack),
    Defmacro => syntax::defmacro(nodes, scope, file_ns),
    Quote => syntax::quote(nodes, scope, file_ns),
    Quasiquote => syntax::quasiquote(nodes, scope, file_ns, call_stack),
    Gensym => syntax::gensym(nodes, scope, file_ns, call_stack),
    If => syntax::syntax_if(nodes, scope, file_ns, call_stack),
    CoreLet => syntax::syntax_let(nodes, scope, file_ns, call_stack),
    Macroexpand => syntax::macroexpand(nodes, scope, file_ns, call_stack),
    Macroexpand1 => syntax::macroexpand_1(nodes, scope, file_ns, call_stack),
    MacroexpandAll => syntax::macroexpand_all(nodes, scope, file_ns, call_stack),
    CallSpread => syntax::call_spread(nodes, scope, file_ns, call_stack),
    Try => syntax::call_try(nodes, scope, file_ns, call_stack),
    // "define reference" although it uses a confusing name "atom"
    Defatom => refs::defatom(nodes, scope, file_ns, call_stack),
    Reset => refs::reset_bang(nodes, scope, file_ns, call_stack),
    // different behaviors, in Rust interpreter it's nil, in js codegen it's nothing
    HintFn => meta::no_op(),
    ArgSpread => CalcitErr::err_nodes(CalcitErrKind::Syntax, "`&` cannot be used as operator", &nodes.to_vec()),
    ArgOptional => CalcitErr::err_nodes(CalcitErrKind::Syntax, "`?` cannot be used as operator", &nodes.to_vec()),
    MacroInterpolate => CalcitErr::err_nodes(CalcitErrKind::Syntax, "`~` cannot be used as operator", &nodes.to_vec()),
    MacroInterpolateSpread => CalcitErr::err_nodes(CalcitErrKind::Syntax, "`~@` cannot be used as operator", &nodes.to_vec()),
    AssertType => CalcitErr::err_nodes(
      CalcitErrKind::Unimplemented,
      "`assert-type` is not supported at runtime yet",
      &nodes.to_vec(),
    ),
  }
}

// detects extra javascript things in js mode,
// mostly internal procs, also some syntaxs
pub fn is_js_syntax_procs(s: &str) -> bool {
  matches!(
    s,
    "aget"
      | "aset"
      | "js-get" // alias for aget
      | "js-set" // alias for aset
      | "js-delete"
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
      | "&raw-code"
      | "js-for-await"
  )
}
