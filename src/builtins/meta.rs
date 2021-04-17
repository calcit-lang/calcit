use crate::primes::CalcitData::*;
use crate::primes::{CalcitData, CalcitItems};

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
