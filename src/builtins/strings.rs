use crate::builtins::math::f64_to_usize;
use crate::primes;
use crate::primes::{Calcit, CalcitItems};

pub fn binary_str_concat(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(a), Some(b)) => {
      let mut s = a.turn_string();
      s.push_str(&b.turn_string());
      Ok(Calcit::Str(s))
    }
    (_, _) => Err(format!("expected 2 arguments, got: {}", primes::CrListWrap(xs.clone()))),
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
    (_, _) => Err(format!("expected 2 arguments, got: {}", primes::CrListWrap(xs.clone()))),
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

pub fn format_number(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Number(n)), Some(Calcit::Number(x))) => {
      let size = f64_to_usize(*x)?;
      Ok(Calcit::Str(format!("{n:.*}", size, n = n)))
    }
    (Some(a), Some(b)) => Err(format!("format-number expected numbers, got: {} {}", a, b)),
    (_, _) => Err(String::from("format-number expected 2 arguments")),
  }
}

pub fn replace(xs: &CalcitItems) -> Result<Calcit, String> {
  Err(String::from("TODO"))
}
pub fn split_lines(xs: &CalcitItems) -> Result<Calcit, String> {
  Err(String::from("TODO"))
}
pub fn substr(xs: &CalcitItems) -> Result<Calcit, String> {
  Err(String::from("TODO"))
}
pub fn compare_string(xs: &CalcitItems) -> Result<Calcit, String> {
  Err(String::from("TODO"))
}
pub fn str_find(xs: &CalcitItems) -> Result<Calcit, String> {
  Err(String::from("TODO"))
}
pub fn starts_with_ques(xs: &CalcitItems) -> Result<Calcit, String> {
  Err(String::from("TODO"))
}
pub fn ends_with_ques(xs: &CalcitItems) -> Result<Calcit, String> {
  Err(String::from("TODO"))
}
pub fn get_char_code(xs: &CalcitItems) -> Result<Calcit, String> {
  Err(String::from("TODO"))
}
pub fn re_matches(xs: &CalcitItems) -> Result<Calcit, String> {
  Err(String::from("TODO"))
}
pub fn re_find(xs: &CalcitItems) -> Result<Calcit, String> {
  Err(String::from("TODO"))
}
pub fn parse_float(xs: &CalcitItems) -> Result<Calcit, String> {
  Err(String::from("TODO"))
}
pub fn pr_str(xs: &CalcitItems) -> Result<Calcit, String> {
  Err(String::from("TODO"))
}
pub fn re_find_index(xs: &CalcitItems) -> Result<Calcit, String> {
  Err(String::from("TODO"))
}
pub fn re_find_all(xs: &CalcitItems) -> Result<Calcit, String> {
  Err(String::from("TODO"))
}
pub fn blank_ques(xs: &CalcitItems) -> Result<Calcit, String> {
  Err(String::from("TODO"))
}
