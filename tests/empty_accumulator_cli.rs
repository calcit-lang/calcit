//! An empty `([])` / `({})` / `(#{})` fold seed takes its element types from
//! what the callback adds, so a declared `Set String` result is proven
//! without `assert-type` on the seed (#2646).

use std::fs;
use std::process::Command;

const FIXTURE: &str = "tests/fixtures/empty-accumulator.cirru";

fn check(source: &str, label: &str) -> (bool, String) {
  let dir = std::env::temp_dir().join(format!("calcit-empty-accumulator-{label}-{}", std::process::id()));
  let _ = fs::remove_dir_all(&dir);
  fs::create_dir_all(&dir).unwrap();
  let snapshot = dir.join("calcit.cirru");
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
fn empty_seeds_take_the_callback_element_types() {
  let source = fs::read_to_string(FIXTURE).unwrap();
  let (ok, text) = check(&source, "pass");
  assert!(ok, "{text}");
  for def in ["q10", "q-list", "q-map", "q-prefix", "q-let"] {
    assert!(text.contains(&format!("[passed] demo.main/{def} (fn)")), "{def}:\n{text}");
  }
}

#[test]
fn a_widened_seed_still_meets_the_declared_result() {
  let source = fs::read_to_string(FIXTURE)
    .unwrap()
    .replace("(include acc $ :title t)", "(include acc $ :done t)");
  let (ok, text) = check(&source, "bad");
  assert!(!ok, "{text}");
  assert!(text.contains("[warning] demo.main/q10 (fn)"), "{text}");
  assert!(
    text.contains("declares return type `set<string>`, but body returns `set<bool>`"),
    "{text}"
  );
}
