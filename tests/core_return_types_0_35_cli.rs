//! 0.35.0 narrows two core return types: `&map:diff-triple` returns
//! `Tuple (Set K) (Map K W) (List (Tuple K V W))` (#2549), and the Cirru list
//! conversions return `List CirruNode` (#2556).

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn run_calcit(args: &[&str], stdin: Option<&str>) -> Output {
  let mut child = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .args(args)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("calcit command should run");
  if let Some(input) = stdin {
    child.stdin.take().expect("stdin").write_all(input.as_bytes()).expect("write stdin");
  }
  child.wait_with_output().expect("calcit output")
}

fn text(output: &Output) -> String {
  format!(
    "{}{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  )
}

#[test]
fn whole_diff_triple_into_a_list_parameter_is_rejected() {
  let snapshot = Path::new("tests/fixtures/core-return-types-0-35.cirru");
  let output = run_calcit(&[snapshot.to_str().unwrap(), "--check-only"], None);
  let text = text(&output);
  assert!(!output.status.success(), "{text}");
  // Only `count-rows triple` breaks; `&list:nth triple 0`, `count triple` and
  // `count-rows rows` (a `List CirruNode` into `List Dynamic`) still pass.
  let mismatches: Vec<&str> = text.lines().filter(|line| line.contains("W_FN_ARG_TYPE_MISMATCH")).collect();
  assert_eq!(mismatches.len(), 1, "{text}");
  assert!(mismatches[0].contains("count-rows"), "{text}");
  assert!(mismatches[0].contains("tuple<set<string>"), "{text}");
}

#[test]
fn cirru_rows_into_a_string_list_parameter_are_rejected() {
  for rows in ["rows $ .unwrap $ .to-list $ cirru-quote (a b)", "rows $ parse-cirru-list |c"] {
    let snippet =
      format!("let\n    names $ fn (xs)\n      hint-fn $ Fn (List String) (-> Number)\n      count xs\n    {rows}\n  names rows\n");
    let output = run_calcit(&["eval", "--stdin"], Some(&snippet));
    let text = text(&output);
    assert!(!output.status.success(), "{text}");
    assert!(text.contains("calcit.core/CirruNode"), "{text}");
  }
}

#[test]
fn tuple_slots_and_cirru_rows_into_dynamic_lists_still_pass() {
  let snippet = "let\n    triple $ &map:diff-triple ({} (|a 1)) ({} (|b 2))\n    any-rows $ fn (xs)\n      hint-fn $ Fn (List Dynamic) (-> Number)\n      count xs\n  [] (&list:nth triple 0) (count triple) (any-rows $ parse-cirru-list |c)\n";
  let output = run_calcit(&["eval", "--stdin"], Some(snippet));
  let text = text(&output);
  assert!(output.status.success(), "{text}");
  assert!(text.contains("([] (#{} |a) 3 1)"), "{text}");
}

#[test]
fn method_spelling_of_diff_triple_returns_the_tuple_too() {
  let snippet = "let\n    rows $ fn (xs)\n      hint-fn $ Fn (List Dynamic) (-> Number)\n      count xs\n    triple $ .diff-triple ({} (|a 1)) ({} (|b 2))\n  rows triple\n";
  let output = run_calcit(&["eval", "--stdin"], Some(snippet));
  let text = text(&output);
  assert!(!output.status.success(), "{text}");
  assert!(text.contains("tuple<set<string>"), "{text}");
}
