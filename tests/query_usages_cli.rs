use std::process::{Command, Output};

const FIXTURE: &str = "tests/fixtures/query-usages-alias.cirru";

fn run_query_usages(target: &str) -> String {
  let output: Output = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .arg(FIXTURE)
    .args(["query", "usages", target])
    .output()
    .expect("calcit command should run");
  assert!(
    output.status.success(),
    "command failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8(output.stdout).expect("stdout should be UTF-8")
}

/// #2236: callers that import the target namespace with `:as` must be reported,
/// alongside `:refer`, fully qualified and same-namespace references.
#[test]
fn usages_resolve_namespace_alias_refer_and_qualified_calls() {
  let out = run_query_usages("app.schema/decode-story");

  assert!(out.contains("Usages: 6"), "unexpected usages report:\n{out}");
  for expected in [
    "app.alias-caller/reading-list [code]",
    "app.alias-caller/reading-list [tests]",
    "app.refer-caller/refer-call [code]",
    "app.refer-caller/qualified-call [code]",
    "app.schema/message-input [code]",
    "app.main/main! [code]",
  ] {
    assert!(out.contains(expected), "missing `{expected}` in:\n{out}");
  }
  assert!(out.contains("schema/decode-story"), "alias call context should be shown:\n{out}");
  // Only `:as` is imported there, so the bare parameter name is not a reference.
  assert!(!out.contains("unrelated-bare"), "bare name without :refer must not match:\n{out}");
}
