use crate::builtins::records::find_in_fields;
use crate::primes::{Calcit, CalcitItems};

use crate::util::number::{f64_to_usize, is_even};

pub fn call_new_map(xs: &CalcitItems) -> Result<Calcit, String> {
  if is_even(xs.len()) {
    let n = xs.len() >> 1;
    let mut ys = im::HashMap::new();
    for i in 0..n {
      ys.insert(xs[i << 1].clone(), xs[(i << 1) + 1].clone());
    }
    Ok(Calcit::Map(ys))
  } else {
    Err(String::from("&{} expected even number of arguments"))
  }
}

pub fn assoc(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1), xs.get(2)) {
    (Some(Calcit::List(xs)), Some(Calcit::Number(n)), Some(a)) => match f64_to_usize(*n) {
      Ok(idx) => {
        if idx < xs.len() {
          let mut ys = xs.clone();
          ys[idx] = a.clone();
          Ok(Calcit::List(ys))
        } else {
          Ok(Calcit::Nil)
        }
      }
      Err(e) => Err(e),
    },
    (Some(Calcit::Map(xs)), Some(a), Some(b)) => {
      let ys = &mut xs.clone();
      ys.insert(a.clone(), b.clone());
      Ok(Calcit::Map(ys.clone()))
    }
    (Some(Calcit::Record(name, fields, values)), Some(a), Some(b)) => match a {
      Calcit::Str(s) | Calcit::Keyword(s) | Calcit::Symbol(s, ..) => match find_in_fields(fields, s) {
        Some(pos) => {
          let mut new_values = values.clone();
          new_values[pos] = b.clone();
          Ok(Calcit::Record(name.clone(), fields.clone(), new_values))
        }
        None => Err(format!("invalid field `{}` for {:?}", s, fields)),
      },
      a => Err(format!("invalid field `{}` for {:?}", a, fields)),
    },
    (Some(a), ..) => Err(format!("assoc expected list or map, got: {}", a)),
    (None, ..) => Err(format!("assoc expected 3 arguments, got: {:?}", xs)),
  }
}

pub fn dissoc(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Map(xs)), Some(a)) => {
      let ys = &mut xs.clone();
      ys.remove(a);
      Ok(Calcit::Map(ys.clone()))
    }
    (Some(Calcit::List(xs)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
      Ok(idx) => {
        let ys = &mut xs.clone();
        ys.remove(idx);
        Ok(Calcit::List(ys.clone()))
      }
      Err(e) => Err(format!("dissoc expected number, {}", e)),
    },
    (Some(a), ..) => Err(format!("dissoc expected a map, got: {}", a)),
    (_, _) => Err(format!("dissoc expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn map_get(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::List(xs)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
      Ok(idx) => match xs.get(idx) {
        Some(v) => Ok(v.clone()),
        None => Ok(Calcit::Nil),
      },
      Err(e) => Err(e),
    },
    (Some(Calcit::Str(s)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
      Ok(idx) => match s.chars().nth(idx) {
        Some(v) => Ok(Calcit::Str(v.to_string())),
        None => Ok(Calcit::Nil),
      },
      Err(e) => Err(e),
    },
    (Some(Calcit::Map(xs)), Some(a)) => {
      let ys = &mut xs.clone();
      match ys.get(a) {
        Some(v) => Ok(v.clone()),
        None => Ok(Calcit::Nil),
      }
    }
    (Some(Calcit::Record(_name, fields, values)), Some(a)) => match a {
      Calcit::Str(k) | Calcit::Keyword(k) | Calcit::Symbol(k, ..) => match find_in_fields(fields, k) {
        Some(idx) => Ok(values[idx].clone()),
        None => Ok(Calcit::Nil),
      },
      a => Err(format!("record field expected to be string/keyword, got {}", a)),
    },
    (Some(a), ..) => Err(format!("&get expected list or map, got: {}", a)),
    (None, ..) => Err(format!("&get expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn contains_ques(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::List(xs)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
      Ok(idx) => Ok(Calcit::Bool(idx < xs.len())),
      Err(e) => Err(e),
    },
    (Some(Calcit::Str(s)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
      Ok(idx) => Ok(Calcit::Bool(idx < s.chars().count())),
      Err(e) => Err(e),
    },
    (Some(Calcit::Map(xs)), Some(a)) => Ok(Calcit::Bool(xs.contains_key(a))),
    (Some(Calcit::Record(_name, fields, _)), Some(a)) => match a {
      Calcit::Str(k) | Calcit::Keyword(k) | Calcit::Symbol(k, ..) => {
        Ok(Calcit::Bool(find_in_fields(fields, k).is_some()))
      }
      a => Err(format!("contains? got invalid field for record: {}", a)),
    },
    (Some(a), ..) => Err(format!("contains? expected list or map, got: {}", a)),
    (None, ..) => Err(format!("contains? expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn call_merge(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Map(xs)), Some(Calcit::Map(ys))) => {
      let mut zs: im::HashMap<Calcit, Calcit> = xs.clone();
      for (k, v) in ys {
        zs.insert(k.clone(), v.clone());
      }
      Ok(Calcit::Map(zs))
    }
    (Some(Calcit::Record(name, fields, values)), Some(Calcit::Map(ys))) => {
      let mut new_values = values.clone();
      for (k, v) in ys {
        match k {
          Calcit::Str(s) | Calcit::Keyword(s) | Calcit::Symbol(s, ..) => match find_in_fields(fields, s) {
            Some(pos) => new_values[pos] = v.clone(),
            None => return Err(format!("invalid field `{}` for {:?}", s, fields)),
          },
          a => return Err(format!("invalid field key: {}", a)),
        }
      }
      Ok(Calcit::Record(name.clone(), fields.clone(), new_values))
    }
    (Some(a), Some(b)) => Err(format!("expected 2 maps, got: {} {}", a, b)),
    (_, _) => Err(format!("expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn includes_ques(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Nil), _) => Err(String::from("nil includes nothing")),
    (Some(Calcit::Map(ys)), Some(a)) => {
      for (_k, v) in ys {
        if v == a {
          return Ok(Calcit::Bool(true));
        }
      }
      Ok(Calcit::Bool(false))
    }
    (Some(Calcit::List(xs)), Some(a)) => Ok(Calcit::Bool(xs.contains(a))),
    (Some(Calcit::Set(xs)), Some(a)) => Ok(Calcit::Bool(xs.contains(a))),
    (Some(Calcit::Str(xs)), Some(Calcit::Str(a))) => Ok(Calcit::Bool(xs.contains(a))),
    (Some(Calcit::Str(_)), Some(a)) => Err(format!("string `contains?` expected a string, got: {}", a)),
    (Some(a), ..) => Err(format!("expected list, map, set, got: {}", a)),
    (None, ..) => Err(format!("expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn to_pairs(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    // get a random order from internals
    Some(Calcit::Map(ys)) => {
      let mut zs: im::HashSet<Calcit> = im::HashSet::new();
      for (k, v) in ys {
        zs.insert(Calcit::List(im::vector![k.clone(), v.clone(),]));
      }
      Ok(Calcit::Set(zs))
    }
    Some(Calcit::Record(_name, fields, values)) => {
      let mut zs: CalcitItems = im::vector![];
      for idx in 0..fields.len() {
        zs.push_back(Calcit::List(im::vector![
          Calcit::Keyword(fields[idx].clone()),
          values[idx].clone(),
        ]));
      }
      Ok(Calcit::List(zs))
    }
    Some(a) => Err(format!("to-pairs expected a map, got {}", a)),
    None => Err(String::from("to-pairs expected 1 argument, got nothing")),
  }
}

pub fn call_merge_non_nil(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Map(xs)), Some(Calcit::Map(ys))) => {
      let mut zs: im::HashMap<Calcit, Calcit> = xs.clone();
      for (k, v) in ys {
        if *v != Calcit::Nil {
          zs.insert(k.clone(), v.clone());
        }
      }
      Ok(Calcit::Map(zs))
    }
    (Some(a), Some(b)) => Err(format!("expected 2 maps, got: {} {}", a, b)),
    (_, _) => Err(format!("expected 2 arguments, got: {:?}", xs)),
  }
}

/// out to list, but with a arbitrary order
pub fn to_list(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Map(m)) => {
      let mut ys: im::Vector<Calcit> = im::vector![];
      for (k, v) in m {
        let zs: im::Vector<Calcit> = im::vector![k.to_owned(), v.to_owned()];
        ys.push_back(Calcit::List(zs));
      }
      Ok(Calcit::List(ys))
    }
    Some(a) => Err(format!("&map:to-list expected a map, got: {}", a)),
    None => Err(String::from("&map:to-list expected a map, got nothing")),
  }
}
