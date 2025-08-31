use crate::{
  builtins,
  calcit::{
    self, Calcit, CalcitErr, CalcitErrKind, CalcitImport, CalcitList, CalcitLocal, CalcitRecord, CalcitSymbolInfo, CalcitSyntax,
    CalcitTuple, GEN_NS, GENERATED_DEF, gen_core_id,
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

use cirru_parser::{Cirru, CirruWriterOptions};

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
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "type-of expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::Nil => Ok(Calcit::tag("nil")),
    Calcit::Bool(..) => Ok(Calcit::tag("bool")),
    Calcit::Number(..) => Ok(Calcit::tag("number")),
    Calcit::Symbol { .. } => Ok(Calcit::tag("symbol")),
    Calcit::Tag(..) => Ok(Calcit::tag("tag")),
    Calcit::Str(..) => Ok(Calcit::tag("string")),
    Calcit::Thunk(..) => Ok(Calcit::tag("thunk")), // internal
    Calcit::Ref(..) => Ok(Calcit::tag("ref")),
    Calcit::Tuple { .. } => Ok(Calcit::tag("tuple")),
    Calcit::Buffer(..) => Ok(Calcit::tag("buffer")),
    Calcit::CirruQuote(..) => Ok(Calcit::tag("cirru-quote")),
    Calcit::Recur(..) => Ok(Calcit::tag("recur")),
    Calcit::List(..) => Ok(Calcit::tag("list")),
    Calcit::Set(..) => Ok(Calcit::tag("set")),
    Calcit::Map(..) => Ok(Calcit::tag("map")),
    Calcit::Record { .. } => Ok(Calcit::tag("record")),
    Calcit::Proc(..) => Ok(Calcit::tag("fn")), // special kind proc, but also fn
    Calcit::Macro { .. } => Ok(Calcit::tag("macro")),
    Calcit::Fn { .. } => Ok(Calcit::tag("fn")),
    Calcit::Syntax(..) => Ok(Calcit::tag("syntax")),
    Calcit::Method(..) => Ok(Calcit::tag("method")),
    Calcit::RawCode(..) => Ok(Calcit::tag("raw-code")),
    Calcit::Local { .. } => Ok(Calcit::tag("local")),
    Calcit::Import { .. } => Ok(Calcit::tag("import")),
    Calcit::Registered(..) => Ok(Calcit::tag("registered")),
    Calcit::AnyRef(..) => Ok(Calcit::tag("any-ref")),
  }
}

pub fn recur(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  Ok(Calcit::Recur(xs.to_vec()))
}

pub fn format_to_lisp(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(v) => Ok(Calcit::Str(v.lisp_str().into())),
    None => CalcitErr::err_str(CalcitErrKind::Arity, "format-to-lisp expected 1 argument, but received none"),
  }
}

pub fn format_to_cirru(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(v) => cirru_parser::format(&[transform_code_to_cirru(v)], CirruWriterOptions { use_inline: false })
      .map(|s| Calcit::Str(s.into()))
      .map_err(|e| CalcitErr::use_str(CalcitErrKind::Syntax, e)),
    None => CalcitErr::err_str(CalcitErrKind::Arity, "format-to-cirru expected 1 argument, but received none"),
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
      Err(e) => CalcitErr::err_str(CalcitErrKind::Syntax, format!("parse-cirru-list failed: {e}")),
    },
    Some(a) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("parse-cirru-list expected a string, but received: {a}"),
    ),
    None => CalcitErr::err_str(CalcitErrKind::Arity, "parse-cirru-list expected 1 argument, but received none"),
  }
}

/// it returns a piece of quoted Cirru data, rather than a list
pub fn parse_cirru(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Str(s)) => match cirru_parser::parse(s) {
      Ok(nodes) => Ok(Calcit::CirruQuote(Cirru::List(nodes))),
      Err(e) => CalcitErr::err_str(CalcitErrKind::Syntax, format!("parse-cirru failed: {e}")),
    },
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("parse-cirru expected a string, but received: {a}")),
    None => CalcitErr::err_str(CalcitErrKind::Arity, "parse-cirru expected 1 argument, but received none"),
  }
}

pub fn format_cirru(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(a) => {
      let options = cirru_parser::CirruWriterOptions { use_inline: false };
      match cirru::calcit_data_to_cirru(a) {
        Ok(v) => {
          if let Cirru::List(ys) = v {
            Ok(Calcit::Str(cirru_parser::format(&ys, options)?.into()))
          } else {
            CalcitErr::err_str(
              CalcitErrKind::Type,
              format!("format-cirru expected a list for Cirru formatting, but received: {v}"),
            )
          }
        }
        Err(e) => CalcitErr::err_str(CalcitErrKind::Syntax, format!("format-cirru failed: {e}")),
      }
    }
    None => CalcitErr::err_str(CalcitErrKind::Arity, "format-cirru expected 1 argument, but received none"),
  }
}

pub fn parse_cirru_edn(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Str(s)) => match cirru_edn::parse(s) {
      Ok(nodes) => match xs.get(1) {
        Some(options) => Ok(edn::edn_to_calcit(&nodes, options)),
        None => Ok(edn::edn_to_calcit(&nodes, &Calcit::Nil)),
      },
      Err(e) => CalcitErr::err_str(CalcitErrKind::Syntax, format!("parse-cirru-edn failed: {e}")),
    },
    Some(a) => CalcitErr::err_str(CalcitErrKind::Type, format!("parse-cirru-edn expected a string, but received: {a}")),
    None => CalcitErr::err_str(CalcitErrKind::Arity, "parse-cirru-edn expected 1 argument, but received none"),
  }
}

pub fn format_cirru_edn(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(a) => Ok(Calcit::Str(cirru_edn::format(&edn::calcit_to_edn(a)?, true)?.into())),
    None => CalcitErr::err_str(CalcitErrKind::Arity, "format-cirru-edn expected 1 argument, but received none"),
  }
}

pub fn cirru_quote_to_list(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&cirru-quote:to-list expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::CirruQuote(ys) => Ok(cirru_to_calcit(ys)),
    a => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&cirru-quote:to-list expected a Cirru quote, but received: {a}"),
    ),
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
    a => CalcitErr::err_str(CalcitErrKind::Type, format!("turn-symbol cannot convert to symbol: {a}")),
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
    a => CalcitErr::err_str(CalcitErrKind::Type, format!("turn-tag cannot convert to tag: {a}")),
  }
}

pub fn new_tuple(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.is_empty() {
    CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("tuple expected at least 1 argument, but received: {}", CalcitList::from(xs)),
    )
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
      class: None,
    }))
  }
}

pub fn new_class_tuple(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() < 2 {
    CalcitErr::err_str(
      CalcitErrKind::Arity,
      format!("tuple expected at least 2 arguments, but received: {}", CalcitList::from(xs)),
    )
  } else {
    let class = xs[0].to_owned();
    if let Calcit::Record(record) = class {
      let extra: Vec<Calcit> = if xs.len() == 2 {
        vec![]
      } else {
        let mut ys: Vec<Calcit> = Vec::with_capacity(xs.len() - 1);
        for item in xs.iter().skip(2) {
          ys.push(item.to_owned());
        }
        ys
      };
      Ok(Calcit::Tuple(CalcitTuple {
        tag: Arc::new(xs[1].to_owned()),
        extra,
        class: Some(Arc::new(record)),
      }))
    } else {
      CalcitErr::err_str(
        CalcitErrKind::Type,
        format!("tuple expected a record as class, but received: {class}"),
      )
    }
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
  match v0 {
    Calcit::Tuple(CalcitTuple { class, .. }) => match class {
      Some(record) => method_record(record, v0, name, method_args, call_stack),
      None => Err(CalcitErr::use_msg_stack(
        CalcitErrKind::Type,
        format!("invoke-method cannot find class for: {v0}"),
        call_stack,
      )),
    },
    Calcit::Record(CalcitRecord { class, .. }) => match class {
      Some(record) => method_record(record, v0, name, method_args, call_stack),
      None => Err(CalcitErr::use_msg_stack(
        CalcitErrKind::Type,
        format!("invoke-method cannot find class for: {v0}"),
        call_stack,
      )),
    },

    // classed should already be preprocessed
    Calcit::List(..) => {
      let class = runner::evaluate_symbol_from_program("&core-list-class", calcit::CORE_NS, None, call_stack)?;
      method_call(&class, v0, name, method_args, call_stack)
    }
    Calcit::Map(..) => {
      let class = runner::evaluate_symbol_from_program("&core-map-class", calcit::CORE_NS, None, call_stack)?;
      method_call(&class, v0, name, method_args, call_stack)
    }
    Calcit::Number(..) => {
      let class = runner::evaluate_symbol_from_program("&core-number-class", calcit::CORE_NS, None, call_stack)?;
      method_call(&class, v0, name, method_args, call_stack)
    }
    Calcit::Str(..) => {
      let class = runner::evaluate_symbol_from_program("&core-string-class", calcit::CORE_NS, None, call_stack)?;
      method_call(&class, v0, name, method_args, call_stack)
    }
    Calcit::Set(..) => {
      let class = &runner::evaluate_symbol_from_program("&core-set-class", calcit::CORE_NS, None, call_stack)?;
      method_call(class, v0, name, method_args, call_stack)
    }
    Calcit::Nil => {
      let class = runner::evaluate_symbol_from_program("&core-nil-class", calcit::CORE_NS, None, call_stack)?;
      method_call(&class, v0, name, method_args, call_stack)
    }
    Calcit::Fn { .. } | Calcit::Proc(..) => {
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
      let content = class.fields.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" ");
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
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&compare expected 2 values, but received:", xs);
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
    (a, b) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&tuple:nth expected a tuple and an index, but received: {a} {b}"),
    ),
  }
}

pub fn assoc(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 3 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&tuple:assoc expected 3 arguments, but received:", xs);
  }
  match (&xs[0], &xs[1]) {
    (Calcit::Tuple(CalcitTuple { tag, extra, class }), Calcit::Number(n)) => match f64_to_usize(*n) {
      Ok(idx) => {
        if idx == 0 {
          Ok(Calcit::Tuple(CalcitTuple {
            tag: Arc::new(xs[2].to_owned()),
            extra: extra.to_owned(),
            class: class.to_owned(),
          }))
        } else if idx - 1 < extra.len() {
          let mut new_extra = extra.to_owned();
          xs[2].clone_into(&mut new_extra[idx - 1]);
          Ok(Calcit::Tuple(CalcitTuple {
            tag: tag.to_owned(),
            extra: new_extra,
            class: class.to_owned(),
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
    (a, b, ..) => CalcitErr::err_str(CalcitErrKind::Type, format!("&tuple:assoc expected a tuple, but received: {a} {b}")),
  }
}

pub fn tuple_count(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&tuple:count expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::Tuple(CalcitTuple { extra, .. }) => Ok(Calcit::Number((extra.len() + 1) as f64)),
    x => CalcitErr::err_str(CalcitErrKind::Type, format!("&tuple:count expected a tuple, but received: {x}")),
  }
}

pub fn tuple_class(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&tuple:class expected 1 argument, but received:", xs);
  }
  match &xs[0] {
    Calcit::Tuple(CalcitTuple { class, .. }) => match class {
      None => Ok(Calcit::Nil),
      Some(class) => Ok(Calcit::Record((**class).to_owned())),
    },
    x => CalcitErr::err_str(CalcitErrKind::Type, format!("&tuple:class expected a tuple, but received: {x}")),
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
    x => CalcitErr::err_str(CalcitErrKind::Type, format!("&tuple:params expected a tuple, but received: {x}")),
  }
}

pub fn tuple_with_class(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_nodes(CalcitErrKind::Arity, "&tuple:with-class expected 2 arguments, but received:", xs);
  }
  match (&xs[0], &xs[1]) {
    (Calcit::Tuple(CalcitTuple { tag, extra, .. }), Calcit::Record(record)) => Ok(Calcit::Tuple(CalcitTuple {
      tag: tag.to_owned(),
      extra: extra.to_owned(),
      class: Some(Arc::new(record.to_owned())),
    })),
    (a, Calcit::Record { .. }) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&tuple:with-class expected a tuple, but received: {a}"),
    ),
    (Calcit::Tuple { .. }, b) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&tuple:with-class expected a record for the second argument, but received: {b}"),
    ),
    (a, b) => CalcitErr::err_str(
      CalcitErrKind::Type,
      format!("&tuple:with-class expected a tuple and a record, but received: {a} {b}"),
    ),
  }
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
