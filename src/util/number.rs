#[allow(dead_code)]
pub fn is_odd(x: usize) -> bool {
  x & 1 == 1
}
pub fn is_even(x: usize) -> bool {
  x & 1 == 0
}

pub fn is_integer(x: f64) -> bool {
  x.fract().abs() <= f64::EPSILON
}

fn is_float_integer(f: f64) -> bool {
  f.fract().abs() <= f64::EPSILON
}

pub fn f64_to_usize(f: f64) -> Result<usize, String> {
  if is_float_integer(f) {
    if f >= 0.0 {
      Ok(f as usize)
    } else {
      Err(format!("usize expected a positive number, but got: {f}"))
    }
  } else {
    Err(format!("cannot extract usize from float: {f}"))
  }
}

pub fn f64_to_i32(f: f64) -> Result<i32, String> {
  if is_float_integer(f) {
    Ok(f as i32)
  } else {
    Err(format!("cannot extract int from float: {f}"))
  }
}
