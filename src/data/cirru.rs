use std::sync::Arc;

use cirru_parser::Cirru;
use im_ternary_tree::TernaryTreeList;

use crate::primes::Calcit;

/// code is CirruNode, and this function parse code(rather than data)
pub fn code_to_calcit(xs: &Cirru, ns: Arc<str>, def: Arc<str>, coord: &[u8]) -> Result<Calcit, String> {
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
      "::" => Ok(Calcit::Symbol {
        sym: (**s).into(),
        ns,
        at_def: def,
        resolved: None,
        location: Some(coord.to_vec()),
      }),
      _ => match s.chars().next().expect("load first char") {
        ':' => Ok(Calcit::kwd(&s[1..])),
        '.' => {
          if s.starts_with(".-") || s.starts_with(".!") {
            // try not to break js interop
            Ok(Calcit::Symbol {
              sym: (**s).into(),
              ns,
              at_def: def,
              resolved: None,
              location: Some(coord.to_vec()),
            })
          } else {
            Ok(Calcit::Proc((**s).into())) // as native method syntax
          }
        }
        '"' | '|' => Ok(Calcit::new_str(&s[1..])),
        '0' if s.starts_with("0x") => match u32::from_str_radix(&s[2..], 16) {
          Ok(n) => Ok(Calcit::Number(n as f64)),
          Err(e) => Err(format!("failed to parse hex: {} => {:?}", s, e)),
        },
        '\'' if s.len() > 1 => Ok(Calcit::List(TernaryTreeList::from(&[
          Calcit::Symbol {
            sym: String::from("quote").into(),
            ns: ns.to_owned(),
            at_def: def.to_owned(),
            resolved: None,
            location: Some(coord.to_vec()),
          },
          Calcit::Symbol {
            sym: String::from(&s[1..]).into(),
            ns,
            at_def: def,
            resolved: None,
            location: Some(coord.to_vec()),
          },
        ]))),
        // TODO also detect simple variables
        '~' if s.starts_with("~@") && s.chars().count() > 2 => Ok(Calcit::List(TernaryTreeList::from(&[
          Calcit::Symbol {
            sym: String::from("~@").into(),
            ns: ns.to_owned(),
            at_def: def.to_owned(),
            resolved: None,
            location: Some(coord.to_vec()),
          },
          Calcit::Symbol {
            sym: String::from(&s[2..]).into(),
            ns,
            at_def: def,
            resolved: None,
            location: Some(coord.to_vec()),
          },
        ]))),
        '~' if s.chars().count() > 1 && !s.starts_with("~@") => Ok(Calcit::List(TernaryTreeList::from(&[
          Calcit::Symbol {
            sym: String::from("~").into(),
            ns: ns.to_owned(),
            at_def: def.to_owned(),
            resolved: None,
            location: Some(coord.to_vec()),
          },
          Calcit::Symbol {
            sym: String::from(&s[1..]).into(),
            ns,
            at_def: def,
            resolved: None,
            location: Some(coord.to_vec()),
          },
        ]))),
        '@' => Ok(Calcit::List(TernaryTreeList::from(&[
          Calcit::Symbol {
            sym: String::from("deref").into(),
            ns: ns.to_owned(),
            at_def: def.to_owned(),
            resolved: None,
            location: Some(coord.to_vec()),
          },
          Calcit::Symbol {
            sym: String::from(&s[1..]).into(),
            ns,
            at_def: def,
            resolved: None,
            location: Some(coord.to_vec()),
          },
        ]))),
        // TODO future work of reader literal expanding
        _ => {
          if let Ok(f) = s.parse::<f64>() {
            Ok(Calcit::Number(f))
          } else {
            Ok(Calcit::Symbol {
              sym: (**s).into(),
              ns,
              at_def: def,
              resolved: None,
              location: Some(coord.to_vec()),
            })
          }
        }
      },
    },
    Cirru::List(ys) => {
      let mut zs: Vec<Calcit> = Vec::with_capacity(ys.len());
      for (idx, y) in ys.iter().enumerate() {
        let mut next_coord = coord.to_owned();
        next_coord.push(idx as u8); // code not supposed to be fatter than 256 children
        match code_to_calcit(y, ns.to_owned(), def.to_owned(), &next_coord) {
          Ok(v) => {
            if !is_comment(&v) {
              zs.push(v.to_owned());
            }
          }
          Err(e) => return Err(e),
        }
      }
      Ok(Calcit::List(TernaryTreeList::from(&zs)))
    }
  }
}

/// transform Cirru to Calcit data directly
pub fn cirru_to_calcit(xs: &Cirru) -> Calcit {
  match xs {
    Cirru::Leaf(s) => Calcit::Str((**s).into()),
    Cirru::List(ys) => {
      let mut zs: Vec<Calcit> = Vec::with_capacity(ys.len());
      for y in ys {
        zs.push(cirru_to_calcit(y));
      }
      Calcit::List(TernaryTreeList::from(&zs))
    }
  }
}

/// for generate Cirru via calcit data manually
pub fn calcit_data_to_cirru(xs: &Calcit) -> Result<Cirru, String> {
  match xs {
    Calcit::Nil => Ok(Cirru::leaf("nil")),
    Calcit::Bool(b) => Ok(Cirru::Leaf(b.to_string().into())),
    Calcit::Number(n) => Ok(Cirru::Leaf(n.to_string().into())),
    Calcit::Str(s) => Ok(Cirru::Leaf((**s).into())),
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
      Some(Calcit::Symbol { sym, .. }) => &**sym == ";",
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
    Calcit::Number(n) => Ok(Cirru::Leaf(n.to_string().into())),
    Calcit::Str(s) => Ok(Cirru::leaf(format!("|{}", s))), // TODO performance
    Calcit::Symbol { sym, .. } => Ok(Cirru::Leaf((**sym).into())), // TODO performance
    Calcit::Keyword(s) => Ok(Cirru::leaf(format!(":{}", s))), // TODO performance
    Calcit::List(xs) => {
      let mut ys: Vec<Cirru> = Vec::with_capacity(xs.len());
      for x in xs {
        ys.push(calcit_to_cirru(x)?);
      }
      Ok(Cirru::List(ys))
    }
    Calcit::Proc(s) => Ok(Cirru::Leaf((**s).into())),
    Calcit::Syntax(s, _ns) => Ok(Cirru::Leaf(s.to_string().into())),
    _ => Err(format!("unknown data to convert to Cirru: {}", x)),
  }
}
