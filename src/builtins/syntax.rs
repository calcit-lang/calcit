use crate::primes::CalcitData;
use crate::primes::CalcitData::*;
use crate::primes::{CalcitScope, FnEvalFn};
use nanoid;

pub fn syntaxDefn(
  expr: Vec<CalcitData>,
  scope: CalcitScope,
  interpret: FnEvalFn,
) -> Result<CalcitData, String> {
  let f = |xs: Vec<CalcitData>, a: CalcitScope, i: FnEvalFn| -> Result<CalcitData, String> {
    Ok(CalcitNil)
  };
  Ok(CalcitSyntax(String::from("TODO"), f))
}
