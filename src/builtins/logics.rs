use std::sync::Arc;

use crate::calcit::{Calcit, CalcitErr, CalcitErrKind};

pub fn binary_equal(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(a), Some(b)) => Ok(Calcit::Bool(a == b)),
    (_, _) => {
      let hint = String::from(
        "💡 Usage: `&= value1 value2`\n  Compares two values for equality\n  Works on any types (numbers, strings, lists, etc.)",
      );
      crate::builtins::err_arity_with_hint("&= requires exactly 2 arguments to compare, but received:", xs, hint)
    }
  }
}

/// `identical?`: scalars (nil, bool, number, string, tag, ...) compare by value, matching JavaScript `===`
/// on primitives and interned tags; heap values (lists, maps, sets, structs, enum values, buffers,
/// functions, host refs) compare by reference, so separately constructed but equal values are not identical.
pub fn identical(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(a), Some(b)) => Ok(Calcit::Bool(is_identical(a, b))),
    (_, _) => {
      let hint = String::from("💡 Usage: `identical? value1 value2`\n  Checks whether two values are the same reference");
      crate::builtins::err_arity_with_hint("identical? requires exactly 2 arguments to compare, but received:", xs, hint)
    }
  }
}

pub fn is_identical(a: &Calcit, b: &Calcit) -> bool {
  use Calcit::*;
  match (a, b) {
    // same as JavaScript `===`: NaN is not identical to itself, `0` and `-0` are identical
    (Number(x), Number(y)) => x == y,
    (List(x), List(y)) => Arc::ptr_eq(x, y),
    (Map(x), Map(y)) => x.ptr_eq(y),
    (Set(x), Set(y)) => x.ptr_eq(y),
    (Struct(x), Struct(y)) => Arc::ptr_eq(&x.values, &y.values) && Arc::ptr_eq(&x.struct_ref, &y.struct_ref),
    (Enum(x), Enum(y)) => {
      Arc::ptr_eq(&x.extra, &y.extra)
        && x.tag == y.tag
        && match (&x.sum_type, &y.sum_type) {
          (Some(p), Some(q)) => Arc::ptr_eq(p, q),
          (None, None) => true,
          _ => false,
        }
    }
    (Ref(_, x), Ref(_, y)) => Arc::ptr_eq(x, y),
    (Buffer(x), Buffer(y)) => Arc::ptr_eq(x, y),
    (BufList(x), BufList(y)) => Arc::ptr_eq(x, y),
    (Fn { info: x, .. }, Fn { info: y, .. }) => Arc::ptr_eq(x, y),
    (Macro { info: x, .. }, Macro { info: y, .. }) => Arc::ptr_eq(x, y),
    (AnyRef(x), AnyRef(y)) => Arc::ptr_eq(&x.0, &y.0),
    // scalars, procs, syntax and definition values have no separate per-construction identity
    _ => a == b,
  }
}

pub fn binary_less(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() == 2 {
    match (&xs[0], &xs[1]) {
      (Calcit::Number(a), Calcit::Number(b)) => Ok(Calcit::Bool(a < b)),
      _ => {
        let msg = format!(
          "&< expects numbers, but received: {} and {}",
          crate::builtins::meta::type_of(&[xs[0].clone()])?.lisp_str(),
          crate::builtins::meta::type_of(&[xs[1].clone()])?.lisp_str()
        );
        let hint =
          String::from("💡 Usage: `&< number1 number2`\n  Compares if first number is less than second\n  Example: `&< 3 5` => true");
        CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
      }
    }
  } else {
    let hint =
      String::from("💡 Usage: `&< number1 number2`\n  Compares if first number is less than second\n  Example: `&< 3 5` => true");
    crate::builtins::err_arity_with_hint("&< requires exactly 2 arguments to compare, but received:", xs, hint)
  }
}

pub fn binary_greater(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() == 2 {
    match (&xs[0], &xs[1]) {
      (Calcit::Number(a), Calcit::Number(b)) => Ok(Calcit::Bool(a > b)),
      _ => {
        let msg = format!(
          "&> expects numbers, but received: {} and {}",
          crate::builtins::meta::type_of(&[xs[0].clone()])?.lisp_str(),
          crate::builtins::meta::type_of(&[xs[1].clone()])?.lisp_str()
        );
        let hint = String::from(
          "💡 Usage: `&> number1 number2`\n  Compares if first number is greater than second\n  Example: `&> 5 3` => true",
        );
        CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
      }
    }
  } else {
    let hint =
      String::from("💡 Usage: `&> value1 value2`\n  Compares if first value is greater than second\n  Example: `&> 5 3` => true");
    crate::builtins::err_arity_with_hint("&> requires exactly 2 arguments to compare, but received:", xs, hint)
  }
}

pub fn not(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    let hint = String::from(
      "💡 Usage: `not value`\n  Negates a boolean value; nil and &unit are also falsey\n  Examples: `not true` => false, `not nil` => true, `not &unit` => true",
    );
    return crate::builtins::err_arity_with_hint("not requires exactly 1 argument (a boolean, nil, or Unit), but received:", xs, hint);
  }
  match &xs[0] {
    Calcit::Nil | Calcit::Unit => Ok(Calcit::Bool(true)),
    Calcit::Bool(b) => Ok(Calcit::Bool(!b)),
    a => {
      let msg = format!(
        "not requires a boolean, nil, or Unit as argument, but received: {}",
        crate::builtins::meta::type_of(std::slice::from_ref(a))?.lisp_str()
      );
      let hint = String::from(
        "💡 Hint: Pass a boolean value (true/false), nil, or &unit to not\n  Examples: `not true` => false, `not nil` => true, `not &unit` => true",
      );
      crate::builtins::err_type_with_hint(msg, hint)
    }
  }
}
