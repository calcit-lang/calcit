use crate::{
  builtins,
  calcit::{
    self, Calcit, CalcitEnum, CalcitErr, CalcitErrKind, CalcitFnArgs, CalcitImport, CalcitList, CalcitLocal, CalcitProc, CalcitRecord,
    CalcitSymbolInfo, CalcitSyntax, CalcitTuple, CalcitTypeAnnotation, GEN_NS, GENERATED_DEF, format_proc_examples_hint, gen_core_id,
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
      classes: vec![],
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
          classes: vec![],
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
          classes: vec![],
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

pub fn struct_with_class(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&struct:with-class expected 2 arguments, but received:", xs);
  }

  let struct_value = xs[0].to_owned();
  let class_value = xs[1].to_owned();
  match (struct_value, class_value) {
    (Calcit::Struct(mut struct_def), Calcit::Record(class_record)) => {
      struct_def.classes = vec![Arc::new(class_record)];
      Ok(Calcit::Struct(struct_def))
    }
    (Calcit::Struct(_), other) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&struct:with-class expected a record as class, but received: {other}"),
    ),
    (other, _) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&struct:with-class expected a struct, but received: {other}"),
    ),
  }
}

pub fn enum_with_class(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&enum:with-class expected 2 arguments, but received:", xs);
  }

  let enum_value = xs[0].to_owned();
  let class_value = xs[1].to_owned();
  match (enum_value, class_value) {
    (Calcit::Record(enum_record), Calcit::Record(class_record)) => Ok(Calcit::Record(CalcitRecord {
      struct_ref: enum_record.struct_ref,
      values: enum_record.values,
      classes: vec![Arc::new(class_record)],
    })),
    (Calcit::Enum(mut enum_def), Calcit::Record(class_record)) => {
      enum_def.set_classes(vec![Arc::new(class_record)]);
      Ok(Calcit::Enum(enum_def))
    }
    (Calcit::Record(_), other) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&enum:with-class expected a record as class, but received: {other}"),
    ),
    (Calcit::Enum(_), other) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&enum:with-class expected a record as class, but received: {other}"),
    ),
    (other, _) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&enum:with-class expected an enum prototype record, but received: {other}"),
    ),
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
  use Calcit::*;
  match v0 {
    Tuple(CalcitTuple { classes, .. }) => match classes.first() {
      Some(record) => method_record(record, v0, name, method_args, call_stack),
      None => Err(CalcitErr::use_msg_stack(
        CalcitErrKind::Type,
        format!("invoke-method cannot find class for: {v0}"),
        call_stack,
      )),
    },
    Record(CalcitRecord { classes, .. }) => match classes.first() {
      Some(record) => method_record(record, v0, name, method_args, call_stack),
      None => Err(CalcitErr::use_msg_stack(
        CalcitErrKind::Type,
        format!("invoke-method cannot find class for: {v0}"),
        call_stack,
      )),
    },

    // classed should already be preprocessed
    List(..) => {
      let class = runner::evaluate_symbol_from_program("&core-list-class", calcit::CORE_NS, None, call_stack)?;
      method_call(&class, v0, name, method_args, call_stack)
    }
    Map(..) => {
      let class = runner::evaluate_symbol_from_program("&core-map-class", calcit::CORE_NS, None, call_stack)?;
      method_call(&class, v0, name, method_args, call_stack)
    }
    Number(..) => {
      let class = runner::evaluate_symbol_from_program("&core-number-class", calcit::CORE_NS, None, call_stack)?;
      method_call(&class, v0, name, method_args, call_stack)
    }
    Str(..) => {
      let class = runner::evaluate_symbol_from_program("&core-string-class", calcit::CORE_NS, None, call_stack)?;
      method_call(&class, v0, name, method_args, call_stack)
    }
    Set(..) => {
      let class = &runner::evaluate_symbol_from_program("&core-set-class", calcit::CORE_NS, None, call_stack)?;
      method_call(class, v0, name, method_args, call_stack)
    }
    Nil => {
      let class = runner::evaluate_symbol_from_program("&core-nil-class", calcit::CORE_NS, None, call_stack)?;
      method_call(&class, v0, name, method_args, call_stack)
    }
    Fn { .. } | Proc(..) => {
      let class = runner::evaluate_symbol_from_program("&core-fn-class", calcit::CORE_NS, None, call_stack)?;
      method_call(&class, v0, name, method_args, call_stack)
    }
    x => Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Type,
      format!("invoke-method cannot decide a class from: {x}"),
      call_stack,
      x.get_location(),
    )),
  }
}

fn method_call(
  class: &Calcit,
  v0: &Calcit,
  name: &str,
  method_args: &[Calcit],
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  // match &class {
  //   Some(record) => method_record(record, v0, name, method_args, call_stack),
  //   None => CalcitErr::err_str(format!("cannot find class for method invoking: {v0}")),
  // }
  match class {
    Calcit::Record(record) => method_record(record, v0, name, method_args, call_stack),
    x => Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Type,
      format!("invoke-method cannot find class for: {v0}"),
      call_stack,
      x.get_location(),
    )),
  }
}

fn method_record(
  class: &CalcitRecord,
  v0: &Calcit,
  name: &str,
  method_args: &[Calcit],
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  match class.get(name) {
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
      let content = class.fields().iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" ");
      Err(CalcitErr::use_msg_stack(
        CalcitErrKind::Type,
        format!("unknown method `.{name}` for {v0}. Available methods: {content}"),
        call_stack,
      ))
    }
  }
}

pub fn native_compare(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(a), Some(b)) => match a.cmp(b) {
      Ordering::Less => Ok(Calcit::Number(-1.0)),
      Ordering::Greater => Ok(Calcit::Number(1.0)),
      Ordering::Equal => Ok(Calcit::Number(0.0)),
    },
    _ => crate::builtins::err_arity("&compare requires 2 arguments, but received:", xs),
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
    (
      Calcit::Tuple(CalcitTuple {
        tag,
        extra,
        classes,
        sum_type,
      }),
      Calcit::Number(n),
    ) => match f64_to_usize(*n) {
      Ok(idx) => {
        if idx == 0 {
          Ok(Calcit::Tuple(CalcitTuple {
            tag: Arc::new(xs[2].to_owned()),
            extra: extra.to_owned(),
            classes: classes.clone(),
            sum_type: sum_type.to_owned(),
          }))
        } else if idx - 1 < extra.len() {
          let mut new_extra = extra.to_owned();
          xs[2].clone_into(&mut new_extra[idx - 1]);
          Ok(Calcit::Tuple(CalcitTuple {
            tag: tag.to_owned(),
            extra: new_extra,
            classes: classes.clone(),
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

pub fn tuple_class(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&tuple:class expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::Tuple(CalcitTuple { classes, .. }) => match classes.first() {
      None => Ok(Calcit::Nil),
      Some(class) => Ok(Calcit::Record((**class).to_owned())),
    },
    x => {
      let msg = format!(
        "&tuple:class requires a tuple, but received: {}",
        type_of(&[x.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeTupleClass).unwrap_or_default();
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

pub fn tuple_with_class(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&tuple:with-class expected 2 arguments, but received:", xs);
  }
  match (&xs[0], &xs[1]) {
    (Calcit::Tuple(CalcitTuple { tag, extra, sum_type, .. }), Calcit::Record(record)) => Ok(Calcit::Tuple(CalcitTuple {
      tag: tag.to_owned(),
      extra: extra.to_owned(),
      classes: vec![Arc::new(record.to_owned())],
      sum_type: sum_type.to_owned(),
    })),
    (a, Calcit::Record { .. }) => {
      let msg = format!(
        "&tuple:with-class requires a tuple, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeTupleWithClass).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    (Calcit::Tuple { .. }, b) => {
      let msg = format!(
        "&tuple:with-class requires a record for the second argument, but received: {}",
        type_of(&[b.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeTupleWithClass).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    (a, b) => {
      let msg = format!(
        "&tuple:with-class requires a tuple and a record, but received: {} and {}",
        type_of(&[a.to_owned()])?.lisp_str(),
        type_of(&[b.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeTupleWithClass).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

/// Inspect and print class methods information for debugging
/// Usage: (&inspect-class-methods value "optional note")
/// Returns the value unchanged while printing class information to stderr
pub fn inspect_class_methods(xs: &[Calcit], call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.is_empty() || xs.len() > 2 {
    return CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "&inspect-class-methods expected 1 or 2 arguments (value, optional note), but received:",
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

  // Get the class record for this value
  let class_result: Result<Calcit, CalcitErr> = match value {
    Calcit::Tuple(CalcitTuple { classes, .. }) => match classes.first() {
      Some(class) => Ok(Calcit::Record((**class).to_owned())),
      None => Ok(Calcit::Nil),
    },
    Calcit::Record(CalcitRecord { classes, .. }) => match classes.first() {
      Some(class) => Ok(Calcit::Record((**class).to_owned())),
      None => Ok(Calcit::Nil),
    },
    Calcit::List(..) => runner::evaluate_symbol_from_program("&core-list-class", calcit::CORE_NS, None, call_stack),
    Calcit::Map(..) => runner::evaluate_symbol_from_program("&core-map-class", calcit::CORE_NS, None, call_stack),
    Calcit::Number(..) => runner::evaluate_symbol_from_program("&core-number-class", calcit::CORE_NS, None, call_stack),
    Calcit::Str(..) => runner::evaluate_symbol_from_program("&core-string-class", calcit::CORE_NS, None, call_stack),
    Calcit::Set(..) => runner::evaluate_symbol_from_program("&core-set-class", calcit::CORE_NS, None, call_stack),
    Calcit::Nil => runner::evaluate_symbol_from_program("&core-nil-class", calcit::CORE_NS, None, call_stack),
    Calcit::Fn { .. } | Calcit::Proc(..) => runner::evaluate_symbol_from_program("&core-fn-class", calcit::CORE_NS, None, call_stack),
    _ => Ok(Calcit::Nil),
  };

  // Print class information
  eprintln!("\n&inspect-class-methods");
  if !note.is_empty() {
    eprintln!("Note: {note}");
  }
  eprintln!("Value type: {}", type_of(&[value.clone()])?);
  eprintln!("Value: {value}");
  eprintln!("Method call syntax: `.method self p1 p2`");
  eprintln!("  - dot is part of the method name, first arg is the receiver");

  match class_result {
    Ok(Calcit::Record(record)) => {
      eprintln!("\nClass: {} ({} methods)", record.name(), record.fields().len());
      eprintln!();

      for (i, field) in record.fields().iter().enumerate() {
        let method_value = &record.values[i];

        match method_value {
          Calcit::Fn { info, .. } => {
            // Extract argument count
            let arg_count = info.args.param_len();

            // Format arguments
            let args_str = match info.args.as_ref() {
              CalcitFnArgs::MarkedArgs(labels) => labels.iter().map(|label| format!("{label}")).collect::<Vec<_>>().join(" "),
              CalcitFnArgs::Args(indices) => indices.iter().map(|idx| CalcitLocal::read_name(*idx)).collect::<Vec<_>>().join(" "),
            };

            // Compact output format
            eprint!("  • .{field} => [fn/{arg_count}");
            if !args_str.is_empty() {
              eprint!(" ({args_str})");
            }
            eprintln!("]  @{}", info.def_ns);

            // Guidance for more info
            eprintln!("      → cr query def '{}/{}'", info.def_ns, info.name);
          }
          Calcit::Proc(proc_name) => {
            // Try to get type signature for better hints
            if let Some(sig) = proc_name.get_type_signature() {
              let arg_types = sig
                .arg_types
                .iter()
                .map(|t| {
                  if matches!(**t, CalcitTypeAnnotation::Dynamic) {
                    "_".to_string()
                  } else {
                    t.to_brief_string()
                  }
                })
                .collect::<Vec<_>>()
                .join(" ");
              let return_type = if matches!(*sig.return_type, CalcitTypeAnnotation::Dynamic) {
                "_".to_string()
              } else {
                sig.return_type.to_brief_string()
              };
              eprintln!(
                "  • .{field} => [proc/{}: {proc_name}]  ({arg_types}) -> {return_type}",
                sig.arg_types.len()
              );
            } else {
              eprintln!("  • .{field} => [proc: {proc_name}]");
            }
          }
          Calcit::Syntax(syntax_name, _) => {
            eprintln!("  • .{field} => [syntax: {syntax_name}]");
          }
          other => {
            eprintln!("  • .{field} => {other}");
          }
        }
      }

      // Check for nested class
      if let Some(parent_class) = record.classes.first() {
        eprintln!();
        eprintln!("Parent class: {} ({} methods)", parent_class.name(), parent_class.fields().len());
        eprintln!(
          "  → Inspect with: (&inspect-class-methods (&tuple:with-class (:: :{}) {}))",
          parent_class.name(),
          parent_class.name()
        );
      }
    }
    Ok(Calcit::Nil) => {
      eprintln!("\nNo class associated with this value.");
      eprintln!("  [This value type doesn't have methods]");
    }
    Ok(other) => {
      eprintln!("\nUnexpected class value: {other}");
    }
    Err(e) => {
      eprintln!("\nError retrieving class: {}", e.msg);
    }
  }

  eprintln!();

  // Return the original value unchanged
  Ok(value.clone())
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
