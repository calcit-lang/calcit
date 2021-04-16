use crate::primes::CalcitData;
use crate::primes::CalcitData::*;
use crate::program;

pub fn evaluate_expr(
  expr: CalcitData,
  scope: im::HashMap<String, CalcitData>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<CalcitData, String> {
  match expr {
    CalcitNil => Ok(expr),
    CalcitBool(_) => Ok(expr),
    CalcitNumber(_) => Ok(expr),
    CalcitSymbol(s, ns) => {
      // TODO
      Ok(CalcitNil)
    }
    CalcitKeyword(_) => Ok(expr),
    CalcitString(_) => Ok(expr),
    // CalcitRef(CalcitData), // TODO
    // CalcitThunk(CirruNode), // TODO
    CalcitList(xs) => match xs.get(0) {
      None => Err(String::from("cannot evaluate empty expr")),
      Some(x) => {
        let v = evaluate_expr((*x).clone(), scope, file_ns, program_code)?;
        match v {
          CalcitFn(_, _, f) => Err(String::from("TODO")),
          CalcitMacro(_, _, f) => Err(String::from("TODO")),
          CalcitSyntax(_, f) => Err(String::from("TODO")),
          CalcitSymbol(s, ns) => Err(format!("cannot evaluate symbol directly: {}/{}", ns, s)),
          a => Err(format!("cannot be used as operator: {}", a)),
        }
      }
    },
    CalcitSet(_) => Err(String::from("unexpected set for expr")),
    CalcitMap(_) => Err(String::from("unexpected map for expr")),
    CalcitRecord(_, _, _) => Err(String::from("unexpected record for expr")),
    CalcitMacro(_, _, _) => Ok(expr),
    CalcitFn(_, _, _) => Ok(expr),
    CalcitSyntax(_, _) => Ok(expr),
  }
}
