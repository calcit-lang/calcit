use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;
use std::sync::Arc;

use im_ternary_tree::TernaryTreeList;

use crate::data::cirru;
use crate::primes;
use crate::primes::Calcit;

use cirru_edn::{Edn, EdnKwd};

// values does not fit are just represented with specical indicates
pub fn calcit_to_edn(x: &Calcit) -> Result<Edn, String> {
  match x {
    Calcit::Nil => Ok(Edn::Nil),
    Calcit::Bool(b) => Ok(Edn::Bool(*b)),
    Calcit::Str(s) => Ok(Edn::Str((**s).into())),
    Calcit::Number(n) => Ok(Edn::Number(*n)),
    Calcit::Keyword(s) => Ok(Edn::Keyword(s.to_owned())),
    Calcit::Symbol { sym, .. } => Ok(Edn::Symbol((**sym).into())),
    Calcit::List(xs) => {
      let mut ys: Vec<Edn> = Vec::with_capacity(xs.len());
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
      let mut ys: HashMap<Edn, Edn> = HashMap::with_capacity(xs.size());
      for (k, x) in xs {
        ys.insert(calcit_to_edn(k)?, calcit_to_edn(x)?);
      }
      Ok(Edn::Map(ys))
    }
    Calcit::Record(name, fields, values) => {
      let mut entries: Vec<(EdnKwd, Edn)> = Vec::with_capacity(fields.len());
      for idx in 0..fields.len() {
        entries.push((fields[idx].to_owned(), calcit_to_edn(&values[idx])?));
      }
      Ok(Edn::Record(name.to_owned(), entries))
    }
    Calcit::Fn { name, def_ns, args, .. } => {
      println!("[Warn] fn to EDN: {}/{} {:?}", def_ns, name, args);
      Ok(Edn::str(x.to_string()))
    }
    Calcit::Proc(name) => Ok(Edn::Symbol((**name).into())),
    Calcit::Syntax(name, _ns) => Ok(Edn::sym(name.to_string())),
    Calcit::Tuple(tag, data) => {
      match &**tag {
        Calcit::Symbol { sym, .. } => {
          if &**sym == "quote" {
            match cirru::calcit_data_to_cirru(data) {
              Ok(v) => Ok(Edn::Quote(v)),
              Err(e) => Err(format!("failed to create quote: {}", e)), // TODO more types to handle
            }
          } else {
            Err(format!("unknown tag for EDN: {}", sym)) // TODO more types to handle
          }
        }
        Calcit::Record(name, _, _) => Ok(Edn::tuple(Edn::Keyword(name.to_owned()), calcit_to_edn(data)?)),
        v => {
          Err(format!("EDN tuple expected 'quote or record, unknown tag: {}", v))
          // TODO more types to handle
        }
      }
    }
    Calcit::Buffer(buf) => Ok(Edn::Buffer(buf.to_owned())),
    Calcit::CirruQuote(code) => Ok(Edn::Quote(code.to_owned())),
    a => Err(format!("not able to generate EDN: {}", a)), // TODO more types to handle
  }
}

pub fn edn_to_calcit(x: &Edn) -> Calcit {
  match x {
    Edn::Nil => Calcit::Nil,
    Edn::Bool(b) => Calcit::Bool(*b),
    Edn::Number(n) => Calcit::Number(*n as f64),
    Edn::Symbol(s) => Calcit::Symbol {
      sym: (**s).into(),
      ns: primes::GEN_NS.into(),
      at_def: primes::GENERATED_DEF.into(),
      resolved: None,
      location: None,
    },
    Edn::Keyword(s) => Calcit::Keyword(s.to_owned()),
    Edn::Str(s) => Calcit::Str((**s).into()),
    Edn::Quote(nodes) => Calcit::CirruQuote(nodes.to_owned()),
    Edn::Tuple(pair) => Calcit::Tuple(Arc::new(edn_to_calcit(&pair.0)), Arc::new(edn_to_calcit(&pair.1))),
    Edn::List(xs) => {
      let mut ys: primes::CalcitItems = TernaryTreeList::Empty;
      for x in xs {
        ys = ys.push_right(edn_to_calcit(x))
      }
      Calcit::List(ys)
    }
    Edn::Set(xs) => {
      let mut ys: rpds::HashTrieSetSync<Calcit> = rpds::HashTrieSet::new_sync();
      for x in xs {
        ys.insert_mut(edn_to_calcit(x));
      }
      Calcit::Set(ys)
    }
    Edn::Map(xs) => {
      let mut ys: rpds::HashTrieMapSync<Calcit, Calcit> = rpds::HashTrieMap::new_sync();
      for (k, v) in xs {
        ys.insert_mut(edn_to_calcit(k), edn_to_calcit(v));
      }
      Calcit::Map(ys)
    }
    Edn::Record(name, entries) => {
      let mut fields: Vec<EdnKwd> = Vec::with_capacity(entries.len());
      let mut values: Vec<Calcit> = Vec::with_capacity(entries.len());
      let mut sorted = entries.to_owned();
      sorted.sort_by(|(a, _), (b, _)| a.cmp(b));
      for v in sorted {
        fields.push(v.0.to_owned());
        values.push(edn_to_calcit(&v.1));
      }
      Calcit::Record(name.to_owned(), Arc::new(fields), Arc::new(values))
    }
    Edn::Buffer(buf) => Calcit::Buffer(buf.to_owned()),
  }
}
