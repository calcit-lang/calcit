use crate::builtins::math::{f32_to_i32, f32_to_usize};
use crate::primes::CalcitData::*;
use crate::primes::{CalcitData, CalcitItems};

pub fn new_list(xs: &CalcitItems) -> Result<CalcitData, String> {
  Ok(CalcitList(xs.clone()))
}

pub fn empty_ques(xs: &CalcitItems) -> Result<CalcitData, String> {
  match xs.get(0) {
    Some(CalcitNil) => Ok(CalcitBool(true)),
    Some(CalcitList(ys)) => Ok(CalcitBool(ys.is_empty())),
    Some(CalcitMap(ys)) => Ok(CalcitBool(ys.is_empty())),
    Some(CalcitString(s)) => Ok(CalcitBool(s.is_empty())),
    Some(a) => Err(format!("empty? expected some seq, got: {}", a)),
    None => Err(String::from("empty? expected 1 argument")),
  }
}

pub fn count(xs: &CalcitItems) -> Result<CalcitData, String> {
  match xs.get(0) {
    Some(CalcitNil) => Ok(CalcitNumber(0.0)),
    Some(CalcitList(ys)) => Ok(CalcitNumber(ys.len() as f32)),
    Some(CalcitMap(ys)) => Ok(CalcitNumber(ys.len() as f32)),
    Some(CalcitString(s)) => Ok(CalcitNumber(s.len() as f32)),
    Some(a) => Err(format!("count expected some seq, got: {}", a)),
    None => Err(String::from("count expected 1 argument")),
  }
}

pub fn nth(xs: &CalcitItems) -> Result<CalcitData, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(CalcitNil), Some(CalcitNumber(_))) => Ok(CalcitNil),
    (Some(CalcitList(ys)), Some(CalcitNumber(n))) => {
      let idx: usize = unsafe { n.to_int_unchecked() };
      match ys.get(idx) {
        Some(v) => Ok(v.clone()),
        None => Ok(CalcitNil),
      }
    }
    (Some(CalcitString(s)), Some(CalcitNumber(n))) => {
      let idx: usize = unsafe { n.to_int_unchecked() };
      match s.chars().nth(idx) {
        Some(v) => Ok(CalcitString(v.to_string())),
        None => Ok(CalcitNil),
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

pub fn slice(xs: &CalcitItems) -> Result<CalcitData, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(CalcitList(ys)), Some(CalcitNumber(from))) => {
      let to_idx = match xs.get(2) {
        Some(CalcitNumber(to)) => {
          let idx: usize = unsafe { to.to_int_unchecked() };
          idx
        }
        Some(a) => return Err(format!("slice expected number index, got: {}", a)),
        None => ys.len(),
      };
      let from_idx: usize = unsafe { from.to_int_unchecked() };
      Ok(CalcitList(ys.clone().slice(from_idx..to_idx)))
    }
    (Some(CalcitList(_)), Some(a)) => Err(format!("slice expected index number, got: {}", a)),
    (Some(CalcitList(_)), None) => Err(String::from("slice expected index numbers")),
    (_, _) => Err(String::from("slice expected 2~3 arguments")),
  }
}

pub fn append(xs: &CalcitItems) -> Result<CalcitData, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(CalcitList(ys)), Some(a)) => {
      let mut zs = ys.clone();
      zs.push_back(a.clone());
      Ok(CalcitList(zs))
    }
    (Some(a), _) => Err(format!("append expected list, got: {}", a)),
    (None, _) => Err(String::from("append expected 2 arguments, got nothing")),
  }
}

pub fn prepend(xs: &CalcitItems) -> Result<CalcitData, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(CalcitList(ys)), Some(a)) => {
      let mut zs = ys.clone();
      zs.push_front(a.clone());
      Ok(CalcitList(zs))
    }
    (Some(a), _) => Err(format!("prepend expected list, got: {}", a)),
    (None, _) => Err(String::from("prepend expected 2 arguments, got nothing")),
  }
}

pub fn rest(xs: &CalcitItems) -> Result<CalcitData, String> {
  match xs.get(0) {
    Some(CalcitNil) => Ok(CalcitNil),
    Some(CalcitList(ys)) => {
      let mut zs = ys.clone();
      zs.pop_front();
      Ok(CalcitList(zs))
    }
    Some(a) => Err(format!("rest expected a list, got: {}", a)),
    None => Err(String::from("rest expected 1 argument")),
  }
}

pub fn butlast(xs: &CalcitItems) -> Result<CalcitData, String> {
  match xs.get(0) {
    Some(CalcitNil) => Ok(CalcitNil),
    Some(CalcitList(ys)) => {
      let mut zs = ys.clone();
      zs.pop_back();
      Ok(CalcitList(zs))
    }
    Some(a) => Err(format!("butlast expected a list, got: {}", a)),
    None => Err(String::from("butlast expected 1 argument")),
  }
}

pub fn concat(xs: &CalcitItems) -> Result<CalcitData, String> {
  let mut ys: CalcitItems = im::vector![];
  for x in xs {
    if let CalcitList(zs) = x {
      for z in zs {
        ys.push_back(z.clone());
      }
    } else {
      return Err(format!("concat expects list arguments, got: {}", x));
    }
  }
  Ok(CalcitList(ys))
}

pub fn range(xs: &CalcitItems) -> Result<CalcitData, String> {
  let (base, bound) = match (xs.get(0), xs.get(1)) {
    (Some(CalcitNumber(bound)), None) => (0.0, *bound),
    (Some(CalcitNumber(base)), Some(CalcitNumber(bound))) => (*base, *bound),
    (Some(a), Some(b)) => return Err(format!("range expected 2 numbers, but got: {} {}", a, b)),
    (_, _) => return Err(format!("invalid arguments for range: {:?}", xs)),
  };

  let step = match xs.get(2) {
    Some(CalcitNumber(n)) => *n,
    Some(a) => return Err(format!("range expected numbers, but got: {}", a)),
    None => 1.0,
  };

  if (bound - base).abs() < f32::EPSILON {
    return Ok(CalcitList(im::vector![CalcitNumber(base)]));
  }

  if step == 0.0 || (bound > base && step < 0.0) || (bound < base && step > 0.0) {
    return Err(String::from("range cannot construct list with step 0"));
  }

  let mut ys: CalcitItems = im::vector![];
  let mut i = base;
  if step > 0.0 {
    while i < bound {
      ys.push_back(CalcitNumber(i));
      i += step;
    }
  } else {
    while i > bound {
      ys.push_back(CalcitNumber(i));
      i += step;
    }
  }
  Ok(CalcitList(ys))
}
