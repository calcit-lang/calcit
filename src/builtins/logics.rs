use crate::primes::{Calcit, CalcitErr, CalcitItems};

pub fn binary_equal(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(a), Some(b)) => Ok(Calcit::Bool(a == b)),
    (_, _) => CalcitErr::err_str(format!("&= expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn binary_less(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(a), Some(b)) => Ok(Calcit::Bool(a < b)),
    (_, _) => CalcitErr::err_str(format!("&< expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn binary_greater(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(a), Some(b)) => Ok(Calcit::Bool(a > b)),
    (_, _) => CalcitErr::err_str(format!("&> expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn not(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_str(format!("not expected bool or nil, got: {:?}", xs));
  }
  match &xs[0] {
    Calcit::Nil => Ok(Calcit::Bool(true)),
    Calcit::Bool(b) => Ok(Calcit::Bool(!b)),
    a => CalcitErr::err_str(format!("not expected bool or nil, got: {}", a)),
  }
}
