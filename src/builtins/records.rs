use std::cmp::Ordering;
use std::ops::Rem;

use crate::primes::{Calcit, CalcitItems, CrListWrap};
use crate::util::number::f64_to_usize;

pub fn new_record(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Symbol(s, ..)) => {
      let mut fields: Vec<String> = vec![];
      let mut values: Vec<Calcit> = vec![];

      for (idx, x) in xs.iter().enumerate() {
        if idx > 0 {
          match x {
            Calcit::Symbol(s, ..) | Calcit::Keyword(s) | Calcit::Str(s) => {
              fields.push(s.to_owned());
              values.push(Calcit::Nil);
            }
            a => return Err(format!("new-record fields accepets keyword/string, got a {}", a)),
          }
        }
      }
      fields.sort();
      Ok(Calcit::Record(s.to_owned(), fields, values))
    }
    Some(a) => Err(format!("new-record expected a name, got {}", a)),
    None => Err(format!("new-record expected arguments, got {:?}", xs)),
  }
}
pub fn call_record(xs: &CalcitItems) -> Result<Calcit, String> {
  let args_size = xs.len();
  if args_size < 2 {
    return Err(format!("&%{{}} expected at least 2 arguments, got {:?}", xs));
  }
  match &xs[0] {
    Calcit::Record(name, def_fields, v0) => {
      if (args_size - 1).rem(2) == 0 {
        let size = (args_size - 1) / 2;
        if size != def_fields.len() {
          return Err(format!("unexpected size in &%{{}}, {} .. {}", size, def_fields.len()));
        }
        let mut fields: Vec<String> = def_fields.clone();
        let mut values: Vec<Calcit> = v0.clone();

        for idx in 0..size {
          let k_idx = idx * 2 + 1;
          let v_idx = k_idx + 1;
          match &xs[k_idx] {
            Calcit::Symbol(s, ..) | Calcit::Keyword(s) | Calcit::Str(s) => match find_in_fields(&def_fields, s) {
              Some(pos) => {
                fields[pos] = s.clone();
                values[pos] = xs[v_idx].clone();
              }
              None => return Err(format!("unexpected field {} for {:?}", s, def_fields)),
            },
            a => return Err(format!("expected field in string/keyword, got: {}", a)),
          }
        }

        Ok(Calcit::Record(name.clone(), fields, values))
      } else {
        Err(format!("&%{{}} expected pairs, got: {:?}", xs))
      }
    }
    a => Err(format!("&%{{}} expected a record as prototype, got {}", a)),
  }
}

/// TODO the name is current `make-record`
pub fn record_from_map(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Record(name, fields, _values)), Some(Calcit::Map(ys))) => {
      let mut pairs: Vec<(String, Calcit)> = vec![];
      let mut values: Vec<Calcit> = vec![];
      for (k, v) in ys {
        match k {
          Calcit::Str(s) | Calcit::Keyword(s) => {
            pairs.push((s.clone(), v.clone()));
          }
          a => return Err(format!("unknown field {}", a)),
        }
      }
      if fields.len() != pairs.len() {
        return Err(format!("invalid fields {:?} for record {:?}", pairs, fields));
      }
      pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
      for idx in 0..fields.len() {
        let (k, v) = &pairs[idx];
        if &fields[idx] == k {
          values.push(v.clone());
        } else {
          return Err(format!(
            "field mismatch: {} {} in {:?} {:?}",
            k, fields[idx], fields, pairs
          ));
        }
      }
      pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
      Ok(Calcit::Record(name.clone(), fields.clone(), values))
    }
    (Some(a), Some(b)) => Err(format!("make-record expected a record and a map, got {} {}", a, b)),
    (_, _) => Err(format!("make-record expected 2 arguments, got {:?}", xs)),
  }
}

pub fn get_record_name(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Record(name, ..)) => Ok(Calcit::Str(name.to_owned())),
    Some(a) => Err(format!("get-record-name expected record, got: {}", a)),
    None => Err(String::from("get-record-name expected record, got nothing")),
  }
}
pub fn turn_map(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Record(_name, fields, values)) => {
      let mut ys: im::HashMap<Calcit, Calcit> = im::HashMap::new();
      for idx in 0..fields.len() {
        ys.insert(Calcit::Keyword(fields[idx].clone()), values[idx].clone());
      }
      Ok(Calcit::Map(ys))
    }
    Some(a) => Err(format!("turn-map expected a record, got {}", a)),
    None => Err(String::from("turn-map expected 1 argument, got nothing")),
  }
}
pub fn relevant_record_ques(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Record(left, left_fields, ..)), Some(Calcit::Record(right, right_fields, ..))) => {
      Ok(Calcit::Bool(left == right && left_fields == right_fields))
    }
    (Some(a), Some(b)) => Err(format!("relevant-record? expected 2 records, got {} {}", a, b)),
    (_, _) => Err(format!("relevant-record? expected 2 arguments, got {:?}", xs)),
  }
}

pub fn find_in_fields(xs: &[String], y: &str) -> Option<usize> {
  if xs.is_empty() {
    return None;
  }
  let mut lower = 0;
  let mut upper = xs.len() - 1;

  while (upper - lower) > 1 {
    let pos = (lower + upper) >> 1;
    let v = xs[pos].clone();
    match y.cmp(&v) {
      Ordering::Less => upper = pos - 1,
      Ordering::Greater => lower = pos + 1,
      Ordering::Equal => return Some(pos),
    }
  }

  match y {
    _ if y == xs[lower] => Some(lower),
    _ if y == xs[upper] => Some(upper),
    _ => None,
  }
}

pub fn count(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Record(_name, fields, _)) => Ok(Calcit::Number(fields.len() as f64)),
    Some(a) => Err(format!("record count expected a record, got: {}", a)),
    None => Err(String::from("record count expected 1 argument")),
  }
}

pub fn contains_ques(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Record(_name, fields, _)), Some(a)) => match a {
      Calcit::Str(k) | Calcit::Keyword(k) | Calcit::Symbol(k, ..) => {
        Ok(Calcit::Bool(find_in_fields(fields, k).is_some()))
      }
      a => Err(format!("contains? got invalid field for record: {}", a)),
    },
    (Some(a), ..) => Err(format!("record contains? expected a record, got: {}", a)),
    (None, ..) => Err(format!("record contains? expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn nth(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Record(_name, fields, values)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
      Ok(idx) => {
        if idx < fields.len() {
          Ok(Calcit::List(im::vector![
            Calcit::Keyword(fields[idx].clone()),
            values[idx].clone()
          ]))
        } else {
          Ok(Calcit::Nil)
        }
      }
      Err(e) => Err(format!("nth expect usize, {}", e)),
    },
    (Some(_), None) => Err(format!("record nth expected a record and index, got: {:?}", xs)),
    (None, Some(_)) => Err(format!("record nth expected a record and index, got: {:?}", xs)),
    (_, _) => Err(format!("nth expected 2 argument, got: {}", CrListWrap(xs.to_owned()))),
  }
}

pub fn get(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Record(_name, fields, values)), Some(a)) => match a {
      Calcit::Str(k) | Calcit::Keyword(k) | Calcit::Symbol(k, ..) => match find_in_fields(fields, k) {
        Some(idx) => Ok(values[idx].clone()),
        None => Ok(Calcit::Nil),
      },
      a => Err(format!("record field expected to be string/keyword, got {}", a)),
    },
    (Some(a), ..) => Err(format!("record &get expected record, got: {}", a)),
    (None, ..) => Err(format!("record &get expected 2 arguments, got: {:?}", xs)),
  }
}
