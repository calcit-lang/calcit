use crate::{
  builtins,
  builtins::records::find_in_fields,
  call_stack,
  call_stack::CallStackList,
  data::{cirru, edn},
  primes,
  primes::{gen_core_id, Calcit, CalcitErr, CalcitItems, CrListWrap},
  runner,
  util::number::f64_to_usize,
};

use cirru_edn::EdnKwd;
use cirru_parser::{Cirru, CirruWriterOptions};

use std::cmp::Ordering;
use std::sync::atomic::AtomicUsize;
use std::sync::{atomic, Arc};

static SYMBOL_INDEX: AtomicUsize = AtomicUsize::new(0);
static JS_SYMBOL_INDEX: AtomicUsize = AtomicUsize::new(0);

pub fn type_of(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_str(format!("type-of expected 1 argument, got: {:?}", xs));
  }
  match &xs[0] {
    Calcit::Nil => Ok(Calcit::kwd("nil")),
    // CalcitRef(Calcit), // TODO
    Calcit::Bool(..) => Ok(Calcit::kwd("bool")),
    Calcit::Number(..) => Ok(Calcit::kwd("number")),
    Calcit::Symbol { .. } => Ok(Calcit::kwd("symbol")),
    Calcit::Keyword(..) => Ok(Calcit::kwd("keyword")),
    Calcit::Str(..) => Ok(Calcit::kwd("string")),
    Calcit::Thunk(..) => Ok(Calcit::kwd("thunk")), // internal
    Calcit::Ref(..) => Ok(Calcit::kwd("ref")),
    Calcit::Tuple(..) => Ok(Calcit::kwd("tuple")),
    Calcit::Buffer(..) => Ok(Calcit::kwd("buffer")),
    Calcit::Recur(..) => Ok(Calcit::kwd("recur")),
    Calcit::List(..) => Ok(Calcit::kwd("list")),
    Calcit::Set(..) => Ok(Calcit::kwd("set")),
    Calcit::Map(..) => Ok(Calcit::kwd("map")),
    Calcit::Record(..) => Ok(Calcit::kwd("record")),
    Calcit::Proc(..) => Ok(Calcit::kwd("fn")), // special kind proc, but also fn
    Calcit::Macro { .. } => Ok(Calcit::kwd("macro")),
    Calcit::Fn { .. } => Ok(Calcit::kwd("fn")),
    Calcit::Syntax(..) => Ok(Calcit::kwd("synta")),
  }
}

pub fn recur(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  Ok(Calcit::Recur(Box::new(xs.to_owned())))
}

pub fn format_to_lisp(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(v) => Ok(Calcit::Str(v.lisp_str().into())),
    None => CalcitErr::err_str("format-to-lisp expected 1 argument"),
  }
}

pub fn format_to_cirru(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(v) => cirru_parser::format(&[transform_code_to_cirru(v)], CirruWriterOptions { use_inline: false })
      .map(|s| Calcit::Str(s.into()))
      .map_err(CalcitErr::use_str),
    None => CalcitErr::err_str("format-to-cirru expected 1 argument"),
  }
}

fn transform_code_to_cirru(x: &Calcit) -> Cirru {
  match x {
    Calcit::List(ys) => {
      let mut xs: Vec<Cirru> = Vec::with_capacity(ys.len());
      for y in &**ys {
        xs.push(transform_code_to_cirru(y));
      }
      Cirru::List(xs)
    }
    Calcit::Symbol { sym, .. } => Cirru::Leaf((**sym).into()),
    Calcit::Syntax(s, _ns) => Cirru::Leaf(s.to_string().into()),
    Calcit::Proc(s) => Cirru::Leaf((**s).into()),
    a => Cirru::leaf(format!("{}", a)),
  }
}

pub fn gensym(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  let idx = SYMBOL_INDEX.fetch_add(1, atomic::Ordering::SeqCst);
  let n = idx + 1; // use 1 as first value since previous implementation did this

  let s = if xs.is_empty() {
    let mut chunk = String::from("G__");
    chunk.push_str(&n.to_string());
    chunk
  } else {
    match &xs[0] {
      Calcit::Str(s) | Calcit::Symbol { sym: s, .. } => {
        let mut chunk = (**s).to_string();
        chunk.push('_');
        chunk.push('_');
        chunk.push_str(&n.to_string());
        chunk
      }
      Calcit::Keyword(s) => {
        let mut chunk = s.to_string();
        chunk.push('_');
        chunk.push('_');
        chunk.push_str(&n.to_string());
        chunk
      }
      a => return CalcitErr::err_str(format!("gensym expected a string, but got: {}", a)),
    }
  };
  Ok(Calcit::Symbol {
    sym: s.into(),
    ns: primes::GEN_NS.to_owned(),
    at_def: primes::GEN_DEF.to_owned(),
    resolved: None,
  })
}

pub fn reset_gensym_index(_xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  let _ = SYMBOL_INDEX.swap(0, atomic::Ordering::SeqCst);
  Ok(Calcit::Nil)
}

pub fn force_reset_gensym_index() -> Result<(), String> {
  let _ = SYMBOL_INDEX.swap(0, atomic::Ordering::SeqCst);
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

/// TODO, move out to calcit
pub fn generate_id(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  let size = match xs.get(0) {
    Some(Calcit::Number(n)) => match f64_to_usize(*n) {
      Ok(size) => Some(size),
      Err(e) => return CalcitErr::err_str(e),
    },
    Some(a) => return CalcitErr::err_str(format!("expected usize, got: {}", a)),
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
    (a, b) => CalcitErr::err_str(format!("generate-id! expected size or charset, got: {:?} {:?}", a, b)),
  }
}

pub fn display_stack(_xs: &CalcitItems, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  call_stack::show_stack(call_stack);
  Ok(Calcit::Nil)
}

pub fn parse_cirru(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => match cirru_parser::parse(s) {
      Ok(nodes) => Ok(cirru::cirru_to_calcit(&Cirru::List(nodes))),
      Err(e) => CalcitErr::err_str(format!("parse-cirru failed, {}", e)),
    },
    Some(a) => CalcitErr::err_str(format!("parse-cirru expected a string, got: {}", a)),
    None => CalcitErr::err_str("parse-cirru expected 1 argument"),
  }
}

pub fn format_cirru(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(a) => {
      let options = cirru_parser::CirruWriterOptions { use_inline: false };
      match cirru::calcit_data_to_cirru(a) {
        Ok(v) => {
          if let Cirru::List(ys) = v {
            Ok(Calcit::Str(cirru_parser::format(&ys, options)?.into()))
          } else {
            CalcitErr::err_str(format!("expected vector for Cirru formatting: {}", v))
          }
        }
        Err(e) => CalcitErr::err_str(format!("format-cirru failed, {}", e)),
      }
    }
    None => CalcitErr::err_str("parse-cirru expected 1 argument"),
  }
}

pub fn parse_cirru_edn(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => match cirru_edn::parse(s) {
      Ok(nodes) => Ok(edn::edn_to_calcit(&nodes)),
      Err(e) => CalcitErr::err_str(format!("parse-cirru-edn failed, {}", e)),
    },
    Some(a) => CalcitErr::err_str(format!("parse-cirru-edn expected a string, got: {}", a)),
    None => CalcitErr::err_str("parse-cirru-edn expected 1 argument"),
  }
}

pub fn format_cirru_edn(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(a) => Ok(Calcit::Str(cirru_edn::format(&edn::calcit_to_edn(a)?, true)?.into())),
    None => CalcitErr::err_str("format-cirru-edn expected 1 argument"),
  }
}

pub fn turn_symbol(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_str(format!("turn-symbol expected 1 argument, got: {:?}", xs));
  }
  match &xs[0] {
    Calcit::Str(s) => Ok(Calcit::Symbol {
      sym: s.to_owned(),
      ns: primes::GEN_NS.to_owned(),
      at_def: primes::GEN_DEF.to_owned(),
      resolved: None,
    }),
    Calcit::Keyword(s) => Ok(Calcit::Symbol {
      sym: s.to_string().into(),
      ns: primes::GEN_NS.to_owned(),
      at_def: primes::GEN_DEF.to_owned(),
      resolved: None,
    }),
    Calcit::Symbol { sym, ns, at_def, resolved } => Ok(Calcit::Symbol {
      sym: sym.to_owned(),
      ns: ns.to_owned(),
      at_def: at_def.to_owned(),
      resolved: resolved.to_owned(),
    }),
    a => CalcitErr::err_str(format!("turn-symbol cannot turn this to symbol: {}", a)),
  }
}

pub fn turn_keyword(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_str(format!("turn-keyword cannot turn this to keyword: {:?}", xs));
  }
  match &xs[0] {
    Calcit::Str(s) => Ok(Calcit::kwd(s)),
    Calcit::Keyword(s) => Ok(Calcit::Keyword(s.to_owned())),
    Calcit::Symbol { sym, .. } => Ok(Calcit::kwd(sym)),
    a => CalcitErr::err_str(format!("turn-keyword cannot turn this to keyword: {}", a)),
  }
}

pub fn new_tuple(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    CalcitErr::err_str(format!("tuple expected 2 arguments, got {}", CrListWrap(xs.to_owned())))
  } else {
    Ok(Calcit::Tuple(Arc::new(xs[0].to_owned()), Arc::new(xs[1].to_owned())))
  }
}

pub fn invoke_method(name: &str, invoke_args: &CalcitItems, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  if invoke_args.is_empty() {
    return Err(CalcitErr::use_msg_stack(
      format!("expected operand for method invoking: {:?}", invoke_args),
      call_stack,
    ));
  }
  let value = invoke_args[0].to_owned();
  let s0 = rpds::HashTrieMap::new_sync();
  let class = match &invoke_args[0] {
    Calcit::Tuple(a, _b) => (**a).to_owned(),
    // classed should already be preprocessed
    Calcit::List(..) => runner::evaluate_symbol("&core-list-class", &s0, primes::CORE_NS, call_stack)?,
    Calcit::Map(..) => runner::evaluate_symbol("&core-map-class", &s0, primes::CORE_NS, call_stack)?,
    Calcit::Number(..) => runner::evaluate_symbol("&core-number-class", &s0, primes::CORE_NS, call_stack)?,
    Calcit::Str(..) => runner::evaluate_symbol("&core-string-class", &s0, primes::CORE_NS, call_stack)?,
    Calcit::Set(..) => runner::evaluate_symbol("&core-set-class", &s0, primes::CORE_NS, call_stack)?,
    Calcit::Record(..) => runner::evaluate_symbol("&core-record-class", &s0, primes::CORE_NS, call_stack)?,
    Calcit::Nil => runner::evaluate_symbol("&core-nil-class", &s0, primes::CORE_NS, call_stack)?,
    Calcit::Fn { .. } | Calcit::Proc(..) => runner::evaluate_symbol("&core-fn-class", &s0, primes::CORE_NS, call_stack)?,
    x => return Err(CalcitErr::use_msg_stack(format!("cannot decide a class from: {:?}", x), call_stack)),
  };
  match &class {
    Calcit::Record(_, fields, values) => {
      match find_in_fields(fields, &EdnKwd::from(name)) {
        Some(idx) => {
          let method_args = invoke_args.skip(1)?.unshift(value);

          match &values[idx] {
            // dirty copy...
            Calcit::Fn {
              def_ns, scope, args, body, ..
            } => runner::run_fn(&method_args, scope, args, body, def_ns.to_owned(), call_stack),
            Calcit::Proc(proc) => builtins::handle_proc(proc, &method_args, call_stack),
            Calcit::Syntax(syn, _ns) => Err(CalcitErr::use_msg_stack(
              format!("cannot get syntax here since instance is always evaluated, got: {}", syn),
              call_stack,
            )),
            y => Err(CalcitErr::use_msg_stack(
              format!("expected a function to invoke, got: {}", y),
              call_stack,
            )),
          }
        }
        None => {
          let mut content = String::from("");
          for k in &**fields {
            content = format!("{},{}", content, k)
          }
          Err(CalcitErr::use_msg_stack(
            format!("missing field `{}` in {}", name, content),
            call_stack,
          ))
        }
      }
    }
    x => Err(CalcitErr::use_msg_stack(
      format!("method invoking expected a record as class, got: {}", x),
      call_stack,
    )),
  }
}

pub fn native_compare(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_str(format!("&compare expected 2 values, got {:?}", xs));
  }
  match xs[0].cmp(&xs[1]) {
    Ordering::Less => Ok(Calcit::Number(-1.0)),
    Ordering::Greater => Ok(Calcit::Number(1.0)),
    Ordering::Equal => Ok(Calcit::Number(0.0)),
  }
}

pub fn tuple_nth(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    return CalcitErr::err_str(format!("&tuple:nth expected 2 argument, got: {}", CrListWrap(xs.to_owned())));
  }
  match (&xs[0], &xs[1]) {
    (Calcit::Tuple(a, b), Calcit::Number(n)) => match f64_to_usize(*n) {
      Ok(0) => Ok((**a).to_owned()),
      Ok(1) => Ok((**b).to_owned()),
      Ok(m) => CalcitErr::err_str(format!("Tuple only got 2 elements, trying to index with {}", m)),
      Err(e) => CalcitErr::err_str(format!("&tuple:nth expect usize, {}", e)),
    },
    (a, b) => CalcitErr::err_str(format!("&tuple:nth expected a tuple and an index, got: {} {}", a, b)),
  }
}

pub fn assoc(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  if xs.len() != 3 {
    return CalcitErr::err_str(format!("tuple:assoc expected 3 arguments, got: {:?}", xs));
  }
  match (&xs[0], &xs[1]) {
    (Calcit::Tuple(a0, a1), Calcit::Number(n)) => match f64_to_usize(*n) {
      Ok(idx) => {
        if idx == 0 {
          Ok(Calcit::Tuple(Arc::new(xs[2].to_owned()), a1.to_owned()))
        } else if idx == 1 {
          Ok(Calcit::Tuple(a0.to_owned(), Arc::new(xs[2].to_owned())))
        } else {
          CalcitErr::err_str(format!("Tuple only has fields of 0,1 , unknown index: {}", idx))
        }
      }
      Err(e) => CalcitErr::err_str(e),
    },
    (a, b, ..) => CalcitErr::err_str(format!("tuple:assoc expected a tuple, got: {} {}", a, b)),
  }
}

pub fn no_op() -> Result<Calcit, CalcitErr> {
  Ok(Calcit::Nil)
}

pub fn get_os(_xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  // https://doc.rust-lang.org/std/env/consts/constant.OS.html
  Ok(Calcit::kwd(&std::env::consts::OS.to_owned()))
}

pub fn async_sleep(xs: &CalcitItems, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  use std::{thread, time};
  let sec = if xs.is_empty() {
    1.0
  } else if let Calcit::Number(n) = xs[0] {
    n
  } else {
    return Err(CalcitErr::use_msg_stack("expected number", call_stack));
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
