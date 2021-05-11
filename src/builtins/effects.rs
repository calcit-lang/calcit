use chrono::{DateTime, TimeZone, Utc};
use std::env;
use std::fs;
use std::process::exit;
use std::sync::Mutex;
use std::time::Instant;

use crate::{
  primes::{Calcit, CalcitItems},
  program,
  util::number::f64_to_i32,
};

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

// TODO
pub fn call_get_calcit_backend(_xs: &CalcitItems) -> Result<Calcit, String> {
  Ok(Calcit::Keyword(String::from("rust")))
}

pub fn quit(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Number(n)) => match f64_to_i32(*n) {
      Ok(code) => exit(code),
      Err(e) => unreachable!("quit failed to get code from f64, {}", e),
    },
    Some(a) => Err(format!("quit expected i32 value, got: {}", a)),
    None => Err(String::from("quit expected a code, got nothing")),
  }
}

pub fn get_env(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => match env::var(s) {
      Ok(v) => Ok(Calcit::Str(v)),
      Err(e) => {
        println!("(get-env {}): {}", s, e);
        Ok(Calcit::Nil)
      }
    },
    Some(a) => Err(format!("get-env expected a string, got {}", a)),
    None => Err(String::from("get-env expected an argument, got nothing")),
  }
}

pub fn read_file(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => match fs::read_to_string(s) {
      Ok(content) => Ok(Calcit::Str(content)),
      Err(e) => Err(format!("read-file failed: {}", e)),
    },
    Some(a) => Err(format!("read-file expected a string, got: {}", a)),
    None => Err(String::from("read-file expected a filename, got nothing")),
  }
}

pub fn write_file(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(path)), Some(Calcit::Str(content))) => match fs::write(path, content) {
      Ok(_) => Ok(Calcit::Nil),
      Err(e) => Err(format!("write-file failed, {}", e)),
    },
    (Some(a), Some(b)) => Err(format!("write-file expected 3 strings, got: {} {}", a, b)),
    (a, b) => Err(format!("write-file expected 2 strings, got: {:?} {:?}", a, b)),
  }
}

/// calcit-js represents DateTime in f64
pub fn parse_time(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Str(s)), None) => match DateTime::parse_from_rfc3339(s) {
      Ok(time) => Ok(Calcit::Number(time.timestamp_millis() as f64)),
      Err(e) => Err(format!("parse-time failed, {}", e)),
    },
    (Some(Calcit::Str(s)), Some(Calcit::Str(f))) => match DateTime::parse_from_str(s, f) {
      Ok(time) => Ok(Calcit::Number(time.timestamp_millis() as f64)),
      Err(e) => Err(format!("parse-time failed, {} {} {}", s, f, e)),
    },
    (Some(a), Some(b)) => Err(format!("parse-time expected 1 or 2 strings, got: {} {}", a, b)),
    (a, b) => Err(format!("parse-time expected 1 or 2 strings, got {:?} {:?}", a, b)),
  }
}

pub fn now_bang(_xs: &CalcitItems) -> Result<Calcit, String> {
  Ok(Calcit::Number(Utc::now().timestamp_millis() as f64))
}

pub fn format_time(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Number(n)), None) => {
      let time = Utc.timestamp((n.floor() / 1000.0) as i64, (n.fract() * 1_000_000.0) as u32);
      Ok(Calcit::Str(time.to_rfc3339()))
    }
    (Some(Calcit::Number(n)), Some(Calcit::Str(f))) => {
      let time = Utc.timestamp((n.floor() / 1000.0) as i64, (n.fract() * 1_000_000.0) as u32);
      Ok(Calcit::Str(time.format(f).to_string()))
    }
    (Some(Calcit::Number(_)), Some(a)) => Err(format!("format-time Rust does not support dynamic format: {}", a)),
    (Some(a), Some(b)) => Err(format!("format-time expected time and string, got: {} {}", a, b)),
    (a, b) => Err(format!("format-time expected time and string, got: {:?} {:?}", a, b)),
  }
}

pub fn ffi_message(xs: &CalcitItems) -> Result<Calcit, String> {
  if xs.len() >= 1 {
    match &xs[0] {
      Calcit::Str(s) | Calcit::Symbol(s, ..) => {
        let items = xs.to_owned().slice(1..);
        program::send_ffi_message(s.to_owned(), items);
        Ok(Calcit::Nil)
      }
      a => Err(format!("&ffi-message expected string, got {}", a)),
    }
  } else {
    Err(String::from("&ffi-message expected arguments but got empty"))
  }
}
