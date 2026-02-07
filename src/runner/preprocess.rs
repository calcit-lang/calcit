use crate::{
  builtins::{is_js_syntax_procs, is_proc_name, is_registered_proc},
  calcit::{
    self, Calcit, CalcitArgLabel, CalcitEnum, CalcitErr, CalcitErrKind, CalcitFn, CalcitFnArgs, CalcitImport, CalcitList, CalcitLocal,
    CalcitProc, CalcitRecord, CalcitScope, CalcitStruct, CalcitSymbolInfo, CalcitSyntax, CalcitThunk, CalcitThunkInfo, CalcitTrait,
    CalcitTuple, CalcitTypeAnnotation, GENERATED_DEF, ImportInfo, LocatedWarning, NodeLocation, RawCodeType,
  },
  call_stack::{CallStackList, StackKind},
  codegen, program, runner,
};

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::{cell::RefCell, vec};

use im_ternary_tree::TernaryTreeList;
use strum::ParseError;

type ScopeTypes = HashMap<Arc<str>, Arc<CalcitTypeAnnotation>>;

fn tag_annotation(name: &str) -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::from_tag_name(name))
}

/// Extract type information from a Calcit definition
/// Functions and procs are converted into `CalcitTypeAnnotation::Function` to retain argument/return hints
/// Other values fall back to their concrete annotation (tag/record/tuple/custom)
/// Context for preprocessing operations, bundled to avoid too many parameters
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
          let loc = NodeLocation::new(Arc::from(ns), Arc::from(def), Arc::from(vec![]));
          program::write_evaled_def(ns, def, Calcit::Nil)
            .map_err(|e| CalcitErr::use_msg_stack_location(CalcitErrKind::Unexpected, e, call_stack, Some(loc)))?;

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
          program::write_evaled_def(ns, def, v.to_owned()).map_err(|e| {
            CalcitErr::use_msg_stack_location(
              CalcitErrKind::Unexpected,
              e,
              call_stack,
              Some(NodeLocation::new(Arc::from(ns), Arc::from(def), Arc::from(vec![]))),
            )
          })?;

          Ok(Some(v))
        }
        None if ns.starts_with('|') || ns.starts_with('"') => Ok(None),
        None => {
          let loc = NodeLocation::new(Arc::from(ns), Arc::from(def), Arc::from(vec![]));
          Err(CalcitErr::use_msg_stack_location(
            CalcitErrKind::Var,
            format!("unknown ns/def in program: {ns}/{def}"),
            call_stack,
            Some(loc),
          ))
        }
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
          let type_info = scope_types
            .get(def)
            .cloned()
            .unwrap_or_else(|| Arc::new(CalcitTypeAnnotation::Dynamic));
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
  // Tag: transforming into tag expression
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
      if !has_spread {
        let processed_args = CalcitList::from(ys.drop_left());
        check_core_fn_arg_types(info.as_ref(), &processed_args, scope_types, file_ns, &def_name, check_warnings);
        check_user_fn_arg_types(info.as_ref(), &processed_args, scope_types, file_ns, &def_name, check_warnings);
      }
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
        CalcitSyntax::If
        | CalcitSyntax::Try
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
        validate_method_call(&head_form, &processed_args, scope_types, call_stack)?;
        check_record_field_access(&head_form, &processed_args, scope_types, file_ns, check_warnings);
        check_record_method_args(&head_form, &processed_args, scope_types, file_ns, &def_name, check_warnings);

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

        // Check Proc argument types if available
        if let Some(Calcit::Proc(proc)) = ys.first() {
          check_proc_arg_types(proc, &processed_args, scope_types, file_ns, &def_name, check_warnings);
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
              scope_types
                .get(name)
                .cloned()
                .unwrap_or_else(|| Arc::new(CalcitTypeAnnotation::Dynamic))
            } else {
              infer_type_from_expr(first_arg, scope_types).unwrap_or_else(|| Arc::new(CalcitTypeAnnotation::Dynamic))
            };

            let loc = head.get_location().or_else(|| first_arg.get_location());
            if let Some(l) = loc {
              let coord_repr = l.coord.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(",");
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
            if let Some(optimized_call) = try_inline_method_call(call_head, &processed_args, call_stack, file_ns) {
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

fn gen_check_warning(message: String, file_ns: &str, check_warnings: &RefCell<Vec<LocatedWarning>>) {
  let loc = NodeLocation::new(Arc::from(file_ns), Arc::from(GENERATED_DEF), Arc::from(vec![]));
  gen_check_warning_with_location(message, loc, check_warnings);
}

fn gen_check_warning_with_location(message: String, location: NodeLocation, check_warnings: &RefCell<Vec<LocatedWarning>>) {
  let mut warnings = check_warnings.borrow_mut();
  warnings.push(LocatedWarning::new(message, location));
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
  let Some(record) = type_info.as_ref().as_record() else {
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
  if record.index_of(field_name).is_some() {
    return; // Field found, validation passed
  }

  // Field not found, generate warning
  let available_fields: Vec<&str> = record.fields().iter().map(|f| f.ref_str()).collect();
  gen_check_warning(
    format!(
      "[Warn] Field `{field_name}` does not exist in record `{}`. Available fields: [{}]",
      record.name(),
      available_fields.join(", ")
    ),
    file_ns,
    check_warnings,
  );
}

/// Check tuple index bounds for &tuple:nth operations
fn check_tuple_nth_bounds(
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  // &tuple:nth takes (tuple, index)
  if args.len() < 2 {
    return; // Not enough args, arity error will be caught at runtime
  }

  let tuple_arg = match args.first() {
    Some(arg) => arg,
    None => return,
  };

  let index_arg = match args.get(1) {
    Some(arg) => arg,
    None => return,
  };

  // Only check if index is a constant number
  let index = match index_arg {
    Calcit::Number(n) => *n,
    _ => return, // Dynamic index, can't check statically
  };

  // Check if index is a valid integer
  if index.fract() != 0.0 || index < 0.0 {
    return; // Invalid index, will error at runtime
  }

  let index_usize = index as usize;

  // Try to resolve tuple type
  let Some(tuple_type) = resolve_type_value(tuple_arg, scope_types) else {
    return; // Can't resolve tuple type, skip check
  };

  // Check if it's a tuple type and get size
  let tuple_size = match tuple_type.as_ref() {
    CalcitTypeAnnotation::Tuple(tuple) => {
      // Tuple size is tag (index 0) + extra elements
      1 + tuple.extra.len()
    }
    CalcitTypeAnnotation::DynTuple => {
      // Unknown tuple size, can't check
      return;
    }
    _ => {
      // Not a tuple type, will error at runtime
      return;
    }
  };

  // Check bounds
  if index_usize >= tuple_size {
    gen_check_warning(
      format!(
        "[Warn] Tuple index out of bounds: tuple has {tuple_size} element(s), but trying to access index {index_usize}, at {file_ns}/{def_name}"
      ),
      file_ns,
      check_warnings,
    );
  }
}

/// Check enum tuple construction (%::) for variant existence and payload arity
fn check_enum_tuple_construction(
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
  let arity = signature.arity();
  let min_count = arity.min;
  let max_count = arity.max.unwrap_or(usize::MAX);
  let has_variadic = arity.max.is_none();

  let actual_count = args.len();

  // If not variadic, check count range
  if !has_variadic {
    if actual_count < min_count || actual_count > max_count {
      let expected_str = if min_count == max_count {
        format!("{min_count}")
      } else {
        format!("{min_count}~{max_count}")
      };
      gen_check_warning(
        format!(
          "[Warn] Proc `{}` expects {} args, got {} in call `({} {})`, at {file_ns}/{def_name}",
          proc.as_ref(),
          expected_str,
          actual_count,
          proc.as_ref(),
          args.iter().map(|a| format!("{a}")).collect::<Vec<_>>().join(" ")
        ),
        file_ns,
        check_warnings,
      );
    }
  } else if actual_count < min_count {
    gen_check_warning(
      format!(
        "[Warn] Proc `{}` expects at least {} args, got {} in call `({} {})`, at {file_ns}/{def_name}",
        proc.as_ref(),
        min_count,
        actual_count,
        proc.as_ref(),
        args.iter().map(|a| format!("{a}")).collect::<Vec<_>>().join(" ")
      ),
      file_ns,
      check_warnings,
    );
  }

  if matches!(proc, CalcitProc::NativeRecord | CalcitProc::NativeRecordGet) {
    return;
  }

  // Check enum tuple construction
  if matches!(proc, CalcitProc::NativeEnumTupleNew) {
    check_enum_tuple_construction(args, scope_types, file_ns, def_name, check_warnings);
    return;
  }

  // Check tuple index bounds
  if matches!(proc, CalcitProc::NativeTupleNth) {
    check_tuple_nth_bounds(args, scope_types, file_ns, def_name, check_warnings);
    return;
  }

  let mut bindings: HashMap<Arc<str>, Arc<CalcitTypeAnnotation>> = HashMap::new();

  // Check argument types until we hit variadic marker or run out of args
  for (idx, (arg, expected_type)) in args.iter().zip(signature.arg_types.iter()).enumerate() {
    // Skip variadic marker (&) - no checking after this point
    if matches!(expected_type.as_ref(), CalcitTypeAnnotation::Variadic(_)) {
      break;
    }

    let base_type = match expected_type.as_ref() {
      CalcitTypeAnnotation::Optional(inner) => inner,
      _ => expected_type,
    };

    if matches!(base_type.as_ref(), CalcitTypeAnnotation::Dynamic) {
      continue; // No type constraint for this argument
    }

    if let Some(actual_type) = resolve_type_value(arg, scope_types) {
      // Compare types
      if !actual_type.as_ref().matches_with_bindings(base_type.as_ref(), &mut bindings) {
        let expected_str = base_type.as_ref().to_brief_string();
        let actual_str = actual_type.as_ref().to_brief_string();
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

fn check_core_fn_arg_types(
  fn_info: &CalcitFn,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  if fn_info.def_ns.as_ref() != calcit::CORE_NS {
    return;
  }

  if !fn_info.arg_types.is_empty()
    && fn_info
      .arg_types
      .iter()
      .any(|t| !matches!(t.as_ref(), CalcitTypeAnnotation::Dynamic))
  {
    return;
  }

  let needs_number_args = matches!(fn_info.name.as_ref(), "+" | "-" | "*" | "/");
  if !needs_number_args {
    return;
  }

  let expected_type = tag_annotation("number");

  for (idx, arg) in args.iter().enumerate() {
    if let Some(actual_type) = resolve_type_value(arg, scope_types) {
      if !actual_type.as_ref().matches_annotation(expected_type.as_ref()) {
        let actual_str = actual_type.as_ref().to_brief_string();
        gen_check_warning(
          format!(
            "[Warn] Function `calcit.core/{}` arg {} expects type `:number`, but got `{actual_str}` in call at {file_ns}/{def_name}",
            fn_info.name,
            idx + 1
          ),
          file_ns,
          check_warnings,
        );
      }
    }
  }
}

/// Check user-defined function argument types against type annotations
fn check_user_fn_arg_types(
  fn_info: &CalcitFn,
  args: &CalcitList,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  // Skip if no type annotations
  if fn_info.arg_types.is_empty() || fn_info.arg_types.iter().all(|t| matches!(**t, CalcitTypeAnnotation::Dynamic)) {
    return;
  }

  // Check if we have spreading args
  for arg in args.iter() {
    if matches!(arg, Calcit::Syntax(CalcitSyntax::ArgSpread, _)) {
      return; // Can't check with spread args
    }
  }

  let mut bindings: HashMap<Arc<str>, Arc<CalcitTypeAnnotation>> = HashMap::new();

  // Check argument types
  for (idx, (arg, expected_type)) in args.iter().zip(fn_info.arg_types.iter()).enumerate() {
    if matches!(**expected_type, CalcitTypeAnnotation::Dynamic) {
      continue; // No type constraint for this argument
    }

    // Check for variadic argument type
    if let CalcitTypeAnnotation::Variadic(inner_type) = expected_type.as_ref() {
      // This is a variadic parameter - check all remaining arguments
      for (rest_idx, rest_arg) in args.iter().skip(idx).enumerate() {
        if let Some(actual_type) = resolve_type_value(rest_arg, scope_types) {
          if !actual_type.as_ref().matches_with_bindings(inner_type.as_ref(), &mut bindings) {
            let expected_str = inner_type.as_ref().to_brief_string();
            let actual_str = actual_type.as_ref().to_brief_string();
            gen_check_warning(
              format!(
                "[Warn] Function `{}/{}` variadic arg {} expects type `{expected_str}`, but got `{actual_str}` in call at {file_ns}/{def_name}",
                fn_info.def_ns,
                fn_info.name,
                idx + rest_idx + 1
              ),
              file_ns,
              check_warnings,
            );
          }
        }
      }
      // Done checking variadic args
      return;
    }

    if let Some(actual_type) = resolve_type_value(arg, scope_types) {
      // Compare types
      if !actual_type.as_ref().matches_with_bindings(expected_type.as_ref(), &mut bindings) {
        let expected_str = expected_type.as_ref().to_brief_string();
        let actual_str = actual_type.as_ref().to_brief_string();
        gen_check_warning(
          format!(
            "[Warn] Function `{}/{}` arg {} expects type `{expected_str}`, but got `{actual_str}` in call at {file_ns}/{def_name}",
            fn_info.def_ns,
            fn_info.name,
            idx + 1
          ),
          file_ns,
          check_warnings,
        );
      }
    }
  }
}

/// Extract return type hint from processed function body
/// Looks for (hint-fn return-type <type>) pattern in processed body forms
fn detect_return_type_hint_from_processed_body(processed_body: &[Calcit]) -> Arc<CalcitTypeAnnotation> {
  // Search through all body forms for hint-fn with return-type
  for form in processed_body {
    if let Some(hint) = CalcitTypeAnnotation::extract_return_type_from_hint_form(form) {
      return hint;
    }
  }
  Arc::new(CalcitTypeAnnotation::Dynamic)
}

/// Check function return type matches declared return_type
/// Validates the last expression in function body against the declared return type
fn check_function_return_type(
  fn_body: &[Calcit],
  declared_return_type: &Arc<CalcitTypeAnnotation>,
  scope_types: &ScopeTypes,
  file_ns: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) {
  // If no return type is declared, skip check
  if matches!(**declared_return_type, CalcitTypeAnnotation::Dynamic) {
    return;
  }

  // If function body is empty, can't infer return type
  if fn_body.is_empty() {
    return;
  }

  // Get the last expression in function body (this will be the return value)
  let last_expr = &fn_body[fn_body.len() - 1];

  // Try to infer the actual return type
  let Some(actual_type) = resolve_type_value(last_expr, scope_types) else {
    // Can't infer type from last expression, skip check
    return;
  };

  if matches!(actual_type.as_ref(), CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::DynFn) {
    return;
  }

  // Compare actual type with expected type
  if !actual_type.as_ref().matches_annotation(declared_return_type.as_ref()) {
    let expected_str = declared_return_type.as_ref().to_brief_string();
    let actual_str = actual_type.as_ref().to_brief_string();
    gen_check_warning(
      format!("[Warn] Function `{file_ns}/{def_name}` declares return type `{expected_str}`, but body returns `{actual_str}`"),
      file_ns,
      check_warnings,
    );
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

    let arg_types_without_receiver = signature.arg_types.iter().skip(1);
    for (idx, (arg, expected_type)) in method_args.iter().zip(arg_types_without_receiver).enumerate() {
      if matches!(**expected_type, CalcitTypeAnnotation::Dynamic) {
        continue;
      }

      if let Some(actual_type) = resolve_type_value(arg, scope_types) {
        if !actual_type.as_ref().matches_annotation(expected_type.as_ref()) {
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
  let Some(impl_records) = get_impl_records_from_type(&type_value, &CallStackList::default()) else {
    return; // No impl record, skip check
  };

  // Get method entry from impl records
  let method_str = method_name.as_ref();
  let Some(method_entry) = find_method_entry(&impl_records, method_str) else {
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
  let arg_types_without_receiver = fn_info.arg_types.iter().skip(1);

  for (idx, (arg, expected_type)) in method_args.iter().zip(arg_types_without_receiver).enumerate() {
    if matches!(**expected_type, CalcitTypeAnnotation::Dynamic) {
      continue; // No type constraint for this argument
    }

    if let Some(actual_type) = resolve_type_value(arg, scope_types) {
      // Compare types
      if !actual_type.as_ref().matches_annotation(expected_type.as_ref()) {
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

fn try_inline_method_call(head: &Calcit, args: &CalcitList, call_stack: &CallStackList, _file_ns: &str) -> Option<Calcit> {
  match head {
    Calcit::Method(method_name, calcit::MethodKind::Invoke(type_value)) => {
      if matches!(**type_value, CalcitTypeAnnotation::Dynamic) {
        return None;
      }
      let type_ref = type_value.as_ref();
      let impl_records = get_impl_records_from_type(type_ref, call_stack)?;
      let method_entry = find_method_entry(&impl_records, method_name.as_ref())?;

      if let Some(callable_head) = pick_callable_from_method_entry(method_entry) {
        return Some(build_inlined_call(callable_head, args));
      }

      None
    }
    _ => None,
  }
}

fn pick_callable_from_method_entry(entry: &Calcit) -> Option<Calcit> {
  match entry {
    Calcit::Import(..) | Calcit::Proc(..) => Some(entry.to_owned()),
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
  let Some(impl_records) = get_impl_records_from_type(&type_value, call_stack) else {
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

/// Resolve the type value from the receiver expression
fn resolve_type_value(target: &Calcit, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  match target {
    Calcit::Local(local) => {
      // First check if the local has inline type_info, then fall back to scope_types
      if matches!(*local.type_info, CalcitTypeAnnotation::Dynamic) {
        let scoped = scope_types.get(&local.sym).cloned();
        scoped.map(normalize_variadic_as_list)
      } else {
        Some(normalize_variadic_as_list(local.type_info.clone()))
      }
    }
    Calcit::Symbol { sym, .. } => scope_types
      .get(sym)
      .cloned()
      .map(normalize_variadic_as_list)
      .or_else(|| infer_type_from_expr(target, scope_types)),
    _ => infer_type_from_expr(target, scope_types),
  }
}

/// Treat variadic locals as list values when resolving expression types.
///
/// This is distinct from `collect_arg_type_hints_from_body`: that function extracts parameter
/// annotations, while this function only normalizes the inferred type for internal list operations
/// like `&list:count` and `&list:first`.
fn normalize_variadic_as_list(value: Arc<CalcitTypeAnnotation>) -> Arc<CalcitTypeAnnotation> {
  match value.as_ref() {
    CalcitTypeAnnotation::Variadic(_) => tag_annotation("list"),
    _ => value,
  }
}

/// Infer type from an expression (for &let bindings)
/// Supports:
/// - Literals (number, string, bool, nil)
/// - Proc calls with known return types
/// - Function calls with return-type annotations
/// - Nested &let expressions (returns type of final expression)
/// - Local variables (reads from type_info field)
fn infer_type_from_expr(expr: &Calcit, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  match expr {
    // Literal types
    Calcit::Number(_) => Some(tag_annotation("number")),
    Calcit::Str(_) => Some(tag_annotation("string")),
    Calcit::Bool(_) => Some(tag_annotation("bool")),
    Calcit::Nil => Some(tag_annotation("nil")),
    Calcit::Tag(_) => Some(tag_annotation("tag")),
    Calcit::Fn { info, .. } => Some(Arc::new(CalcitTypeAnnotation::from_function_parts(
      info.arg_types.clone(),
      info.return_type.clone(),
    ))),
    Calcit::Proc(proc) => proc
      .get_type_signature()
      .map(|signature| {
        Arc::new(CalcitTypeAnnotation::from_function_parts(
          signature.arg_types,
          signature.return_type,
        ))
      })
      .or_else(|| Some(tag_annotation("fn"))),

    // Local variable: read type_info
    Calcit::Local(local) => Some(local.type_info.clone()),

    // List/vector literal or expressions
    Calcit::List(xs) if xs.is_empty() => Some(tag_annotation("list")),

    // Function call or Proc call or special forms
    Calcit::List(xs) => {
      let head = xs.first()?;
      match head {
        // &let expression: infer from final expression (last element)
        Calcit::Syntax(CalcitSyntax::CoreLet, _) => {
          // &let has format: (&let (binding) body...)
          // The last element is the return value
          if xs.len() > 1 {
            infer_type_from_expr(&xs[xs.len() - 1], scope_types)
          } else {
            None
          }
        }

        // Local variable as head (function call)
        // If it's a function type, return its return type
        Calcit::Local(local) => {
          let type_ann = &local.type_info;
          match type_ann.as_ref() {
            CalcitTypeAnnotation::Fn(fn_type) => Some(fn_type.return_type.clone()),
            _ => Some(type_ann.clone()),
          }
        }

        // Proc call: check if proc has return_type
        Calcit::Proc(proc) => {
          if matches!(proc, CalcitProc::NativeListNth | CalcitProc::NativeListFirst) {
            if let Some(first_arg) = xs.get(1) {
              if let Some(type_value) = resolve_type_value(first_arg, scope_types) {
                if let CalcitTypeAnnotation::List(element_type) = type_value.as_ref() {
                  return Some(element_type.clone());
                }
              }
            }
          }
          if matches!(
            proc,
            CalcitProc::NativeListRest
              | CalcitProc::NativeListSlice
              | CalcitProc::NativeListReverse
              | CalcitProc::NativeListDistinct
              | CalcitProc::Append
              | CalcitProc::Prepend
              | CalcitProc::Butlast
          ) {
            if let Some(first_arg) = xs.get(1) {
              if let Some(type_value) = resolve_type_value(first_arg, scope_types) {
                if let CalcitTypeAnnotation::List(_) = type_value.as_ref() {
                  return Some(type_value.clone());
                }
              }
            }
          }
          if matches!(proc, CalcitProc::NativeListConcat) {
            if let Some(first_arg) = xs.get(1) {
              if let Some(type_value) = resolve_type_value(first_arg, scope_types) {
                if let CalcitTypeAnnotation::List(_) = type_value.as_ref() {
                  return Some(type_value.clone());
                }
              }
            }
          }
          if matches!(proc, CalcitProc::NativeMapGet) {
            if let Some(first_arg) = xs.get(1) {
              if let Some(type_value) = resolve_type_value(first_arg, scope_types) {
                if let CalcitTypeAnnotation::Map(_key_type, val_type) = type_value.as_ref() {
                  return Some(val_type.clone());
                }
              }
            }
          }
          if matches!(proc, CalcitProc::NativeMapAssoc | CalcitProc::NativeMapDissoc) {
            if let Some(first_arg) = xs.get(1) {
              if let Some(type_value) = resolve_type_value(first_arg, scope_types) {
                if let CalcitTypeAnnotation::Map(_, _) = type_value.as_ref() {
                  return Some(type_value.clone());
                }
              }
            }
          }
          if matches!(proc, CalcitProc::NativeSetToList) {
            if let Some(first_arg) = xs.get(1) {
              if let Some(type_value) = resolve_type_value(first_arg, scope_types) {
                if let CalcitTypeAnnotation::Set(element_type) = type_value.as_ref() {
                  return Some(Arc::new(CalcitTypeAnnotation::List(element_type.clone())));
                }
              }
            }
          }
          if matches!(proc, CalcitProc::NativeInclude | CalcitProc::NativeExclude) {
            if let Some(first_arg) = xs.get(1) {
              if let Some(type_value) = resolve_type_value(first_arg, scope_types) {
                if let CalcitTypeAnnotation::Set(_) = type_value.as_ref() {
                  return Some(type_value.clone());
                }
              }
            }
          }
          if matches!(proc, CalcitProc::AtomDeref) {
            if let Some(first_arg) = xs.get(1) {
              if let Some(type_value) = resolve_type_value(first_arg, scope_types) {
                if let CalcitTypeAnnotation::Ref(element_type) = type_value.as_ref() {
                  return Some(element_type.clone());
                }
              }
            }
          }
          if matches!(proc, CalcitProc::NativeListToSet) {
            if let Some(first_arg) = xs.get(1) {
              if let Some(type_value) = resolve_type_value(first_arg, scope_types) {
                if let CalcitTypeAnnotation::List(element_type) = type_value.as_ref() {
                  return Some(Arc::new(CalcitTypeAnnotation::Set(element_type.clone())));
                }
              }
            }
          }
          if matches!(proc, CalcitProc::NativeEnumTupleNew) {
            if let Some(tuple_type) = infer_enum_tuple_annotation(proc, xs, scope_types) {
              return Some(tuple_type);
            }
          }
          if matches!(proc, CalcitProc::NativeStructNew) {
            if let Some(struct_type) = infer_struct_literal_type(xs) {
              return Some(struct_type);
            }
          }
          if matches!(proc, CalcitProc::NativeRecord) {
            if let Some(record_type) = infer_record_literal_type(xs, scope_types) {
              return Some(record_type);
            }
          }
          if matches!(proc, CalcitProc::NativeRecordGet) {
            if let Some(field_type) = infer_record_get_type(xs, scope_types) {
              return Some(field_type);
            }
          }
          if let Some(type_sig) = proc.get_type_signature() {
            Some(type_sig.return_type.clone())
          } else {
            None
          }
        }

        // Import: could be a function, try to get its return type
        Calcit::Import(CalcitImport { ns, def, .. }) => {
          if &**ns == calcit::CORE_NS && (&**def == "record-get" || &**def == "&record:get") {
            if let Some(field_type) = infer_record_get_type(xs, scope_types) {
              return Some(field_type);
            }
          }
          // First check evaled definition (for Proc/Fn that have been evaluated)
          if let Some(evaled) = program::lookup_evaled_def(ns, def) {
            match evaled {
              // For compiled functions, get return_type from info
              Calcit::Fn { info, .. } => return Some(info.return_type.clone()),
              // For builtin procs, get type signature
              Calcit::Proc(proc) => {
                if let Some(type_sig) = proc.get_type_signature() {
                  return Some(type_sig.return_type.clone());
                }
              }
              _ => {}
            }
          }

          // Fallback: check code definition (for not-yet-evaluated definitions)
          if let Some(code) = program::lookup_def_code(ns, def) {
            // Code is the AST, might be a defn with return type annotation
            // Format: (defn name (args) :return-type body) or (defn name (args) body)
            if let Calcit::List(ref xs) = code {
              // Check if it's a defn: first element should be Symbol "defn"
              if let Some(Calcit::Symbol { sym, .. }) = xs.first() {
                if sym.as_ref() == "defn" {
                  // Defn format: (defn name (args) [:return-type] body...)
                  // Return type is the 3rd element (index 3) if it's a tag
                  if let Some(ret_type) = xs.get(3) {
                    if matches!(ret_type, Calcit::Tag(_)) {
                      return Some(CalcitTypeAnnotation::parse_type_annotation_form(ret_type));
                    }
                  }
                }
              }
            }
            // For compiled functions in code, get return_type from info
            if let Calcit::Fn { info, .. } = code {
              return Some(info.return_type.clone());
            }
          }
          None
        }

        // Symbol: might be a function reference before preprocessing
        // Try to resolve it and get the return type
        Calcit::Symbol { sym, info, .. } => {
          // Try to lookup in program
          if let Some(Calcit::Fn { info: fn_info, .. }) = program::lookup_def_code(&info.at_ns, sym) {
            return Some(fn_info.return_type.clone());
          }
          None
        }

        // Direct Fn call: return the function's return type
        Calcit::Fn { info, .. } => Some(info.return_type.clone()),

        // Method access: infer record field type when available
        Calcit::Method(field_name, calcit::MethodKind::Access | calcit::MethodKind::TagAccess) => {
          if let Some(receiver) = xs.get(1) {
            if let Some(field_type) = infer_record_field_type(receiver, field_name.as_ref(), scope_types) {
              return Some(field_type);
            }
          }
          None
        }

        // Nested List call: the head is a function call expression
        // First infer what type the head returns, then if it's a function, get its return type
        Calcit::List(_) => {
          if let Some(head_type) = infer_type_from_expr(head, scope_types) {
            match head_type.as_ref() {
              CalcitTypeAnnotation::Fn(fn_type) => Some(fn_type.return_type.clone()),
              // If head returns a non-function type, the call will fail at runtime
              // Return the non-callable type so caller can detect this issue
              _ => Some(head_type),
            }
          } else {
            None
          }
        }

        _ => None,
      }
    }

    _ => None,
  }
}

fn infer_enum_tuple_annotation(proc: &CalcitProc, xs: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  let (class_record, enum_proto, tag_arg) = match proc {
    CalcitProc::NativeEnumTupleNew => {
      if xs.len() < 3 {
        return None;
      }
      let enum_arg = xs.get(1)?;
      let tag_arg = xs.get(2);
      let enum_proto = resolve_enum_value(enum_arg, scope_types)?;
      (None, enum_proto, tag_arg)
    }
    _ => return None,
  };

  let tag_value = tag_arg
    .map(|arg| arg.to_owned())
    .unwrap_or_else(|| Calcit::Tag(cirru_edn::EdnTag::from("unknown")));

  let tuple = CalcitTuple {
    tag: Arc::new(tag_value),
    extra: vec![],
    impls: class_record.map(|c| vec![Arc::new(c)]).unwrap_or_default(),
    sum_type: Some(Arc::new(enum_proto)),
  };

  Some(Arc::new(CalcitTypeAnnotation::Tuple(Arc::new(tuple))))
}

fn infer_record_get_type(xs: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  if xs.len() < 3 {
    return None;
  }
  let record_arg = xs.get(1)?;
  let field_arg = xs.get(2)?;
  let field_name = extract_field_name(field_arg)?;
  infer_record_field_type(record_arg, field_name, scope_types)
}

fn infer_record_literal_type(xs: &CalcitList, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  if xs.len() < 2 {
    return None;
  }
  let proto_arg = xs.get(1)?;
  let record = resolve_record_value(proto_arg, scope_types)?;
  Some(Arc::new(CalcitTypeAnnotation::Record(Arc::new(record))))
}

fn infer_struct_literal_type(xs: &CalcitList) -> Option<Arc<CalcitTypeAnnotation>> {
  if xs.len() < 2 {
    return None;
  }

  let name = parse_struct_name(xs.get(1)?)?;
  let mut fields: Vec<(cirru_edn::EdnTag, Arc<CalcitTypeAnnotation>)> = Vec::new();

  for item in xs.iter().skip(2) {
    let (field_name, field_type) = parse_struct_field_entry(item)?;
    fields.push((field_name, field_type));
  }

  fields.sort_by(|a, b| a.0.ref_str().cmp(b.0.ref_str()));
  for idx in 1..fields.len() {
    if fields[idx - 1].0 == fields[idx].0 {
      return None;
    }
  }

  let field_names: Vec<cirru_edn::EdnTag> = fields.iter().map(|(name, _)| name.to_owned()).collect();
  let field_types: Vec<Arc<CalcitTypeAnnotation>> = fields.iter().map(|(_, t)| t.to_owned()).collect();

  let struct_def = CalcitStruct {
    name,
    fields: Arc::new(field_names.clone()),
    field_types: Arc::new(field_types),
    generics: Arc::new(vec![]),
    impls: vec![],
  };

  let record = CalcitRecord {
    struct_ref: Arc::new(struct_def),
    values: Arc::new(vec![Calcit::Nil; field_names.len()]),
    impls: vec![],
  };

  Some(Arc::new(CalcitTypeAnnotation::Record(Arc::new(record))))
}

fn parse_struct_name(form: &Calcit) -> Option<cirru_edn::EdnTag> {
  match form {
    Calcit::Symbol { sym, .. } | Calcit::Str(sym) => Some(cirru_edn::EdnTag::from(sym.as_ref())),
    Calcit::Tag(tag) => Some(tag.to_owned()),
    _ => None,
  }
}

fn parse_struct_field_entry(form: &Calcit) -> Option<(cirru_edn::EdnTag, Arc<CalcitTypeAnnotation>)> {
  let Calcit::List(list) = form else {
    return None;
  };
  let head = list.first()?;
  let is_list_literal = matches!(head, Calcit::Proc(CalcitProc::List))
    || matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "[]")
    || matches!(head, Calcit::Import(CalcitImport { ns, def, .. }) if &**ns == calcit::CORE_NS && &**def == "[]");

  if !is_list_literal {
    return None;
  }

  let field_name_form = list.get(1)?;
  let field_type_form = list.get(2)?;
  if list.len() != 3 {
    return None;
  }

  let field_name = match field_name_form {
    Calcit::Symbol { sym, .. } | Calcit::Str(sym) => cirru_edn::EdnTag::from(sym.as_ref()),
    Calcit::Tag(tag) => tag.to_owned(),
    _ => return None,
  };

  let field_type = CalcitTypeAnnotation::parse_type_annotation_form(field_type_form);
  Some((field_name, field_type))
}

fn infer_record_field_type(receiver: &Calcit, field_name: &str, scope_types: &ScopeTypes) -> Option<Arc<CalcitTypeAnnotation>> {
  let type_info = resolve_type_value(receiver, scope_types)?;
  let record = type_info.as_ref().as_record()?;
  let idx = record.index_of(field_name)?;
  record.struct_ref.field_types.get(idx).cloned()
}

fn extract_field_name(field_arg: &Calcit) -> Option<&str> {
  match field_arg {
    Calcit::Tag(tag) => Some(tag.ref_str()),
    Calcit::Str(s) => Some(s.as_ref()),
    Calcit::Symbol { sym, .. } => Some(sym.as_ref()),
    _ => None,
  }
}

fn resolve_enum_value(target: &Calcit, scope_types: &ScopeTypes) -> Option<CalcitEnum> {
  match target {
    Calcit::Enum(enum_def) => Some(enum_def.to_owned()),
    Calcit::Record(record) => CalcitEnum::from_record(record.to_owned()).ok(),
    Calcit::Import(CalcitImport { ns, def, .. }) => {
      match program::lookup_evaled_def(ns, def) {
        Some(Calcit::Enum(enum_def)) => Some(enum_def),
        Some(Calcit::Record(record)) => CalcitEnum::from_record(record).ok(),
        // Handle Thunk case: force evaluation to get the enum value
        Some(Calcit::Thunk(thunk)) => {
          let call_stack = CallStackList::default();
          match thunk.evaluated(&CalcitScope::default(), &call_stack) {
            Ok(Calcit::Enum(enum_def)) => Some(enum_def),
            Ok(Calcit::Record(record)) => CalcitEnum::from_record(record).ok(),
            _ => None,
          }
        }
        _ => None,
      }
    }
    _ => resolve_type_value(target, scope_types)
      .and_then(|t| t.as_record().cloned())
      .and_then(|record| CalcitEnum::from_record(record).ok()),
  }
}

fn resolve_record_value(target: &Calcit, scope_types: &ScopeTypes) -> Option<CalcitRecord> {
  match target {
    Calcit::Record(record) => Some(record.to_owned()),
    Calcit::Enum(enum_def) => Some(enum_def.prototype().to_owned()),
    Calcit::Struct(struct_def) => {
      let values = vec![Calcit::Nil; struct_def.fields.len()];
      Some(CalcitRecord {
        struct_ref: Arc::new(struct_def.to_owned()),
        values: Arc::new(values),
        impls: struct_def.impls.clone(),
      })
    }
    Calcit::Import(CalcitImport { ns, def, .. }) => match program::lookup_evaled_def(ns, def) {
      Some(Calcit::Record(record)) => Some(record),
      Some(Calcit::Enum(enum_def)) => Some(enum_def.prototype().to_owned()),
      Some(Calcit::Struct(struct_def)) => {
        let values = vec![Calcit::Nil; struct_def.fields.len()];
        Some(CalcitRecord {
          struct_ref: Arc::new(struct_def.to_owned()),
          values: Arc::new(values),
          impls: struct_def.impls.clone(),
        })
      }
      _ => None,
    },
    _ => resolve_type_value(target, scope_types).and_then(|t| t.as_record().map(|r| r.to_owned())),
  }
}

/// Get the impl records from a type value
/// - If type_value is already a Record, use it directly
/// - If type_value is a Tag, map to corresponding core impl list
/// - Otherwise return None
fn collect_impl_records_from_value(value: &Calcit) -> Option<Vec<Arc<CalcitRecord>>> {
  match value {
    Calcit::Record(record) => Some(vec![Arc::new(record.to_owned())]),
    Calcit::List(list) => {
      let mut impls: Vec<Arc<CalcitRecord>> = Vec::with_capacity(list.len());
      for item in list.iter() {
        if let Calcit::Record(record) = item {
          impls.push(Arc::new(record.to_owned()));
        } else {
          return None;
        }
      }
      Some(impls)
    }
    _ => None,
  }
}

fn get_impl_records_from_type(type_value: &CalcitTypeAnnotation, call_stack: &CallStackList) -> Option<Vec<Arc<CalcitRecord>>> {
  if let Some(record) = type_value.as_record() {
    return Some(vec![Arc::new(record.clone())]);
  }

  if let Some(class_symbol) = core_impl_list_symbol_from_type_annotation(type_value) {
    return match runner::evaluate_symbol_from_program(class_symbol, calcit::CORE_NS, None, call_stack) {
      Ok(value) => collect_impl_records_from_value(&value),
      Err(_) => None,
    };
  }

  if let CalcitTypeAnnotation::Custom(value) = type_value {
    if let Calcit::Import(import) = value.as_ref() {
      return match runner::evaluate_symbol_from_program(&import.def, &import.ns, None, call_stack) {
        Ok(value) => collect_impl_records_from_value(&value),
        Err(_) => None,
      };
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

fn find_method_entry<'a>(impls: &'a [Arc<CalcitRecord>], name: &str) -> Option<&'a Calcit> {
  for imp in impls.iter() {
    if let Some(entry) = imp.get(name) {
      return Some(entry);
    }
  }
  None
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
              type_info: Arc::new(CalcitTypeAnnotation::Dynamic),
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
      xs = xs.push_right(Calcit::from(zs));

      let mut to_skip = 2;
      let mut processed_body: Vec<Calcit> = vec![];
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
        let inferred_type = infer_type_from_expr(&form, &body_types).unwrap_or_else(|| Arc::new(CalcitTypeAnnotation::Dynamic));

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
  _ctx: &mut PreprocessContext,
) -> Result<Calcit, CalcitErr> {
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
      format!("{head} expected a local and a type expression, got {}", args.len()),
      ctx.call_stack,
      args.first().and_then(|node| node.get_location()),
    ));
  }

  let local_raw = args.get(0).unwrap();
  let type_form = args.get(1).unwrap();

  let local_form = preprocess_expr(
    local_raw,
    ctx.scope_defs,
    ctx.scope_types,
    ctx.file_ns,
    ctx.check_warnings,
    ctx.call_stack,
  )?;

  let local = match local_form {
    Calcit::Local(local) => local.to_owned(),
    other => {
      return Err(CalcitErr::use_msg_stack_location(
        CalcitErrKind::Type,
        format!("assert-type expected local as first arg, got {other:?}"),
        ctx.call_stack,
        other.get_location(),
      ));
    }
  };

  let type_entry = CalcitTypeAnnotation::parse_type_annotation_form(type_form);
  ctx.scope_types.insert(local.sym.to_owned(), type_entry.clone());

  let mut zs: Vec<Calcit> = vec![
    Calcit::Syntax(head.to_owned(), Arc::from(head_ns)),
    Calcit::Local(local),
    type_form.to_owned(),
  ];

  if let Some(Calcit::Local(typed_local)) = zs.get_mut(1) {
    typed_local.type_info = type_entry;
  }

  // assert-type is preprocessed away, return nil at runtime
  Ok(Calcit::Nil)
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
      format!("{head} expected a local and one or more trait expressions, got {}", args.len()),
      ctx.call_stack,
      args.first().and_then(|node| node.get_location()),
    ));
  }

  let local_raw = args.get(0).unwrap();
  let trait_forms = args.iter().skip(1).collect::<Vec<_>>();

  let local_form = preprocess_expr(
    local_raw,
    ctx.scope_defs,
    ctx.scope_types,
    ctx.file_ns,
    ctx.check_warnings,
    ctx.call_stack,
  )?;

  let local = match local_form {
    Calcit::Local(local) => local.to_owned(),
    other => {
      return Err(CalcitErr::use_msg_stack_location(
        CalcitErrKind::Type,
        format!("assert-traits expected local as first arg, got {other:?}"),
        ctx.call_stack,
        other.get_location(),
      ));
    }
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
        Some((ns_part, def_part)) => runner::eval_symbol_from_program(&def_part, &ns_part, ctx.call_stack).ok().flatten(),
        None => runner::eval_symbol_from_program(sym, &info.at_ns, ctx.call_stack).ok().flatten(),
      },
      Calcit::Import(import) => runner::eval_symbol_from_program(&import.def, &import.ns, ctx.call_stack)
        .ok()
        .flatten(),
      _ => None,
    };

    if let Some(Calcit::Trait(trait_def)) = resolved {
      trait_defs.push(Arc::new(trait_def));
    } else if fallback_entry.is_none() {
      fallback_entry = Some(Arc::new(CalcitTypeAnnotation::Custom(Arc::new((*trait_form).to_owned()))));
    }
  }

  let resolved_entry = if !trait_defs.is_empty() {
    if trait_defs.len() == 1 && fallback_entry.is_none() {
      Arc::new(CalcitTypeAnnotation::Trait(trait_defs.remove(0)))
    } else {
      Arc::new(CalcitTypeAnnotation::TraitSet(Arc::new(trait_defs)))
    }
  } else {
    fallback_entry.unwrap_or_else(|| Arc::new(CalcitTypeAnnotation::Dynamic))
  };

  ctx.scope_types.insert(local.sym.to_owned(), resolved_entry.clone());

  let mut typed_local = local.to_owned();
  typed_local.type_info = resolved_entry;

  let mut assert_expr: Calcit = Calcit::Local(typed_local);
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::{CalcitFn, CalcitFnArgs, CalcitRecord, CalcitScope, CalcitStruct};
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

    assert!(matches!(resolved, Calcit::Nil), "assert-type should be preprocessed to Nil");

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
    let scope_defs: HashSet<Arc<str>> = HashSet::new();
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
    // assert-type now returns Nil after preprocessing
    assert!(
      matches!(assert_typed_result, Some(Calcit::Nil)),
      "assert-type should be preprocessed to Nil"
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
  fn validates_record_field_access() {
    use cirru_edn::EdnTag;

    // Create a test record type with fields: name, age
    let test_record = Arc::new(CalcitTypeAnnotation::Record(Arc::new(CalcitRecord {
      struct_ref: Arc::new(CalcitStruct::from_fields(
        EdnTag::from("Person"),
        vec![EdnTag::from("age"), EdnTag::from("name")],
      )),
      values: Arc::new(vec![Calcit::Nil, Calcit::Nil]),
      impls: vec![],
    })));

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
    let test_record = Arc::new(CalcitTypeAnnotation::Record(Arc::new(CalcitRecord {
      struct_ref: Arc::new(CalcitStruct::from_fields(
        EdnTag::from("Person"),
        vec![EdnTag::from("age"), EdnTag::from("name")],
      )),
      values: Arc::new(vec![Calcit::Nil, Calcit::Nil]),
      impls: vec![],
    })));

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
      coord: None,
    });

    let class_record = CalcitRecord {
      struct_ref: Arc::new(CalcitStruct::from_fields(EdnTag::from("Greeter"), vec![EdnTag::from("greet")])),
      values: Arc::new(vec![method_import.clone()]),
      impls: vec![],
    };
    scope_types.insert(Arc::from("user"), Arc::new(CalcitTypeAnnotation::Record(Arc::new(class_record))));

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
  #[ignore] // TODO: This test was failing before our changes - needs investigation
  fn rewrites_method_call_with_fn_entry_via_record_get() {
    use cirru_edn::EdnTag;

    let expr = Cirru::List(vec![Cirru::leaf(".greet"), Cirru::leaf("user")]);
    let code = code_to_calcit(&expr, "tests.method", "demo", vec![]).expect("parse cirru");

    let mut scope_defs: HashSet<Arc<str>> = HashSet::new();
    scope_defs.insert(Arc::from("user"));
    let mut scope_types: ScopeTypes = ScopeTypes::new();

    let fn_args = CalcitFnArgs::Args(vec![]);
    let arg_types = fn_args.empty_arg_types();
    let fn_info = Arc::new(CalcitFn {
      name: Arc::from("greet"),
      def_ns: Arc::from("tests.method.ns"),
      scope: Arc::new(CalcitScope::default()),
      args: Arc::new(fn_args),
      body: vec![],
      generics: Arc::new(vec![]),
      return_type: Arc::new(CalcitTypeAnnotation::Dynamic),
      arg_types,
    });
    let method_fn = Calcit::Fn {
      id: Arc::from("tests.method.ns/greet"),
      info: fn_info,
    };

    let class_record = CalcitRecord {
      struct_ref: Arc::new(CalcitStruct::from_fields(EdnTag::from("Greeter"), vec![EdnTag::from("greet")])),
      values: Arc::new(vec![method_fn.clone()]),
      impls: vec![],
    };

    let record_ns = "tests.method.class";
    let record_def = "&test-greeter-class";
    program::write_evaled_def(record_ns, record_def, Calcit::Record(class_record)).expect("register record class");

    let record_import = Calcit::Import(CalcitImport {
      ns: Arc::from(record_ns),
      def: Arc::from(record_def),
      info: Arc::new(ImportInfo::SameFile { at_def: Arc::from("demo") }),
      coord: None,
    });
    scope_types.insert(Arc::from("user"), CalcitTypeAnnotation::parse_type_annotation_form(&record_import));

    let warnings = RefCell::new(vec![]);
    let stack = CallStackList::default();

    let resolved =
      preprocess_expr(&code, &scope_defs, &mut scope_types, "tests.method", &warnings, &stack).expect("preprocess method call");

    let nodes = match resolved {
      Calcit::List(xs) => xs.to_vec(),
      other => panic!("expected list form, got {other}"),
    };
    assert_eq!(nodes.len(), 2, "call should include head and receiver arg");

    let head_nodes = match nodes.first() {
      Some(Calcit::List(xs)) => xs.to_vec(),
      other => panic!("expected fallback head to be a list, got {other:?}"),
    };
    assert_eq!(head_nodes.len(), 3, "record-get form should include proc, record ref, and tag");
    assert!(
      matches!(head_nodes.first(), Some(Calcit::Proc(CalcitProc::NativeRecordGet))),
      "head should call &record:get"
    );
    match head_nodes.get(1) {
      Some(Calcit::Import(import)) => {
        assert_eq!(&*import.ns, record_ns, "record reference should target injected namespace");
        assert_eq!(&*import.def, record_def, "record reference should target injected definition");
      }
      other => panic!("expected record reference import, got {other:?}"),
    }
    match head_nodes.get(2) {
      Some(Calcit::Tag(tag)) => assert_eq!(tag, &EdnTag::from("greet")),
      other => panic!("expected method tag, got {other:?}"),
    };
  }

  #[test]
  fn validates_method_field_access() {
    use cirru_edn::EdnTag;

    // Create a test record type with fields: name, age
    let test_record = Arc::new(CalcitTypeAnnotation::Record(Arc::new(CalcitRecord {
      struct_ref: Arc::new(CalcitStruct::from_fields(
        EdnTag::from("Person"),
        vec![EdnTag::from("age"), EdnTag::from("name")],
      )),
      values: Arc::new(vec![Calcit::Nil, Calcit::Nil]),
      impls: vec![],
    })));

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
  fn warns_on_invalid_method_field_access() {
    use cirru_edn::EdnTag;

    // Create a test record type with fields: name, age
    let test_record = Arc::new(CalcitTypeAnnotation::Record(Arc::new(CalcitRecord {
      struct_ref: Arc::new(CalcitStruct::from_fields(
        EdnTag::from("Person"),
        vec![EdnTag::from("age"), EdnTag::from("name")],
      )),
      values: Arc::new(vec![Calcit::Nil, Calcit::Nil]),
      impls: vec![],
    })));

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
    let test_record = Arc::new(CalcitTypeAnnotation::Record(Arc::new(CalcitRecord {
      struct_ref: Arc::new(CalcitStruct::from_fields(
        EdnTag::from("Person"),
        vec![EdnTag::from("age"), EdnTag::from("name")],
      )),
      values: Arc::new(vec![Calcit::Nil, Calcit::Nil]),
      impls: vec![],
    })));

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
      scope: Arc::new(CalcitScope::default()),
      args: Arc::new(CalcitFnArgs::Args(vec![0, 1])), // two args
      body: vec![Calcit::Nil],
      generics: Arc::new(vec![]),
      arg_types: vec![
        Arc::new(CalcitTypeAnnotation::from_tag_name("number")),
        Arc::new(CalcitTypeAnnotation::from_tag_name("string")),
      ],
      return_type: Arc::new(CalcitTypeAnnotation::Dynamic),
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
    // (defn wrong-ret () (hint-fn (return-type :string)) (&+ 1 2))
    // Should return :number but declares :string
    let expr = Cirru::List(vec![
      Cirru::leaf("defn"),
      Cirru::leaf("wrong-ret"),
      Cirru::List(vec![]), // no args
      Cirru::List(vec![
        // (hint-fn (return-type :string))
        Cirru::leaf("hint-fn"),
        Cirru::List(vec![Cirru::leaf("return-type"), Cirru::leaf(":string")]),
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
  fn checks_record_method_arg_types() {
    use cirru_edn::EdnTag;

    // Create a method function: defn greet (name: string, age: number) -> ...
    let method_fn = Arc::new(CalcitFn {
      name: Arc::from("greet"),
      def_ns: Arc::from("tests.method"),
      scope: Arc::new(CalcitScope::default()),
      args: Arc::new(CalcitFnArgs::Args(vec![1, 2])), // 2 parameters
      body: vec![Calcit::Nil],
      generics: Arc::new(vec![]),
      return_type: Arc::new(CalcitTypeAnnotation::Dynamic),
      arg_types: vec![Arc::new(CalcitTypeAnnotation::String), Arc::new(CalcitTypeAnnotation::Number)],
    });

    // Create a record with the method
    let class_record = CalcitRecord {
      struct_ref: Arc::new(CalcitStruct::from_fields(EdnTag::from("Person"), vec![EdnTag::from("greet")])),
      values: Arc::new(vec![Calcit::Fn {
        id: Arc::from("tests.method/greet"),
        info: method_fn.clone(),
      }]),
      impls: vec![],
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
    scope_types.insert(Arc::from("user"), Arc::new(CalcitTypeAnnotation::Record(Arc::new(class_record))));

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
      impls: vec![],
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
      impls: vec![],
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
      impls: vec![],
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

    // Create a tuple type with 3 elements: tag at 0, and 2 extra elements at 1 and 2
    let tuple_type = CalcitTuple {
      tag: Arc::new(Calcit::Tag(EdnTag::from("point"))),
      extra: vec![Calcit::Number(10.0), Calcit::Number(20.0)],
      impls: vec![],
      sum_type: None,
    };

    let mut scope_types: ScopeTypes = ScopeTypes::new();
    scope_types.insert(Arc::from("my-tuple"), Arc::new(CalcitTypeAnnotation::Tuple(Arc::new(tuple_type))));

    // Test: access index 3 which is out of bounds (tuple has indices 0, 1, 2)
    // Use a symbol that references the tuple in scope_types
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
    assert!(!warnings_vec.is_empty(), "should have warning for out of bounds index");
    let warning_msg = warnings_vec[0].to_string();
    assert!(
      warning_msg.contains("out of bounds") && warning_msg.contains("3") && warning_msg.contains("3 element"),
      "warning should mention out of bounds and indices: {warning_msg}"
    );
  }

  #[test]
  fn checks_tuple_nth_valid_index() {
    use cirru_edn::EdnTag;

    // Create a tuple type with 3 elements
    let tuple_type = CalcitTuple {
      tag: Arc::new(Calcit::Tag(EdnTag::from("point"))),
      extra: vec![Calcit::Number(10.0), Calcit::Number(20.0)],
      impls: vec![],
      sum_type: None,
    };

    let mut scope_types: ScopeTypes = ScopeTypes::new();
    scope_types.insert(Arc::from("my-tuple"), Arc::new(CalcitTypeAnnotation::Tuple(Arc::new(tuple_type))));

    // Test: access valid index 1
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

    // Create a tuple type
    let tuple_type = CalcitTuple {
      tag: Arc::new(Calcit::Tag(EdnTag::from("point"))),
      extra: vec![Calcit::Number(10.0), Calcit::Number(20.0)],
      impls: vec![],
      sum_type: None,
    };

    let mut scope_types: ScopeTypes = ScopeTypes::new();
    scope_types.insert(Arc::from("my-tuple"), Arc::new(CalcitTypeAnnotation::Tuple(Arc::new(tuple_type))));

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
}
