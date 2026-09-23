use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn emit_js(emit_path: &Path) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .args(["--tips-level", "none", "--init-fn", "app.main/manifest-main!", "--emit-path"])
    .arg(emit_path)
    .args(["examples/wasi-command/calcit.cirru", "js"])
    .output()
    .expect("Calcit CLI should run")
}

#[test]
fn js_codegen_creates_nested_output_and_only_reports_written_artifacts() {
  let root = tempfile::tempdir().expect("temporary output root");
  let emit_path = root.path().join("not-created/js");
  assert!(!emit_path.exists());

  let output = emit_js(&emit_path);
  assert!(
    output.status.success(),
    "stdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  assert!(emit_path.join("app.main.mjs").is_file());
  let stdout = String::from_utf8_lossy(&output.stdout);
  let emitted: Vec<_> = stdout.lines().filter_map(|line| line.strip_prefix("emitted: ")).collect();
  assert!(!emitted.is_empty(), "expected artifact paths in stdout: {stdout}");
  for path in emitted {
    assert!(Path::new(path).is_file(), "reported artifact does not exist: {path}");
  }
}

#[test]
fn js_codegen_rejects_output_path_that_is_a_file_without_claiming_success() {
  let root = tempfile::tempdir().expect("temporary output root");
  let emit_path = root.path().join("blocked-js");
  fs::write(&emit_path, "keep this file").expect("block output directory with a regular file");

  let output = emit_js(&emit_path);
  assert!(
    !output.status.success(),
    "codegen falsely succeeded:\n{}",
    String::from_utf8_lossy(&output.stdout)
  );
  assert!(!String::from_utf8_lossy(&output.stdout).contains("emitted:"));
  assert!(String::from_utf8_lossy(&output.stderr).contains("blocked-js"));
  assert_eq!(fs::read_to_string(&emit_path).expect("blocking file remains"), "keep this file");
}

#[test]
fn js_codegen_does_not_report_partial_output_when_an_artifact_is_blocked() {
  let root = tempfile::tempdir().expect("temporary output root");
  let emit_path = root.path().join("js-out");
  fs::create_dir(&emit_path).expect("output directory should create");
  fs::create_dir(emit_path.join("app.main.mjs")).expect("block a generated artifact with a directory");

  let output = emit_js(&emit_path);
  assert!(!output.status.success());
  assert!(!String::from_utf8_lossy(&output.stdout).contains("emitted:"));
  assert!(String::from_utf8_lossy(&output.stderr).contains("app.main.mjs"));
}
