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
  Err(String::from("TODO"))
}
pub fn call_difference(xs: &CalcitItems) -> Result<Calcit, String> {
  Err(String::from("TODO"))
}
pub fn call_union(xs: &CalcitItems) -> Result<Calcit, String> {
  Err(String::from("TODO"))
}
pub fn call_intersection(xs: &CalcitItems) -> Result<Calcit, String> {
  Err(String::from("TODO"))
}
pub fn set_to_list(xs: &CalcitItems) -> Result<Calcit, String> {
  Err(String::from("TODO"))
}
