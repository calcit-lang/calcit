use crate::builtins;
use crate::builtins::{is_proc_name, is_syntax_name};
use crate::primes::CalcitData;
use crate::primes::CalcitData::*;
use crate::primes::{CalcitScope, CORE_NS};
use crate::program;

pub fn evaluate_expr(
  expr: CalcitData,
  scope: CalcitScope,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<CalcitData, String> {
  match expr {
    CalcitNil => Ok(expr),
    CalcitBool(_) => Ok(expr),
    CalcitNumber(_) => Ok(expr),
    CalcitSymbol(s, ns) => evaluate_symbol(&s, scope, &ns, program_code),
    CalcitKeyword(_) => Ok(expr),
    CalcitString(_) => Ok(expr),
    // CalcitRef(CalcitData), // TODO
    // CalcitThunk(CirruNode), // TODO
    CalcitList(xs) => match xs.get(0) {
      None => Err(String::from("cannot evaluate empty expr")),
      Some(x) => {
        let v = evaluate_expr((*x).clone(), scope.clone(), file_ns, program_code)?;
        let rest_nodes = xs.clone().slice(1..);
        match v {
          CalcitProc(p) => {
            let mut args = im::Vector::new();
            for (idx, x) in xs.iter().enumerate() {
              // TODO arguments spreading syntax
              if idx > 0 {
                let v = evaluate_expr(x.clone(), scope.clone(), file_ns, program_code)?;
                args.push_back(v)
              }
            }
            builtins::handle_proc(&p, args)
          }
          CalcitSyntax(s) => builtins::handle_syntax(&s, rest_nodes, scope),
          CalcitFn(_, _, scope, args, body) => {
            run_fn(rest_nodes, scope, args, body, file_ns, program_code)
          }
          CalcitMacro(_, _, args, body) => {
            run_macro(rest_nodes, scope, args, body, file_ns, program_code)
          }
          CalcitSymbol(s, ns) => Err(format!("cannot evaluate symbol directly: {}/{}", ns, s)),
          a => Err(format!("cannot be used as operator: {}", a)),
        }
      }
    },
    CalcitSet(_) => Err(String::from("unexpected set for expr")),
    CalcitMap(_) => Err(String::from("unexpected map for expr")),
    CalcitRecord(..) => Err(String::from("unexpected record for expr")),
    CalcitProc(_) => Ok(expr),
    CalcitMacro(..) => Ok(expr),
    CalcitFn(..) => Ok(expr),
    CalcitSyntax(_) => Ok(expr),
  }
}

pub fn evaluate_symbol(
  sym: &str,
  scope: CalcitScope,
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
        return Ok(CalcitSyntax(sym.to_string()));
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
        None => Err(format!("unknown builtin fn name: {}", sym)),
      }
    }
  }
}

fn parse_ns_def(s: &str) -> Option<(String, String)> {
  let pieces: Vec<&str> = s.split('/').collect();
  if pieces.len() == 2 {
    if pieces[0].len() > 0 && pieces[1].len() > 0 {
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
      Some(code) => evaluate_expr(code, im::HashMap::new(), ns, program_code),
      None => Err(String::from("cannot find code for def")),
    },
  }
}

pub fn run_fn(
  values: im::Vector<CalcitData>,
  scope: CalcitScope,
  args: im::Vector<CalcitData>,
  body: im::Vector<CalcitData>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<CalcitData, String> {
  let mut body_scope = scope.clone();
  // TODO arguments spreading syntax
  if values.len() != args.len() {
    return Err(String::from("arguments length mismatch"));
  }
  for idx in 0..args.len() {
    match &args[idx] {
      CalcitSymbol(k, _) => {
        body_scope.insert(k.clone(), values[idx].clone());
      }
      _ => return Err(String::from("expected argument in a symbol")),
    }
  }
  let mut ret: CalcitData = CalcitNil;
  for line in body {
    match evaluate_expr(line, body_scope.clone(), file_ns, program_code) {
      Ok(v) => ret = v,
      Err(e) => return Err(e),
    }
  }
  Ok(ret)
}

// TODO
pub fn run_macro(
  values: im::Vector<CalcitData>,
  scope: CalcitScope,
  args: im::Vector<CalcitData>,
  body: im::Vector<CalcitData>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<CalcitData, String> {
  Err(String::from("TODO"))
}
