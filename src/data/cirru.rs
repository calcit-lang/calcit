use crate::primes::{Calcit, CalcitItems};
use cirru_parser::Cirru;

/// code is CirruNode, and this function parse code(rather than data)
pub fn code_to_calcit(xs: &Cirru, ns: &Box<str>, def: &Box<str>) -> Result<Calcit, String> {
  match xs {
    Cirru::Leaf(s) => match &**s {
      "nil" => Ok(Calcit::Nil),
      "true" => Ok(Calcit::Bool(true)),
      "false" => Ok(Calcit::Bool(false)),
      "&E" => Ok(Calcit::Number(std::f64::consts::E)),
      "&PI" => Ok(Calcit::Number(std::f64::consts::PI)),
      "&newline" => Ok(Calcit::new_str("\n")),
      "&tab" => Ok(Calcit::new_str("\t")),
      "&calcit-version" => Ok(Calcit::new_str(env!("CARGO_PKG_VERSION"))),
      "" => Err(String::from("Empty string is invalid")),
      // special tuple syntax
      "::" => Ok(Calcit::Symbol(s.to_owned(), ns.to_owned(), def.to_owned(), None)),
      _ => match s.chars().next().unwrap() {
        ':' => Ok(Calcit::kwd(&s[1..])),
        '.' => {
          if s.starts_with(".-") || s.starts_with(".!") {
            // try not to break js interop
            Ok(Calcit::Symbol(s.to_owned(), ns.to_owned(), def.to_owned(), None))
          } else {
            Ok(Calcit::Proc(s.to_owned())) // as native method syntax
          }
        }
        '"' | '|' => Ok(Calcit::new_str(&s[1..])),
        '0' if s.starts_with("0x") => match u32::from_str_radix(&s[2..], 16) {
          Ok(n) => Ok(Calcit::Number(n as f64)),
          Err(e) => Err(format!("failed to parse hex: {} => {:?}", s, e)),
        },
        '\'' if s.len() > 1 => Ok(Calcit::List(
          rpds::vector_sync![]
            .push_back(Calcit::Symbol(
              String::from("quote").into_boxed_str(),
              ns.to_owned(),
              def.to_owned(),
              None,
            ))
            .push_back(Calcit::Symbol(
              String::from(&s[1..]).into_boxed_str(),
              ns.to_owned(),
              def.to_owned(),
              None,
            )),
        )),
        // TODO also detect simple variables
        '~' if s.starts_with("~@") && s.chars().count() > 2 => Ok(Calcit::List(
          rpds::vector_sync![]
            .push_back(Calcit::Symbol(
              String::from("~@").into_boxed_str(),
              ns.to_owned(),
              def.to_owned(),
              None,
            ))
            .push_back(Calcit::Symbol(
              String::from(&s[2..]).into_boxed_str(),
              ns.to_owned(),
              def.to_owned(),
              None,
            )),
        )),
        '~' if s.chars().count() > 1 && !s.starts_with("~@") => Ok(Calcit::List(
          rpds::vector_sync![]
            .push_back(Calcit::Symbol(
              String::from("~").into_boxed_str(),
              ns.to_owned(),
              def.to_owned(),
              None,
            ))
            .push_back(Calcit::Symbol(
              String::from(&s[1..]).into_boxed_str(),
              ns.to_owned(),
              def.to_owned(),
              None,
            )),
        )),
        '@' => Ok(Calcit::List(
          rpds::vector_sync![]
            .push_back(Calcit::Symbol(
              String::from("deref").into_boxed_str(),
              ns.to_owned(),
              def.to_owned(),
              None,
            ))
            .push_back(Calcit::Symbol(
              String::from(&s[1..]).into_boxed_str(),
              ns.to_owned(),
              def.to_owned(),
              None,
            )),
        )),
        // TODO future work of reader literal expanding
        _ => {
          if let Ok(f) = s.parse::<f64>() {
            Ok(Calcit::Number(f))
          } else {
            Ok(Calcit::Symbol(s.to_owned(), ns.to_owned(), def.to_owned(), None))
          }
        }
      },
    },
    Cirru::List(ys) => {
      let mut zs: CalcitItems = rpds::VectorSync::new_sync();
      for y in ys {
        match code_to_calcit(y, ns, def) {
          Ok(v) => {
            if !is_comment(&v) {
              zs.push_back_mut(v.to_owned())
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
    Cirru::Leaf(s) => Calcit::Str(s.to_owned()),
    Cirru::List(ys) => {
      let mut zs: CalcitItems = rpds::vector_sync![];
      for y in ys {
        zs.push_back_mut(cirru_to_calcit(y))
      }
      Calcit::List(zs)
    }
  }
}

/// for generate Cirru via calcit data manually
pub fn calcit_data_to_cirru(xs: &Calcit) -> Result<Cirru, String> {
  match xs {
    Calcit::Nil => Ok(Cirru::leaf("nil")),
    Calcit::Bool(b) => Ok(Cirru::Leaf(b.to_string().into_boxed_str())),
    Calcit::Number(n) => Ok(Cirru::Leaf(n.to_string().into_boxed_str())),
    Calcit::Str(s) => Ok(Cirru::leaf(s.to_owned())),
    Calcit::List(ys) => {
      let mut zs: Vec<Cirru> = Vec::with_capacity(ys.len());
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

fn is_comment(x: &Calcit) -> bool {
  match x {
    Calcit::List(ys) => match ys.get(0) {
      Some(Calcit::Symbol(s, ..)) => &**s == ";",
      _ => false,
    },
    _ => false,
  }
}

/// converting data for display in Cirru syntax
pub fn calcit_to_cirru(x: &Calcit) -> Result<Cirru, String> {
  match x {
    Calcit::Nil => Ok(Cirru::leaf("nil")),
    Calcit::Bool(true) => Ok(Cirru::leaf("true")),
    Calcit::Bool(false) => Ok(Cirru::leaf("false")),
    Calcit::Number(n) => Ok(Cirru::Leaf(n.to_string().into_boxed_str())),
    Calcit::Str(s) => Ok(Cirru::leaf(format!("|{}", s))),     // TODO performance
    Calcit::Symbol(s, ..) => Ok(Cirru::Leaf(s.to_owned())),   // TODO performance
    Calcit::Keyword(s) => Ok(Cirru::leaf(format!(":{}", s))), // TODO performance
    Calcit::List(xs) => {
      let mut ys: Vec<Cirru> = Vec::with_capacity(xs.len());
      for x in xs {
        ys.push(calcit_to_cirru(x)?);
      }
      Ok(Cirru::List(ys))
    }
    Calcit::Proc(s) => Ok(Cirru::Leaf(s.to_owned())),
    Calcit::Syntax(s, _ns) => Ok(Cirru::Leaf(s.to_string().into_boxed_str())),
    _ => Err(format!("unknown data to convert to Cirru: {}", x)),
  }
}
