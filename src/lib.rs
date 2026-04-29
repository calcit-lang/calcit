pub mod data;

pub mod builtins;
pub mod calcit;
pub mod call_stack;
pub mod call_tree;
pub mod cli_args;
pub mod codegen;
pub mod detailed_snapshot;
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
use std::sync::atomic::{AtomicBool, Ordering};

pub use calcit::{
  Calcit, CalcitErr, CalcitFnTypeAnnotation, CalcitProc, CalcitSyntax, CalcitTypeAnnotation, ProcTypeSignature, SyntaxTypeSignature,
};

use crate::util::string::strip_shebang;

static QUIET_TOOL_OUTPUT: AtomicBool = AtomicBool::new(false);

pub fn set_quiet_tool_output(v: bool) {
  QUIET_TOOL_OUTPUT.store(v, Ordering::Relaxed);
}

pub fn quiet_tool_output() -> bool {
  QUIET_TOOL_OUTPUT.load(Ordering::Relaxed)
}

pub fn load_core_snapshot() -> Result<snapshot::Snapshot, String> {
  // load core libs
  let bytes = include_bytes!(concat!(env!("OUT_DIR"), "/calcit-core.rmp"));
  let mut snapshot = snapshot::decode_binary_snapshot(bytes).map_err(|e| {
    eprintln!("\n{e}");
    "Failed to deserialize core snapshot".to_string()
  })?;
  let path = "calcit-internal://calcit-core.cirru";
  let meta_ns = format!("{}.$meta", snapshot.package);
  snapshot.files.insert(meta_ns.to_owned(), snapshot::gen_meta_ns(&meta_ns, path));
  Ok(snapshot)
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

  // Pre-compile all defs that contain `bind-type` calls so that global type
  // slots (e.g. `*dispatch-op`) are populated before any component or utility
  // def is compiled.  Without this, component defs that are compiled as
  // transitive deps of the entry point may see an unbound type slot and
  // produce structurally different (non-deterministic) JS output.
  match runner::preprocess::precompile_bind_type_defs(&check_warnings, &CallStackList::default()) {
    Ok(()) => {}
    Err(failure) => {
      eprintln!("\nfailed preprocessing bind-type defs, {failure}");
      let headline = failure.headline();
      call_stack::display_stack_with_docs(&headline, &failure.stack, failure.location.as_ref(), failure.hint.as_deref())?;
      return CalcitErr::err_str(failure.kind, headline);
    }
  };

  match runner::preprocess::ensure_ns_def_compiled(&init_ns, &init_def, &check_warnings, &CallStackList::default()) {
    Ok(()) => {}
    Err(failure) => {
      eprintln!("\nfailed preprocessing, {failure}");
      let headline = failure.headline();
      call_stack::display_stack_with_docs(&headline, &failure.stack, failure.location.as_ref(), failure.hint.as_deref())?;
      return CalcitErr::err_str(failure.kind, headline);
    }
  };

  let warnings = check_warnings.borrow();
  if !warnings.is_empty() {
    return Err(CalcitErr {
      kind: CalcitErrKind::Unexpected,
      msg: format!("Found {} warnings, runner blocked", warnings.len()),
      code: None,
      warnings: Box::new(warnings.to_owned()),
      stack: CallStackList::default(),
      location: None,
      hint: None,
    });
  }

  match runner::evaluate_symbol_from_program(&init_def, &init_ns, None, &CallStackList::default()) {
    Ok(entry) => match entry {
      Calcit::Fn { info, .. } => {
        let result = runner::run_fn(params, &info, &CallStackList::default());
        match result {
          Ok(v) => Ok(v),
          Err(failure) => {
            call_stack::display_stack_with_docs(&failure.msg, &failure.stack, failure.location.as_ref(), failure.hint.as_deref())?;
            Err(failure)
          }
        }
      }
      _ => CalcitErr::err_str(CalcitErrKind::Type, format!("expected function entry, got: {entry}")),
    },
    Err(failure) => {
      call_stack::display_stack_with_docs(&failure.msg, &failure.stack, failure.location.as_ref(), failure.hint.as_deref())?;
      Err(failure)
    }
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

  let display_path = if file_path.starts_with("./") {
    file_path.clone()
  } else if file_path.starts_with('/') {
    if let Ok(stripped) = Path::new(&file_path).strip_prefix(module_folder) {
      format!("<mods>/{}", stripped.display())
    } else {
      file_path.clone()
    }
  } else {
    format!("<mods>/{file_path}")
  };

  if !quiet_tool_output() {
    println!("loading: {display_path}");
  }

  let mut content = fs::read_to_string(&fullpath).unwrap_or_else(|_| panic!("expected Cirru snapshot {fullpath:?}"));
  strip_shebang(&mut content);
  let data = cirru_edn::parse(&content).map_err(|e| {
    eprintln!("\nFailed to parse file '{}':", fullpath.display());
    eprintln!("{e}");
    format!("Failed to parse file '{}'", fullpath.display())
  })?;
  // println!("reading: {}", content);
  let snapshot = snapshot::load_snapshot_data(&data, &fullpath.display().to_string())?;
  Ok(snapshot)
}
