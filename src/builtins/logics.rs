use crate::primes::{Calcit, CalcitItems};

pub fn binary_equal(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(a), Some(b)) => Ok(Calcit::Bool(a == b)),
    (_, _) => Err(format!("&= expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn binary_less(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(a), Some(b)) => Ok(Calcit::Bool(a < b)),
    (_, _) => Err(format!("&= expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn binary_greater(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(a), Some(b)) => Ok(Calcit::Bool(a > b)),
    (_, _) => Err(format!("&= expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn not(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Nil) => Ok(Calcit::Bool(true)),
    Some(Calcit::Bool(b)) => Ok(Calcit::Bool(!b)),
    Some(a) => Err(format!("&= expected bool or nil, got: {}", a)),
    None => Err(String::from("&= expected 2 arguments, got nothing")),
  }
}
