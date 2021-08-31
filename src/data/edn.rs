use crate::data::cirru;
use crate::primes;
use crate::primes::Calcit;
use cirru_edn::Edn;
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;

// values does not fit are just represented with specical indicates
pub fn calcit_to_edn(x: &Calcit) -> Result<Edn, String> {
  match x {
    Calcit::Nil => Ok(Edn::Nil),
    Calcit::Bool(b) => Ok(Edn::Bool(*b)),
    Calcit::Str(s) => Ok(Edn::Str(s.to_owned())),
    Calcit::Number(n) => Ok(Edn::Number(*n)), // TODO
    Calcit::Keyword(s) => Ok(Edn::Keyword(s.to_owned())),
    Calcit::Symbol(s, ..) => Ok(Edn::Symbol(s.to_owned())),
    Calcit::List(xs) => {
      let mut ys: Vec<Edn> = vec![];
      for x in xs {
        ys.push(calcit_to_edn(x)?);
      }
      Ok(Edn::List(ys))
    }
    Calcit::Set(xs) => {
      let mut ys: HashSet<Edn> = HashSet::new();
      for x in xs {
        ys.insert(calcit_to_edn(x)?);
      }
      Ok(Edn::Set(ys))
    }
    Calcit::Map(xs) => {
      let mut ys: HashMap<Edn, Edn> = HashMap::new();
      for (k, x) in xs {
        ys.insert(calcit_to_edn(k)?, calcit_to_edn(x)?);
      }
      Ok(Edn::Map(ys))
    }
    Calcit::Record(name, fields, values) => {
      let mut ys: Vec<Edn> = vec![];
      for v in values {
        ys.push(calcit_to_edn(v)?)
      }
      Ok(Edn::Record(name.to_owned(), fields.to_owned(), ys))
    }
    Calcit::Fn(name, ..) => Err(format!("unable to generate EDN from function: {}", name)),
    Calcit::Proc(name) => Ok(Edn::Symbol(name.to_owned())),
    Calcit::Syntax(name, _ns) => Ok(Edn::Symbol(name.to_owned())),
    Calcit::Tuple(tag, data) => {
      match &**tag {
        Calcit::Symbol(sym, ..) => {
          if sym == "quote" {
            match cirru::calcit_data_to_cirru(&**data) {
              Ok(v) => Ok(Edn::Quote(v)),
              Err(e) => Err(format!("failed to create quote: {}", e)), // TODO more types to handle
            }
          } else {
            Err(format!("unknown tag for EDN: {}", sym)) // TODO more types to handle
          }
        }
        v => {
          Err(format!("unknonwn tag type for EDN: {}", v)) // TODO more types to handle
        }
      }
    }
    a => Err(format!("not able to generate EDN: {}", a)), // TODO more types to handle
  }
}

pub fn edn_to_calcit(x: &Edn) -> Calcit {
  match x {
    Edn::Nil => Calcit::Nil,
    Edn::Bool(b) => Calcit::Bool(*b),
    Edn::Number(n) => Calcit::Number(*n as f64),
    Edn::Symbol(s) => Calcit::Symbol(
      s.to_owned(),
      String::from(primes::GENERATED_NS),
      String::from(primes::GENERATED_DEF),
      None,
    ),
    Edn::Keyword(s) => Calcit::Keyword(s.to_owned()),
    Edn::Str(s) => Calcit::Str(s.to_owned()),
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
      Calcit::Record(name.to_owned(), fields.to_owned(), ys)
    }
  }
}
