use crate::calcit::{Calcit, CalcitErr, CalcitErrKind};

use crate::util::number::{f64_to_i32, is_integer};

pub fn binary_add(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(a)), Some(Calcit::Number(b))) => Ok(Calcit::Number(a + b)),
    (Some(a), Some(b)) => CalcitErr::err_str(CalcitErrKind::Type, format!("&+ expected 2 numbers, but received: {a} {b}")),
    (_, _) if xs.len() != 2 => CalcitErr::err_str(CalcitErrKind::Arity, "&+ expected 2 arguments, but received none"),
    _ => CalcitErr::err_str(CalcitErrKind::Arity, "&+ received invalid arguments"),
  }
}

pub fn binary_minus(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(a)), Some(Calcit::Number(b))) => Ok(Calcit::Number(a - b)),
    (Some(a), Some(b)) => CalcitErr::err_str(CalcitErrKind::Type, format!("&- expected 2 numbers, but received: {a} {b}")),
    (_, _) if xs.len() != 2 => CalcitErr::err_str(CalcitErrKind::Arity, "&- expected 2 arguments, but received none"),
    _ => CalcitErr::err_str(CalcitErrKind::Arity, "&- received invalid arguments"),
  }
}

pub fn binary_multiply(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(a)), Some(Calcit::Number(b))) => Ok(Calcit::Number(a * b)),
    (Some(a), Some(b)) => CalcitErr::err_str(CalcitErrKind::Type, format!("&* expected 2 numbers, but received: {a} {b}")),
    (_, _) if xs.len() != 2 => CalcitErr::err_str(CalcitErrKind::Arity, "&* expected 2 arguments, but received none"),
    _ => CalcitErr::err_str(CalcitErrKind::Arity, "&* received invalid arguments"),
  }
}

pub fn binary_divide(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(a)), Some(Calcit::Number(b))) => Ok(Calcit::Number(a / b)),
    (Some(a), Some(b)) => CalcitErr::err_str(CalcitErrKind::Type, format!("&/ expected 2 numbers, but received: {a} {b}")),
    (_, _) if xs.len() != 2 => CalcitErr::err_str(CalcitErrKind::Arity, "&/ expected 2 arguments, but received none"),
    _ => CalcitErr::err_str(CalcitErrKind::Arity, "&/ received invalid arguments"),
  }
}

pub fn round_ques(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => Ok(Calcit::Bool(is_integer(*n))),
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("&math:round? expected a number, but received: {a}")),
    a => CalcitErr::err_str(CalcitErrKind::Arity, format!("&math:round? expected 1 number, but received: {a:?}")),
  }
}

pub fn floor(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.floor())),
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("&math:floor expected a number, but received: {a}")),
    a => CalcitErr::err_str(CalcitErrKind::Arity, format!("&math:floor expected 1 number, but received: {a:?}")),
  }
}

// TODO semantics of Rust and JavaScript are different
pub fn fractional(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n - n.floor())),
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("&math:fract expected a number, but received: {a}")),
    a => CalcitErr::err_str(CalcitErrKind::Arity, format!("&math:fract expected 1 number, but received: {a:?}")),
  }
}

pub fn rem(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(base)), Some(Calcit::Number(step))) => match (f64_to_i32(*base), f64_to_i32(*step)) {
      (Ok(a), Ok(b)) => Ok(Calcit::Number((a % b) as f64)),
      (Err(a), _) => CalcitErr::err_str(CalcitErrKind::Type, a),
      (_, Err(a)) => CalcitErr::err_str(CalcitErrKind::Type, a),
    },
    (Some(a), Some(b)) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&math:rem expected 2 numbers, but received: {a:?} {b:?}"),
    ),
    (a, b) => CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&math:rem expected 2 numbers, but received: {a:?} {b:?}"),
    ),
  }
}

pub fn round(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.round())),
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("&math:round expected a number, but received: {a}")),
    a => CalcitErr::err_str(CalcitErrKind::Arity, format!("&math:round expected 1 number, but received: {a:?}")),
  }
}
pub fn sin(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.sin())),
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("&math:sin expected a number, but received: {a}")),
    a => CalcitErr::err_str(CalcitErrKind::Arity, format!("&math:sin expected 1 number, but received: {a:?}")),
  }
}
pub fn cos(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.cos())),
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("&math:cos expected a number, but received: {a}")),
    a => CalcitErr::err_str(CalcitErrKind::Arity, format!("&math:cos expected 1 number, but received: {a:?}")),
  }
}
pub fn pow(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(base)), Some(Calcit::Number(step))) => Ok(Calcit::Number(base.powf(*step))),
    (Some(a), Some(b)) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&math:pow expected 2 numbers, but received: {a:?} {b:?}"),
    ),
    (a, b) => CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&math:pow expected 2 numbers, but received: {a:?} {b:?}"),
    ),
  }
}
pub fn ceil(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.ceil())),
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("&math:ceil expected a number, but received: {a}")),
    a => CalcitErr::err_str(CalcitErrKind::Arity, format!("&math:ceil expected 1 number, but received: {a:?}")),
  }
}
pub fn sqrt(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.sqrt())),
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("&math:sqrt expected a number, but received: {a}")),
    a => CalcitErr::err_str(CalcitErrKind::Arity, format!("&math:sqrt expected 1 number, but received: {a:?}")),
  }
}

pub fn bit_shr(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(n)), Some(Calcit::Number(m))) => match (f64_to_i32(*n), f64_to_i32(*m)) {
      (Ok(value), Ok(step)) => Ok(Calcit::Number((value >> step) as f64)),
      (Err(e), _) => CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&math:bit-shr expected an integer for initial value, but received: {e}"),
      ),
      (_, Err(e)) => CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&math:bit-shr expected an integer for step, but received: {e}"),
      ),
    },
    (Some(a), Some(b)) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&math:bit-shr expected 2 numbers, but received: {a} {b}"),
    ),
    (a, b) => CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&math:bit-shr expected 2 numbers, but received: {a:?} {b:?}"),
    ),
  }
}

pub fn bit_shl(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(n)), Some(Calcit::Number(m))) => match (f64_to_i32(*n), f64_to_i32(*m)) {
      (Ok(value), Ok(step)) => Ok(Calcit::Number((value << step) as f64)),
      (Err(e), _) => CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&math:bit-shl expected an integer for initial value, but received: {e}"),
      ),
      (_, Err(e)) => CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&math:bit-shl expected an integer for step, but received: {e}"),
      ),
    },
    (Some(a), Some(b)) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&math:bit-shl expected 2 numbers, but received: {a} {b}"),
    ),
    (a, b) => CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&math:bit-shl expected 2 numbers, but received: {a:?} {b:?}"),
    ),
  }
}

pub fn bit_and(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(n)), Some(Calcit::Number(m))) => match (f64_to_i32(*n), f64_to_i32(*m)) {
      (Ok(value), Ok(step)) => Ok(Calcit::Number((value & step) as f64)),
      (Err(e), _) => CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&math:bit-and expected an integer for initial value, but received: {e}"),
      ),
      (_, Err(e)) => CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&math:bit-and expected an integer for step, but received: {e}"),
      ),
    },
    (Some(a), Some(b)) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&math:bit-and expected 2 numbers, but received: {a} {b}"),
    ),
    (a, b) => CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&math:bit-and expected 2 numbers, but received: {a:?} {b:?}"),
    ),
  }
}

pub fn bit_or(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(n)), Some(Calcit::Number(m))) => match (f64_to_i32(*n), f64_to_i32(*m)) {
      (Ok(value), Ok(step)) => Ok(Calcit::Number((value | step) as f64)),
      (Err(e), _) => CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&math:bit-or expected an integer for initial value, but received: {e}"),
      ),
      (_, Err(e)) => CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&math:bit-or expected an integer for step, but received: {e}"),
      ),
    },
    (Some(a), Some(b)) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&math:bit-or expected 2 numbers, but received: {a} {b}"),
    ),
    (a, b) => CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&math:bit-or expected 2 numbers, but received: {a:?} {b:?}"),
    ),
  }
}

pub fn bit_xor(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(n)), Some(Calcit::Number(m))) => match (f64_to_i32(*n), f64_to_i32(*m)) {
      (Ok(value), Ok(step)) => Ok(Calcit::Number((value ^ step) as f64)),
      (Err(e), _) => CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&math:bit-xor expected an integer for initial value, but received: {e}"),
      ),
      (_, Err(e)) => CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&math:bit-xor expected an integer for step, but received: {e}"),
      ),
    },
    (Some(a), Some(b)) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&math:bit-xor expected 2 numbers, but received: {a} {b}"),
    ),
    (a, b) => CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&math:bit-xor expected 2 numbers, but received: {a:?} {b:?}"),
    ),
  }
}

pub fn bit_not(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => match f64_to_i32(*n) {
      Ok(value) => Ok(Calcit::Number(!value as f64)),
      Err(e) => CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&math:bit-not expected an integer for initial value, but received: {e}"),
      ),
    },
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("&math:bit-not expected a number, but received: {a}")),
    a => CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&math:bit-not expected 1 number, but received: {a:?}"),
    ),
  }
}
