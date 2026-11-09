//! A callback passed before the arguments that fix its generics, as in
//! `Fn (Fn Store Op (-> Store)) Store Op (-> Store)`, takes `Store` and `Op`
//! from those later arguments instead of needing a `hint-fn` (#2648).

use std::fs;
use std::process::Command;

#[test]
fn callbacks_take_generics_from_later_arguments() {
  let dir = std::env::temp_dir().join(format!("calcit-callback-generic-{}", std::process::id()));
  let _ = fs::remove_dir_all(&dir);
  fs::create_dir_all(&dir).unwrap();
  let snapshot = dir.join("calcit.cirru");
  fs::copy("tests/fixtures/callback-generic-binding.cirru", &snapshot).unwrap();
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
  assert!(!output.status.success(), "{text}");
  for def in ["inline", "let-bound"] {
    assert!(text.contains(&format!("[passed] demo.main/{def} (fn)")), "{def}:\n{text}");
  }
  assert!(!text.contains("E_CALL_ARGUMENT_UNPROVEN"), "{text}");
  // The callback's `store` is the String bound from `|base`, so `+` rejects it.
  assert!(
    text.contains("`calcit.core/+` arg 1 expects type `:number`, but got `:string` in call at demo.main/inline-bad"),
    "{text}"
  );
}
