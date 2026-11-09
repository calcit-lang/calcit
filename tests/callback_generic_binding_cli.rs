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
  // `param-bound` calls a parameter typed with the generic signature, which
  // takes the indirect-call path rather than the named-function one.
  for def in ["inline", "let-bound", "alias-bound", "param-bound", "rest-bound"] {
    assert!(text.contains(&format!("[passed] demo.main/{def} (fn)")), "{def}:\n{text}");
  }
  assert!(!text.contains("E_CALL_ARGUMENT_UNPROVEN"), "{text}");
  // The callback's `store` is the String bound from `|base`, so `+` rejects it.
  for def in ["inline-bad", "let-bound-bad", "alias-bound-bad", "param-bound-bad"] {
    assert!(
      text.contains(&format!(
        "`calcit.core/+` arg 1 expects type `:number`, but got `:string` in call at demo.main/{def}"
      )),
      "{def}:\n{text}"
    );
  }
  // The callback must return `Store`, so a Number result is rejected too,
  // whether the callee is named or reached through a typed parameter.
  let returned = "arg 1 expects type `fn('Store, 'Op) -> 'Store`, but got `fn(:string, :number) -> :number`";
  for def in ["inline-bad", "let-bound-bad", "alias-bound-bad"] {
    assert!(
      text.contains(&format!("`demo.main/apply-op` {returned} in call at demo.main/{def}")),
      "{def}:\n{text}"
    );
  }
  assert!(
    text.contains(&format!("calling `run` {returned} at demo.main/param-bound-bad")),
    "{text}"
  );
}
