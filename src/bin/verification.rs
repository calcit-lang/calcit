use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use calcit::calcit::LocatedWarning;
use calcit::call_stack::CallStackList;
use calcit::cli_args::VerifyCommand;
use calcit::snapshot::{self, VerificationCheckKind, VerificationFailurePolicy};
use calcit::{ProgramEntries, program, runner, util};
use md5::{Digest, Md5};
use serde::Serialize;

use crate::cli_handlers::{StructuredOutputFormat, format_json_value_as_edn};
use crate::{apply_strict_feature_policy_defaults, attach_missing_core_namespaces};

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

#[derive(Debug, Clone, Serialize)]
struct PreflightSnapshot {
  path: String,
  format: &'static str,
  revision: String,
  active_entries: Vec<String>,
  targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PreflightToolEvidence {
  tool: String,
  declared: Option<String>,
  observed: Option<String>,
  sources: Vec<String>,
  status: String,
  next: String,
}

#[derive(Debug, Clone, Serialize)]
struct PreflightExternalGate {
  name: String,
  status: &'static str,
  executed: bool,
  next: &'static str,
}

#[derive(Debug)]
struct ProcsEvidence {
  declared: String,
  installed: Option<String>,
  locked: Option<String>,
  sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PreflightReport {
  status: String,
  snapshot: PreflightSnapshot,
  tools: Vec<PreflightToolEvidence>,
  external_gates: Vec<PreflightExternalGate>,
  diagnostics: Vec<VerifyDiagnostic>,
}

impl PreflightReport {
  pub(crate) fn is_failed(&self) -> bool {
    self.status == "failed"
  }
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

fn parse_version_text(output: &str) -> Option<semver::Version> {
  output
    .split(|ch: char| !(ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '+')))
    .filter(|part| !part.is_empty())
    .map(|part| part.strip_prefix('v').unwrap_or(part))
    .find_map(|part| semver::Version::parse(part).ok())
}

fn observe_tool_version(tool: &str) -> Result<String, String> {
  let executable = if tool == "rustc" { "rustc" } else { tool };
  let output = Command::new(executable)
    .arg("--version")
    .output()
    .map_err(|error| format!("failed to execute `{executable} --version`: {error}"))?;
  if !output.status.success() {
    return Err(format!(
      "`{executable} --version` exited with status {}: {}",
      output.status,
      String::from_utf8_lossy(&output.stderr).trim()
    ));
  }
  let stdout = String::from_utf8_lossy(&output.stdout);
  parse_version_text(&stdout).map(|version| version.to_string()).ok_or_else(|| {
    format!(
      "could not parse a semantic version from `{executable} --version`: {}",
      stdout.trim()
    )
  })
}

fn read_declared_calcit_version(base_dir: &Path) -> Result<Option<(String, String)>, String> {
  let path = base_dir.join("deps.cirru");
  if !path.is_file() {
    return Ok(None);
  }
  let content = fs::read_to_string(&path).map_err(|error| format!("Failed to read '{}': {error}", path.display()))?;
  let data = cirru_edn::parse(&content).map_err(|error| format!("Failed to parse '{}': {error}", path.display()))?;
  let deps = data
    .view_map()
    .map_err(|error| format!("Invalid dependency manifest '{}': {error}", path.display()))?;
  let value = deps.get_or_nil("calcit-version");
  if matches!(value, cirru_edn::Edn::Nil) {
    return Ok(None);
  }
  let declared = match value {
    cirru_edn::Edn::Str(value) | cirru_edn::Edn::Symbol(value) => value.to_string(),
    _ => {
      return Err(format!(
        "Invalid :calcit-version in '{}': expected a string, got {value}",
        path.display()
      ));
    }
  };
  semver::Version::parse(&declared)
    .map_err(|error| format!("Invalid :calcit-version '{declared}' in '{}': {error}", path.display()))?;
  Ok(Some((declared, path.display().to_string())))
}

fn package_requirement(value: &serde_json::Value) -> Option<String> {
  ["dependencies", "devDependencies"]
    .iter()
    .find_map(|group| value.get(group)?.get("@calcit/procs")?.as_str().map(ToOwned::to_owned))
}

fn read_procs_evidence(base_dir: &Path) -> Result<Option<ProcsEvidence>, String> {
  let package_path = base_dir.join("package.json");
  if !package_path.is_file() {
    return Ok(None);
  }
  let content = fs::read_to_string(&package_path).map_err(|error| format!("Failed to read '{}': {error}", package_path.display()))?;
  let package: serde_json::Value =
    serde_json::from_str(&content).map_err(|error| format!("Failed to parse '{}': {error}", package_path.display()))?;
  let Some(mut declared) = package_requirement(&package) else {
    return Ok(None);
  };
  if declared == "workspace:."
    && let Some(version) = package.get("version").and_then(serde_json::Value::as_str)
  {
    declared = version.to_owned();
  }
  let mut sources = vec![package_path.display().to_string()];
  let installed_path = base_dir.join("node_modules/@calcit/procs/package.json");
  let installed = if installed_path.is_file() {
    let content =
      fs::read_to_string(&installed_path).map_err(|error| format!("Failed to read '{}': {error}", installed_path.display()))?;
    let value: serde_json::Value =
      serde_json::from_str(&content).map_err(|error| format!("Failed to parse '{}': {error}", installed_path.display()))?;
    sources.push(installed_path.display().to_string());
    value.get("version").and_then(serde_json::Value::as_str).map(ToOwned::to_owned)
  } else {
    None
  };
  let lock_path = base_dir.join("yarn.lock");
  let locked = if lock_path.is_file() {
    let content = fs::read_to_string(&lock_path).map_err(|error| format!("Failed to read '{}': {error}", lock_path.display()))?;
    let mut in_procs = false;
    let mut found = None;
    for line in content.lines() {
      if !line.starts_with(' ') {
        in_procs = line.contains("@calcit/procs@");
      } else if in_procs && (line.trim_start().starts_with("version:") || line.trim_start().starts_with("resolution:")) {
        found = parse_version_text(line).map(|version| version.to_string());
        if found.is_some() {
          break;
        }
      }
    }
    sources.push(lock_path.display().to_string());
    found
  } else {
    None
  };
  Ok(Some(ProcsEvidence {
    declared,
    installed,
    locked,
    sources,
  }))
}

fn preflight_diagnostic(code: &str, message: String, detail: serde_json::Value) -> VerifyDiagnostic {
  VerifyDiagnostic {
    code: code.to_owned(),
    phase: "preflight".to_owned(),
    severity: "error".to_owned(),
    message,
    detail: Some(detail),
  }
}

pub(crate) fn collect_preflight(
  snapshot: &snapshot::Snapshot,
  snapshot_file: &str,
  revision: &str,
  active_entries: &[String],
) -> Result<PreflightReport, String> {
  let input_path = PathBuf::from(snapshot_file);
  let base_dir = input_path.parent().unwrap_or(Path::new("."));
  let mut diagnostics = Vec::new();
  let mut tools = Vec::new();

  let declared_calcit = read_declared_calcit_version(base_dir)?;
  let current_calcit = env!("CARGO_PKG_VERSION").to_owned();
  let calcit_matches = declared_calcit.as_ref().is_none_or(|(declared, _)| declared == &current_calcit);
  if !calcit_matches {
    let (declared, source) = declared_calcit.as_ref().expect("mismatched declaration should exist");
    diagnostics.push(preflight_diagnostic(
      "E_PREFLIGHT_CALCIT_VERSION",
      format!("Declared Calcit {declared} does not match the running Calcit {current_calcit}."),
      serde_json::json!({ "tool": "calcit", "declared": declared, "observed": current_calcit.clone(), "source": source }),
    ));
  }
  tools.push(PreflightToolEvidence {
    tool: "calcit".to_owned(),
    declared: declared_calcit.as_ref().map(|(version, _)| version.clone()),
    observed: Some(current_calcit.clone()),
    sources: declared_calcit.iter().map(|(_, path)| path.clone()).collect(),
    status: if declared_calcit.is_none() {
      "observed"
    } else if calcit_matches {
      "matched"
    } else {
      "mismatch"
    }
    .to_owned(),
    next: if calcit_matches {
      "No action required.".to_owned()
    } else {
      "Use the declared Calcit version or explicitly upgrade deps.cirru first.".to_owned()
    },
  });

  if let Some(ProcsEvidence {
    declared,
    installed,
    locked,
    sources,
  }) = read_procs_evidence(base_dir)?
  {
    let exact_declared = semver::Version::parse(declared.strip_prefix("npm:").unwrap_or(&declared)).ok();
    let installed_version = installed.as_deref().and_then(|value| semver::Version::parse(value).ok());
    let locked_version = locked.as_deref().and_then(|value| semver::Version::parse(value).ok());
    let calcit_version = semver::Version::parse(&current_calcit).expect("crate version should be semantic");
    let matched = exact_declared.as_ref() == Some(&calcit_version)
      && installed_version.as_ref().is_none_or(|version| version == &calcit_version)
      && locked_version.as_ref().is_none_or(|version| version == &calcit_version);
    if !matched {
      diagnostics.push(preflight_diagnostic(
        "E_PREFLIGHT_PROCS_VERSION",
        format!("@calcit/procs must resolve exactly to the running Calcit version {current_calcit}."),
        serde_json::json!({
          "tool": "@calcit/procs",
          "declared": declared.clone(),
          "installed": installed.clone(),
          "locked": locked.clone(),
          "calcit": current_calcit.clone()
        }),
      ));
    }
    tools.push(PreflightToolEvidence {
      tool: "@calcit/procs".to_owned(),
      declared: Some(declared),
      observed: installed.or(locked),
      sources,
      status: if matched { "matched" } else { "mismatch" }.to_owned(),
      next: if matched {
        "No action required.".to_owned()
      } else {
        format!("Pin @calcit/procs to {current_calcit}, refresh the lockfile, and install dependencies.")
      },
    });
  }

  let declared_hosts = snapshot
    .verification
    .host_requirements
    .iter()
    .map(|(tool, requirement)| (tool.clone(), requirement.clone()))
    .collect::<BTreeMap<_, _>>();
  for (tool, requirement) in declared_hosts {
    let observed = observe_tool_version(&tool).ok();
    let matched = observed.as_deref().is_some_and(|version| {
      semver::VersionReq::parse(&requirement)
        .expect("host requirements are validated while loading")
        .matches(&semver::Version::parse(version).expect("observed host version is semantic"))
    });
    if !matched {
      diagnostics.push(preflight_diagnostic(
        if observed.is_some() {
          "E_PREFLIGHT_HOST_VERSION"
        } else {
          "E_PREFLIGHT_HOST_MISSING"
        },
        format!(
          "Declared {tool} requirement {requirement} is not satisfied by {}.",
          observed.as_deref().unwrap_or("the current PATH")
        ),
        serde_json::json!({ "tool": tool.clone(), "declared": requirement.clone(), "observed": observed.clone() }),
      ));
    }
    tools.push(PreflightToolEvidence {
      tool: tool.clone(),
      declared: Some(requirement.clone()),
      observed,
      sources: vec![format!("verification.host-requirements.{tool}")],
      status: if matched { "matched" } else { "mismatch" }.to_owned(),
      next: if matched {
        "No action required.".to_owned()
      } else {
        format!("Install a {tool} version satisfying {requirement} before running external gates.")
      },
    });
  }
  tools.sort_by(|left, right| left.tool.cmp(&right.tool));

  let targets = active_entries
    .iter()
    .filter_map(|name| snapshot.entries.get(name).map(|entry| format!("{name}:{}", target_label(entry))))
    .collect::<Vec<_>>();
  let external_gates = snapshot
    .verification
    .external_gates
    .iter()
    .map(|name| PreflightExternalGate {
      name: name.clone(),
      status: "caller-required",
      executed: false,
      next: "Run this gate in the project CI or invoking workflow; Calcit does not execute it.",
    })
    .collect();

  Ok(PreflightReport {
    status: if diagnostics.is_empty() { "passed" } else { "failed" }.to_owned(),
    snapshot: PreflightSnapshot {
      path: snapshot_file.to_owned(),
      format: "cirru-edn",
      revision: revision.to_owned(),
      active_entries: active_entries.to_vec(),
      targets,
    },
    tools,
    external_gates,
    diagnostics,
  })
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
  inference: &EntryInference,
) -> VerifyCheckResult {
  let mut diagnostics = Vec::new();
  match check {
    VerificationCheckKind::Strict => {
      if let Some(error) = &inference.error {
        diagnostics.push(error.clone());
      }
      diagnostics.extend(warning_diagnostics(&inference.warnings));
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

fn output_value(
  profile: &str,
  revision: Option<&str>,
  policy: Option<VerificationFailurePolicy>,
  results: &[VerifyCheckResult],
  preflight: Option<&PreflightReport>,
  error: Option<&str>,
) -> serde_json::Value {
  let passed =
    error.is_none() && results.iter().all(|result| result.status == "passed") && preflight.is_none_or(|report| !report.is_failed());
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
  serde_json::json!({
    "schema_version": VERIFY_OUTPUT_SCHEMA_VERSION,
    "command": "analyze.verify",
    "revision": revision,
    "data": {
      "profile": profile,
      "on_failure": policy.map(VerificationFailurePolicy::as_str),
      "status": if passed { "passed" } else { "failed" },
      "preflight": preflight,
      "checks": results,
    },
    "diagnostics": diagnostics,
  })
}

fn print_structured(
  format: StructuredOutputFormat,
  profile: &str,
  revision: Option<&str>,
  policy: Option<VerificationFailurePolicy>,
  results: &[VerifyCheckResult],
  preflight: Option<&PreflightReport>,
  error: Option<&str>,
) -> Result<(), String> {
  let value = output_value(profile, revision, policy, results, preflight, error);
  match format {
    StructuredOutputFormat::Edn => println!("{}", format_json_value_as_edn(&value)?),
    StructuredOutputFormat::Json => println!("{value}"),
    StructuredOutputFormat::Human => unreachable!("human verification output is rendered separately"),
  }
  Ok(())
}

fn print_human(
  profile: &str,
  revision: &str,
  policy: VerificationFailurePolicy,
  results: &[VerifyCheckResult],
  preflight: &PreflightReport,
) {
  println!("# Verification `{profile}`\n");
  println!("- revision: `{revision}`");
  println!("- on failure: `{}`", policy.as_str());
  println!(
    "- status: **{}**",
    if results.iter().all(|result| result.status == "passed") && !preflight.is_failed() {
      "PASS"
    } else {
      "FAIL"
    }
  );
  println!("\n## Preflight\n");
  println!("- status: **{}**", preflight.status.to_uppercase());
  for tool in &preflight.tools {
    println!(
      "- `{}`: {} (declared: `{}`, observed: `{}`)",
      tool.tool,
      tool.status,
      tool.declared.as_deref().unwrap_or("not declared"),
      tool.observed.as_deref().unwrap_or("not found")
    );
  }
  for gate in &preflight.external_gates {
    println!("- external gate `{}`: caller must run it", gate.name);
  }
  for result in results {
    println!("\n## `{}` · `{}`\n", result.entry, result.check);
    println!("- target: `{}`", result.target);
    println!("- status: **{}**", result.status.to_uppercase());
    for diagnostic in &result.diagnostics {
      println!("- `{}`: {}", diagnostic.code, diagnostic.message);
    }
  }
}

fn run_inner(
  options: &VerifyCommand,
  output_format: StructuredOutputFormat,
  snapshot_file: &str,
  strict_diagnostics: bool,
) -> Result<(), String> {
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
  let preflight = collect_preflight(&base_snapshot, snapshot_file, &revision, &profile.entries)?;

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
    let inference = infer_entry(&selected, &project_namespaces);
    for check in &profile.checks {
      let result = evaluate_check(*check, entry_name, &target, &revision, &inference);
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

  match output_format {
    StructuredOutputFormat::Human => print_human(&options.profile, &revision, profile.on_failure, &results, &preflight),
    StructuredOutputFormat::Edn | StructuredOutputFormat::Json => print_structured(
      output_format,
      &options.profile,
      Some(&revision),
      Some(profile.on_failure),
      &results,
      Some(&preflight),
      None,
    )?,
  }
  if results.iter().all(|result| result.status == "passed") && !preflight.is_failed() {
    Ok(())
  } else {
    Err(format!("Verification profile `{}` failed", options.profile))
  }
}

pub fn run(options: &VerifyCommand, snapshot_file: &str, strict_diagnostics: bool) -> Result<(), String> {
  let output_format = StructuredOutputFormat::parse(&options.format, "verify")?;
  match run_inner(options, output_format, snapshot_file, strict_diagnostics) {
    Ok(()) => Ok(()),
    Err(message) => {
      if matches!(output_format, StructuredOutputFormat::Edn | StructuredOutputFormat::Json)
        && !message.starts_with("Verification profile `")
      {
        let revision = fs::read_to_string(snapshot_file).ok().map(|content| content_revision(&content));
        print_structured(
          output_format,
          &options.profile,
          revision.as_deref(),
          None,
          &[],
          None,
          Some(&message),
        )?;
      }
      Err(message)
    }
  }
}
