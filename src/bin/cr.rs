use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

#[cfg(not(target_arch = "wasm32"))]
mod injection;

use calcit::calcit::LocatedWarning;
use calcit::snapshot::ChangesDict;
use calcit::util::string::strip_shebang;
use dirs::home_dir;
use im_ternary_tree::TernaryTreeList;
use notify::RecursiveMode;
use notify_debouncer_mini::new_debouncer;

use calcit::{
  builtins, call_stack, cli_args, codegen, codegen::emit_js::gen_stack, codegen::COMPILE_ERRORS_FILE, program, runner, snapshot, util,
  ProgramEntries,
};

#[derive(Debug, Clone)]
pub struct CLIOptions {
  entry_path: PathBuf,
  emit_path: String,
  reload_libs: bool,
  emit_js: bool,
  emit_ir: bool,
}

fn main() -> Result<(), String> {
  builtins::effects::init_effects_states();

  // get dirty functions injected
  #[cfg(not(target_arch = "wasm32"))]
  injection::inject_platform_apis();

  let cli_matches = cli_args::parse_cli();
  let cli_options = CLIOptions {
    // has default value
    entry_path: Path::new(cli_matches.value_of("input").expect("input file")).to_owned(),
    emit_path: cli_matches.value_of("emit-path").unwrap_or("js-out").to_owned(),
    reload_libs: cli_matches.is_present("reload-libs"),
    emit_js: cli_matches.is_present("emit-js"),
    emit_ir: cli_matches.is_present("emit-ir"),
  };
  let mut eval_once = cli_matches.is_present("once");
  let assets_watch = cli_matches.value_of("watch-dir");

  println!("calcit version: {}", cli_args::CALCIT_VERSION);

  let core_snapshot = calcit::load_core_snapshot()?;

  let mut snapshot = snapshot::Snapshot::default(); // placeholder data

  let module_folder = home_dir()
    .map(|buf| buf.as_path().join(".config/calcit/modules/"))
    .expect("failed to load $HOME");
  println!("module folder: {}", module_folder.to_str().expect("extract path"));

  let base_dir = cli_options.entry_path.parent().expect("extract parent");

  if let Some(snippet) = cli_matches.value_of("eval") {
    eval_once = true;
    match snapshot::create_file_from_snippet(snippet) {
      Ok(main_file) => {
        snapshot.files.insert(String::from("app.main").into(), main_file);
      }
      Err(e) => return Err(e),
    }
    if let Some(cli_deps) = cli_matches.values_of("dep") {
      for module_path in cli_deps {
        let module_data = calcit::load_module(module_path, base_dir, &module_folder)?;
        for (k, v) in &module_data.files {
          snapshot.files.insert(k.to_owned(), v.to_owned());
        }
      }
    }
  } else {
    if !Path::new(&cli_options.entry_path).exists() {
      return Err("compact.cirru does not exist".to_owned());
    }
    // load entry file
    let mut content =
      fs::read_to_string(&cli_options.entry_path).unwrap_or_else(|_| panic!("expected Cirru snapshot: {:?}", cli_options.entry_path));
    strip_shebang(&mut content);
    let data = cirru_edn::parse(&content)?;
    // println!("reading: {}", content);
    snapshot = snapshot::load_snapshot_data(&data, cli_options.entry_path.to_str().expect("extract path"))?;

    // config in entry will overwrite default configs
    if let Some(entry) = cli_matches.value_of("entry") {
      if snapshot.entries.contains_key(entry) {
        println!("running entry: {entry}");
        snapshot.configs = snapshot.entries[entry].to_owned();
      } else {
        return Err(format!(
          "unknown entry `{}` in `{}`",
          entry,
          snapshot.entries.keys().map(|x| (*x).to_owned()).collect::<Vec<_>>().join("/")
        ));
      }
    }

    // attach modules
    for module_path in &snapshot.configs.modules {
      let module_data = calcit::load_module(module_path, base_dir, &module_folder)?;
      for (k, v) in &module_data.files {
        snapshot.files.insert(k.to_owned(), v.to_owned());
      }
    }
  }
  let init_fn = cli_matches.value_of("init-fn").unwrap_or(&snapshot.configs.init_fn);
  let reload_fn = cli_matches.value_of("reload-fn").unwrap_or(&snapshot.configs.reload_fn);
  let (init_ns, init_def) = util::string::extract_ns_def(init_fn)?;
  let (reload_ns, reload_def) = util::string::extract_ns_def(reload_fn)?;
  let entries: ProgramEntries = ProgramEntries {
    init_fn: init_fn.into(),
    reload_fn: reload_fn.into(),
    init_def: init_def.into(),
    init_ns: init_ns.into(),
    reload_ns: reload_ns.into(),
    reload_def: reload_def.into(),
  };

  // attach core
  for (k, v) in core_snapshot.files {
    snapshot.files.insert(k.to_owned(), v.to_owned());
  }

  // now global states
  {
    let mut prgm = { program::PROGRAM_CODE_DATA.write().expect("open program data") };
    *prgm = program::extract_program_data(&snapshot)?;
  }

  let check_warnings: &RefCell<Vec<LocatedWarning>> = &RefCell::new(vec![]);

  // make sure builtin classes are touched
  runner::preprocess::preprocess_ns_def(
    calcit::calcit::CORE_NS,
    calcit::calcit::BUILTIN_CLASSES_ENTRY,
    calcit::calcit::BUILTIN_CLASSES_ENTRY,
    None,
    check_warnings,
    &rpds::List::new_sync(),
  )
  .map_err(|e| e.msg)?;

  let task = if cli_options.emit_js {
    run_codegen(&entries, &cli_options.emit_path, false)
  } else if cli_options.emit_ir {
    run_codegen(&entries, &cli_options.emit_path, true)
  } else {
    let started_time = Instant::now();

    let v = calcit::run_program(entries.init_ns.to_owned(), entries.init_def.to_owned(), TernaryTreeList::Empty).map_err(|e| {
      LocatedWarning::print_list(&e.warnings);
      e.msg
    })?;

    let duration = Instant::now().duration_since(started_time);
    println!("took {}ms: {v}", duration.as_micros() as f64 / 1000.0);
    Ok(())
  };

  if eval_once {
    task?;
  } else {
    // error are only printed in watch mode
    match task {
      Ok(_) => {}
      Err(e) => {
        eprintln!("\nfailed to run, {e}");
      }
    }
  }

  if !eval_once {
    runner::track::track_task_add();
    let copied_assets = Arc::new(assets_watch.map(|s| s.to_owned()));
    let copied_settings = Arc::new(cli_options);
    let copied_entries = Arc::new(entries);
    std::thread::spawn(move || watch_files(copied_entries, copied_settings, copied_assets));
  }
  runner::track::exit_when_cleared();
  Ok(())
}

pub fn watch_files(entries: Arc<ProgramEntries>, settings: Arc<CLIOptions>, assets_watch: Arc<Option<String>>) {
  println!("\nRunning: in watch mode...\n");
  let (tx, rx) = channel();
  let mut debouncer = new_debouncer(Duration::from_millis(200), tx).expect("create watcher");
  let config = notify::Config::default();
  debouncer
    .watcher()
    .configure(config.with_compare_contents(true))
    .expect("config watcher");

  let inc_path = settings.entry_path.parent().expect("extract parent").join(".compact-inc.cirru");
  if !inc_path.exists() {
    if let Err(e) = fs::write(&inc_path, "").map_err(|e| -> String { e.to_string() }) {
      eprintln!("file writing error: {e}");
    };
  }

  debouncer.watcher().watch(&inc_path, RecursiveMode::NonRecursive).expect("watch");

  if let Some(assets_folder) = assets_watch.as_ref() {
    match debouncer.watcher().watch(Path::new(assets_folder), RecursiveMode::Recursive) {
      Ok(_) => {
        println!("assets to watch: {assets_folder}");
      }
      Err(e) => println!("failed to watch path `{assets_folder}`: {e}"),
    }
  };

  loop {
    match rx.recv() {
      Ok(Ok(_event)) => {
        // load new program code
        let mut content = fs::read_to_string(&inc_path).expect("reading inc file");
        strip_shebang(&mut content);
        if content.trim().is_empty() {
          eprintln!("failed re-compiling, got empty inc file");
          continue;
        }
        if let Err(e) = recall_program(&content, &entries, &settings) {
          eprintln!("error: {e}");
        };
      }
      Ok(Err(e)) => println!("watch error: {e:?}"),
      Err(e) => eprintln!("watch error: {e:?}"),
    }
  }
}

// overwrite previous state

fn recall_program(content: &str, entries: &ProgramEntries, settings: &CLIOptions) -> Result<(), String> {
  println!("\n-------- file change --------\n");

  // Steps:
  // 1. load changes file, and patch to program_code
  // 2. clears evaled states, gensym counter
  // 3. rerun program, and catch error

  let data = cirru_edn::parse(content)?;
  // println!("\ndata: {}", &data);
  let changes: ChangesDict = data.try_into()?;
  // println!("\nchanges: {:?}", changes);
  program::apply_code_changes(&changes)?;
  // println!("\nprogram code: {:?}", new_code);

  // clear data in evaled states
  program::clear_all_program_evaled_defs(entries.init_ns.to_owned(), entries.reload_ns.to_owned(), settings.reload_libs)?;
  builtins::meta::force_reset_gensym_index()?;

  let task = if settings.emit_js {
    run_codegen(entries, &settings.emit_path, false)
  } else if settings.emit_ir {
    run_codegen(entries, &settings.emit_path, true)
  } else {
    // run from `reload_fn` after reload
    let started_time = Instant::now();
    let task_size = runner::track::count_pending_tasks();
    println!("checking pending tasks: {task_size}");
    if task_size > 1 {
      // when there's services, make sure their code get preprocessed too
      let check_warnings: &RefCell<Vec<LocatedWarning>> = &RefCell::new(vec![]);
      if let Err(e) = runner::preprocess::preprocess_ns_def(
        &entries.init_ns,
        &entries.init_def,
        &entries.init_def,
        None,
        check_warnings,
        &rpds::List::new_sync(),
      ) {
        return Err(e.to_string());
      }

      let warnings = check_warnings.borrow();
      throw_on_warnings(&warnings)?;
    }
    let v = calcit::run_program(entries.reload_ns.to_owned(), entries.reload_def.to_owned(), TernaryTreeList::Empty).map_err(|e| {
      LocatedWarning::print_list(&e.warnings);
      e.msg
    })?;
    let duration = Instant::now().duration_since(started_time);
    println!("took {}ms: {v}", duration.as_micros() as f64 / 1000.0);
    Ok(())
  };

  match task {
    Ok(_) => {}
    Err(e) => {
      eprintln!("\nfailed to reload, {e}")
    }
  }

  Ok(())
}

fn run_codegen(entries: &ProgramEntries, emit_path: &str, ir_mode: bool) -> Result<(), String> {
  let started_time = Instant::now();

  if ir_mode {
    builtins::effects::modify_cli_running_mode(builtins::effects::CliRunningMode::Ir)?;
  } else {
    builtins::effects::modify_cli_running_mode(builtins::effects::CliRunningMode::Js)?;
  }

  let code_emit_path = Path::new(emit_path);
  if !code_emit_path.exists() {
    let _ = fs::create_dir(code_emit_path);
  }

  let js_file_path = code_emit_path.join(format!("{COMPILE_ERRORS_FILE}.mjs"));

  let check_warnings: &RefCell<Vec<LocatedWarning>> = &RefCell::new(vec![]);
  gen_stack::clear_stack();

  // preprocess to init
  match runner::preprocess::preprocess_ns_def(
    &entries.init_ns,
    &entries.init_def,
    &entries.init_def,
    None,
    check_warnings,
    &rpds::List::new_sync(),
  ) {
    Ok(_) => (),
    Err(failure) => {
      eprintln!("\nfailed preprocessing, {failure}");
      call_stack::display_stack(&failure.msg, &failure.stack, failure.location.as_ref())?;

      let _ = fs::write(
        &js_file_path,
        format!(
          "export default \"Preprocessing failed:\\n{}\";",
          failure.msg.trim().escape_default()
        ),
      );
      return Err(failure.msg);
    }
  }

  // preprocess to reload
  match runner::preprocess::preprocess_ns_def(
    &entries.reload_ns,
    &entries.reload_def,
    &entries.init_def,
    None,
    check_warnings,
    &rpds::List::new_sync(),
  ) {
    Ok(_) => (),
    Err(failure) => {
      eprintln!("\nfailed preprocessing, {failure}");
      call_stack::display_stack(&failure.msg, &failure.stack, failure.location.as_ref())?;
      return Err(failure.msg);
    }
  }

  let warnings = check_warnings.borrow();
  throw_on_js_warnings(&warnings, &js_file_path)?;

  // clear if there are no errors
  let no_error_code = String::from("export default null;");
  if !(js_file_path.exists() && fs::read_to_string(&js_file_path).map_err(|e| e.to_string())? == no_error_code) {
    let _ = fs::write(&js_file_path, no_error_code);
  }

  if ir_mode {
    match codegen::gen_ir::emit_ir(&entries.init_fn, &entries.reload_fn, emit_path) {
      Ok(_) => (),
      Err(failure) => {
        eprintln!("\nfailed codegen, {failure}");
        call_stack::display_stack(&failure, &gen_stack::get_gen_stack(), None)?;
        return Err(failure);
      }
    }
  } else {
    // TODO entry ns
    match codegen::emit_js::emit_js(&entries.init_ns, emit_path) {
      Ok(_) => (),
      Err(failure) => {
        eprintln!("\nfailed codegen, {failure}");
        call_stack::display_stack(&failure, &gen_stack::get_gen_stack(), None)?;
        return Err(failure);
      }
    }
  }
  let duration = Instant::now().duration_since(started_time);
  println!("took {}ms", duration.as_micros() as f64 / 1000.0);
  Ok(())
}

fn throw_on_js_warnings(warnings: &[LocatedWarning], js_file_path: &Path) -> Result<(), String> {
  if !warnings.is_empty() {
    let mut content: String = String::from("");
    for warn in warnings {
      println!("{warn}");
      content = format!("{content}\n{warn}");
    }

    let _ = fs::write(js_file_path, format!("export default \"{}\";", content.trim().escape_default()));
    Err(format!(
      "Found {} warnings, codegen blocked. errors in {}.mjs",
      warnings.len(),
      COMPILE_ERRORS_FILE,
    ))
  } else {
    Ok(())
  }
}

fn throw_on_warnings(warnings: &[LocatedWarning]) -> Result<(), String> {
  if !warnings.is_empty() {
    let mut content: String = String::from("");
    for warn in warnings {
      println!("{warn}");
      content = format!("{content}\n{warn}");
    }

    Err(format!("Found {} warnings in preprocessing, re-run blocked.", warnings.len()))
  } else {
    Ok(())
  }
}
