//! Branches with no common type and a `cond` without a final `true` branch
//! get their own diagnostic codes instead of `got unknown` or a bare string
//! raised by the macro (#2651).

use std::process::Command;

#[test]
fn branch_and_cond_failures_have_dedicated_codes() {
  let output = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args(["--tips-level", "none", "tests/fixtures/branch-diagnostics.cirru"])
    .args(["--check-only", "--ns", "demo.main", "--format", "json"])
    .output()
    .expect("calcit command should run");
  assert!(!output.status.success());
  let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("one JSON document");
  let diagnostics = report["diagnostics"].as_array().expect("diagnostics array");
  let find = |definition: &str| {
    diagnostics
      .iter()
      .find(|item| item["definition"] == definition)
      .unwrap_or_else(|| panic!("no diagnostic for {definition}: {diagnostics:#?}"))
  };
  // `recur-mixed` also has a `recur` exit, which adds no value branch, and
  // `async-mixed` names the adopted types of its pending exits.
  for definition in [
    "demo.main/if-mixed",
    "demo.main/cond-mixed",
    "demo.main/recur-mixed",
    "demo.main/async-mixed",
  ] {
    let item = find(definition);
    assert_eq!(item["code"], "E_BRANCH_TYPE_MISMATCH", "{item:#?}");
    let message = item["message"].as_str().unwrap();
    assert!(
      message.ends_with("branch 1: `:number`, branch 2: `:string` (does not satisfy the return type)"),
      "{message}"
    );
  }
  let item = find("demo.main/match-mixed");
  assert_eq!(item["code"], "E_BRANCH_TYPE_MISMATCH", "{item:#?}");
  assert!(
    item["message"]
      .as_str()
      .unwrap()
      .contains("`:string` (does not satisfy the return type)")
  );

  let item = find("demo.main/cond-no-default");
  assert_eq!(item["code"], "E_COND_MISSING_DEFAULT", "{item:#?}");
  assert_eq!(item["location"]["def"], "cond-no-default");
  assert!(item["hint"].as_str().unwrap().contains("true <fallback>"));

  for passing in ["demo.main/if-ok", "demo.main/cond-raise"] {
    assert!(
      diagnostics.iter().all(|item| item["definition"] != passing),
      "{passing}: {diagnostics:#?}"
    );
  }
  assert_eq!(diagnostics.len(), 6, "{diagnostics:#?}");
}
