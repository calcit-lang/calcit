use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use colored::Colorize;

use crate::calcit::LocatedWarning;
use crate::call_stack::CallStackList;
use crate::codegen::emit_wasm::WasmTarget;
use crate::util::string::strip_shebang;
use crate::{ProgramEntries, call_stack, codegen, program, runner, snapshot, util};

#[derive(Clone, Debug)]
pub struct WasmCliOptions {
  pub input: String,
  pub emit_path: String,
  pub init_fn: Option<String>,
  pub reload_fn: Option<String>,
  pub entry: Option<String>,
  pub check_only: bool,
}

pub fn run(options: &WasmCliOptions, target: WasmTarget) -> Result<(), String> {
  let core_snapshot = crate::load_core_snapshot()?;

  let input_path = PathBuf::from(&options.input);
  crate::validate_snapshot_path(&input_path)?;
  let input_path_str = input_path.to_string_lossy().to_string();
  if !input_path.exists() {
    return Err(format!("{} does not exist", input_path.display()));
  }

  let mut content = fs::read_to_string(&input_path).unwrap_or_else(|_| panic!("expected Cirru snapshot: {}", input_path.display()));
  strip_shebang(&mut content);
  let data = cirru_edn::parse(&content).map_err(|e| {
    eprintln!("\nFailed to parse entry file '{}':", input_path.display());
    eprintln!("{e}");
    format!("Failed to parse entry file '{}'", input_path.display())
  })?;
  let mut snapshot = snapshot::load_snapshot_data(&data, &input_path_str)?;
  let project_namespaces: HashSet<String> = snapshot.files.keys().cloned().collect();

  snapshot.select_entry(options.entry.as_deref())?;
  if options.entry.is_some() {
    println!("running entry: {}", snapshot.active_entry_name());
  }

  let base_dir = input_path.parent().expect("extract parent");
  let module_folder = crate::project_module_folder(base_dir);

  let module_paths = snapshot.active_entry()?.modules.clone();
  for module_path in &module_paths {
    let module_data = crate::load_module(module_path, base_dir, &module_folder)?;
    crate::merge_project_module_files(&mut snapshot, &module_data, module_path)?;
  }

  let selected_entry = snapshot.active_entry()?;
  let config_init = selected_entry.init_fn.to_string();
  let config_reload = selected_entry.reload_fn.to_string();
  let init_fn = options.init_fn.as_deref().unwrap_or(&config_init);
  let reload_fn = options.reload_fn.as_deref().unwrap_or(&config_reload);
  let (init_ns, init_def) = util::string::extract_ns_def(init_fn)?;
  let (reload_ns, reload_def) = util::string::extract_ns_def(reload_fn)?;
  let entries = ProgramEntries {
    init_fn: Arc::from(init_fn),
    reload_fn: Arc::from(reload_fn),
    init_def: init_def.into(),
    init_ns: init_ns.into(),
    reload_ns: reload_ns.into(),
    reload_def: reload_def.into(),
  };

  for (namespace, file) in core_snapshot.files {
    snapshot.files.entry(namespace).or_insert(file);
  }
  runner::preprocess::set_project_namespaces(&project_namespaces);

  {
    let mut program_data = program::PROGRAM_CODE_DATA.write().expect("open program data");
    *program_data = program::extract_program_data(&snapshot)?;
  }

  let check_warnings = RefCell::new(vec![]);
  runner::preprocess::ensure_ns_def_compiled(
    crate::calcit::CORE_NS,
    crate::calcit::BUILTIN_IMPLS_ENTRY,
    &check_warnings,
    &CallStackList::default(),
  )
  .map_err(|e| e.msg)?;

  if options.check_only {
    run_check_only(&entries, target)
  } else {
    run_wasm_codegen(&entries, &options.emit_path, target)
  }
}

fn run_check_only(entries: &ProgramEntries, target: WasmTarget) -> Result<(), String> {
  let started_time = Instant::now();
  let check_warnings = RefCell::new(vec![]);

  eprintln!("{}", "Check-only mode: validating code...".dimmed());

  preprocess_entry(&entries.init_ns, &entries.init_def, &entries.init_fn, "init_fn", &check_warnings)?;
  preprocess_entry(
    &entries.reload_ns,
    &entries.reload_def,
    &entries.reload_fn,
    "reload_fn",
    &check_warnings,
  )?;

  if target == WasmTarget::Wasi {
    preprocess_wasm_namespace(entries, &check_warnings)?;
    codegen::emit_wasm::validate_wasm_target(&entries.init_ns, &entries.init_def, target)?;
  }

  let warnings = check_warnings.borrow();
  if !warnings.is_empty() {
    eprintln!("\n{} ({} warnings)", "Warnings:".yellow(), warnings.len());
    LocatedWarning::print_list(&warnings);
    return Err(format!("Found {} warnings during preprocessing", warnings.len()));
  }

  let duration = Instant::now().duration_since(started_time);
  println!(
    "\n{} {}",
    "✓ Check passed".green().bold(),
    format!("({}ms)", duration.as_micros() as f64 / 1000.0).dimmed()
  );

  Ok(())
}

fn preprocess_entry(
  namespace: &str,
  definition: &str,
  display_name: &str,
  entry_kind: &str,
  check_warnings: &RefCell<Vec<LocatedWarning>>,
) -> Result<(), String> {
  match runner::preprocess::ensure_ns_def_compiled(namespace, definition, check_warnings, &CallStackList::default()) {
    Ok(_) => {
      println!("  {} {}", "✓".green(), format!("{display_name} preprocessed").dimmed());
      Ok(())
    }
    Err(failure) => {
      eprintln!("\n{} preprocessing {entry_kind}", "✗".red());
      let headline = failure.headline();
      call_stack::display_stack_with_docs(&headline, &failure.stack, failure.location.as_ref(), failure.hint.as_deref())?;
      Err(headline)
    }
  }
}

fn run_wasm_codegen(entries: &ProgramEntries, emit_path: &str, target: WasmTarget) -> Result<(), String> {
  let started_time = Instant::now();
  codegen::set_codegen_mode(true);

  let check_warnings = RefCell::new(vec![]);
  preprocess_wasm_namespace(entries, &check_warnings)?;
  codegen::emit_wasm::emit_wasm(&entries.init_ns, &entries.init_def, emit_path, target)?;

  let duration = Instant::now().duration_since(started_time);
  println!("{}", format!("took {}ms", duration.as_micros() as f64 / 1000.0).dimmed());
  Ok(())
}

fn preprocess_wasm_namespace(entries: &ProgramEntries, check_warnings: &RefCell<Vec<LocatedWarning>>) -> Result<(), String> {
  // WASM codegen exports every compilable function, so preprocess all defs in target namespace.
  let all_defs = program::list_source_def_names(&entries.init_ns);
  for def_name in &all_defs {
    if let Err(failure) =
      runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, def_name, check_warnings, &CallStackList::default())
    {
      let headline = failure.headline();
      call_stack::display_stack_with_docs(&headline, &failure.stack, failure.location.as_ref(), failure.hint.as_deref())?;
      return Err(format!(
        "WASM preprocessing failed for {}/{}: {headline}",
        entries.init_ns, def_name
      ));
    }
  }
  Ok(())
}
