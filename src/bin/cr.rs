use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::time::Duration;
use std::time::Instant;

#[cfg(not(target_arch = "wasm32"))]
mod injection;

use calcit::calcit::LocatedWarning;
use calcit::call_stack::CallStackList;
use calcit::cli_args::{CalcitCommand, ToplevelCalcit};
use calcit::snapshot::ChangesDict;
use calcit::util::string::strip_shebang;
use colored::Colorize;
use dirs::home_dir;
use notify::RecursiveMode;
use notify_debouncer_mini::new_debouncer;

use calcit::{
  ProgramEntries, builtins, call_stack, cli_args, codegen, codegen::COMPILE_ERRORS_FILE, codegen::emit_js::gen_stack, program, runner,
  snapshot, util,
};

fn main() -> Result<(), String> {
  builtins::effects::init_effects_states();

  let cli_args: ToplevelCalcit = argh::from_env();

  let mut eval_once = cli_args.once;
  let assets_watch = cli_args.watch_dir.to_owned();

  println!("{}", format!("calcit version: {}", cli_args::CALCIT_VERSION).dimmed());
  if cli_args.version {
    return Ok(());
  }

  // get dirty functions injected
  #[cfg(not(target_arch = "wasm32"))]
  injection::inject_platform_apis();

  let core_snapshot = calcit::load_core_snapshot()?;

  let mut snapshot = snapshot::Snapshot::default(); // placeholder data

  let module_folder = home_dir()
    .map(|buf| buf.as_path().join(".config/calcit/modules/"))
    .expect("failed to load $HOME");
  println!(
    "{}",
    format!("module folder: {}", module_folder.to_str().expect("extract path")).dimmed()
  );

  if cli_args.disable_stack {
    call_stack::set_using_stack(false);
    println!("stack trace disabled.")
  }

  let input_path = PathBuf::from(&cli_args.input);
  let base_dir = input_path.parent().expect("extract parent");

  if let Some(CalcitCommand::Eval(ref command)) = cli_args.subcommand {
    let snippet = &command.snippet;
    eval_once = true;
    match snapshot::create_file_from_snippet(snippet) {
      Ok(main_file) => {
        snapshot.files.insert(String::from("app.main"), main_file);
      }
      Err(e) => return Err(e),
    }

    for module_path in &command.dep {
      let module_data = calcit::load_module(module_path, base_dir, &module_folder)?;
      for (k, v) in &module_data.files {
        snapshot.files.insert(k.to_owned(), v.to_owned());
      }
    }
  } else {
    if !Path::new(&cli_args.input).exists() {
      return Err(format!("{} does not exist", cli_args.input));
    }
    // load entry file
    let mut content = fs::read_to_string(&cli_args.input).unwrap_or_else(|_| panic!("expected Cirru snapshot: {}", cli_args.input));
    strip_shebang(&mut content);
    let data = cirru_edn::parse(&content)?;
    // println!("reading: {}", content);
    snapshot = snapshot::load_snapshot_data(&data, &cli_args.input)?;

    // config in entry will overwrite default configs
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

    // attach modules
    for module_path in &snapshot.configs.modules {
      let module_data = calcit::load_module(module_path, base_dir, &module_folder)?;
      for (k, v) in &module_data.files {
        snapshot.files.insert(k.to_owned(), v.to_owned());
      }
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
    check_warnings,
    &CallStackList::default(),
  )
  .map_err(|e| e.msg)?;

  let task = if let Some(CalcitCommand::EmitJs(js_options)) = &cli_args.subcommand {
    if js_options.once {
      // redundant config, during watching mode, emit once
      eval_once = true;
    }
    if cli_args.skip_arity_check {
      codegen::set_code_gen_skip_arity_check(true);
    }
    run_codegen(&entries, &cli_args.emit_path, false)
  } else if let Some(CalcitCommand::EmitIr(ir_options)) = &cli_args.subcommand {
    if ir_options.once {
      // redundant config, during watching mode, emit once
      eval_once = true;
    }
    run_codegen(&entries, &cli_args.emit_path, true)
  } else {
    let started_time = Instant::now();

    let v = calcit::run_program(entries.init_ns.to_owned(), entries.init_def.to_owned(), &[]).map_err(|e| {
      LocatedWarning::print_list(&e.warnings);
      e.msg
    })?;

    let duration = Instant::now().duration_since(started_time);
    println!("{}{}", format!("took {}ms: ", duration.as_micros() as f64 / 1000.0).dimmed(), v);
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
    let args = cli_args.clone();
    std::thread::spawn(move || watch_files(entries, args, assets_watch));
  }
  runner::track::exit_when_cleared();
  Ok(())
}

pub fn watch_files(entries: ProgramEntries, settings: ToplevelCalcit, assets_watch: Option<String>) {
  println!("\nRunning: in watch mode...\n");
  let (tx, rx) = channel();
  let mut debouncer = new_debouncer(Duration::from_millis(200), tx).expect("create watcher");
  let config = notify::Config::default();
  debouncer
    .watcher()
    .configure(config.with_compare_contents(true))
    .expect("config watcher");

  let inc_path = PathBuf::from(&settings.input)
    .parent()
    .expect("extract parent")
    .join(".compact-inc.cirru");
  if !inc_path.exists()
    && let Err(e) = fs::write(&inc_path, "").map_err(|e| -> String { e.to_string() })
  {
    eprintln!("file writing error: {e}");
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

fn recall_program(content: &str, entries: &ProgramEntries, settings: &ToplevelCalcit) -> Result<(), String> {
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
  println!("cleared evaled states and reset gensym index.");

  let task = if let Some(CalcitCommand::EmitJs(_)) = settings.subcommand {
    run_codegen(entries, &settings.emit_path, false)
  } else if let Some(CalcitCommand::EmitIr(_)) = settings.subcommand {
    run_codegen(entries, &settings.emit_path, true)
  } else {
    // run from `reload_fn` after reload
    let started_time = Instant::now();
    let task_size = runner::track::count_pending_tasks();
    println!("checking pending tasks: {task_size}");
    if task_size > 1 {
      // when there's services, make sure their code get preprocessed too
      let check_warnings: &RefCell<Vec<LocatedWarning>> = &RefCell::new(vec![]);
      if let Err(e) =
        runner::preprocess::preprocess_ns_def(&entries.init_ns, &entries.init_def, check_warnings, &CallStackList::default())
      {
        return Err(e.to_string());
      }

      let warnings = check_warnings.borrow();
      throw_on_warnings(&warnings)?;
    }
    let v = calcit::run_program(entries.reload_ns.to_owned(), entries.reload_def.to_owned(), &[]).map_err(|e| {
      LocatedWarning::print_list(&e.warnings);
      e.msg
    })?;
    let duration = Instant::now().duration_since(started_time);
    println!("{}{}", format!("took {}ms: ", duration.as_micros() as f64 / 1000.0).dimmed(), v);
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
  codegen::set_codegen_mode(true);

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
  match runner::preprocess::preprocess_ns_def(&entries.init_ns, &entries.init_def, check_warnings, &CallStackList::default()) {
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
  match runner::preprocess::preprocess_ns_def(&entries.reload_ns, &entries.reload_def, check_warnings, &CallStackList::default()) {
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
  println!("{}", format!("took {}ms", duration.as_micros() as f64 / 1000.0).dimmed());
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
