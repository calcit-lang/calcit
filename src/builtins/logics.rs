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
      "💡 Usage: `not boolean-value`\n  Negates a boolean value\n  Examples: `not true` => false, `not nil` => true, `not &unit` => true",
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
