mod type_checking;
mod type_inference;
mod type_rewriting;

use crate::{
  builtins::{self, is_js_syntax_procs, is_proc_name, is_registered_proc},
  calcit::{
    self, Calcit, CalcitArgLabel, CalcitCallKind, CalcitErr, CalcitErrKind, CalcitFn, CalcitFnArgs, CalcitFnTypeAnnotation, CalcitImpl,
    CalcitImport, CalcitList, CalcitLocal, CalcitNumberBinaryOp, CalcitProc, CalcitScope, CalcitStructDef, CalcitSymbolInfo,
    CalcitSyntax, CalcitTrait, CalcitTraitMemberKind, CalcitTypeAnnotation, GENERATED_DEF, ImportInfo, LocatedWarning,
    MacroExpansionType, MacroSignature, MacroSyntaxType, NodeLocation, ParamShape, ParamShapeToken, RawCodeType, SchemaKind,
    brief_type_of_value, compare_param_shapes, pop_type_slot_override, push_type_slot_override, register_type_slot,
  },
  call_stack::{CallStackList, StackKind},
  codegen, program, runner,
};

use type_checking::{
  CallTypeCheckInfo, check_core_fn_arg_types, check_function_return_type, check_local_fn_call_arg_types, check_proc_arg_types,
  check_user_fn_arg_types, detect_return_type_hint_from_processed_body,
};
pub use type_inference::infer_static_type_from_expr;
use type_inference::{
  extract_literal_list_items, find_struct_lookup_in_literal_path, fully_typed_literal_assoc_path, fully_typed_literal_lookup_path,
  infer_struct_field_type, infer_type_from_expr, resolve_enum_value, resolve_program_value_for_preprocess, resolve_type_value,
};
use type_rewriting::{
  build_enum_ref_node, build_struct_ref_node, try_rewrite_enum_args_to_named_enums, try_rewrite_local_fn_enum_args_to_named_enums,
  try_rewrite_loose_struct_args_to_structs, try_rewrite_map_args_to_structs,
};

use std::cell::Cell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, RwLock};
use std::{cell::RefCell, vec};

use cirru_edn::EdnTag;
use im_ternary_tree::TernaryTreeList;
use strum::ParseError;

pub(crate) type ScopeTypes = HashMap<Arc<str>, Arc<CalcitTypeAnnotation>>;

static WARN_DYN_METHOD: AtomicBool = AtomicBool::new(false);
static VERBOSE_PREPROCESS: AtomicBool = AtomicBool::new(false);
static PROJECT_NAMESPACES: LazyLock<RwLock<HashSet<Arc<str>>>> = LazyLock::new(|| RwLock::new(HashSet::new()));

pub fn set_project_namespaces(namespaces: &HashSet<String>) {
  let mut target = PROJECT_NAMESPACES.write().expect("write project namespaces");
  target.clear();
  target.extend(namespaces.iter().map(|ns| Arc::from(ns.as_str())));
}

fn should_emit_project_source_lint(file_ns: &str) -> bool {
  let namespaces = PROJECT_NAMESPACES.read().expect("read project namespaces");
  namespace_is_project_source(&namespaces, file_ns)
}

fn namespace_is_project_source(namespaces: &HashSet<Arc<str>>, file_ns: &str) -> bool {
  namespaces.is_empty() || namespaces.contains(file_ns)
}

fn format_inspect_type_coord(coord: &[u16]) -> String {
  format!("@{}", coord.iter().map(u16::to_string).collect::<Vec<_>>().join("."))
}

thread_local! {
  static PREPROCESS_COMPILE_GUARD: RefCell<HashSet<(Arc<str>, Arc<str>)>> = RefCell::new(HashSet::new());
  /// When set, `preprocess_defn` for anonymous `fn` will use this as the expected
  /// function type to inject arg types into the fn body's scope_types.
  /// This enables type checking for callback parameters (e.g., `d!` in `:on-click $ fn (e d!) ...`).
  static EXPECTED_FN_TYPE: RefCell<Option<Arc<CalcitFnTypeAnnotation>>> = const { RefCell::new(None) };
  /// When set, the hashmap literal preprocessor uses this struct definition to look up
  /// field types and inject EXPECTED_FN_TYPE for Fn-typed fields.
  /// This enables type propagation through struct literals (e.g., `:on-click $ fn (e d!) ...` in DomProps).
  static EXPECTED_STRUCT_TYPE: RefCell<Option<CalcitStructDef>> = const { RefCell::new(None) };
  /// Feature flags of the current function being preprocessed.
  /// Used to check whether js-ffi calls are permitted.
  static CURRENT_FN_FEATURES: RefCell<Option<Arc<HashSet<EdnTag>>>> = const { RefCell::new(None) };
  static PREPROCESS_DEPTH: Cell<usize> = const { Cell::new(0) };
}

pub fn set_verbose_preprocess(enabled: bool) {
  VERBOSE_PREPROCESS.store(enabled, Ordering::SeqCst);
}

struct PreprocessTrace {
  ns: Arc<str>,
  def: Arc<str>,
}

impl PreprocessTrace {
  fn enter(ns: &str, def: &str) -> Option<Self> {
    if !VERBOSE_PREPROCESS.load(Ordering::Relaxed) {
      return None;
    }
    PREPROCESS_DEPTH.with(|depth| {
      eprintln!("[verbose] preprocess enter depth={} {ns}/{def}", depth.get());
      depth.set(depth.get() + 1);
    });
    Some(Self {
      ns: Arc::from(ns),
      def: Arc::from(def),
    })
  }
}

impl Drop for PreprocessTrace {
  fn drop(&mut self) {
    PREPROCESS_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    eprintln!("[verbose] preprocess leave {}/{}", self.ns, self.def);
  }
}

fn with_preprocess_compile_guard<T>(ns: &str, def: &str, f: impl FnOnce() -> Result<T, CalcitErr>) -> Result<Option<T>, CalcitErr> {
  let key = (Arc::from(ns), Arc::from(def));
  let inserted = PREPROCESS_COMPILE_GUARD.with(|guard| guard.borrow_mut().insert(key.clone()));
  if !inserted {
    return Ok(None);
  }

  let result = f();
  PREPROCESS_COMPILE_GUARD.with(|guard| {
    guard.borrow_mut().remove(&key);
  });
  result.map(Some)
}

pub fn set_warn_dyn_method(enabled: bool) {
  WARN_DYN_METHOD.store(enabled, Ordering::SeqCst);
}

fn warn_dyn_method_enabled() -> bool {
  WARN_DYN_METHOD.load(Ordering::Relaxed)
}

pub fn is_warn_dyn_method_enabled() -> bool {
  warn_dyn_method_enabled()
}

pub(crate) fn tag_annotation(name: &str) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::from_tag_name(name))
}

fn removed_data_api_replacement(name: &str) -> Option<String> {
  match name {
    "tuple?" => Some("enum? (values) or enum-def? (definitions)".to_owned()),
    "tuple-enum" => Some("enum-definition".to_owned()),
    "&record:struct" => Some("&struct:definition".to_owned()),
    "&tuple:enum" => Some("&enum:definition".to_owned()),
    "&tuple:enum-has-variant?" => Some("&enum-def:has-variant?".to_owned()),
    "&tuple:enum-variant-arity" => Some("&enum-def:variant-arity".to_owned()),
    "&tuple:validate-enum" => Some("&enum:validate".to_owned()),
    _ if name.starts_with("&record:") => Some(name.replacen("&record:", "&struct:", 1)),
    _ if name.starts_with("&tuple:") => Some(name.replacen("&tuple:", "&enum:", 1)),
    _ => None,
  }
}

fn warn_on_removed_data_api_call(
  head: &Calcit,
  call_location: Option<NodeLocation>,
  file_ns: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  let Calcit::Import(CalcitImport { ns, def, .. }) = head else {
    return;
  };
  if ns.as_ref() != calcit::CORE_NS {
    return;
  }
  let Some(replacement) = removed_data_api_replacement(def) else {
    return;
  };
  let message = format!("[Warn] `{def}` was removed by the struct/enum data-model migration; use `{replacement}`");
  if let Some(location) = call_location {
    gen_check_warning_with_location_code(message, "W_REMOVED_DATA_API", location, check_warnings);
  } else {
    gen_check_warning_code(message, "W_REMOVED_DATA_API", file_ns, check_warnings);
  }
}

/// An anonymous struct has a runtime field set but no nominal declaration from
/// which the preprocessor can derive a required field type.
fn is_anonymous_struct_type(type_info: &CalcitTypeAnnotation) -> bool {
  matches!(
    type_info,
    CalcitTypeAnnotation::Custom(value)
      if matches!(value.as_ref(), Calcit::Tag(tag) if matches!(tag.ref_str().trim_start_matches(':'), "record" | "struct"))
  )
}

struct RequiredStructFieldWarningContext<'a> {
  file_ns: &'a str,
  def_name: &'a str,
  location: Option<NodeLocation>,
  call_stack: &'a CallStackList,
}

fn find_calcit_location_matching(value: &Calcit, predicate: fn(&NodeLocation) -> bool) -> Option<NodeLocation> {
  match value {
    Calcit::List(items) => items.iter().find_map(|item| find_calcit_location_matching(item, predicate)),
    Calcit::Recur(items) => items.iter().find_map(|item| find_calcit_location_matching(item, predicate)),
    _ => value.get_location().filter(predicate),
  }
}

fn warn_required_struct_field_type(
  field_name: &str,
  receiver: &Calcit,
  receiver_type: Option<&CalcitTypeAnnotation>,
  context: RequiredStructFieldWarningContext<'_>,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  let RequiredStructFieldWarningContext {
    file_ns,
    def_name,
    location,
    call_stack,
  } = context;
  // Macros such as `defimpl`, `tag-match`, and typed `%{}` constructors may
  // create tag-access forms without a source coordinate. Those probes are
  // implementation details, not source-level required Struct reads. Preserve
  // diagnostics whenever either side still carries an attributable location.
  let attributable_location = location
    .as_ref()
    .filter(|location| location.def.as_ref() != GENERATED_DEF)
    .cloned()
    .or_else(|| find_calcit_location_matching(receiver, |location| location.def.as_ref() != GENERATED_DEF));
  let generated_context = location.as_ref().is_some_and(|location| location.def.as_ref() == GENERATED_DEF)
    || find_calcit_location_matching(receiver, |location| location.def.as_ref() == GENERATED_DEF).is_some()
    || call_stack.0.iter().any(|frame| matches!(frame.kind, StackKind::Macro));
  if attributable_location.is_none() && generated_context {
    return;
  }

  let receiver_type_text = receiver_type
    .map(CalcitTypeAnnotation::to_brief_string)
    .unwrap_or_else(|| ":unknown".to_owned());
  let message = format!(
    "[Warn] required field access `(:{field_name} value)` needs a statically typed Struct with a declared `:{field_name}` field, but the receiver is `{receiver_type_text}` at {file_ns}/{def_name}. Define the Struct and narrow/unwrap the receiver first; use `(get value :{field_name})` only when absence is intentional and handle the returned Option"
  );
  gen_check_warning_code_at(
    message,
    "W_REQUIRED_STRUCT_FIELD_TYPE",
    file_ns,
    attributable_location,
    check_warnings,
  );
}

/// Extract type information from a Calcit definition
/// Functions and procs are converted into `CalcitTypeAnnotation::Function` to retain argument/return hints
/// Other values fall back to their concrete annotation.
pub struct PreprocessContext<'a> {
  scope_defs: &'a HashSet<Arc<str>>,
  scope_types: &'a mut ScopeTypes,
  file_ns: &'a str,
  check_warnings: &'a RefCell<Vec<LocatedWarning>>,
  call_stack: &'a CallStackList,
}

impl<'a> PreprocessContext<'a> {
  fn new(
    scope_defs: &'a HashSet<Arc<str>>,
    scope_types: &'a mut ScopeTypes,
    file_ns: &'a str,
    check_warnings: &'a RefCell<Vec<LocatedWarning>>,
    call_stack: &'a CallStackList,
  ) -> Self {
    Self {
      scope_defs,
      scope_types,
      file_ns,
      check_warnings,
      call_stack,
    }
  }
}

fn store_preprocessed_compiled_output(ns: &str, def: &str, source_code: &Calcit, resolved_code: &Calcit) {
  let preprocessed_code = resolved_code.to_owned();
  let codegen_form = resolved_code.to_owned();
  let deps = program::collect_compiled_deps(&codegen_form);
  let type_summary = calcit::CalcitTypeAnnotation::summarize_code(source_code).map(Arc::from);
  program::store_compiled_output(
    ns,
    def,
    program::CompiledDefPayload {
      version_id: 0,
      preprocessed_code,
      codegen_form,
      deps,
      type_summary,
      source_code: Some(source_code.to_owned()),
      schema: program::lookup_def_schema(ns, def),
      doc: program::lookup_def_doc(ns, def).map(Arc::from).unwrap_or_else(|| Arc::from("")),
      examples: program::lookup_def_examples(ns, def).unwrap_or_default(),
    },
  );
}

fn ensure_ns_def_preprocessed(
  raw_ns: &str,
  raw_def: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  let ns = raw_ns;
  let def = raw_def;

  if program::lookup_compiled_def(ns, def).is_some() {
    return Ok(());
  }

  let _trace = PreprocessTrace::enter(ns, def);

  // Save and clear EXPECTED_FN_TYPE to prevent leaking into nested def compilation.
  // The calling context's fn type hint is only meant for the immediate expression being processed,
  // not for defs compiled transitively during symbol resolution.
  let saved_fn_type = EXPECTED_FN_TYPE.with(|cell| cell.borrow_mut().take());
  let saved_struct_type = EXPECTED_STRUCT_TYPE.with(|cell| cell.borrow_mut().take());

  let Some(()) = with_preprocess_compile_guard(ns, def, || match program::lookup_def_code(ns, def) {
    Some(code) => {
      let next_stack = call_stack.extend(ns, def, StackKind::Fn, &code, &[]);

      let mut scope_types = ScopeTypes::new();
      let context_label = format!("{ns}/{def}");
      let resolved_code = builtins::meta::with_compiling_def(ns, def, || {
        calcit::with_type_annotation_warning_context(context_label, || {
          preprocess_expr(&code, &HashSet::new(), &mut scope_types, ns, check_warnings, &next_stack)
        })
      })?;
      store_preprocessed_compiled_output(ns, def, &code, &resolved_code);

      Ok(())
    }
    None if ns.starts_with('|') || ns.starts_with('"') => Ok(()),
    None => {
      let loc = NodeLocation::new(Arc::from(ns), Arc::from(def), Arc::from(vec![]));
      Err(CalcitErr::use_msg_stack_location(
        CalcitErrKind::Var,
        format!("unknown ns/def in program: {ns}/{def}"),
        call_stack,
        Some(loc),
      ))
    }
  })?
  else {
    // Restore saved type hints (even when compilation was skipped by the guard)
    EXPECTED_FN_TYPE.with(|cell| *cell.borrow_mut() = saved_fn_type);
    EXPECTED_STRUCT_TYPE.with(|cell| *cell.borrow_mut() = saved_struct_type);
    return Ok(());
  };

  // Restore saved type hints after compilation
  EXPECTED_FN_TYPE.with(|cell| *cell.borrow_mut() = saved_fn_type);
  EXPECTED_STRUCT_TYPE.with(|cell| *cell.borrow_mut() = saved_struct_type);

  Ok(())
}

pub fn ensure_ns_def_compiled(
  raw_ns: &str,
  raw_def: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  ensure_ns_def_preprocessed(raw_ns, raw_def, check_warnings, call_stack)
}

/// Preprocess a `(with-type-slot (:slot-name TypeExpr) body...)` form.
///
/// The first argument must be a two-element list `(:slot-name TypeExpr)`.
/// The type is resolved and pushed as a scoped override, then all body
/// expressions are preprocessed under that scope, and finally the override
/// is popped. The wrapper is always erased from the preprocessed output.
fn preprocess_with_type_slot_block(
  head_form: &Calcit,
  args: &CalcitList,
  scope_defs: &HashSet<Arc<str>>,
  scope_types: &mut ScopeTypes,
  file_ns: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  let head_location = head_form.get_location();
  // --- Parse the binding pair ---
  let Some(binding_expr) = args.first() else {
    return Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Arity,
      "with-type-slot expected a binding pair as first argument, e.g. `(:dispatch-op Op)`",
      call_stack,
      head_location,
    ));
  };

  let Calcit::List(binding_list) = binding_expr else {
    return Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Type,
      format!("with-type-slot binding must be a list, got: {binding_expr}"),
      call_stack,
      binding_expr.get_location(),
    ));
  };

  if binding_list.len() != 2 {
    return Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Arity,
      format!(
        "with-type-slot binding must be a 2-element list (:slot-name TypeExpr), got {} elements",
        binding_list.len()
      ),
      call_stack,
      binding_expr.get_location(),
    ));
  }

  // Slot name (first element of binding pair)
  let slot_name: Arc<str> = match &binding_list[0] {
    Calcit::Tag(tag) => Arc::from(tag.ref_str()),
    Calcit::Str(s) => Arc::from(s.as_ref()),
    other => {
      return Err(CalcitErr::use_msg_stack_location(
        CalcitErrKind::Type,
        format!("with-type-slot name must be a tag or string, got: {other}"),
        call_stack,
        other.get_location(),
      ));
    }
  };

  // Type expression (second element of binding pair — preprocess it first)
  let raw_type_expr = &binding_list[1];
  let processed_type_expr = preprocess_expr(raw_type_expr, scope_defs, scope_types, file_ns, check_warnings, call_stack)?;

  // Resolve the preprocessed expression to a type annotation
  let resolved = match &processed_type_expr {
    Calcit::Import(crate::calcit::CalcitImport { ns, def, .. }) => resolve_program_value_for_preprocess(ns, def, None),
    Calcit::Symbol { sym, info, .. } => resolve_program_value_for_preprocess(&info.at_ns, sym, None),
    other => Some(other.to_owned()),
  };
  let Some(resolved) = resolved else {
    return Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Unexpected,
      format!("with-type-slot could not resolve type value for slot `{slot_name}`"),
      call_stack,
      raw_type_expr.get_location(),
    ));
  };

  let import_path: Option<(Arc<str>, Arc<str>)> = match &processed_type_expr {
    Calcit::Import(crate::calcit::CalcitImport { ns, def, .. }) => Some((ns.clone(), def.clone())),
    _ => None,
  };

  let type_annotation: Arc<CalcitTypeAnnotation> = if let Some((ns, def)) = &import_path {
    match &resolved {
      Calcit::EnumDef(_) | Calcit::StructDef(_) => {
        Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from(format!("{ns}/{def}")), Arc::new(vec![])))
      }
      Calcit::Struct(struct_value) => Arc::new(CalcitTypeAnnotation::StructValue(struct_value.struct_ref.clone())),
      other => {
        return Err(CalcitErr::use_msg_stack_location(
          CalcitErrKind::Unexpected,
          format!(
            "with-type-slot expected an enum or struct import, got: {}",
            brief_type_of_value(other)
          ),
          call_stack,
          raw_type_expr.get_location(),
        ));
      }
    }
  } else {
    match &resolved {
      Calcit::EnumDef(enum_def) => Arc::new(CalcitTypeAnnotation::Enum(Arc::new(enum_def.to_owned()), Arc::new(vec![]))),
      Calcit::StructDef(struct_def) => Arc::new(CalcitTypeAnnotation::Struct(Arc::new(struct_def.to_owned()), Arc::new(vec![]))),
      Calcit::Struct(struct_value) => Arc::new(CalcitTypeAnnotation::StructValue(struct_value.struct_ref.clone())),
      other => match infer_type_from_expr(other, scope_types) {
        Some(inferred)
          if matches!(
            inferred.as_ref(),
            CalcitTypeAnnotation::Enum(_, _) | CalcitTypeAnnotation::Struct(_, _) | CalcitTypeAnnotation::StructValue(_)
          ) =>
        {
          inferred
        }
        _ => {
          return Err(CalcitErr::use_msg_stack_location(
            CalcitErrKind::Unexpected,
            format!(
              "with-type-slot expected an enum, struct, or record as type value, got: {}",
              brief_type_of_value(other)
            ),
            call_stack,
            raw_type_expr.get_location(),
          ));
        }
      },
    }
  };

  let body_args = args.drop_left();
  if body_args.is_empty() {
    return Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Arity,
      "with-type-slot expected at least one body expression",
      call_stack,
      head_location,
    ));
  }

  // Push the scoped override
  push_type_slot_override(slot_name.clone(), type_annotation);

  // Preprocess body expressions under the override
  let mut preprocessed_body: Vec<Calcit> = Vec::with_capacity(body_args.len());
  let mut preprocess_err: Option<CalcitErr> = None;
  for expr in body_args.iter() {
    match preprocess_expr(expr, scope_defs, scope_types, file_ns, check_warnings, call_stack) {
      Ok(form) => preprocessed_body.push(form),
      Err(e) => {
        preprocess_err = Some(e);
        break;
      }
    }
  }

  // Always pop the override, even on error
  pop_type_slot_override(&slot_name);

  if let Some(e) = preprocess_err {
    return Err(e);
  }

  // The binding is compile-time-only and must never escape into runtime/codegen.
  if preprocessed_body.len() == 1 {
    return Ok(preprocessed_body.remove(0));
  }
  // `do` expands to `&let () body...`; construct the expanded sequence directly so
  // already-preprocessed body expressions are not expanded or checked a second time.
  let mut result_items = vec![
    Calcit::Syntax(CalcitSyntax::CoreLet, Arc::from(file_ns)),
    Calcit::from(CalcitList::default()),
  ];
  result_items.extend(preprocessed_body);
  Ok(Calcit::from(CalcitList::from(result_items.as_slice())))
}

fn lookup_callable_ns_def_for_preprocess(
  raw_ns: &str,
  raw_def: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<Option<Calcit>, CalcitErr> {
  ensure_ns_def_compiled(raw_ns, raw_def, check_warnings, call_stack)?;
  Ok(
    match program::resolve_compiled_executable_def(raw_ns, raw_def, call_stack).ok().flatten() {
      value @ Some(Calcit::Macro { .. } | Calcit::Fn { .. }) => value,
      _ => None,
    },
  )
}

fn resolve_trait_def_from_source_code(code: &Calcit) -> Option<CalcitTrait> {
  if let Calcit::Thunk(thunk) = code {
    return resolve_trait_def_from_source_code(thunk.get_code());
  }

  let Calcit::List(items) = code else {
    return None;
  };

  if let Some(head) = items.first()
    && (matches!(head, Calcit::Syntax(CalcitSyntax::Quote, _))
      || matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "quote")
      || matches!(head, Calcit::Import(CalcitImport { ns, def, .. }) if &**ns == calcit::CORE_NS && &**def == "quote"))
    && let Some(inner) = items.get(1)
  {
    return resolve_trait_def_from_source_code(inner);
  }

  let head = items.first()?;
  if matches!(head, Calcit::Proc(CalcitProc::NativeTraitNew))
    || matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "&trait::new")
    || matches!(head, Calcit::Import(CalcitImport { ns, def, .. }) if &**ns == calcit::CORE_NS && &**def == "&trait::new")
  {
    return parse_trait_new_source(items.as_ref());
  }
  if matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "deftrait")
    || matches!(head, Calcit::Import(CalcitImport { ns, def, .. }) if &**ns == calcit::CORE_NS && &**def == "deftrait")
  {
    return parse_deftrait_source(items.as_ref());
  }

  None
}

fn parse_trait_name_from_source(form: &Calcit) -> Option<EdnTag> {
  match form {
    Calcit::Symbol { sym, .. } | Calcit::Str(sym) => Some(EdnTag::from(sym.as_ref())),
    Calcit::Tag(tag) => Some(tag.to_owned()),
    _ => None,
  }
}

fn parse_trait_method_name_from_source(form: &Calcit) -> Option<EdnTag> {
  match form {
    Calcit::Method(name, _) | Calcit::Symbol { sym: name, .. } | Calcit::Str(name) => Some(EdnTag::from(name.as_ref())),
    Calcit::Tag(tag) => Some(tag.to_owned()),
    _ => None,
  }
}

fn parse_trait_member_kind_from_source(form: &Calcit) -> CalcitTraitMemberKind {
  match form {
    Calcit::Tag(_) => CalcitTraitMemberKind::Field,
    _ => CalcitTraitMemberKind::Method,
  }
}

type TraitMemberSpecs = (Vec<EdnTag>, Vec<Arc<CalcitTypeAnnotation>>, Vec<CalcitTraitMemberKind>);

fn parse_trait_method_specs_from_source<'a>(items: impl Iterator<Item = &'a Calcit>) -> Option<TraitMemberSpecs> {
  let mut methods: Vec<EdnTag> = vec![];
  let mut method_types: Vec<Arc<CalcitTypeAnnotation>> = vec![];
  let mut member_kinds: Vec<CalcitTraitMemberKind> = vec![];

  for item in items {
    let Calcit::List(entry) = item else {
      return None;
    };
    if entry.len() != 2 {
      return None;
    }

    let method_name = parse_trait_method_name_from_source(entry.first()?)?;
    let type_form = entry.get(1)?;
    let method_type = calcit::with_type_annotation_warning_context(format!("trait:{}", method_name.ref_str()), || {
      CalcitTypeAnnotation::parse_type_annotation_form(type_form)
    });
    methods.push(method_name);
    method_types.push(method_type);
    member_kinds.push(parse_trait_member_kind_from_source(entry.first()?));
  }

  Some((methods, method_types, member_kinds))
}

fn parse_trait_new_source(items: &CalcitList) -> Option<CalcitTrait> {
  let name = parse_trait_name_from_source(items.get(1)?)?;
  let method_specs = match items.get(2)? {
    Calcit::List(list) => list,
    _ => return None,
  };
  let (methods, method_types, member_kinds) = parse_trait_method_specs_from_source(method_specs.iter())?;
  Some(CalcitTrait::new_with_member_kinds(name, methods, method_types, Some(member_kinds)))
}

fn parse_deftrait_source(items: &CalcitList) -> Option<CalcitTrait> {
  let name = parse_trait_name_from_source(items.get(1)?)?;
  let (methods, method_types, member_kinds) = parse_trait_method_specs_from_source(items.iter().skip(2))?;
  Some(CalcitTrait::new_with_member_kinds(name, methods, method_types, Some(member_kinds)))
}

fn resolve_where_bound_type_for_body(bound: &crate::calcit::CalcitGenericBound, file_ns: &str) -> Option<Arc<CalcitTypeAnnotation>> {
  let mut traits = Vec::with_capacity(bound.traits.len());
  for trait_ref in bound.traits.iter() {
    if !trait_ref.methods.is_empty() {
      traits.push(trait_ref.clone());
      continue;
    }

    let raw_name = trait_ref.name.ref_str();
    let (trait_ns, trait_name) = if let Some((ns, name)) = raw_name.rsplit_once('/') {
      (Arc::from(ns), Arc::from(name))
    } else if program::has_def_code(file_ns, raw_name) {
      (Arc::from(file_ns), Arc::from(raw_name))
    } else if let Some(target_ns) = program::lookup_def_target_in_import(file_ns, raw_name) {
      (target_ns, Arc::from(raw_name))
    } else if program::has_def_code(calcit::CORE_NS, raw_name) {
      (Arc::from(calcit::CORE_NS), Arc::from(raw_name))
    } else {
      return None;
    };

    let resolved = program::lookup_def_code(&trait_ns, &trait_name)
      .and_then(|code| resolve_trait_def_from_source_code(&code))?
      .with_definition_ref(&trait_ns, &trait_name);
    traits.push(Arc::new(resolved));
  }

  match traits.len() {
    0 => None,
    1 => Some(Arc::new(CalcitTypeAnnotation::Trait(traits.remove(0)))),
    _ => Some(Arc::new(CalcitTypeAnnotation::TraitSet(Arc::new(traits)))),
  }
}

fn map_type_refs_for_body<F>(annotation: Arc<CalcitTypeAnnotation>, resolve_type_ref: &F) -> Arc<CalcitTypeAnnotation>
where
  F: Fn(&Arc<str>, Arc<Vec<Arc<CalcitTypeAnnotation>>>) -> Arc<CalcitTypeAnnotation>,
{
  match annotation.as_ref() {
    CalcitTypeAnnotation::TypeRef(name, args) => {
      let resolved_args = Arc::new(
        args
          .iter()
          .map(|arg| map_type_refs_for_body(arg.clone(), resolve_type_ref))
          .collect::<Vec<_>>(),
      );
      resolve_type_ref(name, resolved_args)
    }
    CalcitTypeAnnotation::List(inner) => Arc::new(CalcitTypeAnnotation::List(map_type_refs_for_body(inner.clone(), resolve_type_ref))),
    CalcitTypeAnnotation::Map(key, value) => Arc::new(CalcitTypeAnnotation::Map(
      map_type_refs_for_body(key.clone(), resolve_type_ref),
      map_type_refs_for_body(value.clone(), resolve_type_ref),
    )),
    CalcitTypeAnnotation::Set(inner) => Arc::new(CalcitTypeAnnotation::Set(map_type_refs_for_body(inner.clone(), resolve_type_ref))),
    CalcitTypeAnnotation::Ref(inner) => Arc::new(CalcitTypeAnnotation::Ref(map_type_refs_for_body(inner.clone(), resolve_type_ref))),
    CalcitTypeAnnotation::Optional(inner) => Arc::new(CalcitTypeAnnotation::Optional(map_type_refs_for_body(
      inner.clone(),
      resolve_type_ref,
    ))),
    CalcitTypeAnnotation::JsNullish(inner) => Arc::new(CalcitTypeAnnotation::JsNullish(map_type_refs_for_body(
      inner.clone(),
      resolve_type_ref,
    ))),
    CalcitTypeAnnotation::Variadic(inner) => Arc::new(CalcitTypeAnnotation::Variadic(map_type_refs_for_body(
      inner.clone(),
      resolve_type_ref,
    ))),
    CalcitTypeAnnotation::Fn(signature) => Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      generics: signature.generics.clone(),
      where_bounds: signature.where_bounds.clone(),
      arg_types: signature
        .arg_types
        .iter()
        .map(|arg| map_type_refs_for_body(arg.clone(), resolve_type_ref))
        .collect(),
      return_type: map_type_refs_for_body(signature.return_type.clone(), resolve_type_ref),
      fn_kind: signature.fn_kind,
      rest_type: signature
        .rest_type
        .as_ref()
        .map(|rest| map_type_refs_for_body(rest.clone(), resolve_type_ref)),
      features: signature.features.clone(),
    }))),
    CalcitTypeAnnotation::Struct(struct_def, args) => Arc::new(CalcitTypeAnnotation::Struct(
      struct_def.clone(),
      Arc::new(
        args
          .iter()
          .map(|arg| map_type_refs_for_body(arg.clone(), resolve_type_ref))
          .collect(),
      ),
    )),
    CalcitTypeAnnotation::Enum(enum_def, args) => Arc::new(CalcitTypeAnnotation::Enum(
      enum_def.clone(),
      Arc::new(
        args
          .iter()
          .map(|arg| map_type_refs_for_body(arg.clone(), resolve_type_ref))
          .collect(),
      ),
    )),
    _ => annotation,
  }
}

/// Resolve named types declared in the same lexical scope, such as a `defstruct`
/// bound by an enclosing `let`. Program-level `TypeRef` resolution cannot see
/// those definitions, so retaining the symbolic reference would make otherwise
/// precise function parameters fall back to `Dynamic` inside the body.
fn resolve_local_type_refs_for_body(annotation: Arc<CalcitTypeAnnotation>, scope_types: &ScopeTypes) -> Arc<CalcitTypeAnnotation> {
  map_type_refs_for_body(annotation, &|name, resolved_args| {
    let lookup_name = name.trim_start_matches('\'').trim_start_matches(':');
    let short_name = lookup_name.rsplit('/').next().unwrap_or(lookup_name);
    let local_type = scope_types.get(lookup_name).or_else(|| scope_types.get(short_name));
    match local_type.map(AsRef::as_ref) {
      Some(CalcitTypeAnnotation::StructDef(struct_def)) => Arc::new(CalcitTypeAnnotation::Struct(struct_def.clone(), resolved_args)),
      Some(CalcitTypeAnnotation::Struct(struct_def, _)) | Some(CalcitTypeAnnotation::StructValue(struct_def)) => {
        Arc::new(CalcitTypeAnnotation::Struct(struct_def.clone(), resolved_args))
      }
      Some(CalcitTypeAnnotation::EnumDef(enum_def)) => Arc::new(CalcitTypeAnnotation::Enum(enum_def.clone(), resolved_args)),
      Some(CalcitTypeAnnotation::Enum(enum_def, _)) | Some(CalcitTypeAnnotation::EnumValue(enum_def)) => {
        Arc::new(CalcitTypeAnnotation::Enum(enum_def.clone(), resolved_args))
      }
      Some(CalcitTypeAnnotation::Trait(trait_def)) if resolved_args.is_empty() => {
        Arc::new(CalcitTypeAnnotation::Trait(trait_def.clone()))
      }
      _ => Arc::new(CalcitTypeAnnotation::TypeRef(name.clone(), resolved_args)),
    }
  })
}

/// Qualify nominal type references from the namespace where their declaration
/// was written. Nested Struct fields often use concise forms such as
/// `'Router`; once that field type flows into a caller in another namespace,
/// retaining only `Router` is ambiguous and prevents required-field lowering.
fn resolve_namespace_type_refs_for_body(annotation: Arc<CalcitTypeAnnotation>, declaring_ns: &str) -> Arc<CalcitTypeAnnotation> {
  map_type_refs_for_body(annotation, &|name, resolved_args| {
    let stripped = name.trim_start_matches('\'').trim_start_matches(':');
    let qualified_name = if let Some((prefix, def)) = stripped.rsplit_once('/') {
      if program::has_def_code(prefix, def) {
        Arc::from(stripped)
      } else if let Some(target_ns) = program::lookup_ns_target_in_import(declaring_ns, prefix) {
        Arc::from(format!("{target_ns}/{def}"))
      } else {
        Arc::from(stripped)
      }
    } else if program::has_def_code(declaring_ns, stripped) {
      Arc::from(format!("{declaring_ns}/{stripped}"))
    } else if let Some(target_ns) = program::lookup_def_target_in_import(declaring_ns, stripped) {
      Arc::from(format!("{target_ns}/{stripped}"))
    } else if program::has_def_code(calcit::CORE_NS, stripped) {
      Arc::from(format!("{}/{stripped}", calcit::CORE_NS))
    } else {
      name.clone()
    };
    Arc::new(CalcitTypeAnnotation::TypeRef(qualified_name, resolved_args))
  })
}

fn unwrap_named_body_parameter_type(annotation: Arc<CalcitTypeAnnotation>, parameter: Option<&Arc<str>>) -> Arc<CalcitTypeAnnotation> {
  let Some(parameter) = parameter else {
    return annotation;
  };
  match annotation.as_ref() {
    CalcitTypeAnnotation::TypeRef(label, args)
      if args.len() == 1 && label.rsplit('/').next().is_some_and(|name| name == parameter.as_ref()) =>
    {
      args.first().expect("checked one named parameter type").clone()
    }
    _ => annotation,
  }
}

fn lookup_trait_ns_def_for_preprocess(
  raw_ns: &str,
  raw_def: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<Option<Arc<CalcitTrait>>, CalcitErr> {
  ensure_ns_def_compiled(raw_ns, raw_def, check_warnings, call_stack)?;
  Ok(
    program::lookup_compiled_def(raw_ns, raw_def)
      .and_then(|compiled| compiled.source_code)
      .and_then(|code| resolve_trait_def_from_source_code(&code))
      .map(|trait_def| trait_def.with_definition_ref(raw_ns, raw_def))
      .map(Arc::new),
  )
}

pub fn compile_source_def_for_snapshot(
  ns: &str,
  def: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  if program::lookup_compiled_def(ns, def).is_some() {
    return Ok(());
  }

  let Some(code) = program::lookup_def_code(ns, def) else {
    let loc = NodeLocation::new(Arc::from(ns), Arc::from(def), Arc::from(vec![]));
    return Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Var,
      format!("unknown ns/def in program: {ns}/{def}"),
      call_stack,
      Some(loc),
    ));
  };

  let mut scope_types = ScopeTypes::new();
  let context_label = format!("{ns}/{def}");
  let resolved_code = builtins::meta::with_compiling_def(ns, def, || {
    calcit::with_type_annotation_warning_context(context_label, || {
      preprocess_expr(&code, &HashSet::new(), &mut scope_types, ns, check_warnings, call_stack)
    })
  })?;

  store_preprocessed_compiled_output(ns, def, &code, &resolved_code);

  Ok(())
}

/// Executable call nodes are immutable after preprocessing and favor contiguous
/// traversal over persistent list updates. Quoted/list values stay untouched.
fn into_executable_call(expr: Calcit) -> Calcit {
  match expr {
    Calcit::List(items) if matches!(items.as_ref(), CalcitList::List(_)) => {
      Calcit::from(CalcitList::executable(items.to_vec(), CalcitCallKind::Normal))
    }
    _ => expr,
  }
}

fn classify_number_binary_call(head: &Calcit, args: &[Calcit], scope_types: &ScopeTypes) -> CalcitCallKind {
  let Calcit::Proc(proc) = head else {
    return CalcitCallKind::Normal;
  };
  let operation = match proc {
    CalcitProc::NativeAdd => CalcitNumberBinaryOp::Add,
    CalcitProc::NativeMinus => CalcitNumberBinaryOp::Subtract,
    CalcitProc::NativeMultiply => CalcitNumberBinaryOp::Multiply,
    CalcitProc::NativeDivide => CalcitNumberBinaryOp::Divide,
    CalcitProc::NativeNumberRem => CalcitNumberBinaryOp::Remainder,
    CalcitProc::NativeLessThan => CalcitNumberBinaryOp::LessThan,
    CalcitProc::NativeGreaterThan => CalcitNumberBinaryOp::GreaterThan,
    _ => return CalcitCallKind::Normal,
  };
  if args.len() != 2
    || !args
      .iter()
      .all(|arg| matches!(resolve_type_value(arg, scope_types).as_deref(), Some(CalcitTypeAnnotation::Number)))
  {
    return CalcitCallKind::Normal;
  }
  CalcitCallKind::NumberBinary(operation)
}

/// Build a location-free import for a compiler-generated core call.
fn core_import(def: &str, file_ns: &str) -> Calcit {
  Calcit::Import(CalcitImport {
    ns: calcit::CORE_NS.into(),
    def: Arc::from(def),
    info: Arc::new(ImportInfo::Core { at_ns: Arc::from(file_ns) }),
    def_id: Some(program::ensure_def_id(calcit::CORE_NS, def).0),
  })
}

/// Store a compiler-generated call in executable list form.
fn generated_call(items: Vec<Calcit>) -> Calcit {
  Calcit::from(CalcitList::from(items.as_slice()))
}

/// Build a compiler-generated call to an ordinary core definition.
fn generated_core_call(def: &str, args: Vec<Calcit>, file_ns: &str) -> Calcit {
  let mut items = Vec::with_capacity(args.len() + 1);
  items.push(core_import(def, file_ns));
  items.extend(args);
  generated_call(items)
}

/// Build an `if` used only inside a generated specialization.
fn generated_if(condition: Calcit, then_branch: Calcit, else_branch: Calcit, file_ns: &str) -> Calcit {
  generated_call(vec![
    Calcit::Syntax(CalcitSyntax::If, Arc::from(file_ns)),
    condition,
    then_branch,
    else_branch,
  ])
}

/// Bind one generated temporary while retaining source evaluation order.
fn generated_let(binding: Calcit, value: Calcit, body: Calcit, file_ns: &str) -> Calcit {
  generated_call(vec![
    Calcit::Syntax(CalcitSyntax::CoreLet, Arc::from(file_ns)),
    generated_call(vec![binding, value]),
    body,
  ])
}

/// Lower the strict core `let` macro without executing its small recursive
/// macro body. Invalid binding shapes deliberately keep the general path so
/// the macro retains its established diagnostics.
fn try_lower_core_let_macro(args: &CalcitList, file_ns: &str) -> Option<Calcit> {
  let Calcit::List(pairs) = args.first()? else {
    return None;
  };
  if !pairs.iter().all(|pair| {
    matches!(
      pair,
      Calcit::List(binding)
        if binding.is_empty() || (binding.len() == 2 && matches!(binding.first(), Some(Calcit::Symbol { .. })))
    )
  }) {
    return None;
  }

  let core_let = || Calcit::Syntax(CalcitSyntax::CoreLet, Arc::from(file_ns));
  let mut body: Vec<Calcit> = args.iter().skip(1).cloned().collect();
  if pairs.is_empty() {
    let mut items = vec![core_let(), Calcit::from(CalcitList::default())];
    items.append(&mut body);
    return Some(Calcit::from(CalcitList::from(items.as_slice())));
  }

  for index in (0..pairs.len()).rev() {
    let mut items = vec![core_let(), pairs[index].to_owned()];
    items.append(&mut body);
    body = vec![Calcit::from(CalcitList::from(items.as_slice()))];
  }
  body.pop()
}

/// Lower the strict core `{}` macro into the internal flat map constructor.
/// Malformed pairs retain the ordinary macro path and its diagnostics.
fn try_lower_core_map_macro(args: &CalcitList) -> Option<Calcit> {
  let mut items = vec![Calcit::Proc(CalcitProc::NativeMap)];
  for pair in args.iter() {
    let Calcit::List(pair_items) = pair else {
      return None;
    };
    if pair_items.len() != 2 {
      return None;
    }
    items.extend(pair_items.iter().cloned());
  }
  Some(Calcit::from(CalcitList::from(items.as_slice())))
}

/// Wrap a body in left-to-right generated temporary bindings.
fn generated_lets(bindings: Vec<(Calcit, Calcit)>, mut body: Calcit, file_ns: &str) -> Calcit {
  for (binding, value) in bindings.into_iter().rev() {
    body = generated_let(binding, value, body, file_ns);
  }
  body
}

/// Allocate a hygienic local for a generated path expression.
fn generated_path_symbol(prefix: &str, file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = CalcitList::from(&[Calcit::Str(Arc::from(prefix))] as &[Calcit]);
  builtins::syntax::gensym(&args.view(), &CalcitScope::default(), file_ns, call_stack)
}

/// Expand one Option-preserving `get-in` traversal step.
fn generated_get_in_step(current: Calcit, path: &[Calcit], file_ns: &str, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let none = generated_core_call("%none", vec![], file_ns);
  let present = if let Some((key, rest)) = path.split_first() {
    let payload = generated_path_symbol("typed_path_value", file_ns, call_stack)?;
    let some_pattern = generated_call(vec![Calcit::tag("some"), payload.to_owned()]);
    let none_pattern = generated_call(vec![Calcit::tag("none")]);
    let some_branch = generated_call(vec![some_pattern, generated_get_in_step(payload, rest, file_ns, call_stack)?]);
    let none_branch = generated_call(vec![none_pattern, none.to_owned()]);
    let matched = generated_call(vec![
      Calcit::Syntax(CalcitSyntax::Match, Arc::from(file_ns)),
      generated_core_call("get", vec![current.to_owned(), key.to_owned()], file_ns),
      some_branch,
      none_branch,
    ]);
    generated_if(
      generated_call(vec![Calcit::Proc(CalcitProc::StructQuestion), current.to_owned()]),
      generated_call(vec![
        Calcit::Proc(CalcitProc::Raise),
        Calcit::Str(Arc::from(
          "get-in does not traverse Struct fields; use (:field value) so the checker can enforce the declared type",
        )),
      ]),
      matched,
      file_ns,
    )
  } else {
    generated_core_call("%some", vec![current.to_owned()], file_ns)
  };

  Ok(generated_if(
    generated_call(vec![Calcit::Proc(CalcitProc::NilQuestion), current]),
    none,
    present,
    file_ns,
  ))
}

/// Expand one Map-only `assoc-in` reconstruction step.
fn generated_assoc_in_step(
  current: Calcit,
  value: &Calcit,
  path: &[Calcit],
  file_ns: &str,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  let Some((key, rest)) = path.split_first() else {
    return Ok(value.to_owned());
  };

  let data = generated_path_symbol("typed_path_data", file_ns, call_stack)?;
  let child = generated_path_symbol("typed_path_child", file_ns, call_stack)?;
  let empty_map = || generated_call(vec![Calcit::Proc(CalcitProc::NativeMap)]);
  let child_value = generated_if(
    generated_call(vec![Calcit::Proc(CalcitProc::NativeMapContains), data.to_owned(), key.to_owned()]),
    generated_call(vec![Calcit::Proc(CalcitProc::NativeMapGet), data.to_owned(), key.to_owned()]),
    empty_map(),
    file_ns,
  );
  let updated_child = generated_let(
    child.to_owned(),
    child_value,
    generated_assoc_in_step(child, value, rest, file_ns, call_stack)?,
    file_ns,
  );
  let assoc = generated_call(vec![
    Calcit::Proc(CalcitProc::NativeMapAssoc),
    data.to_owned(),
    key.to_owned(),
    updated_child,
  ]);
  let normalized = generated_let(
    data,
    generated_if(
      generated_call(vec![Calcit::Proc(CalcitProc::NilQuestion), current.to_owned()]),
      empty_map(),
      current.to_owned(),
      file_ns,
    ),
    assoc,
    file_ns,
  );

  Ok(generated_if(
    generated_call(vec![Calcit::Proc(CalcitProc::StructQuestion), current]),
    generated_call(vec![
      Calcit::Proc(CalcitProc::Raise),
      Calcit::Str(Arc::from(
        "assoc-in does not traverse Struct fields; use assoc with a direct field key",
      )),
    ]),
    normalized,
    file_ns,
  ))
}

/// Expand eligible core path calls and leave every uncertain case untouched.
fn try_expand_typed_literal_path_call(
  head: &Calcit,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  call_stack: &CallStackList,
) -> Result<Option<Calcit>, CalcitErr> {
  let Calcit::Import(CalcitImport { ns, def, .. }) = head else {
    return Ok(None);
  };
  if ns.as_ref() != calcit::CORE_NS {
    return Ok(None);
  }

  let Some(base) = args.first() else { return Ok(None) };
  let Some(path_arg) = args.get(1) else { return Ok(None) };
  let Some(base_type) = resolve_type_value(base, scope_types) else {
    return Ok(None);
  };

  match def.as_ref() {
    "get-in" if args.len() == 2 => {
      let Some(path) = fully_typed_literal_lookup_path(base_type.as_ref(), path_arg) else {
        return Ok(None);
      };
      let base_binding = generated_path_symbol("typed_path_base", file_ns, call_stack)?;
      let mut path_bindings = Vec::with_capacity(path.len());
      let mut path_locals = Vec::with_capacity(path.len());
      for key in path {
        let binding = generated_path_symbol("typed_path_key", file_ns, call_stack)?;
        path_locals.push(binding.to_owned());
        path_bindings.push((binding, key));
      }
      let body = generated_get_in_step(base_binding.to_owned(), &path_locals, file_ns, call_stack)?;
      let body = generated_lets(path_bindings, body, file_ns);
      Ok(Some(generated_let(base_binding, base.to_owned(), body, file_ns)))
    }
    "assoc-in" if args.len() == 3 => {
      let Some(path) = fully_typed_literal_assoc_path(base_type.as_ref(), path_arg) else {
        return Ok(None);
      };
      let base_binding = generated_path_symbol("typed_path_base", file_ns, call_stack)?;
      let value_binding = generated_path_symbol("typed_path_replacement", file_ns, call_stack)?;
      let mut path_bindings = Vec::with_capacity(path.len());
      let mut path_locals = Vec::with_capacity(path.len());
      for key in path {
        let binding = generated_path_symbol("typed_path_key", file_ns, call_stack)?;
        path_locals.push(binding.to_owned());
        path_bindings.push((binding, key));
      }
      let body = generated_assoc_in_step(base_binding.to_owned(), &value_binding, &path_locals, file_ns, call_stack)?;
      let body = generated_let(value_binding, args[2].to_owned(), body, file_ns);
      let body = generated_lets(path_bindings, body, file_ns);
      Ok(Some(generated_let(base_binding, base.to_owned(), body, file_ns)))
    }
    _ => Ok(None),
  }
}

/// Reprocess generated code without surfacing diagnostics for synthetic guards.
/// Caller expressions have already been preprocessed with the user's warning
/// sink before specialization, so discarding only this second pass avoids
/// compiler-generated `nil?`/`struct?` warnings without hiding source warnings.
fn preprocess_generated_path_expansion(
  expanded: &Calcit,
  scope_defs: &HashSet<Arc<str>>,
  scope_types: &mut ScopeTypes,
  file_ns: &str,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  let generated_warnings = RefCell::new(vec![]);
  preprocess_expr(expanded, scope_defs, scope_types, file_ns, &generated_warnings, call_stack)
}

pub fn preprocess_expr(
  expr: &Calcit,
  scope_defs: &HashSet<Arc<str>>,
  scope_types: &mut ScopeTypes,
  file_ns: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  // println!("preprocessing @{} {}", file_ns, expr);
  match expr {
    Calcit::Symbol {
      sym: def, info, location, ..
    } => {
      match runner::parse_ns_def(def) {
        Some((ns_alias, def_part)) => {
          if &*ns_alias == "js" {
            require_js_ffi_feature(
              &format!("raw JavaScript global `js/{def_part}`"),
              location
                .as_ref()
                .map(|coord| NodeLocation::new(info.at_ns.to_owned(), info.at_def.to_owned(), coord.to_owned())),
              &info.at_ns,
              &info.at_def,
              check_warnings,
              call_stack,
            )?;
            Ok(Calcit::RawCode(RawCodeType::Js, def_part))
          } else if is_registered_proc(def) {
            // registered proc with namespace-qualified name (e.g. "calcit.cli/list-ns")
            // must precede has_def_code: calcit.cli/* also exist in core as metadata stubs
            Ok(Calcit::Registered(def.to_owned()))
          } else if let Some(target_ns) = program::lookup_ns_target_in_import(&info.at_ns, &ns_alias) {
            // make sure the target is preprocessed
            ensure_ns_def_compiled(&target_ns, &def_part, check_warnings, call_stack)?;

            let form = Calcit::Import(CalcitImport {
              ns: target_ns.to_owned(),
              def: def_part.to_owned(),
              info: Arc::new(ImportInfo::NsAs {
                alias: ns_alias.to_owned(),
                at_def: info.at_def.to_owned(),
                at_ns: ns_alias,
              }),
              def_id: Some(program::ensure_def_id(&target_ns, &def_part).0),
            });
            Ok(form)
          } else if program::has_def_code(&ns_alias, &def_part) {
            // refer to namespace/def directly for some usages

            // make sure the target is preprocessed
            ensure_ns_def_compiled(&ns_alias, &def_part, check_warnings, call_stack)?;

            let form = Calcit::Import(CalcitImport {
              ns: ns_alias.to_owned(),
              def: def_part.to_owned(),
              info: Arc::new(ImportInfo::NsReferDef {
                at_ns: info.at_ns.to_owned(),
                at_def: info.at_def.to_owned(),
              }),
              def_id: Some(program::ensure_def_id(&ns_alias, &def_part).0),
            });

            Ok(form)
          } else {
            Err(CalcitErr::use_msg_stack_location(
              CalcitErrKind::Var,
              format!("unknown ns target: {def}"),
              call_stack,
              expr.get_location(),
            ))
          }
        }
        None => {
          let def_ns = &info.at_ns;
          let at_def = &info.at_def;
          // println!("def {} - {} {} {}", def, def_ns, file_ns, at_def);
          // `todo!` is a compiler-known completion marker, not a normal
          // callable binding. Keep it recognizable even when a local happens
          // to use the same name, so scaffold completion checks stay sound.
          if def.as_ref() == "todo!" {
            Ok(Calcit::Proc(CalcitProc::Todo))
          } else if scope_defs.contains(def) {
            let type_info = scope_types.get(def).cloned().unwrap_or_else(|| calcit::DYNAMIC_TYPE.clone());
            Ok(Calcit::Local(CalcitLocal {
              idx: CalcitLocal::track_sym(def),
              sym: def.to_owned(),
              info: Arc::new(CalcitSymbolInfo {
                at_ns: def_ns.to_owned(),
                at_def: at_def.to_owned(),
              }),
              location: location.to_owned(),
              type_info,
            }))
          } else if CalcitSyntax::is_valid(def) {
            Ok(Calcit::Syntax(
              def.parse().map_err(|e: ParseError| {
                CalcitErr::use_msg_stack_location(
                  CalcitErrKind::Syntax,
                  def.to_string() + " " + &e.to_string(),
                  call_stack,
                  expr.get_location(),
                )
              })?,
              def_ns.to_owned(),
            ))
          } else if *def == info.at_def {
            // call function from same file
            // println!("same file: {}/{} at {}/{}", def_ns, def, file_ns, at_def);

            // make sure the target is preprocessed
            ensure_ns_def_compiled(def_ns, def, check_warnings, call_stack)?;

            let form = Calcit::Import(CalcitImport {
              ns: def_ns.to_owned(),
              def: def.to_owned(),
              info: Arc::new(ImportInfo::SameFile {
                at_def: info.at_def.to_owned(),
              }),
              def_id: Some(program::ensure_def_id(def_ns, def).0),
            });
            Ok(form)
          } else if let Ok(p) = def.parse::<CalcitProc>() {
            Ok(Calcit::Proc(p))
          } else if program::has_def_code(calcit::CORE_NS, def) {
            // println!("find in core def: {}", def);

            // make sure the target is preprocessed
            ensure_ns_def_compiled(calcit::CORE_NS, def, check_warnings, call_stack)?;

            let form = Calcit::Import(CalcitImport {
              ns: calcit::CORE_NS.into(),
              def: def.to_owned(),
              info: Arc::new(ImportInfo::Core { at_ns: file_ns.into() }),
              def_id: Some(program::ensure_def_id(calcit::CORE_NS, def).0),
            });
            Ok(form)
          } else if program::has_def_code(def_ns, def) {
            // same file
            // println!("again same file: {}/{} at {}/{}", def_ns, def, file_ns, at_def);

            // make sure the target is preprocessed
            ensure_ns_def_compiled(def_ns, def, check_warnings, call_stack)?;

            let form = Calcit::Import(CalcitImport {
              ns: def_ns.to_owned(),
              def: def.to_owned(),
              info: Arc::new(if &**def_ns == file_ns {
                ImportInfo::SameFile {
                  at_def: info.at_def.to_owned(),
                }
              } else {
                ImportInfo::NsReferDef {
                  at_ns: file_ns.into(),
                  at_def: at_def.to_owned(),
                }
              }),
              def_id: Some(program::ensure_def_id(def_ns, def).0),
            });
            Ok(form)
          } else if is_registered_proc(def) {
            Ok(Calcit::Registered(def.to_owned()))
          } else {
            match program::lookup_def_target_in_import(def_ns, def) {
              // referred to another namespace/def
              Some(target_ns) => {
                // effect
                // TODO js syntax to handle in future

                // make sure the target is preprocessed
                ensure_ns_def_compiled(&target_ns, def, check_warnings, call_stack)?;

                let form = Calcit::Import(CalcitImport {
                  ns: target_ns.to_owned(),
                  def: def.to_owned(),
                  info: Arc::new(ImportInfo::NsReferDef {
                    at_ns: def_ns.to_owned(),
                    at_def: at_def.to_owned(),
                  }),
                  def_id: Some(program::ensure_def_id(&target_ns, def).0),
                });
                Ok(form)
              }
              None if codegen::codegen_mode() && is_js_syntax_procs(def) => {
                require_js_ffi_feature(
                  &format!("JavaScript syntax `{def}`"),
                  location
                    .as_ref()
                    .map(|coord| NodeLocation::new(def_ns.to_owned(), at_def.to_owned(), coord.to_owned())),
                  def_ns,
                  at_def,
                  check_warnings,
                  call_stack,
                )?;
                Ok(expr.to_owned())
              }
              None => {
                let from_default = program::lookup_default_target_in_import(def_ns, def);
                if let Some(target_ns) = from_default {
                  Ok(Calcit::Import(CalcitImport {
                    ns: target_ns.to_owned(),
                    def: Arc::from("default"),
                    info: Arc::new(ImportInfo::JsDefault {
                      alias: def.to_owned(),
                      at_ns: file_ns.into(),
                      at_def: at_def.to_owned(),
                    }),
                    def_id: None,
                  }))
                } else {
                  let mut names: Vec<Arc<str>> = Vec::with_capacity(scope_defs.len());
                  for def in scope_defs {
                    names.push(def.to_owned());
                  }
                  let node_location = NodeLocation::new(def_ns.to_owned(), at_def.to_owned(), location.to_owned().unwrap_or_default());
                  if let Some(replacement) = removed_data_api_replacement(def) {
                    gen_check_warning_with_location_code(
                      format!("[Warn] `{def}` was removed by the struct/enum data-model migration; use `{replacement}`"),
                      "W_REMOVED_DATA_API",
                      node_location,
                      check_warnings,
                    );
                  } else {
                    gen_check_warning_with_location(
                      format!("[Warn] unknown `{def}` in {def_ns}/{at_def}, locals {{{}}}", names.join(" ")),
                      node_location,
                      check_warnings,
                    );
                  }
                  Ok(expr.to_owned())
                }
              }
            }
          }
        }
      }
    }
    Calcit::List(xs) => {
      if xs.is_empty() {
        Ok(expr.to_owned())
      } else {
        // TODO whether function bothers this...
        // println!("start calling: {}", expr);
        preprocess_list_call(xs, scope_defs, scope_types, file_ns, check_warnings, call_stack).map(into_executable_call)
      }
    }
    Calcit::Number(..)
    | Calcit::Str(..)
    | Calcit::Nil
    | Calcit::Unit
    | Calcit::Bool(..)
    | Calcit::Tag(..)
    | Calcit::CirruQuote(..)
    | Calcit::StructDef(..)
    | Calcit::EnumDef(..)
    | Calcit::Struct(..)
    | Calcit::Local(..) => Ok(expr.to_owned()),
    Calcit::Method(..) => Ok(expr.to_owned()),
    Calcit::Proc(..) => Ok(expr.to_owned()),
    Calcit::Syntax(..) => Ok(expr.to_owned()),
    Calcit::Import { .. } => Ok(expr.to_owned()),
    _ => {
      eprintln!("unknown expr: {expr}");
      gen_check_warning(
        format!("[Warn] unexpected data during preprocess: {expr:?}"),
        file_ns,
        check_warnings,
      );
      Ok(expr.to_owned())
    }
  }
}

fn preprocess_list_call(
  xs: &CalcitList,
  scope_defs: &HashSet<Arc<str>>,
  scope_types: &mut ScopeTypes,
  file_ns: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  let head = &xs[0];
  let call_location = derive_call_expr_location(head);
  let head_form = preprocess_expr(head, scope_defs, scope_types, file_ns, check_warnings, call_stack)?;
  let args = xs.drop_left();
  let mut def_name = grab_def_name(head);
  if def_name.as_ref() == "??"
    && let Some(receiver) = args.first()
  {
    def_name = grab_def_name(receiver);
  }
  // `struct-match` expands all nominal branches before runtime guards select
  // one. Branch locals can temporarily carry the scrutinee's type while the
  // generated code is preprocessed, so validating those generated probes here
  // would report fields from the non-selected Struct variants as source errors.
  let inside_struct_match_expansion = call_stack
    .0
    .iter()
    .any(|frame| matches!(frame.kind, StackKind::Macro) && frame.def.as_ref() == "struct-match");
  let call_info = CallTypeCheckInfo {
    file_ns,
    def_name: &def_name,
    call_location: call_location.clone(),
  };
  warn_on_removed_data_api_call(&head_form, call_location.clone(), file_ns, check_warnings);

  let has_anonymous_definition_marker = matches!(args.first(), Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "_");
  let is_constructor_named = |name: &str| {
    matches!(&head_form, Calcit::Import(CalcitImport { ns, def, .. }) if ns.as_ref() == calcit::CORE_NS && def.as_ref() == name)
      || matches!(&head_form, Calcit::Symbol { sym, .. } if sym.as_ref() == name)
  };

  // `%{} _ (:field value) ...` is the canonical anonymous-struct
  // constructor. Lower it before macro arguments are preprocessed so `_`
  // cannot be mistaken for a normal unresolved symbol.
  if has_anonymous_definition_marker && is_constructor_named("%{}") {
    let mut items = vec![Calcit::Proc(CalcitProc::NativeLooseStruct)];
    for entry in args.iter().skip(1) {
      let Calcit::List(pair) = entry else {
        return CalcitErr::err_str(
          CalcitErrKind::Type,
          format!("%{{}} _ expects (:field value) entries, but received: {entry}"),
        );
      };
      if pair.len() != 2 {
        return CalcitErr::err_str(
          CalcitErrKind::Arity,
          format!("%{{}} _ expects (:field value) entries, but received: {entry}"),
        );
      }
      items.extend(pair.iter().cloned());
    }
    return preprocess_expr(
      &Calcit::from(CalcitList::from(items.as_slice())),
      scope_defs,
      scope_types,
      file_ns,
      check_warnings,
      call_stack,
    );
  }

  // `%:: _ :variant ...` is the canonical anonymous-enum constructor.
  if has_anonymous_definition_marker
    && (matches!(&head_form, Calcit::Proc(CalcitProc::NativeNamedEnumNew)) || is_constructor_named("%::"))
  {
    let mut items = vec![Calcit::Proc(CalcitProc::NativeEnum)];
    items.extend(args.iter().skip(1).cloned());
    return preprocess_expr(
      &Calcit::from(CalcitList::from(items.as_slice())),
      scope_defs,
      scope_types,
      file_ns,
      check_warnings,
      call_stack,
    );
  }

  // A typed `.:field receiver` is the prefix form of external-object field
  // access. Preserve the source spelling while lowering it to a direct JS
  // property operation later in codegen.
  if let Calcit::Method(field_name, calcit::MethodKind::TagAccess) = &head_form
    && args.len() == 1
    && let Some(receiver_type) = resolve_type_value(&args[0], scope_types)
    && let Some(traits) = trait_list_from_type(receiver_type.as_ref())
    && traits.iter().any(|trait_def| trait_is_external_object(trait_def.as_ref()))
    && find_trait_field_type(&traits, field_name.as_ref()).is_some()
  {
    let typed_access = Calcit::Method(field_name.clone(), calcit::MethodKind::ExternalAccess(receiver_type));
    require_js_ffi_feature_for_operation(&typed_access, file_ns, def_name.as_ref(), check_warnings, call_stack)?;
    let processed_receiver = preprocess_expr(&args[0], scope_defs, scope_types, file_ns, check_warnings, call_stack)?;
    return Ok(Calcit::from(CalcitList::from(&[typed_access, processed_receiver])));
  }

  // === Postfix struct field access / method call detection ===
  // Pattern: (expr :field) where expr has a known struct type → rewrite to (.-field expr)
  // Pattern: (expr .method args...) where expr has a known struct/enum/trait type → rewrite to (.method expr args...)
  // When type is unknown (Dynamic), silently fall through — :tag / .method may be normal arguments.
  if let Some(rewritten) =
    try_rewrite_struct_enum_constructor_head_call(&head_form, &args, scope_types, file_ns, &def_name, check_warnings, call_stack)?
  {
    return preprocess_expr(&rewritten, scope_defs, scope_types, file_ns, check_warnings, call_stack);
  }

  if !args.is_empty() {
    let first_arg = &args[0];
    match first_arg {
      Calcit::Tag(field_tag) if args.len() == 1 => {
        if let Some(type_info) = resolve_type_value(&head_form, scope_types) {
          if let Some(traits) = trait_list_from_type(type_info.as_ref())
            && traits.iter().any(|trait_def| trait_is_external_object(trait_def.as_ref()))
            && find_trait_field_type(&traits, field_tag.ref_str()).is_some()
          {
            let typed_access = Calcit::Method(
              Arc::from(field_tag.ref_str()),
              calcit::MethodKind::ExternalAccess(type_info.clone()),
            );
            require_js_ffi_feature_for_operation(&typed_access, file_ns, def_name.as_ref(), check_warnings, call_stack)?;
            return Ok(Calcit::from(CalcitList::from(&[typed_access, head_form])));
          }
          if let Some(struct_def) = type_info.as_ref().resolve_to_struct()
            && let Some(idx) = struct_def.index_of(field_tag.ref_str())
          {
            // Rewrite to (&struct:nth expr idx :field-tag) — same as the existing
            // `(:tag expr)` rewrite, works in both Rust runtime and JS codegen.
            // A nominal struct has a fixed field set, so this returns the field
            // directly rather than wrapping it in Option.
            let items: Vec<Calcit> = vec![
              Calcit::Proc(CalcitProc::NativeStructNth),
              head_form,
              Calcit::Number(idx as f64),
              Calcit::Tag(field_tag.to_owned()),
            ];
            return Ok(Calcit::from(CalcitList::from(items.as_slice())));
          }

          if type_info.as_ref().resolve_to_struct().is_some() {
            if def_name.as_ref() != GENERATED_DEF && !inside_struct_match_expansion {
              check_field_in_struct(&head_form, first_arg, scope_types, file_ns, check_warnings);
            }
            return Ok(Calcit::from(CalcitList::from(&[
              Calcit::Proc(CalcitProc::NativeStructGet),
              head_form,
              first_arg.to_owned(),
            ])));
          }
          if is_anonymous_struct_type(type_info.as_ref()) {
            warn_required_struct_field_type(
              field_tag.ref_str(),
              &head_form,
              Some(type_info.as_ref()),
              RequiredStructFieldWarningContext {
                file_ns,
                def_name: def_name.as_ref(),
                location: first_arg.get_location(),
                call_stack,
              },
              check_warnings,
            );
            return Ok(Calcit::from(CalcitList::from(&[
              Calcit::Proc(CalcitProc::NativeStructGet),
              head_form,
              first_arg.to_owned(),
            ])));
          }
        }
        // For non-struct types (Dynamic, Fn, etc.), silently fall through —
        // `:tag` is a normal argument like `(list :a)` or `(f :a)`.
      }
      Calcit::Method(method_name, method_kind) => {
        // Only handle Invoke kinds (regular Calcit method calls)
        if matches!(method_kind, calcit::MethodKind::Invoke(_))
          && let Some(type_info) = resolve_type_value(&head_form, scope_types)
        {
          // Nominal and trait receivers always use method syntax, including the
          // unknown-method error path. Other statically known values participate
          // only when their actual method table contains the requested method;
          // this keeps `(f .map)` available as an ordinary function argument while
          // allowing typed values such as `n .show` and `xs .map callback`.
          let is_nominal_or_trait = type_info.as_ref().resolve_to_struct().is_some()
            || type_info.as_ref().resolve_to_enum().is_some()
            // A quoted named type can remain as a TypeRef while the core value is
            // bootstrapping. It is still explicit static evidence, and normal
            // method dispatch/codegen does not require resolving its impl table.
            || matches!(type_info.as_ref(), CalcitTypeAnnotation::TypeRef(..))
            || trait_list_from_type(type_info.as_ref()).is_some();
          let has_known_method = static_method_descriptors(type_info.as_ref()).is_some_and(|methods| {
            let expected = format!(".{method_name}");
            methods.iter().any(|method| method.name == expected)
          });
          // `Show` and `Debug` are presentation contracts. Keep them on the
          // typed-method path whenever the receiver has a static Calcit type:
          // an omitted `Show` implementation must be a type error rather than
          // silently becoming a prefix argument. Dynamic and external JS
          // values retain the existing escape hatch.
          let is_display_contract = matches!(method_name.as_ref(), "show" | "debug")
            && !matches!(type_info.as_ref(), CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::JsObject);

          if is_nominal_or_trait || has_known_method || is_display_contract {
            // Rewrite to (.method expr remaining_args...) — already handled by codegen
            let is_external = trait_list_from_type(type_info.as_ref())
              .is_some_and(|traits| traits.iter().any(|trait_def| trait_is_external_object(trait_def.as_ref())));
            let method_kind = if is_external {
              calcit::MethodKind::ExternalInvoke(type_info.clone())
            } else {
              calcit::MethodKind::Invoke(type_info.clone())
            };
            let typed_method = Calcit::Method(method_name.clone(), method_kind);
            let expected_method_args = expected_method_argument_types(type_info.as_ref(), method_name.as_ref());
            let mut processed_args = CalcitList::new_inner_from(&[head_form]);
            for (arg_idx, arg) in args.iter().skip(1).enumerate() {
              let expected_fn = expected_method_args
                .as_ref()
                .and_then(|types| types.get(arg_idx))
                .and_then(|expected| expected.resolve_to_fn());
              let previous_fn = EXPECTED_FN_TYPE.with(|cell| {
                let mut slot = cell.borrow_mut();
                let previous = slot.take();
                *slot = expected_fn;
                previous
              });
              let processed = preprocess_expr(arg, scope_defs, scope_types, file_ns, check_warnings, call_stack);
              EXPECTED_FN_TYPE.with(|cell| *cell.borrow_mut() = previous_fn);
              processed_args = processed_args.push(processed?);
            }
            let processed_args = CalcitList::from(processed_args);
            if is_display_contract {
              validate_method_call(&typed_method, &processed_args, scope_types, call_stack)?;
            }
            check_struct_method_args(&typed_method, &processed_args, scope_types, file_ns, &def_name, check_warnings);
            require_js_ffi_feature_for_operation(&typed_method, file_ns, def_name.as_ref(), check_warnings, call_stack)?;

            let mut ys = CalcitList::new_inner_from(&[typed_method]);
            for arg in processed_args.iter() {
              ys = ys.push(arg.to_owned());
            }
            return Ok(Calcit::from(CalcitList::from(ys)));
          }
          // For non-struct/trait types (Dynamic, Fn, etc.), silently fall through —
          // `.method` is a normal argument.
        }
        if file_ns != calcit::CORE_NS
          && resolve_type_value(&head_form, scope_types).is_none_or(|type_info| is_dynamic_annotation(type_info.as_ref()))
        {
          let location = head_form.get_location().or_else(|| first_arg.get_location());
          if matches!(method_kind, calcit::MethodKind::Invoke(_))
            && let Some((receiver_requirement, helper_hint)) = dynamic_nominal_method_requirement(method_name.as_ref())
          {
            let message = format!(
              "[Warn] postfix nominal method `.{method_name}` requires a statically known {receiver_requirement} receiver in {file_ns}/{def_name}, but the receiver is Dynamic; narrow it with a schema/assertion before using method syntax, or call {helper_hint} at an intentional Dynamic boundary"
            );
            if let Some(loc) = location {
              gen_check_warning_with_location_code(message, "W_DYNAMIC_NOMINAL_METHOD_RECEIVER", loc, check_warnings);
            } else {
              gen_check_warning_code(message, "W_DYNAMIC_NOMINAL_METHOD_RECEIVER", file_ns, check_warnings);
            }
          } else if warn_dyn_method_enabled() {
            let message = format!(
              "[Warn] postfix method `.{method_name}` has a dynamic receiver in {file_ns}/{def_name}; use prefix syntax `(.{method_name} receiver ...)`, assert-traits, or unsafe-coerce at an FFI boundary"
            );
            if let Some(loc) = location {
              gen_check_warning_with_location_code(message, "P_DYNAMIC_POSTFIX_METHOD", loc, check_warnings);
            } else {
              gen_check_warning_code(message, "P_DYNAMIC_POSTFIX_METHOD", file_ns, check_warnings);
            }
          }
        }
      }
      _ => {}
    }
  }

  let head_value = match &head_form {
    Calcit::Import(CalcitImport { ns, def, .. }) => lookup_callable_ns_def_for_preprocess(ns, def, check_warnings, call_stack)?,
    _ => None,
  };

  // == Tips ==
  // Macro from value: will be called during processing
  // Func from value: for checking arity
  // Tag: transforming into tag expression
  // Syntax: handled directly during preprocessing
  // Thunk: invalid here

  match head_value {
    Some(Calcit::Macro { id: macro_id, info }) => {
      let macro_name = format!("{}/{}", info.def_ns, info.name);
      runner::macro_metrics::record_expansion(&macro_name, info.signature.as_ref());
      let mut current_values: Vec<Calcit> = args.to_vec();
      let mut macro_type_bindings = validate_macro_call_inputs(
        info.name.as_ref(),
        info.signature.as_ref(),
        &args,
        scope_types,
        call_stack,
        call_location.clone(),
      )?;

      warn_on_trait_impl_method_tag_syntax(info.as_ref(), &args, file_ns, def_name.as_ref(), check_warnings);

      // println!("eval macro: {}", primes::CrListWrap(xs.to_owned()));
      // println!("macro... {} {}", x, CrListWrap(current_values.to_owned()));

      let code = Calcit::List(Arc::new(xs.to_owned()));
      let next_stack = call_stack.extend_owned(&info.def_ns, &info.name, StackKind::Macro, code, args.to_vec());

      let native_lowered = if info.def_ns.as_ref() == calcit::CORE_NS {
        match info.name.as_ref() {
          "let" => try_lower_core_let_macro(&args, file_ns),
          "{}" => try_lower_core_map_macro(&args),
          _ => None,
        }
      } else {
        None
      };
      if let Some(lowered) = native_lowered {
        runner::macro_metrics::record_native_fast_path(&macro_name);
        let _post_preprocess_timer =
          runner::macro_metrics::PhaseTimer::start(&macro_name, runner::macro_metrics::MacroMetricPhase::PostPreprocess);
        return preprocess_expr(&lowered, scope_defs, scope_types, file_ns, check_warnings, &next_stack);
      }

      let mut body_scope = CalcitScope::default();
      let frame_checkpoint = body_scope.frame_checkpoint();

      let mut cache_lookup = Some(runner::macro_cache::lookup(
        &macro_name,
        &macro_id,
        info.signature.as_ref(),
        &current_values,
        call_location.as_ref(),
        file_ns,
      ));
      match cache_lookup.as_ref().expect("macro cache lookup") {
        runner::macro_cache::CacheLookup::Hit(_) => runner::macro_metrics::record_cache_hit(&macro_name),
        runner::macro_cache::CacheLookup::Miss { reason, .. } => {
          runner::macro_metrics::record_cache_miss(&macro_name, reason, *reason != "cold-call-site")
        }
        runner::macro_cache::CacheLookup::Bypass(reason) => runner::macro_metrics::record_cache_bypass(&macro_name, reason),
      }

      let execute_macro = || -> Result<Calcit, CalcitErr> {
        let mut cache_miss = None;
        let mut evaluator_gensym_end = None;
        loop {
          // need to handle recursion
          body_scope.restore_frame(frame_checkpoint);
          runner::bind_marked_args(&mut body_scope, &info.args, &current_values, &next_stack)?;
          let code = match cache_lookup.take() {
            Some(runner::macro_cache::CacheLookup::Hit(code)) => code,
            lookup => {
              if let Some(runner::macro_cache::CacheLookup::Miss { token, .. }) = lookup {
                cache_miss = Some(token);
              }
              let evaluator_timer =
                runner::macro_metrics::PhaseTimer::start(&macro_name, runner::macro_metrics::MacroMetricPhase::Evaluator);
              let evaluate_body = || runner::evaluate_lines(&info.body.to_vec(), &body_scope, file_ns, &next_stack);
              let evaluated = runner::macro_capability::with_macro_context(
                Arc::from(macro_name.as_str()),
                info.signature.capabilities.clone(),
                call_location.clone(),
                evaluate_body,
              )?;
              evaluator_gensym_end = Some(builtins::meta::current_gensym_index(file_ns));
              drop(evaluator_timer);
              evaluated
            }
          };
          match code {
            Calcit::Recur(ys) => {
              current_values = ys;
              let recur_args = CalcitList::from(current_values.as_slice());
              macro_type_bindings = validate_macro_call_inputs(
                info.name.as_ref(),
                info.signature.as_ref(),
                &recur_args,
                scope_types,
                &next_stack,
                call_location.clone(),
              )?;
            }
            _ => {
              let _post_preprocess_timer =
                runner::macro_metrics::PhaseTimer::start(&macro_name, runner::macro_metrics::MacroMetricPhase::PostPreprocess);
              let processed = preprocess_expr(&code, scope_defs, scope_types, file_ns, check_warnings, &next_stack)?;
              validate_macro_expansion_result(
                info.name.as_ref(),
                info.signature.as_ref(),
                (&code, &processed),
                scope_types,
                macro_type_bindings,
                &next_stack,
                call_location.clone(),
              )?;
              if let Some(token) = cache_miss.take() {
                runner::macro_cache::store(
                  token,
                  &code,
                  evaluator_gensym_end.expect("cache miss evaluates the macro before storing its expansion"),
                );
              }
              return Ok(processed);
            }
          }
        }
      };
      execute_macro()
    }

    Some(Calcit::Fn { info, .. }) => {
      match &*info.args {
        CalcitFnArgs::MarkedArgs(xs) => {
          check_fn_marked_args(xs, &info.arg_types, &args, file_ns, &info.name, &def_name, check_warnings);
        }
        CalcitFnArgs::Args(xs) => {
          check_fn_args(xs, &info.arg_types, &args, file_ns, &info.name, &def_name, check_warnings);
        }
      }
      let mut ys = CalcitList::new_inner_from(std::slice::from_ref(&head_form));
      let mut has_spread = false;

      // Process arguments with type-aware preprocessing for Fn-typed params.
      // When the expected param type is Fn(...), set EXPECTED_FN_TYPE so that
      // preprocess_defn can inject arg types into anonymous fn params' scope_types.
      for (arg_idx, a) in args.iter().enumerate() {
        if let Calcit::Syntax(CalcitSyntax::ArgSpread, _) = a {
          has_spread = true;
          ys = ys.push(a.to_owned());
          continue;
        }

        // Set expected fn type hint if this arg position has a Fn-typed param
        let expected_fn = if arg_idx < info.arg_types.len() {
          if let CalcitTypeAnnotation::Fn(fn_annot) = info.arg_types[arg_idx].as_ref() {
            Some(fn_annot.clone())
          } else {
            None
          }
        } else {
          None
        };

        // Set expected struct type hint if this arg position has a struct-typed param
        // This enables field-type-aware preprocessing of hashmap literals (e.g., DomProps)
        let expected_struct = if arg_idx < info.arg_types.len() {
          info.arg_types[arg_idx].resolve_to_struct_with_ref().map(|(s, _)| s)
        } else {
          None
        };

        if let Some(fn_annot) = expected_fn {
          EXPECTED_FN_TYPE.with(|cell| cell.borrow_mut().replace(fn_annot));
        }
        if let Some(struct_def) = expected_struct {
          EXPECTED_STRUCT_TYPE.with(|cell| cell.borrow_mut().replace(struct_def));
        }

        let result = preprocess_expr(a, scope_defs, scope_types, file_ns, check_warnings, call_stack);

        // Always clear the hints after preprocessing, even on error
        EXPECTED_FN_TYPE.with(|cell| *cell.borrow_mut() = None);
        EXPECTED_STRUCT_TYPE.with(|cell| *cell.borrow_mut() = None);

        let form = result?;

        ys = ys.push(form);
      }
      if !has_spread {
        let mut current_args = CalcitList::from(ys.drop_left());
        // Core helpers such as `get` resolve to ordinary functions, so validate
        // their statically known struct fields in this branch as well.
        check_struct_field_access(&head_form, &current_args, scope_types, file_ns, call_stack, check_warnings);
        warn_on_nominal_enum_legacy_absence_use(&head_form, &current_args, scope_types, file_ns, def_name.as_ref(), check_warnings);
        warn_on_legacy_js_nullish_predicate(&head_form, &current_args, scope_types, file_ns, def_name.as_ref(), check_warnings);
        let mut any_rewritten = false;
        // Rewrite hashmap literal args to struct literals when the expected type is a struct
        if let Some(rewritten) = try_rewrite_map_args_to_structs(info.as_ref(), &current_args, file_ns, &def_name, check_warnings) {
          current_args = rewritten;
          any_rewritten = true;
        }
        // Rewrite loose struct literal args (`?{}`) to struct literals when the expected type is a struct
        if let Some(rewritten) =
          try_rewrite_loose_struct_args_to_structs(info.as_ref(), &current_args, file_ns, &def_name, check_warnings)
        {
          current_args = rewritten;
          any_rewritten = true;
        }
        // Rewrite untyped enum literal args to named enums when the expected type is an enum
        if let Some(rewritten) = try_rewrite_enum_args_to_named_enums(info.as_ref(), &current_args, file_ns, &def_name, check_warnings)
        {
          current_args = rewritten;
          any_rewritten = true;
        }
        // Rebuild ys only once after all rewrites
        if any_rewritten {
          let mut new_ys = CalcitList::new_inner_from(std::slice::from_ref(&head_form));
          for item in current_args.iter() {
            new_ys = new_ys.push(item.to_owned());
          }
          ys = new_ys;
        }
        check_core_fn_arg_types(
          info.as_ref(),
          &current_args,
          scope_types,
          file_ns,
          &def_name,
          call_location.clone(),
          check_warnings,
        );
        check_user_fn_arg_types(info.as_ref(), &head_form, &current_args, scope_types, &call_info, check_warnings);
      }
      if has_spread {
        ys = ys.prepend(Calcit::Syntax(CalcitSyntax::CallSpread, info.def_ns.to_owned()));
        Ok(Calcit::from(CalcitList::from(ys)))
      } else {
        // Try to specialize polymorphic calls when receiver type is known
        if let Calcit::Import(CalcitImport { ns, def, .. }) = &head_form {
          let current_args = CalcitList::from(ys.drop_left());
          if matches!(def.as_ref(), "get-in" | "assoc-in")
            && let Some(expanded) = try_expand_typed_literal_path_call(&head_form, &current_args, scope_types, file_ns, call_stack)?
          {
            return preprocess_generated_path_expansion(&expanded, scope_defs, scope_types, file_ns, call_stack);
          }
          if let Some(specialized) = try_specialize_polymorphic_call(ns, def, &current_args, scope_types, file_ns) {
            return Ok(specialized);
          }
        }
        Ok(Calcit::from(CalcitList::from(ys)))
      }
    }

    _ => match &head_form {
      Calcit::Tag(tag) => {
        if args.len() == 1 {
          // Preprocess the argument first to get type info
          let processed_arg = preprocess_expr(&args[0], scope_defs, scope_types, file_ns, check_warnings, call_stack)?;
          // Try to resolve the arg type as a struct value for indexed access optimization
          if let Some(type_info) = resolve_type_value(&processed_arg, scope_types)
            && let Some(struct_def) = type_info.as_ref().resolve_to_struct()
            && let Some(idx) = struct_def.index_of(tag.ref_str())
          {
            // Emit (&struct:nth processed_arg idx :field-tag)
            // The field tag lets every backend reject stale index metadata after schema drift.
            let items: Vec<Calcit> = vec![
              Calcit::Proc(CalcitProc::NativeStructNth),
              processed_arg,
              Calcit::Number(idx as f64),
              Calcit::Tag(tag.to_owned()),
            ];
            let nth_call = Calcit::from(CalcitList::from(items.as_slice()));
            return Ok(nth_call);
          }
          if let Some(type_info) = resolve_type_value(&processed_arg, scope_types) {
            if type_info.as_ref().resolve_to_struct().is_some() {
              if def_name.as_ref() != GENERATED_DEF && !inside_struct_match_expansion {
                check_field_in_struct(&processed_arg, head, scope_types, file_ns, check_warnings);
              }
              return Ok(Calcit::from(CalcitList::from(&[
                Calcit::Proc(CalcitProc::NativeStructGet),
                processed_arg,
                head.to_owned(),
              ])));
            }
            if is_anonymous_struct_type(type_info.as_ref()) {
              warn_required_struct_field_type(
                tag.ref_str(),
                &processed_arg,
                Some(type_info.as_ref()),
                RequiredStructFieldWarningContext {
                  file_ns,
                  def_name: def_name.as_ref(),
                  location: head.get_location(),
                  call_stack,
                },
                check_warnings,
              );
              return Ok(Calcit::from(CalcitList::from(&[
                Calcit::Proc(CalcitProc::NativeStructGet),
                processed_arg,
                head.to_owned(),
              ])));
            }
          }
          // A tag in function position is the required Struct-field accessor.
          // Do not silently turn it into an Option-producing collection lookup:
          // that makes the expression's contract depend on whether inference
          // happened to recover a Struct type. Keep the fallback only to let
          // preprocessing continue and collect more diagnostics; check-only and
          // normal execution reject the warning below.
          let receiver_type = resolve_type_value(&processed_arg, scope_types);
          warn_required_struct_field_type(
            tag.ref_str(),
            &processed_arg,
            receiver_type.as_deref(),
            RequiredStructFieldWarningContext {
              file_ns,
              def_name: def_name.as_ref(),
              location: head.get_location(),
              call_stack,
            },
            check_warnings,
          );

          // Preserve the old lowering after recording the hard diagnostic so
          // later expressions can still be checked in the same pass.
          let get_method = Calcit::Import(CalcitImport {
            ns: calcit::CORE_NS.into(),
            def: "get".into(),
            info: Arc::new(ImportInfo::Core { at_ns: Arc::from(file_ns) }),
            def_id: Some(program::ensure_def_id(calcit::CORE_NS, "get").0),
          });

          let code = Calcit::from(CalcitList::from(&[get_method, args[0].to_owned(), head.to_owned()]));
          preprocess_expr(&code, scope_defs, scope_types, file_ns, check_warnings, call_stack)
        } else {
          Err(CalcitErr::use_msg_stack_location(
            CalcitErrKind::Arity,
            format!("{head} expected 1 hashmap to call"),
            call_stack,
            head.get_location(),
          ))
        }
      }

      Calcit::Syntax(name, name_ns) => match name {
        CalcitSyntax::Quasiquote => {
          let mut ctx = PreprocessContext::new(scope_defs, scope_types, file_ns, check_warnings, call_stack);
          Ok(preprocess_quasiquote(name, name_ns, &args, &mut ctx)?)
        }
        CalcitSyntax::Defn | CalcitSyntax::Defmacro | CalcitSyntax::DefWasmExport | CalcitSyntax::DefWasmImport => {
          let mut ctx = PreprocessContext::new(scope_defs, scope_types, file_ns, check_warnings, call_stack);
          Ok(preprocess_defn(name, name_ns, &args, &mut ctx)?)
        }
        CalcitSyntax::CoreLet => {
          let mut ctx = PreprocessContext::new(scope_defs, scope_types, file_ns, check_warnings, call_stack);
          Ok(preprocess_core_let(name, name_ns, &args, &mut ctx)?)
        }
        CalcitSyntax::If => {
          let mut ctx = PreprocessContext::new(scope_defs, scope_types, file_ns, check_warnings, call_stack);
          Ok(preprocess_if(name, name_ns, &args, &mut ctx)?)
        }
        CalcitSyntax::Try
        | CalcitSyntax::Macroexpand
        | CalcitSyntax::MacroexpandAll
        | CalcitSyntax::Macroexpand1
        | CalcitSyntax::Gensym
        | CalcitSyntax::Reset => {
          let mut ctx = PreprocessContext::new(scope_defs, scope_types, file_ns, check_warnings, call_stack);
          Ok(preprocess_each_items(name, name_ns, &args, &mut ctx)?)
        }
        CalcitSyntax::Quote | CalcitSyntax::Eval => Ok(preprocess_quote(name, name_ns, &args, scope_defs, file_ns)?),
        CalcitSyntax::HintFn => {
          let mut ctx = PreprocessContext::new(scope_defs, scope_types, file_ns, check_warnings, call_stack);
          preprocess_hint_fn(name, name_ns, &args, &mut ctx)
        }
        CalcitSyntax::Defatom => {
          let mut ctx = PreprocessContext::new(scope_defs, scope_types, file_ns, check_warnings, call_stack);
          Ok(preprocess_defatom(name, name_ns, &args, &mut ctx)?)
        }
        CalcitSyntax::CallSpread => {
          let mut ys = vec![head_form];

          args.traverse_result::<CalcitErr>(&mut |a| {
            let form = preprocess_expr(a, scope_defs, scope_types, file_ns, check_warnings, call_stack)?;
            ys.push(form);
            Ok(())
          })?;
          Ok(Calcit::from(ys))
        }
        CalcitSyntax::AssertType => {
          let mut ctx = PreprocessContext::new(scope_defs, scope_types, file_ns, check_warnings, call_stack);
          preprocess_assert_type(name, name_ns, &args, &mut ctx)
        }
        CalcitSyntax::UnsafeCoerce => {
          let mut ctx = PreprocessContext::new(scope_defs, scope_types, file_ns, check_warnings, call_stack);
          preprocess_unsafe_coerce(name, name_ns, &args, &mut ctx)
        }
        CalcitSyntax::ParseCirruEdnAs | CalcitSyntax::TryParseCirruEdnAs => {
          let mut ctx = PreprocessContext::new(scope_defs, scope_types, file_ns, check_warnings, call_stack);
          preprocess_parse_cirru_edn_as(name, name_ns, &args, &mut ctx)
        }
        CalcitSyntax::DecodeMapAs | CalcitSyntax::TryDecodeMapAs => {
          let mut ctx = PreprocessContext::new(scope_defs, scope_types, file_ns, check_warnings, call_stack);
          preprocess_decode_map_as(name, name_ns, &args, &mut ctx)
        }
        CalcitSyntax::AssertTraits => {
          let mut ctx = PreprocessContext::new(scope_defs, scope_types, file_ns, check_warnings, call_stack);
          preprocess_assert_traits(name, name_ns, &args, &mut ctx)
        }
        CalcitSyntax::Match => {
          let mut ctx = PreprocessContext::new(scope_defs, scope_types, file_ns, check_warnings, call_stack);
          preprocess_match(name, name_ns, &args, &mut ctx)
        }
        CalcitSyntax::ArgSpread => CalcitErr::err_nodes(CalcitErrKind::Syntax, "`&` cannot be preprocessed as operator", &xs.to_vec()),
        CalcitSyntax::ArgOptional => {
          CalcitErr::err_nodes(CalcitErrKind::Syntax, "`?` cannot be preprocessed as operator", &xs.to_vec())
        }
        CalcitSyntax::MacroInterpolate => {
          CalcitErr::err_nodes(CalcitErrKind::Syntax, "`~` cannot be preprocessed as operator", &xs.to_vec())
        }
        CalcitSyntax::MacroInterpolateSpread => {
          CalcitErr::err_nodes(CalcitErrKind::Syntax, "`~@` cannot be preprocessed as operator", &xs.to_vec())
        }
      },
      Calcit::Thunk(..) => Err(CalcitErr::use_msg_stack_location(
        CalcitErrKind::Unexpected,
        format!("does not know how to preprocess a thunk: {head}"),
        call_stack,
        head.get_location(),
      )),

      // with-type-slot must be intercepted before the generic Proc arm so that its body
      // args are preprocessed while the scoped type override is active.
      Calcit::Proc(CalcitProc::WithTypeSlot) => {
        preprocess_with_type_slot_block(&head_form, &args, scope_defs, scope_types, file_ns, check_warnings, call_stack)
      }

      Calcit::Method(_, _)
      | Calcit::Proc(..)
      | Calcit::Local { .. }
      | Calcit::Import { .. }
      | Calcit::Registered { .. }
      | Calcit::List(..)
      | Calcit::RawCode(..)
      | Calcit::Symbol { .. }
      | Calcit::StructDef(..)
      | Calcit::EnumDef(..) => {
        // Postfix method syntax is represented as `(receiver .method ...)` at
        // this stage. The receiver is deliberately not callable; the method
        // rewrite below validates and resolves the actual call target.
        if !matches!(args.first(), Some(Calcit::Method(..))) {
          check_callable_type(&head_form, scope_types, file_ns, &def_name, check_warnings);
        }

        let mut ys = CalcitList::new_inner_from(std::slice::from_ref(&head_form));
        let mut has_spread = false;

        // When the head is NativeMap and we have an expected struct type (from the calling context),
        // use struct field types to inject EXPECTED_FN_TYPE for Fn-typed fields.
        let struct_hint = if matches!(&head_form, Calcit::Proc(CalcitProc::NativeMap)) {
          EXPECTED_STRUCT_TYPE.with(|cell| cell.borrow().clone())
        } else {
          None
        };

        if let Some(ref struct_def) = struct_hint {
          // Struct-aware hashmap preprocessing: iterate key-value pairs
          let items: Vec<&Calcit> = args.iter().collect();
          for (i, item) in items.iter().enumerate() {
            if let Calcit::Syntax(CalcitSyntax::ArgSpread, _) = item {
              has_spread = true;
              ys = ys.push((*item).to_owned());
              continue;
            }

            // For value positions (odd indices), look up the preceding key's field type.
            // Set EXPECTED_FN_TYPE for Fn-typed fields so that inline fn literals get param type injection.
            if i % 2 == 1
              && let Some(Calcit::Tag(key_tag)) = items.get(i - 1)
              && let Some(field_idx) = struct_def.fields.iter().position(|f| f == key_tag)
              && let Some(field_type) = struct_def.field_types.get(field_idx)
              && let Some(fn_annot) = field_type.resolve_to_fn()
            {
              EXPECTED_FN_TYPE.with(|cell| cell.borrow_mut().replace(fn_annot));
            }

            let result = preprocess_expr(item, scope_defs, scope_types, file_ns, check_warnings, call_stack);

            // Clear fn type hint after processing a value
            if i % 2 == 1 {
              EXPECTED_FN_TYPE.with(|cell| *cell.borrow_mut() = None);
            }

            let form = result?;

            ys = ys.push(form);
          }
        } else {
          args.traverse_result::<CalcitErr>(&mut |a| {
            if let Calcit::Syntax(CalcitSyntax::ArgSpread, _) = a {
              has_spread = true;
              ys = ys.push(a.to_owned());
              return Ok(());
            }
            let form = preprocess_expr(a, scope_defs, scope_types, file_ns, check_warnings, call_stack)?;
            ys = ys.push(form);
            Ok(())
          })?;
        }

        // Check for struct field access after processing arguments
        let processed_args = CalcitList::from(ys.drop_left()); // Skip the head, convert to CalcitList
        validate_method_call(&head_form, &processed_args, scope_types, call_stack)?;
        check_struct_field_access(&head_form, &processed_args, scope_types, file_ns, call_stack, check_warnings);
        check_struct_update_fields(&head_form, &processed_args, scope_types, file_ns, &def_name, check_warnings);
        check_struct_method_args(&head_form, &processed_args, scope_types, file_ns, &def_name, check_warnings);
        check_typed_js_field_operation(
          &head_form,
          &processed_args,
          scope_types,
          file_ns,
          &def_name,
          check_warnings,
          call_stack,
        )?;
        if let Some(rewritten) = rewrite_typed_js_field_operation(&head_form, &processed_args, scope_types) {
          let rewritten_head = match &rewritten {
            Calcit::List(items) => items.first().expect("typed JS field rewrite must have a head"),
            _ => &rewritten,
          };
          require_js_ffi_feature_for_operation(rewritten_head, file_ns, &def_name, check_warnings, call_stack)?;
          return Ok(rewritten);
        }

        // Optimize &struct:get to &struct:nth when field index can be resolved at compile time
        if matches!(&head_form, Calcit::Proc(CalcitProc::NativeStructGet))
          && processed_args.len() == 2
          && let (Some(struct_arg), Some(Calcit::Tag(field_tag))) = (processed_args.first(), processed_args.get(1))
          && let Some(type_info) = resolve_type_value(struct_arg, scope_types)
          && let Some(struct_def) = type_info.as_ref().resolve_to_struct()
          && let Some(idx) = struct_def.index_of(field_tag.ref_str())
        {
          ys = CalcitList::new_inner_from(&[
            Calcit::Proc(CalcitProc::NativeStructNth),
            struct_arg.to_owned(),
            Calcit::Number(idx as f64),
            Calcit::Tag(field_tag.to_owned()),
          ]);
        }

        // Optimize &struct:assoc to &struct:assoc-at when field index can be resolved at compile time
        if matches!(&head_form, Calcit::Proc(CalcitProc::NativeStructAssoc))
          && processed_args.len() == 3
          && let (Some(struct_arg), Some(Calcit::Tag(field_tag)), Some(value_arg)) =
            (processed_args.first(), processed_args.get(1), processed_args.get(2))
          && let Some(type_info) = resolve_type_value(struct_arg, scope_types)
          && let Some(struct_def) = type_info.as_ref().resolve_to_struct()
          && let Some(idx) = struct_def.index_of(field_tag.ref_str())
        {
          ys = CalcitList::new_inner_from(&[
            Calcit::Proc(CalcitProc::NativeStructAssocAt),
            struct_arg.to_owned(),
            Calcit::Number(idx as f64),
            Calcit::Tag(field_tag.to_owned()),
            value_arg.to_owned(),
          ]);
        }

        // Optimize &struct:with to &struct:with-at when all field indices can be resolved at compile time
        if matches!(&head_form, Calcit::Proc(CalcitProc::NativeStructWith))
          && processed_args.len() >= 3
          && (processed_args.len() - 1) % 2 == 0
          && let Some(struct_arg) = processed_args.first()
          && let Some(type_info) = resolve_type_value(struct_arg, scope_types)
          && let Some(struct_def) = type_info.as_ref().resolve_to_struct()
        {
          let pair_count = (processed_args.len() - 1) / 2;
          let mut all_resolved = true;
          let mut new_args: Vec<Calcit> = Vec::with_capacity(1 + pair_count * 3);
          new_args.push(struct_arg.to_owned());

          for i in 0..pair_count {
            let k_idx = 1 + i * 2;
            let v_idx = k_idx + 1;
            if let Some(Calcit::Tag(field_tag)) = processed_args.get(k_idx) {
              if let Some(idx) = struct_def.index_of(field_tag.ref_str()) {
                new_args.push(Calcit::Number(idx as f64));
                new_args.push(Calcit::Tag(field_tag.to_owned()));
                if let Some(val) = processed_args.get(v_idx) {
                  new_args.push(val.to_owned());
                } else {
                  all_resolved = false;
                  break;
                }
              } else {
                all_resolved = false;
                break;
              }
            } else {
              all_resolved = false;
              break;
            }
          }

          if all_resolved {
            let mut items: Vec<Calcit> = Vec::with_capacity(1 + new_args.len());
            items.push(Calcit::Proc(CalcitProc::NativeStructWithAt));
            items.extend(new_args);
            ys = CalcitList::new_inner_from(&items);
          }
        }

        // Infer type for Method(Invoke) and update the head if type info is available
        if let Calcit::Method(method_name, calcit::MethodKind::Invoke(_)) = &head_form
          && let Some(receiver) = processed_args.first()
          && let Some(type_value) = resolve_type_value(receiver, scope_types)
        {
          // Reconstruct the list with updated Method node carrying inferred type
          let is_external = trait_list_from_type(type_value.as_ref())
            .is_some_and(|traits| traits.iter().any(|trait_def| trait_is_external_object(trait_def.as_ref())));
          let method_kind = if is_external {
            calcit::MethodKind::ExternalInvoke(type_value)
          } else {
            calcit::MethodKind::Invoke(type_value)
          };
          let typed_method = Calcit::Method(method_name.clone(), method_kind);
          ys = CalcitList::new_inner_from(&[typed_method]);
          for item in processed_args.iter() {
            ys = ys.push(item.to_owned());
          }
        }

        if let Some(call_head) = ys.first() {
          // Recompute processed_args from ys after optimization rewrites (e.g. struct:get→nth, assoc→assoc-at)
          let processed_args = CalcitList::from(ys.drop_left());
          require_js_ffi_feature_for_operation(call_head, file_ns, def_name.as_ref(), check_warnings, call_stack)?;
          warn_on_nullable_js_ffi_dereference(call_head, &processed_args, scope_types, file_ns, def_name.as_ref(), check_warnings);
          warn_on_untyped_js_ffi_field_access(call_head, &processed_args, scope_types, file_ns, def_name.as_ref(), check_warnings);
          warn_on_nominal_enum_legacy_absence_use(call_head, &processed_args, scope_types, file_ns, def_name.as_ref(), check_warnings);
          warn_on_legacy_js_nullish_predicate(call_head, &processed_args, scope_types, file_ns, def_name.as_ref(), check_warnings);
          warn_on_dynamic_trait_call(call_head, &processed_args, scope_types, file_ns, def_name.as_ref(), check_warnings);
          warn_on_method_name_conflict(call_head, &processed_args, scope_types, file_ns, def_name.as_ref(), check_warnings);
        }

        // Check Proc argument types if available
        if let Some(Calcit::Proc(proc)) = ys.first() {
          let processed_args = CalcitList::from(ys.drop_left());
          if matches!(proc, CalcitProc::Todo) {
            if processed_args.len() > 1 {
              return Err(CalcitErr::use_msg_stack_location(
                CalcitErrKind::Arity,
                format!("todo! expects 0~1 arguments, got {}", processed_args.len()),
                call_stack,
                call_location.clone(),
              ));
            }
            if let Some(message) = processed_args.first()
              && !matches!(message, Calcit::Str(_))
            {
              let argument_location = message.get_location().or_else(|| call_location.clone());
              return Err(CalcitErr::use_msg_stack_location(
                CalcitErrKind::Type,
                "todo! expects an optional static String message",
                call_stack,
                argument_location,
              ));
            }
            let enclosing_def = call_location
              .as_ref()
              .map(|location| location.def.as_ref())
              .or_else(|| call_stack.0.first().map(|frame| frame.def.as_ref()))
              .unwrap_or(def_name.as_ref());
            let message = match processed_args.first() {
              None => ": implementation is pending".to_owned(),
              Some(Calcit::Str(message)) => format!(": {message}"),
              Some(_) => unreachable!("invalid todo! message was rejected above"),
            };
            gen_check_warning_code_at(
              format!("[Warn] TODO placeholder in {file_ns}/{enclosing_def}{message}"),
              "W_TODO",
              file_ns,
              call_location.clone(),
              check_warnings,
            );
          }
          check_proc_arg_types(
            proc,
            &processed_args,
            scope_types,
            file_ns,
            &def_name,
            call_location.clone(),
            check_warnings,
          );

          // Try predicate folding for type-predicate procs
          if let Some(specialized) =
            try_specialize_polymorphic_call(calcit::CORE_NS, proc.as_ref(), &processed_args, scope_types, file_ns)
          {
            return Ok(specialized);
          }
        }

        // Check Local function call argument types if the local has a known Fn type
        // Also rewrite untyped enum args to named enums when the local fn expects an enum type.
        if let Some(Calcit::Local(local)) = ys.first() {
          let local_sym = local.sym.clone();
          let local_type_info = local.type_info.clone();
          let local_type = if matches!(*local_type_info, CalcitTypeAnnotation::Dynamic) {
            scope_types.get(&local_sym).cloned()
          } else {
            Some(local_type_info)
          };

          if let Some(ref ty) = local_type
            && let CalcitTypeAnnotation::Fn(fn_annot) = ty.as_ref()
            && let Some(rewritten) =
              try_rewrite_local_fn_enum_args_to_named_enums(fn_annot, &local_sym, &processed_args, file_ns, &def_name, check_warnings)
          {
            ys = CalcitList::new_inner_from(&[ys.first().unwrap().to_owned()]);
            for item in rewritten.iter() {
              ys = ys.push(item.to_owned());
            }
          }
          if let Some(Calcit::Local(local)) = ys.first() {
            let updated_args = CalcitList::from(ys.drop_left());
            check_local_fn_call_arg_types(&head_form, local, &updated_args, scope_types, &call_info, check_warnings);
          }
        }

        // Eagerly execute type-slot procs during preprocessing so that TypeSlot
        // annotations can be resolved by the type checker within the same compilation.
        if let Some(Calcit::Proc(CalcitProc::DeftypeSlot)) = ys.first()
          && let Some(slot_name) = processed_args.first().and_then(|arg| match arg {
            Calcit::Tag(tag) => Some(Arc::from(tag.ref_str())),
            Calcit::Str(text) => Some(Arc::from(text.as_ref())),
            _ => None,
          })
        {
          register_type_slot(slot_name)
            .map_err(|msg| CalcitErr::use_msg_stack_location(CalcitErrKind::Unexpected, msg, call_stack, head.get_location()))?;
        }
        // Handle &inspect-type: print type information for the given symbol
        if let Some(Calcit::Proc(CalcitProc::NativeInspectType)) = ys.first() {
          if let Some(first_arg) = processed_args.first() {
            // Look up the type of the symbol in scope_types
            let sym_name = match first_arg {
              Calcit::Symbol { sym, .. } => Some(sym.as_ref()),
              Calcit::Local(local) => Some(local.sym.as_ref()),
              _ => None,
            };
            let type_info = if let Some(name) = sym_name {
              scope_types.get(name).cloned().unwrap_or_else(|| calcit::DYNAMIC_TYPE.clone())
            } else {
              infer_type_from_expr(first_arg, scope_types).unwrap_or_else(|| calcit::DYNAMIC_TYPE.clone())
            };

            let loc = head.get_location().or_else(|| first_arg.get_location());
            if let Some(l) = loc {
              eprintln!(
                "[&inspect-type] in {}/{} {}\n  {} => {}",
                l.ns,
                l.def,
                format_inspect_type_coord(&l.coord),
                first_arg,
                type_info.describe()
              );
            } else {
              eprintln!(
                "[&inspect-type] in {}/{}\n  {} => {}",
                file_ns,
                def_name,
                first_arg,
                type_info.describe()
              );
            }

            if let Some(Calcit::Tag(tag)) = processed_args.get(1)
              && tag.ref_str().trim_start_matches(':') == "fail-on-dynamic"
              && matches!(*type_info, CalcitTypeAnnotation::Dynamic)
            {
              let msg = format!("&inspect-type failed to infer type for {first_arg}");
              if let Some(loc) = head.get_location() {
                gen_check_warning_with_location(msg, loc, check_warnings);
              } else {
                gen_check_warning(msg, file_ns, check_warnings);
              }
            }
          }
          // Return nil for &inspect-type
          return Ok(Calcit::Nil);
        }

        if !has_spread
          && let Some(call_head @ Calcit::Import(CalcitImport { ns, def, .. })) = ys.first()
          && ns.as_ref() == calcit::CORE_NS
          && matches!(def.as_ref(), "get-in" | "assoc-in")
          && let Some(expanded) = try_expand_typed_literal_path_call(call_head, &processed_args, scope_types, file_ns, call_stack)?
        {
          return preprocess_generated_path_expansion(&expanded, scope_defs, scope_types, file_ns, call_stack);
        }

        if !has_spread
          && let Some(call_head) = ys.first()
          && let Some(optimized_call) = try_inline_method_call(call_head, &processed_args, scope_types, file_ns)
        {
          return Ok(optimized_call);
        }

        if has_spread {
          ys = ys.prepend(Calcit::Syntax(CalcitSyntax::CallSpread, file_ns.into()));
          Ok(Calcit::from(CalcitList::List(ys)))
        } else {
          let items = ys.to_vec();
          let kind = classify_number_binary_call(&items[0], &items[1..], scope_types);
          Ok(Calcit::from(CalcitList::executable(items, kind)))
        }
      }
      h => {
        let loc = h.get_location();
        Err(CalcitErr::use_msg_stack_location(
          CalcitErrKind::Unexpected,
          format!("unknown head `{h}` in {xs}"),
          call_stack,
          loc,
        ))
      }
    },
  }
}

fn try_rewrite_struct_enum_constructor_head_call(
  head_form: &Calcit,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  _call_stack: &CallStackList,
) -> Result<Option<Calcit>, CalcitErr> {
  // Constructor sugar is only valid when the head is the data definition itself.
  // A struct instance and its struct prototype intentionally share most type
  // information, so `resolve_type_value` alone cannot distinguish them.
  let constructor_kind = match head_form {
    Calcit::StructDef(_) => Some("defstruct"),
    Calcit::EnumDef(_) => Some("defenum"),
    Calcit::Import(CalcitImport { ns, def, .. }) => data_definition_kind(ns, def),
    Calcit::Symbol { sym, info, .. } => data_definition_kind(&info.at_ns, sym),
    _ => None,
  };

  let Some(type_info) = resolve_type_value(head_form, scope_types) else {
    return Ok(None);
  };

  let resolved_struct_definition = match type_info.as_ref() {
    CalcitTypeAnnotation::StructDef(struct_def) => Some((struct_def.as_ref().clone(), None)),
    other => other.resolve_to_struct_with_ref(),
  };
  if constructor_kind == Some("defstruct")
    && let Some((struct_def, ns_def_path)) = resolved_struct_definition
  {
    // Only attempt constructor-style rewriting for tag/value-style positional args.
    // Method-call syntax (expr .method ...) is represented as `Method` in the first arg
    // and must be handled by the method-branch below.
    if matches!(args.first(), Some(Calcit::Method(..))) {
      return Ok(None);
    }

    if !args.len().is_multiple_of(2) {
      gen_check_warning(
        format!(
          "[Warn] struct constructor rewrite skipped: `{}` has {} positional argument(s), expected key/value pairs, at {}/{}",
          brief_type_of_value(head_form),
          args.len(),
          file_ns,
          def_name,
        ),
        file_ns,
        check_warnings,
      );
      return Ok(None);
    }

    let mut provided_fields: std::collections::HashMap<EdnTag, &Calcit> = std::collections::HashMap::new();
    let args_items = args.to_vec();
    for chunk in args_items.chunks(2) {
      if let [Calcit::Tag(key), value] = chunk {
        if !struct_def.fields.iter().any(|f| f == key) {
          gen_check_warning(
            format!(
              "[Warn] struct constructor rewrite skipped for `{}`: key `:{}` is not a field of struct `{}` at {}/{}",
              brief_type_of_value(head_form),
              key,
              struct_def.name,
              file_ns,
              def_name,
            ),
            file_ns,
            check_warnings,
          );
          return Ok(None);
        }
        if provided_fields.insert(key.to_owned(), value).is_some() {
          gen_check_warning(
            format!(
              "[Warn] struct constructor rewrite skipped for `{}`: duplicate field `:{}` at {}/{}",
              struct_def.name, key, file_ns, def_name,
            ),
            file_ns,
            check_warnings,
          );
          return Ok(None);
        }
      } else {
        gen_check_warning(
          format!(
            "[Warn] struct constructor rewrite skipped for `{}`: all arguments must be tag/value pairs at {}/{}",
            brief_type_of_value(head_form),
            file_ns,
            def_name,
          ),
          file_ns,
          check_warnings,
        );
        return Ok(None);
      }
    }

    for (idx, field) in struct_def.fields.iter().enumerate() {
      if !provided_fields.contains_key(field)
        && !struct_def
          .field_types
          .get(idx)
          .is_some_and(|field_type| matches!(field_type.as_ref(), CalcitTypeAnnotation::Optional(_)) || field_type.is_option_type())
      {
        gen_check_warning(
          format!(
            "[Warn] struct constructor rewrite skipped for `{}`: required field `:{}` is missing at {}/{}",
            struct_def.name, field, file_ns, def_name,
          ),
          file_ns,
          check_warnings,
        );
        return Ok(None);
      }
    }

    for (field, value) in &provided_fields {
      let Some(field_idx) = struct_def.fields.iter().position(|candidate| candidate == field) else {
        continue;
      };
      let Some(expected_type) = struct_def.field_types.get(field_idx) else {
        continue;
      };
      if matches!(expected_type.as_ref(), CalcitTypeAnnotation::Dynamic) {
        continue;
      }
      if let Some(actual_type) = resolve_type_value(value, scope_types)
        && !actual_type.as_ref().matches_annotation(expected_type.as_ref())
      {
        gen_check_warning(
          format!(
            "[Warn] struct `{}` field `:{}` expects type `{}`, but got `{}` at {}/{}",
            struct_def.name,
            field,
            expected_type.to_brief_string(),
            actual_type.to_brief_string(),
            file_ns,
            def_name,
          ),
          file_ns,
          check_warnings,
        );
      }
    }

    // `resolve_type_value` intentionally describes the definition value, but
    // that annotation does not always retain its source path. Keep the
    // already-resolved Import from the call head so JS codegen receives a
    // runtime reference instead of an embedded compiler-only StructDef.
    let constructor_path = constructor_definition_path(head_form).or(ns_def_path);
    let struct_ref_node = build_struct_ref_node(&struct_def, constructor_path, file_ns, def_name);
    let mut struct_items: Vec<Calcit> = Vec::with_capacity(struct_def.fields.len() * 2 + 2);
    struct_items.push(Calcit::Proc(CalcitProc::NativeStruct));
    struct_items.push(struct_ref_node);
    for (field_idx, field) in struct_def.fields.iter().enumerate() {
      struct_items.push(Calcit::Tag(field.to_owned()));
      if let Some(value) = provided_fields.get(field) {
        struct_items.push((*value).to_owned());
      } else if struct_def
        .field_types
        .get(field_idx)
        .is_some_and(|field_type| field_type.is_option_type())
      {
        // Nominal Option fields use the typed `%none` variant when omitted.
        // Keep legacy Optional<T> fields on their historical nil representation.
        struct_items.push(Calcit::from(vec![Calcit::Import(CalcitImport {
          ns: calcit::CORE_NS.into(),
          def: "%none".into(),
          info: Arc::new(ImportInfo::Core { at_ns: Arc::from(file_ns) }),
          def_id: None,
        })]));
      } else {
        struct_items.push(Calcit::Nil);
      }
    }

    return Ok(Some(Calcit::from(struct_items)));
  }

  let resolved_enum_definition = match type_info.as_ref() {
    CalcitTypeAnnotation::EnumDef(enum_def) => Some((enum_def.as_ref().clone(), None)),
    other => other.resolve_to_enum_with_ref(),
  };
  if constructor_kind == Some("defenum")
    && let Some((enum_def, ns_def_path)) = resolved_enum_definition
  {
    let Some(first_arg) = args.first() else {
      gen_check_warning(
        format!(
          "[Warn] enum constructor rewrite skipped: `{}` is missing variant tag at {}/{}",
          brief_type_of_value(head_form),
          file_ns,
          def_name
        ),
        file_ns,
        check_warnings,
      );
      return Ok(None);
    };

    let Calcit::Tag(tag) = first_arg else {
      gen_check_warning(
        format!(
          "[Warn] enum constructor rewrite skipped for `{}`: first argument should be a variant tag, at {}/{}",
          brief_type_of_value(head_form),
          file_ns,
          def_name,
        ),
        file_ns,
        check_warnings,
      );
      return Ok(None);
    };

    if enum_def.find_variant(tag).is_none() {
      let variants: Vec<&str> = enum_def.variants().iter().map(|v| v.tag.ref_str()).collect();
      gen_check_warning(
        format!(
          "[Warn] enum `{}` does not have variant `:{}`. Available: [{}], at {}/{}",
          enum_def.name(),
          tag.ref_str(),
          variants.join(", "),
          file_ns,
          def_name,
        ),
        file_ns,
        check_warnings,
      );
      return Ok(None);
    }

    // Direct `Op :variant` calls reach here with an Import head. Preserve its
    // namespace/definition path: lowering to an embedded enum prototype works
    // in the native evaluator but cannot be emitted as JavaScript.
    let constructor_path = constructor_definition_path(head_form).or(ns_def_path);
    let enum_ref_node = build_enum_ref_node(enum_def, constructor_path, file_ns, def_name);
    let mut items: Vec<Calcit> = Vec::with_capacity(args.len() + 1);
    items.push(Calcit::Proc(CalcitProc::NativeNamedEnumNew));
    items.push(enum_ref_node);
    items.extend(args.to_vec());

    return Ok(Some(Calcit::from(items)));
  }

  Ok(None)
}

fn constructor_definition_path(head_form: &Calcit) -> Option<(Arc<str>, Arc<str>)> {
  match head_form {
    Calcit::Import(CalcitImport { ns, def, .. }) => Some((ns.clone(), def.clone())),
    _ => None,
  }
}

fn data_definition_kind(ns: &str, def: &str) -> Option<&'static str> {
  let Calcit::List(code) = program::lookup_def_code(ns, def)? else {
    return None;
  };
  match code.first() {
    Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "defstruct" => Some("defstruct"),
    Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "defenum" => Some("defenum"),
    _ => None,
  }
}

/// detects arguments of top-level functions when possible
fn check_fn_marked_args(
  defined_args: &[CalcitArgLabel],
  arg_types: &[Arc<CalcitTypeAnnotation>],
  params: &CalcitList,
  file_ns: &str,
  f_name: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  let param_len = defined_args.iter().filter(|arg| matches!(arg, CalcitArgLabel::Idx(_))).count();
  let has_rest = defined_args.iter().any(|arg| matches!(arg, CalcitArgLabel::RestMark));
  let trailing_options = if has_rest {
    0
  } else {
    calcit::trailing_option_arg_count(arg_types, param_len)
  };
  if trailing_options > 0 && (param_len - trailing_options..=param_len).contains(&params.len()) {
    return;
  }

  let mut i = 0;
  let mut j = 0;
  let mut optional = false;

  loop {
    let d = defined_args.get(i);
    let r = params.get(j);

    match (d, r) {
      (None, None) => return,
      (_, Some(Calcit::Symbol { sym, .. })) if &**sym == "&" => {
        // dynamic values, can't tell yet
        return;
      }
      (Some(CalcitArgLabel::RestMark), _) => {
        // dynamic args rule, all okay
        return;
      }
      (Some(CalcitArgLabel::OptionalMark), _) => {
        // dynamic args rule, all okay
        optional = true;
        i += 1;
        continue;
      }
      (Some(_), None) => {
        if optional {
          i += 1;
          j += 1;
          continue;
        } else {
          gen_check_warning(
            format!("[Warn] lack of args in {f_name} `{defined_args:?}` with `{params}`, at {file_ns}/{def_name}"),
            file_ns,
            check_warnings,
          );
          return;
        }
      }
      (None, Some(_)) => {
        gen_check_warning(
          format!("[Warn] too many args for {f_name} `{defined_args:?}` with `{params}`, at {file_ns}/{def_name}"),
          file_ns,
          check_warnings,
        );
        return;
      }
      (Some(_), Some(_)) => {
        i += 1;
        j += 1;
        continue;
      }
    }
  }
}

/// quick path check function without marks
fn check_fn_args(
  defined_args: &[u16],
  arg_types: &[Arc<CalcitTypeAnnotation>],
  params: &CalcitList,
  file_ns: &str,
  f_name: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  let expected_size = defined_args.len();
  let actual_size = params.len();
  let trailing_options = calcit::trailing_option_arg_count(arg_types, expected_size);

  for (idx, item) in params.iter().enumerate() {
    if let Calcit::Syntax(CalcitSyntax::ArgSpread, _) = item {
      if expected_size < (idx + 1) {
        let args = CalcitLocal::display_args(defined_args);
        gen_check_warning(
          format!("[Warn] expected {expected_size} args in {f_name} `{args}`, got spreading form `{params}`, at {file_ns}/{def_name}"),
          file_ns,
          check_warnings,
        );
      }
      return; // no need to check
    }
  }

  let accepts_omission = trailing_options > 0 && (expected_size - trailing_options..=expected_size).contains(&actual_size);
  if expected_size != actual_size && !accepts_omission {
    gen_check_warning(
      format!("[Warn] expected {expected_size} args in {f_name} `{defined_args:?}` with `{params}`, at {file_ns}/{def_name}"),
      file_ns,
      check_warnings,
    );
  }
}

// Retrieves the definition name from a symbol or local receiver.
fn grab_def_name(x: &Calcit) -> Arc<str> {
  match x {
    Calcit::Symbol { info, .. } | Calcit::Local(CalcitLocal { info, .. }) => info.at_def.to_owned(),
    _ => String::from("??").into(),
  }
}

/// Capability validation for operations that lower directly to JavaScript.
///
/// This deliberately runs after ordinary resolution and does not alter type
/// annotations, trait lookup, or generic bindings. A function's `:features`
/// declares what its implementation body may do; callers do not inherit it.
fn require_js_ffi_feature(
  operation: &str,
  location: Option<NodeLocation>,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  if !codegen::codegen_mode() {
    return Ok(());
  }
  // Core runtime initialization uses internal lowering forms that are not
  // project-level FFI boundaries and do not carry public function schemas.
  if file_ns == calcit::CORE_NS {
    return Ok(());
  }
  validate_js_ffi_definition_target(operation, location.clone(), file_ns, def_name, call_stack)?;
  let policy = program::active_feature_policy("js-ffi");
  if matches!(policy, crate::snapshot::FeaturePolicy::Allow) {
    return Ok(());
  }

  let has_ffi = CURRENT_FN_FEATURES.with(|cell| {
    cell
      .borrow()
      .as_ref()
      .is_some_and(|features| features.contains(&EdnTag::new("js-ffi")))
  }) || program::lookup_def_schema(file_ns, def_name)
    .as_ref()
    .as_fn()
    .is_some_and(|fn_annot| fn_annot.features.contains(&EdnTag::new("js-ffi")));

  if has_ffi {
    return Ok(());
  }

  let message = format!(
    "[Warn] {operation} used in {file_ns}/{def_name} without `:js-ffi` feature in schema — isolate host operations in a binding function with `:features $ #{{}} :js-ffi`; read `calcit docs read js-interop.md --full` for the adapter and capability policy"
  );
  if matches!(policy, crate::snapshot::FeaturePolicy::Error) {
    return Err(CalcitErr::use_msg_stack_location_with_code(
      CalcitErrKind::Type,
      message.replacen("[Warn]", "[Error]", 1),
      "E_JS_FFI_FEATURE_REQUIRED",
      call_stack,
      location,
    ));
  }
  if let Some(location) = location {
    gen_check_warning_with_location_code(message, "W_JS_FFI_FEATURE_REQUIRED", location, check_warnings);
  } else {
    gen_check_warning_code(message, "W_JS_FFI_FEATURE_REQUIRED", file_ns, check_warnings);
  }
  Ok(())
}

fn ffi_metadata_value<'a>(ffi: &'a cirru_edn::Edn, key: &str) -> Option<&'a cirru_edn::Edn> {
  match ffi {
    cirru_edn::Edn::Struct(value) => value.pairs.iter().find(|(field, _)| field.ref_str() == key).map(|(_, value)| value),
    cirru_edn::Edn::Map(value) => value.get(&cirru_edn::Edn::tag(key)),
    _ => None,
  }
}

fn ffi_metadata_target(ffi: &cirru_edn::Edn) -> Option<crate::snapshot::SnapshotTarget> {
  let value = ffi_metadata_value(ffi, "target")?;
  let name = match value {
    cirru_edn::Edn::Tag(tag) => tag.ref_str(),
    cirru_edn::Edn::Str(text) | cirru_edn::Edn::Symbol(text) => text.trim_start_matches(':'),
    _ => return None,
  };
  match name {
    "browser" => Some(crate::snapshot::SnapshotTarget::Browser),
    "node" => Some(crate::snapshot::SnapshotTarget::Node),
    "native" => Some(crate::snapshot::SnapshotTarget::Native),
    "wasm" => Some(crate::snapshot::SnapshotTarget::Wasm),
    _ => None,
  }
}

fn validate_js_ffi_target(
  expected: crate::snapshot::SnapshotTarget,
  operation: &str,
  location: Option<NodeLocation>,
  file_ns: &str,
  def_name: &str,
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  let Some(active) = program::active_entry_target() else {
    return Ok(());
  };
  if active == expected {
    return Ok(());
  }
  let message = format!(
    "[Error] {operation} requires `{}` target, but the selected entry targets `{}` in {file_ns}/{def_name}; read `calcit docs read js-interop.md --full` for target-specific bindings",
    expected.as_str(),
    active.as_str()
  );
  Err(CalcitErr::use_msg_stack_location_with_code(
    CalcitErrKind::Type,
    message,
    "E_JS_FFI_TARGET_MISMATCH",
    call_stack,
    location,
  ))
}

fn validate_js_ffi_definition_target(
  operation: &str,
  location: Option<NodeLocation>,
  file_ns: &str,
  def_name: &str,
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  let Some(ffi) = program::lookup_def_ffi(file_ns, def_name) else {
    return Ok(());
  };
  let Some(expected) = ffi_metadata_target(&ffi) else {
    return Ok(());
  };
  validate_js_ffi_target(expected, operation, location, file_ns, def_name, call_stack)
}

fn js_ffi_operation_name(head: &Calcit) -> Option<&'static str> {
  match head {
    Calcit::Symbol { sym, .. } if matches!(sym.as_ref(), "js-get" | "aget") => Some("JavaScript field read"),
    Calcit::Symbol { sym, .. } if matches!(sym.as_ref(), "js-set" | "aset") => Some("JavaScript field write"),
    Calcit::Method(_, calcit::MethodKind::InvokeNative | calcit::MethodKind::InvokeNativeOptional) => {
      Some("native JavaScript method call")
    }
    Calcit::Method(_, calcit::MethodKind::Access | calcit::MethodKind::AccessOptional) => Some("native JavaScript property access"),
    Calcit::Method(_, calcit::MethodKind::ExternalAccess(_)) => Some("external-object field access"),
    Calcit::Method(_, calcit::MethodKind::ExternalGet(_)) => Some("external-object field read"),
    Calcit::Method(_, calcit::MethodKind::ExternalSet(_)) => Some("external-object field write"),
    Calcit::Method(_, calcit::MethodKind::ExternalInvoke(_)) => Some("external-object method call"),
    Calcit::Import(CalcitImport { ns, def, .. }) if ns.as_ref() == calcit::CORE_NS && def.as_ref() == "unsafe-coerce" => {
      Some("unsafe host assertion `unsafe-coerce`")
    }
    _ => None,
  }
}

fn require_js_ffi_feature_for_operation(
  head: &Calcit,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  if let Some(operation) = js_ffi_operation_name(head) {
    if let Some(receiver_type) = match head {
      Calcit::Method(_, calcit::MethodKind::ExternalAccess(value))
      | Calcit::Method(_, calcit::MethodKind::ExternalGet(value))
      | Calcit::Method(_, calcit::MethodKind::ExternalSet(value))
      | Calcit::Method(_, calcit::MethodKind::ExternalInvoke(value)) => Some(value.as_ref()),
      _ => None,
    } && let Some(traits) = trait_list_from_type(receiver_type)
    {
      for trait_def in traits {
        if let Some(ffi) = trait_def
          .definition_ref
          .as_deref()
          .and_then(|definition| definition.rsplit_once('/'))
          .and_then(|(ns, def)| program::lookup_def_ffi(ns, def))
          && let Some(expected) = ffi_metadata_target(&ffi)
        {
          validate_js_ffi_target(expected, operation, head.get_location(), file_ns, def_name, call_stack)?;
        }
      }
    }
    require_js_ffi_feature(operation, head.get_location(), file_ns, def_name, check_warnings, call_stack)?;
  }
  Ok(())
}

pub(crate) fn gen_check_warning(message: String, file_ns: &str, check_warnings: &RefCell<Vec<LocatedWarning>>) {
  let loc = NodeLocation::new(Arc::from(file_ns), Arc::from(GENERATED_DEF), Arc::from(vec![]));
  gen_check_warning_with_location(message, loc, check_warnings);
}

pub(crate) fn gen_check_warning_code(
  message: String,
  code: &'static str,
  file_ns: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  let loc = NodeLocation::new(Arc::from(file_ns), Arc::from(GENERATED_DEF), Arc::from(vec![]));
  gen_check_warning_with_location_code(message, code, loc, check_warnings);
}

pub(crate) fn gen_check_warning_code_at(
  message: String,
  code: &'static str,
  file_ns: &str,
  location: Option<NodeLocation>,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  if let Some(location) = location {
    gen_check_warning_with_location_code(message, code, location, check_warnings);
  } else {
    gen_check_warning_code(message, code, file_ns, check_warnings);
  }
}

fn gen_check_warning_with_location(message: String, location: NodeLocation, check_warnings: &RefCell<Vec<LocatedWarning>>) {
  let mut warnings = check_warnings.borrow_mut();
  warnings.push(LocatedWarning::new(message, location));
}

fn gen_check_warning_with_location_code(
  message: String,
  code: &'static str,
  location: NodeLocation,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  let mut warnings = check_warnings.borrow_mut();
  warnings.push(LocatedWarning::new_with_detail(message, location, Some(code.to_string()), None));
}

fn derive_call_expr_location(head: &Calcit) -> Option<NodeLocation> {
  let location = head.get_location()?;
  let mut parent_coord = (*location.coord).clone();
  parent_coord.pop();
  Some(NodeLocation::new(
    location.ns.clone(),
    location.def.clone(),
    Arc::from(parent_coord),
  ))
}

/// Check recur arity in function body
/// Recursively walks the expression tree to find recur calls and validates argument count
/// Skips checking for macro-generated functions (containing %, $, etc.)
fn check_recur_arity_in_expr(
  expr: &Calcit,
  expected_arity: usize,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  match expr {
    Calcit::Recur(args) => {
      // Runtime recur value (from macro expansion)
      let actual_arity = args.len();
      if actual_arity != expected_arity {
        let location = expr
          .get_location()
          .unwrap_or_else(|| NodeLocation::new(Arc::from(file_ns), Arc::from(def_name), Arc::new(vec![])));
        gen_check_warning_with_location(
          format!("[Warn] recur expects {expected_arity} args but got {actual_arity} in {file_ns}/{def_name}"),
          location,
          check_warnings,
        );
      }
      // Also check nested expressions in recur arguments
      for arg in args {
        check_recur_arity_in_expr(arg, expected_arity, file_ns, def_name, check_warnings);
      }
    }
    Calcit::List(xs) => {
      if xs.is_empty() {
        return;
      }
      if let Some(Calcit::Syntax(s, _)) = xs.first()
        && (s == &CalcitSyntax::Quote || s == &CalcitSyntax::Quasiquote)
      {
        // Do not inspect quoted data for recur arity.
        return;
      }
      // Check if this is a recur call: (recur arg1 arg2 ...)
      if let Some(Calcit::Proc(CalcitProc::Recur)) = xs.first() {
        // This is a recur call in preprocessed form
        let actual_arity = xs.len() - 1; // Subtract 1 for the recur proc itself
        if actual_arity != expected_arity {
          let location = expr
            .get_location()
            .unwrap_or_else(|| NodeLocation::new(Arc::from(file_ns), Arc::from(def_name), Arc::new(vec![])));
          gen_check_warning_with_location(
            format!("[Warn] recur expects {expected_arity} args but got {actual_arity} in {file_ns}/{def_name}"),
            location,
            check_warnings,
          );
        }
      } else if let Some(Calcit::Syntax(s, _)) = xs.first()
        && (s == &CalcitSyntax::Defn
          || s == &CalcitSyntax::Defmacro
          || s == &CalcitSyntax::DefWasmExport
          || s == &CalcitSyntax::DefWasmImport)
      {
        // This is a separate function scope. It will be checked by its own preprocess_defn call.
        return;
      }
      // Recurse into all list items
      for item in xs.iter() {
        check_recur_arity_in_expr(item, expected_arity, file_ns, def_name, check_warnings);
      }
    }
    Calcit::Fn { info, .. } => {
      // Check recur inside nested lambda functions with their own arity
      // Get the arity of this nested function
      let nested_arity = match &*info.args {
        CalcitFnArgs::Args(args) => args.len(),
        CalcitFnArgs::MarkedArgs(args) => {
          // Count actual parameters, excluding & and ? markers
          args
            .iter()
            .filter(|a| !matches!(a, CalcitArgLabel::RestMark | CalcitArgLabel::OptionalMark))
            .count()
        }
      };
      // Check body with nested function's arity
      for body_expr in &info.body {
        check_recur_arity_in_expr(body_expr, nested_arity, file_ns, def_name, check_warnings);
      }
    }
    _ => {
      // For other types, we don't need to recurse into them
      // because recur can only appear in certain contexts
    }
  }
}

fn check_impl_traits_top_level_in_expr(expr: &Calcit, file_ns: &str, def_name: &str, check_warnings: &RefCell<Vec<LocatedWarning>>) {
  if !warn_dyn_method_enabled() {
    return;
  }

  match expr {
    Calcit::List(xs) => {
      if xs.is_empty() {
        return;
      }

      if let Some(Calcit::Syntax(s, _)) = xs.first()
        && (s == &CalcitSyntax::Quote || s == &CalcitSyntax::Quasiquote)
      {
        return;
      }

      let is_impl_traits = matches!(
        xs.first(),
        Some(Calcit::Import(CalcitImport { ns, def, .. })) if ns.as_ref() == calcit::CORE_NS && def.as_ref() == "impl-traits"
      ) || matches!(xs.first(), Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "impl-traits");

      if is_impl_traits {
        let msg = format!(
          "[Warn] `impl-traits` inside {file_ns}/{def_name} may block preprocess specialization; prefer top-level `def` bindings"
        );
        if let Some(loc) = expr.get_location() {
          gen_check_warning_with_location(msg, loc.clone(), check_warnings);
        } else {
          gen_check_warning(msg, file_ns, check_warnings);
        }
      }

      for item in xs.iter() {
        check_impl_traits_top_level_in_expr(item, file_ns, def_name, check_warnings);
      }
    }
    Calcit::Fn { info, .. } => {
      for body_expr in &info.body {
        check_impl_traits_top_level_in_expr(body_expr, file_ns, def_name, check_warnings);
      }
    }
    _ => {}
  }
}

/// Check struct field access during preprocessing
/// Validates that field names exist in struct types when type information is available
fn check_struct_field_access(
  head: &Calcit,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  call_stack: &CallStackList,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  // Check if this is a call to &struct:get
  if let Calcit::Proc(CalcitProc::NativeStructGet) = head {
    // &struct:get takes 2 args: (struct_value, field)
    if args.len() >= 2
      && let (Some(struct_arg), Some(field_arg)) = (args.first(), args.get(1))
    {
      check_field_in_struct(struct_arg, field_arg, scope_types, file_ns, check_warnings);
      warn_on_raw_struct_field_access(struct_arg, field_arg, scope_types, file_ns, call_stack, check_warnings);
    }
  }
  // Also check calcit.core imports that perform required struct field access.
  else if let Calcit::Import(CalcitImport { ns, def, .. }) = head {
    if &**ns == calcit::CORE_NS
      && (&**def == "record-get" || &**def == "&struct:get")
      && args.len() >= 2
      && let (Some(struct_arg), Some(field_arg)) = (args.first(), args.get(1))
    {
      check_field_in_struct(struct_arg, field_arg, scope_types, file_ns, check_warnings);
      warn_on_raw_struct_field_access(struct_arg, field_arg, scope_types, file_ns, call_stack, check_warnings);
    }
    if &**ns == calcit::CORE_NS
      && &**def == "get"
      && args.len() >= 2
      && let Some(struct_arg) = args.first()
      && let Some(type_info) = resolve_type_value(struct_arg, scope_types)
      && (type_info.as_ref().resolve_to_struct().is_some() || is_anonymous_struct_type(type_info.as_ref()))
    {
      let field_text = args.get(1).map(Calcit::lisp_str).unwrap_or_else(|| "<field>".to_owned());
      let message = format!(
        "[Warn] `get` is the Option-returning lookup API for maps and indexed collections, not Struct fields, at {file_ns}. Use `({field_text} value)` so the checker can return the field's declared type and reject unknown fields"
      );
      gen_check_warning_code_at(
        message,
        "W_STRUCT_FIELD_OPTIONAL_LOOKUP",
        file_ns,
        struct_arg.get_location(),
        check_warnings,
      );
    }
    if &**ns == calcit::CORE_NS
      && matches!(&**def, "get-in" | "contains-in?" | "assoc-in" | "update-in" | "dissoc-in")
      && args.len() >= 2
      && let (Some(base_arg), Some(path_arg)) = (args.first(), args.get(1))
      && let Some(base_type) = resolve_type_value(base_arg, scope_types)
    {
      let literal_path = extract_literal_list_items(path_arg);
      let known_struct_with_dynamic_path =
        literal_path.is_none() && (base_type.as_ref().resolve_to_struct().is_some() || is_anonymous_struct_type(base_type.as_ref()));
      let struct_step = find_struct_lookup_in_literal_path(base_type.as_ref(), path_arg);

      if known_struct_with_dynamic_path || struct_step.is_some() {
        let (segment_text, location_text, location) = match struct_step {
          Some((index, segment)) => (
            format!("segment {} `{}`", index + 1, segment.lisp_str()),
            "enters a Struct".to_owned(),
            segment.get_location().or_else(|| path_arg.get_location()),
          ),
          None => (
            "a dynamic path".to_owned(),
            "starts from a Struct and cannot prove that the path is empty".to_owned(),
            path_arg.get_location().or_else(|| base_arg.get_location()),
          ),
        };
        let message = format!(
          "[Warn] `{def}` {location_text} at {segment_text} in {file_ns}. Collection path APIs do not traverse Struct fields. End the path before the Struct, unwrap/narrow the value, then use `(:field value)` for reads or `assoc`/`update` for declared field writes"
        );
        gen_check_warning_code_at(message, "W_STRUCT_PATH_OPERATION", file_ns, location, check_warnings);
      }
    }
  }
  // Check for Method(Access) which handles .-field syntax: (.-field struct_value)
  else if let Calcit::Method(field_name, calcit::MethodKind::Access) = head {
    // .-field takes 1 arg: the struct
    if let Some(struct_arg) = args.first() {
      // Create a tag for the field name to match the check_field_in_struct signature
      let field_tag = Calcit::Tag(cirru_edn::EdnTag::from(&**field_name));
      check_field_in_struct(struct_arg, &field_tag, scope_types, file_ns, check_warnings);
    }
  }
}

fn warn_on_raw_struct_field_access(
  receiver: &Calcit,
  field: &Calcit,
  scope_types: &ScopeTypes,
  file_ns: &str,
  call_stack: &CallStackList,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  if !should_emit_project_source_lint(file_ns)
    || file_ns == calcit::CORE_NS
    || call_stack
      .0
      .iter()
      .any(|frame| matches!(frame.kind, StackKind::Macro) && frame.def.as_ref() == "defimpl")
  {
    return;
  }

  let Calcit::Tag(field_tag) = field else {
    return;
  };
  let field_text = format!(":{}", field_tag.ref_str());
  let receiver_type = resolve_type_value(receiver, scope_types);
  let (code, message) = match receiver_type.as_deref().and_then(CalcitTypeAnnotation::resolve_to_struct) {
    Some(_) => (
      "W_STRUCT_RAW_ACCESS",
      format!(
        "[Warn] direct `&struct:get` for field `{field_text}` in {file_ns} bypasses the typed source syntax. Use `({field_text} value)` or `value.{field_text}` so the checker exposes the declared field type and lowers the read to indexed `&struct:nth` access"
      ),
    ),
    None if matches!(receiver_type.as_deref(), Some(CalcitTypeAnnotation::TypeRef(..))) => {
      let type_text = receiver_type
        .as_deref()
        .map(CalcitTypeAnnotation::to_brief_string)
        .unwrap_or_else(|| ":unknown".to_owned());
      (
        "W_STRUCT_DYNAMIC_RAW_ACCESS",
        format!(
          "[Warn] direct `&struct:get` for field `{field_text}` in {file_ns} has unresolved nominal receiver `{type_text}`. Its declaration namespace or dependency could not be recovered, so the field cannot be checked or specialized. Keep/restore a qualified schema such as `'app.schema/Type`, then use `({field_text} value)` or `value.{field_text}`; do not use `&struct:get` to hide an unresolved TypeRef"
        ),
      )
    }
    None => {
      let type_text = receiver_type
        .as_deref()
        .map(CalcitTypeAnnotation::to_brief_string)
        .unwrap_or_else(|| ":unknown".to_owned());
      (
        "W_STRUCT_DYNAMIC_RAW_ACCESS",
        format!(
          "[Warn] direct `&struct:get` for field `{field_text}` in {file_ns} has receiver type `{type_text}`, so the field cannot be statically checked or specialized. Add/narrow a named Struct schema, then use `({field_text} value)` or `value.{field_text}`; reserve `&struct:get` for an intentional reusable `defimpl` or core/runtime boundary"
        ),
      )
    }
  };

  gen_check_warning_code_at(
    message,
    code,
    file_ns,
    field.get_location().or_else(|| receiver.get_location()),
    check_warnings,
  );
}

/// Validate statically known struct updates before they are rewritten to the indexed runtime procs.
/// This is the checking counterpart to preserving the receiver's nominal return type in inference.
fn check_struct_update_fields(
  head: &Calcit,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  let pairs: Vec<(&Calcit, &Calcit)> = match head {
    Calcit::Proc(CalcitProc::NativeStructAssoc) if args.len() == 3 => match (args.get(1), args.get(2)) {
      (Some(field), Some(value)) => vec![(field, value)],
      _ => return,
    },
    Calcit::Proc(CalcitProc::NativeStructAssocAt) if args.len() == 4 => match (args.get(2), args.get(3)) {
      (Some(field), Some(value)) => vec![(field, value)],
      _ => return,
    },
    Calcit::Proc(CalcitProc::NativeStructWith) if args.len() >= 3 && (args.len() - 1).is_multiple_of(2) => {
      let items = args.iter().skip(1).collect::<Vec<_>>();
      items.chunks_exact(2).map(|pair| (pair[0], pair[1])).collect()
    }
    Calcit::Proc(CalcitProc::NativeStructWithAt) if args.len() >= 4 && (args.len() - 1).is_multiple_of(3) => {
      let items = args.iter().skip(1).collect::<Vec<_>>();
      items.chunks_exact(3).map(|triple| (triple[1], triple[2])).collect()
    }
    _ => return,
  };

  let Some(struct_arg) = args.first() else { return };
  for (field_arg, value_arg) in pairs {
    check_field_in_struct(struct_arg, field_arg, scope_types, file_ns, check_warnings);

    let field_name = match field_arg {
      Calcit::Tag(tag) => tag.ref_str(),
      Calcit::Str(name) => name.as_ref(),
      Calcit::Symbol { sym, .. } => sym.as_ref(),
      _ => continue,
    };
    let Some(expected_type) = infer_struct_field_type(struct_arg, field_name, scope_types) else {
      continue;
    };
    if matches!(expected_type.as_ref(), CalcitTypeAnnotation::Dynamic) {
      continue;
    }
    if let Some(actual_type) = resolve_type_value(value_arg, scope_types)
      && !actual_type.as_ref().matches_annotation(expected_type.as_ref())
    {
      gen_check_warning(
        format!(
          "[Warn] struct update field `:{field_name}` expects type `{}`, but got `{}` at {file_ns}/{def_name}",
          expected_type.to_brief_string(),
          actual_type.to_brief_string(),
        ),
        file_ns,
        check_warnings,
      );
    }
  }
}

/// Helper to validate a field exists in a struct type
fn check_field_in_struct(
  struct_arg: &Calcit,
  field_arg: &Calcit,
  scope_types: &ScopeTypes,
  file_ns: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  // Macro-generated struct dispatch (for example struct pattern matching)
  // may deliberately probe variant-specific fields after its own nominal
  // guard. Source-level field diagnostics are emitted before expansion, so do
  // not report those internal probes as user mistakes.
  if [struct_arg.get_location(), field_arg.get_location()]
    .into_iter()
    .flatten()
    .any(|location| location.def.as_ref() == GENERATED_DEF)
  {
    return;
  }

  // Get the type of the struct argument - reuse resolve_type_value
  let Some(type_info) = resolve_type_value(struct_arg, scope_types) else {
    return; // No type info available
  };

  // Only validate struct types
  let Some(struct_def) = type_info.as_ref().resolve_to_struct() else {
    return; // Not a struct type
  };

  // Extract field name from the argument
  let field_name = match field_arg {
    Calcit::Tag(tag) => tag.ref_str(),
    Calcit::Str(s) => s.as_ref(),
    Calcit::Symbol { sym, .. } => sym.as_ref(),
    _ => return, // Can't check dynamic field names
  };

  // Check if field exists in struct
  if struct_def.index_of(field_name).is_some() {
    return; // Field found, validation passed
  }

  // Field not found, generate warning
  let available_fields: Vec<&str> = struct_def.fields.iter().map(|f| f.ref_str()).collect();
  gen_check_warning_code_at(
    format!(
      "[Warn] Field `:{field_name}` does not exist in struct `{}`. Available fields: [{}]. Struct field access is required and never returns nil/Option for a missing field; use a declared field instead",
      struct_def.name,
      available_fields
        .iter()
        .map(|field| format!(":{field}"))
        .collect::<Vec<_>>()
        .join(", ")
    ),
    "W_UNKNOWN_STRUCT_FIELD",
    file_ns,
    field_arg.get_location().or_else(|| struct_arg.get_location()),
    check_warnings,
  );
}

/// Check enum index bounds for &enum:nth operations.
/// NOTE: Static bounds checking is not available for a dynamic enum since
/// the enum definition does not carry the per-variant payload sizes. This function is
/// a no-op: bounds errors will be caught at runtime instead.
pub(crate) fn check_enum_nth_bounds(
  _args: &CalcitList,
  _scope_types: &ScopeTypes,
  _file_ns: &str,
  _def_name: &str,
  _check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
}

/// Check enum construction (%::) for variant existence and payload arity
pub(crate) fn check_enum_construction(
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  // %:: takes: (enum-proto, tag, ...payloads)
  // args here excludes the proc itself, so: (enum-proto, tag, ...payloads)
  if args.len() < 2 {
    return; // Not enough args to check
  }

  let enum_arg = match args.first() {
    Some(arg) => arg,
    None => return,
  };

  let tag_arg = match args.get(1) {
    Some(arg) => arg,
    None => return,
  };

  // Resolve enum prototype
  let Some(enum_proto) = resolve_enum_value(enum_arg, scope_types) else {
    return; // Can't resolve enum, skip check
  };

  // Extract tag name
  let tag_name = match tag_arg {
    Calcit::Tag(tag) => tag.ref_str(),
    Calcit::Symbol { sym, .. } => sym.as_ref(),
    _ => return, // Dynamic tag, can't check statically
  };

  // Check if variant exists
  let Some(variant) = enum_proto.find_variant_by_name(tag_name) else {
    let available_variants: Vec<&str> = enum_proto.variants().iter().map(|v| v.tag.ref_str()).collect();
    gen_check_warning(
      format!(
        "[Warn] Enum `{}` does not have variant `:{tag_name}`. Available variants: [{}], at {file_ns}/{def_name}",
        enum_proto.name(),
        available_variants.join(", ")
      ),
      file_ns,
      check_warnings,
    );
    return;
  };

  // Check payload arity
  let expected_arity = variant.arity();
  let actual_arity = args.len().saturating_sub(2); // Subtract enum-proto and tag

  if expected_arity != actual_arity {
    gen_check_warning(
      format!(
        "[Warn] Enum `{}::{}` expects {} payload(s), but got {}, at {file_ns}/{def_name}",
        enum_proto.name(),
        tag_name,
        expected_arity,
        actual_arity
      ),
      file_ns,
      check_warnings,
    );
    return;
  }

  // Check payload types
  for (idx, (payload_arg, expected_type)) in args.iter().skip(2).zip(variant.payload_types().iter()).enumerate() {
    if matches!(expected_type.as_ref(), CalcitTypeAnnotation::Dynamic) {
      continue; // No type constraint for this payload
    }

    if let Some(actual_type) = resolve_type_value(payload_arg, scope_types)
      && !actual_type.as_ref().matches_annotation(expected_type.as_ref())
    {
      let expected_str = expected_type.as_ref().to_brief_string();
      let actual_str = actual_type.as_ref().to_brief_string();
      gen_check_warning(
        format!(
          "[Warn] Enum `{}::{}` payload {} expects type `{expected_str}`, but got `{actual_str}`, at {file_ns}/{def_name}",
          enum_proto.name(),
          tag_name,
          idx + 1
        ),
        file_ns,
        check_warnings,
      );
    }
  }
}
/// Check struct method call arguments (count and types)
/// Validates that method calls have correct number and types of arguments
fn check_struct_method_args(
  head: &Calcit,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  // Only check Method(Invoke) calls
  let Calcit::Method(method_name, calcit::MethodKind::Invoke(_) | calcit::MethodKind::ExternalInvoke(_)) = head else {
    return;
  };

  // Need receiver to get method info
  let Some(receiver) = args.first() else {
    return;
  };

  // Get receiver type
  let Some(type_value) = resolve_type_value(receiver, scope_types) else {
    return; // No type info, skip check
  };

  if let Some(traits) = trait_list_from_type(type_value.as_ref())
    && let Some((trait_def, method_type)) = find_trait_method_type(&traits, method_name.as_ref())
  {
    let Some(signature) = method_type.as_function() else {
      return;
    };

    let Ok(method_args) = args.skip(1) else {
      return;
    };

    let expected_count = signature.arg_types.len();
    let actual_with_receiver = method_args.len() + 1;
    let trailing_options = if signature.rest_type.is_none() {
      calcit::trailing_option_arg_count(&signature.arg_types, expected_count)
    } else {
      0
    };
    let accepts_omission = trailing_options > 0 && (expected_count - trailing_options..=expected_count).contains(&actual_with_receiver);
    if expected_count != 0 && expected_count != actual_with_receiver && !accepts_omission {
      gen_check_warning(
        format!(
          "[Warn] Method `.{method_name}` expects {expected_count} args (including receiver), got {actual_with_receiver} in call at {file_ns}/{def_name}"
        ),
        file_ns,
        check_warnings,
      );
      return;
    }

    let mut bindings: HashMap<Arc<str>, Arc<CalcitTypeAnnotation>> = HashMap::new();
    if let Some(expected_receiver) = signature.arg_types.first()
      && let Some(receiver_type) = resolve_type_value(receiver, scope_types)
    {
      receiver_type
        .as_ref()
        .matches_with_bindings(expected_receiver.as_ref(), &mut bindings);
    }
    let arg_types_without_receiver = signature.arg_types.iter().skip(1);
    for (idx, (arg, expected_type)) in method_args.iter().zip(arg_types_without_receiver).enumerate() {
      if matches!(**expected_type, CalcitTypeAnnotation::Dynamic) {
        continue;
      }

      if let Some(actual_type) = resolve_type_value(arg, scope_types)
        && !actual_type.as_ref().matches_with_bindings(expected_type.as_ref(), &mut bindings)
      {
        let expected_str = expected_type.substitute_type_vars(&bindings).to_brief_string();
        let actual_str = actual_type.as_ref().to_brief_string();
        gen_check_warning(
          format!(
            "[Warn] Method `.{method_name}` arg {} expects type `{expected_str}`, but got `{actual_str}` in call at {file_ns}/{def_name} (trait {})",
            idx + 2,
            trait_def.name
          ),
          file_ns,
          check_warnings,
        );
      }
    }
    return;
  }

  // Get impl structs for the type
  let Some(impl_values) = get_impls_from_type(&type_value) else {
    return; // No impl struct, skip check
  };

  // Get method entry from impl records
  let method_str = method_name.as_ref();
  let Some(method_entry) = find_method_entry_for_type(type_value.as_ref(), &impl_values, method_str) else {
    return; // Method not found (will be caught by validate_method_call)
  };

  // Get function info from method entry
  let declared_schema = match method_entry {
    Calcit::Import(import) => Some((
      program::lookup_def_schema(&import.ns, &import.def),
      format!("{} / {}", import.ns, import.def),
    )),
    Calcit::Fn { info, .. } if info.def_ref.is_some() => Some((
      program::lookup_def_schema(&info.def_ns, &info.name),
      format!("{} / {}", info.def_ns, info.name),
    )),
    _ => None,
  };
  if let Some((schema, implementation_name)) = declared_schema
    && schema.contains_type_var()
    && type_value.as_ref().resolve_to_enum().is_some()
  {
    let Some(signature) = schema.as_function() else {
      return;
    };
    let Ok(method_args) = args.skip(1) else {
      return;
    };
    let expected_count = signature.arg_types.len();
    let actual_with_receiver = method_args.len() + 1;
    let trailing_options = if signature.rest_type.is_none() {
      calcit::trailing_option_arg_count(&signature.arg_types, expected_count)
    } else {
      0
    };
    let accepts_omission = trailing_options > 0 && (expected_count - trailing_options..=expected_count).contains(&actual_with_receiver);
    if expected_count != 0 && expected_count != actual_with_receiver && !accepts_omission {
      gen_check_warning(
        format!(
          "[Warn] Method `.{method_name}` expects {expected_count} args (including receiver), got {actual_with_receiver} in call at {file_ns}/{def_name}"
        ),
        file_ns,
        check_warnings,
      );
      return;
    }

    let mut bindings: HashMap<Arc<str>, Arc<CalcitTypeAnnotation>> = HashMap::new();
    if let Some(expected_receiver) = signature.arg_types.first() {
      type_value.as_ref().matches_with_bindings(expected_receiver.as_ref(), &mut bindings);
    }
    for (idx, (arg, expected_type)) in method_args.iter().zip(signature.arg_types.iter().skip(1)).enumerate() {
      if matches!(**expected_type, CalcitTypeAnnotation::Dynamic) {
        continue;
      }
      if let Some(actual_type) = resolve_type_value(arg, scope_types)
        && !actual_type.as_ref().matches_with_bindings(expected_type.as_ref(), &mut bindings)
      {
        let expected_str = expected_type.substitute_type_vars(&bindings).to_brief_string();
        let actual_str = actual_type.as_ref().to_brief_string();
        gen_check_warning_code(
          format!(
            "[Warn] Method `.{method_name}` arg {} expects type `{expected_str}`, but got `{actual_str}` in call at {file_ns}/{def_name} (implementation {implementation_name})",
            idx + 2,
          ),
          "W_METHOD_ARG_TYPE_MISMATCH",
          file_ns,
          check_warnings,
        );
      }
    }
    return;
  }

  let fn_info: Option<&CalcitFn> = match method_entry {
    Calcit::Fn { info, .. } => Some(info.as_ref()),
    Calcit::Proc(_proc) => {
      // Procs will be inlined and checked by check_proc_arg_types later
      // Skip checking here to avoid duplicate warnings
      return;
    }
    _ => None,
  };

  let Some(fn_info) = fn_info else {
    return; // Can't get function info, skip check
  };

  // Method args exclude receiver (first argument in args list)
  let Ok(method_args) = args.skip(1) else {
    return;
  };

  // Check argument count
  // For method calls like `data .map f`, the receiver is already the first arg
  // So we need: actual_count + 1 (receiver) = expected_count
  let expected_count = fn_info.args.as_ref().param_len();
  let actual_count = method_args.len();
  let actual_with_receiver = actual_count + 1; // Include receiver in count

  // Check for variadic args (has RestMark)
  let has_variadic = match fn_info.args.as_ref() {
    CalcitFnArgs::MarkedArgs(xs) => xs.iter().any(|label| matches!(label, CalcitArgLabel::RestMark)),
    CalcitFnArgs::Args(_) => false,
  };

  let trailing_options = if has_variadic {
    0
  } else {
    calcit::trailing_option_arg_count(&fn_info.arg_types, expected_count)
  };
  let accepts_omission = trailing_options > 0 && (expected_count - trailing_options..=expected_count).contains(&actual_with_receiver);
  if !has_variadic && expected_count != actual_with_receiver && !accepts_omission {
    gen_check_warning(
      format!(
        "[Warn] Method `.{method_name}` expects {expected_count} args (including receiver), got {actual_with_receiver} in call at {file_ns}/{def_name}"
      ),
      file_ns,
      check_warnings,
    );
    return;
  }

  // Check argument types if available
  // method_args excludes receiver, but arg_types[0] is for receiver
  // So we need to skip the first type and check remaining args
  let mut bindings: HashMap<Arc<str>, Arc<CalcitTypeAnnotation>> = HashMap::new();
  let arg_types_without_receiver: Vec<Arc<CalcitTypeAnnotation>> = fn_info.arg_types.iter().skip(1).cloned().collect();

  for (idx, (arg, expected_type)) in method_args.iter().zip(arg_types_without_receiver.iter()).enumerate() {
    if matches!(**expected_type, CalcitTypeAnnotation::Dynamic) {
      continue; // No type constraint for this argument
    }

    // Handle variadic argument type (same as check_user_fn_arg_types)
    if let CalcitTypeAnnotation::Variadic(inner_type) = expected_type.as_ref() {
      for (rest_idx, rest_arg) in method_args.iter().skip(idx).enumerate() {
        if let Some(actual_type) = resolve_type_value(rest_arg, scope_types)
          && !actual_type.as_ref().matches_with_bindings(inner_type.as_ref(), &mut bindings)
        {
          let expected_str = inner_type.as_ref().to_brief_string();
          let actual_str = actual_type.as_ref().to_brief_string();
          gen_check_warning(
            format!(
              "[Warn] Method `.{method_name}` variadic arg {} expects type `{expected_str}`, but got `{actual_str}` in call at {file_ns}/{def_name}",
              idx + rest_idx + 2
            ),
            file_ns,
            check_warnings,
          );
        }
      }
      return;
    }

    if let Some(actual_type) = resolve_type_value(arg, scope_types) {
      // Compare types
      if !actual_type.as_ref().matches_with_bindings(expected_type.as_ref(), &mut bindings) {
        let expected_str = expected_type.as_ref().to_brief_string();
        let actual_str = actual_type.as_ref().to_brief_string();
        gen_check_warning(
          format!(
            "[Warn] Method `.{method_name}` arg {} expects type `{expected_str}`, but got `{actual_str}` in call at {file_ns}/{def_name}",
            idx + 2 // +2 because idx is 0-based and we skip receiver (arg 1)
          ),
          file_ns,
          check_warnings,
        );
      }
    }
  }
}

/// Resolve method argument types after binding the receiver's generic payloads.
/// This is used before preprocessing inline callbacks, so their parameter and
/// return contracts are available while the callback body is checked.
fn expected_method_argument_types(type_value: &CalcitTypeAnnotation, method_name: &str) -> Option<Vec<Arc<CalcitTypeAnnotation>>> {
  let signature = if let Some(traits) = trait_list_from_type(type_value) {
    find_trait_method_type(&traits, method_name).map(|(_, method_type)| method_type.clone())?
  } else {
    let impl_values = get_impls_from_type(type_value)?;
    let method_entry = find_method_entry_for_type(type_value, &impl_values, method_name)?;
    match method_entry {
      Calcit::Import(import) => program::lookup_def_schema(&import.ns, &import.def),
      Calcit::Fn { info, .. } if info.def_ref.is_some() => program::lookup_def_schema(&info.def_ns, &info.name),
      Calcit::Fn { info, .. } => Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
        generics: info.generics.clone(),
        where_bounds: info.where_bounds.clone(),
        arg_types: info.arg_types.clone(),
        return_type: info.return_type.clone(),
        fn_kind: SchemaKind::Fn,
        rest_type: info.rest_type.clone(),
        features: Arc::new(HashSet::new()),
      }))),
      _ => return None,
    }
  };
  let fn_annotation = signature.as_function()?;
  let expected_receiver = fn_annotation.arg_types.first()?;
  let mut bindings = HashMap::new();
  if !type_value.matches_with_bindings(expected_receiver.as_ref(), &mut bindings) {
    return None;
  }
  Some(
    fn_annotation
      .arg_types
      .iter()
      .skip(1)
      .map(|arg| arg.substitute_type_vars(&bindings))
      .collect(),
  )
}

fn warn_on_dynamic_trait_call(
  head: &Calcit,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  if file_ns == calcit::CORE_NS {
    return;
  }

  if !warn_dyn_method_enabled() {
    return;
  }

  let Calcit::Method(method_name, calcit::MethodKind::Invoke(_) | calcit::MethodKind::ExternalInvoke(_)) = head else {
    return;
  };

  let Some(receiver) = args.first() else {
    return;
  };

  let receiver_type = resolve_type_value(receiver, scope_types);
  let warn = match receiver_type.as_ref().map(|value| value.as_ref()) {
    None => true,
    Some(ann) if is_trait_annotation(ann) => false,
    Some(ann) => is_dynamic_annotation(ann),
  };

  if !warn {
    return;
  }

  let message = format!(
    "[Warn] dynamic trait call `.{method_name}` cannot be monomorphized in {file_ns}/{def_name}; add assert-traits, or use unsafe-coerce only at a trusted FFI boundary"
  );

  if let Some(loc) = head.get_location().or_else(|| receiver.get_location()) {
    gen_check_warning_with_location_code(message, "P_DYNAMIC_METHOD_DISPATCH", loc, check_warnings);
  } else {
    gen_check_warning_code(message, "P_DYNAMIC_METHOD_DISPATCH", file_ns, check_warnings);
  }
}

fn warn_on_nullable_js_ffi_dereference(
  head: &Calcit,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  if file_ns == calcit::CORE_NS {
    return;
  }

  let is_raw_dereference = matches!(
    head,
    Calcit::Method(_, calcit::MethodKind::Access | calcit::MethodKind::InvokeNative)
  ) || matches!(head, Calcit::Symbol { sym, .. } if matches!(sym.as_ref(), "aget" | "js-get"));
  if !is_raw_dereference {
    return;
  }

  let Some(receiver) = args.first() else {
    return;
  };
  let Some(receiver_type) = resolve_type_value(receiver, scope_types) else {
    return;
  };
  let CalcitTypeAnnotation::JsNullish(inner) = receiver_type.as_ref() else {
    return;
  };
  if !matches!(inner.as_ref(), CalcitTypeAnnotation::JsObject) {
    return;
  }

  let operation = match head {
    Calcit::Method(name, calcit::MethodKind::Access) => format!(".-{name}"),
    Calcit::Method(name, calcit::MethodKind::InvokeNative) => format!(".!{name}"),
    Calcit::Method(name, _) => format!(".{name}"),
    Calcit::Symbol { sym, .. } => sym.to_string(),
    _ => "JS FFI access".to_owned(),
  };
  let message = format!(
    "[Warn] JsNullish FFI value is dereferenced by `{operation}` in {file_ns}/{def_name}; use optional access, narrow with `js-present?`/`js-nullish?`, then validate or explicitly `unsafe-coerce` the opaque JsObject value"
  );

  if let Some(location) = head.get_location().or_else(|| receiver.get_location()) {
    gen_check_warning_with_location_code(message, "W_JS_FFI_NULLABLE_DEREF", location, check_warnings);
  } else {
    gen_check_warning_code(message, "W_JS_FFI_NULLABLE_DEREF", file_ns, check_warnings);
  }
}

fn static_js_field_name(form: &Calcit) -> Option<&str> {
  match form {
    Calcit::Tag(tag) => Some(tag.ref_str()),
    Calcit::Str(name) => Some(name.as_ref()),
    _ => None,
  }
}

fn rewrite_typed_js_field_operation(head: &Calcit, args: &CalcitList, scope_types: &ScopeTypes) -> Option<Calcit> {
  let operation = match head {
    Calcit::Symbol { sym, .. } if sym.as_ref() == "js-get" => "get",
    Calcit::Symbol { sym, .. } if sym.as_ref() == "js-set" => "set",
    _ => return None,
  };
  let receiver = args.first()?;
  let field_name = static_js_field_name(args.get(1)?)?;
  let receiver_type = resolve_type_value(receiver, scope_types)?;
  let traits = trait_list_from_type(receiver_type.as_ref())?;
  let (trait_def, _) = find_trait_field_type(&traits, field_name)?;
  if !trait_is_external_object(trait_def) {
    return None;
  }
  let kind = if operation == "get" {
    calcit::MethodKind::ExternalGet(receiver_type)
  } else {
    calcit::MethodKind::ExternalSet(receiver_type)
  };
  let mut rewritten = vec![Calcit::Method(Arc::from(field_name), kind), receiver.clone()];
  if operation == "set" {
    rewritten.push(args.get(2)?.clone());
  }
  Some(Calcit::from(rewritten))
}

fn check_typed_js_field_operation(
  head: &Calcit,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  let operation = match head {
    Calcit::Symbol { sym, .. } if sym.as_ref() == "js-get" => "js-get",
    Calcit::Symbol { sym, .. } if sym.as_ref() == "js-set" => "js-set",
    _ => return Ok(()),
  };
  let (Some(receiver), Some(key)) = (args.first(), args.get(1)) else {
    return Ok(());
  };
  let Some(field_name) = static_js_field_name(key) else {
    return Ok(());
  };
  let Some(receiver_type) = resolve_type_value(receiver, scope_types) else {
    return Ok(());
  };
  let Some(traits) = trait_list_from_type(receiver_type.as_ref()) else {
    return Ok(());
  };
  let external_traits = traits
    .iter()
    .filter(|trait_def| trait_is_external_object(trait_def.as_ref()))
    .cloned()
    .collect::<Vec<_>>();
  if external_traits.is_empty() {
    return Ok(());
  }
  let Some((field_trait, field_type)) = find_trait_field_type(&external_traits, field_name) else {
    let message = format!(
      "[Warn] `{operation}` cannot access undeclared external-object field `:{field_name}` in {file_ns}/{def_name}; declare the field on the external trait, use a dynamic key, or use raw `aget`/`aset`"
    );
    gen_check_warning_code_at(
      message,
      "W_JS_FFI_UNKNOWN_FIELD",
      file_ns,
      key.get_location().or_else(|| head.get_location()),
      check_warnings,
    );
    return Ok(());
  };
  let policy = program::active_feature_policy("js-ffi");
  if operation == "js-set"
    && !matches!(policy, crate::snapshot::FeaturePolicy::Allow)
    && !external_trait_field_is_writable(field_trait, field_name)
  {
    let message = format!(
      "[Warn] `js-set` cannot write read-only external-object field `:{field_name}` in {file_ns}/{def_name}; add `:writable $ #{{}} :{field_name}` to that trait's `:ffi` metadata or expose a mutating method instead; read `calcit docs read js-interop.md --full` for external-object contracts"
    );
    let location = key.get_location().or_else(|| head.get_location());
    if matches!(policy, crate::snapshot::FeaturePolicy::Error) {
      return Err(CalcitErr::use_msg_stack_location_with_code(
        CalcitErrKind::Type,
        message.replacen("[Warn]", "[Error]", 1),
        "E_JS_FFI_FIELD_READONLY",
        call_stack,
        location,
      ));
    }
    gen_check_warning_code_at(message, "W_JS_FFI_FIELD_READONLY", file_ns, location, check_warnings);
  }
  if operation == "js-set"
    && let Some(value) = args.get(2)
    && let Some(value_type) = resolve_type_value(value, scope_types)
    && !value_type.as_ref().matches_annotation(field_type.as_ref())
  {
    let message = format!(
      "[Warn] `js-set` field `:{field_name}` expects {}, got {} in {file_ns}/{def_name}",
      field_type.to_brief_string(),
      value_type.to_brief_string()
    );
    gen_check_warning_code_at(
      message,
      "W_JS_FFI_FIELD_TYPE_MISMATCH",
      file_ns,
      value.get_location().or_else(|| head.get_location()),
      check_warnings,
    );
  }
  Ok(())
}

/// Raw `.-`/`.!`/`aget`/`aset`/`js-get`/`js-set` on a bare `JsObject` receiver
/// (no external-object trait attached) has nothing left to check statically.
/// This is opt-in (behind `--warn-dyn-method`) since it fires on any untyped
/// FFI touch point, including code that intentionally stays dynamic.
fn warn_on_untyped_js_ffi_field_access(
  head: &Calcit,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  if file_ns == calcit::CORE_NS {
    return;
  }

  if !warn_dyn_method_enabled() {
    return;
  }

  let (operation, field_name) = match head {
    Calcit::Method(name, calcit::MethodKind::Access) => (format!(".-{name}"), name.to_string()),
    Calcit::Method(name, calcit::MethodKind::InvokeNative) => (format!(".!{name}"), name.to_string()),
    Calcit::Symbol { sym, .. } if matches!(sym.as_ref(), "aget" | "js-get" | "aset" | "js-set") => {
      let Some(field_name) = args.get(1).and_then(static_js_field_name) else {
        // A dynamic (non-literal) key cannot be described by a trait field
        // either, so there is no actionable next step to suggest here.
        return;
      };
      (sym.to_string(), field_name.to_owned())
    }
    _ => return,
  };

  let Some(receiver) = args.first() else {
    return;
  };
  let Some(receiver_type) = resolve_type_value(receiver, scope_types) else {
    return;
  };
  // Only the bare, non-nullable `JsObject` case is targeted here: it already
  // proves the value is a raw host object, and the key is a literal the
  // developer already knows, so declaring a trait is directly actionable.
  // Nullable receivers and declared traits are covered by other diagnostics.
  if !matches!(receiver_type.as_ref(), CalcitTypeAnnotation::JsObject) {
    return;
  }

  let message = format!(
    "[Warn] `{operation}` accesses untyped JS field `:{field_name}` in {file_ns}/{def_name}; still permitted, but declaring an external-object trait (`deftrait ... :ffi {{:kind :external-object ...}}`) for the receiver unlocks static field/method checking and `:names` mapping"
  );

  gen_check_warning_code_at(
    message,
    "W_JS_FFI_UNTYPED_ACCESS",
    file_ns,
    head.get_location().or_else(|| receiver.get_location()),
    check_warnings,
  );
}

fn warn_on_legacy_js_nullish_predicate(
  head: &Calcit,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  if file_ns == calcit::CORE_NS {
    return;
  }
  let Some(operation) = canonical_absence_operation_name(head) else {
    return;
  };
  if !matches!(operation, "nil?" | "some?") {
    return;
  }
  let Some(value) = args.first() else {
    return;
  };
  let Some(value_type) = resolve_type_value(value, scope_types) else {
    return;
  };
  if !matches!(value_type.as_ref(), CalcitTypeAnnotation::JsNullish(_)) {
    return;
  }

  let message = format!(
    "[Warn] `{operation}` consumes a JsNullish FFI value in {file_ns}/{def_name}; use `js-nullish?` or `js-present?` so host nullability stays explicit"
  );
  if let Some(location) = head.get_location().or_else(|| value.get_location()) {
    gen_check_warning_with_location_code(message, "W_JS_FFI_NULLABLE_PREDICATE", location, check_warnings);
  } else {
    gen_check_warning_code(message, "W_JS_FFI_NULLABLE_PREDICATE", file_ns, check_warnings);
  }
}

fn canonical_absence_operation_name(head: &Calcit) -> Option<&str> {
  match head {
    Calcit::Import(CalcitImport { ns, def, .. }) if ns.as_ref() == calcit::CORE_NS => Some(def.as_ref()),
    // Polymorphic membership calls are specialized before the final warning
    // pass. Keep their source-level meaning so the same typed-membership
    // exemption applies to both `(includes? set value)` and its native form.
    Calcit::Proc(CalcitProc::NativeListContains | CalcitProc::NativeMapContains) => Some("contains?"),
    Calcit::Proc(CalcitProc::NativeListIncludes | CalcitProc::NativeMapIncludes | CalcitProc::NativeSetIncludes) => Some("includes?"),
    Calcit::Proc(proc) => Some(proc.as_ref()),
    Calcit::Method(name, _) => Some(name.as_ref()),
    _ => None,
  }
}

fn nominal_enum_type_name(annotation: &CalcitTypeAnnotation) -> Option<String> {
  if let CalcitTypeAnnotation::TypeRef(name, _) = annotation {
    match name.as_ref() {
      "calcit.core/Option" => return Some("Option".to_owned()),
      "calcit.core/Result" => return Some("Result".to_owned()),
      _ => {}
    }
  }
  None
}

fn nominal_enum_expression_name(value: &Calcit, scope_types: &ScopeTypes) -> Option<String> {
  if let Some(value_type) = resolve_type_value(value, scope_types)
    && let Some(enum_name) = nominal_enum_type_name(value_type.as_ref())
  {
    return Some(enum_name);
  }

  let Calcit::List(items) = value else {
    return None;
  };
  let declared_enum_name = match items.first() {
    Some(Calcit::Import(CalcitImport { ns, def, .. })) => match program::lookup_def_schema(ns, def).as_ref() {
      CalcitTypeAnnotation::Fn(info) => nominal_enum_type_name(info.return_type.as_ref()),
      _ => None,
    },
    Some(Calcit::Fn { info, .. }) => nominal_enum_type_name(info.return_type.as_ref()),
    _ => None,
  };
  if let Some(enum_name) = declared_enum_name {
    return Some(enum_name);
  }
  match items.first().and_then(canonical_absence_operation_name) {
    Some(
      "%some" | "%none" | "find" | "find-index" | "index-of" | "first" | "last" | "nth" | "get" | "get-in" | "get-env" | "impl-origin"
      | "enum-definition",
    ) => Some("Option".to_owned()),
    Some("%ok" | "%err" | "parse-float") => Some("Result".to_owned()),
    _ => None,
  }
}

/// Return the nominal element type used by a membership operation, when the
/// collection has enough static information to prove it. `includes?` checks
/// map values while `contains?` checks map keys.
fn nominal_enum_membership_element_name(operation: &str, collection: &Calcit, scope_types: &ScopeTypes) -> Option<String> {
  if let Some(collection_type) = resolve_type_value(collection, scope_types) {
    let element_type = match (operation, collection_type.as_ref()) {
      ("includes?", CalcitTypeAnnotation::List(item) | CalcitTypeAnnotation::Set(item)) => item,
      ("includes?", CalcitTypeAnnotation::Map(_, value)) => value,
      ("contains?", CalcitTypeAnnotation::Set(item)) => item,
      ("contains?", CalcitTypeAnnotation::Map(key, _)) => key,
      _ => return None,
    };
    if let Some(enum_name) = nominal_enum_type_name(element_type.as_ref()) {
      return Some(enum_name);
    }
  }

  // The first warning pass runs while a literal's generic constructor can
  // still lack a synthesized return type. Its members have already been
  // preprocessed, though, so a homogeneous literal still proves membership
  // safe without relying on Dynamic inference.
  let Calcit::List(items) = collection else {
    return None;
  };
  let supports_element_membership = matches!(
    (operation, items.first()),
    ("includes?", Some(Calcit::Proc(CalcitProc::List | CalcitProc::Set))) | ("contains?", Some(Calcit::Proc(CalcitProc::Set)))
  );
  if !supports_element_membership {
    return None;
  }
  let mut elements = items.iter().skip(1);
  let element_name = nominal_enum_expression_name(elements.next()?, scope_types)?;
  elements
    .all(|item| nominal_enum_expression_name(item, scope_types).as_deref() == Some(element_name.as_str()))
    .then_some(element_name)
}

fn warn_on_nominal_enum_legacy_absence_use(
  head: &Calcit,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  if file_ns == calcit::CORE_NS {
    return;
  }

  let Some(operation) = canonical_absence_operation_name(head) else {
    return;
  };
  if !matches!(
    operation,
    "nil?"
      | "some?"
      | "list?"
      | "map?"
      | "set?"
      | "tag?"
      | "number?"
      | "string?"
      | "keyword?"
      | "symbol?"
      | "fn?"
      | "bool?"
      | "buffer?"
      | "cirru-quote?"
      | "ref?"
      | "macro?"
      | "syntax?"
      | "enum?"
      | "struct?"
      | "get"
      | "nth"
      | "first"
      | "last"
      | "count"
      | "empty?"
      | "contains?"
      | "includes?"
      | "assoc"
      | "assoc-in"
      | "dissoc"
      | "dissoc-in"
      | "merge"
      | "merge-non-nil"
      | "update"
      | "update-in"
      | "="
      | "&="
      | "&compare"
  ) {
    return;
  }

  // Membership is safe when both the collection element and candidate carry
  // the same nominal enum. In particular, Set<Option<T>> membership compares
  // Option values; it does not recover a nullable payload.
  if matches!(operation, "contains?" | "includes?")
    && args.len() == 2
    && let (Some(collection), Some(candidate)) = (args.first(), args.get(1))
    && let Some(candidate_enum) = nominal_enum_expression_name(candidate, scope_types)
    && nominal_enum_membership_element_name(operation, collection, scope_types).as_deref() == Some(candidate_enum.as_str())
  {
    return;
  }

  let nominal_args = args
    .iter()
    .filter_map(|value| {
      let enum_name = nominal_enum_expression_name(value, scope_types)?;
      Some((value, enum_name))
    })
    .collect::<Vec<_>>();
  let Some((value, enum_name)) = nominal_args.first() else {
    return;
  };

  // Equality between values of the same nominal enum is intentional and safe.
  // A nominal value compared with a payload or a Dynamic value, however, is a
  // common migration bug after a nullable API starts returning Option/Result.
  if matches!(operation, "=" | "&=") && nominal_args.len() == args.len() && nominal_args.iter().all(|(_, current)| current == enum_name)
  {
    return;
  }

  let guidance = match operation {
    "nil?" | "some?" if enum_name == "Option" => {
      "use `option:none?`/`option:some?` (or the corresponding methods) instead of nullable-value predicates".to_owned()
    }
    "nil?" | "some?" => "use `tag-match` to inspect the nominal enum variant".to_owned(),
    "=" | "&=" if enum_name == "Option" => {
      "compare Option values only with other Options, or unwrap/pattern-match before comparing a payload".to_owned()
    }
    "=" | "&=" => "compare values of the same nominal enum, or pattern-match before comparing a payload".to_owned(),
    "&compare" if enum_name == "Option" => "unwrap or pattern-match the Option before comparing its payload".to_owned(),
    "list?" | "map?" | "set?" | "struct?" | "enum?" | "struct-def?" | "enum-def?" | "tag?" | "number?" | "string?" | "keyword?"
    | "symbol?" | "fn?" | "bool?" | "buffer?" | "cirru-quote?" | "ref?" | "macro?" | "syntax?" => {
      "pattern-match the nominal enum before applying a payload type predicate".to_owned()
    }
    _ if enum_name == "Option" => {
      "use `if-let`/`match` for branches or an Option method such as `.unwrap-or` to access the payload".to_owned()
    }
    _ => "use `tag-match` instead of positional access on the nominal enum".to_owned(),
  };
  let message = format!(
    "[Warn] `{operation}` consumes nominal enum `{enum_name}` value `{value}` in {file_ns}/{def_name}; this often indicates a nullable-returning API migrated to a nominal type; {guidance}"
  );
  if let Some(location) = head.get_location().or_else(|| value.get_location()) {
    gen_check_warning_with_location_code(message, "W_NOMINAL_ENUM_LEGACY_USE", location, check_warnings);
  } else {
    gen_check_warning_code(message, "W_NOMINAL_ENUM_LEGACY_USE", file_ns, check_warnings);
  }
}

fn warn_on_nominal_enum_truthiness(
  value: &Calcit,
  scope_types: &ScopeTypes,
  file_ns: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  if file_ns == calcit::CORE_NS {
    return;
  }
  let Some(value_type) = resolve_type_value(value, scope_types) else {
    return;
  };
  let Some(enum_name) = nominal_enum_type_name(value_type.as_ref()) else {
    return;
  };

  let guidance = if enum_name == "Option" {
    "use `option:some?`, `option:none?`, or `tag-match`; `%none` is a truthy nominal value"
  } else {
    "use `tag-match` to select a nominal enum variant explicitly"
  };
  let message = format!(
    "[Warn] nominal enum `{enum_name}` is used directly as an `if` condition in {file_ns}; this can silently select the truthy branch; {guidance}"
  );
  if let Some(location) = value.get_location() {
    gen_check_warning_with_location_code(message, "W_NOMINAL_ENUM_LEGACY_USE", location, check_warnings);
  } else {
    gen_check_warning_code(message, "W_NOMINAL_ENUM_LEGACY_USE", file_ns, check_warnings);
  }
}

fn warn_on_trait_impl_method_tag_syntax(
  macro_info: &crate::calcit::CalcitMacro,
  args: &CalcitList,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  if file_ns == calcit::CORE_NS {
    return;
  }

  if macro_info.def_ns.as_ref() != calcit::CORE_NS {
    return;
  }

  // External-object traits intentionally use `:field` entries for typed
  // property access. Their CodeEntry metadata opts them into this shape.
  let trait_name = if macro_info.name.as_ref() == "deftrait" {
    args.first().and_then(parse_trait_name_from_source).map(|name| name.to_string())
  } else {
    None
  };
  let source_name = trait_name.as_deref().unwrap_or(def_name);

  if macro_name_is_external_object(macro_info, file_ns, source_name) {
    return;
  }

  let (macro_name, pair_start_idx) = match macro_info.name.as_ref() {
    "deftrait" => ("deftrait", 1),
    "defimpl" => ("defimpl", 2),
    _ => return,
  };

  for entry in args.iter().skip(pair_start_idx) {
    let Calcit::List(pair) = entry else {
      continue;
    };

    let Some(Calcit::Tag(method_name)) = pair.first() else {
      continue;
    };

    let message = format!(
      "[Warn] `{macro_name}` method key `:{method_name}` in {file_ns}/{source_name} uses legacy tag style; prefer dot method key `.{method_name}` for migration (`:{method_name}` remains compatible)"
    );

    if let Some(loc) = entry.get_location() {
      gen_check_warning_with_location(message, loc, check_warnings);
    } else {
      gen_check_warning(message, file_ns, check_warnings);
    }
  }
}

fn macro_name_is_external_object(macro_info: &crate::calcit::CalcitMacro, file_ns: &str, def_name: &str) -> bool {
  if macro_info.name.as_ref() != "deftrait" {
    return false;
  }
  let metadata_external = program::lookup_def_ffi(file_ns, def_name).is_some_and(|ffi| match ffi {
    cirru_edn::Edn::Struct(value) => value
      .pairs
      .iter()
      .find(|(key, _)| key.ref_str() == "kind")
      .is_some_and(|(_, value)| matches!(value, cirru_edn::Edn::Tag(tag) if tag.ref_str() == "external-object")),
    cirru_edn::Edn::Map(value) => value
      .get(&cirru_edn::Edn::Tag(EdnTag::new("kind")))
      .is_some_and(|value| matches!(value, cirru_edn::Edn::Tag(tag) if tag.ref_str() == "external-object")),
    _ => false,
  });
  metadata_external
    || program::lookup_def_code(file_ns, def_name)
      .and_then(|code| resolve_trait_def_from_source_code(&code))
      .is_some_and(|trait_def| trait_def.member_kinds.contains(&CalcitTraitMemberKind::Field))
}

fn extract_hint_fn_legacy_clause_name(form: &Calcit) -> Option<&str> {
  match form {
    Calcit::Symbol { sym, .. } => match sym.as_ref() {
      "return-type" => Some("return-type"),
      "generics" => Some("generics"),
      "type-vars" => Some("type-vars"),
      _ => None,
    },
    Calcit::Import(CalcitImport { def, .. }) => match def.as_ref() {
      "return-type" => Some("return-type"),
      "generics" => Some("generics"),
      "type-vars" => Some("type-vars"),
      _ => None,
    },
    _ => None,
  }
}

fn warn_on_method_name_conflict(
  head: &Calcit,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  if file_ns == calcit::CORE_NS {
    return;
  }

  if !warn_dyn_method_enabled() {
    return;
  }

  let Calcit::Method(method_name, calcit::MethodKind::Invoke(_) | calcit::MethodKind::ExternalInvoke(_)) = head else {
    return;
  };

  let Some(receiver) = args.first() else {
    return;
  };

  let Some(type_value) = resolve_type_value(receiver, scope_types) else {
    return;
  };

  let Some(impl_values) = get_impls_from_type(type_value.as_ref()) else {
    return;
  };

  if impl_values.len() < 2 {
    return;
  }

  let last_wins = core_impl_list_symbol_from_type_annotation(type_value.as_ref()).is_none();
  let matched_impls: Vec<&Arc<CalcitImpl>> = if last_wins {
    impl_values
      .iter()
      .rev()
      .filter(|imp| imp.get(method_name.as_ref()).is_some() && imp.origin().is_some())
      .collect()
  } else {
    impl_values
      .iter()
      .filter(|imp| imp.get(method_name.as_ref()).is_some() && imp.origin().is_some())
      .collect()
  };

  if matched_impls.len() < 2 {
    return;
  }

  let mut trait_names: Vec<String> = vec![];
  let mut seen = HashSet::new();
  for imp in &matched_impls {
    if let Some(origin) = imp.origin() {
      let trait_name = origin.name.to_string();
      if seen.insert(trait_name.clone()) {
        trait_names.push(trait_name);
      }
    }
  }

  if trait_names.len() < 2 {
    return;
  }

  let selected_trait = matched_impls
    .first()
    .and_then(|imp| imp.origin())
    .map(|origin| origin.name.to_string())
    .unwrap_or_else(|| "<unknown>".to_string());

  let message = format!(
    "[Warn] method `.{}` has multiple trait candidates ({}) in {}/{}; current dispatch picks `{}` by precedence, use `&trait-call` to disambiguate",
    method_name,
    trait_names.join(", "),
    file_ns,
    def_name,
    selected_trait,
  );

  if let Some(loc) = head.get_location().or_else(|| receiver.get_location()) {
    gen_check_warning_with_location(message, loc, check_warnings);
  } else {
    gen_check_warning(message, file_ns, check_warnings);
  }
}

/// Try to specialize a polymorphic core function call at compile time.
/// When the receiver's type is statically known, replaces the generic call
/// (e.g. `(count x)`) with a direct proc call (e.g. `(&list:count x)`),
/// eliminating runtime type-dispatch chains.
///
/// Two flavours of target:
///   * A built-in `CalcitProc` (the majority — `count`, `empty?`, `first`,
///     ...). The resulting head is a `Calcit::Proc`.
///   * A Calcit-level user definition in `calcit.core` (e.g. `&list:map`,
///     `&map:filter`, `&set:filter`). These are used for higher-order
///     collection ops that are written in Calcit itself. The resulting head
///     is a `Calcit::Import` pointing at the core def.
fn try_specialize_polymorphic_call(
  fn_ns: &str,
  fn_def: &str,
  processed_args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
) -> Option<Calcit> {
  use CalcitProc::*;
  use CalcitTypeAnnotation as T;

  if fn_ns != calcit::CORE_NS {
    return None;
  }

  // Refuse to specialize calls *inside* the polymorphic dispatcher definitions
  // themselves (avoids cycles if someone adds `(map ...)` inside `&list:map`).
  if matches!(
    fn_def,
    "&list:map" | "&map:map" | "&set:map" | "&list:filter" | "&map:filter" | "&set:filter"
  ) {
    return None;
  }

  // Get receiver (first argument) and its type
  let receiver = processed_args.first()?;
  let receiver_type = resolve_type_value(receiver, scope_types)?;

  // --- Type predicate folding: when the receiver's static type is known, we
  // can fold `(list? x)` / `(map? x)` / ... to a literal Bool. This removes
  // a user-def call + a `type-of` proc call from every such check.
  //
  // We only fold the positive case (type is definitely the queried kind) to
  // keep the matrix small; negative folding would need to enumerate every
  // other variant of `CalcitTypeAnnotation`, which is fragile as the enum
  // grows. Leaving dynamic / unknown / mismatched cases to runtime is safe.
  let predicate_true = matches!(
    (fn_def, receiver_type.as_ref()),
    ("list?", T::List(_))
      | ("map?", T::Map(_, _))
      | ("set?", T::Set(_))
      | ("string?", T::String)
      | ("number?", T::Number)
      | ("bool?", T::Bool)
      | ("tag?", T::Tag)
      | ("fn?", T::Fn(_) | T::DynFn)
      | ("enum?", T::EnumValue(_) | T::AnonymousEnum | T::Enum(_, _))
      | ("struct?", T::StructValue(_) | T::Struct(_, _))
      | ("struct-def?", T::StructDef(_))
      | ("enum-def?", T::EnumDef(_))
  );
  if predicate_true {
    return Some(Calcit::Bool(true));
  }

  // --- Calcit-level (user def) specializations: higher-order collection ops ---
  let core_def_name: Option<&'static str> = match (fn_def, receiver_type.as_ref()) {
    ("map", T::List(_)) => Some("&list:map"),
    ("map", T::Map(_, _)) => Some("&map:map"),
    ("filter", T::List(_)) => Some("&list:filter"),
    ("filter", T::Map(_, _)) => Some("&map:filter"),
    ("filter", T::Set(_)) => Some("&set:filter"),
    _ => None,
  };
  if let Some(def_name) = core_def_name {
    let head = Calcit::Import(CalcitImport {
      ns: calcit::CORE_NS.into(),
      def: def_name.into(),
      info: Arc::new(ImportInfo::Core { at_ns: Arc::from(file_ns) }),
      def_id: Some(program::ensure_def_id(calcit::CORE_NS, def_name).0),
    });
    let mut items: Vec<Calcit> = Vec::with_capacity(processed_args.len() + 1);
    items.push(head);
    for arg in processed_args.iter() {
      items.push(arg.to_owned());
    }
    return Some(Calcit::from(items));
  }

  let proc = match (fn_def, receiver_type.as_ref()) {
    // count
    ("count", T::List(_)) => NativeListCount,
    ("count", T::Map(_, _)) => NativeMapCount,
    ("count", T::Set(_)) => NativeSetCount,
    ("count", T::String) => NativeStrCount,
    ("count", T::EnumValue(_) | T::AnonymousEnum) => NativeEnumCount,
    ("count", T::StructValue(_)) => NativeStructCount,
    // empty?
    ("empty?", T::List(_)) => NativeListEmpty,
    ("empty?", T::Map(_, _)) => NativeMapEmpty,
    ("empty?", T::Set(_)) => NativeSetEmpty,
    ("empty?", T::String) => NativeStrEmpty,
    // contains?
    ("contains?", T::List(_)) => NativeListContains,
    ("contains?", T::Map(_, _)) => NativeMapContains,
    ("contains?", T::Set(_)) => NativeSetIncludes,
    ("contains?", T::String) => NativeStrContains,
    ("contains?", T::StructValue(_)) => NativeStructContains,
    // rest
    ("rest", T::List(_)) => NativeListRest,
    // assoc
    ("assoc", T::List(_)) => NativeListAssoc,
    ("assoc", T::Map(_, _)) => NativeMapAssoc,
    ("assoc", T::EnumValue(_) | T::AnonymousEnum) => NativeEnumAssoc,
    ("assoc", T::StructValue(_)) => NativeStructAssoc,
    // includes?
    ("includes?", T::List(_)) => NativeListIncludes,
    ("includes?", T::Map(_, _)) => NativeMapIncludes,
    ("includes?", T::Set(_)) => NativeSetIncludes,
    ("includes?", T::String) => NativeStrIncludes,
    // reverse (only list has a native proc)
    ("reverse", T::List(_)) => NativeListReverse,
    _ => return None,
  };

  // Build specialized call: (proc arg1 arg2 ...)
  let mut items: Vec<Calcit> = Vec::with_capacity(processed_args.len() + 1);
  items.push(Calcit::Proc(proc));
  for arg in processed_args.iter() {
    items.push(arg.to_owned());
  }
  Some(Calcit::from(items))
}

fn try_inline_method_call(head: &Calcit, args: &CalcitList, scope_types: &ScopeTypes, file_ns: &str) -> Option<Calcit> {
  match head {
    Calcit::Method(method_name, calcit::MethodKind::Invoke(type_value)) => {
      let mut resolved_type = type_value.clone();
      if matches!(**type_value, CalcitTypeAnnotation::Dynamic)
        && let Some(receiver) = args.first()
        && let Some(inferred) = resolve_type_value(receiver, scope_types)
        && !matches!(inferred.as_ref(), CalcitTypeAnnotation::Dynamic)
      {
        resolved_type = inferred;
      }
      if matches!(resolved_type.as_ref(), CalcitTypeAnnotation::Dynamic) {
        return None;
      }
      let type_ref = resolved_type.as_ref();
      let impl_values = get_impls_from_type(type_ref)?;
      let (_impl_index, _impl_value, method_entry) = find_method_entry_with_impl(type_ref, &impl_values, method_name.as_ref())?;

      if let Some(callable_head) = pick_callable_from_method_entry(method_entry, file_ns) {
        return Some(build_inlined_call(callable_head, args, scope_types));
      }

      None
    }
    _ => None,
  }
}

fn pick_callable_from_method_entry(entry: &Calcit, _file_ns: &str) -> Option<Calcit> {
  match entry {
    // Avoid inlining Fn literals: JS codegen would embed large function bodies and lose closure semantics.
    Calcit::Import(..) | Calcit::Proc(..) | Calcit::Registered(..) | Calcit::Symbol { .. } => Some(entry.to_owned()),
    Calcit::Fn { info, .. }
      if info
        .def_ref
        .as_ref()
        .is_some_and(|def_ref| !def_ref.is_macro_gen && program::has_def_code(def_ref.def_ns.as_ref(), def_ref.def_name.as_ref())) =>
    {
      Some(entry.to_owned())
    }
    _ => None,
  }
}

fn build_inlined_call(callable_head: Calcit, args: &CalcitList, scope_types: &ScopeTypes) -> Calcit {
  let mut call_nodes: Vec<Calcit> = Vec::with_capacity(args.len() + 1);
  call_nodes.push(callable_head);
  for item in args.iter() {
    call_nodes.push(item.to_owned());
  }
  let kind = classify_number_binary_call(&call_nodes[0], &call_nodes[1..], scope_types);
  Calcit::from(CalcitList::executable(call_nodes, kind))
}

fn find_method_entry_with_impl<'a>(
  type_ref: &CalcitTypeAnnotation,
  impls: &'a [Arc<CalcitImpl>],
  name: &str,
) -> Option<(usize, &'a Arc<CalcitImpl>, &'a Calcit)> {
  let last_wins = core_impl_list_symbol_from_type_annotation(type_ref).is_none();
  if last_wins {
    for (idx, imp) in impls.iter().enumerate().rev() {
      if let Some(entry) = imp.get(name) {
        return Some((idx, imp, entry));
      }
    }
  } else {
    for (idx, imp) in impls.iter().enumerate() {
      if let Some(entry) = imp.get(name) {
        return Some((idx, imp, entry));
      }
    }
  }
  None
}

fn append_string_method_receiver_hint(mut message: String, method_name: &str, type_desc: &str) -> String {
  let replacement = match method_name {
    "trim" => "trim",
    "blank?" => "blank?",
    _ => return message,
  };
  message.push_str(&format!(
    ". String API hint: `.{method_name}` requires a String receiver, but this receiver was inferred as `{type_desc}`; fix or narrow the receiver type, or use `({replacement} receiver)` for direct argument-type diagnostics"
  ));
  message
}

fn validate_method_call(
  head: &Calcit,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  // Only validate Method(Invoke) calls
  let Calcit::Method(method_name, calcit::MethodKind::Invoke(inferred_receiver_type)) = head else {
    return Ok(());
  };

  // Need receiver to validate
  let Some(receiver) = args.first() else {
    return Ok(());
  };

  // Trait declarations encode a member signature as `deftrait T .show :fn`.
  // The temporary Dynamic method receiver and the trailing tag are syntax, not
  // a value-level method call, so the trait declaration owns its validation.
  if matches!(inferred_receiver_type.as_ref(), CalcitTypeAnnotation::Dynamic) && matches!(receiver, Calcit::Tag(_)) {
    return Ok(());
  }

  // Get receiver type
  // Postfix method rewriting records the receiver type on the method itself.
  // Literals do not always carry a separately resolvable type in `ScopeTypes`,
  // so prefer that evidence and only fall back to resolving the expression.
  let type_value = resolve_type_value(receiver, scope_types).unwrap_or_else(|| inferred_receiver_type.clone());

  if let Some(traits) = trait_list_from_type(type_value.as_ref()) {
    let method_str = method_name.as_ref();
    if traits.iter().rev().any(|trait_def| {
      trait_def
        .methods
        .iter()
        .zip(trait_def.member_kinds.iter())
        .any(|(method, kind)| *kind == CalcitTraitMemberKind::Method && method.ref_str() == method_str)
    }) {
      return Ok(());
    }

    let methods_list = collect_trait_method_names(&traits).join(" ");
    let type_desc = describe_type(type_value.as_ref());
    return Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Type,
      append_string_method_receiver_hint(
        format!("unknown method `.{method_name}` for {type_desc}. Available methods: {methods_list}"),
        method_name,
        &type_desc,
      ),
      call_stack,
      head.get_location(),
    ));
  }

  // `Show` is opt-in even while the core implementation tables are still
  // bootstrapping. Without this explicit branch, a primitive receiver can
  // have no resolved impl table yet and method validation would skip it.
  if method_name.as_ref() == "show"
    && static_method_descriptors(type_value.as_ref()).is_some_and(|methods| methods.iter().all(|method| method.name != ".show"))
  {
    let type_desc = describe_type(type_value.as_ref());
    return Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Type,
      format!(
        "unknown method `.show` for {type_desc}. Show is opt-in; attach an explicit `defimpl ... calcit.core/Show` implementation, or use `.debug` for the built-in diagnostic representation"
      ),
      call_stack,
      head.get_location(),
    ));
  }

  // Get impl structs for the type
  let Some(impl_values) = get_impls_from_type(&type_value) else {
    return Ok(()); // No impl struct, skip validation
  };

  // Check if method exists in the impls
  let method_str = method_name.as_ref();
  if impl_values
    .iter()
    .any(|struct_def| struct_def.fields().iter().any(|field| field.ref_str() == method_str))
  {
    return Ok(()); // Method found, validation passed
  }

  // Method not found, generate error
  let mut methods = vec![];
  for struct_def in impl_values.iter() {
    for field in struct_def.fields().iter() {
      methods.push(field.to_string());
    }
  }
  let methods_list = methods.join(" ");
  let type_desc = describe_type(type_value.as_ref());
  Err(CalcitErr::use_msg_stack_location(
    CalcitErrKind::Type,
    append_string_method_receiver_hint(
      format!("unknown method `.{method_name}` for {type_desc}. Available methods: {methods_list}"),
      method_name,
      &type_desc,
    ),
    call_stack,
    head.get_location(),
  ))
}

/// Check if a type annotation represents a callable type (function or method)
fn is_callable_type(type_ann: &CalcitTypeAnnotation) -> bool {
  match type_ann {
    CalcitTypeAnnotation::Fn(_) => true,
    CalcitTypeAnnotation::DynFn => true,
    CalcitTypeAnnotation::Optional(inner) => is_callable_type(inner.as_ref()),
    CalcitTypeAnnotation::Dynamic => true,
    _ => false,
  }
}

/// Check if an expression's inferred type is callable, and warn if not
fn check_callable_type(
  expr: &Calcit,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  // Skip check for expressions that are obviously callable at runtime
  match expr {
    // These are always callable
    Calcit::Fn { .. }
    | Calcit::Proc(..)
    | Calcit::Import { .. }
    | Calcit::Registered { .. }
    | Calcit::Method(_, _)
    | Calcit::RawCode(..)
    | Calcit::Symbol { .. } => (),

    // For List expressions, check if it's a function call that returns a callable
    Calcit::List(_) => {
      if let Some(type_ann) = infer_type_from_expr(expr, scope_types)
        && !is_callable_type(&type_ann)
      {
        let type_desc = describe_type(&type_ann);
        gen_check_warning(
          format!("[Warn] trying to call a non-function value of type {type_desc}. Expression: `{expr}`, at {file_ns}/{def_name}"),
          file_ns,
          check_warnings,
        );
      }
    }

    // For Local variables, check their type info
    Calcit::Local(local) => {
      let type_ann = if matches!(*local.type_info, CalcitTypeAnnotation::Dynamic) {
        scope_types.get(&local.sym).map(|t| t.as_ref()).unwrap_or(&*local.type_info)
      } else {
        &*local.type_info
      };
      if !is_callable_type(type_ann) {
        let type_desc = describe_type(type_ann);
        gen_check_warning(
          format!(
            "[Warn] trying to call variable `{}` of non-function type {type_desc}, at {file_ns}/{def_name}",
            local.sym
          ),
          file_ns,
          check_warnings,
        );
      }
    }

    // Other types are definitely not callable
    _ => {
      if let Some(type_ann) = infer_type_from_expr(expr, scope_types)
        && !is_callable_type(&type_ann)
      {
        let type_desc = describe_type(&type_ann);
        gen_check_warning(
          format!("[Warn] trying to call a non-function value of type {type_desc}. Expression: `{expr}`, at {file_ns}/{def_name}"),
          file_ns,
          check_warnings,
        );
      }
    }
  }
}

fn macro_syntax_contract_label(contract: &MacroSyntaxType) -> String {
  match contract {
    MacroSyntaxType::Syntax => "Syntax".to_owned(),
    MacroSyntaxType::SyntaxSymbol => "SyntaxSymbol".to_owned(),
    MacroSyntaxType::SyntaxList => "SyntaxList".to_owned(),
    MacroSyntaxType::Expr(semantic) => format!("Expr<{}>", semantic.to_brief_string()),
  }
}

fn macro_input_contract(signature: &MacroSignature, idx: usize) -> Option<&MacroSyntaxType> {
  if idx < signature.required_inputs.len() {
    signature.required_inputs.get(idx)
  } else if idx < signature.required_inputs.len() + signature.optional_inputs.len() {
    signature.optional_inputs.get(idx - signature.required_inputs.len())
  } else {
    signature.rest_input.as_ref()
  }
}

fn validate_macro_call_inputs(
  macro_name: &str,
  signature: &MacroSignature,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  call_stack: &CallStackList,
  call_location: Option<NodeLocation>,
) -> Result<HashMap<Arc<str>, Arc<CalcitTypeAnnotation>>, CalcitErr> {
  let mut bindings = HashMap::new();
  let min = signature.required_inputs.len();
  let max = signature.required_inputs.len() + signature.optional_inputs.len();
  if args.len() < min || (signature.rest_input.is_none() && args.len() > max) {
    let expected = if signature.rest_input.is_some() {
      format!("at least {min}")
    } else if min == max {
      min.to_string()
    } else {
      format!("{min}..={max}")
    };
    return Err(CalcitErr::use_msg_stack_location_with_code(
      CalcitErrKind::Arity,
      format!(
        "macro input-syntax violation in `{macro_name}`: expected {expected} argument(s), got {}",
        args.len()
      ),
      "E_MACRO_INPUT_ARITY",
      call_stack,
      call_location,
    ));
  }
  for (idx, arg) in args.iter().enumerate() {
    let Some(contract) = macro_input_contract(signature, idx) else {
      continue;
    };
    let shape_matches = match contract {
      MacroSyntaxType::Syntax | MacroSyntaxType::Expr(_) => true,
      MacroSyntaxType::SyntaxSymbol => matches!(arg, Calcit::Symbol { .. }),
      MacroSyntaxType::SyntaxList => matches!(arg, Calcit::List(_)),
    };
    if !shape_matches {
      return Err(CalcitErr::use_msg_stack_location_with_code(
        CalcitErrKind::Type,
        format!(
          "macro input-syntax violation in `{macro_name}` at argument {}: expected {}, got {}",
          idx + 1,
          macro_syntax_contract_label(contract),
          brief_type_of_value(arg)
        ),
        "E_MACRO_INPUT_SYNTAX",
        call_stack,
        arg.get_location().or_else(|| call_location.clone()),
      ));
    }
    if let MacroSyntaxType::Expr(expected) = contract
      && let Some(actual) = infer_type_from_expr(arg, scope_types)
      && !matches!(actual.as_ref(), CalcitTypeAnnotation::Dynamic)
      && !actual.as_ref().matches_with_bindings(expected.as_ref(), &mut bindings)
    {
      return Err(CalcitErr::use_msg_stack_location_with_code(
        CalcitErrKind::Type,
        format!(
          "macro input-syntax violation in `{macro_name}` at argument {}: expression expects semantic type `{}`, got `{}`",
          idx + 1,
          expected.to_brief_string(),
          actual.to_brief_string()
        ),
        "E_MACRO_INPUT_EXPR_TYPE",
        call_stack,
        arg.get_location().or_else(|| call_location.clone()),
      ));
    }
  }
  Ok(bindings)
}

fn is_definition_syntax(code: &Calcit) -> bool {
  let Calcit::List(xs) = code else { return false };
  match xs.first() {
    Some(Calcit::Syntax(head, _)) => matches!(
      head,
      CalcitSyntax::Defn | CalcitSyntax::Defmacro | CalcitSyntax::DefWasmExport | CalcitSyntax::DefWasmImport
    ),
    Some(Calcit::Symbol { sym, .. }) => matches!(
      sym.as_ref(),
      "defn" | "defmacro" | "defstruct" | "defenum" | "deftrait" | "defimpl" | "defwasm-export" | "defwasm-import"
    ),
    _ => false,
  }
}

fn validate_macro_expansion_result(
  macro_name: &str,
  signature: &MacroSignature,
  expansion: (&Calcit, &Calcit),
  scope_types: &ScopeTypes,
  mut bindings: HashMap<Arc<str>, Arc<CalcitTypeAnnotation>>,
  call_stack: &CallStackList,
  call_location: Option<NodeLocation>,
) -> Result<(), CalcitErr> {
  let (raw_expansion, processed) = expansion;
  match &signature.expansion {
    MacroExpansionType::Dynamic => Ok(()),
    MacroExpansionType::Declarations => {
      let valid = is_definition_syntax(raw_expansion)
        || matches!(raw_expansion, Calcit::List(xs) if xs.first().is_some_and(|head| matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "do")) && xs.iter().skip(1).all(is_definition_syntax));
      if valid {
        Ok(())
      } else {
        Err(CalcitErr::use_msg_stack_location_with_code(
          CalcitErrKind::Type,
          format!("macro expansion-result violation in `{macro_name}`: expected Declarations, got `{raw_expansion}`"),
          "E_MACRO_EXPANSION_DECLARATIONS",
          call_stack,
          raw_expansion.get_location().or(call_location),
        ))
      }
    }
    MacroExpansionType::Definition(expected) => {
      if !is_definition_syntax(raw_expansion) {
        return Err(CalcitErr::use_msg_stack_location_with_code(
          CalcitErrKind::Type,
          format!(
            "macro expansion-result violation in `{macro_name}`: expected Definition<{}>",
            expected.to_brief_string()
          ),
          "E_MACRO_EXPANSION_DEFINITION",
          call_stack,
          raw_expansion.get_location().or(call_location),
        ));
      }
      if let Some(actual) = resolve_type_value(processed, scope_types)
        && !matches!(actual.as_ref(), CalcitTypeAnnotation::Dynamic)
        && !actual.as_ref().matches_with_bindings(expected.as_ref(), &mut bindings)
      {
        return Err(CalcitErr::use_msg_stack_location_with_code(
          CalcitErrKind::Type,
          format!(
            "macro expansion-result violation in `{macro_name}`: expected definition type `{}`, got `{}`",
            expected.to_brief_string(),
            actual.to_brief_string()
          ),
          "E_MACRO_EXPANSION_DEFINITION_TYPE",
          call_stack,
          raw_expansion.get_location().or(call_location),
        ));
      }
      Ok(())
    }
    MacroExpansionType::Expr(expected) => {
      let Some(actual) = resolve_type_value(processed, scope_types) else {
        return Ok(());
      };
      if matches!(actual.as_ref(), CalcitTypeAnnotation::Dynamic)
        || actual.as_ref().matches_with_bindings(expected.as_ref(), &mut bindings)
      {
        Ok(())
      } else {
        Err(CalcitErr::use_msg_stack_location_with_code(
          CalcitErrKind::Type,
          format!(
            "macro expansion-result violation in `{macro_name}`: expected Expr<{}>, got `{}`",
            expected.to_brief_string(),
            actual.to_brief_string()
          ),
          "E_MACRO_EXPANSION_EXPR_TYPE",
          call_stack,
          raw_expansion.get_location().or(call_location),
        ))
      }
    }
  }
}
/// Get the impl records from a type value
/// - If type_value is already a Struct, use it directly
/// - If type_value is a Tag, map to corresponding core impl list
/// - Otherwise return None
fn collect_impls_from_value(value: &Calcit) -> Option<Vec<Arc<CalcitImpl>>> {
  let resolve_impl = |value: &Calcit| -> Option<CalcitImpl> {
    match value {
      Calcit::Impl(imp) => Some(imp.to_owned()),
      Calcit::Import(import) => match resolve_program_value_for_preprocess(&import.ns, &import.def, import.def_id) {
        Some(Calcit::Impl(imp)) => Some(imp),
        _ => None,
      },
      Calcit::Symbol { sym, info, .. } => match resolve_program_value_for_preprocess(&info.at_ns, sym, None) {
        Some(Calcit::Impl(imp)) => Some(imp),
        _ => None,
      },
      _ => None,
    }
  };

  match value {
    Calcit::Impl(_) | Calcit::Import(_) | Calcit::Symbol { .. } => resolve_impl(value).map(|imp| vec![Arc::new(imp)]),
    Calcit::List(list) => {
      let mut impls: Vec<Arc<CalcitImpl>> = Vec::with_capacity(list.len());
      for item in list.iter() {
        let imp = resolve_impl(item)?;
        impls.push(Arc::new(imp));
      }
      Some(impls)
    }
    _ => None,
  }
}

fn get_impls_from_type(type_value: &CalcitTypeAnnotation) -> Option<Vec<Arc<CalcitImpl>>> {
  if let Some(struct_def) = type_value.resolve_to_struct() {
    // Prepend core struct impls; user impls come after and win (last_wins=true)
    let mut impls = resolve_core_impls("&core-struct-impls").unwrap_or_default();
    impls.extend(struct_def.impls.iter().cloned());
    return Some(impls);
  }

  if let CalcitTypeAnnotation::Struct(struct_def, _) = type_value {
    return Some(struct_def.impls.to_owned());
  }

  if let Some(enum_def) = type_value.resolve_to_enum() {
    // Prepend core enum impls; user impls come after and win (last_wins=true)
    let mut impls = resolve_core_impls("&core-enum-impls").unwrap_or_default();
    impls.extend(enum_def.impls.iter().cloned());
    return Some(impls);
  }

  if let CalcitTypeAnnotation::AnonymousEnum = type_value {
    // Untyped enum: only core impls
    if let Some(core_impls) = resolve_core_impls("&core-enum-impls") {
      return Some(core_impls);
    }
  }

  if let Some(class_symbol) = core_impl_list_symbol_from_type_annotation(type_value) {
    return match resolve_program_value_for_preprocess(calcit::CORE_NS, class_symbol, None) {
      Some(value) => collect_impls_from_value(&value),
      None => None,
    };
  }

  if let CalcitTypeAnnotation::Custom(value) = type_value {
    match value.as_ref() {
      Calcit::Import(import) => {
        return match resolve_program_value_for_preprocess(&import.ns, &import.def, import.def_id) {
          Some(value) => collect_impls_from_value(&value),
          None => None,
        };
      }
      Calcit::Symbol { sym, info, .. } => {
        let (target_ns, target_def) = match runner::parse_ns_def(sym) {
          Some((ns_part, def_part)) => (ns_part, def_part),
          None => (info.at_ns.to_owned(), sym.to_owned()),
        };
        return match resolve_program_value_for_preprocess(&target_ns, &target_def, None) {
          Some(value) => collect_impls_from_value(&value),
          None => None,
        };
      }
      _ => {}
    }
  }

  None
}

/// Resolve core impl records from a symbol name in calcit.core
fn resolve_core_impls(symbol: &str) -> Option<Vec<Arc<CalcitImpl>>> {
  resolve_program_value_for_preprocess(calcit::CORE_NS, symbol, None).and_then(|v| collect_impls_from_value(&v))
}

fn trait_list_from_type(type_value: &CalcitTypeAnnotation) -> Option<Vec<Arc<CalcitTrait>>> {
  match type_value {
    CalcitTypeAnnotation::Trait(trait_def) => Some(vec![trait_def.to_owned()]),
    CalcitTypeAnnotation::TraitSet(traits) => Some(traits.as_ref().to_owned()),
    CalcitTypeAnnotation::Optional(inner) => trait_list_from_type(inner.as_ref()),
    _ => None,
  }
}

pub(crate) fn trait_is_external_object(trait_def: &CalcitTrait) -> bool {
  let Some(def_ref) = trait_def.definition_ref.as_deref() else {
    return false;
  };
  let Some((ns, def)) = def_ref.rsplit_once('/') else { return false };
  let Some(ffi) = program::lookup_def_ffi(ns, def) else {
    return false;
  };
  match ffi {
    cirru_edn::Edn::Struct(value) => value
      .pairs
      .iter()
      .find(|(key, _)| key.ref_str() == "kind")
      .is_some_and(|(_, value)| matches!(value, cirru_edn::Edn::Tag(tag) if tag.ref_str() == "external-object")),
    cirru_edn::Edn::Map(value) => value
      .get(&cirru_edn::Edn::Tag(EdnTag::new("kind")))
      .is_some_and(|value| matches!(value, cirru_edn::Edn::Tag(tag) if tag.ref_str() == "external-object")),
    _ => false,
  }
}

fn external_trait_field_is_writable(trait_def: &CalcitTrait, field_name: &str) -> bool {
  let Some(def_ref) = trait_def.definition_ref.as_deref() else {
    return false;
  };
  let Some((ns, def)) = def_ref.rsplit_once('/') else {
    return false;
  };
  let Some(ffi) = program::lookup_def_ffi(ns, def) else {
    return false;
  };
  let Some(cirru_edn::Edn::Set(values)) = ffi_metadata_value(&ffi, "writable") else {
    return false;
  };
  values.0.iter().any(|value| match value {
    cirru_edn::Edn::Tag(tag) => tag.ref_str() == field_name,
    cirru_edn::Edn::Str(name) | cirru_edn::Edn::Symbol(name) => name.as_ref().trim_start_matches(':') == field_name,
    _ => false,
  })
}

fn is_trait_annotation(type_value: &CalcitTypeAnnotation) -> bool {
  matches!(type_value, CalcitTypeAnnotation::Trait(_) | CalcitTypeAnnotation::TraitSet(_))
    || matches!(type_value, CalcitTypeAnnotation::Optional(inner) if is_trait_annotation(inner.as_ref()))
}

fn is_dynamic_annotation(type_value: &CalcitTypeAnnotation) -> bool {
  matches!(type_value, CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::DynFn)
    || matches!(type_value, CalcitTypeAnnotation::Optional(inner) if is_dynamic_annotation(inner.as_ref()))
}

fn dynamic_nominal_method_requirement(method_name: &str) -> Option<(&'static str, &'static str)> {
  match method_name {
    "some?" | "none?" | "unwrap" | "fold" => Some(("Option", "the matching `option:*` function")),
    "ok?" | "err?" | "map-err" => Some(("Result", "the matching `result:*` function")),
    "unwrap-or" | "and-then" | "or-else" => Some(("Option or Result", "the matching `option:*` or `result:*` function")),
    _ => None,
  }
}

/// Rank annotations by how much static information they erase. Root-level
/// dynamic callable/value types are weaker than an equally dynamic container,
/// because the latter still preserves useful shape information.
fn annotation_dynamic_weight(type_value: &CalcitTypeAnnotation) -> usize {
  match type_value {
    CalcitTypeAnnotation::Dynamic => 200,
    CalcitTypeAnnotation::DynFn => 100,
    CalcitTypeAnnotation::List(inner)
    | CalcitTypeAnnotation::Set(inner)
    | CalcitTypeAnnotation::Ref(inner)
    | CalcitTypeAnnotation::Optional(inner)
    | CalcitTypeAnnotation::Variadic(inner) => annotation_dynamic_weight(inner),
    CalcitTypeAnnotation::Map(key, value) => annotation_dynamic_weight(key) + annotation_dynamic_weight(value),
    CalcitTypeAnnotation::Fn(signature) => {
      signature.arg_types.iter().map(|arg| annotation_dynamic_weight(arg)).sum::<usize>()
        + signature.rest_type.as_ref().map_or(0, |rest| annotation_dynamic_weight(rest))
        + annotation_dynamic_weight(signature.return_type.as_ref())
    }
    CalcitTypeAnnotation::Struct(_, args) | CalcitTypeAnnotation::Enum(_, args) | CalcitTypeAnnotation::TypeRef(_, args) => {
      args.iter().map(|arg| annotation_dynamic_weight(arg)).sum()
    }
    _ => 0,
  }
}

fn find_trait_method_type<'a>(
  traits: &'a [Arc<CalcitTrait>],
  method_name: &str,
) -> Option<(&'a CalcitTrait, &'a Arc<CalcitTypeAnnotation>)> {
  for trait_def in traits.iter().rev() {
    if let Some(method_idx) = trait_def.method_index(method_name)
      && let Some(method_type) = trait_def.method_types.get(method_idx)
    {
      return Some((trait_def.as_ref(), method_type));
    }
  }
  None
}

pub(crate) fn find_trait_field_type<'a>(
  traits: &'a [Arc<CalcitTrait>],
  field_name: &str,
) -> Option<(&'a CalcitTrait, &'a Arc<CalcitTypeAnnotation>)> {
  for trait_def in traits.iter().rev() {
    if let Some(field_idx) = trait_def.field_index(field_name)
      && let Some(field_type) = trait_def.method_types.get(field_idx)
    {
      return Some((trait_def.as_ref(), field_type));
    }
  }
  None
}

fn collect_trait_method_names(traits: &[Arc<CalcitTrait>]) -> Vec<String> {
  let mut seen = std::collections::HashSet::new();
  let mut names = vec![];
  for trait_def in traits.iter().rev() {
    for (method, kind) in trait_def.methods.iter().zip(trait_def.member_kinds.iter()) {
      if *kind != CalcitTraitMemberKind::Method {
        continue;
      }
      let name = method.to_string();
      if seen.insert(name.clone()) {
        names.push(name);
      }
    }
  }
  names
}

fn core_impl_list_symbol_from_type_annotation(type_value: &CalcitTypeAnnotation) -> Option<&'static str> {
  match type_value {
    CalcitTypeAnnotation::List(_) => Some("&core-list-impls"),
    CalcitTypeAnnotation::String => Some("&core-string-impls"),
    CalcitTypeAnnotation::Map(_, _) => Some("&core-map-impls"),
    CalcitTypeAnnotation::Set(_) => Some("&core-set-impls"),
    CalcitTypeAnnotation::Number => Some("&core-number-impls"),
    CalcitTypeAnnotation::DynFn | CalcitTypeAnnotation::Fn(_) => Some("&core-fn-impls"),
    CalcitTypeAnnotation::Optional(inner) => core_impl_list_symbol_from_type_annotation(inner.as_ref()),
    _ => None,
  }
}

/// Read-only method metadata resolved with the same precedence rules as static
/// method validation and inlining.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticMethodDescriptor {
  pub name: String,
  pub origin: String,
}

/// List methods available for a statically known type, ordered from higher to
/// lower dispatch precedence. Returns `None` when method metadata cannot be
/// resolved (for example for a dynamic or external JS type).
pub fn static_method_descriptors(type_value: &CalcitTypeAnnotation) -> Option<Vec<StaticMethodDescriptor>> {
  if let Some(traits) = trait_list_from_type(type_value) {
    let mut seen = HashSet::new();
    let mut methods = vec![];
    for trait_def in traits.iter().rev() {
      for (method, kind) in trait_def.methods.iter().zip(trait_def.member_kinds.iter()) {
        if *kind != CalcitTraitMemberKind::Method {
          continue;
        }
        let name = format!(".{}", method.ref_str());
        if seen.insert(name.clone()) {
          methods.push(StaticMethodDescriptor {
            name,
            origin: trait_def.name.ref_str().to_owned(),
          });
        }
      }
    }
    return Some(methods);
  }

  if let Some(impls) = get_impls_from_type(type_value) {
    let last_wins = core_impl_list_symbol_from_type_annotation(type_value).is_none();
    let ordered_impls: Box<dyn Iterator<Item = &Arc<CalcitImpl>>> = if last_wins {
      Box::new(impls.iter().rev())
    } else {
      Box::new(impls.iter())
    };
    let mut seen = HashSet::new();
    let mut methods = vec![];

    for imp in ordered_impls {
      let origin = imp.trait_name().unwrap_or_else(|| imp.name()).ref_str().to_owned();
      for field in imp.fields().iter() {
        let name = format!(".{}", field.ref_str());
        if seen.insert(name.clone()) {
          methods.push(StaticMethodDescriptor {
            name,
            origin: origin.clone(),
          });
        }
      }
    }
    return Some(methods);
  }

  match type_value {
    CalcitTypeAnnotation::Dynamic
    | CalcitTypeAnnotation::JsObject
    | CalcitTypeAnnotation::Custom(_)
    | CalcitTypeAnnotation::TypeRef(_, _)
    | CalcitTypeAnnotation::TypeSlot(_) => None,
    CalcitTypeAnnotation::Optional(inner) => static_method_descriptors(inner.as_ref()),
    _ => Some(vec![]),
  }
}

fn find_method_entry<'a>(impls: &'a [Arc<CalcitImpl>], name: &str, last_wins: bool) -> Option<&'a Calcit> {
  if last_wins {
    for imp in impls.iter().rev() {
      if let Some(entry) = imp.get(name) {
        return Some(entry);
      }
    }
  } else {
    for imp in impls.iter() {
      if let Some(entry) = imp.get(name) {
        return Some(entry);
      }
    }
  }
  None
}

fn find_method_entry_for_type<'a>(type_ref: &CalcitTypeAnnotation, impls: &'a [Arc<CalcitImpl>], name: &str) -> Option<&'a Calcit> {
  // builtin impl lists are ordered by priority in calcit-core
  let last_wins = core_impl_list_symbol_from_type_annotation(type_ref).is_none();
  // user-defined values: impl-traits appends, so later impls override earlier ones
  find_method_entry(impls, name, last_wins)
}

/// Describe the type for error messages
fn describe_type(type_value: &CalcitTypeAnnotation) -> String {
  type_value.describe()
}

// tradition rule for processing exprs
pub fn preprocess_each_items(
  head: &CalcitSyntax,
  head_ns: &str,
  args: &CalcitList,
  ctx: &mut PreprocessContext,
) -> Result<Calcit, CalcitErr> {
  let mut xs: TernaryTreeList<Calcit> = TernaryTreeList::from(&[Calcit::Syntax(head.to_owned(), Arc::from(head_ns))]);
  args.traverse_result::<CalcitErr>(&mut |a| {
    let form = preprocess_expr(a, ctx.scope_defs, ctx.scope_types, ctx.file_ns, ctx.check_warnings, ctx.call_stack)?;
    xs = xs.push_right(form);
    Ok(())
  })?;
  Ok(Calcit::List(Arc::new(xs.into())))
}

fn preprocess_if(head: &CalcitSyntax, head_ns: &str, args: &CalcitList, ctx: &mut PreprocessContext) -> Result<Calcit, CalcitErr> {
  if args.len() < 2 {
    return preprocess_each_items(head, head_ns, args, ctx);
  }
  if args.len() > 3 {
    return Err(CalcitErr::use_msg_stack(
      CalcitErrKind::Syntax,
      format!("if expects 2 or 3 arguments, got {}", args.len()),
      ctx.call_stack,
    ));
  }

  let cond_form = preprocess_expr(
    args.first().unwrap(),
    ctx.scope_defs,
    ctx.scope_types,
    ctx.file_ns,
    ctx.check_warnings,
    ctx.call_stack,
  )?;

  warn_on_nominal_enum_truthiness(&cond_form, ctx.scope_types, ctx.file_ns, ctx.check_warnings);

  let narrowing = extract_predicate_bindings(&cond_form, ctx.scope_types);
  let mut true_scope_types = ctx.scope_types.clone();
  if let Some((sym, inferred)) = &narrowing.true_binding {
    true_scope_types.insert(sym.clone(), inferred.clone());
  }

  let true_form = preprocess_expr(
    args.get(1).unwrap(),
    ctx.scope_defs,
    &mut true_scope_types,
    ctx.file_ns,
    ctx.check_warnings,
    ctx.call_stack,
  )?;

  let false_form = if let Some(false_branch) = args.get(2) {
    let mut false_scope_types = ctx.scope_types.clone();
    if let Some((sym, inferred)) = &narrowing.false_binding {
      false_scope_types.insert(sym.clone(), inferred.clone());
    }
    Some(preprocess_expr(
      false_branch,
      ctx.scope_defs,
      &mut false_scope_types,
      ctx.file_ns,
      ctx.check_warnings,
      ctx.call_stack,
    )?)
  } else {
    None
  };

  // P7: constant folding — eliminate dead branch when condition is a literal
  match &cond_form {
    Calcit::Bool(true) => return Ok(true_form),
    Calcit::Bool(false) | Calcit::Nil => return Ok(false_form.unwrap_or(Calcit::Nil)),
    _ => {}
  }

  let mut xs: TernaryTreeList<Calcit> = TernaryTreeList::from(&[Calcit::Syntax(head.to_owned(), Arc::from(head_ns))]);
  xs = xs.push_right(cond_form);
  xs = xs.push_right(true_form);
  if let Some(f) = false_form {
    xs = xs.push_right(f);
  }

  Ok(Calcit::List(Arc::new(xs.into())))
}

struct PredicateNarrowing {
  true_binding: Option<(Arc<str>, Arc<CalcitTypeAnnotation>)>,
  false_binding: Option<(Arc<str>, Arc<CalcitTypeAnnotation>)>,
}

fn extract_predicate_bindings(cond_form: &Calcit, scope_types: &ScopeTypes) -> PredicateNarrowing {
  let empty = PredicateNarrowing {
    true_binding: None,
    false_binding: None,
  };
  let Calcit::List(items) = cond_form else {
    return empty;
  };
  if items.len() != 2 {
    return empty;
  }
  let Some(pred_name) = (match items.first() {
    Some(Calcit::Symbol { sym, .. }) => Some(sym.as_ref()),
    Some(Calcit::Import(CalcitImport { def, .. })) => Some(def.as_ref()),
    Some(Calcit::Proc(proc)) => Some(proc.as_ref()),
    _ => None,
  }) else {
    return empty;
  };
  let target = match items.get(1) {
    Some(t) => t,
    None => return empty,
  };
  let sym = match target {
    Calcit::Local(local) => local.sym.to_owned(),
    Calcit::Symbol { sym, .. } => sym.to_owned(),
    _ => return empty,
  };

  // Simple type predicates: narrow the true branch to the asserted type
  if let Some(ann) = match pred_name {
    "list?" => Some(tag_annotation("list")),
    "map?" => Some(tag_annotation("map")),
    "set?" => Some(tag_annotation("set")),
    "string?" => Some(tag_annotation("string")),
    "number?" => Some(tag_annotation("number")),
    "enum?" => Some(tag_annotation("enum")),
    "struct?" => Some(tag_annotation("struct")),
    "enum-def?" => Some(tag_annotation("enum-def")),
    "struct-def?" => Some(tag_annotation("struct-def")),
    "tag?" => Some(tag_annotation("tag")),
    "bool?" => Some(tag_annotation("bool")),
    "symbol?" => Some(tag_annotation("symbol")),
    "fn?" => Some(tag_annotation("fn")),
    _ => None,
  } {
    return PredicateNarrowing {
      true_binding: Some((sym, ann)),
      false_binding: None,
    };
  }

  // Legacy nil?/some? narrow only legacy Optional values. JavaScript host
  // nullability uses dedicated predicates so the FFI boundary stays visible.
  match pred_name {
    "nil?" => {
      let false_binding = scope_types.get(&sym).and_then(|current| {
        if let CalcitTypeAnnotation::Optional(inner) = current.as_ref() {
          Some((sym.clone(), inner.clone()))
        } else {
          None
        }
      });
      PredicateNarrowing {
        true_binding: Some((sym, Arc::new(CalcitTypeAnnotation::Nil))),
        false_binding,
      }
    }
    "some?" => {
      let true_binding = scope_types.get(&sym).and_then(|current| {
        if let CalcitTypeAnnotation::Optional(inner) = current.as_ref() {
          Some((sym.clone(), inner.clone()))
        } else {
          None
        }
      });
      PredicateNarrowing {
        true_binding,
        false_binding: Some((sym, Arc::new(CalcitTypeAnnotation::Nil))),
      }
    }
    "js-nullish?" => {
      let false_binding = scope_types.get(&sym).and_then(|current| {
        if let CalcitTypeAnnotation::JsNullish(inner) = current.as_ref() {
          Some((sym.clone(), inner.clone()))
        } else {
          None
        }
      });
      PredicateNarrowing {
        true_binding: Some((sym, Arc::new(CalcitTypeAnnotation::JsNullish(calcit::DYNAMIC_TYPE.clone())))),
        false_binding,
      }
    }
    "js-present?" => {
      let true_binding = scope_types.get(&sym).and_then(|current| {
        if let CalcitTypeAnnotation::JsNullish(inner) = current.as_ref() {
          Some((sym.clone(), inner.clone()))
        } else {
          None
        }
      });
      PredicateNarrowing {
        true_binding,
        false_binding: Some((sym, Arc::new(CalcitTypeAnnotation::JsNullish(calcit::DYNAMIC_TYPE.clone())))),
      }
    }
    _ => empty,
  }
}

fn resolve_enum_type_for_match(
  type_ref: &CalcitTypeAnnotation,
  file_ns: &str,
  scope_types: &ScopeTypes,
) -> Option<calcit::CalcitEnumDef> {
  if let Some(enum_def) = type_ref.resolve_to_enum() {
    return Some(enum_def);
  }
  let CalcitTypeAnnotation::TypeRef(name, _) = type_ref else {
    return None;
  };
  let stripped = name.trim_start_matches('\'').trim_start_matches(':');
  let short_name = stripped.rsplit('/').next().unwrap_or(stripped);
  if let Some(local_type) = scope_types.get(stripped).or_else(|| scope_types.get(short_name)) {
    if let CalcitTypeAnnotation::EnumDef(enum_def) = local_type.as_ref() {
      return Some(enum_def.as_ref().to_owned());
    }
    if let Some(enum_def) = local_type.resolve_to_enum() {
      return Some(enum_def);
    }
  }
  let (target_ns, target_def) = if let Some((ns, def)) = stripped.rsplit_once('/') {
    (Arc::from(ns), Arc::from(def))
  } else if program::has_def_code(file_ns, stripped) {
    (Arc::from(file_ns), Arc::from(stripped))
  } else if let Some(target_ns) = program::lookup_def_target_in_import(file_ns, stripped) {
    (target_ns, Arc::from(stripped))
  } else {
    (Arc::from(calcit::CORE_NS), Arc::from(stripped))
  };
  match resolve_program_value_for_preprocess(&target_ns, &target_def, None) {
    Some(Calcit::EnumDef(enum_def)) => Some(enum_def),
    Some(Calcit::Struct(struct_value)) => calcit::CalcitEnumDef::from_struct(struct_value).ok(),
    _ => None,
  }
}

fn is_match_wildcard(pattern: &Calcit) -> bool {
  matches!(
    pattern,
    Calcit::Symbol { sym, .. } | Calcit::Local(CalcitLocal { sym, .. }) if sym.as_ref() == "_"
  )
}

/// Arrange validated match branches by enum declaration slot. The final slot
/// stores an optional trailing wildcard branch. Decline forms whose source
/// order carries semantics that an indexed table cannot preserve.
fn build_indexed_match_table(enum_def: &calcit::CalcitEnumDef, branches: &[Calcit]) -> Option<Calcit> {
  let mut slots = vec![Calcit::Nil; enum_def.variants().len() + 1];
  let wildcard_slot = enum_def.variants().len();

  for (branch_idx, branch) in branches.iter().enumerate() {
    let Calcit::List(pair) = branch else { return None };
    if pair.len() != 2 {
      return None;
    }
    let pattern = &pair[0];
    if is_match_wildcard(pattern) {
      if branch_idx + 1 != branches.len() || !matches!(slots[wildcard_slot], Calcit::Nil) {
        return None;
      }
      slots[wildcard_slot] = branch.to_owned();
      continue;
    }

    let Calcit::List(pattern_items) = pattern else { return None };
    let Some(Calcit::Tag(tag)) = pattern_items.first() else {
      return None;
    };
    let variant_idx = enum_def.variant_index(tag)?;
    if !matches!(slots[variant_idx], Calcit::Nil) {
      return None;
    }
    slots[variant_idx] = branch.to_owned();
  }

  Some(Calcit::from(CalcitList::Vector(slots)))
}

/// Preprocess `match` syntax and perform exhaustiveness checking.
/// Input form (pair-based): `(match <value> (<pattern1> <body1>) (<pattern2> <body2>) ...)`
///
/// Each pattern is either:
/// - a list `(:tag binding1 binding2 ...)` for enum variant matching
/// - the symbol `_` for a wildcard/default case
fn preprocess_match(head: &CalcitSyntax, head_ns: &str, args: &CalcitList, ctx: &mut PreprocessContext) -> Result<Calcit, CalcitErr> {
  if args.is_empty() {
    return Err(CalcitErr::use_msg_stack(
      CalcitErrKind::Syntax,
      "match expected a value expression and branches".to_owned(),
      ctx.call_stack,
    ));
  }

  // After the value, remaining args are pairs: (pattern body) (pattern body) ...
  // Each pair is a 2-element list where first is the pattern and second is the body.
  // Cirru naturally creates this when each branch is on its own indented line.
  let branch_count = args.len() - 1;
  if branch_count == 0 {
    return Err(CalcitErr::use_msg_stack(
      CalcitErrKind::Syntax,
      "match expected value followed by (pattern body) pairs, got 0 branches".to_owned(),
      ctx.call_stack,
    ));
  }

  let mut xs: Vec<Calcit> = vec![Calcit::Syntax(head.to_owned(), Arc::from(head_ns))];

  // Preprocess the value expression
  let value_form = preprocess_expr(
    args.first().unwrap(),
    ctx.scope_defs,
    ctx.scope_types,
    ctx.file_ns,
    ctx.check_warnings,
    ctx.call_stack,
  )?;

  // Try to infer enum type from the value expression for exhaustiveness checking
  // and payload typing. Applied generic arguments are kept so a matched payload
  // can be specialized instead of falling back to Dynamic.
  let inferred_match_type = infer_type_from_expr(&value_form, ctx.scope_types);
  let enum_match = inferred_match_type.and_then(|t| match t.as_ref() {
    CalcitTypeAnnotation::EnumValue(enum_ref) => Some((enum_ref.as_ref().to_owned(), Arc::new(vec![]))),
    CalcitTypeAnnotation::Enum(enum_ref, args) => Some((enum_ref.as_ref().to_owned(), args.clone())),
    CalcitTypeAnnotation::TypeRef(_, args) => {
      resolve_enum_type_for_match(t.as_ref(), ctx.file_ns, ctx.scope_types).map(|enum_ref| (enum_ref, args.clone()))
    }
    CalcitTypeAnnotation::TypeSlot(name) => calcit::resolve_type_slot(name).and_then(|resolved| match resolved.as_ref() {
      CalcitTypeAnnotation::Enum(e, args) => Some((e.as_ref().to_owned(), args.clone())),
      CalcitTypeAnnotation::EnumValue(e) => Some((e.as_ref().to_owned(), Arc::new(vec![]))),
      _ => None,
    }),
    _ => None,
  });
  let enum_def = enum_match.as_ref().map(|(enum_def, _)| enum_def);
  let mut enum_bindings: HashMap<Arc<str>, Arc<CalcitTypeAnnotation>> = HashMap::new();
  if let Some((enum_def, applied_args)) = enum_match.as_ref() {
    for (name, applied) in enum_def.generics().iter().zip(applied_args.iter()) {
      if !matches!(applied.as_ref(), CalcitTypeAnnotation::Dynamic) {
        enum_bindings.insert(name.clone(), applied.clone());
      }
    }
    for bound in enum_def.where_bounds() {
      if !enum_bindings.contains_key(&bound.name)
        && let Some(trait_type) = resolve_where_bound_type_for_body(bound, ctx.file_ns)
      {
        enum_bindings.insert(bound.name.clone(), trait_type);
      }
    }
  }

  xs.push(value_form);

  // Collect matched tags for exhaustiveness checking
  let mut matched_tags: Vec<Arc<str>> = vec![];
  let mut has_wildcard = false;

  // Iterate branch pairs: each arg after value is (pattern body)
  for branch_idx in 1..args.len() {
    let branch = &args[branch_idx];
    let pair = match branch {
      Calcit::List(pair_xs) if pair_xs.len() == 2 => pair_xs,
      other => {
        return Err(CalcitErr::use_msg_stack_location(
          CalcitErrKind::Syntax,
          format!("match branch expected a 2-element list (pattern body), got: {other}"),
          ctx.call_stack,
          other.get_location(),
        ));
      }
    };
    let pattern = &pair[0];
    let body = &pair[1];

    match pattern {
      // Wildcard: `_`
      Calcit::Symbol { sym, .. } if sym.as_ref() == "_" => {
        has_wildcard = true;
        let processed_body = preprocess_expr(
          body,
          ctx.scope_defs,
          ctx.scope_types,
          ctx.file_ns,
          ctx.check_warnings,
          ctx.call_stack,
        )?;
        xs.push(Calcit::from(CalcitList::from(&[pattern.to_owned(), processed_body])));
      }
      // Tag pattern: (:tag binding1 binding2 ...)
      Calcit::List(pat_xs) if !pat_xs.is_empty() => {
        let pat_tag = match &pat_xs[0] {
          Calcit::Tag(t) => t.ref_str(),
          other => {
            return Err(CalcitErr::use_msg_stack_location(
              CalcitErrKind::Syntax,
              format!("match pattern expected a tag as first element, got: {other}"),
              ctx.call_stack,
              other.get_location(),
            ));
          }
        };

        // Validate variant exists in enum and check arity
        if let Some(enum_def) = enum_def {
          if let Some(variant) = enum_def.find_variant_by_name(pat_tag) {
            let expected_arity = variant.arity();
            let actual_arity = pat_xs.len() - 1;
            if expected_arity != actual_arity {
              gen_check_warning(
                format!(
                  "[Warn] match: variant `{}::{}` expects {} payload(s), but pattern binds {}, at {}/{}",
                  enum_def.name(),
                  pat_tag,
                  expected_arity,
                  actual_arity,
                  ctx.file_ns,
                  ctx.call_stack.0.first().map(|f| f.def.as_ref()).unwrap_or("?")
                ),
                ctx.file_ns,
                ctx.check_warnings,
              );
            }
          } else {
            let available: Vec<&str> = enum_def.variants().iter().map(|v| v.tag.ref_str()).collect();
            gen_check_warning(
              format!(
                "[Warn] match: enum `{}` has no variant `:{pat_tag}`. Available: [{}], at {}/{}",
                enum_def.name(),
                available.join(", "),
                ctx.file_ns,
                ctx.call_stack.0.first().map(|f| f.def.as_ref()).unwrap_or("?")
              ),
              ctx.file_ns,
              ctx.check_warnings,
            );
          }
        }

        matched_tags.push(Arc::from(pat_tag));

        // Create scope with bindings for the body
        let mut body_defs = ctx.scope_defs.to_owned();
        let mut body_types = ctx.scope_types.clone();
        let mut processed_pattern: Vec<Calcit> = vec![pat_xs[0].to_owned()]; // Keep the tag

        for (bind_idx, binding) in pat_xs.iter().skip(1).enumerate() {
          match binding {
            Calcit::Symbol { sym, info, location } => {
              body_defs.insert(sym.to_owned());

              // Infer payload type from enum variant definition
              let payload_type = enum_def
                .and_then(|e| e.find_variant_by_name(pat_tag))
                .and_then(|v| v.payload_types().get(bind_idx).cloned())
                .map(|payload| payload.substitute_type_vars(&enum_bindings))
                .map(|payload| resolve_local_type_refs_for_body(payload, &body_types))
                .unwrap_or_else(|| crate::calcit::DYNAMIC_TYPE.clone());

              let local = Calcit::Local(CalcitLocal {
                idx: CalcitLocal::track_sym(sym),
                sym: sym.to_owned(),
                info: Arc::new(CalcitSymbolInfo {
                  at_ns: info.at_ns.to_owned(),
                  at_def: info.at_def.to_owned(),
                }),
                location: location.to_owned(),
                type_info: payload_type.clone(),
              });

              body_types.insert(sym.to_owned(), payload_type);
              processed_pattern.push(local);
            }
            other => {
              return Err(CalcitErr::use_msg_stack_location(
                CalcitErrKind::Syntax,
                format!("match pattern binding expected a symbol, got: {other}"),
                ctx.call_stack,
                other.get_location(),
              ));
            }
          }
        }

        let processed_body = preprocess_expr(body, &body_defs, &mut body_types, ctx.file_ns, ctx.check_warnings, ctx.call_stack)?;
        xs.push(Calcit::from(CalcitList::from(&[
          Calcit::from(CalcitList::from(processed_pattern.as_slice())),
          processed_body,
        ])));
      }
      other => {
        return Err(CalcitErr::use_msg_stack_location(
          CalcitErrKind::Syntax,
          format!("match pattern expected (:tag ...) or _, got: {other}"),
          ctx.call_stack,
          other.get_location(),
        ));
      }
    }
  }

  // Exhaustiveness checking
  if let Some(enum_def) = enum_def
    && !has_wildcard
  {
    let all_variants: BTreeSet<&str> = enum_def.variants().iter().map(|v| v.tag.ref_str()).collect();
    let covered: BTreeSet<&str> = matched_tags.iter().map(|t| t.as_ref()).collect();
    let missing: Vec<&str> = all_variants.difference(&covered).copied().collect();

    if !missing.is_empty() {
      gen_check_warning(
        format!(
          "[Warn] match on `{}` is not exhaustive. Missing variant(s): [{}], at {}/{}",
          enum_def.name(),
          missing.iter().map(|t| format!(":{t}")).collect::<Vec<_>>().join(", "),
          ctx.file_ns,
          ctx.call_stack.0.first().map(|f| f.def.as_ref()).unwrap_or("?")
        ),
        ctx.file_ns,
        ctx.check_warnings,
      );
    }
  }

  if let Some(enum_def) = enum_def
    && let Some(table) = build_indexed_match_table(enum_def, &xs[2..])
  {
    return Ok(Calcit::from(CalcitList::Vector(vec![
      xs[0].to_owned(),
      xs[1].to_owned(),
      Calcit::EnumDef(enum_def.to_owned()),
      table,
    ])));
  }

  Ok(Calcit::List(Arc::from(CalcitList::Vector(xs))))
}

/// Maps a source-facing macro contract to the runtime type visible inside the macro body.
fn macro_contract_body_type(contract: &MacroSyntaxType) -> Arc<CalcitTypeAnnotation> {
  match contract {
    MacroSyntaxType::SyntaxSymbol => Arc::new(CalcitTypeAnnotation::Symbol),
    MacroSyntaxType::SyntaxList => Arc::new(CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Syntax(Arc::new(
      MacroSyntaxType::Syntax,
    ))))),
    MacroSyntaxType::Syntax | MacroSyntaxType::Expr(_) => Arc::new(CalcitTypeAnnotation::Syntax(Arc::new(contract.clone()))),
  }
}

/// Builds body parameter types in required, optional, and rest binding order.
fn strict_macro_body_parameter_types(signature: &MacroSignature) -> Vec<Arc<CalcitTypeAnnotation>> {
  let required_types = signature.required_inputs.iter().map(macro_contract_body_type);
  let optional_types = signature
    .optional_inputs
    .iter()
    .map(|contract| Arc::new(CalcitTypeAnnotation::Optional(macro_contract_body_type(contract))));
  let rest_type = signature
    .rest_input
    .iter()
    .map(|contract| Arc::new(CalcitTypeAnnotation::List(macro_contract_body_type(contract))));
  required_types.chain(optional_types).chain(rest_type).collect()
}

pub fn preprocess_defn(
  head: &CalcitSyntax,
  head_ns: &str,
  args: &CalcitList,
  ctx: &mut PreprocessContext,
) -> Result<Calcit, CalcitErr> {
  // println!("defn args: {}", primes::CrListWrap(args.to_owned()));
  let mut xs: TernaryTreeList<Calcit> = TernaryTreeList::from(&[Calcit::Syntax(head.to_owned(), Arc::from(head_ns))]);
  match (args.first(), args.get(1)) {
    (
      Some(Calcit::Symbol {
        sym: def_name,
        info,
        location,
        ..
      }),
      Some(Calcit::List(ys)),
    ) => {
      let mut body_defs: HashSet<Arc<str>> = ctx.scope_defs.to_owned();
      let mut body_types: ScopeTypes = ctx.scope_types.clone();
      let mut param_symbols: Vec<Arc<str>> = vec![];
      let mut has_marked_args = false; // Track if function has & or ? markers

      xs = xs.push_right(Calcit::Symbol {
        sym: def_name.to_owned(),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: info.at_ns.to_owned(),
          at_def: info.at_def.to_owned(),
        }),
        location: location.to_owned(),
      });
      let mut zs = vec![];

      ys.traverse_result(&mut |y| {
        match y {
          Calcit::Syntax(CalcitSyntax::ArgSpread, _)
          | Calcit::Syntax(CalcitSyntax::ArgOptional, _)
          | Calcit::Syntax(CalcitSyntax::MacroInterpolate, _)
          | Calcit::Syntax(CalcitSyntax::MacroInterpolateSpread, _) => {
            has_marked_args = true; // Mark that this function has special args
            zs.push(y.to_owned());
            Ok(())
          }
          Calcit::Symbol {
            sym,
            info,
            location: arg_location,
            ..
          } => {
            param_symbols.push(sym.to_owned());
            let loc = NodeLocation::new(
              info.at_ns.to_owned(),
              info.at_def.to_owned(),
              arg_location.to_owned().unwrap_or_default(),
            );
            check_symbol(sym, args, loc, ctx.check_warnings);
            body_types.remove(sym);
            let s = Calcit::Local(CalcitLocal {
              idx: CalcitLocal::track_sym(sym),
              sym: sym.to_owned(),
              info: Arc::new(CalcitSymbolInfo {
                at_ns: info.at_ns.to_owned(),
                at_def: info.at_def.to_owned(),
              }),
              location: arg_location.to_owned(),
              type_info: crate::calcit::DYNAMIC_TYPE.clone(),
            });
            // println!("created local: {:?}", s);
            zs.push(s);

            // track local in scope
            body_defs.insert(sym.to_owned());
            Ok(())
          }
          _ => Err(CalcitErr::use_msg_stack_location(
            CalcitErrKind::Type,
            format!("expected defn args to be symbols, got: {y}"),
            ctx.call_stack,
            y.get_location(),
          )),
        }
      })?;
      let def_schema = program::lookup_def_schema(ctx.file_ns, def_name.as_ref());
      if matches!(head, CalcitSyntax::DefWasmImport) && !has_valid_wasm_import_body(args) {
        return Err(CalcitErr::use_msg_stack_location(
          CalcitErrKind::Syntax,
          "defwasm-import expects exactly two literal strings for the WASM module and field name",
          ctx.call_stack,
          Some(NodeLocation::new(
            info.at_ns.to_owned(),
            info.at_def.to_owned(),
            location.to_owned().unwrap_or_default(),
          )),
        ));
      }
      warn_on_legacy_optional_public_schema(ctx.file_ns, def_name.as_ref(), &def_schema, ctx.check_warnings);
      let schema_issues = validate_def_schema_during_preprocess(head, ctx.file_ns, def_name.as_ref(), ys, &def_schema);
      let definition_location = NodeLocation::new(
        info.at_ns.to_owned(),
        info.at_def.to_owned(),
        location.to_owned().unwrap_or_default(),
      );
      if !schema_issues.is_empty() {
        let details = schema_issues.join("\n  - ");
        return Err(CalcitErr::use_msg_stack_location_with_code(
          CalcitErrKind::Type,
          format!("schema mismatch while preprocessing definition:\n  - {details}"),
          "E_SCHEMA_DEF_MISMATCH",
          ctx.call_stack,
          Some(definition_location),
        ));
      }

      // Inject declared argument types into the function body. Call-site checks alone are not
      // enough: without these bindings, local method dispatch and return inference inside a named
      // `defn` unnecessarily fall back to Dynamic. Anonymous callbacks still use EXPECTED_FN_TYPE.
      let body_fn_hint = args
        .iter()
        .skip(2)
        .find_map(CalcitTypeAnnotation::extract_fn_annotation_from_hint_form)
        .and_then(|annotation| match annotation.as_ref() {
          CalcitTypeAnnotation::Fn(fn_annotation) => Some(fn_annotation.clone()),
          _ => None,
        });
      let effective_fn_schema: Option<Arc<CalcitFnTypeAnnotation>> = body_fn_hint.or_else(|| match def_schema.as_ref() {
        CalcitTypeAnnotation::Fn(fn_annot) => Some(fn_annot.clone()),
        CalcitTypeAnnotation::Dynamic => EXPECTED_FN_TYPE.with(|cell| cell.borrow().clone()),
        _ => None,
      });
      if let CalcitTypeAnnotation::Macro(signature) = def_schema.as_ref() {
        for (param_sym, type_info) in param_symbols.iter().zip(strict_macro_body_parameter_types(signature)) {
          body_types.insert(param_sym.clone(), type_info);
        }
      }
      if let Some(fn_annot) = &effective_fn_schema {
        let body_type_bindings: HashMap<Arc<str>, Arc<CalcitTypeAnnotation>> = fn_annot
          .where_bounds
          .iter()
          .filter_map(|bound| resolve_where_bound_type_for_body(bound, ctx.file_ns).map(|trait_type| (bound.name.clone(), trait_type)))
          .collect();
        let parameter_types = fn_annot
          .arg_types
          .iter()
          .enumerate()
          .map(|(idx, arg_type)| {
            let substituted = arg_type.substitute_type_vars(&body_type_bindings);
            let positional = unwrap_named_body_parameter_type(substituted, param_symbols.get(idx));
            let local = resolve_local_type_refs_for_body(positional, &body_types);
            resolve_namespace_type_refs_for_body(local, ctx.file_ns)
          })
          .chain(fn_annot.rest_type.iter().map(|rest_type| {
            let local = resolve_local_type_refs_for_body(rest_type.substitute_type_vars(&body_type_bindings), &body_types);
            Arc::new(CalcitTypeAnnotation::Variadic(resolve_namespace_type_refs_for_body(
              local,
              ctx.file_ns,
            )))
          }))
          .collect::<Vec<_>>();
        for (param_sym, arg_type) in param_symbols.iter().zip(parameter_types) {
          if !matches!(arg_type.as_ref(), CalcitTypeAnnotation::Dynamic) {
            body_types.insert(param_sym.to_owned(), arg_type);
          }
        }
      }

      // Keep the parameter nodes themselves typed as well, so expression-level inspection at the
      // declaration path reports the same annotation as references in the body.
      for param in &mut zs {
        if let Calcit::Local(local) = param
          && let Some(type_info) = body_types.get(&local.sym)
        {
          local.type_info = type_info.clone();
        }
      }
      xs = xs.push_right(Calcit::from(zs.clone()));

      let mut to_skip = 2;
      let mut processed_body: Vec<Calcit> = vec![];

      if let CalcitTypeAnnotation::Fn(fn_annot) = def_schema.as_ref() {
        let schema_calcit = fn_annot.to_schema_calcit();
        let schema_hint = Calcit::from(vec![Calcit::Syntax(CalcitSyntax::HintFn, Arc::from(ctx.file_ns)), schema_calcit]);
        processed_body.push(schema_hint.to_owned());
        xs = xs.push_right(schema_hint);
      }

      // Set current function features for FFI permission checks during body preprocessing.
      // Save previous state for restoration after the function body is processed.
      let prev_features = CURRENT_FN_FEATURES.with(|cell| {
        let mut guard = cell.borrow_mut();
        let old = guard.take();
        *guard = match def_schema.as_ref() {
          CalcitTypeAnnotation::Macro(signature) => Some(signature.features.clone()),
          _ => effective_fn_schema.as_ref().map(|fn_annot| fn_annot.features.clone()),
        };
        old
      });

      args.traverse_result::<CalcitErr>(&mut |a| {
        if to_skip > 0 {
          to_skip -= 1;
          return Ok(());
        }
        let form = preprocess_expr(a, &body_defs, &mut body_types, ctx.file_ns, ctx.check_warnings, ctx.call_stack)?;
        processed_body.push(form.clone());
        xs = xs.push_right(form);
        Ok(())
      })?;

      // Check function return type if declared
      // Extract return type hint from processed body (after preprocessing)
      let detected_return_type = detect_return_type_hint_from_processed_body(&processed_body);
      let return_type_hint = if matches!(detected_return_type.as_ref(), CalcitTypeAnnotation::Dynamic) {
        effective_fn_schema
          .as_ref()
          .map(|schema| schema.return_type.clone())
          .unwrap_or(detected_return_type)
      } else {
        detected_return_type
      };
      check_function_return_type(
        &processed_body,
        &return_type_hint,
        &body_types,
        ctx.file_ns,
        def_name.as_ref(),
        ctx.check_warnings,
      );

      // Check recur arity in function body
      // Skip checking for:
      // 1. Functions with marked args (& or ?) - complex arity rules
      // 2. calcit.core functions - external library, should be fixed separately
      let is_core_ns = ctx.file_ns == calcit::CORE_NS;
      if !has_marked_args && !is_core_ns {
        let expected_arity = param_symbols.len();
        for body_expr in &processed_body {
          check_recur_arity_in_expr(body_expr, expected_arity, ctx.file_ns, def_name.as_ref(), ctx.check_warnings);
        }
      }

      for body_expr in &processed_body {
        check_impl_traits_top_level_in_expr(body_expr, ctx.file_ns, def_name.as_ref(), ctx.check_warnings);
      }

      // Restore previous function features
      CURRENT_FN_FEATURES.with(|cell| *cell.borrow_mut() = prev_features);

      Ok(Calcit::List(Arc::new(xs.into())))
    }
    (Some(a), Some(b)) => Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Syntax,
      format!("defn/defmacro expected name and args: {a} {b}"),
      ctx.call_stack,
      a.get_location().or_else(|| b.get_location()),
    )),
    (a, b) => {
      let loc = a
        .and_then(|node| node.get_location())
        .or_else(|| b.and_then(|node| node.get_location()));
      Err(CalcitErr::use_msg_stack_location(
        CalcitErrKind::Syntax,
        format!("defn or defmacro expected name and args, got: {a:?} {b:?}",),
        ctx.call_stack,
        loc,
      ))
    }
  }
}

fn has_valid_wasm_import_body(args: &CalcitList) -> bool {
  matches!(
    (args.get(2), args.get(3), args.get(4)),
    (Some(Calcit::Str(_)), Some(Calcit::Str(_)), None)
  )
}

// warn if this symbol is used
fn check_symbol(sym: &str, args: &CalcitList, location: NodeLocation, check_warnings: &RefCell<Vec<LocatedWarning>>) {
  if is_proc_name(sym) || CalcitSyntax::is_valid(sym) || program::has_def_code(calcit::CORE_NS, sym) {
    gen_check_warning_with_location(
      format!("[Warn] local binding `{sym}` shadowed `calcit.core/{sym}`, with {args}"),
      location,
      check_warnings,
    );
  }
}

pub fn preprocess_core_let(
  head: &CalcitSyntax,
  // where the symbol was defined
  head_ns: &str,
  args: &CalcitList,
  ctx: &mut PreprocessContext,
) -> Result<Calcit, CalcitErr> {
  let mut xs: Vec<Calcit> = vec![Calcit::Syntax(head.to_owned(), Arc::from(head_ns))];
  let mut body_defs: HashSet<Arc<str>> = ctx.scope_defs.to_owned();
  let mut body_types: ScopeTypes = ctx.scope_types.clone();
  let binding = match args.first() {
    Some(Calcit::List(ys)) if ys.is_empty() => Calcit::from(CalcitList::default()),
    Some(Calcit::List(ys)) if ys.len() == 2 => match (&ys[0], &ys[1]) {
      (Calcit::Symbol { sym, info, location }, a) => {
        let loc = NodeLocation::new(
          info.at_ns.to_owned(),
          info.at_def.to_owned(),
          location.to_owned().unwrap_or_default(),
        );
        check_symbol(sym, ys, loc, ctx.check_warnings);
        body_defs.insert(sym.to_owned());
        let form = preprocess_expr(a, &body_defs, &mut body_types, ctx.file_ns, ctx.check_warnings, ctx.call_stack)?;

        // Try to infer type from the binding expression
        let inferred_type = infer_type_from_expr(&form, &body_types).unwrap_or_else(|| crate::calcit::DYNAMIC_TYPE.clone());

        let name = Calcit::Local(CalcitLocal {
          idx: CalcitLocal::track_sym(sym),
          sym: sym.to_owned(),
          info: Arc::new(CalcitSymbolInfo {
            at_ns: info.at_ns.to_owned(),
            at_def: info.at_def.to_owned(),
          }),
          location: location.to_owned(),
          type_info: inferred_type.clone(),
        });

        // Also store in scope_types for later use
        body_types.insert(sym.to_owned(), inferred_type);

        Calcit::from(CalcitList::from(&[name, form]))
      }
      (a, b) => {
        return Err(CalcitErr::use_msg_stack_location(
          CalcitErrKind::Syntax,
          format!("invalid pair for &let binding: {a} {b}"),
          ctx.call_stack,
          a.get_location().or_else(|| b.get_location()),
        ));
      }
    },
    Some(a @ Calcit::List(_)) => {
      return Err(CalcitErr::use_msg_stack_location(
        CalcitErrKind::Syntax,
        format!("expected binding of a pair, got: {a}"),
        ctx.call_stack,
        a.get_location(),
      ));
    }
    Some(a) => {
      return Err(CalcitErr::use_msg_stack_location(
        CalcitErrKind::Syntax,
        format!("expected binding of a pair, got: {a}"),
        ctx.call_stack,
        a.get_location(),
      ));
    }
    None => {
      return Err(CalcitErr::use_msg_stack(
        CalcitErrKind::Syntax,
        "expected binding of a pair, got nothing".to_owned(),
        ctx.call_stack,
      ));
    }
  };
  xs.push(binding);

  let mut skipped_head = false;
  args.traverse_result::<CalcitErr>(&mut |a| {
    if !skipped_head {
      skipped_head = true;
      return Ok(());
    }
    let form = preprocess_expr(a, &body_defs, &mut body_types, ctx.file_ns, ctx.check_warnings, ctx.call_stack)?;
    xs.push(form);
    Ok(())
  })?;
  Ok(Calcit::List(Arc::from(CalcitList::Vector(xs))))
}

pub fn preprocess_quote(
  head: &CalcitSyntax,
  head_ns: &str,
  args: &CalcitList,
  _scope_defs: &HashSet<Arc<str>>,
  _file_ns: &str,
) -> Result<Calcit, CalcitErr> {
  let mut xs: TernaryTreeList<Calcit> = TernaryTreeList::from(&[Calcit::Syntax(head.to_owned(), Arc::from(head_ns))]);

  args.traverse_result::<CalcitErr>(&mut |a| {
    xs = xs.push_right(a.to_owned());
    Ok(())
  })?;
  Ok(Calcit::List(Arc::new(xs.into())))
}

pub fn preprocess_defatom(
  head: &CalcitSyntax,
  head_ns: &str,
  args: &CalcitList,
  ctx: &mut PreprocessContext,
) -> Result<Calcit, CalcitErr> {
  let mut xs: TernaryTreeList<Calcit> = TernaryTreeList::from(&[Calcit::Syntax(head.to_owned(), Arc::from(head_ns))]);

  args.traverse_result::<CalcitErr>(&mut |a| {
    // TODO
    let form = preprocess_expr(a, ctx.scope_defs, ctx.scope_types, ctx.file_ns, ctx.check_warnings, ctx.call_stack)?;
    xs = xs.push_right(form.to_owned());
    Ok(())
  })?;
  Ok(Calcit::List(Arc::new(CalcitList::List(xs))))
}

/// need to handle experssions inside unquote snippets
pub fn preprocess_quasiquote(
  head: &CalcitSyntax,
  head_ns: &str,
  args: &CalcitList,
  ctx: &mut PreprocessContext,
) -> Result<Calcit, CalcitErr> {
  let mut xs: TernaryTreeList<Calcit> = TernaryTreeList::from(&[Calcit::Syntax(head.to_owned(), Arc::from(head_ns))]);

  args.traverse_result::<CalcitErr>(&mut |a| {
    let form = preprocess_quasiquote_internal(a, ctx.scope_defs, ctx.scope_types, ctx.file_ns, ctx.check_warnings, ctx.call_stack)?;
    xs = xs.push_right(form);
    Ok(())
  })?;
  Ok(Calcit::List(Arc::new(xs.into())))
}

pub fn preprocess_quasiquote_internal(
  x: &Calcit,
  scope_defs: &HashSet<Arc<str>>,
  scope_types: &mut ScopeTypes,
  file_ns: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  match x {
    Calcit::List(ys) if ys.is_empty() => Ok(x.to_owned()),
    Calcit::List(ys) => match &ys[0] {
      Calcit::Syntax(CalcitSyntax::MacroInterpolate, _) | &Calcit::Syntax(CalcitSyntax::MacroInterpolateSpread, _) => {
        let mut xs = vec![];
        for y in &**ys {
          let form = preprocess_expr(y, scope_defs, scope_types, file_ns, check_warnings, call_stack)?;
          xs.push(form.to_owned());
        }
        Ok(Calcit::from(xs))
      }
      _ => {
        let mut xs = vec![];
        for y in &**ys {
          xs.push(preprocess_quasiquote_internal(y, scope_defs, scope_types, file_ns, check_warnings, call_stack)?.to_owned());
        }
        Ok(Calcit::from(xs))
      }
    },
    _ => Ok(x.to_owned()),
  }
}

pub fn preprocess_hint_fn(
  head: &CalcitSyntax,
  head_ns: &str,
  args: &CalcitList,
  ctx: &mut PreprocessContext,
) -> Result<Calcit, CalcitErr> {
  let mut legacy_clauses: BTreeSet<&str> = BTreeSet::new();
  let mut error_location: Option<NodeLocation> = None;

  for item in args {
    let Calcit::List(inner) = item else {
      continue;
    };
    let Some(head) = inner.first() else {
      continue;
    };

    if let Some(name) = extract_hint_fn_legacy_clause_name(head) {
      legacy_clauses.insert(name);
      if error_location.is_none() {
        error_location = item.get_location();
      }
    }
  }

  if !legacy_clauses.is_empty() {
    let clauses = legacy_clauses.into_iter().collect::<Vec<_>>().join(", ");
    return Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Syntax,
      format!(
        "legacy hint-fn clauses are no longer supported ({clauses}); use schema map form like `(hint-fn $ {{}} (:args ...) (:return ...))`"
      ),
      ctx.call_stack,
      error_location,
    ));
  }

  // preserve hint-fn for JS codegen or other metadata needs
  let mut ys = vec![Calcit::Syntax(head.to_owned(), Arc::from(head_ns))];
  for a in args {
    ys.push(a.to_owned());
  }

  // The two-argument form annotates an existing local function value. Keep the annotation in
  // lexical scope so later calls, callback checks, and `type-at` retain the complete signature.
  // The one-argument form is metadata injected into a function body and has no target to refine.
  if args.len() >= 2
    && let Some(type_entry) = CalcitTypeAnnotation::extract_fn_annotation_from_hint_form(&Calcit::from(ys.clone()))
    && let Some(target_raw) = args.first()
  {
    let target_form = preprocess_expr(
      target_raw,
      ctx.scope_defs,
      ctx.scope_types,
      ctx.file_ns,
      ctx.check_warnings,
      ctx.call_stack,
    )?;
    if let Calcit::Local(local) = target_form {
      ctx.scope_types.insert(local.sym.to_owned(), type_entry.clone());
      let mut typed_local = local;
      typed_local.type_info = type_entry;
      ys[1] = Calcit::Local(typed_local);
    }
  }
  Ok(Calcit::from(ys))
}

pub fn preprocess_assert_type(
  head: &CalcitSyntax,
  head_ns: &str,
  args: &CalcitList,
  ctx: &mut PreprocessContext,
) -> Result<Calcit, CalcitErr> {
  if args.len() != 2 {
    return Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Arity,
      format!("{head} expected an expression and a type expression, got {}", args.len()),
      ctx.call_stack,
      args.first().and_then(|node| node.get_location()),
    ));
  }

  let target_raw = args.get(0).unwrap();
  let type_form = args.get(1).unwrap();

  let target_form = preprocess_expr(
    target_raw,
    ctx.scope_defs,
    ctx.scope_types,
    ctx.file_ns,
    ctx.check_warnings,
    ctx.call_stack,
  )?;

  // The type position may refer directly to a Struct/Enum definition. Resolve
  // only an unquoted symbol here: a quoted symbol remains a TypeVar or an
  // explicit nominal TypeRef, while `assert-type value Store` can use the
  // definition value itself without redundant quote syntax. The target must
  // resolve to a concrete StructDef/EnumDef; visible function or value names
  // are kept as-is instead of being parsed as resolved types.
  let asserted_type_form = match type_form {
    Calcit::Symbol { sym, info, .. } if !sym.starts_with('\'') => {
      let nominal_target = runner::parse_ns_def(sym)
        .filter(|(ns, def)| {
          program::lookup_def_code(ns, def).is_some_and(|code| crate::calcit::type_annotation::code_resolves_to_nominal_type_def(&code))
        })
        .or_else(|| {
          if program::lookup_def_code(&info.at_ns, sym)
            .is_some_and(|code| crate::calcit::type_annotation::code_resolves_to_nominal_type_def(&code))
          {
            Some((info.at_ns.clone(), sym.clone()))
          } else {
            program::lookup_def_target_in_import(&info.at_ns, sym)
              .filter(|ns| {
                program::lookup_def_code(ns, sym)
                  .is_some_and(|code| crate::calcit::type_annotation::code_resolves_to_nominal_type_def(&code))
              })
              .map(|ns| (ns, sym.clone()))
          }
        });
      if nominal_target.is_some() {
        preprocess_expr(
          type_form,
          ctx.scope_defs,
          ctx.scope_types,
          ctx.file_ns,
          ctx.check_warnings,
          ctx.call_stack,
        )?
      } else {
        type_form.to_owned()
      }
    }
    _ => type_form.to_owned(),
  };

  let local_nominal_type = match type_form {
    Calcit::Symbol { sym, .. } if !sym.starts_with('\'') => ctx.scope_types.get(sym).and_then(|type_info| match type_info.as_ref() {
      CalcitTypeAnnotation::StructDef(struct_def) => Some(Arc::new(CalcitTypeAnnotation::Struct(struct_def.clone(), Arc::new(vec![])))),
      CalcitTypeAnnotation::EnumDef(enum_def) => Some(Arc::new(CalcitTypeAnnotation::Enum(enum_def.clone(), Arc::new(vec![])))),
      _ => None,
    }),
    _ => None,
  };

  let asserted_target = target_form;
  if let Calcit::Local(local) = &asserted_target {
    let asserted_type = local_nominal_type.unwrap_or_else(|| CalcitTypeAnnotation::parse_type_annotation_form(&asserted_type_form));
    let current_type = resolve_type_value(&asserted_target, ctx.scope_types).unwrap_or_else(|| local.type_info.clone());
    let type_entry = if current_type.as_ref().matches_annotation(asserted_type.as_ref())
      && annotation_dynamic_weight(current_type.as_ref()) < annotation_dynamic_weight(asserted_type.as_ref())
    {
      current_type
    } else {
      asserted_type
    };
    ctx.scope_types.insert(local.sym.to_owned(), type_entry.clone());

    let mut typed_local = local.to_owned();
    typed_local.type_info = type_entry;

    return Ok(Calcit::Local(typed_local));
  }

  Ok(Calcit::from(vec![
    Calcit::Syntax(head.to_owned(), Arc::from(head_ns)),
    asserted_target,
    asserted_type_form,
  ]))
}

pub fn preprocess_unsafe_coerce(
  head: &CalcitSyntax,
  head_ns: &str,
  args: &CalcitList,
  ctx: &mut PreprocessContext,
) -> Result<Calcit, CalcitErr> {
  if args.len() != 2 {
    return Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Arity,
      format!("{head} expected a value and a type expression, got {}", args.len()),
      ctx.call_stack,
      args.first().and_then(|node| node.get_location()),
    ));
  }

  let target_form = preprocess_expr(
    args.first().expect("validated unsafe-coerce target"),
    ctx.scope_defs,
    ctx.scope_types,
    ctx.file_ns,
    ctx.check_warnings,
    ctx.call_stack,
  )?;

  Ok(Calcit::from(vec![
    Calcit::Syntax(head.to_owned(), Arc::from(head_ns)),
    target_form,
    args.get(1).expect("declared unsafe-coerce type").to_owned(),
  ]))
}

pub fn preprocess_parse_cirru_edn_as(
  head: &CalcitSyntax,
  head_ns: &str,
  args: &CalcitList,
  ctx: &mut PreprocessContext,
) -> Result<Calcit, CalcitErr> {
  if args.len() != 2 {
    return Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Arity,
      format!("{head} expected a string and a type expression, got {}", args.len()),
      ctx.call_stack,
      args.first().and_then(Calcit::get_location),
    ));
  }

  let text_form = preprocess_expr(
    args.first().expect("validated parse-cirru-edn-as text"),
    ctx.scope_defs,
    ctx.scope_types,
    ctx.file_ns,
    ctx.check_warnings,
    ctx.call_stack,
  )?;
  if let Some(actual) = resolve_type_value(&text_form, ctx.scope_types)
    && !matches!(actual.as_ref(), CalcitTypeAnnotation::String | CalcitTypeAnnotation::Dynamic)
  {
    return Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Type,
      format!("{head} expected String input, got {}", actual.to_brief_string()),
      ctx.call_stack,
      text_form.get_location(),
    ));
  }
  let type_form = args.get(1).expect("validated parse-cirru-edn-as type");
  let target = CalcitTypeAnnotation::parse_type_annotation_form_with_generics(type_form, &[]);
  let decoder = crate::calcit::data_shape::DataShapeGraph::build(target.as_ref(), ctx.file_ns).map_err(|error| {
    CalcitErr::use_msg_stack_location(
      CalcitErrKind::Type,
      format!("{head} cannot derive a decoder: {error}"),
      ctx.call_stack,
      type_form.get_location(),
    )
  })?;

  Ok(Calcit::from(vec![
    Calcit::Syntax(head.to_owned(), Arc::from(head_ns)),
    text_form,
    type_form.to_owned(),
    decoder.into_calcit_handle(),
  ]))
}

pub fn preprocess_decode_map_as(
  head: &CalcitSyntax,
  head_ns: &str,
  args: &CalcitList,
  ctx: &mut PreprocessContext,
) -> Result<Calcit, CalcitErr> {
  if args.len() != 2 {
    return Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Arity,
      format!("{head} expected a value and a type expression, got {}", args.len()),
      ctx.call_stack,
      args.first().and_then(Calcit::get_location),
    ));
  }
  let value_form = preprocess_expr(
    args.first().expect("validated decode-map-as value"),
    ctx.scope_defs,
    ctx.scope_types,
    ctx.file_ns,
    ctx.check_warnings,
    ctx.call_stack,
  )?;
  let type_form = args.get(1).expect("validated decode-map-as type");
  let target = CalcitTypeAnnotation::parse_type_annotation_form_with_generics(type_form, &[]);
  let decoder = crate::calcit::data_shape::DataShapeGraph::build_open(target.as_ref(), ctx.file_ns).map_err(|error| {
    CalcitErr::use_msg_stack_location(
      CalcitErrKind::Type,
      format!("{head} cannot derive a runtime map decoder: {error}"),
      ctx.call_stack,
      type_form.get_location(),
    )
  })?;
  Ok(Calcit::from(vec![
    Calcit::Syntax(head.to_owned(), Arc::from(head_ns)),
    value_form,
    type_form.to_owned(),
    decoder.into_calcit_handle(),
  ]))
}

pub fn preprocess_assert_traits(
  head: &CalcitSyntax,
  _head_ns: &str,
  args: &CalcitList,
  ctx: &mut PreprocessContext,
) -> Result<Calcit, CalcitErr> {
  if args.len() < 2 {
    return Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Arity,
      format!(
        "assert-traits expected an expression and one or more trait definitions, got {head} with {} argument(s).",
        args.len()
      ),
      ctx.call_stack,
      args.first().and_then(|node| node.get_location()),
    ));
  }

  let target_raw = args.get(0).unwrap();
  let trait_forms = args.iter().skip(1).collect::<Vec<_>>();

  let target_form = preprocess_expr(
    target_raw,
    ctx.scope_defs,
    ctx.scope_types,
    ctx.file_ns,
    ctx.check_warnings,
    ctx.call_stack,
  )?;
  let local_opt = match &target_form {
    Calcit::Local(local) => Some(local.to_owned()),
    _ => None,
  };

  let mut trait_defs: Vec<Arc<CalcitTrait>> = vec![];
  let mut fallback_entry: Option<Arc<CalcitTypeAnnotation>> = None;

  for trait_form in trait_forms.iter() {
    let parsed_entry = CalcitTypeAnnotation::parse_type_annotation_form(trait_form);
    if let CalcitTypeAnnotation::Trait(trait_def) = parsed_entry.as_ref() {
      trait_defs.push(trait_def.to_owned());
      continue;
    }

    let resolved = match trait_form {
      Calcit::Symbol { sym, info, .. } => match runner::parse_ns_def(sym) {
        Some((ns_part, def_part)) => lookup_trait_ns_def_for_preprocess(&ns_part, &def_part, ctx.check_warnings, ctx.call_stack)
          .ok()
          .flatten(),
        None => lookup_trait_ns_def_for_preprocess(&info.at_ns, sym, ctx.check_warnings, ctx.call_stack)
          .ok()
          .flatten(),
      },
      Calcit::Import(import) => lookup_trait_ns_def_for_preprocess(&import.ns, &import.def, ctx.check_warnings, ctx.call_stack)
        .ok()
        .flatten(),
      _ => None,
    };

    if let Some(trait_def) = resolved {
      trait_defs.push(trait_def);
    } else if fallback_entry.is_none() {
      fallback_entry = Some(Arc::new(CalcitTypeAnnotation::Custom(Arc::new((*trait_form).to_owned()))));
    }
  }

  let mut assert_target = target_form;

  if let Some(local) = local_opt {
    let existing_entry = ctx.scope_types.get(&local.sym).cloned().or_else(|| {
      if matches!(*local.type_info, CalcitTypeAnnotation::Dynamic) {
        None
      } else {
        Some(local.type_info.clone())
      }
    });

    let resolved_entry = if let Some(existing) = existing_entry.as_ref() {
      let ann = existing.as_ref();
      if !is_dynamic_annotation(ann) && !is_trait_annotation(ann) {
        existing.clone()
      } else if let Some(fallback) = fallback_entry.as_ref() {
        let fallback_ann = fallback.as_ref();
        if !is_dynamic_annotation(fallback_ann) && !is_trait_annotation(fallback_ann) {
          fallback.clone()
        } else if !trait_defs.is_empty() {
          if trait_defs.len() == 1 {
            Arc::new(CalcitTypeAnnotation::Trait(trait_defs.remove(0)))
          } else {
            Arc::new(CalcitTypeAnnotation::TraitSet(Arc::new(trait_defs)))
          }
        } else {
          fallback.clone()
        }
      } else if !trait_defs.is_empty() {
        if trait_defs.len() == 1 {
          Arc::new(CalcitTypeAnnotation::Trait(trait_defs.remove(0)))
        } else {
          Arc::new(CalcitTypeAnnotation::TraitSet(Arc::new(trait_defs)))
        }
      } else {
        crate::calcit::DYNAMIC_TYPE.clone()
      }
    } else if let Some(fallback) = fallback_entry.as_ref() {
      let fallback_ann = fallback.as_ref();
      if !is_dynamic_annotation(fallback_ann) && !is_trait_annotation(fallback_ann) {
        fallback.clone()
      } else if !trait_defs.is_empty() {
        if trait_defs.len() == 1 {
          Arc::new(CalcitTypeAnnotation::Trait(trait_defs.remove(0)))
        } else {
          Arc::new(CalcitTypeAnnotation::TraitSet(Arc::new(trait_defs)))
        }
      } else {
        fallback.clone()
      }
    } else if !trait_defs.is_empty() {
      if trait_defs.len() == 1 {
        Arc::new(CalcitTypeAnnotation::Trait(trait_defs.remove(0)))
      } else {
        Arc::new(CalcitTypeAnnotation::TraitSet(Arc::new(trait_defs)))
      }
    } else {
      crate::calcit::DYNAMIC_TYPE.clone()
    };

    ctx.scope_types.insert(local.sym.to_owned(), resolved_entry.clone());

    let mut typed_local = local.to_owned();
    typed_local.type_info = resolved_entry;
    assert_target = Calcit::Local(typed_local);
  }

  let mut assert_expr: Calcit = assert_target;
  for trait_form in trait_forms.iter() {
    let trait_value = preprocess_expr(
      trait_form,
      ctx.scope_defs,
      ctx.scope_types,
      ctx.file_ns,
      ctx.check_warnings,
      ctx.call_stack,
    )?;
    assert_expr = Calcit::from(vec![Calcit::Proc(CalcitProc::NativeAssertTraits), assert_expr, trait_value]);
  }

  Ok(assert_expr)
}

fn analyze_def_schema_param_shape(args: &CalcitList) -> ParamShape {
  ParamShape::from_tokens(args.iter().map(|item| match item {
    Calcit::Syntax(CalcitSyntax::ArgOptional, _) => ParamShapeToken::OptionalMark,
    Calcit::Syntax(CalcitSyntax::ArgSpread, _) => ParamShapeToken::RestMark,
    _ => ParamShapeToken::Binding,
  }))
}

fn contains_legacy_optional(annotation: &CalcitTypeAnnotation) -> bool {
  match annotation {
    CalcitTypeAnnotation::Optional(_) => true,
    CalcitTypeAnnotation::List(inner)
    | CalcitTypeAnnotation::Set(inner)
    | CalcitTypeAnnotation::Ref(inner)
    | CalcitTypeAnnotation::Variadic(inner)
    | CalcitTypeAnnotation::JsNullish(inner) => contains_legacy_optional(inner),
    CalcitTypeAnnotation::Map(key, value) => contains_legacy_optional(key) || contains_legacy_optional(value),
    CalcitTypeAnnotation::Fn(info) => {
      info.arg_types.iter().any(|item| contains_legacy_optional(item))
        || info.rest_type.as_ref().is_some_and(|item| contains_legacy_optional(item))
        || contains_legacy_optional(info.return_type.as_ref())
    }
    CalcitTypeAnnotation::Macro(signature) => {
      let contract_contains = |contract: &MacroSyntaxType| match contract {
        MacroSyntaxType::Expr(semantic) => contains_legacy_optional(semantic),
        MacroSyntaxType::Syntax | MacroSyntaxType::SyntaxSymbol | MacroSyntaxType::SyntaxList => false,
      };
      signature.required_inputs.iter().any(contract_contains)
        || signature.optional_inputs.iter().any(contract_contains)
        || signature.rest_input.as_ref().is_some_and(contract_contains)
        || match &signature.expansion {
          MacroExpansionType::Expr(semantic) | MacroExpansionType::Definition(semantic) => contains_legacy_optional(semantic),
          MacroExpansionType::Dynamic | MacroExpansionType::Declarations => false,
        }
    }
    CalcitTypeAnnotation::Syntax(contract) => match contract.as_ref() {
      MacroSyntaxType::Expr(semantic) => contains_legacy_optional(semantic),
      MacroSyntaxType::Syntax | MacroSyntaxType::SyntaxSymbol | MacroSyntaxType::SyntaxList => false,
    },
    CalcitTypeAnnotation::Struct(_, args) | CalcitTypeAnnotation::Enum(_, args) | CalcitTypeAnnotation::TypeRef(_, args) => {
      args.iter().any(|item| contains_legacy_optional(item))
    }
    _ => false,
  }
}

fn warn_on_legacy_optional_public_schema(
  ns: &str,
  def_name: &str,
  schema: &CalcitTypeAnnotation,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  if !contains_legacy_optional(schema) {
    return;
  }
  // Core `&` definitions are semver-private primitives and may still model
  // nullable host/runtime values. Public core wrappers must obey the same
  // nominal absence rule as application code. `optionally` is the one explicit
  // bridge from an internal Optional<T> value to Option<T>.
  if ns == calcit::CORE_NS && (def_name.starts_with('&') || def_name == "optionally") {
    return;
  }
  gen_check_warning_code(
    format!(
      "[Warn] {ns}/{def_name} exposes legacy Optional<T> in its function schema; use Option<T> for Calcit absence, JsNullish<T> only for JavaScript FFI, Result<T,E> for failures, or Unit for effects"
    ),
    "W_LEGACY_OPTIONAL_SCHEMA",
    ns,
    check_warnings,
  );
}

fn validate_def_schema_during_preprocess(
  head: &CalcitSyntax,
  ns: &str,
  def_name: &str,
  args: &CalcitList,
  schema: &CalcitTypeAnnotation,
) -> Vec<String> {
  if let CalcitTypeAnnotation::Macro(signature) = schema {
    if !matches!(head, CalcitSyntax::Defmacro) {
      let code_kind = match head {
        CalcitSyntax::Defn => "defn",
        CalcitSyntax::DefWasmExport => "defwasm-export",
        CalcitSyntax::DefWasmImport => "defwasm-import",
        _ => "non-macro definition",
      };
      return vec![format!(
        "[E_SCHEMA_KIND] {ns}/{def_name}: schema :kind is :macro but code uses {code_kind}"
      )];
    }
    let code_shape = analyze_def_schema_param_shape(args);
    let schema_shape = ParamShape {
      required: signature.required_inputs.len(),
      optional: signature.optional_inputs.len(),
      has_rest: signature.rest_input.is_some(),
      errors: vec![],
    };
    return compare_param_shapes(&format!("{ns}/{def_name}"), &code_shape, &schema_shape);
  }
  let CalcitTypeAnnotation::Fn(fn_annot) = schema else {
    return vec![];
  };

  let code_kind = match head {
    CalcitSyntax::Defn => "defn",
    CalcitSyntax::DefWasmExport => "defwasm-export",
    CalcitSyntax::DefWasmImport => "defwasm-import",
    CalcitSyntax::Defmacro => "defmacro",
    _ => return vec![],
  };

  let mut issues: Vec<String> = vec![];

  match (fn_annot.fn_kind, code_kind) {
    (SchemaKind::Fn, "defmacro") => {
      issues.push(format!(
        "[E_SCHEMA_KIND] {ns}/{def_name}: schema :kind is :fn but code uses defmacro"
      ));
    }
    (SchemaKind::Macro, "defn" | "defwasm-export" | "defwasm-import") => {
      issues.push(format!(
        "[E_SCHEMA_KIND] {ns}/{def_name}: schema :kind is :macro but code uses {code_kind}"
      ));
    }
    _ => {}
  }

  let mut code_shape = analyze_def_schema_param_shape(args);
  let mut schema_shape = ParamShape::from_schema(&fn_annot.arg_types, fn_annot.rest_type.is_some());
  if code_kind != "defmacro" {
    code_shape = code_shape.as_fixed_arity();
    schema_shape = schema_shape.as_fixed_arity();
  }
  issues.extend(compare_param_shapes(&format!("{ns}/{def_name}"), &code_shape, &schema_shape));

  issues
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::{
    CalcitEnumDef, CalcitFn, CalcitFnArgs, CalcitFnUsageMeta, CalcitImport, CalcitMacro, CalcitScope, CalcitStructDef,
    CalcitStructValue, ImportInfo,
  };
  use crate::data::cirru::code_to_calcit;
  use cirru_parser::Cirru;
  use std::sync::{LazyLock, Mutex};

  static PREPROCESS_TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

  fn strict_macro_signature(
    required_inputs: Vec<MacroSyntaxType>,
    optional_inputs: Vec<MacroSyntaxType>,
    rest_input: Option<MacroSyntaxType>,
    expansion: MacroExpansionType,
  ) -> MacroSignature {
    MacroSignature {
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(vec![]),
      required_inputs: Arc::new(required_inputs),
      optional_inputs: Arc::new(optional_inputs),
      rest_input,
      expansion,
      capabilities: Arc::new(HashSet::new()),
      features: Arc::new(HashSet::new()),
    }
  }

  fn test_symbol(name: &str) -> Calcit {
    Calcit::Symbol {
      sym: Arc::from(name),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.macro-signature"),
        at_def: Arc::from("demo"),
      }),
      location: Some(Arc::from(vec![1, 2, 3])),
    }
  }

  #[test]
  fn lowers_valid_core_let_pairs_to_nested_core_let_forms() {
    let pair_a = Calcit::from(vec![test_symbol("a"), Calcit::Number(1.0)]);
    let pair_b = Calcit::from(vec![test_symbol("b"), Calcit::Number(2.0)]);
    let pairs = Calcit::from(vec![pair_a.clone(), pair_b.clone()]);
    let body = test_symbol("b");
    let args = CalcitList::from(&[pairs, body] as &[Calcit]);

    let lowered = try_lower_core_let_macro(&args, "tests.core-let").expect("valid pairs use native lowering");
    let Calcit::List(outer) = lowered else {
      panic!("expected outer core let")
    };
    assert!(matches!(outer.first(), Some(Calcit::Syntax(CalcitSyntax::CoreLet, _))));
    assert_eq!(outer.get(1), Some(&pair_a));
    let Some(Calcit::List(inner)) = outer.get(2) else {
      panic!("multiple pairs should nest core lets")
    };
    assert!(matches!(inner.first(), Some(Calcit::Syntax(CalcitSyntax::CoreLet, _))));
    assert_eq!(inner.get(1), Some(&pair_b));
  }

  #[test]
  fn leaves_malformed_core_let_pairs_on_the_general_macro_path() {
    for pair in [
      Calcit::from(vec![test_symbol("x")]),
      Calcit::from(vec![test_symbol("x"), Calcit::Number(1.0), Calcit::Number(2.0)]),
      Calcit::from(vec![Calcit::Number(1.0), Calcit::Number(2.0)]),
    ] {
      let pairs = Calcit::from(vec![pair]);
      let args = CalcitList::from(&[pairs, Calcit::Number(2.0)] as &[Calcit]);
      assert!(try_lower_core_let_macro(&args, "tests.core-let").is_none());
    }

    let empty_pair = Calcit::from(CalcitList::default());
    let args = CalcitList::from(&[Calcit::from(vec![empty_pair]), Calcit::Number(2.0)] as &[Calcit]);
    assert!(try_lower_core_let_macro(&args, "tests.core-let").is_some());
  }

  #[test]
  fn lowers_valid_core_map_pairs_to_the_flat_native_map_constructor() {
    let pair_a = Calcit::from(vec![Calcit::Tag(EdnTag::from("a")), Calcit::Number(1.0)]);
    let pair_b = Calcit::from(vec![Calcit::Tag(EdnTag::from("b")), Calcit::Number(2.0)]);
    let args = CalcitList::from(&[pair_a, pair_b] as &[Calcit]);
    let lowered = try_lower_core_map_macro(&args).expect("valid pairs use native lowering");
    let Calcit::List(items) = lowered else {
      panic!("expected native map call")
    };
    assert!(matches!(items.first(), Some(Calcit::Proc(CalcitProc::NativeMap))));
    assert_eq!(items.len(), 5);
    assert_eq!(items.get(1), Some(&Calcit::Tag(EdnTag::from("a"))));
    assert_eq!(items.get(4), Some(&Calcit::Number(2.0)));
  }

  #[test]
  fn leaves_malformed_core_map_pairs_on_the_general_macro_path() {
    for pair in [Calcit::Number(1.0), Calcit::from(vec![Calcit::Tag(EdnTag::from("a"))])] {
      let args = CalcitList::from(&[pair] as &[Calcit]);
      assert!(try_lower_core_map_macro(&args).is_none());
    }
    assert!(try_lower_core_map_macro(&CalcitList::default()).is_some());
  }

  #[test]
  fn phase_aware_macro_inputs_distinguish_symbols_lists_quotes_and_expr_generics() {
    let signature = strict_macro_signature(
      vec![
        MacroSyntaxType::SyntaxSymbol,
        MacroSyntaxType::Expr(Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")))),
      ],
      vec![MacroSyntaxType::SyntaxList],
      Some(MacroSyntaxType::Syntax),
      MacroExpansionType::Expr(Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")))),
    );
    let syntax_list = Calcit::from(vec![test_symbol("f"), Calcit::Number(1.0)]);
    let quoted_literal = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::Quote, Arc::from("tests.macro-signature")),
      test_symbol("literal"),
    ]);
    let args = CalcitList::from(vec![test_symbol("name"), Calcit::Number(1.0), syntax_list, quoted_literal].as_slice());
    let bindings = validate_macro_call_inputs(
      "typed-macro",
      &signature,
      &args,
      &ScopeTypes::new(),
      &CallStackList::default(),
      None,
    )
    .expect("valid syntax contracts");
    assert!(matches!(bindings.get("T").map(Arc::as_ref), Some(CalcitTypeAnnotation::Number)));

    let invalid = CalcitList::from(vec![Calcit::from(vec![test_symbol("quoted")]), Calcit::Number(1.0)].as_slice());
    let err = validate_macro_call_inputs(
      "typed-macro",
      &signature,
      &invalid,
      &ScopeTypes::new(),
      &CallStackList::default(),
      None,
    )
    .expect_err("a list is not symbol syntax");
    assert_eq!(err.code.as_deref(), Some("E_MACRO_INPUT_SYNTAX"));
    assert!(err.msg.contains("input-syntax violation"));

    validate_macro_expansion_result(
      "typed-macro",
      &signature,
      (&Calcit::Number(2.0), &Calcit::Number(2.0)),
      &ScopeTypes::new(),
      bindings.clone(),
      &CallStackList::default(),
      None,
    )
    .expect("generic expansion type follows Expr<T> input");
    let err = validate_macro_expansion_result(
      "typed-macro",
      &signature,
      (&Calcit::Str(Arc::from("wrong")), &Calcit::Str(Arc::from("wrong"))),
      &ScopeTypes::new(),
      bindings,
      &CallStackList::default(),
      None,
    )
    .expect_err("wrong expansion result type");
    assert_eq!(err.code.as_deref(), Some("E_MACRO_EXPANSION_EXPR_TYPE"));
    assert!(err.msg.contains("expansion-result violation"));
  }

  #[test]
  fn strict_macro_rest_body_binding_is_a_list_of_declared_syntax() {
    let rest_contract = MacroSyntaxType::Expr(Arc::new(CalcitTypeAnnotation::Dynamic));
    let signature = strict_macro_signature(vec![], vec![], Some(rest_contract.clone()), MacroExpansionType::Dynamic);
    let parameter_types = strict_macro_body_parameter_types(&signature);
    assert!(matches!(
      parameter_types.as_slice(),
      [item]
        if matches!(
          item.as_ref(),
          CalcitTypeAnnotation::List(inner)
            if matches!(inner.as_ref(), CalcitTypeAnnotation::Syntax(contract) if contract.as_ref() == &rest_contract)
        )
    ));

    let list_signature = strict_macro_signature(
      vec![MacroSyntaxType::SyntaxList, MacroSyntaxType::SyntaxSymbol],
      vec![],
      None,
      MacroExpansionType::Dynamic,
    );
    let parameter_types = strict_macro_body_parameter_types(&list_signature);
    assert!(matches!(parameter_types[0].as_ref(), CalcitTypeAnnotation::List(_)));
    assert!(matches!(parameter_types[1].as_ref(), CalcitTypeAnnotation::Symbol));
  }

  #[test]
  fn phase_aware_macro_definition_and_declarations_outputs_are_distinct() {
    let definition = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::Defn, Arc::from("tests.macro-signature")),
      test_symbol("generated"),
      Calcit::from(vec![]),
      Calcit::Number(1.0),
    ]);
    let definition_signature = strict_macro_signature(
      vec![],
      vec![],
      None,
      MacroExpansionType::Definition(Arc::new(CalcitTypeAnnotation::Dynamic)),
    );
    validate_macro_expansion_result(
      "define-one",
      &definition_signature,
      (&definition, &definition),
      &ScopeTypes::new(),
      HashMap::new(),
      &CallStackList::default(),
      None,
    )
    .expect("definition output");

    let declarations_signature = strict_macro_signature(vec![], vec![], None, MacroExpansionType::Declarations);
    let err = validate_macro_expansion_result(
      "define-many",
      &declarations_signature,
      (&Calcit::Number(1.0), &Calcit::Number(1.0)),
      &ScopeTypes::new(),
      HashMap::new(),
      &CallStackList::default(),
      None,
    )
    .expect_err("expression is not declarations");
    assert_eq!(err.code.as_deref(), Some("E_MACRO_EXPANSION_DECLARATIONS"));
  }

  fn lock_preprocess_test_state() -> std::sync::MutexGuard<'static, ()> {
    PREPROCESS_TEST_LOCK.lock().unwrap_or_else(|err| err.into_inner())
  }

  fn match_branch(tag: &str, body: &str) -> Calcit {
    Calcit::from(vec![
      Calcit::from(vec![Calcit::Tag(cirru_edn::EdnTag::from(tag))]),
      Calcit::Tag(cirru_edn::EdnTag::from(body)),
    ])
  }

  fn wildcard_match_branch(body: &str) -> Calcit {
    Calcit::from(vec![
      Calcit::Symbol {
        sym: Arc::from("_"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.match"),
          at_def: Arc::from("dispatch"),
        }),
        location: None,
      },
      Calcit::Tag(cirru_edn::EdnTag::from(body)),
    ])
  }

  fn indexed_match_enum() -> CalcitEnumDef {
    let fields = vec![
      cirru_edn::EdnTag::from("idle"),
      cirru_edn::EdnTag::from("running"),
      cirru_edn::EdnTag::from("done"),
    ];
    CalcitEnumDef::from_struct(CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef::from_fields(cirru_edn::EdnTag::from("State"), fields)),
      values: Arc::new(vec![Calcit::from(vec![]), Calcit::from(vec![]), Calcit::from(vec![])]),
    })
    .expect("valid enum")
  }

  #[test]
  fn indexed_match_table_uses_variant_order_and_declines_ambiguous_forms() {
    let enum_def = indexed_match_enum();
    let done = match_branch("done", "finished");
    let idle = match_branch("idle", "waiting");
    let wildcard = wildcard_match_branch("unknown");
    let table = build_indexed_match_table(&enum_def, &[done.clone(), idle.clone(), wildcard.clone()]).expect("indexed table");
    let Calcit::List(slots) = table else { panic!("expected table") };

    assert_eq!(slots[0], idle);
    assert!(matches!(slots[1], Calcit::Nil));
    assert_eq!(slots[2], done);
    assert_eq!(slots[3], wildcard);
    assert!(build_indexed_match_table(&enum_def, &[match_branch("idle", "first"), match_branch("idle", "second")]).is_none());
    assert!(build_indexed_match_table(&enum_def, &[wildcard_match_branch("early"), match_branch("done", "late")]).is_none());
    assert!(build_indexed_match_table(&enum_def, &[match_branch("missing", "unknown")]).is_none());
  }

  #[test]
  fn preprocessed_calls_use_contiguous_nodes() {
    let expr = Cirru::List(vec![
      Cirru::leaf("+"),
      Cirru::leaf("1"),
      Cirru::List(vec![Cirru::leaf("*"), Cirru::leaf("2"), Cirru::leaf("3")]),
    ]);
    let code = code_to_calcit(&expr, "tests.executable", "main", vec![]).expect("parse call");
    let warnings = RefCell::new(vec![]);

    let resolved = preprocess_expr(
      &code,
      &HashSet::new(),
      &mut ScopeTypes::new(),
      "tests.executable",
      &warnings,
      &CallStackList::default(),
    )
    .expect("preprocess call");

    let Calcit::List(outer) = resolved else {
      panic!("expected outer call")
    };
    assert!(matches!(outer.as_ref(), CalcitList::Call(_, CalcitCallKind::Normal)));
    assert!(matches!(
      outer.get(2),
      Some(Calcit::List(inner)) if matches!(inner.as_ref(), CalcitList::Call(_, CalcitCallKind::Normal))
    ));
  }

  #[test]
  fn executable_conversion_makes_syntax_contiguous_and_keeps_quoted_lists_persistent() {
    let quoted = Calcit::List(Arc::new(CalcitList::List(TernaryTreeList::from(vec![
      Calcit::Number(1.0),
      Calcit::Number(2.0),
    ]))));
    let code = Calcit::from(vec![Calcit::Syntax(CalcitSyntax::Quote, Arc::from("tests.executable")), quoted]);
    let warnings = RefCell::new(vec![]);

    let resolved = preprocess_expr(
      &code,
      &HashSet::new(),
      &mut ScopeTypes::new(),
      "tests.executable",
      &warnings,
      &CallStackList::default(),
    )
    .expect("preprocess quote");

    let Calcit::List(outer) = resolved else {
      panic!("expected quote call")
    };
    assert!(matches!(outer.as_ref(), CalcitList::Call(_, CalcitCallKind::Normal)));
    assert!(matches!(
      outer.get(1),
      Some(Calcit::List(inner)) if matches!(inner.as_ref(), CalcitList::List(_))
    ));
  }

  #[test]
  fn classifies_exact_number_binary_native_calls() {
    let typed_local = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from("typed-number")),
      sym: Arc::from("typed-number"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.executable"),
        at_def: Arc::from("main"),
      }),
      location: None,
      type_info: Arc::new(CalcitTypeAnnotation::Number),
    });
    let args = [typed_local, Calcit::Number(1.0)];

    for (proc, operation) in [
      (CalcitProc::NativeAdd, CalcitNumberBinaryOp::Add),
      (CalcitProc::NativeMinus, CalcitNumberBinaryOp::Subtract),
      (CalcitProc::NativeMultiply, CalcitNumberBinaryOp::Multiply),
      (CalcitProc::NativeDivide, CalcitNumberBinaryOp::Divide),
      (CalcitProc::NativeNumberRem, CalcitNumberBinaryOp::Remainder),
      (CalcitProc::NativeLessThan, CalcitNumberBinaryOp::LessThan),
      (CalcitProc::NativeGreaterThan, CalcitNumberBinaryOp::GreaterThan),
    ] {
      assert_eq!(
        classify_number_binary_call(&Calcit::Proc(proc), &args, &ScopeTypes::new()),
        CalcitCallKind::NumberBinary(operation)
      );
    }
  }

  #[test]
  fn preprocessed_number_binary_call_carries_static_operation() {
    let expr = Cirru::List(vec![Cirru::leaf("&+"), Cirru::leaf("1"), Cirru::leaf("2")]);
    let code = code_to_calcit(&expr, "tests.executable", "main", vec![]).expect("parse native number call");
    let warnings = RefCell::new(vec![]);

    let resolved = preprocess_expr(
      &code,
      &HashSet::new(),
      &mut ScopeTypes::new(),
      "tests.executable",
      &warnings,
      &CallStackList::default(),
    )
    .expect("preprocess native number call");

    let Calcit::List(call) = resolved else {
      panic!("expected executable call")
    };
    assert_eq!(call.call_kind(), CalcitCallKind::NumberBinary(CalcitNumberBinaryOp::Add));
  }

  #[test]
  fn statically_inlined_method_call_retains_number_operation_metadata() {
    let typed_local = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from("typed-number")),
      sym: Arc::from("typed-number"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.executable"),
        at_def: Arc::from("main"),
      }),
      location: None,
      type_info: Arc::new(CalcitTypeAnnotation::Number),
    });
    let args = CalcitList::from(&[typed_local, Calcit::Number(3.0)] as &[Calcit]);

    let resolved = build_inlined_call(Calcit::Proc(CalcitProc::NativeNumberRem), &args, &ScopeTypes::new());
    let Calcit::List(call) = resolved else {
      panic!("expected executable call")
    };

    assert!(matches!(
      call.as_ref(),
      CalcitList::Call(_, CalcitCallKind::NumberBinary(CalcitNumberBinaryOp::Remainder))
    ));
  }

  #[test]
  fn statically_inlined_method_call_keeps_dynamic_arguments_on_normal_dispatch() {
    let dynamic_local = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from("dynamic-number")),
      sym: Arc::from("dynamic-number"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.executable"),
        at_def: Arc::from("main"),
      }),
      location: None,
      type_info: calcit::DYNAMIC_TYPE.clone(),
    });
    let args = CalcitList::from(&[dynamic_local, Calcit::Number(3.0)] as &[Calcit]);

    let resolved = build_inlined_call(Calcit::Proc(CalcitProc::NativeNumberRem), &args, &ScopeTypes::new());
    let Calcit::List(call) = resolved else {
      panic!("expected executable call")
    };

    assert!(matches!(call.as_ref(), CalcitList::Call(_, CalcitCallKind::Normal)));
  }

  #[test]
  fn leaves_dynamic_and_non_binary_native_calls_unspecialized() {
    let dynamic_local = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from("dynamic-number")),
      sym: Arc::from("dynamic-number"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.executable"),
        at_def: Arc::from("main"),
      }),
      location: None,
      type_info: crate::calcit::DYNAMIC_TYPE.clone(),
    });
    let dynamic_args = [dynamic_local, Calcit::Number(1.0)];
    let unary_args = [Calcit::Number(1.0)];

    assert_eq!(
      classify_number_binary_call(&Calcit::Proc(CalcitProc::NativeAdd), &dynamic_args, &ScopeTypes::new()),
      CalcitCallKind::Normal
    );
    assert_eq!(
      classify_number_binary_call(&Calcit::Proc(CalcitProc::NativeAdd), &unary_args, &ScopeTypes::new()),
      CalcitCallKind::Normal
    );
  }

  #[test]
  fn expands_only_fully_typed_literal_path_calls() {
    let _guard = lock_preprocess_test_state();
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let nested_map = Arc::new(CalcitTypeAnnotation::Map(
      Arc::new(CalcitTypeAnnotation::Tag),
      Arc::new(CalcitTypeAnnotation::Map(Arc::new(CalcitTypeAnnotation::Tag), number)),
    ));
    let typed_base = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from("typed-path-base")),
      sym: Arc::from("typed-path-base"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.typed-path"),
        at_def: Arc::from("main"),
      }),
      location: None,
      type_info: nested_map,
    });
    let literal_path = generated_call(vec![Calcit::Proc(CalcitProc::List), Calcit::tag("user"), Calcit::tag("score")]);
    let stack = CallStackList::default();

    let get_args = CalcitList::from(&[typed_base.to_owned(), literal_path.to_owned()] as &[Calcit]);
    let expanded_get = try_expand_typed_literal_path_call(
      &core_import("get-in", "tests.typed-path"),
      &get_args,
      &ScopeTypes::new(),
      "tests.typed-path",
      &stack,
    )
    .expect("build get-in expansion")
    .expect("typed get-in expansion");
    let get_code = expanded_get.lisp_str();
    assert!(get_code.contains("match"));
    assert!(get_code.contains("get-in does not traverse Struct fields"));
    assert!(!get_code.contains("calcit.core/get-in"));
    assert_eq!(get_code.matches("typed-path-base").count(), 1, "caller base is evaluated once");
    assert_eq!(get_code.matches(":user").count(), 1, "first path expression is evaluated once");
    assert_eq!(get_code.matches(":score").count(), 1, "second path expression is evaluated once");

    let assoc_args = CalcitList::from(&[
      typed_base.to_owned(),
      literal_path.to_owned(),
      Calcit::Str(Arc::from("typed-path-value-input")),
    ] as &[Calcit]);
    let expanded_assoc = try_expand_typed_literal_path_call(
      &core_import("assoc-in", "tests.typed-path"),
      &assoc_args,
      &ScopeTypes::new(),
      "tests.typed-path",
      &stack,
    )
    .expect("build assoc-in expansion")
    .expect("typed assoc-in expansion");
    let assoc_code = expanded_assoc.lisp_str();
    assert!(assoc_code.contains("&map:contains?"));
    assert!(assoc_code.contains("assoc-in does not traverse Struct fields"));
    assert!(!assoc_code.contains("calcit.core/assoc-in"));
    assert_eq!(assoc_code.matches("typed-path-base").count(), 1, "caller base is evaluated once");
    assert_eq!(assoc_code.matches(":user").count(), 1, "first path expression is evaluated once");
    assert_eq!(assoc_code.matches(":score").count(), 1, "second path expression is evaluated once");
    assert_eq!(
      assoc_code.matches("typed-path-value-input").count(),
      1,
      "replacement is evaluated once"
    );
    assert!(
      assoc_code.find(":score").expect("second path expression")
        < assoc_code.find("typed-path-value-input").expect("replacement expression"),
      "path expressions are evaluated before the replacement"
    );

    let dynamic_base = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from("dynamic-path-base")),
      sym: Arc::from("dynamic-path-base"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.typed-path"),
        at_def: Arc::from("main"),
      }),
      location: None,
      type_info: calcit::DYNAMIC_TYPE.clone(),
    });
    let dynamic_args = CalcitList::from(&[dynamic_base, literal_path] as &[Calcit]);
    assert!(
      try_expand_typed_literal_path_call(
        &core_import("get-in", "tests.typed-path"),
        &dynamic_args,
        &ScopeTypes::new(),
        "tests.typed-path",
        &stack,
      )
      .expect("dynamic path decision")
      .is_none()
    );
  }

  #[test]
  fn generated_path_guards_suppress_only_their_synthetic_warnings() {
    let _guard = lock_preprocess_test_state();
    let option_number = Arc::new(CalcitTypeAnnotation::TypeRef(
      Arc::from("calcit.core/Option"),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]),
    ));
    let option_value = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from("optional-value")),
      sym: Arc::from("optional-value"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.typed-path"),
        at_def: Arc::from("main"),
      }),
      location: None,
      type_info: option_number,
    });
    let guard = generated_call(vec![Calcit::Proc(CalcitProc::NilQuestion), option_value]);
    let direct_warnings = RefCell::new(vec![]);
    let mut direct_scope_types = ScopeTypes::new();
    preprocess_expr(
      &guard,
      &HashSet::new(),
      &mut direct_scope_types,
      "tests.typed-path",
      &direct_warnings,
      &CallStackList::default(),
    )
    .expect("preprocess synthetic guard with the normal warning sink");
    assert_eq!(direct_warnings.borrow().len(), 1);
    assert_eq!(direct_warnings.borrow()[0].code(), Some("W_NOMINAL_ENUM_LEGACY_USE"));

    let mut generated_scope_types = ScopeTypes::new();
    preprocess_generated_path_expansion(
      &guard,
      &HashSet::new(),
      &mut generated_scope_types,
      "tests.typed-path",
      &CallStackList::default(),
    )
    .expect("preprocess synthetic guard with its private warning sink");
  }

  #[test]
  fn removed_data_apis_point_to_their_struct_enum_replacements() {
    let cases = [
      ("tuple?", "enum? (values) or enum-def? (definitions)"),
      ("tuple-enum", "enum-definition"),
      ("&record:get", "&struct:get"),
      ("&record:struct", "&struct:definition"),
      ("&tuple:nth", "&enum:nth"),
      ("&tuple:enum", "&enum:definition"),
      ("&tuple:enum-has-variant?", "&enum-def:has-variant?"),
      ("&tuple:enum-variant-arity", "&enum-def:variant-arity"),
      ("&tuple:validate-enum", "&enum:validate"),
    ];

    for (legacy, replacement) in cases {
      assert_eq!(removed_data_api_replacement(legacy).as_deref(), Some(replacement));
    }
    assert_eq!(removed_data_api_replacement("&map:get"), None);
  }

  #[test]
  fn js_nullish_ffi_values_require_safe_dereference_and_dedicated_predicates() {
    let sym: Arc<str> = Arc::from("host");
    let receiver = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&sym),
      sym,
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.js-ffi"),
        at_def: Arc::from("demo"),
      }),
      location: None,
      type_info: Arc::new(CalcitTypeAnnotation::JsNullish(Arc::new(CalcitTypeAnnotation::JsObject))),
    });
    let args = CalcitList::from(std::slice::from_ref(&receiver));
    let warnings = RefCell::new(vec![]);

    warn_on_nullable_js_ffi_dereference(
      &Calcit::Method(Arc::from("read"), calcit::MethodKind::InvokeNative),
      &args,
      &ScopeTypes::new(),
      "tests.js-ffi",
      "demo",
      &warnings,
    );
    assert_eq!(warnings.borrow().len(), 1);
    assert_eq!(warnings.borrow()[0].code(), Some("W_JS_FFI_NULLABLE_DEREF"));

    let optional_warnings = RefCell::new(vec![]);
    warn_on_nullable_js_ffi_dereference(
      &Calcit::Method(Arc::from("read"), calcit::MethodKind::InvokeNativeOptional),
      &args,
      &ScopeTypes::new(),
      "tests.js-ffi",
      "demo",
      &optional_warnings,
    );
    assert!(optional_warnings.borrow().is_empty());

    let predicate_warnings = RefCell::new(vec![]);
    warn_on_legacy_js_nullish_predicate(
      &Calcit::Proc(CalcitProc::NilQuestion),
      &args,
      &ScopeTypes::new(),
      "tests.js-ffi",
      "demo",
      &predicate_warnings,
    );
    assert_eq!(predicate_warnings.borrow().len(), 1);
    assert_eq!(predicate_warnings.borrow()[0].code(), Some("W_JS_FFI_NULLABLE_PREDICATE"));

    let mut scope_types = ScopeTypes::new();
    scope_types.insert(
      Arc::from("host"),
      Arc::new(CalcitTypeAnnotation::JsNullish(Arc::new(CalcitTypeAnnotation::JsObject))),
    );
    let predicate = Calcit::from(vec![
      Calcit::Symbol {
        sym: Arc::from("js-present?"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.js-ffi"),
          at_def: Arc::from("demo"),
        }),
        location: None,
      },
      receiver,
    ]);
    let narrowing = extract_predicate_bindings(&predicate, &scope_types);
    assert!(matches!(
      narrowing.true_binding,
      Some((name, inferred)) if name.as_ref() == "host" && matches!(inferred.as_ref(), CalcitTypeAnnotation::JsObject)
    ));
  }

  #[test]
  fn nominal_options_warn_on_legacy_nil_and_enum_operations() {
    let core_head = |operation: &str| {
      Calcit::Import(CalcitImport {
        ns: Arc::from(calcit::CORE_NS),
        def: Arc::from(operation),
        info: Arc::new(ImportInfo::Core {
          at_ns: Arc::from("tests.option-migration"),
        }),
        def_id: None,
      })
    };
    let sym: Arc<str> = Arc::from("found");
    let option_value = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&sym),
      sym,
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.option-migration"),
        at_def: Arc::from("demo"),
      }),
      location: None,
      type_info: Arc::new(CalcitTypeAnnotation::TypeRef(
        Arc::from("calcit.core/Option"),
        Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]),
      )),
    });
    let args = CalcitList::from(std::slice::from_ref(&option_value));

    for operation in ["some?", "get", "assoc", "dissoc", "merge", "&compare", "struct?"] {
      let head = core_head(operation);
      let warnings = RefCell::new(vec![]);
      warn_on_nominal_enum_legacy_absence_use(&head, &args, &ScopeTypes::new(), "tests.option-migration", "demo", &warnings);
      assert_eq!(warnings.borrow().len(), 1, "{operation} should warn");
      assert_eq!(warnings.borrow()[0].code(), Some("W_NOMINAL_ENUM_LEGACY_USE"));
    }

    let method_warnings = RefCell::new(vec![]);
    warn_on_nominal_enum_legacy_absence_use(
      &Calcit::Method(Arc::from("count"), calcit::MethodKind::Invoke(calcit::DYNAMIC_TYPE.clone())),
      &args,
      &ScopeTypes::new(),
      "tests.option-migration",
      "demo",
      &method_warnings,
    );
    assert_eq!(method_warnings.borrow().len(), 1, "structural Option methods should warn");
    assert!(method_warnings.borrow()[0].message().contains(".unwrap-or"));

    let direct_get = Calcit::from(vec![core_head("get"), Calcit::Nil, Calcit::Number(0.0)]);
    let direct_get_warnings = RefCell::new(vec![]);
    warn_on_nominal_enum_legacy_absence_use(
      &core_head("count"),
      &CalcitList::from(std::slice::from_ref(&direct_get)),
      &ScopeTypes::new(),
      "tests.option-migration",
      "demo",
      &direct_get_warnings,
    );
    assert_eq!(direct_get_warnings.borrow().len(), 1, "direct get payload misuse should warn");
    assert!(direct_get_warnings.borrow()[0].message().contains(".unwrap-or"));

    let equality_head = core_head("=");
    let mixed_equality_args = CalcitList::from(&[option_value.to_owned(), Calcit::Number(1.0)] as &[Calcit]);
    let mixed_equality_warnings = RefCell::new(vec![]);
    warn_on_nominal_enum_legacy_absence_use(
      &equality_head,
      &mixed_equality_args,
      &ScopeTypes::new(),
      "tests.option-migration",
      "demo",
      &mixed_equality_warnings,
    );
    assert_eq!(mixed_equality_warnings.borrow().len(), 1, "Option-to-payload equality should warn");

    let nominal_equality_args = CalcitList::from(&[option_value.to_owned(), option_value.to_owned()] as &[Calcit]);
    let nominal_equality_warnings = RefCell::new(vec![]);
    warn_on_nominal_enum_legacy_absence_use(
      &equality_head,
      &nominal_equality_args,
      &ScopeTypes::new(),
      "tests.option-migration",
      "demo",
      &nominal_equality_warnings,
    );
    assert!(
      nominal_equality_warnings.borrow().is_empty(),
      "Option-to-Option equality should stay valid"
    );

    let option_constructor = Calcit::from(vec![core_head("%some"), Calcit::Number(1.0)]);
    let constructor_equality_args = CalcitList::from(&[option_value.to_owned(), option_constructor.to_owned()] as &[Calcit]);
    let constructor_equality_warnings = RefCell::new(vec![]);
    warn_on_nominal_enum_legacy_absence_use(
      &equality_head,
      &constructor_equality_args,
      &ScopeTypes::new(),
      "tests.option-migration",
      "demo",
      &constructor_equality_warnings,
    );
    assert!(
      constructor_equality_warnings.borrow().is_empty(),
      "Option equality with a core Option constructor should stay valid"
    );

    let option_set_sym: Arc<str> = Arc::from("option-set");
    let option_set = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&option_set_sym),
      sym: option_set_sym,
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.option-migration"),
        at_def: Arc::from("demo"),
      }),
      location: None,
      type_info: Arc::new(CalcitTypeAnnotation::Set(Arc::new(CalcitTypeAnnotation::TypeRef(
        Arc::from("calcit.core/Option"),
        Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]),
      )))),
    });
    let option_membership_args = CalcitList::from(&[option_set.to_owned(), option_value.to_owned()] as &[Calcit]);
    let option_membership_warnings = RefCell::new(vec![]);
    warn_on_nominal_enum_legacy_absence_use(
      &core_head("includes?"),
      &option_membership_args,
      &ScopeTypes::new(),
      "tests.option-migration",
      "demo",
      &option_membership_warnings,
    );
    assert!(
      option_membership_warnings.borrow().is_empty(),
      "membership in a Set<Option<T>> should compare nominal Option values without warning"
    );

    let specialized_option_membership_warnings = RefCell::new(vec![]);
    warn_on_nominal_enum_legacy_absence_use(
      &Calcit::Proc(CalcitProc::NativeSetIncludes),
      &CalcitList::from(&[option_set, option_constructor.to_owned()] as &[Calcit]),
      &ScopeTypes::new(),
      "tests.option-migration",
      "demo",
      &specialized_option_membership_warnings,
    );
    assert!(
      specialized_option_membership_warnings.borrow().is_empty(),
      "the specialized Set membership proc should retain the Option membership exemption"
    );

    let literal_option_set = Calcit::from(vec![Calcit::Proc(CalcitProc::Set), option_constructor.to_owned()]);
    let literal_option_membership_warnings = RefCell::new(vec![]);
    warn_on_nominal_enum_legacy_absence_use(
      &core_head("includes?"),
      &CalcitList::from(&[literal_option_set, option_constructor.to_owned()] as &[Calcit]),
      &ScopeTypes::new(),
      "tests.option-migration",
      "demo",
      &literal_option_membership_warnings,
    );
    assert!(
      literal_option_membership_warnings.borrow().is_empty(),
      "a Set literal with Option elements should remain warning-free before generic return inference"
    );

    let literal_option_list = Calcit::from(vec![Calcit::Proc(CalcitProc::List), option_constructor.to_owned()]);
    let list_contains_option_warnings = RefCell::new(vec![]);
    warn_on_nominal_enum_legacy_absence_use(
      &core_head("contains?"),
      &CalcitList::from(&[literal_option_list, option_constructor.to_owned()] as &[Calcit]),
      &ScopeTypes::new(),
      "tests.option-migration",
      "demo",
      &list_contains_option_warnings,
    );
    assert_eq!(
      list_contains_option_warnings.borrow().len(),
      1,
      "List contains? checks an index, so it must not use the element-membership exemption"
    );

    let mixed_literal_option_set = Calcit::from(vec![
      Calcit::Proc(CalcitProc::Set),
      option_constructor.to_owned(),
      Calcit::Number(1.0),
    ]);
    let mixed_literal_option_membership_warnings = RefCell::new(vec![]);
    warn_on_nominal_enum_legacy_absence_use(
      &core_head("includes?"),
      &CalcitList::from(&[mixed_literal_option_set, option_constructor] as &[Calcit]),
      &ScopeTypes::new(),
      "tests.option-migration",
      "demo",
      &mixed_literal_option_membership_warnings,
    );
    assert_eq!(
      mixed_literal_option_membership_warnings.borrow().len(),
      1,
      "a mixed literal cannot prove Option membership is intentional"
    );

    let option_key_sym: Arc<str> = Arc::from("option-key-map");
    let option_key_map = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&option_key_sym),
      sym: option_key_sym,
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.option-migration"),
        at_def: Arc::from("demo"),
      }),
      location: None,
      type_info: Arc::new(CalcitTypeAnnotation::Map(
        Arc::new(CalcitTypeAnnotation::TypeRef(
          Arc::from("calcit.core/Option"),
          Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]),
        )),
        Arc::new(CalcitTypeAnnotation::String),
      )),
    });
    let option_key_map_args = CalcitList::from(&[option_key_map.to_owned(), option_value.to_owned()] as &[Calcit]);
    let option_key_map_warnings = RefCell::new(vec![]);
    warn_on_nominal_enum_legacy_absence_use(
      &core_head("contains?"),
      &option_key_map_args,
      &ScopeTypes::new(),
      "tests.option-migration",
      "demo",
      &option_key_map_warnings,
    );
    assert!(
      option_key_map_warnings.borrow().is_empty(),
      "Map<Option<T>, V> key membership via contains? should compare nominal Option keys without warning"
    );

    let option_value_sym: Arc<str> = Arc::from("option-value-map");
    let option_value_map = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&option_value_sym),
      sym: option_value_sym,
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.option-migration"),
        at_def: Arc::from("demo"),
      }),
      location: None,
      type_info: Arc::new(CalcitTypeAnnotation::Map(
        Arc::new(CalcitTypeAnnotation::String),
        Arc::new(CalcitTypeAnnotation::TypeRef(
          Arc::from("calcit.core/Option"),
          Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]),
        )),
      )),
    });
    let option_value_map_args = CalcitList::from(&[option_value_map.to_owned(), option_value.to_owned()] as &[Calcit]);
    let option_value_map_warnings = RefCell::new(vec![]);
    warn_on_nominal_enum_legacy_absence_use(
      &core_head("includes?"),
      &option_value_map_args,
      &ScopeTypes::new(),
      "tests.option-migration",
      "demo",
      &option_value_map_warnings,
    );
    assert!(
      option_value_map_warnings.borrow().is_empty(),
      "Map<K, Option<T>> value membership via includes? should compare nominal Option values without warning"
    );

    let reversed_option_key_map_warnings = RefCell::new(vec![]);
    warn_on_nominal_enum_legacy_absence_use(
      &core_head("includes?"),
      &CalcitList::from(&[option_key_map.to_owned(), option_value.to_owned()] as &[Calcit]),
      &ScopeTypes::new(),
      "tests.option-migration",
      "demo",
      &reversed_option_key_map_warnings,
    );
    assert_eq!(
      reversed_option_key_map_warnings.borrow().len(),
      1,
      "includes? on Map<Option<T>, V> checks the String value, so the Option key must not suppress the warning"
    );

    let reversed_option_value_map_warnings = RefCell::new(vec![]);
    warn_on_nominal_enum_legacy_absence_use(
      &core_head("contains?"),
      &CalcitList::from(&[option_value_map.to_owned(), option_value.to_owned()] as &[Calcit]),
      &ScopeTypes::new(),
      "tests.option-migration",
      "demo",
      &reversed_option_value_map_warnings,
    );
    assert_eq!(
      reversed_option_value_map_warnings.borrow().len(),
      1,
      "contains? on Map<K, Option<T>> checks the String key, so the Option value must not suppress the warning"
    );

    let specialized_option_key_map_warnings = RefCell::new(vec![]);
    warn_on_nominal_enum_legacy_absence_use(
      &Calcit::Proc(CalcitProc::NativeMapContains),
      &CalcitList::from(&[option_key_map.to_owned(), option_value.to_owned()] as &[Calcit]),
      &ScopeTypes::new(),
      "tests.option-migration",
      "demo",
      &specialized_option_key_map_warnings,
    );
    assert!(
      specialized_option_key_map_warnings.borrow().is_empty(),
      "the specialized Map contains proc should retain the Option key membership exemption"
    );

    let specialized_option_value_map_warnings = RefCell::new(vec![]);
    warn_on_nominal_enum_legacy_absence_use(
      &Calcit::Proc(CalcitProc::NativeMapIncludes),
      &CalcitList::from(&[option_value_map.to_owned(), option_value.to_owned()] as &[Calcit]),
      &ScopeTypes::new(),
      "tests.option-migration",
      "demo",
      &specialized_option_value_map_warnings,
    );
    assert!(
      specialized_option_value_map_warnings.borrow().is_empty(),
      "the specialized Map includes proc should retain the Option value membership exemption"
    );

    let application_get = Calcit::Symbol {
      sym: Arc::from("get"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.option-migration"),
        at_def: Arc::from("demo"),
      }),
      location: None,
    };
    let application_get_warnings = RefCell::new(vec![]);
    warn_on_nominal_enum_legacy_absence_use(
      &application_get,
      &args,
      &ScopeTypes::new(),
      "tests.option-migration",
      "demo",
      &application_get_warnings,
    );
    assert!(
      application_get_warnings.borrow().is_empty(),
      "application-defined get must not warn"
    );

    let application_option_sym: Arc<str> = Arc::from("application-option");
    let application_option = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&application_option_sym),
      sym: application_option_sym,
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.option-migration"),
        at_def: Arc::from("demo"),
      }),
      location: None,
      type_info: Arc::new(CalcitTypeAnnotation::TypeRef(
        Arc::from("app.model/Option"),
        Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]),
      )),
    });
    let application_option_args = CalcitList::from(std::slice::from_ref(&application_option));
    let application_option_warnings = RefCell::new(vec![]);
    warn_on_nominal_enum_legacy_absence_use(
      &core_head("some?"),
      &application_option_args,
      &ScopeTypes::new(),
      "tests.option-migration",
      "demo",
      &application_option_warnings,
    );
    assert!(
      application_option_warnings.borrow().is_empty(),
      "application-defined Option must not warn"
    );

    let truthiness_warnings = RefCell::new(vec![]);
    warn_on_nominal_enum_truthiness(&option_value, &ScopeTypes::new(), "tests.option-migration", &truthiness_warnings);
    assert_eq!(truthiness_warnings.borrow().len(), 1, "Option truthiness should warn");
    assert_eq!(truthiness_warnings.borrow()[0].code(), Some("W_NOMINAL_ENUM_LEGACY_USE"));
  }

  struct WarnDynMethodGuard {
    prev: bool,
  }

  impl WarnDynMethodGuard {
    fn new(enabled: bool) -> Self {
      let prev = warn_dyn_method_enabled();
      set_warn_dyn_method(enabled);
      Self { prev }
    }
  }

  impl Drop for WarnDynMethodGuard {
    fn drop(&mut self) {
      set_warn_dyn_method(self.prev);
    }
  }

  #[test]
  fn static_method_descriptors_follow_user_impl_precedence() {
    let low_impl = Arc::new(CalcitImpl {
      name: EdnTag::new("LowImpl"),
      origin: None,
      fields: Arc::new(vec![EdnTag::new("low"), EdnTag::new("shared")]),
      values: Arc::new(vec![Calcit::Nil, Calcit::Nil]),
    });
    let high_impl = Arc::new(CalcitImpl {
      name: EdnTag::new("HighImpl"),
      origin: None,
      fields: Arc::new(vec![EdnTag::new("high"), EdnTag::new("shared")]),
      values: Arc::new(vec![Calcit::Nil, Calcit::Nil]),
    });
    let type_value = CalcitTypeAnnotation::Struct(
      Arc::new(CalcitStructDef {
        name: EdnTag::new("Demo"),
        fields: Arc::new(vec![]),
        field_types: Arc::new(vec![]),
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        impls: vec![low_impl, high_impl],
      }),
      Arc::new(vec![]),
    );

    let methods = static_method_descriptors(&type_value).expect("struct method metadata should resolve");

    assert_eq!(
      methods,
      vec![
        StaticMethodDescriptor {
          name: ".high".to_owned(),
          origin: "HighImpl".to_owned(),
        },
        StaticMethodDescriptor {
          name: ".shared".to_owned(),
          origin: "HighImpl".to_owned(),
        },
        StaticMethodDescriptor {
          name: ".low".to_owned(),
          origin: "LowImpl".to_owned(),
        },
      ]
    );
  }

  #[test]
  fn passes_assert_type_through_preprocess() {
    let expr = Cirru::List(vec![Cirru::leaf("assert-type"), Cirru::leaf("x"), Cirru::leaf(":fn")]);
    let code = code_to_calcit(&expr, "tests.assert", "main", vec![]).expect("parse cirru");
    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("x"));
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let resolved =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.assert", &warnings, &stack).expect("preprocess assert-type");

    assert!(matches!(resolved, Calcit::Local(_)), "local assert-type should return typed local");

    // Check that type info is stored in scope_types
    assert!(scope_types.contains_key("x"), "type should be registered in scope");
    if let Some(type_val) = scope_types.get("x") {
      assert!(matches!(type_val.as_ref(), CalcitTypeAnnotation::DynFn), "type should be fn");
    }
  }

  #[test]
  fn assert_type_resolves_local_struct_definitions() {
    let expr = Cirru::List(vec![Cirru::leaf("assert-type"), Cirru::leaf("x"), Cirru::leaf("LocalPerson")]);
    let code = code_to_calcit(&expr, "tests.assert", "main", vec![]).expect("parse cirru");
    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("x"));
    scope_defs.insert(Arc::from("LocalPerson"));
    let struct_def = Arc::new(CalcitStructDef::from_fields(EdnTag::from("Person"), vec![EdnTag::from("name")]));
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    scope_types.insert(
      Arc::from("LocalPerson"),
      Arc::new(CalcitTypeAnnotation::StructDef(struct_def.clone())),
    );
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let resolved =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.assert", &warnings, &stack).expect("preprocess assert-type");

    assert!(
      matches!(resolve_type_value(&resolved, &scope_types).as_deref(), Some(CalcitTypeAnnotation::Struct(def, args)) if def == &struct_def && args.is_empty()),
      "a local StructDef in type position should become its instance type, got {resolved}"
    );
  }

  #[test]
  fn inspect_type_locations_use_at_paths_without_brackets() {
    assert_eq!(format_inspect_type_coord(&[3, 5, 1]), "@3.5.1");
  }

  #[test]
  fn broad_assert_type_does_not_erase_inferred_element_type() {
    let expr = Cirru::List(vec![Cirru::leaf("assert-type"), Cirru::leaf("xs"), Cirru::leaf(":list")]);
    let code = code_to_calcit(&expr, "tests.assert", "main", vec![]).expect("parse cirru");
    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("xs"));
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    scope_types.insert(
      Arc::from("xs"),
      Arc::new(CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Number))),
    );
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let resolved =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.assert", &warnings, &stack).expect("preprocess assert-type");

    assert!(matches!(
      resolve_type_value(&resolved, &scope_types).as_deref(),
      Some(CalcitTypeAnnotation::List(inner)) if matches!(inner.as_ref(), CalcitTypeAnnotation::Number)
    ));
  }

  #[test]
  fn unsafe_coerce_preserves_boundary_node_and_declared_expression_type() {
    let expr = Cirru::List(vec![Cirru::leaf("unsafe-coerce"), Cirru::leaf("x"), Cirru::leaf(":number")]);
    let code = code_to_calcit(&expr, "tests.unsafe-coerce", "main", vec![]).expect("parse cirru");
    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("x"));
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let resolved =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.unsafe-coerce", &warnings, &stack).expect("preprocess coercion");

    let Calcit::List(nodes) = &resolved else {
      panic!("unsafe-coerce should remain visible in the preprocessed tree");
    };
    assert!(matches!(nodes.first(), Some(Calcit::Syntax(CalcitSyntax::UnsafeCoerce, _))));
    assert!(matches!(
      infer_type_from_expr(&resolved, &scope_types).map(|t| t.as_ref().clone()),
      Some(CalcitTypeAnnotation::Number)
    ));
    assert!(
      !scope_types.contains_key("x"),
      "coercing one expression must not retype every later use of the local"
    );
    assert!(warnings.borrow().is_empty());
  }

  #[test]
  fn strict_edn_decode_rejects_dynamic_during_preprocess() {
    let expr = Cirru::List(vec![
      Cirru::leaf("parse-cirru-edn-as"),
      Cirru::leaf("|do 1"),
      Cirru::leaf(":dynamic"),
    ]);
    let code = code_to_calcit(&expr, "tests.edn", "main", vec![]).expect("parse strict decoder");
    let scope_defs: HashSet<Arc<str>> = HashSet::new();
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let error = preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.edn", &warnings, &stack)
      .expect_err("Dynamic decoder must be rejected before runtime");
    assert!(error.msg.contains("Dynamic is forbidden"), "unexpected error: {error:?}");
  }

  #[test]
  fn safe_strict_edn_decode_rejects_known_non_string_input() {
    let expr = Cirru::List(vec![
      Cirru::leaf("try-parse-cirru-edn-as"),
      Cirru::leaf("1"),
      Cirru::leaf(":number"),
    ]);
    let code = code_to_calcit(&expr, "tests.edn", "main", vec![]).expect("parse safe strict decoder");
    let scope_defs: HashSet<Arc<str>> = HashSet::new();
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let error = preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.edn", &warnings, &stack)
      .expect_err("known non-String input must be rejected before runtime");
    assert!(
      error.msg.contains("expected String input, got :number"),
      "unexpected error: {error:?}"
    );
  }

  #[test]
  fn strict_edn_decode_retains_compiled_data_shape() {
    let expr = Cirru::List(vec![Cirru::leaf("parse-cirru-edn-as"), Cirru::leaf("|1"), Cirru::leaf(":number")]);
    let code = code_to_calcit(&expr, "tests.edn", "main", vec![]).expect("parse strict decoder");
    let scope_defs: HashSet<Arc<str>> = HashSet::new();
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let resolved =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.edn", &warnings, &stack).expect("preprocess strict decoder");
    let Calcit::List(nodes) = resolved else {
      panic!("strict decoder should remain a syntax node");
    };
    assert_eq!(nodes.len(), 4);
    assert!(
      nodes
        .get(3)
        .and_then(crate::calcit::data_shape::DataShapeGraph::from_calcit_handle)
        .is_some(),
      "preprocessing should retain the compiled graph"
    );
  }

  #[test]
  fn warns_on_dynamic_postfix_method_when_enabled() {
    let _lock = lock_preprocess_test_state();
    let _warn_guard = WarnDynMethodGuard::new(true);
    let expr = Cirru::List(vec![Cirru::leaf("receiver"), Cirru::leaf(".show")]);
    let code = code_to_calcit(&expr, "tests.dynamic-postfix", "main", vec![]).expect("parse cirru");
    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("receiver"));
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.dynamic-postfix", &warnings, &stack)
      .expect("dynamic postfix call should continue preprocessing");

    let warnings = warnings.borrow();
    let matched = warnings
      .iter()
      .filter(|warning| warning.code() == Some("P_DYNAMIC_POSTFIX_METHOD"))
      .collect::<Vec<_>>();
    assert_eq!(matched.len(), 1, "expected one dynamic postfix warning, got: {warnings:?}");
    assert!(matched[0].message().contains("unsafe-coerce"));
  }

  #[test]
  fn nominal_container_methods_warn_on_dynamic_receivers_without_opt_in() {
    let _lock = lock_preprocess_test_state();
    let _warn_guard = WarnDynMethodGuard::new(false);
    let expr = Cirru::List(vec![Cirru::leaf("receiver"), Cirru::leaf(".unwrap-or"), Cirru::leaf("0")]);
    let code = code_to_calcit(&expr, "tests.dynamic-option", "main", vec![]).expect("parse cirru");
    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("receiver"));
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.dynamic-option", &warnings, &stack)
      .expect("dynamic Option method should preprocess far enough to report a warning");

    let warnings = warnings.borrow();
    let matched = warnings
      .iter()
      .filter(|warning| warning.code() == Some("W_DYNAMIC_NOMINAL_METHOD_RECEIVER"))
      .collect::<Vec<_>>();
    assert_eq!(matched.len(), 1, "expected one Dynamic nominal-method warning, got: {warnings:?}");
    assert!(matched[0].message().contains("statically known Option or Result receiver"));
    assert!(matched[0].message().contains("`option:*` or `result:*`"));
    assert!(
      warnings.iter().all(|warning| warning.code() != Some("P_DYNAMIC_POSTFIX_METHOD")),
      "the unconditional diagnostic should not be duplicated by the opt-in policy warning"
    );
  }

  #[test]
  fn native_js_members_do_not_trigger_dynamic_nominal_method_warning() {
    let _lock = lock_preprocess_test_state();
    let _warn_guard = WarnDynMethodGuard::new(false);

    for method in [".-unwrap-or", ".!unwrap-or"] {
      let expr = Cirru::List(vec![Cirru::leaf("receiver"), Cirru::leaf(method), Cirru::leaf("0")]);
      let code = code_to_calcit(&expr, "tests.dynamic-js-member", "main", vec![]).expect("parse cirru");
      let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
      scope_defs.insert(Arc::from("receiver"));
      let mut scope_types: ScopeTypes = ScopeTypes::new();
      let warnings = RefCell::new(vec![]);

      preprocess_expr(
        &code,
        &scope_defs,
        &mut scope_types,
        "tests.dynamic-js-member",
        &warnings,
        &CallStackList::default(),
      )
      .expect("native JavaScript member token should remain outside nominal method diagnostics");

      assert!(
        warnings
          .borrow()
          .iter()
          .all(|warning| warning.code() != Some("W_DYNAMIC_NOMINAL_METHOD_RECEIVER")),
        "{method} must not be treated as an Option/Result method"
      );
    }
  }

  #[test]
  fn parses_optional_type_annotation() {
    let expr = Cirru::List(vec![
      Cirru::leaf("assert-type"),
      Cirru::leaf("x"),
      Cirru::List(vec![Cirru::leaf("::"), Cirru::leaf(":optional"), Cirru::leaf(":string")]),
    ]);
    let code = code_to_calcit(&expr, "tests.assert", "main", vec![]).expect("parse cirru");
    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("x"));
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let resolved =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.assert", &warnings, &stack).expect("preprocess assert-type");

    assert!(matches!(resolved, Calcit::Local(_)), "local assert-type should return typed local");

    if let Some(type_val) = scope_types.get("x") {
      match type_val.as_ref() {
        CalcitTypeAnnotation::Optional(inner) => {
          assert!(
            matches!(inner.as_ref(), CalcitTypeAnnotation::String),
            "optional inner type should be :string"
          );
        }
        other => panic!("expected optional type annotation, got {other:?}"),
      }
    }
  }

  #[test]
  fn parses_js_nullish_type_annotation_without_treating_it_as_optional() {
    let expr = Cirru::List(vec![
      Cirru::leaf("assert-type"),
      Cirru::leaf("x"),
      Cirru::List(vec![Cirru::leaf("::"), Cirru::leaf(":js-nullish"), Cirru::leaf(":js-object")]),
    ]);
    let code = code_to_calcit(&expr, "tests.assert", "main", vec![]).expect("parse cirru");
    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("x"));
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.assert", &warnings, &stack).expect("preprocess assert-type");

    assert!(matches!(
      scope_types.get("x").map(AsRef::as_ref),
      Some(CalcitTypeAnnotation::JsNullish(inner)) if matches!(inner.as_ref(), CalcitTypeAnnotation::JsObject)
    ));
  }

  #[test]
  fn warns_on_invalid_optional_arity() {
    let expr = Cirru::List(vec![
      Cirru::leaf("assert-type"),
      Cirru::leaf("x"),
      Cirru::List(vec![
        Cirru::leaf("::"),
        Cirru::leaf(":optional"),
        Cirru::leaf(":string"),
        Cirru::leaf(":extra"),
      ]),
    ]);
    let code = code_to_calcit(&expr, "tests.assert", "main", vec![]).expect("parse cirru");
    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("x"));
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let _resolved =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.assert", &warnings, &stack).expect("preprocess assert-type");

    if let Some(type_val) = scope_types.get("x") {
      match type_val.as_ref() {
        CalcitTypeAnnotation::Optional(inner) => {
          assert!(
            matches!(inner.as_ref(), CalcitTypeAnnotation::String),
            "should still parse the first argument as inner type even if arity is wrong"
          );
        }
        other => panic!("expected optional type annotation, got {other:?}"),
      }
    }
  }

  #[test]
  fn warns_on_optional_type_mismatch() {
    let expr = Cirru::List(vec![
      Cirru::leaf("&let"),
      Cirru::List(vec![Cirru::leaf("x"), Cirru::leaf("nil")]),
      Cirru::List(vec![
        Cirru::leaf("assert-type"),
        Cirru::leaf("x"),
        Cirru::List(vec![Cirru::leaf("::"), Cirru::leaf(":optional"), Cirru::leaf(":number")]),
      ]),
      Cirru::List(vec![Cirru::leaf("&+"), Cirru::leaf("x"), Cirru::leaf("1")]),
    ]);

    let code = code_to_calcit(&expr, "tests.optional", "demo", vec![]).expect("parse cirru");
    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("x"));
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let _resolved =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.optional", &warnings, &stack).expect("preprocess optional");

    let warnings_vec = warnings.borrow();
    assert!(!warnings_vec.is_empty(), "should warn on optional mismatch");
    let warning_msg = warnings_vec[0].to_string();
    assert!(
      warning_msg.contains("Proc `&+` arg 1 expects type `:number`"),
      "warning should mention proc arg mismatch: {warning_msg}"
    );
    assert!(
      warning_msg.contains(":number?"),
      "warning should mention optional actual type: {warning_msg}"
    );
  }

  #[test]
  fn propagates_type_info_across_scope() {
    let expr = Cirru::List(vec![
      Cirru::leaf("&let"),
      Cirru::List(vec![Cirru::leaf("x"), Cirru::leaf("1")]),
      Cirru::List(vec![Cirru::leaf("assert-type"), Cirru::leaf("x"), Cirru::leaf(":fn")]),
      Cirru::leaf("x"),
    ]);
    let code = code_to_calcit(&expr, "tests.assert", "demo", vec![]).expect("parse cirru");
    let scope_defs: HashSet<Arc<str>> = HashSet::new();
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let resolved =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.assert", &warnings, &stack).expect("preprocess assert-type");
    let nodes = match resolved {
      Calcit::List(xs) => xs.to_vec(),
      other => panic!("expected list, got {other}"),
    };

    let assert_typed_result = nodes.get(2);
    // local assert-type should become typed local while local type info is injected
    assert!(
      matches!(assert_typed_result, Some(Calcit::Local(_))),
      "local assert-type should be preprocessed into typed local"
    );

    // Check that type info persists in the trailing reference
    if let Some(Calcit::Local(local)) = nodes.get(3) {
      assert!(
        !matches!(*local.type_info, CalcitTypeAnnotation::Dynamic),
        "type info should persist for later usages"
      );
      // Verify the type value
      assert!(matches!(local.type_info.as_ref(), CalcitTypeAnnotation::DynFn), "type should be fn");
    } else {
      panic!("expected trailing local expression");
    }
  }

  #[test]
  fn passes_assert_type_expression_without_local_binding() {
    let expr = Cirru::List(vec![
      Cirru::leaf("assert-type"),
      Cirru::List(vec![Cirru::leaf("&+"), Cirru::leaf("1"), Cirru::leaf("2")]),
      Cirru::leaf(":number"),
    ]);
    let code = code_to_calcit(&expr, "tests.assert", "expr", vec![]).expect("parse cirru");
    let scope_defs: HashSet<Arc<str>> = HashSet::new();
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let resolved =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.assert", &warnings, &stack).expect("preprocess assert-type");

    let nodes = match resolved {
      Calcit::List(xs) => xs.to_vec(),
      other => panic!("assert-type should remain syntax form, got {other}"),
    };
    assert!(
      matches!(nodes.first(), Some(Calcit::Syntax(CalcitSyntax::AssertType, _))),
      "assert-type head should remain syntax"
    );
    assert!(scope_types.is_empty(), "expression assert-type should not mutate local scope types");
  }

  #[test]
  fn assert_type_direct_def_resolution_rejects_visible_values() {
    let _guard = lock_preprocess_test_state();

    program::PROGRAM_CODE_DATA.write().expect("open program code").insert(
      Arc::from("tests.assert"),
      program::ProgramFileData {
        import_map: HashMap::new(),
        defs: HashMap::from([(
          Arc::from("answer"),
          program::ProgramDefEntry {
            code: Calcit::Number(42.0),
            schema: calcit::DYNAMIC_TYPE.clone(),
            doc: Arc::from(""),
            examples: vec![],
            ffi: None,
          },
        )]),
      },
    );

    let assert_value_expr = Cirru::List(vec![Cirru::leaf("assert-type"), Cirru::leaf("x"), Cirru::leaf("answer")]);
    let value_code = code_to_calcit(&assert_value_expr, "tests.assert", "demo", vec![]).expect("parse assert-type value");
    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("x"));
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();
    let resolved = preprocess_expr(&value_code, &scope_defs, &mut scope_types, "tests.assert", &warnings, &stack)
      .expect("preprocess visible value assert-type");
    let asserted_type = resolve_type_value(&resolved, &scope_types);
    assert!(
      matches!(resolved, Calcit::Local(local) if !matches!(local.type_info.as_ref(), CalcitTypeAnnotation::Number)),
      "a visible value name must not be treated as a resolved nominal type, got {asserted_type:?}"
    );
  }

  #[test]
  fn passes_assert_traits_expression_without_local_binding() {
    let expr = Cirru::List(vec![
      Cirru::leaf("assert-traits"),
      Cirru::List(vec![Cirru::leaf("&+"), Cirru::leaf("1"), Cirru::leaf("2")]),
      Cirru::leaf("Show"),
    ]);
    let code = code_to_calcit(&expr, "tests.assert", "expr", vec![]).expect("parse cirru");
    let scope_defs: HashSet<Arc<str>> = HashSet::new();
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let resolved =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.assert", &warnings, &stack).expect("preprocess assert-traits");

    match resolved {
      Calcit::List(xs) => {
        assert!(
          matches!(xs.first(), Some(Calcit::Proc(CalcitProc::NativeAssertTraits))),
          "assert-traits expression should compile to runtime assert proc"
        );
      }
      other => panic!("assert-traits expression should be preserved for runtime check, got {other}"),
    }

    assert!(
      scope_types.is_empty(),
      "expression assert-traits should not mutate local scope types"
    );
  }

  #[test]
  fn lookup_trait_for_preprocess_reads_source_backed_trait_without_runtime_value() {
    let _guard = lock_preprocess_test_state();

    let trait_code = code_to_calcit(
      &Cirru::List(vec![
        Cirru::leaf("deftrait"),
        Cirru::leaf("MySourceTrait"),
        Cirru::List(vec![Cirru::leaf(".show"), Cirru::leaf(":fn")]),
      ]),
      "tests.source-trait",
      "MySourceTrait",
      vec![],
    )
    .expect("parse trait def");

    let mut program_code = program::PROGRAM_CODE_DATA.write().expect("open program code");
    program_code.insert(
      Arc::from("tests.source-trait"),
      program::ProgramFileData {
        import_map: HashMap::new(),
        defs: HashMap::from([(
          Arc::from("MySourceTrait"),
          program::ProgramDefEntry {
            code: trait_code,
            schema: calcit::DYNAMIC_TYPE.clone(),
            doc: Arc::from(""),
            examples: vec![],
            ffi: None,
          },
        )]),
      },
    );
    drop(program_code);

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let trait_def = lookup_trait_ns_def_for_preprocess("tests.source-trait", "MySourceTrait", &warnings, &stack)
      .expect("lookup trait")
      .expect("trait should resolve from source-backed compiled data");

    assert_eq!(trait_def.name.ref_str(), "MySourceTrait");
    assert_eq!(trait_def.definition_ref.as_deref(), Some("tests.source-trait/MySourceTrait"));
    assert!(trait_def.has_method("show"));
  }

  fn seed_external_field_trait(writable: bool) -> Arc<CalcitTrait> {
    let ns = "tests.external-field";
    let def = "HostElement";
    let trait_code = code_to_calcit(
      &Cirru::List(vec![
        Cirru::leaf("deftrait"),
        Cirru::leaf(def),
        Cirru::List(vec![Cirru::leaf(":value"), Cirru::leaf("'String")]),
      ]),
      ns,
      def,
      vec![],
    )
    .expect("parse external trait");
    let trait_def = resolve_trait_def_from_source_code(&trait_code)
      .expect("resolve external trait")
      .with_definition_ref(ns, def);
    program::PROGRAM_CODE_DATA.write().expect("open program code").insert(
      Arc::from(ns),
      program::ProgramFileData {
        import_map: HashMap::new(),
        defs: HashMap::from([(
          Arc::from(def),
          program::ProgramDefEntry {
            code: trait_code,
            schema: calcit::DYNAMIC_TYPE.clone(),
            doc: Arc::from(""),
            examples: vec![],
            ffi: Some(cirru_edn::Edn::map_from_iter(
              [
                (cirru_edn::Edn::tag("backend"), cirru_edn::Edn::tag("js")),
                (cirru_edn::Edn::tag("kind"), cirru_edn::Edn::tag("external-object")),
              ]
              .into_iter()
              .chain(writable.then(|| {
                (
                  cirru_edn::Edn::tag("writable"),
                  cirru_edn::Edn::Set(cirru_edn::EdnSetView(HashSet::from([cirru_edn::Edn::tag("value")]))),
                )
              })),
            )),
          },
        )]),
      },
    );
    Arc::new(trait_def)
  }

  struct JsFfiFeaturePolicyGuard;

  impl JsFfiFeaturePolicyGuard {
    fn with(policy: crate::snapshot::FeaturePolicy) -> Self {
      program::configure_entry_feature_policy(&HashMap::from([("js-ffi".to_owned(), policy)]));
      Self
    }

    fn require() -> Self {
      Self::with(crate::snapshot::FeaturePolicy::Error)
    }

    fn warn() -> Self {
      Self::with(crate::snapshot::FeaturePolicy::Warn)
    }
  }

  impl Drop for JsFfiFeaturePolicyGuard {
    fn drop(&mut self) {
      program::configure_entry_feature_policy(&HashMap::new());
    }
  }

  struct CodegenModeGuard(bool);

  impl CodegenModeGuard {
    fn enabled() -> Self {
      let previous = codegen::codegen_mode();
      codegen::set_codegen_mode(true);
      Self(previous)
    }
  }

  impl Drop for CodegenModeGuard {
    fn drop(&mut self) {
      codegen::set_codegen_mode(self.0);
    }
  }

  fn external_field_test_symbol(name: &str) -> Calcit {
    Calcit::Symbol {
      sym: Arc::from(name),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.external-field"),
        at_def: Arc::from("demo"),
      }),
      location: None,
    }
  }

  fn external_field_test_receiver(trait_def: Arc<CalcitTrait>) -> Calcit {
    let sym: Arc<str> = Arc::from("element");
    Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&sym),
      sym,
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.external-field"),
        at_def: Arc::from("demo"),
      }),
      location: None,
      type_info: Arc::new(CalcitTypeAnnotation::Trait(trait_def)),
    })
  }

  #[test]
  fn js_get_infers_external_trait_field_payload() {
    let _guard = lock_preprocess_test_state();
    let receiver = external_field_test_receiver(seed_external_field_trait(true));
    let expression = Calcit::from(vec![
      external_field_test_symbol("js-get"),
      receiver,
      Calcit::Tag(EdnTag::new("value")),
    ]);

    assert!(matches!(
      infer_static_type_from_expr(&expression).as_deref(),
      Some(CalcitTypeAnnotation::JsNullish(inner)) if matches!(inner.as_ref(), CalcitTypeAnnotation::String)
    ));

    let receiver = external_field_test_receiver(seed_external_field_trait(true));
    let rewritten = rewrite_typed_js_field_operation(
      &external_field_test_symbol("js-get"),
      &CalcitList::from(&[receiver, Calcit::Tag(EdnTag::new("value"))]),
      &ScopeTypes::new(),
    )
    .expect("typed js-get should rewrite");
    assert!(matches!(
      rewritten,
      Calcit::List(items)
        if matches!(items.first(), Some(Calcit::Method(name, calcit::MethodKind::ExternalGet(_))) if name.as_ref() == "value")
    ));
  }

  #[test]
  fn typed_js_field_rewrite_obeys_js_ffi_capability_policy() {
    let _guard = lock_preprocess_test_state();
    let _feature_policy = JsFfiFeaturePolicyGuard::require();
    let _codegen_mode = CodegenModeGuard::enabled();
    let receiver = external_field_test_receiver(seed_external_field_trait(true));
    let rewritten = rewrite_typed_js_field_operation(
      &external_field_test_symbol("js-get"),
      &CalcitList::from(&[receiver, Calcit::Tag(EdnTag::new("value"))]),
      &ScopeTypes::new(),
    )
    .expect("typed js-get should rewrite");
    let rewritten_head = match &rewritten {
      Calcit::List(items) => items.first().expect("typed JS field rewrite must have a head"),
      _ => &rewritten,
    };
    let error = require_js_ffi_feature_for_operation(
      rewritten_head,
      "tests.external-field",
      "unmarked",
      &RefCell::new(vec![]),
      &CallStackList::default(),
    )
    .expect_err("rewritten typed js-get must require the js-ffi feature");

    assert_eq!(error.code(), Some("E_JS_FFI_FEATURE_REQUIRED"));
    assert!(error.to_string().contains("calcit docs read js-interop.md --full"));
  }

  #[test]
  fn unlowered_js_field_operations_still_require_js_ffi_capability() {
    let _guard = lock_preprocess_test_state();
    let _feature_policy = JsFfiFeaturePolicyGuard::require();
    let _codegen_mode = CodegenModeGuard::enabled();

    for (operation, key) in [
      ("js-get", Calcit::Tag(EdnTag::new("missing"))),
      ("aget", untyped_js_ffi_test_receiver()),
    ] {
      let expression = Calcit::from(vec![external_field_test_symbol(operation), untyped_js_ffi_test_receiver(), key]);
      let scope_defs = HashSet::new();
      let mut scope_types = ScopeTypes::new();
      let warnings = RefCell::new(vec![]);
      let error = preprocess_expr(
        &expression,
        &scope_defs,
        &mut scope_types,
        "tests.untyped-ffi",
        &warnings,
        &CallStackList::default(),
      )
      .expect_err("raw JS field operations must be gated even when typed lowering is unavailable");
      assert_eq!(error.code(), Some("E_JS_FFI_FEATURE_REQUIRED"), "operation: {operation}");
    }
  }

  #[test]
  fn js_set_checks_external_trait_static_fields() {
    let _guard = lock_preprocess_test_state();
    let receiver = external_field_test_receiver(seed_external_field_trait(true));
    let head = external_field_test_symbol("js-set");

    let valid_warnings = RefCell::new(vec![]);
    check_typed_js_field_operation(
      &head,
      &CalcitList::from(&[receiver.clone(), Calcit::Tag(EdnTag::new("value")), Calcit::Str(Arc::from("ok"))]),
      &ScopeTypes::new(),
      "tests.external-field",
      "valid",
      &valid_warnings,
      &CallStackList::default(),
    )
    .expect("writable external field should be accepted");
    assert!(valid_warnings.borrow().is_empty());
    let rewritten = rewrite_typed_js_field_operation(
      &head,
      &CalcitList::from(&[receiver.clone(), Calcit::Tag(EdnTag::new("value")), Calcit::Str(Arc::from("ok"))]),
      &ScopeTypes::new(),
    )
    .expect("typed js-set should rewrite");
    assert!(matches!(
      rewritten,
      Calcit::List(items)
        if matches!(items.first(), Some(Calcit::Method(name, calcit::MethodKind::ExternalSet(_))) if name.as_ref() == "value")
    ));

    let unknown_warnings = RefCell::new(vec![]);
    check_typed_js_field_operation(
      &head,
      &CalcitList::from(&[receiver.clone(), Calcit::Tag(EdnTag::new("missing")), Calcit::Str(Arc::from("x"))]),
      &ScopeTypes::new(),
      "tests.external-field",
      "unknown",
      &unknown_warnings,
      &CallStackList::default(),
    )
    .expect("unknown external field remains a warning");
    assert_eq!(unknown_warnings.borrow()[0].code(), Some("W_JS_FFI_UNKNOWN_FIELD"));

    let mismatch_warnings = RefCell::new(vec![]);
    check_typed_js_field_operation(
      &head,
      &CalcitList::from(&[receiver, Calcit::Tag(EdnTag::new("value")), Calcit::Number(1.0)]),
      &ScopeTypes::new(),
      "tests.external-field",
      "mismatch",
      &mismatch_warnings,
      &CallStackList::default(),
    )
    .expect("mismatched external field value remains a warning");
    assert_eq!(mismatch_warnings.borrow()[0].code(), Some("W_JS_FFI_FIELD_TYPE_MISMATCH"));
  }

  #[test]
  fn js_set_requires_external_field_writable_metadata() {
    let _guard = lock_preprocess_test_state();
    let _feature_policy = JsFfiFeaturePolicyGuard::warn();
    let receiver = external_field_test_receiver(seed_external_field_trait(false));
    let warnings = RefCell::new(vec![]);
    check_typed_js_field_operation(
      &external_field_test_symbol("js-set"),
      &CalcitList::from(&[receiver, Calcit::Tag(EdnTag::new("value")), Calcit::Str(Arc::from("blocked"))]),
      &ScopeTypes::new(),
      "tests.external-field",
      "readonly",
      &warnings,
      &CallStackList::default(),
    )
    .expect("warn policy should not reject a read-only field");

    assert_eq!(warnings.borrow().len(), 1);
    assert_eq!(warnings.borrow()[0].code(), Some("W_JS_FFI_FIELD_READONLY"));
  }

  #[test]
  fn js_set_error_policy_rejects_readonly_external_field() {
    let _guard = lock_preprocess_test_state();
    let _feature_policy = JsFfiFeaturePolicyGuard::require();
    let receiver = external_field_test_receiver(seed_external_field_trait(false));
    let error = check_typed_js_field_operation(
      &external_field_test_symbol("js-set"),
      &CalcitList::from(&[receiver, Calcit::Tag(EdnTag::new("value")), Calcit::Str(Arc::from("blocked"))]),
      &ScopeTypes::new(),
      "tests.external-field",
      "readonly",
      &RefCell::new(vec![]),
      &CallStackList::default(),
    )
    .expect_err("error policy must reject a read-only external field");

    assert_eq!(error.code(), Some("E_JS_FFI_FIELD_READONLY"));
    assert!(error.to_string().contains("calcit docs read js-interop.md --full"));
  }

  #[test]
  fn js_ffi_error_policy_rejects_unmarked_host_operations() {
    let _guard = lock_preprocess_test_state();
    let _feature_policy = JsFfiFeaturePolicyGuard::require();
    let _codegen_mode = CodegenModeGuard::enabled();
    let warnings = RefCell::new(vec![]);
    let error = require_js_ffi_feature(
      "raw JavaScript global `js/document`",
      None,
      "tests.js-ffi",
      "unmarked",
      &warnings,
      &CallStackList::default(),
    )
    .expect_err("strict policy must reject an unmarked host operation");

    assert_eq!(error.code(), Some("E_JS_FFI_FEATURE_REQUIRED"));
    assert!(error.to_string().contains("calcit docs read js-interop.md --full"));
    assert!(warnings.borrow().is_empty());
  }

  fn untyped_js_ffi_test_receiver() -> Calcit {
    let sym: Arc<str> = Arc::from("host");
    Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&sym),
      sym,
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.untyped-ffi"),
        at_def: Arc::from("demo"),
      }),
      location: None,
      type_info: Arc::new(CalcitTypeAnnotation::JsObject),
    })
  }

  #[test]
  fn warns_on_untyped_js_ffi_field_access_when_enabled() {
    let _lock = lock_preprocess_test_state();
    let _warn_guard = WarnDynMethodGuard::new(true);
    let receiver = untyped_js_ffi_test_receiver();
    let head = Calcit::Method(Arc::from("value"), calcit::MethodKind::Access);
    let warnings = RefCell::new(vec![]);

    warn_on_untyped_js_ffi_field_access(
      &head,
      &CalcitList::from(std::slice::from_ref(&receiver)),
      &ScopeTypes::new(),
      "tests.untyped-ffi",
      "demo",
      &warnings,
    );

    assert_eq!(warnings.borrow()[0].code(), Some("W_JS_FFI_UNTYPED_ACCESS"));
  }

  #[test]
  fn untyped_js_ffi_field_access_warning_is_opt_in_and_scoped() {
    let _lock = lock_preprocess_test_state();
    let receiver = untyped_js_ffi_test_receiver();
    let head = Calcit::Method(Arc::from("value"), calcit::MethodKind::Access);

    // Disabled by default: the flag must be explicitly turned on.
    let disabled_warnings = RefCell::new(vec![]);
    warn_on_untyped_js_ffi_field_access(
      &head,
      &CalcitList::from(std::slice::from_ref(&receiver)),
      &ScopeTypes::new(),
      "tests.untyped-ffi",
      "demo",
      &disabled_warnings,
    );
    assert!(disabled_warnings.borrow().is_empty());

    let _warn_guard = WarnDynMethodGuard::new(true);

    // A dynamic (non-literal) key gives no actionable static field to suggest.
    let dynamic_key_warnings = RefCell::new(vec![]);
    warn_on_untyped_js_ffi_field_access(
      &external_field_test_symbol("aget"),
      &CalcitList::from(&[receiver.clone(), external_field_test_symbol("k")]),
      &ScopeTypes::new(),
      "tests.untyped-ffi",
      "demo",
      &dynamic_key_warnings,
    );
    assert!(dynamic_key_warnings.borrow().is_empty());

    // Already-typed external-object receivers are covered by other diagnostics.
    let typed_receiver = external_field_test_receiver(seed_external_field_trait(true));
    let typed_warnings = RefCell::new(vec![]);
    warn_on_untyped_js_ffi_field_access(
      &external_field_test_symbol("aget"),
      &CalcitList::from(&[typed_receiver, Calcit::Tag(EdnTag::new("value"))]),
      &ScopeTypes::new(),
      "tests.untyped-ffi",
      "demo",
      &typed_warnings,
    );
    assert!(typed_warnings.borrow().is_empty());
  }

  #[test]
  fn named_function_schema_types_are_visible_inside_the_body() {
    let _guard = lock_preprocess_test_state();

    let ns = "tests.named-schema-body";
    let def = "add-one";
    let source_code = code_to_calcit(
      &Cirru::List(vec![
        Cirru::leaf("defn"),
        Cirru::leaf(def),
        Cirru::List(vec![Cirru::leaf("x")]),
        Cirru::List(vec![Cirru::leaf("&+"), Cirru::leaf("x"), Cirru::leaf("1")]),
      ]),
      ns,
      def,
      vec![],
    )
    .expect("parse typed function");
    let number_type = Arc::new(CalcitTypeAnnotation::Number);
    let schema = Arc::new(CalcitTypeAnnotation::from_function_parts(vec![number_type.clone()], number_type));

    let mut program_code = program::PROGRAM_CODE_DATA.write().expect("open program code");
    program_code.insert(
      Arc::from(ns),
      program::ProgramFileData {
        import_map: HashMap::new(),
        defs: HashMap::from([(
          Arc::from(def),
          program::ProgramDefEntry {
            code: source_code,
            schema,
            doc: Arc::from(""),
            examples: vec![],
            ffi: None,
          },
        )]),
      },
    );
    drop(program_code);

    let warnings = RefCell::new(vec![]);
    compile_source_def_for_snapshot(ns, def, &warnings, &CallStackList::default()).expect("compile typed source function");
    let compiled = program::lookup_compiled_def(ns, def).expect("compiled output");
    let Calcit::List(defn_nodes) = compiled.preprocessed_code else {
      panic!("expected preprocessed defn");
    };
    let Calcit::List(params) = defn_nodes.get(2).expect("parameter list") else {
      panic!("expected parameter list");
    };
    let Some(Calcit::Local(param)) = params.first() else {
      panic!("expected local parameter");
    };
    assert!(matches!(param.type_info.as_ref(), CalcitTypeAnnotation::Number));

    let Calcit::List(body) = defn_nodes.get(defn_nodes.len() - 1).expect("function body") else {
      panic!("expected call body");
    };
    let Some(Calcit::Local(reference)) = body.get(1) else {
      panic!("expected local reference in body");
    };
    assert!(matches!(reference.type_info.as_ref(), CalcitTypeAnnotation::Number));
    assert!(warnings.borrow().is_empty(), "typed body should not emit warnings");
  }

  #[test]
  fn infers_imported_generic_return_type_from_compiled_function_without_runtime_ready() {
    let _guard = lock_preprocess_test_state();

    let ns = "tests.generic-infer";
    let def = "identity";
    let def_id = program::lookup_def_id(ns, def).unwrap_or_else(|| {
      program::mark_runtime_def_cold(ns, def);
      program::lookup_def_id(ns, def).expect("register def id")
    });

    let generic_name: Arc<str> = Arc::from("T");
    program::write_compiled_def(
      ns,
      def,
      program::CompiledDef {
        def_id,
        version_id: 0,
        kind: program::CompiledDefKind::Fn,
        preprocessed_code: Calcit::Fn {
          id: Arc::from("tests.generic-infer/identity"),
          info: Arc::new(CalcitFn {
            name: Arc::from(def),
            def_ns: Arc::from(ns),
            def_ref: None,
            usage: CalcitFnUsageMeta::default(),
            scope: Arc::new(CalcitScope::default()),
            args: Arc::new(CalcitFnArgs::Args(vec![CalcitLocal::track_sym(&Arc::from("x"))])),
            call_shape: crate::calcit::CalcitFnCallShape::fixed(1),
            body: vec![],
            generics: Arc::new(vec![generic_name.clone()]),
            where_bounds: Arc::new(vec![]),
            return_type: Arc::new(CalcitTypeAnnotation::TypeVar(generic_name.clone())),
            arg_types: vec![Arc::new(CalcitTypeAnnotation::TypeVar(generic_name))],
            rest_type: None,
          }),
        },
        codegen_form: Calcit::Nil,
        deps: vec![],
        type_summary: None,
        source_code: None,
        schema: calcit::DYNAMIC_TYPE.clone(),
        doc: Arc::from(""),
        examples: vec![],
      },
    );

    let call = Calcit::List(Arc::new(CalcitList::from(
      &[
        Calcit::Import(CalcitImport {
          ns: Arc::from(ns),
          def: Arc::from(def),
          info: Arc::new(ImportInfo::NsReferDef {
            at_ns: Arc::from("tests.caller"),
            at_def: Arc::from("demo"),
          }),
          def_id: Some(def_id.0),
        }),
        Calcit::Number(1.0),
      ][..],
    )));

    let inferred = infer_type_from_expr(&call, &ScopeTypes::new()).expect("infer import call type");
    assert!(matches!(inferred.as_ref(), CalcitTypeAnnotation::Number));

    let call = Calcit::List(Arc::new(CalcitList::from(
      &[
        Calcit::Symbol {
          sym: Arc::from(def),
          info: Arc::new(CalcitSymbolInfo {
            at_ns: Arc::from(ns),
            at_def: Arc::from("demo"),
          }),
          location: None,
        },
        Calcit::Number(1.0),
      ][..],
    )));

    let inferred = infer_type_from_expr(&call, &ScopeTypes::new()).expect("infer symbol call type");
    assert!(matches!(inferred.as_ref(), CalcitTypeAnnotation::Number));
  }

  #[test]
  fn ensure_ns_def_compiled_refreshes_source_backed_output_even_when_runtime_is_ready() {
    let _guard = lock_preprocess_test_state();

    let ns = "tests.runtime-shortcut";
    let def = "value";
    let source_code = Calcit::Number(1.0);

    let mut program_code = program::PROGRAM_CODE_DATA.write().expect("open program code");
    program_code.insert(
      Arc::from(ns),
      program::ProgramFileData {
        import_map: HashMap::new(),
        defs: HashMap::from([(
          Arc::from(def),
          program::ProgramDefEntry {
            code: source_code,
            schema: calcit::DYNAMIC_TYPE.clone(),
            doc: Arc::from(""),
            examples: vec![],
            ffi: None,
          },
        )]),
      },
    );
    drop(program_code);

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();
    program::write_runtime_ready(ns, def, Calcit::Number(99.0)).expect("seed stale runtime value");

    ensure_ns_def_compiled(ns, def, &warnings, &stack).expect("compile source-backed def with ready runtime cell");
    let compiled = program::lookup_compiled_def(ns, def).expect("compiled output should exist");
    assert_eq!(compiled.preprocessed_code, Calcit::Number(1.0));
  }

  #[test]
  fn ensure_ns_def_compiled_handles_recursive_source_with_compile_guard() {
    let _guard = lock_preprocess_test_state();

    let ns = "tests.recursive-compile";
    let def = "loop";
    let recursive_code = code_to_calcit(
      &Cirru::List(vec![
        Cirru::leaf("defn"),
        Cirru::leaf(def),
        Cirru::List(vec![]),
        Cirru::List(vec![Cirru::leaf(def)]),
      ]),
      ns,
      def,
      vec![],
    )
    .expect("parse recursive fn");

    let mut program_code = program::PROGRAM_CODE_DATA.write().expect("open program code");
    program_code.insert(
      Arc::from(ns),
      program::ProgramFileData {
        import_map: HashMap::new(),
        defs: HashMap::from([(
          Arc::from(def),
          program::ProgramDefEntry {
            code: recursive_code,
            schema: calcit::DYNAMIC_TYPE.clone(),
            doc: Arc::from(""),
            examples: vec![],
            ffi: None,
          },
        )]),
      },
    );
    drop(program_code);

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();
    ensure_ns_def_compiled(ns, def, &warnings, &stack).expect("compile recursive source def");
    assert!(
      program::lookup_compiled_def(ns, def).is_some(),
      "recursive source def should compile once"
    );
  }

  #[test]
  fn validates_struct_field_access() {
    use cirru_edn::EdnTag;

    // Create a test struct type with fields: name, age
    let test_struct = Arc::new(CalcitTypeAnnotation::StructValue(Arc::new(CalcitStructDef::from_fields(
      EdnTag::from("Person"),
      vec![EdnTag::from("age"), EdnTag::from("name")],
    ))));

    let expr = Cirru::List(vec![Cirru::leaf("&struct:get"), Cirru::leaf("user"), Cirru::leaf(":name")]);

    let code = code_to_calcit(&expr, "tests.struct", "demo", vec![]).expect("parse cirru");
    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("user"));
    let mut scope_types: ScopeTypes = ScopeTypes::new();

    // Manually insert the struct type for testing
    scope_types.insert(Arc::from("user"), test_struct.clone());

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let _resolved =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.struct", &warnings, &stack).expect("preprocess should succeed");

    let warnings = warnings.borrow();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].code(), Some("W_STRUCT_RAW_ACCESS"));
    assert!(warnings[0].message().contains("Use `(:name value)`"));
  }

  #[test]
  fn quoted_local_struct_definition_resolves_parameter_type_refs() {
    let box_def = Arc::new(CalcitStructDef::from_fields(EdnTag::from("Box"), vec![EdnTag::from("value")]));
    let mut scope_types = ScopeTypes::new();
    scope_types.insert(Arc::from("Box"), Arc::new(CalcitTypeAnnotation::StructDef(box_def.clone())));
    let number = Arc::new(CalcitTypeAnnotation::Number);
    let annotation = Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from("'Box"), Arc::new(vec![number.clone()])));

    let resolved = resolve_local_type_refs_for_body(annotation, &scope_types);

    assert!(
      matches!(resolved.as_ref(), CalcitTypeAnnotation::Struct(def, args) if def == &box_def && args.as_slice() == [number]),
      "local defstruct should become a concrete applied Struct annotation, got {resolved}"
    );
  }

  #[test]
  fn nested_struct_field_type_inherits_the_declaring_namespace() {
    let _guard = lock_preprocess_test_state();
    calcit::register_program_lookups(program::lookup_runtime_ready, program::lookup_def_code, program::lookup_def_schema);

    let ns = "tests.nested-struct-owner";
    let mut router_def = CalcitStructDef::from_fields(EdnTag::from("Router"), vec![EdnTag::from("name")]);
    router_def.field_types = Arc::new(vec![crate::calcit::DYNAMIC_TYPE.clone()]);
    let mut store_def = CalcitStructDef::from_fields(EdnTag::from("ClientStore"), vec![EdnTag::from("router")]);
    store_def.field_types = Arc::new(vec![Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from("Router"), Arc::new(vec![])))]);
    let event_def = Arc::new(
      CalcitEnumDef::from_struct(CalcitStructValue {
        struct_ref: Arc::new(CalcitStructDef::from_fields(EdnTag::from("Event"), vec![EdnTag::from("none")])),
        values: Arc::new(vec![Calcit::Nil]),
      })
      .expect("valid Event enum"),
    );

    program::PROGRAM_CODE_DATA.write().expect("open program code").insert(
      Arc::from(ns),
      program::ProgramFileData {
        import_map: HashMap::new(),
        defs: HashMap::from([
          (
            Arc::from("Router"),
            program::ProgramDefEntry {
              code: Calcit::StructDef(router_def.clone()),
              schema: crate::calcit::DYNAMIC_TYPE.clone(),
              doc: Arc::from(""),
              examples: vec![],
              ffi: None,
            },
          ),
          (
            Arc::from("ClientStore"),
            program::ProgramDefEntry {
              code: Calcit::StructDef(store_def.clone()),
              schema: crate::calcit::DYNAMIC_TYPE.clone(),
              doc: Arc::from(""),
              examples: vec![],
              ffi: None,
            },
          ),
          (
            Arc::from("Event"),
            program::ProgramDefEntry {
              code: Calcit::EnumDef(event_def.as_ref().clone()),
              schema: crate::calcit::DYNAMIC_TYPE.clone(),
              doc: Arc::from(""),
              examples: vec![],
              ffi: None,
            },
          ),
        ]),
      },
    );
    program::write_runtime_ready(ns, "Router", Calcit::StructDef(router_def)).expect("register Router runtime metadata");
    program::write_runtime_ready(ns, "ClientStore", Calcit::StructDef(store_def)).expect("register ClientStore runtime metadata");
    program::write_runtime_ready(ns, "Event", Calcit::EnumDef(event_def.as_ref().clone())).expect("register Event runtime metadata");

    let store_type = Arc::new(CalcitTypeAnnotation::TypeRef(
      Arc::from(format!("{ns}/ClientStore")),
      Arc::new(vec![]),
    ));
    let receiver = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from("store")),
      sym: Arc::from("store"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.consumer"),
        at_def: Arc::from("demo"),
      }),
      location: None,
      type_info: store_type,
    });

    let inferred = infer_struct_field_type(&receiver, "router", &ScopeTypes::new());
    assert!(
      matches!(
        inferred.as_deref(),
        Some(CalcitTypeAnnotation::TypeRef(name, args)) if name.as_ref() == format!("{ns}/Router") && args.is_empty()
      ),
      "nested field should resolve relative to its owner, got {inferred:?}"
    );

    let struct_marker = CalcitTypeAnnotation::from_tag_name("struct-def");
    let enum_marker = CalcitTypeAnnotation::from_tag_name("enum-def");
    let struct_ref = CalcitTypeAnnotation::TypeRef(Arc::from("Router"), Arc::new(vec![]));
    let enum_ref = CalcitTypeAnnotation::TypeRef(Arc::from("Event"), Arc::new(vec![]));
    crate::calcit::with_type_annotation_warning_context(format!("{ns}/ClientStore"), || {
      assert!(
        struct_ref.matches_annotation(&struct_marker),
        "unqualified struct TypeRef should resolve in its source namespace"
      );
      assert!(
        enum_ref.matches_annotation(&enum_marker),
        "unqualified enum TypeRef should resolve in its source namespace"
      );
    });
  }

  #[test]
  fn match_resolves_local_enum_definitions_with_applied_args() {
    use crate::calcit::{CalcitEnumDef, CalcitStructValue};

    let generic: Arc<str> = Arc::from("T");
    let enum_struct = CalcitStructDef {
      name: EdnTag::from("Wrapped"),
      fields: Arc::new(vec![EdnTag::from("empty"), EdnTag::from("some")]),
      field_types: Arc::new(vec![crate::calcit::DYNAMIC_TYPE.clone(), crate::calcit::DYNAMIC_TYPE.clone()]),
      generics: Arc::new(vec![generic.clone()]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    };
    let enum_def = Arc::new(
      CalcitEnumDef::from_struct(CalcitStructValue {
        struct_ref: Arc::new(enum_struct),
        values: Arc::new(vec![
          Calcit::from(CalcitList::default()),
          Calcit::from(vec![CalcitTypeAnnotation::TypeVar(generic).to_calcit()]),
        ]),
      })
      .expect("valid local enum fixture"),
    );
    let mut scope_types = ScopeTypes::new();
    scope_types.insert(Arc::from("Wrapped"), Arc::new(CalcitTypeAnnotation::EnumDef(enum_def.clone())));
    let type_ref = CalcitTypeAnnotation::TypeRef(Arc::from("Wrapped"), Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]));

    assert_eq!(
      resolve_enum_type_for_match(&type_ref, "tests.enum", &scope_types),
      Some(enum_def.as_ref().to_owned())
    );
  }

  #[test]
  fn raw_struct_access_requires_static_evidence_outside_defimpl() {
    let head = Calcit::Proc(CalcitProc::NativeStructGet);
    let receiver = Calcit::Symbol {
      sym: Arc::from("value"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.struct"),
        at_def: Arc::from("demo"),
      }),
      location: None,
    };
    let args = CalcitList::from(&[receiver, Calcit::Tag(EdnTag::from("name"))][..]);
    let warnings = RefCell::new(vec![]);

    check_struct_field_access(
      &head,
      &args,
      &ScopeTypes::new(),
      "tests.struct",
      &CallStackList::default(),
      &warnings,
    );

    let warnings = warnings.borrow();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].code(), Some("W_STRUCT_DYNAMIC_RAW_ACCESS"));
    assert!(warnings[0].message().contains("Add/narrow a named Struct schema"));
  }

  #[test]
  fn raw_struct_access_explains_unresolved_nominal_receivers() {
    let head = Calcit::Proc(CalcitProc::NativeStructGet);
    let receiver = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from("router")),
      sym: Arc::from("router"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.consumer"),
        at_def: Arc::from("demo"),
      }),
      location: None,
      type_info: Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from("Router"), Arc::new(vec![]))),
    });
    let args = CalcitList::from(&[receiver, Calcit::Tag(EdnTag::from("name"))][..]);
    let warnings = RefCell::new(vec![]);

    check_struct_field_access(
      &head,
      &args,
      &ScopeTypes::new(),
      "tests.consumer",
      &CallStackList::default(),
      &warnings,
    );

    let warnings = warnings.borrow();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].code(), Some("W_STRUCT_DYNAMIC_RAW_ACCESS"));
    assert!(warnings[0].message().contains("unresolved nominal receiver `'Router`"));
    assert!(warnings[0].message().contains("qualified schema such as `'app.schema/Type`"));
  }

  #[test]
  fn dynamic_raw_struct_field_keys_do_not_suggest_tag_syntax() {
    let head = Calcit::Proc(CalcitProc::NativeStructGet);
    let receiver = Calcit::Symbol {
      sym: Arc::from("value"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.struct"),
        at_def: Arc::from("demo"),
      }),
      location: None,
    };
    let field = Calcit::Symbol {
      sym: Arc::from("field"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.struct"),
        at_def: Arc::from("demo"),
      }),
      location: None,
    };
    let args = CalcitList::from(&[receiver, field][..]);
    let warnings = RefCell::new(vec![]);

    check_struct_field_access(
      &head,
      &args,
      &ScopeTypes::new(),
      "tests.struct",
      &CallStackList::default(),
      &warnings,
    );

    assert!(warnings.borrow().is_empty());
  }

  #[test]
  fn project_source_lints_exclude_loaded_module_namespaces() {
    let namespaces = HashSet::from([Arc::from("app.main"), Arc::from("app.comp")]);
    assert!(namespace_is_project_source(&namespaces, "app.comp"));
    assert!(!namespace_is_project_source(&namespaces, "respo.core"));
    assert!(namespace_is_project_source(&HashSet::new(), "tests.default"));
  }

  #[test]
  fn reusable_defimpl_may_use_explicit_raw_struct_access() {
    let head = Calcit::Proc(CalcitProc::NativeStructGet);
    let receiver = Calcit::Symbol {
      sym: Arc::from("value"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.struct"),
        at_def: Arc::from("demo"),
      }),
      location: None,
    };
    let args = CalcitList::from(&[receiver, Calcit::Tag(EdnTag::from("name"))][..]);
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default().extend(calcit::CORE_NS, "defimpl", StackKind::Macro, &Calcit::Nil, &[]);

    check_struct_field_access(&head, &args, &ScopeTypes::new(), "tests.struct", &stack, &warnings);

    assert!(warnings.borrow().is_empty());
  }

  #[test]
  fn common_get_rejects_known_struct_receivers() {
    let struct_type = Arc::new(CalcitTypeAnnotation::StructValue(Arc::new(CalcitStructDef::from_fields(
      EdnTag::from("Person"),
      vec![EdnTag::from("name")],
    ))));
    let mut scope_types = ScopeTypes::new();
    scope_types.insert(Arc::from("person"), struct_type);

    let head = Calcit::Import(CalcitImport {
      ns: Arc::from(calcit::CORE_NS),
      def: Arc::from("get"),
      info: Arc::new(ImportInfo::Core {
        at_ns: Arc::from("tests.struct"),
      }),
      def_id: None,
    });
    let args = CalcitList::from(
      &[
        Calcit::Symbol {
          sym: Arc::from("person"),
          info: Arc::new(CalcitSymbolInfo {
            at_ns: Arc::from("tests.struct"),
            at_def: Arc::from("demo"),
          }),
          location: None,
        },
        Calcit::Tag(EdnTag::from("missing")),
      ][..],
    );
    let warnings = RefCell::new(vec![]);

    check_struct_field_access(&head, &args, &scope_types, "tests.struct", &CallStackList::default(), &warnings);

    let warnings = warnings.borrow();
    assert_eq!(warnings.len(), 1);
    let message = warnings[0].message();
    assert_eq!(warnings[0].code(), Some("W_STRUCT_FIELD_OPTIONAL_LOOKUP"));
    assert!(message.contains("`get` is the Option-returning lookup API"));
    assert!(message.contains("Use `(:missing value)`"));
  }

  #[test]
  fn prefix_map_field_access_requires_a_typed_struct() {
    let expr = Cirru::List(vec![Cirru::leaf(":name"), Cirru::leaf("record")]);
    let code = code_to_calcit(&expr, "tests.struct", "demo", vec![]).expect("parse required field access");
    let mut scope_defs = HashSet::new();
    scope_defs.insert(Arc::from("record"));
    let mut scope_types = ScopeTypes::new();
    scope_types.insert(
      Arc::from("record"),
      Arc::new(CalcitTypeAnnotation::Map(
        Arc::new(CalcitTypeAnnotation::Tag),
        Arc::new(CalcitTypeAnnotation::String),
      )),
    );

    let warnings = RefCell::new(vec![]);
    let _ = preprocess_expr(
      &code,
      &scope_defs,
      &mut scope_types,
      "tests.struct",
      &warnings,
      &CallStackList::default(),
    );

    let warnings = warnings.borrow();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].code(), Some("W_REQUIRED_STRUCT_FIELD_TYPE"));
    assert!(warnings[0].message().contains("needs a statically typed Struct"));
    assert!(
      warnings[0]
        .message()
        .contains("use `(get value :name)` only when absence is intentional")
    );
  }

  #[test]
  fn macro_generated_prefix_field_probe_does_not_emit_required_struct_warning() {
    let receiver = Calcit::Tag(EdnTag::from("guide"));
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default().extend(calcit::CORE_NS, "%{}", StackKind::Macro, &Calcit::Nil, &[]);

    warn_required_struct_field_type(
      "key",
      &receiver,
      Some(&CalcitTypeAnnotation::Tag),
      RequiredStructFieldWarningContext {
        file_ns: "tests.consumer",
        def_name: "demo",
        location: None,
        call_stack: &stack,
      },
      &warnings,
    );

    assert!(
      warnings.borrow().is_empty(),
      "generated macro probes must not be reported as source-level Struct reads"
    );

    let source_warnings = RefCell::new(vec![]);
    warn_required_struct_field_type(
      "key",
      &receiver,
      Some(&CalcitTypeAnnotation::Tag),
      RequiredStructFieldWarningContext {
        file_ns: "tests.consumer",
        def_name: "demo",
        location: Some(NodeLocation::new(Arc::from("tests.consumer"), Arc::from("demo"), Arc::new(vec![1]))),
        call_stack: &stack,
      },
      &source_warnings,
    );
    assert_eq!(
      source_warnings.borrow().first().and_then(LocatedWarning::code),
      Some("W_REQUIRED_STRUCT_FIELD_TYPE"),
      "macro arguments with a source coordinate must keep required Struct diagnostics"
    );

    let nested_receiver = code_to_calcit(
      &Cirru::List(vec![Cirru::leaf("identity"), Cirru::leaf("value")]),
      "tests.consumer",
      "demo",
      vec![2],
    )
    .expect("parse source-located receiver");
    let nested_source_warnings = RefCell::new(vec![]);
    warn_required_struct_field_type(
      "key",
      &nested_receiver,
      Some(&CalcitTypeAnnotation::Tag),
      RequiredStructFieldWarningContext {
        file_ns: "tests.consumer",
        def_name: "demo",
        location: None,
        call_stack: &stack,
      },
      &nested_source_warnings,
    );
    assert_eq!(
      nested_source_warnings.borrow().first().and_then(LocatedWarning::code),
      Some("W_REQUIRED_STRUCT_FIELD_TYPE"),
      "source locations nested in a receiver expression must keep required Struct diagnostics"
    );

    let mixed_location_warnings = RefCell::new(vec![]);
    warn_required_struct_field_type(
      "key",
      &nested_receiver,
      Some(&CalcitTypeAnnotation::Tag),
      RequiredStructFieldWarningContext {
        file_ns: "tests.consumer",
        def_name: "demo",
        location: Some(NodeLocation::new(
          Arc::from("tests.consumer"),
          Arc::from(GENERATED_DEF),
          Arc::new(vec![]),
        )),
        call_stack: &stack,
      },
      &mixed_location_warnings,
    );
    let mixed_location_warnings = mixed_location_warnings.borrow();
    assert_eq!(
      mixed_location_warnings.first().and_then(LocatedWarning::code),
      Some("W_REQUIRED_STRUCT_FIELD_TYPE"),
      "a generated location must not hide an attributable receiver location"
    );
    assert_eq!(mixed_location_warnings[0].location().def.as_ref(), "demo");
  }

  #[test]
  fn postfix_nominal_struct_access_uses_direct_field_lookup() {
    let expr = Cirru::List(vec![Cirru::leaf("person"), Cirru::leaf(":name")]);
    let code = code_to_calcit(&expr, "tests.struct", "demo", vec![]).expect("parse postfix field access");
    let mut scope_defs = HashSet::new();
    scope_defs.insert(Arc::from("person"));
    let mut scope_types = ScopeTypes::new();
    scope_types.insert(
      Arc::from("person"),
      Arc::new(CalcitTypeAnnotation::StructValue(Arc::new(CalcitStructDef::from_fields(
        EdnTag::from("Person"),
        vec![EdnTag::from("name")],
      )))),
    );

    let warnings = RefCell::new(vec![]);
    let resolved = preprocess_expr(
      &code,
      &scope_defs,
      &mut scope_types,
      "tests.struct",
      &warnings,
      &CallStackList::default(),
    )
    .expect("preprocess nominal postfix field access");

    let Calcit::List(items) = resolved else {
      panic!("expected specialized field access call");
    };
    assert!(
      matches!(items.first(), Some(Calcit::Proc(CalcitProc::NativeStructNth))),
      "nominal field access should bypass Option-producing get: {items}"
    );
  }

  #[test]
  fn postfix_loose_struct_access_requires_a_nominal_declaration() {
    let expr = Cirru::List(vec![Cirru::leaf("record"), Cirru::leaf(":name")]);
    let code = code_to_calcit(&expr, "tests.struct", "demo", vec![]).expect("parse postfix field access");
    let mut scope_defs = HashSet::new();
    scope_defs.insert(Arc::from("record"));
    let mut scope_types = ScopeTypes::new();
    scope_types.insert(Arc::from("record"), tag_annotation("record"));

    let warnings = RefCell::new(vec![]);
    let resolved = preprocess_expr(
      &code,
      &scope_defs,
      &mut scope_types,
      "tests.struct",
      &warnings,
      &CallStackList::default(),
    )
    .expect("preprocess loose record postfix field access");

    let Calcit::List(items) = resolved else {
      panic!("expected required record get call");
    };
    assert!(
      matches!(items.first(), Some(Calcit::Proc(CalcitProc::NativeStructGet))),
      "loose record access keeps the raw lookup only after recording a hard diagnostic: {items}"
    );
    let warnings = warnings.borrow();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].code(), Some("W_REQUIRED_STRUCT_FIELD_TYPE"));
    assert!(warnings[0].message().contains("with a declared `:name` field"));
  }

  #[test]
  fn rewrites_struct_head_call_to_struct_ctor() {
    use crate::data::cirru::code_to_calcit;
    use cirru_edn::EdnTag;
    use cirru_parser::Cirru;

    let person_struct = CalcitStructDef::from_fields(EdnTag::from("Person"), vec![EdnTag::from("name"), EdnTag::from("age")]);

    let expr = Cirru::List(vec![
      Cirru::leaf("Person"),
      Cirru::leaf(":name"),
      Cirru::leaf("|Alice"),
      Cirru::leaf(":age"),
      Cirru::leaf("20"),
    ]);

    let parsed = code_to_calcit(&expr, "tests.struct", "demo", vec![]).expect("parse struct ctor");
    let Calcit::List(parsed_items) = parsed else {
      panic!("expected parsed call");
    };
    let mut code_items = parsed_items.to_vec();
    code_items[0] = Calcit::StructDef(person_struct.clone());
    let code = Calcit::from(code_items);

    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("Person"));

    let mut scope_types: ScopeTypes = ScopeTypes::new();
    scope_types.insert(
      Arc::from("Person"),
      Arc::new(CalcitTypeAnnotation::Struct(Arc::new(person_struct.clone()), Arc::new(vec![]))),
    );

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let result =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.struct", &warnings, &stack).expect("preprocess struct head call");

    let items = match result {
      Calcit::List(xs) => xs.to_vec(),
      other => panic!("expected list form, got {other}"),
    };

    assert!(matches!(items.first(), Some(Calcit::Proc(CalcitProc::NativeStruct))));
    match items.get(1) {
      Some(Calcit::StructDef(struct_def)) => assert_eq!(struct_def.name, person_struct.name),
      other => panic!("expected struct prototype at position 1, got {other:?}"),
    }
    assert_eq!(*items.get(2).expect("name field key"), Calcit::Tag(EdnTag::from("name")));
    assert_eq!(*items.get(4).expect("age field key"), Calcit::Tag(EdnTag::from("age")));
    assert_eq!(*items.get(3).expect("name value"), Calcit::Str(Arc::from("Alice")));
    assert_eq!(*items.get(5).expect("age value"), Calcit::Number(20.0));
  }

  #[test]
  fn rewrites_omitted_option_struct_field_to_none() {
    use crate::data::cirru::code_to_calcit;
    use cirru_edn::EdnTag;
    use cirru_parser::Cirru;

    let mut person_struct = CalcitStructDef::from_fields(EdnTag::from("Person"), vec![EdnTag::from("name"), EdnTag::from("trace")]);
    person_struct.field_types = Arc::new(vec![
      Arc::new(CalcitTypeAnnotation::String),
      Arc::new(CalcitTypeAnnotation::TypeRef(
        Arc::from("calcit.core/Option"),
        Arc::new(vec![Arc::new(CalcitTypeAnnotation::String)]),
      )),
    ]);

    let expr = Cirru::List(vec![Cirru::leaf("Person"), Cirru::leaf(":name"), Cirru::leaf("|Alice")]);
    let parsed = code_to_calcit(&expr, "tests.struct", "demo", vec![]).expect("parse struct ctor");
    let Calcit::List(parsed_items) = parsed else {
      panic!("expected parsed call");
    };
    let mut code_items = parsed_items.to_vec();
    code_items[0] = Calcit::StructDef(person_struct.clone());
    let mut scope_types = ScopeTypes::new();
    scope_types.insert(
      Arc::from("Person"),
      Arc::new(CalcitTypeAnnotation::Struct(Arc::new(person_struct.clone()), Arc::new(vec![]))),
    );

    let warnings = RefCell::new(vec![]);
    let result = try_rewrite_struct_enum_constructor_head_call(
      code_items.first().expect("struct head"),
      &CalcitList::from(&code_items[1..]),
      &scope_types,
      "tests.struct",
      "demo",
      &warnings,
      &CallStackList::default(),
    )
    .expect("rewrite struct ctor")
    .expect("struct ctor should rewrite");
    let Calcit::List(items) = result else {
      panic!("expected rewritten struct list");
    };
    assert!(matches!(items.first(), Some(Calcit::Proc(CalcitProc::NativeStruct))));
    assert!(matches!(items.get(3), Some(Calcit::Str(value)) if value.as_ref() == "Alice"));
    let Some(Calcit::List(none_call)) = items.get(5) else {
      panic!("expected omitted Option field to become a %none call");
    };
    assert!(
      matches!(none_call.first(), Some(Calcit::Import(CalcitImport { ns, def, .. })) if ns.as_ref() == calcit::CORE_NS && def.as_ref() == "%none")
    );
    assert!(warnings.borrow().is_empty());
  }

  #[test]
  fn rewrites_enum_head_call_to_enum_ctor() {
    use crate::calcit::CalcitEnumDef;
    use crate::data::cirru::code_to_calcit;
    use cirru_edn::EdnTag;
    use cirru_parser::Cirru;

    let enum_struct = CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef::from_fields(
        EdnTag::from("Result"),
        vec![EdnTag::from("ok"), EdnTag::from("err")],
      )),
      values: Arc::new(vec![
        Calcit::List(Arc::new(CalcitList::default())),
        Calcit::List(Arc::new(CalcitList::default())),
      ]),
    };
    let result_enum = CalcitEnumDef::from_struct(enum_struct.clone()).expect("valid result enum");

    let expr = Cirru::List(vec![Cirru::leaf("Result"), Cirru::leaf(":ok")]);

    let parsed = code_to_calcit(&expr, "tests.struct", "demo", vec![]).expect("parse enum ctor");
    let Calcit::List(parsed_items) = parsed else {
      panic!("expected parsed call");
    };
    let mut code_items = parsed_items.to_vec();
    code_items[0] = Calcit::EnumDef(result_enum.clone());
    let code = Calcit::from(code_items);

    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("Result"));

    let mut scope_types: ScopeTypes = ScopeTypes::new();
    scope_types.insert(
      Arc::from("Result"),
      Arc::new(CalcitTypeAnnotation::Enum(Arc::new(result_enum.clone()), Arc::new(vec![]))),
    );

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let result =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.struct", &warnings, &stack).expect("preprocess enum head call");

    let items = match result {
      Calcit::List(xs) => xs.to_vec(),
      other => panic!("expected list form, got {other}"),
    };

    assert!(matches!(items.first(), Some(Calcit::Proc(CalcitProc::NativeNamedEnumNew))));
    match items.get(1) {
      Some(Calcit::Struct(enum_struct)) => assert_eq!(enum_struct.struct_ref.name, *result_enum.name()),
      other => panic!("expected enum prototype at position 1, got {other:?}"),
    }
    assert_eq!(*items.get(2).expect("tag key"), Calcit::Tag(EdnTag::from("ok")));
    assert_eq!(items.len(), 3);
  }

  #[test]
  fn preserves_import_path_for_direct_named_enum_constructor() {
    let head = Calcit::Import(CalcitImport {
      ns: Arc::from("tests.schema"),
      def: Arc::from("Op"),
      info: Arc::new(ImportInfo::NsAs {
        alias: Arc::from("schema"),
        at_ns: Arc::from("tests.consumer"),
        at_def: Arc::from("make-op"),
      }),
      def_id: Some(7),
    });

    assert_eq!(
      constructor_definition_path(&head),
      Some((Arc::from("tests.schema"), Arc::from("Op")))
    );
  }

  #[test]
  fn rejects_struct_head_call_with_odd_args() {
    use crate::data::cirru::code_to_calcit;
    use cirru_edn::EdnTag;
    use cirru_parser::Cirru;

    let person_struct = CalcitStructDef::from_fields(EdnTag::from("Person"), vec![EdnTag::from("name"), EdnTag::from("age")]);

    let expr = Cirru::List(vec![Cirru::leaf("Person"), Cirru::leaf(":name")]);

    let parsed = code_to_calcit(&expr, "tests.struct", "demo", vec![]).expect("parse struct ctor");
    let Calcit::List(parsed_items) = parsed else {
      panic!("expected parsed call");
    };
    let mut code_items = parsed_items.to_vec();
    code_items[0] = Calcit::StructDef(person_struct.clone());
    let code = Calcit::from(code_items);

    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("Person"));

    let mut scope_types: ScopeTypes = ScopeTypes::new();
    scope_types.insert(
      Arc::from("Person"),
      Arc::new(CalcitTypeAnnotation::Struct(Arc::new(person_struct.clone()), Arc::new(vec![]))),
    );

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let result =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.struct", &warnings, &stack).expect("preprocess struct head call");

    let items = match result {
      Calcit::List(xs) => xs.to_vec(),
      other => panic!("expected list form, got {other}"),
    };

    assert!(
      !matches!(items.first(), Some(Calcit::Proc(CalcitProc::NativeStruct))),
      "should not rewrite when struct constructor args are odd"
    );

    let warnings_vec = warnings.borrow();
    assert!(!warnings_vec.is_empty(), "should warn on odd key/value arguments");
    assert!(
      warnings_vec.iter().any(|w| w.to_string().contains("expected key/value pairs")),
      "warning should mention expected key/value pairs"
    );
  }

  #[test]
  fn rejects_struct_head_call_with_unknown_field() {
    use crate::data::cirru::code_to_calcit;
    use cirru_edn::EdnTag;
    use cirru_parser::Cirru;

    let person_struct = CalcitStructDef::from_fields(EdnTag::from("Person"), vec![EdnTag::from("name"), EdnTag::from("age")]);

    let expr = Cirru::List(vec![
      Cirru::leaf("Person"),
      Cirru::leaf(":email"),
      Cirru::leaf("|alice@example.com"),
      Cirru::leaf(":age"),
      Cirru::leaf("20"),
    ]);

    let parsed = code_to_calcit(&expr, "tests.struct", "demo", vec![]).expect("parse struct ctor");
    let Calcit::List(parsed_items) = parsed else {
      panic!("expected parsed call");
    };
    let mut code_items = parsed_items.to_vec();
    code_items[0] = Calcit::StructDef(person_struct.clone());
    let code = Calcit::from(code_items);

    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("Person"));

    let mut scope_types: ScopeTypes = ScopeTypes::new();
    scope_types.insert(
      Arc::from("Person"),
      Arc::new(CalcitTypeAnnotation::Struct(Arc::new(person_struct.clone()), Arc::new(vec![]))),
    );

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let result =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.struct", &warnings, &stack).expect("preprocess struct head call");

    let items = match result {
      Calcit::List(xs) => xs.to_vec(),
      other => panic!("expected list form, got {other}"),
    };

    assert!(
      !matches!(items.first(), Some(Calcit::Proc(CalcitProc::NativeStruct))),
      "should not rewrite when key not in struct fields"
    );

    let warnings_vec = warnings.borrow();
    assert!(!warnings_vec.is_empty(), "should warn on unknown struct field");
    assert!(
      warnings_vec.iter().any(|w| w.to_string().contains(":email")),
      "warning should mention unknown field"
    );
  }

  #[test]
  fn rejects_struct_head_call_with_duplicate_or_missing_required_field() {
    use cirru_edn::EdnTag;

    let person_struct = CalcitStructDef::from_fields(EdnTag::from("Person"), vec![EdnTag::from("name"), EdnTag::from("age")]);
    let warnings = RefCell::new(vec![]);
    let scope_types = ScopeTypes::new();
    let stack = CallStackList::default();

    let duplicate_args = CalcitList::from(
      vec![
        Calcit::Tag(EdnTag::from("name")),
        Calcit::Str(Arc::from("Alice")),
        Calcit::Tag(EdnTag::from("name")),
        Calcit::Str(Arc::from("Bob")),
      ]
      .as_slice(),
    );
    let duplicate_result = try_rewrite_struct_enum_constructor_head_call(
      &Calcit::StructDef(person_struct.clone()),
      &duplicate_args,
      &scope_types,
      "tests.struct",
      "demo",
      &warnings,
      &stack,
    )
    .expect("check duplicate fields");
    assert!(duplicate_result.is_none());
    assert!(
      warnings
        .borrow()
        .iter()
        .any(|warning| warning.to_string().contains("duplicate field"))
    );

    warnings.borrow_mut().clear();
    let missing_args = CalcitList::from(vec![Calcit::Tag(EdnTag::from("name")), Calcit::Str(Arc::from("Alice"))].as_slice());
    let missing_result = try_rewrite_struct_enum_constructor_head_call(
      &Calcit::StructDef(person_struct),
      &missing_args,
      &scope_types,
      "tests.struct",
      "demo",
      &warnings,
      &stack,
    )
    .expect("check missing fields");
    assert!(missing_result.is_none());
    assert!(
      warnings
        .borrow()
        .iter()
        .any(|warning| warning.to_string().contains("required field `:age` is missing"))
    );
  }

  #[test]
  fn struct_and_enum_instances_are_not_constructor_heads() {
    use crate::calcit::{CalcitEnumDef, CalcitEnumValue};
    use cirru_edn::EdnTag;

    let person_struct = CalcitStructDef::from_fields(EdnTag::from("Person"), vec![EdnTag::from("name")]);
    let person = Calcit::Struct(CalcitStructValue {
      struct_ref: Arc::new(person_struct),
      values: Arc::new(vec![Calcit::Str(Arc::from("Alice"))]),
    });
    let enum_struct = CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef::from_fields(EdnTag::from("Result"), vec![EdnTag::from("ok")])),
      values: Arc::new(vec![Calcit::List(Arc::new(CalcitList::default()))]),
    };
    let result_enum = CalcitEnumDef::from_struct(enum_struct).expect("valid enum");
    let enum_value = Calcit::Enum(CalcitEnumValue {
      tag: Arc::new(Calcit::Tag(EdnTag::from("ok"))),
      extra: vec![],
      sum_type: Some(Arc::new(result_enum)),
    });
    let args = CalcitList::from(vec![Calcit::Tag(EdnTag::from("name")), Calcit::Str(Arc::from("Bob"))].as_slice());
    let scope_types = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    assert!(
      try_rewrite_struct_enum_constructor_head_call(&person, &args, &scope_types, "tests.struct", "demo", &warnings, &stack,)
        .expect("record instance boundary")
        .is_none()
    );
    assert!(
      try_rewrite_struct_enum_constructor_head_call(&enum_value, &args, &scope_types, "tests.struct", "demo", &warnings, &stack,)
        .expect("enum instance boundary")
        .is_none()
    );
    assert!(warnings.borrow().is_empty());
  }

  #[test]
  fn warns_on_struct_constructor_field_type_mismatch() {
    use cirru_edn::EdnTag;

    let mut point_struct = CalcitStructDef::from_fields(EdnTag::from("Point"), vec![EdnTag::from("x")]);
    point_struct.field_types = Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]);
    let args = CalcitList::from(vec![Calcit::Tag(EdnTag::from("x")), Calcit::Str(Arc::from("wrong"))].as_slice());
    let warnings = RefCell::new(vec![]);
    let result = try_rewrite_struct_enum_constructor_head_call(
      &Calcit::StructDef(point_struct),
      &args,
      &ScopeTypes::new(),
      "tests.struct",
      "demo",
      &warnings,
      &CallStackList::default(),
    )
    .expect("rewrite typed struct constructor");

    assert!(result.is_some());
    assert!(warnings.borrow().iter().any(|warning| {
      let message = warning.to_string();
      message.contains("field `:x`") && message.contains("expects type")
    }));
  }

  #[test]
  fn validates_typed_struct_update_fields_after_generic_substitution() {
    use cirru_edn::EdnTag;

    let generic: Arc<str> = Arc::from("T");
    let mut box_struct = CalcitStructDef::from_fields(EdnTag::from("Box"), vec![EdnTag::from("value")]);
    box_struct.generics = Arc::new(vec![generic.clone()]);
    box_struct.field_types = Arc::new(vec![Arc::new(CalcitTypeAnnotation::TypeVar(generic))]);
    let receiver = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from("box")),
      sym: Arc::from("box"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.struct"),
        at_def: Arc::from("demo"),
      }),
      location: None,
      type_info: Arc::new(CalcitTypeAnnotation::Struct(
        Arc::new(box_struct),
        Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]),
      )),
    });
    let args = CalcitList::from(vec![receiver, Calcit::Tag(EdnTag::from("value")), Calcit::Str(Arc::from("wrong"))].as_slice());
    let warnings = RefCell::new(vec![]);

    check_struct_update_fields(
      &Calcit::Proc(CalcitProc::NativeStructAssoc),
      &args,
      &ScopeTypes::new(),
      "tests.struct",
      "demo",
      &warnings,
    );

    assert!(warnings.borrow().iter().any(|warning| {
      let message = warning.to_string();
      message.contains("struct update field `:value`") && message.contains("expects type `:number`")
    }));
  }

  #[test]
  fn preserves_struct_postfix_method_calls() {
    use crate::calcit::MethodKind;
    use crate::data::cirru::code_to_calcit;
    use cirru_edn::EdnTag;
    use cirru_parser::Cirru;

    let cat_struct = CalcitStructDef::from_fields(EdnTag::from("Cat"), vec![EdnTag::from("name"), EdnTag::from("color")]);

    let expr = Cirru::List(vec![Cirru::leaf("kitty"), Cirru::leaf(".rename"), Cirru::leaf("|LagopusB")]);
    let code = code_to_calcit(&expr, "tests.struct", "demo", vec![]).expect("parse struct method call");

    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("kitty"));

    let mut scope_types: ScopeTypes = ScopeTypes::new();
    scope_types.insert(
      Arc::from("kitty"),
      Arc::new(CalcitTypeAnnotation::StructValue(Arc::new(cat_struct))),
    );

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let result =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.struct", &warnings, &stack).expect("preprocess struct method call");

    let nodes = match result {
      Calcit::List(xs) => xs.to_vec(),
      other => panic!("expected list form, got {other}"),
    };

    assert!(
      !matches!(nodes.first(), Some(Calcit::Proc(CalcitProc::NativeStruct))),
      "method-call path should not be rewritten as a struct constructor"
    );
    assert!(
      matches!(nodes.first(), Some(Calcit::Method(_, MethodKind::Invoke(_)))),
      "method-call should become typed method form"
    );
    assert!(warnings.borrow().is_empty(), "should not warn for valid method-call syntax");
  }

  #[test]
  fn rejects_enum_head_call_with_missing_tag() {
    use crate::calcit::{CalcitEnumDef, CalcitStructValue};
    use crate::data::cirru::code_to_calcit;
    use cirru_edn::EdnTag;
    use cirru_parser::Cirru;

    let enum_struct = CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef::from_fields(
        EdnTag::from("Result"),
        vec![EdnTag::from("ok"), EdnTag::from("err")],
      )),
      values: Arc::new(vec![
        Calcit::List(Arc::new(CalcitList::default())),
        Calcit::List(Arc::new(CalcitList::default())),
      ]),
    };
    let result_enum = CalcitEnumDef::from_struct(enum_struct.clone()).expect("valid result enum");

    let expr = Cirru::List(vec![Cirru::leaf("Result")]);

    let parsed = code_to_calcit(&expr, "tests.struct", "demo", vec![]).expect("parse enum ctor");
    let Calcit::List(parsed_items) = parsed else {
      panic!("expected parsed call");
    };
    let mut code_items = parsed_items.to_vec();
    code_items[0] = Calcit::EnumDef(result_enum.clone());
    let code = Calcit::from(code_items);

    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("Result"));

    let mut scope_types: ScopeTypes = ScopeTypes::new();
    scope_types.insert(
      Arc::from("Result"),
      Arc::new(CalcitTypeAnnotation::Enum(Arc::new(result_enum.clone()), Arc::new(vec![]))),
    );

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let result =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.struct", &warnings, &stack).expect("preprocess enum head call");

    let items = match result {
      Calcit::List(xs) => xs.to_vec(),
      other => panic!("expected list form, got {other}"),
    };

    assert!(
      !matches!(items.first(), Some(Calcit::Proc(CalcitProc::NativeNamedEnumNew))),
      "should not rewrite when enum constructor lacks variant tag"
    );

    let warnings_vec = warnings.borrow();
    assert!(!warnings_vec.is_empty(), "should warn on missing enum variant tag");
    assert!(
      warnings_vec.iter().any(|w| w.to_string().contains("missing variant tag")),
      "warning should mention missing variant tag"
    );
  }

  #[test]
  fn rejects_enum_head_call_with_invalid_tag() {
    use crate::calcit::{CalcitEnumDef, CalcitStructValue};
    use crate::data::cirru::code_to_calcit;
    use cirru_edn::EdnTag;
    use cirru_parser::Cirru;

    let enum_struct = CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef::from_fields(
        EdnTag::from("Result"),
        vec![EdnTag::from("ok"), EdnTag::from("err")],
      )),
      values: Arc::new(vec![
        Calcit::List(Arc::new(CalcitList::default())),
        Calcit::List(Arc::new(CalcitList::default())),
      ]),
    };
    let result_enum = CalcitEnumDef::from_struct(enum_struct.clone()).expect("valid result enum");

    let expr = Cirru::List(vec![Cirru::leaf("Result"), Cirru::leaf(":bad")]);

    let parsed = code_to_calcit(&expr, "tests.struct", "demo", vec![]).expect("parse enum ctor");
    let Calcit::List(parsed_items) = parsed else {
      panic!("expected parsed call");
    };
    let mut code_items = parsed_items.to_vec();
    code_items[0] = Calcit::EnumDef(result_enum.clone());
    let code = Calcit::from(code_items);

    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("Result"));

    let mut scope_types: ScopeTypes = ScopeTypes::new();
    scope_types.insert(
      Arc::from("Result"),
      Arc::new(CalcitTypeAnnotation::Enum(Arc::new(result_enum.clone()), Arc::new(vec![]))),
    );

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let result =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.struct", &warnings, &stack).expect("preprocess enum head call");

    let items = match result {
      Calcit::List(xs) => xs.to_vec(),
      other => panic!("expected list form, got {other}"),
    };

    assert!(
      !matches!(items.first(), Some(Calcit::Proc(CalcitProc::NativeNamedEnumNew))),
      "should not rewrite when enum variant is not valid"
    );

    let warnings_vec = warnings.borrow();
    assert!(!warnings_vec.is_empty(), "should warn on invalid enum variant");
    assert!(
      warnings_vec.iter().any(|w| w.to_string().contains("does not have variant")),
      "warning should mention unknown enum variant"
    );
  }

  #[test]
  fn rejects_enum_head_call_with_non_tag_first_arg() {
    use crate::calcit::{CalcitEnumDef, CalcitStructValue};
    use crate::data::cirru::code_to_calcit;
    use cirru_edn::EdnTag;
    use cirru_parser::Cirru;

    let enum_struct = CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef::from_fields(
        EdnTag::from("Mode"),
        vec![EdnTag::from("dark"), EdnTag::from("light")],
      )),
      values: Arc::new(vec![
        Calcit::List(Arc::new(CalcitList::default())),
        Calcit::List(Arc::new(CalcitList::default())),
      ]),
    };
    let mode_enum = CalcitEnumDef::from_struct(enum_struct.clone()).expect("valid mode enum");

    let expr = Cirru::List(vec![Cirru::leaf("Mode"), Cirru::leaf("dark")]);

    let parsed = code_to_calcit(&expr, "tests.struct", "demo", vec![]).expect("parse enum ctor");
    let Calcit::List(parsed_items) = parsed else {
      panic!("expected parsed call");
    };
    let mut code_items = parsed_items.to_vec();
    code_items[0] = Calcit::EnumDef(mode_enum.clone());
    let code = Calcit::from(code_items);

    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("Mode"));

    let mut scope_types: ScopeTypes = ScopeTypes::new();
    scope_types.insert(
      Arc::from("Mode"),
      Arc::new(CalcitTypeAnnotation::Enum(Arc::new(mode_enum.clone()), Arc::new(vec![]))),
    );

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let result =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.struct", &warnings, &stack).expect("preprocess enum head call");

    let items = match result {
      Calcit::List(xs) => xs.to_vec(),
      other => panic!("expected list form, got {other}"),
    };

    assert!(
      !matches!(items.first(), Some(Calcit::Proc(CalcitProc::NativeNamedEnumNew))),
      "should not rewrite when enum variant prefix is not a tag"
    );

    let warnings_vec = warnings.borrow();
    assert!(!warnings_vec.is_empty(), "should warn on non-tag first arg");
    assert!(
      warnings_vec
        .iter()
        .any(|w| w.to_string().contains("first argument should be a variant tag")),
      "warning should mention non-tag first argument"
    );
  }

  #[test]
  fn warns_on_invalid_struct_field() {
    use cirru_edn::EdnTag;

    // Create a test struct type with fields: name, age
    let test_struct = Arc::new(CalcitTypeAnnotation::StructValue(Arc::new(CalcitStructDef::from_fields(
      EdnTag::from("Person"),
      vec![EdnTag::from("age"), EdnTag::from("name")],
    ))));

    // Test expression: (&struct:get user :email) with user already typed
    let expr = Cirru::List(vec![
      Cirru::leaf("&struct:get"),
      Cirru::leaf("user"),
      Cirru::leaf(":email"), // invalid field
    ]);

    let code = code_to_calcit(&expr, "tests.struct", "demo", vec![]).expect("parse cirru");

    // Set up scope with user variable
    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("user"));

    let mut scope_types: ScopeTypes = ScopeTypes::new();
    // Pre-populate with struct type
    scope_types.insert(Arc::from("user"), test_struct.clone());

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let _resolved =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.struct", &warnings, &stack).expect("preprocess should succeed");

    // Should have a warning about invalid field
    let warnings_vec = warnings.borrow();
    assert!(!warnings_vec.is_empty(), "should have warning for invalid field");
    let warning_msg = warnings_vec[0].to_string();
    assert!(
      warning_msg.contains("email"),
      "warning should mention the invalid field: {warning_msg}"
    );
    assert!(
      warning_msg.contains("Person"),
      "warning should mention the struct type: {warning_msg}"
    );
  }

  #[test]
  fn rewrites_method_call_when_class_and_method_are_known() {
    use cirru_edn::EdnTag;

    let expr = Cirru::List(vec![Cirru::leaf(".greet"), Cirru::leaf("user")]);
    let code = code_to_calcit(&expr, "tests.method", "demo", vec![]).expect("parse cirru");

    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("user"));
    let mut scope_types: ScopeTypes = ScopeTypes::new();

    let method_import = Calcit::Import(CalcitImport {
      ns: Arc::from("tests.method.ns"),
      def: Arc::from("greet"),
      info: Arc::new(ImportInfo::SameFile { at_def: Arc::from("demo") }),
      def_id: None,
    });

    let method_impl = CalcitImpl {
      name: EdnTag::from("Greeter"),
      origin: None,
      fields: Arc::new(vec![EdnTag::from("greet")]),
      values: Arc::new(vec![method_import.clone()]),
    };

    let class_struct = CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef {
        name: EdnTag::from("Greeter"),
        fields: Arc::new(vec![EdnTag::from("greet")]),
        field_types: Arc::new(vec![calcit::DYNAMIC_TYPE.clone()]),
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        impls: vec![Arc::new(method_impl)],
      }),
      values: Arc::new(vec![method_import.clone()]),
    };
    scope_types.insert(
      Arc::from("user"),
      Arc::new(CalcitTypeAnnotation::StructValue(class_struct.struct_ref.clone())),
    );

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let resolved =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.method", &warnings, &stack).expect("preprocess method call");

    let nodes = match resolved {
      Calcit::List(xs) => xs.to_vec(),
      other => panic!("expected list form, got {other}"),
    };

    assert!(
      matches!(nodes.first(), Some(Calcit::Import(_))),
      "method head should be rewritten to import"
    );
    assert_eq!(nodes.len(), 2, "call should keep receiver argument");
  }

  #[test]
  fn validates_method_field_access() {
    use cirru_edn::EdnTag;

    // Create a test struct type with fields: name, age
    let test_struct = Arc::new(CalcitTypeAnnotation::StructValue(Arc::new(CalcitStructDef::from_fields(
      EdnTag::from("Person"),
      vec![EdnTag::from("age"), EdnTag::from("name")],
    ))));

    // Test expression: (user.-name) - wrapped in a list to trigger method parsing
    let expr = Cirru::List(vec![Cirru::leaf("user.-name")]);

    let code = code_to_calcit(&expr, "tests.struct", "demo", vec![]).expect("parse cirru");

    // Set up scope with user variable
    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("user"));

    let mut scope_types: ScopeTypes = ScopeTypes::new();
    // Pre-populate with struct type
    scope_types.insert(Arc::from("user"), test_struct.clone());

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let _resolved =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.struct", &warnings, &stack).expect("preprocess should succeed");

    // Should not have warnings for valid field
    let warnings_vec = warnings.borrow();
    assert!(
      warnings_vec.is_empty(),
      "should not have warnings for valid field access, got: {warnings_vec:?}"
    );
  }

  #[test]
  fn warns_on_trait_impl_method_tag_syntax() {
    let _lock = lock_preprocess_test_state();
    let _warn_guard = WarnDynMethodGuard::new(true);

    let expr = Cirru::List(vec![
      Cirru::leaf("defimpl"),
      Cirru::leaf("MyFooImpl"),
      Cirru::leaf("MyFoo"),
      Cirru::List(vec![Cirru::leaf(":foo"), Cirru::leaf("myfoo:foo")]),
    ]);
    let code = code_to_calcit(&expr, "tests.trait", "demo", vec![]).expect("parse cirru");

    let args = match code {
      Calcit::List(xs) => xs.drop_left(),
      other => panic!("expected list form, got {other}"),
    };

    let macro_info = CalcitMacro {
      name: Arc::from("defimpl"),
      def_ns: Arc::from(calcit::CORE_NS),
      args: Arc::new(vec![]),
      body: Arc::new(vec![]),
      signature: Arc::new(strict_macro_signature(vec![], vec![], None, MacroExpansionType::Dynamic)),
    };

    let warnings = RefCell::new(vec![]);

    warn_on_trait_impl_method_tag_syntax(&macro_info, &args, "tests.trait", "demo", &warnings);

    let warning_msgs: Vec<String> = warnings.borrow().iter().map(|w| w.to_string()).collect();
    assert!(
      warning_msgs
        .iter()
        .any(|msg| msg.contains("defimpl") && msg.contains("legacy tag style") && msg.contains(".foo")),
      "expected migration warning for trait/impl method key, got: {warning_msgs:?}"
    );
  }

  #[test]
  fn rejects_legacy_hint_fn_clause_syntax() {
    use crate::data::cirru::code_to_calcit;

    let hint_form = Cirru::List(vec![
      Cirru::leaf("hint-fn"),
      Cirru::List(vec![Cirru::leaf("return-type"), Cirru::leaf(":string")]),
      Cirru::List(vec![
        Cirru::leaf("generics"),
        Cirru::List(vec![Cirru::leaf("quote"), Cirru::leaf("T")]),
      ]),
    ]);
    let hint = code_to_calcit(&hint_form, "tests.hint", "demo", vec![]).expect("parse cirru");

    let scope_defs: HashSet<Arc<str>> = HashSet::new();
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let err = preprocess_expr(&hint, &scope_defs, &mut scope_types, "tests.hint", &warnings, &stack)
      .expect_err("legacy hint-fn clauses should be rejected");
    let msg = err.msg;
    assert!(
      msg.contains("legacy hint-fn clauses are no longer supported") && msg.contains("return-type") && msg.contains("generics"),
      "expected hard error for legacy hint-fn clauses, got: {msg}"
    );
  }

  #[test]
  fn accepts_schema_hint_fn_syntax() {
    use crate::data::cirru::code_to_calcit;

    let hint_form = Cirru::List(vec![
      Cirru::leaf("hint-fn"),
      Cirru::List(vec![
        Cirru::leaf("{}"),
        Cirru::List(vec![Cirru::leaf(":args"), Cirru::List(vec![Cirru::leaf("[]")])]),
        Cirru::List(vec![Cirru::leaf(":return"), Cirru::leaf(":string")]),
      ]),
    ]);
    let hint = code_to_calcit(&hint_form, "tests.hint", "demo", vec![]).expect("parse cirru");

    let scope_defs: HashSet<Arc<str>> = HashSet::new();
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let result = preprocess_expr(&hint, &scope_defs, &mut scope_types, "tests.hint", &warnings, &stack);

    assert!(result.is_ok(), "schema hint-fn should preprocess successfully: {result:?}");
  }

  #[test]
  fn local_hint_fn_refines_later_function_references() {
    use crate::data::cirru::code_to_calcit;

    let hint_form = Cirru::List(vec![
      Cirru::leaf("hint-fn"),
      Cirru::leaf("f"),
      Cirru::List(vec![
        Cirru::leaf("{}"),
        Cirru::List(vec![
          Cirru::leaf(":args"),
          Cirru::List(vec![Cirru::leaf("[]"), Cirru::leaf(":number")]),
        ]),
        Cirru::List(vec![Cirru::leaf(":return"), Cirru::leaf(":number")]),
      ]),
    ]);
    let hint = code_to_calcit(&hint_form, "tests.hint", "demo", vec![]).expect("parse hint-fn");

    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("f"));
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let resolved_hint =
      preprocess_expr(&hint, &scope_defs, &mut scope_types, "tests.hint", &warnings, &stack).expect("preprocess local hint-fn");
    let annotation = scope_types.get("f").expect("hint-fn should refine the lexical binding");
    let CalcitTypeAnnotation::Fn(fn_annotation) = annotation.as_ref() else {
      panic!("expected complete function annotation, got {annotation:?}");
    };
    assert!(matches!(fn_annotation.arg_types.as_slice(), [arg] if matches!(arg.as_ref(), CalcitTypeAnnotation::Number)));
    assert!(matches!(fn_annotation.return_type.as_ref(), CalcitTypeAnnotation::Number));
    assert!(
      matches!(resolved_hint, Calcit::List(ref xs) if matches!(xs.get(1), Some(Calcit::Local(local)) if matches!(local.type_info.as_ref(), CalcitTypeAnnotation::Fn(_)))),
      "the annotation target should carry the refined type"
    );

    let call = code_to_calcit(&Cirru::List(vec![Cirru::leaf("f"), Cirru::leaf("1")]), "tests.hint", "demo", vec![])
      .expect("parse local function call");
    let resolved_call =
      preprocess_expr(&call, &scope_defs, &mut scope_types, "tests.hint", &warnings, &stack).expect("preprocess local function call");
    let inferred = infer_type_from_expr(&resolved_call, &scope_types).expect("infer hinted local call return");
    assert!(matches!(inferred.as_ref(), CalcitTypeAnnotation::Number));
  }

  #[test]
  fn body_hint_fn_fills_omitted_parameter_types() {
    use crate::data::cirru::code_to_calcit;

    let schema = Cirru::List(vec![
      Cirru::leaf("{}"),
      Cirru::List(vec![Cirru::leaf(":return"), Cirru::leaf(":number")]),
    ]);
    let expr = Cirru::List(vec![
      Cirru::leaf("&let"),
      Cirru::List(vec![
        Cirru::leaf("f"),
        Cirru::List(vec![
          Cirru::leaf("defn"),
          Cirru::leaf("f%"),
          Cirru::List(vec![Cirru::leaf("x")]),
          Cirru::List(vec![Cirru::leaf("hint-fn"), schema]),
          Cirru::List(vec![Cirru::leaf("&+"), Cirru::leaf("x"), Cirru::leaf("1")]),
        ]),
      ]),
      Cirru::List(vec![Cirru::leaf("f"), Cirru::leaf("1")]),
    ]);
    let code = code_to_calcit(&expr, "tests.hint", "demo", vec![]).expect("parse local function");
    let scope_defs: HashSet<Arc<str>> = HashSet::new();
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let resolved =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.hint", &warnings, &stack).expect("preprocess body hint-fn");
    let inferred = infer_type_from_expr(&resolved, &scope_types).expect("infer hinted anonymous function call");
    assert!(
      matches!(inferred.as_ref(), CalcitTypeAnnotation::Number),
      "expected number return from body-hinted function, got {inferred:?}; resolved: {resolved}"
    );
    assert!(warnings.borrow().is_empty(), "valid body hint should not emit warnings");
  }

  #[test]
  fn named_body_hint_parameter_keeps_its_declared_type() {
    let labelled = Arc::new(CalcitTypeAnnotation::TypeRef(
      Arc::from("n"),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]),
    ));
    let parameter = Arc::from("n");

    assert!(matches!(
      unwrap_named_body_parameter_type(labelled, Some(&parameter)).as_ref(),
      CalcitTypeAnnotation::Number
    ));
  }

  #[test]
  fn warns_on_invalid_method_field_access() {
    use cirru_edn::EdnTag;

    // Create a test struct type with fields: name, age
    let test_struct = Arc::new(CalcitTypeAnnotation::StructValue(Arc::new(CalcitStructDef::from_fields(
      EdnTag::from("Person"),
      vec![EdnTag::from("age"), EdnTag::from("name")],
    ))));

    // Test expression: (user.-email) - invalid field, wrapped in list
    let expr = Cirru::List(vec![Cirru::leaf("user.-email")]);

    let code = code_to_calcit(&expr, "tests.struct", "demo", vec![]).expect("parse cirru");

    // Set up scope with user variable
    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("user"));

    let mut scope_types: ScopeTypes = ScopeTypes::new();
    // Pre-populate with struct type
    scope_types.insert(Arc::from("user"), test_struct.clone());

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let _resolved =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.struct", &warnings, &stack).expect("preprocess should succeed");

    // Should have a warning about invalid field
    let warnings_vec = warnings.borrow();
    assert!(!warnings_vec.is_empty(), "should have warning for invalid field");

    let warning_msg = warnings_vec[0].to_string();
    assert!(
      warning_msg.contains("email"),
      "warning should mention the invalid field: {warning_msg}"
    );
    assert!(
      warning_msg.contains("Person"),
      "warning should mention the struct type: {warning_msg}"
    );
  }

  #[test]
  fn rejects_method_on_struct_without_field() {
    use cirru_edn::EdnTag;

    // Create a test struct type with limited methods
    let test_struct = Arc::new(CalcitTypeAnnotation::StructValue(Arc::new(CalcitStructDef::from_fields(
      EdnTag::from("Person"),
      vec![EdnTag::from("age"), EdnTag::from("name")],
    ))));

    // Test expression: (.slice person 1 3) - trying to call non-existent method
    let expr = Cirru::List(vec![
      Cirru::leaf(".slice"),
      Cirru::leaf("person"),
      Cirru::leaf("1"),
      Cirru::leaf("3"),
    ]);

    let code = code_to_calcit(&expr, "tests.method", "demo", vec![]).expect("parse cirru");

    // Set up scope with person variable
    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("person"));

    let mut scope_types: ScopeTypes = ScopeTypes::new();
    // Pre-populate with struct type
    scope_types.insert(Arc::from("person"), test_struct.clone());

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let result = preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.method", &warnings, &stack);
    assert!(result.is_err(), "preprocess should reject method call on struct without that field");
    if let Err(err) = result {
      let msg = format!("{err}");
      assert!(msg.contains(".slice"), "error should mention the method name: {msg}");
      assert!(
        msg.contains("Person") || msg.contains("struct"),
        "error should mention the struct type: {msg}"
      );
    }
  }

  #[test]
  fn string_method_receiver_hint_names_inferred_type_and_function_form() {
    let receiver_type = Arc::new(CalcitTypeAnnotation::StructValue(Arc::new(CalcitStructDef::from_fields(
      EdnTag::from("MapLike"),
      vec![],
    ))));
    let head = Calcit::Method(Arc::from("trim"), calcit::MethodKind::Invoke(receiver_type.clone()));
    let receiver = Calcit::Local(CalcitLocal {
      idx: 0,
      sym: Arc::from("value"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.method"),
        at_def: Arc::from("demo"),
      }),
      location: None,
      type_info: receiver_type,
    });
    let args = CalcitList::from(&[receiver][..]);

    let error = validate_method_call(&head, &args, &ScopeTypes::new(), &CallStackList::default())
      .expect_err("trim should reject a non-String receiver through method validation");
    let message = error.to_string();

    assert!(message.contains("requires a String receiver"), "message: {message}");
    assert!(message.contains("inferred as `struct MapLike`"), "message: {message}");
    assert!(message.contains("`(trim receiver)`"), "message: {message}");
  }

  #[test]
  fn show_on_builtin_value_is_a_type_error_but_debug_is_a_typed_method() {
    let show_expr = Cirru::List(vec![Cirru::leaf("1"), Cirru::leaf(".show")]);
    let show_code = code_to_calcit(&show_expr, "tests.method", "demo", vec![]).expect("parse show call");
    let warnings = RefCell::new(vec![]);
    let mut scope_types = ScopeTypes::new();
    let show_error = preprocess_expr(
      &show_code,
      &HashSet::new(),
      &mut scope_types,
      "tests.method",
      &warnings,
      &CallStackList::default(),
    )
    .expect_err("a Number must not receive the opt-in Show method");
    assert!(show_error.to_string().contains("unknown method `.show`"), "error: {show_error}");

    let debug_expr = Cirru::List(vec![Cirru::leaf("1"), Cirru::leaf(".debug")]);
    let debug_code = code_to_calcit(&debug_expr, "tests.method", "demo", vec![]).expect("parse debug call");
    let debug_value = preprocess_expr(
      &debug_code,
      &HashSet::new(),
      &mut scope_types,
      "tests.method",
      &warnings,
      &CallStackList::default(),
    )
    .expect("a Number should receive the built-in Debug method");
    let Calcit::List(debug_call) = debug_value else {
      panic!("debug call should stay a list");
    };
    assert!(matches!(debug_call.first(), Some(Calcit::Method(name, calcit::MethodKind::Invoke(_))) if name.as_ref() == "debug"));
  }

  #[test]
  fn option_mismatch_between_nominal_payloads_does_not_suggest_unwrap() {
    let option_number = Arc::new(CalcitTypeAnnotation::TypeRef(
      Arc::from("Option"),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]),
    ));
    let option_string = Arc::new(CalcitTypeAnnotation::TypeRef(
      Arc::from("Option"),
      Arc::new(vec![Arc::new(CalcitTypeAnnotation::String)]),
    ));
    let fn_info = CalcitFn {
      name: Arc::from("expects-option-number"),
      def_ns: Arc::from("tests.option"),
      def_ref: None,
      usage: crate::calcit::CalcitFnUsageMeta::default(),
      scope: Arc::new(CalcitScope::default()),
      args: Arc::new(CalcitFnArgs::Args(vec![0])),
      call_shape: crate::calcit::CalcitFnCallShape::fixed(1),
      body: vec![Calcit::Nil],
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![option_number],
      return_type: crate::calcit::DYNAMIC_TYPE.clone(),
      rest_type: None,
    };
    let args = CalcitList::from(
      &[Calcit::Local(CalcitLocal {
        idx: 0,
        sym: Arc::from("value"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.option"),
          at_def: Arc::from("demo"),
        }),
        location: None,
        type_info: option_string,
      })][..],
    );
    let head = Calcit::Import(CalcitImport {
      ns: Arc::from("tests.option"),
      def: Arc::from("expects-option-number"),
      info: Arc::new(ImportInfo::NsReferDef {
        at_ns: Arc::from("tests.option"),
        at_def: Arc::from("demo"),
      }),
      def_id: None,
    });
    let warnings = RefCell::new(vec![]);
    let call_info = CallTypeCheckInfo {
      file_ns: "tests.option",
      def_name: "demo",
      call_location: None,
    };

    check_user_fn_arg_types(&fn_info, &head, &args, &ScopeTypes::new(), &call_info, &warnings);

    let warnings = warnings.borrow();
    let warning = warnings.first().expect("Option payload mismatch should warn");
    assert_eq!(warning.code(), Some("W_FN_ARG_TYPE_MISMATCH"));
    assert!(!warning.message().contains("option:unwrap-or"), "warning: {warning:?}");
    assert!(!warning.message().contains("tag-match"), "warning: {warning:?}");
  }

  #[test]
  fn checks_user_function_arg_types() {
    // Test the check_user_fn_arg_types function directly
    let fn_info = CalcitFn {
      name: Arc::from("demo-fn"),
      def_ns: Arc::from("tests.user_fn"),
      def_ref: None,
      usage: crate::calcit::CalcitFnUsageMeta::default(),
      scope: Arc::new(CalcitScope::default()),
      args: Arc::new(CalcitFnArgs::Args(vec![0, 1])), // two args
      call_shape: crate::calcit::CalcitFnCallShape::fixed(2),
      body: vec![Calcit::Nil],
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![
        Arc::new(CalcitTypeAnnotation::from_tag_name("number")),
        Arc::new(CalcitTypeAnnotation::from_tag_name("string")),
      ],
      return_type: crate::calcit::DYNAMIC_TYPE.clone(),
      rest_type: None,
    };

    // Create arguments: ("|hello" 42) - reversed types
    let args = CalcitList::from(
      &vec![
        Calcit::Str(Arc::from("hello")), // string
        Calcit::Number(42.0),            // number
      ][..],
    );

    let scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);

    let dummy_head = Calcit::Import(CalcitImport {
      ns: Arc::from("tests.user_fn"),
      def: Arc::from("demo-fn"),
      info: Arc::new(ImportInfo::NsReferDef {
        at_ns: Arc::from("tests.user_fn"),
        at_def: Arc::from("demo-fn"),
      }),
      def_id: None,
    });
    let call_info = CallTypeCheckInfo {
      file_ns: "tests.user_fn",
      def_name: "demo",
      call_location: None,
    };
    check_user_fn_arg_types(&fn_info, &dummy_head, &args, &scope_types, &call_info, &warnings);

    // Should have warnings about type mismatches
    let warnings_vec = warnings.borrow();

    assert!(
      warnings_vec.len() >= 2,
      "should have at least 2 warnings for arg type mismatches, got {} warnings: {:?}",
      warnings_vec.len(),
      warnings_vec.iter().map(|w| w.to_string()).collect::<Vec<_>>()
    );

    // Check first warning (arg 1: expected number, got string)
    let warning1 = warnings_vec.iter().find(|w| w.to_string().contains("arg 1"));
    assert!(
      warning1.is_some(),
      "should have warning for arg 1, warnings: {:?}",
      warnings_vec.iter().map(|w| w.to_string()).collect::<Vec<_>>()
    );
    let msg1 = warning1.unwrap().to_string();
    assert!(
      msg1.contains("number") || msg1.contains(":number"),
      "warning should mention expected type: {msg1}"
    );
    assert!(
      msg1.contains("string") || msg1.contains(":string"),
      "warning should mention actual type: {msg1}"
    );

    // Check second warning (arg 2: expected string, got number)
    let warning2 = warnings_vec.iter().find(|w| w.to_string().contains("arg 2"));
    assert!(
      warning2.is_some(),
      "should have warning for arg 2, warnings: {:?}",
      warnings_vec.iter().map(|w| w.to_string()).collect::<Vec<_>>()
    );
    let msg2 = warning2.unwrap().to_string();
    assert!(
      msg2.contains("string") || msg2.contains(":string"),
      "warning should mention expected type: {msg2}"
    );
    assert!(
      msg2.contains("number") || msg2.contains(":number"),
      "warning should mention actual type: {msg2}"
    );
  }

  #[test]
  fn user_function_arg_warning_falls_back_to_call_location_for_literal_args() {
    let fn_info = CalcitFn {
      name: Arc::from("plus1"),
      def_ns: Arc::from("tests.user_fn"),
      def_ref: None,
      usage: CalcitFnUsageMeta::default(),
      scope: Arc::new(CalcitScope::default()),
      args: Arc::new(CalcitFnArgs::Args(vec![0])),
      call_shape: crate::calcit::CalcitFnCallShape::fixed(1),
      body: vec![Calcit::Nil],
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![Arc::new(CalcitTypeAnnotation::from_tag_name("number"))],
      return_type: crate::calcit::DYNAMIC_TYPE.clone(),
      rest_type: None,
    };

    let expr = Cirru::List(vec![Cirru::leaf("plus1"), Cirru::leaf(":tag")]);
    let code = code_to_calcit(&expr, "tests.user_fn", "demo", vec![9]).expect("parse cirru");
    let Calcit::List(items) = code else {
      panic!("expected list call");
    };
    let head = items.first().expect("call head");
    let args = items.drop_left();
    let call_location = derive_call_expr_location(head).expect("source-backed call location");

    let warnings = RefCell::new(vec![]);
    let dummy_head = Calcit::Import(CalcitImport {
      ns: Arc::from("tests.user_fn"),
      def: Arc::from("plus1"),
      info: Arc::new(ImportInfo::NsReferDef {
        at_ns: Arc::from("tests.user_fn"),
        at_def: Arc::from("demo"),
      }),
      def_id: None,
    });

    let call_info = CallTypeCheckInfo {
      file_ns: "tests.user_fn",
      def_name: "demo",
      call_location: Some(call_location.clone()),
    };
    check_user_fn_arg_types(&fn_info, &dummy_head, &args, &ScopeTypes::new(), &call_info, &warnings);

    let warnings_vec = warnings.borrow();
    assert_eq!(warnings_vec.len(), 1, "expected one warning, got: {warnings_vec:?}");
    assert_eq!(warnings_vec[0].location(), &call_location);
    assert_eq!(warnings_vec[0].location().coord.as_ref(), &vec![9]);
  }

  #[test]
  fn user_function_where_bounds_warn_on_missing_trait_impl() {
    let show_trait = Arc::new(crate::calcit::CalcitTrait::new(
      EdnTag::new("Renderable"),
      vec![EdnTag::new("show")],
      vec![crate::calcit::DYNAMIC_TYPE.clone()],
    ));
    let hint_generics = Calcit::List(Arc::new(CalcitList::from(&[
      Calcit::Symbol {
        sym: Arc::from("[]"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.where"),
          at_def: Arc::from("print-it"),
        }),
        location: None,
      },
      Calcit::Symbol {
        sym: Arc::from("T"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.where"),
          at_def: Arc::from("print-it"),
        }),
        location: None,
      },
    ])));
    let where_map = Calcit::List(Arc::new(CalcitList::from(&[
      Calcit::Symbol {
        sym: Arc::from("{}"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.where"),
          at_def: Arc::from("print-it"),
        }),
        location: None,
      },
      Calcit::List(Arc::new(CalcitList::from(&[
        Calcit::Symbol {
          sym: Arc::from("T"),
          info: Arc::new(CalcitSymbolInfo {
            at_ns: Arc::from("tests.where"),
            at_def: Arc::from("print-it"),
          }),
          location: None,
        },
        Calcit::Trait((*show_trait).clone()),
      ]))),
    ])));
    let hint_schema = Calcit::List(Arc::new(CalcitList::from(&[
      Calcit::Symbol {
        sym: Arc::from("{}"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.where"),
          at_def: Arc::from("print-it"),
        }),
        location: None,
      },
      Calcit::List(Arc::new(CalcitList::from(&[Calcit::tag("generics"), hint_generics]))),
      Calcit::List(Arc::new(CalcitList::from(&[Calcit::tag("where"), where_map]))),
    ])));
    let hint_form = Calcit::List(Arc::new(CalcitList::from(&[
      Calcit::Syntax(CalcitSyntax::HintFn, Arc::from("tests.where")),
      hint_schema,
    ])));

    let fn_info = CalcitFn {
      name: Arc::from("print-it"),
      def_ns: Arc::from("tests.where"),
      def_ref: None,
      usage: crate::calcit::CalcitFnUsageMeta::default(),
      scope: Arc::new(CalcitScope::default()),
      args: Arc::new(CalcitFnArgs::Args(vec![0])),
      call_shape: crate::calcit::CalcitFnCallShape::fixed(1),
      body: vec![hint_form],
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(vec![crate::calcit::CalcitGenericBound {
        name: Arc::from("T"),
        traits: Arc::new(vec![show_trait.clone()]),
      }]),
      arg_types: vec![Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")))],
      return_type: crate::calcit::DYNAMIC_TYPE.clone(),
      rest_type: None,
    };

    let arg_local = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from("value")),
      sym: Arc::from("value"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.where"),
        at_def: Arc::from("demo"),
      }),
      location: None,
      type_info: crate::calcit::DYNAMIC_TYPE.clone(),
    });
    let args = CalcitList::from(&[arg_local] as &[Calcit]);

    let mut shown_struct = crate::calcit::CalcitStructDef::from_fields(EdnTag::new("Shown"), vec![EdnTag::new("name")]);
    shown_struct.impls = vec![Arc::new(crate::calcit::CalcitImpl {
      name: EdnTag::new("ShowImpl"),
      origin: Some(show_trait.clone()),
      fields: Arc::new(vec![EdnTag::new("show")]),
      values: Arc::new(vec![Calcit::Nil]),
    })];
    let plain_struct = crate::calcit::CalcitStructDef::from_fields(EdnTag::new("Plain"), vec![EdnTag::new("name")]);

    let mut ok_scope_types: ScopeTypes = ScopeTypes::new();
    ok_scope_types.insert(
      Arc::from("value"),
      Arc::new(CalcitTypeAnnotation::Struct(Arc::new(shown_struct), Arc::new(vec![]))),
    );
    let ok_warnings = RefCell::new(vec![]);
    let dummy_head = Calcit::Import(CalcitImport {
      ns: Arc::from("tests.where"),
      def: Arc::from("print-it"),
      info: Arc::new(ImportInfo::NsReferDef {
        at_ns: Arc::from("tests.where"),
        at_def: Arc::from("demo"),
      }),
      def_id: None,
    });
    let call_info = CallTypeCheckInfo {
      file_ns: "tests.where",
      def_name: "demo",
      call_location: None,
    };
    check_user_fn_arg_types(&fn_info, &dummy_head, &args, &ok_scope_types, &call_info, &ok_warnings);
    assert!(
      ok_warnings.borrow().is_empty(),
      "satisfied where-bound should not warn: {:?}",
      ok_warnings.borrow()
    );

    let mut bad_scope_types: ScopeTypes = ScopeTypes::new();
    bad_scope_types.insert(
      Arc::from("value"),
      Arc::new(CalcitTypeAnnotation::Struct(Arc::new(plain_struct), Arc::new(vec![]))),
    );
    let bad_warnings = RefCell::new(vec![]);
    check_user_fn_arg_types(&fn_info, &dummy_head, &args, &bad_scope_types, &call_info, &bad_warnings);
    let warnings_vec = bad_warnings.borrow();
    assert_eq!(
      warnings_vec.len(),
      1,
      "missing trait impl should emit one warning: {warnings_vec:?}"
    );
    let message = warnings_vec[0].to_string();
    assert!(
      message.contains("trait bound") && message.contains("Renderable"),
      "warning should mention missing where-bound: {message}"
    );
  }

  #[test]
  fn local_function_where_bounds_warn_on_missing_trait_impl() {
    let show_trait = Arc::new(crate::calcit::CalcitTrait::new(
      EdnTag::new("Renderable"),
      vec![EdnTag::new("show")],
      vec![crate::calcit::DYNAMIC_TYPE.clone()],
    ));

    let local_fn = CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from("printer")),
      sym: Arc::from("printer"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.where"),
        at_def: Arc::from("demo"),
      }),
      location: None,
      type_info: Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
        generics: Arc::new(vec![Arc::from("T")]),
        where_bounds: Arc::new(vec![crate::calcit::CalcitGenericBound {
          name: Arc::from("T"),
          traits: Arc::new(vec![show_trait.clone()]),
        }]),
        arg_types: vec![Arc::new(CalcitTypeAnnotation::TypeVar(Arc::from("T")))],
        return_type: crate::calcit::DYNAMIC_TYPE.clone(),
        fn_kind: SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(HashSet::new()),
      }))),
    };
    let head_form = Calcit::Local(local_fn.clone());

    let arg_local = Calcit::Local(CalcitLocal {
      idx: CalcitLocal::track_sym(&Arc::from("value")),
      sym: Arc::from("value"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.where"),
        at_def: Arc::from("demo"),
      }),
      location: None,
      type_info: crate::calcit::DYNAMIC_TYPE.clone(),
    });
    let args = CalcitList::from(&[arg_local] as &[Calcit]);

    let mut shown_struct = crate::calcit::CalcitStructDef::from_fields(EdnTag::new("Shown"), vec![EdnTag::new("name")]);
    shown_struct.impls = vec![Arc::new(CalcitImpl {
      name: EdnTag::new("ShowImpl"),
      origin: Some(show_trait.clone()),
      fields: Arc::new(vec![EdnTag::new("show")]),
      values: Arc::new(vec![Calcit::Nil]),
    })];
    let plain_struct = crate::calcit::CalcitStructDef::from_fields(EdnTag::new("Plain"), vec![EdnTag::new("name")]);

    let call_info = CallTypeCheckInfo {
      file_ns: "tests.where",
      def_name: "demo",
      call_location: None,
    };

    let mut ok_scope_types: ScopeTypes = ScopeTypes::new();
    ok_scope_types.insert(
      Arc::from("value"),
      Arc::new(CalcitTypeAnnotation::Struct(Arc::new(shown_struct), Arc::new(vec![]))),
    );
    let ok_warnings = RefCell::new(vec![]);
    check_local_fn_call_arg_types(&head_form, &local_fn, &args, &ok_scope_types, &call_info, &ok_warnings);
    assert!(
      ok_warnings.borrow().is_empty(),
      "satisfied local fn where-bound should not warn: {:?}",
      ok_warnings.borrow()
    );

    let mut bad_scope_types: ScopeTypes = ScopeTypes::new();
    bad_scope_types.insert(
      Arc::from("value"),
      Arc::new(CalcitTypeAnnotation::Struct(Arc::new(plain_struct), Arc::new(vec![]))),
    );
    let bad_warnings = RefCell::new(vec![]);
    check_local_fn_call_arg_types(&head_form, &local_fn, &args, &bad_scope_types, &call_info, &bad_warnings);
    let warnings_vec = bad_warnings.borrow();
    assert_eq!(
      warnings_vec.len(),
      1,
      "missing local fn trait impl should emit one warning: {warnings_vec:?}"
    );
    let message = warnings_vec[0].to_string();
    assert!(
      message.contains("trait bound") && message.contains("Renderable"),
      "local fn warning should mention missing where-bound: {message}"
    );
  }

  #[test]
  fn checks_function_return_type() {
    use crate::data::cirru::code_to_calcit;
    use cirru_parser::Cirru;

    // Test defn with wrong return type
    // (defn wrong-ret () (hint-fn ({} (:return :string))) (&+ 1 2))
    // Should return :number but declares :string
    let expr = Cirru::List(vec![
      Cirru::leaf("defn"),
      Cirru::leaf("wrong-ret"),
      Cirru::List(vec![]), // no args
      Cirru::List(vec![
        // (hint-fn ({} (:return :string)))
        Cirru::leaf("hint-fn"),
        Cirru::List(vec![
          Cirru::leaf("{}"),
          Cirru::List(vec![Cirru::leaf(":return"), Cirru::leaf(":string")]),
        ]),
      ]),
      Cirru::List(vec![
        // (&+ 1 2) - returns :number
        Cirru::leaf("&+"),
        Cirru::leaf("1"),
        Cirru::leaf("2"),
      ]),
    ]);

    let code = code_to_calcit(&expr, "tests.return_type", "demo", vec![]).expect("parse cirru");

    let scope_defs: HashSet<Arc<str>> = HashSet::new();
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    // Preprocess the defn expression
    let _result = preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.return_type", &warnings, &stack);

    // Should have warning about return type mismatch
    let warnings_vec = warnings.borrow();
    assert!(!warnings_vec.is_empty(), "should have warning for return type mismatch");

    let warning_msg = warnings_vec[0].to_string();
    assert!(
      warning_msg.contains("return") && warning_msg.contains("type"),
      "warning should mention return type: {warning_msg}"
    );
    assert!(
      warning_msg.contains("string") || warning_msg.contains(":string"),
      "warning should mention declared type: {warning_msg}"
    );
    assert!(
      warning_msg.contains("number") || warning_msg.contains(":number"),
      "warning should mention actual type: {warning_msg}"
    );
  }

  #[test]
  fn todo_placeholder_emits_todo_warning_without_return_type_mismatch() {
    use crate::data::cirru::code_to_calcit;
    use cirru_parser::Cirru;

    let expr = Cirru::List(vec![
      Cirru::leaf("defn"),
      Cirru::leaf("unfinished"),
      Cirru::List(vec![]),
      Cirru::List(vec![
        Cirru::leaf("hint-fn"),
        Cirru::List(vec![
          Cirru::leaf("{}"),
          Cirru::List(vec![Cirru::leaf(":return"), Cirru::leaf(":string")]),
        ]),
      ]),
      Cirru::List(vec![Cirru::leaf("todo!"), Cirru::leaf("|write this")]),
    ]);
    let code = code_to_calcit(&expr, "tests.todo", "unfinished", vec![]).expect("parse todo expression");
    let mut scope_types = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);

    preprocess_expr(
      &code,
      &HashSet::new(),
      &mut scope_types,
      "tests.todo",
      &warnings,
      &CallStackList::default(),
    )
    .expect("preprocess todo expression");

    let warning_messages = warnings.borrow().iter().map(ToString::to_string).collect::<Vec<_>>();
    assert!(
      warning_messages
        .iter()
        .any(|message| message.contains("W_TODO") && message.contains("write this")),
      "todo warning missing: {warning_messages:?}"
    );
    assert!(
      !warning_messages.iter().any(|message| message.contains("W_FN_RETURN_TYPE_MISMATCH")),
      "todo should be accepted for any declared return type: {warning_messages:?}"
    );
  }

  #[test]
  fn todo_placeholder_cannot_be_shadowed_by_a_local_binding() {
    use crate::data::cirru::code_to_calcit;
    use cirru_parser::Cirru;

    let code =
      code_to_calcit(&Cirru::List(vec![Cirru::leaf("todo!")]), "tests.todo", "shadowed", vec![]).expect("parse todo expression");
    let scope_defs = HashSet::from([Arc::from("todo!")]);
    let mut scope_types = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let processed = preprocess_expr(
      &code,
      &scope_defs,
      &mut scope_types,
      "tests.todo",
      &warnings,
      &CallStackList::default(),
    )
    .expect("preprocess todo expression");

    assert!(
      matches!(processed, Calcit::List(ref items) if matches!(items.first(), Some(Calcit::Proc(CalcitProc::Todo)))),
      "todo should remain a compiler-known proc: {processed}"
    );
    assert!(warnings.borrow().iter().any(|warning| warning.to_string().contains("W_TODO")));
  }

  #[test]
  fn todo_placeholder_requires_a_static_string_message() {
    use crate::data::cirru::code_to_calcit;
    use cirru_parser::Cirru;

    let code = code_to_calcit(
      &Cirru::List(vec![Cirru::leaf("todo!"), Cirru::leaf("1")]),
      "tests.todo",
      "message",
      vec![],
    )
    .expect("parse todo expression");
    let mut scope_types = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let error = preprocess_expr(
      &code,
      &HashSet::new(),
      &mut scope_types,
      "tests.todo",
      &warnings,
      &CallStackList::default(),
    )
    .expect_err("non-literal todo! message should be rejected before code generation");
    assert!(error.to_string().contains("static String"), "unexpected error: {error}");
    assert!(warnings.borrow().is_empty(), "invalid todo! should not emit a completion warning");
  }

  #[test]
  fn checks_function_return_type_from_if_expression() {
    use crate::data::cirru::code_to_calcit;
    use cirru_parser::Cirru;

    // (defn wrong-ret-if ()
    //   (hint-fn ({} (:return :string)))
    //   (if true 1 2))
    let expr = Cirru::List(vec![
      Cirru::leaf("defn"),
      Cirru::leaf("wrong-ret-if"),
      Cirru::List(vec![]),
      Cirru::List(vec![
        Cirru::leaf("hint-fn"),
        Cirru::List(vec![
          Cirru::leaf("{}"),
          Cirru::List(vec![Cirru::leaf(":return"), Cirru::leaf(":string")]),
        ]),
      ]),
      Cirru::List(vec![Cirru::leaf("if"), Cirru::leaf("true"), Cirru::leaf("1"), Cirru::leaf("2")]),
    ]);

    let code = code_to_calcit(&expr, "tests.return_type", "demo", vec![]).expect("parse cirru");

    let scope_defs: HashSet<Arc<str>> = HashSet::new();
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let _result = preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.return_type", &warnings, &stack);

    let warnings_vec = warnings.borrow();
    assert!(
      !warnings_vec.is_empty(),
      "should have warning for if-expression return type mismatch"
    );

    let warning_msg = warnings_vec[0].to_string();
    assert!(
      warning_msg.contains("return") && warning_msg.contains("type"),
      "warning should mention return type: {warning_msg}"
    );
    assert!(
      warning_msg.contains("string") || warning_msg.contains(":string"),
      "warning should mention declared type: {warning_msg}"
    );
    assert!(
      warning_msg.contains("number") || warning_msg.contains(":number"),
      "warning should mention inferred if-branch type: {warning_msg}"
    );
  }

  #[test]
  fn checks_struct_method_arg_types() {
    use cirru_edn::EdnTag;

    // Create a method function: defn greet (name: string, age: number) -> ...
    let method_fn = Arc::new(CalcitFn {
      name: Arc::from("greet"),
      def_ns: Arc::from("tests.method"),
      def_ref: None,
      usage: crate::calcit::CalcitFnUsageMeta::default(),
      scope: Arc::new(CalcitScope::default()),
      args: Arc::new(CalcitFnArgs::Args(vec![1, 2])), // 2 parameters
      call_shape: crate::calcit::CalcitFnCallShape::fixed(2),
      body: vec![Calcit::Nil],
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      return_type: crate::calcit::DYNAMIC_TYPE.clone(),
      arg_types: vec![Arc::new(CalcitTypeAnnotation::String), Arc::new(CalcitTypeAnnotation::Number)],
      rest_type: None,
    });

    // Create a struct with the method
    let method_value = Calcit::Fn {
      id: Arc::from("tests.method/greet"),
      info: method_fn.clone(),
    };

    let method_impl = CalcitImpl {
      name: EdnTag::from("Person"),
      origin: None,
      fields: Arc::new(vec![EdnTag::from("greet")]),
      values: Arc::new(vec![method_value.clone()]),
    };

    let class_struct = CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef {
        name: EdnTag::from("Person"),
        fields: Arc::new(vec![EdnTag::from("greet")]),
        field_types: Arc::new(vec![calcit::DYNAMIC_TYPE.clone()]),
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        impls: vec![Arc::new(method_impl)],
      }),
      values: Arc::new(vec![method_value]),
    };

    // Test expression: (.greet user |hello) - wrong argument type
    // greet expects (string, number) but we pass (string, string)
    let expr = Cirru::List(vec![
      Cirru::leaf(".greet"),
      Cirru::leaf("user"),
      Cirru::leaf("|hello"), // Should be number, but got string
    ]);

    let code = code_to_calcit(&expr, "tests.method", "demo", vec![]).expect("parse cirru");

    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("user"));
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    scope_types.insert(
      Arc::from("user"),
      Arc::new(CalcitTypeAnnotation::StructValue(class_struct.struct_ref.clone())),
    );

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let _result = preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.method", &warnings, &stack).expect("preprocess");

    // Should have warning about argument type mismatch
    let warnings_vec = warnings.borrow();
    assert!(!warnings_vec.is_empty(), "should have warning for wrong argument type");

    let warning_msg = warnings_vec[0].to_string();
    assert!(
      warning_msg.contains("Method") || warning_msg.contains("greet"),
      "warning should mention method: {warning_msg}"
    );
    assert!(
      warning_msg.contains("number") && warning_msg.contains("string"),
      "warning should mention type mismatch: {warning_msg}"
    );
  }

  #[test]
  fn checks_enum_invalid_variant() {
    use crate::calcit::CalcitEnumDef;
    use cirru_edn::EdnTag;

    // Create a test enum: Result with :ok and :err variants
    let enum_struct = CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef::from_fields(
        EdnTag::from("Result"),
        vec![EdnTag::from("err"), EdnTag::from("ok")],
      )),
      // :err expects 1 string payload, :ok expects 0 payloads
      values: Arc::new(vec![
        Calcit::from(vec![Calcit::tag("string")]), // :err payload types
        Calcit::from(CalcitList::default()),       // :ok payload types (empty)
      ]),
    };
    let enum_proto = CalcitEnumDef::from_struct(enum_struct.clone()).expect("valid enum");

    // Test: create enum value with invalid variant :invalid
    let args = CalcitList::from(
      &vec![
        Calcit::EnumDef(enum_proto), // enum prototype
        Calcit::tag("invalid"),      // invalid variant tag
      ][..],
    );

    let scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);

    check_enum_construction(&args, &scope_types, "tests.enum", "demo", &warnings);

    let warnings_vec = warnings.borrow();
    assert!(!warnings_vec.is_empty(), "should have warning for invalid variant");
    let warning_msg = warnings_vec[0].to_string();
    assert!(
      warning_msg.contains("invalid") && warning_msg.contains("Result"),
      "warning should mention invalid variant and enum name: {warning_msg}"
    );
    assert!(
      warning_msg.contains("err") || warning_msg.contains("ok"),
      "warning should list available variants: {warning_msg}"
    );
  }

  #[test]
  fn checks_enum_wrong_arity() {
    use crate::calcit::CalcitEnumDef;
    use cirru_edn::EdnTag;

    // Create a test enum: Result with :ok (0 payloads) and :err (1 payload)
    let enum_struct = CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef::from_fields(
        EdnTag::from("Result"),
        vec![EdnTag::from("err"), EdnTag::from("ok")],
      )),
      values: Arc::new(vec![
        Calcit::from(vec![Calcit::tag("string")]), // :err expects 1 payload
        Calcit::from(CalcitList::default()),       // :ok expects 0 payloads
      ]),
    };
    let enum_proto = CalcitEnumDef::from_struct(enum_struct.clone()).expect("valid enum");

    // Test: create :err enum without the required payload
    let args = CalcitList::from(
      &vec![
        Calcit::EnumDef(enum_proto), // enum prototype
        Calcit::tag("err"),          // :err variant expects 1 payload
                                     // missing payload!
      ][..],
    );

    let scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);

    check_enum_construction(&args, &scope_types, "tests.enum", "demo", &warnings);

    let warnings_vec = warnings.borrow();
    assert!(!warnings_vec.is_empty(), "should have warning for wrong arity");
    let warning_msg = warnings_vec[0].to_string();
    assert!(
      warning_msg.contains("err") && warning_msg.contains("Result"),
      "warning should mention variant and enum name: {warning_msg}"
    );
    assert!(
      warning_msg.contains("expects 1") && warning_msg.contains("got 0"),
      "warning should mention expected vs actual arity: {warning_msg}"
    );
  }

  #[test]
  fn checks_enum_payload_type() {
    use crate::calcit::CalcitEnumDef;
    use cirru_edn::EdnTag;

    // Create a test enum: Result with :err (string payload)
    let enum_struct = CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef::from_fields(
        EdnTag::from("Result"),
        vec![EdnTag::from("err"), EdnTag::from("ok")],
      )),
      values: Arc::new(vec![
        Calcit::from(vec![Calcit::tag("string")]), // :err expects string payload
        Calcit::from(CalcitList::default()),       // :ok expects no payloads
      ]),
    };
    let enum_proto = CalcitEnumDef::from_struct(enum_struct.clone()).expect("valid enum");

    // Test: create :err enum with number instead of string
    let args = CalcitList::from(
      &vec![
        Calcit::EnumDef(enum_proto), // enum prototype
        Calcit::tag("err"),          // :err variant
        Calcit::Number(42.0),        // should be string, not number!
      ][..],
    );

    let scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);

    check_enum_construction(&args, &scope_types, "tests.enum", "demo", &warnings);

    let warnings_vec = warnings.borrow();
    assert!(!warnings_vec.is_empty(), "should have warning for payload type mismatch");
    let warning_msg = warnings_vec[0].to_string();
    assert!(
      warning_msg.contains("payload 1"),
      "warning should mention payload index: {warning_msg}"
    );
    assert!(
      warning_msg.contains("string") && warning_msg.contains("number"),
      "warning should mention expected and actual types: {warning_msg}"
    );
  }

  #[test]
  fn checks_enum_nth_out_of_bounds() {
    use cirru_edn::EdnTag;
    let _ = EdnTag::from("point"); // tag retained for documentation

    // With a dynamic enum, size is not statically known;
    // bounds checking falls through (same as AnonymousEnum). Verify no warning.
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    scope_types.insert(Arc::from("my-enum"), Arc::new(CalcitTypeAnnotation::AnonymousEnum));

    let args = CalcitList::from(
      &vec![
        Calcit::Symbol {
          sym: Arc::from("my-enum"),
          info: Arc::new(CalcitSymbolInfo {
            at_ns: Arc::from("tests.enum"),
            at_def: Arc::from("demo"),
          }),
          location: None,
        },
        Calcit::Number(3.0),
      ][..],
    );

    let warnings = RefCell::new(vec![]);

    check_enum_nth_bounds(&args, &scope_types, "tests.enum", "demo", &warnings);

    let warnings_vec = warnings.borrow();
    assert!(
      warnings_vec.is_empty(),
      "AnonymousEnum: no static bounds checking, should have no warning"
    );
  }

  #[test]
  fn checks_enum_nth_valid_index() {
    use cirru_edn::EdnTag;
    let _ = EdnTag::from("point"); // tag retained for documentation

    // AnonymousEnum: bounds checking not performed, no warning expected
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    scope_types.insert(Arc::from("my-enum"), Arc::new(CalcitTypeAnnotation::AnonymousEnum));

    let args = CalcitList::from(
      &vec![
        Calcit::Symbol {
          sym: Arc::from("my-enum"),
          info: Arc::new(CalcitSymbolInfo {
            at_ns: Arc::from("tests.enum"),
            at_def: Arc::from("demo"),
          }),
          location: None,
        },
        Calcit::Number(1.0),
      ][..],
    );

    let warnings = RefCell::new(vec![]);

    check_enum_nth_bounds(&args, &scope_types, "tests.enum", "demo", &warnings);

    let warnings_vec = warnings.borrow();
    assert!(warnings_vec.is_empty(), "should have no warnings for valid index");
  }

  #[test]
  fn checks_enum_nth_dynamic_index() {
    use cirru_edn::EdnTag;
    let _ = EdnTag::from("point"); // tag retained for documentation

    // AnonymousEnum: dynamic index - should skip checking
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    scope_types.insert(Arc::from("my-enum"), Arc::new(CalcitTypeAnnotation::AnonymousEnum));

    // Test: dynamic index (variable) - should skip checking
    let args = CalcitList::from(
      &vec![
        Calcit::Symbol {
          sym: Arc::from("my-enum"),
          info: Arc::new(CalcitSymbolInfo {
            at_ns: Arc::from("tests.enum"),
            at_def: Arc::from("demo"),
          }),
          location: None,
        },
        Calcit::Local(CalcitLocal {
          idx: CalcitLocal::track_sym(&Arc::from("idx")),
          sym: Arc::from("idx"),
          info: Arc::new(CalcitSymbolInfo {
            at_ns: Arc::from("tests.enum"),
            at_def: Arc::from("demo"),
          }),
          location: None,
          type_info: Arc::new(CalcitTypeAnnotation::Number),
        }),
      ][..],
    );

    let warnings = RefCell::new(vec![]);

    check_enum_nth_bounds(&args, &scope_types, "tests.enum", "demo", &warnings);

    let warnings_vec = warnings.borrow();
    assert!(warnings_vec.is_empty(), "should skip check for dynamic index");
  }

  #[test]
  fn warns_on_dynamic_trait_call() {
    let _lock = lock_preprocess_test_state();
    let _guard = WarnDynMethodGuard::new(true);

    let expr = Cirru::List(vec![Cirru::leaf(".greet"), Cirru::leaf("user")]);
    let code = code_to_calcit(&expr, "tests.trait", "demo", vec![]).expect("parse cirru");

    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("user"));
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let _resolved =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.trait", &warnings, &stack).expect("preprocess method call");

    let warnings_vec = warnings.borrow();
    assert!(!warnings_vec.is_empty(), "should warn on dynamic trait call");
    assert_eq!(warnings_vec[0].code(), Some("P_DYNAMIC_METHOD_DISPATCH"));
    let warning_msg = warnings_vec[0].to_string();
    assert!(
      warning_msg.contains("dynamic trait call") && warning_msg.contains(".greet"),
      "warning should mention method: {warning_msg}"
    );
  }

  #[test]
  fn fails_fast_on_if_with_too_many_arguments() {
    let expr = Cirru::List(vec![
      Cirru::leaf("if"),
      Cirru::leaf("true"),
      Cirru::leaf("1"),
      Cirru::leaf("2"),
      Cirru::leaf("3"),
    ]);
    let code = code_to_calcit(&expr, "tests.if", "demo", vec![]).expect("parse cirru");

    let scope_defs: HashSet<Arc<str>> = HashSet::new();
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let result = preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.if", &warnings, &stack);
    assert!(result.is_err(), "preprocess should reject if with too many arguments");
    if let Err(err) = result {
      let msg = format!("{err}");
      assert!(msg.contains("if expects 2 or 3 arguments"), "error should mention if arity: {msg}");
    }
  }

  fn fn_schema_annotation(kind: SchemaKind, arg_count: usize, has_rest: bool) -> Arc<CalcitTypeAnnotation> {
    let mut arg_types = Vec::with_capacity(arg_count);
    for _ in 0..arg_count {
      arg_types.push(Arc::new(CalcitTypeAnnotation::Number));
    }
    Arc::new(CalcitTypeAnnotation::Fn(Arc::new(crate::calcit::CalcitFnTypeAnnotation {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types,
      return_type: Arc::new(CalcitTypeAnnotation::Number),
      fn_kind: kind,
      rest_type: has_rest.then(|| Arc::new(CalcitTypeAnnotation::Number)),
      features: Arc::new(HashSet::new()),
    })))
  }

  #[test]
  fn catches_schema_arity_mismatch_during_preprocess() {
    let args = CalcitList::from(
      &[
        Calcit::Symbol {
          sym: Arc::from("a"),
          info: Arc::new(CalcitSymbolInfo {
            at_ns: Arc::from("tests.schema"),
            at_def: Arc::from("demo"),
          }),
          location: None,
        },
        Calcit::Symbol {
          sym: Arc::from("b"),
          info: Arc::new(CalcitSymbolInfo {
            at_ns: Arc::from("tests.schema"),
            at_def: Arc::from("demo"),
          }),
          location: None,
        },
      ][..],
    );

    let issues = validate_def_schema_during_preprocess(
      &CalcitSyntax::Defn,
      "tests.schema",
      "demo",
      &args,
      &fn_schema_annotation(SchemaKind::Fn, 3, false),
    );

    assert_eq!(issues.len(), 1, "expected 1 issue, got: {issues:?}");
    assert!(issues[0].contains("schema has 3 required arg(s) but code has 2"));
  }

  #[test]
  fn strict_macro_validates_parameter_shape_during_preprocess() {
    let args = CalcitList::from(
      &[Calcit::Symbol {
        sym: Arc::from("value"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.schema"),
          at_def: Arc::from("demo"),
        }),
        location: None,
      }][..],
    );
    let schema = CalcitTypeAnnotation::Macro(Arc::new(strict_macro_signature(vec![], vec![], None, MacroExpansionType::Dynamic)));
    let issues = validate_def_schema_during_preprocess(&CalcitSyntax::Defmacro, "tests.schema", "demo", &args, &schema);
    assert!(
      issues.iter().any(|issue| issue.starts_with("[E_SCHEMA_REQUIRED_ARGS]")),
      "strict macro schema must constrain parameters: {issues:?}"
    );
  }

  #[test]
  fn warns_on_legacy_optional_in_public_function_schemas() {
    let schema = CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![Arc::new(CalcitTypeAnnotation::Number)],
      return_type: Arc::new(CalcitTypeAnnotation::Optional(Arc::new(CalcitTypeAnnotation::Number))),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(HashSet::new()),
    }));
    let warnings = RefCell::new(vec![]);

    warn_on_legacy_optional_public_schema("tests.schema", "demo", &schema, &warnings);

    assert_eq!(warnings.borrow().len(), 1);
    assert_eq!(warnings.borrow()[0].code(), Some("W_LEGACY_OPTIONAL_SCHEMA"));

    let core_public_warnings = RefCell::new(vec![]);
    warn_on_legacy_optional_public_schema(calcit::CORE_NS, "future-public-api", &schema, &core_public_warnings);
    assert_eq!(
      core_public_warnings.borrow().len(),
      1,
      "public core APIs must not expose Optional<T>"
    );

    let core_raw_warnings = RefCell::new(vec![]);
    warn_on_legacy_optional_public_schema(calcit::CORE_NS, "&raw-lookup", &schema, &core_raw_warnings);
    assert!(core_raw_warnings.borrow().is_empty(), "raw core primitives are semver-private");

    let bridge_warnings = RefCell::new(vec![]);
    warn_on_legacy_optional_public_schema(calcit::CORE_NS, "optionally", &schema, &bridge_warnings);
    assert!(
      bridge_warnings.borrow().is_empty(),
      "optionally is the explicit nullable-to-nominal bridge"
    );
  }

  #[test]
  fn catches_schema_kind_mismatch_during_preprocess() {
    let args = CalcitList::from(
      &[Calcit::Symbol {
        sym: Arc::from("a"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.schema"),
          at_def: Arc::from("demo"),
        }),
        location: None,
      }][..],
    );

    let issues = validate_def_schema_during_preprocess(
      &CalcitSyntax::Defn,
      "tests.schema",
      "demo",
      &args,
      &fn_schema_annotation(SchemaKind::Macro, 1, false),
    );

    assert_eq!(issues.len(), 1, "expected 1 issue, got: {issues:?}");
    assert!(issues[0].contains("schema :kind is :macro but code uses defn"));
  }

  #[test]
  fn rejects_macro_schemas_for_wasm_function_declarations() {
    let args = CalcitList::from(
      &[Calcit::Symbol {
        sym: Arc::from("a"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("tests.schema"),
          at_def: Arc::from("demo"),
        }),
        location: None,
      }][..],
    );

    for head in [CalcitSyntax::DefWasmExport, CalcitSyntax::DefWasmImport] {
      let issues = validate_def_schema_during_preprocess(
        &head,
        "tests.schema",
        "demo",
        &args,
        &fn_schema_annotation(SchemaKind::Macro, 1, false),
      );
      assert_eq!(issues.len(), 1, "expected one issue for {head}");
      assert!(issues[0].contains("schema :kind is :macro"));
    }
  }

  #[test]
  fn validate_def_schema_skips_rest_binding_name() {
    let info = Arc::new(CalcitSymbolInfo {
      at_ns: Arc::from("calcit.core"),
      at_def: Arc::from("include"),
    });
    let args = CalcitList::from(&[
      Calcit::Local(CalcitLocal {
        idx: CalcitLocal::track_sym(&Arc::from("base")),
        sym: Arc::from("base"),
        info: info.clone(),
        location: None,
        type_info: crate::calcit::DYNAMIC_TYPE.clone(),
      }),
      Calcit::Syntax(CalcitSyntax::ArgSpread, Arc::from("test")),
      Calcit::Local(CalcitLocal {
        idx: CalcitLocal::track_sym(&Arc::from("xs")),
        sym: Arc::from("xs"),
        info,
        location: None,
        type_info: crate::calcit::DYNAMIC_TYPE.clone(),
      }),
    ] as &[Calcit]);
    let schema = CalcitTypeAnnotation::Fn(Arc::new(crate::calcit::CalcitFnTypeAnnotation {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![crate::calcit::DYNAMIC_TYPE.clone()],
      return_type: crate::calcit::DYNAMIC_TYPE.clone(),
      fn_kind: SchemaKind::Fn,
      rest_type: Some(crate::calcit::DYNAMIC_TYPE.clone()),
      features: Arc::new(HashSet::new()),
    }));

    let issues = validate_def_schema_during_preprocess(&CalcitSyntax::Defn, "calcit.core", "include", &args, &schema);
    assert!(issues.is_empty(), "rest binding should not count as a required arg: {issues:?}");
  }

  #[test]
  fn validate_def_schema_reports_macro_required_and_rest_mismatches() {
    let args = CalcitList::from(&[
      Calcit::Local(CalcitLocal {
        idx: CalcitLocal::track_sym(&Arc::from("args")),
        sym: Arc::from("args"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("calcit.core"),
          at_def: Arc::from("fn"),
        }),
        location: None,
        type_info: crate::calcit::DYNAMIC_TYPE.clone(),
      }),
      Calcit::Syntax(CalcitSyntax::ArgSpread, Arc::from("test")),
      Calcit::Local(CalcitLocal {
        idx: CalcitLocal::track_sym(&Arc::from("body")),
        sym: Arc::from("body"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("calcit.core"),
          at_def: Arc::from("fn"),
        }),
        location: None,
        type_info: crate::calcit::DYNAMIC_TYPE.clone(),
      }),
    ] as &[Calcit]);
    let schema = CalcitTypeAnnotation::Fn(Arc::new(crate::calcit::CalcitFnTypeAnnotation {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![crate::calcit::DYNAMIC_TYPE.clone()],
      return_type: crate::calcit::DYNAMIC_TYPE.clone(),
      fn_kind: SchemaKind::Macro,
      rest_type: None,
      features: Arc::new(HashSet::new()),
    }));

    let issues = validate_def_schema_during_preprocess(&CalcitSyntax::Defmacro, "calcit.core", "fn", &args, &schema);
    assert!(issues.iter().any(|issue| issue.starts_with("[E_SCHEMA_REST_ARGS]")), "{issues:?}");
  }

  #[test]
  fn validate_def_schema_skips_optional_marker() {
    let args = CalcitList::from(&[
      Calcit::Local(CalcitLocal {
        idx: CalcitLocal::track_sym(&Arc::from("xs")),
        sym: Arc::from("xs"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("calcit.core"),
          at_def: Arc::from("slice"),
        }),
        location: None,
        type_info: crate::calcit::DYNAMIC_TYPE.clone(),
      }),
      Calcit::Local(CalcitLocal {
        idx: CalcitLocal::track_sym(&Arc::from("n")),
        sym: Arc::from("n"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("calcit.core"),
          at_def: Arc::from("slice"),
        }),
        location: None,
        type_info: crate::calcit::DYNAMIC_TYPE.clone(),
      }),
      Calcit::Syntax(CalcitSyntax::ArgOptional, Arc::from("test")),
      Calcit::Local(CalcitLocal {
        idx: CalcitLocal::track_sym(&Arc::from("m")),
        sym: Arc::from("m"),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: Arc::from("calcit.core"),
          at_def: Arc::from("slice"),
        }),
        location: None,
        type_info: crate::calcit::DYNAMIC_TYPE.clone(),
      }),
    ] as &[Calcit]);

    let shape = analyze_def_schema_param_shape(&args);
    assert_eq!(shape.required, 2);
    assert_eq!(shape.optional, 1);
    assert!(!shape.has_rest);
    assert!(shape.errors.is_empty());
  }
}
