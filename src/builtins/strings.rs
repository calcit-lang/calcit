use std::cmp::Ordering;

use crate::primes;
use crate::primes::{Calcit, CalcitItems, CrListWrap};
use crate::util::number::f64_to_usize;

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
    (Some(Calcit::Str(s)), None) => Ok(Calcit::Str(s.trim().to_owned())),
    (Some(Calcit::Str(s)), Some(Calcit::Str(p))) => {
      if p.len() == 1 {
        let c: char = p.chars().next().unwrap();
        Ok(Calcit::Str(s.trim_matches(c).to_owned()))
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
    Some(a) => Ok(Calcit::Str(a.turn_string())),
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
    Some(Calcit::Number(n)) => Ok(Calcit::Str(n.to_string())),
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
        if !p.is_empty() {
          ys.push_back(Calcit::Str(p.to_owned()));
        }
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
  match (xs.get(0), xs.get(1), xs.get(2)) {
    (Some(Calcit::Str(s)), Some(Calcit::Str(p)), Some(Calcit::Str(r))) => Ok(Calcit::Str(s.replace(p, r))),
    (Some(a), Some(b), Some(c)) => Err(format!("str:replace expected 3 strings, got: {} {} {}", a, b, c)),
    (_, _, _) => Err(format!(
      "str:replace expected 3 arguments, got: {}",
      primes::CrListWrap(xs.clone())
    )),
  }
}
pub fn split_lines(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => {
      let lines = s.split('\n');
      let mut ys = im::vector![];
      for line in lines {
        ys.push_back(Calcit::Str(line.to_owned()));
      }
      Ok(Calcit::List(ys))
    }
    Some(a) => Err(format!("split-lines expected 1 string, got: {}", a)),
    _ => Err(String::from("split-lines expected 1 argument, got nothing")),
  }
}
pub fn substr(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
      Ok(from) => {
        let to: usize = match xs.get(2) {
          Some(Calcit::Number(n2)) => match f64_to_usize(*n2) {
            Ok(idx2) => idx2,
            Err(e) => return Err(format!("substr expected number, got: {}", e)),
          },
          Some(a) => return Err(format!("substr expected number, got: {}", a)),
          None => s.chars().count(),
        };
        if from >= to {
          Ok(Calcit::Str(String::from("")))
        } else {
          Ok(Calcit::Str(s[from..to].to_string()))
        }
      }
      Err(e) => Err(e),
    },
    (Some(a), Some(b)) => Err(format!("substr expected string and number, got: {} {}", a, b)),
    (_, _) => Err(format!("substr expected string and numbers, got: {:?}", xs)),
  }
}

pub fn compare_string(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(a)), Some(Calcit::Str(b))) => {
      let v = match a.cmp(&b) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
      };
      Ok(Calcit::Number(v as f64))
    }
    (Some(a), Some(b)) => Err(format!("compare-string expected 2 strings, got: {}, {}", a, b)),
    (_, _) => Err(format!("compare-string expected 2 string, got: {:?}", xs)),
  }
}

pub fn find_index(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Str(pattern))) => match s.find(pattern) {
      Some(idx) => Ok(Calcit::Number(idx as f64)),
      None => Ok(Calcit::Number(-1.0)), // TODO maybe nil?
    },
    (Some(a), Some(b)) => Err(format!("str:find-index expected 2 strings, got: {} {}", a, b)),
    (_, _) => Err(String::from("str:find-index expected 2 arguments, got nothing")),
  }
}
pub fn starts_with_ques(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Str(pattern))) => Ok(Calcit::Bool(s.starts_with(pattern))),
    (Some(a), Some(b)) => Err(format!("starts-with? expected 2 strings, got: {} {}", a, b)),
    (_, _) => Err(String::from("starts-with? expected 2 arguments, got nothing")),
  }
}
pub fn ends_with_ques(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Str(pattern))) => Ok(Calcit::Bool(s.ends_with(pattern))),
    (Some(a), Some(b)) => Err(format!("ends-with? expected 2 strings, got: {} {}", a, b)),
    (_, _) => Err(String::from("ends-with? expected 2 arguments, got nothing")),
  }
}
pub fn get_char_code(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => {
      if s.chars().count() == 1 {
        match s.chars().next() {
          Some(c) => Ok(Calcit::Number((c as u32) as f64)),
          None => unreachable!("expected a character"),
        }
      } else {
        Err(format!("get-char-code expected a character, got: {}", s))
      }
    }
    Some(a) => Err(format!("get-char-code expected 2 strings, got: {}", a)),
    _ => Err(String::from("get-char-code expected 2 arguments, got nothing")),
  }
}
pub fn parse_float(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => match s.parse::<f64>() {
      Ok(n) => Ok(Calcit::Number(n)),
      Err(e) => Err(format!("parse-float failed, {}", e)),
    },
    Some(a) => Err(format!("starts-with? expected 1 string, got: {}", a)),
    _ => Err(String::from("starts-with? expected 1 argument, got nothing")),
  }
}

pub fn pr_str(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(a) => Ok(Calcit::Str(a.to_string())),
    None => Err(String::from("pr-str expected 1 argument, got nothing")),
  }
}
pub fn blank_ques(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => Ok(Calcit::Bool(s.trim().is_empty())),
    Some(a) => Err(format!("blank? expected 1 string, got: {}", a)),
    None => Err(String::from("blank? expected 1 argument, got nothing")),
  }
}

pub fn escape(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => {
      let mut chunk = String::from("\"");
      chunk.push_str(&s.escape_default().to_string());
      chunk.push('"');
      Ok(Calcit::Str(chunk))
    }
    Some(a) => Err(format!("escape expected 1 string, got {}", a)),
    None => Err(String::from("escape expected 1 argument, got nothing")),
  }
}

pub fn count(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => Ok(Calcit::Number(s.chars().count() as f64)),
    Some(a) => Err(format!("string count expected a string, got: {}", a)),
    None => Err(String::from("string count expected 1 argument")),
  }
}

pub fn empty_ques(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => Ok(Calcit::Bool(s.is_empty())),
    Some(a) => Err(format!("string empty? expected a string, got: {}", a)),
    None => Err(String::from("string empty? expected 1 argument")),
  }
}

pub fn contains_ques(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
      Ok(idx) => Ok(Calcit::Bool(idx < s.chars().count())),
      Err(e) => Err(e),
    },
    (Some(a), ..) => Err(format!("strings contains? expected a string, got: {}", a)),
    (None, ..) => Err(format!("strings contains? expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn includes_ques(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(xs)), Some(Calcit::Str(a))) => Ok(Calcit::Bool(xs.contains(a))),
    (Some(Calcit::Str(_)), Some(a)) => Err(format!("string `includes?` expected a string, got: {}", a)),
    (Some(a), ..) => Err(format!("string `includes?` expected string, got: {}", a)),
    (None, ..) => Err(format!("string `includes?` expected 2 arguments, got: {:?}", xs)),
  }
}
pub fn nth(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
      Ok(idx) => match s.chars().nth(idx) {
        Some(v) => Ok(Calcit::Str(v.to_string())),
        None => Ok(Calcit::Nil),
      },
      Err(e) => Err(format!("nth expect usize, {}", e)),
    },
    (Some(_), None) => Err(format!("string nth expected a string and index, got: {:?}", xs)),
    (None, Some(_)) => Err(format!("string nth expected a string and index, got: {:?}", xs)),
    (_, _) => Err(format!("nth expected 2 argument, got: {}", CrListWrap(xs.to_owned()))),
  }
}

pub fn first(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => match s.chars().next() {
      Some(c) => Ok(Calcit::Str(c.to_string())),
      None => Ok(Calcit::Nil),
    },
    Some(a) => Err(format!("str:first expected a string, got: {}", a)),
    None => Err(String::from("str:first expected 1 argument")),
  }
}

pub fn rest(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => {
      let mut buffer = String::from("");
      let mut is_first = true;
      for c in s.chars() {
        if is_first {
          is_first = false;
          continue;
        }
        buffer.push(c)
      }
      Ok(Calcit::Str(buffer.to_owned()))
    }
    Some(a) => Err(format!("str:rest expected a string, got: {}", a)),
    None => Err(String::from("str:rest expected 1 argument")),
  }
}
