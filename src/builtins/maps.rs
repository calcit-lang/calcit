use crate::primes::{Calcit, CalcitItems};

use crate::builtins::math::{f32_to_usize, is_even};

pub fn call_new_map(xs: &CalcitItems) -> Result<Calcit, String> {
  if is_even(xs.len()) {
    let n = xs.len() >> 1;
    let mut ys = im::HashMap::new();
    for i in 0..n {
      ys.insert(xs[i << 1].clone(), xs[(i << 1) + 1].clone());
    }
    Ok(Calcit::Map(ys))
  } else {
    Err(String::from("&{} expected even number of arguments"))
  }
}

pub fn assoc(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1), xs.get(2)) {
    (Some(Calcit::List(xs)), Some(Calcit::Number(n)), Some(a)) => match f32_to_usize(*n) {
      Ok(idx) => {
        if idx < xs.len() {
          let mut ys = xs.clone();
          ys[idx] = a.clone();
          Ok(Calcit::List(ys))
        } else {
          Ok(Calcit::Nil)
        }
      }
      Err(e) => Err(e),
    },
    (Some(Calcit::Map(xs)), Some(a), Some(b)) => {
      let ys = &mut xs.clone();
      ys.insert(a.clone(), b.clone());
      Ok(Calcit::Map(ys.clone()))
    }
    (Some(a), ..) => Err(format!("expected list or map, got: {}", a)),
    (None, ..) => Err(format!("expected 3 arguments, got: {:?}", xs)),
  }
}

pub fn map_get(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::List(xs)), Some(Calcit::Number(n))) => match f32_to_usize(*n) {
      Ok(idx) => {
        if idx < xs.len() {
          Ok(xs[idx].clone())
        } else {
          Ok(Calcit::Nil)
        }
      }
      Err(e) => Err(e),
    },
    (Some(Calcit::Map(xs)), Some(a)) => {
      let ys = &mut xs.clone();
      match ys.get(a) {
        Some(v) => Ok(v.clone()),
        None => Ok(Calcit::Nil),
      }
    }
    (Some(a), ..) => Err(format!("expected list or map, got: {}", a)),
    (None, ..) => Err(format!("expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn contains_ques(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::List(xs)), Some(Calcit::Number(n))) => match f32_to_usize(*n) {
      Ok(idx) => {
        if idx < xs.len() {
          Ok(Calcit::Bool(true))
        } else {
          Ok(Calcit::Bool(false))
        }
      }
      Err(e) => Err(e),
    },
    (Some(Calcit::Map(xs)), Some(a)) => Ok(Calcit::Bool(xs.contains_key(a))),
    (Some(a), ..) => Err(format!("expected list or map, got: {}", a)),
    (None, ..) => Err(format!("expected 2 arguments, got: {:?}", xs)),
  }
}
