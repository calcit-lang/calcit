use regex::Regex;

use crate::primes::{Calcit, CalcitItems};

pub fn re_matches(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Str(pattern))) => match Regex::new(pattern) {
      Ok(p) => Ok(Calcit::Bool(p.is_match(s))),
      Err(e) => Err(format!("re-matches failed, {}", e)),
    },
    (Some(a), Some(b)) => Err(format!("re-matches expected a string and a pattern, got: {} {}", a, b)),
    (_, _) => Err(format!("re-matches expected 2 arguments, got {:?}", xs)),
  }
}

pub fn re_find_index(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Str(pattern))) => match Regex::new(pattern) {
      Ok(p) => match p.find(s) {
        Some(matched) => Ok(Calcit::Number(matched.start() as f64)),
        None => Ok(Calcit::Number(-1.0)), // TODO maybe nil
      },
      Err(e) => Err(format!("re-find-index failed, {}", e)),
    },
    (Some(a), Some(b)) => Err(format!(
      "re-find-index expected a string and a pattern, got: {} {}",
      a, b
    )),
    (_, _) => Err(format!("re-find-index expected 2 arguments, got {:?}", xs)),
  }
}

/// takes stirng and patterns, returns a matches string
pub fn re_find(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Str(pattern))) => match Regex::new(pattern) {
      Ok(p) => {
        let mut matched = p.find_iter(s);
        match matched.next() {
          Some(v) => Ok(Calcit::Str(v.as_str().to_string())),
          None => Ok(Calcit::Nil), // TODO maybe nil
        }
      }
      Err(e) => Err(format!("re-find failed, {}", e)),
    },
    (Some(a), Some(b)) => Err(format!("re-find expected a string and a pattern, got: {} {}", a, b)),
    (_, _) => Err(format!("re-find expected 2 arguments, got {:?}", xs)),
  }
}

pub fn re_find_all(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Str(pattern))) => match Regex::new(pattern) {
      Ok(p) => {
        let mut ys: CalcitItems = im::vector![];
        for v in p.find_iter(s) {
          ys.push_back(Calcit::Str(v.as_str().to_string()))
        }
        Ok(Calcit::List(ys))
      }
      Err(e) => Err(format!("re-find-all failed, {}", e)),
    },
    (Some(a), Some(b)) => Err(format!("re-find-all expected a string and a pattern, got: {} {}", a, b)),
    (_, _) => Err(format!("re-find-all expected 2 arguments, got {:?}", xs)),
  }
}
