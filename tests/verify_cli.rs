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
    let path = std::env::temp_dir().join(format!("calcit-verify-cli-{}-{nonce}-{counter}", std::process::id()));
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
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .arg(snapshot)
    .args(args)
    .output()
    .expect("calcit command should run")
}

fn snapshot_source(entries: &str) -> String {
  format!(
    r#"
{{}}
  :about |verify-fixture
  :package |verify-fixture
  :version |0.0.0
  :entries $ {{}}
{entries}
  :verification $ {{}} (:schema-version 1)
    :profiles $ {{}} $ :release
      {{}} (:on-failure :continue)
        :entries $ [] :default :js
        :checks $ [] :strict :dynamic-methods :quality
  :files $ {{}} $ 'verify-fixture.main
    %{{}} 'FileEntry
      :defs $ {{}}
        'main! $ %{{}} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {{}} (:return 'Unit)
            :args $ []
        'reload! $ %{{}} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {{}} (:return 'Unit)
            :args $ []
      :ns $ %{{}} 'NsEntry (:doc |)
        :code $ quote $ ns verify-fixture.main
"#
  )
}

const VALID_ENTRIES: &str = r#"    :default $ {} (:description |) (:init-fn 'verify-fixture.main/main!) (:mode :native) (:reload-fn 'verify-fixture.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
    :js $ {} (:description |) (:init-fn 'verify-fixture.main/main!) (:mode :js) (:reload-fn 'verify-fixture.main/reload!) (:target :node)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}"#;

#[test]
fn verification_profile_uses_one_contract_for_native_and_js_entries() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::write(&snapshot, snapshot_source(VALID_ENTRIES)).expect("fixture should write");

  let output = run_calcit(&snapshot, &["analyze", "verify", "--profile", "release", "--format", "json"]);
  assert!(
    output.status.success(),
    "verification failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout should contain one JSON value");
  assert_eq!(value["schema_version"], 1);
  assert_eq!(value["command"], "analyze.verify");
  assert_eq!(value["data"]["profile"], "release");
  assert_eq!(value["data"]["status"], "passed");
  let checks = value["data"]["checks"].as_array().expect("checks should be an array");
  assert_eq!(checks.len(), 6);
  assert_eq!(checks[0]["entry"], "default");
  assert_eq!(checks[0]["target"], "native");
  assert_eq!(checks[3]["entry"], "js");
  assert_eq!(checks[3]["target"], "node");
  assert!(checks.iter().all(|check| check["revision"] == value["revision"]));

  let config = run_calcit(&snapshot, &["config", "show", "--format", "json"]);
  assert!(config.status.success());
  let config_value: serde_json::Value = serde_json::from_slice(&config.stdout).expect("config stdout should contain one JSON value");
  assert_eq!(config_value["data"]["verification_schema_version"], 1);
  assert_eq!(config_value["data"]["verification_profiles"][0]["name"], "release");

  let human = run_calcit(&snapshot, &["analyze", "verify", "--profile", "release"]);
  assert!(human.status.success());
  let stdout = String::from_utf8(human.stdout).expect("human output should be UTF-8");
  assert!(stdout.starts_with("# Verification `release`\n"));
  assert!(stdout.contains("## `default` · `strict`\n"));
  assert!(stdout.contains("## `js` · `strict`\n"));
}

#[test]
fn invalid_profile_configuration_fails_before_checks_with_json_diagnostic() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  let mut source = snapshot_source(VALID_ENTRIES);
  source = source.replace(":entries $ [] :default :js", ":entries $ [] :default :missing");
  fs::write(&snapshot, source).expect("fixture should write");

  let output = run_calcit(&snapshot, &["analyze", "verify", "--profile", "release", "--format", "json"]);
  assert!(!output.status.success());
  let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("failure stdout should remain one JSON value");
  assert_eq!(value["command"], "analyze.verify");
  assert_eq!(value["data"]["status"], "failed");
  assert_eq!(value["data"]["checks"].as_array().map(Vec::len), Some(0));
  assert!(value["revision"].as_str().is_some_and(|revision| revision.starts_with("md5:")));
  assert_eq!(value["diagnostics"][0]["code"], "E_VERIFY_CONFIG");
  assert!(
    value["diagnostics"][0]["message"]
      .as_str()
      .unwrap()
      .contains("unknown entry `missing`")
  );
}

#[test]
fn stop_policy_does_not_run_checks_after_the_first_failure() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  let source = snapshot_source(VALID_ENTRIES)
    .replace(":on-failure :continue", ":on-failure :stop")
    .replace(":checks $ [] :strict :dynamic-methods :quality", ":checks $ [] :quality :strict")
    .replacen(
      ":schema $ :: 'Fn $ {} (:return 'Unit)\n            :args $ []",
      ":schema $ :: 'Dynamic",
      1,
    );
  fs::write(&snapshot, source).expect("fixture should write");

  let output = run_calcit(&snapshot, &["analyze", "verify", "--profile", "release", "--format", "json"]);
  assert!(!output.status.success());
  let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout should contain one JSON value");
  let checks = value["data"]["checks"].as_array().expect("checks should be an array");
  assert_eq!(
    checks.len(),
    1,
    "stdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  assert_eq!(checks[0]["check"], "quality");
  assert_eq!(checks[0]["status"], "failed");
}
