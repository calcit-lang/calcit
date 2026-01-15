use std::{sync::Arc, vec};

use cirru_parser::Cirru;

use crate::calcit::{Calcit, CalcitImport, CalcitList, CalcitLocal, CalcitProc, CalcitSyntax, MethodKind};

/// code is CirruNode, and this function parse code(rather than data)
pub fn code_to_calcit(xs: &Cirru, ns: &str, def: &str, coord: Vec<u16>) -> Result<Calcit, String> {
  let symbol_info = Arc::new(crate::calcit::CalcitSymbolInfo {
    at_ns: Arc::from(ns),
    at_def: Arc::from(def),
  });
  let coord = Arc::from(coord);
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
      "&" => Ok(Calcit::Syntax(CalcitSyntax::ArgSpread, ns.into())),
      "?" => Ok(Calcit::Syntax(CalcitSyntax::ArgOptional, ns.into())),
      "~" => Ok(Calcit::Syntax(CalcitSyntax::MacroInterpolate, ns.into())),
      "~@" => Ok(Calcit::Syntax(CalcitSyntax::MacroInterpolateSpread, ns.into())),
      "assert-type" => Ok(Calcit::Syntax(CalcitSyntax::AssertType, ns.into())),
      "" => Err(String::from("Empty string is invalid")),
      // special tuple syntax
      "::" => Ok(Calcit::Proc(CalcitProc::NativeTuple)),
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
            Ok(Calcit::Method(s[1..].to_owned().into(), MethodKind::Invoke(None)))
          }
        }
        '"' | '|' => Ok(Calcit::new_str(&s[1..])),
        '0' if s.starts_with("0x") => match u32::from_str_radix(&s[2..], 16) {
          Ok(n) => Ok(Calcit::Number(n as f64)),
          Err(e) => Err(format!("failed to parse hex: {s} => {e:?}")),
        },
        '\'' if s.len() > 1 => Ok(Calcit::from(CalcitList::from(&[
          Calcit::Syntax(CalcitSyntax::Quote, ns.into()),
          Calcit::Symbol {
            sym: Arc::from(&s[1..]),
            info: Arc::clone(&symbol_info),
            location: Some(Arc::clone(&coord)),
          },
        ]))),
        '~' if s.starts_with("~@") && s.chars().count() > 2 => Ok(Calcit::from(CalcitList::from(&[
          Calcit::Syntax(CalcitSyntax::MacroInterpolateSpread, ns.into()),
          Calcit::Symbol {
            sym: Arc::from(&s[2..]),
            info: Arc::clone(&symbol_info),
            location: Some(Arc::clone(&coord)),
          },
        ]))),
        '~' if s.chars().count() > 1 && !s.starts_with("~@") => Ok(Calcit::from(CalcitList::from(&[
          Calcit::Syntax(CalcitSyntax::MacroInterpolate, ns.into()),
          Calcit::Symbol {
            sym: Arc::from(&s[1..]),
            info: Arc::clone(&symbol_info),
            location: Some(Arc::clone(&coord)),
          },
        ]))),
        '@' => Ok(Calcit::from(CalcitList::from(&[
          // `deref` expands to `.deref` or `&atom:deref`
          Calcit::Symbol {
            sym: Arc::from("deref"),
            info: Arc::clone(&symbol_info),
            location: Some(Arc::clone(&coord)),
          },
          Calcit::Symbol {
            sym: Arc::from(&s[1..]),
            info: Arc::clone(&symbol_info),
            location: Some(Arc::clone(&coord)),
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
              info: Arc::clone(&symbol_info),
              location: Some(Arc::clone(&coord)),
            })
          }
        }
      },
    },
    Cirru::List(ys) => {
      let mut zs: Vec<Calcit> = vec![];
      for (idx, y) in ys.iter().enumerate() {
        if idx > 65535 {
          return Err(format!("Cirru code too large, index: {idx}"));
        }

        if let Cirru::List(ys) = y
          && ys.len() > 1
        {
          if ys[0] == Cirru::leaf(";") {
            continue;
          }
          if ys[0] == Cirru::leaf("cirru-quote") {
            // special rule for Cirru code
            if ys.len() == 2 {
              zs.push(Calcit::CirruQuote(ys[1].to_owned()));
              continue;
            }
            return Err(format!("expected 1 argument, got: {ys:?}"));
          }
        }
        let mut next_coord: Vec<u16> = (*coord).to_owned();
        next_coord.push(idx as u16); // clamp to prevent overflow, code not supposed to be larger than 65536 children

        if let Cirru::Leaf(s) = y {
          // dirty hack to support shorthand of method calling,
          // this feature is EXPERIMENTAL and might change in future
          if let Some((obj, method)) = split_leaf_to_method_call(s) {
            if idx == 0 {
              zs.push(method);
              zs.push(Calcit::Symbol {
                sym: Arc::from(obj),
                info: Arc::clone(&symbol_info),
                location: Some(next_coord.to_owned().into()),
              });
              continue;
            } else {
              // turn a.-b into (.-b a) , a shorthand
              zs.push(Calcit::from(CalcitList::from(&[
                method,
                Calcit::Symbol {
                  sym: Arc::from(obj),
                  info: Arc::clone(&symbol_info),
                  location: Some(next_coord.to_owned().into()),
                },
              ])));
              continue;
            }
          }
        }

        zs.push(code_to_calcit(y, ns, def, next_coord)?);
      }
      Ok(Calcit::from(CalcitList::Vector(zs)))
    }
  }
}

/// split `a.b` into `.b` and `a`, `a.-b` into `.-b` and `a`, `a.!b` into `.!b` and `a`, etc.
/// some characters available for variables are okey here, for example `-`, `!`, `?`, `*``, etc.
fn split_leaf_to_method_call(s: &str) -> Option<(String, Calcit)> {
  let prefixes = [
    (".:", MethodKind::TagAccess),
    (".-", MethodKind::Access),
    (".!", MethodKind::InvokeNative),
    (".", MethodKind::Invoke(None)),
  ];

  for (prefix, kind) in prefixes.iter() {
    if let Some((obj, method)) = s.split_once(prefix) {
      if is_valid_symbol(obj) && is_valid_symbol(method) {
        let method_kind = if matches!(kind, MethodKind::Invoke(_)) {
          MethodKind::Invoke(None)
        } else {
          kind.to_owned()
        };
        return Some((obj.to_owned(), Calcit::Method(method.into(), method_kind)));
      }
    }
  }

  None
}

fn is_valid_symbol(s: &str) -> bool {
  // empty space is not valid symbol
  if s.is_empty() {
    return false;
  }
  // symbol should not start with a digit
  if s.chars().next().unwrap().is_ascii_digit() {
    return false;
  }
  // every character should be valid, a-z, A-Z, 0-9, -, _, ?, !, *, etc.
  for c in s.chars() {
    if !(c.is_alphanumeric() || matches!(c, '-' | '_' | '?' | '!' | '*')) {
      return false;
    }
  }
  true
}

/// transform Cirru to Calcit data directly
pub fn cirru_to_calcit(xs: &Cirru) -> Calcit {
  match xs {
    Cirru::Leaf(s) => Calcit::Str((**s).into()),
    Cirru::List(ys) => {
      let mut zs: Vec<Calcit> = vec![];
      for y in ys {
        zs.push(cirru_to_calcit(y));
      }
      Calcit::from(CalcitList::Vector(zs))
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
      ys.traverse_result(&mut |y| match calcit_data_to_cirru(y) {
        Ok(v) => {
          zs.push(v);
          Ok(())
        }
        Err(e) => Err(e),
      })?;
      Ok(Cirru::List(zs))
    }
    a => Err(format!("unknown data for cirru: {a}")),
  }
}

/// converting data for display in Cirru syntax
pub fn calcit_to_cirru(x: &Calcit) -> Result<Cirru, String> {
  use Calcit::*;
  match x {
    Nil => Ok(Cirru::leaf("nil")),
    Bool(true) => Ok(Cirru::leaf("true")),
    Bool(false) => Ok(Cirru::leaf("false")),
    Number(n) => Ok(Cirru::Leaf(n.to_string().into())),
    Str(s) => Ok(Cirru::leaf(format!("|{s}"))),
    Symbol { sym, .. } => Ok(Cirru::Leaf(sym.to_owned())),
    Local(CalcitLocal { sym, .. }) => Ok(Cirru::Leaf(sym.to_owned())),
    Import(CalcitImport { ns, def, .. }) => Ok(Cirru::Leaf((format!("{ns}/{def}")).into())),
    Registered(s) => Ok(Cirru::Leaf(s.as_ref().into())),
    Tag(s) => Ok(Cirru::leaf(format!(":{s}"))),
    List(xs) => {
      let mut ys: Vec<Cirru> = Vec::with_capacity(xs.len());
      xs.traverse_result::<String>(&mut |x| {
        ys.push(calcit_to_cirru(x)?);
        Ok(())
      })?;
      Ok(Cirru::List(ys))
    }
    Proc(s) => Ok(Cirru::Leaf(s.as_ref().into())),
    Fn { .. } => Ok(Cirru::Leaf(format!("(fn {x})").into())), // TODO more details
    Syntax(s, _ns) => Ok(Cirru::Leaf(s.as_ref().into())),
    CirruQuote(code) => Ok(code.to_owned()),
    Method(name, kind) => {
      use MethodKind::*;
      match kind {
        Access => Ok(Cirru::leaf(format!(".-{name}"))),
        InvokeNative => Ok(Cirru::leaf(format!(".!{name}"))),
        Invoke(_) => Ok(Cirru::leaf(format!(".{name}"))),
        TagAccess => Ok(Cirru::leaf(format!(".:{name}"))),
        AccessOptional => Ok(Cirru::leaf(format!(".?-{name}"))),
        InvokeNativeOptional => Ok(Cirru::leaf(format!(".?!{name}"))),
      }
    }
    _ => Err(format!("unknown data to convert to Cirru: {x}")),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_assert_type_list() {
    let expr = Cirru::List(vec![Cirru::leaf("assert-type"), Cirru::leaf("x"), Cirru::leaf(":fn")]);

    let calcit = code_to_calcit(&expr, "tests.ns", "demo", vec![]).expect("parse assert-type");
    let list_arc = match calcit {
      Calcit::List(xs) => xs,
      other => panic!("expected list, got {other}"),
    };
    assert_eq!(list_arc.len(), 3);
    let items = list_arc.to_vec();
    assert!(matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::AssertType, _))));
    assert!(matches!(items.get(1), Some(Calcit::Symbol { .. })));
    assert!(matches!(items.get(2), Some(Calcit::Tag(_))));
  }
}
