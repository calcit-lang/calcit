use crate::primes::CalcitData;
use crate::primes::CalcitData::*;

pub fn new_list(xs: im::Vector<CalcitData>) -> Result<CalcitData, String> {
  Ok(CalcitList(xs))
}
