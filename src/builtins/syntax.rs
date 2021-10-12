//! this file does not cover all syntax instances.
//! syntaxes related to data are maintained the corresponding files
//! Rust has limits on Closures, callbacks need to be handled specifically

use std::cell::RefCell;
use std::collections::HashSet;

use crate::builtins;
use crate::primes::{gen_core_id, Calcit, CalcitErr, CalcitItems, CalcitScope};
use crate::program::ProgramCodeData;
use crate::runner;

pub fn defn(expr: &CalcitItems, scope: &CalcitScope, file_ns: &str, _program: &ProgramCodeData) -> Result<Calcit, CalcitErr> {
  match (expr.get(0), expr.get(1)) {
    (Some(Calcit::Symbol(s, ..)), Some(Calcit::List(xs))) => Ok(Calcit::Fn(
      s.to_owned(),
      file_ns.to_owned(),
      gen_core_id(),
      scope.to_owned(),
      Box::new(xs.to_owned()),
      Box::new(expr.skip(2)),
    )),
    (Some(a), Some(b)) => Err(CalcitErr::use_string(format!("invalid args type for defn: {} , {}", a, b))),
    _ => Err(CalcitErr::use_str("inefficient arguments for defn")),
  }
}

pub fn defmacro(expr: &CalcitItems, _scope: &CalcitScope, def_ns: &str, _program: &ProgramCodeData) -> Result<Calcit, CalcitErr> {
  match (expr.get(0), expr.get(1)) {
    (Some(Calcit::Symbol(s, ..)), Some(Calcit::List(xs))) => Ok(Calcit::Macro(
      s.to_owned(),
      def_ns.to_owned(),
      gen_core_id(),
      Box::new(xs.to_owned()),
      Box::new(expr.skip(2)),
    )),
    (Some(a), Some(b)) => Err(CalcitErr::use_string(format!("invalid structure for defmacro: {} {}", a, b))),
    _ => Err(CalcitErr::use_string(format!(
      "invalid structure for defmacro: {}",
      Calcit::List(expr.to_owned())
    ))),
  }
}

pub fn quote(expr: &CalcitItems, _scope: &CalcitScope, _file_ns: &str, _program: &ProgramCodeData) -> Result<Calcit, CalcitErr> {
  if expr.len() == 1 {
    Ok(expr[0].to_owned())
  } else {
    Err(CalcitErr::use_string(format!("unexpected data for quote: {:?}", expr)))
  }
}

pub fn syntax_if(expr: &CalcitItems, scope: &CalcitScope, file_ns: &str, program_code: &ProgramCodeData) -> Result<Calcit, CalcitErr> {
  match (expr.get(0), expr.get(1)) {
    _ if expr.len() > 3 => Err(CalcitErr::use_string(format!("too many nodes for if: {:?}", expr))),
    (Some(cond), Some(true_branch)) => {
      let cond_value = runner::evaluate_expr(cond, scope, file_ns, program_code)?;
      match cond_value {
        Calcit::Nil | Calcit::Bool(false) => match expr.get(2) {
          Some(false_branch) => runner::evaluate_expr(false_branch, scope, file_ns, program_code),
          None => Ok(Calcit::Nil),
        },
        _ => runner::evaluate_expr(true_branch, scope, file_ns, program_code),
      }
    }
    (None, _) => Err(CalcitErr::use_string(format!("insufficient nodes for if: {:?}", expr))),
    _ => Err(CalcitErr::use_string(format!("invalid if form: {:?}", expr))),
  }
}

pub fn eval(expr: &CalcitItems, scope: &CalcitScope, file_ns: &str, program_code: &ProgramCodeData) -> Result<Calcit, CalcitErr> {
  if expr.len() == 1 {
    let v = runner::evaluate_expr(&expr[0], scope, file_ns, program_code)?;
    runner::evaluate_expr(&v, scope, file_ns, program_code)
  } else {
    Err(CalcitErr::use_string(format!("unexpected data for evaling: {:?}", expr)))
  }
}

pub fn syntax_let(expr: &CalcitItems, scope: &CalcitScope, file_ns: &str, program_code: &ProgramCodeData) -> Result<Calcit, CalcitErr> {
  match expr.get(0) {
    Some(Calcit::Nil) => runner::evaluate_lines(&expr.skip(1), scope, file_ns, program_code),
    Some(Calcit::List(xs)) if xs.len() == 2 => {
      let mut body_scope = scope.to_owned();
      match (&xs[0], &xs[1]) {
        (Calcit::Symbol(s, ..), ys) => {
          let value = runner::evaluate_expr(ys, scope, file_ns, program_code)?;
          body_scope.insert(s.to_owned(), value);
        }
        (a, _) => return Err(CalcitErr::use_string(format!("invalid binding name: {}", a))),
      }
      runner::evaluate_lines(&expr.skip(1), &body_scope, file_ns, program_code)
    }
    Some(Calcit::List(xs)) => Err(CalcitErr::use_string(format!("invalid length: {:?}", xs))),
    Some(_) => Err(CalcitErr::use_string(format!("invalid node for &let: {:?}", expr))),
    None => Err(CalcitErr::use_str("&let expected a pair or a nil")),
  }
}

// code replaced from `~` and `~@` returns different results
#[derive(Clone, PartialEq, Debug)]
enum SpanResult {
  Single(Calcit),
  Range(CalcitItems),
}

pub fn quasiquote(expr: &CalcitItems, scope: &CalcitScope, file_ns: &str, program_code: &ProgramCodeData) -> Result<Calcit, CalcitErr> {
  match expr.get(0) {
    None => Err(CalcitErr::use_str("quasiquote expected a node")),
    Some(code) => {
      match replace_code(code, scope, file_ns, program_code)? {
        SpanResult::Single(v) => {
          // println!("replace result: {:?}", v);
          Ok(v)
        }
        SpanResult::Range(xs) => Err(CalcitErr::use_string(format!(
          "expected single result from quasiquote, got {:?}",
          xs
        ))),
      }
    }
  }
}

fn replace_code(c: &Calcit, scope: &CalcitScope, file_ns: &str, program_code: &ProgramCodeData) -> Result<SpanResult, CalcitErr> {
  if !has_unquote(c) {
    return Ok(SpanResult::Single(c.to_owned()));
  }
  match c {
    Calcit::List(ys) => match (ys.get(0), ys.get(1)) {
      (Some(Calcit::Symbol(sym, ..)), Some(expr)) if sym == "~" => {
        let value = runner::evaluate_expr(expr, scope, file_ns, program_code)?;
        Ok(SpanResult::Single(value))
      }
      (Some(Calcit::Symbol(sym, ..)), Some(expr)) if sym == "~@" => {
        let ret = runner::evaluate_expr(expr, scope, file_ns, program_code)?;
        match ret {
          Calcit::List(zs) => Ok(SpanResult::Range(zs)),
          _ => Err(CalcitErr::use_string(format!("unknown result from unquote-slice: {}", ret))),
        }
      }
      (_, _) => {
        let mut ret = im::vector![];
        for y in ys {
          match replace_code(y, scope, file_ns, program_code)? {
            SpanResult::Single(z) => ret.push_back(z),
            SpanResult::Range(pieces) => {
              for piece in pieces {
                ret.push_back(piece);
              }
            }
          }
        }
        Ok(SpanResult::Single(Calcit::List(ret)))
      }
    },
    _ => Ok(SpanResult::Single(c.to_owned())),
  }
}

pub fn has_unquote(xs: &Calcit) -> bool {
  match xs {
    Calcit::List(ys) => {
      for y in ys {
        if has_unquote(y) {
          return true;
        }
      }
      false
    }
    Calcit::Symbol(s, ..) if s == "~" || s == "~@" => true,
    _ => false,
  }
}

pub fn macroexpand(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<Calcit, CalcitErr> {
  if expr.len() == 1 {
    let quoted_code = runner::evaluate_expr(&expr[0], scope, file_ns, program_code)?;

    match quoted_code.to_owned() {
      Calcit::List(xs) => {
        if xs.is_empty() {
          return Ok(quoted_code);
        }
        let v = runner::evaluate_expr(&xs[0], scope, file_ns, program_code)?;
        match v {
          Calcit::Macro(_, def_ns, _, args, body) => {
            let mut xs_cloned = xs;
            // mutable operation
            let mut rest_nodes = xs_cloned.slice(1..);
            // println!("macro: {:?} ... {:?}", args, rest_nodes);
            // keep expanding until return value is not a recur
            loop {
              let body_scope = runner::bind_args(&args, &rest_nodes, scope)?;
              let v = runner::evaluate_lines(&body, &body_scope, &def_ns, program_code)?;
              match v {
                Calcit::Recur(rest_code) => {
                  rest_nodes = rest_code;
                }
                _ => return Ok(v),
              }
            }
          }
          _ => Ok(quoted_code),
        }
      }
      a => Ok(a),
    }
  } else {
    Err(CalcitErr::use_string(format!(
      "macroexpand expected excaclty 1 argument, got: {:?}",
      expr
    )))
  }
}

pub fn macroexpand_1(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<Calcit, CalcitErr> {
  if expr.len() == 1 {
    let quoted_code = runner::evaluate_expr(&expr[0], scope, file_ns, program_code)?;
    // println!("quoted: {}", quoted_code);
    match quoted_code.to_owned() {
      Calcit::List(xs) => {
        if xs.is_empty() {
          return Ok(quoted_code);
        }
        let v = runner::evaluate_expr(&xs[0], scope, file_ns, program_code)?;
        match v {
          Calcit::Macro(_, def_ns, _, args, body) => {
            let mut xs_cloned = xs;
            let body_scope = runner::bind_args(&args, &xs_cloned.slice(1..), scope)?;
            runner::evaluate_lines(&body, &body_scope, &def_ns, program_code)
          }
          _ => Ok(quoted_code),
        }
      }
      a => Ok(a),
    }
  } else {
    Err(CalcitErr::use_string(format!(
      "macroexpand expected excaclty 1 argument, got: {:?}",
      expr
    )))
  }
}

pub fn macroexpand_all(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<Calcit, CalcitErr> {
  if expr.len() == 1 {
    let quoted_code = runner::evaluate_expr(&expr[0], scope, file_ns, program_code)?;

    match quoted_code.to_owned() {
      Calcit::List(xs) => {
        if xs.is_empty() {
          return Ok(quoted_code);
        }
        let v = runner::evaluate_expr(&xs[0], scope, file_ns, program_code)?;
        match v {
          Calcit::Macro(_, def_ns, _, args, body) => {
            let mut xs_cloned = xs;
            // mutable operation
            let mut rest_nodes = xs_cloned.slice(1..);
            let check_warnings: &RefCell<Vec<String>> = &RefCell::new(vec![]);
            // println!("macro: {:?} ... {:?}", args, rest_nodes);
            // keep expanding until return value is not a recur
            loop {
              let body_scope = runner::bind_args(&args, &rest_nodes, scope)?;
              let v = runner::evaluate_lines(&body, &body_scope, &def_ns, program_code)?;
              match v {
                Calcit::Recur(rest_code) => {
                  rest_nodes = rest_code;
                }
                _ => {
                  let (resolved, _v) = runner::preprocess::preprocess_expr(&v, &HashSet::new(), file_ns, program_code, check_warnings)?;
                  let warnings = check_warnings.to_owned().into_inner();
                  if !warnings.is_empty() {
                    for message in &warnings {
                      println!("{}", message);
                    }
                  }

                  return Ok(resolved);
                }
              }
            }
          }
          _ => {
            let check_warnings: &RefCell<Vec<String>> = &RefCell::new(vec![]);
            let (resolved, _v) =
              runner::preprocess::preprocess_expr(&quoted_code, &HashSet::new(), file_ns, program_code, check_warnings)?;
            let warnings = check_warnings.to_owned().into_inner();
            if !warnings.is_empty() {
              for message in &warnings {
                println!("{}", message);
              }
            }
            Ok(resolved)
          }
        }
      }
      a => Ok(a),
    }
  } else {
    Err(CalcitErr::use_string(format!(
      "macroexpand expected excaclty 1 argument, got: {:?}",
      expr
    )))
  }
}

pub fn call_try(expr: &CalcitItems, scope: &CalcitScope, file_ns: &str, program_code: &ProgramCodeData) -> Result<Calcit, CalcitErr> {
  if expr.len() == 2 {
    let xs = runner::evaluate_expr(&expr[0], scope, file_ns, program_code);

    match &xs {
      // dirty since only functions being call directly then we become fast
      Ok(v) => Ok(v.to_owned()),
      Err(failure) => {
        let f = runner::evaluate_expr(&expr[1], scope, file_ns, program_code)?;
        let err_data = Calcit::Str(failure.msg.to_owned());
        match f {
          Calcit::Fn(_, def_ns, _, def_scope, args, body) => {
            let values = im::vector![err_data];
            runner::run_fn(&values, &def_scope, &args, &body, &def_ns, program_code)
          }
          Calcit::Proc(proc) => builtins::handle_proc(&proc, &im::vector![err_data]),
          a => Err(CalcitErr::use_string(format!("try expected a function handler, got: {}", a))),
        }
      }
    }
  } else {
    Err(CalcitErr::use_string(format!("try expected 2 arguments, got: {:?}", expr)))
  }
}
