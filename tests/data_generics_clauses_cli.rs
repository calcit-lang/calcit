//! `defstruct` / `defenum` declare generics and bounds with the same
//! `(:generics …)` and `(:where …)` clauses as `Fn` (#2700); the 0.41
//! positional `([] T)` / `({} (T Debug))` spelling still loads and
//! `edit format` rewrites it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const FIXTURE: &str = "tests/fixtures/data-generics-clauses.cirru";

fn temp_snapshot(source: &str, label: &str) -> (PathBuf, PathBuf) {
  let dir = std::env::temp_dir().join(format!("calcit-data-generics-{label}-{}", std::process::id()));
  let _ = fs::remove_dir_all(&dir);
  fs::create_dir_all(&dir).unwrap();
  let snapshot = dir.join("calcit.cirru");
  fs::write(&snapshot, source).unwrap();
  (dir, snapshot)
}

fn run(snapshot: &Path, args: &[&str]) -> (bool, String) {
  let output = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args(["--tips-level", "none"])
    .arg(snapshot)
    .args(args)
    .output()
    .expect("calcit command should run");
  let text = format!(
    "{}{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  (output.status.success(), text)
}

fn check(source: &str, label: &str) -> (bool, String) {
  let (dir, snapshot) = temp_snapshot(source, label);
  let result = run(&snapshot, &["--check-only", "--ns", "demo.main"]);
  let _ = fs::remove_dir_all(&dir);
  result
}

fn replaced(from: &str, to: &str) -> String {
  let source = fs::read_to_string(FIXTURE).unwrap();
  assert!(source.contains(from), "fixture should contain `{from}`");
  source.replace(from, to)
}

#[test]
fn generics_and_where_clauses_type_check() {
  let (ok, text) = check(&fs::read_to_string(FIXTURE).unwrap(), "pass");
  assert!(ok, "{text}");
  for def in ["Box", "Pair", "Shown", "box-value", "first-of", "main!"] {
    assert!(text.contains(&format!("[passed] demo.main/{def} ")), "{def}:\n{text}");
  }
}

#[test]
fn clause_generics_are_bound_at_call_sites() {
  let (ok, text) = check(&replaced("box-value $ Box :value 1", "box-value $ Box :value |a"), "bound");
  assert!(!ok, "{text}");
  assert!(text.contains("box-value"), "{text}");
}

#[test]
fn positional_generics_still_load_and_format_into_clauses() {
  let legacy = replaced("defstruct Box (:generics T) (:value T)", "defstruct Box ([] T) (:value T)")
    .replace("defenum Pair (:generics A B)", "defenum Pair ([] A B)")
    .replace(
      "defstruct Shown (:generics T)\n            :where $ T Debug",
      "defstruct Shown ([] T)\n            {} $ T Debug",
    );
  assert!(
    !legacy.contains("(:generics T") && !legacy.contains("(:generics A"),
    "every declaration uses the positional spelling"
  );
  let (dir, snapshot) = temp_snapshot(&legacy, "legacy");
  let (ok, text) = run(&snapshot, &["--check-only", "--ns", "demo.main"]);
  assert!(ok, "positional generics stay accepted in 0.42:\n{text}");
  let (ok, text) = run(&snapshot, &["edit", "format"]);
  assert!(ok, "{text}");
  let formatted = fs::read_to_string(&snapshot).unwrap();
  let _ = fs::remove_dir_all(&dir);
  assert!(formatted.contains("defstruct Box (:generics T) (:value T)"), "{formatted}");
  assert!(formatted.contains("defenum Pair (:generics A B)"), "{formatted}");
  assert!(formatted.contains(":where $ T Debug"), "{formatted}");
  assert!(!formatted.contains("([] T)"), "{formatted}");
}

#[test]
fn where_clause_without_generics_is_rejected() {
  let (ok, text) = check(
    &replaced(
      "defstruct Box (:generics T) (:value T)",
      "defstruct Box (:where (T Debug)) (:value Number)",
    ),
    "where-only",
  );
  assert!(!ok, "{text}");
  assert!(text.contains("follows a `(:generics …)` clause"), "{text}");
}
