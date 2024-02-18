use crate::{
  builtins::{is_js_syntax_procs, is_proc_name, is_registered_proc},
  calcit::{
    self, Calcit, CalcitArgLabel, CalcitErr, CalcitImport, CalcitList, CalcitProc, CalcitScope, CalcitSymbolInfo, CalcitSyntax,
    CalcitThunk, CalcitThunkInfo, ImportInfo, LocatedWarning, NodeLocation, RawCodeType, GENERATED_DEF,
  },
  call_stack::{CallStackList, StackKind},
  program, runner,
};

use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::Arc;

use im_ternary_tree::TernaryTreeList;
use strum::ParseError;

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
          program::write_evaled_def(ns, def, Calcit::Nil).map_err(|e| CalcitErr::use_msg_stack(e, call_stack))?;

          let next_stack = call_stack.extend(ns, def, StackKind::Fn, &code, &TernaryTreeList::Empty);

          let resolved_code = preprocess_expr(&code, &HashSet::new(), ns, check_warnings, &next_stack)?;
          // println!("\n resolve code to run: {:?}", resolved_code);
          let v = if is_fn_or_macro(&resolved_code) {
            match runner::evaluate_expr(&resolved_code, &CalcitScope::default(), ns, &next_stack) {
              Ok(ret) => ret,
              Err(e) => return Err(e),
            }
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
          program::write_evaled_def(ns, def, v.to_owned()).map_err(|e| CalcitErr::use_msg_stack(e, call_stack))?;

          Ok(Some(v))
        }
        None if ns.starts_with('|') || ns.starts_with('"') => Ok(None),
        None => Err(CalcitErr::use_msg_stack(
          format!("unknown ns/def in program: {ns}/{def}"),
          call_stack,
        )),
      }
    }
  }
}

fn is_fn_or_macro(code: &Calcit) -> bool {
  match code {
    Calcit::List(xs) => match xs.get_inner(0) {
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
          Err(CalcitErr::use_msg_stack(format!("unknown ns target: {def}"), call_stack))
        }
      }
      None => {
        let def_ns = &info.at_ns;
        let at_def = &info.at_def;
        let def_ref = &**def;
        // println!("def {} - {} {} {}", def_ref, def_ns, file_ns, at_def);
        if def_ref == "~" || def_ref == "~@" || def_ref == "&" || def_ref == "?" {
          Ok(Calcit::Symbol {
            sym: def.to_owned(),
            info: Arc::new(CalcitSymbolInfo {
              at_ns: def_ns.to_owned(),
              at_def: at_def.to_owned(),
            }),
            location: location.to_owned(),
          })
        } else if scope_defs.contains(def) {
          Ok(Calcit::Local {
            sym: def.to_owned(),
            info: Arc::new(CalcitSymbolInfo {
              at_ns: def_ns.to_owned(),
              at_def: at_def.to_owned(),
            }),
            location: location.to_owned(),
          })
        } else if CalcitSyntax::is_valid(def) {
          Ok(Calcit::Syntax(
            def
              .parse()
              .map_err(|e: ParseError| CalcitErr::use_msg_stack(def.to_string() + " " + &e.to_string(), call_stack))?,
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
            // TODO check js_mode
            None if is_js_syntax_procs(def) => Ok(expr.to_owned()),
            None if def.starts_with('.') => Ok(expr.to_owned()),
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
        process_list_call(xs, scope_defs, file_ns, check_warnings, call_stack)
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
      println!("unknown expr: {}", expr);
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

fn process_list_call(
  xs: &CalcitList,
  scope_defs: &HashSet<Arc<str>>,
  file_ns: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  let head = &xs[0];
  let head_form = preprocess_expr(head, scope_defs, file_ns, check_warnings, call_stack)?;
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
      let mut current_values: TernaryTreeList<Calcit> = (&args).into();

      // println!("eval macro: {}", primes::CrListWrap(xs.to_owned()));
      // println!("macro... {} {}", x, CrListWrap(current_values.to_owned()));

      let code = Calcit::List(Arc::new(xs.to_owned()));
      let next_stack = call_stack.extend(&info.def_ns, &info.name, StackKind::Macro, &code, &args.0);

      let mut body_scope = CalcitScope::default();

      loop {
        // need to handle recursion
        // println!("evaling line: {:?}", body);
        runner::bind_args(&mut body_scope, &info.args, &current_values, &next_stack)?;
        let code = runner::evaluate_lines(&info.body, &body_scope, file_ns, &next_stack)?;
        match code {
          Calcit::Recur(ys) => {
            current_values = (*ys).to_owned();
          }
          _ => {
            // println!("gen code: {} {}", code, &code.lisp_str());
            return preprocess_expr(&code, scope_defs, file_ns, check_warnings, &next_stack);
          }
        }
      }
    }

    Some(Calcit::Fn { info, .. }) => {
      check_fn_args(&info.args, &args, file_ns, &info.name, &def_name, check_warnings);
      let mut ys = CalcitList::new_inner_from(&[head_form.to_owned()]);
      for a in &args {
        let form = preprocess_expr(a, scope_defs, file_ns, check_warnings, call_stack)?;
        ys = ys.push(form);
      }
      Ok(Calcit::from(CalcitList(ys)))
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
          preprocess_expr(&code, scope_defs, file_ns, check_warnings, call_stack)
        } else {
          Err(CalcitErr::use_msg_stack(format!("{head} expected 1 hashmap to call"), call_stack))
        }
      }

      Calcit::Syntax(name, name_ns) => match name {
        CalcitSyntax::Quasiquote => Ok(preprocess_quasiquote(
          name,
          name_ns,
          &args,
          scope_defs,
          file_ns,
          check_warnings,
          call_stack,
        )?),
        CalcitSyntax::Defn | CalcitSyntax::Defmacro => Ok(preprocess_defn(
          name,
          name_ns,
          &args,
          scope_defs,
          file_ns,
          check_warnings,
          call_stack,
        )?),
        CalcitSyntax::CoreLet => Ok(preprocess_core_let(
          name,
          name_ns,
          &args,
          scope_defs,
          file_ns,
          check_warnings,
          call_stack,
        )?),
        CalcitSyntax::If
        | CalcitSyntax::Try
        | CalcitSyntax::Macroexpand
        | CalcitSyntax::MacroexpandAll
        | CalcitSyntax::Macroexpand1
        | CalcitSyntax::Gensym
        | CalcitSyntax::Reset => Ok(preprocess_each_items(
          name,
          name_ns,
          &args,
          scope_defs,
          file_ns,
          check_warnings,
          call_stack,
        )?),
        CalcitSyntax::Quote | CalcitSyntax::Eval | CalcitSyntax::HintFn => {
          Ok(preprocess_quote(name, name_ns, &args, scope_defs, file_ns)?)
        }
        CalcitSyntax::Defatom => Ok(preprocess_defatom(
          name,
          name_ns,
          &args,
          scope_defs,
          file_ns,
          check_warnings,
          call_stack,
        )?),
      },
      Calcit::Thunk(..) => Err(CalcitErr::use_msg_stack(
        format!("does not know how to preprocess a thunk: {head}"),
        call_stack,
      )),

      Calcit::Method(_, _) => {
        let mut ys = CalcitList::new_inner_from(&[head.to_owned()]);
        for a in &args {
          let form = preprocess_expr(a, scope_defs, file_ns, check_warnings, call_stack)?;
          ys = ys.push(form);
        }
        Ok(Calcit::from(CalcitList(ys)))
      }
      Calcit::Proc(..)
      | Calcit::Local { .. }
      | Calcit::Import { .. }
      | Calcit::Registered { .. }
      | Calcit::List(..)
      | Calcit::RawCode(..)
      | Calcit::Symbol { .. } => {
        let mut ys = CalcitList::new_inner_from(&[head_form]);
        for a in &args {
          let form = preprocess_expr(a, scope_defs, file_ns, check_warnings, call_stack)?;
          ys = ys.push(form);
        }
        Ok(Calcit::from(CalcitList(ys)))
      }
      h => {
        unreachable!("unknown head: {:?}", h);
      }
    },
  }
}

/// detects arguments of top-level functions when possible
fn check_fn_args(
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
    let r = params.get_inner(j);

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
          let mut warnings = check_warnings.borrow_mut();
          let loc = NodeLocation::new(Arc::from(file_ns), Arc::from(GENERATED_DEF), Arc::from(vec![]));
          warnings.push(LocatedWarning::new(
            format!(
              "[Warn] lack of args in {} `{:?}` with `{}`, at {}/{}",
              f_name, defined_args, params, file_ns, def_name
            ),
            loc,
          ));
          return;
        }
      }
      (None, Some(_)) => {
        let mut warnings = check_warnings.borrow_mut();
        let loc = NodeLocation::new(Arc::from(file_ns), Arc::from(GENERATED_DEF), Arc::from(vec![]));
        warnings.push(LocatedWarning::new(
          format!(
            "[Warn] too many args for {} `{:?}` with `{}`, at {}/{}",
            f_name, defined_args, params, file_ns, def_name
          ),
          loc,
        ));
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

// TODO this native implementation only handles symbols
fn grab_def_name(x: &Calcit) -> Arc<str> {
  match x {
    Calcit::Symbol { info, .. } => info.at_def.to_owned(),
    _ => String::from("??").into(),
  }
}

// tradition rule for processing exprs
pub fn preprocess_each_items(
  head: &CalcitSyntax,
  head_ns: &str,
  args: &CalcitList,
  scope_defs: &HashSet<Arc<str>>,
  file_ns: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  let mut xs: TernaryTreeList<Calcit> = TernaryTreeList::from(&[Calcit::Syntax(head.to_owned(), Arc::from(head_ns))]);
  for a in args {
    let form = preprocess_expr(a, scope_defs, file_ns, check_warnings, call_stack)?;
    xs = xs.push_right(form);
  }
  Ok(Calcit::List(Arc::new(xs.into())))
}

pub fn preprocess_defn(
  head: &CalcitSyntax,
  head_ns: &str,
  args: &CalcitList,
  scope_defs: &HashSet<Arc<str>>,
  file_ns: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  // println!("defn args: {}", primes::CrListWrap(args.to_owned()));
  let mut xs: TernaryTreeList<Calcit> = TernaryTreeList::from(&[Calcit::Syntax(head.to_owned(), Arc::from(head_ns))]);
  match (args.get_inner(0), args.get_inner(1)) {
    (
      Some(Calcit::Symbol {
        sym: def_name,
        info,
        location,
        ..
      }),
      Some(Calcit::List(ys)),
    ) => {
      let mut body_defs: HashSet<Arc<str>> = scope_defs.to_owned();

      xs = xs.push_right(Calcit::Symbol {
        sym: def_name.to_owned(),
        info: Arc::new(CalcitSymbolInfo {
          at_ns: info.at_ns.to_owned(),
          at_def: info.at_def.to_owned(),
        }),
        location: location.to_owned(),
      });
      let mut zs = CalcitList::new_inner();
      for y in &**ys {
        match y {
          s @ Calcit::Symbol {
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
            check_symbol(sym, args, loc, check_warnings);
            if &**sym == "&" || &**sym == "?" {
              zs = zs.push_right(s.to_owned());
              continue;
            } else {
              let s = Calcit::Local {
                sym: sym.to_owned(),
                info: Arc::new(CalcitSymbolInfo {
                  at_ns: info.at_ns.to_owned(),
                  at_def: info.at_def.to_owned(),
                }),
                location: arg_location.to_owned(),
              };
              // println!("created local: {:?}", s);
              zs = zs.push_right(s);

              // track local in scope
              body_defs.insert(sym.to_owned());
            }
          }
          _ => {
            return Err(CalcitErr::use_msg_stack(
              format!("expected defn args to be symbols, got: {y}"),
              call_stack,
            ))
          }
        }
      }
      xs = xs.push_right(Calcit::List(Arc::new(zs.into())));

      for a in args.into_iter().skip(2) {
        let form = preprocess_expr(a, &body_defs, file_ns, check_warnings, call_stack)?;
        xs = xs.push_right(form);
      }
      Ok(Calcit::List(Arc::new(xs.into())))
    }
    (Some(a), Some(b)) => Err(CalcitErr::use_msg_stack_location(
      format!("defn/defmacro expected name and args: {a} {b}"),
      call_stack,
      a.get_location().or_else(|| b.get_location()),
    )),
    (a, b) => Err(CalcitErr::use_msg_stack(
      format!("defn or defmacro expected name and args, got: {a:?} {b:?}",),
      call_stack,
    )),
  }
}

// warn if this symbol is used
fn check_symbol(sym: &str, args: &CalcitList, location: NodeLocation, check_warnings: &RefCell<Vec<LocatedWarning>>) {
  if is_proc_name(sym) || CalcitSyntax::is_valid(sym) || program::has_def_code(calcit::CORE_NS, sym) {
    let mut warnings = check_warnings.borrow_mut();
    warnings.push(LocatedWarning::new(
      format!("[Warn] local binding `{}` shadowed `calcit.core/{}`, with {}", sym, sym, args),
      location,
    ));
  }
}

pub fn preprocess_core_let(
  head: &CalcitSyntax,
  // where the symbol was defined
  head_ns: &str,
  args: &CalcitList,
  scope_defs: &HashSet<Arc<str>>,
  // where called
  file_ns: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  let mut xs: TernaryTreeList<Calcit> = TernaryTreeList::from(&[Calcit::Syntax(head.to_owned(), Arc::from(head_ns))]);
  let mut body_defs: HashSet<Arc<str>> = scope_defs.to_owned();
  let binding = match args.get_inner(0) {
    Some(Calcit::List(ys)) if ys.is_empty() => Calcit::from(CalcitList::default()),
    Some(Calcit::List(ys)) if ys.len() == 2 => match (&ys[0], &ys[1]) {
      (Calcit::Symbol { sym, info, location }, a) => {
        let loc = NodeLocation {
          ns: Arc::from(head_ns),
          def: GENERATED_DEF.into(),
          coord: Arc::from(vec![]),
        };
        check_symbol(sym, args, loc, check_warnings);
        body_defs.insert(sym.to_owned());
        let form = preprocess_expr(a, &body_defs, file_ns, check_warnings, call_stack)?;
        let name = Calcit::Local {
          sym: sym.to_owned(),
          info: Arc::new(CalcitSymbolInfo {
            at_ns: info.at_ns.to_owned(),
            at_def: info.at_def.to_owned(),
          }),
          location: location.to_owned(),
        };
        Calcit::from(CalcitList::from(&[name, form]))
      }
      (a, b) => {
        return Err(CalcitErr::use_msg_stack_location(
          format!("invalid pair for &let binding: {a} {b}"),
          call_stack,
          a.get_location().or_else(|| b.get_location()),
        ))
      }
    },
    Some(a @ Calcit::List(_)) => {
      return Err(CalcitErr::use_msg_stack(
        format!("expected binding of a pair, got: {a}"),
        call_stack,
      ))
    }
    Some(a) => {
      return Err(CalcitErr::use_msg_stack_location(
        format!("expected binding of a pair, got: {a}"),
        call_stack,
        a.get_location(),
      ))
    }
    None => {
      return Err(CalcitErr::use_msg_stack(
        "expected binding of a pair, got nothing".to_owned(),
        call_stack,
      ))
    }
  };
  xs = xs.push_right(binding);
  for a in args.into_iter().skip(1) {
    let form = preprocess_expr(a, &body_defs, file_ns, check_warnings, call_stack)?;
    xs = xs.push_right(form);
  }
  Ok(Calcit::List(Arc::from(CalcitList(xs))))
}

pub fn preprocess_quote(
  head: &CalcitSyntax,
  head_ns: &str,
  args: &CalcitList,
  _scope_defs: &HashSet<Arc<str>>,
  _file_ns: &str,
) -> Result<Calcit, CalcitErr> {
  let mut xs: TernaryTreeList<Calcit> = TernaryTreeList::from(&[Calcit::Syntax(head.to_owned(), Arc::from(head_ns))]);
  for a in args {
    xs = xs.push_right(a.to_owned());
  }
  Ok(Calcit::List(Arc::new(xs.into())))
}

pub fn preprocess_defatom(
  head: &CalcitSyntax,
  head_ns: &str,
  args: &CalcitList,
  scope_defs: &HashSet<Arc<str>>,
  file_ns: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  let mut xs: TernaryTreeList<Calcit> = TernaryTreeList::from(&[Calcit::Syntax(head.to_owned(), Arc::from(head_ns))]);
  for a in args {
    // TODO
    let form = preprocess_expr(a, scope_defs, file_ns, check_warnings, call_stack)?;
    xs = xs.push_right(form.to_owned());
  }
  Ok(Calcit::List(Arc::new(CalcitList(xs))))
}

/// need to handle experssions inside unquote snippets
pub fn preprocess_quasiquote(
  head: &CalcitSyntax,
  head_ns: &str,
  args: &CalcitList,
  scope_defs: &HashSet<Arc<str>>,
  file_ns: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  let mut xs: TernaryTreeList<Calcit> = TernaryTreeList::from(&[Calcit::Syntax(head.to_owned(), Arc::from(head_ns))]);
  for a in args {
    xs = xs.push_right(preprocess_quasiquote_internal(a, scope_defs, file_ns, check_warnings, call_stack)?);
  }
  Ok(Calcit::List(Arc::new(xs.into())))
}

pub fn preprocess_quasiquote_internal(
  x: &Calcit,
  scope_defs: &HashSet<Arc<str>>,
  file_ns: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  match x {
    Calcit::List(ys) if ys.is_empty() => Ok(x.to_owned()),
    Calcit::List(ys) => match &ys.0[0] {
      Calcit::Symbol { sym, .. } if &**sym == "~" || &**sym == "~@" => {
        let mut xs = CalcitList::new_inner();
        for y in &ys.0 {
          let form = preprocess_expr(y, scope_defs, file_ns, check_warnings, call_stack)?;
          xs = xs.push_right(form.to_owned());
        }
        Ok(Calcit::List(Arc::new(xs.into())))
      }
      _ => {
        let mut xs = CalcitList::new_inner();
        for y in &ys.0 {
          xs = xs.push_right(preprocess_quasiquote_internal(y, scope_defs, file_ns, check_warnings, call_stack)?.to_owned());
        }
        Ok(Calcit::List(Arc::new(xs.into())))
      }
    },
    _ => Ok(x.to_owned()),
  }
}
