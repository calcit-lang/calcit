use crate::primes::{Calcit, CalcitItems};

pub fn new_set(xs: &CalcitItems) -> Result<Calcit, String> {
  let mut ys = im::HashSet::new();
  for x in xs {
    ys.insert(x.clone());
  }
  Ok(Calcit::Set(ys))
}

pub fn call_include(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Set(xs)), Some(a)) => {
      let mut ys = xs.clone();
      ys.insert(a.clone());
      Ok(Calcit::Set(ys))
    }
    (Some(a), _) => Err(format!("&include expect a set, but got: {}", a)),
    (a, b) => Err(format!("invalid arguments for &include: {:?} {:?}", a, b)),
  }
}

pub fn call_exclude(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Set(xs)), Some(a)) => {
      let mut ys = xs.clone();
      ys.remove(a);
      Ok(Calcit::Set(ys))
    }
    (Some(a), _) => Err(format!("&exclude expect a set, but got: {}", a)),
    (a, b) => Err(format!("invalid arguments for &exclude: {:?} {:?}", a, b)),
  }
}
pub fn call_difference(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Set(a)), Some(Calcit::Set(b))) => {
      // im::HashSet::difference has different semantics
      // https://docs.rs/im/12.2.0/im/struct.HashSet.html#method.difference
      let mut ys = a.to_owned();
      for item in b {
        ys.remove(item);
      }
      Ok(Calcit::Set(ys))
    }
    (Some(a), Some(b)) => Err(format!("&difference expected 2 sets: {} {}", a, b)),
    (a, b) => Err(format!("&difference expected 2 arguments: {:?} {:?}", a, b)),
  }
}
pub fn call_union(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Set(a)), Some(Calcit::Set(b))) => Ok(Calcit::Set(a.clone().union(b.clone()))),
    (Some(a), Some(b)) => Err(format!("&union expected 2 sets: {} {}", a, b)),
    (a, b) => Err(format!("&union expected 2 arguments: {:?} {:?}", a, b)),
  }
}
pub fn call_intersection(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Set(a)), Some(Calcit::Set(b))) => Ok(Calcit::Set(a.clone().intersection(b.clone()))),
    (Some(a), Some(b)) => Err(format!("&intersection expected 2 sets: {} {}", a, b)),
    (a, b) => Err(format!("&intersection expected 2 arguments: {:?} {:?}", a, b)),
  }
}

/// turn hashset into list with a random order from internals
pub fn set_to_list(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Set(xs)) => {
      let mut ys: CalcitItems = im::vector![];
      for x in xs {
        ys.push_back(x.clone());
      }
      Ok(Calcit::List(ys))
    }
    Some(a) => Err(format!("set->list expected a set: {}", a)),
    None => Err(String::from("set->list expected 1 argument, got none")),
  }
}

pub fn count(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Set(ys)) => Ok(Calcit::Number(ys.len() as f64)),
    Some(a) => Err(format!("set count expected a set, got: {}", a)),
    None => Err(String::from("set count expected 1 argument")),
  }
}

pub fn empty_ques(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Set(ys)) => Ok(Calcit::Bool(ys.is_empty())),
    Some(a) => Err(format!("set empty? expected some set, got: {}", a)),
    None => Err(String::from("set empty? expected 1 argument")),
  }
}
