//! 0.35.0 passes Map entries to `filter` / `map` callbacks as `Tuple K V`, and a
//! `map` callback returning a `[] key value` pair produces `Map K2 V2` (#2561).

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
fn map_callbacks_infer_result_keys_and_values_in_every_spelling() {
  for call in [
    "map m $ fn (pair) $ let[] (k v) pair $ [] k (+ v 1)",
    ".map m $ fn (pair) $ let[] (k v) pair $ [] k (+ v 1)",
    "&map:map m $ fn (pair) ([] (&list:nth pair 0) (+ 1 (&list:nth pair 1)))",
  ] {
    let snippet = format!("let\n    m $ &{{}} :a 1\n    out $ {call}\n  assert-type out $ quote $ Map Tag Number\n  , out\n");
    let (ok, text) = eval(&snippet);
    assert!(ok, "{call}:\n{text}");
    assert!(text.contains("({} (:a 2))"), "{call}:\n{text}");
  }
}

#[test]
fn filter_callbacks_receive_typed_entries() {
  for call in [
    "filter m $ fn (pair) $ let[] (k v) pair $ > v 1",
    ".filter m $ fn (pair) $ > (&list:nth pair 1) 1",
  ] {
    let snippet = format!("let\n    m $ &{{}} :a 1 :b 2\n    out $ {call}\n  assert-type out $ quote $ Map Tag Number\n  , out\n");
    let (ok, text) = eval(&snippet);
    assert!(ok, "{call}:\n{text}");
    assert!(text.contains("({} (:b 2))"), "{call}:\n{text}");
  }
}

#[test]
fn callbacks_declared_over_open_lists_are_rejected() {
  for (call, code) in [
    (
      "map m $ fn (pair) (hint-fn $ Fn (List Dynamic) (-> $ List Dynamic)) pair",
      "W_FN_ARG_TYPE_MISMATCH",
    ),
    (
      ".filter m $ fn (pair) (hint-fn $ Fn (List String) (-> Bool)) true",
      "W_METHOD_ARG_TYPE_MISMATCH",
    ),
    (
      "map m $ fn (pair) (hint-fn $ Fn (Tuple Tag Number) (-> $ List Dynamic)) ([] (&list:nth pair 0) 2)",
      "W_FN_ARG_TYPE_MISMATCH",
    ),
  ] {
    let snippet = format!("let\n    m $ &{{}} :a 1\n  {call}\n");
    let (ok, text) = eval(&snippet);
    assert!(!ok, "{call} should be rejected:\n{text}");
    assert!(text.contains(code) && text.contains("tuple<"), "{call}:\n{text}");
  }
}
