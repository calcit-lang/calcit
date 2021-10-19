use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::Duration;
use std::time::Instant;

#[cfg(not(target_arch = "wasm32"))]
mod injection;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use calcit_runner::{builtins, call_stack, cli_args, codegen, program, runner, snapshot, util};

struct ProgramSettings {
  entry_path: PathBuf,
  emit_path: String,
  reload_libs: bool,
  emit_js: bool,
  emit_ir: bool,
}

pub const COMPILE_ERRORS_FILE: &str = "calcit.build-errors";

fn main() -> Result<(), String> {
  builtins::effects::init_effects_states();

  // get dirty functions injected
  #[cfg(not(target_arch = "wasm32"))]
  injection::inject_platform_apis();

  let cli_matches = cli_args::parse_cli();
  let settings = ProgramSettings {
    // has default value
    entry_path: Path::new(cli_matches.value_of("input").unwrap()).to_owned(),
    emit_path: cli_matches.value_of("emit-path").or(Some("js-out")).unwrap().to_owned(),
    reload_libs: cli_matches.is_present("reload-libs"),
    emit_js: cli_matches.is_present("emit-js"),
    emit_ir: cli_matches.is_present("emit-ir"),
  };
  let mut eval_once = cli_matches.is_present("once");
  let assets_watch = cli_matches.value_of("watch-dir");

  println!("calcit version: {}", cli_args::CALCIT_VERSION);

  let core_snapshot = calcit_runner::load_core_snapshot()?;

  let mut snapshot = snapshot::gen_default(); // placeholder data

  if let Some(snippet) = cli_matches.value_of("eval") {
    eval_once = true;
    match snapshot::create_file_from_snippet(snippet) {
      Ok(main_file) => {
        snapshot.files.insert(String::from("app.main"), main_file);
      }
      Err(e) => return Err(e),
    }
    if let Some(cli_deps) = cli_matches.values_of("dep") {
      for module_path in cli_deps {
        let module_data = calcit_runner::load_module(module_path, settings.entry_path.parent().unwrap())?;
        for (k, v) in &module_data.files {
          snapshot.files.insert(k.to_owned(), v.to_owned());
        }
      }
    }
  } else {
    // load entry file
    let content = fs::read_to_string(settings.entry_path.to_owned())
      .unwrap_or_else(|_| panic!("expected Cirru snapshot: {:?}", settings.entry_path));

    let data = cirru_edn::parse(&content)?;
    // println!("reading: {}", content);
    snapshot = snapshot::load_snapshot_data(data, settings.entry_path.to_str().unwrap())?;
    // attach modules
    for module_path in &snapshot.configs.modules {
      let module_data = calcit_runner::load_module(module_path, settings.entry_path.parent().unwrap())?;
      for (k, v) in &module_data.files {
        snapshot.files.insert(k.to_owned(), v.to_owned());
      }
    }
  }
  let init_fn = cli_matches.value_of("init-fn").or(Some(&snapshot.configs.init_fn)).unwrap();
  let reload_fn = cli_matches.value_of("reload-fn").or(Some(&snapshot.configs.reload_fn)).unwrap();

  // attach core
  for (k, v) in core_snapshot.files {
    snapshot.files.insert(k.to_owned(), v.to_owned());
  }

  // now global states
  {
    let mut prgm = { program::PROGRAM_CODE_DATA.write().unwrap() };
    *prgm = program::extract_program_data(&snapshot)?;
  }

  let check_warnings: &RefCell<Vec<String>> = &RefCell::new(vec![]);

  // make sure builtin classes are touched
  runner::preprocess::preprocess_ns_def(
    calcit_runner::primes::CORE_NS,
    calcit_runner::primes::BUILTIN_CLASSES_ENTRY,
    calcit_runner::primes::BUILTIN_CLASSES_ENTRY,
    None,
    check_warnings,
  )
  .map_err(|e| e.msg)?;

  let task = if settings.emit_js {
    run_codegen(init_fn, reload_fn, &settings.emit_path, false)
  } else if settings.emit_ir {
    run_codegen(init_fn, reload_fn, &settings.emit_path, true)
  } else {
    let started_time = Instant::now();

    let v = calcit_runner::run_program(init_fn, im::vector![]).map_err(|e| {
      for w in e.warnings {
        println!("{}", w);
      }
      e.msg
    })?;

    let duration = Instant::now().duration_since(started_time);
    println!("took {}ms: {}", duration.as_micros() as f64 / 1000.0, v);
    Ok(())
  };

  if eval_once {
    task?;
  } else {
    // error are only printed in watch mode
    match task {
      Ok(_) => {}
      Err(e) => {
        println!("\nfailed to run, {}", e);
      }
    }
  }

  if !eval_once {
    println!("\nRunner: in watch mode...\n");
    let (tx, rx) = channel();
    let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_millis(200)).unwrap();

    let inc_path = settings.entry_path.parent().unwrap().join(".compact-inc.cirru");
    if !inc_path.exists() {
      fs::write(&inc_path, "").map_err(|e| -> String { e.to_string() })?;
    }

    watcher.watch(&inc_path, RecursiveMode::NonRecursive).unwrap();

    if let Some(assets_folder) = assets_watch {
      watcher.watch(assets_folder, RecursiveMode::Recursive).unwrap();
      println!("assets to watch: {}", assets_folder);
    }

    loop {
      let mut change_happened = false;
      match rx.recv() {
        Ok(event) => {
          match event {
            notify::DebouncedEvent::NoticeWrite(..) => {
              // ignored
            }
            notify::DebouncedEvent::Write(_) => {
              // mark state dirty
              change_happened = true;
            }
            _ => println!("other file event: {:?}, ignored", event),
          }
        }
        Err(e) => println!("watch error: {:?}", e),
      }
      if change_happened {
        // load new program code
        let content = fs::read_to_string(&inc_path).unwrap();
        if content.trim() == "" {
          println!("failed re-compiling, got empty inc file");
          continue;
        }
        recall_program(&content, init_fn, reload_fn, &settings)?;
      }
    }
  } else {
    runner::track::exit_when_cleared();
    Ok(())
  }
}

// overwrite previous state

fn recall_program(content: &str, init_fn: &str, reload_fn: &str, settings: &ProgramSettings) -> Result<(), String> {
  println!("\n-------- file change --------\n");
  call_stack::clear_stack();

  // Steps:
  // 1. load changes file, and patch to program_code
  // 2. clears evaled states, gensym counter
  // 3. rerun program, and catch error

  let data = cirru_edn::parse(content)?;
  let changes = snapshot::load_changes_info(data)?;

  // println!("\ndata: {}", &data);
  // println!("\nchanges: {:?}", changes);
  program::apply_code_changes(&changes)?;
  // println!("\nprogram code: {:?}", new_code);

  // clear data in evaled states
  program::clear_all_program_evaled_defs(init_fn, reload_fn, settings.reload_libs)?;
  builtins::meta::force_reset_gensym_index()?;

  let task = if settings.emit_js {
    run_codegen(init_fn, reload_fn, &settings.emit_path, false)
  } else if settings.emit_ir {
    run_codegen(init_fn, reload_fn, &settings.emit_path, true)
  } else {
    // run from `reload_fn` after reload
    let started_time = Instant::now();
    let v = calcit_runner::run_program(reload_fn, im::vector![]).map_err(|e| {
      for w in e.warnings {
        println!("{}", w);
      }
      e.msg
    })?;
    let duration = Instant::now().duration_since(started_time);
    println!("took {}ms: {}", duration.as_micros() as f64 / 1000.0, v);
    Ok(())
  };

  match task {
    Ok(_) => {}
    Err(e) => {
      println!("\nfailed to reload, {}", e)
    }
  }

  Ok(())
}

fn run_codegen(init_fn: &str, reload_fn: &str, emit_path: &str, ir_mode: bool) -> Result<(), String> {
  let started_time = Instant::now();

  let (init_ns, init_def) = util::string::extract_ns_def(init_fn)?;
  let (reload_ns, reload_def) = util::string::extract_ns_def(reload_fn)?;

  if ir_mode {
    builtins::effects::modify_cli_running_mode(builtins::effects::CliRunningMode::Ir)?;
  } else {
    builtins::effects::modify_cli_running_mode(builtins::effects::CliRunningMode::Js)?;
  }

  let code_emit_path = Path::new(emit_path);
  if !code_emit_path.exists() {
    let _ = fs::create_dir(code_emit_path);
  }

  let js_file_path = code_emit_path.join(format!("{}.js", COMPILE_ERRORS_FILE)); // TODO mjs_mode

  let check_warnings: &RefCell<Vec<String>> = &RefCell::new(vec![]);

  // preprocess to init
  match runner::preprocess::preprocess_ns_def(&init_ns, &init_def, &init_def, None, check_warnings) {
    Ok(_) => (),
    Err(failure) => {
      println!("\nfailed preprocessing, {}", failure);
      call_stack::display_stack(&failure.msg)?;

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
  match runner::preprocess::preprocess_ns_def(&reload_ns, &reload_def, &init_def, None, check_warnings) {
    Ok(_) => (),
    Err(failure) => {
      println!("\nfailed preprocessing, {}", failure);
      call_stack::display_stack(&failure.msg)?;
      return Err(failure.msg);
    }
  }
  let warnings = check_warnings.to_owned().into_inner();
  let mut content: String = String::from("");
  if !warnings.is_empty() {
    for message in &warnings {
      println!("{}", message);
      content = format!("{}\n{}", content, message);
    }

    let _ = fs::write(&js_file_path, format!("export default \"{}\";", content.trim().escape_default()));
    return Err(format!(
      "Found {} warnings, codegen blocked. errors in {}.js",
      warnings.len(),
      COMPILE_ERRORS_FILE,
    ));
  }

  // clear if there are no errors
  let no_error_code = String::from("export default null;");
  if !(js_file_path.exists() && fs::read_to_string(js_file_path.to_owned()).unwrap() == no_error_code) {
    let _ = fs::write(&js_file_path, no_error_code);
  }

  if ir_mode {
    match codegen::gen_ir::emit_ir(init_fn, reload_fn, emit_path) {
      Ok(_) => (),
      Err(failure) => {
        println!("\nfailed codegen, {}", failure);
        call_stack::display_stack(&failure)?;
        return Err(failure);
      }
    }
  } else {
    // TODO entry ns
    match codegen::emit_js::emit_js(&init_ns, emit_path) {
      Ok(_) => (),
      Err(failure) => {
        println!("\nfailed codegen, {}", failure);
        call_stack::display_stack(&failure)?;
        return Err(failure);
      }
    }
  }
  let duration = Instant::now().duration_since(started_time);
  println!("took {}ms", duration.as_micros() as f64 / 1000.0);
  Ok(())
}
