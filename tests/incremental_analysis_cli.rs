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

fn dynamic_report(snapshot: &Path, incremental: bool) -> serde_json::Value {
  let mut args = vec!["--compat-types", "analyze", "dynamic-methods"];
  if incremental {
    args.push("--incremental");
  }
  args.extend(["--format", "json"]);
  let output = run_calcit(snapshot, &args);
  assert_success(&output, "dynamic-methods");
  serde_json::from_slice(&output.stdout).expect("dynamic method analysis stdout should contain one JSON envelope")
}

fn incremental_check(snapshot: &Path) -> Output {
  run_calcit(snapshot, &["--compat-types", "--check-only", "--incremental"])
}

fn report_with_entry(snapshot: &Path, analyzer: &str, entry: &str, incremental: bool) -> serde_json::Value {
  let mut args = vec!["--entry", entry, "analyze", analyzer, "--deps"];
  if incremental {
    args.push("--incremental");
  }
  args.extend(["--format", "json"]);
  let output = run_calcit(snapshot, &args);
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
  let second_dependency = directory.0.join("dep-second");
  fs::create_dir(&second_dependency).expect("second dependency directory should create");
  let second_dependency_snapshot = second_dependency.join("calcit.cirru");
  fs::copy("tests/fixtures/ffi-boundary-evidence.cirru", &second_dependency_snapshot).expect("second dependency fixture should copy");
  let transitive_dependency = directory.0.join("dep-leaf");
  fs::create_dir(&transitive_dependency).expect("transitive dependency directory should create");
  let transitive_dependency_snapshot = transitive_dependency.join("calcit.cirru");
  fs::copy("tests/fixtures/ffi-boundary-evidence.cirru", &transitive_dependency_snapshot)
    .expect("transitive dependency fixture should copy");
  let add_transitive_module = run_calcit(&second_dependency_snapshot, &["config", "add-module", "./dep-leaf/"]);
  assert_success(&add_transitive_module, "add transitive dependency module");
  let second_dependency_content = fs::read_to_string(&second_dependency_snapshot).expect("second dependency should read");
  let add_module = run_calcit(&snapshot, &["config", "add-module", "./dep-second/"]);
  assert_success(&add_module, "add second local dependency module");

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
  assert_eq!(cold_cache["input"]["sources"], 4);
  assert_eq!(cold_cache["input"]["main_reused"], false);
  assert_eq!(cold_cache["input"]["module_hits"], 0);
  assert_eq!(cold_cache["input"]["module_misses"], 2);
  assert_eq!(cold_cache["input"]["module_miss_reasons"]["cache-missing"], 2);
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
  assert_eq!(warm["data"]["cache"]["input"]["main_reused"], true);
  assert_eq!(warm["data"]["cache"]["input"]["module_hits"], 2);
  assert_eq!(warm["data"]["cache"]["input"]["module_misses"], 0);
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
    assert_eq!(resolution_changed["data"]["cache"]["input"]["status"], "partial");
    assert_eq!(resolution_changed["data"]["cache"]["input"]["reason"], "module-resolution-changed");
    assert_eq!(resolution_changed["data"]["cache"]["input"]["main_reused"], true);
    assert_eq!(resolution_changed["data"]["cache"]["input"]["module_hits"], 1);
    assert_eq!(resolution_changed["data"]["cache"]["input"]["module_misses"], 1);
  }

  let mut dependency_content = fs::read_to_string(&dependency_snapshot).expect("dependency fixture should read");
  dependency_content.push('\n');
  fs::write(&dependency_snapshot, dependency_content).expect("dependency fixture should update");
  let dependency_changed = report(&snapshot, "check-types");
  assert_eq!(dependency_changed["data"]["cache"]["status"], "warm");
  assert_eq!(dependency_changed["data"]["cache"]["input"]["status"], "partial");
  assert_eq!(dependency_changed["data"]["cache"]["input"]["reason"], "source-changed");
  assert_eq!(dependency_changed["data"]["cache"]["input"]["main_reused"], true);
  assert_eq!(dependency_changed["data"]["cache"]["input"]["module_hits"], 1);
  assert_eq!(dependency_changed["data"]["cache"]["input"]["module_misses"], 1);
  assert_eq!(
    without_cache(dependency_changed.clone()),
    without_cache(full_report(&snapshot, "check-types"))
  );

  let mut transitive_content = fs::read_to_string(&transitive_dependency_snapshot).expect("transitive dependency should read");
  transitive_content.push('\n');
  fs::write(&transitive_dependency_snapshot, transitive_content).expect("transitive dependency should update");
  let transitive_changed = report(&snapshot, "check-types");
  assert_eq!(transitive_changed["data"]["cache"]["input"]["status"], "partial");
  assert_eq!(transitive_changed["data"]["cache"]["input"]["reason"], "source-changed");
  assert_eq!(transitive_changed["data"]["cache"]["input"]["main_reused"], true);
  assert_eq!(transitive_changed["data"]["cache"]["input"]["module_hits"], 1);
  assert_eq!(transitive_changed["data"]["cache"]["input"]["module_misses"], 1);

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
  let edit = run_calcit(
    &snapshot,
    &[
      "edit",
      "schema",
      "ffi-evidence.main/cache-probe",
      "--code",
      "quote 'ffi-evidence.main/reload!",
    ],
  );
  assert_success(&edit, "add cache probe dependency schema");

  let partial = report(&snapshot, "check-types");
  assert_eq!(partial["data"]["cache"]["status"], "partial");
  assert_eq!(partial["data"]["cache"]["hits"], definition_count);
  assert_eq!(partial["data"]["cache"]["misses"], 1);
  assert_eq!(partial["data"]["cache"]["miss_reasons"]["not-cached"], 1);
  assert_eq!(partial["data"]["cache"]["input"]["status"], "partial");
  assert_eq!(partial["data"]["cache"]["input"]["reason"], "source-changed");
  assert_eq!(partial["data"]["cache"]["input"]["main_reused"], false);
  assert_eq!(partial["data"]["cache"]["input"]["module_hits"], 2);
  assert_eq!(partial["data"]["cache"]["input"]["module_misses"], 0);
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
      "ffi-evidence.main/reload!",
      "--code",
      "quote $ defn reload! () (do nil &unit)",
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

  fs::remove_file(&second_dependency_snapshot).expect("second dependency should become temporarily unavailable");
  let interrupted = report(&snapshot, "check-types");
  assert_eq!(interrupted["data"]["cache"]["input"]["status"], "partial");
  assert_eq!(interrupted["data"]["cache"]["input"]["reason"], "module-load-failed");
  assert_eq!(interrupted["data"]["cache"]["input"]["module_hits"], 1);
  assert_eq!(interrupted["data"]["cache"]["input"]["module_misses"], 1);
  assert_eq!(
    interrupted["data"]["cache"]["input"]["module_miss_reasons"]["module-load-failed"],
    1
  );
  fs::write(&second_dependency_snapshot, &second_dependency_content).expect("second dependency should recover");
  let recovered_modules = report(&snapshot, "check-types");
  assert_eq!(recovered_modules["data"]["cache"]["input"]["status"], "warm");
  assert_eq!(recovered_modules["data"]["cache"]["input"]["module_hits"], 2);
  assert_eq!(without_cache(interrupted), without_cache(recovered_modules));

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
fn incremental_analysis_honors_selected_entry_modules_and_policy() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  let original = fs::read_to_string("tests/fixtures/ffi-boundary-evidence.cirru").expect("analysis fixture should read");
  let default_entry = r#"  :entries $ {} $ :default
    {}
      :description "|Browser evidence fixture."
      :init-fn 'ffi-evidence.main/main!
      :mode :js
      :reload-fn 'ffi-evidence.main/reload!
      :target :browser
      :feature-policy $ {} $ :js-ffi :error
      :modules $ []
      :type-slots $ {}"#;
  let multiple_entries = r#"  :entries $ {}
    :default $ {}
      :description "|Default incremental entry."
      :init-fn 'ffi-evidence.main/main!
      :mode :js
      :reload-fn 'ffi-evidence.main/reload!
      :target :browser
      :feature-policy $ {} $ :js-ffi :error
      :modules $ [] |./dep-default/
      :type-slots $ {}
    :test $ {}
      :description "|Selected incremental entry."
      :init-fn 'ffi-evidence.main/main!
      :mode :js
      :reload-fn 'ffi-evidence.main/reload!
      :target :browser
      :feature-policy $ {} $ :js-ffi :allow
      :modules $ [] |./dep-test/
      :type-slots $ {} $ :cache-probe :dynamic"#;
  let configured = original.replacen(default_entry, multiple_entries, 1);
  assert_ne!(configured, original, "fixture entry block should be replaced");
  fs::write(&snapshot, configured).expect("multi-entry fixture should write");

  for (dependency, package) in [("dep-default", "default-dependency"), ("dep-test", "test-dependency")] {
    let dependency_directory = directory.0.join(dependency);
    fs::create_dir(&dependency_directory).expect("dependency directory should create");
    fs::write(dependency_directory.join("calcit.cirru"), original.replace("ffi-evidence", package))
      .expect("dependency fixture should write");
  }

  let default = report_with_entry(&snapshot, "check-types", "default", true);
  let definition_count = default["data"]["cache"]["misses"]
    .as_u64()
    .expect("definition miss count should be numeric");
  assert_eq!(default["data"]["cache"]["input"]["entry"], "default");
  assert_eq!(default["data"]["cache"]["input"]["module_misses"], 1);

  let selected = report_with_entry(&snapshot, "check-types", "test", true);
  assert_eq!(selected["data"]["cache"]["input"]["entry"], "test");
  assert_eq!(selected["data"]["cache"]["input"]["status"], "partial");
  assert_eq!(selected["data"]["cache"]["input"]["main_reused"], true);
  assert_eq!(selected["data"]["cache"]["input"]["module_hits"], 0);
  assert_eq!(selected["data"]["cache"]["input"]["module_misses"], 1);
  assert_eq!(selected["data"]["cache"]["input"]["module_miss_reasons"]["not-cached"], 1);
  assert_eq!(selected["data"]["cache"]["status"], "cold");
  assert_eq!(selected["data"]["cache"]["miss_reasons"]["entry-policy-changed"], definition_count);
  let selected_ids = selected["data"]["definitions"]
    .as_array()
    .expect("definitions should be an array")
    .iter()
    .filter_map(|row| row["id"].as_str())
    .collect::<Vec<_>>();
  assert!(selected_ids.contains(&"test-dependency.main/main!"));
  assert!(!selected_ids.contains(&"default-dependency.main/main!"));

  let selected_warm = report_with_entry(&snapshot, "check-types", "test", true);
  assert_eq!(selected_warm["data"]["cache"]["input"]["entry"], "test");
  assert_eq!(selected_warm["data"]["cache"]["input"]["status"], "warm");
  assert_eq!(selected_warm["data"]["cache"]["input"]["module_hits"], 1);
  assert_eq!(selected_warm["data"]["cache"]["status"], "warm");

  let selected_full = report_with_entry(&snapshot, "check-types", "test", false);
  assert_eq!(without_cache(selected_warm), without_cache(selected_full));
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
  fs::write(directory.0.join(".calcit/analysis-input-cache-v2.cirru"), "not-cirru-edn").expect("input cache corruption should write");

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

#[test]
fn incremental_dynamic_methods_reuses_only_an_unchanged_entry_dependency_closure() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("tests/fixtures/ffi-boundary-evidence.cirru", &snapshot).expect("analysis fixture should copy");

  let cold = dynamic_report(&snapshot, true);
  assert_eq!(cold["data"]["cache"]["scope"], "entry-dependency-closure");
  assert_eq!(cold["data"]["cache"]["status"], "cold");
  assert_eq!(cold["data"]["cache"]["preprocessing_cached"], false);

  let warm = dynamic_report(&snapshot, true);
  assert_eq!(warm["data"]["cache"]["status"], "warm");
  assert_eq!(warm["data"]["cache"]["preprocessing_cached"], true);
  assert_eq!(warm["data"]["cache"]["dependency_index"]["changed"], 0);
  assert_eq!(without_cache(warm.clone()), without_cache(dynamic_report(&snapshot, false)));

  let add_unrelated = run_calcit(
    &snapshot,
    &[
      "edit",
      "def",
      "ffi-evidence.main/cache-probe",
      "--code",
      "quote $ defn cache-probe () 1",
    ],
  );
  assert_success(&add_unrelated, "add unrelated definition");
  let unrelated = dynamic_report(&snapshot, true);
  assert_eq!(unrelated["data"]["cache"]["status"], "warm");
  assert_eq!(unrelated["data"]["cache"]["preprocessing_cached"], true);
  assert_eq!(unrelated["data"]["cache"]["dependency_index"]["changed"], 1);
  assert_eq!(without_cache(unrelated), without_cache(dynamic_report(&snapshot, false)));

  let change_root = run_calcit(
    &snapshot,
    &[
      "edit",
      "def",
      "ffi-evidence.main/reload!",
      "--overwrite",
      "--code",
      "quote $ defn reload! () if true &unit &unit",
    ],
  );
  assert_success(&change_root, "change reload root");
  let invalidated = dynamic_report(&snapshot, true);
  assert_eq!(invalidated["data"]["cache"]["status"], "cold");
  assert_eq!(invalidated["data"]["cache"]["preprocessing_cached"], false);
  assert_eq!(invalidated["data"]["cache"]["reason"], "dependency-closure-changed");

  let settled = dynamic_report(&snapshot, true);
  assert_eq!(settled["data"]["cache"]["status"], "warm");
  assert_eq!(settled["data"]["cache"]["preprocessing_cached"], true);
  assert_eq!(without_cache(settled), without_cache(dynamic_report(&snapshot, false)));
}

#[test]
fn incremental_check_only_reuses_only_a_successful_unchanged_entry_closure() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("tests/fixtures/ffi-boundary-evidence.cirru", &snapshot).expect("analysis fixture should copy");

  let cold = incremental_check(&snapshot);
  assert_success(&cold, "cold incremental strict check");
  let cold_stdout = String::from_utf8_lossy(&cold.stdout);
  assert!(
    cold_stdout.contains("preprocessing-cached=false status=cold"),
    "stdout: {cold_stdout}"
  );

  let warm = incremental_check(&snapshot);
  assert_success(&warm, "warm incremental strict check");
  let warm_stdout = String::from_utf8_lossy(&warm.stdout);
  assert!(warm_stdout.contains("Check passed (cached)"), "stdout: {warm_stdout}");
  assert!(
    warm_stdout.contains("preprocessing-cached=true status=warm"),
    "stdout: {warm_stdout}"
  );

  let add_unrelated = run_calcit(
    &snapshot,
    &[
      "edit",
      "def",
      "ffi-evidence.main/cache-probe",
      "--code",
      "quote $ defn cache-probe () 1",
    ],
  );
  assert_success(&add_unrelated, "add out-of-closure definition");
  let unrelated = incremental_check(&snapshot);
  assert_success(&unrelated, "strict check after out-of-closure edit");
  let unrelated_stdout = String::from_utf8_lossy(&unrelated.stdout);
  assert!(
    unrelated_stdout.contains("preprocessing-cached=true status=warm"),
    "stdout: {unrelated_stdout}"
  );

  let schema_change = run_calcit(
    &snapshot,
    &[
      "edit",
      "schema",
      "ffi-evidence.main/query-host",
      "--code",
      "quote $ :: 'Fn $ {} (:return 'Dynamic) (:args $ [] 'JsObject) (:features $ #{} :js-ffi)",
    ],
  );
  assert_success(&schema_change, "change reachable schema");
  let invalidated = incremental_check(&snapshot);
  assert_success(&invalidated, "strict check after reachable schema edit");
  let invalidated_stdout = String::from_utf8_lossy(&invalidated.stdout);
  assert!(
    invalidated_stdout.contains("preprocessing-cached=false status=cold reason=dependency-closure-changed"),
    "stdout: {invalidated_stdout}"
  );

  let break_root = run_calcit(
    &snapshot,
    &[
      "edit",
      "def",
      "ffi-evidence.main/reload!",
      "--overwrite",
      "--code",
      "quote $ defn reload! () missing-strict-check-definition",
    ],
  );
  assert_success(&break_root, "break a strict-check root");
  let failed = incremental_check(&snapshot);
  assert!(!failed.status.success(), "changed invalid root must not reuse a successful result");
  let failed_stdout = String::from_utf8_lossy(&failed.stdout);
  assert!(!failed_stdout.contains("Check passed (cached)"), "stdout: {failed_stdout}");
}

#[test]
fn incremental_check_only_rejects_non_check_and_keep_going_modes() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("tests/fixtures/ffi-boundary-evidence.cirru", &snapshot).expect("analysis fixture should copy");

  let direct_run = run_calcit(&snapshot, &["--incremental"]);
  assert!(!direct_run.status.success(), "incremental direct execution should fail");
  assert!(
    String::from_utf8_lossy(&direct_run.stderr).contains("only available with direct `--check-only`"),
    "stderr: {}",
    String::from_utf8_lossy(&direct_run.stderr)
  );

  let keep_going = run_calcit(&snapshot, &["--check-only", "--incremental", "--keep-going"]);
  assert!(!keep_going.status.success(), "incremental keep-going should fail");
  assert!(
    String::from_utf8_lossy(&keep_going.stderr).contains("does not support `--keep-going`"),
    "stderr: {}",
    String::from_utf8_lossy(&keep_going.stderr)
  );
}

#[test]
fn incremental_dynamic_methods_invalidates_schema_import_and_type_slot_changes() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("tests/fixtures/ffi-boundary-evidence.cirru", &snapshot).expect("analysis fixture should copy");

  let cold = dynamic_report(&snapshot, true);
  assert_eq!(cold["data"]["cache"]["status"], "cold");
  let warm = dynamic_report(&snapshot, true);
  assert_eq!(warm["data"]["cache"]["status"], "warm");

  let schema_change = run_calcit(
    &snapshot,
    &[
      "edit",
      "schema",
      "ffi-evidence.main/query-host",
      "--code",
      "quote $ :: 'Fn $ {} (:return 'Dynamic) (:args $ [] 'JsObject) (:features $ #{} :js-ffi)",
    ],
  );
  assert_success(&schema_change, "change a reachable definition schema");
  let schema_invalidated = dynamic_report(&snapshot, true);
  assert_eq!(schema_invalidated["data"]["cache"]["status"], "cold");
  assert_eq!(schema_invalidated["data"]["cache"]["reason"], "dependency-closure-changed");
  assert_eq!(
    schema_invalidated["data"]["cache"]["dependency_index"]["miss_reasons"]["definition-changed"],
    1
  );
  assert_eq!(without_cache(schema_invalidated), without_cache(dynamic_report(&snapshot, false)));
  assert_eq!(dynamic_report(&snapshot, true)["data"]["cache"]["status"], "warm");

  let import_change = run_calcit(
    &snapshot,
    &[
      "edit",
      "add-import",
      "ffi-evidence.main",
      "--code",
      "quote $ ffi.helpers :refer $ dependency-number",
    ],
  );
  assert_success(&import_change, "change a reachable namespace import");
  let import_invalidated = dynamic_report(&snapshot, true);
  assert_eq!(import_invalidated["data"]["cache"]["status"], "cold");
  assert_eq!(import_invalidated["data"]["cache"]["reason"], "dependency-closure-changed");
  assert!(
    import_invalidated["data"]["cache"]["dependency_index"]["miss_reasons"]["namespace-changed"]
      .as_u64()
      .expect("namespace miss count should be numeric")
      > 0
  );
  assert_eq!(without_cache(import_invalidated), without_cache(dynamic_report(&snapshot, false)));
  assert_eq!(dynamic_report(&snapshot, true)["data"]["cache"]["status"], "warm");

  let type_slot_change = run_calcit(&snapshot, &["config", "set-type-slot", ":dispatch-op", ":dynamic"]);
  assert_success(&type_slot_change, "change the active entry type-slot policy");
  let type_slot_invalidated = dynamic_report(&snapshot, true);
  assert_eq!(type_slot_invalidated["data"]["cache"]["status"], "cold");
  assert_eq!(type_slot_invalidated["data"]["cache"]["reason"], "entry-policy-changed");
  assert_eq!(
    without_cache(type_slot_invalidated),
    without_cache(dynamic_report(&snapshot, false))
  );
  assert_eq!(dynamic_report(&snapshot, true)["data"]["cache"]["status"], "warm");
}

#[test]
fn incremental_dynamic_methods_rejects_a_cached_module_that_can_no_longer_load() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  let fixture = fs::read_to_string("tests/fixtures/ffi-boundary-evidence.cirru").expect("analysis fixture should read");
  fs::write(&snapshot, &fixture).expect("analysis fixture should write");

  let dependency_directory = directory.0.join("dep");
  fs::create_dir(&dependency_directory).expect("dependency directory should create");
  let dependency_snapshot = dependency_directory.join("calcit.cirru");
  fs::write(&dependency_snapshot, fixture.replace("ffi-evidence", "cached-dependency")).expect("dependency fixture should write");
  let add_module = run_calcit(&snapshot, &["config", "add-module", "./dep/"]);
  assert_success(&add_module, "add cached dependency module");

  let cold = dynamic_report(&snapshot, true);
  assert_eq!(cold["data"]["cache"]["preprocessing_cached"], false);
  let warm = dynamic_report(&snapshot, true);
  assert_eq!(warm["data"]["cache"]["preprocessing_cached"], true);

  fs::remove_file(&dependency_snapshot).expect("cached dependency should become unavailable");
  let incremental = run_calcit(
    &snapshot,
    &["--compat-types", "analyze", "dynamic-methods", "--incremental", "--format", "json"],
  );
  assert!(!incremental.status.success(), "incomplete incremental snapshot should fail");
  assert!(
    String::from_utf8_lossy(&incremental.stderr).contains("requires a complete Snapshot"),
    "incremental failure should explain the completeness requirement: {}",
    String::from_utf8_lossy(&incremental.stderr)
  );

  let uncached = run_calcit(&snapshot, &["--compat-types", "analyze", "dynamic-methods", "--format", "json"]);
  assert!(!uncached.status.success(), "incomplete uncached snapshot should fail");
}
