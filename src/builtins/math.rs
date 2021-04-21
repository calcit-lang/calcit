use crate::primes::{Calcit, CalcitItems};

pub fn f32_to_usize(f: f32) -> Result<usize, String> {
  if f.fract() == 0.0 {
    if f >= 0.0 {
      Ok(f as usize)
    } else {
      Err(format!("usize expected a positive number, but got: {}", f))
    }
  } else {
    Err(format!("cannot extract usize from float: {}", f))
  }
}

pub fn f32_to_i32(f: f32) -> Result<i32, String> {
  if f.fract() == 0.0 {
    Ok(f as i32)
  } else {
    Err(format!("cannot extract int from float: {}", f))
  }
}

pub fn binary_add(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Number(a)), Some(Calcit::Number(b))) => Ok(Calcit::Number(a + b)),
    (Some(a), Some(b)) => Err(format!("invalid types for &+: {} {}", a, b)),
    (_, _) if xs.len() != 2 => Err(String::from("&+ expected 2 arguments")),
    _ => Err(String::from("invalid arguments")),
  }
}

pub fn binary_minus(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Number(a)), Some(Calcit::Number(b))) => Ok(Calcit::Number(a - b)),
    (Some(a), Some(b)) => Err(format!("invalid types for &-: {} {}", a, b)),
    (_, _) if xs.len() != 2 => Err(String::from("&- expected 2 arguments")),
    _ => Err(String::from("invalid arguments")),
  }
}

pub fn binary_multiply(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Number(a)), Some(Calcit::Number(b))) => Ok(Calcit::Number(a * b)),
    (Some(a), Some(b)) => Err(format!("invalid types for &*: {} {}", a, b)),
    (_, _) if xs.len() != 2 => Err(String::from("&* expected 2 arguments")),
    _ => Err(String::from("invalid arguments")),
  }
}

pub fn binary_divide(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Number(a)), Some(Calcit::Number(b))) => Ok(Calcit::Number(a / b)),
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
