use crate::builtins::{is_js_syntax_procs, is_proc_name};
use crate::call_stack::{pop_call_stack, push_call_stack, StackKind};
use crate::primes;
use crate::primes::{Calcit, CalcitErr, CalcitItems, CalcitSyntax, ImportRule, SymbolResolved::*};
use crate::program;
use crate::runner;

use std::cell::RefCell;
use std::collections::HashSet;

/// returns the resolved symbol,
/// if code related is not preprocessed, do it internally
pub fn preprocess_ns_def(
  ns: &str,
  def: &str,
  // pass original string representation, TODO codegen currently relies on this
  original_sym: &str,
  import_rule: Option<ImportRule>, // returns form and possible value
  check_warnings: &RefCell<Vec<String>>,
) -> Result<(Calcit, Option<Calcit>), CalcitErr> {
  // println!("preprocessing def: {}/{}", ns, def);
  match program::lookup_evaled_def(ns, def) {
    Some(v) => {
      // println!("{}/{} has inited", ns, def);
      Ok((
        Calcit::Symbol(
          original_sym.to_owned(),
          ns.to_owned(),
          def.to_owned(),
          Some(Box::new(ResolvedDef(ns.to_owned(), def.to_owned(), import_rule))),
        ),
        Some(v),
      ))
    }
    None => {
      // println!("init for... {}/{}", ns, def);
      match program::lookup_def_code(ns, def) {
        Some(code) => {
          // write a nil value first to prevent dead loop
          program::write_evaled_def(ns, def, Calcit::Nil).map_err(CalcitErr::use_string)?;

          push_call_stack(ns, def, StackKind::Fn, code.to_owned(), &im::vector![]);

          let (resolved_code, _resolve_value) = preprocess_expr(&code, &HashSet::new(), ns, check_warnings)?;
          // println!("\n resolve code to run: {:?}", resolved_code);
          let v = if is_fn_or_macro(&resolved_code) {
            match runner::evaluate_expr(&resolved_code, &im::HashMap::new(), ns) {
              Ok(ret) => ret,
              Err(e) => return Err(e),
            }
          } else {
            Calcit::Thunk(Box::new(resolved_code), None)
          };
          // println!("\nwriting value to: {}/{} {:?}", ns, def, v);
          program::write_evaled_def(ns, def, v.to_owned()).map_err(CalcitErr::use_string)?;
          pop_call_stack();
          Ok((
            Calcit::Symbol(
              original_sym.to_owned(),
              ns.to_owned(),
              def.to_owned(),
              Some(Box::new(ResolvedDef(
                ns.to_owned(),
                def.to_owned(),
                Some(ImportRule::NsReferDef(ns.to_owned(), def.to_owned())),
              ))),
            ),
            Some(v),
          ))
        }
        None if ns.starts_with('|') || ns.starts_with('"') => Ok((
          Calcit::Symbol(
            original_sym.to_owned(),
            ns.to_owned(),
            def.to_owned(),
            Some(Box::new(ResolvedDef(ns.to_owned(), def.to_owned(), import_rule))),
          ),
          None,
        )),
        None => Err(CalcitErr::use_string(format!("unknown ns/def in program: {}/{}", ns, def))),
      }
    }
  }
}

fn is_fn_or_macro(code: &Calcit) -> bool {
  match code {
    Calcit::List(xs) => match xs.get(0) {
      Some(Calcit::Symbol(s, ..)) => s == "defn" || s == "defmacro",
      Some(Calcit::Syntax(s, ..)) => s == &CalcitSyntax::Defn || s == &CalcitSyntax::Defmacro,
      _ => false,
    },
    _ => false,
  }
}

pub fn preprocess_expr(
  expr: &Calcit,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  check_warnings: &RefCell<Vec<String>>,
) -> Result<(Calcit, Option<Calcit>), CalcitErr> {
  // println!("preprocessing @{} {}", file_ns, expr);
  match expr {
    Calcit::Symbol(def, def_ns, at_def, _) => match runner::parse_ns_def(def) {
      Some((ns_alias, def_part)) => {
        match program::lookup_ns_target_in_import(def_ns, &ns_alias) {
          Some(target_ns) => {
            // TODO js syntax to handle in future
            preprocess_ns_def(&target_ns, &def_part, def, None, check_warnings)
          }
          None if ns_alias == "js" => Ok((
            Calcit::Symbol(
              def.to_owned(),
              def_ns.to_owned(),
              at_def.to_owned(),
              Some(Box::new(ResolvedDef(String::from("js"), def_part, None))),
            ),
            None,
          )), // js code
          None => Err(CalcitErr::use_string(format!("unknown ns target: {}", def))),
        }
      }
      None => {
        if def == "~" || def == "~@" || def == "&" || def == "?" {
          Ok((
            Calcit::Symbol(def.to_owned(), def_ns.to_owned(), at_def.to_owned(), Some(Box::new(ResolvedRaw))),
            None,
          ))
        } else if scope_defs.contains(def) {
          Ok((
            Calcit::Symbol(def.to_owned(), def_ns.to_owned(), at_def.to_owned(), Some(Box::new(ResolvedLocal))),
            None,
          ))
        } else if CalcitSyntax::is_core_syntax(def) {
          Ok((
            Calcit::Syntax(CalcitSyntax::from(def).map_err(CalcitErr::use_string)?, def_ns.to_owned()),
            None,
          ))
        } else if is_proc_name(def) {
          Ok((Calcit::Proc(def.to_owned()), None))
        } else if program::has_def_code(primes::CORE_NS, def) {
          preprocess_ns_def(primes::CORE_NS, def, def, None, check_warnings)
        } else if program::has_def_code(def_ns, def) {
          preprocess_ns_def(def_ns, def, def, None, check_warnings)
        } else {
          match program::lookup_def_target_in_import(def_ns, def) {
            Some(target_ns) => {
              // effect
              // TODO js syntax to handle in future
              preprocess_ns_def(&target_ns, def, def, None, check_warnings)
            }
            // TODO check js_mode
            None if is_js_syntax_procs(def) => Ok((expr.to_owned(), None)),
            None if def.starts_with('.') => Ok((expr.to_owned(), None)),
            None => {
              let from_default = program::lookup_default_target_in_import(def_ns, def);
              if let Some(target_ns) = from_default {
                let target = Some(Box::new(ResolvedDef(
                  target_ns.to_owned(),
                  def.to_owned(),
                  Some(ImportRule::NsDefault(target_ns)),
                )));
                Ok((Calcit::Symbol(def.to_owned(), def_ns.to_owned(), at_def.to_owned(), target), None))
              } else {
                let mut names: Vec<String> = vec![];
                for def in scope_defs {
                  names.push(def.to_owned());
                }
                let mut warnings = check_warnings.borrow_mut();
                warnings.push(format!(
                  "[Warn] unknown `{}` in {}/{}, locals {{{}}}",
                  def,
                  def_ns,
                  at_def,
                  names.join(" ")
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
        process_list_call(xs, scope_defs, file_ns, check_warnings)
      }
    }
    Calcit::Number(..) | Calcit::Str(..) | Calcit::Nil | Calcit::Bool(..) | Calcit::Keyword(..) => {
      Ok((expr.to_owned(), Some(expr.to_owned())))
    }
    Calcit::Proc(..) => {
      // maybe detect method in future
      Ok((expr.to_owned(), Some(expr.to_owned())))
    }

    _ => {
      let mut warnings = check_warnings.borrow_mut();
      warnings.push(format!("[Warn] unexpected data during preprocess: {:?}", expr));
      Ok((expr.to_owned(), None))
    }
  }
}

fn process_list_call(
  xs: &CalcitItems,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  check_warnings: &RefCell<Vec<String>>,
) -> Result<(Calcit, Option<Calcit>), CalcitErr> {
  let head = &xs[0];
  let (head_form, head_evaled) = preprocess_expr(head, scope_defs, file_ns, check_warnings)?;
  let args = xs.skip(1);
  let def_name = grab_def_name(head);

  // println!(
  //   "handling list call: {} {:?}, {}",
  //   primes::CrListWrap(xs.to_owned()),
  //   head_form,
  //   if head_evaled.is_some() {
  //     head_evaled.to_owned().unwrap()
  //   } else {
  //     Calcit::Nil
  //   }
  // );

  match (head_form.to_owned(), head_evaled) {
    (Calcit::Keyword(..), _) => {
      if args.len() == 1 {
        let code = Calcit::List(im::vector![
          Calcit::Symbol(
            String::from("get"),
            String::from(primes::CORE_NS),
            String::from(primes::GENERATED_DEF),
            Some(Box::new(ResolvedDef(String::from(primes::CORE_NS), String::from("get"), None)))
          ),
          args[0].to_owned(),
          head.to_owned()
        ]);
        preprocess_expr(&code, scope_defs, file_ns, check_warnings)
      } else {
        Err(CalcitErr::use_string(format!("{} expected single argument", head)))
      }
    }
    (Calcit::Macro(name, def_ns, _, def_args, body), _)
    | (Calcit::Symbol(..), Some(Calcit::Macro(name, def_ns, _, def_args, body))) => {
      let mut current_values = args.to_owned();

      // println!("eval macro: {}", primes::CrListWrap(xs.to_owned()));
      // println!("macro... {} {}", x, CrListWrap(current_values.to_owned()));

      let code = Calcit::List(xs.to_owned());
      push_call_stack(&def_ns, &name, StackKind::Macro, code, &args);

      loop {
        // need to handle recursion
        // println!("evaling line: {:?}", body);
        let body_scope = runner::bind_args(&def_args, &current_values, &im::HashMap::new())?;
        let code = runner::evaluate_lines(&body, &body_scope, &def_ns)?;
        match code {
          Calcit::Recur(ys) => {
            current_values = ys;
          }
          _ => {
            // println!("gen code: {} {}", code, &code.lisp_str());
            let (final_code, v) = preprocess_expr(&code, scope_defs, file_ns, check_warnings)?;
            pop_call_stack();
            return Ok((final_code, v));
          }
        }
      }
    }
    (Calcit::Syntax(name, name_ns), _) => match name {
      CalcitSyntax::Quasiquote => Ok((
        preprocess_quasiquote(&name, &name_ns, &args, scope_defs, file_ns, check_warnings)?,
        None,
      )),
      CalcitSyntax::Defn | CalcitSyntax::Defmacro => {
        Ok((preprocess_defn(&name, &name_ns, &args, scope_defs, file_ns, check_warnings)?, None))
      }
      CalcitSyntax::CoreLet => Ok((
        preprocess_call_let(&name, &name_ns, &args, scope_defs, file_ns, check_warnings)?,
        None,
      )),
      CalcitSyntax::If
      | CalcitSyntax::Try
      | CalcitSyntax::Macroexpand
      | CalcitSyntax::MacroexpandAll
      | CalcitSyntax::Macroexpand1
      | CalcitSyntax::Reset => Ok((
        preprocess_each_items(&name, &name_ns, &args, scope_defs, file_ns, check_warnings)?,
        None,
      )),
      CalcitSyntax::Quote | CalcitSyntax::Eval | CalcitSyntax::HintFn => {
        Ok((preprocess_quote(&name, &name_ns, &args, scope_defs, file_ns)?, None))
      }
      CalcitSyntax::Defatom => Ok((
        preprocess_defatom(&name, &name_ns, &args, scope_defs, file_ns, check_warnings)?,
        None,
      )),
    },
    (Calcit::Thunk(..), _) => Err(CalcitErr::use_string(format!("does not know how to preprocess a thunk: {}", head))),

    (_, Some(Calcit::Fn(f_name, _name_ns, _id, _scope, f_args, _f_body))) => {
      check_fn_args(&f_args, &args, file_ns, &f_name, &def_name, check_warnings);
      let mut ys = im::vector![head_form];
      for a in args {
        let (form, _v) = preprocess_expr(&a, scope_defs, file_ns, check_warnings)?;
        ys.push_back(form);
      }
      Ok((Calcit::List(ys), None))
    }
    (_, _) => {
      let mut ys = im::vector![head_form];
      for a in args {
        let (form, _v) = preprocess_expr(&a, scope_defs, file_ns, check_warnings)?;
        ys.push_back(form);
      }
      Ok((Calcit::List(ys), None))
    }
  }
}

// detects arguments of top-level functions when possible
fn check_fn_args(
  defined_args: &CalcitItems,
  params: &CalcitItems,
  file_ns: &str,
  f_name: &str,
  def_name: &str,
  check_warnings: &RefCell<Vec<String>>,
) {
  let mut i = 0;
  let mut j = 0;
  let mut optional = false;

  loop {
    let d = defined_args.get(i);
    let r = params.get(j);

    match (d, r) {
      (None, None) => return,
      (_, Some(Calcit::Symbol(sym, ..))) if sym == "&" => {
        // dynamic values, can't tell yet
        return;
      }
      (Some(Calcit::Symbol(sym, ..)), _) if sym == "&" => {
        // dynamic args rule, all okay
        return;
      }
      (Some(Calcit::Symbol(sym, ..)), _) if sym == "?" => {
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
          warnings.push(format!(
            "[Warn] lack of args in {} `{}` with `{}`, at {}/{}",
            f_name,
            primes::CrListWrap(defined_args.to_owned()),
            primes::CrListWrap(params.to_owned()),
            file_ns,
            def_name
          ));
          return;
        }
      }
      (None, Some(_)) => {
        let mut warnings = check_warnings.borrow_mut();
        warnings.push(format!(
          "[Warn] too many args for {} `{}` with `{}`, at {}/{}",
          f_name,
          primes::CrListWrap(defined_args.to_owned()),
          primes::CrListWrap(params.to_owned()),
          file_ns,
          def_name
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
fn grab_def_name(x: &Calcit) -> String {
  match x {
    Calcit::Symbol(_, _, def_name, _) => def_name.to_owned(),
    _ => String::from("??"),
  }
}

// tradition rule for processing exprs
pub fn preprocess_each_items(
  head: &CalcitSyntax,
  head_ns: &str,
  args: &CalcitItems,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  check_warnings: &RefCell<Vec<String>>,
) -> Result<Calcit, CalcitErr> {
  let mut xs: CalcitItems = im::vector![Calcit::Syntax(head.to_owned(), head_ns.to_owned())];
  for a in args {
    let (form, _v) = preprocess_expr(a, scope_defs, file_ns, check_warnings)?;
    xs.push_back(form);
  }
  Ok(Calcit::List(xs))
}

pub fn preprocess_defn(
  head: &CalcitSyntax,
  head_ns: &str,
  args: &CalcitItems,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  check_warnings: &RefCell<Vec<String>>,
) -> Result<Calcit, CalcitErr> {
  // println!("defn args: {}", primes::CrListWrap(args.to_owned()));
  let mut xs: CalcitItems = im::vector![Calcit::Syntax(head.to_owned(), head_ns.to_owned())];
  match (args.get(0), args.get(1)) {
    (Some(Calcit::Symbol(def_name, def_name_ns, at_def, _)), Some(Calcit::List(ys))) => {
      let mut body_defs: HashSet<String> = scope_defs.to_owned();

      xs.push_back(Calcit::Symbol(
        def_name.to_owned(),
        def_name_ns.to_owned(),
        at_def.to_owned(),
        Some(Box::new(ResolvedRaw)),
      ));
      let mut zs: CalcitItems = im::vector![];
      for y in ys {
        match y {
          Calcit::Symbol(sym, def_ns, at_def, _) => {
            check_symbol(sym, args, check_warnings);
            zs.push_back(Calcit::Symbol(
              sym.to_owned(),
              def_ns.to_owned(),
              at_def.to_owned(),
              Some(Box::new(ResolvedRaw)),
            ));
            // skip argument syntax marks
            if sym != "&" && sym != "?" {
              body_defs.insert(sym.to_owned());
            }
          }
          _ => return Err(CalcitErr::use_string(format!("expected defn args to be symbols, got: {}", y))),
        }
      }
      xs.push_back(Calcit::List(zs));

      for (idx, a) in args.iter().enumerate() {
        if idx >= 2 {
          let (form, _v) = preprocess_expr(a, &body_defs, file_ns, check_warnings)?;
          xs.push_back(form);
        }
      }
      Ok(Calcit::List(xs))
    }
    (Some(a), Some(b)) => Err(CalcitErr::use_string(format!("defn/defmacro expected name and args: {} {}", a, b))),
    (a, b) => Err(CalcitErr::use_string(format!(
      "defn or defmacro expected name and args, got {:?} {:?}",
      a, b,
    ))),
  }
}

// warn if this symbol is used
fn check_symbol(sym: &str, args: &CalcitItems, check_warnings: &RefCell<Vec<String>>) {
  if is_proc_name(sym) || CalcitSyntax::is_core_syntax(sym) || program::has_def_code(primes::CORE_NS, sym) {
    let mut warnings = check_warnings.borrow_mut();
    warnings.push(format!(
      "[Warn] local binding `{}` shadowed `calcit.core/{}`, with {}",
      sym,
      sym,
      primes::CrListWrap(args.to_owned())
    ));
  }
}

pub fn preprocess_call_let(
  head: &CalcitSyntax,
  head_ns: &str,
  args: &CalcitItems,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  check_warnings: &RefCell<Vec<String>>,
) -> Result<Calcit, CalcitErr> {
  let mut xs: CalcitItems = im::vector![Calcit::Syntax(head.to_owned(), head_ns.to_owned())];
  let mut body_defs: HashSet<String> = scope_defs.to_owned();
  let binding = match args.get(0) {
    Some(Calcit::Nil) => Calcit::Nil,
    Some(Calcit::List(ys)) if ys.len() == 2 => match (&ys[0], &ys[1]) {
      (Calcit::Symbol(sym, ..), a) => {
        check_symbol(sym, args, check_warnings);
        body_defs.insert(sym.to_owned());
        let (form, _v) = preprocess_expr(a, &body_defs, file_ns, check_warnings)?;
        Calcit::List(im::vector![ys[0].to_owned(), form])
      }
      (a, b) => return Err(CalcitErr::use_string(format!("invalid pair for &let binding: {} {}", a, b))),
    },
    Some(Calcit::List(ys)) => return Err(CalcitErr::use_string(format!("expected binding of a pair, got {:?}", ys))),
    Some(a) => return Err(CalcitErr::use_string(format!("expected binding of a pair, got {}", a))),
    None => return Err(CalcitErr::use_str("expected binding of a pair, got nothing")),
  };
  xs.push_back(binding);
  for (idx, a) in args.iter().enumerate() {
    if idx > 0 {
      let (form, _v) = preprocess_expr(a, &body_defs, file_ns, check_warnings)?;
      xs.push_back(form);
    }
  }
  Ok(Calcit::List(xs))
}

pub fn preprocess_quote(
  head: &CalcitSyntax,
  head_ns: &str,
  args: &CalcitItems,
  _scope_defs: &HashSet<String>,
  _file_ns: &str,
) -> Result<Calcit, CalcitErr> {
  let mut xs: CalcitItems = im::vector![Calcit::Syntax(head.to_owned(), head_ns.to_owned())];
  for a in args {
    xs.push_back(a.to_owned());
  }
  Ok(Calcit::List(xs))
}

pub fn preprocess_defatom(
  head: &CalcitSyntax,
  head_ns: &str,
  args: &CalcitItems,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  check_warnings: &RefCell<Vec<String>>,
) -> Result<Calcit, CalcitErr> {
  let mut xs: CalcitItems = im::vector![Calcit::Syntax(head.to_owned(), head_ns.to_owned())];
  for a in args {
    // TODO
    let (form, _v) = preprocess_expr(a, scope_defs, file_ns, check_warnings)?;
    xs.push_back(form.to_owned());
  }
  Ok(Calcit::List(xs))
}

/// need to handle experssions inside unquote snippets
pub fn preprocess_quasiquote(
  head: &CalcitSyntax,
  head_ns: &str,
  args: &CalcitItems,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  check_warnings: &RefCell<Vec<String>>,
) -> Result<Calcit, CalcitErr> {
  let mut xs: CalcitItems = im::vector![Calcit::Syntax(head.to_owned(), head_ns.to_owned())];
  for a in args {
    xs.push_back(preprocess_quasiquote_internal(a, scope_defs, file_ns, check_warnings)?);
  }
  Ok(Calcit::List(xs))
}

pub fn preprocess_quasiquote_internal(
  x: &Calcit,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  check_warnings: &RefCell<Vec<String>>,
) -> Result<Calcit, CalcitErr> {
  match x {
    Calcit::List(ys) if ys.is_empty() => Ok(x.to_owned()),
    Calcit::List(ys) => match &ys[0] {
      Calcit::Symbol(s, _, _, _) if s == "~" || s == "~@" => {
        let mut xs: CalcitItems = im::vector![];
        for y in ys {
          let (form, _) = preprocess_expr(y, scope_defs, file_ns, check_warnings)?;
          xs.push_back(form.to_owned());
        }
        Ok(Calcit::List(xs))
      }
      _ => {
        let mut xs: CalcitItems = im::vector![];
        for y in ys {
          xs.push_back(preprocess_quasiquote_internal(y, scope_defs, file_ns, check_warnings)?.to_owned());
        }
        Ok(Calcit::List(xs))
      }
    },
    _ => Ok(x.to_owned()),
  }
}
