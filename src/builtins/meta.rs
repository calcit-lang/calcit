use crate::call_stack;
use crate::data::cirru;
use crate::data::edn;
use crate::primes;
use crate::primes::{Calcit, CalcitItems};
use crate::util::number::f64_to_usize;
use std::sync::atomic::{AtomicUsize, Ordering};

static SYMBOL_INDEX: AtomicUsize = AtomicUsize::new(0);
static JS_SYMBOL_INDEX: AtomicUsize = AtomicUsize::new(0);

pub fn type_of(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(a) => match a {
      Calcit::Nil => Ok(Calcit::Keyword(String::from("nil"))),
      // CalcitRef(Calcit), // TODO
      Calcit::Bool(..) => Ok(Calcit::Keyword(String::from("bool"))),
      Calcit::Number(..) => Ok(Calcit::Keyword(String::from("number"))),
      Calcit::Symbol(..) => Ok(Calcit::Keyword(String::from("symbol"))),
      Calcit::Keyword(..) => Ok(Calcit::Keyword(String::from("keyword"))),
      Calcit::Str(..) => Ok(Calcit::Keyword(String::from("string"))),
      Calcit::Thunk(..) => Ok(Calcit::Keyword(String::from("thunk"))), // internal
      Calcit::Ref(..) => Ok(Calcit::Keyword(String::from("ref"))),
      Calcit::Recur(..) => Ok(Calcit::Keyword(String::from("recur"))),
      Calcit::List(..) => Ok(Calcit::Keyword(String::from("list"))),
      Calcit::Set(..) => Ok(Calcit::Keyword(String::from("set"))),
      Calcit::Map(..) => Ok(Calcit::Keyword(String::from("map"))),
      Calcit::Record(..) => Ok(Calcit::Keyword(String::from("record"))),
      Calcit::Proc(..) => Ok(Calcit::Keyword(String::from("fn"))), // special kind proc, but also fn
      Calcit::Macro(..) => Ok(Calcit::Keyword(String::from("macro"))),
      Calcit::Fn(..) => Ok(Calcit::Keyword(String::from("fn"))),
      Calcit::Syntax(..) => Ok(Calcit::Keyword(String::from("synta"))),
    },
    None => Err(String::from("type-of expected 1 argument")),
  }
}

pub fn recur(xs: &CalcitItems) -> Result<Calcit, String> {
  Ok(Calcit::Recur(xs.clone()))
}

pub fn format_to_lisp(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(v) => Ok(Calcit::Str(v.lisp_str())),
    None => Err(String::from("format-to-lisp expected 1 argument")),
  }
}

pub fn gensym(xs: &CalcitItems) -> Result<Calcit, String> {
  let idx = SYMBOL_INDEX.fetch_add(1, Ordering::SeqCst);
  let n = idx + 1; // use 1 as first value since previous implementation did this

  let s = match xs.get(0) {
    Some(Calcit::Str(s)) | Some(Calcit::Keyword(s)) | Some(Calcit::Symbol(s, ..)) => {
      let mut chunk = s.clone();
      chunk.push('_');
      chunk.push('_');
      chunk.push_str(&n.to_string());
      chunk
    }
    Some(a) => return Err(format!("gensym expected a string, but got: {}", a)),
    None => {
      let mut chunk = String::from("G__");
      chunk.push_str(&n.to_string());
      chunk
    }
  };
  Ok(Calcit::Symbol(s, primes::GENERATED_NS.to_string(), None))
}

pub fn reset_gensym_index(_xs: &CalcitItems) -> Result<Calcit, String> {
  let _ = SYMBOL_INDEX.swap(0, Ordering::SeqCst);
  Ok(Calcit::Nil)
}

pub fn force_reset_gensym_index() -> Result<(), String> {
  let _ = SYMBOL_INDEX.swap(0, Ordering::SeqCst);
  Ok(())
}

pub fn reset_js_gensym_index() {
  let _ = JS_SYMBOL_INDEX.swap(0, Ordering::SeqCst);
}

// for emitting js
pub fn js_gensym(name: &str) -> String {
  let idx = JS_SYMBOL_INDEX.fetch_add(1, Ordering::SeqCst);
  let n = idx + 1; // use 1 as first value since previous implementation did this

  let mut chunk = String::from(name);
  chunk.push_str("_AUTO_");
  chunk.push_str(&n.to_string());
  chunk
}

pub fn generate_id(xs: &CalcitItems) -> Result<Calcit, String> {
  let size = match xs.get(0) {
    Some(Calcit::Number(n)) => match f64_to_usize(*n) {
      Ok(size) => Some(size),
      Err(e) => return Err(e),
    },
    Some(a) => return Err(format!("expected usize, got: {}", a)),
    None => None, // nanoid defaults to 21
  };

  match (size, xs.get(1)) {
    (None, None) => Ok(Calcit::Str(nanoid!())),
    (Some(n), None) => Ok(Calcit::Str(nanoid!(n))),
    (Some(n), Some(Calcit::Str(s))) => {
      let mut charset: Vec<char> = vec![];
      for c in s.chars() {
        charset.push(c);
      }
      Ok(Calcit::Str(nanoid!(n, &charset)))
    }
    (a, b) => Err(format!("generate-id! expected size or charset, got: {:?} {:?}", a, b)),
  }
}

pub fn display_stack(_xs: &CalcitItems) -> Result<Calcit, String> {
  call_stack::show_stack();
  Ok(Calcit::Nil)
}

pub fn parse_cirru(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => match cirru_parser::parse(s) {
      Ok(nodes) => Ok(cirru::cirru_to_calcit(&nodes)),
      Err(e) => Err(format!("parse-cirru failed, {}", e)),
    },
    Some(a) => Err(format!("parse-cirru expected a string, got: {}", a)),
    None => Err(String::from("parse-cirru expected 1 argument")),
  }
}

pub fn write_cirru(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(a) => {
      let options = cirru_parser::CirruWriterOptions { use_inline: false };
      match cirru::calcit_data_to_cirru(a) {
        Ok(v) => Ok(Calcit::Str(cirru_parser::format(&v, options))),
        Err(e) => Err(format!("write-cirru failed, {}", e)),
      }
    }
    None => Err(String::from("parse-cirru expected 1 argument")),
  }
}

pub fn parse_cirru_edn(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => match cirru_edn::parse(s) {
      Ok(nodes) => Ok(edn::edn_to_calcit(&nodes)),
      Err(e) => Err(format!("parse-cirru-edn failed, {}", e)),
    },
    Some(a) => Err(format!("parse-cirru-edn expected a string, got: {}", a)),
    None => Err(String::from("parse-cirru-edn expected 1 argument")),
  }
}

pub fn write_cirru_edn(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(a) => Ok(Calcit::Str(cirru_edn::format(&edn::calcit_to_edn(a), true))),
    None => Err(String::from("write-cirru-edn expected 1 argument")),
  }
}

pub fn turn_symbol(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => Ok(Calcit::Symbol(s.clone(), primes::GENERATED_NS.to_string(), None)),
    Some(Calcit::Keyword(s)) => Ok(Calcit::Symbol(s.clone(), primes::GENERATED_NS.to_string(), None)),
    Some(Calcit::Symbol(s, ns, resolved)) => Ok(Calcit::Symbol(s.clone(), ns.clone(), resolved.clone())),
    Some(a) => Err(format!("turn-symbol cannot turn this to symbol: {}", a)),
    None => Err(String::from("turn-symbol expected 1 argument, got nothing")),
  }
}

pub fn turn_keyword(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => Ok(Calcit::Keyword(s.clone())),
    Some(Calcit::Keyword(s)) => Ok(Calcit::Keyword(s.clone())),
    Some(Calcit::Symbol(s, ..)) => Ok(Calcit::Keyword(s.clone())),
    Some(a) => Err(format!("turn-keyword cannot turn this to keyword: {}", a)),
    None => Err(String::from("turn-keyword expected 1 argument, got nothing")),
  }
}
