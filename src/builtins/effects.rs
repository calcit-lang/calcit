use crate::primes::CalcitData;
use crate::primes::CalcitData::*;

pub fn echo(xs: im::Vector<CalcitData>) -> Result<CalcitData, String> {
  let mut s = String::from("");
  for (idx, x) in xs.iter().enumerate() {
    if idx > 0 {
      s.push(' ');
    }
    s.push_str(&x.turn_string());
  }
  println!("{}", s);
  Ok(CalcitNil)
}

pub fn echo_values(xs: im::Vector<CalcitData>) -> Result<CalcitData, String> {
  let mut s = String::from("");
  for (idx, x) in xs.iter().enumerate() {
    if idx > 0 {
      s.push(' ');
    }
    s.push_str(&format!("{}", x));
  }
  println!("{}", s);
  Ok(CalcitNil)
}
