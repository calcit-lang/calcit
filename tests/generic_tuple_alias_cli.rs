//! A generic `Tuple A B` constructor proves its `[] a b` tail slot by slot,
//! and a plain alias such as `Vec3` for `Tuple Number Number Number` checks
//! return literals, `nth` and `count` as the tuple it names.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

const FIXTURE: &str = "tests/fixtures/generic-tuple-alias.cirru";

fn check(source: &str, label: &str) -> (bool, String) {
  let dir = std::env::temp_dir().join(format!("calcit-generic-tuple-alias-{label}-{}", std::process::id()));
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
fn generic_tuples_and_tuple_aliases_pass() {
  let source = fs::read_to_string(FIXTURE).unwrap();
  let (ok, text) = check(&source, "pass");
  assert!(ok, "{text}");
  for def in ["pair", "swap", "v-add", "v-len2", "v-count", "main!"] {
    assert!(text.contains(&format!("[passed] demo.main/{def} (fn)")), "{def}:\n{text}");
  }
}

#[test]
fn swapped_generic_slots_are_rejected() {
  let (ok, text) = check(&replaced("defn pair (a b) ([] a b)", "defn pair (a b) ([] b a)"), "swap");
  assert!(!ok, "{text}");
  assert!(text.contains("tuple<'B, 'A>"), "{text}");
}

#[test]
fn tuple_alias_slots_are_still_checked() {
  let (ok, text) = check(&replaced("[] (&+ x p) (&+ y q) (&+ z r)", "[] (&+ x p) |y (&+ z r)"), "slot");
  assert!(!ok, "{text}");
  assert!(text.contains("tuple<number, string, number>"), "{text}");

  let (ok, text) = check(&replaced("(&* (nth a 1) (nth a 1))", "(&* (nth a 3) (nth a 1))"), "index");
  assert!(!ok, "{text}");
  assert!(text.contains("out of range for a tuple of 3"), "{text}");
}
