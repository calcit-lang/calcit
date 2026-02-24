use crate::{
  builtins,
  calcit::{
    self, Calcit, CalcitEnum, CalcitErr, CalcitErrKind, CalcitImpl, CalcitImport, CalcitList, CalcitLocal, CalcitProc, CalcitRecord,
    CalcitStruct, CalcitSymbolInfo, CalcitSyntax, CalcitTrait, CalcitTuple, CalcitTypeAnnotation, GEN_NS, GENERATED_DEF,
    brief_type_of_value, format_proc_examples_hint, gen_core_id, value_matches_type_annotation,
  },
  call_stack::{self, CallStackList},
  codegen::gen_ir::dump_code,
  data::{
    cirru::{self, cirru_to_calcit},
    data_to_calcit,
    edn::{self, edn_to_calcit},
  },
  runner,
  util::number::f64_to_usize,
};

use cirru_edn::EdnTag;
use cirru_parser::Cirru;

use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, atomic};
use std::{cmp::Ordering, collections::HashMap};
use std::{collections::hash_map::DefaultHasher, sync::Mutex};
use std::{
  hash::{Hash, Hasher},
  sync::LazyLock,
};

static JS_SYMBOL_INDEX: AtomicUsize = AtomicUsize::new(0);

pub(crate) static NS_SYMBOL_DICT: LazyLock<Mutex<HashMap<Arc<str>, usize>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn type_of(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  use Calcit::*;

  match &xs[0] {
    Nil => Ok(Calcit::tag("nil")),
    Bool(..) => Ok(Calcit::tag("bool")),
    Number(..) => Ok(Calcit::tag("number")),
    Symbol { .. } => Ok(Calcit::tag("symbol")),
    Tag(..) => Ok(Calcit::tag("tag")),
    Str(..) => Ok(Calcit::tag("string")),
    Thunk(..) => Ok(Calcit::tag("thunk")), // internal
    Ref(..) => Ok(Calcit::tag("ref")),
    Tuple { .. } => Ok(Calcit::tag("tuple")),
    Buffer(..) => Ok(Calcit::tag("buffer")),
    CirruQuote(..) => Ok(Calcit::tag("cirru-quote")),
    Recur(..) => Ok(Calcit::tag("recur")),
    List(..) => Ok(Calcit::tag("list")),
    Set(..) => Ok(Calcit::tag("set")),
    Map(..) => Ok(Calcit::tag("map")),
    Record { .. } => Ok(Calcit::tag("record")),
    Struct { .. } => Ok(Calcit::tag("struct")),
    Enum { .. } => Ok(Calcit::tag("enum")),
    Proc(..) => Ok(Calcit::tag("fn")), // special kind proc, but also fn
    Macro { .. } => Ok(Calcit::tag("macro")),
    Fn { .. } => Ok(Calcit::tag("fn")),
    Syntax(..) => Ok(Calcit::tag("syntax")),
    Method(..) => Ok(Calcit::tag("method")),
    RawCode(..) => Ok(Calcit::tag("raw-code")),
    Local { .. } => Ok(Calcit::tag("local")),
    Import { .. } => Ok(Calcit::tag("import")),
    Registered(..) => Ok(Calcit::tag("registered")),
    Trait(..) => Ok(Calcit::tag("trait")),
    Impl(..) => Ok(Calcit::tag("impl")),
    AnyRef(..) => Ok(Calcit::tag("any-ref")),
  }
}

pub fn recur(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  Ok(Calcit::Recur(xs.to_vec()))
}

pub fn format_to_lisp(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(v) => Ok(Calcit::Str(v.lisp_str().into())),
    None => crate::builtins::err_arity("format-to-lisp requires 1 argument, but received:", xs),
  }
}

pub fn format_to_cirru(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(v) => cirru_parser::format(&[transform_code_to_cirru(v)], false.into())
      .map(|s| Calcit::Str(s.into()))
      .map_err(|e| CalcitErr::use_str(CalcitErrKind::Syntax, e)),
    None => crate::builtins::err_arity("format-to-cirru requires 1 argument, but received:", xs),
  }
}

fn transform_code_to_cirru(x: &Calcit) -> Cirru {
  match x {
    Calcit::List(ys) => {
      let mut xs: Vec<Cirru> = Vec::with_capacity(ys.len());
      ys.traverse(&mut |y| {
        xs.push(transform_code_to_cirru(y));
      });
      Cirru::List(xs)
    }
    Calcit::Symbol { sym, .. } => Cirru::Leaf((**sym).into()),
    Calcit::Local(CalcitLocal { sym, .. }) => Cirru::Leaf((**sym).into()),
    Calcit::Import(CalcitImport { def, .. }) => Cirru::Leaf((format!("{def}")).into()), // TODO ns
    Calcit::Registered(alias) => Cirru::Leaf((**alias).into()),
    Calcit::Syntax(s, _ns) => Cirru::Leaf(s.as_ref().into()),
    Calcit::Proc(s) => Cirru::Leaf(s.as_ref().into()),
    a => Cirru::leaf(format!("{a}")),
  }
}

pub fn reset_gensym_index(_xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  force_reset_gensym_index()?;
  Ok(Calcit::Nil)
}

pub fn force_reset_gensym_index() -> Result<(), String> {
  let mut ns_symbol_dict = NS_SYMBOL_DICT.lock().expect("write symbols");
  ns_symbol_dict.clear();
  Ok(())
}

pub fn reset_js_gensym_index() {
  let _ = JS_SYMBOL_INDEX.swap(0, atomic::Ordering::SeqCst);
}

// for emitting js
pub fn js_gensym(name: &str) -> String {
  let idx = JS_SYMBOL_INDEX.fetch_add(1, atomic::Ordering::SeqCst);
  let n = idx + 1; // use 1 as first value since previous implementation did this

  let mut chunk = String::from(name);
  chunk.push_str("_AUTO_");
  chunk.push_str(&n.to_string());
  chunk
}

/// TODO, move to registered functions
pub fn generate_id(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  let size = match xs.first() {
    Some(Calcit::Number(n)) => match f64_to_usize(*n) {
      Ok(size) => Some(size),
      Err(e) => return CalcitErr::err_str(CalcitErrKind::Type, e),
    },
    Some(a) => {
      return CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("generate-id! expected a number for size, but received: {a}"),
      );
    }
    None => None, // nanoid defaults to 21
  };

  match (size, xs.get(1)) {
    (None, None) => Ok(Calcit::Str(gen_core_id())),
    (Some(_n), None) => Ok(Calcit::Str(gen_core_id())),
    (Some(_n), Some(Calcit::Str(s))) => {
      let mut charset: Vec<char> = Vec::with_capacity(s.len());
      for c in s.chars() {
        charset.push(c);
      }
      Ok(Calcit::Str(gen_core_id()))
    }
    (a, b) => CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("generate-id! expected a number for size or a string for charset, but received: {a:?} {b:?}"),
    ),
  }
}

pub fn display_stack(_xs: &[Calcit], call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  call_stack::show_stack(call_stack);
  Ok(Calcit::Nil)
}

pub fn parse_cirru_list(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Str(s)) => match cirru_parser::parse(s) {
      Ok(nodes) => Ok(cirru::cirru_to_calcit(&Cirru::List(nodes))),
      Err(e) => {
        eprintln!("\nparse-cirru-list failed:");
        eprintln!("{}", e.format_detailed(Some(s)));
        CalcitErr::err_str(CalcitErrKind::Syntax, "parse-cirru-list failed")
      }
    },
    Some(a) => {
      let msg = format!(
        "parse-cirru-list requires a string, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::ParseCirruList).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    None => {
      let hint = format_proc_examples_hint(&CalcitProc::ParseCirruList).unwrap_or_default();
      CalcitErr::err_str_with_hint(
        CalcitErrKind::Arity,
        "parse-cirru-list requires 1 argument, but received none".to_string(),
        hint,
      )
    }
  }
}

/// it returns a piece of quoted Cirru data, rather than a list
pub fn parse_cirru(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Str(s)) => match cirru_parser::parse(s) {
      Ok(nodes) => Ok(Calcit::CirruQuote(Cirru::List(nodes))),
      Err(e) => {
        eprintln!("\nparse-cirru failed:");
        eprintln!("{}", e.format_detailed(Some(s)));
        CalcitErr::err_str(CalcitErrKind::Syntax, "parse-cirru failed")
      }
    },
    Some(a) => {
      let msg = format!(
        "parse-cirru requires a string, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::ParseCirru).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    None => {
      let hint = format_proc_examples_hint(&CalcitProc::ParseCirru).unwrap_or_default();
      CalcitErr::err_str_with_hint(
        CalcitErrKind::Arity,
        "parse-cirru requires 1 argument, but received none".to_string(),
        hint,
      )
    }
  }
}

pub fn format_cirru(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(a) => match cirru::calcit_data_to_cirru(a) {
      Ok(v) => {
        if let Cirru::List(ys) = v {
          Ok(Calcit::Str(cirru_parser::format(&ys, false.into())?.into()))
        } else {
          CalcitErr::err_str(
            CalcitErrKind::Type,
            format!("format-cirru expected a list for Cirru formatting, but received: {v}"),
          )
        }
      }
      Err(e) => CalcitErr::err_str(CalcitErrKind::Syntax, format!("format-cirru failed: {e}")),
    },
    None => CalcitErr::err_str(CalcitErrKind::Arity, "format-cirru expected 1 argument, but received none"),
  }
}

pub fn format_cirru_one_liner(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(a) => match cirru::calcit_data_to_cirru(a) {
      Ok(v) => {
        // Format the expression directly
        match cirru_parser::format_expr_one_liner(&v) {
          Ok(s) => Ok(Calcit::Str(s.into())),
          Err(e) => CalcitErr::err_str(CalcitErrKind::Syntax, format!("format-cirru-one-liner failed: {e}")),
        }
      }
      Err(e) => CalcitErr::err_str(CalcitErrKind::Syntax, format!("format-cirru-one-liner failed: {e}")),
    },
    None => {
      let hint = format_proc_examples_hint(&CalcitProc::FormatCirruOneLiner).unwrap_or_default();
      CalcitErr::err_str_with_hint(
        CalcitErrKind::Arity,
        "format-cirru-one-liner requires 1 argument, but received none".to_string(),
        hint,
      )
    }
  }
}

pub fn parse_cirru_edn(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Str(s)) => match cirru_edn::parse(s) {
      Ok(nodes) => match xs.get(1) {
        Some(options) => Ok(edn::edn_to_calcit(&nodes, options)),
        None => Ok(edn::edn_to_calcit(&nodes, &Calcit::Nil)),
      },
      Err(e) => {
        eprintln!("\nparse-cirru-edn failed:");
        eprintln!("{e}");
        CalcitErr::err_str(CalcitErrKind::Syntax, "parse-cirru-edn failed")
      }
    },
    Some(a) => {
      let msg = format!(
        "parse-cirru-edn requires a string, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::ParseCirruEdn).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    None => {
      let hint = format_proc_examples_hint(&CalcitProc::ParseCirruEdn).unwrap_or_default();
      CalcitErr::err_str_with_hint(
        CalcitErrKind::Arity,
        "parse-cirru-edn requires 1 argument, but received none".to_string(),
        hint,
      )
    }
  }
}

pub fn format_cirru_edn(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(a) => Ok(Calcit::Str(cirru_edn::format(&edn::calcit_to_edn(a)?, true)?.into())),
    None => {
      let hint = format_proc_examples_hint(&CalcitProc::FormatCirruEdn).unwrap_or_default();
      CalcitErr::err_str_with_hint(
        CalcitErrKind::Arity,
        "format-cirru-edn requires 1 argument, but received none".to_string(),
        hint,
      )
    }
  }
}

pub fn cirru_quote_to_list(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&cirru-quote:to-list expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::CirruQuote(ys) => Ok(cirru_to_calcit(ys)),
    a => {
      let msg = format!(
        "&cirru-quote:to-list requires a Cirru quote, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeCirruQuoteToList).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

/// missing location for a dynamic symbol
pub fn turn_symbol(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "turn-symbol expected 1 argument, but received:", xs);
  }
  let info = Arc::new(CalcitSymbolInfo {
    at_ns: calcit::GEN_NS.into(),
    at_def: calcit::GENERATED_DEF.into(),
  });
  match &xs[0] {
    Calcit::Str(s) => Ok(Calcit::Symbol {
      sym: s.to_owned(),
      info: info.to_owned(),
      location: None,
    }),
    Calcit::Tag(s) => Ok(Calcit::Symbol {
      sym: s.arc_str(),
      info: info.to_owned(),
      location: None,
    }),
    a @ Calcit::Symbol { .. } => Ok(a.to_owned()),
    a => {
      let msg = format!("turn-symbol cannot convert to symbol: {}", type_of(&[a.to_owned()])?.lisp_str());
      let hint = format_proc_examples_hint(&CalcitProc::TurnSymbol).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn turn_tag(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "turn-tag expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::Str(s) => Ok(Calcit::tag(s)),
    Calcit::Tag(s) => Ok(Calcit::Tag(s.to_owned())),
    Calcit::Symbol { sym, .. } => Ok(Calcit::tag(sym)),
    a => {
      let msg = format!("turn-tag cannot convert to tag: {}", type_of(&[a.to_owned()])?.lisp_str());
      let hint = format_proc_examples_hint(&CalcitProc::TurnTag).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn new_tuple(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.is_empty() {
    let msg = format!("tuple requires at least 1 argument (tag), but received: {} arguments", xs.len());
    CalcitErr::err_str(CalcitErrKind::Arity, msg)
  } else {
    let extra: Vec<Calcit> = if xs.len() == 1 {
      vec![]
    } else {
      let mut ys: Vec<Calcit> = Vec::with_capacity(xs.len() - 1);
      for item in xs.iter().skip(1) {
        ys.push(item.to_owned());
      }
      ys
    };
    Ok(Calcit::Tuple(CalcitTuple {
      tag: Arc::new(xs[0].to_owned()),
      extra,
      sum_type: None,
    }))
  }
}

pub fn new_enum_tuple_no_class(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() < 2 {
    CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("%:: expected at least 2 arguments, but received: {}", CalcitList::from(xs)),
    )
  } else {
    let enum_value = xs[0].to_owned();
    match enum_value {
      Calcit::Record(enum_record) => {
        let enum_proto = match CalcitEnum::from_record(enum_record.clone()) {
          Ok(proto) => proto,
          Err(msg) => {
            return CalcitErr::err_str(CalcitErrKind::Type, format!("%:: expected a valid enum prototype, but {msg}"));
          }
        };

        let tag_value = &xs[1];
        let tag_name = match tag_value {
          Calcit::Tag(t) => t.ref_str(),
          other => {
            let msg = format!("%:: requires a tag, but received: {}", type_of(&[other.to_owned()])?.lisp_str());
            let hint = format_proc_examples_hint(&CalcitProc::NativeEnumTupleNew).unwrap_or_default();
            return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
          }
        };

        match enum_proto.find_variant_by_name(tag_name) {
          Some(variant) => {
            let payload_count = xs.len() - 2;
            let expected_arity = variant.arity();
            if payload_count != expected_arity {
              return CalcitErr::err_str(
                CalcitErrKind::Arity,
                format!("enum variant `{tag_name}` expects {expected_arity} payload(s), but received: {payload_count}"),
              );
            }
            // Validate payload types against enum variant type annotations
            for (idx, (payload, expected_type)) in xs.iter().skip(2).zip(variant.payload_types().iter()).enumerate() {
              if !matches!(expected_type.as_ref(), CalcitTypeAnnotation::Dynamic)
                && !value_matches_type_annotation(payload, expected_type)
              {
                return CalcitErr::err_str(
                  CalcitErrKind::Type,
                  format!(
                    "%:: enum `{}::{}` payload {} expects type `{}`, but received `{}` ({})",
                    enum_proto.name(),
                    tag_name,
                    idx + 1,
                    expected_type.to_brief_string(),
                    brief_type_of_value(payload),
                    payload.lisp_str()
                  ),
                );
              }
            }
          }
          None => {
            return CalcitErr::err_str(
              CalcitErrKind::Type,
              format!("enum `{}` does not have variant `{}`", enum_proto.name(), tag_name),
            );
          }
        }

        let extra: Vec<Calcit> = xs.iter().skip(2).cloned().collect();
        Ok(Calcit::Tuple(CalcitTuple {
          tag: Arc::new(xs[1].to_owned()),
          extra,
          sum_type: Some(Arc::new(enum_proto)),
        }))
      }
      Calcit::Enum(enum_def) => {
        let enum_proto = enum_def.clone();

        let tag_value = &xs[1];
        let tag_name = match tag_value {
          Calcit::Tag(t) => t.ref_str(),
          other => {
            let msg = format!("%:: requires a tag, but received: {}", type_of(&[other.to_owned()])?.lisp_str());
            let hint = format_proc_examples_hint(&CalcitProc::NativeEnumTupleNew).unwrap_or_default();
            return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
          }
        };

        match enum_proto.find_variant_by_name(tag_name) {
          Some(variant) => {
            let payload_count = xs.len() - 2;
            let expected_arity = variant.arity();
            if payload_count != expected_arity {
              return CalcitErr::err_str(
                CalcitErrKind::Arity,
                format!("enum variant `{tag_name}` expects {expected_arity} payload(s), but received: {payload_count}"),
              );
            }
            // Validate payload types against enum variant type annotations
            for (idx, (payload, expected_type)) in xs.iter().skip(2).zip(variant.payload_types().iter()).enumerate() {
              if !matches!(expected_type.as_ref(), CalcitTypeAnnotation::Dynamic)
                && !value_matches_type_annotation(payload, expected_type)
              {
                return CalcitErr::err_str(
                  CalcitErrKind::Type,
                  format!(
                    "%:: enum `{}::{}` payload {} expects type `{}`, but received `{}` ({})",
                    enum_proto.name(),
                    tag_name,
                    idx + 1,
                    expected_type.to_brief_string(),
                    brief_type_of_value(payload),
                    payload.lisp_str()
                  ),
                );
              }
            }
          }
          None => {
            return CalcitErr::err_str(
              CalcitErrKind::Type,
              format!("enum `{}` does not have variant `{}`", enum_proto.name(), tag_name),
            );
          }
        }

        let extra: Vec<Calcit> = xs.iter().skip(2).cloned().collect();
        Ok(Calcit::Tuple(CalcitTuple {
          tag: Arc::new(xs[1].to_owned()),
          extra,
          sum_type: Some(Arc::new(enum_proto)),
        }))
      }
      other => CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("%:: expected a record as enum prototype, but received: {other}"),
      ),
    }
  }
}

/// Get the enum prototype from a tuple
pub fn tuple_enum(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&tuple:enum expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::Tuple(t) => match &t.sum_type {
      Some(enum_proto) => Ok(Calcit::Enum((**enum_proto).clone())),
      None => Ok(Calcit::Nil),
    },
    a => {
      let msg = format!(
        "&tuple:enum requires a tuple, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeTupleEnum).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn trait_new(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&trait::new expected 2 arguments, but received:", xs);
  }
  fn normalize_type_form(form: &Calcit) -> Calcit {
    match form {
      Calcit::List(list) => {
        let mut items: Vec<Calcit> = list.iter().map(normalize_type_form).collect();
        let is_list_literal = matches!(items.first(), Some(Calcit::Proc(CalcitProc::List)))
          || matches!(items.first(), Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "[]");
        if is_list_literal {
          items.remove(0);
        }
        Calcit::List(Arc::new(CalcitList::from(items.as_slice())))
      }
      Calcit::Tuple(tuple) => {
        let tag = tuple.tag.to_owned();
        let extra = tuple.extra.to_owned();
        let sum_type = tuple.sum_type.to_owned();
        Calcit::Tuple(CalcitTuple { tag, extra, sum_type })
      }
      _ => form.to_owned(),
    }
  }

  let name = match &xs[0] {
    Calcit::Symbol { sym, .. } => cirru_edn::EdnTag::new(sym.as_ref()),
    Calcit::Tag(tag) => tag.to_owned(),
    other => {
      return CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&trait::new expects a tag/symbol as name, but received: {other}"),
      );
    }
  };
  fn contains_dynamic(annotation: &CalcitTypeAnnotation) -> bool {
    use CalcitTypeAnnotation as T;
    match annotation {
      T::Dynamic => true,
      T::List(inner) | T::Set(inner) | T::Ref(inner) | T::Variadic(inner) | T::Optional(inner) => contains_dynamic(inner),
      T::Map(k, v) => contains_dynamic(k) || contains_dynamic(v),
      T::Fn(info) => info.arg_types.iter().any(|t| contains_dynamic(t)) || contains_dynamic(info.return_type.as_ref()),
      T::AppliedStruct { args, .. } => args.iter().any(|t| contains_dynamic(t)),
      _ => false,
    }
  }

  let (methods, method_types) = match &xs[1] {
    Calcit::List(list) => {
      let mut items = Vec::with_capacity(list.len());
      let mut types = Vec::with_capacity(list.len());
      for item in list.iter() {
        match item {
          Calcit::List(entry) => {
            if entry.len() != 2 {
              return CalcitErr::err_str(
                CalcitErrKind::Type,
                format!("&trait::new expects (method type) pairs, but received: {item}"),
              );
            }
            let name = match entry.first().unwrap() {
              Calcit::Tag(tag) => tag.to_owned(),
              Calcit::Symbol { sym, .. } => cirru_edn::EdnTag::new(sym.as_ref()),
              other => {
                return CalcitErr::err_str(
                  CalcitErrKind::Type,
                  format!("&trait::new expects method names as tags/symbols, but received: {other}"),
                );
              }
            };
            let type_form = entry.get(1).unwrap();
            let type_form_value = match type_form {
              Calcit::CirruQuote(ys) => cirru_to_calcit(ys),
              _ => type_form.to_owned(),
            };
            let type_form_value = normalize_type_form(&type_form_value);
            let method_type = CalcitTypeAnnotation::parse_type_annotation_form(&type_form_value);
            if matches!(method_type.as_ref(), CalcitTypeAnnotation::Dynamic) {
              return CalcitErr::err_str(
                CalcitErrKind::Type,
                format!("&trait::new does not allow Dynamic in method signatures, use :fn (DynFn) if needed: {type_form_value}"),
              );
            }
            if !matches!(method_type.as_ref(), CalcitTypeAnnotation::Fn(_) | CalcitTypeAnnotation::DynFn) {
              return CalcitErr::err_str(
                CalcitErrKind::Type,
                format!("&trait::new expects method type to be :fn or (:: :fn ...), but received: {type_form_value}"),
              );
            }
            if matches!(method_type.as_ref(), CalcitTypeAnnotation::Fn(_)) && contains_dynamic(method_type.as_ref()) {
              return CalcitErr::err_str(
                CalcitErrKind::Type,
                format!("&trait::new does not allow Dynamic inside method signatures: {type_form_value}"),
              );
            }
            items.push(name);
            types.push(method_type);
          }
          Calcit::Tag(_) | Calcit::Symbol { .. } => {
            return CalcitErr::err_str(
              CalcitErrKind::Type,
              format!("&trait::new expects (method type) pairs, but received method name without type: {item}"),
            );
          }
          other => {
            return CalcitErr::err_str(
              CalcitErrKind::Type,
              format!("&trait::new expects (method type) pairs, but received: {other}"),
            );
          }
        }
      }
      (items, types)
    }
    other => {
      return CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&trait::new expects a list of method specs, but received: {other}"),
      );
    }
  };

  Ok(Calcit::Trait(CalcitTrait::new(name, methods, method_types)))
}

fn collect_trait_records(xs: &[Calcit], proc_name: &str) -> Result<Vec<Arc<CalcitImpl>>, CalcitErr> {
  let mut traits: Vec<Arc<CalcitImpl>> = Vec::with_capacity(xs.len());
  for item in xs {
    match item {
      Calcit::Impl(imp) => traits.push(Arc::new(imp.to_owned())),
      other => {
        return Err(CalcitErr::use_str(
          CalcitErrKind::Type,
          format!("{proc_name} expects trait impls as impls, but received: {other}"),
        ));
      }
    }
  }
  Ok(traits)
}

pub fn record_impl_traits(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() < 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&record:impl-traits expected 2+ arguments, but received:", xs);
  }
  match &xs[0] {
    Calcit::Record(record) => {
      let mut impls = record.struct_ref.impls.clone();
      impls.extend(collect_trait_records(&xs[1..], "&record:impl-traits")?);
      let mut next_struct = (*record.struct_ref).clone();
      next_struct.impls = impls;

      Ok(Calcit::Record(CalcitRecord {
        struct_ref: Arc::new(next_struct),
        values: record.values.to_owned(),
      }))
    }
    other => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&record:impl-traits expected a record, but received: {other}"),
    ),
  }
}

pub fn tuple_impl_traits(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() < 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&tuple:impl-traits expected 2+ arguments, but received:", xs);
  }
  match &xs[0] {
    Calcit::Tuple(tuple) => {
      let mut next_sum_type = match &tuple.sum_type {
        Some(s) => (**s).clone(),
        None => {
          let tag_name = match &*tuple.tag {
            Calcit::Tag(t) => t.to_owned(),
            _ => EdnTag::from("tag"),
          };
          let record = CalcitRecord {
            struct_ref: Arc::new(CalcitStruct::from_fields(EdnTag::from("anonymous-tuple"), vec![tag_name])),
            values: Arc::new(vec![Calcit::List(Arc::new(CalcitList::from(
              &vec![Calcit::tag("any"); tuple.extra.len()][..],
            )))]),
          };
          CalcitEnum::from_record(record).map_err(|msg| {
            CalcitErr::use_msg_stack_location(
              CalcitErrKind::Type,
              format!("failed to create anonymous enum for tuple, {msg}"),
              &CallStackList::default(),
              tuple.tag.get_location(),
            )
          })?
        }
      };
      next_sum_type.impls.extend(collect_trait_records(&xs[1..], "&tuple:impl-traits")?);
      Ok(Calcit::Tuple(CalcitTuple {
        tag: tuple.tag.to_owned(),
        extra: tuple.extra.to_owned(),
        sum_type: Some(Arc::new(next_sum_type)),
      }))
    }
    other => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&tuple:impl-traits expected a tuple, but received: {other}"),
    ),
  }
}

pub fn struct_impl_traits(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() < 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&struct:impl-traits expected 2+ arguments, but received:", xs);
  }
  match &xs[0] {
    Calcit::Struct(struct_def) => {
      let mut next = struct_def.to_owned();
      let mut next_impls = next.impls.clone();
      next_impls.extend(collect_trait_records(&xs[1..], "&struct:impl-traits")?);
      next.impls = next_impls;
      Ok(Calcit::Struct(next))
    }
    other => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&struct:impl-traits expected a struct, but received: {other}"),
    ),
  }
}

pub fn enum_impl_traits(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() < 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&enum:impl-traits expected 2+ arguments, but received:", xs);
  }
  match &xs[0] {
    Calcit::Enum(enum_def) => {
      let mut next = enum_def.to_owned();
      next.impls.extend(collect_trait_records(&xs[1..], "&enum:impl-traits")?);
      Ok(Calcit::Enum(next))
    }
    Calcit::Record(record) => {
      let mut impls = record.struct_ref.impls.clone();
      impls.extend(collect_trait_records(&xs[1..], "&enum:impl-traits")?);
      let mut next_struct = (*record.struct_ref).clone();
      next_struct.impls = impls;
      Ok(Calcit::Record(CalcitRecord {
        struct_ref: Arc::new(next_struct),
        values: record.values.to_owned(),
      }))
    }
    other => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&enum:impl-traits expected an enum or enum record, but received: {other}"),
    ),
  }
}

pub fn impl_origin(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&impl:origin expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::Impl(imp) => match &imp.origin {
      Some(trait_def) => Ok(Calcit::Trait(trait_def.as_ref().to_owned())),
      None => Ok(Calcit::Nil),
    },
    other => CalcitErr::err_str(CalcitErrKind::Type, format!("&impl:origin expected an impl, but received: {other}")),
  }
}

pub fn impl_get(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&impl:get expected 2 arguments, but received:", xs);
  }
  let name = match &xs[1] {
    Calcit::Tag(tag) => tag.ref_str(),
    Calcit::Str(s) => s.as_ref(),
    Calcit::Symbol { sym, .. } => sym.as_ref(),
    other => {
      let msg = format!(
        "&impl:get expects method name as tag/string/symbol, but received: {}",
        type_of(&[other.to_owned()])?.lisp_str()
      );
      return CalcitErr::err_str(CalcitErrKind::Type, msg);
    }
  };

  match &xs[0] {
    Calcit::Impl(imp) => match imp.get(name) {
      Some(value) => Ok(value.to_owned()),
      None => CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&impl:get cannot find method `{name}` in impl `{}`", imp.name),
      ),
    },
    other => CalcitErr::err_str(CalcitErrKind::Type, format!("&impl:get expected an impl, but received: {other}")),
  }
}

pub fn impl_nth(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&impl:nth expected 2 arguments, but received:", xs);
  }
  let index = match &xs[1] {
    Calcit::Number(n) if n.fract() == 0.0 && *n >= 0.0 => *n as usize,
    other => {
      let msg = format!(
        "&impl:nth expects a non-negative integer index, but received: {}",
        type_of(&[other.to_owned()])?.lisp_str()
      );
      return CalcitErr::err_str(CalcitErrKind::Type, msg);
    }
  };

  match &xs[0] {
    Calcit::Impl(imp) => match imp.values.get(index) {
      Some(value) => Ok(value.to_owned()),
      None => CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&impl:nth index {index} out of bounds for impl `{}`", imp.name),
      ),
    },
    other => CalcitErr::err_str(CalcitErrKind::Type, format!("&impl:nth expected an impl, but received: {other}")),
  }
}

fn parse_enum_record(enum_record: &CalcitRecord, proc_name: &str) -> Result<CalcitEnum, CalcitErr> {
  match CalcitEnum::from_record(enum_record.to_owned()) {
    Ok(proto) => Ok(proto),
    Err(msg) => Err(CalcitErr::use_str(
      CalcitErrKind::Type,
      format!("{proc_name} expected a valid enum record, but {msg}"),
    )),
  }
}

/// Check if an enum has a variant
pub fn tuple_enum_has_variant(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "&tuple:enum-has-variant? expected 2 arguments, but received:",
      xs,
    );
  }
  match (&xs[0], &xs[1]) {
    (Calcit::Record(enum_record), Calcit::Tag(tag)) => {
      let enum_proto = parse_enum_record(enum_record, "&tuple:enum-has-variant?")?;
      Ok(Calcit::Bool(enum_proto.find_variant(tag).is_some()))
    }
    (Calcit::Enum(enum_def), Calcit::Tag(tag)) => Ok(Calcit::Bool(enum_def.find_variant(tag).is_some())),
    (Calcit::Record(_) | Calcit::Enum(_), other) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&tuple:enum-has-variant? expected a tag as second argument, but received: {other}"),
    ),
    (other, _) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&tuple:enum-has-variant? expected an enum as first argument, but received: {other}"),
    ),
  }
}

/// Get the arity of a variant in an enum
pub fn tuple_enum_variant_arity(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "&tuple:enum-variant-arity expected 2 arguments, but received:",
      xs,
    );
  }
  match (&xs[0], &xs[1]) {
    (Calcit::Record(enum_record), Calcit::Tag(tag)) => {
      let enum_proto = parse_enum_record(enum_record, "&tuple:enum-variant-arity")?;
      match enum_proto.find_variant(tag) {
        Some(variant) => Ok(Calcit::Number(variant.arity() as f64)),
        None => CalcitErr::err_str(
          CalcitErrKind::Type,
          format!(
            "&tuple:enum-variant-arity: enum `{}` does not have variant `{}`",
            enum_proto.name(),
            tag
          ),
        ),
      }
    }
    (Calcit::Enum(enum_def), Calcit::Tag(tag)) => match enum_def.find_variant(tag) {
      Some(variant) => Ok(Calcit::Number(variant.arity() as f64)),
      None => CalcitErr::err_str(
        CalcitErrKind::Type,
        format!(
          "&tuple:enum-variant-arity: enum `{}` does not have variant `{}`",
          enum_def.name(),
          tag
        ),
      ),
    },
    (Calcit::Record(_) | Calcit::Enum(_), other) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&tuple:enum-variant-arity expected a tag as second argument, but received: {other}"),
    ),
    (other, _) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&tuple:enum-variant-arity expected an enum as first argument, but received: {other}"),
    ),
  }
}

/// Validate enum tuple tag and arity if enum metadata exists
pub fn tuple_validate_enum(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&tuple:validate-enum expected 2 arguments, but received:", xs);
  }
  match (&xs[0], &xs[1]) {
    (Calcit::Tuple(tuple), Calcit::Tag(tag)) => {
      let tuple_value = Calcit::Tuple(tuple.to_owned());
      if let Some(enum_proto) = &tuple.sum_type {
        match enum_proto.find_variant(tag) {
          Some(variant) => {
            let expected = variant.arity();
            let actual = tuple.extra.len();
            if expected != actual {
              return CalcitErr::err_str(
                CalcitErrKind::Type,
                format!("enum variant expects {expected} payload(s), got {actual} for {tuple_value}"),
              );
            }
          }
          None => {
            return CalcitErr::err_str(CalcitErrKind::Type, format!("enum does not have variant {tag} for {tuple_value}"));
          }
        }
      }
      Ok(Calcit::Nil)
    }
    (Calcit::Tuple(_), other) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&tuple:validate-enum expected a tag as second argument, but received: {other}"),
    ),
    (other, _) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&tuple:validate-enum expected a tuple as first argument, but received: {other}"),
    ),
  }
}

pub fn invoke_method(name: &str, method_args: &[Calcit], call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if method_args.is_empty() {
    return Err(CalcitErr::use_msg_stack(
      CalcitErrKind::Arity,
      format!("invoke-method expected an operand, but received none: {method_args:?}"),
      call_stack,
    ));
  }
  let v0 = &method_args[0];
  if runner::preprocess::is_warn_dyn_method_enabled() {
    let value_type = type_of(&[v0.to_owned()])
      .map(|t| t.lisp_str())
      .unwrap_or_else(|_| "<unknown>".to_string());
    eprintln!(
      "[warn-dyn-method] runtime invoke-method lookup for .{name} on {value_type} {v0}",
      v0 = v0.lisp_str()
    );
  }
  use Calcit::*;
  match v0 {
    // user-defined values: impl-traits appends, so later impls override earlier ones
    Tuple(tuple) => method_call_impls(tuple.impls(), v0, name, method_args, call_stack, true),
    Record(record) => method_call_impls(&record.struct_ref.impls, v0, name, method_args, call_stack, true),

    // builtin values should already be preprocessed
    List(..) => {
      let impls_value = runner::evaluate_symbol_from_program("&core-list-impls", calcit::CORE_NS, None, call_stack)?;
      method_call(&impls_value, v0, name, method_args, call_stack)
    }
    Map(..) => {
      let impls_value = runner::evaluate_symbol_from_program("&core-map-impls", calcit::CORE_NS, None, call_stack)?;
      method_call(&impls_value, v0, name, method_args, call_stack)
    }
    Number(..) => {
      let impls_value = runner::evaluate_symbol_from_program("&core-number-impls", calcit::CORE_NS, None, call_stack)?;
      method_call(&impls_value, v0, name, method_args, call_stack)
    }
    Str(..) => {
      let impls_value = runner::evaluate_symbol_from_program("&core-string-impls", calcit::CORE_NS, None, call_stack)?;
      method_call(&impls_value, v0, name, method_args, call_stack)
    }
    Set(..) => {
      let impls_value = &runner::evaluate_symbol_from_program("&core-set-impls", calcit::CORE_NS, None, call_stack)?;
      method_call(impls_value, v0, name, method_args, call_stack)
    }
    Fn { .. } | Proc(..) => {
      let impls_value = runner::evaluate_symbol_from_program("&core-fn-impls", calcit::CORE_NS, None, call_stack)?;
      method_call(&impls_value, v0, name, method_args, call_stack)
    }
    x => Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Type,
      format!("invoke-method cannot resolve impls for value: {x}"),
      call_stack,
      x.get_location(),
    )),
  }
}

fn collect_impl_records_from_value(impls_value: &Calcit, call_stack: &CallStackList) -> Result<Vec<Arc<CalcitImpl>>, CalcitErr> {
  match impls_value {
    Calcit::Impl(imp) => Ok(vec![Arc::new(imp.to_owned())]),
    Calcit::List(list) => {
      let mut impls: Vec<Arc<CalcitImpl>> = Vec::with_capacity(list.len());
      for item in list.iter() {
        match item {
          Calcit::Impl(imp) => impls.push(Arc::new(imp.to_owned())),
          other => {
            return Err(CalcitErr::use_msg_stack(
              CalcitErrKind::Type,
              format!("invoke-method expects impls in list, but received: {other}"),
              call_stack,
            ));
          }
        }
      }
      Ok(impls)
    }
    other => Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Type,
      format!("invoke-method cannot resolve impls from: {other}"),
      call_stack,
      other.get_location(),
    )),
  }
}

fn method_call(
  impls_value: &Calcit,
  v0: &Calcit,
  name: &str,
  method_args: &[Calcit],
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  let impls = collect_impl_records_from_value(impls_value, call_stack)?;
  // builtin impl lists are ordered by priority in calcit-core
  method_call_impls(&impls, v0, name, method_args, call_stack, false)
}

fn method_call_impls(
  impls: &[Arc<CalcitImpl>],
  v0: &Calcit,
  name: &str,
  method_args: &[Calcit],
  call_stack: &CallStackList,
  last_wins: bool,
) -> Result<Calcit, CalcitErr> {
  if impls.is_empty() {
    return Err(CalcitErr::use_msg_stack(
      CalcitErrKind::Type,
      format!("invoke-method cannot resolve impls for: {v0}"),
      call_stack,
    ));
  }
  if last_wins {
    for imp in impls.iter().rev() {
      if imp.get(name).is_some() {
        return method_record(imp, v0, name, method_args, call_stack);
      }
    }
  } else {
    for imp in impls.iter() {
      if imp.get(name).is_some() {
        return method_record(imp, v0, name, method_args, call_stack);
      }
    }
  }
  let mut fields: Vec<String> = vec![];
  for imp in impls {
    for field in imp.fields().iter() {
      fields.push(field.to_string());
    }
  }
  let content = fields.join(" ");
  Err(CalcitErr::use_msg_stack(
    CalcitErrKind::Type,
    format!("unknown method `.{name}` for {v0}. Available methods: {content}"),
    call_stack,
  ))
}

fn method_record(
  impl_record: &CalcitImpl,
  v0: &Calcit,
  name: &str,
  method_args: &[Calcit],
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  match impl_record.get(name) {
    Some(v) => {
      match v {
        // dirty copy...
        Calcit::Fn { info, .. } => runner::run_fn(method_args, info, call_stack),
        Calcit::Proc(proc) => builtins::handle_proc(*proc, method_args, call_stack),
        Calcit::Syntax(syn, _ns) => Err(CalcitErr::use_msg_stack(
          CalcitErrKind::Syntax,
          format!("invoke-method cannot get syntax here since instance is always evaluated, but received: {syn}"),
          call_stack,
        )),
        y => Err(CalcitErr::use_msg_stack_location(
          CalcitErrKind::Type,
          format!("invoke-method expected a function to invoke, but received: {y}"),
          call_stack,
          y.get_location(),
        )),
      }
    }
    None => {
      let content = impl_record.fields().iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" ");
      Err(CalcitErr::use_msg_stack(
        CalcitErrKind::Type,
        format!("unknown method `.{name}` for {v0}. Available methods: {content}"),
        call_stack,
      ))
    }
  }
}

pub fn native_compare(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return crate::builtins::err_arity("&compare requires 2 arguments, but received:", xs);
  }
  match xs[0].cmp(&xs[1]) {
    Ordering::Less => Ok(Calcit::Number(-1.0)),
    Ordering::Greater => Ok(Calcit::Number(1.0)),
    Ordering::Equal => Ok(Calcit::Number(0.0)),
  }
}

pub fn tuple_nth(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&tuple:nth expected 2 arguments, but received:", xs);
  }
  match (&xs[0], &xs[1]) {
    (Calcit::Tuple(CalcitTuple { tag, extra, .. }), Calcit::Number(n)) => match f64_to_usize(*n) {
      Ok(0) => Ok((**tag).to_owned()) as Result<Calcit, CalcitErr>,
      Ok(m) => {
        if m - 1 < extra.len() {
          Ok(extra[m - 1].to_owned())
        } else {
          let size = extra.len() + 1;
          CalcitErr::err_str(
            CalcitErrKind::Arity,
            format!("&tuple:nth index out of range. Tuple has {size} elements, but trying to index with {m}"),
          )
        }
      }
      Err(e) => CalcitErr::err_str(CalcitErrKind::Type, format!("&tuple:nth expected a valid index, {e}")),
    },
    (a, b) => {
      let msg = format!(
        "&tuple:nth requires a tuple and an index, but received: {} and {}",
        type_of(&[a.to_owned()])?.lisp_str(),
        type_of(&[b.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeTupleNth).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn assoc(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 3 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&tuple:assoc expected 3 arguments, but received:", xs);
  }
  match (&xs[0], &xs[1]) {
    (Calcit::Tuple(CalcitTuple { tag, extra, sum_type }), Calcit::Number(n)) => match f64_to_usize(*n) {
      Ok(idx) => {
        if idx == 0 {
          Ok(Calcit::Tuple(CalcitTuple {
            tag: Arc::new(xs[2].to_owned()),
            extra: extra.to_owned(),
            sum_type: sum_type.to_owned(),
          }))
        } else if idx - 1 < extra.len() {
          let mut new_extra = extra.to_owned();
          xs[2].clone_into(&mut new_extra[idx - 1]);
          Ok(Calcit::Tuple(CalcitTuple {
            tag: tag.to_owned(),
            extra: new_extra,
            sum_type: sum_type.to_owned(),
          }))
        } else {
          CalcitErr::err_str(
            CalcitErrKind::Arity,
            format!("&tuple:assoc index out of range. Tuple only has fields at index 0, 1, but received unknown index: {idx}"),
          )
        }
      }
      Err(e) => CalcitErr::err_str(CalcitErrKind::Type, e),
    },
    (a, b, ..) => {
      let msg = format!(
        "&tuple:assoc requires a tuple and an index, but received: {} and {}",
        type_of(&[a.to_owned()])?.lisp_str(),
        type_of(&[b.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeTupleAssoc).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn tuple_count(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&tuple:count expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::Tuple(CalcitTuple { extra, .. }) => Ok(Calcit::Number((extra.len() + 1) as f64)),
    x => {
      let msg = format!(
        "&tuple:count requires a tuple, but received: {}",
        type_of(&[x.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeTupleCount).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn tuple_impls(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&tuple:impls expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::Tuple(tuple) => Ok(Calcit::from(
      tuple.impls().iter().map(|imp| Calcit::Impl((**imp).to_owned())).collect::<Vec<_>>(),
    )),
    x => {
      let msg = format!(
        "&tuple:impls requires a tuple, but received: {}",
        type_of(&[x.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeTupleImpls).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn tuple_params(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&tuple:params expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::Tuple(CalcitTuple { extra, .. }) => {
      // Ok(Calcit::List(extra.iter().map(|x| Arc::new(x.to_owned())).collect_into(vec![])))
      let mut ys = vec![];
      for x in extra {
        ys.push(x.to_owned());
      }
      Ok(Calcit::from(ys))
    }
    x => {
      let msg = format!(
        "&tuple:params requires a tuple, but received: {}",
        type_of(&[x.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeTupleParams).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

fn collect_impl_records_for_value(value: &Calcit, call_stack: &CallStackList) -> Result<Vec<Arc<CalcitImpl>>, CalcitErr> {
  match value {
    Calcit::Tuple(tuple) => Ok(tuple.impls().to_owned()),
    Calcit::Record(record) => Ok(record.struct_ref.impls.to_owned()),
    Calcit::List(..) => {
      let impls_value = runner::evaluate_symbol_from_program("&core-list-impls", calcit::CORE_NS, None, call_stack)?;
      collect_impl_records_from_value(&impls_value, call_stack)
    }
    Calcit::Map(..) => {
      let impls_value = runner::evaluate_symbol_from_program("&core-map-impls", calcit::CORE_NS, None, call_stack)?;
      collect_impl_records_from_value(&impls_value, call_stack)
    }
    Calcit::Number(..) => {
      let impls_value = runner::evaluate_symbol_from_program("&core-number-impls", calcit::CORE_NS, None, call_stack)?;
      collect_impl_records_from_value(&impls_value, call_stack)
    }
    Calcit::Str(..) => {
      let impls_value = runner::evaluate_symbol_from_program("&core-string-impls", calcit::CORE_NS, None, call_stack)?;
      collect_impl_records_from_value(&impls_value, call_stack)
    }
    Calcit::Set(..) => {
      let impls_value = runner::evaluate_symbol_from_program("&core-set-impls", calcit::CORE_NS, None, call_stack)?;
      collect_impl_records_from_value(&impls_value, call_stack)
    }
    Calcit::Fn { .. } | Calcit::Proc(..) => {
      let impls_value = runner::evaluate_symbol_from_program("&core-fn-impls", calcit::CORE_NS, None, call_stack)?;
      collect_impl_records_from_value(&impls_value, call_stack)
    }
    other => Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Type,
      format!("&assert-traits cannot resolve impls for: {other}"),
      call_stack,
      other.get_location(),
    )),
  }
}

fn iter_impls_in_precedence_order<'a>(
  value: &'a Calcit,
  impls: &'a [Arc<CalcitImpl>],
) -> Box<dyn Iterator<Item = &'a Arc<CalcitImpl>> + 'a> {
  match value {
    // user values are last-wins, so higher precedence is later entries
    Calcit::Tuple(..) | Calcit::Record(..) => Box::new(impls.iter().rev()),
    // builtin core impl lists are first-wins and order-sensitive
    _ => Box::new(impls.iter()),
  }
}

fn collect_method_names(value: &Calcit, impls: &[Arc<CalcitImpl>]) -> Vec<String> {
  let mut seen: HashMap<String, ()> = HashMap::new();
  let mut methods: Vec<String> = Vec::new();

  for imp in iter_impls_in_precedence_order(value, impls) {
    for field in imp.fields().iter() {
      let name = format!(".{}", field.ref_str());
      if !seen.contains_key(&name) {
        seen.insert(name.clone(), ());
        methods.push(name);
      }
    }
  }

  methods
}

fn trait_method_names(trait_def: &CalcitTrait) -> String {
  trait_def
    .methods
    .iter()
    .map(|x| format!(":{}", x.ref_str()))
    .collect::<Vec<_>>()
    .join(" ")
}

/// Explicit trait method call that bypasses `.method` dispatch.
/// Usage: (&trait-call Trait :method receiver & args)
///
/// Notes:
/// - It selects the impl record by matching `impl.origin` with the target trait.
/// - It still applies the same precedence rule as `.method` within the impl list.
pub fn trait_call(xs: &[Calcit], call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.len() < 3 {
    return CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "&trait-call expected 3+ arguments (trait, method, receiver, & args), but received:",
      xs,
    );
  }

  let trait_def = match &xs[0] {
    Calcit::Trait(trait_def) => trait_def.to_owned(),
    other => {
      return CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&trait-call expected a trait definition as first argument, but received: {other}"),
      );
    }
  };

  let method_name = match &xs[1] {
    Calcit::Tag(tag) => tag.ref_str().to_string(),
    Calcit::Symbol { sym, .. } => sym.as_ref().to_string(),
    Calcit::Str(s) => s.as_ref().to_string(),
    other => {
      return CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&trait-call expected method name as tag/symbol/string, but received: {other}"),
      );
    }
  };

  if !trait_def.has_method(&method_name) {
    return Err(CalcitErr::use_msg_stack(
      CalcitErrKind::Type,
      format!(
        "&trait-call: trait {} does not define method :{}. Available methods: {}",
        trait_def.name,
        method_name,
        trait_method_names(&trait_def)
      ),
      call_stack,
    ));
  }

  let receiver = &xs[2];
  let impls = collect_impl_records_for_value(receiver, call_stack)?;

  let mut selected_impl: Option<&Arc<CalcitImpl>> = None;
  for imp in iter_impls_in_precedence_order(receiver, &impls) {
    if imp.origin().is_some_and(|origin| origin.as_ref() == &trait_def) {
      selected_impl = Some(imp);
      break;
    }
  }

  let mut method_args: Vec<Calcit> = Vec::with_capacity(xs.len().saturating_sub(2));
  method_args.push(receiver.to_owned());
  method_args.extend_from_slice(&xs[3..]);

  if let Some(impl_record) = selected_impl {
    return method_record(impl_record.as_ref(), receiver, &method_name, &method_args, call_stack);
  }

  if let Some(default_impl) = trait_def.get_default(&method_name) {
    return runner::run_fn(&method_args, default_impl, call_stack);
  }

  Err(CalcitErr::use_msg_stack_location(
    CalcitErrKind::Type,
    format!(
      "&trait-call: cannot find impl for trait {} on {receiver}. Hint: use `defimpl` to create impls tagged by trait.",
      trait_def.name
    ),
    call_stack,
    receiver.get_location(),
  ))
}

/// Returns a list of method names (with leading dot) that can be invoked on a value at runtime.
/// Usage: (&methods-of value)
pub fn methods_of(xs: &[Calcit], call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&methods-of expected 1 argument, but received:", xs);
  }

  let value = &xs[0];
  let impls = collect_impl_records_for_value(value, call_stack)?;
  let methods = collect_method_names(value, &impls);
  Ok(Calcit::from(methods.into_iter().map(|s| Calcit::Str(s.into())).collect::<Vec<_>>()))
}

/// Inspect and print method information for debugging.
/// Usage: (&inspect-methods value "optional note")
/// Returns the value unchanged while printing method information to stderr.
pub fn inspect_methods(xs: &[Calcit], call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.is_empty() || xs.len() > 2 {
    return CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "&inspect-methods expected 1 or 2 arguments (value, optional note), but received:",
      xs,
    );
  }

  let value = &xs[0];
  let note = if xs.len() == 2 {
    match &xs[1] {
      Calcit::Str(s) => s.as_ref(),
      _ => "(non-string note)",
    }
  } else {
    ""
  };

  let impls = collect_impl_records_for_value(value, call_stack)?;
  let methods = collect_method_names(value, &impls);

  eprintln!("\n&inspect-methods");
  if !note.is_empty() {
    eprintln!("Note: {note}");
  }
  eprintln!("Value type: {}", type_of(&[value.clone()])?);
  eprintln!("Value: {value}");
  eprintln!("Method call syntax: `.method self p1 p2`");
  eprintln!("  - dot is part of the method name, first arg is the receiver");

  eprintln!("\nImpl records (high → low precedence): {}", impls.len());
  for (idx, imp) in iter_impls_in_precedence_order(value, &impls).enumerate() {
    let mut method_keys = imp.fields().iter().map(|x| format!(".{}", x.ref_str())).collect::<Vec<_>>();
    method_keys.sort();
    let origin_label = imp.trait_name().unwrap_or_else(|| imp.name());
    eprintln!("  #{idx}: {}  ({})", origin_label, method_keys.join(" "));
  }

  eprintln!("\nAll methods (unique, high → low): {}", methods.len());
  eprintln!("  {}", methods.join(" "));
  eprintln!();

  Ok(value.to_owned())
}

pub fn assert_traits(xs: &[Calcit], call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&assert-traits expected 2 arguments, but received:", xs);
  }

  let value = &xs[0];
  let trait_def = match &xs[1] {
    Calcit::Trait(trait_def) => trait_def.to_owned(),
    other => {
      return CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&assert-traits expected a trait definition, but received: {other}"),
      );
    }
  };

  let impls = collect_impl_records_for_value(value, call_stack)?;
  let mut missing: Vec<String> = Vec::new();
  for method in trait_def.methods.iter() {
    let method_name = method.ref_str();
    let exists = impls.iter().any(|imp| imp.get(method_name).is_some());
    if !exists {
      missing.push(method.to_string());
    }
  }

  if !missing.is_empty() {
    let mut fields: Vec<String> = vec![];
    for imp in impls.iter() {
      for field in imp.fields().iter() {
        fields.push(field.to_string());
      }
    }
    let available = fields.join(" ");
    let missing_list = missing.join(" ");
    return Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Type,
      format!("assert-traits failed: {value} does not implement {trait_def}. Missing: {missing_list}. Available: {available}"),
      call_stack,
      value.get_location(),
    ));
  }

  Ok(value.to_owned())
}

#[allow(dead_code)]
pub fn register_calcit_builtin_impls(_xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  // JS runtime uses this to register builtin impls. Native runtime treats it as a no-op.
  Ok(Calcit::Nil)
}

pub fn no_op() -> Result<Calcit, CalcitErr> {
  Ok(Calcit::Nil)
}

pub fn get_os(_xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  // https://doc.rust-lang.org/std/env/consts/constant.OS.html
  Ok(Calcit::tag(std::env::consts::OS))
}

pub fn async_sleep(xs: Vec<Calcit>, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  use std::{thread, time};
  let sec = if xs.is_empty() {
    1.0
  } else if let Calcit::Number(n) = xs[0] {
    n
  } else {
    return Err(CalcitErr::use_msg_stack(
      CalcitErrKind::Type,
      "async-sleep expected a number, but received an invalid type",
      call_stack,
    ));
  };

  runner::track::track_task_add();

  let _handle = thread::spawn(move || {
    let ten_secs = time::Duration::from_secs(sec.round() as u64);
    // let _now = time::Instant::now();
    thread::sleep(ten_secs);

    runner::track::track_task_release();
  });

  // handle.join();

  Ok(Calcit::Nil)
}

pub fn format_ternary_tree(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&format-ternary-tree expected 1 argument, but received:", xs);
  }

  match &xs[0] {
    Calcit::List(ys) => match &**ys {
      CalcitList::List(ys) => Ok(Calcit::Str(ys.format_inline().into())),
      a => CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("&format-ternary-tree expected a list, but received a vector: {a}"),
      ),
    },
    a => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&format-ternary-tree expected a list, but received: {a}"),
    ),
  }
}

pub fn buffer(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.is_empty() {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&buffer expected hex values, but received none:", xs);
  }
  let mut buf: Vec<u8> = Vec::new();
  for x in xs {
    match x {
      Calcit::Number(n) => {
        let n = n.round() as u8;
        buf.push(n);
      }
      Calcit::Str(y) => {
        if y.len() == 2 {
          match hex::decode(&**y) {
            Ok(b) => {
              if b.len() == 1 {
                buf.push(b[0])
              } else {
                return CalcitErr::err_str(
                  CalcitErrKind::Type,
                  format!("&buffer hex for buffer might be too large, but received: {b:?}"),
                );
              }
            }
            Err(e) => {
              return CalcitErr::err_str(
                CalcitErrKind::Type,
                format!("&buffer expected a length 2 hex string, but received: {y} {e}"),
              );
            }
          }
        } else {
          return CalcitErr::err_str(
            CalcitErrKind::Type,
            format!("&buffer expected a length 2 hex string, but received: {y}"),
          );
        }
      }
      _ => return CalcitErr::err_str(CalcitErrKind::Type, format!("&buffer expected a hex string, but received: {x}")),
    }
  }
  Ok(Calcit::Buffer(buf))
}

pub fn hash(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&hash expected 1 argument, but received:", xs);
  }

  let mut s = DefaultHasher::new();
  xs[0].hash(&mut s);
  Ok(Calcit::Number(s.finish() as f64))
}

/// extract out calcit internal meta code
pub fn extract_code_into_edn(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "&extract-code-into-edn expected 1 argument, but received:",
      xs,
    );
  }
  Ok(edn_to_calcit(&dump_code(&xs[0]), &Calcit::Nil))
}

/// turns data back into code in generating js
pub fn data_to_code(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&data-to-code expected 1 argument, but received:", xs);
  }

  match data_to_calcit(&xs[0], GEN_NS, GENERATED_DEF) {
    Ok(v) => Ok(v),
    Err(e) => CalcitErr::err_str(CalcitErrKind::Syntax, format!("&data-to-code failed: {e}")),
  }
}

/// util function to read CirruQuote, only used in list
pub fn cirru_nth(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&cirru-nth expected 2 arguments, but received:", xs);
  }
  match (&xs[0], &xs[1]) {
    (Calcit::CirruQuote(code), Calcit::Number(n)) => match f64_to_usize(*n) {
      Ok(idx) => match code {
        Cirru::List(xs) => match xs.get(idx) {
          Some(v) => Ok(Calcit::CirruQuote(v.to_owned())),
          None => CalcitErr::err_str(CalcitErrKind::Arity, format!("&cirru-nth index out of range: {idx}")),
        },
        Cirru::Leaf(xs) => CalcitErr::err_str(CalcitErrKind::Type, format!("&cirru-nth does not work on leaf: {xs}")),
      },
      Err(e) => CalcitErr::err_str(CalcitErrKind::Type, format!("&cirru-nth expected a valid index, {e}")),
    },
    (Calcit::CirruQuote(_c), x) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&cirru-nth expected a number for index, but received: {x}"),
    ),
    (x, _y) => CalcitErr::err_str(CalcitErrKind::Type, format!("&cirru-nth expected a Cirru quote, but received: {x}")),
  }
}

pub fn cirru_type(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&cirru-type expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::CirruQuote(code) => match code {
      Cirru::List(_) => Ok(Calcit::Tag("list".into())),
      Cirru::Leaf(_) => Ok(Calcit::Tag("leaf".into())),
    },
    a => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&cirru-type expected a Cirru quote, but received: {a}"),
    ),
  }
}

pub fn is_spreading_mark(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "is-spreading-mark? expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::Syntax(CalcitSyntax::ArgSpread, _) => Ok(Calcit::Bool(true)),
    _ => Ok(Calcit::Bool(false)),
  }
}
