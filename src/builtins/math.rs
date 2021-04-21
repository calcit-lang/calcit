use crate::primes;
use crate::primes::{Calcit, CalcitItems};
use rand::prelude::*;

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

fn rand_number(n: f32) -> f32 {
  let mut rng = rand::thread_rng();
  let y: f32 = rng.gen(); // generates a float between 0 and 1
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
