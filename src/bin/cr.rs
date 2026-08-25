use std::cell::RefCell;
#[allow(unused_imports)]
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::Duration;
use std::time::Instant;

#[cfg(not(target_arch = "wasm32"))]
mod injection;

mod cli_handlers;

#[path = "../deprecated_api.rs"]
mod deprecated_api;
#[path = "../quality_gate.rs"]
mod quality_gate;
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

use calcit::calcit::{CalcitFnTypeAnnotation, CalcitTypeAnnotation, LocatedWarning, SchemaKind};
use calcit::call_stack::CallStackList;
use calcit::cli_args::{
  AnalyzeSubcommand, CalcitCommand, CallGraphCommand, CheckTypesCommand, CountCallsCommand, DeprecatedCommand, DynamicMethodsCommand,
  EffectsGraphCommand, QualityCommand, TestCommand, ToplevelCalcit, WeakTypesCommand,
};
use calcit::snapshot::ChangesDict;
use calcit::util::string::strip_shebang;
use colored::Colorize;
use notify::RecursiveMode;
use notify_debouncer_mini::new_debouncer;

use calcit::{
  ProgramEntries, builtins, call_stack, cli_args, codegen, codegen::COMPILE_ERRORS_FILE, codegen::emit_js::gen_stack, program, runner,
  snapshot, util,
};
use cirru_edn::EdnTag;
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

fn run_deprecated(options: &DeprecatedCommand, snapshot: &snapshot::Snapshot) -> Result<(), String> {
  match options.format.as_str() {
    "human" | "text" => print!("{}", deprecated_api::format_deprecated_api_report(options, snapshot)?),
    "json" => println!("{}", deprecated_api::format_deprecated_api_json(options, snapshot)?),
    other => return Err(format!("Unknown deprecated output format `{other}`. Expected `human` or `json`.")),
  }
  Ok(())
}

fn run_quality(options: &QualityCommand, snapshot: &snapshot::Snapshot) -> Result<(), String> {
  if !matches!(options.format.as_str(), "human" | "text" | "json") {
    return Err(format!(
      "Unknown quality output format `{}`. Expected `human` or `json`.",
      options.format
    ));
  }
  let outcome = quality_gate::analyze_quality(options, snapshot)?;
  match options.format.as_str() {
    "human" | "text" => print!("{}", quality_gate::format_quality_report(&outcome)),
    "json" => println!("{}", quality_gate::format_quality_json(&outcome)?),
    _ => unreachable!("quality output format was validated before analysis"),
  }
  if outcome.passed {
    Ok(())
  } else {
    Err(format!(
      "Static quality gate failed with {} regression(s). Run `calcit docs read library-quality.md --full` for the baseline and CI workflow.",
      outcome.violations.len()
    ))
  }
}

fn collect_dynamic_method_findings(
  warnings: Vec<LocatedWarning>,
  include_dependencies: bool,
  project_namespaces: &HashSet<String>,
) -> Vec<LocatedWarning> {
  let mut unique = BTreeMap::new();
  for (occurrence_index, warning) in warnings.into_iter().enumerate() {
    if !matches!(warning.code(), Some("P_DYNAMIC_METHOD_DISPATCH" | "P_DYNAMIC_POSTFIX_METHOD")) {
      continue;
    }
    let location = warning.location();
    if !include_dependencies && !project_namespaces.contains(location.ns.as_ref()) {
      continue;
    }
    // A precise Snapshot coordinate identifies one call and can be de-duplicated
    // when init/reload reach the same definition. Generated macro forms may only
    // carry `coord=[]`; retain each of those occurrences to avoid undercounting
    // repeated identical calls at an imprecise fallback location.
    let occurrence_key = if location.coord.is_empty() { occurrence_index + 1 } else { 0 };
    let key = (
      location.ns.to_string(),
      location.def.to_string(),
      location.coord.to_vec(),
      warning.code().unwrap_or_default().to_owned(),
      warning.message().to_owned(),
      occurrence_key,
    );
    unique.entry(key).or_insert(warning);
  }
  unique.into_values().collect()
}

fn run_dynamic_methods(
  options: &DynamicMethodsCommand,
  entries: &ProgramEntries,
  snapshot: &snapshot::Snapshot,
  project_namespaces: &HashSet<String>,
) -> Result<(), String> {
  if !matches!(options.format.as_str(), "human" | "text" | "json") {
    return Err(format!(
      "Unknown dynamic-methods output format `{}`. Expected `human`, `text`, or `json`.",
      options.format
    ));
  }

  let previous_warn_setting = runner::preprocess::is_warn_dyn_method_enabled();
  runner::preprocess::set_warn_dyn_method(true);
  let collected = (|| {
    let warnings = RefCell::new(Vec::new());
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())
      .map_err(|failure| failure.msg)?;
    runner::preprocess::ensure_ns_def_compiled(&entries.reload_ns, &entries.reload_def, &warnings, &CallStackList::default())
      .map_err(|failure| failure.msg)?;
    Ok::<Vec<LocatedWarning>, String>(warnings.into_inner())
  })();
  runner::preprocess::set_warn_dyn_method(previous_warn_setting);

  let findings = collect_dynamic_method_findings(collected?, options.deps, project_namespaces);
  let finding_count = findings.len();
  let passed = options.max.is_none_or(|limit| finding_count <= limit);
  let revision_ids = snapshot
    .files
    .iter()
    .filter(|(namespace, _)| options.deps || project_namespaces.contains(namespace.as_str()))
    .flat_map(|(namespace, file)| file.defs.keys().map(|definition| (namespace.clone(), definition.clone())))
    .collect::<Vec<_>>();
  let revision = type_coverage::analysis_revision(snapshot, &revision_ids)?;

  match options.format.as_str() {
    "human" | "text" => {
      println!("Dynamic method dispatch analysis");
      println!("- scope: {}", if options.deps { "project+dependencies" } else { "project" });
      println!("- revision: {revision}");
      println!("- findings: {finding_count}");
      if let Some(limit) = options.max {
        println!("- policy: {} (limit {limit})", if passed { "PASS" } else { "FAIL" });
      }
      if !options.summary_only {
        for warning in &findings {
          println!("- {warning}");
        }
      }
    }
    "json" => {
      let rows = if options.summary_only {
        Vec::new()
      } else {
        findings.iter().map(LocatedWarning::as_json).collect::<Vec<_>>()
      };
      println!(
        "{}",
        serde_json::json!({
          "schema_version": 1,
          "command": "analyze.dynamic-methods",
          "revision": revision,
          "data": {
            "filters": {
              "include_dependencies": options.deps,
              "summary_only": options.summary_only,
              "max": options.max,
            },
            "summary": {
              "findings": finding_count,
              "passed": passed,
            },
            "findings": rows,
          },
          "diagnostics": if passed {
            Vec::<serde_json::Value>::new()
          } else {
            vec![serde_json::json!({
              "code": "E_DYNAMIC_METHOD_POLICY",
              "phase": "analysis",
              "severity": "error",
              "message": format!("Dynamic method dispatch findings {finding_count} exceed limit {}.", options.max.unwrap_or_default()),
            })]
          },
        })
      );
    }
    _ => unreachable!("dynamic-methods output format was validated before analysis"),
  }

  if passed {
    Ok(())
  } else {
    Err(format!(
      "Dynamic method dispatch policy failed: {finding_count} finding(s) exceed --max {}.",
      options.max.unwrap_or_default()
    ))
  }
}

fn attach_missing_core_namespaces(snapshot: &mut snapshot::Snapshot, core_snapshot: snapshot::Snapshot) {
  for (namespace, file) in core_snapshot.files {
    snapshot.files.entry(namespace).or_insert(file);
  }
}

const CLI_STACK_SIZE: usize = 32 * 1024 * 1024;

fn main() -> Result<(), String> {
  let worker = std::thread::Builder::new()
    .name("calcit-cli".to_owned())
    .stack_size(CLI_STACK_SIZE)
    .spawn(run_cli)
    .map_err(|error| format!("Failed to start Calcit CLI worker: {error}"))?;

  match worker.join() {
    Ok(result) => result,
    Err(payload) => std::panic::resume_unwind(payload),
  }
}

fn run_cli() -> Result<(), String> {
  let cli_args: ToplevelCalcit = argh::from_env();
  cli_handlers::warn_on_global_temp_snapshot_path(&cli_args.input);
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
  let _macro_metrics_report = runner::macro_metrics::ReportOnDrop::new(cli_args.macro_metrics);

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
      AnalyzeSubcommand::Deprecated(options) => {
        let snapshot = cli_handlers::load_snapshot_for_static_analysis(&cli_args.input)?;
        return run_deprecated(options, &snapshot);
      }
      AnalyzeSubcommand::Quality(options) => {
        let snapshot = cli_handlers::load_snapshot_for_static_analysis(&cli_args.input)?;
        return run_quality(options, &snapshot);
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
  let mut project_namespaces: HashSet<String> = HashSet::new();

  if cli_args.disable_stack {
    call_stack::set_using_stack(false);
    if !calcit::quiet_tool_output() {
      println!("stack trace disabled.")
    }
  }

  let input_path = calcit::resolve_snapshot_path_alias(&PathBuf::from(&cli_args.input));
  let input_path_str = input_path.to_string_lossy().to_string();
  let base_dir = input_path.parent().expect("extract parent");
  let module_folder = calcit::project_module_folder(base_dir);
  if !calcit::quiet_tool_output() {
    eprintln!("{}", format!("project module folder: {}", module_folder.display()).dimmed());
  }

  if let Some(CalcitCommand::Exec(ref command)) = cli_args.subcommand {
    eval_once = true;
    let mut buf = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf).map_err(|e| format!("Failed to read from stdin: {e}"))?;
    {
      let main_file = snapshot::create_file_from_snippet(&buf)?;
      snapshot.files.insert(String::from("app.main"), main_file);
      project_namespaces.insert(String::from("app.main"));
    }

    for module_path in &command.dep {
      let module_data = calcit::load_module(module_path, base_dir, &module_folder)?;
      calcit::merge_project_module_files(&mut snapshot, &module_data, module_path)?;
    }
  } else if let Some(CalcitCommand::Eval(ref command)) = cli_args.subcommand {
    eval_once = true;
    let snippet = if let Some(ref s) = command.snippet {
      s.clone()
    } else {
      return Err(
        "No snippet provided. Use a positional argument with `calcit eval`, or use `calcit exec` to read from stdin.".to_string(),
      );
    };
    {
      let main_file = snapshot::create_file_from_snippet(&snippet)?;
      snapshot.files.insert(String::from("app.main"), main_file);
      project_namespaces.insert(String::from("app.main"));
    }

    for module_path in &command.dep {
      let module_data = calcit::load_module(module_path, base_dir, &module_folder)?;
      calcit::merge_project_module_files(&mut snapshot, &module_data, module_path)?;
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
    project_namespaces.extend(snapshot.files.keys().cloned());

    snapshot.select_entry(cli_args.entry.as_deref())?;
    if cli_args.entry.is_some() && !calcit::quiet_tool_output() {
      println!("running entry: {}", snapshot.active_entry_name());
    }

    // attach modules
    let module_paths = snapshot.active_entry()?.modules.clone();
    for module_path in &module_paths {
      let module_data = calcit::load_module(module_path, base_dir, &module_folder)?;
      calcit::merge_project_module_files(&mut snapshot, &module_data, module_path)?;
    }
  }
  let selected_entry = snapshot.active_entry()?.clone();
  let configured_run_mode = selected_entry.mode;
  let config_init = selected_entry.init_fn;
  let config_reload = selected_entry.reload_fn;
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

  // Attach built-in core namespaces without replacing a source Snapshot's own
  // calcit.core entries. This matters when developing and testing calcit-core.cirru
  // with an older globally installed `calcit` binary.
  attach_missing_core_namespaces(&mut snapshot, core_snapshot);
  runner::preprocess::set_project_namespaces(&project_namespaces);

  // Dynamic usage is a project-health signal, not a type-check failure. Keep
  // it on stderr so command stdout remains machine-readable for Agent/CI use.
  if !calcit::quiet_tool_output()
    && let Ok(summary) = type_coverage::collect_dynamic_usage_summary(&snapshot)
    && let Some(notice) = type_coverage::format_dynamic_usage_notice(summary)
  {
    eprintln!("{notice}");
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

  let use_configured_js_mode = should_emit_js(&cli_args.subcommand, configured_run_mode);

  let task = if check_only {
    run_check_only(&entries)
  } else if let Some(CalcitCommand::Test(test_options)) = &cli_args.subcommand {
    eval_once = true;
    run_tests(test_options, &snapshot, &project_namespaces)
  } else if use_configured_js_mode || matches!(&cli_args.subcommand, Some(CalcitCommand::EmitJs(_))) {
    let watch = match &cli_args.subcommand {
      Some(CalcitCommand::EmitJs(options)) => options.watch,
      _ => cli_args.watch,
    };
    if !watch {
      // `calcit js` defaults to once mode; use --watch/-w to keep watching
      eval_once = true;
    }
    if cli_args.skip_arity_check {
      codegen::set_code_gen_skip_arity_check(true);
    }
    run_codegen_with_timeout(&entries, &cli_args.emit_path, false, cli_args.timeout, cli_args.verbose)
  } else if let Some(CalcitCommand::EmitIr(ir_options)) = &cli_args.subcommand {
    if !ir_options.watch {
      // `calcit ir` defaults to once mode; use --watch/-w to keep watching
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
      AnalyzeSubcommand::CheckExamples(check_options) => run_check_examples(
        &check_options.ns,
        check_options.definition.as_deref(),
        check_options.js,
        &cli_args.emit_path,
        &snapshot,
      ),
      AnalyzeSubcommand::CheckTypes(check_types_options) => run_check_types(check_types_options, &snapshot),
      AnalyzeSubcommand::WeakTypes(weak_type_options) => run_weak_types(weak_type_options, &snapshot),
      AnalyzeSubcommand::DynamicMethods(options) => run_dynamic_methods(options, &entries, &snapshot, &project_namespaces),
      AnalyzeSubcommand::Deprecated(deprecated_options) => run_deprecated(deprecated_options, &snapshot),
      AnalyzeSubcommand::Quality(quality_options) => run_quality(quality_options, &snapshot),
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
    std::thread::spawn(move || watch_files(entries, args, assets_watch, configured_run_mode));
  }
  runner::track::exit_when_cleared();
  Ok(())
}

#[derive(Debug, Clone)]
struct RunnableTest {
  namespace: String,
  definition: String,
  name: String,
  synthetic_definition: String,
  code: Cirru,
}

impl RunnableTest {
  fn id(&self) -> String {
    format!("{}/{}#{}", self.namespace, self.definition, self.name)
  }
}

#[derive(Debug, serde::Serialize)]
struct TestReportRow {
  id: String,
  status: &'static str,
  #[serde(skip_serializing_if = "Option::is_none")]
  duration_ms: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  error: Option<String>,
}

#[derive(Debug, serde::Serialize)]
struct TestReport {
  schema_version: u32,
  command: &'static str,
  mode: &'static str,
  detail: &'static str,
  selected: usize,
  executed: usize,
  passed: usize,
  failed: usize,
  duration_ms: f64,
  tests: Vec<TestReportRow>,
}

struct TestOutputGuard {
  redirect_stdout: bool,
  silence_program_output: bool,
}

impl TestOutputGuard {
  fn new(redirect_stdout: bool, silence_program_output: bool) -> Self {
    if redirect_stdout {
      injection::set_stdout_to_stderr(true);
    }
    if silence_program_output {
      injection::set_program_output_silenced(true);
    }
    Self {
      redirect_stdout,
      silence_program_output,
    }
  }
}

impl Drop for TestOutputGuard {
  fn drop(&mut self) {
    if self.redirect_stdout {
      injection::set_stdout_to_stderr(false);
    }
    if self.silence_program_output {
      injection::set_program_output_silenced(false);
    }
  }
}

fn print_test_json(
  mode: &'static str,
  tests: &[RunnableTest],
  passed: usize,
  failed: usize,
  duration_ms: f64,
  summary_only: bool,
  rows: Vec<TestReportRow>,
) {
  let report = make_test_report(mode, tests, passed, failed, duration_ms, summary_only, rows);
  println!("{}", serde_json::to_string(&report).expect("test report should serialize"));
}

fn make_test_report(
  mode: &'static str,
  tests: &[RunnableTest],
  passed: usize,
  failed: usize,
  duration_ms: f64,
  summary_only: bool,
  rows: Vec<TestReportRow>,
) -> TestReport {
  TestReport {
    schema_version: 1,
    command: "test",
    mode,
    detail: if summary_only { "summary" } else { "full" },
    selected: tests.len(),
    executed: passed + failed,
    passed,
    failed,
    duration_ms,
    tests: rows,
  }
}

fn run_tests(options: &TestCommand, snapshot: &snapshot::Snapshot, project_namespaces: &HashSet<String>) -> Result<(), String> {
  let json_mode = match options.format.as_str() {
    "human" | "text" => false,
    "json" => true,
    other => return Err(format!("Unknown test output format `{other}`. Expected `human` or `json`.")),
  };
  let _output_guard = TestOutputGuard::new(json_mode, options.summary_only);
  let scope = options.target.as_deref().map(parse_test_scope).transpose()?;
  if let Some((namespace, definition)) = &scope {
    let file = snapshot
      .files
      .get(namespace)
      .ok_or_else(|| format!("Test namespace `{namespace}` not found"))?;
    if let Some(definition) = definition
      && !file.defs.contains_key(definition)
    {
      return Err(format!("Test definition `{namespace}/{definition}` not found"));
    }
  }
  let affected_ids = if options.affected.is_empty() {
    None
  } else {
    Some(resolve_affected_definition_ids(&options.affected, snapshot)?)
  };
  let required_tags = options.tag.iter().map(|tag| tag.trim_start_matches(':')).collect::<HashSet<_>>();
  let excluded_tags = options
    .exclude_tag
    .iter()
    .map(|tag| tag.trim_start_matches(':'))
    .collect::<HashSet<_>>();
  let mut tests = Vec::new();

  let mut namespaces = snapshot.files.keys().collect::<Vec<_>>();
  namespaces.sort();
  for namespace in namespaces {
    if scope.is_none() && !project_namespaces.contains(namespace) {
      continue;
    }
    if let Some((scope_ns, _)) = &scope
      && namespace != scope_ns
    {
      continue;
    }
    let file = snapshot.files.get(namespace).expect("namespace key should exist");
    let mut definitions = file.defs.keys().collect::<Vec<_>>();
    definitions.sort();
    for definition in definitions {
      if let Some((_, Some(scope_def))) = &scope
        && definition != scope_def
      {
        continue;
      }
      let entry = file.defs.get(definition).expect("definition key should exist");
      for test in &entry.tests {
        if options.name.as_ref().is_some_and(|name| name != &test.name) {
          continue;
        }
        if !required_tags.iter().all(|tag| test.tags.iter().any(|item| item.ref_str() == *tag)) {
          continue;
        }
        if excluded_tags.iter().any(|tag| test.tags.iter().any(|item| item.ref_str() == *tag)) {
          continue;
        }
        tests.push(RunnableTest {
          namespace: namespace.clone(),
          definition: definition.clone(),
          name: test.name.clone(),
          synthetic_definition: String::new(),
          code: test.code.clone(),
        });
      }
    }
  }

  tests.sort_by_key(RunnableTest::id);
  if options.list && options.affected.is_empty() {
    if json_mode {
      let rows = if options.summary_only {
        vec![]
      } else {
        tests
          .iter()
          .map(|test| TestReportRow {
            id: test.id(),
            status: "selected",
            duration_ms: None,
            error: None,
          })
          .collect()
      };
      print_test_json("list", &tests, 0, 0, 0.0, options.summary_only, rows);
    } else if !options.summary_only {
      for test in &tests {
        println!("{}", test.id());
      }
      println!("{} test(s)", tests.len());
    }
    return Ok(());
  }
  if tests.is_empty() {
    if json_mode {
      print_test_json(
        if options.list { "list" } else { "run" },
        &tests,
        0,
        0,
        0.0,
        options.summary_only,
        vec![],
      );
    }
    if let Some(error) = no_tests_matched_error(options) {
      return Err(error);
    }
    if !json_mode {
      println!("No tests matched.");
    }
    return Ok(());
  }

  let mut temp_snapshot = snapshot.clone();
  for (index, test) in tests.iter_mut().enumerate() {
    let file = temp_snapshot
      .files
      .get_mut(&test.namespace)
      .expect("test namespace should exist in temporary snapshot");
    let mut synthetic = format!("&calcit:test:{index}");
    while file.defs.contains_key(&synthetic) {
      synthetic.push('_');
    }
    let code = Cirru::List(vec![
      Cirru::leaf("defn"),
      Cirru::Leaf(Arc::from(synthetic.as_str())),
      Cirru::List(vec![]),
      test.code.clone(),
    ]);
    file.defs.insert(synthetic.clone(), snapshot::CodeEntry::from_code(code));
    test.synthetic_definition = synthetic;
  }

  {
    let mut program_data = program::PROGRAM_CODE_DATA.write().expect("open program data");
    *program_data = program::extract_program_data(&temp_snapshot)?;
  }

  let mut compile_errors = std::collections::HashMap::new();
  if let Some(affected_ids) = &affected_ids {
    // `--affected` needs every candidate's static dependency graph. Ordinary
    // test runs compile lazily through `run_program_with_docs` below, so a
    // fail-fast run does not preprocess tests it will never execute.
    for test in &tests {
      let warnings = RefCell::new(Vec::new());
      if let Err(failure) =
        runner::preprocess::ensure_ns_def_compiled(&test.namespace, &test.synthetic_definition, &warnings, &CallStackList::default())
      {
        compile_errors.insert(test.id(), failure.headline());
      } else if !warnings.borrow().is_empty() {
        let warnings = warnings.borrow();
        compile_errors.insert(
          test.id(),
          format!(
            "Found {} warnings, test blocked: {}",
            warnings.len(),
            warnings.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
          ),
        );
      }
    }
    let compiled = program::clone_existing_compiled_program();
    let compiled_by_id = compiled
      .values()
      .flat_map(|file| file.defs.values())
      .map(|definition| (definition.def_id, definition))
      .collect::<std::collections::HashMap<_, _>>();
    tests.retain(|test| {
      compile_errors.contains_key(&test.id())
        || options
          .affected
          .iter()
          .any(|target| target == &format!("{}/{}", test.namespace, test.definition))
        || compiled_test_depends_on(&compiled, &compiled_by_id, test, affected_ids)
    });
  }

  if tests.is_empty() {
    if json_mode {
      print_test_json(
        if options.list { "list" } else { "run" },
        &tests,
        0,
        0,
        0.0,
        options.summary_only,
        vec![],
      );
    }
    if let Some(error) = no_tests_matched_error(options) {
      return Err(error);
    } else if !json_mode {
      println!("No tests are affected.");
    }
    return Ok(());
  }

  if options.list {
    if json_mode {
      let rows = if options.summary_only {
        vec![]
      } else {
        tests
          .iter()
          .map(|test| TestReportRow {
            id: test.id(),
            status: "selected",
            duration_ms: None,
            error: None,
          })
          .collect()
      };
      print_test_json("list", &tests, 0, 0, 0.0, options.summary_only, rows);
    } else if !options.summary_only {
      for test in &tests {
        println!("{}", test.id());
      }
      println!("{} test(s)", tests.len());
    }
    return Ok(());
  }

  if !json_mode {
    println!("Running {} test(s)...", tests.len());
  }
  let started = Instant::now();
  let mut passed = 0usize;
  let mut failures = Vec::new();
  let mut rows = Vec::new();
  for test in &tests {
    let id = test.id();
    let test_started = Instant::now();
    let result = if let Some(error) = compile_errors.remove(&id) {
      Err(error)
    } else {
      calcit::run_program_with_docs(
        Arc::from(test.namespace.as_str()),
        Arc::from(test.synthetic_definition.as_str()),
        &[],
      )
      .map(|_| ())
      .map_err(|failure| {
        LocatedWarning::print_list(&failure.warnings);
        failure.msg
      })
    };
    match result {
      Ok(()) => {
        passed += 1;
        if !json_mode && !options.summary_only {
          println!("  {} {id}", "PASS".green());
        }
        if !options.summary_only {
          rows.push(TestReportRow {
            id,
            status: "passed",
            duration_ms: Some(test_started.elapsed().as_secs_f64() * 1000.0),
            error: None,
          });
        }
      }
      Err(error) => {
        if !json_mode {
          println!("  {} {id}: {error}", "FAIL".red());
        }
        failures.push((id.clone(), error.clone()));
        if !options.summary_only {
          rows.push(TestReportRow {
            id,
            status: "failed",
            duration_ms: Some(test_started.elapsed().as_secs_f64() * 1000.0),
            error: Some(error),
          });
        }
        if options.fail_fast {
          break;
        }
      }
    }
  }

  let duration_ms = started.elapsed().as_secs_f64() * 1000.0;
  if json_mode {
    print_test_json("run", &tests, passed, failures.len(), duration_ms, options.summary_only, rows);
  } else {
    println!(
      "Test result: {} passed; {} failed; {:.2}ms",
      passed.to_string().green(),
      failures.len().to_string().red(),
      duration_ms
    );
  }
  if failures.is_empty() {
    Ok(())
  } else {
    Err(format!("{} test(s) failed", failures.len()))
  }
}

fn no_tests_matched_error(options: &TestCommand) -> Option<String> {
  options
    .name
    .as_ref()
    .map(|name| format!("Test named `{name}` was not found in the selected scope"))
    .or_else(|| options.require_match.then(|| "No tests matched the requested selection".to_owned()))
}

fn parse_test_scope(target: &str) -> Result<(String, Option<String>), String> {
  if target.contains('/') {
    let (namespace, definition) = util::string::extract_ns_def(target)?;
    Ok((namespace, Some(definition)))
  } else if target.trim().is_empty() {
    Err("Test scope must be a namespace or namespace/definition".to_owned())
  } else {
    Ok((target.to_owned(), None))
  }
}

fn resolve_affected_definition_ids(targets: &[String], snapshot: &snapshot::Snapshot) -> Result<HashSet<program::DefId>, String> {
  let mut ids = HashSet::with_capacity(targets.len());
  for target in targets {
    let (namespace, definition) = util::string::extract_ns_def(target)?;
    if !snapshot
      .files
      .get(&namespace)
      .is_some_and(|file| file.defs.contains_key(&definition))
    {
      return Err(format!("Affected definition `{target}` not found"));
    }
    ids.insert(program::ensure_def_id(&namespace, &definition));
  }
  Ok(ids)
}

fn compiled_test_depends_on(
  compiled: &program::CompiledProgram,
  compiled_by_id: &std::collections::HashMap<program::DefId, &program::CompiledDef>,
  test: &RunnableTest,
  affected_ids: &HashSet<program::DefId>,
) -> bool {
  let Some(root) = compiled
    .get(test.namespace.as_str())
    .and_then(|file| file.get(&test.synthetic_definition))
  else {
    return true;
  };
  let mut pending = root.deps.clone();
  let mut visited = HashSet::new();
  while let Some(def_id) = pending.pop() {
    if affected_ids.contains(&def_id) {
      return true;
    }
    if visited.insert(def_id)
      && let Some(definition) = compiled_by_id.get(&def_id)
    {
      pending.extend(definition.deps.iter().copied());
    }
  }
  false
}

fn should_emit_js(subcommand: &Option<CalcitCommand>, configured_run_mode: snapshot::SnapshotRunMode) -> bool {
  matches!(subcommand, Some(CalcitCommand::EmitJs(_))) || (subcommand.is_none() && configured_run_mode == snapshot::SnapshotRunMode::Js)
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

pub fn watch_files(
  entries: ProgramEntries,
  settings: ToplevelCalcit,
  assets_watch: Option<String>,
  configured_run_mode: snapshot::SnapshotRunMode,
) {
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
        if let Err(e) = recall_program(&content, &entries, &settings, configured_run_mode) {
          eprintln!("error: {e}");
        };
      }
      Ok(Err(e)) => println!("watch error: {e:?}"),
      Err(e) => eprintln!("watch error: {e:?}"),
    }
  }
}

// overwrite previous state

fn recall_program(
  content: &str,
  entries: &ProgramEntries,
  settings: &ToplevelCalcit,
  configured_run_mode: snapshot::SnapshotRunMode,
) -> Result<(), String> {
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

  let task = if should_emit_js(&settings.subcommand, configured_run_mode) {
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
    // Macro/type preprocessing follows transitive definition dependencies and
    // can be deeply recursive in real module graphs (for example UI ->
    // Markdown -> math parser helpers). Keep this above the ordinary Rust
    // thread default so the CLI returns diagnostics instead of aborting.
    .stack_size(64 * 1024 * 1024)
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

fn run_check_examples(
  target_ns: &str,
  target_def: Option<&str>,
  js_mode: bool,
  emit_path: &str,
  snapshot: &snapshot::Snapshot,
) -> Result<(), String> {
  match (js_mode, target_def) {
    (true, Some(definition)) => println!("Checking JavaScript examples for definition: {target_ns}/{definition}"),
    (true, None) => println!("Checking JavaScript examples in namespace: {target_ns}"),
    (false, Some(definition)) => println!("Checking examples for definition: {target_ns}/{definition}"),
    (false, None) => println!("Checking examples in namespace: {target_ns}"),
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
        tests: Vec::new(),
        tags: std::collections::HashSet::new(),
        code: check_function_code,
        schema: if js_mode {
          check_examples_js_schema()
        } else {
          calcit::calcit::DYNAMIC_TYPE.clone()
        },
        ffi: None,
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

  let result = if js_mode {
    run_js_examples(target_ns, &check_fn_name, emit_path)
  } else {
    calcit::run_program_with_docs(Arc::from(target_ns), Arc::from(check_fn_name.as_str()), &[])
      .map(|_| ())
      .map_err(|err| {
        LocatedWarning::print_list(&err.warnings);
        err.msg
      })
  };

  let duration = Instant::now().duration_since(started_time);

  match result {
    Ok(()) => {
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
    Err(e) => Err(format!("Failed to run examples: {e}")),
  }
}

fn run_js_examples(target_ns: &str, check_fn_name: &str, emit_path: &str) -> Result<(), String> {
  let check_fn_path = format!("{target_ns}/{check_fn_name}");
  let entries = ProgramEntries {
    init_fn: Arc::from(check_fn_path.clone()),
    reload_fn: Arc::from(check_fn_path),
    init_def: Arc::from(check_fn_name),
    init_ns: Arc::from(target_ns),
    reload_ns: Arc::from(target_ns),
    reload_def: Arc::from(check_fn_name),
  };
  run_codegen(&entries, emit_path, false, false)?;

  let runner_path = Path::new(emit_path).join(format!(".calcit-check-examples-{}.mjs", std::process::id()));
  fs::write(&runner_path, js_examples_runner_source(target_ns, check_fn_name))
    .map_err(|err| format!("Failed to write JavaScript examples runner at {}: {err}", runner_path.display()))?;

  let status = std::process::Command::new("node")
    .arg(&runner_path)
    .status()
    .map_err(|err| format!("Failed to start Node.js for JavaScript examples: {err}"));
  let _ = fs::remove_file(&runner_path);

  match status {
    Ok(status) if status.success() => Ok(()),
    Ok(status) => Err(format!("JavaScript examples exited with status {status}")),
    Err(err) => Err(err),
  }
}

fn js_examples_runner_source(target_ns: &str, check_fn_name: &str) -> String {
  let module_path = serde_json::to_string(&format!("./{target_ns}.mjs")).expect("JavaScript module path should serialize");
  let check_fn = codegen::emit_js::escape_symbol_for_js(check_fn_name);
  format!("import * as examples from {module_path};\nexamples.{check_fn}();\n")
}

fn check_examples_js_schema() -> Arc<CalcitTypeAnnotation> {
  Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
    generics: Arc::new(vec![]),
    where_bounds: Arc::new(vec![]),
    arg_types: vec![],
    return_type: calcit::calcit::DYNAMIC_TYPE.clone(),
    fn_kind: SchemaKind::Fn,
    rest_type: None,
    features: Arc::new(HashSet::from([EdnTag::from("js-ffi")])),
  })))
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
  use std::collections::BTreeSet;
  use std::fs;

  #[test]
  fn attaching_core_preserves_source_namespaces_and_fills_missing_ones() {
    let mut project = snapshot::Snapshot::default();
    let mut local_core = snapshot::gen_meta_ns("calcit.core", "source");
    local_core.ns.doc = "source core".to_owned();
    project.files.insert("calcit.core".to_owned(), local_core);

    let mut embedded = snapshot::Snapshot::default();
    let mut embedded_core = snapshot::gen_meta_ns("calcit.core", "embedded");
    embedded_core.ns.doc = "embedded core".to_owned();
    embedded.files.insert("calcit.core".to_owned(), embedded_core);
    embedded
      .files
      .insert("calcit.internal".to_owned(), snapshot::gen_meta_ns("calcit.internal", "embedded"));

    attach_missing_core_namespaces(&mut project, embedded);

    assert_eq!(project.files["calcit.core"].ns.doc, "source core");
    assert!(project.files.contains_key("calcit.internal"));
  }

  #[test]
  fn dynamic_method_findings_are_scoped_deduplicated_and_code_filtered() {
    fn warning(namespace: &str, code: &str, coord: Vec<u16>) -> LocatedWarning {
      LocatedWarning::new_with_detail(
        format!("warning {code}"),
        calcit::calcit::NodeLocation::new(Arc::from(namespace), Arc::from("run"), Arc::from(coord)),
        Some(code.to_owned()),
        None,
      )
    }

    let project_namespaces = HashSet::from(["app.main".to_owned()]);
    let warnings = vec![
      warning("app.main", "P_DYNAMIC_METHOD_DISPATCH", vec![1]),
      warning("app.main", "P_DYNAMIC_METHOD_DISPATCH", vec![1]),
      warning("app.main", "P_DYNAMIC_METHOD_DISPATCH", vec![]),
      warning("app.main", "P_DYNAMIC_METHOD_DISPATCH", vec![]),
      warning("app.main", "W_JS_FFI_UNTYPED_ACCESS", vec![2]),
      warning("dep.lib", "P_DYNAMIC_POSTFIX_METHOD", vec![3]),
    ];

    let project_only = collect_dynamic_method_findings(warnings.clone(), false, &project_namespaces);
    assert_eq!(
      project_only.len(),
      3,
      "precise duplicates collapse but fallback occurrences remain distinct"
    );
    assert_eq!(project_only[0].location().ns.as_ref(), "app.main");

    let with_dependencies = collect_dynamic_method_findings(warnings, true, &project_namespaces);
    assert_eq!(with_dependencies.len(), 4);
    assert_eq!(with_dependencies[0].location().ns.as_ref(), "app.main");
    assert_eq!(with_dependencies[3].location().ns.as_ref(), "dep.lib");
  }

  #[test]
  fn configured_js_entry_controls_bare_invocation() {
    assert!(should_emit_js(&None, snapshot::SnapshotRunMode::Js));
    assert!(!should_emit_js(&None, snapshot::SnapshotRunMode::Native));
    assert!(!should_emit_js(
      &Some(CalcitCommand::Eval(cli_args::EvalCommand {
        snippet: Some("1".to_owned()),
        dep: vec![],
      })),
      snapshot::SnapshotRunMode::Js,
    ));
  }

  #[test]
  fn js_examples_runner_calls_the_escaped_generated_entry() {
    let check_fn = "&calcit:check-examples";
    let source = js_examples_runner_source("demo.main", check_fn);

    assert!(source.contains("import * as examples from \"./demo.main.mjs\";"));
    assert!(source.contains(&format!("examples.{}();", codegen::emit_js::escape_symbol_for_js(check_fn))));
  }

  #[test]
  fn js_examples_schema_allows_js_ffi() {
    let generated_schema = check_examples_js_schema();
    let CalcitTypeAnnotation::Fn(schema) = generated_schema.as_ref() else {
      panic!("generated JavaScript examples should use a function schema");
    };

    assert!(schema.features.contains(&EdnTag::from("js-ffi")));
  }

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
  fn schema_rest_named_enum_is_treated_as_type_only() {
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

  fn macro_schema_with_optional(required: usize, optional: usize, has_rest: bool) -> CalcitTypeAnnotation {
    let mut arg_types = vec![calcit::calcit::DYNAMIC_TYPE.clone(); required];
    arg_types.extend((0..optional).map(|_| {
      Arc::new(CalcitTypeAnnotation::TypeRef(
        Arc::from("Option"),
        Arc::new(vec![calcit::calcit::DYNAMIC_TYPE.clone()]),
      ))
    }));
    CalcitTypeAnnotation::Fn(Arc::new(calcit::calcit::CalcitFnTypeAnnotation {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types,
      return_type: calcit::calcit::DYNAMIC_TYPE.clone(),
      fn_kind: SchemaKind::Macro,
      rest_type: has_rest.then(|| calcit::calcit::DYNAMIC_TYPE.clone()),
      features: Arc::new(HashSet::new()),
    }))
  }

  fn code_entry(code: Cirru, schema: CalcitTypeAnnotation) -> snapshot::CodeEntry {
    snapshot::CodeEntry {
      doc: String::new(),
      examples: vec![],
      tests: vec![],
      tags: HashSet::new(),
      code,
      schema: Arc::new(schema),
      ffi: None,
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
    assert_eq!(row.return_type_hints, vec!["'String"]);
  }

  #[test]
  fn dynamic_usage_summary_escalates_by_ratio() {
    let summary = type_coverage::DynamicUsageSummary {
      total_positions: 20,
      dynamic_positions: 1,
    };
    assert_eq!(summary.severity(), None);

    let summary = type_coverage::DynamicUsageSummary {
      total_positions: 10,
      dynamic_positions: 2,
    };
    assert_eq!(summary.severity(), Some("notice"));

    let summary = type_coverage::DynamicUsageSummary {
      total_positions: 10,
      dynamic_positions: 3,
    };
    assert_eq!(summary.severity(), Some("warning"));
    assert!(
      type_coverage::format_dynamic_usage_notice(summary)
        .expect("dynamic notice")
        .contains("30.0%")
    );
  }

  #[test]
  fn type_coverage_does_not_mark_unknown_payload_as_full() {
    let entry = code_entry(leaf("unknown-value"), CalcitTypeAnnotation::Dynamic);

    let row = type_coverage::analyze_code_entry("app.main", "unknown", &entry);

    assert_eq!(row.kind, type_coverage::DefKind::Other);
    assert_eq!(row.level, type_coverage::CoverageLevel::None);
  }

  #[test]
  fn type_coverage_does_not_mark_whole_dynamic_macros_as_full() {
    let entry = code_entry(defmacro_code(&["form"]), CalcitTypeAnnotation::Dynamic);
    let row = type_coverage::analyze_code_entry("app.main", "expand-form", &entry);

    assert_eq!(row.kind, type_coverage::DefKind::Macro);
    assert_eq!(row.level, type_coverage::CoverageLevel::None);
    assert!(row.schema_issues.iter().any(|issue| issue.starts_with("[W_MACRO_SCHEMA_DYNAMIC]")));

    let mut missing = code_entry(defmacro_code(&["form"]), CalcitTypeAnnotation::Dynamic);
    missing.schema = calcit::calcit::DYNAMIC_TYPE.clone();
    let row = type_coverage::analyze_code_entry("app.main", "missing-schema", &missing);
    assert!(row.schema_issues.iter().any(|issue| issue.starts_with("[W_SCHEMA_MISSING]")));

    let partially_typed = code_entry(defmacro_code(&["form"]), fn_schema_annotation(SchemaKind::Macro, 1, false));
    let row = type_coverage::analyze_code_entry("app.main", "expand-form", &partially_typed);
    assert_eq!(row.level, type_coverage::CoverageLevel::Partial);
    assert!(row.schema_issues.iter().any(|issue| issue.contains("W_SCHEMA_DYNAMIC")));
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
  fn type_coverage_skips_generic_and_where_declarations_as_struct_fields() {
    let entry = code_entry(
      list(vec![
        leaf("defstruct"),
        leaf("ShownBox"),
        list(vec![leaf("'T")]),
        list(vec![list(vec![leaf("{}"), list(vec![leaf("'T"), leaf("Show")])])]),
        list(vec![list(vec![leaf(":value"), leaf("'T")])]),
      ]),
      CalcitTypeAnnotation::Dynamic,
    );

    let row = type_coverage::analyze_code_entry("app.main", "ShownBox", &entry);

    assert_eq!(row.kind, type_coverage::DefKind::Data);
    assert_eq!(row.level, type_coverage::CoverageLevel::Full);
    assert_eq!(row.generics, vec!["'T"]);
    assert_eq!(row.where_bounds, vec!["('T Show)"]);
  }

  #[test]
  fn type_coverage_does_not_treat_defimpl_root_schema_as_dynamic_debt() {
    let entry = code_entry(
      list(vec![
        leaf("defimpl"),
        leaf("ShowImpl"),
        leaf("Show"),
        list(vec![leaf(".show"), leaf("nil")]),
      ]),
      CalcitTypeAnnotation::Dynamic,
    );

    let row = type_coverage::analyze_code_entry("app.main", "ShowImpl", &entry);
    assert_eq!(row.kind, type_coverage::DefKind::Data);
    assert_eq!(row.level, type_coverage::CoverageLevel::Full);
    let selected = std::collections::BTreeSet::from([type_coverage::WeakTypeKind::SchemaDynamic]);
    assert!(type_coverage::analyze_weak_types_entry("app.main", "ShowImpl", &entry, &selected).is_none());
  }

  #[test]
  fn type_coverage_treats_any_as_legacy_dynamic() {
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
    assert_eq!(row.level, type_coverage::CoverageLevel::Partial);
    let weak = type_coverage::analyze_weak_types_entry("app.main", "Envelope", &entry, &type_coverage::WeakTypeKind::all())
      .expect("legacy :any must remain visible as dynamic debt");
    assert!(
      weak
        .occurrences
        .iter()
        .any(|occurrence| { occurrence.kind == type_coverage::WeakTypeKind::CodeDynamic && occurrence.detail.contains("legacy-any") })
    );
  }

  #[test]
  fn type_coverage_marks_dynamic_struct_fields_as_partial() {
    let entry = code_entry(
      list(vec![
        leaf("defstruct"),
        leaf("Boxed"),
        list(vec![list(vec![leaf(":value"), leaf(":dynamic")])]),
      ]),
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
  fn validate_macro_required_arity_mismatch_is_reported() {
    let schema = fn_schema_annotation(SchemaKind::Macro, 1, false);
    let code = defmacro_code(&["a", "b"]);
    let issues = type_coverage::validate_def_vs_schema("myns", "my-macro", &code, &schema);
    assert!(
      issues.iter().any(|issue| issue.starts_with("[E_SCHEMA_REQUIRED_ARGS]")),
      "{issues:?}"
    );
  }

  #[test]
  fn validate_macro_optional_and_rest_shapes() {
    let code = list(vec![
      leaf("defmacro"),
      leaf("test-macro"),
      list(vec![leaf("a"), leaf("?"), leaf("b"), leaf("&"), leaf("xs")]),
      leaf("nil"),
    ]);
    let schema = macro_schema_with_optional(1, 1, true);
    let issues = type_coverage::validate_def_vs_schema("myns", "my-macro", &code, &schema);
    assert!(issues.is_empty(), "well-formed optional/rest macro should pass: {issues:?}");

    let mismatch = macro_schema_with_optional(2, 0, false);
    let issues = type_coverage::validate_def_vs_schema("myns", "my-macro", &code, &mismatch);
    assert!(
      issues.iter().any(|issue| issue.starts_with("[E_SCHEMA_REQUIRED_ARGS]")),
      "{issues:?}"
    );
    assert!(
      issues.iter().any(|issue| issue.starts_with("[E_SCHEMA_OPTIONAL_ARGS]")),
      "{issues:?}"
    );
    assert!(issues.iter().any(|issue| issue.starts_with("[E_SCHEMA_REST_ARGS]")), "{issues:?}");
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
    assert!(type_coverage::parse_weak_type_kinds("unresolved-type-slot").is_ok());
    assert!(type_coverage::parse_weak_type_kinds("unsafe-coerce").is_ok());
  }

  #[test]
  fn analyze_weak_types_surfaces_only_unbound_type_slots() {
    calcit::calcit::clear_type_slots();
    let slot: Arc<str> = Arc::from("dispatch-op");
    let entry = code_entry(
      list(vec![leaf("defn"), leaf("dispatch!"), list(vec![leaf("op")]), leaf("op")]),
      CalcitTypeAnnotation::Fn(Arc::new(calcit::calcit::CalcitFnTypeAnnotation {
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types: vec![Arc::new(CalcitTypeAnnotation::TypeSlot(slot.clone()))],
        return_type: Arc::new(CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::TypeSlot(slot)))),
        fn_kind: SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(HashSet::new()),
      })),
    );

    let selected = BTreeSet::from([type_coverage::WeakTypeKind::UnresolvedTypeSlot]);
    let row =
      type_coverage::analyze_weak_types_entry("app.main", "dispatch!", &entry, &selected).expect("unbound slots should be reported");
    assert_eq!(row.occurrences.len(), 2);
    assert!(
      row
        .occurrences
        .iter()
        .all(|occurrence| occurrence.kind == type_coverage::WeakTypeKind::UnresolvedTypeSlot)
    );
    assert!(row.occurrences.iter().any(|occurrence| occurrence.path == "schema.args.0"));
    assert!(row.occurrences.iter().any(|occurrence| occurrence.path == "schema.return.item"));

    let coverage = type_coverage::analyze_code_entry("app.main", "dispatch!", &entry);
    assert_eq!(coverage.level, type_coverage::CoverageLevel::Partial);
    assert!(coverage.schema_issues.iter().any(|issue| issue.contains("W_UNRESOLVED_TYPE_SLOT")));

    calcit::calcit::configure_entry_type_slots(&std::collections::HashMap::from([(
      "dispatch-op".to_owned(),
      "app.schema/Op".to_owned(),
    )]))
    .expect("configure slot binding");
    assert!(type_coverage::analyze_weak_types_entry("app.main", "dispatch!", &entry, &selected).is_none());

    calcit::calcit::configure_entry_type_slots(&std::collections::HashMap::from([(
      "dispatch-op".to_owned(),
      ":dynamic".to_owned(),
    )]))
    .expect("configure explicit dynamic slot binding");
    let dynamic_slot = type_coverage::analyze_weak_types_entry(
      "app.main",
      "dispatch!",
      &entry,
      &BTreeSet::from([type_coverage::WeakTypeKind::SchemaDynamic]),
    )
    .expect("explicit Dynamic slot binding should remain visible");
    assert!(
      dynamic_slot
        .occurrences
        .iter()
        .all(|occurrence| { occurrence.intent == type_coverage::WeakTypeIntent::IntentionalTypeSlotDynamic })
    );
    calcit::calcit::clear_type_slots();
  }

  #[test]
  fn parse_weak_type_intents_rejects_unknown_values() {
    let err = type_coverage::parse_weak_type_intents("unresolved,guessed").expect_err("unknown intents should fail");
    assert!(err.contains("guessed"), "err: {err}");
    assert!(type_coverage::parse_weak_type_intents("declared-unit,declared-optional").is_ok());
    assert!(type_coverage::parse_weak_type_intents("explicit-unsafe").is_ok());
  }

  #[test]
  fn analyze_weak_types_inventories_unsafe_coerce_with_target_and_path() {
    let entry = snapshot::CodeEntry {
      doc: "".to_owned(),
      examples: vec![],
      tests: vec![],
      tags: HashSet::new(),
      code: list(vec![
        leaf("defn"),
        leaf("load-id"),
        list(vec![]),
        list(vec![leaf("unsafe-coerce"), leaf("js/nanoid"), leaf("'String")]),
      ]),
      schema: CalcitTypeAnnotation::Fn(Arc::new(calcit::calcit::CalcitFnTypeAnnotation {
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types: vec![],
        return_type: Arc::new(CalcitTypeAnnotation::String),
        fn_kind: SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(HashSet::from([cirru_edn::EdnTag::new("js-ffi")])),
      }))
      .into(),
      ffi: None,
    };

    let row = type_coverage::analyze_weak_types_entry(
      "js-ffi.raw.ids",
      "load-id",
      &entry,
      &BTreeSet::from([type_coverage::WeakTypeKind::UnsafeCoerce]),
    )
    .expect("unsafe coercion should be inventoried");
    assert_eq!(row.occurrences.len(), 1);
    let occurrence = &row.occurrences[0];
    assert_eq!(occurrence.kind, type_coverage::WeakTypeKind::UnsafeCoerce);
    assert_eq!(occurrence.intent, type_coverage::WeakTypeIntent::ExplicitUnsafe);
    assert_eq!(occurrence.path, "code@3");
    assert_eq!(occurrence.detail, "unsafe-coerce:target='String");
    assert_eq!(
      occurrence.unsafe_evidence,
      Some(type_coverage::UnsafeCoerceEvidence {
        source_form: "raw-js-value",
        target_schema: "'String".to_owned(),
        js_ffi_feature: true,
        raw_adapter_namespace: true,
      })
    );

    let mut snapshot = snapshot::Snapshot {
      package: "js-ffi".to_owned(),
      ..snapshot::Snapshot::default()
    };
    snapshot.files.insert(
      "js-ffi.raw.ids".to_owned(),
      snapshot::FileInSnapShot {
        ns: snapshot::NsEntry {
          doc: String::new(),
          code: list(vec![leaf("ns"), leaf("js-ffi.raw.ids")]),
        },
        defs: std::collections::HashMap::from([("load-id".to_owned(), entry)]),
      },
    );
    let options = WeakTypesCommand {
      ns: Some("js-ffi.raw.ids".to_owned()),
      ns_prefix: None,
      only: Some("unsafe-coerce".to_owned()),
      intent: None,
      format: "json".to_owned(),
      deps: false,
      summary_only: false,
    };
    let json = type_coverage::format_weak_types_json(&options, &snapshot).expect("unsafe evidence JSON should format");
    let value: serde_json::Value = serde_json::from_str(&json).expect("unsafe evidence JSON should parse");
    assert_eq!(value["schema_version"], 5);
    assert_eq!(
      value["data"]["definitions"][0]["occurrences"][0]["evidence"]["source_form"],
      "raw-js-value"
    );
    assert_eq!(
      value["data"]["definitions"][0]["occurrences"][0]["evidence"]["js_ffi_feature"],
      true
    );
    assert_eq!(
      value["data"]["definitions"][0]["occurrences"][0]["evidence"]["raw_adapter_namespace"],
      true
    );
  }

  #[test]
  fn analyze_weak_types_skips_nested_quoted_unsafe_coerce_but_keeps_unquote() {
    let unsafe_coerce = || list(vec![leaf("unsafe-coerce"), leaf("value"), leaf("String")]);
    let entry = code_entry(
      list(vec![
        leaf("defn"),
        leaf("template"),
        list(vec![]),
        list(vec![leaf("quote"), list(vec![leaf("do"), unsafe_coerce()])]),
        list(vec![leaf("quasiquote"), list(vec![leaf("do"), unsafe_coerce()])]),
        list(vec![
          leaf("quasiquote"),
          list(vec![leaf("do"), list(vec![leaf("~"), unsafe_coerce()])]),
        ]),
      ]),
      CalcitTypeAnnotation::Dynamic,
    );

    let row = type_coverage::analyze_weak_types_entry(
      "app.macro",
      "template",
      &entry,
      &BTreeSet::from([type_coverage::WeakTypeKind::UnsafeCoerce]),
    )
    .expect("the quasiquote unquote should remain executable");

    assert_eq!(row.occurrences.len(), 1, "occurrences: {:?}", row.occurrences);
    assert_eq!(row.occurrences[0].path, "code@5.1.1.1");
    assert_eq!(
      row.occurrences[0].unsafe_evidence.as_ref().map(|evidence| evidence.source_form),
      Some("value")
    );
  }

  #[test]
  fn analyze_weak_types_marks_dynamic_ffi_boundaries_as_intentional() {
    let entry = snapshot::CodeEntry {
      doc: "".to_owned(),
      examples: vec![],
      tests: vec![],
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
      ffi: None,
    };

    let row = type_coverage::analyze_weak_types_entry("app.main", "ffi-wrapper", &entry, &type_coverage::WeakTypeKind::all())
      .expect("should find weak types");

    for occurrence in row.occurrences.iter().filter(|occurrence| {
      !matches!(
        occurrence.kind,
        type_coverage::WeakTypeKind::CodeNil | type_coverage::WeakTypeKind::UnsafeCoerce
      )
    }) {
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
      tests: vec![],
      tags: HashSet::new(),
      code: list(vec![leaf("defn"), leaf("demo"), list(vec![leaf("value")]), leaf("nil")]),
      schema: CalcitTypeAnnotation::Fn(Arc::new(calcit::calcit::CalcitFnTypeAnnotation {
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types: vec![calcit::calcit::DYNAMIC_TYPE.clone()],
        return_type: calcit::calcit::DYNAMIC_TYPE.clone(),
        fn_kind: SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(HashSet::new()),
      }))
      .into(),
      ffi: None,
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
    assert_eq!(weak_value["schema_version"], 5);
    assert_eq!(weak_value["command"], "analyze.weak-types");
    assert_eq!(weak_value["data"]["filters"]["intent"], "unresolved");
    assert_eq!(weak_value["data"]["definitions"][0]["occurrences"][0]["path"], "schema.args.0");
    assert!(weak_value["data"]["definitions"][0]["occurrences"][0]["impact"].is_string());
    assert_eq!(weak_value["diagnostics"][0]["code"], "W_DYNAMIC_TYPE_DEBT");
    assert!(
      weak_value["diagnostics"]
        .as_array()
        .expect("diagnostics should be an array")
        .iter()
        .any(|diagnostic| diagnostic["code"] == "W_NIL_TYPE_DEBT")
    );
    assert_eq!(weak_value["data"]["summary"]["intents"]["declared-unit"], 0);
    assert_eq!(check_value["diagnostics"][0]["code"], "W_TYPE_COVERAGE_GAPS");

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
      tests: vec![],
      tags: std::collections::HashSet::new(),
      code: list(vec![
        leaf("defn"),
        leaf("demo"),
        list(vec![]),
        list(vec![leaf("assert-type"), leaf("x"), leaf(":dynamic")]),
        leaf("nil"),
      ]),
      schema: fn_schema_annotation(SchemaKind::Fn, 1, false).into(),
      ffi: None,
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
      tests: vec![],
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
      ffi: None,
    };

    let row = type_coverage::analyze_weak_types_entry("app.main", "branchy", &entry, &type_coverage::WeakTypeKind::all())
      .expect("should find hits");
    let details = row.occurrences.iter().map(|item| item.detail.as_str()).collect::<Vec<_>>();

    assert!(details.contains(&"schema-dynamic:rest"), "details: {details:?}");
    assert!(details.contains(&"code-nil:if-then"), "details: {details:?}");
    assert!(details.contains(&"code-nil:if-else"), "details: {details:?}");
  }

  #[test]
  fn analyze_weak_types_classifies_only_returned_nil_as_declared_unit_debt() {
    let unit_entry = snapshot::CodeEntry {
      doc: "".to_owned(),
      examples: vec![],
      tests: vec![],
      tags: HashSet::new(),
      code: list(vec![
        leaf("defn"),
        leaf("unit-step"),
        list(vec![]),
        list(vec![leaf("do"), leaf("nil"), leaf("nil")]),
      ]),
      schema: CalcitTypeAnnotation::Fn(Arc::new(calcit::calcit::CalcitFnTypeAnnotation {
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types: vec![],
        return_type: Arc::new(CalcitTypeAnnotation::Unit),
        fn_kind: SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(HashSet::new()),
      }))
      .into(),
      ffi: None,
    };
    let unit_row = type_coverage::analyze_weak_types_entry(
      "app.main",
      "unit-step",
      &unit_entry,
      &BTreeSet::from([type_coverage::WeakTypeKind::CodeNil]),
    )
    .expect("unit nil occurrences");
    assert_eq!(
      unit_row.occurrences[0].intent,
      type_coverage::WeakTypeIntent::Unresolved,
      "an intermediate Nil does not inherit the function return contract"
    );
    assert_eq!(
      unit_row.occurrences[1].intent,
      type_coverage::WeakTypeIntent::DeclaredUnit,
      "the returned Nil is identified as violating the Unit contract"
    );

    let single_do_entry = snapshot::CodeEntry {
      doc: "".to_owned(),
      examples: vec![],
      tests: vec![],
      tags: HashSet::new(),
      code: list(vec![
        leaf("defn"),
        leaf("single-unit-step"),
        list(vec![]),
        list(vec![leaf("do"), leaf("nil")]),
      ]),
      schema: unit_entry.schema.clone(),
      ffi: None,
    };
    let single_do_row = type_coverage::analyze_weak_types_entry(
      "app.main",
      "single-unit-step",
      &single_do_entry,
      &BTreeSet::from([type_coverage::WeakTypeKind::CodeNil]),
    )
    .expect("single-expression do nil occurrence");
    assert_eq!(
      single_do_row.occurrences[0].intent,
      type_coverage::WeakTypeIntent::DeclaredUnit,
      "the sole expression in do is its return position"
    );

    let unit_macro_entry = snapshot::CodeEntry {
      doc: "".to_owned(),
      examples: vec![],
      tests: vec![],
      tags: HashSet::new(),
      code: list(vec![leaf("defn"), leaf("explicit-unit"), list(vec![]), list(vec![leaf(";nil")])]),
      schema: unit_entry.schema.clone(),
      ffi: None,
    };
    let unit_macro_row = type_coverage::analyze_weak_types_entry(
      "app.main",
      "explicit-unit",
      &unit_macro_entry,
      &BTreeSet::from([type_coverage::WeakTypeKind::CodeNil]),
    )
    .expect("explicit ;nil occurrence");
    assert_eq!(unit_macro_row.occurrences[0].detail, "code-nil:nil-macro:literal");
    assert_eq!(unit_macro_row.occurrences[0].intent, type_coverage::WeakTypeIntent::DeclaredUnit);

    let quoted_nil_entry = snapshot::CodeEntry {
      doc: "".to_owned(),
      examples: vec![],
      tests: vec![],
      tags: HashSet::new(),
      code: list(vec![
        leaf("defn"),
        leaf("nil-code"),
        list(vec![]),
        list(vec![leaf("quote"), leaf("nil")]),
      ]),
      schema: unit_entry.schema.clone(),
      ffi: None,
    };
    assert!(
      type_coverage::analyze_weak_types_entry(
        "app.main",
        "nil-code",
        &quoted_nil_entry,
        &BTreeSet::from([type_coverage::WeakTypeKind::CodeNil]),
      )
      .is_none(),
      "quoted Nil is code data rather than a runtime nil value"
    );

    let empty_unit_entry = snapshot::CodeEntry {
      doc: "".to_owned(),
      examples: vec![],
      tests: vec![],
      tags: HashSet::new(),
      code: list(vec![leaf("defn"), leaf("implicit-unit"), list(vec![])]),
      schema: unit_entry.schema.clone(),
      ffi: None,
    };
    assert!(
      type_coverage::analyze_weak_types_entry(
        "app.main",
        "implicit-unit",
        &empty_unit_entry,
        &BTreeSet::from([type_coverage::WeakTypeKind::CodeNil]),
      )
      .is_none(),
      "an empty Unit function should not need an explicit nil form"
    );

    let optional_entry = snapshot::CodeEntry {
      doc: "".to_owned(),
      examples: vec![],
      tests: vec![],
      tags: HashSet::new(),
      code: list(vec![
        leaf("defn"),
        leaf("lookup"),
        list(vec![]),
        list(vec![leaf("if"), leaf("found?"), leaf("1"), leaf("nil")]),
      ]),
      schema: CalcitTypeAnnotation::Fn(Arc::new(calcit::calcit::CalcitFnTypeAnnotation {
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types: vec![],
        return_type: Arc::new(CalcitTypeAnnotation::Optional(Arc::new(CalcitTypeAnnotation::Number))),
        fn_kind: SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(HashSet::new()),
      }))
      .into(),
      ffi: None,
    };
    let optional_row = type_coverage::analyze_weak_types_entry(
      "app.main",
      "lookup",
      &optional_entry,
      &BTreeSet::from([type_coverage::WeakTypeKind::CodeNil]),
    )
    .expect("optional nil occurrence");
    assert_eq!(optional_row.occurrences[0].intent, type_coverage::WeakTypeIntent::DeclaredOptional);
  }

  #[test]
  fn analyze_weak_types_entry_classifies_nested_schema_dynamic_shapes() {
    let entry = snapshot::CodeEntry {
      doc: "".to_owned(),
      examples: vec![],
      tests: vec![],
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
      ffi: None,
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
      tests: vec![],
      tags: std::collections::HashSet::new(),
      code: leaf("demo"),
      schema: Arc::new(CalcitTypeAnnotation::Map(
        Arc::new(CalcitTypeAnnotation::Tag),
        Arc::new(CalcitTypeAnnotation::List(calcit::calcit::DYNAMIC_TYPE.clone())),
      )),
      ffi: None,
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

  #[test]
  fn affected_test_selection_uses_transitive_compiled_dependencies() {
    let _guard = GLOBAL_TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    builtins::effects::init_effects_states();
    injection::inject_platform_apis();

    let namespace = format!("app.test-metadata-{}", std::process::id());
    let snippet = format!("ns {namespace}\n\ndefn add (a b)\n  + a b\n\ndefn consumer ()\n  add 1 2\n\ndefn unrelated () nil");
    let mut project = snapshot::Snapshot::default();
    project.files.insert(
      namespace.clone(),
      snapshot::create_file_from_snippet(&snippet).expect("test project should parse"),
    );
    let file = project.files.get_mut(&namespace).expect("test namespace should exist");
    file.defs.get_mut("add").expect("add should exist").tests.push(snapshot::TestEntry {
      name: "direct".to_owned(),
      code: list(vec![leaf("assert="), leaf("3"), list(vec![leaf("add"), leaf("1"), leaf("2")])]),
      tags: HashSet::new(),
    });
    file
      .defs
      .get_mut("consumer")
      .expect("consumer should exist")
      .tests
      .push(snapshot::TestEntry {
        name: "transitive".to_owned(),
        code: list(vec![leaf("assert="), leaf("3"), list(vec![leaf("consumer")])]),
        tags: HashSet::new(),
      });
    file
      .defs
      .get_mut("unrelated")
      .expect("unrelated should exist")
      .tests
      .push(snapshot::TestEntry {
        name: "must-not-run".to_owned(),
        code: list(vec![leaf("raise"), leaf("|unrelated-test-ran")]),
        tags: HashSet::new(),
      });
    for (core_ns, core_file) in calcit::load_core_snapshot().expect("core snapshot should load").files {
      project.files.insert(core_ns, core_file);
    }

    let options = TestCommand {
      target: None,
      name: None,
      tag: vec![],
      exclude_tag: vec![],
      affected: vec![format!("{namespace}/add")],
      list: false,
      fail_fast: false,
      require_match: false,
      summary_only: false,
      format: "human".to_owned(),
    };
    let project_namespaces = HashSet::from([namespace]);
    run_tests(&options, &project, &project_namespaces).expect("only direct and transitive tests should run");
  }

  #[test]
  fn default_test_scope_excludes_core_and_dependency_namespaces() {
    let _guard = GLOBAL_TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    builtins::effects::init_effects_states();
    injection::inject_platform_apis();

    let namespace = format!("app.default-test-scope-{}", std::process::id());
    let snippet = format!("ns {namespace}\n\ndefn local () nil");
    let mut project = snapshot::Snapshot::default();
    project.files.insert(
      namespace.clone(),
      snapshot::create_file_from_snippet(&snippet).expect("test project should parse"),
    );
    project
      .files
      .get_mut(&namespace)
      .expect("project namespace should exist")
      .defs
      .get_mut("local")
      .expect("local should exist")
      .tests
      .push(snapshot::TestEntry {
        name: "project-test".to_owned(),
        code: leaf("true"),
        tags: HashSet::new(),
      });

    let mut core = calcit::load_core_snapshot().expect("core snapshot should load");
    core
      .files
      .get_mut("calcit.core")
      .expect("calcit.core should exist")
      .defs
      .get_mut("assert")
      .expect("calcit.core/assert should exist")
      .tests
      .push(snapshot::TestEntry {
        name: "must-not-run-by-default".to_owned(),
        code: list(vec![leaf("raise"), leaf("|core-test-ran")]),
        tags: HashSet::new(),
      });
    for (core_ns, core_file) in core.files {
      project.files.insert(core_ns, core_file);
    }

    let default_options = TestCommand {
      target: None,
      name: None,
      tag: vec![],
      exclude_tag: vec![],
      affected: vec![],
      list: false,
      fail_fast: false,
      require_match: false,
      summary_only: false,
      format: "human".to_owned(),
    };
    let project_namespaces = HashSet::from([namespace]);
    run_tests(&default_options, &project, &project_namespaces).expect("default scope should run only the project test");

    let explicit_core_options = TestCommand {
      target: Some("calcit.core/assert".to_owned()),
      name: Some("must-not-run-by-default".to_owned()),
      ..default_options
    };
    let error =
      run_tests(&explicit_core_options, &project, &project_namespaces).expect_err("an explicitly scoped core test should still run");
    assert!(error.contains("1 test(s) failed"), "unexpected error: {error}");
  }

  #[test]
  fn test_filters_can_exclude_tags_and_require_a_match() {
    let _guard = GLOBAL_TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    builtins::effects::init_effects_states();
    injection::inject_platform_apis();

    let namespace = format!("app.test-filter-{}", std::process::id());
    let snippet = format!("ns {namespace}\n\ndefn fast () true\n\ndefn slow () true");
    let mut project = snapshot::Snapshot::default();
    project.files.insert(
      namespace.clone(),
      snapshot::create_file_from_snippet(&snippet).expect("test project should parse"),
    );
    let file = project.files.get_mut(&namespace).expect("test namespace should exist");
    file
      .defs
      .get_mut("fast")
      .expect("fast should exist")
      .tests
      .push(snapshot::TestEntry {
        name: "fast-guard".to_owned(),
        code: leaf("true"),
        tags: HashSet::from([EdnTag::new("fast")]),
      });
    file
      .defs
      .get_mut("slow")
      .expect("slow should exist")
      .tests
      .push(snapshot::TestEntry {
        name: "slow-guard".to_owned(),
        code: list(vec![leaf("raise"), leaf("|slow-test-ran")]),
        tags: HashSet::from([EdnTag::new("slow")]),
      });
    for (core_ns, core_file) in calcit::load_core_snapshot().expect("core snapshot should load").files {
      project.files.insert(core_ns, core_file);
    }

    let options = TestCommand {
      target: None,
      name: None,
      tag: vec![],
      exclude_tag: vec!["slow".to_owned()],
      affected: vec![],
      list: false,
      fail_fast: false,
      require_match: false,
      summary_only: false,
      format: "human".to_owned(),
    };
    let project_namespaces = HashSet::from([namespace]);
    run_tests(&options, &project, &project_namespaces).expect("excluded slow test should not run");

    let missing_options = TestCommand {
      tag: vec!["missing".to_owned()],
      require_match: true,
      ..options
    };
    let error = run_tests(&missing_options, &project, &project_namespaces).expect_err("missing selection should fail when required");
    assert_eq!(error, "No tests matched the requested selection");
  }

  #[test]
  fn summary_test_report_omits_rows_and_tracks_executed_count() {
    let tests = vec![RunnableTest {
      namespace: "app.main".to_owned(),
      definition: "demo".to_owned(),
      name: "guard".to_owned(),
      synthetic_definition: "&calcit:test:0".to_owned(),
      code: leaf("true"),
    }];
    let report = make_test_report("run", &tests, 1, 0, 1.5, true, vec![]);
    assert_eq!(report.detail, "summary");
    assert_eq!(report.selected, 1);
    assert_eq!(report.executed, 1);
    assert!(report.tests.is_empty());
  }
}
