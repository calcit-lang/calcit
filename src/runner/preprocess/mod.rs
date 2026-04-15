mod type_checking;
mod type_inference;
mod type_rewriting;

use crate::{
  builtins::{is_js_syntax_procs, is_proc_name, is_registered_proc},
  calcit::{
    self, Calcit, CalcitArgLabel, CalcitErr, CalcitErrKind, CalcitFn, CalcitFnArgs, CalcitFnTypeAnnotation, CalcitImpl, CalcitImport,
    CalcitList, CalcitLocal, CalcitProc, CalcitScope, CalcitStruct, CalcitSymbolInfo, CalcitSyntax, CalcitTrait, CalcitTypeAnnotation,
    GENERATED_DEF, ImportInfo, LocatedWarning, NodeLocation, RawCodeType, SchemaKind, bind_type_slot, brief_type_of_value,
    register_type_slot,
  },
  call_stack::{CallStackList, StackKind},
  codegen, program, runner,
};

use type_checking::{
  check_core_fn_arg_types, check_function_return_type, check_local_fn_call_arg_types, check_proc_arg_types, check_user_fn_arg_types,
  detect_return_type_hint_from_processed_body,
};
use type_inference::{infer_type_from_expr, resolve_enum_value, resolve_program_value_for_preprocess, resolve_type_value};
use type_rewriting::{
  try_rewrite_local_fn_tuple_args_to_enum_tuples, try_rewrite_loose_record_args_to_struct_records, try_rewrite_map_args_to_records,
  try_rewrite_tuple_args_to_enum_tuples,
};

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{cell::RefCell, vec};

use cirru_edn::EdnTag;
use im_ternary_tree::TernaryTreeList;
use strum::ParseError;

pub(crate) type ScopeTypes = HashMap<Arc<str>, Arc<CalcitTypeAnnotation>>;

static WARN_DYN_METHOD: AtomicBool = AtomicBool::new(false);

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
      let resolved_code = calcit::with_type_annotation_warning_context(context_label, || {
        preprocess_expr(&code, &HashSet::new(), &mut scope_types, ns, check_warnings, &next_stack)
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

  if let Some(head) = items.first() {
    if matches!(head, Calcit::Syntax(CalcitSyntax::Quote, _))
      || matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "quote")
      || matches!(head, Calcit::Import(CalcitImport { ns, def, .. }) if &**ns == calcit::CORE_NS && &**def == "quote")
    {
      if let Some(inner) = items.get(1) {
        return resolve_trait_def_from_source_code(inner);
      }
    }
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
  let resolved_code = calcit::with_type_annotation_warning_context(context_label, || {
    preprocess_expr(&code, &HashSet::new(), &mut scope_types, ns, check_warnings, call_stack)
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
              None if codegen::codegen_mode() && is_js_syntax_procs(def) => Ok(expr.to_owned()),
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
    Calcit::Number(..) | Calcit::Str(..) | Calcit::Nil | Calcit::Bool(..) | Calcit::Tag(..) | Calcit::CirruQuote(..) => {
      Ok(expr.to_owned())
    }
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
  let head_form = preprocess_expr(head, scope_defs, scope_types, file_ns, check_warnings, call_stack)?;
  let args = xs.drop_left();
  let def_name = grab_def_name(head);

  let head_value = match &head_form {
    Calcit::Import(CalcitImport { ns, def, .. }) => lookup_callable_ns_def_for_preprocess(ns, def, check_warnings, call_stack)?,
    _ => None,
  };

  // println!(
  //   "handling list call: {} {:?}, {}",
  //   primes::CrListWrap(xs.to_owned()),
  //   head_form,
  //   if head_runtime_value.is_some() {
  //     head_runtime_value.to_owned().expect("debug")
  //   } else {
  //     Calcit::Nil
  //   }
  // );

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
      let mut ys = CalcitList::new_inner_from(&[head_form.to_owned()]);
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
          let mut new_ys = CalcitList::new_inner_from(&[head_form.to_owned()]);
          for item in current_args.iter() {
            new_ys = new_ys.push(item.to_owned());
          }
          ys = new_ys;
        }
        check_core_fn_arg_types(info.as_ref(), &current_args, scope_types, file_ns, &def_name, check_warnings);
        check_user_fn_arg_types(info.as_ref(), &current_args, scope_types, file_ns, &def_name, check_warnings);
      }
      if has_spread {
        ys = ys.prepend(Calcit::Syntax(CalcitSyntax::CallSpread, info.def_ns.to_owned()));
        Ok(Calcit::from(CalcitList::from(ys)))
      } else {
        Ok(Calcit::from(CalcitList::from(ys)))
      }
    }

    _ => match &head_form {
      Calcit::Tag(tag) => {
        if args.len() == 1 {
          // Preprocess the argument first to get type info
          let processed_arg = preprocess_expr(&args[0], scope_defs, scope_types, file_ns, check_warnings, call_stack)?;
          // Try to resolve the arg type as a struct record for indexed access optimization
          if let Some(type_info) = resolve_type_value(&processed_arg, scope_types) {
            if let Some(struct_def) = type_info.as_ref().as_struct() {
              if let Some(idx) = struct_def.index_of(tag.ref_str()) {
                // Emit (&record:nth processed_arg idx) — O(1) direct index access
                let nth_call = Calcit::from(CalcitList::from(&[
                  Calcit::Proc(CalcitProc::NativeRecordNth),
                  processed_arg,
                  Calcit::Number(idx as f64),
                ]));
                return Ok(nth_call);
              }
            }
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

      Calcit::Method(_, _)
      | Calcit::Proc(..)
      | Calcit::Local { .. }
      | Calcit::Import { .. }
      | Calcit::Registered { .. }
      | Calcit::List(..)
      | Calcit::RawCode(..)
      | Calcit::Symbol { .. } => {
        // Check if the head (the thing being called) is actually callable
        check_callable_type(&head_form, scope_types, file_ns, &def_name, check_warnings);

        let mut ys = CalcitList::new_inner_from(&[head_form.to_owned()]);
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
            if i % 2 == 1 {
              if let Some(Calcit::Tag(key_tag)) = items.get(i - 1) {
                if let Some(field_idx) = struct_def.fields.iter().position(|f| f == key_tag) {
                  if let Some(field_type) = struct_def.field_types.get(field_idx) {
                    if let Some(fn_annot) = field_type.resolve_to_fn() {
                      EXPECTED_FN_TYPE.with(|cell| cell.borrow_mut().replace(fn_annot));
                    }
                  }
                }
              }
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
        if matches!(&head_form, Calcit::Proc(CalcitProc::NativeRecordGet)) && processed_args.len() == 2 {
          if let (Some(record_arg), Some(Calcit::Tag(field_tag))) = (processed_args.first(), processed_args.get(1)) {
            if let Some(type_info) = resolve_type_value(record_arg, scope_types) {
              if let Some(struct_def) = type_info.as_ref().as_struct() {
                if let Some(idx) = struct_def.index_of(field_tag.ref_str()) {
                  ys = CalcitList::new_inner_from(&[
                    Calcit::Proc(CalcitProc::NativeRecordNth),
                    record_arg.to_owned(),
                    Calcit::Number(idx as f64),
                  ]);
                }
              }
            }
          }
        }

        // Infer type for Method(Invoke) and update the head if type info is available
        if let Calcit::Method(method_name, calcit::MethodKind::Invoke(_)) = &head_form {
          if let Some(receiver) = processed_args.first() {
            if let Some(type_value) = resolve_type_value(receiver, scope_types) {
              // Reconstruct the list with updated Method node carrying inferred type
              let typed_method = Calcit::Method(method_name.clone(), calcit::MethodKind::Invoke(type_value));
              ys = CalcitList::new_inner_from(&[typed_method]);
              for item in processed_args.iter() {
                ys = ys.push(item.to_owned());
              }
            }
          }
        }

        if let Some(call_head) = ys.first() {
          warn_on_dynamic_trait_call(call_head, &processed_args, scope_types, file_ns, def_name.as_ref(), check_warnings);
          warn_on_method_name_conflict(call_head, &processed_args, scope_types, file_ns, def_name.as_ref(), check_warnings);
        }

        // Check Proc argument types if available
        if let Some(Calcit::Proc(proc)) = ys.first() {
          check_proc_arg_types(proc, &processed_args, scope_types, file_ns, &def_name, check_warnings);
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

          if let Some(ref ty) = local_type {
            if let CalcitTypeAnnotation::Fn(fn_annot) = ty.as_ref() {
              if let Some(rewritten) = try_rewrite_local_fn_tuple_args_to_enum_tuples(
                fn_annot,
                &local_sym,
                &processed_args,
                file_ns,
                &def_name,
                check_warnings,
              ) {
                ys = CalcitList::new_inner_from(&[ys.first().unwrap().to_owned()]);
                for item in rewritten.iter() {
                  ys = ys.push(item.to_owned());
                }
              }
            }
          }
          if let Some(Calcit::Local(local)) = ys.first() {
            let updated_args = CalcitList::from(ys.drop_left());
            check_local_fn_call_arg_types(local, &updated_args, scope_types, file_ns, &def_name, check_warnings);
          }
        }

        // Eagerly execute type-slot procs during preprocessing so that TypeSlot
        // annotations can be resolved by the type checker within the same compilation.
        if let Some(Calcit::Proc(CalcitProc::DeftypeSlot)) = ys.first() {
          if let Some(slot_name) = processed_args.first().and_then(|arg| match arg {
            Calcit::Tag(tag) => Some(Arc::from(tag.ref_str())),
            Calcit::Str(text) => Some(Arc::from(text.as_ref())),
            _ => None,
          }) {
            register_type_slot(slot_name)
              .map_err(|msg| CalcitErr::use_msg_stack_location(CalcitErrKind::Unexpected, msg, call_stack, head.get_location()))?;
          }
        }
        if let Some(Calcit::Proc(CalcitProc::BindType)) = ys.first() {
          if let (Some(slot_arg), Some(type_arg)) = (processed_args.first(), processed_args.get(1)) {
            let slot_name = match slot_arg {
              Calcit::Tag(tag) => tag.ref_str(),
              Calcit::Str(text) => text.as_ref(),
              _ => {
                return Err(CalcitErr::use_msg_stack_location(
                  CalcitErrKind::Unexpected,
                  format!("bind-type expected a tag or string as slot name, got: {slot_arg}"),
                  call_stack,
                  head.get_location(),
                ));
              }
            };
            let resolved = match type_arg {
              Calcit::Import(CalcitImport { ns, def, def_id, .. }) => resolve_program_value_for_preprocess(ns, def, *def_id),
              Calcit::Symbol { sym, info, .. } => resolve_program_value_for_preprocess(&info.at_ns, sym, None),
              other => Some(other.to_owned()),
            };
            // When the type was resolved from an Import, preserve the ns/def path as a TypeRef
            // so that downstream resolution (e.g., resolve_to_enum_with_ref) can produce Import nodes
            // for JS codegen compatibility.
            let import_path: Option<(Arc<str>, Arc<str>)> = match type_arg {
              Calcit::Import(CalcitImport { ns, def, .. }) => Some((ns.clone(), def.clone())),
              _ => None,
            };
            let Some(resolved) = resolved else {
              return Err(CalcitErr::use_msg_stack_location(
                CalcitErrKind::Unexpected,
                format!("bind-type expected a resolvable type value for slot `{slot_name}`"),
                call_stack,
                head.get_location(),
              ));
            };
            let type_annotation = if let Some((ns, def)) = &import_path {
              // When bound from an import, use TypeRef to preserve ns/def for JS codegen
              match &resolved {
                Calcit::Enum(_) | Calcit::Struct(_) => {
                  Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from(format!("{ns}/{def}")), Arc::new(vec![])))
                }
                _ => match &resolved {
                  Calcit::Record(record) => Arc::new(CalcitTypeAnnotation::Record(record.struct_ref.clone())),
                  _ => {
                    return Err(CalcitErr::use_msg_stack_location(
                      CalcitErrKind::Unexpected,
                      format!(
                        "bind-type expected an enum or struct import, got: {}",
                        brief_type_of_value(&resolved)
                      ),
                      call_stack,
                      head.get_location(),
                    ));
                  }
                },
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
                        "bind-type expected an enum, struct, or record as type value, got: {}",
                        brief_type_of_value(other)
                      ),
                      call_stack,
                      head.get_location(),
                    ));
                  }
                },
              }
            };
            bind_type_slot(slot_name, type_annotation)
              .map_err(|msg| CalcitErr::use_msg_stack_location(CalcitErrKind::Unexpected, msg, call_stack, head.get_location()))?;
          }
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
              let coord_repr = l.coord.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(".");
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

            if let Some(Calcit::Tag(tag)) = processed_args.get(1) {
              if tag.ref_str().trim_start_matches(':') == "fail-on-dynamic" && matches!(*type_info, CalcitTypeAnnotation::Dynamic) {
                let msg = format!("&inspect-type failed to infer type for {first_arg}");
                if let Some(loc) = head.get_location() {
                  gen_check_warning_with_location(msg, loc, check_warnings);
                } else {
                  gen_check_warning(msg, file_ns, check_warnings);
                }
              }
            }
          }
          // Return nil for &inspect-type
          return Ok(Calcit::Nil);
        }

        if !has_spread {
          if let Some(call_head) = ys.first() {
            if let Some(optimized_call) = try_inline_method_call(call_head, &processed_args, scope_types, file_ns) {
              return Ok(optimized_call);
            }
          }
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
      if let Some(Calcit::Syntax(s, _)) = xs.first() {
        if s == &CalcitSyntax::Quote || s == &CalcitSyntax::Quasiquote {
          // Do not inspect quoted data for recur arity.
          return;
        }
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
      } else if let Some(Calcit::Syntax(s, _)) = xs.first() {
        if s == &CalcitSyntax::Defn || s == &CalcitSyntax::Defmacro {
          // This is a separate function scope. It will be checked by its own preprocess_defn call.
          return;
        }
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

      if let Some(Calcit::Syntax(s, _)) = xs.first() {
        if s == &CalcitSyntax::Quote || s == &CalcitSyntax::Quasiquote {
          return;
        }
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
    if args.len() >= 2 {
      if let (Some(record_arg), Some(field_arg)) = (args.first(), args.get(1)) {
        check_field_in_record(record_arg, field_arg, scope_types, file_ns, check_warnings);
      }
    }
  }
  // Also check for Import of &record:get from calcit.core
  else if let Calcit::Import(CalcitImport { ns, def, .. }) = head {
    if &**ns == calcit::CORE_NS && (&**def == "record-get" || &**def == "&record:get") && args.len() >= 2 {
      if let (Some(record_arg), Some(field_arg)) = (args.first(), args.get(1)) {
        check_field_in_record(record_arg, field_arg, scope_types, file_ns, check_warnings);
      }
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
  let Some(struct_def) = type_info.as_ref().as_struct() else {
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

    if let Some(actual_type) = resolve_type_value(payload_arg, scope_types) {
      if !actual_type.as_ref().matches_annotation(expected_type.as_ref()) {
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

      if let Some(actual_type) = resolve_type_value(arg, scope_types) {
        if !actual_type.as_ref().matches_with_bindings(expected_type.as_ref(), &mut bindings) {
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
        if let Some(actual_type) = resolve_type_value(rest_arg, scope_types) {
          if !actual_type.as_ref().matches_with_bindings(inner_type.as_ref(), &mut bindings) {
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

  let message =
    format!("[Warn] dynamic trait call `.{method_name}` cannot be monomorphized in {file_ns}/{def_name}; add assert-traits to clarify");

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

fn try_inline_method_call(head: &Calcit, args: &CalcitList, scope_types: &ScopeTypes, file_ns: &str) -> Option<Calcit> {
  match head {
    Calcit::Method(method_name, calcit::MethodKind::Invoke(type_value)) => {
      let mut resolved_type = type_value.clone();
      if matches!(**type_value, CalcitTypeAnnotation::Dynamic) {
        if let Some(receiver) = args.first() {
          if let Some(inferred) = resolve_type_value(receiver, scope_types) {
            if !matches!(inferred.as_ref(), CalcitTypeAnnotation::Dynamic) {
              resolved_type = inferred;
            }
          }
        }
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
      if let Some(type_ann) = infer_type_from_expr(expr, scope_types) {
        if !is_callable_type(&type_ann) {
          let type_desc = describe_type(&type_ann);
          gen_check_warning(
            format!("[Warn] trying to call a non-function value of type {type_desc}. Expression: {expr}, at {file_ns}/{def_name}"),
            file_ns,
            check_warnings,
          );
        }
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
      if let Some(type_ann) = infer_type_from_expr(expr, scope_types) {
        if !is_callable_type(&type_ann) {
          let type_desc = describe_type(&type_ann);
          gen_check_warning(
            format!("[Warn] trying to call a non-function value of type {type_desc}. Expression: {expr}, at {file_ns}/{def_name}"),
            file_ns,
            check_warnings,
          );
        }
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
  if let Some(struct_def) = type_value.as_struct() {
    return Some(struct_def.impls.to_owned());
  }

  if let CalcitTypeAnnotation::Struct(struct_def, _) = type_value {
    return Some(struct_def.impls.to_owned());
  }

  if let CalcitTypeAnnotation::Enum(enum_def, _) = type_value {
    return Some(enum_def.impls.to_owned());
  }

  if let CalcitTypeAnnotation::Tuple(enum_def) = type_value {
    return Some(enum_def.impls.to_owned());
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
    if let Some(method_idx) = trait_def.method_index(method_name) {
      if let Some(method_type) = trait_def.method_types.get(method_idx) {
        return Some((trait_def.as_ref(), method_type));
      }
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

  let mut xs: TernaryTreeList<Calcit> = TernaryTreeList::from(&[Calcit::Syntax(head.to_owned(), Arc::from(head_ns))]);
  let cond_form = preprocess_expr(
    args.first().unwrap(),
    ctx.scope_defs,
    ctx.scope_types,
    ctx.file_ns,
    ctx.check_warnings,
    ctx.call_stack,
  )?;
  xs = xs.push_right(cond_form.to_owned());

  let refined_binding = extract_predicate_binding(&cond_form);
  let mut true_scope_types = ctx.scope_types.clone();
  if let Some((sym, inferred)) = refined_binding {
    true_scope_types.insert(sym, inferred);
  }

  let true_form = preprocess_expr(
    args.get(1).unwrap(),
    ctx.scope_defs,
    &mut true_scope_types,
    ctx.file_ns,
    ctx.check_warnings,
    ctx.call_stack,
  )?;
  xs = xs.push_right(true_form);

  if let Some(false_branch) = args.get(2) {
    let mut false_scope_types = ctx.scope_types.clone();
    let false_form = preprocess_expr(
      false_branch,
      ctx.scope_defs,
      &mut false_scope_types,
      ctx.file_ns,
      ctx.check_warnings,
      ctx.call_stack,
    )?;
    xs = xs.push_right(false_form);
  }

  Ok(Calcit::List(Arc::new(xs.into())))
}

fn extract_predicate_binding(cond_form: &Calcit) -> Option<(Arc<str>, Arc<CalcitTypeAnnotation>)> {
  let Calcit::List(items) = cond_form else {
    return None;
  };
  if items.len() != 2 {
    return None;
  }
  let pred_name = match items.first()? {
    Calcit::Symbol { sym, .. } => Some(sym.as_ref()),
    Calcit::Import(CalcitImport { def, .. }) => Some(def.as_ref()),
    _ => None,
  }?;
  let ann = match pred_name {
    "list?" => Some(tag_annotation("list")),
    "map?" => Some(tag_annotation("map")),
    "set?" => Some(tag_annotation("set")),
    "string?" => Some(tag_annotation("string")),
    "number?" => Some(tag_annotation("number")),
    "tuple?" => Some(tag_annotation("tuple")),
    _ => None,
  }?;
  let target = items.get(1)?;
  let sym = match target {
    Calcit::Local(local) => Some(local.sym.to_owned()),
    Calcit::Symbol { sym, .. } => Some(sym.to_owned()),
    _ => None,
  }?;
  Some((sym, ann))
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
  if let Some(ref enum_def) = enum_def {
    if !has_wildcard {
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
      xs = xs.push_right(Calcit::from(zs.clone()));

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

      // Inject arg types into body_types for anonymous fn when the calling context
      // provides an expected function type via EXPECTED_FN_TYPE.
      // Named defn (where def_schema is Fn) already has call-site checking via check_user_fn_arg_types,
      // so we skip injection to avoid false positives from internal runtime handling.
      let effective_fn_schema: Option<Arc<CalcitFnTypeAnnotation>> = match def_schema.as_ref() {
        CalcitTypeAnnotation::Dynamic => {
          // Check if there's an expected fn type from the calling context (anonymous fn case)
          EXPECTED_FN_TYPE.with(|cell| cell.borrow().clone())
        }
        _ => None,
      };
      if let Some(fn_annot) = &effective_fn_schema {
        for (param_sym, arg_type) in param_symbols.iter().zip(fn_annot.arg_types.iter()) {
          if !matches!(arg_type.as_ref(), CalcitTypeAnnotation::Dynamic) {
            body_types.insert(param_sym.to_owned(), arg_type.to_owned());
          }
        }
      }

      let mut to_skip = 2;
      let mut processed_body: Vec<Calcit> = vec![];

      if let CalcitTypeAnnotation::Fn(fn_annot) = def_schema.as_ref() {
        let schema_calcit = fn_annot.to_schema_calcit();
        let schema_hint = Calcit::from(vec![Calcit::Syntax(CalcitSyntax::HintFn, Arc::from(ctx.file_ns)), schema_calcit]);
        processed_body.push(schema_hint.to_owned());
        xs = xs.push_right(schema_hint);
      }

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
            return_type: Arc::new(CalcitTypeAnnotation::TypeVar(generic_name.clone())),
            arg_types: vec![Arc::new(CalcitTypeAnnotation::TypeVar(generic_name))],
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
      arg_types: vec![
        Arc::new(CalcitTypeAnnotation::from_tag_name("number")),
        Arc::new(CalcitTypeAnnotation::from_tag_name("string")),
      ],
      return_type: crate::calcit::DYNAMIC_TYPE.clone(),
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

    check_user_fn_arg_types(&fn_info, &args, &scope_types, "tests.user_fn", "demo", &warnings);

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
      return_type: crate::calcit::DYNAMIC_TYPE.clone(),
      arg_types: vec![Arc::new(CalcitTypeAnnotation::String), Arc::new(CalcitTypeAnnotation::Number)],
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
      arg_types,
      return_type: Arc::new(CalcitTypeAnnotation::Number),
      fn_kind: kind,
      rest_type: has_rest.then(|| Arc::new(CalcitTypeAnnotation::Number)),
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
      arg_types: vec![crate::calcit::DYNAMIC_TYPE.clone()],
      return_type: crate::calcit::DYNAMIC_TYPE.clone(),
      fn_kind: SchemaKind::Fn,
      rest_type: Some(crate::calcit::DYNAMIC_TYPE.clone()),
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
      arg_types: vec![crate::calcit::DYNAMIC_TYPE.clone()],
      return_type: crate::calcit::DYNAMIC_TYPE.clone(),
      fn_kind: SchemaKind::Macro,
      rest_type: None,
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
