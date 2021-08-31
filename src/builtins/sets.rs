use crate::primes::{Calcit, CalcitItems};

pub fn new_set(xs: &CalcitItems) -> Result<Calcit, String> {
  let mut ys = im::HashSet::new();
  for x in xs {
    ys.insert(x.to_owned());
  }
  Ok(Calcit::Set(ys))
}

pub fn call_include(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Set(xs)), Some(a)) => {
      let mut ys = xs.to_owned();
      ys.insert(a.to_owned());
      Ok(Calcit::Set(ys))
    }
    (Some(a), _) => Err(format!("&include expect a set, but got: {}", a)),
    (a, b) => Err(format!("invalid arguments for &include: {:?} {:?}", a, b)),
  }
}

pub fn call_exclude(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Set(xs)), Some(a)) => {
      let mut ys = xs.to_owned();
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
    (Some(Calcit::Set(a)), Some(Calcit::Set(b))) => Ok(Calcit::Set(a.to_owned().union(b.to_owned()))),
    (Some(a), Some(b)) => Err(format!("&union expected 2 sets: {} {}", a, b)),
    (a, b) => Err(format!("&union expected 2 arguments: {:?} {:?}", a, b)),
  }
}
pub fn call_intersection(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Set(a)), Some(Calcit::Set(b))) => Ok(Calcit::Set(a.to_owned().intersection(b.to_owned()))),
    (Some(a), Some(b)) => Err(format!("&set:intersection expected 2 sets: {} {}", a, b)),
    (a, b) => Err(format!("&set:intersection expected 2 arguments: {:?} {:?}", a, b)),
  }
}

/// turn hashset into list with a random order from internals
pub fn set_to_list(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Set(xs)) => {
      let mut ys: CalcitItems = im::vector![];
      for x in xs {
        ys.push_back(x.to_owned());
      }
      Ok(Calcit::List(ys))
    }
    Some(a) => Err(format!("&set:to-list expected a set: {}", a)),
    None => Err(String::from("&set:to-list expected 1 argument, got none")),
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

pub fn includes_ques(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Set(xs)), Some(a)) => Ok(Calcit::Bool(xs.contains(a))),
    (Some(a), ..) => Err(format!("sets `includes?` expected set, got: {}", a)),
    (None, ..) => Err(format!("sets `includes?` expected 2 arguments, got: {:?}", xs)),
  }
}

/// use builtin function since sets need to be handled specifically
pub fn first(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Set(ys)) => match ys.iter().next() {
      // TODO first element of a set.. need to be more sure...
      Some(v) => Ok(v.to_owned()),
      None => Ok(Calcit::Nil),
    },
    Some(a) => Err(format!("set:first expected a set, got: {}", a)),
    None => Err(String::from("set:first expected 1 argument")),
  }
}

pub fn rest(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Set(ys)) => match ys.iter().next() {
      Some(y0) => {
        let mut zs = ys.to_owned();
        zs.remove(y0);
        Ok(Calcit::Set(zs))
      }
      None => Ok(Calcit::Nil),
    },
    Some(a) => Err(format!("set:rest expected a set, got: {}", a)),
    None => Err(String::from("set:rest expected 1 argument")),
  }
}
