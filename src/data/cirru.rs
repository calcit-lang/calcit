use std::sync::Arc;

use cirru_parser::Cirru;
use im_ternary_tree::TernaryTreeList;

use crate::primes::{Calcit, CalcitProc, MethodKind};

/// code is CirruNode, and this function parse code(rather than data)
pub fn code_to_calcit(xs: &Cirru, ns: Arc<str>, def: Arc<str>, coord: Vec<u8>) -> Result<Calcit, String> {
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
        sym: s.clone(),
        info: Arc::new(crate::primes::CalcitSymbolInfo {
          ns,
          at_def: def,
          resolved: None,
        }),
        location: Some(coord),
      }),
      _ => match s.chars().next().expect("load first char") {
        ':' if s.len() > 1 && s.chars().nth(1) != Some(':') => Ok(Calcit::tag(&s[1..])),
        '.' => {
          if let Some(stripped) = s.strip_prefix(".-") {
            Ok(Calcit::Method(stripped.into(), MethodKind::Access))
          } else if let Some(stripped) = s.strip_prefix(".!") {
            Ok(Calcit::Method(stripped.into(), MethodKind::InvokeNative))
          } else if let Some(stripped) = s.strip_prefix(".?-") {
            Ok(Calcit::Method(stripped.into(), MethodKind::AccessOptional))
          } else if let Some(stripped) = s.strip_prefix(".?!") {
            Ok(Calcit::Method(stripped.into(), MethodKind::InvokeNativeOptional))
          } else {
            Ok(Calcit::Method(s[1..].to_owned().into(), MethodKind::Invoke))
          }
        }
        '"' | '|' => Ok(Calcit::new_str(&s[1..])),
        '0' if s.starts_with("0x") => match u32::from_str_radix(&s[2..], 16) {
          Ok(n) => Ok(Calcit::Number(n as f64)),
          Err(e) => Err(format!("failed to parse hex: {s} => {e:?}")),
        },
        '\'' if s.len() > 1 => Ok(Calcit::List(TernaryTreeList::from(&[
          Calcit::Symbol {
            sym: Arc::from("quote"),
            info: Arc::new(crate::primes::CalcitSymbolInfo {
              ns: ns.to_owned(),
              at_def: def.clone(),
              resolved: None,
            }),
            location: Some(coord.clone()),
          },
          Calcit::Symbol {
            sym: Arc::from(&s[1..]),
            info: Arc::new(crate::primes::CalcitSymbolInfo {
              ns,
              at_def: def,
              resolved: None,
            }),
            location: Some(coord.clone()),
          },
        ]))),
        // TODO also detect simple variables
        '~' if s.starts_with("~@") && s.chars().count() > 2 => Ok(Calcit::List(TernaryTreeList::from(&[
          Calcit::Symbol {
            sym: Arc::from("~@"),
            info: Arc::new(crate::primes::CalcitSymbolInfo {
              ns: ns.to_owned(),
              at_def: def.to_owned(),
              resolved: None,
            }),
            location: Some(coord.clone()),
          },
          Calcit::Symbol {
            sym: Arc::from(&s[2..]),
            info: Arc::new(crate::primes::CalcitSymbolInfo {
              ns,
              at_def: def,
              resolved: None,
            }),
            location: Some(coord.clone()),
          },
        ]))),
        '~' if s.chars().count() > 1 && !s.starts_with("~@") => Ok(Calcit::List(TernaryTreeList::from(&[
          Calcit::Symbol {
            sym: Arc::from("~"),
            info: Arc::new(crate::primes::CalcitSymbolInfo {
              ns: ns.to_owned(),
              at_def: def.to_owned(),
              resolved: None,
            }),
            location: Some(coord.clone()),
          },
          Calcit::Symbol {
            sym: Arc::from(&s[1..]),
            info: Arc::new(crate::primes::CalcitSymbolInfo {
              ns,
              at_def: def,
              resolved: None,
            }),
            location: Some(coord.clone()),
          },
        ]))),
        '@' => Ok(Calcit::List(TernaryTreeList::from(&[
          Calcit::Symbol {
            sym: Arc::from("deref"),
            info: Arc::new(crate::primes::CalcitSymbolInfo {
              ns: ns.to_owned(),
              at_def: def.to_owned(),
              resolved: None,
            }),
            location: Some(coord.clone()),
          },
          Calcit::Symbol {
            sym: Arc::from(&s[1..]),
            info: Arc::new(crate::primes::CalcitSymbolInfo {
              ns,
              at_def: def,
              resolved: None,
            }),
            location: Some(coord.clone()),
          },
        ]))),
        // TODO future work of reader literal expanding
        _ => {
          if let Ok(p) = s.parse::<CalcitProc>() {
            Ok(Calcit::Proc(p))
          } else if let Ok(f) = s.parse::<f64>() {
            Ok(Calcit::Number(f))
          } else {
            Ok(Calcit::Symbol {
              sym: (**s).into(),
              info: Arc::new(crate::primes::CalcitSymbolInfo {
                ns,
                at_def: def,
                resolved: None,
              }),
              location: Some(coord.clone()),
            })
          }
        }
      },
    },
    Cirru::List(ys) => {
      let mut zs: Vec<Calcit> = Vec::with_capacity(ys.len());
      for (idx, y) in ys.iter().enumerate() {
        let mut next_coord: Vec<u8> = coord.to_owned();
        next_coord.push(idx as u8); // code not supposed to be fatter than 256 children

        if let Cirru::List(ys) = y {
          if ys.len() > 1 {
            if ys[0] == Cirru::leaf(";") {
              continue;
            }
            if ys[0] == Cirru::leaf("cirru-quote") {
              // special rule for Cirru code
              if ys.len() == 2 {
                zs.push(Calcit::CirruQuote(ys[1].clone()));
              } else {
                return Err(format!("expected 1 argument, got: {ys:?}"));
              }
              continue;
            }
          }
        }

        zs.push(code_to_calcit(y, ns.to_owned(), def.to_owned(), next_coord)?)
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
    Calcit::CirruQuote(code) => Ok(code.to_owned()),
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
    a => Err(format!("unknown data for cirru: {a}")),
  }
}

/// converting data for display in Cirru syntax
pub fn calcit_to_cirru(x: &Calcit) -> Result<Cirru, String> {
  match x {
    Calcit::Nil => Ok(Cirru::leaf("nil")),
    Calcit::Bool(true) => Ok(Cirru::leaf("true")),
    Calcit::Bool(false) => Ok(Cirru::leaf("false")),
    Calcit::Number(n) => Ok(Cirru::Leaf(n.to_string().into())),
    Calcit::Str(s) => Ok(Cirru::leaf(format!("|{s}"))),            // TODO performance
    Calcit::Symbol { sym, .. } => Ok(Cirru::Leaf((**sym).into())), // TODO performance
    Calcit::Tag(s) => Ok(Cirru::leaf(format!(":{s}"))),            // TODO performance
    Calcit::List(xs) => {
      let mut ys: Vec<Cirru> = Vec::with_capacity(xs.len());
      for x in xs {
        ys.push(calcit_to_cirru(x)?);
      }
      Ok(Cirru::List(ys))
    }
    Calcit::Proc(s) => Ok(Cirru::Leaf(s.as_ref().into())),
    Calcit::Syntax(s, _ns) => Ok(Cirru::Leaf(s.as_ref().into())),
    Calcit::CirruQuote(code) => Ok(code.to_owned()),
    Calcit::Method(name, kind) => match kind {
      MethodKind::Access => Ok(Cirru::leaf(format!(".-{name}"))),
      MethodKind::InvokeNative => Ok(Cirru::leaf(format!(".!{name}"))),
      MethodKind::Invoke => Ok(Cirru::leaf(format!(".{name}"))),
      MethodKind::AccessOptional => Ok(Cirru::leaf(format!(".?-{name}"))),
      MethodKind::InvokeNativeOptional => Ok(Cirru::leaf(format!(".?!{name}"))),
    },
    _ => Err(format!("unknown data to convert to Cirru: {x}")),
  }
}
