use crate::primes::CalcitData;
use crate::primes::CalcitData::*;
use crate::primes::CalcitScope;
use crate::program::ProgramCodeData;
use crate::runner;

pub fn defn(
  expr: im::Vector<CalcitData>,
  scope: CalcitScope,
  _file_ns: &str,
  _program: &ProgramCodeData,
) -> Result<CalcitData, String> {
  match (expr.get(0), expr.get(1)) {
    (Some(CalcitSymbol(s, _ns)), Some(CalcitList(xs))) => Ok(CalcitFn(
      s.to_string(),
      nanoid!(),
      scope,
      xs.clone(),
      expr.clone().slice(2..),
    )),
    (Some(a), Some(b)) => Err(format!("invalid args type for defn: {} , {}", a, b)),
    _ => Err(String::from("inefficient arguments for defn")),
  }
}

pub fn defmacro(
  expr: im::Vector<CalcitData>,
  _scope: CalcitScope,
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
      CalcitList(expr)
    )),
  }
}

pub fn quote(
  expr: im::Vector<CalcitData>,
  _scope: CalcitScope,
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
  expr: im::Vector<CalcitData>,
  scope: CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<CalcitData, String> {
  match (expr.get(0), expr.get(1)) {
    _ if expr.len() > 3 => Err(format!("too many nodes for if: {:?}", expr)),
    (Some(cond), Some(true_branch)) => {
      let cond_value = runner::evaluate_expr(cond.clone(), scope.clone(), file_ns, program_code)?;
      match cond_value {
        CalcitNil | CalcitBool(false) => match expr.get(2) {
          Some(false_branch) => {
            runner::evaluate_expr(false_branch.clone(), scope, file_ns, program_code)
          }
          None => Ok(CalcitNil),
        },
        _ => runner::evaluate_expr(true_branch.clone(), scope, file_ns, program_code),
      }
    }
    (None, _) => Err(format!("insufficient nodes for if: {:?}", expr)),
    _ => Err(format!("invalid if form: {:?}", expr)),
  }
}

pub fn eval(
  expr: im::Vector<CalcitData>,
  scope: CalcitScope,
  file_ns: &str,
  program_code: &ProgramCodeData,
) -> Result<CalcitData, String> {
  if expr.len() == 1 {
    let v = runner::evaluate_expr(expr[0].clone(), scope.clone(), file_ns, program_code)?;
    runner::evaluate_expr(v, scope, file_ns, program_code)
  } else {
    Err(format!("unexpected data for evaling: {:?}", expr))
  }
}

/*

pub fn syntax_let(expr: im::Vector<CalcitData>, _scope: CalcitScope,_file_ns: &str, _program: &ProgramCodeData) -> Result<CalcitData, String> {
}

pub fn quasiquote(expr: im::Vector<CalcitData>, _scope: CalcitScope,_file_ns: &str, _program: &ProgramCodeData) -> Result<CalcitData, String> {
}

// TODO macroexpand-all
pub fn macroexpand(expr: im::Vector<CalcitData>, scope: CalcitScope,_file_ns: &str, _program: &ProgramCodeData) -> Result<CalcitData, String> {
}

*/
