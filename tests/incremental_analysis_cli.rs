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

fn full_report(snapshot: &Path, analyzer: &str) -> serde_json::Value {
  let output = run_calcit(snapshot, &["analyze", analyzer, "--format", "json"]);
  assert_success(&output, analyzer);
  serde_json::from_slice(&output.stdout).expect("analysis stdout should contain one JSON envelope")
}

fn without_cache(mut report: serde_json::Value) -> serde_json::Value {
  report["data"]
    .as_object_mut()
    .expect("analysis data should be an object")
    .remove("cache");
  report["data"]["filters"]
    .as_object_mut()
    .expect("analysis filters should be an object")
    .remove("incremental");
  report
}

#[test]
fn incremental_analysis_reuses_unchanged_definitions_and_reports_invalidation() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("tests/fixtures/ffi-boundary-evidence.cirru", &snapshot).expect("analysis fixture should copy");
  let dependency_target = directory.0.join("dep-a");
  fs::create_dir(&dependency_target).expect("dependency target directory should create");
  fs::copy("tests/fixtures/ffi-boundary-evidence.cirru", dependency_target.join("calcit.cirru"))
    .expect("dependency fixture should copy");
  let dependency = directory.0.join("dep");
  #[cfg(unix)]
  std::os::unix::fs::symlink(&dependency_target, &dependency).expect("dependency symlink should create");
  #[cfg(not(unix))]
  fs::create_dir(&dependency).expect("dependency directory should create");
  let dependency_snapshot = dependency.join("calcit.cirru");
  #[cfg(not(unix))]
  fs::copy("tests/fixtures/ffi-boundary-evidence.cirru", &dependency_snapshot).expect("dependency fixture should copy");
  let add_module = run_calcit(&snapshot, &["config", "add-module", "./dep/"]);
  assert_success(&add_module, "add local dependency module");

  let cold = report(&snapshot, "check-types");
  let cold_cache = &cold["data"]["cache"];
  let definition_count = cold_cache["misses"].as_u64().expect("cold miss count should be numeric");
  assert!(definition_count > 0);
  assert_eq!(cold_cache["scope"], "definition-local-inventory");
  assert_eq!(cold_cache["preprocessing_cached"], false);
  assert_eq!(cold_cache["status"], "cold");
  assert_eq!(cold_cache["hits"], 0);
  assert_eq!(cold_cache["miss_reasons"]["cache-missing"], definition_count);
  assert_eq!(cold_cache["input"]["status"], "cold");
  assert_eq!(cold_cache["input"]["reason"], "cache-missing");
  assert_eq!(cold_cache["input"]["sources"], 2);
  assert_eq!(cold_cache["dependency_index"]["status"], "cold");
  assert_eq!(cold_cache["dependency_index"]["hits"], 0);
  assert_eq!(cold_cache["dependency_index"]["misses"], definition_count);
  assert_eq!(cold_cache["dependency_index"]["changed"], definition_count);
  let unresolved_dependencies = cold_cache["dependency_index"]["unresolved"]
    .as_u64()
    .expect("unresolved dependency count should be numeric");

  let warm = report(&snapshot, "check-types");
  assert_eq!(warm["data"]["cache"]["status"], "warm");
  assert_eq!(warm["data"]["cache"]["hits"], definition_count);
  assert_eq!(warm["data"]["cache"]["misses"], 0);
  assert_eq!(warm["data"]["cache"]["input"]["status"], "warm");
  assert_eq!(warm["data"]["cache"]["dependency_index"]["status"], "warm");
  assert_eq!(warm["data"]["cache"]["dependency_index"]["hits"], definition_count);
  assert_eq!(warm["data"]["cache"]["dependency_index"]["misses"], 0);
  assert_eq!(warm["data"]["cache"]["dependency_index"]["changed"], 0);
  assert_eq!(warm["data"]["cache"]["dependency_index"]["affected"], 0);
  assert_eq!(warm["data"]["cache"]["dependency_index"]["unresolved"], unresolved_dependencies);
  assert_eq!(without_cache(cold.clone()), without_cache(warm.clone()));

  #[cfg(unix)]
  {
    let replacement = directory.0.join("dep-b");
    fs::create_dir(&replacement).expect("replacement dependency directory should create");
    fs::copy("tests/fixtures/ffi-boundary-evidence.cirru", replacement.join("calcit.cirru"))
      .expect("replacement dependency fixture should copy");
    fs::remove_file(&dependency).expect("dependency symlink should remove");
    std::os::unix::fs::symlink(&replacement, &dependency).expect("replacement dependency symlink should create");
    let resolution_changed = report(&snapshot, "check-types");
    assert_eq!(resolution_changed["data"]["cache"]["status"], "warm");
    assert_eq!(resolution_changed["data"]["cache"]["input"]["status"], "cold");
    assert_eq!(resolution_changed["data"]["cache"]["input"]["reason"], "module-resolution-changed");
  }

  let mut dependency_content = fs::read_to_string(&dependency_snapshot).expect("dependency fixture should read");
  dependency_content.push('\n');
  fs::write(&dependency_snapshot, dependency_content).expect("dependency fixture should update");
  let dependency_changed = report(&snapshot, "check-types");
  assert_eq!(dependency_changed["data"]["cache"]["status"], "warm");
  assert_eq!(dependency_changed["data"]["cache"]["input"]["status"], "cold");
  assert_eq!(dependency_changed["data"]["cache"]["input"]["reason"], "source-changed");

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
  assert_eq!(partial["data"]["cache"]["input"]["reason"], "source-changed");
  assert_eq!(partial["data"]["cache"]["dependency_index"]["status"], "partial");
  assert_eq!(partial["data"]["cache"]["dependency_index"]["hits"], definition_count);
  assert_eq!(partial["data"]["cache"]["dependency_index"]["misses"], 1);
  assert_eq!(partial["data"]["cache"]["dependency_index"]["changed"], 1);
  assert_eq!(partial["data"]["cache"]["dependency_index"]["affected"], 1);

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

  let edit = run_calcit(
    &snapshot,
    &[
      "edit",
      "def",
      "ffi-evidence.main/query-host",
      "--code",
      "quote $ defn query-host (host) unsafe-coerce host 'String",
      "--overwrite",
    ],
  );
  assert_success(&edit, "change dependency target definition");

  let propagated = report(&snapshot, "check-types");
  let propagated_dependencies = &propagated["data"]["cache"]["dependency_index"];
  assert_eq!(propagated_dependencies["changed"], 1);
  assert!(
    propagated_dependencies["affected"]
      .as_u64()
      .expect("affected dependency count should be numeric")
      > 1,
    "changing a referenced definition should affect at least one caller"
  );
  assert!(
    propagated_dependencies["miss_reasons"]["dependency-affected"]
      .as_u64()
      .expect("dependency-affected count should be numeric")
      > 0,
    "affected callers should be retraced before persisting the graph"
  );

  let settled = report(&snapshot, "check-types");
  assert_eq!(settled["data"]["cache"]["dependency_index"]["status"], "warm");
  assert_eq!(settled["data"]["cache"]["dependency_index"]["changed"], 0);
  assert_eq!(settled["data"]["cache"]["dependency_index"]["affected"], 0);

  let weak_full = full_report(&snapshot, "weak-types");
  let weak_cold = report(&snapshot, "weak-types");
  assert_eq!(weak_cold["data"]["cache"]["status"], "cold");
  assert_eq!(weak_cold["data"]["cache"]["miss_reasons"]["analysis-missing"], definition_count + 1);
  assert_eq!(without_cache(weak_cold.clone()), without_cache(weak_full));

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

  let schema_evidence = run_calcit(
    &snapshot,
    &[
      "analyze",
      "weak-types",
      "--schema-evidence",
      "--incremental",
      "--summary-only",
      "--format",
      "json",
    ],
  );
  assert_success(&schema_evidence, "incremental schema evidence");
  let schema_evidence: serde_json::Value =
    serde_json::from_slice(&schema_evidence.stdout).expect("schema evidence stdout should contain one JSON envelope");
  assert_eq!(schema_evidence["data"]["cache"]["input"]["status"], "bypassed");
  assert_eq!(
    schema_evidence["data"]["cache"]["input"]["reason"],
    "schema-evidence-requires-preprocessing"
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
  fs::write(directory.0.join(".calcit/analysis-input-cache-v1.cirru"), "not-cirru-edn").expect("input cache corruption should write");

  let recovered_input = report(&snapshot, "check-types");
  assert_eq!(recovered_input["data"]["cache"]["status"], "warm");
  assert_eq!(recovered_input["data"]["cache"]["input"]["status"], "cold");
  assert_eq!(recovered_input["data"]["cache"]["input"]["reason"], "cache-corrupt");

  fs::write(directory.0.join(".calcit/analysis-cache-v1.cirru"), "not-cirru-edn").expect("definition cache corruption should write");

  let recovered = report(&snapshot, "check-types");
  assert_eq!(recovered["data"]["cache"]["status"], "cold");
  assert_eq!(recovered["data"]["cache"]["miss_reasons"]["cache-corrupt"], definition_count);
  assert_eq!(recovered["data"]["cache"]["input"]["status"], "warm");
}
