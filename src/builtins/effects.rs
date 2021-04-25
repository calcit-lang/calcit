use std::sync::Mutex;
use std::time::Instant;

use crate::primes::{Calcit, CalcitItems};

#[derive(Clone, Debug)]
pub enum CliRunningMode {
  Eval,
  Js,
  Ir,
}

lazy_static! {
  static ref STARTED_INSTANT: Mutex<Instant> = Mutex::new(Instant::now());
  static ref CLI_RUNNING_MODE: Mutex<CliRunningMode> = Mutex::new(CliRunningMode::Eval);
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

pub fn modify_cli_running_mode(mode: CliRunningMode) -> Result<(), String> {
  // obscure https://doc.rust-lang.org/std/sync/struct.Mutex.html#method.get_mut
  *CLI_RUNNING_MODE.lock().unwrap() = mode;
  Ok(())
}

pub fn calcit_running_mode(_xs: &CalcitItems) -> Result<Calcit, String> {
  let mode = CLI_RUNNING_MODE.lock().unwrap().to_owned();
  match mode {
    CliRunningMode::Eval => Ok(Calcit::Keyword(String::from("eval"))),
    CliRunningMode::Js => Ok(Calcit::Keyword(String::from("js"))),
    CliRunningMode::Ir => Ok(Calcit::Keyword(String::from("ir"))),
  }
}
