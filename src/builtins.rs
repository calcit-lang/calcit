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
  match name {
    // meta
    CalcitProc::TypeOf => meta::type_of(args),
    CalcitProc::Recur => meta::recur(args),
    CalcitProc::FormatToLisp => meta::format_to_lisp(args),
    CalcitProc::FormatToCirru => meta::format_to_cirru(args),
    CalcitProc::NativeResetGenSymIndex => meta::reset_gensym_index(args),
    CalcitProc::NativeGetCalcitRunningMode => effects::calcit_running_mode(args),
    CalcitProc::GenerateId => meta::generate_id(args),
    CalcitProc::TurnSymbol => meta::turn_symbol(args),
    CalcitProc::TurnTag => meta::turn_tag(args),
    CalcitProc::NativeCompare => meta::native_compare(args),
    CalcitProc::NativeGetOs => meta::get_os(args),
    CalcitProc::NativeFormatTernaryTree => meta::format_ternary_tree(args),
    CalcitProc::NativeBuffer => meta::buffer(args),
    CalcitProc::NativeHash => meta::hash(args),
    CalcitProc::NativeExtractCodeIntoEdn => meta::extract_code_into_edn(args),
    CalcitProc::NativeDataToCode => meta::data_to_code(args),
    CalcitProc::NativeCirruNth => meta::cirru_nth(args),
    CalcitProc::NativeCirruType => meta::cirru_type(args),
    CalcitProc::IsSpreadingMark => meta::is_spreading_mark(args),
    // tuple
    CalcitProc::NativeTuple => meta::new_tuple(args), // unstable solution for the name
    CalcitProc::NativeClassTuple => meta::new_class_tuple(args),
    CalcitProc::NativeTupleNth => meta::tuple_nth(args),
    CalcitProc::NativeTupleAssoc => meta::assoc(args),
    CalcitProc::NativeTupleCount => meta::tuple_count(args),
    CalcitProc::NativeTupleClass => meta::tuple_class(args),
    CalcitProc::NativeTupleParams => meta::tuple_params(args),
    CalcitProc::NativeTupleWithClass => meta::tuple_with_class(args),
    // effects
    CalcitProc::NativeDisplayStack => meta::display_stack(args, call_stack),
    CalcitProc::Raise => effects::raise(args),
    CalcitProc::Quit => effects::quit(args),
    CalcitProc::GetEnv => effects::get_env(args),
    CalcitProc::NativeGetCalcitBackend => effects::call_get_calcit_backend(args),
    CalcitProc::ReadFile => effects::read_file(args),
    CalcitProc::WriteFile => effects::write_file(args),
    // external data format
    CalcitProc::ParseCirru => meta::parse_cirru(args),
    CalcitProc::ParseCirruList => meta::parse_cirru_list(args),
    CalcitProc::FormatCirru => meta::format_cirru(args),
    CalcitProc::ParseCirruEdn => meta::parse_cirru_edn(args),
    CalcitProc::FormatCirruEdn => meta::format_cirru_edn(args),
    CalcitProc::NativeCirruQuoteToList => meta::cirru_quote_to_list(args),
    // time
    CalcitProc::CpuTime => effects::cpu_time(args),
    // logics
    CalcitProc::NativeEquals => logics::binary_equal(args),
    CalcitProc::NativeLessThan => logics::binary_less(args),
    CalcitProc::NativeGreaterThan => logics::binary_greater(args),
    CalcitProc::Not => logics::not(args),
    // in Rust, no real pointer `identical?`, fallback to value equal
    CalcitProc::Identical => logics::binary_equal(args),
    // math
    CalcitProc::NativeAdd => math::binary_add(args),
    CalcitProc::NativeMinus => math::binary_minus(args),
    CalcitProc::NativeMultiply => math::binary_multiply(args),
    CalcitProc::NativeDivide => math::binary_divide(args),
    CalcitProc::Floor => math::floor(args),
    CalcitProc::Sin => math::sin(args),
    CalcitProc::Cos => math::cos(args),
    CalcitProc::Pow => math::pow(args),
    CalcitProc::Ceil => math::ceil(args),
    CalcitProc::Sqrt => math::sqrt(args),
    CalcitProc::Round => math::round(args),
    CalcitProc::IsRound => math::round_ques(args),
    CalcitProc::NativeNumberRem => math::rem(args),
    CalcitProc::NativeNumberFract => math::fractional(args),
    CalcitProc::NativeNumberFormat => strings::format_number(args),
    CalcitProc::NativeNumberDisplayBy => strings::display_number_by(args),
    CalcitProc::BitShr => math::bit_shr(args),
    CalcitProc::BitShl => math::bit_shl(args),
    CalcitProc::BitAnd => math::bit_and(args),
    CalcitProc::BitOr => math::bit_or(args),
    CalcitProc::BitXor => math::bit_xor(args),
    CalcitProc::BitNot => math::bit_not(args),
    // strings
    CalcitProc::Trim => strings::trim(args),
    CalcitProc::NativeStr => strings::call_str(args),
    CalcitProc::TurnString => strings::turn_string(args),
    CalcitProc::Split => strings::split(args),
    CalcitProc::SplitLines => strings::split_lines(args),
    CalcitProc::StartsWith => strings::starts_with_ques(args),
    CalcitProc::EndsWith => strings::ends_with_ques(args),
    CalcitProc::GetCharCode => strings::get_char_code(args),
    CalcitProc::CharFromCode => strings::char_from_code(args),
    CalcitProc::ParseFloat => strings::parse_float(args),
    CalcitProc::PrStr => strings::lispy_string(args),
    CalcitProc::IsBlank => strings::blank_ques(args),
    CalcitProc::NativeStrConcat => strings::binary_str_concat(args),
    CalcitProc::NativeStrSlice => strings::str_slice(args),
    CalcitProc::NativeStrCompare => strings::compare_string(args),
    CalcitProc::NativeStrFindIndex => strings::find_index(args),
    CalcitProc::NativeStrReplace => strings::replace(args),
    CalcitProc::NativeStrEscape => strings::escape(args),
    CalcitProc::NativeStrCount => strings::count(args),
    CalcitProc::NativeStrEmpty => strings::empty_ques(args),
    CalcitProc::NativeStrContains => strings::contains_ques(args),
    CalcitProc::NativeStrIncludes => strings::includes_ques(args),
    CalcitProc::NativeStrNth => strings::nth(args),
    CalcitProc::NativeStrFirst => strings::first(args),
    CalcitProc::NativeStrRest => strings::rest(args),
    CalcitProc::NativeStrPadLeft => strings::pad_left(args),
    CalcitProc::NativeStrPadRight => strings::pad_right(args),
    // lists
    CalcitProc::List => lists::new_list(args),
    CalcitProc::Append => lists::append(args),
    CalcitProc::Prepend => lists::prepend(args),
    CalcitProc::Butlast => lists::butlast(args),
    CalcitProc::NativeListConcat => lists::concat(args),
    CalcitProc::Range => lists::range(args),
    CalcitProc::Sort => lists::sort(args, call_stack),
    CalcitProc::Foldl => lists::foldl(args, call_stack),
    CalcitProc::FoldlShortcut => lists::foldl_shortcut(args, call_stack),
    CalcitProc::FoldrShortcut => lists::foldr_shortcut(args, call_stack),
    CalcitProc::NativeListReverse => lists::reverse(args),
    CalcitProc::NativeListSlice => lists::slice(args),
    CalcitProc::NativeListAssocBefore => lists::assoc_before(args),
    CalcitProc::NativeListAssocAfter => lists::assoc_after(args),
    CalcitProc::NativeListCount => lists::count(args),
    CalcitProc::NativeListEmpty => lists::empty_ques(args),
    CalcitProc::NativeListContains => lists::contains_ques(args),
    CalcitProc::NativeListIncludes => lists::includes_ques(args),
    CalcitProc::NativeListNth => lists::nth(args),
    CalcitProc::NativeListFirst => lists::first(args),
    CalcitProc::NativeListRest => lists::rest(args),
    CalcitProc::NativeListAssoc => lists::assoc(args),
    CalcitProc::NativeListDissoc => lists::dissoc(args),
    CalcitProc::NativeListToSet => lists::list_to_set(args),
    CalcitProc::NativeListDistinct => lists::distinct(args),
    // maps
    CalcitProc::NativeMap => maps::call_new_map(args),
    CalcitProc::NativeMerge => maps::call_merge(args),
    CalcitProc::ToPairs => maps::to_pairs(args),
    CalcitProc::NativeMergeNonNil => maps::call_merge_non_nil(args),
    CalcitProc::NativeMapToList => maps::to_list(args),
    CalcitProc::NativeMapCount => maps::count(args),
    CalcitProc::NativeMapEmpty => maps::empty_ques(args),
    CalcitProc::NativeMapContains => maps::contains_ques(args),
    CalcitProc::NativeMapIncludes => maps::includes_ques(args),
    CalcitProc::NativeMapDestruct => maps::destruct(args),
    CalcitProc::NativeMapGet => maps::get(args),
    CalcitProc::NativeMapAssoc => maps::assoc(args),
    CalcitProc::NativeMapDissoc => maps::dissoc(args),
    CalcitProc::NativeMapDiffNew => maps::diff_new(args),
    CalcitProc::NativeMapDiffKeys => maps::diff_keys(args),
    CalcitProc::NativeMapCommonKeys => maps::common_keys(args),
    // sets
    CalcitProc::Set => sets::new_set(args),
    CalcitProc::NativeInclude => sets::call_include(args),
    CalcitProc::NativeExclude => sets::call_exclude(args),
    CalcitProc::NativeDifference => sets::call_difference(args),
    CalcitProc::NativeUnion => sets::call_union(args),
    CalcitProc::NativeSetIntersection => sets::call_intersection(args),
    CalcitProc::NativeSetToList => sets::set_to_list(args),
    CalcitProc::NativeSetCount => sets::count(args),
    CalcitProc::NativeSetEmpty => sets::empty_ques(args),
    CalcitProc::NativeSetIncludes => sets::includes_ques(args),
    CalcitProc::NativeSetDestruct => sets::destruct(args),
    // refs
    CalcitProc::Atom => refs::atom(args),
    CalcitProc::AtomDeref => refs::atom_deref(args),
    CalcitProc::AddWatch => refs::add_watch(args),
    CalcitProc::RemoveWatch => refs::remove_watch(args),
    // records
    CalcitProc::NewRecord => records::new_record(args),
    CalcitProc::NewClassRecord => records::new_class_record(args),
    CalcitProc::NativeRecord => records::call_record(args),
    CalcitProc::NativeRecordWith => records::record_with(args),
    CalcitProc::NativeRecordClass => records::get_class(args),
    CalcitProc::NativeRecordWithClass => records::with_class(args),
    CalcitProc::NativeRecordFromMap => records::record_from_map(args),
    CalcitProc::NativeRecordGetName => records::get_record_name(args),
    CalcitProc::NativeRecordToMap => records::turn_map(args),
    CalcitProc::NativeRecordMatches => records::matches(args),
    CalcitProc::NativeRecordCount => records::count(args),
    CalcitProc::NativeRecordContains => records::contains_ques(args),
    CalcitProc::NativeRecordGet => records::get(args),
    CalcitProc::NativeRecordAssoc => records::assoc(args),
    CalcitProc::NativeRecordExtendAs => records::extend_as(args),
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
  match name {
    CalcitSyntax::Defn => syntax::defn(nodes, scope, file_ns),
    CalcitSyntax::Eval => syntax::eval(nodes, scope, file_ns, call_stack),
    CalcitSyntax::Defmacro => syntax::defmacro(nodes, scope, file_ns),
    CalcitSyntax::Quote => syntax::quote(nodes, scope, file_ns),
    CalcitSyntax::Quasiquote => syntax::quasiquote(nodes, scope, file_ns, call_stack),
    CalcitSyntax::Gensym => syntax::gensym(nodes, scope, file_ns, call_stack),
    CalcitSyntax::If => syntax::syntax_if(nodes, scope, file_ns, call_stack),
    CalcitSyntax::CoreLet => syntax::syntax_let(nodes, scope, file_ns, call_stack),
    CalcitSyntax::Macroexpand => syntax::macroexpand(nodes, scope, file_ns, call_stack),
    CalcitSyntax::Macroexpand1 => syntax::macroexpand_1(nodes, scope, file_ns, call_stack),
    CalcitSyntax::MacroexpandAll => syntax::macroexpand_all(nodes, scope, file_ns, call_stack),
    CalcitSyntax::CallSpread => syntax::call_spread(nodes, scope, file_ns, call_stack),
    CalcitSyntax::Try => syntax::call_try(nodes, scope, file_ns, call_stack),
    // "define reference" although it uses a confusing name "atom"
    CalcitSyntax::Defatom => refs::defatom(nodes, scope, file_ns, call_stack),
    CalcitSyntax::Reset => refs::reset_bang(nodes, scope, file_ns, call_stack),
    // different behaviors, in Rust interpreter it's nil, in js codegen it's nothing
    CalcitSyntax::HintFn => meta::no_op(),
    CalcitSyntax::ArgSpread => CalcitErr::err_nodes(CalcitErrKind::Syntax, "`&` cannot be used as operator", &nodes.to_vec()),
    CalcitSyntax::ArgOptional => CalcitErr::err_nodes(CalcitErrKind::Syntax, "`?` cannot be used as operator", &nodes.to_vec()),
    CalcitSyntax::MacroInterpolate => CalcitErr::err_nodes(CalcitErrKind::Syntax, "`~` cannot be used as operator", &nodes.to_vec()),
    CalcitSyntax::MacroInterpolateSpread => {
      CalcitErr::err_nodes(CalcitErrKind::Syntax, "`~@` cannot be used as operator", &nodes.to_vec())
    }
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
