use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use cirru_edn::Edn;

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

fn run_calcit_with_path(snapshot: &Path, args: &[&str], path: &Path) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .env("PATH", path)
    .arg("--tips-level")
    .arg("none")
    .arg(snapshot)
    .args(args)
    .output()
    .expect("calcit command should run with an isolated PATH")
}

fn edn_map_field<'a>(value: &'a Edn, key: &str) -> &'a Edn {
  let Edn::Map(map) = value else {
    panic!("expected an EDN map, got {value:?}");
  };
  map.get(&Edn::tag(key)).unwrap_or_else(|| panic!("missing EDN field :{key}"))
}

fn json_envelope_as_edn(value: &serde_json::Value) -> Edn {
  match value {
    serde_json::Value::Null => Edn::Nil,
    serde_json::Value::Bool(value) => Edn::Bool(*value),
    serde_json::Value::Number(value) => Edn::Number(value.as_f64().expect("verification numbers should fit f64")),
    serde_json::Value::String(value) => Edn::str(value.as_str()),
    serde_json::Value::Array(values) => Edn::List(cirru_edn::EdnListView(values.iter().map(json_envelope_as_edn).collect())),
    serde_json::Value::Object(values) => Edn::map_from_iter(
      values
        .iter()
        .map(|(key, value)| (Edn::tag(key.replace('_', "-")), json_envelope_as_edn(value))),
    ),
  }
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
  assert_eq!(value["data"]["preflight"]["status"], "passed");
  assert_eq!(value["data"]["preflight"]["snapshot"]["format"], "cirru-edn");
  assert_eq!(value["data"]["preflight"]["snapshot"]["active_entries"][0], "default");
  let checks = value["data"]["checks"].as_array().expect("checks should be an array");
  assert_eq!(checks.len(), 6);
  assert_eq!(checks[0]["entry"], "default");
  assert_eq!(checks[0]["target"], "native");
  assert_eq!(checks[3]["entry"], "js");
  assert_eq!(checks[3]["target"], "node");
  assert!(checks.iter().all(|check| check["revision"] == value["revision"]));

  let edn_output = run_calcit(&snapshot, &["analyze", "verify", "--profile", "release", "--format", "edn"]);
  assert!(edn_output.status.success());
  let edn_text = String::from_utf8(edn_output.stdout).expect("EDN stdout should be UTF-8");
  let edn = cirru_edn::parse(&edn_text).expect("stdout should contain one Cirru EDN value");
  assert_eq!(edn, json_envelope_as_edn(&value));
  assert_eq!(edn_map_field(&edn, "schema-version"), &Edn::Number(1.0));
  assert_eq!(edn_map_field(&edn, "command"), &Edn::str("analyze.verify"));
  assert_eq!(edn_map_field(edn_map_field(&edn, "data"), "profile"), &Edn::str("release"));
  assert_eq!(edn_map_field(edn_map_field(&edn, "data"), "status"), &Edn::str("passed"));
  let Edn::List(edn_checks) = edn_map_field(edn_map_field(&edn, "data"), "checks") else {
    panic!("EDN checks should be a list");
  };
  assert_eq!(edn_checks.0.len(), checks.len());

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
fn declared_host_requirement_fails_before_external_gate_without_guessing_other_tools() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  let source = snapshot_source(VALID_ENTRIES).replace(
    ":profiles $ {}",
    ":host-requirements $ {} (:node |>=999.0.0)\n    :external-gates $ [] |yarn-build\n    :profiles $ {}",
  );
  fs::write(&snapshot, source).expect("fixture should write");
  let fixture_bin = directory.0.join("bin");
  fs::create_dir(&fixture_bin).expect("fixture bin directory should create");
  fs::copy(
    env!("CARGO_BIN_EXE_calcit"),
    fixture_bin.join(format!("node{}", std::env::consts::EXE_SUFFIX)),
  )
  .expect("fixture node executable should copy");

  let output = run_calcit_with_path(
    &snapshot,
    &["analyze", "verify", "--profile", "release", "--format", "json"],
    &fixture_bin,
  );
  assert!(!output.status.success());
  let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout should contain one JSON value");
  let preflight = &value["data"]["preflight"];
  assert_eq!(value["data"]["status"], "failed");
  assert_eq!(preflight["status"], "failed");
  assert!(
    preflight["diagnostics"]
      .as_array()
      .expect("preflight diagnostics should be an array")
      .iter()
      .any(|diagnostic| diagnostic["code"] == "E_PREFLIGHT_HOST_VERSION")
  );
  let tools = preflight["tools"].as_array().expect("preflight tools should be an array");
  assert!(tools.iter().any(|tool| tool["tool"] == "node" && tool["status"] == "mismatch"));
  assert!(
    !tools
      .iter()
      .any(|tool| matches!(tool["tool"].as_str(), Some("yarn" | "rustc" | "caps")))
  );
  assert_eq!(preflight["external_gates"][0]["name"], "yarn-build");
  assert_eq!(preflight["external_gates"][0]["executed"], false);
  assert!(value["data"]["checks"].as_array().is_some_and(|checks| !checks.is_empty()));
}

#[test]
fn preflight_reads_procs_resolution_from_yarn_lock_without_installing_dependencies() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::write(&snapshot, snapshot_source(VALID_ENTRIES)).expect("fixture should write");
  let version = env!("CARGO_PKG_VERSION");
  fs::write(
    directory.0.join("package.json"),
    format!(r#"{{"dependencies":{{"@calcit/procs":"{version}"}}}}"#),
  )
  .expect("package manifest should write");
  fs::write(
    directory.0.join("yarn.lock"),
    format!("\"@calcit/procs@npm:{version}\":\n  resolution: \"@calcit/procs@npm:{version}\"\n"),
  )
  .expect("lockfile should write");

  let output = run_calcit(&snapshot, &["analyze", "verify", "--profile", "release", "--format", "json"]);
  assert!(
    output.status.success(),
    "verification failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout should contain one JSON value");
  let tools = value["data"]["preflight"]["tools"]
    .as_array()
    .expect("preflight tools should be an array");
  let procs = tools
    .iter()
    .find(|tool| tool["tool"] == "@calcit/procs")
    .expect("preflight should report @calcit/procs");
  assert_eq!(procs["declared"], version);
  assert_eq!(procs["observed"], version);
  assert_eq!(procs["status"], "matched");
}

#[test]
fn verification_rejects_duplicate_normalized_host_requirement_keys() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  let source = snapshot_source(VALID_ENTRIES).replace(
    ":profiles $ {}",
    ":host-requirements $ {} (:node |>=20.0.0) (|node |>=24.0.0)\n    :profiles $ {}",
  );
  fs::write(&snapshot, source).expect("fixture should write");

  let output = run_calcit(&snapshot, &["analyze", "verify", "--profile", "release", "--format", "json"]);
  assert!(!output.status.success());
  let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout should contain one JSON value");
  assert_eq!(value["diagnostics"][0]["code"], "E_VERIFY_CONFIG");
  assert!(
    value["diagnostics"][0]["message"]
      .as_str()
      .is_some_and(|message| message.contains("duplicate tool `node`"))
  );
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

  let edn_output = run_calcit(&snapshot, &["analyze", "verify", "--profile", "release", "--format", "edn"]);
  assert!(!edn_output.status.success());
  let edn_text = String::from_utf8(edn_output.stdout).expect("EDN failure stdout should be UTF-8");
  let edn = cirru_edn::parse(&edn_text).expect("failure stdout should remain one Cirru EDN value");
  assert_eq!(edn, json_envelope_as_edn(&value));
  assert_eq!(edn_map_field(edn_map_field(&edn, "data"), "status"), &Edn::str("failed"));
  let Edn::List(diagnostics) = edn_map_field(&edn, "diagnostics") else {
    panic!("EDN diagnostics should be a list");
  };
  assert_eq!(edn_map_field(&diagnostics.0[0], "code"), &Edn::str("E_VERIFY_CONFIG"));
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
