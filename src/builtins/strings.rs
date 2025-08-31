use std::cmp::Ordering;

use crate::calcit::{Calcit, CalcitErr, CalcitErrKind, CalcitList};
use crate::util::number::f64_to_usize;

pub fn binary_str_concat(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Nil), Some(Calcit::Nil)) => Ok(Calcit::new_str("")),
    (Some(Calcit::Nil), Some(b)) => Ok(Calcit::Str(b.turn_string().into())),
    (Some(a), Some(Calcit::Nil)) => Ok(Calcit::Str(a.turn_string().into())),
    (Some(Calcit::Str(s1)), Some(Calcit::Str(s2))) => {
      let mut s = String::with_capacity(s1.len() + s2.len());
      s.push_str(s1);
      s.push_str(s2);
      Ok(Calcit::Str(s.into()))
    }
    (Some(a), Some(b)) => {
      let mut s = a.turn_string();
      s.push_str(&b.turn_string());
      Ok(Calcit::Str(s.into()))
    }
    (_, _) => CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&str:concat expected 2 arguments, but received: {}", CalcitList::from(xs)),
    ),
  }
}

pub fn trim(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Str(s)), None) => Ok(Calcit::Str(s.trim().to_owned().into())),
    (Some(Calcit::Str(s)), Some(Calcit::Str(p))) => {
      if p.len() == 1 {
        let c: char = p.chars().next().expect("first char");
        Ok(Calcit::Str(s.trim_matches(c).to_owned().into()))
      } else {
        CalcitErr::err_str(
          CalcitErrKind::Type,
          format!("trim expected a single character pattern, but received: {p}"),
        )
      }
    }
    (Some(a), Some(b)) => CalcitErr::err_str(CalcitErrKind::Type, format!("trim expected 2 strings, but received: {a} {b}")),
    (_, _) => CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("trim expected 1 or 2 arguments, but received: {}", CalcitList::from(xs)),
    ),
  }
}

/// just format value to string
pub fn call_str(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(a) => Ok(Calcit::Str(a.turn_string().into())),
    None => CalcitErr::err_str(CalcitErrKind::Arity, "&str expected 1 argument, but received none"),
  }
}

pub fn turn_string(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Nil) => Ok(Calcit::new_str("")),
    Some(Calcit::Bool(b)) => Ok(Calcit::Str(b.to_string().into())),
    Some(Calcit::Str(s)) => Ok(Calcit::Str(s.to_owned())),
    Some(Calcit::Tag(s)) => Ok(Calcit::Str(s.arc_str())),
    Some(Calcit::Symbol { sym, .. }) => Ok(Calcit::Str(sym.to_owned())),
    Some(Calcit::Number(n)) => Ok(Calcit::Str(n.to_string().into())),
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("turn-string cannot convert to string: {a}")),
    None => CalcitErr::err_str(CalcitErrKind::Arity, "turn-string expected 1 argument, but received none"),
  }
}

pub fn split(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Str(pattern))) => {
      let pieces = (**s)
        .split(&**pattern)
        .filter(|s| !s.is_empty())
        .map(|s| Calcit::Str(s.into()))
        .collect::<Vec<Calcit>>();
      Ok(Calcit::from(pieces))
    }
    (Some(a), Some(b)) => CalcitErr::err_str(CalcitErrKind::Type, format!("split expected 2 strings, but received: {a} {b}")),
    (_, _) => CalcitErr::err_str(CalcitErrKind::Arity, "split expected 2 arguments, but received none"),
  }
}

pub fn format_number(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(n)), Some(Calcit::Number(x))) => {
      let size = f64_to_usize(*x)?;
      Ok(Calcit::Str(format!("{n:.size$}").into()))
    }
    (Some(a), Some(b)) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&number:format expected 2 numbers, but received: {a} {b}"),
    ),
    (_, _) => CalcitErr::err_str(CalcitErrKind::Arity, "&number:format expected 2 arguments, but received none"),
  }
}

/// displays in binary, octal, or hexadecimal
pub fn display_number_by(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Number(n)), Some(Calcit::Number(x))) => {
      let value = f64_to_usize(*n)? as i32;
      let size = f64_to_usize(*x)?;
      match size {
        2 => Ok(Calcit::Str(format!("{value:#01b}").into())),
        8 => Ok(Calcit::Str(format!("{value:#01o}").into())),
        16 => Ok(Calcit::Str(format!("{value:#01x}").into())),
        _ => CalcitErr::err_str(
          CalcitErrKind::Type,
          format!("&number:display-by only supports base 2, 8, or 16, but received: {size}"),
        ),
      }
    }
    (Some(a), Some(b)) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&number:display-by expected 2 numbers, but received: {a} {b}"),
    ),
    (_, _) => CalcitErr::err_str(CalcitErrKind::Arity, "&number:display-by expected 2 arguments, but received none"),
  }
}

pub fn replace(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1), xs.get(2)) {
    (Some(Calcit::Str(s)), Some(Calcit::Str(p)), Some(Calcit::Str(r))) => Ok(Calcit::Str(s.replace(&**p, r).into())),
    (Some(a), Some(b), Some(c)) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&str:replace expected 3 strings, but received: {a} {b} {c}"),
    ),
    (_, _, _) => CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("&str:replace expected 3 arguments, but received: {}", CalcitList::from(xs)),
    ),
  }
}
pub fn split_lines(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Str(s)) => {
      let lines = s.lines().map(|line| Calcit::Str(line.to_owned().into())).collect::<Vec<Calcit>>();
      Ok(Calcit::from(lines))
    }
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("split-lines expected 1 string, but received: {a}")),
    _ => CalcitErr::err_str(CalcitErrKind::Arity, "split-lines expected 1 argument, but received none"),
  }
}
pub fn str_slice(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1), xs.get(2)) {
    (Some(Calcit::Str(s)), Some(Calcit::Number(n_from)), n_to) => {
      let from = f64_to_usize(*n_from)?;
      let to = match n_to {
        Some(Calcit::Number(n)) => f64_to_usize(*n)?,
        Some(a) => {
          return CalcitErr::err_str(
            CalcitErrKind::Type,
            format!("&str:slice expected a number for index, but received: {a}"),
          );
        }
        None => s.chars().count(),
      };

      if from >= to {
        Ok(Calcit::new_str(""))
      } else {
        let s: String = s.chars().skip(from).take(to - from).collect();
        Ok(Calcit::Str(s.into()))
      }
    }
    (Some(a), Some(b), ..) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&str:slice expected a string and a number, but received: {a} {b}"),
    ),
    (_, _, _) => CalcitErr::err_nodes(CalcitErrKind::Arity, "&str:slice expected a string and numbers, but received:", xs),
  }
}

pub fn compare_string(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Str(a)), Some(Calcit::Str(b))) => {
      let v = match a.cmp(b) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
      };
      Ok(Calcit::Number(v as f64))
    }
    (Some(a), Some(b)) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&str:compare expected 2 strings, but received: {a}, {b}"),
    ),
    (_, _) => CalcitErr::err_nodes(CalcitErrKind::Arity, "&str:compare expected 2 strings, but received:", xs),
  }
}

/// returns -1 if not found
pub fn find_index(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Str(pattern))) => match s.find(&**pattern) {
      Some(idx) => Ok(Calcit::Number(idx as f64)),
      None => Ok(Calcit::Number(-1.0)),
    },
    (Some(a), Some(b)) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&str:find-index expected 2 strings, but received: {a} {b}"),
    ),
    (_, _) => CalcitErr::err_str(CalcitErrKind::Arity, "&str:find-index expected 2 arguments, but received none"),
  }
}
pub fn starts_with_ques(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Str(pattern))) => Ok(Calcit::Bool(s.starts_with(&**pattern))),
    (Some(Calcit::Tag(s)), Some(Calcit::Tag(pattern))) => Ok(Calcit::Bool((*s.ref_str()).starts_with(pattern.ref_str()))),
    (Some(Calcit::Tag(s)), Some(Calcit::Str(pattern))) => Ok(Calcit::Bool((*s.ref_str()).starts_with(&**pattern))),
    (Some(a), Some(b)) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("starts-with? expected 2 strings, but received: {a} {b}"),
    ),
    (_, _) => CalcitErr::err_str(CalcitErrKind::Arity, "starts-with? expected 2 arguments, but received none"),
  }
}
pub fn ends_with_ques(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Str(pattern))) => Ok(Calcit::Bool(s.ends_with(&**pattern))),
    (Some(a), Some(b)) => CalcitErr::err_str(CalcitErrKind::Type, format!("ends-with? expected 2 strings, but received: {a} {b}")),
    (_, _) => CalcitErr::err_str(CalcitErrKind::Arity, "ends-with? expected 2 arguments, but received none"),
  }
}
pub fn get_char_code(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Str(s)) => {
      if s.chars().count() == 1 {
        match s.chars().next() {
          Some(c) => Ok(Calcit::Number((c as u32) as f64)),
          None => unreachable!("expected a character"),
        }
      } else {
        CalcitErr::err_str(
          CalcitErrKind::Type,
          format!("get-char-code expected a single character string, but received: {s}"),
        )
      }
    }
    Some(a) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("get-char-code expected a character, but received: {a}"),
    ),
    _ => CalcitErr::err_str(CalcitErrKind::Arity, "get-char-code expected 1 argument, but received none"),
  }
}
pub fn char_from_code(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(x)) => match f64_to_usize(*x) {
      Ok(n) => Ok(Calcit::Str((char::from_u32(n as u32).expect("create char")).to_string().into())),
      Err(e) => CalcitErr::err_str(CalcitErrKind::Type, format!("char-from-code expected a number, but received: {e}")),
    },
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("char-from-code expected 1 number, but received: {a}")),
    _ => CalcitErr::err_str(CalcitErrKind::Arity, "char-from-code expected 1 argument, but received none"),
  }
}
pub fn parse_float(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Str(s)) => match s.parse::<f64>() {
      Ok(n) => Ok(Calcit::Number(n)),
      Err(e) => CalcitErr::err_str(CalcitErrKind::Syntax, format!("parse-float failed: {e}")),
    },
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("parse-float expected 1 string, but received: {a}")),
    _ => CalcitErr::err_str(CalcitErrKind::Arity, "parse-float expected 1 argument, but received none"),
  }
}

pub fn lispy_string(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(a) => Ok(Calcit::Str(a.to_string().into())),
    None => CalcitErr::err_str(CalcitErrKind::Arity, "to-lispy-string expected 1 argument, but received none"),
  }
}
pub fn blank_ques(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Str(s)) => Ok(Calcit::Bool(s.trim().is_empty())),
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("blank? expected 1 string, but received: {a}")),
    None => CalcitErr::err_str(CalcitErrKind::Arity, "blank? expected 1 argument, but received none"),
  }
}

pub fn escape(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Str(s)) => {
      let mut chunk = String::from('"');
      chunk.push_str(&s.escape_default().to_string());
      chunk.push('"');
      Ok(Calcit::Str(chunk.into()))
    }
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("escape expected 1 string, but received: {a}")),
    None => CalcitErr::err_str(CalcitErrKind::Arity, "escape expected 1 argument, but received none"),
  }
}

pub fn count(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Str(s)) => Ok(Calcit::Number(s.chars().count() as f64)),
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("&str:count expected a string, but received: {a}")),
    None => CalcitErr::err_str(CalcitErrKind::Arity, "&str:count expected 1 argument, but received none"),
  }
}

pub fn empty_ques(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Str(s)) => Ok(Calcit::Bool(s.is_empty())),
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("&str:empty? expected a string, but received: {a}")),
    None => CalcitErr::err_str(CalcitErrKind::Arity, "&str:empty? expected 1 argument, but received none"),
  }
}

pub fn contains_ques(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
      Ok(idx) => Ok(Calcit::Bool(idx < s.chars().count())),
      Err(e) => CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&str:contains? expected a valid index, but received: {e}"),
      ),
    },
    (Some(a), ..) => CalcitErr::err_str(CalcitErrKind::Type, format!("&str:contains? expected a string, but received: {a}")),
    (None, ..) => CalcitErr::err_nodes(CalcitErrKind::Arity, "&str:contains? expected 2 arguments, but received:", xs),
  }
}

pub fn includes_ques(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Str(xs)), Some(Calcit::Str(a))) => Ok(Calcit::Bool(xs.contains(&**a))),
    (Some(Calcit::Str(_)), Some(a)) => {
      CalcitErr::err_str(CalcitErrKind::Type, format!("&str:includes? expected a string, but received: {a}"))
    }
    (Some(a), ..) => CalcitErr::err_str(CalcitErrKind::Type, format!("&str:includes? expected a string, but received: {a}")),
    (None, ..) => CalcitErr::err_nodes(CalcitErrKind::Arity, "&str:includes? expected 2 arguments, but received:", xs),
  }
}
pub fn nth(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Str(s)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
      Ok(idx) => match s.chars().nth(idx) {
        Some(v) => Ok(Calcit::Str(v.to_string().into())),
        None => Ok(Calcit::Nil),
      },
      Err(e) => CalcitErr::err_str(CalcitErrKind::Type, format!("&str:nth expected a valid index, but received: {e}")),
    },
    (Some(_), None) => CalcitErr::err_nodes(CalcitErrKind::Arity, "&str:nth expected a string and an index, but received:", xs),
    (None, Some(_)) => CalcitErr::err_nodes(CalcitErrKind::Arity, "&str:nth expected a string and an index, but received:", xs),
    (_, _) => CalcitErr::err_nodes(CalcitErrKind::Arity, "&str:nth expected 2 arguments, but received:", xs),
  }
}

pub fn first(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Str(s)) => match s.chars().next() {
      Some(c) => Ok(Calcit::Str(c.to_string().into())),
      None => Ok(Calcit::Nil),
    },
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("&str:first expected a string, but received: {a}")),
    None => CalcitErr::err_str(CalcitErrKind::Arity, "&str:first expected 1 argument, but received none"),
  }
}

pub fn rest(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Str(s)) => Ok(Calcit::Str(s.chars().skip(1).collect::<String>().into())),
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("&str:rest expected a string, but received: {a}")),
    None => CalcitErr::err_str(CalcitErrKind::Arity, "&str:rest expected 1 argument, but received none"),
  }
}

pub fn pad_left(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() == 3 {
    match (&xs[0], &xs[1], &xs[2]) {
      (Calcit::Str(s), Calcit::Number(n), Calcit::Str(pattern)) => {
        let size = n.floor() as usize;
        if pattern.is_empty() {
          return CalcitErr::err_str(CalcitErrKind::Arity, "&str:pad-left expected a non-empty pattern");
        }
        if s.len() >= size {
          return Ok(xs[0].to_owned());
        }

        let pad_size = size - s.len();
        let mut buffer = String::with_capacity(size);
        // Directly iterate over pattern characters
        for c in pattern.chars().cycle().take(pad_size) {
          buffer.push(c);
        }
        buffer.push_str(s);
        Ok(Calcit::Str(buffer.into()))
      }
      (a, b, c) => CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&str:pad-left expected a string, a number, and a string, but received: {a} {b} {c}"),
      ),
    }
  } else {
    CalcitErr::err_nodes(CalcitErrKind::Arity, "&str:pad-left expected 3 arguments, but received:", xs)
  }
}

pub fn pad_right(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() == 3 {
    match (&xs[0], &xs[1], &xs[2]) {
      (Calcit::Str(s), Calcit::Number(n), Calcit::Str(pattern)) => {
        let size = n.floor() as usize;
        if pattern.is_empty() {
          return CalcitErr::err_str(CalcitErrKind::Arity, "&str:pad-right expected a non-empty pattern");
        }
        if s.len() >= size {
          return Ok(xs[0].to_owned());
        }

        let mut buffer = String::with_capacity(size);
        buffer.push_str(s);
        // Directly iterate over pattern characters
        for c in pattern.chars().cycle().take(size - s.len()) {
          buffer.push(c);
        }
        Ok(Calcit::Str(buffer.into()))
      }
      (a, b, c) => CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&str:pad-right expected a string, a number, and a string, but received: {a} {b} {c}"),
      ),
    }
  } else {
    CalcitErr::err_nodes(CalcitErrKind::Arity, "&str:pad-right expected 3 arguments, but received:", xs)
  }
}
