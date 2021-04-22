use crate::primes::{Calcit, CalcitItems};

use crate::builtins::math::{f64_to_usize, is_even};

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
    (Some(Calcit::List(xs)), Some(Calcit::Number(n)), Some(a)) => match f64_to_usize(*n) {
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
    (Some(a), ..) => Err(format!("assoc expected list or map, got: {}", a)),
    (None, ..) => Err(format!("assoc expected 3 arguments, got: {:?}", xs)),
  }
}

pub fn dissoc(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Map(xs)), Some(a)) => {
      let ys = &mut xs.clone();
      ys.remove(a);
      Ok(Calcit::Map(ys.clone()))
    }
    (Some(Calcit::List(xs)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
      Ok(idx) => {
        let ys = &mut xs.clone();
        ys.remove(idx);
        Ok(Calcit::List(ys.clone()))
      }
      Err(e) => Err(format!("dissoc expected number, {}", e)),
    },
    (Some(a), ..) => Err(format!("dissoc expected a map, got: {}", a)),
    (_, _) => Err(format!("dissoc expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn map_get(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::List(xs)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
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
    (Some(Calcit::List(xs)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
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

pub fn call_merge(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Map(xs)), Some(Calcit::Map(ys))) => {
      let mut zs: im::HashMap<Calcit, Calcit> = xs.clone();
      for (k, v) in ys {
        zs.insert(k.clone(), v.clone());
      }
      Ok(Calcit::Map(zs))
    }
    (Some(a), Some(b)) => Err(format!("expected 2 maps, got: {} {}", a, b)),
    (_, _) => Err(format!("expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn includes_ques(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Nil), _) => Err(String::from("nil includes nothing")),
    (Some(Calcit::Map(ys)), Some(a)) => {
      for (_k, v) in ys {
        if v == a {
          return Ok(Calcit::Bool(true));
        }
      }
      Ok(Calcit::Bool(false))
    }
    (Some(Calcit::List(xs)), Some(a)) => Ok(Calcit::Bool(xs.contains(a))),
    (Some(Calcit::Set(xs)), Some(a)) => Ok(Calcit::Bool(xs.contains(a))),
    (Some(Calcit::Str(xs)), Some(Calcit::Str(a))) => Ok(Calcit::Bool(xs.contains(a))),
    (Some(Calcit::Str(_)), Some(a)) => Err(format!("string `contains?` expected a string, got: {}", a)),
    (Some(a), ..) => Err(format!("expected list, map, set, got: {}", a)),
    (None, ..) => Err(format!("expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn to_pairs(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    // get a random order from internals
    Some(Calcit::Map(ys)) => {
      let mut zs: im::HashSet<Calcit> = im::HashSet::new();
      for (k, v) in ys {
        zs.insert(Calcit::List(im::vector![k.clone(), v.clone(),]));
      }
      Ok(Calcit::Set(zs))
    }
    Some(a) => Err(format!("to-pairs expected a map, got {}", a)),
    None => Err(String::from("to-pairs expected 1 argument, got nothing")),
  }
}
pub fn call_merge_non_nil(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Map(xs)), Some(Calcit::Map(ys))) => {
      let mut zs: im::HashMap<Calcit, Calcit> = xs.clone();
      for (k, v) in ys {
        if *v != Calcit::Nil {
          zs.insert(k.clone(), v.clone());
        }
      }
      Ok(Calcit::Map(zs))
    }
    (Some(a), Some(b)) => Err(format!("expected 2 maps, got: {} {}", a, b)),
    (_, _) => Err(format!("expected 2 arguments, got: {:?}", xs)),
  }
}
