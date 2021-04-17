use crate::primes::{CalcitData, CalcitData::*, CalcitItems};

pub fn echo(xs: &CalcitItems) -> Result<CalcitData, String> {
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

pub fn echo_values(xs: &CalcitItems) -> Result<CalcitData, String> {
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

pub fn raise(xs: &CalcitItems) -> Result<CalcitData, String> {
  let mut s = String::from("");
  for (idx, x) in xs.iter().enumerate() {
    if idx > 0 {
      s.push(' ');
    }
    s.push_str(&x.turn_string());
  }
  Err(s)
}
