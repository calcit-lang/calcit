use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use calcit::calcit::LocatedWarning;
use calcit::call_stack::CallStackList;
use calcit::cli_args::{QualityCommand, VerifyCommand};
use calcit::snapshot::{self, VerificationCheckKind, VerificationFailurePolicy};
use calcit::{ProgramEntries, program, runner, util};
use md5::{Digest, Md5};
use serde::Serialize;

use crate::{apply_strict_feature_policy_defaults, attach_missing_core_namespaces, collect_dynamic_method_findings, quality_gate};

const VERIFY_OUTPUT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize)]
struct VerifyDiagnostic {
  code: String,
  phase: String,
  severity: String,
  message: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  detail: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
struct VerifyCheckResult {
  check: String,
  entry: String,
  target: String,
  revision: String,
  status: String,
  diagnostics: Vec<VerifyDiagnostic>,
}

#[derive(Debug, Clone)]
struct EntryInference {
  warnings: Vec<LocatedWarning>,
  error: Option<VerifyDiagnostic>,
}

fn content_revision(content: &str) -> String {
  let mut hasher = Md5::new();
  hasher.update(content.as_bytes());
  format!("md5:{}", hex::encode(hasher.finalize()))
}

fn target_label(entry: &snapshot::SnapshotEntry) -> String {
  entry
    .target
    .map(|target| target.as_str().to_owned())
    .unwrap_or_else(|| entry.mode.as_str().to_owned())
}

fn program_entries(snapshot: &snapshot::Snapshot) -> Result<ProgramEntries, String> {
  let entry = snapshot.active_entry()?;
  let (init_ns, init_def) = util::string::extract_ns_def(&entry.init_fn)?;
  let (reload_ns, reload_def) = util::string::extract_ns_def(&entry.reload_fn)?;
  Ok(ProgramEntries {
    init_fn: Arc::from(entry.init_fn.as_str()),
    reload_fn: Arc::from(entry.reload_fn.as_str()),
    init_ns: Arc::from(init_ns),
    init_def: Arc::from(init_def),
    reload_ns: Arc::from(reload_ns),
    reload_def: Arc::from(reload_def),
  })
}

fn prepare_entry_snapshot(
  base_snapshot: &snapshot::Snapshot,
  entry_name: &str,
  base_dir: &Path,
  module_folder: &Path,
  module_cache: &mut HashMap<String, snapshot::Snapshot>,
  core_snapshot: &snapshot::Snapshot,
  strict_diagnostics: bool,
) -> Result<snapshot::Snapshot, String> {
  let mut selected = base_snapshot.clone();
  selected.select_entry(Some(entry_name))?;
  let module_paths = selected.active_entry()?.modules.clone();
  for module_path in module_paths {
    if !module_cache.contains_key(&module_path) {
      let loaded = calcit::load_module(&module_path, base_dir, module_folder)?;
      module_cache.insert(module_path.clone(), loaded);
    }
    calcit::merge_project_module_files(
      &mut selected,
      module_cache.get(&module_path).expect("module inserted in cache"),
      &module_path,
    )?;
  }
  apply_strict_feature_policy_defaults(&mut selected, strict_diagnostics)?;
  attach_missing_core_namespaces(&mut selected, core_snapshot.clone());
  Ok(selected)
}

fn infer_entry(snapshot: &snapshot::Snapshot, project_namespaces: &HashSet<String>) -> EntryInference {
  let result = (|| {
    let entries = program_entries(snapshot)?;
    program::clear_runtime_caches_for_reload(entries.init_ns.clone(), entries.reload_ns.clone(), true)?;
    runner::preprocess::set_project_namespaces(project_namespaces);
    {
      let mut data = program::PROGRAM_CODE_DATA.write().map_err(|_| "open program data".to_owned())?;
      *data = program::extract_program_data(snapshot)?;
    }
    let builtin_warnings = RefCell::new(Vec::new());
    runner::preprocess::ensure_ns_def_compiled(
      calcit::calcit::CORE_NS,
      calcit::calcit::BUILTIN_IMPLS_ENTRY,
      &builtin_warnings,
      &CallStackList::default(),
    )?;
    let warnings = RefCell::new(Vec::new());
    runner::preprocess::ensure_ns_def_compiled(&entries.init_ns, &entries.init_def, &warnings, &CallStackList::default())?;
    runner::preprocess::ensure_ns_def_compiled(&entries.reload_ns, &entries.reload_def, &warnings, &CallStackList::default())?;
    Ok::<Vec<LocatedWarning>, calcit::calcit::CalcitErr>(warnings.into_inner())
  })();

  match result {
    Ok(warnings) => EntryInference { warnings, error: None },
    Err(error) => EntryInference {
      warnings: (*error.warnings).clone(),
      error: Some(VerifyDiagnostic {
        code: error.code.unwrap_or_else(|| "E_VERIFY_PREPROCESS".to_owned()),
        phase: "preprocess".to_owned(),
        severity: "error".to_owned(),
        message: error.msg,
        detail: error
          .location
          .map(|location| serde_json::json!({ "location": location.to_string() })),
      }),
    },
  }
}

fn warning_diagnostics(warnings: &[LocatedWarning]) -> Vec<VerifyDiagnostic> {
  warnings
    .iter()
    .map(|warning| VerifyDiagnostic {
      code: warning.code().unwrap_or("W_VERIFY_PREPROCESS").to_owned(),
      phase: "preprocess".to_owned(),
      severity: "warning".to_owned(),
      message: warning.message().to_owned(),
      detail: Some(warning.as_json()),
    })
    .collect()
}

fn evaluate_check(
  check: VerificationCheckKind,
  entry_name: &str,
  target: &str,
  revision: &str,
  selected: &snapshot::Snapshot,
  inference: Option<&EntryInference>,
  project_namespaces: &HashSet<String>,
) -> VerifyCheckResult {
  let mut diagnostics = Vec::new();
  match check {
    VerificationCheckKind::Strict => {
      let inference = inference.expect("strict check requires inference");
      if let Some(error) = &inference.error {
        diagnostics.push(error.clone());
      }
      diagnostics.extend(warning_diagnostics(&inference.warnings));
    }
    VerificationCheckKind::DynamicMethods => {
      let inference = inference.expect("dynamic-methods check requires inference");
      if let Some(error) = &inference.error {
        diagnostics.push(error.clone());
      } else {
        let findings = collect_dynamic_method_findings(inference.warnings.clone(), false, project_namespaces);
        diagnostics.extend(findings.iter().map(|warning| VerifyDiagnostic {
          code: warning.code().unwrap_or("E_DYNAMIC_METHOD_POLICY").to_owned(),
          phase: "analysis".to_owned(),
          severity: "error".to_owned(),
          message: warning.message().to_owned(),
          detail: Some(warning.as_json()),
        }));
      }
    }
    VerificationCheckKind::Quality => {
      let options = QualityCommand {
        ns: None,
        ns_prefix: None,
        deps: false,
        baseline: None,
        write_baseline: None,
        format: "json".to_owned(),
      };
      match quality_gate::analyze_quality(&options, selected) {
        Ok(outcome) => diagnostics.extend(outcome.violations.into_iter().map(|violation| VerifyDiagnostic {
          code: "E_QUALITY_REGRESSION".to_owned(),
          phase: "analysis".to_owned(),
          severity: "error".to_owned(),
          message: format!(
            "{} exceeds zero-debt limit: actual {}, limit {}",
            violation.metric, violation.actual, violation.limit
          ),
          detail: serde_json::to_value(violation).ok(),
        })),
        Err(message) => diagnostics.push(VerifyDiagnostic {
          code: "E_VERIFY_QUALITY".to_owned(),
          phase: "analysis".to_owned(),
          severity: "error".to_owned(),
          message,
          detail: None,
        }),
      }
    }
  }
  VerifyCheckResult {
    check: check.as_str().to_owned(),
    entry: entry_name.to_owned(),
    target: target.to_owned(),
    revision: revision.to_owned(),
    status: if diagnostics.is_empty() { "passed" } else { "failed" }.to_owned(),
    diagnostics,
  }
}

fn print_json(
  profile: &str,
  revision: Option<&str>,
  policy: Option<VerificationFailurePolicy>,
  results: &[VerifyCheckResult],
  error: Option<&str>,
) {
  let passed = error.is_none() && results.iter().all(|result| result.status == "passed");
  let diagnostics = error
    .map(|message| {
      vec![serde_json::json!({
        "code": "E_VERIFY_CONFIG",
        "phase": "configuration",
        "severity": "error",
        "message": message,
      })]
    })
    .unwrap_or_default();
  println!(
    "{}",
    serde_json::json!({
      "schema_version": VERIFY_OUTPUT_SCHEMA_VERSION,
      "command": "analyze.verify",
      "revision": revision,
      "data": {
        "profile": profile,
        "on_failure": policy.map(VerificationFailurePolicy::as_str),
        "status": if passed { "passed" } else { "failed" },
        "checks": results,
      },
      "diagnostics": diagnostics,
    })
  );
}

fn print_human(profile: &str, revision: &str, policy: VerificationFailurePolicy, results: &[VerifyCheckResult]) {
  println!("# Verification `{profile}`\n");
  println!("- revision: `{revision}`");
  println!("- on failure: `{}`", policy.as_str());
  println!(
    "- status: **{}**",
    if results.iter().all(|result| result.status == "passed") {
      "PASS"
    } else {
      "FAIL"
    }
  );
  for result in results {
    println!("\n## `{}` · `{}`\n", result.entry, result.check);
    println!("- target: `{}`", result.target);
    println!("- status: **{}**", result.status.to_uppercase());
    for diagnostic in &result.diagnostics {
      println!("- `{}`: {}", diagnostic.code, diagnostic.message);
    }
  }
}

fn run_inner(options: &VerifyCommand, snapshot_file: &str, strict_diagnostics: bool) -> Result<(), String> {
  let input_path = PathBuf::from(snapshot_file);
  calcit::validate_snapshot_path(&input_path)?;
  let mut content =
    fs::read_to_string(&input_path).map_err(|error| format!("Failed to read Snapshot '{}': {error}", input_path.display()))?;
  util::string::strip_shebang(&mut content);
  let revision = content_revision(&content);
  let data = cirru_edn::parse(&content).map_err(|error| format!("Failed to parse Snapshot '{}': {error}", input_path.display()))?;
  let base_snapshot = snapshot::load_snapshot_data(&data, snapshot_file)?;
  let profile = base_snapshot.verification.profiles.get(&options.profile).cloned().ok_or_else(|| {
    let mut available = base_snapshot.verification.profiles.keys().cloned().collect::<Vec<_>>();
    available.sort();
    format!(
      "Unknown verification profile `{}`. Available profiles: {}",
      options.profile,
      if available.is_empty() {
        "<none>".to_owned()
      } else {
        available.join(", ")
      }
    )
  })?;

  let base_dir = input_path.parent().unwrap_or(Path::new("."));
  let module_folder = calcit::project_module_folder(base_dir);
  let core_snapshot = calcit::load_core_snapshot()?;
  let project_namespaces = base_snapshot.files.keys().cloned().collect::<HashSet<_>>();
  let mut module_cache = HashMap::new();
  let mut results = Vec::new();
  let mut stopped = false;

  for entry_name in &profile.entries {
    let selected = prepare_entry_snapshot(
      &base_snapshot,
      entry_name,
      base_dir,
      &module_folder,
      &mut module_cache,
      &core_snapshot,
      strict_diagnostics,
    )?;
    let entry = selected.active_entry()?;
    let target = target_label(entry);
    let mut inference = None;
    for check in &profile.checks {
      if matches!(check, VerificationCheckKind::Strict | VerificationCheckKind::DynamicMethods) && inference.is_none() {
        inference = Some(infer_entry(&selected, &project_namespaces));
      }
      let result = evaluate_check(
        *check,
        entry_name,
        &target,
        &revision,
        &selected,
        inference.as_ref(),
        &project_namespaces,
      );
      let failed = result.status == "failed";
      results.push(result);
      if failed && profile.on_failure == VerificationFailurePolicy::Stop {
        stopped = true;
        break;
      }
    }
    if stopped {
      break;
    }
  }

  match options.format.as_str() {
    "human" | "text" => print_human(&options.profile, &revision, profile.on_failure, &results),
    "json" => print_json(&options.profile, Some(&revision), Some(profile.on_failure), &results, None),
    _ => unreachable!("verification output format validated before execution"),
  }
  if results.iter().all(|result| result.status == "passed") {
    Ok(())
  } else {
    Err(format!("Verification profile `{}` failed", options.profile))
  }
}

pub fn run(options: &VerifyCommand, snapshot_file: &str, strict_diagnostics: bool) -> Result<(), String> {
  if !matches!(options.format.as_str(), "human" | "text" | "json") {
    return Err(format!(
      "Unknown verify output format `{}`. Expected `human` or `json`.",
      options.format
    ));
  }
  match run_inner(options, snapshot_file, strict_diagnostics) {
    Ok(()) => Ok(()),
    Err(message) => {
      if options.format == "json" && !message.starts_with("Verification profile `") {
        let revision = fs::read_to_string(snapshot_file).ok().map(|content| content_revision(&content));
        print_json(&options.profile, revision.as_deref(), None, &[], Some(&message));
      }
      Err(message)
    }
  }
}
