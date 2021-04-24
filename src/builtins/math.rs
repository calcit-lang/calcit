use crate::primes::{Calcit, CalcitItems};
use rand::prelude::*;

pub fn f64_to_usize(f: f64) -> Result<usize, String> {
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

pub fn f64_to_i32(f: f64) -> Result<i32, String> {
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

pub fn is_integer(x: f64) -> bool {
  x.fract() == 0.0
}

fn rand_number(n: f64) -> f64 {
  let mut rng = rand::thread_rng();
  let y: f64 = rng.gen(); // generates a float between 0 and 1
  y * n
}

pub fn rand(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (None, None) => Ok(Calcit::Number(rand_number(100.0))),
    (Some(Calcit::Number(n)), None) => Ok(Calcit::Number(rand_number(*n))),
    (Some(Calcit::Number(from)), Some(Calcit::Number(to))) => {
      let delta = to - from;

      Ok(Calcit::Number(from + rand_number(delta)))
    }
    (a, b) => Err(format!("rand expected 0~2 numbers: {:?} {:?}", a, b)),
  }
}

pub fn rand_int(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (None, None) => Ok(Calcit::Number(rand_number(100.0).floor())),
    (Some(Calcit::Number(n)), None) => Ok(Calcit::Number(rand_number(*n).floor())),
    (Some(Calcit::Number(from)), Some(Calcit::Number(to))) => {
      let delta = to - from;

      Ok(Calcit::Number((from + rand_number(delta)).floor()))
    }
    (a, b) => Err(format!("rand expected 0~2 numbers: {:?} {:?}", a, b)),
  }
}

pub fn floor(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.floor())),
    Some(a) => Err(format!("rand expected a number: {}", a)),
    a => Err(format!("rand expected 1 number: {:?}", a)),
  }
}

// TODO mod or rem
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
