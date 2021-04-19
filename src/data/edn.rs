use crate::primes::{CalcitData, CalcitData::*};
use cirru_edn::CirruEdn;
use cirru_edn::CirruEdn::*;
use cirru_parser::CirruNode;
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;

pub fn as_string(data: CirruEdn) -> Result<String, String> {
  match data {
    CirruEdnString(s) => Ok(s),
    a => Err(format!("failed to convert to string: {}", a)),
  }
}

#[allow(dead_code)]
pub fn as_bool(data: CirruEdn) -> Result<bool, String> {
  match data {
    CirruEdnBool(b) => Ok(b),
    a => Err(format!("failed to convert to bool: {}", a)),
  }
}

#[allow(dead_code)]
pub fn as_number(data: CirruEdn) -> Result<f32, String> {
  match data {
    CirruEdnNumber(n) => Ok(n),
    a => Err(format!("failed to convert to number: {}", a)),
  }
}

pub fn as_cirru(data: CirruEdn) -> Result<CirruNode, String> {
  match data {
    CirruEdnQuote(c) => Ok(c),
    a => Err(format!("failed to convert to cirru code: {}", a)),
  }
}

pub fn as_vec(data: CirruEdn) -> Result<Vec<CirruEdn>, String> {
  match data {
    CirruEdnList(xs) => Ok(xs),
    CirruEdnNil => Err(String::from("cannot get from nil")),
    a => Err(format!("failed to convert to vec: {}", a)),
  }
}

pub fn as_map(data: CirruEdn) -> Result<HashMap<CirruEdn, CirruEdn>, String> {
  match data {
    CirruEdnMap(xs) => Ok(xs),
    CirruEdnNil => Err(String::from("cannot get from nil")),
    a => Err(format!("failed to convert to map: {}", a)),
  }
}

/// detects by index
#[allow(dead_code)]
pub fn vec_get(data: &CirruEdn, idx: usize) -> CirruEdn {
  match data {
    CirruEdnList(xs) => {
      if idx < xs.len() {
        xs[idx].clone()
      } else {
        CirruEdnNil
      }
    }
    _ => CirruEdnNil,
  }
}

/// detects by keyword then string, return nil if not found
pub fn map_get(data: &CirruEdn, k: &str) -> CirruEdn {
  let key: String = k.to_string();
  match data {
    CirruEdnMap(xs) => {
      if xs.contains_key(&CirruEdnKeyword(key.clone())) {
        xs[&CirruEdnKeyword(key)].clone()
      } else if xs.contains_key(&CirruEdnString(key.clone())) {
        xs[&CirruEdnString(key)].clone()
      } else {
        CirruEdnNil
      }
    }
    _ => CirruEdnNil,
  }
}

// values does not fit are just represented with specical indicates
pub fn calcit_to_edn(x: &CalcitData) -> CirruEdn {
  match x {
    CalcitNil => CirruEdnNil,
    CalcitBool(b) => CirruEdnBool(*b),
    CalcitString(s) => CirruEdnString(s.clone()),
    CalcitNumber(n) => CirruEdnNumber(*n),
    CalcitKeyword(s) => CirruEdnKeyword(s.clone()),
    CalcitSymbol(s, _ns) => CirruEdnSymbol(s.clone()),
    CalcitList(xs) => {
      let mut ys: Vec<CirruEdn> = vec![];
      for x in xs {
        ys.push(calcit_to_edn(x));
      }
      CirruEdnList(ys)
    }
    CalcitSet(xs) => {
      let mut ys: HashSet<CirruEdn> = HashSet::new();
      for x in xs {
        ys.insert(calcit_to_edn(x));
      }
      CirruEdnSet(ys)
    }
    CalcitMap(xs) => {
      let mut ys: HashMap<CirruEdn, CirruEdn> = HashMap::new();
      for (k, x) in xs {
        ys.insert(calcit_to_edn(k), calcit_to_edn(x));
      }
      CirruEdnMap(ys)
    }
    CalcitFn(name, ..) => CirruEdnString(format!("&fn {}", name)),
    CalcitProc(name) => CirruEdnString(format!("&proc {}", name)),
    a => CirruEdnString(format!("TODO {}", a)), // TODO more types to handle
  }
}

// TODO
pub fn edn_to_calcit(x: CirruEdn) -> CalcitData {
  CalcitNil
}
