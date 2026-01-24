use std::env;
use std::fs;
use std::process::exit;
use std::sync::LazyLock;
use std::sync::RwLock;
use std::time::Instant;

use crate::{
  builtins::meta::type_of,
  calcit::{Calcit, CalcitErr, CalcitErrKind, CalcitProc, format_proc_examples_hint},
  util::number::f64_to_i32,
};

#[derive(Clone, Debug, Copy)]
pub enum CliRunningMode {
  Eval,
  Js,
  Ir,
}

static STARTED_INSTANT: LazyLock<RwLock<Instant>> = LazyLock::new(|| RwLock::new(Instant::now()));
static CLI_RUNNING_MODE: LazyLock<RwLock<CliRunningMode>> = LazyLock::new(|| RwLock::new(CliRunningMode::Eval));

pub fn raise(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  let mut s = String::from("");
  for (idx, x) in xs.iter().enumerate() {
    if idx > 0 {
      s.push(' ');
    }
    s.push_str(&x.turn_string());
  }
  CalcitErr::err_str(CalcitErrKind::Effect, s)
}

pub fn init_effects_states() {
  // trigger lazy instant
  let _eff = STARTED_INSTANT.read().expect("read instant");
}

pub fn cpu_time(_xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  let now = Instant::now();
  let started = STARTED_INSTANT.read().expect("read instant").to_owned();

  let time = match now.checked_duration_since(started) {
    Some(n) => (n.as_micros() as f64) / 1000.0,
    None => {
      eprintln!("[Warn] got none CPU time from: {started:?} -> {now:?}");
      0.0
    }
  };

  Ok(Calcit::Number(time))
}

pub fn modify_cli_running_mode(mode: CliRunningMode) -> Result<(), String> {
  *CLI_RUNNING_MODE.write().expect("set mode") = mode;
  Ok(())
}

pub fn calcit_running_mode(_xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  let mode = CLI_RUNNING_MODE.read().expect("read mode").to_owned();
  match mode {
    CliRunningMode::Eval => Ok(Calcit::tag("eval")),
    CliRunningMode::Js => Ok(Calcit::tag("js")),
    CliRunningMode::Ir => Ok(Calcit::tag("ir")),
  }
}

/// is evaluating in Rust, not for js
pub fn is_rust_eval() -> bool {
  let mode = CLI_RUNNING_MODE.read().expect("read mode").to_owned();
  matches!(mode, CliRunningMode::Eval)
}

pub fn call_get_calcit_backend(_xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  Ok(Calcit::tag("rust"))
}

pub fn quit(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Number(n)) => match f64_to_i32(*n) {
      Ok(code) => exit(code),
      Err(e) => unreachable!("quit failed to get code from f64, {}", e),
    },
    Some(a) => {
      let msg = format!(
        "quit requires an i32 number, but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::Quit).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    None => CalcitErr::err_str(CalcitErrKind::Arity, "quit expected a code, got nothing"),
  }
}

pub fn get_env(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  if xs.len() > 2 {
    return CalcitErr::err_str(CalcitErrKind::Arity, "get-env get 1~2 arguments");
  }
  match xs.first() {
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
    Some(a) => {
      let msg = format!(
        "get-env requires a string (environment variable name), but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::GetEnv).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    None => CalcitErr::err_str(CalcitErrKind::Arity, "get-env expected an argument, got nothing"),
  }
}

pub fn read_file(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs.first() {
    Some(Calcit::Str(s)) => match fs::read_to_string(&**s) {
      Ok(content) => Ok(Calcit::Str(content.into())),
      Err(e) => CalcitErr::err_str(CalcitErrKind::Effect, format!("read-file failed at {}: {e}", &**s)),
    },
    Some(a) => {
      let msg = format!(
        "read-file requires a string (file path), but received: {}",
        type_of(&[a.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::ReadFile).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    None => CalcitErr::err_str(CalcitErrKind::Arity, "read-file expected a filename, got nothing"),
  }
}

pub fn write_file(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match (xs.first(), xs.get(1)) {
    (Some(Calcit::Str(path)), Some(Calcit::Str(content))) => match fs::write(&**path, &**content) {
      Ok(_) => Ok(Calcit::Nil),
      Err(e) => CalcitErr::err_str(CalcitErrKind::Effect, format!("write-file failed, {e}")),
    },
    (Some(a), Some(b)) => {
      let msg = format!(
        "write-file requires 2 strings (path and content), but received: {} and {}",
        type_of(&[a.to_owned()])?.lisp_str(),
        type_of(&[b.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::WriteFile).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    (a, b) => {
      let msg = format!(
        "write-file requires 2 arguments (path and content), but received: {} arguments",
        if a.is_none() {
          0
        } else if b.is_none() {
          1
        } else {
          2
        }
      );
      let hint = format_proc_examples_hint(&CalcitProc::WriteFile).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Arity, msg, hint)
    }
  }
}
