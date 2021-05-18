use crate::builtins::{is_js_syntax_procs, is_proc_name, is_syntax_name};
use crate::call_stack::{pop_call_stack, push_call_stack, StackKind};
use crate::primes;
use crate::primes::{Calcit, CalcitItems, ImportRule, SymbolResolved::*};
use crate::program;
use crate::runner;
use std::collections::HashSet;

/// returns the resolved symbol,
/// if code related is not preprocessed, do it internal
pub fn preprocess_ns_def(
  ns: &str,
  def: &str,
  program_code: &program::ProgramCodeData,
  // pass original string representation, TODO codegen currently relies on this
  original_sym: &str,
  import_rule: Option<ImportRule>, // returns form and possible value
) -> Result<(Calcit, Option<Calcit>), String> {
  // println!("preprocessing def: {}/{}", ns, def);
  match program::lookup_evaled_def(ns, def) {
    Some(v) => {
      // println!("{}/{} has inited", ns, def);
      Ok((
        Calcit::Symbol(
          original_sym.to_owned(),
          ns.to_owned(),
          Some(ResolvedDef(ns.to_owned(), def.to_owned(), import_rule)),
        ),
        Some(v),
      ))
    }
    None => {
      // println!("init for... {}/{}", ns, def);
      match program::lookup_def_code(ns, def, program_code) {
        Some(code) => {
          // write a nil value first to prevent dead loop
          program::write_evaled_def(ns, def, Calcit::Nil)?;

          push_call_stack(ns, def, StackKind::Fn, code.clone(), &im::vector![]);

          let (resolved_code, _resolve_value) = preprocess_expr(&code, &HashSet::new(), ns, program_code)?;
          // println!("\n resolve code to run: {:?}", resolved_code);
          let v = if is_fn_or_macro(&resolved_code) {
            match runner::evaluate_expr(&resolved_code, &im::HashMap::new(), ns, program_code) {
              Ok(ret) => ret,
              Err(e) => return Err(e),
            }
          } else {
            Calcit::Thunk(Box::new(resolved_code))
          };
          // println!("\nwriting value to: {}/{} {:?}", ns, def, v);
          program::write_evaled_def(ns, def, v.clone())?;
          pop_call_stack();
          Ok((
            Calcit::Symbol(
              original_sym.to_owned(),
              ns.to_owned(),
              Some(ResolvedDef(
                ns.to_owned(),
                def.to_owned(),
                Some(ImportRule::NsReferDef(ns.to_owned(), def.to_owned())),
              )),
            ),
            Some(v),
          ))
        }
        None if ns.starts_with('|') || ns.starts_with('"') => Ok((
          Calcit::Symbol(
            original_sym.to_owned(),
            ns.to_owned(),
            Some(ResolvedDef(ns.to_owned(), def.to_owned(), import_rule)),
          ),
          None,
        )),
        None => Err(format!("unknown ns/def in program: {}/{}", ns, def)),
      }
    }
  }
}

fn is_fn_or_macro(code: &Calcit) -> bool {
  match code {
    Calcit::List(xs) => match xs.get(0) {
      Some(Calcit::Symbol(s, ..)) => s == "defn" || s == "defmacro",
      Some(Calcit::Syntax(s, ..)) => s == "defn" || s == "defmacro",
      _ => false,
    },
    _ => false,
  }
}

pub fn preprocess_expr(
  expr: &Calcit,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<(Calcit, Option<Calcit>), String> {
  // println!("preprocessing @{} {}", file_ns, expr);
  match expr {
    Calcit::Symbol(def, def_ns, _) => match runner::parse_ns_def(def) {
      Some((ns_alias, def_part)) => {
        match program::lookup_ns_target_in_import(&def_ns, &ns_alias, program_code) {
          Some(target_ns) => {
            // TODO js syntax to handle in future
            preprocess_ns_def(&target_ns, &def_part, program_code, def, None)
          }
          None if ns_alias == "js" => Ok((
            Calcit::Symbol(
              def.clone(),
              def_ns.clone(),
              Some(ResolvedDef(String::from("js"), def_part, None)),
            ),
            None,
          )), // js code
          None => Err(format!("unknown ns target: {}", def)),
        }
      }
      None => {
        if def == "~" || def == "~@" || def == "&" || def == "?" {
          Ok((Calcit::Symbol(def.clone(), def_ns.clone(), Some(ResolvedRaw)), None))
        } else if scope_defs.contains(def) {
          Ok((
            Calcit::Symbol(def.to_owned(), def_ns.to_owned(), Some(ResolvedLocal)),
            None,
          ))
        } else if is_syntax_name(def) {
          Ok((Calcit::Syntax(def.to_owned(), def_ns.to_owned()), None))
        } else if is_proc_name(def) {
          Ok((Calcit::Proc(def.to_owned()), None))
        } else if program::has_def_code(primes::CORE_NS, def, program_code) {
          preprocess_ns_def(primes::CORE_NS, def, program_code, def, None)
        } else if program::has_def_code(def_ns, def, program_code) {
          preprocess_ns_def(def_ns, def, program_code, def, None)
        } else {
          match program::lookup_def_target_in_import(def_ns, def, program_code) {
            Some(target_ns) => {
              // effect
              // TODO js syntax to handle in future
              preprocess_ns_def(&target_ns, &def, program_code, def, None)
            }
            // TODO check js_mode
            None if is_js_syntax_procs(def) => Ok((expr.clone(), None)),
            None if def.starts_with('.') => Ok((expr.clone(), None)),
            None => {
              let from_default = program::lookup_default_target_in_import(def_ns, def, program_code);
              if let Some(target_ns) = from_default {
                let target = Some(ResolvedDef(
                  target_ns.to_owned(),
                  def.to_owned(),
                  Some(ImportRule::NsDefault(target_ns)),
                ));
                Ok((Calcit::Symbol(def.to_owned(), def_ns.to_owned(), target), None))
              } else {
                println!("[Warn] unknown symbol in scope: {}/{} {:?}", def_ns, def, scope_defs);
                Ok((expr.clone(), None))
              }
            }
          }
        }
      }
    },
    Calcit::List(xs) => {
      if xs.is_empty() {
        Ok((expr.clone(), None))
      } else {
        // TODO whether function bothers this...
        // println!("start calling: {}", expr);
        process_list_call(&xs, scope_defs, file_ns, program_code)
      }
    }
    Calcit::Number(..) | Calcit::Str(..) | Calcit::Nil | Calcit::Bool(..) | Calcit::Keyword(..) | Calcit::Proc(..) => {
      Ok((expr.clone(), Some(expr.clone())))
    }

    _ => {
      println!("[Warn] unexpected data during preprocess: {:?}", expr);
      Ok((expr.clone(), None))
    }
  }
}

fn process_list_call(
  xs: &CalcitItems,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<(Calcit, Option<Calcit>), String> {
  let mut chunk_xs = xs.clone();
  let head = &chunk_xs.pop_front().unwrap();
  let (head_form, head_evaled) = preprocess_expr(&head, scope_defs, file_ns, program_code)?;
  let args = &chunk_xs;

  // println!(
  //   "handling list call: {} {:?}, {}",
  //   primes::CrListWrap(xs.clone()),
  //   head_form,
  //   if head_evaled.is_some() {
  //     head_evaled.clone().unwrap()
  //   } else {
  //     Calcit::Nil
  //   }
  // );

  match (head_form, head_evaled) {
    (Calcit::Keyword(..), _) => {
      if args.len() == 1 {
        let code = Calcit::List(im::vector![
          Calcit::Proc(String::from("&get")),
          args[0].clone(),
          head.clone()
        ]);
        preprocess_expr(&code, scope_defs, file_ns, program_code)
      } else {
        Err(format!("{} expected single argument", head))
      }
    }
    (Calcit::Macro(name, def_ns, _, def_args, body), _)
    | (Calcit::Symbol(..), Some(Calcit::Macro(name, def_ns, _, def_args, body))) => {
      let mut current_values = args.clone();

      // println!("eval macro: {}", primes::CrListWrap(xs.clone()));
      // println!("macro... {} {}", x, CrListWrap(current_values.clone()));

      let code = Calcit::List(xs.clone());
      push_call_stack(&def_ns, &name, StackKind::Macro, code, &args);

      loop {
        // need to handle recursion
        // println!("evaling line: {:?}", body);
        let body_scope = runner::bind_args(&def_args, &current_values, &im::HashMap::new())?;
        let code = runner::evaluate_lines(&body, &body_scope, &def_ns, program_code)?;
        match code {
          Calcit::Recur(ys) => {
            current_values = ys;
          }
          _ => {
            // println!("gen code: {} {}", code, &code.lisp_str());
            let (final_code, v) = preprocess_expr(&code, scope_defs, file_ns, program_code)?;
            pop_call_stack();
            return Ok((final_code, v));
          }
        }
      }
    }
    (Calcit::Syntax(name, name_ns), _) => match name.as_str() {
      "quote-replace" | "quasiquote" => Ok((
        preprocess_quasiquote(&name, &name_ns, args, scope_defs, file_ns, program_code)?,
        None,
      )),
      "defn" | "defmacro" => Ok((
        preprocess_defn(&name, &name_ns, args, scope_defs, file_ns, program_code)?,
        None,
      )),
      "&let" => Ok((
        preprocess_call_let(&name, &name_ns, args, scope_defs, file_ns, program_code)?,
        None,
      )),
      "if" | "assert" | "do" | "try" | "macroexpand" | "macroexpand-all" | "macroexpand-1" | "foldl"
      | "foldl-shortcut" | "sort" | "reset!" => Ok((
        preprocess_each_items(&name, &name_ns, args, scope_defs, file_ns, program_code)?,
        None,
      )),
      "quote" | "eval" => Ok((
        preprocess_quote(&name, &name_ns, args, scope_defs, file_ns, program_code)?,
        None,
      )),
      "defatom" => Ok((
        preprocess_defatom(&name, &name_ns, args, scope_defs, file_ns, program_code)?,
        None,
      )),
      _ => Err(format!("unknown syntax: {}", head)),
    },
    (Calcit::Thunk(..), _) => Err(format!("does not know how to preprocess a thunk: {}", head)),
    (_, _) => {
      let (head_form, _v) = preprocess_expr(&head, scope_defs, file_ns, program_code)?;
      let mut ys = im::vector![head_form];
      for a in args {
        let (form, _v) = preprocess_expr(&a, scope_defs, file_ns, program_code)?;
        ys.push_back(form);
      }
      Ok((Calcit::List(ys), None))
    }
  }
}

// tradition rule for processing exprs
pub fn preprocess_each_items(
  head: &str,
  head_ns: &str,
  args: &CalcitItems,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<Calcit, String> {
  let mut xs: CalcitItems = im::vector![Calcit::Syntax(head.to_owned(), head_ns.to_owned())];
  for a in args {
    let (form, _v) = preprocess_expr(a, scope_defs, file_ns, program_code)?;
    xs.push_back(form);
  }
  Ok(Calcit::List(xs))
}

pub fn preprocess_defn(
  head: &str,
  head_ns: &str,
  args: &CalcitItems,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<Calcit, String> {
  // println!("defn args: {}", primes::CrListWrap(args.clone()));
  let mut xs: CalcitItems = im::vector![Calcit::Syntax(head.to_owned(), head_ns.to_owned())];
  match (args.get(0), args.get(1)) {
    (Some(Calcit::Symbol(def_name, def_name_ns, _)), Some(Calcit::List(ys))) => {
      let mut body_defs: HashSet<String> = scope_defs.clone();

      xs.push_back(Calcit::Symbol(def_name.clone(), def_name_ns.clone(), Some(ResolvedRaw)));
      let mut zs: CalcitItems = im::vector![];
      for y in ys {
        match y {
          Calcit::Symbol(sym, def_ns, _) => {
            check_symbol(sym, program_code);
            zs.push_back(Calcit::Symbol(sym.clone(), def_ns.clone(), Some(ResolvedRaw)));
            // skip argument syntax marks
            if sym != "&" && sym != "?" {
              body_defs.insert(sym.clone());
            }
          }
          _ => return Err(format!("expected defn args to be symbols, got: {}", y)),
        }
      }
      xs.push_back(Calcit::List(zs));

      for (idx, a) in args.iter().enumerate() {
        if idx >= 2 {
          let (form, _v) = preprocess_expr(a, &body_defs, file_ns, program_code)?;
          xs.push_back(form);
        }
      }
      Ok(Calcit::List(xs))
    }
    (Some(a), Some(b)) => Err(format!("defn/defmacro expected name and args: {} {}", a, b)),
    (a, b) => Err(format!("defn or defmacro expected name and args, got {:?} {:?}", a, b,)),
  }
}

// warn if this symbol is used
fn check_symbol(sym: &str, program_code: &program::ProgramCodeData) {
  if is_proc_name(sym) || is_syntax_name(sym) || program::has_def_code(primes::CORE_NS, sym, program_code) {
    println!("[Warn] local binding `{}` shadowed `calcit.core/{}`", sym, sym);
  }
}

pub fn preprocess_call_let(
  head: &str,
  head_ns: &str,
  args: &CalcitItems,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<Calcit, String> {
  let mut xs: CalcitItems = im::vector![Calcit::Syntax(head.to_owned(), head_ns.to_owned())];
  let mut body_defs: HashSet<String> = scope_defs.clone();
  let binding = match args.get(0) {
    Some(Calcit::Nil) => Calcit::Nil,
    Some(Calcit::List(ys)) if ys.len() == 2 => match (&ys[0], &ys[1]) {
      (Calcit::Symbol(sym, ..), a) => {
        check_symbol(sym, program_code);
        body_defs.insert(sym.clone());
        let (form, _v) = preprocess_expr(a, &body_defs, file_ns, program_code)?;
        Calcit::List(im::vector![ys[0].clone(), form])
      }
      (a, b) => return Err(format!("invalid pair for &let binding: {} {}", a, b)),
    },
    Some(Calcit::List(ys)) => return Err(format!("expected binding of a pair, got {:?}", ys)),
    Some(a) => return Err(format!("expected binding of a pair, got {}", a)),
    None => return Err(String::from("expected binding of a pair, got nothing")),
  };
  xs.push_back(binding);
  for (idx, a) in args.iter().enumerate() {
    if idx > 0 {
      let (form, _v) = preprocess_expr(a, &body_defs, file_ns, program_code)?;
      xs.push_back(form);
    }
  }
  Ok(Calcit::List(xs))
}

pub fn preprocess_quote(
  head: &str,
  head_ns: &str,
  args: &CalcitItems,
  _scope_defs: &HashSet<String>,
  _file_ns: &str,
  _program_code: &program::ProgramCodeData,
) -> Result<Calcit, String> {
  let mut xs: CalcitItems = im::vector![Calcit::Syntax(head.to_owned(), head_ns.to_owned())];
  for a in args {
    xs.push_back(a.clone());
  }
  Ok(Calcit::List(xs))
}

pub fn preprocess_defatom(
  head: &str,
  head_ns: &str,
  args: &CalcitItems,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<Calcit, String> {
  let mut xs: CalcitItems = im::vector![Calcit::Syntax(head.to_owned(), head_ns.to_owned())];
  for a in args {
    // TODO
    let (form, _v) = preprocess_expr(a, &scope_defs, file_ns, program_code)?;
    xs.push_back(form.clone());
  }
  Ok(Calcit::List(xs))
}

/// need to handle experssions inside unquote snippets
pub fn preprocess_quasiquote(
  head: &str,
  head_ns: &str,
  args: &CalcitItems,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<Calcit, String> {
  let mut xs: CalcitItems = im::vector![Calcit::Syntax(head.to_owned(), head_ns.to_owned())];
  for a in args {
    xs.push_back(preprocess_quasiquote_internal(a, scope_defs, file_ns, program_code)?);
  }
  Ok(Calcit::List(xs))
}

pub fn preprocess_quasiquote_internal(
  x: &Calcit,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<Calcit, String> {
  match x {
    Calcit::List(ys) if ys.is_empty() => Ok(x.to_owned()),
    Calcit::List(ys) => match &ys[0] {
      Calcit::Symbol(s, _, _) if s == "~" || s == "~@" => {
        let mut xs: CalcitItems = im::vector![];
        for y in ys {
          let (form, _) = preprocess_expr(y, scope_defs, file_ns, program_code)?;
          xs.push_back(form.to_owned());
        }
        Ok(Calcit::List(xs))
      }
      _ => {
        let mut xs: CalcitItems = im::vector![];
        for y in ys {
          xs.push_back(preprocess_quasiquote_internal(y, scope_defs, file_ns, program_code)?.to_owned());
        }
        Ok(Calcit::List(xs))
      }
    },
    _ => Ok(x.to_owned()),
  }
}
