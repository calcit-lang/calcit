//! Deprecated CLI aliases (#1566) keep their stdout contract for one non-patch
//! release, print a single migration hint on stderr, and match the replacement form.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const HINT_PREFIX: &str = "[Deprecated] `calcit ";

fn run_calcit(snapshot: &Path, args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .arg(snapshot)
    .args(args)
    .output()
    .expect("calcit command should run")
}

/// Timing fields are the only nondeterministic part of these reports.
fn stable_stdout(output: &Output) -> String {
  String::from_utf8_lossy(&output.stdout)
    .lines()
    .map(|line| {
      if line.starts_with("- result:") {
        line.split(" (").next().unwrap_or(line).to_owned()
      } else if line.contains("duration_ms") || line.contains("duration-ms") {
        line.split("duration").next().unwrap_or(line).to_owned()
      } else {
        line.to_owned()
      }
    })
    .collect::<Vec<_>>()
    .join("\n")
}

fn hint_lines(output: &Output) -> Vec<String> {
  String::from_utf8_lossy(&output.stderr)
    .lines()
    .filter(|line| line.starts_with(HINT_PREFIX))
    .map(str::to_owned)
    .collect()
}

fn assert_alias_matches(snapshot: &Path, old: &[&str], new: &[&str], replacement: &str) {
  let old_output = run_calcit(snapshot, old);
  let new_output = run_calcit(snapshot, new);
  assert_eq!(
    old_output.status.code(),
    new_output.status.code(),
    "{old:?} and {new:?} should exit alike\nold stderr:\n{}\nnew stderr:\n{}",
    String::from_utf8_lossy(&old_output.stderr),
    String::from_utf8_lossy(&new_output.stderr)
  );
  assert!(!old_output.stdout.is_empty(), "{old:?} should report on stdout");
  assert_eq!(stable_stdout(&old_output), stable_stdout(&new_output), "{old:?} vs {new:?}");
  let hints = hint_lines(&old_output);
  assert_eq!(hints.len(), 1, "{old:?} should print exactly one hint: {hints:?}");
  assert!(
    hints[0].contains(replacement),
    "{old:?} hint should name `{replacement}`: {}",
    hints[0]
  );
  assert!(hint_lines(&new_output).is_empty(), "{new:?} must not print a deprecation hint");
  assert!(
    !String::from_utf8_lossy(&old_output.stdout).contains("[Deprecated]"),
    "{old:?} must keep the hint off stdout"
  );
}

#[test]
fn type_debt_aliases_match_weak_types_views() {
  let snapshot = Path::new("calcit/test.cirru");
  assert_alias_matches(
    snapshot,
    &["analyze", "check-types", "--summary-only"],
    &["analyze", "weak-types", "--only", "coverage", "--summary-only"],
    "weak-types --only coverage",
  );
  assert_alias_matches(
    snapshot,
    &["analyze", "check-types", "--only", "none", "--ns", "app.main", "--format", "json"],
    &[
      "analyze",
      "weak-types",
      "--only",
      "coverage",
      "--coverage-level",
      "none",
      "--ns",
      "app.main",
      "--format",
      "json",
    ],
    "weak-types --only coverage",
  );
  assert_alias_matches(
    snapshot,
    &["analyze", "deprecated", "--format", "json"],
    &["analyze", "weak-types", "--only", "deprecated-call", "--format", "json"],
    "weak-types --only deprecated-call",
  );
  assert_alias_matches(
    Path::new("calcit/test-wasm.cirru"),
    &["analyze", "dynamic-methods", "--format", "json"],
    &["analyze", "weak-types", "--only", "dynamic-method", "--format", "json"],
    "weak-types --only dynamic-method",
  );
}

#[test]
fn weak_types_views_offer_edn_equivalent_to_json() {
  for (snapshot, view) in [
    ("calcit/test.cirru", "coverage"),
    ("calcit/test.cirru", "deprecated-call"),
    ("calcit/test-wasm.cirru", "dynamic-method"),
  ] {
    let args = |format| vec!["analyze", "weak-types", "--only", view, "--summary-only", "--format", format];
    let json = run_calcit(Path::new(snapshot), &args("json"));
    let edn = run_calcit(Path::new(snapshot), &args("edn"));
    assert!(edn.status.success(), "{view}: {}", String::from_utf8_lossy(&edn.stderr));
    let json: serde_json::Value = serde_json::from_slice(&json.stdout).expect("one JSON document");
    let cirru_edn::Edn::Map(root) = cirru_edn::parse(&String::from_utf8_lossy(&edn.stdout)).expect("one Cirru EDN document") else {
      panic!("{view}: EDN envelope should be a map");
    };
    assert_eq!(
      root.get(&cirru_edn::Edn::tag("command")),
      Some(&cirru_edn::Edn::str(json["command"].as_str().expect("command name"))),
      "{view}"
    );
    assert_eq!(
      root.get(&cirru_edn::Edn::tag("revision")),
      Some(&cirru_edn::Edn::str(json["revision"].as_str().expect("revision"))),
      "{view}"
    );
  }
}

#[test]
fn weak_types_views_reject_options_they_cannot_honor() {
  let snapshot = Path::new("calcit/test.cirru");
  for args in [
    &["analyze", "weak-types", "--only", "coverage,code-nil"][..],
    &["analyze", "weak-types", "--only", "dynamic-method", "--ns", "app.main"][..],
    &["analyze", "weak-types", "--coverage-level", "none"][..],
  ] {
    let output = run_calcit(snapshot, args);
    assert!(!output.status.success(), "{args:?} should be rejected");
    assert!(output.stdout.is_empty(), "{args:?} should not report on stdout");
  }
}

#[test]
fn check_public_alias_matches_check_only_namespace_scope() {
  let snapshot = Path::new("calcit/test-wasi-command.cirru");
  assert_alias_matches(
    snapshot,
    &["analyze", "check-public", "--ns", "calcit.test", "--deps", "--summary-only"],
    &["--check-only", "--ns", "calcit.test", "--deps", "--summary-only"],
    "--check-only --ns",
  );
  assert_alias_matches(
    snapshot,
    &["analyze", "check-public", "--ns", "calcit.test", "--deps", "--format", "json"],
    &["--check-only", "--ns", "calcit.test", "--deps", "--format", "json"],
    "--check-only --ns",
  );
  // A namespace that is not loaded fails the same way in both forms.
  assert_alias_matches(
    snapshot,
    &["analyze", "check-public", "--ns", "app.missing", "--format", "json"],
    &["--check-only", "--ns", "app.missing", "--format", "json"],
    "--check-only --ns",
  );

  let json = run_calcit(snapshot, &["--check-only", "--ns", "calcit.test", "--deps", "--format", "json"]);
  let edn = run_calcit(snapshot, &["--check-only", "--ns", "calcit.test", "--deps", "--format", "edn"]);
  assert!(edn.status.success(), "{}", String::from_utf8_lossy(&edn.stderr));
  let json: serde_json::Value = serde_json::from_slice(&json.stdout).expect("one JSON document");
  let edn = cirru_edn::parse(&String::from_utf8_lossy(&edn.stdout)).expect("one Cirru EDN document");
  let cirru_edn::Edn::Map(root) = edn else {
    panic!("EDN envelope should be a map");
  };
  assert_eq!(
    root.get(&cirru_edn::Edn::tag("command")),
    Some(&cirru_edn::Edn::str(json["command"].as_str().expect("command name")))
  );
  let Some(cirru_edn::Edn::Map(data)) = root.get(&cirru_edn::Edn::tag("data")) else {
    panic!("EDN data should be a map");
  };
  let Some(cirru_edn::Edn::List(ids)) = data.get(&cirru_edn::Edn::tag("checked-definition-ids")) else {
    panic!("EDN should list checked definition ids");
  };
  assert_eq!(ids.len(), json["data"]["checked_definition_ids"].as_array().expect("ids").len());

  for args in [
    &["--ns", "calcit.test"][..],
    &["--check-only", "--deps"][..],
    &["--check-only", "--ns", "calcit.test", "--keep-going"][..],
  ] {
    let output = run_calcit(snapshot, args);
    assert!(!output.status.success(), "{args:?} should be rejected");
  }
}

#[test]
fn quality_and_query_aliases_keep_output_and_hint() {
  let snapshot = Path::new("calcit/test.cirru");
  let quality = run_calcit(snapshot, &["analyze", "quality", "--format", "json"]);
  let hints = hint_lines(&quality);
  assert_eq!(hints.len(), 1, "{hints:?}");
  assert!(hints[0].contains("--check-only") && hints[0].contains("weak-types"), "{}", hints[0]);
  serde_json::from_slice::<serde_json::Value>(&quality.stdout).expect("quality keeps one JSON document on stdout");

  let pkg = run_calcit(snapshot, &["query", "pkg"]);
  assert!(pkg.status.success());
  assert_eq!(String::from_utf8_lossy(&pkg.stdout), "app\n");
  assert!(hint_lines(&pkg)[0].contains("`calcit query config`"));
  let config = run_calcit(snapshot, &["query", "config"]);
  assert!(String::from_utf8_lossy(&config.stdout).contains("Package: `app`"));
  assert!(hint_lines(&config).is_empty());

  let modules = run_calcit(snapshot, &["query", "modules"]);
  assert!(modules.status.success());
  assert!(hint_lines(&modules)[0].contains("`calcit config modules`"));
  let config_modules = run_calcit(snapshot, &["config", "modules"]);
  assert!(hint_lines(&config_modules).is_empty());
  let module_lines = |output: &Output| {
    String::from_utf8_lossy(&output.stdout)
      .lines()
      .filter(|line| line.starts_with("  "))
      .map(str::to_owned)
      .collect::<Vec<_>>()
  };
  assert_eq!(module_lines(&modules), module_lines(&config_modules));
}

struct TempProject(PathBuf);

impl TempProject {
  fn create(name: &str) -> Self {
    let path = std::env::temp_dir().join(format!("calcit-deprecated-aliases-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("temporary project should create");
    let project = Self(path);
    fs::copy("calcit/add.cirru", project.snapshot()).expect("copy minimal snapshot");
    let output = run_calcit(
      &project.snapshot(),
      &["edit", "def", "app.main/items", "--code", "quote $ def items $ [] 1 2 3 4"],
    );
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    project
  }

  fn snapshot(&self) -> PathBuf {
    self.0.join("calcit.cirru")
  }
}

impl Drop for TempProject {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

#[test]
fn batch_delete_alias_matches_edit_transaction() {
  let legacy = TempProject::create("batch");
  let output = run_calcit(
    &legacy.snapshot(),
    &["tree", "batch-delete", "app.main/items", "--paths", "@2.1", "--paths", "@2.3"],
  );
  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  let hints = hint_lines(&output);
  assert_eq!(hints.len(), 1, "{hints:?}");
  assert!(hints[0].contains("edit transaction"), "{}", hints[0]);

  let transaction = TempProject::create("transaction");
  let output = run_calcit(
    &transaction.snapshot(),
    &[
      "edit",
      "transaction",
      "--code",
      r#"[["tree","delete","app.main/items","--path","@2.3"],["tree","delete","app.main/items","--path","@2.1"]]"#,
    ],
  );
  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  assert!(hint_lines(&output).is_empty());

  let read = |project: &TempProject| fs::read_to_string(project.snapshot()).expect("snapshot should read");
  assert_eq!(read(&legacy), read(&transaction));
  assert!(read(&legacy).contains("[] 2 4"), "{}", read(&legacy));
}
