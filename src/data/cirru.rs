use crate::primes::{Calcit, CalcitItems};
use cirru_parser::Cirru;
use regex::Regex;

/// code is CirruNode, and this function parse code(rather than data)
pub fn code_to_calcit(xs: &Cirru, ns: &str) -> Result<Calcit, String> {
  match xs {
    Cirru::Leaf(s) => match s.as_str() {
      "nil" => Ok(Calcit::Nil),
      "true" => Ok(Calcit::Bool(true)),
      "false" => Ok(Calcit::Bool(false)),
      "&E" => Ok(Calcit::Number(std::f64::consts::E)),
      "&PI" => Ok(Calcit::Number(std::f64::consts::PI)),
      "&newline" => Ok(Calcit::Str(String::from("\n"))),
      "&tab" => Ok(Calcit::Str(String::from("\t"))),
      "" => Err(String::from("Empty string is invalid")),
      _ => match s.chars().next().unwrap() {
        ':' => Ok(Calcit::Keyword(String::from(&s[1..]))),
        '"' | '|' => Ok(Calcit::Str(String::from(&s[1..]))),
        '0' if s.starts_with("0x") => match u8::from_str_radix(&s[2..], 16) {
          Ok(n) => Ok(Calcit::Number(n as f64)),
          Err(e) => Err(format!("failed to parse hex: {} => {:?}", s, e)),
        },
        '\'' => Ok(Calcit::List(im::vector![
          Calcit::Symbol(String::from("quote"), ns.to_string(), None),
          Calcit::Symbol(String::from(&s[1..]), ns.to_string(), None),
        ])),
        // TODO also detect simple variables
        '~' if s.starts_with("~@") && s.chars().count() > 2 => Ok(Calcit::List(im::vector![
          Calcit::Symbol(String::from("~@"), ns.to_string(), None),
          Calcit::Symbol(String::from(&s[2..]), ns.to_string(), None),
        ])),
        '~' if s.chars().count() > 1 && !s.starts_with("~@") => Ok(Calcit::List(im::vector![
          Calcit::Symbol(String::from("~"), ns.to_string(), None),
          Calcit::Symbol(String::from(&s[1..]), ns.to_string(), None),
        ])),
        '@' => Ok(Calcit::List(im::vector![
          Calcit::Symbol(String::from("deref"), ns.to_string(), None),
          Calcit::Symbol(String::from(&s[1..]), ns.to_string(), None),
        ])),
        // TODO future work of reader literal expanding
        _ => {
          if matches_float(&s) {
            let f: f64 = s.parse().unwrap();
            Ok(Calcit::Number(f))
          } else {
            Ok(Calcit::Symbol(s.clone(), ns.to_string(), None))
          }
        }
      },
    },
    Cirru::List(ys) => {
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
      Ok(Calcit::List(zs))
    }
  }
}

/// transform Cirru to Calcit data directly
pub fn cirru_to_calcit(xs: &Cirru) -> Calcit {
  match xs {
    Cirru::Leaf(s) => Calcit::Str(s.clone()),
    Cirru::List(ys) => {
      let mut zs: CalcitItems = im::vector![];
      for y in ys {
        zs.push_back(cirru_to_calcit(y))
      }
      Calcit::List(zs)
    }
  }
}

/// for generate Cirru via calcit data manually
pub fn calcit_data_to_cirru(xs: &Calcit) -> Result<Cirru, String> {
  match xs {
    Calcit::Nil => Ok(Cirru::Leaf("nil".to_string())),
    Calcit::Bool(b) => Ok(Cirru::Leaf(b.to_string())),
    Calcit::Number(n) => Ok(Cirru::Leaf(n.to_string())),
    Calcit::Str(s) => Ok(Cirru::Leaf(s.clone())),
    Calcit::List(ys) => {
      let mut zs: Vec<Cirru> = vec![];
      for y in ys {
        match calcit_data_to_cirru(y) {
          Ok(v) => {
            zs.push(v);
          }
          Err(e) => return Err(e),
        }
      }
      Ok(Cirru::List(zs))
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

fn is_comment(x: &Calcit) -> bool {
  match x {
    Calcit::List(ys) => match ys.get(0) {
      Some(Calcit::Symbol(s, ..)) => s == ";",
      _ => false,
    },
    _ => false,
  }
}

/// converting data for display in Cirru syntax
pub fn calcit_to_cirru(x: &Calcit) -> Cirru {
  match x {
    Calcit::Nil => Cirru::Leaf(String::from("nil")),
    Calcit::Bool(true) => Cirru::Leaf(String::from("true")),
    Calcit::Bool(false) => Cirru::Leaf(String::from("false")),
    Calcit::Number(n) => Cirru::Leaf(n.to_string()),
    Calcit::Str(s) => Cirru::Leaf(format!("|{}", s)), // TODO performance
    Calcit::Symbol(s, ..) => Cirru::Leaf(s.to_string()), // TODO performance
    Calcit::Keyword(s) => Cirru::Leaf(format!(":{}", s)), // TODO performance
    Calcit::List(xs) => {
      let mut ys: Vec<Cirru> = vec![];
      for x in xs {
        ys.push(calcit_to_cirru(x));
      }
      Cirru::List(ys)
    }
    a => Cirru::List(vec![Cirru::Leaf(String::from("TODO")), Cirru::Leaf(a.to_string())]),
  }
}
