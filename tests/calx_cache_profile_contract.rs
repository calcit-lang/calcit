use std::process::Command;

use serde_json::Value;

fn run(args: &[&str]) -> std::process::Output {
  Command::new(env!("CARGO_BIN_EXE_calcit-calx-bench"))
    .args(args)
    .output()
    .expect("cache profile runner must start")
}

#[test]
fn cache_profile_stdout_is_one_versioned_correctness_checked_json_value() {
  let output = run(&[
    "--kernel",
    "affine",
    "--size",
    "10",
    "--cache-profile-warmup",
    "1",
    "--cache-profile-iterations",
    "2",
  ]);
  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  let stdout = String::from_utf8(output.stdout).expect("cache profile stdout must be UTF-8");
  let report: Value = serde_json::from_str(stdout.trim()).expect("stdout must contain exactly one JSON value");

  assert_eq!(report["schema"], "calcit-calx-cache-profile/1");
  assert_eq!(report["workload"], "revision-validated-cache-hit-plus-fresh-vm");
  assert_eq!(report["correctness"], true);
  assert_eq!(report["cache"]["misses"], 1);
  assert_eq!(report["cache"]["initialMissReason"], "empty");
  assert_eq!(report["cache"]["entries"], 1);
  assert!(report["cache"]["hits"].as_u64().is_some_and(|value| value >= 3));
  assert!(report["cache"]["estimatedBytes"].as_u64().is_some_and(|value| value > 0));
  assert!(report["runtime"]["hitPreparePerIterationNs"].as_u64().is_some());
  assert!(report["runtime"]["revisionValidationPerIterationNs"].as_u64().is_some());
  assert!(report["runtime"]["bindingAttachmentPerIterationNs"].as_u64().is_some());
  assert!(report["runtime"]["freshVmSetupPerIterationNs"].as_u64().is_some());
  assert!(report["runtime"]["freshVmExecutionPerIterationNs"].as_u64().is_some());
  assert!(report["runtime"]["reusedVmExecutionPerIterationNs"].as_u64().is_some());
  assert!(report["runtime"]["cachedNativeExecutionPerIterationNs"].as_u64().is_some());
}

#[test]
fn compile_and_cache_profile_modes_are_mutually_exclusive() {
  let output = run(&["--compile-profile-iterations", "1", "--cache-profile-iterations", "1"]);
  assert!(!output.status.success());
  assert!(output.stdout.is_empty(), "failure must not emit partial JSON");
  assert!(
    String::from_utf8_lossy(&output.stderr).contains("mutually exclusive"),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
}
