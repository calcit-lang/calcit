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

/// just format value to string
pub fn call_str(xs: &CalcitItems) -> Result<CalcitData, String> {
  match xs.get(0) {
    Some(a) => Ok(CalcitString(a.to_string())),
    None => Err(String::from("&str expected 1 argument, got nothing")),
  }
}

pub fn turn_string(xs: &CalcitItems) -> Result<CalcitData, String> {
  match xs.get(0) {
    Some(CalcitNil) => Ok(CalcitString(String::from(""))),
    Some(CalcitBool(b)) => Ok(CalcitString(b.to_string())),
    Some(CalcitString(s)) => Ok(CalcitString(s.clone())),
    Some(CalcitKeyword(s)) => Ok(CalcitString(s.clone())),
    Some(CalcitSymbol(s, _ns)) => Ok(CalcitString(s.clone())),
    Some(a) => Err(format!("turn-string cannot turn this to string: {}", a)),
    None => Err(String::from("turn-string expected 1 argument, got nothing")),
  }
}

pub fn split(xs: &CalcitItems) -> Result<CalcitData, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(CalcitString(s)), Some(CalcitString(pattern))) => {
      let pieces = s.split(pattern);
      let mut ys: CalcitItems = im::vector![];
      for p in pieces {
        ys.push_back(CalcitString(p.to_string()));
      }
      Ok(CalcitList(ys))
    }
    (Some(a), Some(b)) => Err(format!("split expected 2 strings, got: {} {}", a, b)),
    (_, _) => Err(String::from("split expected 2 arguments, got nothing")),
  }
}
