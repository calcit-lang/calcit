use crate::primes::CalcitData;
use crate::primes::CalcitData::*;
use crate::primes::CalcitScope;
use im;

pub fn defn(expr: im::Vector<CalcitData>, scope: CalcitScope) -> Result<CalcitData, String> {
  match (expr.get(0), expr.get(1)) {
    (Some(CalcitSymbol(s, _ns)), Some(CalcitList(xs))) => Ok(CalcitFn(
      s.to_string(),
      nanoid!(),
      scope,
      xs.clone(),
      expr.clone().slice(2..),
    )),
    (Some(a), Some(b)) => Err(format!("invalid args type: {} , {}", a, b)),
    _ => Err(String::from("inefficient arguments for defn")),
  }
}
