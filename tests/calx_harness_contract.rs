use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::Value;

const BOOTSTRAP: &str = "docs/run/calx-harness-bootstrap.json";
const ARCHIVED_REPORT: &str = "benchmarks/calx/20260831-macos-arm64.json";

#[test]
fn bootstrap_manifest_has_valid_tracking_ownership_and_assets() {
  let manifest = read_json(BOOTSTRAP);
  assert_eq!(manifest["schema"], "calcit-calx-harness-bootstrap/1");
  assert_eq!(manifest["status"], "experimental-benchmark");
  assert_eq!(manifest["targetRepository"]["confirmed"], false);
  assert_eq!(
    manifest["targetRepository"]["existingCalcitCalxRole"],
    "native-ffi-demo-do-not-absorb-harness"
  );
  for key in ["parent", "contract", "bootstrap", "cutover"] {
    assert!(manifest["tracking"][key].as_str().is_some_and(|value| value.contains('#')));
  }

  let moved = asset_paths(&manifest["move"]);
  let copied = asset_paths(&manifest["copyWithProvenance"]);
  let stayed = manifest["stayInCore"]
    .as_array()
    .expect("stayInCore array")
    .iter()
    .map(|value| value.as_str().expect("stay path").to_owned())
    .collect::<BTreeSet<_>>();
  for path in moved.iter().chain(copied.iter()).chain(stayed.iter()) {
    assert!(Path::new(path).exists(), "contract asset does not exist: {path}");
  }
  assert!(moved.is_disjoint(&stayed));
  assert_eq!(copied, BTreeSet::from(["tests/fixtures/calx/scalar-kernels.cirru".to_owned()]));
  assert!(stayed.contains("tests/fixtures/calx/scalar-kernels.cirru"));

  assert_eq!(manifest["migrationTests"]["rust"], 3);
  assert_eq!(manifest["migrationTests"]["node"], 4);
  assert_eq!(manifest["reportContract"]["preserveRawSamples"], true);
  assert_eq!(manifest["reportContract"]["absoluteCiThresholds"], false);
}

#[test]
fn archived_suite_is_a_traceable_schema_v2_raw_report() {
  let report = read_json(ARCHIVED_REPORT);
  assert_eq!(report["schema"], "calcit-calx-benchmark-suite/2");
  assert_eq!(report["environment"]["gitDirty"], false);
  assert!(non_empty_string(&report["environment"]["gitCommit"]));
  assert!(non_empty_string(&report["environment"]["rustc"]));
  assert!(non_empty_string(&report["environment"]["cargo"]));
  assert!(non_empty_string(&report["environment"]["node"]));

  let profiles = report["profiles"].as_array().expect("profiles array");
  assert_eq!(profiles.len(), 2);
  for profile in profiles {
    assert!(matches!(profile["profile"].as_str(), Some("debug" | "release")));
    for case in profile["cases"].as_array().expect("case array") {
      let samples = case["rawSamples"].as_array().expect("rawSamples array");
      assert!(!samples.is_empty());
      for sample in samples {
        assert_eq!(sample["report"]["schema"], "calcit-calx-benchmark/2");
        assert_eq!(sample["report"]["correctness"], true);
        assert!(non_empty_string(&sample["report"]["environment"]["calxVmVersion"]));
        assert!(non_empty_string(&sample["report"]["environment"]["packageVersion"]));
      }
    }
  }
}

fn read_json(path: &str) -> Value {
  serde_json::from_slice(&fs::read(path).unwrap_or_else(|error| panic!("read {path}: {error}")))
    .unwrap_or_else(|error| panic!("parse {path}: {error}"))
}

fn asset_paths(value: &Value) -> BTreeSet<String> {
  value
    .as_array()
    .expect("asset array")
    .iter()
    .map(|asset| asset["path"].as_str().expect("asset path").to_owned())
    .collect()
}

fn non_empty_string(value: &Value) -> bool {
  value.as_str().is_some_and(|value| !value.is_empty())
}
