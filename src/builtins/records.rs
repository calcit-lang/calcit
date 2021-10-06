use std::cmp::Ordering;
use std::ops::Rem;

use crate::primes::{keyword::load_order_key, lookup_order_kwd_str, Calcit, CalcitItems};

pub fn new_record(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Symbol(s, ..)) => {
      let mut fields: Vec<usize> = vec![];
      let mut values: Vec<Calcit> = vec![];

      for (idx, x) in xs.iter().enumerate() {
        if idx > 0 {
          match x {
            Calcit::Symbol(s, ..) | Calcit::Str(s) => {
              fields.push(load_order_key(s));
            }
            Calcit::Keyword(s) => {
              fields.push(s.to_owned());
            }
            a => return Err(format!("new-record fields accepets keyword/string, got a {}", a)),
          }
          values.push(Calcit::Nil);
        }
      }
      fields.sort_unstable(); // all values are nil
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
        let mut fields: Vec<usize> = def_fields.to_owned();
        let mut values: Vec<Calcit> = v0.to_owned();

        for idx in 0..size {
          let k_idx = idx * 2 + 1;
          let v_idx = k_idx + 1;
          match &xs[k_idx] {
            Calcit::Keyword(s) => match find_in_fields(def_fields, s.to_owned()) {
              Some(pos) => {
                fields[pos] = s.to_owned();
                values[pos] = xs[v_idx].to_owned();
              }
              None => return Err(format!("unexpected field {} for {:?}", s, def_fields)),
            },
            Calcit::Symbol(s, ..) | Calcit::Str(s) => match find_in_fields(def_fields, load_order_key(s)) {
              Some(pos) => {
                fields[pos] = load_order_key(s);
                values[pos] = xs[v_idx].to_owned();
              }
              None => return Err(format!("unexpected field {} for {:?}", s, def_fields)),
            },
            a => return Err(format!("expected field in string/keyword, got: {}", a)),
          }
        }

        Ok(Calcit::Record(name.to_owned(), fields, values))
      } else {
        Err(format!("&%{{}} expected pairs, got: {:?}", xs))
      }
    }
    a => Err(format!("&%{{}} expected a record as prototype, got {}", a)),
  }
}

pub fn record_from_map(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Record(name, fields, _values)), Some(Calcit::Map(ys))) => {
      let mut pairs: Vec<(String, Calcit)> = vec![];
      for (k, v) in ys {
        match k {
          Calcit::Str(s) => {
            pairs.push((s.to_owned(), v.to_owned()));
          }
          Calcit::Keyword(s) => {
            pairs.push((lookup_order_kwd_str(s), v.to_owned()));
          }
          a => return Err(format!("unknown field {}", a)),
        }
      }
      if fields.len() != pairs.len() {
        return Err(format!("invalid fields {:?} for record {:?}", pairs, fields));
      }
      pairs.sort_by(|(a, _), (b, _)| load_order_key(a).cmp(&load_order_key(b)));
      let mut values: Vec<Calcit> = vec![];
      for idx in 0..fields.len() {
        let (k, v) = &pairs[idx];
        if fields[idx] == load_order_key(k) {
          values.push(v.to_owned());
        } else {
          return Err(format!(
            "field mismatch: {} {} in {:?} {:?}",
            load_order_key(k),
            fields[idx],
            fields,
            pairs
          ));
        }
      }
      Ok(Calcit::Record(name.to_owned(), fields.to_owned(), values))
    }
    (Some(a), Some(b)) => Err(format!("&record:from-map expected a record and a map, got {} {}", a, b)),
    (_, _) => Err(format!("&record:from-map expected 2 arguments, got {:?}", xs)),
  }
}

pub fn get_record_name(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Record(name, ..)) => Ok(Calcit::Str(name.to_owned())),
    Some(a) => Err(format!("&record:get-name expected record, got: {}", a)),
    None => Err(String::from("&record:get-name expected record, got nothing")),
  }
}
pub fn turn_map(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Record(_name, fields, values)) => {
      let mut ys: im::HashMap<Calcit, Calcit> = im::HashMap::new();
      for idx in 0..fields.len() {
        ys.insert(Calcit::Keyword(fields[idx].to_owned()), values[idx].to_owned());
      }
      Ok(Calcit::Map(ys))
    }
    Some(a) => Err(format!("&record:to-map expected a record, got {}", a)),
    None => Err(String::from("&record:to-map expected 1 argument, got nothing")),
  }
}
pub fn matches(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Record(left, left_fields, ..)), Some(Calcit::Record(right, right_fields, ..))) => {
      Ok(Calcit::Bool(left == right && left_fields == right_fields))
    }
    (Some(a), Some(b)) => Err(format!("&record:matches? expected 2 records, got {} {}", a, b)),
    (_, _) => Err(format!("&record:matches? expected 2 arguments, got {:?}", xs)),
  }
}

pub fn find_in_fields(xs: &[usize], y: usize) -> Option<usize> {
  if xs.is_empty() {
    return None;
  }
  let mut lower = 0;
  let mut upper = xs.len() - 1;

  while (upper - lower) > 1 {
    let pos = (lower + upper) >> 1;
    let v = xs[pos].to_owned();
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
      Calcit::Str(k) | Calcit::Symbol(k, ..) => Ok(Calcit::Bool(find_in_fields(fields, load_order_key(k)).is_some())),
      Calcit::Keyword(k) => Ok(Calcit::Bool(find_in_fields(fields, k.to_owned()).is_some())),
      a => Err(format!("contains? got invalid field for record: {}", a)),
    },
    (Some(a), ..) => Err(format!("record contains? expected a record, got: {}", a)),
    (None, ..) => Err(format!("record contains? expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn get(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Record(_name, fields, values)), Some(a)) => match a {
      Calcit::Str(k) | Calcit::Symbol(k, ..) => match find_in_fields(fields, load_order_key(k)) {
        Some(idx) => Ok(values[idx].to_owned()),
        None => Ok(Calcit::Nil),
      },
      Calcit::Keyword(k) => match find_in_fields(fields, k.to_owned()) {
        Some(idx) => Ok(values[idx].to_owned()),
        None => Ok(Calcit::Nil),
      },
      a => Err(format!("record field expected to be string/keyword, got {}", a)),
    },
    (Some(a), ..) => Err(format!("record &get expected record, got: {}", a)),
    (None, ..) => Err(format!("record &get expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn assoc(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1), xs.get(2)) {
    (Some(Calcit::Record(name, fields, values)), Some(a), Some(b)) => match a {
      Calcit::Str(s) | Calcit::Symbol(s, ..) => match find_in_fields(fields, load_order_key(s)) {
        Some(pos) => {
          let mut new_values = values.to_owned();
          new_values[pos] = b.to_owned();
          Ok(Calcit::Record(name.to_owned(), fields.to_owned(), new_values))
        }
        None => Err(format!("invalid field `{}` for {:?}", s, fields)),
      },
      Calcit::Keyword(s) => match find_in_fields(fields, s.to_owned()) {
        Some(pos) => {
          let mut new_values = values.to_owned();
          new_values[pos] = b.to_owned();
          Ok(Calcit::Record(name.to_owned(), fields.to_owned(), new_values))
        }
        None => Err(format!("invalid field `{}` for {:?}", s, fields)),
      },
      a => Err(format!("invalid field `{}` for {:?}", a, fields)),
    },
    (Some(a), ..) => Err(format!("record:assoc expected a record, got: {}", a)),
    (None, ..) => Err(format!("record:assoc expected 3 arguments, got: {:?}", xs)),
  }
}

pub fn extend_as(xs: &CalcitItems) -> Result<Calcit, String> {
  if xs.len() != 4 {
    return Err(format!("record:extend-as expected 4 arguments, got: {:?}", xs));
  }
  match (xs.get(0), xs.get(1), xs.get(2), xs.get(3)) {
    (Some(Calcit::Record(_name, fields, values)), Some(n), Some(a), Some(new_value)) => match a {
      Calcit::Str(s) | Calcit::Symbol(s, ..) => match find_in_fields(fields, load_order_key(s)) {
        Some(_pos) => Err(format!("field `{}` already existed", s)),
        None => extend_record_field(s, n, fields, values, new_value),
      },
      Calcit::Keyword(s) => match find_in_fields(fields, s.to_owned()) {
        Some(_pos) => Err(format!("field `{}` already existed", s)),
        None => extend_record_field(&lookup_order_kwd_str(s), n, fields, values, new_value),
      },
      a => return Err(format!("invalid field `{}` for {:?}", a, fields)),
    },
    (Some(a), ..) => return Err(format!("record:extend-as expected a record, got: {}", a)),
    (None, ..) => return Err(format!("record:extend-as expected 4 arguments, got: {:?}", xs)),
  }
}

fn extend_record_field(
  s: &str,
  n: &Calcit,
  fields: &[usize],
  values: &[Calcit],
  new_value: &Calcit,
) -> Result<Calcit, String> {
  let mut next_fields: Vec<usize> = vec![];
  let mut next_values: Vec<Calcit> = vec![];
  let mut inserted: bool = false;
  let idx_s = load_order_key(s);

  for (i, k) in fields.iter().enumerate() {
    if inserted {
      next_fields.push(k.to_owned());
      next_values.push(values[i].to_owned());
    } else {
      match idx_s.cmp(k) {
        Ordering::Less => {
          next_fields.push(idx_s);
          next_values.push(new_value.to_owned());

          next_fields.push(k.to_owned());
          next_values.push(values[i].to_owned());
          inserted = true;
        }
        Ordering::Greater => {
          next_fields.push(k.to_owned());
          next_values.push(values[i].to_owned());
        }
        Ordering::Equal => {
          unreachable!("does not equal")
        }
      }
    }
  }
  if !inserted {
    next_fields.push(idx_s.to_owned());
    next_values.push(new_value.to_owned());
  }

  let new_name: Result<String, String> = match n {
    Calcit::Str(s) | Calcit::Symbol(s, ..) => Ok(s.to_owned()),
    Calcit::Keyword(s) => Ok(lookup_order_kwd_str(s)),
    _ => Err(format!("")),
  };

  Ok(Calcit::Record(new_name?, next_fields, next_values))
}
