mod type_checking;
mod type_inference;
mod type_rewriting;

use crate::{
  builtins::{self, is_js_syntax_procs, is_proc_name, is_registered_proc},
  calcit::{
    self, Calcit, CalcitArgLabel, CalcitErr, CalcitErrKind, CalcitFn, CalcitFnArgs, CalcitFnTypeAnnotation, CalcitImpl, CalcitImport,
    CalcitList, CalcitLocal, CalcitProc, CalcitScope, CalcitStruct, CalcitSymbolInfo, CalcitSyntax, CalcitTrait, CalcitTypeAnnotation,
    GENERATED_DEF, ImportInfo, LocatedWarning, NodeLocation, RawCodeType, SchemaKind, brief_type_of_value, pop_type_slot_override,
    push_type_slot_override, register_type_slot,
  },
  call_stack::{CallStackList, StackKind},
  codegen, program, runner,
};

use type_checking::{
  CallTypeCheckInfo, check_core_fn_arg_types, check_function_return_type, check_local_fn_call_arg_types, check_proc_arg_types,
  check_user_fn_arg_types, detect_return_type_hint_from_processed_body,
};
pub use type_inference::infer_static_type_from_expr;
use type_inference::{infer_type_from_expr, resolve_enum_value, resolve_program_value_for_preprocess, resolve_type_value};
use type_rewriting::{
  build_enum_ref_node, build_struct_ref_node, try_rewrite_local_fn_tuple_args_to_enum_tuples,
  try_rewrite_loose_record_args_to_struct_records, try_rewrite_map_args_to_records, try_rewrite_tuple_args_to_enum_tuples,
};

use std::cell::Cell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{cell::RefCell, vec};

use cirru_edn::EdnTag;
use im_ternary_tree::TernaryTreeList;
use strum::ParseError;

pub(crate) type ScopeTypes = HashMap<Arc<str>, Arc<CalcitTypeAnnotation>>;

static WARN_DYN_METHOD: AtomicBool = AtomicBool::new(false);
static VERBOSE_PREPROCESS: AtomicBool = AtomicBool::new(false);

thread_local! {
  static PREPROCESS_COMPILE_GUARD: RefCell<HashSet<(Arc<str>, Arc<str>)>> = RefCell::new(HashSet::new());
  /// When set, `preprocess_defn` for anonymous `fn` will use this as the expected
  /// function type to inject arg types into the fn body's scope_types.
  /// This enables type checking for callback parameters (e.g., `d!` in `:on-click $ fn (e d!) ...`).
  static EXPECTED_FN_TYPE: RefCell<Option<Arc<CalcitFnTypeAnnotation>>> = const { RefCell::new(None) };
  /// When set, the hashmap literal preprocessor uses this struct definition to look up
  /// field types and inject EXPECTED_FN_TYPE for Fn-typed fields.
  /// This enables type propagation through record literals (e.g., `:on-click $ fn (e d!) ...` in DomProps).
  static EXPECTED_STRUCT_TYPE: RefCell<Option<CalcitStruct>> = const { RefCell::new(None) };
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

/// Extract type information from a Calcit definition
/// Functions and procs are converted into `CalcitTypeAnnotation::Function` to retain argument/return hints
/// Other values fall back to their concrete annotation (tag/record/tuple/custom)
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
      Calcit::Enum(_) | Calcit::Struct(_) => {
        Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from(format!("{ns}/{def}")), Arc::new(vec![])))
      }
      Calcit::Record(record) => Arc::new(CalcitTypeAnnotation::Record(record.struct_ref.clone())),
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
      Calcit::Enum(enum_def) => Arc::new(CalcitTypeAnnotation::Enum(Arc::new(enum_def.to_owned()), Arc::new(vec![]))),
      Calcit::Struct(struct_def) => Arc::new(CalcitTypeAnnotation::Struct(Arc::new(struct_def.to_owned()), Arc::new(vec![]))),
      Calcit::Record(record) => Arc::new(CalcitTypeAnnotation::Record(record.struct_ref.clone())),
      other => match infer_type_from_expr(other, scope_types) {
        Some(inferred)
          if matches!(
            inferred.as_ref(),
            CalcitTypeAnnotation::Enum(_, _) | CalcitTypeAnnotation::Struct(_, _) | CalcitTypeAnnotation::Record(_)
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

fn parse_trait_method_specs_from_source<'a>(
  items: impl Iterator<Item = &'a Calcit>,
) -> Option<(Vec<EdnTag>, Vec<Arc<CalcitTypeAnnotation>>)> {
  let mut methods: Vec<EdnTag> = vec![];
  let mut method_types: Vec<Arc<CalcitTypeAnnotation>> = vec![];

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
  }

  Some((methods, method_types))
}

fn parse_trait_new_source(items: &CalcitList) -> Option<CalcitTrait> {
  let name = parse_trait_name_from_source(items.get(1)?)?;
  let method_specs = match items.get(2)? {
    Calcit::List(list) => list,
    _ => return None,
  };
  let (methods, method_types) = parse_trait_method_specs_from_source(method_specs.iter())?;
  Some(CalcitTrait::new(name, methods, method_types))
}

fn parse_deftrait_source(items: &CalcitList) -> Option<CalcitTrait> {
  let name = parse_trait_name_from_source(items.get(1)?)?;
  let (methods, method_types) = parse_trait_method_specs_from_source(items.iter().skip(2))?;
  Some(CalcitTrait::new(name, methods, method_types))
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
            Ok(Calcit::RawCode(RawCodeType::Js, def_part))
            // TODO js syntax to handle in future
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
          if scope_defs.contains(def) {
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
                // Check FFI permission: only allow if the current function's schema has :js-ffi feature
                let has_ffi = CURRENT_FN_FEATURES.with(|cell| {
                  cell
                    .borrow()
                    .as_ref()
                    .is_some_and(|features| features.contains(&EdnTag::new("js-ffi")))
                }) || program::lookup_def_schema(&info.at_ns, &info.at_def)
                  .as_ref()
                  .as_fn()
                  .is_some_and(|fn_annot| fn_annot.features.contains(&EdnTag::new("js-ffi")));
                if !has_ffi {
                  gen_check_warning_with_location(
                    format!(
                      "[Warn] JS FFI function `{def}` used in `{def_ns}/{at_def}` without `:js-ffi` feature in schema — consider isolating FFI operations into dedicated binding functions with `:features $ #{{}} :js-ffi`",
                    ),
                    NodeLocation::new(def_ns.to_owned(), at_def.to_owned(), location.to_owned().unwrap_or_default()),
                    check_warnings,
                  );
                }
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
                  gen_check_warning_with_location(
                    format!("[Warn] unknown `{def}` in {def_ns}/{at_def}, locals {{{}}}", names.join(" ")),
                    NodeLocation::new(def_ns.to_owned(), at_def.to_owned(), location.to_owned().unwrap_or_default()),
                    check_warnings,
                  );
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
        preprocess_list_call(xs, scope_defs, scope_types, file_ns, check_warnings, call_stack)
      }
    }
    Calcit::Number(..)
    | Calcit::Str(..)
    | Calcit::Nil
    | Calcit::Bool(..)
    | Calcit::Tag(..)
    | Calcit::CirruQuote(..)
    | Calcit::Struct(..)
    | Calcit::Enum(..)
    | Calcit::Record(..) => Ok(expr.to_owned()),
    Calcit::Method(..) => Ok(expr.to_owned()),
    Calcit::Proc(..) => Ok(expr.to_owned()),
    Calcit::Syntax(..) => Ok(expr.to_owned()),
    Calcit::Import { .. } => Ok(expr.to_owned()),
    _ => {
      println!("unknown expr: {expr}");
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
  let def_name = grab_def_name(head);
  let call_info = CallTypeCheckInfo {
    file_ns,
    def_name: &def_name,
    call_location: call_location.clone(),
  };

  // === Postfix record field access / method call detection ===
  // Pattern: (expr :field) where expr has a known record type → rewrite to (.-field expr)
  // Pattern: (expr .method args...) where expr has a known record/trait type → rewrite to (.method expr args...)
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
        if let Some(type_info) = resolve_type_value(&head_form, scope_types)
          && let Some(struct_def) = type_info.as_ref().resolve_to_struct()
          && let Some(idx) = struct_def.index_of(field_tag.ref_str())
        {
          // Rewrite to (&record:nth expr idx :field-tag) — same as the existing
          // `(:tag expr)` rewrite, works in both Rust runtime and JS codegen.
          let items: Vec<Calcit> = vec![
            Calcit::Proc(CalcitProc::NativeRecordNth),
            head_form,
            Calcit::Number(idx as f64),
            Calcit::Tag(field_tag.to_owned()),
          ];
          return Ok(Calcit::from(CalcitList::from(items.as_slice())));
        }
        // For non-record types (Dynamic, Fn, etc.), silently fall through —
        // `:tag` is a normal argument like `(list :a)` or `(f :a)`.
      }
      Calcit::Method(method_name, method_kind) => {
        // Only handle Invoke kinds (regular Calcit method calls)
        if matches!(method_kind, calcit::MethodKind::Invoke(_))
          && let Some(type_info) = resolve_type_value(&head_form, scope_types)
        {
          // Only trigger for struct/record or trait types — NOT for Fn/Proc which
          // also appear to have impls via core-impl lookups (e.g. `::` → tuple).
          let is_record_or_trait =
            type_info.as_ref().resolve_to_struct().is_some() || trait_list_from_type(type_info.as_ref()).is_some();

          if is_record_or_trait {
            // Rewrite to (.method expr remaining_args...) — already handled by codegen
            let typed_method = Calcit::Method(method_name.clone(), calcit::MethodKind::Invoke(type_info.clone()));
            let mut ys = CalcitList::new_inner_from(&[typed_method, head_form]);
            for arg in args.iter().skip(1) {
              ys = ys.push(arg.to_owned());
            }
            return Ok(Calcit::from(CalcitList::from(ys)));
          }
          // For non-record/trait types (Dynamic, Fn, etc.), silently fall through —
          // `.method` is a normal argument.
        }
        if warn_dyn_method_enabled()
          && file_ns != calcit::CORE_NS
          && resolve_type_value(&head_form, scope_types).is_none_or(|type_info| is_dynamic_annotation(type_info.as_ref()))
        {
          let message = format!(
            "[Warn] postfix method `.{method_name}` has a dynamic receiver in {file_ns}/{def_name}; use prefix syntax `(.{method_name} receiver ...)`, assert-traits, or unsafe-coerce at an FFI boundary"
          );
          if let Some(loc) = head_form.get_location().or_else(|| first_arg.get_location()) {
            gen_check_warning_with_location_code(message, "P_DYNAMIC_POSTFIX_METHOD", loc, check_warnings);
          } else {
            gen_check_warning_code(message, "P_DYNAMIC_POSTFIX_METHOD", file_ns, check_warnings);
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
    Some(Calcit::Macro { info, .. }) => {
      let mut current_values: Vec<Calcit> = args.to_vec();

      warn_on_trait_impl_method_tag_syntax(info.as_ref(), &args, file_ns, def_name.as_ref(), check_warnings);

      // println!("eval macro: {}", primes::CrListWrap(xs.to_owned()));
      // println!("macro... {} {}", x, CrListWrap(current_values.to_owned()));

      let code = Calcit::List(Arc::new(xs.to_owned()));
      let next_stack = call_stack.extend_owned(&info.def_ns, &info.name, StackKind::Macro, code, args.to_vec());

      let mut body_scope = CalcitScope::default();

      loop {
        // need to handle recursion
        // println!("evaluating line: {:?}", body);
        runner::bind_marked_args(&mut body_scope, &info.args, &current_values, &next_stack)?;
        let code = runner::evaluate_lines(&info.body.to_vec(), &body_scope, file_ns, &next_stack)?;
        match code {
          Calcit::Recur(ys) => {
            current_values = ys;
          }
          _ => {
            // println!("gen code: {} {}", code, &code.lisp_str());
            return preprocess_expr(&code, scope_defs, scope_types, file_ns, check_warnings, &next_stack);
          }
        }
      }
    }

    Some(Calcit::Fn { info, .. }) => {
      match &*info.args {
        CalcitFnArgs::MarkedArgs(xs) => {
          check_fn_marked_args(xs, &args, file_ns, &info.name, &def_name, check_warnings);
        }
        CalcitFnArgs::Args(xs) => {
          check_fn_args(xs, &args, file_ns, &info.name, &def_name, check_warnings);
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
        let mut any_rewritten = false;
        // Rewrite hashmap literal args to record literals when the expected type is a struct
        if let Some(rewritten) = try_rewrite_map_args_to_records(info.as_ref(), &current_args, file_ns, &def_name, check_warnings) {
          current_args = rewritten;
          any_rewritten = true;
        }
        // Rewrite loose record literal args (`?{}`) to struct record literals when the expected type is a struct
        if let Some(rewritten) =
          try_rewrite_loose_record_args_to_struct_records(info.as_ref(), &current_args, file_ns, &def_name, check_warnings)
        {
          current_args = rewritten;
          any_rewritten = true;
        }
        // Rewrite untyped tuple literal args to enum tuples when the expected type is an enum
        if let Some(rewritten) = try_rewrite_tuple_args_to_enum_tuples(info.as_ref(), &current_args, file_ns, &def_name, check_warnings)
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
          // Try to resolve the arg type as a struct record for indexed access optimization
          if let Some(type_info) = resolve_type_value(&processed_arg, scope_types)
            && let Some(struct_def) = type_info.as_ref().resolve_to_struct()
            && let Some(idx) = struct_def.index_of(tag.ref_str())
          {
            // Emit (&record:nth processed_arg idx :field-tag)
            // The 3rd arg (field tag) is used by JS codegen where field order differs from Rust
            let items: Vec<Calcit> = vec![
              Calcit::Proc(CalcitProc::NativeRecordNth),
              processed_arg,
              Calcit::Number(idx as f64),
              Calcit::Tag(tag.to_owned()),
            ];
            let nth_call = Calcit::from(CalcitList::from(items.as_slice()));
            return Ok(nth_call);
          }
          // Fallback: rewrite to (get arg :tag) and re-preprocess
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
        CalcitSyntax::Defn | CalcitSyntax::Defmacro => {
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
      | Calcit::Struct(..)
      | Calcit::Enum(..) => {
        // Check if the head (the thing being called) is actually callable
        check_callable_type(&head_form, scope_types, file_ns, &def_name, check_warnings);

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

        // Check for record field access after processing arguments
        let processed_args = CalcitList::from(ys.drop_left()); // Skip the head, convert to CalcitList
        validate_method_call(&head_form, &processed_args, scope_types, call_stack)?;
        check_record_field_access(&head_form, &processed_args, scope_types, file_ns, check_warnings);
        check_record_method_args(&head_form, &processed_args, scope_types, file_ns, &def_name, check_warnings);

        // Optimize &record:get to &record:nth when field index can be resolved at compile time
        if matches!(&head_form, Calcit::Proc(CalcitProc::NativeRecordGet))
          && processed_args.len() == 2
          && let (Some(record_arg), Some(Calcit::Tag(field_tag))) = (processed_args.first(), processed_args.get(1))
          && let Some(type_info) = resolve_type_value(record_arg, scope_types)
          && let Some(struct_def) = type_info.as_ref().resolve_to_struct()
          && let Some(idx) = struct_def.index_of(field_tag.ref_str())
        {
          ys = CalcitList::new_inner_from(&[
            Calcit::Proc(CalcitProc::NativeRecordNth),
            record_arg.to_owned(),
            Calcit::Number(idx as f64),
            Calcit::Tag(field_tag.to_owned()),
          ]);
        }

        // Optimize &record:assoc to &record:assoc-at when field index can be resolved at compile time
        if matches!(&head_form, Calcit::Proc(CalcitProc::NativeRecordAssoc))
          && processed_args.len() == 3
          && let (Some(record_arg), Some(Calcit::Tag(field_tag)), Some(value_arg)) =
            (processed_args.first(), processed_args.get(1), processed_args.get(2))
          && let Some(type_info) = resolve_type_value(record_arg, scope_types)
          && let Some(struct_def) = type_info.as_ref().resolve_to_struct()
          && let Some(idx) = struct_def.index_of(field_tag.ref_str())
        {
          ys = CalcitList::new_inner_from(&[
            Calcit::Proc(CalcitProc::NativeRecordAssocAt),
            record_arg.to_owned(),
            Calcit::Number(idx as f64),
            Calcit::Tag(field_tag.to_owned()),
            value_arg.to_owned(),
          ]);
        }

        // Optimize &record:with to &record:with-at when all field indices can be resolved at compile time
        if matches!(&head_form, Calcit::Proc(CalcitProc::NativeRecordWith))
          && processed_args.len() >= 3
          && (processed_args.len() - 1) % 2 == 0
          && let Some(record_arg) = processed_args.first()
          && let Some(type_info) = resolve_type_value(record_arg, scope_types)
          && let Some(struct_def) = type_info.as_ref().resolve_to_struct()
        {
          let pair_count = (processed_args.len() - 1) / 2;
          let mut all_resolved = true;
          let mut new_args: Vec<Calcit> = Vec::with_capacity(1 + pair_count * 3);
          new_args.push(record_arg.to_owned());

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
            items.push(Calcit::Proc(CalcitProc::NativeRecordWithAt));
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
          let typed_method = Calcit::Method(method_name.clone(), calcit::MethodKind::Invoke(type_value));
          ys = CalcitList::new_inner_from(&[typed_method]);
          for item in processed_args.iter() {
            ys = ys.push(item.to_owned());
          }
        }

        if let Some(call_head) = ys.first() {
          // Recompute processed_args from ys after optimization rewrites (e.g. record:get→nth, assoc→assoc-at)
          let processed_args = CalcitList::from(ys.drop_left());
          warn_on_dynamic_trait_call(call_head, &processed_args, scope_types, file_ns, def_name.as_ref(), check_warnings);
          warn_on_method_name_conflict(call_head, &processed_args, scope_types, file_ns, def_name.as_ref(), check_warnings);
        }

        // Check Proc argument types if available
        if let Some(Calcit::Proc(proc)) = ys.first() {
          let processed_args = CalcitList::from(ys.drop_left());
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
        // Also rewrite untyped tuple args to typed enum tuples when the local fn expects an enum type.
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
              try_rewrite_local_fn_tuple_args_to_enum_tuples(fn_annot, &local_sym, &processed_args, file_ns, &def_name, check_warnings)
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
              let coord_repr = format!("@{}", l.coord.iter().map(|c| c.to_string()).collect::<Vec<_>>().join("."));
              eprintln!(
                "[&inspect-type] in {}/{} [{}]\n  {} => {}",
                l.ns,
                l.def,
                coord_repr,
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
          && let Some(call_head) = ys.first()
          && let Some(optimized_call) = try_inline_method_call(call_head, &processed_args, scope_types, file_ns)
        {
          return Ok(optimized_call);
        }

        if has_spread {
          ys = ys.prepend(Calcit::Syntax(CalcitSyntax::CallSpread, file_ns.into()));
          Ok(Calcit::from(CalcitList::List(ys)))
        } else {
          Ok(Calcit::from(CalcitList::List(ys)))
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
  // A record instance and its struct prototype intentionally share most type
  // information, so `resolve_type_value` alone cannot distinguish them.
  let constructor_kind = match head_form {
    Calcit::Struct(_) => Some("defstruct"),
    Calcit::Enum(_) => Some("defenum"),
    Calcit::Import(CalcitImport { ns, def, .. }) => data_definition_kind(ns, def),
    Calcit::Symbol { sym, info, .. } => data_definition_kind(&info.at_ns, sym),
    _ => None,
  };

  let Some(type_info) = resolve_type_value(head_form, scope_types) else {
    return Ok(None);
  };

  if constructor_kind == Some("defstruct")
    && let Some((struct_def, ns_def_path)) = type_info.as_ref().resolve_to_struct_with_ref()
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
        && !matches!(
          struct_def.field_types.get(idx).map(AsRef::as_ref),
          Some(CalcitTypeAnnotation::Optional(_))
        )
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

    let struct_ref_node = build_struct_ref_node(&struct_def, ns_def_path, file_ns, def_name);
    let mut record_items: Vec<Calcit> = Vec::with_capacity(struct_def.fields.len() * 2 + 2);
    record_items.push(Calcit::Proc(CalcitProc::NativeRecord));
    record_items.push(struct_ref_node);
    for field in struct_def.fields.iter() {
      record_items.push(Calcit::Tag(field.to_owned()));
      if let Some(value) = provided_fields.get(field) {
        record_items.push((*value).to_owned());
      } else {
        record_items.push(Calcit::Nil);
      }
    }

    return Ok(Some(Calcit::from(record_items)));
  }

  if constructor_kind == Some("defenum")
    && let Some((enum_def, ns_def_path)) = type_info.as_ref().resolve_to_enum_with_ref()
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

    let enum_ref_node = build_enum_ref_node(enum_def, ns_def_path, file_ns, def_name);
    let mut items: Vec<Calcit> = Vec::with_capacity(args.len() + 1);
    items.push(Calcit::Proc(CalcitProc::NativeEnumTupleNew));
    items.push(enum_ref_node);
    items.extend(args.to_vec());

    return Ok(Some(Calcit::from(items)));
  }

  Ok(None)
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
  params: &CalcitList,
  file_ns: &str,
  f_name: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
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
  params: &CalcitList,
  file_ns: &str,
  f_name: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  let expected_size = defined_args.len();
  let actual_size = params.len();

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

  if expected_size != actual_size {
    gen_check_warning(
      format!("[Warn] expected {expected_size} args in {f_name} `{defined_args:?}` with `{params}`, at {file_ns}/{def_name}"),
      file_ns,
      check_warnings,
    );
  }
}

// TODO this native implementation only handles symbols
fn grab_def_name(x: &Calcit) -> Arc<str> {
  match x {
    Calcit::Symbol { info, .. } => info.at_def.to_owned(),
    _ => String::from("??").into(),
  }
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
        && (s == &CalcitSyntax::Defn || s == &CalcitSyntax::Defmacro)
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

/// Check record field access during preprocessing
/// Validates that field names exist in record types when type information is available
fn check_record_field_access(
  head: &Calcit,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  // Check if this is a call to &record:get
  if let Calcit::Proc(CalcitProc::NativeRecordGet) = head {
    // &record:get takes 2 args: (record, field)
    if args.len() >= 2
      && let (Some(record_arg), Some(field_arg)) = (args.first(), args.get(1))
    {
      check_field_in_record(record_arg, field_arg, scope_types, file_ns, check_warnings);
    }
  }
  // Also check for Import of &record:get from calcit.core
  else if let Calcit::Import(CalcitImport { ns, def, .. }) = head {
    if &**ns == calcit::CORE_NS
      && (&**def == "record-get" || &**def == "&record:get")
      && args.len() >= 2
      && let (Some(record_arg), Some(field_arg)) = (args.first(), args.get(1))
    {
      check_field_in_record(record_arg, field_arg, scope_types, file_ns, check_warnings);
    }
  }
  // Check for Method(Access) which handles .-field syntax: (.-field record)
  else if let Calcit::Method(field_name, calcit::MethodKind::Access) = head {
    // .-field takes 1 arg: the record
    if let Some(record_arg) = args.first() {
      // Create a tag for the field name to match the check_field_in_record signature
      let field_tag = Calcit::Tag(cirru_edn::EdnTag::from(&**field_name));
      check_field_in_record(record_arg, &field_tag, scope_types, file_ns, check_warnings);
    }
  }
}

/// Helper to validate a field exists in a record type
fn check_field_in_record(
  record_arg: &Calcit,
  field_arg: &Calcit,
  scope_types: &ScopeTypes,
  file_ns: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  // Get the type of the record argument - reuse resolve_type_value
  let Some(type_info) = resolve_type_value(record_arg, scope_types) else {
    return; // No type info available
  };

  // Only validate record types
  let Some(struct_def) = type_info.as_ref().resolve_to_struct() else {
    return; // Not a record type
  };

  // Extract field name from the argument
  let field_name = match field_arg {
    Calcit::Tag(tag) => tag.ref_str(),
    Calcit::Str(s) => s.as_ref(),
    Calcit::Symbol { sym, .. } => sym.as_ref(),
    _ => return, // Can't check dynamic field names
  };

  // Check if field exists in record
  if struct_def.index_of(field_name).is_some() {
    return; // Field found, validation passed
  }

  // Field not found, generate warning
  let available_fields: Vec<&str> = struct_def.fields.iter().map(|f| f.ref_str()).collect();
  gen_check_warning(
    format!(
      "[Warn] Field `{field_name}` does not exist in record `{}`. Available fields: [{}]",
      struct_def.name,
      available_fields.join(", ")
    ),
    file_ns,
    check_warnings,
  );
}

/// Check tuple index bounds for &tuple:nth operations.
/// NOTE: Static bounds checking is not available for `Tuple(Arc<CalcitEnum>)` since
/// the enum definition does not carry the per-variant payload sizes. This function is
/// a no-op: bounds errors will be caught at runtime instead.
pub(crate) fn check_tuple_nth_bounds(
  _args: &CalcitList,
  _scope_types: &ScopeTypes,
  _file_ns: &str,
  _def_name: &str,
  _check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
}

/// Check enum tuple construction (%::) for variant existence and payload arity
pub(crate) fn check_enum_tuple_construction(
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
/// Check record method call arguments (count and types)
/// Validates that method calls have correct number and types of arguments
fn check_record_method_args(
  head: &Calcit,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  // Only check Method(Invoke) calls
  let Calcit::Method(method_name, calcit::MethodKind::Invoke(_)) = head else {
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

  if let Some(traits) = trait_list_from_type(type_value.as_ref()) {
    let Some((trait_def, method_type)) = find_trait_method_type(&traits, method_name.as_ref()) else {
      return;
    };

    let Some(signature) = method_type.as_function() else {
      return;
    };

    let Ok(method_args) = args.skip(1) else {
      return;
    };

    let expected_count = signature.arg_types.len();
    let actual_with_receiver = method_args.len() + 1;
    if expected_count != 0 && expected_count != actual_with_receiver {
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
    let arg_types_without_receiver = signature.arg_types.iter().skip(1);
    for (idx, (arg, expected_type)) in method_args.iter().zip(arg_types_without_receiver).enumerate() {
      if matches!(**expected_type, CalcitTypeAnnotation::Dynamic) {
        continue;
      }

      if let Some(actual_type) = resolve_type_value(arg, scope_types)
        && !actual_type.as_ref().matches_with_bindings(expected_type.as_ref(), &mut bindings)
      {
        let expected_str = expected_type.as_ref().to_brief_string();
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

  // Get impl records for the type
  let Some(impl_records) = get_impl_records_from_type(&type_value) else {
    return; // No impl record, skip check
  };

  // Get method entry from impl records
  let method_str = method_name.as_ref();
  let Some(method_entry) = find_method_entry_for_type(type_value.as_ref(), &impl_records, method_str) else {
    return; // Method not found (will be caught by validate_method_call)
  };

  // Get function info from method entry
  let fn_info: Option<&CalcitFn> = match method_entry {
    Calcit::Fn { info, .. } => Some(info.as_ref()),
    Calcit::Import(_import) => {
      // Imports will be inlined and checked by check_proc_arg_types later
      // Skip checking here to avoid duplicate warnings
      return;
    }
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

  if !has_variadic && expected_count != actual_with_receiver {
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

  let Calcit::Method(method_name, calcit::MethodKind::Invoke(_)) = head else {
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
    gen_check_warning_with_location(message, loc, check_warnings);
  } else {
    gen_check_warning(message, file_ns, check_warnings);
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
      "[Warn] `{macro_name}` method key `:{method_name}` in {file_ns}/{def_name} uses legacy tag style; prefer dot method key `.{method_name}` for migration (`:{method_name}` remains compatible)"
    );

    if let Some(loc) = entry.get_location() {
      gen_check_warning_with_location(message, loc, check_warnings);
    } else {
      gen_check_warning(message, file_ns, check_warnings);
    }
  }
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

  let Calcit::Method(method_name, calcit::MethodKind::Invoke(_)) = head else {
    return;
  };

  let Some(receiver) = args.first() else {
    return;
  };

  let Some(type_value) = resolve_type_value(receiver, scope_types) else {
    return;
  };

  let Some(impl_records) = get_impl_records_from_type(type_value.as_ref()) else {
    return;
  };

  if impl_records.len() < 2 {
    return;
  }

  let last_wins = core_impl_list_symbol_from_type_annotation(type_value.as_ref()).is_none();
  let matched_impls: Vec<&Arc<CalcitImpl>> = if last_wins {
    impl_records
      .iter()
      .rev()
      .filter(|imp| imp.get(method_name.as_ref()).is_some() && imp.origin().is_some())
      .collect()
  } else {
    impl_records
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
      | ("tuple?", T::Tuple(_) | T::DynTuple)
      | ("record?", T::Record(_) | T::Struct(_, _))
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
    ("count", T::Tuple(_) | T::DynTuple) => NativeTupleCount,
    ("count", T::Record(_)) => NativeRecordCount,
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
    ("contains?", T::Record(_)) => NativeRecordContains,
    // first
    ("first", T::List(_)) => NativeListFirst,
    ("first", T::String) => NativeStrFirst,
    // rest
    ("rest", T::List(_)) => NativeListRest,
    // nth
    ("nth", T::List(_)) => NativeListNth,
    ("nth", T::String) => NativeStrNth,
    ("nth", T::Tuple(_) | T::DynTuple) => NativeTupleNth,
    // get
    ("get", T::Map(_, _)) => NativeMapGet,
    ("get", T::List(_)) => NativeListNth,
    ("get", T::String) => NativeStrNth,
    ("get", T::Tuple(_) | T::DynTuple) => NativeTupleNth,
    ("get", T::Record(_)) => NativeRecordGet,
    // assoc
    ("assoc", T::List(_)) => NativeListAssoc,
    ("assoc", T::Map(_, _)) => NativeMapAssoc,
    ("assoc", T::Tuple(_) | T::DynTuple) => NativeTupleAssoc,
    ("assoc", T::Record(_)) => NativeRecordAssoc,
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
      let impl_records = get_impl_records_from_type(type_ref)?;
      let (_impl_index, _impl_record, method_entry) = find_method_entry_with_impl(type_ref, &impl_records, method_name.as_ref())?;

      if let Some(callable_head) = pick_callable_from_method_entry(method_entry, file_ns) {
        return Some(build_inlined_call(callable_head, args));
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

fn build_inlined_call(callable_head: Calcit, args: &CalcitList) -> Calcit {
  let mut call_nodes: Vec<Calcit> = Vec::with_capacity(args.len() + 1);
  call_nodes.push(callable_head);
  for item in args.iter() {
    call_nodes.push(item.to_owned());
  }
  Calcit::from(call_nodes)
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

fn validate_method_call(
  head: &Calcit,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  // Only validate Method(Invoke) calls
  let Calcit::Method(method_name, calcit::MethodKind::Invoke(_)) = head else {
    return Ok(());
  };

  // Need receiver to validate
  let Some(receiver) = args.first() else {
    return Ok(());
  };

  // Get receiver type
  let Some(type_value) = resolve_type_value(receiver, scope_types) else {
    return Ok(()); // No type info, skip validation
  };

  if let Some(traits) = trait_list_from_type(type_value.as_ref()) {
    let method_str = method_name.as_ref();
    if traits
      .iter()
      .rev()
      .any(|trait_def| trait_def.methods.iter().any(|method| method.ref_str() == method_str))
    {
      return Ok(());
    }

    let methods_list = collect_trait_method_names(&traits).join(" ");
    let type_desc = describe_type(type_value.as_ref());
    return Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Type,
      format!("unknown method `.{method_name}` for {type_desc}. Available methods: {methods_list}"),
      call_stack,
      head.get_location(),
    ));
  }

  // Get impl records for the type
  let Some(impl_records) = get_impl_records_from_type(&type_value) else {
    return Ok(()); // No impl record, skip validation
  };

  // Check if method exists in the impls
  let method_str = method_name.as_ref();
  if impl_records
    .iter()
    .any(|record| record.fields().iter().any(|field| field.ref_str() == method_str))
  {
    return Ok(()); // Method found, validation passed
  }

  // Method not found, generate error
  let mut methods = vec![];
  for record in impl_records.iter() {
    for field in record.fields().iter() {
      methods.push(field.to_string());
    }
  }
  let methods_list = methods.join(" ");
  let type_desc = describe_type(type_value.as_ref());
  Err(CalcitErr::use_msg_stack_location(
    CalcitErrKind::Type,
    format!("unknown method `.{method_name}` for {type_desc}. Available methods: {methods_list}"),
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
/// Get the impl records from a type value
/// - If type_value is already a Record, use it directly
/// - If type_value is a Tag, map to corresponding core impl list
/// - Otherwise return None
fn collect_impl_records_from_value(value: &Calcit) -> Option<Vec<Arc<CalcitImpl>>> {
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

fn get_impl_records_from_type(type_value: &CalcitTypeAnnotation) -> Option<Vec<Arc<CalcitImpl>>> {
  if let Some(struct_def) = type_value.resolve_to_struct() {
    // Prepend core record impls; user impls come after and win (last_wins=true)
    let mut impls = resolve_core_impl_records("&core-record-impls").unwrap_or_default();
    impls.extend(struct_def.impls.iter().cloned());
    return Some(impls);
  }

  if let CalcitTypeAnnotation::Struct(struct_def, _) = type_value {
    return Some(struct_def.impls.to_owned());
  }

  if let CalcitTypeAnnotation::Enum(enum_def, _) = type_value {
    // Prepend core tuple impls; user impls come after and win (last_wins=true)
    let mut impls = resolve_core_impl_records("&core-tuple-impls").unwrap_or_default();
    impls.extend(enum_def.impls.iter().cloned());
    return Some(impls);
  }

  if let CalcitTypeAnnotation::Tuple(enum_def) = type_value {
    // Prepend core tuple impls; user impls come after and win (last_wins=true)
    let mut impls = resolve_core_impl_records("&core-tuple-impls").unwrap_or_default();
    impls.extend(enum_def.impls.iter().cloned());
    return Some(impls);
  }

  if let CalcitTypeAnnotation::DynTuple = type_value {
    // Untyped tuple: only core impls
    if let Some(core_impls) = resolve_core_impl_records("&core-tuple-impls") {
      return Some(core_impls);
    }
  }

  if let Some(class_symbol) = core_impl_list_symbol_from_type_annotation(type_value) {
    return match resolve_program_value_for_preprocess(calcit::CORE_NS, class_symbol, None) {
      Some(value) => collect_impl_records_from_value(&value),
      None => None,
    };
  }

  if let CalcitTypeAnnotation::Custom(value) = type_value {
    match value.as_ref() {
      Calcit::Import(import) => {
        return match resolve_program_value_for_preprocess(&import.ns, &import.def, import.def_id) {
          Some(value) => collect_impl_records_from_value(&value),
          None => None,
        };
      }
      Calcit::Symbol { sym, info, .. } => {
        let (target_ns, target_def) = match runner::parse_ns_def(sym) {
          Some((ns_part, def_part)) => (ns_part, def_part),
          None => (info.at_ns.to_owned(), sym.to_owned()),
        };
        return match resolve_program_value_for_preprocess(&target_ns, &target_def, None) {
          Some(value) => collect_impl_records_from_value(&value),
          None => None,
        };
      }
      _ => {}
    }
  }

  None
}

/// Resolve core impl records from a symbol name in calcit.core
fn resolve_core_impl_records(symbol: &str) -> Option<Vec<Arc<CalcitImpl>>> {
  resolve_program_value_for_preprocess(calcit::CORE_NS, symbol, None).and_then(|v| collect_impl_records_from_value(&v))
}

fn trait_list_from_type(type_value: &CalcitTypeAnnotation) -> Option<Vec<Arc<CalcitTrait>>> {
  match type_value {
    CalcitTypeAnnotation::Trait(trait_def) => Some(vec![trait_def.to_owned()]),
    CalcitTypeAnnotation::TraitSet(traits) => Some(traits.as_ref().to_owned()),
    CalcitTypeAnnotation::Optional(inner) => trait_list_from_type(inner.as_ref()),
    _ => None,
  }
}

fn is_trait_annotation(type_value: &CalcitTypeAnnotation) -> bool {
  matches!(type_value, CalcitTypeAnnotation::Trait(_) | CalcitTypeAnnotation::TraitSet(_))
    || matches!(type_value, CalcitTypeAnnotation::Optional(inner) if is_trait_annotation(inner.as_ref()))
}

fn is_dynamic_annotation(type_value: &CalcitTypeAnnotation) -> bool {
  matches!(type_value, CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::DynFn)
    || matches!(type_value, CalcitTypeAnnotation::Optional(inner) if is_dynamic_annotation(inner.as_ref()))
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

fn collect_trait_method_names(traits: &[Arc<CalcitTrait>]) -> Vec<String> {
  let mut seen = std::collections::HashSet::new();
  let mut names = vec![];
  for trait_def in traits.iter().rev() {
    for method in trait_def.methods.iter() {
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
      for method in trait_def.methods.iter() {
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

  if let Some(impls) = get_impl_records_from_type(type_value) {
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
    "tuple?" => Some(tag_annotation("tuple")),
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

  // nil?/some?: narrow both branches when the variable has Optional type
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
        true_binding: Some((sym, Arc::new(CalcitTypeAnnotation::Unit))),
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
        false_binding: Some((sym, Arc::new(CalcitTypeAnnotation::Unit))),
      }
    }
    _ => empty,
  }
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
  let enum_def = infer_type_from_expr(&value_form, ctx.scope_types).and_then(|t| match t.as_ref() {
    CalcitTypeAnnotation::Tuple(enum_ref) => Some(enum_ref.as_ref().to_owned()),
    CalcitTypeAnnotation::TypeSlot(name) => calcit::resolve_type_slot(name).and_then(|resolved| match resolved.as_ref() {
      CalcitTypeAnnotation::Enum(e, _) => Some(e.as_ref().to_owned()),
      _ => None,
    }),
    _ => None,
  });

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
        if let Some(ref enum_def) = enum_def {
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
                .as_ref()
                .and_then(|e| e.find_variant_by_name(pat_tag))
                .and_then(|v| v.payload_types().get(bind_idx).cloned())
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
  if let Some(ref enum_def) = enum_def
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

  Ok(Calcit::List(Arc::from(CalcitList::Vector(xs))))
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
      let schema_issues = validate_def_schema_during_preprocess(head, ctx.file_ns, def_name.as_ref(), ys, &def_schema);
      if !schema_issues.is_empty() {
        let details = schema_issues.join("\n  - ");
        return Err(CalcitErr::use_msg_stack_location_with_code(
          CalcitErrKind::Type,
          format!("schema mismatch while preprocessing definition:\n  - {details}"),
          "E_SCHEMA_DEF_MISMATCH",
          ctx.call_stack,
          Some(NodeLocation::new(
            info.at_ns.to_owned(),
            info.at_def.to_owned(),
            location.to_owned().unwrap_or_default(),
          )),
        ));
      }

      // Inject declared argument types into the function body. Call-site checks alone are not
      // enough: without these bindings, local method dispatch and return inference inside a named
      // `defn` unnecessarily fall back to Dynamic. Anonymous callbacks still use EXPECTED_FN_TYPE.
      let effective_fn_schema: Option<Arc<CalcitFnTypeAnnotation>> = match def_schema.as_ref() {
        CalcitTypeAnnotation::Fn(fn_annot) => Some(fn_annot.clone()),
        CalcitTypeAnnotation::Dynamic => EXPECTED_FN_TYPE.with(|cell| cell.borrow().clone()),
        _ => None,
      };
      if let Some(fn_annot) = &effective_fn_schema {
        let parameter_types = fn_annot.arg_types.iter().cloned().chain(
          fn_annot
            .rest_type
            .iter()
            .map(|rest_type| Arc::new(CalcitTypeAnnotation::Variadic(rest_type.clone()))),
        );
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
          CalcitTypeAnnotation::Fn(fn_annot) => Some(fn_annot.features.clone()),
          _ => None,
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
      let return_type_hint = detect_return_type_hint_from_processed_body(&processed_body);
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

  let asserted_target = target_form;
  if let Calcit::Local(local) = &asserted_target {
    let type_entry = CalcitTypeAnnotation::parse_type_annotation_form(type_form);
    ctx.scope_types.insert(local.sym.to_owned(), type_entry.clone());

    let mut typed_local = local.to_owned();
    typed_local.type_info = type_entry;

    return Ok(Calcit::Local(typed_local));
  }

  Ok(Calcit::from(vec![
    Calcit::Syntax(head.to_owned(), Arc::from(head_ns)),
    asserted_target,
    type_form.to_owned(),
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

fn analyze_def_schema_param_arity(args: &CalcitList) -> (usize, bool) {
  let mut required_count = 0;
  let mut has_rest = false;
  let mut skip_rest_binding = false;

  for item in args {
    if matches!(item, Calcit::Syntax(CalcitSyntax::ArgSpread, _)) {
      has_rest = true;
      skip_rest_binding = true;
      continue;
    }
    if matches!(item, Calcit::Syntax(CalcitSyntax::ArgOptional, _)) {
      continue;
    }
    if skip_rest_binding {
      skip_rest_binding = false;
      continue;
    }
    required_count += 1;
  }

  (required_count, has_rest)
}

fn validate_def_schema_during_preprocess(
  head: &CalcitSyntax,
  ns: &str,
  def_name: &str,
  args: &CalcitList,
  schema: &CalcitTypeAnnotation,
) -> Vec<String> {
  let CalcitTypeAnnotation::Fn(fn_annot) = schema else {
    return vec![];
  };

  let code_kind = match head {
    CalcitSyntax::Defn => "defn",
    CalcitSyntax::Defmacro => "defmacro",
    _ => return vec![],
  };

  let mut issues: Vec<String> = vec![];

  match (fn_annot.fn_kind, code_kind) {
    (SchemaKind::Fn, "defmacro") => {
      issues.push(format!("{ns}/{def_name}: schema :kind is :fn but code uses defmacro"));
    }
    (SchemaKind::Macro, "defn") => {
      issues.push(format!("{ns}/{def_name}: schema :kind is :macro but code uses defn"));
    }
    _ => {}
  }

  if code_kind == "defmacro" {
    return issues;
  }

  let (required_count, has_rest) = analyze_def_schema_param_arity(args);
  let schema_required = fn_annot.arg_types.len();
  let schema_has_rest = fn_annot.rest_type.is_some();

  if required_count != schema_required {
    issues.push(format!(
      "{ns}/{def_name}: schema has {schema_required} required arg(s) but code has {required_count}"
    ));
  }
  if has_rest != schema_has_rest {
    if has_rest {
      issues.push(format!("{ns}/{def_name}: code has & rest param but schema has no :rest"));
    } else {
      issues.push(format!("{ns}/{def_name}: schema has :rest but code has no & param"));
    }
  }

  issues
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::{
    CalcitFn, CalcitFnArgs, CalcitFnUsageMeta, CalcitImport, CalcitMacro, CalcitRecord, CalcitScope, CalcitStruct, ImportInfo,
  };
  use crate::data::cirru::code_to_calcit;
  use cirru_parser::Cirru;
  use std::sync::{LazyLock, Mutex};

  static PREPROCESS_TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

  fn lock_preprocess_test_state() -> std::sync::MutexGuard<'static, ()> {
    PREPROCESS_TEST_LOCK.lock().unwrap_or_else(|err| err.into_inner())
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
      Arc::new(CalcitStruct {
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
    assert!(trait_def.has_method("show"));
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
  fn validates_record_field_access() {
    use cirru_edn::EdnTag;

    // Create a test record type with fields: name, age
    let test_record = Arc::new(CalcitTypeAnnotation::Record(Arc::new(CalcitStruct::from_fields(
      EdnTag::from("Person"),
      vec![EdnTag::from("age"), EdnTag::from("name")],
    ))));

    // Test expression: (assert-type user <record-type>) (&record:get user :name)
    let expr = Cirru::List(vec![
      Cirru::leaf("&let"),
      Cirru::List(vec![Cirru::leaf("user"), Cirru::leaf("nil")]),
      Cirru::List(vec![
        Cirru::leaf("assert-type"),
        Cirru::leaf("user"),
        Cirru::leaf("record-type"), // placeholder, will be replaced
      ]),
      Cirru::List(vec![Cirru::leaf("&record:get"), Cirru::leaf("user"), Cirru::leaf(":name")]),
    ]);

    let code = code_to_calcit(&expr, "tests.record", "demo", vec![]).expect("parse cirru");
    let scope_defs: HashSet<Arc<str>> = HashSet::new();
    let mut scope_types: ScopeTypes = ScopeTypes::new();

    // Manually insert the record type for testing
    scope_types.insert(Arc::from("user"), test_record.clone());

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    // This should not produce warnings since :name exists
    let _resolved =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.record", &warnings, &stack).expect("preprocess should succeed");

    // Currently no warnings expected for valid field access
    // In future, we'll check warnings.borrow().is_empty()
  }

  #[test]
  fn rewrites_struct_head_call_to_record_ctor() {
    use crate::data::cirru::code_to_calcit;
    use cirru_edn::EdnTag;
    use cirru_parser::Cirru;

    let person_struct = CalcitStruct::from_fields(EdnTag::from("Person"), vec![EdnTag::from("name"), EdnTag::from("age")]);

    let expr = Cirru::List(vec![
      Cirru::leaf("Person"),
      Cirru::leaf(":name"),
      Cirru::leaf("|Alice"),
      Cirru::leaf(":age"),
      Cirru::leaf("20"),
    ]);

    let parsed = code_to_calcit(&expr, "tests.record", "demo", vec![]).expect("parse struct ctor");
    let Calcit::List(parsed_items) = parsed else {
      panic!("expected parsed call");
    };
    let mut code_items = parsed_items.to_vec();
    code_items[0] = Calcit::Struct(person_struct.clone());
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
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.record", &warnings, &stack).expect("preprocess struct head call");

    let items = match result {
      Calcit::List(xs) => xs.to_vec(),
      other => panic!("expected list form, got {other}"),
    };

    assert!(matches!(items.first(), Some(Calcit::Proc(CalcitProc::NativeRecord))));
    match items.get(1) {
      Some(Calcit::Struct(struct_def)) => assert_eq!(struct_def.name, person_struct.name),
      other => panic!("expected struct prototype at position 1, got {other:?}"),
    }
    assert_eq!(*items.get(2).expect("name field key"), Calcit::Tag(EdnTag::from("name")));
    assert_eq!(*items.get(4).expect("age field key"), Calcit::Tag(EdnTag::from("age")));
    assert_eq!(*items.get(3).expect("name value"), Calcit::Str(Arc::from("Alice")));
    assert_eq!(*items.get(5).expect("age value"), Calcit::Number(20.0));
  }

  #[test]
  fn rewrites_enum_head_call_to_enum_tuple_ctor() {
    use crate::calcit::CalcitEnum;
    use crate::data::cirru::code_to_calcit;
    use cirru_edn::EdnTag;
    use cirru_parser::Cirru;

    let enum_record = CalcitRecord {
      struct_ref: Arc::new(CalcitStruct::from_fields(
        EdnTag::from("Result"),
        vec![EdnTag::from("ok"), EdnTag::from("err")],
      )),
      values: Arc::new(vec![
        Calcit::List(Arc::new(CalcitList::default())),
        Calcit::List(Arc::new(CalcitList::default())),
      ]),
    };
    let result_enum = CalcitEnum::from_record(enum_record.clone()).expect("valid result enum");

    let expr = Cirru::List(vec![Cirru::leaf("Result"), Cirru::leaf(":ok")]);

    let parsed = code_to_calcit(&expr, "tests.record", "demo", vec![]).expect("parse enum ctor");
    let Calcit::List(parsed_items) = parsed else {
      panic!("expected parsed call");
    };
    let mut code_items = parsed_items.to_vec();
    code_items[0] = Calcit::Enum(result_enum.clone());
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
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.record", &warnings, &stack).expect("preprocess enum head call");

    let items = match result {
      Calcit::List(xs) => xs.to_vec(),
      other => panic!("expected list form, got {other}"),
    };

    assert!(matches!(items.first(), Some(Calcit::Proc(CalcitProc::NativeEnumTupleNew))));
    match items.get(1) {
      Some(Calcit::Record(enum_record)) => assert_eq!(enum_record.struct_ref.name, *result_enum.name()),
      other => panic!("expected enum prototype at position 1, got {other:?}"),
    }
    assert_eq!(*items.get(2).expect("tag key"), Calcit::Tag(EdnTag::from("ok")));
    assert_eq!(items.len(), 3);
  }

  #[test]
  fn rejects_struct_head_call_with_odd_args() {
    use crate::data::cirru::code_to_calcit;
    use cirru_edn::EdnTag;
    use cirru_parser::Cirru;

    let person_struct = CalcitStruct::from_fields(EdnTag::from("Person"), vec![EdnTag::from("name"), EdnTag::from("age")]);

    let expr = Cirru::List(vec![Cirru::leaf("Person"), Cirru::leaf(":name")]);

    let parsed = code_to_calcit(&expr, "tests.record", "demo", vec![]).expect("parse struct ctor");
    let Calcit::List(parsed_items) = parsed else {
      panic!("expected parsed call");
    };
    let mut code_items = parsed_items.to_vec();
    code_items[0] = Calcit::Struct(person_struct.clone());
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
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.record", &warnings, &stack).expect("preprocess struct head call");

    let items = match result {
      Calcit::List(xs) => xs.to_vec(),
      other => panic!("expected list form, got {other}"),
    };

    assert!(
      !matches!(items.first(), Some(Calcit::Proc(CalcitProc::NativeRecord))),
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

    let person_struct = CalcitStruct::from_fields(EdnTag::from("Person"), vec![EdnTag::from("name"), EdnTag::from("age")]);

    let expr = Cirru::List(vec![
      Cirru::leaf("Person"),
      Cirru::leaf(":email"),
      Cirru::leaf("|alice@example.com"),
      Cirru::leaf(":age"),
      Cirru::leaf("20"),
    ]);

    let parsed = code_to_calcit(&expr, "tests.record", "demo", vec![]).expect("parse struct ctor");
    let Calcit::List(parsed_items) = parsed else {
      panic!("expected parsed call");
    };
    let mut code_items = parsed_items.to_vec();
    code_items[0] = Calcit::Struct(person_struct.clone());
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
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.record", &warnings, &stack).expect("preprocess struct head call");

    let items = match result {
      Calcit::List(xs) => xs.to_vec(),
      other => panic!("expected list form, got {other}"),
    };

    assert!(
      !matches!(items.first(), Some(Calcit::Proc(CalcitProc::NativeRecord))),
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

    let person_struct = CalcitStruct::from_fields(EdnTag::from("Person"), vec![EdnTag::from("name"), EdnTag::from("age")]);
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
      &Calcit::Struct(person_struct.clone()),
      &duplicate_args,
      &scope_types,
      "tests.record",
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
      &Calcit::Struct(person_struct),
      &missing_args,
      &scope_types,
      "tests.record",
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
  fn record_and_enum_instances_are_not_constructor_heads() {
    use crate::calcit::{CalcitEnum, CalcitTuple};
    use cirru_edn::EdnTag;

    let person_struct = CalcitStruct::from_fields(EdnTag::from("Person"), vec![EdnTag::from("name")]);
    let person = Calcit::Record(CalcitRecord {
      struct_ref: Arc::new(person_struct),
      values: Arc::new(vec![Calcit::Str(Arc::from("Alice"))]),
    });
    let enum_record = CalcitRecord {
      struct_ref: Arc::new(CalcitStruct::from_fields(EdnTag::from("Result"), vec![EdnTag::from("ok")])),
      values: Arc::new(vec![Calcit::List(Arc::new(CalcitList::default()))]),
    };
    let result_enum = CalcitEnum::from_record(enum_record).expect("valid enum");
    let enum_value = Calcit::Tuple(CalcitTuple {
      tag: Arc::new(Calcit::Tag(EdnTag::from("ok"))),
      extra: vec![],
      sum_type: Some(Arc::new(result_enum)),
    });
    let args = CalcitList::from(vec![Calcit::Tag(EdnTag::from("name")), Calcit::Str(Arc::from("Bob"))].as_slice());
    let scope_types = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    assert!(
      try_rewrite_struct_enum_constructor_head_call(&person, &args, &scope_types, "tests.record", "demo", &warnings, &stack,)
        .expect("record instance boundary")
        .is_none()
    );
    assert!(
      try_rewrite_struct_enum_constructor_head_call(&enum_value, &args, &scope_types, "tests.record", "demo", &warnings, &stack,)
        .expect("enum instance boundary")
        .is_none()
    );
    assert!(warnings.borrow().is_empty());
  }

  #[test]
  fn warns_on_record_constructor_field_type_mismatch() {
    use cirru_edn::EdnTag;

    let mut point_struct = CalcitStruct::from_fields(EdnTag::from("Point"), vec![EdnTag::from("x")]);
    point_struct.field_types = Arc::new(vec![Arc::new(CalcitTypeAnnotation::Number)]);
    let args = CalcitList::from(vec![Calcit::Tag(EdnTag::from("x")), Calcit::Str(Arc::from("wrong"))].as_slice());
    let warnings = RefCell::new(vec![]);
    let result = try_rewrite_struct_enum_constructor_head_call(
      &Calcit::Struct(point_struct),
      &args,
      &ScopeTypes::new(),
      "tests.record",
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
  fn preserves_struct_postfix_method_calls() {
    use crate::calcit::MethodKind;
    use crate::data::cirru::code_to_calcit;
    use cirru_edn::EdnTag;
    use cirru_parser::Cirru;

    let cat_struct = CalcitStruct::from_fields(EdnTag::from("Cat"), vec![EdnTag::from("name"), EdnTag::from("color")]);

    let expr = Cirru::List(vec![Cirru::leaf("kitty"), Cirru::leaf(".rename"), Cirru::leaf("|LagopusB")]);
    let code = code_to_calcit(&expr, "tests.record", "demo", vec![]).expect("parse struct method call");

    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("kitty"));

    let mut scope_types: ScopeTypes = ScopeTypes::new();
    scope_types.insert(Arc::from("kitty"), Arc::new(CalcitTypeAnnotation::Record(Arc::new(cat_struct))));

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let result =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.record", &warnings, &stack).expect("preprocess struct method call");

    let nodes = match result {
      Calcit::List(xs) => xs.to_vec(),
      other => panic!("expected list form, got {other}"),
    };

    assert!(
      !matches!(nodes.first(), Some(Calcit::Proc(CalcitProc::NativeRecord))),
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
    use crate::calcit::{CalcitEnum, CalcitRecord};
    use crate::data::cirru::code_to_calcit;
    use cirru_edn::EdnTag;
    use cirru_parser::Cirru;

    let enum_record = CalcitRecord {
      struct_ref: Arc::new(CalcitStruct::from_fields(
        EdnTag::from("Result"),
        vec![EdnTag::from("ok"), EdnTag::from("err")],
      )),
      values: Arc::new(vec![
        Calcit::List(Arc::new(CalcitList::default())),
        Calcit::List(Arc::new(CalcitList::default())),
      ]),
    };
    let result_enum = CalcitEnum::from_record(enum_record.clone()).expect("valid result enum");

    let expr = Cirru::List(vec![Cirru::leaf("Result")]);

    let parsed = code_to_calcit(&expr, "tests.record", "demo", vec![]).expect("parse enum ctor");
    let Calcit::List(parsed_items) = parsed else {
      panic!("expected parsed call");
    };
    let mut code_items = parsed_items.to_vec();
    code_items[0] = Calcit::Enum(result_enum.clone());
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
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.record", &warnings, &stack).expect("preprocess enum head call");

    let items = match result {
      Calcit::List(xs) => xs.to_vec(),
      other => panic!("expected list form, got {other}"),
    };

    assert!(
      !matches!(items.first(), Some(Calcit::Proc(CalcitProc::NativeEnumTupleNew))),
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
    use crate::calcit::{CalcitEnum, CalcitRecord};
    use crate::data::cirru::code_to_calcit;
    use cirru_edn::EdnTag;
    use cirru_parser::Cirru;

    let enum_record = CalcitRecord {
      struct_ref: Arc::new(CalcitStruct::from_fields(
        EdnTag::from("Result"),
        vec![EdnTag::from("ok"), EdnTag::from("err")],
      )),
      values: Arc::new(vec![
        Calcit::List(Arc::new(CalcitList::default())),
        Calcit::List(Arc::new(CalcitList::default())),
      ]),
    };
    let result_enum = CalcitEnum::from_record(enum_record.clone()).expect("valid result enum");

    let expr = Cirru::List(vec![Cirru::leaf("Result"), Cirru::leaf(":bad")]);

    let parsed = code_to_calcit(&expr, "tests.record", "demo", vec![]).expect("parse enum ctor");
    let Calcit::List(parsed_items) = parsed else {
      panic!("expected parsed call");
    };
    let mut code_items = parsed_items.to_vec();
    code_items[0] = Calcit::Enum(result_enum.clone());
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
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.record", &warnings, &stack).expect("preprocess enum head call");

    let items = match result {
      Calcit::List(xs) => xs.to_vec(),
      other => panic!("expected list form, got {other}"),
    };

    assert!(
      !matches!(items.first(), Some(Calcit::Proc(CalcitProc::NativeEnumTupleNew))),
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
    use crate::calcit::{CalcitEnum, CalcitRecord};
    use crate::data::cirru::code_to_calcit;
    use cirru_edn::EdnTag;
    use cirru_parser::Cirru;

    let enum_record = CalcitRecord {
      struct_ref: Arc::new(CalcitStruct::from_fields(
        EdnTag::from("Mode"),
        vec![EdnTag::from("dark"), EdnTag::from("light")],
      )),
      values: Arc::new(vec![
        Calcit::List(Arc::new(CalcitList::default())),
        Calcit::List(Arc::new(CalcitList::default())),
      ]),
    };
    let mode_enum = CalcitEnum::from_record(enum_record.clone()).expect("valid mode enum");

    let expr = Cirru::List(vec![Cirru::leaf("Mode"), Cirru::leaf("dark")]);

    let parsed = code_to_calcit(&expr, "tests.record", "demo", vec![]).expect("parse enum ctor");
    let Calcit::List(parsed_items) = parsed else {
      panic!("expected parsed call");
    };
    let mut code_items = parsed_items.to_vec();
    code_items[0] = Calcit::Enum(mode_enum.clone());
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
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.record", &warnings, &stack).expect("preprocess enum head call");

    let items = match result {
      Calcit::List(xs) => xs.to_vec(),
      other => panic!("expected list form, got {other}"),
    };

    assert!(
      !matches!(items.first(), Some(Calcit::Proc(CalcitProc::NativeEnumTupleNew))),
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
  fn warns_on_invalid_record_field() {
    use cirru_edn::EdnTag;

    // Create a test record type with fields: name, age
    let test_record = Arc::new(CalcitTypeAnnotation::Record(Arc::new(CalcitStruct::from_fields(
      EdnTag::from("Person"),
      vec![EdnTag::from("age"), EdnTag::from("name")],
    ))));

    // Test expression: (&record:get user :email) with user already typed
    let expr = Cirru::List(vec![
      Cirru::leaf("&record:get"),
      Cirru::leaf("user"),
      Cirru::leaf(":email"), // invalid field
    ]);

    let code = code_to_calcit(&expr, "tests.record", "demo", vec![]).expect("parse cirru");

    // Set up scope with user variable
    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("user"));

    let mut scope_types: ScopeTypes = ScopeTypes::new();
    // Pre-populate with record type
    scope_types.insert(Arc::from("user"), test_record.clone());

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let _resolved =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.record", &warnings, &stack).expect("preprocess should succeed");

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
      "warning should mention the record type: {warning_msg}"
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

    let class_record = CalcitRecord {
      struct_ref: Arc::new(CalcitStruct {
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
      Arc::new(CalcitTypeAnnotation::Record(class_record.struct_ref.clone())),
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

    // Create a test record type with fields: name, age
    let test_record = Arc::new(CalcitTypeAnnotation::Record(Arc::new(CalcitStruct::from_fields(
      EdnTag::from("Person"),
      vec![EdnTag::from("age"), EdnTag::from("name")],
    ))));

    // Test expression: (user.-name) - wrapped in a list to trigger method parsing
    let expr = Cirru::List(vec![Cirru::leaf("user.-name")]);

    let code = code_to_calcit(&expr, "tests.record", "demo", vec![]).expect("parse cirru");

    // Set up scope with user variable
    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("user"));

    let mut scope_types: ScopeTypes = ScopeTypes::new();
    // Pre-populate with record type
    scope_types.insert(Arc::from("user"), test_record.clone());

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let _resolved =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.record", &warnings, &stack).expect("preprocess should succeed");

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
  fn warns_on_invalid_method_field_access() {
    use cirru_edn::EdnTag;

    // Create a test record type with fields: name, age
    let test_record = Arc::new(CalcitTypeAnnotation::Record(Arc::new(CalcitStruct::from_fields(
      EdnTag::from("Person"),
      vec![EdnTag::from("age"), EdnTag::from("name")],
    ))));

    // Test expression: (user.-email) - invalid field, wrapped in list
    let expr = Cirru::List(vec![Cirru::leaf("user.-email")]);

    let code = code_to_calcit(&expr, "tests.record", "demo", vec![]).expect("parse cirru");

    // Set up scope with user variable
    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("user"));

    let mut scope_types: ScopeTypes = ScopeTypes::new();
    // Pre-populate with record type
    scope_types.insert(Arc::from("user"), test_record.clone());

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let _resolved =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.record", &warnings, &stack).expect("preprocess should succeed");

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
      "warning should mention the record type: {warning_msg}"
    );
  }

  #[test]
  fn rejects_method_on_record_without_field() {
    use cirru_edn::EdnTag;

    // Create a test record type with limited methods
    let test_record = Arc::new(CalcitTypeAnnotation::Record(Arc::new(CalcitStruct::from_fields(
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
    // Pre-populate with record type
    scope_types.insert(Arc::from("person"), test_record.clone());

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let result = preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.method", &warnings, &stack);
    assert!(result.is_err(), "preprocess should reject method call on record without that field");
    if let Err(err) = result {
      let msg = format!("{err}");
      assert!(msg.contains(".slice"), "error should mention the method name: {msg}");
      assert!(
        msg.contains("Person") || msg.contains("record"),
        "error should mention the record type: {msg}"
      );
    }
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
      EdnTag::new("Show"),
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

    let mut shown_struct = crate::calcit::CalcitStruct::from_fields(EdnTag::new("Shown"), vec![EdnTag::new("name")]);
    shown_struct.impls = vec![Arc::new(crate::calcit::CalcitImpl {
      name: EdnTag::new("ShowImpl"),
      origin: Some(show_trait.clone()),
      fields: Arc::new(vec![EdnTag::new("show")]),
      values: Arc::new(vec![Calcit::Nil]),
    })];
    let plain_struct = crate::calcit::CalcitStruct::from_fields(EdnTag::new("Plain"), vec![EdnTag::new("name")]);

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
      message.contains("trait bound") && message.contains("Show"),
      "warning should mention missing where-bound: {message}"
    );
  }

  #[test]
  fn local_function_where_bounds_warn_on_missing_trait_impl() {
    let show_trait = Arc::new(crate::calcit::CalcitTrait::new(
      EdnTag::new("Show"),
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

    let mut shown_struct = crate::calcit::CalcitStruct::from_fields(EdnTag::new("Shown"), vec![EdnTag::new("name")]);
    shown_struct.impls = vec![Arc::new(CalcitImpl {
      name: EdnTag::new("ShowImpl"),
      origin: Some(show_trait.clone()),
      fields: Arc::new(vec![EdnTag::new("show")]),
      values: Arc::new(vec![Calcit::Nil]),
    })];
    let plain_struct = crate::calcit::CalcitStruct::from_fields(EdnTag::new("Plain"), vec![EdnTag::new("name")]);

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
      message.contains("trait bound") && message.contains("Show"),
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
  fn checks_record_method_arg_types() {
    use cirru_edn::EdnTag;

    // Create a method function: defn greet (name: string, age: number) -> ...
    let method_fn = Arc::new(CalcitFn {
      name: Arc::from("greet"),
      def_ns: Arc::from("tests.method"),
      def_ref: None,
      usage: crate::calcit::CalcitFnUsageMeta::default(),
      scope: Arc::new(CalcitScope::default()),
      args: Arc::new(CalcitFnArgs::Args(vec![1, 2])), // 2 parameters
      body: vec![Calcit::Nil],
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      return_type: crate::calcit::DYNAMIC_TYPE.clone(),
      arg_types: vec![Arc::new(CalcitTypeAnnotation::String), Arc::new(CalcitTypeAnnotation::Number)],
      rest_type: None,
    });

    // Create a record with the method
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

    let class_record = CalcitRecord {
      struct_ref: Arc::new(CalcitStruct {
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
      Arc::new(CalcitTypeAnnotation::Record(class_record.struct_ref.clone())),
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
  fn checks_enum_tuple_invalid_variant() {
    use crate::calcit::CalcitEnum;
    use cirru_edn::EdnTag;

    // Create a test enum: Result with :ok and :err variants
    let enum_record = CalcitRecord {
      struct_ref: Arc::new(CalcitStruct::from_fields(
        EdnTag::from("Result"),
        vec![EdnTag::from("err"), EdnTag::from("ok")],
      )),
      // :err expects 1 string payload, :ok expects 0 payloads
      values: Arc::new(vec![
        Calcit::from(vec![Calcit::tag("string")]), // :err payload types
        Calcit::from(CalcitList::default()),       // :ok payload types (empty)
      ]),
    };
    let enum_proto = CalcitEnum::from_record(enum_record.clone()).expect("valid enum");

    // Test: create enum tuple with invalid variant :invalid
    let args = CalcitList::from(
      &vec![
        Calcit::Enum(enum_proto), // enum prototype
        Calcit::tag("invalid"),   // invalid variant tag
      ][..],
    );

    let scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);

    check_enum_tuple_construction(&args, &scope_types, "tests.enum", "demo", &warnings);

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
  fn checks_enum_tuple_wrong_arity() {
    use crate::calcit::CalcitEnum;
    use cirru_edn::EdnTag;

    // Create a test enum: Result with :ok (0 payloads) and :err (1 payload)
    let enum_record = CalcitRecord {
      struct_ref: Arc::new(CalcitStruct::from_fields(
        EdnTag::from("Result"),
        vec![EdnTag::from("err"), EdnTag::from("ok")],
      )),
      values: Arc::new(vec![
        Calcit::from(vec![Calcit::tag("string")]), // :err expects 1 payload
        Calcit::from(CalcitList::default()),       // :ok expects 0 payloads
      ]),
    };
    let enum_proto = CalcitEnum::from_record(enum_record.clone()).expect("valid enum");

    // Test: create :err tuple but without the required payload
    let args = CalcitList::from(
      &vec![
        Calcit::Enum(enum_proto), // enum prototype
        Calcit::tag("err"),       // :err variant expects 1 payload
                                  // missing payload!
      ][..],
    );

    let scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);

    check_enum_tuple_construction(&args, &scope_types, "tests.enum", "demo", &warnings);

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
  fn checks_enum_tuple_payload_type() {
    use crate::calcit::CalcitEnum;
    use cirru_edn::EdnTag;

    // Create a test enum: Result with :err (string payload)
    let enum_record = CalcitRecord {
      struct_ref: Arc::new(CalcitStruct::from_fields(
        EdnTag::from("Result"),
        vec![EdnTag::from("err"), EdnTag::from("ok")],
      )),
      values: Arc::new(vec![
        Calcit::from(vec![Calcit::tag("string")]), // :err expects string payload
        Calcit::from(CalcitList::default()),       // :ok expects no payloads
      ]),
    };
    let enum_proto = CalcitEnum::from_record(enum_record.clone()).expect("valid enum");

    // Test: create :err tuple with number instead of string
    let args = CalcitList::from(
      &vec![
        Calcit::Enum(enum_proto), // enum prototype
        Calcit::tag("err"),       // :err variant
        Calcit::Number(42.0),     // should be string, not number!
      ][..],
    );

    let scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);

    check_enum_tuple_construction(&args, &scope_types, "tests.enum", "demo", &warnings);

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
  fn checks_tuple_nth_out_of_bounds() {
    use cirru_edn::EdnTag;
    let _ = EdnTag::from("point"); // tag retained for documentation

    // With Arc<CalcitEnum> typed tuples, size is not statically known;
    // bounds checking falls through (same as DynTuple). Verify no warning.
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    scope_types.insert(Arc::from("my-tuple"), Arc::new(CalcitTypeAnnotation::DynTuple));

    let args = CalcitList::from(
      &vec![
        Calcit::Symbol {
          sym: Arc::from("my-tuple"),
          info: Arc::new(CalcitSymbolInfo {
            at_ns: Arc::from("tests.tuple"),
            at_def: Arc::from("demo"),
          }),
          location: None,
        },
        Calcit::Number(3.0),
      ][..],
    );

    let warnings = RefCell::new(vec![]);

    check_tuple_nth_bounds(&args, &scope_types, "tests.tuple", "demo", &warnings);

    let warnings_vec = warnings.borrow();
    assert!(
      warnings_vec.is_empty(),
      "DynTuple: no static bounds checking, should have no warning"
    );
  }

  #[test]
  fn checks_tuple_nth_valid_index() {
    use cirru_edn::EdnTag;
    let _ = EdnTag::from("point"); // tag retained for documentation

    // DynTuple: bounds checking not performed, no warning expected
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    scope_types.insert(Arc::from("my-tuple"), Arc::new(CalcitTypeAnnotation::DynTuple));

    let args = CalcitList::from(
      &vec![
        Calcit::Symbol {
          sym: Arc::from("my-tuple"),
          info: Arc::new(CalcitSymbolInfo {
            at_ns: Arc::from("tests.tuple"),
            at_def: Arc::from("demo"),
          }),
          location: None,
        },
        Calcit::Number(1.0),
      ][..],
    );

    let warnings = RefCell::new(vec![]);

    check_tuple_nth_bounds(&args, &scope_types, "tests.tuple", "demo", &warnings);

    let warnings_vec = warnings.borrow();
    assert!(warnings_vec.is_empty(), "should have no warnings for valid index");
  }

  #[test]
  fn checks_tuple_nth_dynamic_index() {
    use cirru_edn::EdnTag;
    let _ = EdnTag::from("point"); // tag retained for documentation

    // DynTuple: dynamic index - should skip checking
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    scope_types.insert(Arc::from("my-tuple"), Arc::new(CalcitTypeAnnotation::DynTuple));

    // Test: dynamic index (variable) - should skip checking
    let args = CalcitList::from(
      &vec![
        Calcit::Symbol {
          sym: Arc::from("my-tuple"),
          info: Arc::new(CalcitSymbolInfo {
            at_ns: Arc::from("tests.tuple"),
            at_def: Arc::from("demo"),
          }),
          location: None,
        },
        Calcit::Local(CalcitLocal {
          idx: CalcitLocal::track_sym(&Arc::from("idx")),
          sym: Arc::from("idx"),
          info: Arc::new(CalcitSymbolInfo {
            at_ns: Arc::from("tests.tuple"),
            at_def: Arc::from("demo"),
          }),
          location: None,
          type_info: Arc::new(CalcitTypeAnnotation::Number),
        }),
      ][..],
    );

    let warnings = RefCell::new(vec![]);

    check_tuple_nth_bounds(&args, &scope_types, "tests.tuple", "demo", &warnings);

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
  fn validate_def_schema_ignores_macro_arity_details() {
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
    assert!(issues.is_empty(), "macro arity differences should be ignored: {issues:?}");
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

    let (required_count, has_rest) = analyze_def_schema_param_arity(&args);
    assert_eq!(required_count, 3, "optional marker should not count as its own arg");
    assert!(!has_rest);
  }
}
