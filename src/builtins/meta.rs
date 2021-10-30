use crate::{
  builtins,
  builtins::records::find_in_fields,
  call_stack,
  call_stack::CallStackVec,
  data::{cirru, edn},
  primes,
  primes::{gen_core_id, keyword::load_order_key, lookup_order_kwd_str, Calcit, CalcitErr, CalcitItems, CrListWrap},
  runner,
  util::number::f64_to_usize,
};

use cirru_parser::{Cirru, CirruWriterOptions};

use std::cmp::Ordering;
use std::sync::atomic;
use std::sync::atomic::AtomicUsize;

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
    Calcit::Symbol(..) => Ok(Calcit::kwd("symbol")),
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
    Calcit::Macro(..) => Ok(Calcit::kwd("macro")),
    Calcit::Fn(..) => Ok(Calcit::kwd("fn")),
    Calcit::Syntax(..) => Ok(Calcit::kwd("synta")),
  }
}

pub fn recur(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  Ok(Calcit::Recur(xs.to_owned()))
}

pub fn format_to_lisp(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(v) => Ok(Calcit::Str(v.lisp_str())),
    None => CalcitErr::err_str("format-to-lisp expected 1 argument"),
  }
}

pub fn format_to_cirru(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(v) => cirru_parser::format(&[transform_code_to_cirru(v)], CirruWriterOptions { use_inline: false })
      .map(Calcit::Str)
      .map_err(CalcitErr::use_str),
    None => CalcitErr::err_str("format-to-cirru expected 1 argument"),
  }
}

fn transform_code_to_cirru(x: &Calcit) -> Cirru {
  match x {
    Calcit::List(ys) => {
      let mut xs: Vec<Cirru> = Vec::with_capacity(ys.len());
      for y in ys {
        xs.push(transform_code_to_cirru(y));
      }
      Cirru::List(xs)
    }
    Calcit::Symbol(s, ..) => Cirru::Leaf(s.to_owned()),
    Calcit::Syntax(s, _ns) => Cirru::Leaf(s.to_string()),
    Calcit::Proc(s) => Cirru::Leaf(s.to_owned()),
    a => Cirru::Leaf(format!("{}", a)),
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
      Calcit::Str(s) | Calcit::Symbol(s, ..) => {
        let mut chunk = s.to_owned();
        chunk.push('_');
        chunk.push('_');
        chunk.push_str(&n.to_string());
        chunk
      }
      Calcit::Keyword(s) => {
        let mut chunk = lookup_order_kwd_str(s);
        chunk.push('_');
        chunk.push('_');
        chunk.push_str(&n.to_string());
        chunk
      }
      a => return CalcitErr::err_str(format!("gensym expected a string, but got: {}", a)),
    }
  };
  Ok(Calcit::Symbol(
    s,
    String::from(primes::GENERATED_NS),
    String::from(primes::GENERATED_DEF),
    None,
  ))
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

pub fn display_stack(_xs: &CalcitItems, call_stack: &CallStackVec) -> Result<Calcit, CalcitErr> {
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
            Ok(Calcit::Str(cirru_parser::format(&ys, options).map_err(CalcitErr::use_str)?))
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
    Some(a) => Ok(Calcit::Str(
      cirru_edn::format(&edn::calcit_to_edn(a).map_err(CalcitErr::use_str)?, true).map_err(CalcitErr::use_str)?,
    )),
    None => CalcitErr::err_str("format-cirru-edn expected 1 argument"),
  }
}

pub fn turn_symbol(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  if xs.len() != 1 {
    return CalcitErr::err_str(format!("turn-symbol expected 1 argument, got: {:?}", xs));
  }
  match &xs[0] {
    Calcit::Str(s) => Ok(Calcit::Symbol(
      s.to_owned(),
      String::from(primes::GENERATED_NS),
      String::from(primes::GENERATED_DEF),
      None,
    )),
    Calcit::Keyword(s) => Ok(Calcit::Symbol(
      lookup_order_kwd_str(s),
      String::from(primes::GENERATED_NS),
      String::from(primes::GENERATED_DEF),
      None,
    )),
    Calcit::Symbol(s, ns, def, resolved) => Ok(Calcit::Symbol(s.to_owned(), ns.to_owned(), def.to_owned(), resolved.to_owned())),
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
    Calcit::Symbol(s, ..) => Ok(Calcit::kwd(s)),
    a => CalcitErr::err_str(format!("turn-keyword cannot turn this to keyword: {}", a)),
  }
}

pub fn new_tuple(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  if xs.len() != 2 {
    CalcitErr::err_str(format!("tuple expected 2 arguments, got {}", CrListWrap(xs.to_owned())))
  } else {
    Ok(Calcit::Tuple(Box::new(xs[0].to_owned()), Box::new(xs[1].to_owned())))
  }
}

pub fn invoke_method(name: &str, invoke_args: &CalcitItems, call_stack: &CallStackVec) -> Result<Calcit, CalcitErr> {
  if invoke_args.is_empty() {
    return Err(CalcitErr::use_msg_stack(
      format!("expected operand for method invoking: {:?}", invoke_args),
      call_stack,
    ));
  }
  let (class, value) = match &invoke_args[0] {
    Calcit::Tuple(a, _b) => ((**a).to_owned(), invoke_args.get(0).unwrap().to_owned()),
    Calcit::Number(..) => {
      // classed should already be preprocessed
      let code = gen_sym("&core-number-class");
      let class = runner::evaluate_expr(&code, &rpds::HashTrieMap::new_sync(), primes::CORE_NS, call_stack)?;
      (class, invoke_args[0].to_owned())
    }
    Calcit::Str(..) => {
      let code = gen_sym("&core-string-class");
      let class = runner::evaluate_expr(&code, &rpds::HashTrieMap::new_sync(), primes::CORE_NS, call_stack)?;
      (class, invoke_args[0].to_owned())
    }
    Calcit::Set(..) => {
      let code = gen_sym("&core-set-class");
      let class = runner::evaluate_expr(&code, &rpds::HashTrieMap::new_sync(), primes::CORE_NS, call_stack)?;
      (class, invoke_args[0].to_owned())
    }
    Calcit::List(..) => {
      let code = gen_sym("&core-list-class");
      let class = runner::evaluate_expr(&code, &rpds::HashTrieMap::new_sync(), primes::CORE_NS, call_stack)?;
      (class, invoke_args[0].to_owned())
    }
    Calcit::Map(..) => {
      let code = gen_sym("&core-map-class");
      let class = runner::evaluate_expr(&code, &rpds::HashTrieMap::new_sync(), primes::CORE_NS, call_stack)?;
      (class, invoke_args[0].to_owned())
    }
    Calcit::Record(..) => {
      let code = gen_sym("&core-record-class");
      let class = runner::evaluate_expr(&code, &rpds::HashTrieMap::new_sync(), primes::CORE_NS, call_stack)?;
      (class, invoke_args[0].to_owned())
    }
    Calcit::Nil => {
      let code = gen_sym("&core-nil-class");
      let class = runner::evaluate_expr(&code, &rpds::HashTrieMap::new_sync(), primes::CORE_NS, call_stack)?;
      (class, invoke_args[0].to_owned())
    }
    Calcit::Fn(..) | Calcit::Proc(..) => {
      let code = gen_sym("&core-fn-class");
      let class = runner::evaluate_expr(&code, &rpds::HashTrieMap::new_sync(), primes::CORE_NS, call_stack)?;
      (class, invoke_args[0].to_owned())
    }
    x => return Err(CalcitErr::use_msg_stack(format!("cannot decide a class from: {:?}", x), call_stack)),
  };
  match &class {
    Calcit::Record(_, fields, values) => {
      match find_in_fields(fields, load_order_key(name)) {
        Some(idx) => {
          let mut method_args: rpds::VectorSync<Calcit> = rpds::vector_sync![];
          method_args.push_back_mut(value);
          let mut at_first = true;
          for x in invoke_args {
            if at_first {
              at_first = false
            } else {
              method_args.push_back_mut(x.to_owned())
            }
          }

          match &values[idx] {
            // dirty copy...
            Calcit::Fn(_, def_ns, _, def_scope, args, body) => runner::run_fn(&method_args, def_scope, args, body, def_ns, call_stack),
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
          for k in fields {
            content = format!("{},{}", content, lookup_order_kwd_str(k))
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

fn gen_sym(sym: &str) -> Calcit {
  Calcit::Symbol(
    String::from("&core-map-class"),
    String::from(primes::CORE_NS),
    String::from(primes::GENERATED_DEF),
    Some(Box::new(primes::SymbolResolved::ResolvedDef(
      String::from(primes::CORE_NS),
      String::from(sym),
      None,
    ))),
  )
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
          Ok(Calcit::Tuple(Box::new(xs[2].to_owned()), a1.to_owned()))
        } else if idx == 1 {
          Ok(Calcit::Tuple(a0.to_owned(), Box::new(xs[2].to_owned())))
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

pub fn async_sleep(xs: &CalcitItems, call_stack: &CallStackVec) -> Result<Calcit, CalcitErr> {
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
