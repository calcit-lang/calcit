use crate::primes::CalcitData;
use crate::primes::CalcitData::*;

pub fn new_set(xs: im::Vector<CalcitData>) -> Result<CalcitData, String> {
  let mut ys = im::HashSet::new();
  for x in xs {
    ys.insert(x);
  }
  Ok(CalcitSet(ys))
}
