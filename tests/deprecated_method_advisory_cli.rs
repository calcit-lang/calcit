use std::process::Command;

/// A deprecated method on a proven receiver is reported as a note and does not fail `--check-only`.
#[test]
fn deprecated_methods_are_advisory_in_check_only() {
  let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/calcit/test-deprecated-methods.cirru");
  let output = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args(["--tips-level", "none", fixture, "--check-only"])
    .output()
    .expect("calcit command should run");
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(output.status.success(), "advisories must not fail the check: {stderr}");
  assert!(stderr.contains("2 deprecated usages"), "stderr: {stderr}");
  assert!(
    stderr.contains("[A_DEPRECATED_METHOD]") && stderr.contains("use `.len`"),
    "stderr: {stderr}"
  );
  assert!(stderr.contains("use `.append`"), "stderr: {stderr}");
  assert!(!stderr.contains("warnings during preprocessing"), "stderr: {stderr}");
}

/// The analyzer reports the same deprecated methods, by receiver type, with call-site paths.
#[test]
fn deprecated_methods_are_reported_by_analyzer() {
  let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/calcit/test-deprecated-methods.cirru");
  let output = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args([
      "--tips-level",
      "none",
      fixture,
      "analyze",
      "weak-types",
      "--only",
      "deprecated-call",
    ])
    .output()
    .expect("calcit command should run");
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
  assert!(stdout.contains("- calls: 2"), "stdout: {stdout}");
  assert!(stdout.contains("calcit.core/.count is deprecated"), "stdout: {stdout}");
  assert!(stdout.contains("calcit.core/.add is deprecated"), "stdout: {stdout}");
}
