use crate::{
  builtins::{is_js_syntax_procs, is_proc_name, is_registered_proc},
  call_stack::{extend_call_stack, CalcitStack, CallStackList, StackKind},
  primes,
  primes::{
    Calcit, CalcitErr, CalcitItems, CalcitProc, CalcitScope, CalcitSyntax, CrListWrap, ImportRule, LocatedWarning, NodeLocation,
    RawCodeType, SymbolResolved::*, GENERATED_DEF,
  },
  program, runner,
};

use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::Arc;

use im_ternary_tree::TernaryTreeList;
use strum::ParseError;

/// only macro and func are cared about during preprocessing
/// only used in preprocess defs
fn pick_macro_fn(x: Calcit) -> Option<Calcit> {
  match &x {
    Calcit::Fn { .. } | Calcit::Macro { .. } => Some(x),
    _ => None,
  }
}

/// returns the resolved symbol,
/// if code related is not preprocessed, do it internally
pub fn preprocess_ns_def(
  raw_ns: Arc<str>,
  raw_def: Arc<str>,
  // pass original string representation, TODO codegen currently relies on this
  raw_sym: Arc<str>,
  import_rule: Option<Arc<ImportRule>>, // returns form and possible value
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &rpds::ListSync<CalcitStack>,
) -> Result<(Calcit, Option<Calcit>), CalcitErr> {
  let ns = &raw_ns;
  let def = &raw_def;
  let original_sym = &raw_sym;
  // println!("preprocessing def: {}/{}", ns, def);
  match program::lookup_evaled_def(ns, def) {
    Some(v) => {
      // println!("{}/{} has inited", ns, def);
      Ok((
        Calcit::Symbol {
          sym: original_sym.to_owned(),
          ns: ns.to_owned(),
          at_def: def.to_owned(),
          resolved: Some(Arc::new(ResolvedDef {
            ns: ns.to_owned(),
            def: def.to_owned(),
            rule: import_rule,
          })),
          location: None,
        },
        pick_macro_fn(v),
      ))
    }
    None => {
      // println!("init for... {}/{}", ns, def);
      match program::lookup_def_code(ns, def) {
        Some(code) => {
          // write a nil value first to prevent dead loop
          program::write_evaled_def(ns, def, Calcit::Nil).map_err(|e| CalcitErr::use_msg_stack(e, call_stack))?;

          let next_stack = extend_call_stack(
            call_stack,
            ns.to_owned(),
            def.to_owned(),
            StackKind::Fn,
            code.to_owned(),
            &TernaryTreeList::Empty,
          );

          let (resolved_code, _resolve_value) = preprocess_expr(&code, &HashSet::new(), ns.to_owned(), check_warnings, &next_stack)?;
          // println!("\n resolve code to run: {:?}", resolved_code);
          let v = if is_fn_or_macro(&resolved_code) {
            match runner::evaluate_expr(&resolved_code, &CalcitScope::default(), ns.to_owned(), &next_stack) {
              Ok(ret) => ret,
              Err(e) => return Err(e),
            }
          } else {
            Calcit::Thunk(Arc::new(resolved_code), None)
          };
          // println!("\nwriting value to: {}/{} {:?}", ns, def, v);
          program::write_evaled_def(ns, def, v.to_owned()).map_err(|e| CalcitErr::use_msg_stack(e, call_stack))?;

          Ok((
            Calcit::Symbol {
              sym: original_sym.to_owned(),
              ns: ns.to_owned(),
              at_def: def.to_owned(),
              resolved: Some(Arc::new(ResolvedDef {
                ns: ns.to_owned(),
                def: def.to_owned(),
                rule: Some(Arc::new(ImportRule::NsReferDef(ns.to_owned(), def.to_owned()))),
              })),
              location: None,
            },
            pick_macro_fn(v),
          ))
        }
        None if ns.starts_with('|') || ns.starts_with('"') => Ok((
          Calcit::Symbol {
            sym: original_sym.to_owned(),
            ns: ns.to_owned(),
            at_def: def.to_owned(),
            resolved: Some(Arc::new(ResolvedDef {
              ns: ns.to_owned(),
              def: def.to_owned(),
              rule: import_rule,
            })),
            location: None,
          },
          None,
        )),
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
    Calcit::List(xs) => match xs.get(0) {
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
  file_ns: Arc<str>,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<(Calcit, Option<Calcit>), CalcitErr> {
  // println!("preprocessing @{} {}", file_ns, expr);
  match expr {
    Calcit::Symbol {
      sym: def,
      ns: def_ns,
      at_def,
      location,
      ..
    } => match runner::parse_ns_def(def) {
      Some((ns_alias, def_part)) => {
        if &*ns_alias == "js" {
          Ok((Calcit::RawCode(RawCodeType::Js, def_part), None))
        } else if let Some(target_ns) = program::lookup_ns_target_in_import(def_ns.to_owned(), &ns_alias) {
          // TODO js syntax to handle in future
          preprocess_ns_def(target_ns, def_part, def.to_owned(), None, check_warnings, call_stack)
        } else if program::has_def_code(&ns_alias, &def_part) {
          // refer to namespace/def directly for some usages
          preprocess_ns_def(ns_alias.to_owned(), def_part, def.to_owned(), None, check_warnings, call_stack)
        } else {
          Err(CalcitErr::use_msg_stack(format!("unknown ns target: {def}"), call_stack))
        }
      }
      None => {
        let def_ref = &**def;
        if def_ref == "~" || def_ref == "~@" || def_ref == "&" || def_ref == "?" {
          Ok((
            Calcit::Symbol {
              sym: def.to_owned(),
              ns: def_ns.to_owned(),
              at_def: at_def.to_owned(),
              resolved: Some(Arc::new(ResolvedRaw)),
              location: location.to_owned(),
            },
            None,
          ))
        } else if scope_defs.contains(def) {
          Ok((
            Calcit::Symbol {
              sym: def.to_owned(),
              ns: def_ns.to_owned(),
              at_def: at_def.to_owned(),
              resolved: Some(Arc::new(ResolvedLocal)),
              location: location.to_owned(),
            },
            None,
          ))
        } else if CalcitSyntax::is_valid(def) {
          Ok((
            Calcit::Syntax(
              def
                .parse()
                .map_err(|e: ParseError| CalcitErr::use_msg_stack(def.to_string() + " " + &e.to_string(), call_stack))?,
              def_ns.to_owned(),
            ),
            None,
          ))
        } else if let Ok(p) = def.parse::<CalcitProc>() {
          Ok((Calcit::Proc(p), None))
        } else if program::has_def_code(primes::CORE_NS, def) {
          preprocess_ns_def(
            primes::CORE_NS.into(),
            def.to_owned(),
            def.to_owned(),
            None,
            check_warnings,
            call_stack,
          )
        } else if program::has_def_code(def_ns, def) {
          preprocess_ns_def(def_ns.to_owned(), def.to_owned(), def.to_owned(), None, check_warnings, call_stack)
        } else if is_registered_proc(def) {
          Ok((
            Calcit::Symbol {
              sym: def.to_owned(),
              ns: def_ns.to_owned(),
              at_def: at_def.to_owned(),
              resolved: Some(Arc::new(ResolvedRaw)),
              location: location.to_owned(),
            },
            None,
          ))
        } else {
          match program::lookup_def_target_in_import(def_ns, def) {
            Some(target_ns) => {
              // effect
              // TODO js syntax to handle in future
              preprocess_ns_def(target_ns, def.to_owned(), def.to_owned(), None, check_warnings, call_stack)
            }
            // TODO check js_mode
            None if is_js_syntax_procs(def) => Ok((expr.to_owned(), None)),
            None if def.starts_with('.') => Ok((expr.to_owned(), None)),
            None => {
              let from_default = program::lookup_default_target_in_import(def_ns, def);
              if let Some(target_ns) = from_default {
                let target = Some(Arc::new(ResolvedDef {
                  ns: target_ns.to_owned(),
                  def: def.to_owned(),
                  rule: Some(Arc::new(ImportRule::NsDefault(target_ns))),
                }));
                Ok((
                  Calcit::Symbol {
                    sym: def.to_owned(),
                    ns: def_ns.to_owned(),
                    at_def: at_def.to_owned(),
                    resolved: target,
                    location: location.to_owned(),
                  },
                  None,
                ))
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
                Ok((expr.to_owned(), None))
              }
            }
          }
        }
      }
    },
    Calcit::List(xs) => {
      if xs.is_empty() {
        Ok((expr.to_owned(), None))
      } else {
        // TODO whether function bothers this...
        // println!("start calling: {}", expr);
        process_list_call(xs, scope_defs, file_ns, check_warnings, call_stack)
      }
    }
    Calcit::Number(..) | Calcit::Str(..) | Calcit::Nil | Calcit::Bool(..) | Calcit::Tag(..) | Calcit::CirruQuote(..) => {
      Ok((expr.to_owned(), None))
    }
    Calcit::Method(..) => Ok((expr.to_owned(), None)),
    Calcit::Proc(..) => Ok((expr.to_owned(), None)),
    Calcit::Syntax(..) => Ok((expr.to_owned(), None)),
    _ => {
      let mut warnings = check_warnings.borrow_mut();
      let loc = NodeLocation {
        ns: file_ns,
        def: GENERATED_DEF.into(),
        coord: vec![],
      };
      warnings.push(LocatedWarning::new(
        format!("[Warn] unexpected data during preprocess: {expr:?}"),
        loc,
      ));
      Ok((expr.to_owned(), None))
    }
  }
}

fn process_list_call(
  xs: &CalcitItems,
  scope_defs: &HashSet<Arc<str>>,
  file_ns: Arc<str>,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<(Calcit, Option<Calcit>), CalcitErr> {
  let head = &xs[0];
  let (head_form, head_evaled) = preprocess_expr(head, scope_defs, file_ns.to_owned(), check_warnings, call_stack)?;
  let args = xs.drop_left();
  let def_name = grab_def_name(head);

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

  match (&head_form, &head_evaled) {
    (Calcit::Tag(..), _) => {
      if args.len() == 1 {
        let code = Calcit::List(TernaryTreeList::from(&[
          Calcit::Symbol {
            sym: String::from("get").into(),
            ns: String::from(primes::CORE_NS).into(),
            at_def: String::from(primes::GENERATED_DEF).into(),
            resolved: Some(Arc::new(ResolvedDef {
              ns: String::from(primes::CORE_NS).into(),
              def: String::from("get").into(),
              rule: None,
            })),
            location: None,
          },
          args[0].to_owned(),
          head.to_owned(),
        ]));
        preprocess_expr(&code, scope_defs, file_ns, check_warnings, call_stack)
      } else {
        Err(CalcitErr::use_msg_stack(format!("{head} expected 1 hashmap to call"), call_stack))
      }
    }
    (
      _,
      Some(Calcit::Macro {
        name,
        def_ns,
        args: def_args,
        body,
        ..
      }),
    ) => {
      let mut current_values = Box::new(args.to_owned());

      // println!("eval macro: {}", primes::CrListWrap(xs.to_owned()));
      // println!("macro... {} {}", x, CrListWrap(current_values.to_owned()));

      let code = Calcit::List(xs.to_owned());
      let next_stack = extend_call_stack(call_stack, def_ns.to_owned(), name.to_owned(), StackKind::Macro, code, &args);

      loop {
        // need to handle recursion
        // println!("evaling line: {:?}", body);
        let body_scope = runner::bind_args(def_args, &current_values, &CalcitScope::default(), &next_stack)?;
        let code = runner::evaluate_lines(body, &body_scope, file_ns.to_owned(), &next_stack)?;
        match code {
          Calcit::Recur(ys) => {
            current_values = Box::new(ys.to_owned());
          }
          _ => {
            // println!("gen code: {} {}", code, &code.lisp_str());
            return preprocess_expr(&code, scope_defs, file_ns, check_warnings, &next_stack);
          }
        }
      }
    }
    (Calcit::Syntax(name, name_ns), _) => match name {
      CalcitSyntax::Quasiquote => Ok((
        preprocess_quasiquote(name, name_ns.to_owned(), &args, scope_defs, file_ns, check_warnings, call_stack)?,
        None,
      )),
      CalcitSyntax::Defn | CalcitSyntax::Defmacro => Ok((
        preprocess_defn(name, name_ns.to_owned(), &args, scope_defs, file_ns, check_warnings, call_stack)?,
        None,
      )),
      CalcitSyntax::CoreLet => Ok((
        preprocess_core_let(name, name_ns.to_owned(), &args, scope_defs, file_ns, check_warnings, call_stack)?,
        None,
      )),
      CalcitSyntax::If
      | CalcitSyntax::Try
      | CalcitSyntax::Macroexpand
      | CalcitSyntax::MacroexpandAll
      | CalcitSyntax::Macroexpand1
      | CalcitSyntax::Gensym
      | CalcitSyntax::Reset => Ok((
        preprocess_each_items(name, name_ns.to_owned(), &args, scope_defs, file_ns, check_warnings, call_stack)?,
        None,
      )),
      CalcitSyntax::Quote | CalcitSyntax::Eval | CalcitSyntax::HintFn => {
        Ok((preprocess_quote(name, name_ns.to_owned(), &args, scope_defs, file_ns)?, None))
      }
      CalcitSyntax::Defatom => Ok((
        preprocess_defatom(name, name_ns.to_owned(), &args, scope_defs, file_ns, check_warnings, call_stack)?,
        None,
      )),
    },
    (Calcit::Thunk(..), _) => Err(CalcitErr::use_msg_stack(
      format!("does not know how to preprocess a thunk: {head}"),
      call_stack,
    )),

    (
      _,
      Some(Calcit::Fn {
        name: f_name,
        args: f_args,
        ..
      }),
    ) => {
      check_fn_args(f_args, &args, file_ns.to_owned(), f_name.to_owned(), def_name, check_warnings);
      let mut ys = Vec::with_capacity(args.len() + 1);
      ys.push(head_form);
      for a in &args {
        let (form, _v) = preprocess_expr(a, scope_defs, file_ns.to_owned(), check_warnings, call_stack)?;
        ys.push(form);
      }
      Ok((Calcit::List(TernaryTreeList::from(&ys)), None))
    }
    (Calcit::Method(_, _), _) => {
      let mut ys = Vec::with_capacity(args.len());
      ys.push(head.to_owned());
      for a in &args {
        let (form, _v) = preprocess_expr(a, scope_defs, file_ns.to_owned(), check_warnings, call_stack)?;
        ys.push(form);
      }
      Ok((Calcit::List(TernaryTreeList::from(&ys)), None))
    }
    (h, he) => {
      if let Calcit::Symbol { sym, resolved, .. } = h {
        if he.is_none() && resolved.is_none() && !is_js_syntax_procs(sym) {
          println!("warning: unresolved symbol `{}` in `{}`", sym, CrListWrap(xs.to_owned()));
        }
      }
      let mut ys = Vec::with_capacity(args.len() + 1);
      ys.push(head_form);
      for a in &args {
        let (form, _v) = preprocess_expr(a, scope_defs, file_ns.to_owned(), check_warnings, call_stack)?;
        ys.push(form);
      }
      Ok((Calcit::List(TernaryTreeList::from(&ys)), None))
    }
  }
}

// detects arguments of top-level functions when possible
fn check_fn_args(
  defined_args: &[Arc<str>],
  params: &CalcitItems,
  file_ns: Arc<str>,
  f_name: Arc<str>,
  def_name: Arc<str>,
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
      (Some(sym), _) if &**sym == "&" => {
        // dynamic args rule, all okay
        return;
      }
      (Some(sym), _) if &**sym == "?" => {
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
          let loc = NodeLocation::new(file_ns.to_owned(), GENERATED_DEF.into(), vec![]);
          warnings.push(LocatedWarning::new(
            format!(
              "[Warn] lack of args in {} `{:?}` with `{}`, at {}/{}",
              f_name,
              defined_args,
              primes::CrListWrap(params.to_owned()),
              file_ns,
              def_name
            ),
            loc,
          ));
          return;
        }
      }
      (None, Some(_)) => {
        let mut warnings = check_warnings.borrow_mut();
        let loc = NodeLocation::new(file_ns.to_owned(), GENERATED_DEF.into(), vec![]);
        warnings.push(LocatedWarning::new(
          format!(
            "[Warn] too many args for {} `{:?}` with `{}`, at {}/{}",
            f_name,
            defined_args,
            primes::CrListWrap(params.to_owned()),
            file_ns,
            def_name
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
    Calcit::Symbol { at_def: def_name, .. } => def_name.to_owned(),
    _ => String::from("??").into(),
  }
}

// tradition rule for processing exprs
pub fn preprocess_each_items(
  head: &CalcitSyntax,
  head_ns: Arc<str>,
  args: &CalcitItems,
  scope_defs: &HashSet<Arc<str>>,
  file_ns: Arc<str>,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  let mut xs: CalcitItems = TernaryTreeList::from(&[Calcit::Syntax(head.to_owned(), head_ns)]);
  for a in args {
    let (form, _v) = preprocess_expr(a, scope_defs, file_ns.to_owned(), check_warnings, call_stack)?;
    xs = xs.push_right(form);
  }
  Ok(Calcit::List(xs))
}

pub fn preprocess_defn(
  head: &CalcitSyntax,
  head_ns: Arc<str>,
  args: &CalcitItems,
  scope_defs: &HashSet<Arc<str>>,
  file_ns: Arc<str>,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  // println!("defn args: {}", primes::CrListWrap(args.to_owned()));
  let mut xs: CalcitItems = TernaryTreeList::from(&[Calcit::Syntax(head.to_owned(), head_ns)]);
  match (args.get(0), args.get(1)) {
    (
      Some(Calcit::Symbol {
        sym: def_name,
        ns: def_name_ns,
        at_def,
        location,
        ..
      }),
      Some(Calcit::List(ys)),
    ) => {
      let mut body_defs: HashSet<Arc<str>> = scope_defs.to_owned();

      xs = xs.push_right(Calcit::Symbol {
        sym: def_name.to_owned(),
        ns: def_name_ns.to_owned(),
        at_def: at_def.to_owned(),
        resolved: Some(Arc::new(ResolvedRaw)),
        location: location.to_owned(),
      });
      let mut zs: CalcitItems = TernaryTreeList::Empty;
      for y in ys {
        match y {
          Calcit::Symbol {
            sym,
            ns: def_ns,
            at_def,
            location: arg_location,
            ..
          } => {
            let loc = NodeLocation::new(def_ns.to_owned(), at_def.to_owned(), arg_location.to_owned().unwrap_or_default());
            check_symbol(sym, args, loc, check_warnings);
            zs = zs.push_right(Calcit::Symbol {
              sym: sym.to_owned(),
              ns: def_ns.to_owned(),
              at_def: at_def.to_owned(),
              resolved: Some(Arc::new(ResolvedRaw)),
              location: arg_location.to_owned(),
            });
            // skip argument syntax marks
            if &**sym != "&" && &**sym != "?" {
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
      xs = xs.push_right(Calcit::List(zs));

      for a in args.into_iter().skip(2) {
        let (form, _v) = preprocess_expr(a, &body_defs, file_ns.to_owned(), check_warnings, call_stack)?;
        xs = xs.push_right(form);
      }
      Ok(Calcit::List(xs))
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
fn check_symbol(sym: &str, args: &CalcitItems, location: NodeLocation, check_warnings: &RefCell<Vec<LocatedWarning>>) {
  if is_proc_name(sym) || CalcitSyntax::is_valid(sym) || program::has_def_code(primes::CORE_NS, sym) {
    let mut warnings = check_warnings.borrow_mut();
    warnings.push(LocatedWarning::new(
      format!(
        "[Warn] local binding `{}` shadowed `calcit.core/{}`, with {}",
        sym,
        sym,
        primes::CrListWrap(args.to_owned())
      ),
      location,
    ));
  }
}

pub fn preprocess_core_let(
  head: &CalcitSyntax,
  // where the symbol was defined
  head_ns: Arc<str>,
  args: &CalcitItems,
  scope_defs: &HashSet<Arc<str>>,
  // where called
  file_ns: Arc<str>,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  let mut xs: CalcitItems = TernaryTreeList::from(&[Calcit::Syntax(head.to_owned(), head_ns.to_owned())]);
  let mut body_defs: HashSet<Arc<str>> = scope_defs.to_owned();
  let binding = match args.get(0) {
    Some(Calcit::List(ys)) if ys.is_empty() => Calcit::List(TernaryTreeList::Empty),
    Some(Calcit::List(ys)) if ys.len() == 2 => match (&ys[0], &ys[1]) {
      (Calcit::Symbol { sym, .. }, a) => {
        let loc = NodeLocation {
          ns: head_ns,
          def: GENERATED_DEF.into(),
          coord: vec![],
        };
        check_symbol(sym, args, loc, check_warnings);
        body_defs.insert(sym.to_owned());
        let (form, _v) = preprocess_expr(a, &body_defs, file_ns.to_owned(), check_warnings, call_stack)?;
        Calcit::List(TernaryTreeList::from(&[ys[0].to_owned(), form]))
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
    let (form, _v) = preprocess_expr(a, &body_defs, file_ns.to_owned(), check_warnings, call_stack)?;
    xs = xs.push_right(form);
  }
  Ok(Calcit::List(xs))
}

pub fn preprocess_quote(
  head: &CalcitSyntax,
  head_ns: Arc<str>,
  args: &CalcitItems,
  _scope_defs: &HashSet<Arc<str>>,
  _file_ns: Arc<str>,
) -> Result<Calcit, CalcitErr> {
  let mut xs: CalcitItems = TernaryTreeList::from(&[Calcit::Syntax(head.to_owned(), head_ns)]);
  for a in args {
    xs = xs.push_right(a.to_owned());
  }
  Ok(Calcit::List(xs))
}

pub fn preprocess_defatom(
  head: &CalcitSyntax,
  head_ns: Arc<str>,
  args: &CalcitItems,
  scope_defs: &HashSet<Arc<str>>,
  file_ns: Arc<str>,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  let mut xs: CalcitItems = TernaryTreeList::from(&[Calcit::Syntax(head.to_owned(), head_ns)]);
  for a in args {
    // TODO
    let (form, _v) = preprocess_expr(a, scope_defs, file_ns.to_owned(), check_warnings, call_stack)?;
    xs = xs.push_right(form.to_owned());
  }
  Ok(Calcit::List(xs))
}

/// need to handle experssions inside unquote snippets
pub fn preprocess_quasiquote(
  head: &CalcitSyntax,
  head_ns: Arc<str>,
  args: &CalcitItems,
  scope_defs: &HashSet<Arc<str>>,
  file_ns: Arc<str>,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  let mut xs: CalcitItems = TernaryTreeList::from(&[Calcit::Syntax(head.to_owned(), head_ns)]);
  for a in args {
    xs = xs.push_right(preprocess_quasiquote_internal(
      a,
      scope_defs,
      file_ns.to_owned(),
      check_warnings,
      call_stack,
    )?);
  }
  Ok(Calcit::List(xs))
}

pub fn preprocess_quasiquote_internal(
  x: &Calcit,
  scope_defs: &HashSet<Arc<str>>,
  file_ns: Arc<str>,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  match x {
    Calcit::List(ys) if ys.is_empty() => Ok(x.to_owned()),
    Calcit::List(ys) => match &ys[0] {
      Calcit::Symbol { sym, .. } if &**sym == "~" || &**sym == "~@" => {
        let mut xs: CalcitItems = TernaryTreeList::Empty;
        for y in ys {
          let (form, _) = preprocess_expr(y, scope_defs, file_ns.to_owned(), check_warnings, call_stack)?;
          xs = xs.push_right(form.to_owned());
        }
        Ok(Calcit::List(xs))
      }
      _ => {
        let mut xs: CalcitItems = TernaryTreeList::Empty;
        for y in ys {
          xs = xs.push_right(preprocess_quasiquote_internal(y, scope_defs, file_ns.to_owned(), check_warnings, call_stack)?.to_owned());
        }
        Ok(Calcit::List(xs))
      }
    },
    _ => Ok(x.to_owned()),
  }
}
