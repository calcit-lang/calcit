use crate::calcit::{Calcit, CalcitErr};

use crate::util::number::{f64_to_i32, is_integer};

pub fn binary_add(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(a)), Some(Calcit::Number(b))) => Ok(Calcit::Number(a + b)),
    (Some(a), Some(b)) => CalcitErr::err_str(format!("invalid types for &+: {a} {b}")),
    (_, _) if xs.len() != 2 => CalcitErr::err_str("&+ expected 2 arguments"),
    _ => CalcitErr::err_str("invalid arguments"),
  }
}

pub fn binary_minus(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(a)), Some(Calcit::Number(b))) => Ok(Calcit::Number(a - b)),
    (Some(a), Some(b)) => CalcitErr::err_str(format!("invalid types for &-: {a} {b}")),
    (_, _) if xs.len() != 2 => CalcitErr::err_str("&- expected 2 arguments"),
    _ => CalcitErr::err_str("invalid arguments"),
  }
}

pub fn binary_multiply(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(a)), Some(Calcit::Number(b))) => Ok(Calcit::Number(a * b)),
    (Some(a), Some(b)) => CalcitErr::err_str(format!("invalid types for &*: {a} {b}")),
    (_, _) if xs.len() != 2 => CalcitErr::err_str("&* expected 2 arguments"),
    _ => CalcitErr::err_str("invalid arguments"),
  }
}

pub fn binary_divide(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(a)), Some(Calcit::Number(b))) => Ok(Calcit::Number(a / b)),
    (Some(a), Some(b)) => CalcitErr::err_str(format!("invalid types for &/: {a} {b}")),
    (_, _) if xs.len() != 2 => CalcitErr::err_str("&/ expected 2 arguments"),
    _ => CalcitErr::err_str("invalid arguments"),
  }
}

pub fn round_ques(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => Ok(Calcit::Bool(is_integer(*n))),
    Some(a) => CalcitErr::err_str(format!("round? expected a number: {a}")),
    a => CalcitErr::err_str(format!("round? expected 1 number: {a:?}")),
  }
}

pub fn floor(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.floor())),
    Some(a) => CalcitErr::err_str(format!("rand expected a number: {a}")),
    a => CalcitErr::err_str(format!("rand expected 1 number: {a:?}")),
  }
}

// TODO semantics of Rust and JavaScript are different
pub fn fractional(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n - n.floor())),
    Some(a) => CalcitErr::err_str(format!("fractional expected a number: {a}")),
    a => CalcitErr::err_str(format!("fractional expected 1 number: {a:?}")),
  }
}

pub fn rem(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(base)), Some(Calcit::Number(step))) => match (f64_to_i32(*base), f64_to_i32(*step)) {
      (Ok(a), Ok(b)) => Ok(Calcit::Number((a % b) as f64)),
      (Err(a), _) => CalcitErr::err_str(a),
      (_, Err(a)) => CalcitErr::err_str(a),
    },
    (Some(a), Some(b)) => CalcitErr::err_str(format!("mod expected 2 numbers, got: {a:?} {b:?}")),
    (a, b) => CalcitErr::err_str(format!("mod expected 2 numbers, got: {a:?} {b:?}")),
  }
}

pub fn round(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.round())),
    Some(a) => CalcitErr::err_str(format!("round expected a number: {a}")),
    a => CalcitErr::err_str(format!("round expected 1 number: {a:?}")),
  }
}
pub fn sin(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.sin())),
    Some(a) => CalcitErr::err_str(format!("sin expected a number: {a}")),
    a => CalcitErr::err_str(format!("sin expected 1 number: {a:?}")),
  }
}
pub fn cos(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.cos())),
    Some(a) => CalcitErr::err_str(format!("cos expected a number: {a}")),
    a => CalcitErr::err_str(format!("cos expected 1 number: {a:?}")),
  }
}
pub fn pow(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(base)), Some(Calcit::Number(step))) => Ok(Calcit::Number(base.powf(*step))),
    (Some(a), Some(b)) => CalcitErr::err_str(format!("pow expected 2 numbers, got: {a:?} {b:?}")),
    (a, b) => CalcitErr::err_str(format!("pow expected 2 numbers, got: {a:?} {b:?}")),
  }
}
pub fn ceil(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.ceil())),
    Some(a) => CalcitErr::err_str(format!("ceil expected a number: {a}")),
    a => CalcitErr::err_str(format!("ceil expected 1 number: {a:?}")),
  }
}
pub fn sqrt(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.sqrt())),
    Some(a) => CalcitErr::err_str(format!("sqrt expected a number: {a}")),
    a => CalcitErr::err_str(format!("sqrt expected 1 number: {a:?}")),
  }
}

pub fn bit_shr(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(n)), Some(Calcit::Number(m))) => match (f64_to_i32(*n), f64_to_i32(*m)) {
      (Ok(value), Ok(step)) => Ok(Calcit::Number((value >> step) as f64)),
      (Err(e), _) => CalcitErr::err_str(format!("bit-shr expect int as initial value: {e}")),
      (_, Err(e)) => CalcitErr::err_str(format!("bit-shr expect int as step: {e}")),
    },
    (Some(a), Some(b)) => CalcitErr::err_str(format!("bit-shr expected 2 numbers, got: {a} {b}")),
    (a, b) => CalcitErr::err_str(format!("bit-shr expected 2 number: {a:?} {b:?}")),
  }
}

pub fn bit_shl(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(n)), Some(Calcit::Number(m))) => match (f64_to_i32(*n), f64_to_i32(*m)) {
      (Ok(value), Ok(step)) => Ok(Calcit::Number((value << step) as f64)),
      (Err(e), _) => CalcitErr::err_str(format!("bit-shl expect int as initial value: {e}")),
      (_, Err(e)) => CalcitErr::err_str(format!("bit-shl expect int as step: {e}")),
    },
    (Some(a), Some(b)) => CalcitErr::err_str(format!("bit-shl expected 2 numbers, got: {a} {b}")),
    (a, b) => CalcitErr::err_str(format!("bit-shl expected 2 number: {a:?} {b:?}")),
  }
}

pub fn bit_and(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(n)), Some(Calcit::Number(m))) => match (f64_to_i32(*n), f64_to_i32(*m)) {
      (Ok(value), Ok(step)) => Ok(Calcit::Number((value & step) as f64)),
      (Err(e), _) => CalcitErr::err_str(format!("bit-and expect int as initial value: {e}")),
      (_, Err(e)) => CalcitErr::err_str(format!("bit-and expect int as step: {e}")),
    },
    (Some(a), Some(b)) => CalcitErr::err_str(format!("bit-and expected 2 numbers, got: {a} {b}")),
    (a, b) => CalcitErr::err_str(format!("bit-and expected 2 number: {a:?} {b:?}")),
  }
}

pub fn bit_or(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(n)), Some(Calcit::Number(m))) => match (f64_to_i32(*n), f64_to_i32(*m)) {
      (Ok(value), Ok(step)) => Ok(Calcit::Number((value | step) as f64)),
      (Err(e), _) => CalcitErr::err_str(format!("bit-or expect int as initial value: {e}")),
      (_, Err(e)) => CalcitErr::err_str(format!("bit-or expect int as step: {e}")),
    },
    (Some(a), Some(b)) => CalcitErr::err_str(format!("bit-or expected 2 numbers, got: {a} {b}")),
    (a, b) => CalcitErr::err_str(format!("bit-or expected 2 number: {a:?} {b:?}")),
  }
}

pub fn bit_xor(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(n)), Some(Calcit::Number(m))) => match (f64_to_i32(*n), f64_to_i32(*m)) {
      (Ok(value), Ok(step)) => Ok(Calcit::Number((value ^ step) as f64)),
      (Err(e), _) => CalcitErr::err_str(format!("bit-xor expect int as initial value: {e}")),
      (_, Err(e)) => CalcitErr::err_str(format!("bit-xor expect int as step: {e}")),
    },
    (Some(a), Some(b)) => CalcitErr::err_str(format!("bit-xor expected 2 numbers, got: {a} {b}")),
    (a, b) => CalcitErr::err_str(format!("bit-xor expected 2 number: {a:?} {b:?}")),
  }
}

pub fn bit_not(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => match f64_to_i32(*n) {
      Ok(value) => Ok(Calcit::Number(!value as f64)),
      Err(e) => CalcitErr::err_str(format!("bit-not expect int as initial value: {e}")),
    },
    Some(a) => CalcitErr::err_str(format!("bit-not expected a number: {a}")),
    a => CalcitErr::err_str(format!("bit-not expected 1 number: {a:?}")),
  }
}
