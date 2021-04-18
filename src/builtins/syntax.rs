use crate::primes::CalcitData::*;
use crate::primes::{CalcitData, CalcitItems, CalcitScope};
use crate::program::ProgramCodeData;
use crate::runner;

pub fn defn(
  expr: &CalcitItems,
  scope: &CalcitScope,
  _file_ns: &str,
  _program: &ProgramCodeData,
) -> Result<CalcitData, String> {
  match (expr.get(0), expr.get(1)) {
    (Some(CalcitSymbol(s, _ns)), Some(CalcitList(xs))) => Ok(CalcitFn(
      s.to_string(),
      nanoid!(),
      scope.clone(),
      xs.clone(),
      expr.clone().slice(2..),
    )),
    (Some(a), Some(b)) => Err(format!("invalid args type for defn: {} , {}", a, b)),
    _ => Err(String::from("inefficient arguments for defn")),
  }
}

pub fn defmacro(
  expr: &CalcitItems,
  _scope: &CalcitScope,
  _file_ns: &str,
  _program: &ProgramCodeData,
) -> Result<CalcitData, String> {
  match (expr.get(0), expr.get(1)) {
    (Some(CalcitSymbol(s, _ns)), Some(CalcitList(xs))) => Ok(CalcitMacro(
      s.to_string(),
      nanoid!(),
      xs.clone(),
      expr.clone().slice(2..),
    )),
    (Some(a), Some(b)) => Err(format!("invalid structure for defmacro: {} {}", a, b)),
    _ => Err(format!(
      "invalid structure for defmacro: {}",
      CalcitList(expr.clone())
    )),
  }
}

pub fn quote(
  expr: &CalcitItems,
  _scope: &CalcitScope,
  _file_ns: &str,
  _program: &ProgramCodeData,
) -> Result<CalcitData, String> {
  if expr.len() == 1 {
    Ok(expr[0].clone())
  } else {
    Err(format!("unexpected data for quote: {:?}", expr))
  }
}

pub fn syntax_if(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<CalcitData, String> {
  match (expr.get(0), expr.get(1)) {
    _ if expr.len() > 3 => Err(format!("too many nodes for if: {:?}", expr)),
    (Some(cond), Some(true_branch)) => {
      let cond_value = runner::evaluate_expr(cond, scope, file_ns, program_code)?;
      match cond_value {
        CalcitNil | CalcitBool(false) => match expr.get(2) {
          Some(false_branch) => runner::evaluate_expr(false_branch, scope, file_ns, program_code),
          None => Ok(CalcitNil),
        },
        _ => runner::evaluate_expr(true_branch, scope, file_ns, program_code),
      }
    }
    (None, _) => Err(format!("insufficient nodes for if: {:?}", expr)),
    _ => Err(format!("invalid if form: {:?}", expr)),
  }
}

pub fn eval(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<CalcitData, String> {
  if expr.len() == 1 {
    let v = runner::evaluate_expr(&expr[0], scope, file_ns, program_code)?;
    runner::evaluate_expr(&v, scope, file_ns, program_code)
  } else {
    Err(format!("unexpected data for evaling: {:?}", expr))
  }
}

pub fn syntax_let(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<CalcitData, String> {
  match expr.get(0) {
    Some(CalcitNil) => {
      runner::evaluate_lines(&expr.clone().slice(1..), scope, file_ns, program_code)
    }
    Some(CalcitList(xs)) if xs.len() == 2 => {
      let mut body_scope = scope.clone();
      match (&xs[0], &xs[1]) {
        (CalcitSymbol(s, _ns), ys) => {
          let value = runner::evaluate_expr(ys, scope, file_ns, program_code)?;
          body_scope.insert(s.to_string(), value);
        }
        (a, _) => return Err(format!("invalid binding name: {}", a)),
      }
      runner::evaluate_lines(&expr.clone().slice(1..), &body_scope, file_ns, program_code)
    }
    Some(CalcitList(xs)) => Err(format!("invalid length: {:?}", xs)),
    Some(_) => Err(format!("invalid node for &let: {:?}", expr)),
    None => Err(String::from("&let expected a pair or a nil")),
  }
}

/// foldl using syntax for performance, theoretically it's not
pub fn foldl(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<CalcitData, String> {
  if expr.len() == 3 {
    let xs = runner::evaluate_expr(&expr[0], scope, file_ns, program_code)?;
    let acc = runner::evaluate_expr(&expr[1], scope, file_ns, program_code)?;
    let f = runner::evaluate_expr(&expr[2], scope, file_ns, program_code)?;
    match (xs.clone(), f.clone()) {
      (CalcitList(xs), CalcitFn(..)) | (CalcitList(xs), CalcitProc(..)) => {
        let mut ret = acc;
        for x in xs {
          let code = CalcitList(im::vector![f.clone(), ret.clone(), x.clone()]);
          ret = runner::evaluate_expr(&code, scope, file_ns, program_code)?;
        }
        Ok(ret)
      }
      (_, _) => Err(format!(
        "foldl expected list and function, got: {} {}",
        xs, f
      )),
    }
  } else {
    Err(format!("foldl expected 3 arguments, got: {:?}", expr))
  }
}

pub fn quasiquote(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<CalcitData, String> {
  // println!("replacing: {:?}", expr);
  match expr.get(0) {
    None => Err(String::from("quasiquote expected a node")),
    Some(code) => {
      let ret = replace_code(code, scope, file_ns, program_code)?;
      match ret.get(0) {
        Some(v) => {
          // println!("replace result: {:?}", v);
          Ok(v.clone())
        }
        None => Err(String::from("missing quote expr")),
      }
    }
  }
}

fn replace_code(
  c: &CalcitData,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<CalcitItems, String> {
  match c {
    CalcitList(ys) => match (ys.get(0), ys.get(1)) {
      (Some(CalcitSymbol(sym, _)), Some(expr)) if sym == "~" => {
        let value = runner::evaluate_expr(expr, scope, file_ns, program_code)?;
        Ok(im::vector![value])
      }
      (Some(CalcitSymbol(sym, _)), Some(expr)) if sym == "~@" => {
        let ret = runner::evaluate_expr(expr, scope, file_ns, program_code)?;
        match ret {
          CalcitList(zs) => Ok(zs),
          _ => Err(format!("unknown result from unquite-slice: {}", ret)),
        }
      }
      (_, _) => {
        let mut ret = im::vector![];
        for y in ys {
          let pieces = replace_code(y, scope, file_ns, program_code)?;
          for piece in pieces {
            ret.push_back(piece);
          }
        }
        Ok(im::vector![CalcitList(ret)])
      }
    },
    _ => Ok(im::vector![c.clone()]),
  }
}

pub fn macroexpand(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<CalcitData, String> {
  if expr.len() == 1 {
    let quoted_code = runner::evaluate_expr(&expr[0], scope, file_ns, program_code)?;

    match quoted_code.clone() {
      CalcitList(xs) => {
        if xs.is_empty() {
          return Ok(quoted_code);
        }
        let v = runner::evaluate_expr(&xs[0], scope, file_ns, program_code)?;
        match v {
          CalcitMacro(_, _, args, body) => {
            let mut xs_cloned = xs;
            // mutable operation
            let mut rest_nodes = xs_cloned.slice(1..);
            // println!("macro: {:?} ... {:?}", args, rest_nodes);
            // keep expanding until return value is not a recur
            loop {
              let body_scope = runner::bind_args(&args, &rest_nodes, scope)?;
              let v = runner::evaluate_lines(&body, &body_scope, file_ns, program_code)?;
              match v {
                CalcitRecur(rest_code) => {
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
    Err(format!(
      "macroexpand expected excaclty 1 argument, got: {:?}",
      expr
    ))
  }
}

pub fn macroexpand_1(
  expr: &CalcitItems,
  scope: &CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<CalcitData, String> {
  if expr.len() == 1 {
    let quoted_code = runner::evaluate_expr(&expr[0], scope, file_ns, program_code)?;
    // println!("quoted: {}", quoted_code);
    match quoted_code.clone() {
      CalcitList(xs) => {
        if xs.is_empty() {
          return Ok(quoted_code);
        }
        let v = runner::evaluate_expr(&xs[0], scope, file_ns, program_code)?;
        match v {
          CalcitMacro(_, _, args, body) => {
            let mut xs_cloned = xs;
            let body_scope = runner::bind_args(&args, &xs_cloned.slice(1..), scope)?;
            runner::evaluate_lines(&body, &body_scope, file_ns, program_code)
          }
          _ => Ok(quoted_code),
        }
      }
      a => Ok(a),
    }
  } else {
    Err(format!(
      "macroexpand expected excaclty 1 argument, got: {:?}",
      expr
    ))
  }
}
