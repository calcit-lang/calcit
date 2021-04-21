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
) -> Result<(), String> {
  if program::has_evaled_def(ns, def) {
    Ok(())
  } else {
    match program::lookup_def_code(ns, def, program_code) {
      Some(code) => {
        let resolved_code = preprocess_expr(&code, &HashSet::new(), ns, program_code)?;
        let v = if is_fn_or_macro(&resolved_code) {
          match runner::evaluate_expr(&resolved_code, &im::HashMap::new(), ns, program_code) {
            Ok(ret) => ret,
            Err(e) => return Err(e),
          }
        } else {
          Calcit::Thunk(Box::new(code))
        };
        let _eff = program::write_evaled_def(ns, def, v);
        Ok(())
      }
      None => Err(format!("unknown ns/def in program: {}/{}", ns, def)),
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

// TODO
pub fn preprocess_expr(
  expr: &Calcit,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<Calcit, String> {
  match expr {
    Calcit::Symbol(def, def_ns, _) => match runner::parse_ns_def(def) {
      Some((ns_alias, def_part)) => {
        match program::lookup_ns_target_in_import(&def_ns, &ns_alias, program_code) {
          Some(target_ns) => {
            // effect
            let _eff = preprocess_expr(expr, scope_defs, def_ns, program_code);
            // TODO js syntax to handle in future
            Ok(Calcit::Symbol(
              def.to_string(),
              def_ns.to_string(),
              Some(ResolvedDef(target_ns, def_part)),
            ))
          }
          None => Err(format!("unknown ns target: {}", def)),
        }
      }
      None => {
        if def == "~" || def == "~@" || def == "&" || def == "?" {
          Ok(expr.clone())
        } else if is_syntax_name(def) {
          Ok(Calcit::Syntax(def.to_string(), def_ns.to_string()))
        } else if is_proc_name(def) {
          Ok(Calcit::Proc(def.to_string()))
        } else if scope_defs.contains(def) {
          Ok(Calcit::Symbol(
            def.to_string(),
            def_ns.to_string(),
            Some(ResolvedLocal),
          ))
        } else if program::has_def_code(primes::CORE_NS, def, program_code) {
          let _eff = preprocess_ns_def(primes::CORE_NS, def, program_code); // effect
          Ok(Calcit::Symbol(
            primes::CORE_NS.to_string(),
            def_ns.clone(),
            Some(ResolvedDef(def_ns.clone(), def.clone())),
          ))
        } else if program::has_def_code(def_ns, def, program_code) {
          let _eff = preprocess_ns_def(def_ns, def, program_code); // effect
          Ok(Calcit::Symbol(
            def_ns.clone(),
            def_ns.clone(),
            Some(ResolvedDef(def_ns.clone(), def.clone())),
          ))
        } else {
          match program::lookup_def_target_in_import(def_ns, def, program_code) {
            Some(target_ns) => {
              // effect
              let _eff = preprocess_expr(expr, scope_defs, file_ns, program_code);
              // TODO js syntax to handle in future
              Ok(Calcit::Symbol(
                def.to_string(),
                def_ns.to_string(),
                Some(ResolvedDef(target_ns, def.to_string())),
              ))
            }
            None => Err(format!("unknown symbol: {}/{}", def_ns, def)),
          }
        }
      }
    },
    Calcit::List(xs) => {
      if xs.is_empty() {
        Ok(expr.clone())
      } else {
        // TODO whether function bothers this...
        process_list_call(&xs, scope_defs, file_ns, program_code)
      }
    }
    Calcit::Number(..) | Calcit::Str(..) | Calcit::Nil | Calcit::Bool(..) | Calcit::Keyword(..) => {
      Ok(expr.clone())
    }

    _ => {
      println!("[Warn] unexpected data during preprocess: {}", expr);
      Ok(expr.clone())
    }
  }
}

fn process_list_call(
  xs: &CalcitItems,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<Calcit, String> {
  let mut chunk_xs = xs.clone();
  let head = &chunk_xs.pop_front().unwrap();
  let value = preprocess_expr(&head, scope_defs, file_ns, program_code)?;
  let args = &chunk_xs;

  match head {
    Calcit::Keyword(..) => {
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
    Calcit::Macro(name, def_ns, _, def_args, body) => {
      let mut current_values = args.clone();

      // println!("eval macro: {} {}", x, primes::format_to_lisp(expr));
      // println!("macro... {} {}", x, CrListWrap(current_values.clone()));

      push_call_stack(
        file_ns,
        &name,
        StackKind::Macro,
        &Some(Calcit::List(xs.clone())),
        &args,
      );

      loop {
        // need to handle recursion
        let body_scope = runner::bind_args(&def_args, &current_values, &im::HashMap::new())?;
        let code = runner::evaluate_lines(&body, &body_scope, def_ns, program_code)?;
        match code {
          Calcit::Recur(ys) => {
            current_values = ys;
          }
          _ => {
            // println!("gen code: {} {}", x, primes::format_to_lisp(&code));
            pop_call_stack();
            return Ok(code);
          }
        }
      }
    }
    Calcit::Syntax(name, _ns) => {
      match name.as_str() {
        ";" | "quote-replace" => Ok(Calcit::List(xs.clone())),
        "defn" | "defmacro" => preprocess_defn(head, args, scope_defs, file_ns, program_code),
        "&let" => preprocess_call_let(head, args, scope_defs, file_ns, program_code),
        "if" | "assert" | "do" | "try" | "macroexpand" | "macroexpand-all" => {
          preprocess_each_items(head, args, scope_defs, file_ns, program_code)
        }
        "quote" | "eval" => preprocess_quote(head, args, scope_defs, file_ns, program_code),
        // TODO
        // "defatom" => {}
        _ => Err(format!("unknown syntax: {}", head)),
      }
    }
    Calcit::Thunk(..) => Err(format!("does not know how to preprocess a thunk: {}", head)),
    _ => {
      let mut ys = im::vector![head.clone()];
      for a in args {
        ys.push_back(preprocess_expr(&a, scope_defs, file_ns, program_code)?);
      }
      Ok(Calcit::List(ys))
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
  Ok(Calcit::Nil)
}

pub fn preprocess_defn(
  head: &Calcit,
  args: &CalcitItems,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<Calcit, String> {
  Ok(Calcit::Nil)
}

pub fn preprocess_call_let(
  head: &Calcit,
  args: &CalcitItems,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<Calcit, String> {
  Ok(Calcit::Nil)
}

pub fn preprocess_quote(
  head: &Calcit,
  args: &CalcitItems,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<Calcit, String> {
  Ok(Calcit::Nil)
}
