use crate::primes::CalcitData::*;
use crate::primes::{CalcitData, CalcitItems};

pub fn new_list(xs: &CalcitItems) -> Result<CalcitData, String> {
  Ok(CalcitList(xs.clone()))
}
