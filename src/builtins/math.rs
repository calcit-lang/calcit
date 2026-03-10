use crate::builtins::meta::type_of;
use crate::calcit::{Calcit, CalcitErr, CalcitErrKind, CalcitProc, format_proc_examples_hint};

use crate::util::number::{f64_to_i32, is_integer};

pub fn binary_add(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(a)), Some(Calcit::Number(b))) => Ok(Calcit::Number(a + b)),
    (Some(a), Some(b)) => {
      let type_a = crate::builtins::meta::type_of(&[a.clone()])?.lisp_str();
      let type_b = crate::builtins::meta::type_of(&[b.clone()])?.lisp_str();
      let msg = format!("&+ requires 2 numbers, but received: ({type_a}, {type_b})");
      let hint = String::from("💡 Usage: `&+ number1 number2`\n  Example: `&+ 3 5` => 8");
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    _ => crate::builtins::err_arity("&+ requires 2 arguments, but received:", xs),
  }
}

pub fn binary_minus(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(a)), Some(Calcit::Number(b))) => Ok(Calcit::Number(a - b)),
    (Some(a), Some(b)) => {
      let type_a = crate::builtins::meta::type_of(&[a.clone()])?.lisp_str();
      let type_b = crate::builtins::meta::type_of(&[b.clone()])?.lisp_str();
      let msg = format!("&- requires 2 numbers, but received: ({type_a}, {type_b})");
      let hint = String::from("💡 Usage: `&- number1 number2`\n  Example: `&- 5 3` => 2");
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    _ => crate::builtins::err_arity("&- requires 2 arguments, but received:", xs),
  }
}

pub fn binary_multiply(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(a)), Some(Calcit::Number(b))) => Ok(Calcit::Number(a * b)),
    (Some(a), Some(b)) => {
      let type_a = crate::builtins::meta::type_of(&[a.clone()])?.lisp_str();
      let type_b = crate::builtins::meta::type_of(&[b.clone()])?.lisp_str();
      let msg = format!("&* requires 2 numbers, but received: ({type_a}, {type_b})");
      let hint = String::from("💡 Usage: `&* number1 number2`\n  Example: `&* 3 4` => 12");
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    _ => crate::builtins::err_arity("&* requires 2 arguments, but received:", xs),
  }
}

pub fn binary_divide(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(a)), Some(Calcit::Number(b))) => Ok(Calcit::Number(a / b)),
    (Some(a), Some(b)) => {
      let type_a = crate::builtins::meta::type_of(&[a.clone()])?.lisp_str();
      let type_b = crate::builtins::meta::type_of(&[b.clone()])?.lisp_str();
      let msg = format!("&/ requires 2 numbers, but received: ({type_a}, {type_b})");
      let hint =
        String::from("💡 Usage: `&/ number1 number2`\n  Example: `&/ 10 2` => 5\n  Warning: Division by zero returns infinity");
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    _ => crate::builtins::err_arity("&/ requires 2 arguments, but received:", xs),
  }
}

pub fn round_ques(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => Ok(Calcit::Bool(is_integer(*n))),
    Some(a) => {
      let msg = format!(
        "&math:round? requires a number, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::IsRound).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    a => CalcitErr::err_str(CalcitErrKind::Arity, format!("&math:round? expected 1 number, but received: {a:?}")),
  }
}

pub fn floor(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.floor())),
    Some(a) => {
      let msg = format!(
        "&math:floor requires a number, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::Floor).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    a => CalcitErr::err_str(CalcitErrKind::Arity, format!("&math:floor expected 1 number, but received: {a:?}")),
  }
}

// TODO semantics of Rust and JavaScript are different
pub fn fractional(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n - n.floor())),
    Some(a) => {
      let msg = format!(
        "&math:fract requires a number, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeNumberFract).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
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
    Some(a) => {
      let msg = format!(
        "&math:round requires a number, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::Round).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    a => CalcitErr::err_str(CalcitErrKind::Arity, format!("&math:round expected 1 number, but received: {a:?}")),
  }
}
pub fn sin(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.sin())),
    Some(a) => {
      let msg = format!(
        "&math:sin requires a number, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::Sin).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    a => CalcitErr::err_str(CalcitErrKind::Arity, format!("&math:sin expected 1 number, but received: {a:?}")),
  }
}
pub fn cos(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.cos())),
    Some(a) => {
      let msg = format!(
        "&math:cos requires a number, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::Cos).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
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
    Some(a) => {
      let msg = format!(
        "&math:ceil requires a number, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::Ceil).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    a => CalcitErr::err_str(CalcitErrKind::Arity, format!("&math:ceil expected 1 number, but received: {a:?}")),
  }
}
pub fn sqrt(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.sqrt())),
    Some(a) => {
      let msg = format!(
        "&math:sqrt requires a number, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::Sqrt).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
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
    Some(a) => {
      let msg = format!(
        "&math:bit-not requires a number, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::BitNot).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    a => CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&math:bit-not expected 1 number, but received: {a:?}"),
    ),
  }
}
