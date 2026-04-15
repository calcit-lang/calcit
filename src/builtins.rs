pub mod effects;
mod json;
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

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, RwLock};

use crate::calcit::{Calcit, CalcitErr, CalcitErrKind, CalcitList, CalcitProc, CalcitScope, CalcitSyntax};
use crate::call_stack::{CallStackList, using_stack};

use im_ternary_tree::TernaryTreeList;
pub(crate) use refs::{ValueAndListeners, quick_build_atom};

pub type FnType = fn(xs: Vec<Calcit>, call_stack: &CallStackList) -> Result<Calcit, CalcitErr>;
pub type SyntaxType = fn(expr: &TernaryTreeList<Calcit>, scope: &CalcitScope, file_ns: &str) -> Result<Calcit, CalcitErr>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisteredProcPlatform {
  Native,
  Wasm,
  Wasi,
  Js,
}

impl std::fmt::Display for RegisteredProcPlatform {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      RegisteredProcPlatform::Native => write!(f, "native"),
      RegisteredProcPlatform::Wasm => write!(f, "wasm"),
      RegisteredProcPlatform::Wasi => write!(f, "wasi"),
      RegisteredProcPlatform::Js => write!(f, "js"),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisteredProcStability {
  Public,
  Experimental,
  Internal,
}

#[derive(Debug, Clone)]
pub struct RegisteredProcDescriptor {
  pub arity_min: usize,
  pub arity_max: Option<usize>,
  pub platforms: Vec<RegisteredProcPlatform>,
  pub stability: RegisteredProcStability,
  pub docs_hint: Option<Arc<str>>,
  pub callback_last: bool,
}

impl Default for RegisteredProcDescriptor {
  fn default() -> Self {
    RegisteredProcDescriptor {
      arity_min: 0,
      arity_max: None,
      platforms: vec![
        RegisteredProcPlatform::Native,
        RegisteredProcPlatform::Wasm,
        RegisteredProcPlatform::Wasi,
        RegisteredProcPlatform::Js,
      ],
      stability: RegisteredProcStability::Public,
      docs_hint: None,
      callback_last: false,
    }
  }
}

pub(crate) static IMPORTED_PROCS: LazyLock<RwLock<HashMap<Arc<str>, FnType>>> = LazyLock::new(|| RwLock::new(HashMap::new()));
pub(crate) static IMPORTED_PROC_DESCRIPTORS: LazyLock<RwLock<HashMap<Arc<str>, RegisteredProcDescriptor>>> =
  LazyLock::new(|| RwLock::new(HashMap::new()));
pub(crate) static WARNED_REGISTERED_PROCS: LazyLock<RwLock<HashSet<Arc<str>>>> = LazyLock::new(|| RwLock::new(HashSet::new()));

pub(crate) fn err_arity<T: Into<String>>(msg: T, xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  CalcitErr::err_nodes(CalcitErrKind::Arity, msg, xs)
}

pub(crate) fn err_arity_with_hint<T: Into<String>, H: Into<String>>(msg: T, xs: &[Calcit], hint: H) -> Result<Calcit, CalcitErr> {
  CalcitErr::err_nodes_with_hint(CalcitErrKind::Arity, msg, xs, hint)
}

pub(crate) fn err_type_with_hint<T: Into<String>, H: Into<String>>(msg: T, hint: H) -> Result<Calcit, CalcitErr> {
  CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
}

pub fn is_proc_name(s: &str) -> bool {
  let builtin = s.parse::<CalcitProc>();
  if builtin.is_ok() { true } else { is_registered_proc(s) }
}

pub fn is_registered_proc(s: &str) -> bool {
  let ps = IMPORTED_PROCS.read().expect("read procs");
  ps.contains_key(s)
}

fn detect_current_platform() -> RegisteredProcPlatform {
  if cfg!(target_arch = "wasm32") {
    RegisteredProcPlatform::Wasm
  } else if cfg!(target_os = "wasi") {
    RegisteredProcPlatform::Wasi
  } else {
    RegisteredProcPlatform::Native
  }
}

fn validate_registered_proc_call(alias: &str, args: &[Calcit]) -> Result<(), CalcitErr> {
  let descriptors = IMPORTED_PROC_DESCRIPTORS.read().expect("read proc descriptors");
  let Some(descriptor) = descriptors.get(alias) else {
    return Ok(());
  };

  let current = detect_current_platform();
  if !descriptor.platforms.is_empty() && !descriptor.platforms.contains(&current) {
    let expected = descriptor.platforms.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");
    let hint = descriptor
      .docs_hint
      .as_ref()
      .map(|s| s.to_string())
      .unwrap_or_else(|| "Fix: use a capability that supports this runtime platform.".to_owned());
    return CalcitErr::err_str_with_hint(
      CalcitErrKind::Type,
      format!("registered proc `{alias}` is not available on current platform. Expected: {expected}; Actual: {current}"),
      hint,
    )
    .map(|_| ());
  }

  let arity = args.len();
  match descriptor.arity_max {
    Some(max) => {
      if arity < descriptor.arity_min || arity > max {
        let expected = if descriptor.arity_min == max {
          format!("{}", descriptor.arity_min)
        } else {
          format!("{}~{}", descriptor.arity_min, max)
        };
        let hint = descriptor
          .docs_hint
          .as_ref()
          .map(|s| s.to_string())
          .unwrap_or_else(|| "Fix: check proc argument count and call contract.".to_owned());
        return CalcitErr::err_str_with_hint(
          CalcitErrKind::Arity,
          format!("registered proc `{alias}` expects {expected} args, but got {arity}"),
          hint,
        )
        .map(|_| ());
      }
    }
    None => {
      if arity < descriptor.arity_min {
        let hint = descriptor
          .docs_hint
          .as_ref()
          .map(|s| s.to_string())
          .unwrap_or_else(|| "Fix: check proc argument count and call contract.".to_owned());
        return CalcitErr::err_str_with_hint(
          CalcitErrKind::Arity,
          format!(
            "registered proc `{alias}` expects at least {} args, but got {arity}",
            descriptor.arity_min
          ),
          hint,
        )
        .map(|_| ());
      }
    }
  }

  if descriptor.callback_last {
    match args.last() {
      Some(Calcit::Fn { .. }) => {}
      Some(v) => {
        return CalcitErr::err_str_with_hint(
          CalcitErrKind::Type,
          format!("registered proc `{alias}` expects callback function as last argument, but got: {v}"),
          descriptor
            .docs_hint
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Fix: put callback function at the last argument position.".to_owned()),
        )
        .map(|_| ());
      }
      None => {
        return CalcitErr::err_str_with_hint(
          CalcitErrKind::Arity,
          format!("registered proc `{alias}` expects callback function as last argument"),
          descriptor
            .docs_hint
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Fix: pass callback function as last argument.".to_owned()),
        )
        .map(|_| ());
      }
    }
  }

  Ok(())
}

fn emit_registered_proc_stability_warning(alias: &str) {
  let descriptors = IMPORTED_PROC_DESCRIPTORS.read().expect("read proc descriptors");
  let Some(descriptor) = descriptors.get(alias) else {
    return;
  };

  let warning_tag = match descriptor.stability {
    RegisteredProcStability::Public => return,
    RegisteredProcStability::Experimental => "experimental",
    RegisteredProcStability::Internal => "internal",
  };

  let mut warned = WARNED_REGISTERED_PROCS.write().expect("open warned proc names");
  if !warned.insert(Arc::from(alias)) {
    return;
  }
  drop(warned);

  let hint = descriptor
    .docs_hint
    .as_ref()
    .map(|s| s.to_string())
    .unwrap_or_else(|| "Fix: check docs for migration guidance and stable alternatives.".to_owned());
  eprintln!("[Warn] registered proc `{alias}` is marked as {warning_tag}. {hint}");
}

pub fn call_registered_proc(alias: &str, args: Vec<Calcit>, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  validate_registered_proc_call(alias, &args)?;
  emit_registered_proc_stability_warning(alias);
  let ps = IMPORTED_PROCS.read().expect("read procs");
  match ps.get(alias) {
    Some(f) => f(args, call_stack),
    None => CalcitErr::err_str(CalcitErrKind::Var, format!("cannot find registered proc: {alias}")),
  }
}

fn check_proc_arity(name: CalcitProc, args: &[Calcit]) -> Result<(), CalcitErr> {
  let Some(signature) = name.get_type_signature() else {
    return Ok(());
  };

  let arity = signature.arity();
  let actual_count = args.len();
  if let Some(max_count) = arity.max {
    if actual_count < arity.min || actual_count > max_count {
      let expected_str = if arity.min == max_count {
        format!("{}", arity.min)
      } else {
        format!("{}~{}", arity.min, max_count)
      };
      let hint = crate::calcit::format_proc_examples_hint(&name).unwrap_or_default();
      return CalcitErr::err_nodes_with_hint(
        CalcitErrKind::Arity,
        format!("Proc `{name}` expects {expected_str} args, but received:"),
        args,
        hint,
      )
      .map(|_| ());
    }
  } else if actual_count < arity.min {
    let hint = crate::calcit::format_proc_examples_hint(&name).unwrap_or_default();
    return CalcitErr::err_nodes_with_hint(
      CalcitErrKind::Arity,
      format!("Proc `{name}` expects at least {} args, but received:", arity.min),
      args,
      hint,
    )
    .map(|_| ());
  }

  Ok(())
}

/// make sure that stack information attached in errors from procs
pub fn handle_proc(name: CalcitProc, args: &[Calcit], call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  check_proc_arity(name, args)?;

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
    NativeEnumTupleNew => meta::new_enum_tuple_no_class(args),
    NativeTupleNth => meta::tuple_nth(args),
    NativeTupleAssoc => meta::assoc(args),
    NativeTupleCount => meta::tuple_count(args),
    NativeTupleImpls => meta::tuple_impls(args),
    NativeTupleParams => meta::tuple_params(args),
    NativeTupleEnum => meta::tuple_enum(args),
    NativeStructNew => records::new_struct(args),
    NativeEnumNew => records::new_enum(args),
    NativeTraitNew => meta::trait_new(args),
    NativeImplNew => records::new_impl(args),
    NativeImplOrigin => meta::impl_origin(args),
    NativeRecordImplTraits => meta::record_impl_traits(args),
    NativeTupleImplTraits => meta::tuple_impl_traits(args),
    NativeStructImplTraits => meta::struct_impl_traits(args),
    NativeEnumImplTraits => meta::enum_impl_traits(args),
    NativeTupleEnumHasVariant => meta::tuple_enum_has_variant(args),
    NativeTupleEnumVariantArity => meta::tuple_enum_variant_arity(args),
    NativeTupleValidateEnum => meta::tuple_validate_enum(args),
    NativeImplGet => meta::impl_get(args),
    NativeImplNth => meta::impl_nth(args),
    // effects
    NativeDisplayStack => meta::display_stack(args, call_stack),
    NativeMethodsOf => meta::methods_of(args, call_stack),
    NativeInspectMethods => meta::inspect_methods(args, call_stack),
    NativeTraitCall => meta::trait_call(args, call_stack),
    NativeInspectType => Ok(Calcit::Nil), // Handled in preprocessing phase
    NativeAssertTraits => meta::assert_traits(args, call_stack),
    Raise => effects::raise(args),
    Quit => effects::quit(args),
    GetEnv => effects::get_env(args),
    NativeGetCalcitBackend => effects::call_get_calcit_backend(args),
    RegisterCalcitBuiltinImpls => meta::register_calcit_builtin_impls(args),
    ReadFile => effects::read_file(args),
    WriteFile => effects::write_file(args),
    // external data format
    ParseCirru => meta::parse_cirru(args),
    ParseCirruList => meta::parse_cirru_list(args),
    FormatCirru => meta::format_cirru(args),
    FormatCirruOneLiner => meta::format_cirru_one_liner(args),
    ParseCirruEdn => meta::parse_cirru_edn(args),
    FormatCirruEdn => meta::format_cirru_edn(args),
    NativeCirruQuoteToList => meta::cirru_quote_to_list(args),
    JsonParse => json::parse(args),
    JsonStringify => json::stringify(args),
    JsonPretty => json::pretty(args),
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
    NativeLooseRecord => records::call_loose_record(args),
    NativeRecord => records::call_record(args),
    NativeRecordPartial => records::call_record_partial(args),
    NativeRecordWith => records::record_with(args),
    NativeRecordImpls => records::get_impls(args),
    NativeRecordFromMap => records::record_from_map(args),
    NativeRecordGetName => records::get_record_name(args),
    NativeRecordStruct => records::get_record_struct(args),
    NativeRecordToMap => records::turn_map(args),
    NativeRecordMatches => records::matches(args),
    NativeRecordCount => records::count(args),
    NativeRecordContains => records::contains_ques(args),
    NativeRecordGet => records::get(args),
    NativeRecordNth => records::record_nth(args),
    NativeRecordAssoc => records::assoc(args),
    NativeRecordAssocAt => records::record_assoc_at(args),
    NativeRecordExtendAs => records::extend_as(args),
    DeftypeSlot => meta::deftype_slot(args),
    BindType => meta::bind_type(args),
  }
}

/// inject into procs
pub fn register_import_proc(name: &str, f: FnType) {
  register_import_proc_with_descriptor(name, f, RegisteredProcDescriptor::default());
}

pub fn register_import_proc_with_descriptor(name: &str, f: FnType, descriptor: RegisteredProcDescriptor) {
  let ps = IMPORTED_PROCS.read().expect("read procs");
  assert!(!ps.contains_key(name), "duplicate registered proc: {name}");
  drop(ps);

  let mut ps = IMPORTED_PROCS.write().expect("open procs");
  (*ps).insert(Arc::from(name), f);
  drop(ps);

  let mut descriptors = IMPORTED_PROC_DESCRIPTORS.write().expect("open proc descriptors");
  descriptors.insert(Arc::from(name), descriptor);
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
    AssertType => syntax::assert_type(nodes, scope, file_ns, call_stack),
    AssertTraits => syntax::assert_traits(nodes, scope, file_ns, call_stack),
    Match => syntax::syntax_match(nodes, scope, file_ns, call_stack),
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

#[cfg(test)]
mod tests {
  use super::*;
  use std::sync::atomic::{AtomicUsize, Ordering};

  static TEST_PROC_COUNTER: AtomicUsize = AtomicUsize::new(0);

  fn unique_name(prefix: &str) -> String {
    let id = TEST_PROC_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{id}")
  }

  fn dummy_proc(_xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
    Ok(Calcit::Nil)
  }

  #[test]
  fn registered_proc_default_descriptor_works() {
    let name = unique_name("test-default-registered-proc");
    register_import_proc(&name, dummy_proc);

    let result = call_registered_proc(&name, vec![], &CallStackList::default());
    assert!(result.is_ok());
  }

  #[test]
  fn registered_proc_namespaced_key_works() {
    let name = unique_name("calcit.ffi/test-registered-proc");
    register_import_proc_with_descriptor(
      &name,
      dummy_proc,
      RegisteredProcDescriptor {
        arity_min: 0,
        arity_max: None,
        platforms: vec![RegisteredProcPlatform::Native],
        stability: RegisteredProcStability::Public,
        docs_hint: None,
        callback_last: false,
      },
    );

    assert!(is_registered_proc(&name));
    let result = call_registered_proc(&name, vec![], &CallStackList::default());
    assert!(result.is_ok());
  }

  #[test]
  fn registered_proc_checks_platform() {
    let name = unique_name("test-platform-registered-proc");
    register_import_proc_with_descriptor(
      &name,
      dummy_proc,
      RegisteredProcDescriptor {
        arity_min: 0,
        arity_max: None,
        platforms: vec![RegisteredProcPlatform::Js],
        stability: RegisteredProcStability::Public,
        docs_hint: Some(Arc::from("Fix: switch to JS runtime or use native alternative.")),
        callback_last: false,
      },
    );

    let error = call_registered_proc(&name, vec![], &CallStackList::default()).expect_err("expected platform mismatch");
    assert_eq!(error.kind, CalcitErrKind::Type);
  }

  #[test]
  fn registered_proc_checks_arity() {
    let name = unique_name("test-arity-registered-proc");
    register_import_proc_with_descriptor(
      &name,
      dummy_proc,
      RegisteredProcDescriptor {
        arity_min: 2,
        arity_max: Some(2),
        platforms: vec![RegisteredProcPlatform::Native],
        stability: RegisteredProcStability::Public,
        docs_hint: Some(Arc::from("Fix: pass exactly 2 arguments.")),
        callback_last: false,
      },
    );

    let args = vec![Calcit::Number(1.0)];
    let error = call_registered_proc(&name, args, &CallStackList::default()).expect_err("expected arity mismatch");
    assert_eq!(error.kind, CalcitErrKind::Arity);
  }

  #[test]
  fn registered_proc_checks_callback_last() {
    let name = unique_name("test-callback-registered-proc");
    register_import_proc_with_descriptor(
      &name,
      dummy_proc,
      RegisteredProcDescriptor {
        arity_min: 1,
        arity_max: None,
        platforms: vec![RegisteredProcPlatform::Native],
        stability: RegisteredProcStability::Experimental,
        docs_hint: Some(Arc::from("Fix: place callback function at the last argument.")),
        callback_last: true,
      },
    );

    let args = vec![Calcit::Number(1.0)];
    let error = call_registered_proc(&name, args, &CallStackList::default()).expect_err("expected callback type mismatch");
    assert_eq!(error.kind, CalcitErrKind::Type);
  }

  #[test]
  fn registered_proc_rejects_duplicate_without_replace() {
    let name = unique_name("test-dup-registered-proc");
    register_import_proc(&name, dummy_proc);

    let panicked = std::panic::catch_unwind(|| {
      register_import_proc_with_descriptor(
        &name,
        dummy_proc,
        RegisteredProcDescriptor {
          arity_min: 0,
          arity_max: None,
          platforms: vec![RegisteredProcPlatform::Native],
          stability: RegisteredProcStability::Public,
          docs_hint: None,
          callback_last: false,
        },
      );
    });

    assert!(panicked.is_err());
  }

  #[test]
  fn registered_proc_warns_once_for_experimental() {
    let name = unique_name("test-warning-registered-proc");
    register_import_proc_with_descriptor(
      &name,
      dummy_proc,
      RegisteredProcDescriptor {
        arity_min: 0,
        arity_max: None,
        platforms: vec![RegisteredProcPlatform::Native],
        stability: RegisteredProcStability::Experimental,
        docs_hint: None,
        callback_last: false,
      },
    );

    let _ = call_registered_proc(&name, vec![], &CallStackList::default()).expect("first call should pass");
    let _ = call_registered_proc(&name, vec![], &CallStackList::default()).expect("second call should pass");

    let warned = WARNED_REGISTERED_PROCS.read().expect("read warned proc names");
    assert!(warned.contains(name.as_str()));
  }
}
