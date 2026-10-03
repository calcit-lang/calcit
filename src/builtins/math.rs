use crate::builtins::meta::type_of;
use crate::calcit::type_annotation::CalcitNumericRefinement;
use crate::calcit::{Calcit, CalcitErr, CalcitErrKind, CalcitProc, format_proc_examples_hint};

use crate::util::number::{f64_to_i32, format_calcit_number};

pub fn binary_add(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(a)), Some(Calcit::Number(b))) => Ok(Calcit::Number(a + b)),
    (Some(a), Some(b)) => {
      let type_a = crate::builtins::meta::type_of(std::slice::from_ref(a))?.lisp_str();
      let type_b = crate::builtins::meta::type_of(std::slice::from_ref(b))?.lisp_str();
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
      let type_a = crate::builtins::meta::type_of(std::slice::from_ref(a))?.lisp_str();
      let type_b = crate::builtins::meta::type_of(std::slice::from_ref(b))?.lisp_str();
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
      let type_a = crate::builtins::meta::type_of(std::slice::from_ref(a))?.lisp_str();
      let type_b = crate::builtins::meta::type_of(std::slice::from_ref(b))?.lisp_str();
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
      let type_a = crate::builtins::meta::type_of(std::slice::from_ref(a))?.lisp_str();
      let type_b = crate::builtins::meta::type_of(std::slice::from_ref(b))?.lisp_str();
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
    Some(Calcit::Number(n)) => Ok(Calcit::Bool(n.is_finite() && n.fract() == 0.0)),
    Some(a) => {
      let msg = format!(
        "&math:round? requires a number, but received: {}",
        type_of(std::slice::from_ref(a))?.lisp_str()
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
        type_of(std::slice::from_ref(a))?.lisp_str()
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
        type_of(std::slice::from_ref(a))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeNumberFract).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    a => CalcitErr::err_str(CalcitErrKind::Arity, format!("&math:fract expected 1 number, but received: {a:?}")),
  }
}

pub fn number_fits(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(value)), Some(Calcit::Tag(target))) => {
      let Some(refinement) = CalcitNumericRefinement::from_name(target.ref_str()) else {
        return CalcitErr::err_str(
          CalcitErrKind::Type,
          format!("&number:fits? expected a numeric refinement tag, got :{target}"),
        );
      };
      Ok(Calcit::Bool(refinement.accepts(*value)))
    }
    (Some(value), Some(target)) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&number:fits? expected a number and a tag, got {value} {target}"),
    ),
    _ => crate::builtins::err_arity("&number:fits? requires 2 arguments, but received:", xs),
  }
}

pub fn rem(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(base)), Some(Calcit::Number(step))) => rem_numbers(*base, *step),
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

/// Largest integer that every backend represents exactly as an f64 (JS `Number.MAX_SAFE_INTEGER`).
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

fn safe_integer(value: f64) -> Option<i64> {
  (value.fract() == 0.0 && value.abs() <= MAX_SAFE_INTEGER).then_some(value as i64)
}

/// Truncated remainder over safe integers; the result takes the dividend's sign and is never `-0`.
pub(crate) fn rem_numbers(base: f64, step: f64) -> Result<Calcit, CalcitErr> {
  match (safe_integer(base), safe_integer(step)) {
    (Some(_), Some(0)) => CalcitErr::err_str(CalcitErrKind::Type, "&number:rem divisor must not be zero"),
    (Some(a), Some(b)) => Ok(Calcit::Number((a % b) as f64)),
    _ => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!(
        "&number:rem requires safe integers, but received: {} {}",
        format_calcit_number(base),
        format_calcit_number(step)
      ),
    ),
  }
}

pub fn round(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => Ok(Calcit::Number(n.round())),
    Some(a) => {
      let msg = format!(
        "&math:round requires a number, but received: {}",
        type_of(std::slice::from_ref(a))?.lisp_str()
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
        type_of(std::slice::from_ref(a))?.lisp_str()
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
        type_of(std::slice::from_ref(a))?.lisp_str()
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
        type_of(std::slice::from_ref(a))?.lisp_str()
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
        type_of(std::slice::from_ref(a))?.lisp_str()
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
      // only the low five bits of the step count, same as JS `>>` and WASM `i32.shr_s`
      (Ok(value), Ok(step)) => Ok(Calcit::Number(value.wrapping_shr(step as u32) as f64)),
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
      // only the low five bits of the step count, same as JS `<<` and WASM `i32.shl`
      (Ok(value), Ok(step)) => Ok(Calcit::Number(value.wrapping_shl(step as u32) as f64)),
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
        type_of(std::slice::from_ref(a))?.lisp_str()
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

#[cfg(test)]
mod remainder_safety_tests {
  use super::{MAX_SAFE_INTEGER, rem_numbers};
  use crate::calcit::Calcit;

  #[test]
  fn native_remainder_errors_never_unwind() {
    let values = [
      0.0,
      -0.0,
      1.0,
      -1.0,
      i32::MIN as f64,
      i32::MAX as f64,
      MAX_SAFE_INTEGER,
      -MAX_SAFE_INTEGER,
      MAX_SAFE_INTEGER + 1.0,
      f64::MIN,
      f64::MAX,
      0.5,
      f64::NAN,
      f64::INFINITY,
      f64::NEG_INFINITY,
    ];
    for base in values {
      for step in values {
        assert!(
          std::panic::catch_unwind(|| rem_numbers(base, step)).is_ok(),
          "native remainder must not unwind for {base} % {step}"
        );
      }
    }
    assert!(rem_numbers(1.0, 0.0).is_err());
    assert!(rem_numbers(MAX_SAFE_INTEGER + 1.0, 3.0).is_err());
    assert_eq!(rem_numbers(i32::MIN as f64, -1.0), Ok(Calcit::Number(0.0)));
    assert_eq!(rem_numbers(-MAX_SAFE_INTEGER, -1.0), Ok(Calcit::Number(0.0)));
  }
}

#[cfg(test)]
mod shift_safety_tests {
  use super::{bit_shl, bit_shr};
  use crate::calcit::Calcit;

  fn shift(f: fn(&[Calcit]) -> Result<Calcit, crate::calcit::CalcitErr>, value: f64, step: f64) -> f64 {
    match f(&[Calcit::Number(value), Calcit::Number(step)]) {
      Ok(Calcit::Number(n)) => n,
      other => panic!("expected a number, got {other:?}"),
    }
  }

  #[test]
  fn shift_counts_use_their_low_five_bits_without_panicking() {
    // same results as JS `<<` / `>>` and WASM `i32.shl` / `i32.shr_s`
    assert_eq!(shift(bit_shl, 1.0, 32.0), 1.0);
    assert_eq!(shift(bit_shl, 1.0, 33.0), 2.0);
    assert_eq!(shift(bit_shl, 1.0, -1.0), i32::MIN as f64);
    assert_eq!(shift(bit_shr, 8.0, 32.0), 8.0);
    assert_eq!(shift(bit_shr, -8.0, -1.0), -1.0);
    assert_eq!(shift(bit_shr, 5.0, 100.0), 0.0);
    for step in [i32::MIN as f64, -33.0, -1.0, 0.0, 31.0, 32.0, 1000.0, i32::MAX as f64] {
      let _ = shift(bit_shl, 123.0, step);
      let _ = shift(bit_shr, -123.0, step);
    }
  }
}
