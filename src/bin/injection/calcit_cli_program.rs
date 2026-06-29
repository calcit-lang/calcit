//! Load a Calcit snapshot with modules and core, then preprocess for analyze/* builtins.

use calcit::calcit::LocatedWarning;
use calcit::call_stack::CallStackList;
use calcit::util::string::strip_shebang;
use calcit::{ProgramEntries, load_core_snapshot, load_module, program, runner, snapshot, util};
use dirs::home_dir;
use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use calcit::calcit::CalcitErr;

pub(crate) fn prepare_program_from_snapshot_file(file_path: &str) -> Result<ProgramEntries, CalcitErr> {
  let input_path = calcit::resolve_snapshot_path_alias(&PathBuf::from(file_path));
  if !input_path.exists() {
    return Err(CalcitErr::from(format!(
      "prepare-program: file not found: {}",
      input_path.display()
    )));
  }

  let input_path_str = input_path.to_string_lossy().to_string();
  let mut content = fs::read_to_string(&input_path)
    .map_err(|e| CalcitErr::from(format!("prepare-program: failed to read `{}`: {e}", input_path.display())))?;
  strip_shebang(&mut content);
  let data = cirru_edn::parse(&content)
    .map_err(|e| CalcitErr::from(format!("prepare-program: failed to parse `{}`: {e}", input_path.display())))?;
  let mut snap = snapshot::load_snapshot_data(&data, &input_path_str).map_err(|e| CalcitErr::from(format!("prepare-program: {e}")))?;

  let base_dir = input_path.parent().unwrap_or_else(|| Path::new("."));
  let module_folder = home_dir()
    .map(|buf| buf.as_path().join(".config/calcit/modules/"))
    .ok_or_else(|| CalcitErr::from("prepare-program: failed to resolve $HOME for module lookup".to_string()))?;

  for module_path in &snap.configs.modules.clone() {
    let module_data =
      load_module(module_path, base_dir, &module_folder).map_err(|e| CalcitErr::from(format!("prepare-program: {e}")))?;
    for (k, v) in &module_data.files {
      if snap.files.contains_key(k) {
        return Err(CalcitErr::from(format!(
          "prepare-program: namespace `{k}` already exists when loading module `{module_path}`"
        )));
      }
      snap.files.insert(k.to_owned(), v.to_owned());
    }
  }

  let core_snapshot = load_core_snapshot().map_err(|e| CalcitErr::from(format!("prepare-program: {e}")))?;
  for (k, v) in core_snapshot.files {
    snap.files.insert(k.to_owned(), v.to_owned());
  }

  let init_fn = snap.configs.init_fn.to_string();
  let reload_fn = snap.configs.reload_fn.to_string();
  let (init_ns, init_def) = util::string::extract_ns_def(&init_fn).map_err(CalcitErr::from)?;
  let (reload_ns, reload_def) = util::string::extract_ns_def(&reload_fn).map_err(CalcitErr::from)?;

  let entries = ProgramEntries {
    init_fn: Arc::from(init_fn.as_str()),
    reload_fn: Arc::from(reload_fn.as_str()),
    init_def: init_def.into(),
    init_ns: init_ns.into(),
    reload_ns: reload_ns.into(),
    reload_def: reload_def.into(),
  };

  {
    let mut prgm = program::PROGRAM_CODE_DATA.write().expect("open program data");
    *prgm = program::extract_program_data(&snap).map_err(CalcitErr::from)?;
  }

  let check_warnings: &RefCell<Vec<LocatedWarning>> = &RefCell::new(vec![]);
  runner::preprocess::ensure_ns_def_compiled(
    calcit::calcit::CORE_NS,
    calcit::calcit::BUILTIN_IMPLS_ENTRY,
    check_warnings,
    &CallStackList::default(),
  )
  .map_err(|e| CalcitErr::from(e.msg))?;
  runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, check_warnings, &CallStackList::default())
    .map_err(|e| CalcitErr::from(e.msg))?;

  Ok(entries)
}

/// Check-only validation: preprocess init_fn and reload_fn; returns `"ok"` or error message.
pub(crate) fn validate_snapshot_file(file_path: &str) -> Result<String, CalcitErr> {
  let entries = prepare_program_from_snapshot_file(file_path)?;
  let check_warnings: &RefCell<Vec<LocatedWarning>> = &RefCell::new(vec![]);
  let stack = &CallStackList::default();

  runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, check_warnings, stack)
    .map_err(|e| CalcitErr::from(e.msg))?;
  runner::preprocess::ensure_ns_def_compiled(&entries.reload_ns, &entries.reload_def, check_warnings, stack)
    .map_err(|e| CalcitErr::from(e.msg))?;

  let warnings = check_warnings.borrow();
  if !warnings.is_empty() {
    return Err(CalcitErr::from(format!(
      "validate-file: found {} warning(s) during preprocessing",
      warnings.len()
    )));
  }
  Ok("ok".to_string())
}
