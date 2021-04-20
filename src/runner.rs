use crate::builtins;
use crate::builtins::{is_proc_name, is_syntax_name};
use crate::call_stack;
use crate::call_stack::{push_call_stack, StackKind};
use crate::primes;
use crate::primes::{CalcitData, CalcitData::*};
use crate::primes::{CalcitItems, CalcitScope, CrListWrap, CORE_NS};
use crate::program;

pub fn evaluate_expr(
  expr: &CalcitData,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<CalcitData, String> {
  // println!("eval code: {}", primes::format_to_lisp(expr));

  match expr {
    CalcitNil => Ok(expr.clone()),
    CalcitBool(_) => Ok(expr.clone()),
    CalcitNumber(_) => Ok(expr.clone()),
    CalcitSymbol(s, _) if s == "&" => Ok(expr.clone()),
    CalcitSymbol(s, ns) => evaluate_symbol(&s, scope, &ns, program_code),
    CalcitKeyword(_) => Ok(expr.clone()),
    CalcitString(_) => Ok(expr.clone()),
    // CalcitRef(CalcitData), // TODO
    // CalcitThunk(CirruNode), // TODO
    CalcitRecur(_) => unreachable!("recur not expected to be from symbol"),
    CalcitList(xs) => match xs.get(0) {
      None => Err(format!("cannot evaluate empty expr: {}", expr)),
      Some(x) => {
        // println!("eval expr: {}", primes::format_to_lisp(expr));
        // println!("eval expr: {}", x);

        let mut added_stack = false;

        let v = evaluate_expr(&x, scope, file_ns, program_code)?;
        let rest_nodes = xs.clone().slice(1..);
        let ret = match &v {
          CalcitProc(p) => {
            let values = evaluate_args(&rest_nodes, scope, file_ns, program_code)?;
            push_call_stack(file_ns, &p, StackKind::Proc, &None, &values);
            added_stack = true;
            builtins::handle_proc(&p, &values)
          }
          CalcitSyntax(s, def_ns) => {
            builtins::handle_syntax(&s, &rest_nodes, scope, def_ns, program_code)
          }
          CalcitFn(name, def_ns, _, def_scope, args, body) => {
            let values = evaluate_args(&rest_nodes, scope, file_ns, program_code)?;
            push_call_stack(file_ns, &name, StackKind::Fn, &Some(expr.clone()), &values);
            added_stack = true;
            run_fn(values, &def_scope, args, body, def_ns, program_code)
          }
          CalcitMacro(name, def_ns, _, args, body) => {
            let mut current_values = rest_nodes.clone();
            let mut macro_ret = CalcitNil;
            // println!("eval macro: {} {}", x, primes::format_to_lisp(expr));
            // println!("macro... {} {}", x, CrListWrap(current_values.clone()));

            push_call_stack(
              file_ns,
              &name,
              StackKind::Macro,
              &Some(expr.clone()),
              &rest_nodes,
            );
            added_stack = true;

            loop {
              // need to handle recursion
              let body_scope = bind_args(&args, &current_values, &im::HashMap::new())?;
              let code = evaluate_lines(&body, &body_scope, def_ns, program_code)?;
              match code {
                CalcitRecur(ys) => {
                  current_values = ys;
                }
                _ => {
                  // println!("gen code: {} {}", x, primes::format_to_lisp(&code));
                  macro_ret = evaluate_expr(&code, scope, file_ns, program_code)?;
                  break;
                }
              }
            }

            Ok(macro_ret)
          }
          CalcitSymbol(s, ns) => Err(format!("cannot evaluate symbol directly: {}/{}", ns, s)),
          a => Err(format!("cannot be used as operator: {}", a)),
        };

        if added_stack && ret.is_ok() {
          call_stack::pop_call_stack();
        }

        ret
      }
    },
    CalcitSet(_) => Err(String::from("unexpected set for expr")),
    CalcitMap(_) => Err(String::from("unexpected map for expr")),
    CalcitRecord(..) => Err(String::from("unexpected record for expr")),
    CalcitProc(_) => Ok(expr.clone()),
    CalcitMacro(..) => Ok(expr.clone()),
    CalcitFn(..) => Ok(expr.clone()),
    CalcitSyntax(_, _) => Ok(expr.clone()),
  }
}

pub fn evaluate_symbol(
  sym: &str,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<CalcitData, String> {
  match parse_ns_def(&sym) {
    Some((ns_part, def_part)) => {
      match program::lookup_ns_target_in_import(file_ns, &ns_part, program_code) {
        Some(target_ns) => match eval_symbol_from_program(&def_part, &target_ns, program_code) {
          Ok(v) => Ok(v),
          Err(e) => Err(e),
        },
        None => Err(String::from("unknown ns target")),
      }
    }
    None => {
      if is_syntax_name(sym) {
        return Ok(CalcitSyntax(sym.to_string(), file_ns.to_string()));
      }
      if is_proc_name(sym) {
        return Ok(CalcitProc(sym.to_string()));
      }
      if program::lookup_ns_def(CORE_NS, sym, program_code).is_some() {
        return eval_symbol_from_program(sym, CORE_NS, program_code);
      }
      if scope.contains_key(sym) {
        return Ok(scope.get(sym).unwrap().clone());
      }
      if program::lookup_ns_def(file_ns, sym, program_code).is_some() {
        return eval_symbol_from_program(sym, file_ns, program_code);
      }
      match program::lookup_def_target_in_import(file_ns, sym, program_code) {
        Some(target_ns) => eval_symbol_from_program(sym, &target_ns, program_code),
        None => Err(format!("unknown symbol: {}", sym)),
      }
    }
  }
}

fn parse_ns_def(s: &str) -> Option<(String, String)> {
  let pieces: Vec<&str> = s.split('/').collect();
  if pieces.len() == 2 {
    if !pieces[0].is_empty() && !pieces[1].is_empty() {
      Some((pieces[0].to_string(), pieces[1].to_string()))
    } else {
      None
    }
  } else {
    None
  }
}

fn eval_symbol_from_program(
  sym: &str,
  ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<CalcitData, String> {
  match program::lookup_evaled_def(ns, sym) {
    Some(v) => Ok(v),
    None => match program::lookup_ns_def(ns, sym, program_code) {
      Some(code) => {
        let v = evaluate_expr(&code, &im::HashMap::new(), ns, program_code)?;
        program::write_evaled_def(ns, sym, v.clone())?;
        Ok(v)
      }
      None => Err(format!("cannot find code for def: {}/{}", ns, sym)),
    },
  }
}

pub fn run_fn(
  values: CalcitItems,
  scope: &CalcitScope,
  args: &CalcitItems,
  body: &CalcitItems,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<CalcitData, String> {
  let mut current_values = values;
  loop {
    let body_scope = bind_args(args, &current_values, scope)?;
    let v = evaluate_lines(body, &body_scope, file_ns, program_code)?;
    match v {
      CalcitRecur(xs) => {
        current_values = xs;
      }
      result => return Ok(result),
    }
  }
}

/// create new scope by wrting new args
/// notice that `&` is a mark for spreading, `?` for optional arguments
pub fn bind_args(
  args: &CalcitItems,
  values: &CalcitItems,
  base_scope: &CalcitScope,
) -> Result<CalcitScope, String> {
  // TODO arguments spreading syntax
  // if values.len() != args.len() {
  //   return Err(format!(
  //     "arguments length mismatch: {} ... {}",
  //     CalcitList(values.clone()),
  //     CalcitList(args.clone()),
  //   ));
  // }
  let mut scope = base_scope.clone();
  let mut spreading = false;
  let mut optional = false;
  let mut collected_args = args.clone();
  let mut collected_values = values.clone();
  while let Some(a) = collected_args.pop_front() {
    if spreading {
      match a {
        CalcitSymbol(s, _) if s == "&" => return Err(format!("invalid & in args: {:?}", args)),
        CalcitSymbol(s, _) if s == "?" => return Err(format!("invalid ? in args: {:?}", args)),
        CalcitSymbol(s, _) => {
          let mut chunk: CalcitItems = im::vector![];
          while let Some(v) = collected_values.pop_front() {
            chunk.push_back(v);
          }
          scope.insert(s, CalcitList(chunk));
          if !collected_args.is_empty() {
            return Err(format!(
              "extra args `{}` after spreading in `{}`",
              CrListWrap(collected_args),
              CrListWrap(args.clone()),
            ));
          }
        }
        b => return Err(format!("invalid argument name: {}", b)),
      }
    } else {
      match a {
        CalcitSymbol(s, _) if s == "&" => spreading = true,
        CalcitSymbol(s, _) if s == "?" => optional = true,
        CalcitSymbol(s, _) => match collected_values.pop_front() {
          Some(v) => {
            scope.insert(s.clone(), v.clone());
          }
          None => {
            if optional {
              scope.insert(s.clone(), CalcitNil);
            } else {
              return Err(format!(
                "too few values `{}` passed to args `{}`",
                CrListWrap(values.clone()),
                CrListWrap(args.clone())
              ));
            }
          }
        },
        b => return Err(format!("invalid argument name: {}", b)),
      }
    }
  }
  if collected_values.is_empty() {
    Ok(scope)
  } else {
    Err(format!(
      "extra args `{}` not handled while passing values `{}` to args `{}`",
      CrListWrap(collected_values),
      CrListWrap(values.clone()),
      CrListWrap(args.clone()),
    ))
  }
}

pub fn evaluate_lines(
  lines: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<CalcitData, String> {
  let mut ret: CalcitData = CalcitNil;
  for line in lines {
    match evaluate_expr(line, scope, file_ns, program_code) {
      Ok(v) => ret = v,
      Err(e) => return Err(e),
    }
  }
  Ok(ret)
}

/// evaluate symbols before calling a function
/// notice that `&` is used to spread a list
pub fn evaluate_args(
  lines: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<CalcitItems, String> {
  let mut ret: CalcitItems = im::vector![];
  let mut spreading = false;
  for line in lines {
    match &evaluate_expr(line, scope, file_ns, program_code) {
      Ok(v) => {
        if spreading {
          match v {
            CalcitSymbol(s, _) if s == "&" => {
              return Err(format!(
                "already in spread mode: {}",
                CrListWrap(lines.clone())
              ))
            }
            CalcitList(xs) => {
              for x in xs {
                ret.push_back(x.clone());
              }
              spreading = false
            }
            a => return Err(format!("expected list for spreading, got: {}", a)),
          }
        } else {
          match v {
            CalcitSymbol(s, _) if s == "&" => spreading = true,
            _ => ret.push_back(v.clone()),
          }
        }
      }
      Err(e) => return Err(e.to_string()),
    }
  }
  Ok(ret)
}
