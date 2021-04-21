use crate::primes;
use crate::primes::{Calcit, CalcitItems};

pub fn binary_str_concat(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(a), Some(b)) => {
      let mut s = a.turn_string();
      s.push_str(&b.turn_string());
      Ok(Calcit::Str(s))
    }
    (_, _) => Err(format!(
      "expected 2 arguments, got: {}",
      primes::CrListWrap(xs.clone())
    )),
  }
}

pub fn trim(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(s)), None) => Ok(Calcit::Str(s.trim().to_string())),
    (Some(Calcit::Str(s)), Some(Calcit::Str(p))) => {
      if p.len() == 1 {
        let c: char = p.chars().next().unwrap();
        Ok(Calcit::Str(s.trim_matches(c).to_string()))
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
pub fn call_str(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(a) => Ok(Calcit::Str(a.to_string())),
    None => Err(String::from("&str expected 1 argument, got nothing")),
  }
}

pub fn turn_string(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Nil) => Ok(Calcit::Str(String::from(""))),
    Some(Calcit::Bool(b)) => Ok(Calcit::Str(b.to_string())),
    Some(Calcit::Str(s)) => Ok(Calcit::Str(s.clone())),
    Some(Calcit::Keyword(s)) => Ok(Calcit::Str(s.clone())),
    Some(Calcit::Symbol(s, ..)) => Ok(Calcit::Str(s.clone())),
    Some(a) => Err(format!("turn-string cannot turn this to string: {}", a)),
    None => Err(String::from("turn-string expected 1 argument, got nothing")),
  }
}

pub fn split(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Str(pattern))) => {
      let pieces = s.split(pattern);
      let mut ys: CalcitItems = im::vector![];
      for p in pieces {
        ys.push_back(Calcit::Str(p.to_string()));
      }
      Ok(Calcit::List(ys))
    }
    (Some(a), Some(b)) => Err(format!("split expected 2 strings, got: {} {}", a, b)),
    (_, _) => Err(String::from("split expected 2 arguments, got nothing")),
  }
}
