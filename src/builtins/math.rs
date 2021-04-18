use crate::primes::CalcitData::*;
use crate::primes::{CalcitData, CalcitItems};

pub fn binary_add(xs: &CalcitItems) -> Result<CalcitData, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(CalcitNumber(a)), Some(CalcitNumber(b))) => Ok(CalcitNumber(a + b)),
    (Some(a), Some(b)) => Err(format!("invalid types for &+: {} {}", a, b)),
    (_, _) if xs.len() != 2 => Err(String::from("&+ expected 2 arguments")),
    _ => Err(String::from("invalid arguments")),
  }
}

pub fn binary_minus(xs: &CalcitItems) -> Result<CalcitData, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(CalcitNumber(a)), Some(CalcitNumber(b))) => Ok(CalcitNumber(a - b)),
    (Some(a), Some(b)) => Err(format!("invalid types for &-: {} {}", a, b)),
    (_, _) if xs.len() != 2 => Err(String::from("&- expected 2 arguments")),
    _ => Err(String::from("invalid arguments")),
  }
}

pub fn binary_multiply(xs: &CalcitItems) -> Result<CalcitData, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(CalcitNumber(a)), Some(CalcitNumber(b))) => Ok(CalcitNumber(a - b)),
    (Some(a), Some(b)) => Err(format!("invalid types for &*: {} {}", a, b)),
    (_, _) if xs.len() != 2 => Err(String::from("&* expected 2 arguments")),
    _ => Err(String::from("invalid arguments")),
  }
}

pub fn binary_divide(xs: &CalcitItems) -> Result<CalcitData, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(CalcitNumber(a)), Some(CalcitNumber(b))) => Ok(CalcitNumber(a - b)),
    (Some(a), Some(b)) => Err(format!("invalid types for &/: {} {}", a, b)),
    (_, _) if xs.len() != 2 => Err(String::from("&/ expected 2 arguments")),
    _ => Err(String::from("invalid arguments")),
  }
}

pub fn is_odd(x: usize) -> bool {
  x & 1 == 1
}
pub fn is_even(x: usize) -> bool {
  x & 1 == 0
}

pub fn is_integer(x: f32) -> bool {
  x.fract() == 0.0
}
