use crate::primes::{CalcitData, CalcitData::*, CalcitItems};

pub fn new_set(xs: &CalcitItems) -> Result<CalcitData, String> {
  let mut ys = im::HashSet::new();
  for x in xs {
    ys.insert(x.clone());
  }
  Ok(CalcitSet(ys))
}
