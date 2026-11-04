//! `encode-map-as` is the inverse of `decode-map-as` and only accepts values of
//! the declared type (#2562).

use std::io::Write;
use std::process::{Command, Output, Stdio};

fn eval(snippet: &str) -> (bool, String) {
  let mut child = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args(["--tips-level", "none", "eval", "--stdin"])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("calcit command should run");
  child
    .stdin
    .take()
    .expect("stdin")
    .write_all(snippet.as_bytes())
    .expect("write stdin");
  let output: Output = child.wait_with_output().expect("calcit output");
  let text = format!(
    "{}{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  (output.status.success(), text)
}

#[test]
fn encoded_struct_drops_absent_options_and_decodes_back() {
  let snippet = "let\n    r $ %{} RuntimeMapResponse (:code 200) (:message $ Option :none) (:body nil) (:meta $ Option :some $ %{} RuntimeMapMeta (:kind :x))\n    m $ encode-map-as r RuntimeMapResponse\n  assert= r $ decode-map-as m RuntimeMapResponse\n  , m\n";
  let (ok, text) = eval(snippet);
  assert!(ok, "{text}");
  assert!(text.contains("(:meta ({} (:kind :x)))"), "{text}");
  assert!(!text.contains(":message"), "{text}");
}

#[test]
fn values_of_another_type_are_rejected_before_running() {
  for value in ["1", "({} (:code 1))", "(Option :some 1)"] {
    let (ok, text) = eval(&format!("encode-map-as {value} RuntimeMapResponse\n"));
    assert!(!ok, "{value} should be rejected:\n{text}");
    assert!(text.contains("E_ENCODE_MAP_AS_TYPE_MISMATCH"), "{value}:\n{text}");
  }
}
