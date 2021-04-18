use crate::primes;
use crate::primes::CalcitData::*;
use crate::primes::{CalcitData, CalcitItems};

pub fn binary_str_concat(xs: &CalcitItems) -> Result<CalcitData, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(a), Some(b)) => {
      let mut s = a.turn_string();
      s.push_str(&b.turn_string());
      Ok(CalcitString(s))
    }
    (_, _) => Err(format!(
      "expected 2 arguments, got: {}",
      primes::CrListWrap(xs.clone())
    )),
  }
}
