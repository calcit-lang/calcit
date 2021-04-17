use crate::primes::CalcitData;
use crate::primes::CalcitData::*;

pub fn binary_add(xs: im::Vector<CalcitData>) -> Result<CalcitData, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(CalcitNumber(a)), Some(CalcitNumber(b))) => Ok(CalcitNumber(a + b)),
    (Some(a), Some(b)) => Err(format!("invalid types for &+: {} {}", a, b)),
    (_, _) if xs.len() != 2 => Err(String::from("&add expected 2 arguments")),
    _ => Err(String::from("invalid arguments")),
  }
}

pub fn is_odd(x: usize) -> bool {
  x & 1 == 1
}
pub fn is_even(x: usize) -> bool {
  x & 1 == 0
}
