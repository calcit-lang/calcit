use crate::primes::CalcitData;
use crate::primes::CalcitData::*;
use cirru_parser::CirruNode;
use cirru_parser::CirruNode::*;
use regex::Regex;

pub fn cirru_to_calcit(xs: CirruNode, ns: &str) -> Result<CalcitData, String> {
  match xs {
    CirruLeaf(s) => match s.as_str() {
      "nil" => Ok(CalcitNil),
      "true" => Ok(CalcitBool(true)),
      "false" => Ok(CalcitBool(false)),
      "" => Err(String::from("Empty string is invalid")),
      _ => match s.chars().nth(0).unwrap() {
        '\'' => Ok(CalcitSymbol(String::from(&s[1..]), String::from(ns))),
        ':' => Ok(CalcitKeyword(String::from(&s[1..]))),
        '"' | '|' => Ok(CalcitString(String::from(&s[1..]))),
        // TODO future work of reader literal expanding
        _ => {
          if matches_float(&s) {
            let f: f32 = s.parse().unwrap();
            Ok(CalcitNumber(f))
          } else {
            Ok(CalcitSymbol(s, format!("{}", ns)))
          }
        }
      },
    },
    CirruList(ys) => {
      let mut zs: im::Vector<CalcitData> = im::Vector::new();
      for y in ys {
        match cirru_to_calcit(y, ns) {
          Ok(v) => zs.push_back(v),
          Err(e) => return Err(e),
        }
      }
      Ok(CalcitList(zs))
    }
  }
}

fn matches_float(x: &str) -> bool {
  let re = Regex::new("^-?[\\d]+(\\.[\\d]+)?$").unwrap(); // TODO special cases not handled
  re.is_match(x)
}
