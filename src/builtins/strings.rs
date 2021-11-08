use std::char;
use std::cmp::Ordering;

use crate::primes;
use crate::primes::{Calcit, CalcitErr, CalcitItems, CrListWrap};
use crate::util::number::f64_to_usize;

use im_ternary_tree::TernaryTreeList;

pub fn binary_str_concat(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Nil), Some(Calcit::Nil)) => Ok(Calcit::new_str("")),
    (Some(Calcit::Nil), Some(b)) => Ok(Calcit::Str(b.turn_string().into())),
    (Some(a), Some(Calcit::Nil)) => Ok(Calcit::Str(a.turn_string().into())),
    (Some(a), Some(b)) => {
      let mut s = a.turn_string();
      s.push_str(&b.turn_string());
      Ok(Calcit::Str(s.into()))
    }
    (_, _) => CalcitErr::err_str(format!("expected 2 arguments, got: {}", primes::CrListWrap(xs.to_owned()))),
  }
}

pub fn trim(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(s)), None) => Ok(Calcit::Str(s.trim().to_owned().into())),
    (Some(Calcit::Str(s)), Some(Calcit::Str(p))) => {
      if p.len() == 1 {
        let c: char = p.chars().next().unwrap();
        Ok(Calcit::Str(s.trim_matches(c).to_owned().into()))
      } else {
        CalcitErr::err_str(format!("trim expected pattern in a char, got {}", p))
      }
    }
    (Some(a), Some(b)) => CalcitErr::err_str(format!("trim expected 2 strings, but got: {} {}", a, b)),
    (_, _) => CalcitErr::err_str(format!("expected 2 arguments, got: {}", primes::CrListWrap(xs.to_owned()))),
  }
}

/// just format value to string
pub fn call_str(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(a) => Ok(Calcit::Str(a.turn_string().into())),
    None => CalcitErr::err_str("&str expected 1 argument, got nothing"),
  }
}

pub fn turn_string(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Nil) => Ok(Calcit::new_str("")),
    Some(Calcit::Bool(b)) => Ok(Calcit::Str(b.to_string().into())),
    Some(Calcit::Str(s)) => Ok(Calcit::Str(s.to_owned())),
    Some(Calcit::Keyword(s)) => Ok(Calcit::Str(s.to_string().into())),
    Some(Calcit::Symbol { sym, .. }) => Ok(Calcit::Str(sym.to_owned())),
    Some(Calcit::Number(n)) => Ok(Calcit::Str(n.to_string().into())),
    Some(a) => CalcitErr::err_str(format!("turn-string cannot turn this to string: {}", a)),
    None => CalcitErr::err_str("turn-string expected 1 argument, got nothing"),
  }
}

pub fn split(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Str(pattern))) => {
      let pieces = (**s).split(&**pattern);
      let mut ys: CalcitItems = TernaryTreeList::Empty;
      for p in pieces {
        if !p.is_empty() {
          ys = ys.push(Calcit::Str(p.to_owned().into()));
        }
      }
      Ok(Calcit::List(ys))
    }
    (Some(a), Some(b)) => CalcitErr::err_str(format!("split expected 2 strings, got: {} {}", a, b)),
    (_, _) => CalcitErr::err_str("split expected 2 arguments, got nothing"),
  }
}

pub fn format_number(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Number(n)), Some(Calcit::Number(x))) => {
      let size = f64_to_usize(*x)?;
      Ok(Calcit::Str(format!("{n:.*}", size, n = n).into()))
    }
    (Some(a), Some(b)) => CalcitErr::err_str(format!("&number:format expected numbers, got: {} {}", a, b)),
    (_, _) => CalcitErr::err_str("&number:format expected 2 arguments"),
  }
}

pub fn replace(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1), xs.get(2)) {
    (Some(Calcit::Str(s)), Some(Calcit::Str(p)), Some(Calcit::Str(r))) => Ok(Calcit::Str(s.replace(&**p, &**r).into())),
    (Some(a), Some(b), Some(c)) => CalcitErr::err_str(format!("str:replace expected 3 strings, got: {} {} {}", a, b, c)),
    (_, _, _) => CalcitErr::err_str(format!(
      "str:replace expected 3 arguments, got: {}",
      primes::CrListWrap(xs.to_owned())
    )),
  }
}
pub fn split_lines(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => {
      let lines = s.split('\n');
      let mut ys = TernaryTreeList::Empty;
      for line in lines {
        ys = ys.push(Calcit::Str(line.to_owned().into()));
      }
      Ok(Calcit::List(ys))
    }
    Some(a) => CalcitErr::err_str(format!("split-lines expected 1 string, got: {}", a)),
    _ => CalcitErr::err_str("split-lines expected 1 argument, got nothing"),
  }
}
pub fn str_slice(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
      Ok(from) => {
        let to: usize = match xs.get(2) {
          Some(Calcit::Number(n2)) => match f64_to_usize(*n2) {
            Ok(idx2) => idx2,
            Err(e) => return CalcitErr::err_str(format!("&str:slice expected number, got: {}", e)),
          },
          Some(a) => return CalcitErr::err_str(format!("&str:slice expected number, got: {}", a)),
          None => s.chars().count(),
        };
        if from >= to {
          Ok(Calcit::new_str(""))
        } else {
          // turn into vec first to also handle UTF8
          let s_vec = s.chars().collect::<Vec<_>>();
          Ok(Calcit::Str(s_vec[from..to].iter().cloned().collect::<String>().into()))
        }
      }
      Err(e) => CalcitErr::err_str(e),
    },
    (Some(a), Some(b)) => CalcitErr::err_str(format!("&str:slice expected string and number, got: {} {}", a, b)),
    (_, _) => CalcitErr::err_str(format!("&str:slice expected string and numbers, got: {:?}", xs)),
  }
}

pub fn compare_string(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(a)), Some(Calcit::Str(b))) => {
      let v = match a.cmp(b) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
      };
      Ok(Calcit::Number(v as f64))
    }
    (Some(a), Some(b)) => CalcitErr::err_str(format!("&str:compare expected 2 strings, got: {}, {}", a, b)),
    (_, _) => CalcitErr::err_str(format!("&str:compare expected 2 string, got: {:?}", xs)),
  }
}

pub fn find_index(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Str(pattern))) => match s.find(&**pattern) {
      Some(idx) => Ok(Calcit::Number(idx as f64)),
      None => Ok(Calcit::Number(-1.0)), // TODO maybe nil?
    },
    (Some(a), Some(b)) => CalcitErr::err_str(format!("str:find-index expected 2 strings, got: {} {}", a, b)),
    (_, _) => CalcitErr::err_str("str:find-index expected 2 arguments, got nothing"),
  }
}
pub fn starts_with_ques(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Str(pattern))) => Ok(Calcit::Bool(s.starts_with(&**pattern))),
    (Some(a), Some(b)) => CalcitErr::err_str(format!("starts-with? expected 2 strings, got: {} {}", a, b)),
    (_, _) => CalcitErr::err_str("starts-with? expected 2 arguments, got nothing"),
  }
}
pub fn ends_with_ques(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Str(pattern))) => Ok(Calcit::Bool(s.ends_with(&**pattern))),
    (Some(a), Some(b)) => CalcitErr::err_str(format!("ends-with? expected 2 strings, got: {} {}", a, b)),
    (_, _) => CalcitErr::err_str("ends-with? expected 2 arguments, got nothing"),
  }
}
pub fn get_char_code(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => {
      if s.chars().count() == 1 {
        match s.chars().next() {
          Some(c) => Ok(Calcit::Number((c as u32) as f64)),
          None => unreachable!("expected a character"),
        }
      } else {
        CalcitErr::err_str(format!("get-char-code expected a character, got: {}", s))
      }
    }
    Some(a) => CalcitErr::err_str(format!("get-char-code expected a charactor, got: {}", a)),
    _ => CalcitErr::err_str("get-char-code expected 1 argument, got nothing"),
  }
}
pub fn char_from_code(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Number(x)) => match f64_to_usize(*x) {
      Ok(n) => Ok(Calcit::Str((char::from_u32(n as u32).unwrap()).to_string().into())),
      Err(e) => return CalcitErr::err_str(format!("char_from_code expected number, got: {}", e)),
    },
    Some(a) => CalcitErr::err_str(format!("char_from_code expected 1 number, got: {}", a)),
    _ => CalcitErr::err_str("char_from_code expected 1 arguments, got nothing"),
  }
}
pub fn parse_float(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => match s.parse::<f64>() {
      Ok(n) => Ok(Calcit::Number(n)),
      Err(e) => CalcitErr::err_str(format!("parse-float failed, {}", e)),
    },
    Some(a) => CalcitErr::err_str(format!("starts-with? expected 1 string, got: {}", a)),
    _ => CalcitErr::err_str("starts-with? expected 1 argument, got nothing"),
  }
}

pub fn pr_str(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(a) => Ok(Calcit::Str(a.to_string().into())),
    None => CalcitErr::err_str("pr-str expected 1 argument, got nothing"),
  }
}
pub fn blank_ques(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => Ok(Calcit::Bool(s.trim().is_empty())),
    Some(a) => CalcitErr::err_str(format!("blank? expected 1 string, got: {}", a)),
    None => CalcitErr::err_str("blank? expected 1 argument, got nothing"),
  }
}

pub fn escape(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => {
      let mut chunk = String::from("\"");
      chunk.push_str(&s.escape_default().to_string());
      chunk.push('"');
      Ok(Calcit::Str(chunk.into()))
    }
    Some(a) => CalcitErr::err_str(format!("escape expected 1 string, got {}", a)),
    None => CalcitErr::err_str("escape expected 1 argument, got nothing"),
  }
}

pub fn count(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => Ok(Calcit::Number(s.chars().count() as f64)),
    Some(a) => CalcitErr::err_str(format!("string count expected a string, got: {}", a)),
    None => CalcitErr::err_str("string count expected 1 argument"),
  }
}

pub fn empty_ques(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => Ok(Calcit::Bool(s.is_empty())),
    Some(a) => CalcitErr::err_str(format!("string empty? expected a string, got: {}", a)),
    None => CalcitErr::err_str("string empty? expected 1 argument"),
  }
}

pub fn contains_ques(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
      Ok(idx) => Ok(Calcit::Bool(idx < s.chars().count())),
      Err(e) => CalcitErr::err_str(e),
    },
    (Some(a), ..) => CalcitErr::err_str(format!("strings contains? expected a string, got: {}", a)),
    (None, ..) => CalcitErr::err_str(format!("strings contains? expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn includes_ques(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(xs)), Some(Calcit::Str(a))) => Ok(Calcit::Bool(xs.contains(&**a))),
    (Some(Calcit::Str(_)), Some(a)) => CalcitErr::err_str(format!("string `includes?` expected a string, got: {}", a)),
    (Some(a), ..) => CalcitErr::err_str(format!("string `includes?` expected string, got: {}", a)),
    (None, ..) => CalcitErr::err_str(format!("string `includes?` expected 2 arguments, got: {:?}", xs)),
  }
}
pub fn nth(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
      Ok(idx) => match s.chars().nth(idx) {
        Some(v) => Ok(Calcit::Str(v.to_string().into())),
        None => Ok(Calcit::Nil),
      },
      Err(e) => CalcitErr::err_str(format!("string nth expect usize, {}", e)),
    },
    (Some(_), None) => CalcitErr::err_str(format!("string nth expected a string and index, got: {:?}", xs)),
    (None, Some(_)) => CalcitErr::err_str(format!("string nth expected a string and index, got: {:?}", xs)),
    (_, _) => CalcitErr::err_str(format!("string nth expected 2 argument, got: {}", CrListWrap(xs.to_owned()))),
  }
}

pub fn first(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => match s.chars().next() {
      Some(c) => Ok(Calcit::Str(c.to_string().into())),
      None => Ok(Calcit::Nil),
    },
    Some(a) => CalcitErr::err_str(format!("str:first expected a string, got: {}", a)),
    None => CalcitErr::err_str("str:first expected 1 argument"),
  }
}

pub fn rest(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => {
      let mut buffer = String::with_capacity(s.len() - 1);
      let mut is_first = true;
      for c in s.chars() {
        if is_first {
          is_first = false;
          continue;
        }
        buffer.push(c)
      }
      Ok(Calcit::Str(buffer.into()))
    }
    Some(a) => CalcitErr::err_str(format!("str:rest expected a string, got: {}", a)),
    None => CalcitErr::err_str("str:rest expected 1 argument"),
  }
}
