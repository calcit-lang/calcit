use crate::calcit::type_annotation::{collect_runtime_type_bindings, validate_runtime_generic_where_bounds};
use crate::{
  builtins,
  calcit::{
    self, Calcit, CalcitEnumDef, CalcitEnumValue, CalcitErr, CalcitErrKind, CalcitImpl, CalcitImport, CalcitList, CalcitLocal,
    CalcitProc, CalcitStructDef, CalcitStructValue, CalcitSymbolInfo, CalcitSyntax, CalcitTrait, CalcitTypeAnnotation, GEN_NS,
    GENERATED_DEF, brief_type_of_value, format_proc_examples_hint, gen_core_id, register_type_slot, value_matches_type_annotation,
  },
  call_stack::{self, CallStackList},
  codegen::gen_ir::dump_code,
  data::{
    cirru::{self, cirru_to_calcit},
    data_to_calcit,
    edn::{self, edn_to_calcit},
  },
  program, runner, snapshot,
  util::number::f64_to_usize,
  util::string::extract_ns_def,
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

// Tracks the current top-level def being preprocessed, keyed as "ns/def".
// This makes gensym counters per-definition rather than per-namespace,
// ensuring stable gensym numbers regardless of preprocessing order.
thread_local! {
  pub(crate) static CURRENT_COMPILING_DEF: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Runs `f` with the current-compiling-def context set to `ns/def`.
/// Restores the previous context on return (supports reentrant calls).
/// Also resets the gensym counter for this def so gensym sequences are always stable.
pub fn with_compiling_def<R, E>(ns: &str, def: &str, f: impl FnOnce() -> Result<R, E>) -> Result<R, E> {
  let prev = CURRENT_COMPILING_DEF.with(|cell| cell.borrow_mut().take());
  let key = format!("{ns}/{def}");
  CURRENT_COMPILING_DEF.with(|cell| *cell.borrow_mut() = Some(key.clone()));
  // Reset counter so gensym numbers within this def always start at 1.
  NS_SYMBOL_DICT
    .lock()
    .expect("reset gensym counter")
    .remove(&Arc::from(key.as_str()));
  let result = f();
  CURRENT_COMPILING_DEF.with(|cell| *cell.borrow_mut() = prev);
  result
}

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
    Enum { .. } => Ok(Calcit::tag("enum")),
    Buffer(..) => Ok(Calcit::tag("buffer")),
    BufList(..) => Ok(Calcit::tag("buf-list")),
    CirruQuote(..) => Ok(Calcit::tag("cirru-quote")),
    Recur(..) => Ok(Calcit::tag("recur")),
    List(..) => Ok(Calcit::tag("list")),
    Set(..) => Ok(Calcit::tag("set")),
    Map(..) => Ok(Calcit::tag("map")),
    Struct { .. } => Ok(Calcit::tag("struct")),
    StructDef { .. } => Ok(Calcit::tag("struct-def")),
    EnumDef { .. } => Ok(Calcit::tag("enum-def")),
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
        type_of(std::slice::from_ref(a))?.lisp_str()
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
        type_of(std::slice::from_ref(a))?.lisp_str()
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
        type_of(std::slice::from_ref(a))?.lisp_str()
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
    Some(a) => {
      let raw = edn::calcit_to_edn(a)?;
      Ok(Calcit::Str(cirru_edn::format(&edn::sanitize_edn_for_format(&raw), true)?.into()))
    }
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
        type_of(std::slice::from_ref(a))?.lisp_str()
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
      let msg = format!(
        "turn-symbol cannot convert to symbol: {}",
        type_of(std::slice::from_ref(a))?.lisp_str()
      );
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
      let msg = format!("turn-tag cannot convert to tag: {}", type_of(std::slice::from_ref(a))?.lisp_str());
      let hint = format_proc_examples_hint(&CalcitProc::TurnTag).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn new_enum_value(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.is_empty() {
    let msg = format!(
      "anonymous enum requires at least 1 argument (tag), but received: {} arguments",
      xs.len()
    );
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
    Ok(Calcit::Enum(CalcitEnumValue {
      tag: Arc::new(xs[0].to_owned()),
      extra,
      sum_type: None,
    }))
  }
}

pub fn new_named_enum_value(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() < 2 {
    CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("%:: expected at least 2 arguments, but received: {}", CalcitList::from(xs)),
    )
  } else {
    let enum_value = xs[0].to_owned();
    match enum_value {
      Calcit::Struct(enum_struct) => {
        let enum_proto = match CalcitEnumDef::from_struct(enum_struct.clone()) {
          Ok(proto) => proto,
          Err(msg) => {
            return CalcitErr::err_str(CalcitErrKind::Type, format!("%:: expected a valid enum prototype, but {msg}"));
          }
        };

        let tag_value = &xs[1];
        let tag_name = match tag_value {
          Calcit::Tag(t) => t.ref_str(),
          other => {
            let msg = format!(
              "%:: requires a tag, but received: {}",
              type_of(std::slice::from_ref(other))?.lisp_str()
            );
            let hint = format_proc_examples_hint(&CalcitProc::NativeNamedEnumNew).unwrap_or_default();
            return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
          }
        };

        match enum_proto.find_variant_by_name(tag_name) {
          Some(variant) => {
            let payload_count = xs.len() - 2;
            let expected_arity = variant.arity();
            let mut bindings = std::collections::HashMap::new();
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
              collect_runtime_type_bindings(payload, expected_type.as_ref(), &mut bindings);
            }
            if let Err(msg) = validate_runtime_generic_where_bounds(&bindings, enum_proto.where_bounds()) {
              return CalcitErr::err_str(
                CalcitErrKind::Type,
                format!("%:: failed generic where-bound validation for enum `{}`: {msg}", enum_proto.name()),
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
        Ok(Calcit::Enum(CalcitEnumValue {
          tag: Arc::new(xs[1].to_owned()),
          extra,
          sum_type: Some(Arc::new(enum_proto)),
        }))
      }
      Calcit::EnumDef(enum_def) => {
        let enum_proto = enum_def.clone();

        let tag_value = &xs[1];
        let tag_name = match tag_value {
          Calcit::Tag(t) => t.ref_str(),
          other => {
            let msg = format!(
              "%:: requires a tag, but received: {}",
              type_of(std::slice::from_ref(other))?.lisp_str()
            );
            let hint = format_proc_examples_hint(&CalcitProc::NativeNamedEnumNew).unwrap_or_default();
            return CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint);
          }
        };

        match enum_proto.find_variant_by_name(tag_name) {
          Some(variant) => {
            let payload_count = xs.len() - 2;
            let expected_arity = variant.arity();
            let mut bindings = std::collections::HashMap::new();
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
              collect_runtime_type_bindings(payload, expected_type.as_ref(), &mut bindings);
            }
            if let Err(msg) = validate_runtime_generic_where_bounds(&bindings, enum_proto.where_bounds()) {
              return CalcitErr::err_str(
                CalcitErrKind::Type,
                format!("%:: failed generic where-bound validation for enum `{}`: {msg}", enum_proto.name()),
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
        Ok(Calcit::Enum(CalcitEnumValue {
          tag: Arc::new(xs[1].to_owned()),
          extra,
          sum_type: Some(Arc::new(enum_proto)),
        }))
      }
      other => CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("%:: expected an EnumDef as prototype, but received: {other}"),
      ),
    }
  }
}

/// Get the enum definition from an enum value
pub fn enum_definition(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&enum:definition expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::Enum(enum_value) => match &enum_value.sum_type {
      Some(enum_proto) => Ok(Calcit::EnumDef((**enum_proto).clone())),
      None => Ok(Calcit::Nil),
    },
    a => {
      let msg = format!(
        "&enum:definition requires an enum value, but received: {}",
        type_of(std::slice::from_ref(a))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeEnumDefinition).unwrap_or_default();
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
      Calcit::Enum(tuple) => {
        let tag = tuple.tag.to_owned();
        let extra = tuple.extra.to_owned();
        let sum_type = tuple.sum_type.to_owned();
        Calcit::Enum(CalcitEnumValue { tag, extra, sum_type })
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
      T::List(inner) | T::Set(inner) | T::Ref(inner) | T::Variadic(inner) | T::Optional(inner) | T::JsNullish(inner) => {
        contains_dynamic(inner)
      }
      T::Map(k, v) => contains_dynamic(k) || contains_dynamic(v),
      T::Fn(info) => {
        info.arg_types.iter().any(|t| contains_dynamic(t))
          || info.rest_type.as_ref().is_some_and(|t| contains_dynamic(t))
          || contains_dynamic(info.return_type.as_ref())
      }
      T::Struct(_, args) | T::Enum(_, args) => args.iter().any(|t| contains_dynamic(t)),
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
            let context_label = format!("&trait::new:{}", name.ref_str());
            let method_type = calcit::with_type_annotation_warning_context(context_label, || {
              CalcitTypeAnnotation::parse_type_annotation_form(&type_form_value)
            });
            if matches!(method_type.as_ref(), CalcitTypeAnnotation::Dynamic) {
              return CalcitErr::err_str(
                CalcitErrKind::Type,
                format!("&trait::new does not allow Dynamic in method signatures, use :fn (DynFn) if needed: {type_form_value}"),
              );
            }
            if !matches!(method_type.as_ref(), CalcitTypeAnnotation::Fn(_) | CalcitTypeAnnotation::DynFn) {
              return CalcitErr::err_str(
                CalcitErrKind::Type,
                format!(
                  "&trait::new expects method type to be :fn or a typed fn schema like (:: :fn ({{}} (:args ...) (:return ...))), but received: {type_form_value}"
                ),
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

  Ok(Calcit::Trait(CalcitTrait::new_runtime(name, methods, method_types)))
}

fn collect_trait_impls(xs: &[Calcit], proc_name: &str) -> Result<Vec<Arc<CalcitImpl>>, CalcitErr> {
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
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&struct:impl-traits expected 2+ arguments, but received:", xs);
  }
  match &xs[0] {
    Calcit::Struct(struct_value) => {
      let mut impls = struct_value.struct_ref.impls.clone();
      impls.extend(collect_trait_impls(&xs[1..], "&struct:impl-traits")?);
      let mut next_struct = (*struct_value.struct_ref).clone();
      next_struct.impls = impls;

      Ok(Calcit::Struct(CalcitStructValue {
        struct_ref: Arc::new(next_struct),
        values: struct_value.values.to_owned(),
      }))
    }
    other => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&struct:impl-traits expected a struct value, but received: {other}"),
    ),
  }
}

pub fn tuple_impl_traits(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() < 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&enum:impl-traits expected 2+ arguments, but received:", xs);
  }
  match &xs[0] {
    Calcit::Enum(enum_value) => {
      let mut next_sum_type = match &enum_value.sum_type {
        Some(s) => (**s).clone(),
        None => {
          let tag_name = match &*enum_value.tag {
            Calcit::Tag(t) => t.to_owned(),
            _ => EdnTag::from("tag"),
          };
          let struct_value = CalcitStructValue {
            struct_ref: Arc::new(CalcitStructDef::from_fields(EdnTag::from("_"), vec![tag_name])),
            values: Arc::new(vec![Calcit::List(Arc::new(CalcitList::from(
              &vec![Calcit::tag("any"); enum_value.extra.len()][..],
            )))]),
          };
          CalcitEnumDef::from_struct(struct_value).map_err(|msg| {
            CalcitErr::use_msg_stack_location(
              CalcitErrKind::Type,
              format!("failed to create anonymous enum, {msg}"),
              &CallStackList::default(),
              enum_value.tag.get_location(),
            )
          })?
        }
      };
      next_sum_type.impls.extend(collect_trait_impls(&xs[1..], "&enum:impl-traits")?);
      Ok(Calcit::Enum(CalcitEnumValue {
        tag: enum_value.tag.to_owned(),
        extra: enum_value.extra.to_owned(),
        sum_type: Some(Arc::new(next_sum_type)),
      }))
    }
    other => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&enum:impl-traits expected an enum value, but received: {other}"),
    ),
  }
}

pub fn struct_impl_traits(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() < 2 {
    return CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "&struct-def:impl-traits expected 2+ arguments, but received:",
      xs,
    );
  }
  match &xs[0] {
    Calcit::StructDef(struct_def) => {
      let mut next = struct_def.to_owned();
      let mut next_impls = next.impls.clone();
      next_impls.extend(collect_trait_impls(&xs[1..], "&struct-def:impl-traits")?);
      next.impls = next_impls;
      Ok(Calcit::StructDef(next))
    }
    other => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&struct-def:impl-traits expected a struct definition, but received: {other}"),
    ),
  }
}

pub fn enum_impl_traits(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() < 2 {
    return CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "&enum-def:impl-traits expected 2+ arguments, but received:",
      xs,
    );
  }
  match &xs[0] {
    Calcit::EnumDef(enum_def) => {
      let mut next = enum_def.to_owned();
      next.impls.extend(collect_trait_impls(&xs[1..], "&enum-def:impl-traits")?);
      Ok(Calcit::EnumDef(next))
    }
    other => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&enum-def:impl-traits expected an enum definition, but received: {other}"),
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
        type_of(std::slice::from_ref(other))?.lisp_str()
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
        type_of(std::slice::from_ref(other))?.lisp_str()
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

fn parse_enum_struct(enum_struct: &CalcitStructValue, proc_name: &str) -> Result<CalcitEnumDef, CalcitErr> {
  match CalcitEnumDef::from_struct(enum_struct.to_owned()) {
    Ok(proto) => Ok(proto),
    Err(msg) => Err(CalcitErr::use_str(
      CalcitErrKind::Type,
      format!("{proc_name} expected a valid EnumDef, but {msg}"),
    )),
  }
}

/// Check if an enum has a variant
pub fn enum_def_has_variant(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "&enum-def:has-variant? expected 2 arguments, but received:",
      xs,
    );
  }
  match (&xs[0], &xs[1]) {
    (Calcit::Struct(enum_struct), Calcit::Tag(tag)) => {
      let enum_proto = parse_enum_struct(enum_struct, "&enum-def:has-variant?")?;
      Ok(Calcit::Bool(enum_proto.find_variant(tag).is_some()))
    }
    (Calcit::EnumDef(enum_def), Calcit::Tag(tag)) => Ok(Calcit::Bool(enum_def.find_variant(tag).is_some())),
    (Calcit::Struct(_) | Calcit::EnumDef(_), other) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&enum-def:has-variant? expected a tag as second argument, but received: {other}"),
    ),
    (other, _) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&enum-def:has-variant? expected an enum as first argument, but received: {other}"),
    ),
  }
}

/// Get the arity of a variant in an enum
pub fn enum_def_variant_arity(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "&enum-def:variant-arity expected 2 arguments, but received:",
      xs,
    );
  }
  match (&xs[0], &xs[1]) {
    (Calcit::Struct(enum_struct), Calcit::Tag(tag)) => {
      let enum_proto = parse_enum_struct(enum_struct, "&enum-def:variant-arity")?;
      match enum_proto.find_variant(tag) {
        Some(variant) => Ok(Calcit::Number(variant.arity() as f64)),
        None => CalcitErr::err_str(
          CalcitErrKind::Type,
          format!(
            "&enum-def:variant-arity: enum `{}` does not have variant `{}`",
            enum_proto.name(),
            tag
          ),
        ),
      }
    }
    (Calcit::EnumDef(enum_def), Calcit::Tag(tag)) => match enum_def.find_variant(tag) {
      Some(variant) => Ok(Calcit::Number(variant.arity() as f64)),
      None => CalcitErr::err_str(
        CalcitErrKind::Type,
        format!(
          "&enum-def:variant-arity: enum `{}` does not have variant `{}`",
          enum_def.name(),
          tag
        ),
      ),
    },
    (Calcit::Struct(_) | Calcit::EnumDef(_), other) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&enum-def:variant-arity expected a tag as second argument, but received: {other}"),
    ),
    (other, _) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&enum-def:variant-arity expected an enum as first argument, but received: {other}"),
    ),
  }
}

/// Validate enum tag and arity if enum metadata exists
pub fn enum_validate(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&enum:validate expected 2 arguments, but received:", xs);
  }
  match (&xs[0], &xs[1]) {
    (Calcit::Enum(enum_value), Calcit::Tag(tag)) => {
      let tuple_value = Calcit::Enum(enum_value.to_owned());
      if let Some(enum_proto) = &enum_value.sum_type {
        match enum_proto.find_variant(tag) {
          Some(variant) => {
            let expected = variant.arity();
            let actual = enum_value.extra.len();
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
    (Calcit::Enum(_), other) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&enum:validate expected a tag as second argument, but received: {other}"),
    ),
    (other, _) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&enum:validate expected an enum value as first argument, but received: {other}"),
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
    let value_type = type_of(std::slice::from_ref(v0))
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
    Enum(tuple) => {
      let user_impls = tuple.impls();
      let has_user_method = user_impls.iter().any(|imp| imp.get(name).is_some());
      if has_user_method {
        method_call_impls(user_impls, v0, name, method_args, call_stack, true)
      } else {
        let impls_value = runner::evaluate_symbol_from_program("&core-enum-impls", calcit::CORE_NS, None, call_stack)?;
        method_call(&impls_value, v0, name, method_args, call_stack)
      }
    }
    Struct(struct_value) => {
      let user_impls = &struct_value.struct_ref.impls;
      let has_user_method = user_impls.iter().any(|imp| imp.get(name).is_some());
      if has_user_method {
        method_call_impls(user_impls, v0, name, method_args, call_stack, true)
      } else {
        let impls_value = runner::evaluate_symbol_from_program("&core-struct-impls", calcit::CORE_NS, None, call_stack)?;
        method_call(&impls_value, v0, name, method_args, call_stack)
      }
    }

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

fn collect_impls_from_value(impls_value: &Calcit, call_stack: &CallStackList) -> Result<Vec<Arc<CalcitImpl>>, CalcitErr> {
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
  let impls = collect_impls_from_value(impls_value, call_stack)?;
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
        return invoke_impl_method(imp, v0, name, method_args, call_stack);
      }
    }
  } else {
    for imp in impls.iter() {
      if imp.get(name).is_some() {
        return invoke_impl_method(imp, v0, name, method_args, call_stack);
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

fn invoke_impl_method(
  impl_value: &CalcitImpl,
  v0: &Calcit,
  name: &str,
  method_args: &[Calcit],
  call_stack: &CallStackList,
) -> Result<Calcit, CalcitErr> {
  match impl_value.get(name) {
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
      let content = impl_value.fields().iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" ");
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

pub fn enum_nth(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&enum:nth expected 2 arguments, but received:", xs);
  }
  match (&xs[0], &xs[1]) {
    (Calcit::Enum(CalcitEnumValue { tag, extra, .. }), Calcit::Number(n)) => match f64_to_usize(*n) {
      Ok(0) => Ok((**tag).to_owned()) as Result<Calcit, CalcitErr>,
      Ok(m) => {
        if m - 1 < extra.len() {
          Ok(extra[m - 1].to_owned())
        } else {
          let size = extra.len() + 1;
          CalcitErr::err_str(
            CalcitErrKind::Arity,
            format!("&enum:nth index out of range. Tuple has {size} elements, but trying to index with {m}"),
          )
        }
      }
      Err(e) => CalcitErr::err_str(CalcitErrKind::Type, format!("&enum:nth expected a valid index, {e}")),
    },
    (a, b) => {
      let msg = format!(
        "&enum:nth requires an enum value and an index, but received: {} and {}",
        type_of(std::slice::from_ref(a))?.lisp_str(),
        type_of(std::slice::from_ref(b))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeEnumNth).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn assoc(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 3 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&enum:assoc expected 3 arguments, but received:", xs);
  }
  match (&xs[0], &xs[1]) {
    (Calcit::Enum(CalcitEnumValue { tag, extra, sum_type }), Calcit::Number(n)) => match f64_to_usize(*n) {
      Ok(idx) => {
        if idx == 0 {
          Ok(Calcit::Enum(CalcitEnumValue {
            tag: Arc::new(xs[2].to_owned()),
            extra: extra.to_owned(),
            sum_type: sum_type.to_owned(),
          }))
        } else if idx - 1 < extra.len() {
          let mut new_extra = extra.to_owned();
          xs[2].clone_into(&mut new_extra[idx - 1]);
          Ok(Calcit::Enum(CalcitEnumValue {
            tag: tag.to_owned(),
            extra: new_extra,
            sum_type: sum_type.to_owned(),
          }))
        } else {
          CalcitErr::err_str(
            CalcitErrKind::Arity,
            format!("&enum:assoc index out of range. Tuple only has fields at index 0, 1, but received unknown index: {idx}"),
          )
        }
      }
      Err(e) => CalcitErr::err_str(CalcitErrKind::Type, e),
    },
    (a, b, ..) => {
      let msg = format!(
        "&enum:assoc requires an enum value and an index, but received: {} and {}",
        type_of(std::slice::from_ref(a))?.lisp_str(),
        type_of(std::slice::from_ref(b))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeEnumAssoc).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn enum_count(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&enum:count expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::Enum(CalcitEnumValue { extra, .. }) => Ok(Calcit::Number((extra.len() + 1) as f64)),
    x => {
      let msg = format!(
        "&enum:count requires an enum value, but received: {}",
        type_of(std::slice::from_ref(x))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeEnumCount).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn enum_impls(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&enum:impls expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::Enum(enum_value) => Ok(Calcit::from(
      enum_value
        .impls()
        .iter()
        .map(|imp| Calcit::Impl((**imp).to_owned()))
        .collect::<Vec<_>>(),
    )),
    x => {
      let msg = format!(
        "&enum:impls requires an enum value, but received: {}",
        type_of(std::slice::from_ref(x))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeEnumImpls).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

pub fn enum_params(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&enum:params expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::Enum(CalcitEnumValue { extra, .. }) => {
      // Ok(Calcit::List(extra.iter().map(|x| Arc::new(x.to_owned())).collect_into(vec![])))
      let mut ys = vec![];
      for x in extra {
        ys.push(x.to_owned());
      }
      Ok(Calcit::from(ys))
    }
    x => {
      let msg = format!(
        "&enum:params requires an enum value, but received: {}",
        type_of(std::slice::from_ref(x))?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::NativeEnumParams).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
  }
}

fn collect_optional_core_impls(name: &str, call_stack: &CallStackList) -> Result<Vec<Arc<CalcitImpl>>, CalcitErr> {
  match runner::evaluate_symbol_from_program(name, calcit::CORE_NS, None, call_stack) {
    Ok(value) => collect_impls_from_value(&value, call_stack),
    // Unit tests and embedding users can construct a named value before the
    // core program is loaded. Attached impls must remain usable in that state.
    Err(err) if err.kind == CalcitErrKind::Var => Ok(vec![]),
    Err(err) => Err(err),
  }
}

fn collect_impls_for_value(value: &Calcit, call_stack: &CallStackList) -> Result<Vec<Arc<CalcitImpl>>, CalcitErr> {
  match value {
    Calcit::Enum(enum_value) => {
      let mut impls = collect_optional_core_impls("&core-enum-impls", call_stack)?;
      impls.extend(enum_value.impls().iter().cloned());
      Ok(impls)
    }
    Calcit::Struct(struct_value) => {
      let mut impls = collect_optional_core_impls("&core-struct-impls", call_stack)?;
      impls.extend(struct_value.struct_ref.impls.iter().cloned());
      Ok(impls)
    }
    // Bare type definitions (not yet instantiated) carry their own attached impls,
    // so introspection tools like `&methods-of` can answer "what methods will
    // instances of this type have" without needing a concrete instance first.
    Calcit::StructDef(struct_def) => {
      let mut impls = collect_optional_core_impls("&core-struct-impls", call_stack)?;
      impls.extend(struct_def.impls.iter().cloned());
      Ok(impls)
    }
    Calcit::EnumDef(enum_def) => {
      let mut impls = collect_optional_core_impls("&core-enum-impls", call_stack)?;
      impls.extend(enum_def.impls().iter().cloned());
      Ok(impls)
    }
    Calcit::List(..) => collect_optional_core_impls("&core-list-impls", call_stack),
    Calcit::Map(..) => collect_optional_core_impls("&core-map-impls", call_stack),
    Calcit::Number(..) => collect_optional_core_impls("&core-number-impls", call_stack),
    Calcit::Str(..) => collect_optional_core_impls("&core-string-impls", call_stack),
    Calcit::Set(..) => collect_optional_core_impls("&core-set-impls", call_stack),
    Calcit::Fn { .. } | Calcit::Proc(..) => collect_optional_core_impls("&core-fn-impls", call_stack),
    Calcit::Nil | Calcit::Bool(..) | Calcit::Tag(..) | Calcit::Symbol { .. } | Calcit::CirruQuote(..) => {
      collect_optional_core_impls("&core-scalar-impls", call_stack)
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
    // user values are last-wins, so higher precedence is later entries.
    // Bare struct/enum definitions attach impls the same way (via `.extend`
    // in `&struct:impl-traits`/`&enum:impl-traits`), so they share this order.
    Calcit::Enum(..) | Calcit::Struct(..) | Calcit::StructDef(..) | Calcit::EnumDef(..) => Box::new(impls.iter().rev()),
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
/// - It selects the impl struct by matching `impl.origin` with the target trait.
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
  let impls = collect_impls_for_value(receiver, call_stack)?;

  let mut selected_impl: Option<&Arc<CalcitImpl>> = None;
  for imp in iter_impls_in_precedence_order(receiver, &impls) {
    if imp.implements_trait(&trait_def) {
      selected_impl = Some(imp);
      break;
    }
  }

  let mut method_args: Vec<Calcit> = Vec::with_capacity(xs.len().saturating_sub(2));
  method_args.push(receiver.to_owned());
  method_args.extend_from_slice(&xs[3..]);

  if let Some(impl_value) = selected_impl {
    return invoke_impl_method(impl_value.as_ref(), receiver, &method_name, &method_args, call_stack);
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

/// Method names declared directly on a bare trait definition (with leading dot).
/// Traits declare methods directly rather than through attached impls, so this
/// is handled separately from `collect_impls_for_value`.
fn trait_dot_method_names(trait_def: &CalcitTrait) -> Vec<String> {
  trait_def.methods.iter().map(|m| format!(".{}", m.ref_str())).collect()
}

/// Returns a list of method names (with leading dot) that can be invoked on a value at runtime.
/// Usage: (&methods-of value)
pub fn methods_of(xs: &[Calcit], call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&methods-of expected 1 argument, but received:", xs);
  }

  let value = &xs[0];
  let methods = if let Calcit::Trait(trait_def) = value {
    trait_dot_method_names(trait_def)
  } else {
    let impls = collect_impls_for_value(value, call_stack)?;
    collect_method_names(value, &impls)
  };
  Ok(Calcit::from(
    methods
      .into_iter()
      .map(|s| {
        let name = s.strip_prefix('.').unwrap_or(&s);
        Calcit::Method(name.into(), crate::calcit::MethodKind::Invoke(crate::calcit::DYNAMIC_TYPE.clone()))
      })
      .collect::<Vec<_>>(),
  ))
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

  eprintln!("\n&inspect-methods");
  if !note.is_empty() {
    eprintln!("Note: {note}");
  }
  eprintln!("Value type: {}", type_of(std::slice::from_ref(value))?);
  eprintln!("Value: {value}");
  eprintln!("Method call syntax: `.method self p1 p2`");
  eprintln!("  - dot is part of the method name, first arg is the receiver");

  let methods = if let Calcit::Trait(trait_def) = value {
    let names = trait_dot_method_names(trait_def);
    eprintln!("\nTrait methods declared directly (no impls): {}", names.len());
    names
  } else {
    let impls = collect_impls_for_value(value, call_stack)?;
    eprintln!("\nImpl records (high → low precedence): {}", impls.len());
    for (idx, imp) in iter_impls_in_precedence_order(value, &impls).enumerate() {
      let mut method_keys = imp.fields().iter().map(|x| format!(".{}", x.ref_str())).collect::<Vec<_>>();
      method_keys.sort();
      let origin_label = imp.trait_name().unwrap_or_else(|| imp.name());
      eprintln!("  #{idx}: {}  ({})", origin_label, method_keys.join(" "));
    }
    collect_method_names(value, &impls)
  };

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

  let impls = collect_impls_for_value(value, call_stack)?;
  let selected_impl = iter_impls_in_precedence_order(value, &impls).find(|imp| imp.implements_trait(&trait_def));

  let Some(selected_impl) = selected_impl else {
    let available = impls
      .iter()
      .filter_map(|imp| imp.trait_name())
      .map(ToString::to_string)
      .collect::<Vec<_>>()
      .join(" ");
    return Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Type,
      format!(
        "assert-traits failed: {value} does not nominally implement {trait_def}. Available trait impls: {}",
        if available.is_empty() { "(none)" } else { &available }
      ),
      call_stack,
      value.get_location(),
    ));
  };

  let missing = trait_def
    .methods
    .iter()
    .filter(|method| selected_impl.get(method.ref_str()).is_none())
    .map(ToString::to_string)
    .collect::<Vec<_>>();
  if !missing.is_empty() {
    return Err(CalcitErr::use_msg_stack_location(
      CalcitErrKind::Type,
      format!(
        "assert-traits failed: impl {} for trait {} is incomplete. Missing: {}",
        selected_impl.name(),
        trait_def.name,
        missing.join(" ")
      ),
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

fn parse_ns_def_arg(value: &Calcit) -> Result<(String, String), CalcitErr> {
  let text = match value {
    Calcit::Str(s) => s.as_ref().trim_start_matches('|').to_string(),
    Calcit::Symbol { sym, .. } => sym.to_string(),
    other => {
      let msg = format!(
        "expected ns/def string or symbol, but received: {}",
        type_of(std::slice::from_ref(other))?.lisp_str()
      );
      return Err(CalcitErr::use_str(CalcitErrKind::Type, msg));
    }
  };
  extract_ns_def(&text).map_err(|e| CalcitErr::use_str(CalcitErrKind::Syntax, e))
}

/// Lookup `:doc` metadata for a definition in `ns/def` form.
pub fn get_def_doc(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&get-def-doc expected 1 argument, but received:", xs);
  }
  let (ns, def) = parse_ns_def_arg(&xs[0])?;
  let doc = program::lookup_def_doc(&ns, &def).unwrap_or_default();
  Ok(Calcit::Str(doc.into()))
}

/// Lookup `:schema` metadata for a definition in `ns/def` form, returned as EDN data.
pub fn get_def_schema(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&get-def-schema expected 1 argument, but received:", xs);
  }
  let (ns, def) = parse_ns_def_arg(&xs[0])?;
  if !program::has_def_code(&ns, &def) {
    return CalcitErr::err_str(CalcitErrKind::Var, format!("definition not found: {ns}/{def}"));
  }
  let schema = program::lookup_def_schema(&ns, &def);
  let edn = snapshot::schema_annotation_to_edn(schema.as_ref());
  Ok(edn::edn_to_calcit(&edn, &Calcit::Nil))
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

pub fn list_question(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "list? expected 1 argument, but received:", xs);
  }
  Ok(Calcit::Bool(matches!(&xs[0], Calcit::List(_))))
}

pub fn tag_question(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "tag? expected 1 argument, but received:", xs);
  }
  Ok(Calcit::Bool(matches!(&xs[0], Calcit::Tag(_))))
}

pub fn symbol_question(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "symbol? expected 1 argument, but received:", xs);
  }
  Ok(Calcit::Bool(matches!(&xs[0], Calcit::Symbol { .. })))
}

pub fn nil_question(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "nil? expected 1 argument, but received:", xs);
  }
  Ok(Calcit::Bool(matches!(&xs[0], Calcit::Nil)))
}

pub fn string_question(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "string? expected 1 argument, but received:", xs);
  }
  Ok(Calcit::Bool(matches!(&xs[0], Calcit::Str(_))))
}

pub fn map_question(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "map? expected 1 argument, but received:", xs);
  }
  Ok(Calcit::Bool(matches!(&xs[0], Calcit::Map(_))))
}

pub fn number_question(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "number? expected 1 argument, but received:", xs);
  }
  Ok(Calcit::Bool(matches!(&xs[0], Calcit::Number(_))))
}

pub fn bool_question(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "bool? expected 1 argument, but received:", xs);
  }
  Ok(Calcit::Bool(matches!(&xs[0], Calcit::Bool(_))))
}

pub fn set_question(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "set? expected 1 argument, but received:", xs);
  }
  Ok(Calcit::Bool(matches!(&xs[0], Calcit::Set(_))))
}

pub fn enum_question(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "enum? expected 1 argument, but received:", xs);
  }
  if matches!(&xs[0], Calcit::EnumDef(_)) {
    return CalcitErr::err_str(
      CalcitErrKind::Type,
      "`enum?` now checks enum values; use `enum-def?` for a value produced by `defenum`",
    );
  }
  Ok(Calcit::Bool(matches!(&xs[0], Calcit::Enum { .. })))
}

pub fn struct_question(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "struct? expected 1 argument, but received:", xs);
  }
  if matches!(&xs[0], Calcit::StructDef(_)) {
    return CalcitErr::err_str(
      CalcitErrKind::Type,
      "`struct?` now checks struct values; use `struct-def?` for a value produced by `defstruct`",
    );
  }
  Ok(Calcit::Bool(matches!(&xs[0], Calcit::Struct { .. })))
}

pub fn fn_question(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "fn? expected 1 argument, but received:", xs);
  }
  Ok(Calcit::Bool(matches!(&xs[0], Calcit::Fn { .. } | Calcit::Proc(_))))
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

/// `deftype-slot` proc: declare a named type slot for late binding.
/// Usage: `(deftype-slot :dispatch-op)`
pub fn deftype_slot(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(
      CalcitErrKind::Arity,
      "deftype-slot expected 1 argument (tag name), but received:",
      xs,
    );
  }
  let name: Arc<str> = match &xs[0] {
    Calcit::Tag(t) => Arc::from(t.ref_str()),
    Calcit::Str(s) => Arc::from(s.as_ref()),
    a => {
      return CalcitErr::err_str(CalcitErrKind::Type, format!("deftype-slot expected a tag or string name, got: {a}"));
    }
  };
  register_type_slot(name).map_err(|e| CalcitErr::use_str(CalcitErrKind::Unexpected, e))?;
  Ok(Calcit::Nil)
}

/// `with-type-slot` runtime stub: type binding is handled entirely at preprocess time.
/// At runtime the body has already been evaluated by the interpreter; this proc is a no-op.
pub fn with_type_slot_runtime(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  // Return the last body value, or nil if the call somehow reaches here with no args.
  Ok(xs.last().cloned().unwrap_or(Calcit::Nil))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::{CalcitGenericBound, CalcitSymbolInfo};

  fn symbol(name: &str) -> Calcit {
    Calcit::Symbol {
      sym: Arc::from(name),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("test.meta"),
        at_def: Arc::from("enum-where-tests"),
      }),
      location: None,
    }
  }

  fn shown_maybe_enum(show_trait: Arc<CalcitTrait>) -> CalcitEnumDef {
    CalcitEnumDef::from_struct(CalcitStructValue {
      struct_ref: Arc::new(CalcitStructDef {
        name: EdnTag::new("ShownMaybe"),
        fields: Arc::new(vec![EdnTag::new("none"), EdnTag::new("some")]),
        field_types: Arc::new(vec![crate::calcit::DYNAMIC_TYPE.clone(); 2]),
        generics: Arc::new(vec![Arc::from("T")]),
        where_bounds: Arc::new(vec![CalcitGenericBound {
          name: Arc::from("T"),
          traits: Arc::new(vec![show_trait]),
        }]),
        impls: vec![],
      }),
      values: Arc::new(vec![
        Calcit::List(Arc::new(CalcitList::Vector(vec![]))),
        Calcit::List(Arc::new(CalcitList::Vector(vec![symbol("T")]))),
      ]),
    })
    .expect("valid enum")
  }

  fn value_with_trait(trait_def: Arc<CalcitTrait>) -> Calcit {
    let mut struct_def = CalcitStructDef::from_fields(EdnTag::new("ShownValue"), vec![]);
    struct_def.impls.push(Arc::new(CalcitImpl {
      name: trait_def.name.clone(),
      origin: Some(trait_def),
      fields: Arc::new(vec![]),
      values: Arc::new(vec![]),
    }));
    Calcit::Struct(CalcitStructValue {
      struct_ref: Arc::new(struct_def),
      values: Arc::new(vec![]),
    })
  }

  #[test]
  fn generic_enum_where_bounds_accept_nominal_trait_values() {
    let show_trait = Arc::new(CalcitTrait::new(EdnTag::new("Renderable"), vec![], vec![]));
    let result = new_named_enum_value(&[
      Calcit::EnumDef(shown_maybe_enum(show_trait.clone())),
      Calcit::tag("some"),
      value_with_trait(show_trait),
    ]);

    assert!(result.is_ok(), "expected shown maybe creation to pass: {result:?}");
  }

  #[test]
  fn generic_enum_where_bounds_reject_missing_nominal_trait() {
    let show_trait = Arc::new(CalcitTrait::new(EdnTag::new("Renderable"), vec![], vec![]));
    let err = new_named_enum_value(&[
      Calcit::EnumDef(shown_maybe_enum(show_trait)),
      Calcit::tag("some"),
      Calcit::Proc(CalcitProc::NativeResetGenSymIndex),
    ])
    .expect_err("expected shown maybe creation to fail on non-Show payload");

    assert!(
      err.msg.contains("does not satisfy `trait Renderable`") || err.msg.contains("does not satisfy `Renderable`"),
      "unexpected error: {err:?}"
    );
  }

  #[test]
  fn optional_core_impl_lookup_allows_embedding_without_core_entries() {
    let impls = collect_optional_core_impls("&missing-test-core-impls", &CallStackList::default())
      .expect("missing core impl list should be treated as unavailable");
    assert!(impls.is_empty());
  }
}
