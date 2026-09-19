use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

struct TempEdnFile(PathBuf);

impl TempEdnFile {
  fn create(content: &str) -> Self {
    let nonce = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("test clock should be valid")
      .as_nanos();
    let path = std::env::temp_dir().join(format!("calcit-parse-edn-{}-{nonce}.cirru", std::process::id()));
    fs::write(&path, content).expect("Cirru EDN fixture should be written");
    Self(path)
  }
}

impl Drop for TempEdnFile {
  fn drop(&mut self) {
    let _ = fs::remove_file(&self.0);
  }
}

fn run_calcit(args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .args(args)
    .output()
    .expect("calcit command should run")
}

fn run_calcit_with_stdin(args: &[&str], input: &str) -> Output {
  let mut child = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .args(args)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("calcit command should spawn");
  child
    .stdin
    .take()
    .expect("stdin should be piped")
    .write_all(input.as_bytes())
    .expect("Cirru EDN should be written to stdin");
  child.wait_with_output().expect("calcit command should finish")
}

fn assert_success(output: &Output) {
  assert!(
    output.status.success(),
    "command failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
}

#[test]
fn parse_edn_file_supports_inputs_larger_than_argument_limits() {
  let expected = "x".repeat(140 * 1024);
  let fixture = TempEdnFile::create(&format!("do |{expected}"));
  let output = run_calcit(&[
    "cirru",
    "parse-edn",
    "--file",
    fixture.0.to_str().expect("fixture path should be UTF-8"),
  ]);

  assert_success(&output);
  let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout should contain one JSON value");
  assert_eq!(parsed.as_str(), Some(expected.as_str()));
}

#[test]
fn parse_edn_file_dash_reads_stdin() {
  let source = "{} (:name |Ada) (:age 23)";
  let inline = run_calcit(&["cirru", "parse-edn", source]);
  let stdin = run_calcit_with_stdin(&["cirru", "parse-edn", "--file", "-"], source);

  assert_success(&inline);
  assert_success(&stdin);
  assert_eq!(stdin.stdout, inline.stdout);
}

#[test]
fn parse_edn_rejects_missing_or_ambiguous_inputs() {
  let missing = run_calcit(&["cirru", "parse-edn"]);
  assert!(!missing.status.success());
  assert!(String::from_utf8_lossy(&missing.stderr).contains("Cirru EDN input is required"));

  let fixture = TempEdnFile::create("[] 1 2");
  let ambiguous = run_calcit(&[
    "cirru",
    "parse-edn",
    "[] 3 4",
    "--file",
    fixture.0.to_str().expect("fixture path should be UTF-8"),
  ]);
  assert!(!ambiguous.status.success());
  assert!(String::from_utf8_lossy(&ambiguous.stderr).contains("Cirru EDN input is ambiguous"));

  let inline_dash = run_calcit(&["cirru", "parse-edn", "--", "-"]);
  assert!(!inline_dash.status.success());
  let inline_dash_stderr = String::from_utf8_lossy(&inline_dash.stderr);
  assert!(inline_dash_stderr.contains("Failed to parse Cirru EDN"));
  assert!(!inline_dash_stderr.contains("read Cirru EDN from stdin"));
}
