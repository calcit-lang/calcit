use crate::data::cirru;
use crate::primes;
use crate::primes::Calcit;
use cirru_edn::Edn;
use cirru_parser::Cirru;
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;

pub fn as_string(data: Edn) -> Result<String, String> {
  match data {
    Edn::Str(s) => Ok(s),
    a => Err(format!("failed to convert to string: {}", a)),
  }
}

#[allow(dead_code)]
pub fn as_bool(data: Edn) -> Result<bool, String> {
  match data {
    Edn::Bool(b) => Ok(b),
    a => Err(format!("failed to convert to bool: {}", a)),
  }
}

#[allow(dead_code)]
pub fn as_number(data: Edn) -> Result<f64, String> {
  match data {
    Edn::Number(n) => Ok(n as f64),
    a => Err(format!("failed to convert to number: {}", a)),
  }
}

pub fn as_cirru(data: Edn) -> Result<Cirru, String> {
  match data {
    Edn::Quote(c) => Ok(c),
    a => Err(format!("failed to convert to cirru code: {}", a)),
  }
}

pub fn as_vec(data: Edn) -> Result<Vec<Edn>, String> {
  match data {
    Edn::List(xs) => Ok(xs),
    Edn::Nil => Err(String::from("cannot get from nil")),
    a => Err(format!("failed to convert to vec: {}", a)),
  }
}

pub fn as_map(data: Edn) -> Result<HashMap<Edn, Edn>, String> {
  match data {
    Edn::Map(xs) => Ok(xs),
    Edn::Nil => Err(String::from("cannot get from nil")),
    a => Err(format!("failed to convert to map: {}", a)),
  }
}

/// detects by index
#[allow(dead_code)]
pub fn vec_get(data: &Edn, idx: usize) -> Edn {
  match data {
    Edn::List(xs) => {
      if idx < xs.len() {
        xs[idx].clone()
      } else {
        Edn::Nil
      }
    }
    _ => Edn::Nil,
  }
}

/// detects by keyword then string, return nil if not found
pub fn map_get(data: &Edn, k: &str) -> Edn {
  let key: String = k.to_string();
  match data {
    Edn::Map(xs) => {
      if xs.contains_key(&Edn::Keyword(key.clone())) {
        xs[&Edn::Keyword(key)].clone()
      } else if xs.contains_key(&Edn::Str(key.clone())) {
        xs[&Edn::Str(key)].clone()
      } else {
        Edn::Nil
      }
    }
    _ => Edn::Nil,
  }
}

// values does not fit are just represented with specical indicates
pub fn calcit_to_edn(x: &Calcit) -> Edn {
  match x {
    Calcit::Nil => Edn::Nil,
    Calcit::Bool(b) => Edn::Bool(*b),
    Calcit::Str(s) => Edn::Str(s.clone()),
    Calcit::Number(n) => Edn::Number(*n as f32), // TODO
    Calcit::Keyword(s) => Edn::Keyword(s.clone()),
    Calcit::Symbol(s, ..) => Edn::Symbol(s.clone()),
    Calcit::List(xs) => {
      let mut ys: Vec<Edn> = vec![];
      for x in xs {
        ys.push(calcit_to_edn(x));
      }
      Edn::List(ys)
    }
    Calcit::Set(xs) => {
      let mut ys: HashSet<Edn> = HashSet::new();
      for x in xs {
        ys.insert(calcit_to_edn(x));
      }
      Edn::Set(ys)
    }
    Calcit::Map(xs) => {
      let mut ys: HashMap<Edn, Edn> = HashMap::new();
      for (k, x) in xs {
        ys.insert(calcit_to_edn(k), calcit_to_edn(x));
      }
      Edn::Map(ys)
    }
    Calcit::Record(name, fields, values) => {
      let mut ys: Vec<Edn> = vec![];
      for v in values {
        ys.push(calcit_to_edn(v))
      }
      Edn::Record(name.clone(), fields.clone(), ys)
    }
    Calcit::Fn(name, ..) => Edn::Str(format!("&fn {}", name)),
    Calcit::Proc(name) => Edn::Str(format!("&proc {}", name)),
    a => Edn::Str(format!("TODO {}", a)), // TODO more types to handle
  }
}

pub fn edn_to_calcit(x: &Edn) -> Calcit {
  match x {
    Edn::Nil => Calcit::Nil,
    Edn::Bool(b) => Calcit::Bool(*b),
    Edn::Number(n) => Calcit::Number(*n as f64),
    Edn::Symbol(s) => Calcit::Symbol(s.clone(), primes::GENERATED_NS.to_string(), None),
    Edn::Keyword(s) => Calcit::Keyword(s.clone()),
    Edn::Str(s) => Calcit::Str(s.clone()),
    Edn::Quote(nodes) => cirru::cirru_to_calcit(nodes),
    Edn::List(xs) => {
      let mut ys: primes::CalcitItems = im::vector![];
      for x in xs {
        ys.push_back(edn_to_calcit(x))
      }
      Calcit::List(ys)
    }
    Edn::Set(xs) => {
      let mut ys: im::HashSet<Calcit> = im::HashSet::new();
      for x in xs {
        ys.insert(edn_to_calcit(x));
      }
      Calcit::Set(ys)
    }
    Edn::Map(xs) => {
      let mut ys: im::HashMap<Calcit, Calcit> = im::HashMap::new();
      for (k, v) in xs {
        ys.insert(edn_to_calcit(k), edn_to_calcit(v));
      }
      Calcit::Map(ys)
    }
    Edn::Record(name, fields, values) => {
      let mut ys: Vec<Calcit> = vec![];
      for v in values {
        ys.push(edn_to_calcit(v));
      }
      Calcit::Record(name.clone(), fields.clone(), ys)
    }
  }
}
