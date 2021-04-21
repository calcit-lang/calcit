use crate::primes::{Calcit, CalcitItems};

pub fn echo(xs: &CalcitItems) -> Result<Calcit, String> {
  let mut s = String::from("");
  for (idx, x) in xs.iter().enumerate() {
    if idx > 0 {
      s.push(' ');
    }
    s.push_str(&x.turn_string());
  }
  println!("{}", s);
  Ok(Calcit::Nil)
}

pub fn echo_values(xs: &CalcitItems) -> Result<Calcit, String> {
  let mut s = String::from("");
  for (idx, x) in xs.iter().enumerate() {
    if idx > 0 {
      s.push(' ');
    }
    s.push_str(&format!("{}", x));
  }
  println!("{}", s);
  Ok(Calcit::Nil)
}

pub fn raise(xs: &CalcitItems) -> Result<Calcit, String> {
  let mut s = String::from("");
  for (idx, x) in xs.iter().enumerate() {
    if idx > 0 {
      s.push(' ');
    }
    s.push_str(&x.turn_string());
  }
  Err(s)
}
