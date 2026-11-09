//! `update` on a known Struct gives the updater the declared field type and
//! checks its result against that type, as `update` on a Map does (#2647).

use std::fs;
use std::process::Command;

#[test]
fn struct_update_callbacks_take_the_field_type() {
  let dir = std::env::temp_dir().join(format!("calcit-struct-update-{}", std::process::id()));
  let _ = fs::remove_dir_all(&dir);
  fs::create_dir_all(&dir).unwrap();
  let snapshot = dir.join("calcit.cirru");
  fs::copy("tests/fixtures/struct-update-callback.cirru", &snapshot).unwrap();
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
  assert!(text.contains("[passed] demo.main/c-ok (fn)"), "{text}");
  // The callback input is the `:title` String, not an open value.
  assert!(
    text.contains("`calcit.core/+` arg 2 expects type `:number`, but got `:string` in call at demo.main/c12"),
    "{text}"
  );
  assert!(!text.contains("E_CALL_ARGUMENT_UNPROVEN"), "{text}");
  // The callback result is checked against the field type.
  assert!(
    text.contains(
      "`calcit.core/update` arg 3 expects type `fn(:string) -> :string`, but got `fn(:string) -> :number` in call at demo.main/c-ret"
    ),
    "{text}"
  );
}
