//! `--check-only` previews the 0.35.0 core return types (#2564): a passing
//! project that calls `&map:diff-triple` or a Cirru list conversion is checked
//! once more under the narrowed types, and what that adds is reported as
//! non-blocking notes.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

const PREVIEW_CODE: &str = "W_0_35_RETURN_TYPE_PREVIEW";

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

fn preview_notes(output: &Output) -> Vec<String> {
  let text = format!(
    "{}{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  text.lines().filter(|line| line.contains(PREVIEW_CODE)).map(str::to_owned).collect()
}

#[test]
fn whole_diff_triple_into_a_list_parameter_is_previewed_but_slot_reads_are_not() {
  let snapshot = Path::new("tests/fixtures/preview-0-35-return-types.cirru");
  let output = run_calcit(&[snapshot.to_str().unwrap(), "--check-only"], None);
  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  let notes = preview_notes(&output);
  // `count-rows triple` breaks in 0.35.0; `&list:nth triple 0`, `count triple`
  // and `count-rows rows` (a `List CirruNode` into `List Dynamic`) do not.
  assert_eq!(notes.len(), 1, "{notes:?}");
  assert!(notes[0].contains("count-rows"), "{notes:?}");
  assert!(notes[0].contains("tuple<set<string>"), "{notes:?}");
}

#[test]
fn cirru_rows_into_a_string_list_parameter_are_previewed() {
  let snippet = "let\n    names $ fn (xs)\n      hint-fn $ Fn (List String) (-> Number)\n      count xs\n    any-rows $ fn (xs)\n      hint-fn $ Fn (List Dynamic) (-> Number)\n      count xs\n    rows $ .unwrap $ .to-list $ cirru-quote (a b)\n    parsed $ parse-cirru-list |c\n  [] (names rows) (names parsed) (any-rows rows)\n";
  let output = run_calcit(&["eval", "--stdin"], Some(snippet));
  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  let notes = preview_notes(&output);
  assert_eq!(notes.len(), 2, "{notes:?}");
  assert!(notes.iter().all(|note| note.contains("calcit.core/CirruNode")), "{notes:?}");
}

#[test]
fn projects_without_the_changed_apis_get_no_preview() {
  let snapshot = Path::new("tests/fixtures/query-entry-page-only-module.cirru");
  let output = run_calcit(&[snapshot.to_str().unwrap(), "--check-only"], None);
  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  assert!(preview_notes(&output).is_empty());
}

#[test]
fn method_spelling_of_diff_triple_also_triggers_the_preview() {
  let snippet = "let\n    rows $ fn (xs)\n      hint-fn $ Fn (List Dynamic) (-> Number)\n      count xs\n    triple $ .diff-triple ({} (|a 1)) ({} (|b 2))\n  rows triple\n";
  let output = run_calcit(&["eval", "--stdin"], Some(snippet));
  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  let notes = preview_notes(&output);
  assert_eq!(notes.len(), 1, "{notes:?}");
  assert!(notes[0].contains("tuple<set<string>"), "{notes:?}");
}
