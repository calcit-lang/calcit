use crate::builtins::{is_proc_name, is_syntax_name};
use crate::call_stack::{pop_call_stack, push_call_stack, StackKind};
use crate::primes;
use crate::primes::{Calcit, CalcitItems, SymbolResolved::*};
use crate::program;
use crate::runner;
use std::collections::HashSet;

/// returns the resolved symbol,
/// if code related is not preprocessed, do it internal
pub fn preprocess_ns_def(
  ns: &str,
  def: &str,
  program_code: &program::ProgramCodeData,
  // returns form and possible value
) -> Result<(Calcit, Option<Calcit>), String> {
  // println!("preprocessing def: {}/{}", ns, def);
  match program::lookup_evaled_def(ns, def) {
    Some(v) => {
      // println!("{}/{} has inited", ns, def);
      Ok((
        Calcit::Symbol(
          def.to_string(),
          ns.to_string(),
          Some(ResolvedDef(ns.to_string(), def.to_string())),
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

          push_call_stack(ns, def, StackKind::Fn, &Some(code.clone()), &im::vector![]);

          let (resolved_code, resolve_value) = preprocess_expr(&code, &HashSet::new(), ns, program_code)?;
          let v = if is_fn_or_macro(&resolved_code) {
            match runner::evaluate_expr(&resolved_code, &im::HashMap::new(), ns, program_code) {
              Ok(ret) => ret,
              Err(e) => return Err(e),
            }
          } else {
            Calcit::Thunk(Box::new(code))
          };
          // println!("writing value to: {}/{}", ns, def);
          program::write_evaled_def(ns, def, v.clone())?;
          pop_call_stack();
          Ok((
            Calcit::Symbol(
              def.to_string(),
              ns.to_string(),
              Some(ResolvedDef(ns.to_string(), def.to_string())),
            ),
            Some(v),
          ))
        }
        None => Err(format!("unknown ns/def in program: {}/{}", ns, def)),
      }
    }
  }
}

fn is_fn_or_macro(code: &Calcit) -> bool {
  match code {
    Calcit::List(xs) => match xs.get(0) {
      Some(Calcit::Symbol(s, ..)) => s == "defn" || s == "defmacro",
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
            preprocess_ns_def(&target_ns, &def_part, program_code)
          }
          None => Err(format!("unknown ns target: {}", def)),
        }
      }
      None => {
        if def == "~" || def == "~@" || def == "&" || def == "?" {
          Ok((expr.clone(), None))
        } else if is_syntax_name(def) {
          Ok((Calcit::Syntax(def.to_string(), def_ns.to_string()), None))
        } else if is_proc_name(def) {
          Ok((Calcit::Proc(def.to_string()), None))
        } else if scope_defs.contains(def) {
          Ok((
            Calcit::Symbol(def.to_string(), def_ns.to_string(), Some(ResolvedLocal)),
            None,
          ))
        } else if program::has_def_code(primes::CORE_NS, def, program_code) {
          preprocess_ns_def(primes::CORE_NS, def, program_code)
        } else if program::has_def_code(def_ns, def, program_code) {
          preprocess_ns_def(def_ns, def, program_code)
        } else {
          match program::lookup_def_target_in_import(def_ns, def, program_code) {
            Some(target_ns) => {
              // effect
              // TODO js syntax to handle in future
              preprocess_ns_def(&target_ns, &def, program_code)
            }
            None => Err(format!("unknown symbol in scope: {}/{} {:?}", def_ns, def, scope_defs)),
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
    Calcit::Number(..) | Calcit::Str(..) | Calcit::Nil | Calcit::Bool(..) | Calcit::Keyword(..) => {
      Ok((expr.clone(), Some(expr.clone())))
    }

    _ => {
      println!("[Warn] unexpected data during preprocess: {}", expr);
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
          Calcit::Symbol("&get".to_string(), primes::GENERATED_NS.to_string(), None),
          args[1].clone(),
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

      let code = Some(Calcit::List(xs.clone()));
      push_call_stack(&def_ns, &name, StackKind::Macro, &code, &args);

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
            // println!("gen code: {} {}", code, primes::format_to_lisp(&code));
            let (final_code, v) = preprocess_expr(&code, scope_defs, file_ns, program_code)?;
            pop_call_stack();
            return Ok((final_code, v));
          }
        }
      }
    }
    (Calcit::Syntax(name, _ns), _) => {
      match name.as_str() {
        "quote-replace" | "quasiquote" => Ok((Calcit::List(xs.clone()), None)),
        "defn" | "defmacro" => Ok((preprocess_defn(head, args, scope_defs, file_ns, program_code)?, None)),
        "&let" => Ok((
          preprocess_call_let(head, args, scope_defs, file_ns, program_code)?,
          None,
        )),
        "if" | "assert" | "do" | "try" | "macroexpand" | "macroexpand-all" | "macroexpand-1" | "foldl" => Ok((
          preprocess_each_items(head, args, scope_defs, file_ns, program_code)?,
          None,
        )),
        "quote" | "eval" => Ok((preprocess_quote(head, args, scope_defs, file_ns, program_code)?, None)),
        // TODO
        // "defatom" => {}
        _ => Err(format!("unknown syntax: {}", head)),
      }
    }
    (Calcit::Thunk(..), _) => Err(format!("does not know how to preprocess a thunk: {}", head)),
    (_, _) => {
      let mut ys = im::vector![head.clone()];
      for a in args {
        let (form, v) = preprocess_expr(&a, scope_defs, file_ns, program_code)?;
        ys.push_back(form);
      }
      Ok((Calcit::List(ys), None))
    }
  }
}

// tradition rule for processing exprs
pub fn preprocess_each_items(
  head: &Calcit,
  args: &CalcitItems,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<Calcit, String> {
  let mut xs: CalcitItems = im::vector![head.clone()];
  for a in args {
    let (form, v) = preprocess_expr(a, scope_defs, file_ns, program_code)?;
    xs.push_back(form);
  }
  Ok(Calcit::List(xs))
}

pub fn preprocess_defn(
  head: &Calcit,
  args: &CalcitItems,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<Calcit, String> {
  // println!("defn args: {}", primes::CrListWrap(args.clone()));
  let mut xs: CalcitItems = im::vector![head.clone()];
  match (args.get(0), args.get(1)) {
    (Some(Calcit::Symbol(..)), Some(Calcit::List(ys))) => {
      let mut body_defs: HashSet<String> = scope_defs.clone();
      for (idx, a) in args.iter().enumerate() {
        match idx {
          0 => {
            xs.push_back(a.clone());
          }
          1 => {
            xs.push_back(a.clone());
            for y in ys {
              match y {
                Calcit::Symbol(sym, ..) => {
                  body_defs.insert(sym.clone());
                }
                _ => return Err(format!("expected defn args to be symbols, got: {}", y)),
              }
            }
          }
          _ => {
            let (form, v) = preprocess_expr(a, &body_defs, file_ns, program_code)?;
            xs.push_back(form);
          }
        }
      }
      Ok(Calcit::List(xs))
    }
    (Some(a), Some(b)) => Err(format!("defn/defmacro expected name and args: {} {}", a, b)),
    (a, b) => Err(format!("defn or defmacro expected name and args, got {:?} {:?}", a, b,)),
  }
}

pub fn preprocess_call_let(
  head: &Calcit,
  args: &CalcitItems,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<Calcit, String> {
  let mut xs: CalcitItems = im::vector![head.clone()];
  let mut body_defs: HashSet<String> = scope_defs.clone();
  let binding = match args.get(0) {
    Some(Calcit::Nil) => Calcit::Nil,
    Some(Calcit::List(ys)) if ys.len() == 2 => match (&ys[0], &ys[1]) {
      (Calcit::Symbol(sym, ..), a) => {
        body_defs.insert(sym.clone());
        let (form, v) = preprocess_expr(a, &body_defs, file_ns, program_code)?;
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
      let (form, v) = preprocess_expr(a, &body_defs, file_ns, program_code)?;
      xs.push_back(form);
    }
  }
  Ok(Calcit::List(xs))
}

pub fn preprocess_quote(
  head: &Calcit,
  args: &CalcitItems,
  _scope_defs: &HashSet<String>,
  _file_ns: &str,
  _program_code: &program::ProgramCodeData,
) -> Result<Calcit, String> {
  let mut xs: CalcitItems = im::vector![head.clone()];
  for a in args {
    xs.push_back(a.clone());
  }
  Ok(Calcit::List(xs))
}
