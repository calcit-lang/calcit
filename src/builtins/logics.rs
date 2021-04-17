use crate::primes::{CalcitData, CalcitData::*, CalcitItems};

pub fn binary_equal(xs: &CalcitItems) -> Result<CalcitData, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(a), Some(b)) => Ok(CalcitBool(a == b)),
    (_, _) => Err(format!("&= expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn binary_less(xs: &CalcitItems) -> Result<CalcitData, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(a), Some(b)) => Ok(CalcitBool(a < b)),
    (_, _) => Err(format!("&= expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn binary_greater(xs: &CalcitItems) -> Result<CalcitData, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(a), Some(b)) => Ok(CalcitBool(a > b)),
    (_, _) => Err(format!("&= expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn not(xs: &CalcitItems) -> Result<CalcitData, String> {
  match xs.get(0) {
    Some(CalcitNil) => Ok(CalcitBool(true)),
    Some(CalcitBool(b)) => Ok(CalcitBool(!b)),
    Some(a) => Err(format!("&= expected bool or nil, got: {}", a)),
    None => Err(String::from("&= expected 2 arguments, got nothing")),
  }
}
