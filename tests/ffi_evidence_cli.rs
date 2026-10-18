use std::path::Path;
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
  let mut command = vec!["analyze", "weak-types"];
  command.extend_from_slice(args);
  run_calcit(Path::new("tests/fixtures/ffi-boundary-evidence.cirru"), &command)
}

fn run_calcit(snapshot: &Path, args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .arg(snapshot)
    .args(args)
    .output()
    .expect("calcit command should run")
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
fn ffi_evidence_uses_import_origin_without_classifying_calcit_calls_as_host_boundaries() {
  let pure = run(&["--ns", "ffi-evidence.pure", "--ffi-evidence", "--format", "json"]);
  assert_success(&pure);
  let pure: serde_json::Value = serde_json::from_slice(&pure.stdout).expect("pure Calcit evidence should parse");
  assert_eq!(pure["data"]["evidence"]["ffi_boundaries"], serde_json::json!([]));
  assert_eq!(pure["data"]["summary"]["ffi_boundaries"], 0);

  let mixed = run(&["--ffi-evidence", "--format", "json"]);
  assert_success(&mixed);
  let mixed: serde_json::Value = serde_json::from_slice(&mixed.stdout).expect("mixed import evidence should parse");
  let boundary = mixed["data"]["evidence"]["ffi_boundaries"]
    .as_array()
    .unwrap()
    .iter()
    .find(|boundary| boundary["definition"] == "ffi-evidence.main/mixed-imports")
    .expect("real Node import must retain its boundary");
  assert_eq!(boundary["classification"], "node");
  let operations = boundary["operations"].as_array().unwrap();
  assert_eq!(operations.len(), 2, "retain executable unquotes, not quoted or Calcit calls");
  assert_eq!(operations[0]["source"], "fs/existsSync");
  assert_eq!(operations[0]["classification"], "node");
  assert_eq!(operations[1]["source"], "existsSync");
  assert_eq!(operations[1]["member"], "existsSync");
  assert_eq!(operations[1]["classification"], "node");
  let adapters = boundary["adapter_candidates"].as_array().unwrap();
  assert_eq!(adapters.len(), 1);
  assert_eq!(adapters[0]["source"], "node:fs/existsSync");
  assert_eq!(adapters[0]["contract_status"], "review-required");
}

#[test]
fn ffi_evidence_coalesces_import_advice_per_selected_namespace() {
  let directory = tempfile::tempdir().expect("temporary fixture directory");
  let snapshot = directory.path().join("calcit.cirru");
  std::fs::copy("tests/fixtures/ffi-boundary-evidence.cirru", &snapshot).expect("copy canonical fixture");
  let canonical = run_calcit(&snapshot, &["analyze", "weak-types", "--ffi-evidence", "--format", "json"]);
  assert_success(&canonical);
  assert!(!String::from_utf8_lossy(&canonical.stderr).contains("redundant `[]` in :require"));
  let canonical: serde_json::Value = serde_json::from_slice(&canonical.stdout).unwrap();

  let namespaces = ["ffi-evidence.main", "ffi-evidence.pure"];
  let mut operations = vec![];
  for namespace in namespaces {
    let queried = run_calcit(&snapshot, &["query", "ns", namespace, "--format", "json"]);
    assert_success(&queried);
    let queried: serde_json::Value = serde_json::from_slice(&queried.stdout).unwrap();
    assert!(queried["data"]["definition_count"].as_u64().unwrap() > 1);
    let mut rules = queried["data"]["ns_code"][2].as_array().unwrap()[1..].to_vec();
    // Deliberately exercise accepted legacy syntax only in this temporary compatibility fixture.
    rules[0].as_array_mut().unwrap().insert(0, "[]".into());
    rules.last_mut().unwrap()[2].as_array_mut().unwrap().insert(0, "[]".into());
    rules.insert(0, "[]".into());
    operations.push(serde_json::json!([
      "edit",
      "imports",
      namespace,
      "--input-format",
      "json-ast",
      "--code",
      serde_json::to_string(&rules).unwrap()
    ]));
  }
  let operations = serde_json::to_string(&operations).unwrap();
  let preview = run_calcit(
    &snapshot,
    &["edit", "transaction", "--code", &operations, "--dry-run", "--format", "json"],
  );
  assert_success(&preview);
  let preview: serde_json::Value = serde_json::from_slice(&preview.stdout).unwrap();
  let applied = run_calcit(
    &snapshot,
    &[
      "edit",
      "transaction",
      "--code",
      &operations,
      "--expect-revision",
      preview["scoped_revision"].as_str().unwrap(),
      "--format",
      "json",
    ],
  );
  assert_success(&applied);

  for scope in [
    vec![],
    vec!["--ns", namespaces[0]],
    vec!["--ns", namespaces[1]],
    vec!["--ns-prefix", "ffi-evidence."],
    vec!["--ns-prefix", "ffi-evidence.ma"],
  ] {
    for format in ["edn", "json"] {
      let mut args = vec!["analyze", "weak-types", "--ffi-evidence", "--format", format];
      args.extend_from_slice(&scope);
      let output = run_calcit(&snapshot, &args);
      assert_success(&output);
      let stderr = String::from_utf8_lossy(&output.stderr);
      for (namespace, last_rule) in namespaces.into_iter().zip([4, 2]) {
        let expected = format!(
          "[Warn] in namespace '{namespace}': redundant `[]` in :require (rule 1 prefix, rule {last_rule} :refer list); import rules and :refer names are already syntax lists; write `respo.core :refer $ render! clear-cache!` or `respo.cursor :as cursor` without `[]`. Legacy markers remain accepted; use `edit imports` or `edit add-import --overwrite` to remove them"
        );
        let selected = match scope.first() {
          Some(&"--ns") => scope[1] == namespace,
          Some(&"--ns-prefix") => namespace.starts_with(scope[1]),
          _ => true,
        };
        assert_eq!(
          stderr.lines().filter(|line| *line == expected).count(),
          usize::from(selected),
          "{scope:?}: {stderr}"
        );
      }
      if format == "edn" {
        assert!(matches!(
          cirru_edn::parse(String::from_utf8_lossy(&output.stdout).as_ref()).unwrap(),
          cirru_edn::Edn::Map(_)
        ));
      } else {
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("one JSON report");
        if scope.is_empty() || scope == ["--ns-prefix", "ffi-evidence."] {
          assert_eq!(report["data"]["evidence"], canonical["data"]["evidence"]);
        } else if scope[1] == namespaces[1] {
          assert_eq!(report["data"]["evidence"]["ffi_boundaries"], serde_json::json!([]));
        }
      }
    }
  }
}

#[test]
fn ffi_evidence_rejects_malformed_imports() {
  let directory = tempfile::tempdir().expect("temporary malformed fixture directory");
  let snapshot = directory.path().join("calcit.cirru");
  let data = cirru_edn::parse(&std::fs::read_to_string("tests/fixtures/ffi-boundary-evidence.cirru").unwrap()).unwrap();
  let mut fixture = calcit::snapshot::load_snapshot_data(&data, snapshot.to_str().unwrap()).unwrap();
  // Invalid imports cannot be authored through the validated mutation CLI; construct this parser boundary directly.
  fixture.files.get_mut("ffi-evidence.main").unwrap().ns.code =
    cirru_parser::parse("ns ffi-evidence.main\n  :require\n    |node:fs :invalid fs")
      .unwrap()
      .remove(0);
  calcit::snapshot::save_snapshot_to_file(&snapshot, &fixture).expect("serialize malformed import fixture");
  let output = run_calcit(
    &snapshot,
    &[
      "analyze",
      "weak-types",
      "--ns",
      "ffi-evidence.main",
      "--ffi-evidence",
      "--format",
      "json",
    ],
  );
  assert!(!output.status.success(), "malformed imports must not become empty evidence");
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(stderr.contains("in namespace 'ffi-evidence.main':"));
  assert!(stderr.contains("unknown import rule kind `:invalid`"));
}

#[test]
fn strict_workflow_reuses_ffi_boundary_evidence() {
  let output = run_strict_fix(&["--format", "json"]);
  assert!(!output.status.success(), "an opaque host return must not prove String");
  let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout should contain one JSON report");
  assert_eq!(report["data"]["workflow"]["status"], "requires-review");
  assert!(
    report["diagnostics"]
      .as_array()
      .unwrap()
      .iter()
      .any(|item| item["code"] == "W_FN_RETURN_TYPE_MISMATCH")
  );
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

  // Only external-object traits are trusted host handle targets for `unsafe-coerce`.
  let return_mismatch = |definition: &str| {
    report["diagnostics"]
      .as_array()
      .unwrap()
      .iter()
      .any(|item| item["code"] == "W_FN_RETURN_TYPE_MISMATCH" && item["definition"] == definition)
  };
  assert!(
    return_mismatch("ffi-strict.main/as-label"),
    "a host value coerced to an ordinary Calcit trait still needs proof"
  );
  assert!(
    !return_mismatch("ffi-strict.main/as-node"),
    "a coercion to an external-object trait stays a retained host boundary"
  );
}

#[test]
fn schema_evidence_honors_dependency_scope_for_targets_and_owners() {
  let project_only = run(&["--schema-evidence", "--format", "json"]);
  assert_success(&project_only);
  let project_report: serde_json::Value =
    serde_json::from_slice(&project_only.stdout).expect("project-only schema evidence should parse");
  assert!(
    project_report["data"]["evidence"]["schema_candidates"]
      .as_array()
      .is_some_and(|candidates| candidates
        .iter()
        .all(|candidate| candidate["definition"] != "ffi.helpers/dependency-number"))
  );

  let with_dependencies = run(&["--schema-evidence", "--deps", "--format", "json"]);
  assert_success(&with_dependencies);
  let dependency_report: serde_json::Value =
    serde_json::from_slice(&with_dependencies.stdout).expect("dependency schema evidence should parse");
  assert!(
    dependency_report["data"]["evidence"]["schema_candidates"]
      .as_array()
      .is_some_and(|candidates| candidates
        .iter()
        .any(|candidate| { candidate["definition"] == "ffi.helpers/dependency-number" && candidate["confidence"] == "exact" }))
  );
}
