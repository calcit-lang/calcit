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
    let path = std::env::temp_dir().join(format!("calcit-incremental-analysis-cli-{}-{nonce}-{counter}", std::process::id()));
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

fn assert_success(output: &Output, context: &str) {
  assert!(
    output.status.success(),
    "{context} failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
}

fn report(snapshot: &Path, analyzer: &str) -> serde_json::Value {
  let output = run_calcit(snapshot, &["analyze", analyzer, "--incremental", "--format", "json"]);
  assert_success(&output, analyzer);
  serde_json::from_slice(&output.stdout).expect("analysis stdout should contain one JSON envelope")
}

fn without_cache(mut report: serde_json::Value) -> serde_json::Value {
  report["data"]
    .as_object_mut()
    .expect("analysis data should be an object")
    .remove("cache");
  report
}

#[test]
fn incremental_analysis_reuses_unchanged_definitions_and_reports_invalidation() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("tests/fixtures/ffi-boundary-evidence.cirru", &snapshot).expect("analysis fixture should copy");

  let cold = report(&snapshot, "check-types");
  let cold_cache = &cold["data"]["cache"];
  let definition_count = cold_cache["misses"].as_u64().expect("cold miss count should be numeric");
  assert!(definition_count > 0);
  assert_eq!(cold_cache["scope"], "definition-local-inventory");
  assert_eq!(cold_cache["preprocessing_cached"], false);
  assert_eq!(cold_cache["status"], "cold");
  assert_eq!(cold_cache["hits"], 0);
  assert_eq!(cold_cache["miss_reasons"]["cache-missing"], definition_count);

  let warm = report(&snapshot, "check-types");
  assert_eq!(warm["data"]["cache"]["status"], "warm");
  assert_eq!(warm["data"]["cache"]["hits"], definition_count);
  assert_eq!(warm["data"]["cache"]["misses"], 0);
  assert_eq!(without_cache(cold.clone()), without_cache(warm.clone()));

  let edit = run_calcit(
    &snapshot,
    &[
      "edit",
      "def",
      "ffi-evidence.main/cache-probe",
      "--code",
      "quote $ defn cache-probe () 1",
    ],
  );
  assert_success(&edit, "add cache probe definition");

  let partial = report(&snapshot, "check-types");
  assert_eq!(partial["data"]["cache"]["status"], "partial");
  assert_eq!(partial["data"]["cache"]["hits"], definition_count);
  assert_eq!(partial["data"]["cache"]["misses"], 1);
  assert_eq!(partial["data"]["cache"]["miss_reasons"]["not-cached"], 1);

  let edit = run_calcit(
    &snapshot,
    &[
      "edit",
      "def",
      "ffi-evidence.main/cache-probe",
      "--code",
      "quote $ defn cache-probe () 2",
      "--overwrite",
    ],
  );
  assert_success(&edit, "change cache probe definition");

  let changed = report(&snapshot, "check-types");
  assert_eq!(changed["data"]["cache"]["status"], "partial");
  assert_eq!(changed["data"]["cache"]["hits"], definition_count);
  assert_eq!(changed["data"]["cache"]["misses"], 1);
  assert_eq!(changed["data"]["cache"]["miss_reasons"]["definition-changed"], 1);

  let weak_cold = report(&snapshot, "weak-types");
  assert_eq!(weak_cold["data"]["cache"]["status"], "cold");
  assert_eq!(weak_cold["data"]["cache"]["miss_reasons"]["analysis-missing"], definition_count + 1);

  let weak_warm = report(&snapshot, "weak-types");
  assert_eq!(weak_warm["data"]["cache"]["status"], "warm");
  assert_eq!(weak_warm["data"]["cache"]["hits"], definition_count + 1);
  assert_eq!(weak_warm["data"]["cache"]["misses"], 0);
  assert_eq!(without_cache(weak_cold), without_cache(weak_warm));

  let policy_change = run_calcit(&snapshot, &["config", "set-type-slot", "cache-probe", ":dynamic"]);
  assert_success(&policy_change, "change entry type-slot policy");
  let policy_cold = report(&snapshot, "weak-types");
  assert_eq!(policy_cold["data"]["cache"]["status"], "cold");
  assert_eq!(
    policy_cold["data"]["cache"]["miss_reasons"]["entry-policy-changed"],
    definition_count + 1
  );
}

#[test]
fn corrupt_incremental_cache_falls_back_to_a_cold_analysis() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("tests/fixtures/ffi-boundary-evidence.cirru", &snapshot).expect("analysis fixture should copy");

  let first = report(&snapshot, "check-types");
  let definition_count = first["data"]["cache"]["misses"]
    .as_u64()
    .expect("cold miss count should be numeric");
  fs::write(directory.0.join(".calcit/analysis-cache-v1.json"), "not-json").expect("cache corruption should write");

  let recovered = report(&snapshot, "check-types");
  assert_eq!(recovered["data"]["cache"]["status"], "cold");
  assert_eq!(recovered["data"]["cache"]["miss_reasons"]["cache-corrupt"], definition_count);
}
