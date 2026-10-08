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
      "&unit" => Ok(Calcit::Unit),
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
      "defwasm-export" => Ok(Calcit::Syntax(CalcitSyntax::DefWasmExport, ns.into())),
      "defwasm-import" => Ok(Calcit::Syntax(CalcitSyntax::DefWasmImport, ns.into())),
      "unsafe-coerce" => Ok(Calcit::Syntax(CalcitSyntax::UnsafeCoerce, ns.into())),
      "js-cast" => Ok(Calcit::Syntax(CalcitSyntax::JsCast, ns.into())),
      "parse-cirru-edn-as" => Ok(Calcit::Syntax(CalcitSyntax::ParseCirruEdnAs, ns.into())),
      "try-parse-cirru-edn-as" => Ok(Calcit::Syntax(CalcitSyntax::TryParseCirruEdnAs, ns.into())),
      "decode-map-as" => Ok(Calcit::Syntax(CalcitSyntax::DecodeMapAs, ns.into())),
      "try-decode-map-as" => Ok(Calcit::Syntax(CalcitSyntax::TryDecodeMapAs, ns.into())),
      "assert-traits" => Ok(Calcit::Syntax(CalcitSyntax::AssertTraits, ns.into())),
      "" => Err(String::from("Empty string is invalid")),
      // anonymous enum constructor syntax
      "::" => Ok(Calcit::Proc(CalcitProc::NativeEnum)),
      // loose struct syntax (struct without a declared name)
      "?{}" => Ok(Calcit::Proc(CalcitProc::NativeLooseStruct)),
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
            Ok(Calcit::Method(
              s[1..].to_owned().into(),
              MethodKind::Invoke(crate::calcit::DYNAMIC_TYPE.clone()),
            ))
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

/// Map each child produced by [`code_to_calcit`] for a source list back to the
/// index of the source child it came from. Comments are dropped by the reader,
/// and a head leaf such as `a.b` expands into two reader children, so reader
/// indexes and Snapshot source indexes differ.
pub fn reader_child_source_indexes(ys: &[Cirru]) -> Vec<usize> {
  let mut indexes = Vec::with_capacity(ys.len());
  for (idx, y) in ys.iter().enumerate() {
    if let Cirru::List(zs) = y
      && zs.len() > 1
      && zs[0] == Cirru::leaf(";")
    {
      continue;
    }
    if idx == 0
      && let Cirru::Leaf(s) = y
      && split_leaf_to_method_call(s).is_some()
    {
      indexes.push(idx);
    }
    indexes.push(idx);
  }
  indexes
}

/// Convert a path into the reader form of `code` into the Snapshot source path.
/// A path that descends into structure synthesized from one source leaf stops
/// at that leaf.
pub fn reader_path_to_source_path(code: &Cirru, path: &[usize]) -> Option<Vec<usize>> {
  let mut node = code;
  let mut source_path = Vec::with_capacity(path.len());
  for reader_index in path {
    let Cirru::List(ys) = node else {
      return Some(source_path);
    };
    let source_index = *reader_child_source_indexes(ys).get(*reader_index)?;
    source_path.push(source_index);
    node = &ys[source_index];
  }
  Some(source_path)
}

/// Convert a Snapshot source path into the path of the first reader node that
/// originates from it. Comments have no reader node and return `None`.
pub fn source_path_to_reader_path(code: &Cirru, path: &[usize]) -> Option<Vec<usize>> {
  let mut node = code;
  let mut reader_path = Vec::with_capacity(path.len());
  for source_index in path {
    let Cirru::List(ys) = node else { return None };
    let reader_index = reader_child_source_indexes(ys).iter().position(|index| index == source_index)?;
    reader_path.push(reader_index);
    node = &ys[*source_index];
  }
  Some(reader_path)
}

/// split `a.b` into `.b` and `a`, `a.-b` into `.-b` and `a`, `a.!b` into `.!b` and `a`, etc.
/// some characters available for variables are okey here, for example `-`, `!`, `?`, `*``, etc.
fn split_leaf_to_method_call(s: &str) -> Option<(String, Calcit)> {
  let prefixes = [
    (".:", MethodKind::TagAccess),
    (".-", MethodKind::Access),
    (".!", MethodKind::InvokeNative),
    (".", MethodKind::Invoke(crate::calcit::DYNAMIC_TYPE.clone())),
  ];

  for (prefix, kind) in prefixes.iter() {
    if let Some((obj, method)) = s.split_once(prefix)
      && is_valid_symbol(obj)
      && is_valid_symbol(method)
    {
      let method_kind = if matches!(kind, MethodKind::Invoke(_)) {
        MethodKind::Invoke(crate::calcit::DYNAMIC_TYPE.clone())
      } else {
        kind.to_owned()
      };
      return Some((obj.to_owned(), Calcit::Method(method.into(), method_kind)));
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
    Calcit::Unit => Ok(Cirru::leaf("&unit")),
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
    Unit => Ok(Cirru::leaf("&unit")),
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
        ExternalAccess(_) => Ok(Cirru::leaf(format!(".:{name}"))),
        ExternalGet(_) => Ok(Cirru::leaf(format!("js-get:{name}"))),
        ExternalSet(_) => Ok(Cirru::leaf(format!("js-set:{name}"))),
        ExternalInvoke(_) => Ok(Cirru::leaf(format!(".{name}"))),
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

  #[test]
  fn parses_explicit_unit_literal() {
    let parsed = code_to_calcit(&Cirru::leaf("&unit"), "tests.ns", "demo", vec![]).expect("parse unit");
    assert!(matches!(parsed, Calcit::Unit));
    assert_eq!(calcit_to_cirru(&parsed).expect("render unit"), Cirru::leaf("&unit"));
  }

  #[test]
  fn parses_hex_number_literal() {
    let upper = code_to_calcit(&Cirru::leaf("0x10"), "tests.ns", "demo", vec![]).expect("parse upper hex literal");
    assert!(matches!(upper, Calcit::Number(n) if n == 16.0));
    let lower = code_to_calcit(&Cirru::leaf("0xf"), "tests.ns", "demo", vec![]).expect("parse lower hex literal");
    assert!(matches!(lower, Calcit::Number(n) if n == 15.0));
  }

  #[test]
  fn parses_strict_edn_decode_as_syntax() {
    let expr = Cirru::List(vec![Cirru::leaf("parse-cirru-edn-as"), Cirru::leaf("|do 1"), Cirru::leaf("Number")]);

    let calcit = code_to_calcit(&expr, "tests.ns", "demo", vec![]).expect("parse strict EDN decoder");
    let Calcit::List(items) = calcit else {
      panic!("expected list");
    };
    assert!(matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::ParseCirruEdnAs, _))));
  }

  #[test]
  fn parses_safe_strict_edn_decode_as_syntax() {
    let expr = Cirru::List(vec![
      Cirru::leaf("try-parse-cirru-edn-as"),
      Cirru::leaf("|do 1"),
      Cirru::leaf("Number"),
    ]);

    let calcit = code_to_calcit(&expr, "tests.ns", "demo", vec![]).expect("parse safe strict EDN decoder");
    let Calcit::List(items) = calcit else {
      panic!("expected list");
    };
    assert!(matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::TryParseCirruEdnAs, _))));
  }

  #[test]
  fn parses_runtime_map_decode_as_syntax() {
    let expr = Cirru::List(vec![Cirru::leaf("decode-map-as"), Cirru::leaf("value"), Cirru::leaf("Response")]);

    let calcit = code_to_calcit(&expr, "tests.ns", "demo", vec![]).expect("parse runtime map decoder");
    let Calcit::List(items) = calcit else {
      panic!("expected list");
    };
    assert!(matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::DecodeMapAs, _))));
  }

  #[test]
  fn parses_safe_runtime_map_decode_as_syntax() {
    let expr = Cirru::List(vec![
      Cirru::leaf("try-decode-map-as"),
      Cirru::leaf("value"),
      Cirru::leaf("Response"),
    ]);

    let calcit = code_to_calcit(&expr, "tests.ns", "demo", vec![]).expect("parse safe runtime map decoder");
    let Calcit::List(items) = calcit else {
      panic!("expected list");
    };
    assert!(matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::TryDecodeMapAs, _))));
  }

  #[test]
  fn parses_assert_traits_list() {
    let expr = Cirru::List(vec![Cirru::leaf("assert-traits"), Cirru::leaf("x"), Cirru::leaf("Show")]);

    let calcit = code_to_calcit(&expr, "tests.ns", "demo", vec![]).expect("parse assert-traits");
    let list_arc = match calcit {
      Calcit::List(xs) => xs,
      other => panic!("expected list, got {other}"),
    };
    assert_eq!(list_arc.len(), 3);
    let items = list_arc.to_vec();
    assert!(matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::AssertTraits, _))));
    assert!(matches!(items.get(1), Some(Calcit::Symbol { .. })));
    assert!(matches!(items.get(2), Some(Calcit::Symbol { .. })));
  }

  #[test]
  fn reader_and_source_paths_skip_comments_and_head_method_sugar() {
    let code = cirru_parser::parse("defn demo () (; note) (let ((x 1)) (; nested) (x.inc) (f x))")
      .expect("parse")
      .remove(0);
    let reader = code_to_calcit(&code, "app.main", "demo", vec![]).expect("convert");
    let Calcit::List(items) = &reader else { panic!("expected list") };
    assert_eq!(items.len(), 4, "the comment has no reader node");

    assert_eq!(reader_path_to_source_path(&code, &[3]), Some(vec![4]));
    assert_eq!(reader_path_to_source_path(&code, &[3, 3]), Some(vec![4, 4]));
    assert_eq!(reader_path_to_source_path(&code, &[3, 3, 1]), Some(vec![4, 4, 1]));
    // `x.inc` at the head becomes two reader children that both point at the leaf.
    assert_eq!(reader_path_to_source_path(&code, &[3, 2, 0]), Some(vec![4, 3, 0]));
    assert_eq!(reader_path_to_source_path(&code, &[3, 2, 1]), Some(vec![4, 3, 0]));

    assert_eq!(source_path_to_reader_path(&code, &[4]), Some(vec![3]));
    assert_eq!(source_path_to_reader_path(&code, &[4, 4]), Some(vec![3, 3]));
    assert_eq!(source_path_to_reader_path(&code, &[3]), None, "comments have no reader node");
  }
}
