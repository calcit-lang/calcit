//! `analyze weak-types --only inferred-dynamic` lists the locals and lambda
//! parameters that compiled as `Dynamic`, which the source scans cannot see
//! because no `Dynamic` is written there (#2650).

use std::process::Command;

fn run(args: &[&str]) -> serde_json::Value {
  let output = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("tests/fixtures/inferred-dynamic.cirru")
    .args(["analyze", "weak-types", "--only", "inferred-dynamic", "--format", "json"])
    .args(args)
    .output()
    .expect("calcit command should run");
  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  serde_json::from_slice(&output.stdout).expect("one JSON document")
}

#[test]
fn open_bindings_are_counted_by_source() {
  let report = run(&[]);
  let data = &report["data"];
  assert_eq!(data["view"], "inferred-dynamic");
  let rows = data["findings"]
    .as_array()
    .unwrap()
    .iter()
    .map(|row| {
      format!(
        "{} {} {} {} {}",
        row["definition"].as_str().unwrap(),
        row["binding"].as_str().unwrap(),
        row["name"].as_str().unwrap(),
        row["type"].as_str().unwrap(),
        row["source"].as_str().unwrap()
      )
    })
    .collect::<Vec<_>>();
  assert_eq!(
    rows,
    [
      // A directly invoked lambda takes its source from the call arguments.
      "demo.main/invoked param z Dynamic dynamic-source",
      "demo.main/open-input let copy Dynamic dynamic-source",
      "demo.main/open-input let field calcit.core/Option Dynamic map-field",
      // A lambda returned from `let` takes no contract from the binding pair.
      "demo.main/returned let copy Dynamic dynamic-source",
      "demo.main/returned param y Dynamic inference-failure",
      "demo.main/untyped param a Dynamic inference-failure",
      "demo.main/untyped param b Dynamic inference-failure",
    ],
    "typed bindings, typed lambdas and the definitions' own parameters are not listed"
  );
  let summary = &data["summary"];
  // The macro-built `gen` binding has no path, so it is counted but not listed.
  assert_eq!(summary["findings"], 8);
  assert_eq!(summary["by_binding"], serde_json::json!({ "let": 4, "param": 4 }));
  assert_eq!(
    summary["by_source"],
    serde_json::json!({ "dynamic-source": 4, "map-field": 1, "inference-failure": 3 })
  );
  assert_eq!(summary["unlocated_bindings"], 1);
  assert_eq!(summary["uncompiled_definitions"], 0);
  assert_eq!(report["diagnostics"], serde_json::json!([]));
}

#[test]
fn summary_only_keeps_counts() {
  let report = run(&["--summary-only"]);
  let summary = &report["data"]["summary"];
  assert_eq!(summary["findings"], 8);
  assert_eq!(summary["by_binding"], serde_json::json!({ "let": 4, "param": 4 }));
  assert_eq!(
    summary["by_source"],
    serde_json::json!({ "dynamic-source": 4, "map-field": 1, "inference-failure": 3 })
  );
  assert_eq!(summary["unlocated_bindings"], 1);
  assert_eq!(report["data"]["findings"], serde_json::json!([]));
}

#[test]
fn view_rejects_evidence_and_incremental_options() {
  for args in [
    &["--only", "inferred-dynamic", "--incremental"][..],
    &["--only", "inferred-dynamic", "--intent", "unresolved"][..],
    &["--only", "inferred-dynamic,code-nil"][..],
  ] {
    let output = Command::new(env!("CARGO_BIN_EXE_calcit"))
      .arg("tests/fixtures/inferred-dynamic.cirru")
      .args(["analyze", "weak-types"])
      .args(args)
      .output()
      .expect("calcit command should run");
    assert!(!output.status.success(), "{args:?} should be rejected");
  }
}
