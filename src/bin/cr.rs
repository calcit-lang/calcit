use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::time::Duration;
use std::time::Instant;

#[cfg(not(target_arch = "wasm32"))]
mod injection;

mod cli_handlers;

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
use calcit::cli_args::{AnalyzeSubcommand, CalcitCommand, CallGraphCommand, CheckTypesCommand, CountCallsCommand, ToplevelCalcit};
use calcit::snapshot::ChangesDict;
use calcit::util::string::strip_shebang;
use colored::Colorize;
use dirs::home_dir;
use notify::RecursiveMode;
use notify_debouncer_mini::new_debouncer;

use calcit::{
  ProgramEntries, builtins,
  calcit::{CalcitProc, CalcitSyntax, CalcitTypeAnnotation, ProcTypeSignature, SchemaKind, SyntaxTypeSignature},
  call_stack, cli_args, codegen,
  codegen::COMPILE_ERRORS_FILE,
  codegen::emit_js::gen_stack,
  program, runner, snapshot, util,
};
use cirru_parser::Cirru;

fn main() -> Result<(), String> {
  builtins::effects::init_effects_states();

  let cli_args: ToplevelCalcit = argh::from_env();

  if let Some(level) = cli_args.tips_level.as_deref() {
    cli_handlers::set_tips_level(level)?;
  }

  if cli_args.tips {
    cli_handlers::set_tips_level("full")?;
  }

  if cli_handlers::should_echo_command(&cli_args) {
    cli_handlers::suppress_command_guidance();
    calcit::set_quiet_tool_output(true);
    cli_handlers::print_command_echo(&cli_args);
  }

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
      _ => {}
    },
    _ => {}
  }

  let mut eval_once = false;
  let is_eval_mode = matches!(&cli_args.subcommand, Some(CalcitCommand::Eval(_)));
  let assets_watch = cli_args.watch_dir.to_owned();

  if !cli_args.version && !calcit::quiet_tool_output() {
    eprintln!("{}", format!("calcit version: {}", cli_args::CALCIT_VERSION).dimmed());
  }
  if cli_args.version {
    println!("{}", cli_args::CALCIT_VERSION);
    return Ok(());
  }

  // get dirty functions injected
  #[cfg(not(target_arch = "wasm32"))]
  injection::set_trace_ffi(cli_args.trace_ffi);
  #[cfg(not(target_arch = "wasm32"))]
  injection::inject_platform_apis();

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

  runner::preprocess::set_warn_dyn_method(cli_args.warn_dyn_method);

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
    run_codegen(&entries, &cli_args.emit_path, false)
  } else if let Some(CalcitCommand::EmitIr(ir_options)) = &cli_args.subcommand {
    if !ir_options.watch {
      // `cr ir` defaults to once mode; use --watch/-w to keep watching
      eval_once = true;
    }
    run_codegen(&entries, &cli_args.emit_path, true)
  } else if let Some(CalcitCommand::Analyze(analyze_cmd)) = &cli_args.subcommand {
    eval_once = true;
    match &analyze_cmd.subcommand {
      AnalyzeSubcommand::CallGraph(call_graph_options) => run_call_graph(&entries, call_graph_options, &snapshot),
      AnalyzeSubcommand::CallGraphDiff(diff_options) => cli_handlers::handle_call_graph_diff_command(diff_options, &cli_args.input),
      AnalyzeSubcommand::CountCalls(count_call_options) => run_count_calls(&entries, count_call_options),
      AnalyzeSubcommand::ProgramDiff(diff_options) => cli_handlers::handle_program_diff_command(diff_options, &cli_args.input),
      AnalyzeSubcommand::CheckExamples(check_options) => run_check_examples(&check_options.ns, &snapshot),
      AnalyzeSubcommand::CheckTypes(check_types_options) => run_check_types(check_types_options, &snapshot),
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
    match codegen::gen_ir::emit_ir(&entries.init_fn, &entries.reload_fn, emit_path) {
      Ok(_) => (),
      Err(failure) => {
        call_stack::display_stack_with_docs(&failure, &gen_stack::get_gen_stack(), None, None)?;
        return Err(failure);
      }
    }
  } else {
    // TODO entry ns
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

fn run_check_examples(target_ns: &str, snapshot: &snapshot::Snapshot) -> Result<(), String> {
  println!("Checking examples in namespace: {target_ns}");

  // Find the target namespace
  let file_data = snapshot
    .files
    .get(target_ns)
    .ok_or_else(|| format!("Namespace '{target_ns}' not found"))?;

  // Collect all functions with examples
  let mut functions_with_examples = Vec::new();
  let mut functions_without_examples = Vec::new();
  let mut total_examples = 0;

  for (def_name, code_entry) in &file_data.defs {
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
      println!("{}{}", format!("took {}ms: ", duration.as_micros() as f64 / 1000.0).dimmed(), value);

      // Print summary
      println!("\n{}", "=== Examples Check Summary ===".bold());
      println!("Namespace: {}", target_ns.cyan());
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DefKind {
  Data,
  Fn,
  Macro,
  Proc,
  Syntax,
  Other,
}

impl DefKind {
  fn as_str(self) -> &'static str {
    match self {
      DefKind::Data => "data",
      DefKind::Fn => "fn",
      DefKind::Macro => "macro",
      DefKind::Proc => "proc",
      DefKind::Syntax => "syntax",
      DefKind::Other => "other",
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum CoverageLevel {
  None,
  Partial,
  Full,
}

impl CoverageLevel {
  fn as_str(self) -> &'static str {
    match self {
      CoverageLevel::None => "none",
      CoverageLevel::Partial => "partial",
      CoverageLevel::Full => "full",
    }
  }
}

#[derive(Debug, Clone)]
struct TypeCoverageRow {
  ns: String,
  def: String,
  kind: DefKind,
  level: CoverageLevel,
  params: Vec<String>,
  param_annotations: BTreeMap<String, Vec<String>>,
  return_type_hints: Vec<String>,
  data_type: Option<String>,
  /// Schema-vs-definition mismatch warnings (kind, arity, rest).
  schema_issues: Vec<String>,
}

fn run_check_types(options: &CheckTypesCommand, snapshot: &snapshot::Snapshot) -> Result<(), String> {
  if let Some(ns) = &options.ns
    && !snapshot.files.contains_key(ns)
  {
    return Err(format!("Namespace not found: {ns}"));
  }

  let mut rows: Vec<TypeCoverageRow> = Vec::new();
  let pkg = snapshot.package.as_str();

  for (ns, file) in &snapshot.files {
    if let Some(exact) = &options.ns
      && ns != exact
    {
      continue;
    }
    if let Some(prefix) = &options.ns_prefix
      && !ns.starts_with(prefix)
    {
      continue;
    }
    if !(options.deps || ns == pkg || ns.starts_with(&format!("{pkg}."))) {
      continue;
    }

    for (def_name, entry) in &file.defs {
      rows.push(analyze_code_entry(ns, def_name, entry));
    }
  }

  if let Some(raw) = &options.only {
    let selected = parse_coverage_levels(raw)?;
    rows.retain(|row| selected.contains(&row.level));
  }

  rows.sort_by(|a, b| {
    a.ns
      .cmp(&b.ns)
      .then(a.level.cmp(&b.level))
      .then(a.kind.as_str().cmp(b.kind.as_str()))
      .then(a.def.cmp(&b.def))
  });

  if rows.is_empty() {
    println!("No definitions found in selected namespace scope.");
    return Ok(());
  }

  let mut level_count: BTreeMap<&'static str, usize> = BTreeMap::new();
  let mut kind_count: BTreeMap<&'static str, usize> = BTreeMap::new();
  let mut ns_set: BTreeSet<String> = BTreeSet::new();

  for row in &rows {
    *level_count.entry(row.level.as_str()).or_insert(0) += 1;
    *kind_count.entry(row.kind.as_str()).or_insert(0) += 1;
    ns_set.insert(row.ns.clone());
  }

  println!("Type coverage check");
  println!("- namespaces: {}", ns_set.len());
  println!("- defs: {}", rows.len());
  if let Some(raw) = &options.only {
    println!("- only: {raw}");
  }
  println!(
    "- levels: full={} partial={} none={}",
    level_count.get("full").copied().unwrap_or(0),
    level_count.get("partial").copied().unwrap_or(0),
    level_count.get("none").copied().unwrap_or(0)
  );
  println!(
    "- kinds: fn={} macro={} proc={} syntax={} data={} other={}",
    kind_count.get("fn").copied().unwrap_or(0),
    kind_count.get("macro").copied().unwrap_or(0),
    kind_count.get("proc").copied().unwrap_or(0),
    kind_count.get("syntax").copied().unwrap_or(0),
    kind_count.get("data").copied().unwrap_or(0),
    kind_count.get("other").copied().unwrap_or(0)
  );
  println!();

  let mut current_ns: Option<&str> = None;

  for row in &rows {
    let typed_params = count_typed_params(&row.params, &row.param_annotations);
    let total_params = row.params.len();

    if current_ns != Some(row.ns.as_str()) {
      println!("namespace: {}", row.ns);
      current_ns = Some(row.ns.as_str());
    }

    println!("- def: {}", row.def);
    println!("  kind: {}", row.kind.as_str());
    println!("  coverage: {}", row.level.as_str());

    match row.kind {
      DefKind::Data => {
        println!("  data-type: {}", row.data_type.clone().unwrap_or_else(|| "unknown".to_string()));
      }
      DefKind::Fn => {
        if row.return_type_hints.is_empty() {
          println!("  return: (no hint)");
        } else {
          println!("  return:");
          for item in &row.return_type_hints {
            println!("    - {item}");
          }
        }

        println!("  params ({typed_params}/{total_params}):");
        if total_params == 0 {
          println!("    - (no params)");
        } else {
          for name in &row.params {
            match row.param_annotations.get(name) {
              Some(types) if !types.is_empty() => {
                println!("    - {} => {}", name, types.join(" | "));
              }
              _ => println!("    - {name} => (no assert-type)"),
            }
          }
        }
      }
      DefKind::Macro => {
        println!("  params ({typed_params}/{total_params}):");
        if total_params == 0 {
          println!("    - (no params)");
        } else {
          for name in &row.params {
            match row.param_annotations.get(name) {
              Some(types) if !types.is_empty() => {
                println!("    - {} => {}", name, types.join(" | "));
              }
              _ => println!("    - {name} => (no assert-type)"),
            }
          }
        }
      }
      DefKind::Proc => {
        if row.return_type_hints.is_empty() {
          println!("  return: (no hint)");
        } else {
          println!("  return:");
          for item in &row.return_type_hints {
            println!("    - {item}");
          }
        }

        println!("  params ({typed_params}/{total_params}):");
        if total_params == 0 {
          println!("    - (no params)");
        } else {
          for name in &row.params {
            match row.param_annotations.get(name) {
              Some(types) if !types.is_empty() => {
                println!("    - {} => {}", name, types.join(" | "));
              }
              _ => println!("    - {name} => (no assert-type)"),
            }
          }
        }
      }
      DefKind::Syntax => {
        if row.return_type_hints.is_empty() {
          println!("  return: (no hint)");
        } else {
          println!("  return:");
          for item in &row.return_type_hints {
            println!("    - {item}");
          }
        }

        println!("  params ({typed_params}/{total_params}):");
        if total_params == 0 {
          println!("    - (no params)");
        } else {
          for name in &row.params {
            match row.param_annotations.get(name) {
              Some(types) if !types.is_empty() => {
                println!("    - {} => {}", name, types.join(" | "));
              }
              _ => println!("    - {name} => (no assert-type)"),
            }
          }
        }
      }
      DefKind::Other => {
        println!("  details: no type pattern recognized");
      }
    }

    if !row.schema_issues.is_empty() {
      println!("  schema-issues:");
      for issue in &row.schema_issues {
        println!("    - {issue}");
      }
    }

    println!();
  }

  Ok(())
}

fn analyze_builtin_syntax(def_name: &str, sig: &SyntaxTypeSignature) -> TypeCoverageRow {
  let params: Vec<String> = sig.param_names.iter().map(|s| s.to_string()).collect();

  let param_annotations: BTreeMap<String, Vec<String>> = sig
    .param_types
    .iter()
    .zip(sig.param_names.iter())
    .map(|(t, name)| {
      let type_str = t.describe();
      (name.to_string(), vec![type_str])
    })
    .collect();

  let return_type_hints = vec![sig.return_type.describe()];

  let typed_count = param_annotations.values().filter(|v| !v.is_empty()).count();
  let level = if params.is_empty() || typed_count == params.len() {
    CoverageLevel::Full
  } else if typed_count > 0 {
    CoverageLevel::Partial
  } else {
    CoverageLevel::None
  };

  TypeCoverageRow {
    ns: calcit::calcit::CORE_NS.to_owned(),
    def: def_name.to_owned(),
    kind: DefKind::Syntax,
    level,
    params,
    param_annotations,
    return_type_hints,
    data_type: None,
    schema_issues: vec![],
  }
}

fn analyze_builtin_proc(def_name: &str, sig: &ProcTypeSignature) -> TypeCoverageRow {
  let params: Vec<String> = sig.arg_types.iter().enumerate().map(|(i, _)| format!("arg{i}")).collect();

  let param_annotations: BTreeMap<String, Vec<String>> = sig
    .arg_types
    .iter()
    .enumerate()
    .map(|(i, t)| {
      let name = format!("arg{i}");
      let type_str = t.describe();
      (name, vec![type_str])
    })
    .collect();

  let return_type_hints = vec![sig.return_type.describe()];

  let typed_count = param_annotations.values().filter(|v| !v.is_empty()).count();
  let level = if params.is_empty() || typed_count == params.len() {
    CoverageLevel::Full
  } else if typed_count > 0 {
    CoverageLevel::Partial
  } else {
    CoverageLevel::None
  };

  TypeCoverageRow {
    ns: calcit::calcit::CORE_NS.to_owned(),
    def: def_name.to_owned(),
    kind: DefKind::Proc,
    level,
    params,
    param_annotations,
    return_type_hints,
    data_type: None,
    schema_issues: vec![],
  }
}

/// Validate that a code entry matches its schema (kind, arity, rest param presence).
/// Returns a list of warning/error messages. Empty means no issues.
/// - `&runtime-implementation` = builtin proc/syntax → always skipped.
/// - Schema `:kind :fn`   → code must use `defn`.
/// - Schema `:kind :macro` → code must use `defmacro`.
/// - Schema `:args` length must match required param count in code.
/// - Schema `:rest` presence must match `&` rest param in code.
fn validate_def_vs_schema(ns: &str, def_name: &str, code: &Cirru, schema: &CalcitTypeAnnotation) -> Vec<String> {
  // builtin proc/syntax — skip structural checks
  if matches!(code, Cirru::Leaf(s) if s.as_ref() == "&runtime-implementation") {
    return vec![];
  }

  let CalcitTypeAnnotation::Fn(fn_annot) = schema else {
    // Non-Fn schema (Dynamic, etc.) has no structural constraints
    return vec![];
  };

  let Cirru::List(xs) = code else {
    return vec![];
  };

  let code_kind = match xs.first() {
    Some(Cirru::Leaf(s)) if s.as_ref() == "defn" => "defn",
    Some(Cirru::Leaf(s)) if s.as_ref() == "defmacro" => "defmacro",
    _ => return vec![], // not a defn/defmacro form — skip
  };

  let mut issues: Vec<String> = vec![];

  // Kind mismatch
  match (fn_annot.fn_kind, code_kind) {
    (SchemaKind::Fn, "defmacro") => {
      issues.push(format!("{ns}/{def_name}: schema :kind is :fn but code uses defmacro"));
    }
    (SchemaKind::Macro, "defn") => {
      issues.push(format!("{ns}/{def_name}: schema :kind is :macro but code uses defn"));
    }
    _ => {}
  }

  if code_kind == "defmacro" {
    return issues;
  }

  // Arity check
  let (required_count, has_rest) = analyze_param_arity(xs.get(2));
  let schema_required = fn_annot.arg_types.len();
  let schema_has_rest = fn_annot.rest_type.is_some();

  if required_count != schema_required {
    issues.push(format!(
      "{ns}/{def_name}: schema has {schema_required} required arg(s) but code has {required_count}"
    ));
  }
  if has_rest != schema_has_rest {
    if has_rest {
      issues.push(format!("{ns}/{def_name}: code has & rest param but schema has no :rest"));
    } else {
      issues.push(format!("{ns}/{def_name}: schema has :rest but code has no & param"));
    }
  }

  issues
}

/// Count required params and detect rest param from a defn/defmacro args form.
fn analyze_param_arity(args: Option<&Cirru>) -> (usize, bool) {
  let Some(Cirru::List(xs)) = args else {
    return (0, false);
  };
  let mut required = 0usize;
  let mut has_rest = false;
  let mut after_amp = false;
  for item in xs.iter() {
    match item {
      Cirru::Leaf(s) => {
        let s = s.as_ref();
        if s == "&" {
          after_amp = true;
        } else if s == "[]" || s == "," || s == "?" {
          // skip structural markers
        } else if after_amp {
          has_rest = true;
        } else if !s.starts_with(':') && !s.starts_with('|') && !s.chars().all(|c| c.is_ascii_digit()) {
          required += 1;
        }
      }
      Cirru::List(_) => {
        if !after_amp {
          required += 1;
        }
      }
    }
  }
  (required, has_rest)
}

fn analyze_code_entry(ns: &str, def_name: &str, entry: &snapshot::CodeEntry) -> TypeCoverageRow {
  // First check if this is a builtin proc in calcit.core
  if ns == calcit::calcit::CORE_NS {
    if let Ok(proc) = (*def_name).parse::<CalcitProc>() {
      if let Some(sig) = proc.get_type_signature() {
        return analyze_builtin_proc(def_name, sig);
      }
    }
    // Then check if this is a builtin syntax
    if let Ok(syntax) = (*def_name).parse::<CalcitSyntax>() {
      if let Some(sig) = syntax.get_type_signature() {
        return analyze_builtin_syntax(def_name, &sig);
      }
    }
  }

  let (kind, params, param_annotations, return_type_hints, data_type, level) = match &entry.code {
    Cirru::List(xs) => match xs.first() {
      Some(Cirru::Leaf(head)) if &**head == "defn" => {
        if let CalcitTypeAnnotation::Fn(fn_annot) = entry.schema.as_ref()
          && let Ok(schema) = snapshot::schema_edn_to_cirru(&fn_annot.to_schema_edn())
          && let Some((params, param_annotations, return_type_hints, level)) = extract_fn_schema_hints(&schema)
        {
          return TypeCoverageRow {
            ns: ns.to_owned(),
            def: def_name.to_owned(),
            kind: DefKind::Fn,
            level,
            params,
            param_annotations,
            return_type_hints,
            data_type: None,
            schema_issues: validate_def_vs_schema(ns, def_name, &entry.code, &entry.schema),
          };
        }
        if std::env::var("CR_DEBUG_SCHEMA").is_ok() {
          let schema_kind = match entry.schema.as_ref() {
            CalcitTypeAnnotation::Fn(fn_annot) => match snapshot::schema_edn_to_cirru(&fn_annot.to_schema_edn()) {
              Ok(schema) => match extract_fn_schema_hints(&schema) {
                Some(_) => "Fn/schema-hints-ok".to_owned(),
                None => "Fn/schema-hints-none".to_owned(),
              },
              Err(e) => format!("Fn/edn-to-cirru-err:{e}"),
            },
            other => format!("non-fn:{other:?}"),
          };
          eprintln!("[debug] {ns}/{def_name}: schema={schema_kind}");
        }

        let args = xs.get(2);
        let body = &xs[3..];
        let params = extract_param_symbols(args);
        let param_annotations = extract_assert_type_annotations(body);
        let return_type_hints = extract_return_type_hints(body);
        let typed_count = count_typed_params(&params, &param_annotations);
        let ret_typed = !return_type_hints.is_empty();
        let level = if ret_typed && (params.is_empty() || typed_count == params.len()) {
          CoverageLevel::Full
        } else if ret_typed || typed_count > 0 {
          CoverageLevel::Partial
        } else {
          CoverageLevel::None
        };
        (DefKind::Fn, params, param_annotations, return_type_hints, None, level)
      }
      Some(Cirru::Leaf(head)) if &**head == "defmacro" => {
        let args = xs.get(2);
        let body = &xs[3..];
        let params = extract_param_symbols(args);
        let param_annotations = extract_assert_type_annotations(body);
        (DefKind::Macro, params, param_annotations, Vec::new(), None, CoverageLevel::Full)
      }
      Some(Cirru::Leaf(head)) if &**head == "def" => {
        let inferred = xs.get(2).and_then(infer_data_type);
        let level = CoverageLevel::Full;
        (DefKind::Data, Vec::new(), BTreeMap::new(), Vec::new(), inferred, level)
      }
      _ => (DefKind::Other, Vec::new(), BTreeMap::new(), Vec::new(), None, CoverageLevel::Full),
    },
    _ => (DefKind::Other, Vec::new(), BTreeMap::new(), Vec::new(), None, CoverageLevel::Full),
  };

  TypeCoverageRow {
    ns: ns.to_owned(),
    def: def_name.to_owned(),
    kind,
    level,
    params,
    param_annotations,
    return_type_hints,
    data_type,
    schema_issues: validate_def_vs_schema(ns, def_name, &entry.code, &entry.schema),
  }
}

fn unwrap_optional_schema(schema: &Cirru) -> &Cirru {
  match schema {
    Cirru::List(items) => {
      if let Some(Cirru::Leaf(head)) = items.first() {
        if &**head == ":optional" && items.len() == 2 {
          return &items[1];
        }
        if &**head == "::" && items.len() == 3 && matches!(items.get(1), Some(Cirru::Leaf(tag)) if &**tag == ":optional") {
          return &items[2];
        }
      }
      schema
    }
    _ => schema,
  }
}

fn schema_to_map(schema: &Cirru) -> Option<BTreeMap<&str, &Cirru>> {
  let schema = unwrap_optional_schema(schema);
  let Cirru::List(items) = schema else {
    return None;
  };
  let Some(Cirru::Leaf(head)) = items.first() else {
    return None;
  };

  let mut data = BTreeMap::new();
  match &**head {
    "&{}" => {
      if (items.len() - 1) % 2 != 0 {
        return None;
      }
      for idx in (1..items.len()).step_by(2) {
        let key = match &items[idx] {
          Cirru::Leaf(s) if s.starts_with(':') => s.as_ref(),
          _ => return None,
        };
        data.insert(key, &items[idx + 1]);
      }
    }
    "{}" => {
      for pair in items.iter().skip(1) {
        let Cirru::List(xs) = pair else {
          return None;
        };
        if xs.len() != 2 {
          return None;
        }
        let key = match &xs[0] {
          Cirru::Leaf(s) if s.starts_with(':') => s.as_ref(),
          _ => return None,
        };
        data.insert(key, &xs[1]);
      }
    }
    _ => return None,
  }
  Some(data)
}

fn is_schema_list_annotation(node: &Cirru) -> bool {
  match node {
    Cirru::Leaf(s) => s.as_ref() == ":list",
    Cirru::List(xs) => {
      matches!(xs.first(), Some(Cirru::Leaf(head)) if &**head == "::")
        && matches!(xs.get(1), Some(Cirru::Leaf(tag)) if &**tag == ":list")
    }
  }
}

fn render_schema_param_type(ty_node: Option<&Cirru>, wrap_rest_as_list: bool) -> String {
  let Some(ty_node) = ty_node else {
    return ":dynamic".to_owned();
  };

  let rendered = render_cirru_inline(ty_node);
  if !wrap_rest_as_list || rendered == ":dynamic" || is_schema_list_annotation(ty_node) {
    rendered
  } else {
    format!(":: :list {rendered}")
  }
}

fn read_schema_param_tuple(item: &Cirru, default_name: &str, wrap_rest_as_list: bool) -> Option<(String, String)> {
  match item {
    Cirru::Leaf(_) => Some((default_name.to_owned(), render_schema_param_type(Some(item), wrap_rest_as_list))),
    Cirru::List(xs) => {
      let Some(Cirru::Leaf(head)) = xs.first() else {
        return None;
      };
      if &**head != "[]" && &**head != "::" {
        return None;
      }

      match xs.len() {
        2 => {
          let ty = render_schema_param_type(xs.get(1), wrap_rest_as_list);
          Some((default_name.to_owned(), ty))
        }
        3 => {
          let ty_node = match xs.get(1) {
            Some(Cirru::Leaf(name)) if name.starts_with('\'') => xs.get(2),
            _ => Some(item),
          };
          let ty = render_schema_param_type(ty_node, wrap_rest_as_list);
          Some((default_name.to_owned(), ty))
        }
        _ => None,
      }
    }
  }
}

type FnSchemaHints = (Vec<String>, BTreeMap<String, Vec<String>>, Vec<String>, CoverageLevel);

fn extract_fn_schema_hints(schema: &Cirru) -> Option<FnSchemaHints> {
  let schema = schema_to_map(schema)?;

  let mut params: Vec<String> = Vec::new();
  let mut param_annotations: BTreeMap<String, Vec<String>> = BTreeMap::new();

  if let Some(args_node) = schema.get(":args")
    && let Cirru::List(items) = args_node
    && matches!(items.first(), Some(Cirru::Leaf(head)) if &**head == "[]")
  {
    for (idx, item) in items.iter().skip(1).enumerate() {
      if let Some((name, ty)) = read_schema_param_tuple(item, &format!("arg{idx}"), false) {
        params.push(name.clone());
        param_annotations.entry(name).or_default().push(ty);
      }
    }
  }

  if let Some(rest_node) = schema.get(":rest")
    && let Some((name, ty)) = read_schema_param_tuple(rest_node, "rest", true)
  {
    params.push(name.clone());
    param_annotations.entry(name).or_default().push(ty);
  }

  let return_type_hints = vec![
    schema
      .get(":return")
      .map_or_else(|| ":dynamic".to_owned(), |v| render_cirru_inline(v)),
  ];

  let typed_count = params
    .iter()
    .filter(|name| {
      param_annotations
        .get(*name)
        .is_some_and(|hints| hints.iter().any(|hint| hint != ":dynamic"))
    })
    .count();

  let ret_typed = return_type_hints.iter().any(|hint| hint != ":dynamic");
  let level = if ret_typed && (params.is_empty() || typed_count == params.len()) {
    CoverageLevel::Full
  } else if ret_typed || typed_count > 0 {
    CoverageLevel::Partial
  } else {
    CoverageLevel::None
  };

  Some((params, param_annotations, return_type_hints, level))
}

fn extract_param_symbols(args: Option<&Cirru>) -> Vec<String> {
  let mut out: Vec<String> = vec![];
  if let Some(node) = args {
    collect_param_symbols(node, &mut out);
  }
  dedup_keep_order(out)
}

fn collect_param_symbols(node: &Cirru, out: &mut Vec<String>) {
  match node {
    Cirru::Leaf(s) => {
      let name = s.as_ref();
      if name == "&" || name == "?" || name == "[]" || name == "," {
        return;
      }
      if name.starts_with('|') || name.starts_with(':') || name.chars().all(|c| c.is_ascii_digit()) {
        return;
      }
      out.push(name.to_string());
    }
    Cirru::List(xs) => {
      for x in xs {
        collect_param_symbols(x, out);
      }
    }
  }
}

fn extract_assert_type_annotations(nodes: &[Cirru]) -> BTreeMap<String, Vec<String>> {
  let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
  for node in nodes {
    collect_assert_type_annotations(node, &mut out);
  }

  for items in out.values_mut() {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    items.retain(|v| seen.insert(v.to_owned()));
  }

  out
}

fn collect_assert_type_annotations(node: &Cirru, out: &mut BTreeMap<String, Vec<String>>) {
  match node {
    Cirru::Leaf(_) => {}
    Cirru::List(xs) => {
      if let Some(Cirru::Leaf(head)) = xs.first()
        && &**head == "assert-type"
        && let Some(Cirru::Leaf(symbol)) = xs.get(1)
        && let Some(ty_node) = xs.get(2)
      {
        out.entry(symbol.to_string()).or_default().push(render_cirru_inline(ty_node));
      }

      for x in xs {
        collect_assert_type_annotations(x, out);
      }
    }
  }
}

fn extract_return_type_hints(nodes: &[Cirru]) -> Vec<String> {
  let mut out: Vec<String> = Vec::new();
  for node in nodes {
    collect_return_type_hints(node, &mut out);
  }

  let mut seen: BTreeSet<String> = BTreeSet::new();
  out.retain(|v| seen.insert(v.to_owned()));
  out
}

fn collect_return_type_hints(node: &Cirru, out: &mut Vec<String>) {
  match node {
    Cirru::Leaf(_) => {}
    Cirru::List(xs) => {
      if let Some(Cirru::Leaf(head)) = xs.first()
        && &**head == "return-type"
        && let Some(ty_node) = xs.get(1)
      {
        out.push(render_cirru_inline(ty_node));
      }

      for x in xs {
        collect_return_type_hints(x, out);
      }
    }
  }
}

fn count_typed_params(params: &[String], annotations: &BTreeMap<String, Vec<String>>) -> usize {
  params
    .iter()
    .filter(|name| annotations.get(*name).is_some_and(|items| !items.is_empty()))
    .count()
}

fn dedup_keep_order(items: Vec<String>) -> Vec<String> {
  let mut seen: BTreeSet<String> = BTreeSet::new();
  let mut out: Vec<String> = Vec::new();
  for item in items {
    if seen.insert(item.to_owned()) {
      out.push(item);
    }
  }
  out
}

fn render_cirru_inline(node: &Cirru) -> String {
  match node {
    Cirru::Leaf(s) => s.to_string(),
    Cirru::List(xs) => {
      let parts = xs.iter().map(render_cirru_inline).collect::<Vec<_>>().join(" ");
      format!("({parts})")
    }
  }
}

fn parse_coverage_levels(raw: &str) -> Result<BTreeSet<CoverageLevel>, String> {
  let mut selected: BTreeSet<CoverageLevel> = BTreeSet::new();

  for part in raw.split(',') {
    let token = part.trim().to_ascii_lowercase();
    if token.is_empty() {
      continue;
    }

    match token.as_str() {
      "none" => {
        selected.insert(CoverageLevel::None);
      }
      "partial" => {
        selected.insert(CoverageLevel::Partial);
      }
      "full" => {
        selected.insert(CoverageLevel::Full);
      }
      _ => {
        return Err(format!(
          "Unknown coverage level `{token}` in --only. Expected comma-separated values from: none,partial,full"
        ));
      }
    }
  }

  if selected.is_empty() {
    return Err("`--only` is empty. Use one or more of: none,partial,full".to_string());
  }

  Ok(selected)
}

fn infer_data_type(node: &Cirru) -> Option<String> {
  match node {
    Cirru::Leaf(s) => {
      let raw = s.as_ref();
      if raw == "nil" {
        Some("nil".to_string())
      } else if raw == "true" || raw == "false" {
        Some("bool".to_string())
      } else if raw.starts_with('|') {
        Some("string".to_string())
      } else if raw.starts_with(':') {
        Some("tag".to_string())
      } else if raw.parse::<f64>().is_ok() {
        Some("number".to_string())
      } else {
        None
      }
    }
    Cirru::List(xs) => match xs.first() {
      Some(Cirru::Leaf(head)) if &**head == "[]" => Some("list".to_string()),
      Some(Cirru::Leaf(head)) if &**head == "{}" || &**head == "&{}" => Some("map".to_string()),
      Some(Cirru::Leaf(head)) if &**head == "#{}" => Some("set".to_string()),
      Some(Cirru::Leaf(head)) if &**head == "::" => Some("tuple".to_string()),
      Some(Cirru::Leaf(head)) if &**head == "defn" || &**head == "fn" => Some("fn".to_string()),
      Some(Cirru::Leaf(head)) if &**head == "defmacro" => Some("macro".to_string()),
      _ => None,
    },
  }
}

#[cfg(test)]
mod tests {
  use super::*;
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
    let (_, param_annotations, _, _) = extract_fn_schema_hints(&schema).expect("schema should parse");

    assert_eq!(param_annotations.get("rest"), Some(&vec![":: :list :number".to_owned()]));
  }

  #[test]
  fn schema_rest_explicit_list_keeps_default_name() {
    let schema = schema_with_rest(list(vec![leaf("::"), leaf(":list"), leaf(":number")]));
    let (params, param_annotations, _, _) = extract_fn_schema_hints(&schema).expect("schema should parse");

    assert_eq!(params, vec!["rest".to_owned()]);
    assert_eq!(param_annotations.get("rest"), Some(&vec!["(:: :list :number)".to_owned()]));
    assert!(!param_annotations.contains_key(":list"));
  }

  #[test]
  fn schema_rest_named_tuple_is_treated_as_type_only() {
    let schema = schema_with_rest(list(vec![leaf("::"), leaf("'ys"), leaf(":number")]));
    let (params, param_annotations, _, _) = extract_fn_schema_hints(&schema).expect("schema should parse");

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
      arg_types,
      return_type: calcit::calcit::DYNAMIC_TYPE.clone(),
      fn_kind: kind,
      rest_type,
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

  #[test]
  fn validate_runtime_impl_is_skipped() {
    let schema = fn_schema_annotation(SchemaKind::Fn, 2, false);
    let code = Cirru::Leaf(Arc::from("&runtime-implementation"));
    let issues = validate_def_vs_schema("calcit.core", "some-proc", &code, &schema);
    assert!(issues.is_empty(), "runtime-implementation should be skipped: {issues:?}");
  }

  #[test]
  fn validate_correct_defn_no_issues() {
    let schema = fn_schema_annotation(SchemaKind::Fn, 2, false);
    let code = defn_code(&["a", "b"], false);
    let issues = validate_def_vs_schema("myns", "my-fn", &code, &schema);
    assert!(issues.is_empty(), "correct defn should have no issues: {issues:?}");
  }

  #[test]
  fn validate_correct_defn_with_rest_no_issues() {
    let schema = fn_schema_annotation(SchemaKind::Fn, 1, true);
    let code = defn_code(&["a"], true);
    let issues = validate_def_vs_schema("myns", "my-fn", &code, &schema);
    assert!(issues.is_empty(), "correct defn with rest should have no issues: {issues:?}");
  }

  #[test]
  fn validate_kind_mismatch_fn_vs_defmacro() {
    let schema = fn_schema_annotation(SchemaKind::Fn, 1, false);
    let code = defmacro_code(&["a"]);
    let issues = validate_def_vs_schema("myns", "my-fn", &code, &schema);
    assert!(!issues.is_empty(), "kind mismatch fn/defmacro should be detected");
    assert!(issues[0].contains(":fn") && issues[0].contains("defmacro"), "issue: {}", issues[0]);
  }

  #[test]
  fn validate_kind_mismatch_macro_vs_defn() {
    let schema = fn_schema_annotation(SchemaKind::Macro, 1, false);
    let code = defn_code(&["a"], false);
    let issues = validate_def_vs_schema("myns", "my-macro", &code, &schema);
    assert!(!issues.is_empty(), "kind mismatch macro/defn should be detected");
    assert!(issues[0].contains(":macro") && issues[0].contains("defn"), "issue: {}", issues[0]);
  }

  #[test]
  fn validate_macro_arity_is_ignored() {
    let schema = fn_schema_annotation(SchemaKind::Macro, 1, false);
    let code = defmacro_code(&["a", "b"]);
    let issues = validate_def_vs_schema("myns", "my-macro", &code, &schema);
    assert!(issues.is_empty(), "macro arity differences should not be reported: {issues:?}");
  }

  #[test]
  fn validate_arity_mismatch_detected() {
    let schema = fn_schema_annotation(SchemaKind::Fn, 3, false); // schema expects 3 args
    let code = defn_code(&["a", "b"], false); // code has 2
    let issues = validate_def_vs_schema("myns", "my-fn", &code, &schema);
    assert!(!issues.is_empty(), "arity mismatch should be detected");
    assert!(issues.iter().any(|i| i.contains("3") && i.contains("2")), "issues: {issues:?}");
  }

  #[test]
  fn validate_rest_mismatch_schema_has_rest_code_does_not() {
    let schema = fn_schema_annotation(SchemaKind::Fn, 1, true); // schema has rest
    let code = defn_code(&["a"], false); // code has no rest
    let issues = validate_def_vs_schema("myns", "my-fn", &code, &schema);
    assert!(!issues.is_empty(), "rest mismatch should be detected");
    assert!(issues.iter().any(|i| i.contains(":rest")), "issues: {issues:?}");
  }

  #[test]
  fn analyze_param_arity_basic() {
    // ([] a b c)
    let args = list(vec![leaf("[]"), leaf("a"), leaf("b"), leaf("c")]);
    let (req, rest) = analyze_param_arity(Some(&args));
    assert_eq!(req, 3);
    assert!(!rest);
  }

  #[test]
  fn analyze_param_arity_with_rest() {
    // ([] a & xs)
    let args = list(vec![leaf("[]"), leaf("a"), leaf("&"), leaf("xs")]);
    let (req, rest) = analyze_param_arity(Some(&args));
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

    let issues = validate_def_vs_schema("calcit.core", "include", &entry.code, &entry.schema);
    assert!(
      issues.is_empty(),
      "include schema should match code: {issues:?}; code={:?}",
      entry.code
    );
  }
}
