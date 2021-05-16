use std::fs;
use std::path::Path;
use std::sync::mpsc::channel;
use std::time::Duration;
use std::time::Instant;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use calcit_runner;
use calcit_runner::{builtins, call_stack, cli_args, codegen, program, runner, snapshot, util};

fn main() -> Result<(), String> {
  builtins::effects::init_effects_states();
  let cli_matches = cli_args::parse_cli();

  let mut eval_once = cli_matches.is_present("once");

  println!("calcit_runner version: {}", cli_args::CALCIT_VERSION);

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
  } else {
    // load entry file
    let entry_path = Path::new(cli_matches.value_of("input").unwrap());
    let content = fs::read_to_string(entry_path).expect(&format!("expected Cirru snapshot: {:?}", entry_path));
    let data = cirru_edn::parse(&content)?;
    // println!("reading: {}", content);
    snapshot = snapshot::load_snapshot_data(data)?;

    // attach modules
    for module_path in &snapshot.configs.modules {
      let module_data = calcit_runner::load_module(&module_path, entry_path.parent().unwrap())?;
      for (k, v) in &module_data.files {
        snapshot.files.insert(k.clone(), v.clone());
      }
    }
  }

  // attach core
  for (k, v) in core_snapshot.files {
    snapshot.files.insert(k.clone(), v.clone());
  }

  let mut program_code = program::extract_program_data(&snapshot)?;

  let init_fn = cli_matches
    .value_of("init-fn")
    .or(Some(&snapshot.configs.init_fn))
    .unwrap();
  let reload_fn = cli_matches
    .value_of("reload-fn")
    .or(Some(&snapshot.configs.reload_fn))
    .unwrap();
  let emit_path = cli_matches.value_of("emit-path").or(Some("js-out")).unwrap();

  let task = if cli_matches.is_present("emit-js") {
    run_codegen(&init_fn, &reload_fn, &program_code, &emit_path, false)
  } else if cli_matches.is_present("emit-ir") {
    run_codegen(&init_fn, &reload_fn, &program_code, &emit_path, true)
  } else {
    calcit_runner::run_program(&init_fn, im::vector![], &program_code)
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
    let entry_path = Path::new(cli_matches.value_of("input").unwrap());
    let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_millis(200)).unwrap();

    let inc_path = entry_path.parent().unwrap().join(".compact-inc.cirru");
    if inc_path.exists() {
      watcher.watch(&inc_path, RecursiveMode::NonRecursive).unwrap();

      loop {
        match rx.recv() {
          Ok(event) => {
            // println!("event: {:?}", event);
            match event {
              notify::DebouncedEvent::NoticeWrite(..) => {
                // ignored
              }
              notify::DebouncedEvent::Write(_) => {
                println!("\n-------- file change --------\n");
                call_stack::clear_stack();

                // Steps:
                // 1. load changes file, and patch to program_code
                // 2. clears evaled states, gensym counter
                // 3. rerun program, and catch error

                // load new program code
                let content = fs::read_to_string(&inc_path).unwrap();
                if content.trim() == "" {
                  println!("failed re-compiling, got empty inc file");
                  continue;
                }
                let data = cirru_edn::parse(&content)?;
                let changes = snapshot::load_changes_info(data.clone())?;

                // println!("\ndata: {}", &data);
                // println!("\nchanges: {:?}", changes);
                let new_code = program::apply_code_changes(&program_code, &changes)?;
                // println!("\nprogram code: {:?}", new_code);

                // clear data in evaled states
                let reload_libs = cli_matches.is_present("reload-libs");
                program::clear_all_program_evaled_defs(&init_fn, &reload_fn, reload_libs)?;
                builtins::meta::force_reset_gensym_index()?;

                let task = if cli_matches.is_present("emit-js") {
                  run_codegen(&init_fn, &reload_fn, &new_code, &emit_path, false)
                } else if cli_matches.is_present("emit-ir") {
                  run_codegen(&init_fn, &reload_fn, &new_code, &emit_path, true)
                } else {
                  // run from `reload_fn` after reload
                  calcit_runner::run_program(&reload_fn, im::vector![], &new_code)
                };

                match task {
                  Ok(_) => {}
                  Err(e) => {
                    println!("\nfailed to reload, {}", e)
                  }
                }

                // overwrite previous state
                program_code = new_code;
              }
              _ => println!("other file event: {:?}, ignored", event),
            }
          }
          Err(e) => println!("watch error: {:?}", e),
        }
      }
    } else {
      Err(format!("path {:?} not existed", inc_path))
    }
  } else {
    Ok(())
  }
}

fn run_codegen(
  init_fn: &str,
  reload_fn: &str,
  program_code: &program::ProgramCodeData,
  emit_path: &str,
  ir_mode: bool,
) -> Result<(), String> {
  let started_time = Instant::now();

  let (init_ns, init_def) = util::string::extract_ns_def(init_fn)?;
  let (reload_ns, reload_def) = util::string::extract_ns_def(reload_fn)?;

  if ir_mode {
    builtins::effects::modify_cli_running_mode(builtins::effects::CliRunningMode::Ir)?;
  } else {
    builtins::effects::modify_cli_running_mode(builtins::effects::CliRunningMode::Js)?;
  }

  // preprocess to init
  match runner::preprocess::preprocess_ns_def(&init_ns, &init_def, &program_code, &init_def) {
    Ok(_) => (),
    Err(failure) => {
      println!("\nfailed preprocessing, {}", failure);
      call_stack::display_stack(&failure)?;
      return Err(failure);
    }
  }

  // preprocess to reload
  match runner::preprocess::preprocess_ns_def(&reload_ns, &reload_def, &program_code, &init_def) {
    Ok(_) => (),
    Err(failure) => {
      println!("\nfailed preprocessing, {}", failure);
      call_stack::display_stack(&failure)?;
      return Err(failure);
    }
  }

  if ir_mode {
    match codegen::gen_ir::emit_ir(&init_ns, &emit_path, &emit_path) {
      Ok(_) => (),
      Err(failure) => {
        println!("\nfailed codegen, {}", failure);
        call_stack::display_stack(&failure)?;
        return Err(failure);
      }
    }
  } else {
    // TODO entry ns
    match codegen::emit_js::emit_js(&init_ns, &emit_path) {
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
