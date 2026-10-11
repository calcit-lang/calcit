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
