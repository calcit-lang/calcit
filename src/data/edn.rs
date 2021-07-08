use crate::data::cirru;
use crate::primes;
use crate::primes::Calcit;
use cirru_edn::Edn;
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;

// values does not fit are just represented with specical indicates
pub fn calcit_to_edn(x: &Calcit) -> Edn {
  match x {
    Calcit::Nil => Edn::Nil,
    Calcit::Bool(b) => Edn::Bool(*b),
    Calcit::Str(s) => Edn::Str(s.clone()),
    Calcit::Number(n) => Edn::Number(*n), // TODO
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
    Calcit::Proc(name) => Edn::Symbol(name.to_owned()),
    Calcit::Syntax(name, _ns) => Edn::Symbol(name.to_owned()),
    Calcit::Tuple(tag, data) => {
      match &**tag {
        Calcit::Symbol(sym, ..) => {
          if sym == "quote" {
            match cirru::calcit_data_to_cirru(&**data) {
              Ok(v) => Edn::Quote(v),
              Err(e) => Edn::Str(format!("TODO quote {}", e)), // TODO more types to handle
            }
          } else {
            Edn::Str(format!("TODO {}", sym)) // TODO more types to handle
          }
        }
        v => {
          Edn::Str(format!("TODO {}", v)) // TODO more types to handle
        }
      }
    }
    a => Edn::Str(format!("TODO {}", a)), // TODO more types to handle
  }
}

pub fn edn_to_calcit(x: &Edn) -> Calcit {
  match x {
    Edn::Nil => Calcit::Nil,
    Edn::Bool(b) => Calcit::Bool(*b),
    Edn::Number(n) => Calcit::Number(*n as f64),
    Edn::Symbol(s) => Calcit::Symbol(
      s.clone(),
      String::from(primes::GENERATED_NS),
      String::from(primes::GENERATED_DEF),
      None,
    ),
    Edn::Keyword(s) => Calcit::Keyword(s.clone()),
    Edn::Str(s) => Calcit::Str(s.clone()),
    Edn::Quote(nodes) => Calcit::Tuple(
      Box::new(Calcit::Symbol(
        String::from("quote"),
        String::from(primes::GENERATED_NS),
        String::from(primes::GENERATED_DEF),
        None,
      )),
      Box::new(cirru::cirru_to_calcit(nodes)),
    ),
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
