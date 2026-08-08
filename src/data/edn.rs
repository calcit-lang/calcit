use std::collections::HashMap;
use std::sync::Arc;

use bisection_key::LexiconKey;
use cirru_parser::Cirru;

use crate::calcit::{self, CalcitEnumDef, CalcitEnumValue, CalcitImpl, CalcitImport, CalcitList, CalcitLocal, CalcitStructDef};
use crate::calcit::{Calcit, CalcitStructValue};
use crate::{calcit::MethodKind, data::cirru};

use cirru_edn::{Edn, EdnEnumView, EdnListView, EdnMapView, EdnSetView, EdnStructView, EdnTag};

// values does not fit are just represented with specical indicates
pub fn calcit_to_edn(x: &Calcit) -> Result<Edn, String> {
  use Calcit::*;
  match x {
    Nil => Ok(Edn::Nil),
    Bool(b) => Ok(Edn::Bool(*b)),
    Str(s) => Ok(Edn::Str((**s).into())),
    Number(n) => Ok(Edn::Number(*n)),
    Tag(s) => Ok(Edn::Tag(s.to_owned())),
    Symbol { sym, .. } => Ok(Edn::Symbol((**sym).into())),
    Local(CalcitLocal { sym, .. }) => Ok(Edn::Symbol((**sym).into())),
    Import(CalcitImport { def, .. }) => Ok(Edn::Symbol((**def).into())),
    Registered(def) => Ok(Edn::Symbol((**def).into())),
    List(xs) => {
      let mut ys = EdnListView::default();
      xs.traverse_result::<String>(&mut |x| {
        ys.push(calcit_to_edn(x)?);
        Ok(())
      })?;
      Ok(ys.into())
    }
    Set(xs) => {
      let mut ys = EdnSetView::default();
      for x in xs {
        ys.insert(calcit_to_edn(x)?);
      }
      Ok(ys.into())
    }
    Map(xs) => {
      let mut ys = EdnMapView::default();
      for (k, x) in xs {
        ys.insert(calcit_to_edn(k)?, calcit_to_edn(x)?);
      }
      Ok(ys.into())
    }
    Struct(CalcitStructValue { struct_ref, values, .. }) => {
      let mut entries = EdnStructView::new(struct_ref.name.arc_str());
      for idx in 0..struct_ref.fields.len() {
        entries.insert(struct_ref.fields[idx].to_owned(), calcit_to_edn(&values[idx])?);
      }
      Ok(entries.into())
    }
    Impl(..) => Ok(Edn::str(x.to_string())),
    Fn { info, .. } => {
      let def_ns = &info.def_ns;
      let name = &info.name;
      let args = &info.args;
      eprintln!("[Warn] fn to EDN: {def_ns}/{name} {args:?}");
      Ok(Edn::str(x.to_string()))
    }
    Proc(name) => Ok(Edn::Symbol(name.as_ref().into())),
    Syntax(name, _ns) => Ok(Edn::sym(name.as_ref())),
    Enum(CalcitEnumValue { tag, extra, sum_type, .. }) => {
      let type_name = sum_type.as_ref().map(|enum_def| enum_def.name().arc_str());
      match &**tag {
        Symbol { sym, .. } => {
          if &**sym == "quote" {
            let data = extra.first().ok_or(format!("quote expected 1 argument, got: {extra:?}"))?; // TODO more types to handle
            match cirru::calcit_data_to_cirru(data) {
              Ok(v) => Ok(Edn::Quote(v)),
              Err(e) => Err(format!("failed to create quote: {e}")), // TODO more types to handle
            }
          } else {
            Err(format!("unknown tag for EDN: {sym}")) // TODO more types to handle
          }
        }
        Struct(CalcitStructValue { struct_ref, .. }) => {
          let mut extra_values = vec![];
          for item in extra {
            extra_values.push(calcit_to_edn(item)?);
          }
          let variant = struct_ref.name.arc_str();
          Ok(match type_name.clone() {
            Some(type_name) => Edn::typed_enum(type_name, variant, extra_values),
            None => Edn::enum_value(variant, extra_values),
          })
        }
        Impl(CalcitImpl { name, .. }) => {
          let mut extra_values = vec![];
          for item in extra {
            extra_values.push(calcit_to_edn(item)?);
          }
          let variant = name.arc_str();
          Ok(match type_name.clone() {
            Some(type_name) => Edn::typed_enum(type_name, variant, extra_values),
            None => Edn::enum_value(variant, extra_values),
          })
        }
        Tag(tag) => {
          let mut extra_values = vec![];
          for item in extra {
            extra_values.push(calcit_to_edn(item)?);
          }
          let variant = tag.arc_str();
          Ok(match type_name.clone() {
            Some(type_name) => Edn::typed_enum(type_name, variant, extra_values),
            None => Edn::enum_value(variant, extra_values),
          })
        }
        v => {
          Err(format!("EDN enum expected a quote or struct tag, got: {v}"))
          // TODO more types to handle
        }
      }
    }
    Buffer(buf) => Ok(Edn::Buffer(buf.to_owned())),
    CirruQuote(code) => Ok(Edn::Quote(code.to_owned())),
    Method(name, kind) => match kind {
      MethodKind::Access => Ok(Edn::Symbol(format!(".-{name}").into())),
      MethodKind::InvokeNative => Ok(Edn::Symbol(format!(".!{name}").into())),
      MethodKind::Invoke(_) => Ok(Edn::Symbol(format!(".{name}").into())),
      MethodKind::TagAccess => Ok(Edn::Symbol(format!(".:{name}").into())),
      MethodKind::AccessOptional => Ok(Edn::Symbol(format!(".?-{name}").into())),
      MethodKind::InvokeNativeOptional => Ok(Edn::Symbol(format!(".?!{name}").into())),
    },
    AnyRef(r) => Ok(Edn::AnyRef(r.to_owned())),
    Ref(_p, pair) => {
      let pair = pair.lock().expect("read ref");
      Ok(Edn::Atom(Box::new(calcit_to_edn(&pair.0)?)))
    }
    a => Err(format!("not able to generate EDN: {a:?}")), // TODO more types to handle
  }
}

const EDN_DISPLAY_CHAR_LIMIT: usize = 220;
/// Snapshot `:code` fields are shown with a larger budget so errors stay debuggable.
const SNAPSHOT_CODE_DISPLAY_CHAR_LIMIT: usize = 520;
const EDN_DISPLAY_MAX_DEPTH: usize = 10;
const EDN_DISPLAY_MAX_COLLECTION: usize = 8;

fn truncate_chars(raw: &str, limit: usize) -> String {
  if raw.chars().count() > limit {
    format!("{}…", raw.chars().take(limit).collect::<String>())
  } else {
    raw.to_owned()
  }
}

/// Convert legacy snapshot `%Expr` / `%Leaf` EDN back to Cirru for error display.
fn legacy_snapshot_edn_to_cirru(edn: Edn) -> Result<Cirru, String> {
  match edn {
    Edn::Quote(code) => Ok(code),
    Edn::Struct(struct_value) => {
      let mut text = None;
      let mut data_map: HashMap<String, Cirru> = HashMap::new();

      for (key, value) in struct_value.pairs.iter() {
        match key.ref_str() {
          "text" => {
            if let Edn::Str(content) = value {
              text = Some(content.to_string());
            }
          }
          "data" => {
            if let Edn::Map(data_edn) = value {
              for (k, v) in data_edn.0.iter() {
                if let Edn::Str(key_str) = k {
                  data_map.insert(key_str.to_string(), legacy_snapshot_edn_to_cirru(v.clone())?);
                }
              }
            }
          }
          _ => {}
        }
      }

      if let Some(t) = text {
        Ok(Cirru::leaf(t))
      } else {
        let mut sorted_items: Vec<_> = data_map.into_iter().collect();
        sorted_items.sort_by(|a, b| {
          let key_a = LexiconKey::new(&a.0).unwrap_or_else(|_| LexiconKey::default());
          let key_b = LexiconKey::new(&b.0).unwrap_or_else(|_| LexiconKey::default());
          key_a.cmp(&key_b)
        });
        Ok(Cirru::List(sorted_items.into_iter().map(|(_, v)| v).collect()))
      }
    }
    _ => Err(format!("not legacy snapshot code: <{}>", edn.type_name())),
  }
}

fn unwrap_snapshot_code_edn(value: &Edn) -> Option<&Edn> {
  match value {
    Edn::Quote(_) => Some(value),
    Edn::Struct(EdnStructView { name, pairs }) => match name.as_ref() {
      "Expr" | "Leaf" => Some(value),
      "CodeEntry" => pairs.iter().find(|(k, _)| k.ref_str() == "code").map(|(_, v)| v),
      _ => None,
    },
    _ => None,
  }
}

/// Format legacy `quote` / `%Expr` snapshot code as clean Cirru text.
fn try_format_snapshot_code_edn(value: &Edn) -> Option<String> {
  let code_edn = unwrap_snapshot_code_edn(value)?;
  let cirru = legacy_snapshot_edn_to_cirru(code_edn.clone()).ok()?;
  let formatted = cirru_parser::format(std::slice::from_ref(&cirru), true.into()).ok()?;
  Some(truncate_chars(formatted.trim(), SNAPSHOT_CODE_DISPLAY_CHAR_LIMIT))
}

fn legacy_snapshot_struct_summary(name: &str, pairs: &[(EdnTag, Edn)]) -> Option<Edn> {
  if matches!(name, "Expr" | "Leaf") {
    return try_format_snapshot_code_edn(&Edn::Struct(EdnStructView {
      name: Arc::from(name),
      pairs: pairs.to_owned(),
    }))
    .map(Edn::str);
  }
  if name == "CodeEntry" {
    let struct_value = Edn::Struct(EdnStructView {
      name: Arc::from(name),
      pairs: pairs.to_owned(),
    });
    return try_format_snapshot_code_edn(&struct_value).map(Edn::str);
  }
  if name == "FileEntry" || name == "FileChangeInfo" {
    return Some(Edn::str(format!("|%{name} legacy>")));
  }
  None
}

fn summarize_edn_debug_type(rest: &str) -> String {
  if rest.starts_with("Record(") {
    if let Some(tag) = extract_edn_tag_name(rest) {
      return format!("struct `{tag}`");
    }
    return "struct (legacy snapshot)".to_string();
  }
  if rest.starts_with("List(") {
    return "list".to_string();
  }
  if rest.starts_with("Map(") {
    return "map".to_string();
  }
  truncate_chars(rest, 72)
}

fn extract_edn_tag_name(debug_text: &str) -> Option<String> {
  let marker = "EdnTag(\"";
  let start = debug_text.find(marker)? + marker.len();
  let tail = &debug_text[start..];
  let end = tail.find('"')?;
  Some(tail[..end].to_owned())
}

/// Shrink serde/deserialize errors that embed full `Edn` Debug trees.
pub fn simplify_deserialize_error_message(msg: &str) -> String {
  const PREFIX: &str = "Cannot deserialize Edn type: ";
  if let Some(rest) = msg.strip_prefix(PREFIX) {
    let summary = summarize_edn_debug_type(rest);
    return format!("{PREFIX}{summary}");
  }
  truncate_chars(msg, 320)
}

/// Compact EDN for human-readable errors (legacy snapshot nodes, deep trees, long collections).
pub fn compact_edn_for_display(e: &Edn, depth: usize) -> Edn {
  if depth >= EDN_DISPLAY_MAX_DEPTH {
    return Edn::str("|…");
  }

  match e {
    Edn::AnyRef(_) => Edn::str("&any-ref"),
    Edn::Quote(code) => {
      if depth >= 4 {
        let preview = cirru_parser::format(std::slice::from_ref(code), true.into()).unwrap_or_else(|_| "<quote>".to_string());
        return Edn::str(truncate_chars(preview.trim(), 96));
      }
      Edn::Quote(code.clone())
    }
    Edn::List(EdnListView(xs)) => {
      let mut ys = EdnListView::default();
      for x in xs.iter().take(EDN_DISPLAY_MAX_COLLECTION) {
        ys.push(compact_edn_for_display(x, depth + 1));
      }
      if xs.len() > EDN_DISPLAY_MAX_COLLECTION {
        ys.push(Edn::str(format!("|…+{} items", xs.len() - EDN_DISPLAY_MAX_COLLECTION)));
      }
      Edn::List(ys)
    }
    Edn::Set(xs) => {
      let mut ys = EdnSetView::default();
      for (idx, x) in xs.0.iter().enumerate() {
        if idx >= EDN_DISPLAY_MAX_COLLECTION {
          break;
        }
        ys.insert(compact_edn_for_display(x, depth + 1));
      }
      if xs.0.len() > EDN_DISPLAY_MAX_COLLECTION {
        ys.insert(Edn::str(format!("|…+{} items", xs.0.len() - EDN_DISPLAY_MAX_COLLECTION)));
      }
      ys.into()
    }
    Edn::Map(xs) => {
      let mut ys = EdnMapView::default();
      for (idx, (k, v)) in xs.0.iter().enumerate() {
        if idx >= EDN_DISPLAY_MAX_COLLECTION {
          break;
        }
        ys.insert(compact_edn_for_display(k, depth + 1), compact_edn_for_display(v, depth + 1));
      }
      if xs.0.len() > EDN_DISPLAY_MAX_COLLECTION {
        ys.insert(Edn::str("|…more"), Edn::str("|…"));
      }
      ys.into()
    }
    Edn::Enum(EdnEnumView { variant, extra, type_name }) => Edn::Enum(EdnEnumView {
      variant: variant.clone(),
      extra: extra.iter().map(|x| compact_edn_for_display(x, depth + 1)).collect(),
      type_name: type_name.clone(),
    }),
    Edn::Struct(EdnStructView { name, pairs }) => {
      if let Some(summary) = legacy_snapshot_struct_summary(name, pairs) {
        return summary;
      }
      let mut ys = EdnStructView::new(name.clone());
      for (idx, (k, v)) in pairs.iter().enumerate() {
        if idx >= EDN_DISPLAY_MAX_COLLECTION {
          ys.insert(EdnTag::from("|…more"), Edn::str("|…"));
          break;
        }
        ys.insert(k.to_owned(), compact_edn_for_display(v, depth + 1));
      }
      ys.into()
    }
    Edn::Atom(inner) => Edn::Atom(Box::new(compact_edn_for_display(inner, depth + 1))),
    other => other.clone(),
  }
}

/// Format EDN for error messages and CLI previews (bounded length, legacy nodes collapsed).
pub fn format_edn_display(value: &Edn) -> String {
  if let Some(code_preview) = try_format_snapshot_code_edn(value) {
    return code_preview;
  }
  let compact = compact_edn_for_display(value, 0);
  let raw = cirru_edn::format(&compact, true).unwrap_or_else(|_| format!("<{}>", compact.type_name()));
  truncate_chars(raw.trim(), EDN_DISPLAY_CHAR_LIMIT)
}

/// `from_edn` failure with a short type summary and compact value preview.
pub fn format_deserialize_error(err: impl std::fmt::Display, context: &Edn) -> String {
  format!(
    "{}; got {}",
    simplify_deserialize_error_message(&err.to_string()),
    format_edn_display(context)
  )
}

/// Recursively replace any `Edn::AnyRef` nodes with a text placeholder so the result is safe to
/// pass to `cirru_edn::format`. Use this in display/error-snapshot paths, NOT in FFI round-trips.
pub fn sanitize_edn_for_format(e: &Edn) -> Edn {
  match e {
    Edn::AnyRef(_) => Edn::str("&any-ref"),
    Edn::List(EdnListView(xs)) => Edn::List(EdnListView(xs.iter().map(sanitize_edn_for_format).collect())),
    Edn::Set(xs) => {
      let mut ys = EdnSetView::default();
      for x in xs.0.iter() {
        ys.insert(sanitize_edn_for_format(x));
      }
      ys.into()
    }
    Edn::Map(xs) => {
      let mut ys = EdnMapView::default();
      for (k, v) in xs.0.iter() {
        ys.insert(sanitize_edn_for_format(k), sanitize_edn_for_format(v));
      }
      ys.into()
    }
    Edn::Enum(EdnEnumView { variant, extra, type_name }) => Edn::Enum(EdnEnumView {
      variant: variant.clone(),
      extra: extra.iter().map(sanitize_edn_for_format).collect(),
      type_name: type_name.clone(),
    }),
    Edn::Struct(EdnStructView { name, pairs }) => {
      let mut ys = EdnStructView::new(name.clone());
      for (k, v) in pairs.iter() {
        ys.insert(k.to_owned(), sanitize_edn_for_format(v));
      }
      ys.into()
    }
    Edn::Atom(inner) => Edn::Atom(Box::new(sanitize_edn_for_format(inner))),
    other => other.clone(),
  }
}

pub fn edn_to_calcit(x: &Edn, options: &Calcit) -> Calcit {
  match x {
    Edn::Nil => Calcit::Nil,
    Edn::Bool(b) => Calcit::Bool(*b),
    Edn::Number(n) => Calcit::Number(*n),
    Edn::Symbol(s) => Calcit::Symbol {
      sym: (**s).into(),
      info: Arc::new(crate::calcit::CalcitSymbolInfo {
        at_ns: calcit::GEN_NS.into(),
        at_def: calcit::GENERATED_DEF.into(),
      }),
      location: None,
    },
    Edn::Tag(s) => Calcit::Tag(s.to_owned()),
    Edn::Str(s) => Calcit::Str((**s).into()),
    Edn::Quote(nodes) => Calcit::CirruQuote(nodes.to_owned()),
    Edn::Enum(EdnEnumView { variant, type_name, extra }) => {
      let sum_type = type_name.as_ref().and_then(|type_name| resolve_type_name(type_name, options));
      Calcit::Enum(CalcitEnumValue {
        tag: Arc::new(Calcit::Tag(EdnTag::new(variant.as_ref()))),
        extra: extra.iter().map(|x| edn_to_calcit(x, options)).collect(),
        sum_type,
      })
    }
    Edn::List(EdnListView(xs)) => {
      let mut ys: Vec<Calcit> = vec![];
      for x in xs {
        ys.push(edn_to_calcit(x, options))
      }
      Calcit::from(CalcitList::Vector(ys))
    }
    Edn::Set(EdnSetView(xs)) => {
      let mut ys: rpds::HashTrieSetSync<Calcit> = rpds::HashTrieSet::new_sync();
      for x in xs {
        ys.insert_mut(edn_to_calcit(x, options));
      }
      Calcit::Set(ys)
    }
    Edn::Map(EdnMapView(xs)) => {
      let mut ys: rpds::HashTrieMapSync<Calcit, Calcit> = rpds::HashTrieMap::new_sync();
      for (k, v) in xs {
        ys.insert_mut(edn_to_calcit(k, options), edn_to_calcit(v, options));
      }
      Calcit::Map(ys)
    }
    Edn::Struct(EdnStructView { name, pairs: entries }) => {
      let mut fields: Vec<EdnTag> = Vec::with_capacity(entries.len());
      let mut values: Vec<Calcit> = Vec::with_capacity(entries.len());
      let mut sorted = entries.to_owned();
      sorted.sort_by(|(a, _), (b, _)| a.cmp(b));
      for v in sorted {
        fields.push(v.0.to_owned());
        values.push(edn_to_calcit(&v.1, options));
      }

      let struct_ref = match find_struct_in_options(name.as_ref(), options) {
        Some(Calcit::Struct(struct_value)) if fields == *struct_value.struct_ref.fields => Arc::clone(&struct_value.struct_ref),
        _ => Arc::new(CalcitStructDef::from_fields(EdnTag::new(name.as_ref()), fields)),
      };
      Calcit::Struct(CalcitStructValue {
        struct_ref,
        values: Arc::new(values),
      })
    }
    Edn::Buffer(buf) => Calcit::Buffer(buf.to_owned()),
    Edn::AnyRef(r) => Calcit::AnyRef(r.to_owned()),
    Edn::Atom(a) => crate::builtins::quick_build_atom(edn_to_calcit(a, options)),
  }
}
/// Find a struct prototype in EDN decoding options.
fn find_struct_in_options<'a>(name: &str, options: &'a Calcit) -> Option<&'a Calcit> {
  match options {
    Calcit::Map(ys) => ys.get(&Calcit::Tag(name.into())),
    _ => None,
  }
}

fn find_enum_in_options<'a>(name: &str, options: &'a Calcit) -> Option<&'a Calcit> {
  match options {
    Calcit::Map(ys) => ys.get(&Calcit::Tag(name.into())),
    _ => None,
  }
}

fn resolve_type_name(type_name: &str, options: &Calcit) -> Option<Arc<CalcitEnumDef>> {
  match find_enum_in_options(type_name, options) {
    Some(Calcit::EnumDef(enum_def)) => Some(Arc::new(enum_def.to_owned())),
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn simplify_deserialize_error_strips_debug_struct() {
    let msg = r#"Cannot deserialize Edn type: Record(EdnStructView { name: EdnTag("Expr"), pairs: [] })"#;
    let out = simplify_deserialize_error_message(msg);
    assert!(out.contains("struct `Expr`"), "out={out}");
    assert!(!out.contains("EdnStructView"));
  }

  #[test]
  fn format_edn_display_shows_legacy_leaf_as_cirru() {
    let mut leaf = EdnStructView::new("Leaf");
    leaf.insert(EdnTag::from("text"), Edn::str("ns respo.core"));
    let out = format_edn_display(&Edn::Struct(leaf));
    assert!(out.contains("ns respo.core"), "out={out}");
  }

  #[test]
  fn format_edn_display_shows_quote_code() {
    let code: Cirru = vec!["ns", "app.demo", ":require", "respo.core", ":refer", "div"].into();
    let out = format_edn_display(&Edn::Quote(code));
    assert!(out.contains("ns app.demo"), "out={out}");
    assert!(out.contains("respo.core"), "out={out}");
  }

  #[test]
  fn format_deserialize_error_shows_legacy_expr_as_cirru() {
    let mut leaf0 = EdnStructView::new("Leaf");
    leaf0.insert(EdnTag::from("text"), Edn::str("ns"));
    let mut leaf1 = EdnStructView::new("Leaf");
    leaf1.insert(EdnTag::from("text"), Edn::str("app.demo"));
    let mut data = EdnMapView::default();
    data.insert(Edn::str("0"), Edn::Struct(leaf0));
    data.insert(Edn::str("1"), Edn::Struct(leaf1));
    let mut expr = EdnStructView::new("Expr");
    expr.insert(EdnTag::from("data"), data.into());
    let err_msg = format_deserialize_error("Cannot deserialize Edn type: struct `Expr`", &Edn::Struct(expr));
    assert!(err_msg.contains("ns app.demo"), "err={err_msg}");
    assert!(!err_msg.contains("legacy-expr"), "err={err_msg}");
  }

  #[test]
  fn dynamic_struct_options_do_not_panic_on_field_mismatch() {
    let declared = Arc::new(CalcitStructDef::from_fields(
      EdnTag::new("Person"),
      vec![EdnTag::new("age"), EdnTag::new("name")],
    ));
    let prototype = Calcit::Struct(CalcitStructValue {
      struct_ref: declared.clone(),
      values: Arc::new(vec![Calcit::Nil, Calcit::Nil]),
    });
    let mut options = rpds::HashTrieMap::new_sync();
    options.insert_mut(Calcit::tag("Person"), prototype);

    let mut input = EdnStructView::new("Person");
    input.insert(EdnTag::new("name"), Edn::str("Ada"));
    let decoded = edn_to_calcit(&Edn::Struct(input), &Calcit::Map(options));
    let Calcit::Struct(struct_value) = decoded else {
      panic!("expected struct");
    };
    assert!(!Arc::ptr_eq(&struct_value.struct_ref, &declared));
    assert_eq!(struct_value.fields().as_slice(), &[EdnTag::new("name")]);
  }
}
