#[macro_use]
extern crate lazy_static;

pub mod data;

pub mod builtins;
pub mod call_stack;
pub mod cli_args;
pub mod codegen;
pub mod primes;
pub mod program;
pub mod runner;
pub mod snapshot;
pub mod util;

use dirs::home_dir;
use std::cell::RefCell;
use std::fs;
use std::path::Path;

pub use primes::{Calcit, CalcitErr, CalcitItems};

pub fn load_core_snapshot() -> Result<snapshot::Snapshot, String> {
  // load core libs
  let bytes = include_bytes!("./cirru/calcit-core.cirru");
  let core_content = String::from_utf8_lossy(bytes).to_string();
  let core_data = cirru_edn::parse(&core_content)?;
  snapshot::load_snapshot_data(core_data, "calcit-internal://calcit-core.cirru")
}

pub fn run_program(init_fn: &str, params: CalcitItems) -> Result<Calcit, CalcitErr> {
  let (init_ns, init_def) = util::string::extract_ns_def(init_fn).map_err(CalcitErr::use_str)?;

  let check_warnings: &RefCell<Vec<String>> = &RefCell::new(vec![]);

  // preprocess to init
  match runner::preprocess::preprocess_ns_def(&init_ns, &init_def, &init_def, None, check_warnings, &rpds::VectorSync::new_sync()) {
    Ok(_) => (),
    Err(failure) => {
      println!("\nfailed preprocessing, {}", failure);
      call_stack::display_stack(&failure.msg, &failure.stack).map_err(CalcitErr::use_str)?;
      return CalcitErr::err_str(failure.msg);
    }
  }

  let warnings = check_warnings.to_owned().into_inner();
  if !warnings.is_empty() {
    return Err(CalcitErr {
      msg: format!("Found {} warnings, runner blocked", warnings.len()),
      warnings: warnings.to_owned(),
      stack: rpds::vector_sync![],
    });
  }
  match program::lookup_evaled_def(&init_ns, &init_def) {
    None => CalcitErr::err_str(format!("entry not initialized: {}/{}", init_ns, init_def)),
    Some(entry) => match entry {
      Calcit::Fn(_, f_ns, _, def_scope, args, body) => {
        let result = runner::run_fn(&params, &def_scope, &args, &body, &f_ns, &rpds::VectorSync::new_sync());
        match result {
          Ok(v) => Ok(v),
          Err(failure) => {
            println!("\nfailed, {}", failure);
            call_stack::display_stack(&failure.msg, &failure.stack).map_err(CalcitErr::use_str)?;
            Err(failure)
          }
        }
      }
      _ => CalcitErr::err_str(format!("expected function entry, got: {}", entry)),
    },
  }
}

pub fn load_module(path: &str, base_dir: &Path) -> Result<snapshot::Snapshot, String> {
  let mut file_path = String::from(path);
  if file_path.ends_with('/') {
    file_path.push_str("compact.cirru");
  }

  let fullpath: String = if file_path.starts_with("./") {
    let new_path = base_dir.join(file_path);
    new_path.to_str().unwrap().to_string()
  } else if file_path.starts_with('/') {
    file_path
  } else {
    match home_dir() {
      Some(buf) => {
        let home = buf.as_path();
        let p = home.join(".config/calcit/modules/").join(file_path);
        p.to_str().unwrap().to_string()
      }
      None => return Err(String::from("failed to load $HOME")),
    }
  };

  println!("loading module: {}", fullpath);

  let content = fs::read_to_string(&fullpath).unwrap_or_else(|_| panic!("expected Cirru snapshot {:?}", fullpath));
  let data = cirru_edn::parse(&content)?;
  // println!("reading: {}", content);
  let snapshot = snapshot::load_snapshot_data(data, &fullpath)?;
  Ok(snapshot)
}
