//! `map` / `map-indexed` over a homogeneous tuple keep its length: the result
//! is a tuple of the same size whose slots carry the callback's output type
//! (F3). Length-changing operations such as `filter` still return a List.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

const FIXTURE: &str = "tests/fixtures/tuple-map-length.cirru";

fn check(source: &str, label: &str) -> (bool, String) {
  let dir = std::env::temp_dir().join(format!("calcit-tuple-map-length-{label}-{}", std::process::id()));
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

fn replaced(from: &str, to: &str) -> String {
  let source = fs::read_to_string(FIXTURE).unwrap();
  assert!(source.contains(from), "fixture should contain `{from}`");
  source.replace(from, to)
}

#[test]
fn homogeneous_tuple_map_keeps_length() {
  let source = fs::read_to_string(FIXTURE).unwrap();
  let (ok, text) = check(&source, "pass");
  assert!(ok, "{text}");
  for def in [
    "v-scale",
    "v-scale-method",
    "v-labels",
    "v-weighted",
    "v-alias-scale",
    "v-keep-big",
    "main!",
  ] {
    assert!(text.contains(&format!("[passed] demo.main/{def} (fn)")), "{def}:\n{text}");
  }
}

#[test]
fn mapped_tuple_slots_follow_the_callback() {
  // The callback returns String, so the slots are String, not Number.
  let (ok, text) = check(
    &replaced(
      "Fn (Tuple Number Number Number) (-> (Tuple String String String))",
      "Fn (Tuple Number Number Number) (-> (Tuple Number Number Number))",
    ),
    "slot",
  );
  assert!(!ok, "{text}");
  assert!(text.contains("demo.main/v-labels"), "{text}");
  assert!(text.contains("tuple<string, string, string>"), "{text}");
}

#[test]
fn mapped_tuple_keeps_the_receiver_length() {
  let (ok, text) = check(
    &replaced(
      "Fn (Tuple Number Number Number) Number (-> (Tuple Number Number Number))\n        'v-scale-method",
      "Fn (Tuple Number Number Number) Number (-> (Tuple Number Number))\n        'v-scale-method",
    ),
    "length",
  );
  assert!(!ok, "{text}");
  assert!(text.contains("demo.main/v-scale"), "{text}");
  assert!(text.contains("tuple<number, number, number>"), "{text}");
}

#[test]
fn filter_over_a_tuple_still_returns_a_list() {
  let (ok, text) = check(
    &replaced(
      "Fn (Tuple Number Number Number) (-> (List Number))",
      "Fn (Tuple Number Number Number) (-> (Tuple Number Number Number))",
    ),
    "filter",
  );
  assert!(!ok, "{text}");
  assert!(text.contains("demo.main/v-keep-big"), "{text}");
  assert!(text.contains("list<number>"), "{text}");
}
