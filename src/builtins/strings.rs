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

pub fn trim(xs: &CalcitItems) -> Result<CalcitData, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(CalcitString(s)), None) => Ok(CalcitString(s.trim().to_string())),
    (Some(CalcitString(s)), Some(CalcitString(p))) => {
      if p.len() == 1 {
        let c: char = p.chars().next().unwrap();
        Ok(CalcitString(s.trim_matches(c).to_string()))
      } else {
        Err(format!("trim expected pattern in a char, got {}", p))
      }
    }
    (Some(a), Some(b)) => Err(format!("trim expected 2 strings, but got: {} {}", a, b)),
    (_, _) => Err(format!(
      "expected 2 arguments, got: {}",
      primes::CrListWrap(xs.clone())
    )),
  }
}
