//! A lambda's `[] a b` tail is checked positionally against an expected
//! `Tuple`, whether the lambda is returned directly or mapped into a
//! `List (Tuple ..)` return (#2645).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

const FIXTURE: &str = "tests/fixtures/tuple-callback-return.cirru";

fn check(source: &str, label: &str) -> (bool, String) {
  let dir = std::env::temp_dir().join(format!("calcit-tuple-callback-{label}-{}", std::process::id()));
  let _ = fs::remove_dir_all(&dir);
  fs::create_dir_all(&dir).unwrap();
  let snapshot: PathBuf = dir.join("calcit.cirru");
  fs::write(&snapshot, source).unwrap();
  let output = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args(["--tips-level", "none"])
    .arg(&snapshot)
    .args(["--check-only", "--ns", "demo.main"])
    .output()
    .expect("calcit command should run");
  let _ = fs::remove_dir_all(&dir);
  let text = format!(
    "{}{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  (output.status.success(), text)
}

#[test]
fn lambda_tails_prove_expected_tuple_returns() {
  let source = fs::read_to_string(FIXTURE).unwrap();
  let (ok, text) = check(&source, "pass");
  assert!(ok, "{text}");
  for def in ["direct", "lam", "q05", "q-map", "q-let"] {
    assert!(text.contains(&format!("[passed] demo.main/{def} (fn)")), "{def}:\n{text}");
  }
}

#[test]
fn swapped_tuple_slots_are_still_rejected() {
  let source = fs::read_to_string(FIXTURE)
    .unwrap()
    .replace("([] (:title t) (:done t))", "([] (:done t) (:title t))")
    .replace("[] title (:done t)", "[] (:done t) title");
  let (ok, text) = check(&source, "swap");
  assert!(!ok, "{text}");
  for def in ["direct", "lam", "q05", "q-map", "q-let"] {
    assert!(text.contains(&format!("[warning] demo.main/{def} (fn)")), "{def}:\n{text}");
  }
  assert!(text.contains("list<tuple<bool, string>>"), "{text}");
}
