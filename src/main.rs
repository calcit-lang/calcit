#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate nanoid;

mod builtins;
mod call_stack;
mod cli_args;
mod codegen;
mod data;
mod primes;
mod program;
mod runner;
mod snapshot;
mod util;

use builtins::effects;
use codegen::emit_js::emit_js;
use dirs::home_dir;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use primes::Calcit;
use std::fs;
use std::path::Path;
use std::sync::mpsc::channel;
use std::time::Duration;
use std::time::Instant;

fn main() -> Result<(), String> {
  builtins::effects::init_effects_states();
  let cli_matches = cli_args::parse_cli();

  let mut eval_once = cli_matches.is_present("once");

  // load core libs
  let bytes = include_bytes!("./cirru/calcit-core.cirru");
  let core_content = String::from_utf8_lossy(bytes).to_string();
  let core_data = cirru_edn::parse(&core_content)?;
  let core_snapshot = snapshot::load_snapshot_data(core_data)?;

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
    let content = fs::read_to_string(entry_path).expect("expected a Cirru snapshot");
    let data = cirru_edn::parse(&content)?;
    // println!("reading: {}", content);
    snapshot = snapshot::load_snapshot_data(data)?;

    // attach modules
    for module_path in &snapshot.configs.modules {
      let module_data = load_module(&module_path, entry_path.parent().unwrap())?;
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

  let init_fn = match cli_matches.value_of("init-fn") {
    Some(v) => v.to_owned(),
    None => snapshot.configs.init_fn,
  };
  let reload_fn = match cli_matches.value_of("reload-fn") {
    Some(v) => v.to_owned(),
    None => snapshot.configs.reload_fn,
  };
  let emit_path = match cli_matches.value_of("emit-path") {
    Some(v) => v.to_owned(),
    None => "js-out".to_owned(),
  };

  if cli_matches.is_present("emit-js") {
    run_codegen(&init_fn, &reload_fn, &program_code, &emit_path)?;
  } else {
    run_program(&init_fn, &reload_fn, &program_code)?;
  }

  if !eval_once {
    println!("\nRunner: in watch mode...\n");
    let (tx, rx) = channel();
    let entry_path = Path::new(cli_matches.value_of("input").unwrap());
    let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_millis(200)).unwrap();

    let inc_path = entry_path.parent().unwrap().join(".compact-inc.cirru");
    if inc_path.exists() {
      watcher.watch(&inc_path, RecursiveMode::Recursive).unwrap();

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

                // Steps:
                // 1. load changes file, and patch to program_code
                // 2. clears evaled states, gensym counter
                // 3. rerun program, and catch error

                // load new program code
                let content = fs::read_to_string(&inc_path).unwrap();
                let data = cirru_edn::parse(&content)?;
                let changes = snapshot::load_changes_info(data.clone())?;

                // println!("\ndata: {}", &data);
                // println!("\nchanges: {:?}", changes);
                let new_code = program::apply_code_changes(&program_code, &changes)?;
                // println!("\nprogram code: {:?}", new_code);

                // clear data in evaled states
                program::clear_all_program_evaled_defs()?;
                builtins::meta::force_reset_gensym_index()?;

                let task = if cli_matches.is_present("emit-js") {
                  run_codegen(&init_fn, &reload_fn, &new_code, &emit_path)
                } else {
                  // run from `reload_fn` after reload
                  run_program(&reload_fn, &init_fn, &new_code)
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

fn run_program(init_fn: &str, reload_fn: &str, program_code: &program::ProgramCodeData) -> Result<(), String> {
  let started_time = Instant::now();

  let (init_ns, init_def) = extract_ns_def(init_fn)?;
  let (reload_ns, reload_def) = extract_ns_def(reload_fn)?;

  // preprocess to init
  match runner::preprocess::preprocess_ns_def(&init_ns, &init_def, &program_code, &init_def) {
    Ok(_) => (),
    Err(failure) => {
      println!("\nfailed preprocessing, {}", failure);
      call_stack::display_stack(&failure);
      return Err(failure);
    }
  }

  // preprocess to reload
  match runner::preprocess::preprocess_ns_def(&reload_ns, &reload_def, &program_code, &init_def) {
    Ok(_) => (),
    Err(failure) => {
      println!("\nfailed preprocessing, {}", failure);
      call_stack::display_stack(&failure);
      return Err(failure);
    }
  }

  match program::lookup_evaled_def(&init_ns, &init_def) {
    None => Err(format!("entry not initialized: {}/{}", init_ns, init_def)),
    Some(entry) => match entry {
      Calcit::Fn(_, f_ns, _, def_scope, args, body) => {
        let result = runner::run_fn(&im::vector![], &def_scope, &args, &body, &f_ns, &program_code);
        match result {
          Ok(v) => {
            let duration = Instant::now().duration_since(started_time);
            println!("took {}ms: {}", duration.as_micros() as f64 / 1000.0, v);
            Ok(())
          }
          Err(failure) => {
            println!("\nfailed, {}", failure);
            call_stack::display_stack(&failure);
            Err(failure)
          }
        }
      }
      _ => Err(format!("expected function entry, got: {}", entry)),
    },
  }
}

fn run_codegen(
  init_fn: &str,
  reload_fn: &str,
  program_code: &program::ProgramCodeData,
  emit_path: &str,
) -> Result<(), String> {
  let started_time = Instant::now();

  let (init_ns, init_def) = extract_ns_def(init_fn)?;
  let (reload_ns, reload_def) = extract_ns_def(reload_fn)?;

  effects::modify_cli_running_mode(effects::CliRunningMode::Js)?;

  // preprocess to init
  match runner::preprocess::preprocess_ns_def(&init_ns, &init_def, &program_code, &init_def) {
    Ok(_) => (),
    Err(failure) => {
      println!("\nfailed preprocessing, {}", failure);
      call_stack::display_stack(&failure);
      return Err(failure);
    }
  }

  // preprocess to reload
  match runner::preprocess::preprocess_ns_def(&reload_ns, &reload_def, &program_code, &init_def) {
    Ok(_) => (),
    Err(failure) => {
      println!("\nfailed preprocessing, {}", failure);
      call_stack::display_stack(&failure);
      return Err(failure);
    }
  }
  emit_js(&init_ns, &emit_path)?; // TODO entry ns
  let duration = Instant::now().duration_since(started_time);
  println!("took {}ms", duration.as_micros() as f64 / 1000.0);
  Ok(())
}

fn extract_ns_def(s: &str) -> Result<(String, String), String> {
  let pieces: Vec<&str> = (&s).split('/').collect();
  if pieces.len() == 2 {
    Ok((pieces[0].to_string(), pieces[1].to_string()))
  } else {
    Err(format!("invalid ns format: {}", s))
  }
}

fn load_module(path: &str, base_dir: &Path) -> Result<snapshot::Snapshot, String> {
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

  let content = fs::read_to_string(&fullpath).expect("expected a Cirru snapshot");
  let data = cirru_edn::parse(&content)?;
  // println!("reading: {}", content);
  let snapshot = snapshot::load_snapshot_data(data)?;
  Ok(snapshot)
}
