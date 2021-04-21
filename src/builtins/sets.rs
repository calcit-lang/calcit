use crate::primes::{Calcit, CalcitItems};

pub fn new_set(xs: &CalcitItems) -> Result<Calcit, String> {
  let mut ys = im::HashSet::new();
  for x in xs {
    ys.insert(x.clone());
  }
  Ok(Calcit::Set(ys))
}
