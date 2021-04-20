use crate::builtins::math::f32_to_usize;
use crate::call_stack;
use crate::data::cirru;
use crate::data::edn;
use crate::primes;
use crate::primes::CalcitData::*;
use crate::primes::{CalcitData, CalcitItems};
use std::sync::atomic::{AtomicUsize, Ordering};

static SYMBOL_INDEX: AtomicUsize = AtomicUsize::new(0);

pub fn type_of(xs: &CalcitItems) -> Result<CalcitData, String> {
  match xs.get(0) {
    Some(a) => match a {
      CalcitNil => Ok(CalcitKeyword(String::from("nil"))),
      // CalcitRef(CalcitData), // TODO
      // CalcitThunk(CirruNode), // TODO
      CalcitBool(..) => Ok(CalcitKeyword(String::from("bool"))),
      CalcitNumber(..) => Ok(CalcitKeyword(String::from("number"))),
      CalcitSymbol(..) => Ok(CalcitKeyword(String::from("symbol"))),
      CalcitKeyword(..) => Ok(CalcitKeyword(String::from("keyword"))),
      CalcitString(..) => Ok(CalcitKeyword(String::from("string"))),
      CalcitRecur(..) => Ok(CalcitKeyword(String::from("recur"))),
      CalcitList(..) => Ok(CalcitKeyword(String::from("list"))),
      CalcitSet(..) => Ok(CalcitKeyword(String::from("set"))),
      CalcitMap(..) => Ok(CalcitKeyword(String::from("map"))),
      CalcitRecord(..) => Ok(CalcitKeyword(String::from("record"))),
      CalcitProc(..) => Ok(CalcitKeyword(String::from("fn"))), // special kind proc, but also fn
      CalcitMacro(..) => Ok(CalcitKeyword(String::from("macro"))),
      CalcitFn(..) => Ok(CalcitKeyword(String::from("fn"))),
      CalcitSyntax(..) => Ok(CalcitKeyword(String::from("synta"))),
    },
    None => Err(String::from("type-of expected 1 argument")),
  }
}

pub fn recur(xs: &CalcitItems) -> Result<CalcitData, String> {
  Ok(CalcitRecur(xs.clone()))
}

pub fn format_to_lisp(xs: &CalcitItems) -> Result<CalcitData, String> {
  match xs.get(0) {
    Some(v) => Ok(CalcitString(primes::format_to_lisp(v))),
    None => Err(String::from("format-to-lisp expected 1 argument")),
  }
}

pub fn gensym(xs: &CalcitItems) -> Result<CalcitData, String> {
  let idx = SYMBOL_INDEX.fetch_add(1, Ordering::SeqCst);

  let s = match xs.get(0) {
    Some(CalcitString(s)) | Some(CalcitKeyword(s)) | Some(CalcitSymbol(s, ..)) => {
      let mut chunk = s.clone();
      chunk.push('_');
      chunk.push('_');
      chunk.push_str(&idx.to_string());
      chunk
    }
    Some(a) => return Err(format!("gensym expected a string, but got: {}", a)),
    None => {
      let mut chunk = String::from("G__");
      chunk.push_str(&idx.to_string());
      chunk
    }
  };
  Ok(CalcitSymbol(s, primes::GENERATED_NS.to_string(), None))
}

pub fn reset_gensym_index(_xs: &CalcitItems) -> Result<CalcitData, String> {
  let _ = SYMBOL_INDEX.swap(0, Ordering::SeqCst);
  Ok(CalcitNil)
}

pub fn get_calcit_running_mode(_xs: &CalcitItems) -> Result<CalcitData, String> {
  Ok(CalcitKeyword(String::from("eval")))
}

pub fn generate_id(xs: &CalcitItems) -> Result<CalcitData, String> {
  let size = match xs.get(0) {
    Some(CalcitNumber(n)) => match f32_to_usize(*n) {
      Ok(size) => Some(size),
      Err(e) => return Err(e),
    },
    Some(a) => return Err(format!("expected usize, got: {}", a)),
    None => None, // nanoid defaults to 21
  };

  match (size, xs.get(1)) {
    (None, None) => Ok(CalcitString(nanoid!())),
    (Some(n), None) => Ok(CalcitString(nanoid!(n))),
    (Some(n), Some(CalcitString(s))) => {
      let mut charset: Vec<char> = vec![];
      for c in s.chars() {
        charset.push(c);
      }
      Ok(CalcitString(nanoid!(n, &charset)))
    }
    (a, b) => Err(format!(
      "generate-id! expected size or charset, got: {:?} {:?}",
      a, b
    )),
  }
}

pub fn display_stack(_xs: &CalcitItems) -> Result<CalcitData, String> {
  call_stack::show_stack();
  Ok(CalcitNil)
}

pub fn parse_cirru(xs: &CalcitItems) -> Result<CalcitData, String> {
  match xs.get(0) {
    Some(CalcitString(s)) => match cirru_parser::parse_cirru(s.clone()) {
      Ok(nodes) => Ok(cirru::cirru_to_calcit(&nodes)),
      Err(e) => Err(format!("parse-cirru failed, {}", e)),
    },
    Some(a) => Err(format!("parse-cirru expected a string, got: {}", a)),
    None => Err(String::from("parse-cirru expected 1 argument")),
  }
}

pub fn write_cirru(xs: &CalcitItems) -> Result<CalcitData, String> {
  match xs.get(0) {
    Some(a) => {
      let options = cirru_parser::CirruWriterOptions { use_inline: false };
      match cirru::calcit_data_to_cirru(a) {
        Ok(v) => Ok(CalcitString(cirru_parser::write_cirru(&v, options))),
        Err(e) => Err(format!("write-cirru failed, {}", e)),
      }
    }
    None => Err(String::from("parse-cirru expected 1 argument")),
  }
}

pub fn parse_cirru_edn(xs: &CalcitItems) -> Result<CalcitData, String> {
  match xs.get(0) {
    Some(CalcitString(s)) => match cirru_edn::parse_cirru_edn(s.clone()) {
      Ok(nodes) => Ok(edn::edn_to_calcit(&nodes)),
      Err(e) => Err(format!("parse-cirru-edn failed, {}", e)),
    },
    Some(a) => Err(format!("parse-cirru-edn expected a string, got: {}", a)),
    None => Err(String::from("parse-cirru-edn expected 1 argument")),
  }
}

pub fn write_cirru_edn(xs: &CalcitItems) -> Result<CalcitData, String> {
  match xs.get(0) {
    Some(a) => Ok(CalcitString(cirru_edn::write_cirru_edn(
      edn::calcit_to_edn(a),
    ))),
    None => Err(String::from("write-cirru-edn expected 1 argument")),
  }
}

pub fn turn_symbol(xs: &CalcitItems) -> Result<CalcitData, String> {
  match xs.get(0) {
    Some(CalcitString(s)) => Ok(CalcitSymbol(
      s.clone(),
      primes::GENERATED_NS.to_string(),
      None,
    )),
    Some(CalcitKeyword(s)) => Ok(CalcitSymbol(
      s.clone(),
      primes::GENERATED_NS.to_string(),
      None,
    )),
    Some(CalcitSymbol(s, ns, resolved)) => {
      Ok(CalcitSymbol(s.clone(), ns.clone(), resolved.clone()))
    }
    Some(a) => Err(format!("turn-symbol cannot turn this to symbol: {}", a)),
    None => Err(String::from("turn-symbol expected 1 argument, got nothing")),
  }
}
