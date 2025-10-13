pub mod data;

pub mod builtins;
pub mod calcit;
pub mod call_stack;
pub mod cli_args;
pub mod codegen;
pub mod detailed_snapshot;
pub mod mcp;
pub mod program;
pub mod runner;
pub mod snapshot;
pub mod util;

use calcit::{CalcitErrKind, LocatedWarning};
use call_stack::CallStackList;
use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::sync::Arc;

pub use calcit::{Calcit, CalcitErr};

use crate::util::string::strip_shebang;

pub fn load_core_snapshot() -> Result<snapshot::Snapshot, String> {
  // load core libs
  let bytes = include_bytes!("./cirru/calcit-core.cirru");
  let core_content = String::from_utf8_lossy(bytes).to_string();
  let core_data = cirru_edn::parse(&core_content)?;
  snapshot::load_snapshot_data(&core_data, "calcit-internal://calcit-core.cirru")
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

pub fn run_program(init_ns: Arc<str>, init_def: Arc<str>, params: &[Calcit]) -> Result<Calcit, CalcitErr> {
  run_program_with_docs(init_ns, init_def, params)
}

pub fn run_program_with_docs(init_ns: Arc<str>, init_def: Arc<str>, params: &[Calcit]) -> Result<Calcit, CalcitErr> {
  let check_warnings = RefCell::new(LocatedWarning::default_list());

  // preprocess to init
  match runner::preprocess::preprocess_ns_def(&init_ns, &init_def, &check_warnings, &CallStackList::default()) {
    Ok(_) => (),
    Err(failure) => {
      eprintln!("\nfailed preprocessing, {failure}");
      call_stack::display_stack_with_docs(&failure.msg, &failure.stack, failure.location.as_ref())?;
      return CalcitErr::err_str(failure.kind, failure.msg);
    }
  }

  let warnings = check_warnings.borrow();
  if !warnings.is_empty() {
    return Err(CalcitErr {
      kind: CalcitErrKind::Unexpected,
      msg: format!("Found {} warnings, runner blocked", warnings.len()),
      warnings: warnings.to_owned(),
      stack: CallStackList::default(),
      location: None,
    });
  }
  match program::lookup_evaled_def(&init_ns, &init_def) {
    None => CalcitErr::err_str(CalcitErrKind::Var, format!("entry not initialized: {init_ns}/{init_def}")),
    Some(entry) => match entry {
      Calcit::Fn { info, .. } => {
        let result = runner::run_fn(params, &info, &CallStackList::default());
        match result {
          Ok(v) => Ok(v),
          Err(failure) => {
            call_stack::display_stack_with_docs(&failure.msg, &failure.stack, failure.location.as_ref())?;
            Err(failure)
          }
        }
      }
      _ => CalcitErr::err_str(CalcitErrKind::Type, format!("expected function entry, got: {entry}")),
    },
  }
}

pub fn load_module(path: &str, base_dir: &Path, module_folder: &Path) -> Result<snapshot::Snapshot, String> {
  let mut file_path = String::from(path);
  if file_path.ends_with('/') {
    file_path.push_str("compact.cirru");
  }

  let fullpath = if file_path.starts_with("./") {
    base_dir.join(&file_path).as_path().to_owned()
  } else if file_path.starts_with('/') {
    Path::new(&file_path).to_owned()
  } else {
    module_folder.join(&file_path).as_path().to_owned()
  };

  println!("loading: {}", file_path.as_str());

  let mut content = fs::read_to_string(&fullpath).unwrap_or_else(|_| panic!("expected Cirru snapshot {fullpath:?}"));
  strip_shebang(&mut content);
  let data = cirru_edn::parse(&content)?;
  // println!("reading: {}", content);
  let snapshot = snapshot::load_snapshot_data(&data, &fullpath.display().to_string())?;
  Ok(snapshot)
}
