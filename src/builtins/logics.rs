use crate::{calcit::{Calcit, CalcitErr, CalcitErrKind}};

pub fn binary_equal(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(a), Some(b)) => Ok(Calcit::Bool(a == b)),
    (_, _) => CalcitErr::err_nodes(CalcitErrKind::Arity, "&= expected 2 arguments, got:", xs),
  }
}

pub fn binary_less(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() == 2 {
    Ok(Calcit::Bool(xs[0] < xs[1]))
  } else {
    CalcitErr::err_nodes(CalcitErrKind::Arity, "&< expected 2 arguments, got:", xs)
  }
}

pub fn binary_greater(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() == 2 {
    Ok(Calcit::Bool(xs[0] > xs[1]))
  } else {
    CalcitErr::err_nodes(CalcitErrKind::Arity, "&> expected 2 arguments, got:", xs)
  }
}

pub fn not(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "not expected bool or nil, got:", xs);
  }
  match &xs[0] {
    Calcit::Nil => Ok(Calcit::Bool(true)),
    Calcit::Bool(b) => Ok(Calcit::Bool(!b)),
    a => CalcitErr::err_str(CalcitErrKind::Type, format!("not expected bool or nil, got: {a}")),
  }
}
