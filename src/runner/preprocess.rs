use crate::{
  builtins::{is_js_syntax_procs, is_proc_name, is_registered_proc},
  calcit::{
    self, Calcit, CalcitArgLabel, CalcitErr, CalcitErrKind, CalcitFnArgs, CalcitImport, CalcitList, CalcitLocal, CalcitProc,
    CalcitScope, CalcitSymbolInfo, CalcitSyntax, CalcitThunk, CalcitThunkInfo, GENERATED_DEF, ImportInfo, LocatedWarning, NodeLocation,
    RawCodeType,
  },
  call_stack::{CallStackList, StackKind},
  codegen, program, runner,
};

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::{cell::RefCell, vec};

use im_ternary_tree::TernaryTreeList;
use strum::ParseError;

type ScopeTypes = HashMap<Arc<str>, Arc<Calcit>>;

/// Context for preprocessing operations, bundled to avoid too many parameters
pub struct PreprocessContext<'a> {
  scope_defs: &'a HashSet<Arc<str>>,
  scope_types: &'a mut ScopeTypes,
  file_ns: &'a str,
  check_warnings: &'a RefCell<Vec<LocatedWarning>>,
  call_stack: &'a CallStackList,
}

/// returns the resolved symbol(only functions and macros are used),
/// if code related is not preprocessed, do it internally.
pub fn preprocess_ns_def(
  raw_ns: &str,
  raw_def: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<Option<Calcit>, CalcitErr> {
  let ns = raw_ns;
  let def = raw_def;
  // println!("preprocessing def: {}/{}", ns, def);
  match program::lookup_evaled_def(ns, def) {
    Some(v) => {
      // println!("{}/{} has inited", ns, def);
      Ok(Some(v))
    }
    None => {
      // println!("init for... {}/{}", ns, def);
      match program::lookup_def_code(ns, def) {
        Some(code) => {
          // write a nil value first to prevent dead loop
          program::write_evaled_def(ns, def, Calcit::Nil)
            .map_err(|e| CalcitErr::use_msg_stack(CalcitErrKind::Unexpected, e, call_stack))?;

          let next_stack = call_stack.extend(ns, def, StackKind::Fn, &code, &[]);

          let mut scope_types = ScopeTypes::new();
          let resolved_code = preprocess_expr(&code, &HashSet::new(), &mut scope_types, ns, check_warnings, &next_stack)?;
          // println!("\n resolve code to run: {:?}", resolved_code);
          let v = if is_fn_or_macro(&resolved_code) {
            runner::evaluate_expr(&resolved_code, &CalcitScope::default(), ns, &next_stack)?
          } else {
            Calcit::Thunk(CalcitThunk::Code {
              code: Arc::new(resolved_code),
              info: Arc::new(CalcitThunkInfo {
                ns: ns.into(),
                def: def.into(),
              }),
            })
          };
          // println!("\nwriting value to: {}/{} {:?}", ns, def, v);
          program::write_evaled_def(ns, def, v.to_owned())
            .map_err(|e| CalcitErr::use_msg_stack(CalcitErrKind::Unexpected, e, call_stack))?;

          Ok(Some(v))
        }
        None if ns.starts_with('|') || ns.starts_with('"') => Ok(None),
        None => Err(CalcitErr::use_msg_stack(
          CalcitErrKind::Var,
          format!("unknown ns/def in program: {ns}/{def}"),
          call_stack,
        )),
      }
    }
  }
}

fn is_fn_or_macro(code: &Calcit) -> bool {
  match code {
    Calcit::List(xs) => match xs.first() {
      Some(Calcit::Symbol { sym, .. }) => &**sym == "defn" || &**sym == "defmacro",
      Some(Calcit::Syntax(s, ..)) => s == &CalcitSyntax::Defn || s == &CalcitSyntax::Defmacro,
      _ => false,
    },
    _ => false,
  }
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
    } => match runner::parse_ns_def(def) {
      Some((ns_alias, def_part)) => {
        if &*ns_alias == "js" {
          Ok(Calcit::RawCode(RawCodeType::Js, def_part))
          // TODO js syntax to handle in future
        } else if let Some(target_ns) = program::lookup_ns_target_in_import(&info.at_ns, &ns_alias) {
          // make sure the target is preprocessed
          let _macro_fn = preprocess_ns_def(&target_ns, &def_part, check_warnings, call_stack)?;

          let form = Calcit::Import(CalcitImport {
            ns: target_ns.to_owned(),
            def: def_part.to_owned(),
            info: Arc::new(ImportInfo::NsAs {
              alias: ns_alias.to_owned(),
              at_def: info.at_def.to_owned(),
              at_ns: ns_alias,
            }),
            coord: program::tip_coord(&target_ns, &def_part),
          });
          Ok(form)
        } else if program::has_def_code(&ns_alias, &def_part) {
          // refer to namespace/def directly for some usages

          // make sure the target is preprocessed
          let _macro_fn = preprocess_ns_def(&ns_alias, &def_part, check_warnings, call_stack)?;

          let form = Calcit::Import(CalcitImport {
            ns: ns_alias.to_owned(),
            def: def_part.to_owned(),
            info: Arc::new(ImportInfo::NsReferDef {
              at_ns: info.at_ns.to_owned(),
              at_def: info.at_def.to_owned(),
            }),
            coord: program::tip_coord(&ns_alias, &def_part),
          });

          Ok(form)
        } else {
          Err(CalcitErr::use_msg_stack(
            CalcitErrKind::Var,
            format!("unknown ns target: {def}"),
            call_stack,
          ))
        }
      }
      None => {
        let def_ns = &info.at_ns;
        let at_def = &info.at_def;
        // println!("def {} - {} {} {}", def, def_ns, file_ns, at_def);
        if scope_defs.contains(def) {
          let type_info = scope_types.get(def).cloned();
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
              CalcitErr::use_msg_stack(CalcitErrKind::Syntax, def.to_string() + " " + &e.to_string(), call_stack)
            })?,
            def_ns.to_owned(),
          ))
        } else if *def == info.at_def {
          // call function from same file
          // println!("same file: {}/{} at {}/{}", def_ns, def, file_ns, at_def);

          // make sure the target is preprocessed
          let _macro_fn = preprocess_ns_def(def_ns, def, check_warnings, call_stack)?;

          let form = Calcit::Import(CalcitImport {
            ns: def_ns.to_owned(),
            def: def.to_owned(),
            info: Arc::new(ImportInfo::SameFile {
              at_def: info.at_def.to_owned(),
            }),
            coord: program::tip_coord(def_ns, def),
          });
          Ok(form)
        } else if let Ok(p) = def.parse::<CalcitProc>() {
          Ok(Calcit::Proc(p))
        } else if program::has_def_code(calcit::CORE_NS, def) {
          // println!("find in core def: {}", def);

          // make sure the target is preprocessed
          let _macro_fn = preprocess_ns_def(calcit::CORE_NS, def, check_warnings, call_stack)?;

          let form = Calcit::Import(CalcitImport {
            ns: calcit::CORE_NS.into(),
            def: def.to_owned(),
            info: Arc::new(ImportInfo::Core { at_ns: file_ns.into() }),
            coord: program::tip_coord(calcit::CORE_NS, def),
          });
          Ok(form)
        } else if program::has_def_code(def_ns, def) {
          // same file
          // println!("again same file: {}/{} at {}/{}", def_ns, def, file_ns, at_def);

          // make sure the target is preprocessed
          let _macro_fn = preprocess_ns_def(def_ns, def, check_warnings, call_stack)?;

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
            coord: program::tip_coord(def_ns, def),
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
              let _macro_fn = preprocess_ns_def(&target_ns, def, check_warnings, call_stack)?;

              let form = Calcit::Import(CalcitImport {
                ns: target_ns.to_owned(),
                def: def.to_owned(),
                info: Arc::new(ImportInfo::NsReferDef {
                  at_ns: def_ns.to_owned(),
                  at_def: at_def.to_owned(),
                }),
                coord: program::tip_coord(&target_ns, def),
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
                  coord: None,
                }))
              } else {
                let mut names: Vec<Arc<str>> = Vec::with_capacity(scope_defs.len());
                for def in scope_defs {
                  names.push(def.to_owned());
                }
                let mut warnings = check_warnings.borrow_mut();
                warnings.push(LocatedWarning::new(
                  format!("[Warn] unknown `{def}` in {def_ns}/{at_def}, locals {{{}}}", names.join(" ")),
                  NodeLocation::new(def_ns.to_owned(), at_def.to_owned(), location.to_owned().unwrap_or_default()),
                ));
                Ok(expr.to_owned())
              }
            }
          }
        }
      }
    },
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
      let mut warnings = check_warnings.borrow_mut();
      let loc = NodeLocation {
        ns: Arc::from(file_ns),
        def: GENERATED_DEF.into(),
        coord: Arc::from(vec![]),
      };
      warnings.push(LocatedWarning::new(
        format!("[Warn] unexpected data during preprocess: {expr:?}"),
        loc,
      ));
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
    Calcit::Import(CalcitImport { ns, def, .. }) => preprocess_ns_def(ns, def, check_warnings, call_stack)?,
    _ => None,
  };

  // println!(
  //   "handling list call: {} {:?}, {}",
  //   primes::CrListWrap(xs.to_owned()),
  //   head_form,
  //   if head_evaled.is_some() {
  //     head_evaled.to_owned().expect("debug")
  //   } else {
  //     Calcit::Nil
  //   }
  // );

  // == Tips ==
  // Macro from value: will be called during processing
  // Func from value: for checking arity
  // Keyword: transforming into tag expression
  // Syntax: handled directly during preprocessing
  // Thunk: invalid here

  match head_value {
    Some(Calcit::Macro { info, .. }) => {
      let mut current_values: Vec<Calcit> = args.to_vec();

      // println!("eval macro: {}", primes::CrListWrap(xs.to_owned()));
      // println!("macro... {} {}", x, CrListWrap(current_values.to_owned()));

      let code = Calcit::List(Arc::new(xs.to_owned()));
      let next_stack = call_stack.extend(&info.def_ns, &info.name, StackKind::Macro, &code, &args.to_vec());

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
      if has_spread {
        ys = ys.prepend(Calcit::Syntax(CalcitSyntax::CallSpread, info.def_ns.to_owned()));
        Ok(Calcit::from(CalcitList::from(ys)))
      } else {
        Ok(Calcit::from(CalcitList::from(ys)))
      }
    }

    _ => match &head_form {
      Calcit::Tag(..) => {
        if args.len() == 1 {
          let get_method = Calcit::Import(CalcitImport {
            ns: calcit::CORE_NS.into(),
            def: "get".into(),
            info: Arc::new(ImportInfo::Core { at_ns: Arc::from(file_ns) }),
            coord: program::tip_coord(calcit::CORE_NS, "get"),
          });

          let code = Calcit::from(CalcitList::from(&[get_method, args[0].to_owned(), head.to_owned()]));
          preprocess_expr(&code, scope_defs, scope_types, file_ns, check_warnings, call_stack)
        } else {
          Err(CalcitErr::use_msg_stack(
            CalcitErrKind::Arity,
            format!("{head} expected 1 hashmap to call"),
            call_stack,
          ))
        }
      }

      Calcit::Syntax(name, name_ns) => match name {
        CalcitSyntax::Quasiquote => {
          let mut ctx = PreprocessContext {
            scope_defs,
            scope_types,
            file_ns,
            check_warnings,
            call_stack,
          };
          Ok(preprocess_quasiquote(name, name_ns, &args, &mut ctx)?)
        }
        CalcitSyntax::Defn | CalcitSyntax::Defmacro => {
          let mut ctx = PreprocessContext {
            scope_defs,
            scope_types,
            file_ns,
            check_warnings,
            call_stack,
          };
          Ok(preprocess_defn(name, name_ns, &args, &mut ctx)?)
        }
        CalcitSyntax::CoreLet => {
          let mut ctx = PreprocessContext {
            scope_defs,
            scope_types,
            file_ns,
            check_warnings,
            call_stack,
          };
          Ok(preprocess_core_let(name, name_ns, &args, &mut ctx)?)
        }
        CalcitSyntax::If
        | CalcitSyntax::Try
        | CalcitSyntax::Macroexpand
        | CalcitSyntax::MacroexpandAll
        | CalcitSyntax::Macroexpand1
        | CalcitSyntax::Gensym
        | CalcitSyntax::Reset => {
          let mut ctx = PreprocessContext {
            scope_defs,
            scope_types,
            file_ns,
            check_warnings,
            call_stack,
          };
          Ok(preprocess_each_items(name, name_ns, &args, &mut ctx)?)
        }
        CalcitSyntax::Quote | CalcitSyntax::Eval | CalcitSyntax::HintFn => {
          Ok(preprocess_quote(name, name_ns, &args, scope_defs, file_ns)?)
        }
        CalcitSyntax::Defatom => {
          let mut ctx = PreprocessContext {
            scope_defs,
            scope_types,
            file_ns,
            check_warnings,
            call_stack,
          };
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
          let mut ctx = PreprocessContext {
            scope_defs,
            scope_types,
            file_ns,
            check_warnings,
            call_stack,
          };
          preprocess_asset_type(name, name_ns, &args, &mut ctx)
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
      Calcit::Thunk(..) => Err(CalcitErr::use_msg_stack(
        CalcitErrKind::Unexpected,
        format!("does not know how to preprocess a thunk: {head}"),
        call_stack,
      )),

      Calcit::Method(_, _)
      | Calcit::Proc(..)
      | Calcit::Local { .. }
      | Calcit::Import { .. }
      | Calcit::Registered { .. }
      | Calcit::List(..)
      | Calcit::RawCode(..)
      | Calcit::Symbol { .. } => {
        let mut ys = CalcitList::new_inner_from(&[head_form.to_owned()]);
        let mut has_spread = false;

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

        // Check for record field access after processing arguments
        let processed_args = CalcitList::from(ys.drop_left()); // Skip the head, convert to CalcitList
        validate_method_call(&head_form, &processed_args, scope_types, file_ns, call_stack)?;
        check_record_field_access(&head_form, &processed_args, scope_types, file_ns, check_warnings);

        // Check Proc argument types if available
        if let Calcit::Proc(proc) = &head_form {
          check_proc_arg_types(proc, &processed_args, scope_types, file_ns, &def_name, check_warnings);
        }

        if has_spread {
          ys = ys.prepend(Calcit::Syntax(CalcitSyntax::CallSpread, file_ns.into()));
          Ok(Calcit::from(CalcitList::List(ys)))
        } else {
          Ok(Calcit::from(CalcitList::List(ys)))
        }
      }
      h => Err(CalcitErr::use_msg_stack(
        CalcitErrKind::Unexpected,
        format!("unknown head `{h}` in {xs}"),
        call_stack,
      )),
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

fn gen_check_warning(message: String, file_ns: &str, check_warnings: &RefCell<Vec<LocatedWarning>>) {
  let mut warnings = check_warnings.borrow_mut();
  let loc = NodeLocation::new(Arc::from(file_ns), Arc::from(GENERATED_DEF), Arc::from(vec![]));
  warnings.push(LocatedWarning::new(message, loc));
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
  // Get the type of the record argument
  let record_type = match record_arg {
    Calcit::Local(CalcitLocal { sym, .. }) => scope_types.get(sym),
    _ => return, // Can't check non-local expressions
  };

  // If we have type info and it's a record, validate the field
  if let Some(type_info) = record_type {
    if let Calcit::Record(record) = &**type_info {
      // Extract field name from the argument
      let field_name = match field_arg {
        Calcit::Tag(tag) => tag.ref_str(),
        Calcit::Str(s) => s,
        Calcit::Symbol { sym, .. } => sym,
        _ => return, // Can't check dynamic field names
      };

      // Check if field exists in record
      if record.index_of(field_name).is_none() {
        let available_fields: Vec<&str> = record.fields.iter().map(|f| f.ref_str()).collect();
        gen_check_warning(
          format!(
            "[Warn] Field `{field_name}` does not exist in record `{}`. Available fields: [{}]",
            record.name,
            available_fields.join(", ")
          ),
          file_ns,
          check_warnings,
        );
      }
    }
  }
}

/// Check Proc argument types against type signature
fn check_proc_arg_types(
  proc: &CalcitProc,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  // Get type signature for this proc
  let Some(signature) = proc.get_type_signature() else {
    return; // No type signature, skip check
  };

  // Check if we have spreading args
  for arg in args.iter() {
    if matches!(arg, Calcit::Syntax(CalcitSyntax::ArgSpread, _)) {
      return; // Can't check with spread args
    }
  }

  // Check argument count and types
  let expected_count = signature.arg_types.len();
  let actual_count = args.len();

  // Check if signature has variadic marker (&)
  let has_variadic = signature.arg_types.iter().any(|t| {
    if let Some(type_val) = t {
      matches!(**type_val, Calcit::Tag(ref tag) if tag.ref_str() == "&")
    } else {
      false
    }
  });

  // If not variadic, check exact count
  if !has_variadic && expected_count != actual_count {
    gen_check_warning(
      format!(
        "[Warn] Proc `{}` expects {} args, got {} in call `({} {})`, at {file_ns}/{def_name}",
        proc.as_ref(),
        expected_count,
        actual_count,
        proc.as_ref(),
        args.iter().map(|a| format!("{a}")).collect::<Vec<_>>().join(" ")
      ),
      file_ns,
      check_warnings,
    );
  }

  // Check argument types until we hit variadic marker or run out of args
  for (idx, (arg, expected_type_opt)) in args.iter().zip(signature.arg_types.iter()).enumerate() {
    // Stop checking if we hit the variadic marker
    if let Some(type_val) = expected_type_opt {
      if matches!(**type_val, Calcit::Tag(ref tag) if tag.ref_str() == "&") {
        return; // Stop checking at variadic marker
      }
    }

    let Some(expected_type) = expected_type_opt else {
      continue; // No type constraint for this argument
    };

    // Try to resolve the actual type of the argument
    let actual_type_opt = match arg {
      Calcit::Local(CalcitLocal { sym, .. }) => scope_types.get(sym).cloned(),
      _ => None, // Can't check non-local expressions yet
    };

    if let Some(actual_type) = actual_type_opt {
      // Compare types
      if !types_match(&actual_type, expected_type) {
        let expected_str = type_to_string(expected_type);
        let actual_str = type_to_string(&actual_type);
        gen_check_warning(
          format!(
            "[Warn] Proc `{}` arg {} expects type `{expected_str}`, but got `{actual_str}` in call at {file_ns}/{def_name}",
            proc.as_ref(),
            idx + 1
          ),
          file_ns,
          check_warnings,
        );
      }
    }
  }
}

/// Helper to check if two types match
fn types_match(actual: &Calcit, expected: &Calcit) -> bool {
  match (actual, expected) {
    (Calcit::Tag(a), Calcit::Tag(e)) => a.ref_str() == e.ref_str(),
    _ => false, // TODO: handle more complex type matching
  }
}

/// Helper to convert type to string for error messages
fn type_to_string(t: &Calcit) -> String {
  match t {
    Calcit::Tag(tag) => format!(":{}", tag.ref_str()),
    _ => format!("{t}"),
  }
}

fn validate_method_call(
  head: &Calcit,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  call_stack: &CallStackList,
) -> Result<(), CalcitErr> {
  if let Calcit::Method(method_name, calcit::MethodKind::Invoke) = head {
    if method_name.as_ref() == "slice" {
      if let Some(receiver) = args.first() {
        if let Some(type_name) = resolve_type_tag(receiver, scope_types) {
          if !matches!(type_name.as_str(), "string" | "list") {
            return Err(CalcitErr::use_msg_stack(
              CalcitErrKind::Type,
              format!(
                "method `.slice` expected a :string or :list target, but `{}` is annotated as :{type_name} in {file_ns}",
                describe_receiver(receiver)
              ),
              call_stack,
            ));
          }
        }
      }
    }
  }
  Ok(())
}

fn resolve_type_tag(target: &Calcit, scope_types: &ScopeTypes) -> Option<String> {
  match target {
    Calcit::Local(local) => {
      if let Some(type_hint) = local.type_info.as_deref() {
        if let Some(name) = extract_tag_name(type_hint) {
          return Some(name);
        }
      }
      scope_types.get(&local.sym).and_then(|hint| extract_tag_name(hint.as_ref()))
    }
    _ => None,
  }
}

fn extract_tag_name(type_value: &Calcit) -> Option<String> {
  match type_value {
    Calcit::Tag(tag) => {
      let label = tag.ref_str();
      Some(label.trim_start_matches(':').to_owned())
    }
    _ => None,
  }
}

fn describe_receiver(expr: &Calcit) -> String {
  match expr {
    Calcit::Local(local) => local.sym.to_string(),
    _ => expr.lisp_str(),
  }
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
            zs.push(y.to_owned());
            Ok(())
          }
          Calcit::Symbol {
            sym,
            info,
            location: arg_location,
            ..
          } => {
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
              type_info: None,
            });
            // println!("created local: {:?}", s);
            zs.push(s);

            // track local in scope
            body_defs.insert(sym.to_owned());
            Ok(())
          }
          _ => Err(CalcitErr::use_msg_stack(
            CalcitErrKind::Type,
            format!("expected defn args to be symbols, got: {y}"),
            ctx.call_stack,
          )),
        }
      })?;
      xs = xs.push_right(Calcit::from(zs));

      let mut to_skip = 2;
      args.traverse_result::<CalcitErr>(&mut |a| {
        if to_skip > 0 {
          to_skip -= 1;
          return Ok(());
        }
        let form = preprocess_expr(a, &body_defs, &mut body_types, ctx.file_ns, ctx.check_warnings, ctx.call_stack)?;
        xs = xs.push_right(form);
        Ok(())
      })?;

      Ok(Calcit::List(Arc::new(xs.into())))
    }
    (Some(a), Some(b)) => Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Syntax,
      format!("defn/defmacro expected name and args: {a} {b}"),
      ctx.call_stack,
      a.get_location().or_else(|| b.get_location()),
    )),
    (a, b) => Err(CalcitErr::use_msg_stack(
      CalcitErrKind::Syntax,
      format!("defn or defmacro expected name and args, got: {a:?} {b:?}",),
      ctx.call_stack,
    )),
  }
}

// warn if this symbol is used
fn check_symbol(sym: &str, args: &CalcitList, location: NodeLocation, check_warnings: &RefCell<Vec<LocatedWarning>>) {
  if is_proc_name(sym) || CalcitSyntax::is_valid(sym) || program::has_def_code(calcit::CORE_NS, sym) {
    let mut warnings = check_warnings.borrow_mut();
    warnings.push(LocatedWarning::new(
      format!("[Warn] local binding `{sym}` shadowed `calcit.core/{sym}`, with {args}"),
      location,
    ));
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
        let name = Calcit::Local(CalcitLocal {
          idx: CalcitLocal::track_sym(sym),
          sym: sym.to_owned(),
          info: Arc::new(CalcitSymbolInfo {
            at_ns: info.at_ns.to_owned(),
            at_def: info.at_def.to_owned(),
          }),
          location: location.to_owned(),
          type_info: None,
        });
        body_types.remove(sym);
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
      return Err(CalcitErr::use_msg_stack(
        CalcitErrKind::Syntax,
        format!("expected binding of a pair, got: {a}"),
        ctx.call_stack,
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

pub fn preprocess_asset_type(
  head: &CalcitSyntax,
  head_ns: &str,
  args: &CalcitList,
  ctx: &mut PreprocessContext,
) -> Result<Calcit, CalcitErr> {
  if args.len() != 2 {
    return Err(CalcitErr::use_msg_stack(
      CalcitErrKind::Arity,
      format!("{head} expected a local and a type expression, got {}", args.len()),
      ctx.call_stack,
    ));
  }

  let mut zs: Vec<Calcit> = vec![Calcit::Syntax(head.to_owned(), Arc::from(head_ns))];
  args.traverse_result::<CalcitErr>(&mut |a| {
    let form = preprocess_expr(a, ctx.scope_defs, ctx.scope_types, ctx.file_ns, ctx.check_warnings, ctx.call_stack)?;
    zs.push(form);
    Ok(())
  })?;

  let local = match zs.get(1) {
    Some(Calcit::Local(local)) => local.to_owned(),
    other => {
      return Err(CalcitErr::use_msg_stack(
        CalcitErrKind::Type,
        format!("assert-type expected local as first arg, got {other:?}"),
        ctx.call_stack,
      ));
    }
  };
  let type_form = zs.get(2).ok_or_else(|| {
    CalcitErr::use_msg_stack(
      CalcitErrKind::Arity,
      "assert-type missing type expression".to_owned(),
      ctx.call_stack,
    )
  })?;

  let type_entry = Arc::new(type_form.to_owned());
  ctx.scope_types.insert(local.sym.to_owned(), type_entry.clone());

  if let Some(slot) = zs.get_mut(1) {
    if let Calcit::Local(mut typed_local) = slot.to_owned() {
      typed_local.type_info = Some(type_entry);
      *slot = Calcit::Local(typed_local);
    }
  }

  // assert-type is preprocessed away, return nil at runtime
  Ok(Calcit::Nil)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::CalcitRecord;
  use crate::data::cirru::code_to_calcit;
  use cirru_parser::Cirru;

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

    // assert-type now returns Nil after preprocessing
    assert!(matches!(resolved, Calcit::Nil), "assert-type should be preprocessed to Nil");

    // Check that type info is stored in scope_types
    assert!(scope_types.contains_key("x"), "type should be registered in scope");
    if let Some(type_val) = scope_types.get("x") {
      assert!(matches!(**type_val, Calcit::Tag(_)), "type should be a tag");
    }
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
    // assert-type now returns Nil after preprocessing
    assert!(
      matches!(assert_typed_result, Some(Calcit::Nil)),
      "assert-type should be preprocessed to Nil"
    );

    // Check that type info persists in the trailing reference
    if let Some(Calcit::Local(local)) = nodes.get(3) {
      assert!(local.type_info.is_some(), "type info should persist for later usages");
      // Verify the type value
      if let Some(type_val) = &local.type_info {
        assert!(matches!(**type_val, Calcit::Tag(_)), "type should be a tag");
      }
    } else {
      panic!("expected trailing local expression");
    }
  }

  #[test]
  fn validates_record_field_access() {
    use cirru_edn::EdnTag;

    // Create a test record type with fields: name, age
    let test_record = Calcit::Record(CalcitRecord {
      name: EdnTag::from("Person"),
      fields: Arc::new(vec![EdnTag::from("age"), EdnTag::from("name")]), // sorted
      values: Arc::new(vec![Calcit::Nil, Calcit::Nil]),
      class: None,
    });

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
    scope_types.insert(Arc::from("user"), Arc::new(test_record));

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
    let test_record = Calcit::Record(CalcitRecord {
      name: EdnTag::from("Person"),
      fields: Arc::new(vec![EdnTag::from("age"), EdnTag::from("name")]), // sorted
      values: Arc::new(vec![Calcit::Nil, Calcit::Nil]),
      class: None,
    });

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
    scope_types.insert(Arc::from("user"), Arc::new(test_record));

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
  fn validates_method_field_access() {
    use cirru_edn::EdnTag;

    // Create a test record type with fields: name, age
    let test_record = Calcit::Record(CalcitRecord {
      name: EdnTag::from("Person"),
      fields: Arc::new(vec![EdnTag::from("age"), EdnTag::from("name")]), // sorted
      values: Arc::new(vec![Calcit::Nil, Calcit::Nil]),
      class: None,
    });

    // Test expression: (user.-name) - wrapped in a list to trigger method parsing
    let expr = Cirru::List(vec![Cirru::leaf("user.-name")]);

    let code = code_to_calcit(&expr, "tests.record", "demo", vec![]).expect("parse cirru");

    // Set up scope with user variable
    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("user"));

    let mut scope_types: ScopeTypes = ScopeTypes::new();
    // Pre-populate with record type
    scope_types.insert(Arc::from("user"), Arc::new(test_record));

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
  fn warns_on_invalid_method_field_access() {
    use cirru_edn::EdnTag;

    // Create a test record type with fields: name, age
    let test_record = Calcit::Record(CalcitRecord {
      name: EdnTag::from("Person"),
      fields: Arc::new(vec![EdnTag::from("age"), EdnTag::from("name")]), // sorted
      values: Arc::new(vec![Calcit::Nil, Calcit::Nil]),
      class: None,
    });

    // Test expression: (user.-email) - invalid field, wrapped in list
    let expr = Cirru::List(vec![Cirru::leaf("user.-email")]);

    let code = code_to_calcit(&expr, "tests.record", "demo", vec![]).expect("parse cirru");

    // Set up scope with user variable
    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("user"));

    let mut scope_types: ScopeTypes = ScopeTypes::new();
    // Pre-populate with record type
    scope_types.insert(Arc::from("user"), Arc::new(test_record));

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
  fn rejects_slice_on_number_type() {
    let expr = Cirru::List(vec![
      Cirru::leaf("&let"),
      Cirru::List(vec![Cirru::leaf("n"), Cirru::leaf("42")]),
      Cirru::List(vec![Cirru::leaf("assert-type"), Cirru::leaf("n"), Cirru::leaf(":number")]),
      Cirru::List(vec![Cirru::leaf(".slice"), Cirru::leaf("n"), Cirru::leaf("1"), Cirru::leaf("3")]),
    ]);

    let code = code_to_calcit(&expr, "tests.slice", "demo", vec![]).expect("parse cirru");
    let scope_defs: HashSet<Arc<str>> = HashSet::new();
    let mut scope_types: ScopeTypes = ScopeTypes::new();
    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let result = preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.slice", &warnings, &stack);
    assert!(result.is_err(), "preprocess should reject mismatched `.slice` receivers");
    if let Err(err) = result {
      let msg = format!("{err}");
      assert!(msg.contains(".slice"), "error should mention the method name: {msg}");
      assert!(msg.contains(":number"), "error should mention the annotated type: {msg}");
    }
  }
}
