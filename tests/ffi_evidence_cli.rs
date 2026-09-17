use std::path::Path;
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .arg(Path::new("tests/fixtures/ffi-boundary-evidence.cirru"))
    .arg("analyze")
    .arg("weak-types")
    .args(args)
    .output()
    .expect("weak-types command should run")
}

fn run_strict_fix(args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .arg(Path::new("tests/fixtures/ffi-boundary-strict.cirru"))
    .arg("fix")
    .arg("--workflow")
    .arg("strict")
    .args(args)
    .output()
    .expect("strict fix command should run")
}

fn assert_success(output: &Output) {
  assert!(
    output.status.success(),
    "command failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
}

#[test]
fn ffi_evidence_classifies_operations_and_emits_review_only_candidates() {
  let output = run(&["--ffi-evidence", "--format", "json"]);
  assert_success(&output);
  let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout should contain one JSON report");
  assert_eq!(report["schema_version"], 8);
  assert_eq!(report["data"]["filters"]["ffi_evidence"], true);
  assert_eq!(report["data"]["evidence"]["contract_status"], "review-required");
  assert_eq!(report["data"]["evidence"]["runtime_trust_inferred"], false);

  let boundaries = report["data"]["evidence"]["ffi_boundaries"]
    .as_array()
    .expect("FFI boundaries should be an array");
  let boundary = boundaries
    .iter()
    .find(|item| item["definition"] == "ffi-evidence.main/query-host")
    .expect("query-host evidence should exist");
  assert_eq!(boundary["diagnostic_code"], "I_FFI_BOUNDARY_EVIDENCE");
  assert_eq!(boundary["classification"], "mixed");
  assert_eq!(boundary["target"], "browser");
  assert_eq!(boundary["js_ffi_feature"], true);
  assert_eq!(
    boundary["callers"],
    serde_json::json!(["ffi-evidence.main/main!", "ffi-evidence.main/quasiquoted-caller"])
  );
  assert!(boundary["unsafe_paths"].as_array().is_some_and(|paths| paths.len() == 2));

  let operations = boundary["operations"].as_array().expect("operations should be an array");
  assert!(operations.iter().any(|item| item["classification"] == "browser"));
  assert!(operations.iter().any(|item| item["classification"] == "webgpu"));
  assert!(operations.iter().any(|item| item["classification"] == "npm-import"));
  assert!(operations.iter().any(|item| item["nullable_evidence"] == "optional-access"));

  assert!(boundary["helper_candidates"].as_array().is_some_and(|helpers| {
    helpers.iter().any(|helper| {
      helper["definition"] == "ffi.helpers/typed-query" && helper["origin"] == "dependency" && helper["compatibility"] == "exact-schema"
    })
  }));
  assert!(boundary["trait_candidates"].as_array().is_some_and(|candidates| {
    candidates.iter().any(|candidate| {
      candidate["fields"]
        .as_array()
        .is_some_and(|fields| fields.iter().any(|field| field == "value"))
        && candidate["methods"]
          .as_array()
          .is_some_and(|methods| methods.iter().any(|method| method == "focus"))
        && candidate["contract_status"] == "review-required"
    })
  }));
  assert!(boundary["adapter_candidates"].as_array().is_some_and(|candidates| {
    candidates
      .iter()
      .any(|candidate| candidate["source"] == "nanoid/default" && candidate["schema_cirru_edn"].is_string())
  }));
}

#[test]
fn ffi_evidence_supports_primary_edn_and_summary_only_output() {
  let edn = run(&["--ffi-evidence", "--format", "edn"]);
  assert_success(&edn);
  let parsed = cirru_edn::parse(String::from_utf8_lossy(&edn.stdout).as_ref()).expect("EDN stdout should parse");
  assert!(matches!(parsed, cirru_edn::Edn::Map(_)));

  let summary = run(&["--ffi-evidence", "--summary-only", "--format", "json"]);
  assert_success(&summary);
  let report: serde_json::Value = serde_json::from_slice(&summary.stdout).expect("summary stdout should parse");
  assert!(report["data"]["summary"]["ffi_boundaries"].as_u64().is_some_and(|count| count > 0));
  assert_eq!(report["data"]["evidence"]["ffi_boundaries"], serde_json::json!([]));
}

#[test]
fn strict_workflow_reuses_ffi_boundary_evidence() {
  let output = run_strict_fix(&["--format", "json"]);
  assert_success(&output);
  let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout should contain one JSON report");
  let boundaries = report["data"]["workflow"]["review_required"]["ffi_boundaries"]
    .as_array()
    .expect("strict workflow should contain FFI boundary evidence");
  let boundary = boundaries
    .iter()
    .find(|item| item["definition"] == "ffi-strict.main/query-host")
    .expect("strict workflow should reuse query-host evidence");
  assert_eq!(boundary["diagnostic_code"], "I_FFI_BOUNDARY_EVIDENCE");
  assert_eq!(boundary["classification"], "browser");
  assert_eq!(boundary["provenance"][3], "no-runtime-trust-inference");
  assert!(boundary["unsafe_paths"].as_array().is_some_and(|items| !items.is_empty()));
}
