use crate::builtins::math::{f32_to_i32, f32_to_usize};
use crate::primes::{Calcit, CalcitItems};

pub fn new_list(xs: &CalcitItems) -> Result<Calcit, String> {
  Ok(Calcit::List(xs.clone()))
}

pub fn empty_ques(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Nil) => Ok(Calcit::Bool(true)),
    Some(Calcit::List(ys)) => Ok(Calcit::Bool(ys.is_empty())),
    Some(Calcit::Map(ys)) => Ok(Calcit::Bool(ys.is_empty())),
    Some(Calcit::Str(s)) => Ok(Calcit::Bool(s.is_empty())),
    Some(a) => Err(format!("empty? expected some seq, got: {}", a)),
    None => Err(String::from("empty? expected 1 argument")),
  }
}

pub fn count(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Nil) => Ok(Calcit::Number(0.0)),
    Some(Calcit::List(ys)) => Ok(Calcit::Number(ys.len() as f32)),
    Some(Calcit::Map(ys)) => Ok(Calcit::Number(ys.len() as f32)),
    Some(Calcit::Str(s)) => Ok(Calcit::Number(s.len() as f32)),
    Some(a) => Err(format!("count expected some seq, got: {}", a)),
    None => Err(String::from("count expected 1 argument")),
  }
}

pub fn nth(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Nil), Some(Calcit::Number(_))) => Ok(Calcit::Nil),
    (Some(Calcit::List(ys)), Some(Calcit::Number(n))) => {
      let idx: usize = unsafe { n.to_int_unchecked() };
      match ys.get(idx) {
        Some(v) => Ok(v.clone()),
        None => Ok(Calcit::Nil),
      }
    }
    (Some(Calcit::Str(s)), Some(Calcit::Number(n))) => {
      let idx: usize = unsafe { n.to_int_unchecked() };
      match s.chars().nth(idx) {
        Some(v) => Ok(Calcit::Str(v.to_string())),
        None => Ok(Calcit::Nil),
      }
    }
    (Some(_), None) => Err(format!(
      "nth expected a ordered seq and index, got: {:?}",
      xs
    )),
    (None, Some(_)) => Err(format!(
      "nth expected a ordered seq and index, got: {:?}",
      xs
    )),
    (_, _) => Err(String::from("nth expected 2 argument")),
  }
}

pub fn slice(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::List(ys)), Some(Calcit::Number(from))) => {
      let to_idx = match xs.get(2) {
        Some(Calcit::Number(to)) => {
          let idx: usize = unsafe { to.to_int_unchecked() };
          idx
        }
        Some(a) => return Err(format!("slice expected number index, got: {}", a)),
        None => ys.len(),
      };
      let from_idx: usize = unsafe { from.to_int_unchecked() };
      Ok(Calcit::List(ys.clone().slice(from_idx..to_idx)))
    }
    (Some(Calcit::List(_)), Some(a)) => Err(format!("slice expected index number, got: {}", a)),
    (Some(Calcit::List(_)), None) => Err(String::from("slice expected index numbers")),
    (_, _) => Err(String::from("slice expected 2~3 arguments")),
  }
}

pub fn append(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::List(ys)), Some(a)) => {
      let mut zs = ys.clone();
      zs.push_back(a.clone());
      Ok(Calcit::List(zs))
    }
    (Some(a), _) => Err(format!("append expected list, got: {}", a)),
    (None, _) => Err(String::from("append expected 2 arguments, got nothing")),
  }
}

pub fn prepend(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::List(ys)), Some(a)) => {
      let mut zs = ys.clone();
      zs.push_front(a.clone());
      Ok(Calcit::List(zs))
    }
    (Some(a), _) => Err(format!("prepend expected list, got: {}", a)),
    (None, _) => Err(String::from("prepend expected 2 arguments, got nothing")),
  }
}

pub fn rest(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Nil) => Ok(Calcit::Nil),
    Some(Calcit::List(ys)) => {
      let mut zs = ys.clone();
      zs.pop_front();
      Ok(Calcit::List(zs))
    }
    Some(a) => Err(format!("rest expected a list, got: {}", a)),
    None => Err(String::from("rest expected 1 argument")),
  }
}

pub fn butlast(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Nil) => Ok(Calcit::Nil),
    Some(Calcit::List(ys)) => {
      let mut zs = ys.clone();
      zs.pop_back();
      Ok(Calcit::List(zs))
    }
    Some(a) => Err(format!("butlast expected a list, got: {}", a)),
    None => Err(String::from("butlast expected 1 argument")),
  }
}

pub fn concat(xs: &CalcitItems) -> Result<Calcit, String> {
  let mut ys: CalcitItems = im::vector![];
  for x in xs {
    if let Calcit::List(zs) = x {
      for z in zs {
        ys.push_back(z.clone());
      }
    } else {
      return Err(format!("concat expects list arguments, got: {}", x));
    }
  }
  Ok(Calcit::List(ys))
}

pub fn range(xs: &CalcitItems) -> Result<Calcit, String> {
  let (base, bound) = match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Number(bound)), None) => (0.0, *bound),
    (Some(Calcit::Number(base)), Some(Calcit::Number(bound))) => (*base, *bound),
    (Some(a), Some(b)) => return Err(format!("range expected 2 numbers, but got: {} {}", a, b)),
    (_, _) => return Err(format!("invalid arguments for range: {:?}", xs)),
  };

  let step = match xs.get(2) {
    Some(Calcit::Number(n)) => *n,
    Some(a) => return Err(format!("range expected numbers, but got: {}", a)),
    None => 1.0,
  };

  if (bound - base).abs() < f32::EPSILON {
    return Ok(Calcit::List(im::vector![Calcit::Number(base)]));
  }

  if step == 0.0 || (bound > base && step < 0.0) || (bound < base && step > 0.0) {
    return Err(String::from("range cannot construct list with step 0"));
  }

  let mut ys: CalcitItems = im::vector![];
  let mut i = base;
  if step > 0.0 {
    while i < bound {
      ys.push_back(Calcit::Number(i));
      i += step;
    }
  } else {
    while i > bound {
      ys.push_back(Calcit::Number(i));
      i += step;
    }
  }
  Ok(Calcit::List(ys))
}
