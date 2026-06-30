use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

#[cfg(not(target_arch = "wasm32"))]
mod injection;

#[cfg(not(target_arch = "wasm32"))]
#[path = "../type_coverage.rs"]
mod type_coverage;

use argh::FromArgs;
use calcit::calcit::LocatedWarning;
use calcit::call_stack::CallStackList;
use calcit::util::string::strip_shebang;
use calcit::{ProgramEntries, builtins, call_stack, codegen, program, runner, snapshot, util};
use colored::Colorize;
use dirs::home_dir;

#[derive(FromArgs, PartialEq, Debug, Clone)]
/// Standalone WASM codegen command.
struct WasmArgs {
  /// emit path for generated artifacts, defaults to "js-out/"
  #[argh(option, default = "String::from(\"js-out/\")")]
  emit_path: String,
  /// specify `init_fn` which is main function
  #[argh(option)]
  init_fn: Option<String>,
  /// specify `reload_fn` which is called after hot reload
  #[argh(option)]
  reload_fn: Option<String>,
  /// specify with config entry
  #[argh(option)]
  entry: Option<String>,
  /// check-only mode: validate without codegen
  #[argh(switch)]
  check_only: bool,
  /// print version only
  #[argh(switch)]
  version: bool,
  /// input source file, defaults to "calcit.cirru" and falls back to "compact.cirru"
  #[argh(positional, default = "String::from(calcit::DEFAULT_SNAPSHOT_FILE)")]
  input: String,
}

fn main() -> Result<(), String> {
  builtins::effects::init_effects_states();

  let cli_args: WasmArgs = argh::from_env();

  if cli_args.version {
    println!("{}", calcit::cli_args::CALCIT_VERSION);
    return Ok(());
  }

  #[cfg(not(target_arch = "wasm32"))]
  injection::inject_platform_apis();

  let core_snapshot = calcit::load_core_snapshot()?;

  let input_path = calcit::resolve_snapshot_path_alias(&PathBuf::from(&cli_args.input));
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

  if let Some(entry) = cli_args.entry.to_owned() {
    if snapshot.entries.contains_key(entry.as_str()) {
      println!("running entry: {entry}");
      snapshot.entries[entry.as_str()].clone_into(&mut snapshot.configs);
    } else {
      return Err(format!(
        "unknown entry `{}` in `{}`",
        entry,
        snapshot.entries.keys().map(|x| (*x).to_owned()).collect::<Vec<_>>().join("/")
      ));
    }
  }

  let base_dir = input_path.parent().expect("extract parent");
  let module_folder = home_dir()
    .map(|buf| buf.as_path().join(".config/calcit/modules/"))
    .expect("failed to load $HOME");

  for module_path in &snapshot.configs.modules {
    let module_data = calcit::load_module(module_path, base_dir, &module_folder)?;
    for (k, v) in &module_data.files {
      if snapshot.files.contains_key(k) {
        return Err(format!("namespace `{k}` already exists when loading module `{module_path}`"));
      }
      snapshot.files.insert(k.to_owned(), v.to_owned());
    }
  }

  let config_init = snapshot.configs.init_fn.to_string();
  let config_reload = snapshot.configs.reload_fn.to_string();
  let init_fn = cli_args.init_fn.as_deref().unwrap_or(&config_init);
  let reload_fn = cli_args.reload_fn.as_deref().unwrap_or(&config_reload);
  let (init_ns, init_def) = util::string::extract_ns_def(init_fn)?;
  let (reload_ns, reload_def) = util::string::extract_ns_def(reload_fn)?;
  let entries: ProgramEntries = ProgramEntries {
    init_fn: Arc::from(init_fn),
    reload_fn: Arc::from(reload_fn),
    init_def: init_def.into(),
    init_ns: init_ns.into(),
    reload_ns: reload_ns.into(),
    reload_def: reload_def.into(),
  };

  for (k, v) in core_snapshot.files {
    snapshot.files.insert(k.to_owned(), v.to_owned());
  }

  {
    let mut prgm = { program::PROGRAM_CODE_DATA.write().expect("open program data") };
    *prgm = program::extract_program_data(&snapshot)?;
  }

  let check_warnings: &RefCell<Vec<LocatedWarning>> = &RefCell::new(vec![]);
  runner::preprocess::ensure_ns_def_compiled(
    calcit::calcit::CORE_NS,
    calcit::calcit::BUILTIN_IMPLS_ENTRY,
    check_warnings,
    &CallStackList::default(),
  )
  .map_err(|e| e.msg)?;

  if cli_args.check_only {
    run_check_only(&entries)?;
    return Ok(());
  }

  run_wasm_codegen(&entries, &cli_args.emit_path)
}

fn run_check_only(entries: &ProgramEntries) -> Result<(), String> {
  let started_time = Instant::now();
  let check_warnings: &RefCell<Vec<LocatedWarning>> = &RefCell::new(vec![]);

  eprintln!("{}", "Check-only mode: validating code...".dimmed());

  match runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, check_warnings, &CallStackList::default()) {
    Ok(_) => {
      println!("  {} {}", "✓".green(), format!("{} preprocessed", entries.init_fn).dimmed());
    }
    Err(failure) => {
      eprintln!("\n{} preprocessing init_fn", "✗".red());
      let headline = failure.headline();
      call_stack::display_stack_with_docs(&headline, &failure.stack, failure.location.as_ref(), failure.hint.as_deref())?;
      return Err(headline);
    }
  }

  match runner::preprocess::ensure_ns_def_compiled(&entries.reload_ns, &entries.reload_def, check_warnings, &CallStackList::default()) {
    Ok(_) => {
      println!("  {} {}", "✓".green(), format!("{} preprocessed", entries.reload_fn).dimmed());
    }
    Err(failure) => {
      eprintln!("\n{} preprocessing reload_fn", "✗".red());
      let headline = failure.headline();
      call_stack::display_stack_with_docs(&headline, &failure.stack, failure.location.as_ref(), failure.hint.as_deref())?;
      return Err(headline);
    }
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

fn run_wasm_codegen(entries: &ProgramEntries, emit_path: &str) -> Result<(), String> {
  let started_time = Instant::now();
  codegen::set_codegen_mode(true);

  let check_warnings: &RefCell<Vec<LocatedWarning>> = &RefCell::new(vec![]);

  // WASM codegen exports every compilable function, so preprocess all defs in target namespace.
  let all_defs = program::list_source_def_names(&entries.init_ns);
  for def_name in &all_defs {
    match runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, def_name, check_warnings, &CallStackList::default()) {
      Ok(_) => (),
      Err(failure) => {
        eprintln!(
          "[wasm] preprocessing failed for {}/{}: {}",
          entries.init_ns,
          def_name,
          failure.headline()
        );
      }
    }
  }

  codegen::emit_wasm::emit_wasm(&entries.init_ns, emit_path)?;

  let duration = Instant::now().duration_since(started_time);
  println!("{}", format!("took {}ms", duration.as_micros() as f64 / 1000.0).dimmed());
  Ok(())
}
