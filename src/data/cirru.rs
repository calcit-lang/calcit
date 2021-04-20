use crate::primes::CalcitData::*;
use crate::primes::{CalcitData, CalcitItems};
use cirru_parser::CirruNode;
use cirru_parser::CirruNode::*;
use regex::Regex;

/// code is CirruNode, and this function parse code(rather than data)
pub fn code_to_calcit(xs: &CirruNode, ns: &str) -> Result<CalcitData, String> {
  match xs {
    CirruLeaf(s) => match s.as_str() {
      "nil" => Ok(CalcitNil),
      "true" => Ok(CalcitBool(true)),
      "false" => Ok(CalcitBool(false)),
      "" => Err(String::from("Empty string is invalid")),
      _ => match s.chars().next().unwrap() {
        ':' => Ok(CalcitKeyword(String::from(&s[1..]))),
        '"' | '|' => Ok(CalcitString(String::from(&s[1..]))),
        '\'' => Ok(CalcitList(im::vector![
          CalcitSymbol(String::from("quote"), ns.to_string()),
          CalcitSymbol(String::from(&s[1..]), ns.to_string()),
        ])),
        '~' if s.starts_with("~@") && s.len() > 2 => Ok(CalcitList(im::vector![
          CalcitSymbol(String::from("~@"), ns.to_string()),
          CalcitSymbol(String::from(&s[2..]), ns.to_string()),
        ])),
        '~' if s.len() > 1 && !s.starts_with("~@") => Ok(CalcitList(im::vector![
          CalcitSymbol(String::from("~"), ns.to_string()),
          CalcitSymbol(String::from(&s[1..]), ns.to_string()),
        ])),
        '@' => Ok(CalcitList(im::vector![
          CalcitSymbol(String::from("@"), ns.to_string()),
          CalcitSymbol(String::from(&s[1..]), ns.to_string()),
        ])),
        // TODO future work of reader literal expanding
        _ => {
          if matches_float(&s) {
            let f: f32 = s.parse().unwrap();
            Ok(CalcitNumber(f))
          } else {
            Ok(CalcitSymbol(s.clone(), ns.to_string()))
          }
        }
      },
    },
    CirruList(ys) => {
      let mut zs: CalcitItems = im::Vector::new();
      for y in ys {
        match code_to_calcit(y, ns) {
          Ok(v) => {
            if !is_comment(&v) {
              zs.push_back(v.clone())
            } else {
            }
          }
          Err(e) => return Err(e),
        }
      }
      Ok(CalcitList(zs))
    }
  }
}

/// transform Cirru to Calcit data directly
pub fn cirru_to_calcit(xs: &CirruNode) -> CalcitData {
  match xs {
    CirruLeaf(s) => CalcitString(s.clone()),
    CirruList(ys) => {
      let mut zs: CalcitItems = im::vector![];
      for y in ys {
        zs.push_back(cirru_to_calcit(y))
      }
      CalcitList(zs)
    }
  }
}

/// for generate Cirru via calcit data manually
pub fn calcit_data_to_cirru(xs: &CalcitData) -> Result<CirruNode, String> {
  match xs {
    CalcitNil => Ok(CirruLeaf("nil".to_string())),
    CalcitBool(b) => Ok(CirruLeaf(b.to_string())),
    CalcitNumber(n) => Ok(CirruLeaf(n.to_string())),
    CalcitString(s) => Ok(CirruLeaf(s.clone())),
    CalcitList(ys) => {
      let mut zs: Vec<CirruNode> = vec![];
      for y in ys {
        match calcit_data_to_cirru(y) {
          Ok(v) => {
            zs.push(v);
          }
          Err(e) => return Err(e),
        }
      }
      Ok(CirruList(zs))
    }
    a => return Err(format!("unknown data for cirru: {}", a)),
  }
}

lazy_static! {
  static ref RE_FLOAT: Regex = Regex::new("^-?[\\d]+(\\.[\\d]+)?$").unwrap(); // TODO special cases not handled
}

fn matches_float(x: &str) -> bool {
  RE_FLOAT.is_match(x)
}

fn is_comment(x: &CalcitData) -> bool {
  match x {
    CalcitList(ys) => match ys.get(0) {
      Some(CalcitSymbol(s, _ns)) => s == ";",
      _ => false,
    },
    _ => false,
  }
}

/// converting data for display in Cirru syntax
pub fn calcit_to_cirru(x: &CalcitData) -> CirruNode {
  match x {
    CalcitNil => CirruLeaf(String::from("nil")),
    CalcitBool(true) => CirruLeaf(String::from("true")),
    CalcitBool(false) => CirruLeaf(String::from("false")),
    CalcitNumber(n) => CirruLeaf(n.to_string()),
    CalcitString(s) => CirruLeaf(format!("|{}", s)), // TODO performance
    CalcitSymbol(s, _ns) => CirruLeaf(s.to_string()), // TODO performance
    CalcitKeyword(s) => CirruLeaf(format!(":{}", s)), // TODO performance
    CalcitList(xs) => {
      let mut ys: Vec<CirruNode> = vec![];
      for x in xs {
        ys.push(calcit_to_cirru(x));
      }
      CirruList(ys)
    }
    a => CirruList(vec![
      CirruLeaf(String::from("TODO")),
      CirruLeaf(a.to_string()),
    ]),
  }
}
