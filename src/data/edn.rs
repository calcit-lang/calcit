use crate::data::cirru;
use crate::primes;
use crate::primes::{keyword::load_order_key, load_kwd, lookup_order_kwd_str, Calcit};
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
    Calcit::Keyword(s) => Ok(Edn::Keyword(lookup_order_kwd_str(s))),
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
      let mut entries: Vec<(String, Edn)> = vec![];
      for idx in 0..fields.len() {
        entries.push((lookup_order_kwd_str(&fields[idx]).to_owned(), calcit_to_edn(&values[idx])?));
      }
      Ok(Edn::Record(lookup_order_kwd_str(name), entries))
    }
    Calcit::Fn(..) => {
      println!("[Warning] unable to generate EDN from function: {}", x);
      Ok(Edn::Str(format!("TODO fn: {}", x)))
    }
    Calcit::Proc(name) => Ok(Edn::Symbol(name.to_owned())),
    Calcit::Syntax(name, _ns) => Ok(Edn::Symbol(name.to_string())),
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
        Calcit::Record(name, _, _) => Ok(Edn::Tuple(
          Box::new(Edn::Keyword(lookup_order_kwd_str(name))),
          Box::new(calcit_to_edn(data)?),
        )),
        v => {
          Err(format!("EDN tuple expected 'quote or record, unknown tag: {}", v))
          // TODO more types to handle
        }
      }
    }
    Calcit::Buffer(buf) => Ok(Edn::Buffer(buf.to_owned())),
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
    Edn::Keyword(s) => load_kwd(s),
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
    Edn::Tuple(tag, v) => Calcit::Tuple(Box::new(edn_to_calcit(&*tag)), Box::new(edn_to_calcit(&*v))),
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
    Edn::Record(name, entries) => {
      let mut fields: Vec<usize> = vec![];
      let mut values: Vec<Calcit> = vec![];
      let mut sorted = entries.to_owned();
      sorted.sort_by(|(a, _), (b, _)| load_order_key(a).cmp(&load_order_key(b)));
      for v in sorted {
        fields.push(load_order_key(&v.0).to_owned());
        values.push(edn_to_calcit(&v.1));
      }
      Calcit::Record(load_order_key(name), fields, values)
    }
    Edn::Buffer(buf) => Calcit::Buffer(buf.to_owned()),
  }
}
