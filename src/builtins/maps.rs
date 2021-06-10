use crate::builtins::records::find_in_fields;
use crate::primes::{Calcit, CalcitItems};

use crate::util::number::is_even;

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

pub fn dissoc(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Map(xs)), Some(a)) => {
      let ys = &mut xs.clone();
      ys.remove(a);
      Ok(Calcit::Map(ys.clone()))
    }
    (Some(a), ..) => Err(format!("map dissoc expected a map, got: {}", a)),
    (_, _) => Err(format!("map dissoc expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn get(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Map(xs)), Some(a)) => {
      let ys = &mut xs.clone();
      match ys.get(a) {
        Some(v) => Ok(v.clone()),
        None => Ok(Calcit::Nil),
      }
    }
    (Some(a), ..) => Err(format!("map &get expected map, got: {}", a)),
    (None, ..) => Err(format!("map &get expected 2 arguments, got: {:?}", xs)),
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

pub fn count(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Map(ys)) => Ok(Calcit::Number(ys.len() as f64)),
    Some(a) => Err(format!("map count expected a map, got: {}", a)),
    None => Err(String::from("map count expected 1 argument")),
  }
}

pub fn empty_ques(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Map(ys)) => Ok(Calcit::Bool(ys.is_empty())),
    Some(a) => Err(format!("map empty? expected some map, got: {}", a)),
    None => Err(String::from("map empty? expected 1 argument")),
  }
}

pub fn contains_ques(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Map(xs)), Some(a)) => Ok(Calcit::Bool(xs.contains_key(a))),
    (Some(a), ..) => Err(format!("map contains? expected a map, got: {}", a)),
    (None, ..) => Err(format!("map contains? expected 2 arguments, got: {:?}", xs)),
  }
}

pub fn includes_ques(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Map(ys)), Some(a)) => {
      for (_k, v) in ys {
        if v == a {
          return Ok(Calcit::Bool(true));
        }
      }
      Ok(Calcit::Bool(false))
    }
    (Some(a), ..) => Err(format!("map `includes?` expected a map, got: {}", a)),
    (None, ..) => Err(format!("map `includes?` expected 2 arguments, got: {:?}", xs)),
  }
}

/// use builtin function since maps need to be handled specifically
pub fn first(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Map(ys)) => match ys.iter().next() {
      // TODO order may not be stable enough
      Some((k, v)) => Ok(Calcit::List(im::vector![k.to_owned(), v.to_owned()])),
      None => Ok(Calcit::Nil),
    },
    Some(a) => Err(format!("map:first expected a map, got: {}", a)),
    None => Err(String::from("map:first expected 1 argument")),
  }
}

pub fn rest(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Map(ys)) => match ys.keys().next() {
      Some(k0) => {
        let mut zs = ys.clone();
        zs.remove(k0);
        Ok(Calcit::Map(zs))
      }
      None => Ok(Calcit::Nil),
    },
    Some(a) => Err(format!("map:rest expected a map, got: {}", a)),
    None => Err(String::from("map:rest expected 1 argument")),
  }
}

pub fn assoc(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1), xs.get(2)) {
    (Some(Calcit::Map(xs)), Some(a), Some(b)) => {
      let ys = &mut xs.clone();
      ys.insert(a.clone(), b.clone());
      Ok(Calcit::Map(ys.clone()))
    }
    (Some(a), ..) => Err(format!("map:assoc expected a map, got: {}", a)),
    (None, ..) => Err(format!("map:assoc expected 3 arguments, got: {:?}", xs)),
  }
}
