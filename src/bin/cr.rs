use std::cell::RefCell;
#[allow(unused_imports)]
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::Duration;
use std::time::Instant;

#[cfg(not(target_arch = "wasm32"))]
mod injection;

mod cli_handlers;

#[path = "../type_coverage.rs"]
mod type_coverage;

#[cfg(test)]
static GLOBAL_TEST_LOCK: std::sync::LazyLock<std::sync::Mutex<()>> = std::sync::LazyLock::new(|| std::sync::Mutex::new(()));

#[cfg(test)]
#[path = "cr_tests/type_fail.rs"]
mod cr_type_fail_tests;

#[cfg(test)]
#[path = "cr_tests/cirru_suite.rs"]
mod cr_cirru_suite_tests;

use calcit::calcit::LocatedWarning;
use calcit::call_stack::CallStackList;
use calcit::cli_args::{
  AnalyzeSubcommand, CalcitCommand, CallGraphCommand, CheckTypesCommand, CountCallsCommand, EffectsGraphCommand, ToplevelCalcit,
  WeakTypesCommand,
};
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
use cirru_parser::Cirru;

fn run_check_types(options: &CheckTypesCommand, snapshot: &snapshot::Snapshot) -> Result<(), String> {
  match options.format.as_str() {
    "human" | "text" => print!("{}", type_coverage::format_check_types(options, snapshot)?),
    "json" => println!("{}", type_coverage::format_check_types_json(options, snapshot)?),
    other => return Err(format!("Unknown check-types output format `{other}`. Expected `human` or `json`.")),
  }
  Ok(())
}

fn run_weak_types(options: &WeakTypesCommand, snapshot: &snapshot::Snapshot) -> Result<(), String> {
  match options.format.as_str() {
    "human" | "text" => print!("{}", type_coverage::format_weak_types(options, snapshot)?),
    "json" => println!("{}", type_coverage::format_weak_types_json(options, snapshot)?),
    other => return Err(format!("Unknown weak-types output format `{other}`. Expected `human` or `json`.")),
  }
  Ok(())
}

fn main() -> Result<(), String> {
  let cli_args: ToplevelCalcit = argh::from_env();
  calcit::project_state::set_active_project_directory_from_snapshot(&cli_args.input);

  cli_handlers::set_cursor_after_mode(&cli_args.cursor_after)?;

  if cli_args.version {
    println!("{}", cli_args::CALCIT_VERSION);
    return Ok(());
  }

  if let Some(level) = cli_args.tips_level.as_deref() {
    cli_handlers::set_tips_level(level)?;
  }

  if cli_args.tips {
    cli_handlers::set_tips_level("full")?;
  }

  // Query/analyze commands may run preprocessing before the normal program-loading path.
  runner::preprocess::set_warn_dyn_method(cli_args.warn_dyn_method);
  runner::preprocess::set_verbose_preprocess(cli_args.verbose);

  if cli_handlers::should_echo_command(&cli_args) {
    cli_handlers::suppress_command_guidance();
    calcit::set_quiet_tool_output(true);
    cli_handlers::print_command_echo(&cli_args);
  }

  builtins::effects::init_effects_states();

  #[cfg(not(target_arch = "wasm32"))]
  injection::inject_platform_apis();

  // Handle standalone commands that don't need full program loading
  match &cli_args.subcommand {
    Some(CalcitCommand::Query(query_cmd)) => {
      return cli_handlers::handle_query_command(query_cmd, &cli_args.input);
    }
    Some(CalcitCommand::Docs(docs_cmd)) => {
      return cli_handlers::handle_docs_command(docs_cmd);
    }
    Some(CalcitCommand::Cirru(cirru_cmd)) => {
      return cli_handlers::handle_cirru_command(cirru_cmd);
    }
    Some(CalcitCommand::Libs(libs_cmd)) => {
      return cli_handlers::handle_libs_command(libs_cmd);
    }
    Some(CalcitCommand::Edit(edit_cmd)) => {
      return cli_handlers::handle_edit_command(edit_cmd, &cli_args.input);
    }
    Some(CalcitCommand::Tree(tree_cmd)) => {
      return cli_handlers::handle_tree_command(tree_cmd, &cli_args.input);
    }
    Some(CalcitCommand::Cursor(cursor_cmd)) => {
      return cli_handlers::handle_cursor_command(cursor_cmd, &cli_args.input);
    }
    Some(CalcitCommand::Config(config_cmd)) => {
      return cli_handlers::handle_config_command(config_cmd, &cli_args.input);
    }
    Some(CalcitCommand::Analyze(analyze_cmd)) => match &analyze_cmd.subcommand {
      AnalyzeSubcommand::ProgramDiff(diff_cmd) => {
        return cli_handlers::handle_program_diff_command(diff_cmd, &cli_args.input);
      }
      AnalyzeSubcommand::CallGraphDiff(diff_cmd) => {
        return cli_handlers::handle_call_graph_diff_command(diff_cmd, &cli_args.input);
      }
      AnalyzeSubcommand::CheckTypes(options) => {
        let snapshot = cli_handlers::load_snapshot_for_static_analysis(&cli_args.input)?;
        return run_check_types(options, &snapshot);
      }
      AnalyzeSubcommand::WeakTypes(options) => {
        let snapshot = cli_handlers::load_snapshot_for_static_analysis(&cli_args.input)?;
        return run_weak_types(options, &snapshot);
      }
      _ => {}
    },
    _ => {}
  }

  let mut eval_once = false;
  let is_eval_mode = matches!(&cli_args.subcommand, Some(CalcitCommand::Eval(_)) | Some(CalcitCommand::Exec(_)));
  let assets_watch = cli_args.watch_dir.to_owned();

  if !calcit::quiet_tool_output() {
    eprintln!("{}", format!("calcit version: {}", cli_args::CALCIT_VERSION).dimmed());
  }

  // get dirty functions injected
  #[cfg(not(target_arch = "wasm32"))]
  injection::set_trace_ffi(cli_args.trace_ffi);

  let core_snapshot = calcit::load_core_snapshot()?;

  let mut snapshot = snapshot::Snapshot::default(); // placeholder data

  let module_folder = home_dir()
    .map(|buf| buf.as_path().join(".config/calcit/modules/"))
    .expect("failed to load $HOME");
  if !calcit::quiet_tool_output() {
    eprintln!(
      "{}",
      format!("module folder: {}", module_folder.to_str().expect("extract path")).dimmed()
    );
  }

  if cli_args.disable_stack {
    call_stack::set_using_stack(false);
    if !calcit::quiet_tool_output() {
      println!("stack trace disabled.")
    }
  }

  let input_path = calcit::resolve_snapshot_path_alias(&PathBuf::from(&cli_args.input));
  let input_path_str = input_path.to_string_lossy().to_string();
  let base_dir = input_path.parent().expect("extract parent");

  if let Some(CalcitCommand::Exec(ref command)) = cli_args.subcommand {
    eval_once = true;
    let mut buf = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf).map_err(|e| format!("Failed to read from stdin: {e}"))?;
    {
      let main_file = snapshot::create_file_from_snippet(&buf)?;
      snapshot.files.insert(String::from("app.main"), main_file);
    }

    for module_path in &command.dep {
      let module_data = calcit::load_module(module_path, base_dir, &module_folder)?;
      for (k, v) in &module_data.files {
        if snapshot.files.contains_key(k) {
          return Err(format!("namespace `{k}` already exists when loading module `{module_path}`"));
        }
        snapshot.files.insert(k.to_owned(), v.to_owned());
      }
    }
  } else if let Some(CalcitCommand::Eval(ref command)) = cli_args.subcommand {
    eval_once = true;
    let snippet = if let Some(ref s) = command.snippet {
      s.clone()
    } else {
      return Err("No snippet provided. Use a positional argument with `cr eval`, or use `cr exec` to read from stdin.".to_string());
    };
    {
      let main_file = snapshot::create_file_from_snippet(&snippet)?;
      snapshot.files.insert(String::from("app.main"), main_file);
    }

    for module_path in &command.dep {
      let module_data = calcit::load_module(module_path, base_dir, &module_folder)?;
      for (k, v) in &module_data.files {
        if snapshot.files.contains_key(k) {
          return Err(format!("namespace `{k}` already exists when loading module `{module_path}`"));
        }
        snapshot.files.insert(k.to_owned(), v.to_owned());
      }
    }
  } else {
    if !input_path.exists() {
      return Err(format!("{} does not exist", input_path.display()));
    }
    // load entry file
    let mut content = fs::read_to_string(&input_path).unwrap_or_else(|_| panic!("expected Cirru snapshot: {}", input_path.display()));
    strip_shebang(&mut content);
    let data = cirru_edn::parse(&content).map_err(|e| {
      eprintln!("\nFailed to parse entry file '{}':", input_path.display());
      eprintln!("{e}");
      format!("Failed to parse entry file '{}'", input_path.display())
    })?;
    // println!("reading: {}", content);
    snapshot = snapshot::load_snapshot_data(&data, &input_path_str)?;

    // config in entry will overwrite default configs
    if let Some(entry) = cli_args.entry.to_owned() {
      if snapshot.entries.contains_key(entry.as_str()) {
        if !calcit::quiet_tool_output() {
          println!("running entry: {entry}");
        }
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
        if snapshot.files.contains_key(k) {
          return Err(format!("namespace `{k}` already exists when loading module `{module_path}`"));
        }
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
  runner::preprocess::ensure_ns_def_compiled(
    calcit::calcit::CORE_NS,
    calcit::calcit::BUILTIN_IMPLS_ENTRY,
    check_warnings,
    &CallStackList::default(),
  )
  .map_err(|e| e.msg)?;

  // Check-only mode: just preprocess/validate without execution or codegen
  let check_only = cli_args.check_only || matches!(&cli_args.subcommand, Some(CalcitCommand::EmitJs(js_opts)) if js_opts.check_only);

  if check_only {
    eval_once = true;
  }

  if is_eval_mode && !check_only {
    run_check_only(&entries)?;
  }

  let task = if check_only {
    run_check_only(&entries)
  } else if let Some(CalcitCommand::EmitJs(js_options)) = &cli_args.subcommand {
    if !js_options.watch {
      // `cr js` defaults to once mode; use --watch/-w to keep watching
      eval_once = true;
    }
    if cli_args.skip_arity_check {
      codegen::set_code_gen_skip_arity_check(true);
    }
    run_codegen_with_timeout(&entries, &cli_args.emit_path, false, cli_args.timeout, cli_args.verbose)
  } else if let Some(CalcitCommand::EmitIr(ir_options)) = &cli_args.subcommand {
    if !ir_options.watch {
      // `cr ir` defaults to once mode; use --watch/-w to keep watching
      eval_once = true;
    }
    run_codegen_with_timeout(&entries, &cli_args.emit_path, true, cli_args.timeout, cli_args.verbose)
  } else if let Some(CalcitCommand::Analyze(analyze_cmd)) = &cli_args.subcommand {
    eval_once = true;
    match &analyze_cmd.subcommand {
      AnalyzeSubcommand::CallGraph(call_graph_options) => run_call_graph(&entries, call_graph_options, &snapshot),
      AnalyzeSubcommand::CallGraphDiff(diff_options) => cli_handlers::handle_call_graph_diff_command(diff_options, &cli_args.input),
      AnalyzeSubcommand::CountCalls(count_call_options) => run_count_calls(&entries, count_call_options),
      AnalyzeSubcommand::ProgramDiff(diff_options) => cli_handlers::handle_program_diff_command(diff_options, &cli_args.input),
      AnalyzeSubcommand::CheckExamples(check_options) => {
        run_check_examples(&check_options.ns, check_options.definition.as_deref(), &snapshot)
      }
      AnalyzeSubcommand::CheckTypes(check_types_options) => run_check_types(check_types_options, &snapshot),
      AnalyzeSubcommand::WeakTypes(weak_type_options) => run_weak_types(weak_type_options, &snapshot),
      AnalyzeSubcommand::EffectsGraph(effects_graph_options) => run_effects_graph(&entries, effects_graph_options),
      AnalyzeSubcommand::JsEscape(options) => run_js_escape(&options.symbol),
      AnalyzeSubcommand::JsUnescape(options) => run_js_unescape(&options.symbol),
    }
  } else {
    if !cli_args.watch {
      // direct run defaults to once mode; use --watch/-w to keep watching
      eval_once = true;
    }
    let started_time = Instant::now();

    let v = calcit::run_program_with_docs(entries.init_ns.to_owned(), entries.init_def.to_owned(), &[]).map_err(|e| {
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

fn run_js_escape(symbol: &str) -> Result<(), String> {
  let escaped = calcit::codegen::emit_js::escape_symbol_for_js(symbol);
  println!("{escaped}");
  Ok(())
}

fn run_js_unescape(symbol: &str) -> Result<(), String> {
  let restored = calcit::codegen::emit_js::unescape_symbol_from_js(symbol);
  println!("{restored}");
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
  // 2. clears runtime caches, gensym counter
  // 3. rerun program, and catch error

  let data = cirru_edn::parse(content).map_err(|e| {
    eprintln!("\nFailed to parse changes file:");
    eprintln!("{e}");
    "Failed to parse changes file".to_string()
  })?;
  // println!("\ndata: {}", &data);
  let changes: ChangesDict = data.try_into()?;

  // Print change summary
  println!("{} Incremental changes detected:", "→".cyan());
  if !changes.added.is_empty() {
    println!(
      "  {} Added namespaces: {}",
      "+".green(),
      changes.added.keys().map(|k| k.as_ref()).collect::<Vec<_>>().join(", ")
    );
  }
  if !changes.removed.is_empty() {
    println!(
      "  {} Removed namespaces: {}",
      "-".red(),
      changes.removed.iter().map(|k| k.as_ref()).collect::<Vec<_>>().join(", ")
    );
  }
  if !changes.changed.is_empty() {
    for (ns, file_changes) in &changes.changed {
      let mut changes_desc = Vec::new();
      if file_changes.ns.is_some() {
        changes_desc.push("ns".to_string());
      }
      if !file_changes.added_defs.is_empty() {
        changes_desc.push(format!("+{} defs", file_changes.added_defs.len()));
      }
      if !file_changes.changed_defs.is_empty() {
        changes_desc.push(format!("~{} defs", file_changes.changed_defs.len()));
      }
      if !file_changes.removed_defs.is_empty() {
        changes_desc.push(format!("-{} defs", file_changes.removed_defs.len()));
      }
      println!("  {} {}: {}", "~".yellow(), ns, changes_desc.join(", "));
    }
  }

  program::apply_code_changes(&changes)?;
  println!("{} Changes applied to program", "✓".green());

  // clear invalidated runtime cache entries
  program::clear_runtime_caches_for_changes(&changes, settings.reload_libs)?;
  builtins::meta::force_reset_gensym_index()?;
  println!("cleared runtime caches and reset gensym index.");

  // Create a minimal snapshot for documentation lookup during incremental updates
  // In practice, this could be enhanced to maintain documentation state

  let task = if let Some(CalcitCommand::EmitJs(_)) = settings.subcommand {
    run_codegen_with_timeout(entries, &settings.emit_path, false, settings.timeout, settings.verbose)
  } else if let Some(CalcitCommand::EmitIr(_)) = settings.subcommand {
    run_codegen_with_timeout(entries, &settings.emit_path, true, settings.timeout, settings.verbose)
  } else {
    // run from `reload_fn` after reload
    let started_time = Instant::now();
    let task_size = runner::track::count_pending_tasks();
    println!("checking pending tasks: {task_size}");
    if task_size > 1 {
      // when there's services, make sure their code get preprocessed too
      let check_warnings: &RefCell<Vec<LocatedWarning>> = &RefCell::new(vec![]);
      if let Err(e) =
        runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, check_warnings, &CallStackList::default())
      {
        return Err(e.to_string());
      }

      let warnings = check_warnings.borrow();
      throw_on_warnings(&warnings)?;
    }
    let v = calcit::run_program_with_docs(entries.reload_ns.to_owned(), entries.reload_def.to_owned(), &[]).map_err(|e| {
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

/// Check-only mode: preprocess init_fn and reload_fn to validate code without execution
fn run_check_only(entries: &ProgramEntries) -> Result<(), String> {
  let started_time = Instant::now();
  let check_warnings: &RefCell<Vec<LocatedWarning>> = &RefCell::new(vec![]);

  eprintln!("{}", "Check-only mode: validating code...".dimmed());

  // preprocess init_fn
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

  // preprocess reload_fn
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

  // Report warnings
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

fn run_codegen_with_timeout(
  entries: &ProgramEntries,
  emit_path: &str,
  ir_mode: bool,
  timeout_secs: u64,
  verbose: bool,
) -> Result<(), String> {
  if timeout_secs == 0 {
    return run_codegen(entries, emit_path, ir_mode, verbose);
  }
  let entries = entries.clone();
  let emit_path = emit_path.to_owned();
  let (tx, rx) = channel();
  std::thread::Builder::new()
    .name("calcit-codegen".into())
    // Macro/type preprocessing can be deeply recursive for large schemas.
    .stack_size(16 * 1024 * 1024)
    .spawn(move || {
      let result = run_codegen(&entries, &emit_path, ir_mode, verbose);
      let _ = tx.send(result);
    })
    .map_err(|err| format!("failed to start codegen thread: {err}"))?;
  match rx.recv_timeout(Duration::from_secs(timeout_secs)) {
    Ok(result) => result,
    Err(RecvTimeoutError::Timeout) => Err(format!(
      "codegen timed out after {timeout_secs}s; re-run with --verbose or --timeout 0 for diagnosis"
    )),
    Err(RecvTimeoutError::Disconnected) => Err("codegen thread terminated without returning a result".into()),
  }
}

fn run_codegen(entries: &ProgramEntries, emit_path: &str, ir_mode: bool, verbose: bool) -> Result<(), String> {
  let started_time = Instant::now();
  let phase = |name: &str| {
    if verbose {
      eprintln!("[verbose] {name} (+{}ms)", started_time.elapsed().as_millis());
    }
  };
  phase("codegen started");
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
  phase("preprocessing init entry");
  match runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, check_warnings, &CallStackList::default()) {
    Ok(_) => (),
    Err(failure) => {
      eprintln!("\nfailed preprocessing, {failure}");
      let headline = failure.headline();
      call_stack::display_stack_with_docs(&headline, &failure.stack, failure.location.as_ref(), failure.hint.as_deref())?;

      let _ = fs::write(
        &js_file_path,
        format!("export default \"Preprocessing failed:\\n{}\";", headline.trim().escape_default()),
      );
      return Err(headline);
    }
  }

  // preprocess to reload
  phase("preprocessing reload entry");
  match runner::preprocess::ensure_ns_def_compiled(&entries.reload_ns, &entries.reload_def, check_warnings, &CallStackList::default()) {
    Ok(_) => (),
    Err(failure) => {
      eprintln!("\nfailed preprocessing, {failure}");
      let headline = failure.headline();
      call_stack::display_stack_with_docs(&headline, &failure.stack, failure.location.as_ref(), failure.hint.as_deref())?;
      return Err(headline);
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
    phase("emitting IR");
    match codegen::gen_ir::emit_ir(&entries.init_fn, &entries.reload_fn, emit_path) {
      Ok(_) => (),
      Err(failure) => {
        call_stack::display_stack_with_docs(&failure, &gen_stack::get_gen_stack(), None, None)?;
        return Err(failure);
      }
    }
  } else {
    // TODO entry ns
    phase("emitting JavaScript");
    match codegen::emit_js::emit_js(&entries.init_ns, emit_path) {
      Ok(_) => (),
      Err(failure) => {
        call_stack::display_stack_with_docs(&failure, &gen_stack::get_gen_stack(), None, None)?;
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

fn run_check_examples(target_ns: &str, target_def: Option<&str>, snapshot: &snapshot::Snapshot) -> Result<(), String> {
  match target_def {
    Some(definition) => println!("Checking examples for definition: {target_ns}/{definition}"),
    None => println!("Checking examples in namespace: {target_ns}"),
  }

  // Find the target namespace
  let file_data = snapshot
    .files
    .get(target_ns)
    .ok_or_else(|| format!("Namespace '{target_ns}' not found"))?;

  if let Some(definition) = target_def
    && !file_data.defs.contains_key(definition)
  {
    return Err(format!("Definition '{target_ns}/{definition}' not found"));
  }

  // Collect all functions with examples
  let mut functions_with_examples = Vec::new();
  let mut functions_without_examples = Vec::new();
  let mut total_examples = 0;

  for (def_name, code_entry) in &file_data.defs {
    if target_def.is_some_and(|target| target != def_name) {
      continue;
    }
    if !code_entry.examples.is_empty() {
      functions_with_examples.push((def_name.clone(), code_entry.examples.len()));
      total_examples += code_entry.examples.len();
    } else {
      functions_without_examples.push(def_name.clone());
    }
  }

  if functions_with_examples.is_empty() {
    println!("No functions with examples found in namespace '{target_ns}'");
    return Ok(());
  }

  // Create a synthetic function that runs all examples
  let mut example_calls = Vec::new();

  for (def_name, code_entry) in &file_data.defs {
    if target_def.is_some_and(|target| target != def_name) {
      continue;
    }
    if !code_entry.examples.is_empty() {
      // Add println before examples: println $ str &newline "|-- run examples for: " def "| --"
      example_calls.push(Cirru::List(vec![
        Cirru::Leaf(Arc::from("println")),
        Cirru::List(vec![
          Cirru::Leaf(Arc::from("str")),
          Cirru::Leaf(Arc::from("&newline")),
          Cirru::Leaf(Arc::from("|-- run examples for: ")),
          Cirru::Leaf(Arc::from(format!("|{def_name}"))),
          Cirru::Leaf(Arc::from("| --")),
        ]),
      ]));
    }
    for example in &code_entry.examples {
      example_calls.push(example.clone());
    }
  }

  // Create the check function as a function definition
  let check_function_code = if example_calls.is_empty() {
    Cirru::List(vec![
      Cirru::Leaf(Arc::from("defn")),
      Cirru::Leaf(Arc::from("&calcit:check-examples")),
      Cirru::List(vec![]), // empty parameter list
      Cirru::Leaf(Arc::from("nil")),
    ])
  } else {
    let mut fn_body = vec![Cirru::Leaf(Arc::from("do"))];
    fn_body.extend(example_calls);

    Cirru::List(vec![
      Cirru::Leaf(Arc::from("defn")),
      Cirru::Leaf(Arc::from("&calcit:check-examples")),
      Cirru::List(vec![]), // empty parameter list
      Cirru::List(fn_body),
    ])
  };

  // Create a temporary snapshot with the check function
  let mut temp_snapshot = snapshot.clone();
  let check_fn_name = "&calcit:check-examples".to_string();

  if let Some(file_data) = temp_snapshot.files.get_mut(target_ns) {
    file_data.defs.insert(
      check_fn_name.clone(),
      snapshot::CodeEntry {
        doc: "Generated function to check all examples in this namespace".to_string(),
        examples: Vec::new(),
        tags: std::collections::HashSet::new(),
        code: check_function_code,
        schema: calcit::calcit::DYNAMIC_TYPE.clone(),
      },
    );
  }

  // Update program data
  {
    let mut prgm = { program::PROGRAM_CODE_DATA.write().expect("open program data") };
    *prgm = program::extract_program_data(&temp_snapshot)?;
  }

  // Run the check function
  let started_time = Instant::now();
  println!("Running {total_examples} examples...");

  let result = calcit::run_program_with_docs(Arc::from(target_ns), Arc::from(check_fn_name.as_str()), &[]);

  let duration = Instant::now().duration_since(started_time);

  match result {
    Ok(value) => {
      let _ = value;
      println!("{}", format!("took {}ms: ok", duration.as_micros() as f64 / 1000.0).dimmed());

      // Print summary
      println!("\n{}", "=== Examples Check Summary ===".bold());
      println!("Namespace: {}", target_ns.cyan());
      if let Some(definition) = target_def {
        println!("Definition: {}", definition.cyan());
      }
      println!("Functions with examples: {}", functions_with_examples.len().to_string().green());
      println!("Total examples run: {}", total_examples.to_string().green());
      println!(
        "Functions without examples: {}",
        functions_without_examples.len().to_string().yellow()
      );

      if !functions_with_examples.is_empty() {
        println!("\n{}", "Functions with examples:".bold());
        for (name, count) in &functions_with_examples {
          println!("  {} ({} examples)", name.green(), count.to_string().cyan());
        }
      }

      if !functions_without_examples.is_empty() {
        println!("\n{}", "Functions without examples:".bold());
        let display_count = std::cmp::min(functions_without_examples.len(), 32);
        let names_to_show: Vec<String> = functions_without_examples
          .iter()
          .take(display_count)
          .map(|name| name.yellow().to_string())
          .collect();

        let display_text = if functions_without_examples.len() > 32 {
          format!("  {} ...", names_to_show.join(" "))
        } else {
          format!("  {}", names_to_show.join(" "))
        };

        println!("{display_text}");
      }

      Ok(())
    }
    Err(e) => {
      LocatedWarning::print_list(&e.warnings);
      Err(format!("Failed to run examples: {}", e.msg))
    }
  }
}

fn run_call_graph(entries: &ProgramEntries, options: &CallGraphCommand, _snapshot: &snapshot::Snapshot) -> Result<(), String> {
  // Determine entry point: use --root if provided, otherwise use init_fn from config
  let (entry_ns, entry_def) = if let Some(ref def_path) = options.root {
    util::string::extract_ns_def(def_path)?
  } else {
    (entries.init_ns.to_string(), entries.init_def.to_string())
  };

  println!("{}", format!("Analyzing call tree from: {entry_ns}/{entry_def}").cyan());

  // Analyze call tree
  let result = calcit::call_tree::analyze_call_graph(
    &entry_ns,
    &entry_def,
    options.include_core,
    options.max_depth,
    options.show_unused,
    None, // TODO: could extract package name from snapshot
    options.ns_prefix.clone(),
  )?;

  // Output result
  if options.format == "json" {
    let json = calcit::call_tree::format_as_json(&result)?;
    println!("{json}");
  } else {
    println!("{}", calcit::call_tree::format_for_llm(&result));
  }

  Ok(())
}

fn run_effects_graph(entries: &ProgramEntries, options: &EffectsGraphCommand) -> Result<(), String> {
  let (entry_ns, entry_def) = if let Some(ref def_path) = options.root {
    util::string::extract_ns_def(def_path)?
  } else {
    (entries.init_ns.to_string(), entries.init_def.to_string())
  };

  println!(
    "{}",
    format!(
      "Analyzing effects graph from: {}",
      calcit::effects_graph::format_entry_label(&entry_ns, &entry_def, options.ns_prefix.as_deref())
    )
    .cyan()
  );

  let detail = match options.detail.as_str() {
    "full" => calcit::effects_graph::EffectsGraphDetail::Full,
    "minimal" => calcit::effects_graph::EffectsGraphDetail::Minimal,
    _ => calcit::effects_graph::EffectsGraphDetail::Summary,
  };

  let result = calcit::effects_graph::analyze_effects_graph(
    &entry_ns,
    &entry_def,
    options.include_core,
    options.max_depth,
    options.ns_prefix.clone(),
    detail,
  )?;

  if options.format == "json" {
    let json = calcit::effects_graph::format_as_json(&result)?;
    println!("{json}");
  } else {
    println!("{}", calcit::effects_graph::format_as_ste_tree(&result, options.color));
  }

  Ok(())
}

fn run_count_calls(entries: &ProgramEntries, options: &CountCallsCommand) -> Result<(), String> {
  // Determine entry point: use --root if provided, otherwise use init_fn from config
  let (entry_ns, entry_def) = if let Some(ref def_path) = options.root {
    util::string::extract_ns_def(def_path)?
  } else {
    (entries.init_ns.to_string(), entries.init_def.to_string())
  };

  println!("{}", format!("Counting calls from: {entry_ns}/{entry_def}").cyan());

  // Count calls
  let result = calcit::call_tree::count_calls(&entry_ns, &entry_def, options.include_core, options.ns_prefix.clone())?;

  // Output result
  if options.format == "json" {
    let json = calcit::call_tree::format_count_as_json(&result)?;
    println!("{json}");
  } else {
    println!("{}", calcit::call_tree::format_count_for_display(&result, &options.sort));
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use calcit::calcit::{CalcitTypeAnnotation, SchemaKind};
  use std::fs;

  fn leaf(text: &str) -> Cirru {
    Cirru::Leaf(Arc::from(text))
  }

  fn list(items: Vec<Cirru>) -> Cirru {
    Cirru::List(items)
  }

  fn schema_with_rest(rest: Cirru) -> Cirru {
    list(vec![
      leaf("{}"),
      list(vec![leaf(":kind"), leaf(":fn")]),
      list(vec![leaf(":args"), list(vec![leaf("[]")])]),
      list(vec![leaf(":rest"), rest]),
      list(vec![leaf(":return"), leaf(":dynamic")]),
    ])
  }

  #[test]
  fn schema_rest_shorthand_normalizes_to_list_annotation() {
    let schema = schema_with_rest(leaf(":number"));
    let (_, param_annotations, _, _) = type_coverage::extract_fn_schema_hints(&schema).expect("schema should parse");

    assert_eq!(param_annotations.get("rest"), Some(&vec![":: :list :number".to_owned()]));
  }

  #[test]
  fn schema_rest_explicit_list_keeps_default_name() {
    let schema = schema_with_rest(list(vec![leaf("::"), leaf(":list"), leaf(":number")]));
    let (params, param_annotations, _, _) = type_coverage::extract_fn_schema_hints(&schema).expect("schema should parse");

    assert_eq!(params, vec!["rest".to_owned()]);
    assert_eq!(param_annotations.get("rest"), Some(&vec!["(:: :list :number)".to_owned()]));
    assert!(!param_annotations.contains_key(":list"));
  }

  #[test]
  fn schema_rest_named_tuple_is_treated_as_type_only() {
    let schema = schema_with_rest(list(vec![leaf("::"), leaf("'ys"), leaf(":number")]));
    let (params, param_annotations, _, _) = type_coverage::extract_fn_schema_hints(&schema).expect("schema should parse");

    assert_eq!(params, vec!["rest".to_owned()]);
    assert_eq!(param_annotations.get("rest"), Some(&vec![":: :list :number".to_owned()]));
  }

  // --- validate_def_vs_schema tests ---

  fn fn_schema_annotation(kind: SchemaKind, arg_count: usize, has_rest: bool) -> CalcitTypeAnnotation {
    let arg_types = vec![calcit::calcit::DYNAMIC_TYPE.clone(); arg_count];
    let rest_type = if has_rest {
      Some(calcit::calcit::DYNAMIC_TYPE.clone())
    } else {
      None
    };
    CalcitTypeAnnotation::Fn(Arc::new(calcit::calcit::CalcitFnTypeAnnotation {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types,
      return_type: calcit::calcit::DYNAMIC_TYPE.clone(),
      fn_kind: kind,
      rest_type,
      features: Arc::new(HashSet::new()),
    }))
  }

  fn defn_code(param_names: &[&str], has_rest: bool) -> Cirru {
    let mut params: Vec<Cirru> = param_names.iter().map(|n| leaf(n)).collect();
    if has_rest {
      params.push(leaf("&"));
      params.push(leaf("rest"));
    }
    list(vec![leaf("defn"), leaf("test-fn"), list(params), leaf("nil")])
  }

  fn defmacro_code(param_names: &[&str]) -> Cirru {
    let params: Vec<Cirru> = param_names.iter().map(|n| leaf(n)).collect();
    list(vec![leaf("defmacro"), leaf("test-macro"), list(params), leaf("nil")])
  }

  fn code_entry(code: Cirru, schema: CalcitTypeAnnotation) -> snapshot::CodeEntry {
    snapshot::CodeEntry {
      doc: String::new(),
      examples: vec![],
      tags: HashSet::new(),
      code,
      schema: Arc::new(schema),
    }
  }

  #[test]
  fn type_coverage_uses_schema_for_fn_value_payloads() {
    let schema = CalcitTypeAnnotation::Fn(Arc::new(calcit::calcit::CalcitFnTypeAnnotation {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![Arc::new(CalcitTypeAnnotation::Number)],
      return_type: Arc::new(CalcitTypeAnnotation::String),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(HashSet::new()),
    }));
    let entry = code_entry(list(vec![leaf("fn"), list(vec![leaf("value")]), leaf("|ok")]), schema);

    let row = type_coverage::analyze_code_entry("app.main", "render", &entry);

    assert_eq!(row.kind, type_coverage::DefKind::Fn);
    assert_eq!(row.level, type_coverage::CoverageLevel::Full);
    assert_eq!(row.params, vec!["arg0"]);
    assert_eq!(row.return_type_hints, vec![":string"]);
  }

  #[test]
  fn type_coverage_does_not_mark_unknown_payload_as_full() {
    let entry = code_entry(leaf("unknown-value"), CalcitTypeAnnotation::Dynamic);

    let row = type_coverage::analyze_code_entry("app.main", "unknown", &entry);

    assert_eq!(row.kind, type_coverage::DefKind::Other);
    assert_eq!(row.level, type_coverage::CoverageLevel::None);
  }

  #[test]
  fn type_coverage_does_not_mark_unknown_data_as_full() {
    let entry = code_entry(
      list(vec![leaf("def"), leaf("remote-value"), leaf("load-remote-value")]),
      CalcitTypeAnnotation::Dynamic,
    );

    let row = type_coverage::analyze_code_entry("app.main", "remote-value", &entry);

    assert_eq!(row.kind, type_coverage::DefKind::Data);
    assert_eq!(row.level, type_coverage::CoverageLevel::None);
    assert_eq!(row.data_type, None);
  }

  #[test]
  fn type_coverage_recognizes_literal_data() {
    let entry = code_entry(list(vec![leaf("def"), leaf("answer"), leaf("42")]), CalcitTypeAnnotation::Dynamic);

    let row = type_coverage::analyze_code_entry("app.main", "answer", &entry);

    assert_eq!(row.kind, type_coverage::DefKind::Data);
    assert_eq!(row.level, type_coverage::CoverageLevel::Full);
    assert_eq!(row.data_type.as_deref(), Some("number"));
  }

  #[test]
  fn type_coverage_uses_explicit_data_schema() {
    let entry = code_entry(
      list(vec![leaf("def"), leaf("answer"), leaf("load-answer")]),
      CalcitTypeAnnotation::Number,
    );

    let row = type_coverage::analyze_code_entry("app.main", "answer", &entry);

    assert_eq!(row.kind, type_coverage::DefKind::Data);
    assert_eq!(row.level, type_coverage::CoverageLevel::Full);
    assert_eq!(row.data_type.as_deref(), Some("number"));
  }

  #[test]
  fn type_coverage_recognizes_embedded_struct_field_types() {
    let entry = code_entry(
      list(vec![
        leaf("defstruct"),
        leaf("Point"),
        list(vec![leaf(":x"), leaf(":number")]),
        list(vec![leaf(":label"), leaf(":string")]),
      ]),
      CalcitTypeAnnotation::Dynamic,
    );

    let row = type_coverage::analyze_code_entry("app.main", "Point", &entry);

    assert_eq!(row.kind, type_coverage::DefKind::Data);
    assert_eq!(row.level, type_coverage::CoverageLevel::Full);
    assert_eq!(row.data_type.as_deref(), Some("struct"));
    assert!(
      type_coverage::analyze_weak_types_entry("app.main", "Point", &entry, &type_coverage::WeakTypeKind::all()).is_none(),
      "defstruct should not receive a false schema-dynamic hit"
    );
  }

  #[test]
  fn type_coverage_treats_any_as_an_explicit_static_contract() {
    let entry = code_entry(
      list(vec![
        leaf("defstruct"),
        leaf("Envelope"),
        list(vec![leaf(":payload"), leaf(":any")]),
      ]),
      CalcitTypeAnnotation::Dynamic,
    );

    let row = type_coverage::analyze_code_entry("app.main", "Envelope", &entry);

    assert_eq!(row.kind, type_coverage::DefKind::Data);
    assert_eq!(row.level, type_coverage::CoverageLevel::Full);
    assert!(
      type_coverage::analyze_weak_types_entry("app.main", "Envelope", &entry, &type_coverage::WeakTypeKind::all()).is_none(),
      "an explicit :any contract must not be reported as unresolved dynamic"
    );
  }

  #[test]
  fn type_coverage_marks_dynamic_struct_fields_as_partial() {
    let entry = code_entry(
      list(vec![leaf("defstruct"), leaf("Boxed"), list(vec![leaf(":value"), leaf(":dynamic")])]),
      CalcitTypeAnnotation::Dynamic,
    );

    let row = type_coverage::analyze_code_entry("app.main", "Boxed", &entry);

    assert_eq!(row.kind, type_coverage::DefKind::Data);
    assert_eq!(row.level, type_coverage::CoverageLevel::Partial);
  }

  #[test]
  fn type_coverage_recognizes_ref_value_schemas() {
    let dynamic_ref = code_entry(
      list(vec![leaf("defatom"), leaf("*cache"), list(vec![leaf("{}")])]),
      CalcitTypeAnnotation::Ref(calcit::calcit::DYNAMIC_TYPE.clone()),
    );
    let typed_ref = code_entry(
      list(vec![leaf("defatom"), leaf("*enabled?"), leaf("false")]),
      CalcitTypeAnnotation::Ref(Arc::new(CalcitTypeAnnotation::Bool)),
    );

    let dynamic_row = type_coverage::analyze_code_entry("app.main", "*cache", &dynamic_ref);
    let typed_row = type_coverage::analyze_code_entry("app.main", "*enabled?", &typed_ref);

    assert_eq!(dynamic_row.kind, type_coverage::DefKind::Data);
    assert_eq!(dynamic_row.level, type_coverage::CoverageLevel::Partial);
    assert!(
      dynamic_row
        .schema_issues
        .iter()
        .any(|issue| issue.contains("[W_SCHEMA_DYNAMIC]") && issue.contains("schema.item") && issue.contains(":: :ref")),
      "dynamic ref should carry an actionable issue: {:?}",
      dynamic_row.schema_issues
    );
    assert_eq!(typed_row.kind, type_coverage::DefKind::Data);
    assert_eq!(typed_row.level, type_coverage::CoverageLevel::Full);
    assert!(typed_row.schema_issues.is_empty(), "typed ref should not carry issues");
  }

  #[test]
  fn validate_runtime_impl_is_skipped() {
    let schema = fn_schema_annotation(SchemaKind::Fn, 2, false);
    let code = Cirru::Leaf(Arc::from("&runtime-implementation"));
    let issues = type_coverage::validate_def_vs_schema("calcit.core", "some-proc", &code, &schema);
    assert!(issues.is_empty(), "runtime-implementation should be skipped: {issues:?}");
  }

  #[test]
  fn validate_correct_defn_no_issues() {
    let schema = fn_schema_annotation(SchemaKind::Fn, 2, false);
    let code = defn_code(&["a", "b"], false);
    let issues = type_coverage::validate_def_vs_schema("myns", "my-fn", &code, &schema);
    assert!(issues.is_empty(), "correct defn should have no issues: {issues:?}");
  }

  #[test]
  fn validate_correct_defn_with_rest_no_issues() {
    let schema = fn_schema_annotation(SchemaKind::Fn, 1, true);
    let code = defn_code(&["a"], true);
    let issues = type_coverage::validate_def_vs_schema("myns", "my-fn", &code, &schema);
    assert!(issues.is_empty(), "correct defn with rest should have no issues: {issues:?}");
  }

  #[test]
  fn validate_kind_mismatch_fn_vs_defmacro() {
    let schema = fn_schema_annotation(SchemaKind::Fn, 1, false);
    let code = defmacro_code(&["a"]);
    let issues = type_coverage::validate_def_vs_schema("myns", "my-fn", &code, &schema);
    assert!(!issues.is_empty(), "kind mismatch fn/defmacro should be detected");
    assert!(issues[0].contains(":fn") && issues[0].contains("defmacro"), "issue: {}", issues[0]);
  }

  #[test]
  fn validate_kind_mismatch_macro_vs_defn() {
    let schema = fn_schema_annotation(SchemaKind::Macro, 1, false);
    let code = defn_code(&["a"], false);
    let issues = type_coverage::validate_def_vs_schema("myns", "my-macro", &code, &schema);
    assert!(!issues.is_empty(), "kind mismatch macro/defn should be detected");
    assert!(issues[0].contains(":macro") && issues[0].contains("defn"), "issue: {}", issues[0]);
  }

  #[test]
  fn validate_macro_arity_is_ignored() {
    let schema = fn_schema_annotation(SchemaKind::Macro, 1, false);
    let code = defmacro_code(&["a", "b"]);
    let issues = type_coverage::validate_def_vs_schema("myns", "my-macro", &code, &schema);
    assert!(issues.is_empty(), "macro arity differences should not be reported: {issues:?}");
  }

  #[test]
  fn validate_arity_mismatch_detected() {
    let schema = fn_schema_annotation(SchemaKind::Fn, 3, false); // schema expects 3 args
    let code = defn_code(&["a", "b"], false); // code has 2
    let issues = type_coverage::validate_def_vs_schema("myns", "my-fn", &code, &schema);
    assert!(!issues.is_empty(), "arity mismatch should be detected");
    assert!(issues.iter().any(|i| i.contains("3") && i.contains("2")), "issues: {issues:?}");
  }

  #[test]
  fn validate_rest_mismatch_schema_has_rest_code_does_not() {
    let schema = fn_schema_annotation(SchemaKind::Fn, 1, true); // schema has rest
    let code = defn_code(&["a"], false); // code has no rest
    let issues = type_coverage::validate_def_vs_schema("myns", "my-fn", &code, &schema);
    assert!(!issues.is_empty(), "rest mismatch should be detected");
    assert!(issues.iter().any(|i| i.contains(":rest")), "issues: {issues:?}");
  }

  #[test]
  fn analyze_param_arity_basic() {
    // ([] a b c)
    let args = list(vec![leaf("[]"), leaf("a"), leaf("b"), leaf("c")]);
    let (req, rest) = type_coverage::analyze_param_arity(Some(&args));
    assert_eq!(req, 3);
    assert!(!rest);
  }

  #[test]
  fn analyze_param_arity_with_rest() {
    // ([] a & xs)
    let args = list(vec![leaf("[]"), leaf("a"), leaf("&"), leaf("xs")]);
    let (req, rest) = type_coverage::analyze_param_arity(Some(&args));
    assert_eq!(req, 1);
    assert!(rest);
  }

  #[test]
  fn validate_core_include_schema_matches_code() {
    let core_file_content = fs::read_to_string("src/cirru/calcit-core.cirru").expect("Failed to read calcit-core.cirru");
    let edn_data = cirru_edn::parse(&core_file_content).expect("Failed to parse cirru content as EDN");
    let snapshot = snapshot::load_snapshot_data(&edn_data, "src/cirru/calcit-core.cirru").expect("Failed to parse snapshot");
    let core_file = snapshot.files.get("calcit.core").expect("calcit.core file should exist");
    let entry = core_file.defs.get("include").expect("include should exist");

    let issues = type_coverage::validate_def_vs_schema("calcit.core", "include", &entry.code, &entry.schema);
    assert!(
      issues.is_empty(),
      "include schema should match code: {issues:?}; code={:?}",
      entry.code
    );
  }

  #[test]
  fn parse_weak_type_kinds_rejects_unknown_values() {
    let err = type_coverage::parse_weak_type_kinds("schema-dynamic,unknown").expect_err("unknown filters should fail");
    assert!(err.contains("unknown"), "err: {err}");
  }

  #[test]
  fn parse_weak_type_intents_rejects_unknown_values() {
    let err = type_coverage::parse_weak_type_intents("unresolved,guessed").expect_err("unknown intents should fail");
    assert!(err.contains("guessed"), "err: {err}");
  }

  #[test]
  fn analyze_weak_types_marks_dynamic_ffi_boundaries_as_intentional() {
    let entry = snapshot::CodeEntry {
      doc: "".to_owned(),
      examples: vec![],
      tags: HashSet::new(),
      code: list(vec![
        leaf("defn"),
        leaf("ffi-wrapper"),
        list(vec![leaf("value")]),
        list(vec![leaf("assert-type"), leaf("value"), leaf(":dynamic")]),
        leaf("nil"),
      ]),
      schema: CalcitTypeAnnotation::Fn(Arc::new(calcit::calcit::CalcitFnTypeAnnotation {
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types: vec![calcit::calcit::DYNAMIC_TYPE.clone()],
        return_type: calcit::calcit::DYNAMIC_TYPE.clone(),
        fn_kind: SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(HashSet::from([cirru_edn::EdnTag::new("js-ffi")])),
      }))
      .into(),
    };

    let row = type_coverage::analyze_weak_types_entry("app.main", "ffi-wrapper", &entry, &type_coverage::WeakTypeKind::all())
      .expect("should find weak types");

    for occurrence in row
      .occurrences
      .iter()
      .filter(|occurrence| occurrence.kind != type_coverage::WeakTypeKind::CodeNil)
    {
      assert_eq!(
        occurrence.intent,
        type_coverage::WeakTypeIntent::IntentionalJsFfi,
        "occurrence: {occurrence:?}"
      );
    }
    assert!(
      row
        .occurrences
        .iter()
        .any(|occurrence| occurrence.kind == type_coverage::WeakTypeKind::CodeNil
          && occurrence.intent == type_coverage::WeakTypeIntent::Unresolved),
      "nil should remain unresolved: {:?}",
      row.occurrences
    );
  }

  #[test]
  fn analysis_json_envelopes_preserve_filters_and_definition_paths() {
    let entry = snapshot::CodeEntry {
      doc: "demo".to_owned(),
      examples: vec![],
      tags: HashSet::new(),
      code: list(vec![leaf("defn"), leaf("demo"), list(vec![leaf("value")]), leaf("nil")]),
      schema: CalcitTypeAnnotation::Fn(Arc::new(calcit::calcit::CalcitFnTypeAnnotation {
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types: vec![calcit::calcit::DYNAMIC_TYPE.clone()],
        return_type: Arc::new(CalcitTypeAnnotation::Unit),
        fn_kind: SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(HashSet::new()),
      }))
      .into(),
    };
    let mut snapshot = snapshot::Snapshot {
      package: "app".to_owned(),
      ..snapshot::Snapshot::default()
    };
    snapshot.files.insert(
      "app.main".to_owned(),
      snapshot::FileInSnapShot {
        ns: snapshot::NsEntry {
          doc: String::new(),
          code: list(vec![leaf("ns"), leaf("app.main")]),
        },
        defs: std::collections::HashMap::from([("demo".to_owned(), entry)]),
      },
    );

    let check_options = CheckTypesCommand {
      ns: Some("app.main".to_owned()),
      ns_prefix: None,
      only: None,
      format: "json".to_owned(),
      deps: false,
      summary_only: false,
    };
    let check_json = type_coverage::format_check_types_json(&check_options, &snapshot).expect("coverage JSON should format");
    let check_value: serde_json::Value = serde_json::from_str(&check_json).expect("coverage JSON should parse");
    assert_eq!(check_value["command"], "analyze.check-types");
    assert_eq!(check_value["data"]["definitions"][0]["id"], "app.main/demo");

    let weak_options = WeakTypesCommand {
      ns: Some("app.main".to_owned()),
      ns_prefix: None,
      only: None,
      intent: Some("unresolved".to_owned()),
      format: "json".to_owned(),
      deps: false,
      summary_only: false,
    };
    let weak_json = type_coverage::format_weak_types_json(&weak_options, &snapshot).expect("weak type JSON should format");
    let weak_value: serde_json::Value = serde_json::from_str(&weak_json).expect("weak type JSON should parse");
    assert_eq!(weak_value["command"], "analyze.weak-types");
    assert_eq!(weak_value["data"]["filters"]["intent"], "unresolved");
    assert_eq!(weak_value["data"]["definitions"][0]["occurrences"][0]["path"], "schema.args.0");

    let mut check_summary_options = check_options.clone();
    check_summary_options.summary_only = true;
    let check_summary_json =
      type_coverage::format_check_types_json(&check_summary_options, &snapshot).expect("coverage summary JSON should format");
    let check_summary: serde_json::Value = serde_json::from_str(&check_summary_json).expect("coverage summary JSON should parse");
    assert_eq!(check_summary["data"]["summary"]["definitions"], 1);
    assert_eq!(check_summary["data"]["definitions"], serde_json::json!([]));

    let mut weak_summary_options = weak_options.clone();
    weak_summary_options.summary_only = true;
    let weak_summary_json =
      type_coverage::format_weak_types_json(&weak_summary_options, &snapshot).expect("weak summary JSON should format");
    let weak_summary: serde_json::Value = serde_json::from_str(&weak_summary_json).expect("weak summary JSON should parse");
    assert_eq!(weak_summary["data"]["summary"]["definitions"], 1);
    assert_eq!(weak_summary["data"]["definitions"], serde_json::json!([]));
  }

  #[test]
  fn analyze_weak_types_entry_finds_schema_and_code_hits() {
    let entry = snapshot::CodeEntry {
      doc: "".to_owned(),
      examples: vec![],
      tags: std::collections::HashSet::new(),
      code: list(vec![
        leaf("defn"),
        leaf("demo"),
        list(vec![]),
        list(vec![leaf("assert-type"), leaf("x"), leaf(":dynamic")]),
        leaf("nil"),
      ]),
      schema: fn_schema_annotation(SchemaKind::Fn, 1, false).into(),
    };

    let row = type_coverage::analyze_weak_types_entry("app.main", "demo", &entry, &type_coverage::WeakTypeKind::all())
      .expect("should find hits");
    let kinds = row.occurrences.iter().map(|item| item.kind).collect::<Vec<_>>();
    let details = row.occurrences.iter().map(|item| item.detail.as_str()).collect::<Vec<_>>();

    assert!(kinds.contains(&type_coverage::WeakTypeKind::SchemaDynamic), "kinds: {kinds:?}");
    assert!(kinds.contains(&type_coverage::WeakTypeKind::CodeDynamic), "kinds: {kinds:?}");
    assert!(kinds.contains(&type_coverage::WeakTypeKind::CodeNil), "kinds: {kinds:?}");
    assert!(details.contains(&"schema-dynamic:arg"), "details: {details:?}");
    assert!(details.contains(&"schema-dynamic:return"), "details: {details:?}");
    assert!(details.contains(&"code-dynamic:assert-type"), "details: {details:?}");
    assert!(details.contains(&"code-nil:literal"), "details: {details:?}");
  }

  #[test]
  fn analyze_weak_types_entry_classifies_nil_branches_and_schema_rest() {
    let entry = snapshot::CodeEntry {
      doc: "".to_owned(),
      examples: vec![],
      tags: std::collections::HashSet::new(),
      code: list(vec![
        leaf("defn"),
        leaf("branchy"),
        list(vec![]),
        list(vec![leaf("if"), leaf("flag"), leaf("nil"), leaf("nil")]),
      ]),
      schema: CalcitTypeAnnotation::Fn(Arc::new(calcit::calcit::CalcitFnTypeAnnotation {
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types: vec![Arc::new(CalcitTypeAnnotation::Number)],
        return_type: Arc::new(CalcitTypeAnnotation::Bool),
        fn_kind: SchemaKind::Fn,
        rest_type: Some(calcit::calcit::DYNAMIC_TYPE.clone()),
        features: Arc::new(HashSet::new()),
      }))
      .into(),
    };

    let row = type_coverage::analyze_weak_types_entry("app.main", "branchy", &entry, &type_coverage::WeakTypeKind::all())
      .expect("should find hits");
    let details = row.occurrences.iter().map(|item| item.detail.as_str()).collect::<Vec<_>>();

    assert!(details.contains(&"schema-dynamic:rest"), "details: {details:?}");
    assert!(details.contains(&"code-nil:if-then"), "details: {details:?}");
    assert!(details.contains(&"code-nil:if-else"), "details: {details:?}");
  }

  #[test]
  fn analyze_weak_types_entry_classifies_nested_schema_dynamic_shapes() {
    let entry = snapshot::CodeEntry {
      doc: "".to_owned(),
      examples: vec![],
      tags: std::collections::HashSet::new(),
      code: list(vec![leaf("defn"), leaf("nested"), list(vec![]), leaf("x")]),
      schema: CalcitTypeAnnotation::Fn(Arc::new(calcit::calcit::CalcitFnTypeAnnotation {
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types: vec![Arc::new(CalcitTypeAnnotation::List(calcit::calcit::DYNAMIC_TYPE.clone()))],
        return_type: Arc::new(CalcitTypeAnnotation::Map(
          Arc::new(CalcitTypeAnnotation::Tag),
          calcit::calcit::DYNAMIC_TYPE.clone(),
        )),
        fn_kind: SchemaKind::Fn,
        rest_type: Some(Arc::new(CalcitTypeAnnotation::Fn(Arc::new(
          calcit::calcit::CalcitFnTypeAnnotation {
            generics: Arc::new(vec![]),
            where_bounds: Arc::new(vec![]),
            arg_types: vec![calcit::calcit::DYNAMIC_TYPE.clone()],
            return_type: Arc::new(CalcitTypeAnnotation::Bool),
            fn_kind: SchemaKind::Fn,
            rest_type: None,
            features: Arc::new(HashSet::new()),
          },
        )))),
        features: Arc::new(HashSet::new()),
      }))
      .into(),
    };

    let row = type_coverage::analyze_weak_types_entry("app.main", "nested", &entry, &type_coverage::WeakTypeKind::all())
      .expect("should find hits");
    let details = row.occurrences.iter().map(|item| item.detail.as_str()).collect::<Vec<_>>();

    assert!(details.contains(&"schema-dynamic:arg:list-item"), "details: {details:?}");
    assert!(details.contains(&"schema-dynamic:return:map-value"), "details: {details:?}");
    assert!(details.contains(&"schema-dynamic:rest:fn-arg"), "details: {details:?}");
  }

  #[test]
  fn analyze_weak_types_entry_classifies_non_fn_composite_root_shapes() {
    let entry = snapshot::CodeEntry {
      doc: "".to_owned(),
      examples: vec![],
      tags: std::collections::HashSet::new(),
      code: leaf("demo"),
      schema: Arc::new(CalcitTypeAnnotation::Map(
        Arc::new(CalcitTypeAnnotation::Tag),
        Arc::new(CalcitTypeAnnotation::List(calcit::calcit::DYNAMIC_TYPE.clone())),
      )),
    };

    let row = type_coverage::analyze_weak_types_entry("app.main", "map-root", &entry, &type_coverage::WeakTypeKind::all())
      .expect("should find hits");
    let details = row.occurrences.iter().map(|item| item.detail.as_str()).collect::<Vec<_>>();

    assert!(details.contains(&"schema-dynamic:root:map-value:list-item"), "details: {details:?}");
  }

  #[test]
  fn extract_schema_dynamic_shape_keeps_nested_suffix() {
    assert_eq!(
      type_coverage::extract_schema_dynamic_position("schema-dynamic:arg:list-item"),
      Some("arg".to_owned())
    );
    assert_eq!(
      type_coverage::extract_schema_dynamic_position("schema-dynamic:return"),
      Some("return".to_owned())
    );
    assert_eq!(type_coverage::extract_schema_dynamic_position("code-dynamic:list-item"), None);

    assert_eq!(
      type_coverage::extract_schema_dynamic_shape("schema-dynamic:arg:list-item"),
      Some("list-item".to_owned())
    );
    assert_eq!(
      type_coverage::extract_schema_dynamic_shape("schema-dynamic:root:map-value:list-item"),
      Some("map-value:list-item".to_owned())
    );
    assert_eq!(type_coverage::extract_schema_dynamic_shape("schema-dynamic:arg"), None);
    assert_eq!(type_coverage::extract_schema_dynamic_shape("code-dynamic:list-item"), None);
  }

  #[test]
  fn extract_schema_dynamic_family_collapses_shape_variants() {
    assert_eq!(
      type_coverage::extract_schema_dynamic_family("schema-dynamic:arg:list-item"),
      Some("list".to_owned())
    );
    assert_eq!(
      type_coverage::extract_schema_dynamic_family("schema-dynamic:return:map-value"),
      Some("map".to_owned())
    );
    assert_eq!(
      type_coverage::extract_schema_dynamic_family("schema-dynamic:rest:fn-return"),
      Some("fn".to_owned())
    );
    assert_eq!(
      type_coverage::extract_schema_dynamic_family("schema-dynamic:root:map-value:list-item"),
      Some("map".to_owned())
    );
    assert_eq!(type_coverage::extract_schema_dynamic_family("schema-dynamic:return"), None);
  }
}
