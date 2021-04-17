use crate::primes::CalcitData;
use crate::primes::CalcitData::*;

pub fn echo(xs: im::Vector<CalcitData>) -> Result<CalcitData, String> {
  let mut s = String::from("");
  for x in xs {
    s.push_str(&(format!("{}", x)));
  }
  println!("{}", s);
  Ok(CalcitNil)
}
