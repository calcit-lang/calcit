use rand::prelude::*;

pub fn is_odd(x: usize) -> bool {
  x & 1 == 1
}
pub fn is_even(x: usize) -> bool {
  x & 1 == 0
}

pub fn is_integer(x: f64) -> bool {
  x.fract() == 0.0
}

pub fn rand_number(n: f64) -> f64 {
  let mut rng = rand::thread_rng();
  let y: f64 = rng.gen(); // generates a float between 0 and 1
  y * n
}

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
