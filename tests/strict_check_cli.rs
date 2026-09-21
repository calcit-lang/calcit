use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEST_DIRECTORY_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
  fn create() -> Self {
    let nonce = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("test clock should be valid")
      .as_nanos();
    let counter = TEST_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("calcit-strict-check-cli-{}-{nonce}-{counter}", std::process::id()));
    fs::create_dir(&path).expect("temporary project should create");
    Self(path)
  }

  fn snapshot(&self) -> PathBuf {
    self.0.join("calcit.cirru")
  }
}

impl Drop for TestDirectory {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

fn run_calcit(snapshot: &Path, args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .arg("--tips-level")
    .arg("none")
    .arg(snapshot)
    .args(args)
    .output()
    .expect("calcit command should run")
}

fn assert_success(output: &Output, context: &str) {
  assert!(
    output.status.success(),
    "{context} failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
}

fn edit_definition(snapshot: &Path, name: &str, code: &str, overwrite: bool) {
  let target = format!("app.main/{name}");
  let mut args = vec!["edit", "def", target.as_str(), "--code", code];
  if overwrite {
    args.push("--overwrite");
  }
  assert_success(&run_calcit(snapshot, &args), "edit definition");
}

fn edit_schema(snapshot: &Path, name: &str, return_type: &str) {
  let target = format!("app.main/{name}");
  let code = format!("quote $ :: 'Fn $ {{}} (:args $ []) (:return '{return_type})");
  assert_success(
    &run_calcit(snapshot, &["edit", "schema", target.as_str(), "--code", code.as_str()]),
    "edit schema",
  );
}

fn prepare_project() -> (TestDirectory, PathBuf) {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/add.cirru", &snapshot).expect("minimal snapshot fixture should copy");

  edit_definition(&snapshot, "bad-a", "quote $ defn bad-a () $ inc |x", false);
  edit_definition(&snapshot, "bad-b", "quote $ defn bad-b () $ inc |y", false);
  edit_definition(&snapshot, "cycle-a", "quote $ defn cycle-a () $ cycle-z", false);
  edit_definition(
    &snapshot,
    "cycle-z",
    "quote $ defn cycle-z () $ do (inc |cycle-error) (cycle-a)",
    false,
  );
  edit_definition(&snapshot, "dependent", "quote $ defn dependent () $ bad-a", false);
  edit_definition(&snapshot, "reload!", "quote $ defn reload! () nil", false);
  edit_definition(&snapshot, "main!", "quote $ defn main! () $ do (dependent) (bad-b) (cycle-a)", true);

  for name in ["bad-a", "bad-b", "cycle-a", "cycle-z", "dependent", "main!"] {
    edit_schema(&snapshot, name, "Number");
  }
  edit_schema(&snapshot, "reload!", "Nil");
  (directory, snapshot)
}

#[test]
fn keep_going_reports_independent_failures_and_blocks_dependents() {
  let (_directory, snapshot) = prepare_project();
  let output = run_calcit(&snapshot, &["--check-only", "--keep-going", "--format", "json"]);
  assert!(!output.status.success(), "strict failures must retain a non-zero exit status");
  let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout should contain one JSON envelope");
  assert_eq!(report["schema_version"], 1);
  assert_eq!(report["command"], "check-only");
  assert_eq!(report["data"]["summary"]["failed"], 3);
  assert_eq!(report["data"]["summary"]["blocked"], 3);
  assert_eq!(report["data"]["summary"]["cascaded"], 0);

  let definitions = report["data"]["definitions"].as_array().expect("definitions should be an array");
  let by_name = definitions
    .iter()
    .map(|item| (item["definition"].as_str().expect("definition should be text"), item))
    .collect::<std::collections::HashMap<_, _>>();
  assert_eq!(by_name["app.main/bad-a"]["status"], "failed");
  assert_eq!(by_name["app.main/bad-b"]["status"], "failed");
  assert_eq!(by_name["app.main/bad-a"]["diagnostics"][0]["expected"], ":number");
  assert_eq!(by_name["app.main/bad-a"]["diagnostics"][0]["actual"], ":string");
  assert_eq!(by_name["app.main/cycle-z"]["status"], "failed");
  assert_eq!(by_name["app.main/cycle-a"]["status"], "blocked");
  assert_eq!(by_name["app.main/cycle-a"]["blocked_by"], serde_json::json!(["app.main/cycle-z"]));
  assert_eq!(by_name["app.main/dependent"]["status"], "blocked");
  assert_eq!(by_name["app.main/dependent"]["blocked_by"], serde_json::json!(["app.main/bad-a"]));
  assert_eq!(by_name["app.main/main!"]["status"], "blocked");

  let repeated = run_calcit(&snapshot, &["--check-only", "--keep-going", "--format", "json"]);
  assert_eq!(
    output.stdout, repeated.stdout,
    "definition and diagnostic ordering should be stable"
  );
}

#[test]
fn keep_going_supports_markdown_human_and_native_edn_reports() {
  let (_directory, snapshot) = prepare_project();
  let human = run_calcit(&snapshot, &["--check-only", "--keep-going"]);
  assert!(!human.status.success());
  let human_stdout = String::from_utf8_lossy(&human.stdout);
  assert!(human_stdout.starts_with("# Strict check\n"), "stdout:\n{human_stdout}");
  assert!(human_stdout.contains("## `app.main/bad-a`\n"), "stdout:\n{human_stdout}");
  assert!(human_stdout.contains("- status: **BLOCKED**"), "stdout:\n{human_stdout}");
  assert!(
    human_stdout.contains("```text\n"),
    "diagnostic text should have a Markdown boundary:\n{human_stdout}"
  );

  let edn = run_calcit(&snapshot, &["--check-only", "--keep-going", "--format", "edn"]);
  assert!(!edn.status.success());
  let edn_stdout = String::from_utf8(edn.stdout).expect("EDN output should be UTF-8");
  let parsed = cirru_edn::parse(&edn_stdout).expect("stdout should contain one Cirru EDN envelope");
  let cirru_edn::Edn::Map(root) = parsed else {
    panic!("EDN envelope should be a map");
  };
  assert_eq!(root.get(&cirru_edn::Edn::tag("command")), Some(&cirru_edn::Edn::str("check-only")));
}

#[test]
fn default_check_only_remains_fail_fast_and_format_requires_keep_going() {
  let (_directory, snapshot) = prepare_project();
  let fail_fast = run_calcit(&snapshot, &["--check-only"]);
  assert!(!fail_fast.status.success());
  assert!(!String::from_utf8_lossy(&fail_fast.stdout).contains("# Strict check"));

  let invalid = run_calcit(&snapshot, &["--check-only", "--format", "json"]);
  assert!(!invalid.status.success());
  assert!(
    String::from_utf8_lossy(&invalid.stderr).contains("only available with `--check-only --keep-going`"),
    "stderr:\n{}",
    String::from_utf8_lossy(&invalid.stderr)
  );

  let explicit_strict = run_calcit(&snapshot, &["--check-only", "--keep-going", "--strict-types", "--format", "json"]);
  assert!(
    !explicit_strict.status.success(),
    "type errors must still fail explicit strict checking"
  );
  let report: serde_json::Value = serde_json::from_slice(&explicit_strict.stdout).expect("strict check should emit one JSON envelope");
  assert_eq!(report["data"]["summary"]["failed"], 3);
  assert!(
    !String::from_utf8_lossy(&explicit_strict.stderr).contains("quality gate"),
    "strict preprocessing must not run a statistical quality gate"
  );
}
