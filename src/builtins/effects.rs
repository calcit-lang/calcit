use std::sync::Mutex;
use std::time::Instant;

use crate::primes::{Calcit, CalcitItems};

lazy_static! {
  static ref STARTED_INSTANT: Mutex<Instant> = Mutex::new(Instant::now());
}

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

pub fn init_effects_states() {
  // trigger lazy instant
  let _eff = STARTED_INSTANT.lock().unwrap();
}

pub fn cpu_time(_xs: &CalcitItems) -> Result<Calcit, String> {
  let now = Instant::now();
  let started = STARTED_INSTANT.lock().unwrap().to_owned();

  let time = match now.checked_duration_since(started) {
    Some(n) => (n.as_micros() as f64) / 1000.0,
    None => {
      println!("[Warn] got none CPU time from: {:?} -> {:?}", started, now);
      0.0
    }
  };

  Ok(Calcit::Number(time))
}
