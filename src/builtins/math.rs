use crate::primes::{Calcit, CalcitItems};

use crate::util::number::{f64_to_i32, is_integer};

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

pub fn round_ques(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Number(n)) => Ok(Calcit::Bool(is_integer(*n))),
    Some(a) => Err(format!("round? expected a number: {}", a)),
    a => Err(format!("round? expected 1 number: {:?}", a)),
  }
}

pub fn floor(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.floor())),
    Some(a) => Err(format!("rand expected a number: {}", a)),
    a => Err(format!("rand expected 1 number: {:?}", a)),
  }
}

// TODO semantics of Rust and JavaScript are different
pub fn fractional(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n - n.floor())),
    Some(a) => Err(format!("fractional expected a number: {}", a)),
    a => Err(format!("fractional expected 1 number: {:?}", a)),
  }
}

pub fn rem(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Number(base)), Some(Calcit::Number(step))) => match (f64_to_i32(*base), f64_to_i32(*step)) {
      (Ok(a), Ok(b)) => Ok(Calcit::Number((a % b) as f64)),
      (Err(a), _) => Err(a),
      (_, Err(a)) => Err(a),
    },
    (Some(a), Some(b)) => Err(format!("mod expected 2 numbers, got: {:?} {:?}", a, b)),
    (a, b) => Err(format!("mod expected 2 numbers, got: {:?} {:?}", a, b)),
  }
}

pub fn round(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.round())),
    Some(a) => Err(format!("round expected a number: {}", a)),
    a => Err(format!("round expected 1 number: {:?}", a)),
  }
}
pub fn sin(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.sin())),
    Some(a) => Err(format!("sin expected a number: {}", a)),
    a => Err(format!("sin expected 1 number: {:?}", a)),
  }
}
pub fn cos(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.cos())),
    Some(a) => Err(format!("cos expected a number: {}", a)),
    a => Err(format!("cos expected 1 number: {:?}", a)),
  }
}
pub fn pow(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Number(base)), Some(Calcit::Number(step))) => Ok(Calcit::Number(base.powf(*step))),
    (Some(a), Some(b)) => Err(format!("pow expected 2 numbers, got: {:?} {:?}", a, b)),
    (a, b) => Err(format!("pow expected 2 numbers, got: {:?} {:?}", a, b)),
  }
}
pub fn ceil(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.ceil())),
    Some(a) => Err(format!("ceil expected a number: {}", a)),
    a => Err(format!("ceil expected 1 number: {:?}", a)),
  }
}
pub fn sqrt(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.sqrt())),
    Some(a) => Err(format!("sqrt expected a number: {}", a)),
    a => Err(format!("sqrt expected 1 number: {:?}", a)),
  }
}

pub fn bit_shr(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Number(n)), Some(Calcit::Number(m))) => match (f64_to_i32(*n), f64_to_i32(*m)) {
      (Ok(value), Ok(step)) => Ok(Calcit::Number((value >> step) as f64)),
      (Err(e), _) => Err(format!("bit-shr expect int as initial value: {}", e)),
      (_, Err(e)) => Err(format!("bit-shr expect int as step: {}", e)),
    },
    (Some(a), Some(b)) => Err(format!("bit-shr expected 2 numbers, got: {} {}", a, b)),
    (a, b) => Err(format!("bit-shr expected 2 number: {:?} {:?}", a, b)),
  }
}

pub fn bit_shl(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Number(n)), Some(Calcit::Number(m))) => match (f64_to_i32(*n), f64_to_i32(*m)) {
      (Ok(value), Ok(step)) => Ok(Calcit::Number((value << step) as f64)),
      (Err(e), _) => Err(format!("bit-shl expect int as initial value: {}", e)),
      (_, Err(e)) => Err(format!("bit-shl expect int as step: {}", e)),
    },
    (Some(a), Some(b)) => Err(format!("bit-shl expected 2 numbers, got: {} {}", a, b)),
    (a, b) => Err(format!("bit-shl expected 2 number: {:?} {:?}", a, b)),
  }
}
