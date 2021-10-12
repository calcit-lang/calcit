use crate::primes::{Calcit, CalcitErr, CalcitItems};

pub fn binary_equal(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(a), Some(b)) => Ok(Calcit::Bool(a == b)),
    (_, _) => Err(CalcitErr::use_string(format!("&= expected 2 arguments, got: {:?}", xs))),
  }
}

pub fn binary_less(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(a), Some(b)) => Ok(Calcit::Bool(a < b)),
    (_, _) => Err(CalcitErr::use_string(format!("&< expected 2 arguments, got: {:?}", xs))),
  }
}

pub fn binary_greater(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(a), Some(b)) => Ok(Calcit::Bool(a > b)),
    (_, _) => Err(CalcitErr::use_string(format!("&> expected 2 arguments, got: {:?}", xs))),
  }
}

pub fn not(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Nil) => Ok(Calcit::Bool(true)),
    Some(Calcit::Bool(b)) => Ok(Calcit::Bool(!b)),
    Some(a) => Err(CalcitErr::use_string(format!("not expected bool or nil, got: {}", a))),
    None => Err(CalcitErr::use_str("not expected 1 argument, got nothing")),
  }
}
