use crate::primes::CalcitData::*;
use crate::primes::{CalcitData, CalcitItems};
use cirru_parser::CirruNode;
use cirru_parser::CirruNode::*;
use regex::Regex;

pub fn cirru_to_calcit(xs: &CirruNode, ns: &str) -> Result<CalcitData, String> {
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
        '~' if s.len() > 1 => Ok(CalcitList(im::vector![
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
        match cirru_to_calcit(y, ns) {
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
