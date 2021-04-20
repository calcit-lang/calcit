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
    Some(CalcitString(s)) | Some(CalcitKeyword(s)) | Some(CalcitSymbol(s, _)) => {
      let mut chunk = s.clone();
      chunk.push('_');
      chunk.push('_');
      chunk.push_str(&idx.to_string());
      chunk
    }
    Some(a) => return Err(format!("gensym expected a string, but got: {}", a)),
    None => String::from("G__"),
  };
  Ok(CalcitSymbol(s, primes::GENERATED_NS.to_string()))
}

pub fn reset_gensym_index(_xs: &CalcitItems) -> Result<CalcitData, String> {
  let _ = SYMBOL_INDEX.swap(0, Ordering::SeqCst);
  Ok(CalcitNil)
}
