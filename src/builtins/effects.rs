use std::env;
use std::fs;
use std::process::exit;
use std::sync::RwLock;
use std::time::Instant;

use crate::{
  primes::{Calcit, CalcitErr, CalcitItems},
  util::number::f64_to_i32,
};

#[derive(Clone, Debug, Copy)]
pub enum CliRunningMode {
  Eval,
  Js,
  Ir,
}

lazy_static! {
  static ref STARTED_INSTANT: RwLock<Instant> = RwLock::new(Instant::now());
  static ref CLI_RUNNING_MODE: RwLock<CliRunningMode> = RwLock::new(CliRunningMode::Eval);
}

pub fn raise(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  let mut s = String::from("");
  for (idx, x) in xs.into_iter().enumerate() {
    if idx > 0 {
      s.push(' ');
    }
    s.push_str(&x.turn_string());
  }
  CalcitErr::err_str(s)
}

pub fn init_effects_states() {
  // trigger lazy instant
  let _eff = STARTED_INSTANT.read().expect("read instant");
}

pub fn cpu_time(_xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  let now = Instant::now();
  let started = STARTED_INSTANT.read().expect("read instant").to_owned();

  let time = match now.checked_duration_since(started) {
    Some(n) => (n.as_micros() as f64) / 1000.0,
    None => {
      println!("[Warn] got none CPU time from: {started:?} -> {now:?}");
      0.0
    }
  };

  Ok(Calcit::Number(time))
}

pub fn modify_cli_running_mode(mode: CliRunningMode) -> Result<(), String> {
  *CLI_RUNNING_MODE.write().expect("set mode") = mode;
  Ok(())
}

pub fn calcit_running_mode(_xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  let mode = CLI_RUNNING_MODE.read().expect("read mode").to_owned();
  match mode {
    CliRunningMode::Eval => Ok(Calcit::tag("eval")),
    CliRunningMode::Js => Ok(Calcit::tag("js")),
    CliRunningMode::Ir => Ok(Calcit::tag("ir")),
  }
}

/// is evaling in Rust, not for js
pub fn is_rust_eval() -> bool {
  let mode = CLI_RUNNING_MODE.read().expect("read mode").to_owned();
  matches!(mode, CliRunningMode::Eval)
}

// TODO
pub fn call_get_calcit_backend(_xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  Ok(Calcit::tag("rust"))
}

pub fn quit(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Number(n)) => match f64_to_i32(*n) {
      Ok(code) => exit(code),
      Err(e) => unreachable!("quit failed to get code from f64, {}", e),
    },
    Some(a) => CalcitErr::err_str(format!("quit expected i32 value, got: {a}")),
    None => CalcitErr::err_str("quit expected a code, got nothing"),
  }
}

pub fn get_env(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  if xs.len() > 2 {
    return CalcitErr::err_str("get-env get 1~2 arguments");
  }
  match xs.get(0) {
    Some(Calcit::Str(s)) => match env::var(&**s) {
      Ok(v) => {
        let has_default = xs.len() == 2;
        if has_default {
          println!("(get-env {s}): {v}");
        }
        Ok(Calcit::Str(v.into()))
      }
      Err(e) => match xs.get(1) {
        Some(v0) => Ok(v0.to_owned()),
        None => {
          println!("(get-env {s}): {e}");
          Ok(Calcit::Nil)
        }
      },
    },
    Some(a) => CalcitErr::err_str(format!("get-env expected a string, got: {a}")),
    None => CalcitErr::err_str("get-env expected an argument, got nothing"),
  }
}

pub fn read_file(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => match fs::read_to_string(&**s) {
      Ok(content) => Ok(Calcit::Str(content.into())),
      Err(e) => CalcitErr::err_str(format!("read-file failed at {}: {e}", &**s)),
    },
    Some(a) => CalcitErr::err_str(format!("read-file expected a string, got: {a}")),
    None => CalcitErr::err_str("read-file expected a filename, got nothing"),
  }
}

pub fn write_file(xs: &CalcitItems) -> Result<Calcit, CalcitErr> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(path)), Some(Calcit::Str(content))) => match fs::write(&**path, &**content) {
      Ok(_) => Ok(Calcit::Nil),
      Err(e) => CalcitErr::err_str(format!("write-file failed, {e}")),
    },
    (Some(a), Some(b)) => CalcitErr::err_str(format!("write-file expected 3 strings, got: {a} {b}")),
    (a, b) => CalcitErr::err_str(format!("write-file expected 2 strings, got: {a:?} {b:?}")),
  }
}
