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
use std::sync::Arc;

pub use primes::{Calcit, CalcitErr, CalcitItems};

pub fn load_core_snapshot() -> Result<snapshot::Snapshot, String> {
  // load core libs
  let bytes = include_bytes!("./cirru/calcit-core.cirru");
  let core_content = String::from_utf8_lossy(bytes).to_string();
  let core_data = cirru_edn::parse(&core_content)?;
  snapshot::load_snapshot_data(core_data, "calcit-internal://calcit-core.cirru")
}

#[derive(Clone, Debug)]
pub struct ProgramEntries {
  pub init_fn: Arc<str>,
  pub init_ns: Arc<str>,
  pub init_def: Arc<str>,
  pub reload_fn: Arc<str>,
  pub reload_ns: Arc<str>,
  pub reload_def: Arc<str>,
}

pub fn run_program(init_ns: Arc<str>, init_def: Arc<str>, params: CalcitItems) -> Result<Calcit, CalcitErr> {
  let check_warnings: RefCell<Vec<String>> = RefCell::new(vec![]);

  // preprocess to init
  match runner::preprocess::preprocess_ns_def(
    init_ns.to_owned(),
    init_def.to_owned(),
    init_def.to_owned(),
    None,
    &check_warnings,
    &rpds::List::new_sync(),
  ) {
    Ok(_) => (),
    Err(failure) => {
      println!("\nfailed preprocessing, {}", failure);
      call_stack::display_stack(&failure.msg, &failure.stack)?;
      return CalcitErr::err_str(failure.msg);
    }
  }

  let warnings = check_warnings.into_inner();
  if !warnings.is_empty() {
    return Err(CalcitErr {
      msg: format!("Found {} warnings, runner blocked", warnings.len()),
      warnings: warnings.to_owned(),
      stack: rpds::List::new_sync(),
    });
  }
  match program::lookup_evaled_def(&init_ns, &init_def) {
    None => CalcitErr::err_str(format!("entry not initialized: {}/{}", init_ns, init_def)),
    Some(entry) => match entry {
      Calcit::Fn {
        def_ns, scope, args, body, ..
      } => {
        let result = runner::run_fn(&params, &scope, &args, &body, def_ns, &rpds::List::new_sync());
        match result {
          Ok(v) => Ok(v),
          Err(failure) => {
            println!("\nfailed, {}", failure);
            call_stack::display_stack(&failure.msg, &failure.stack)?;
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
