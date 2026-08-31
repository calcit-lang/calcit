use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::Value;

const BOOTSTRAP: &str = "docs/run/calx-harness-bootstrap.json";
const ARCHIVED_REPORT: &str = "benchmarks/calx/20260831-macos-arm64.json";
const EXPECTED_CALCIT_COMMIT: &str = "88bb5a2250ba65b0e35c4d1809e6d49a14c61623";
const EXPECTED_CALCIT_VERSION: &str = "0.13.72";
const EXPECTED_CALX_VM_VERSION: &str = "0.3.0";

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
  assert_eq!(manifest["tracking"]["parent"], "calcit-lang/calcit#547");
  assert_eq!(manifest["tracking"]["contract"], "calcit-lang/calcit#557");
  assert_eq!(manifest["tracking"]["bootstrap"], "calcit-lang/calcit#558");
  assert_eq!(manifest["tracking"]["cutover"], "calcit-lang/calcit#559");

  let moved = asset_paths(&manifest["move"]);
  let copied = asset_paths(&manifest["copyWithProvenance"]);
  let stayed = manifest["stayInCore"]
    .as_array()
    .expect("stayInCore array")
    .iter()
    .map(|value| value.as_str().expect("stay path").to_owned())
    .collect::<BTreeSet<_>>();
  assert_eq!(
    moved,
    string_set(&[
      "src/bin/calx_bench.rs",
      "scripts/bench-calx-e2e.mjs",
      "scripts/bench-calx-settings.mjs",
      "scripts/bench-calx-settings.test.mjs",
      "docs/run/calx-benchmark.md",
      "benchmarks/calx/README.md",
      "benchmarks/calx/20260831-macos-arm64.json",
    ])
  );
  assert_eq!(
    stayed,
    string_set(&[
      "src/codegen/calx.rs",
      "src/codegen/calx/lowering.rs",
      "src/program/tests.rs",
      "tests/fixtures/calx/scalar-kernels.cirru",
      "tests/fixtures/calx/scalar-kernels.golden.txt",
      "tests/fixtures/calx/generated-program.golden.txt",
      "tests/fixtures/calx/fallback.cirru",
      "tests/fixtures/calx/fallback.golden.txt",
      "tests/fixtures/calx/typed-imports.cirru",
      "tests/fixtures/calx/trap.golden.txt",
    ])
  );
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
  assert_eq!(report["environment"]["platform"], "darwin");
  assert_eq!(report["environment"]["release"], "25.5.0");
  assert_eq!(report["environment"]["architecture"], "arm64");
  assert_eq!(report["environment"]["cpuModel"], "Apple M1 Pro");
  assert_eq!(report["environment"]["logicalCpuCount"], 8);
  assert_eq!(report["environment"]["totalMemoryBytes"], 17_179_869_184_u64);
  assert_eq!(report["environment"]["gitDirty"], false);
  assert_eq!(report["environment"]["gitCommit"], EXPECTED_CALCIT_COMMIT);
  assert_eq!(
    report["environment"]["rustc"],
    "rustc 1.97.1 (8bab26f4f 2026-07-14)\nbinary: rustc\ncommit-hash: 8bab26f4f68e0e26f0bb7960be334d5b520ea452\ncommit-date: 2026-07-14\nhost: aarch64-apple-darwin\nrelease: 1.97.1\nLLVM version: 22.1.6"
  );
  assert_eq!(report["environment"]["cargo"], "cargo 1.97.1 (c980f4866 2026-06-30)");
  assert_eq!(report["environment"]["node"], "v24.4.1");

  let profiles = report["profiles"].as_array().expect("profiles array");
  assert_eq!(profiles.len(), 2);
  assert_eq!(
    profiles
      .iter()
      .map(|profile| profile["profile"].as_str().expect("profile name"))
      .collect::<BTreeSet<_>>(),
    BTreeSet::from(["debug", "release"])
  );
  let mut raw_sample_count = 0;
  for profile in profiles {
    let profile_name = profile["profile"].as_str().expect("profile name");
    let cases = profile["cases"].as_array().expect("case array");
    assert_eq!(cases.len(), 13);
    for case in cases {
      let samples = case["rawSamples"].as_array().expect("rawSamples array");
      assert_eq!(samples.len(), 7);
      raw_sample_count += samples.len();
      for sample in samples {
        assert_eq!(sample["report"]["schema"], "calcit-calx-benchmark/2");
        assert_eq!(sample["report"]["correctness"], true);
        assert_eq!(sample["report"]["environment"]["packageVersion"], EXPECTED_CALCIT_VERSION);
        assert_eq!(sample["report"]["environment"]["calxVmVersion"], EXPECTED_CALX_VM_VERSION);
        assert_eq!(sample["report"]["environment"]["profile"], profile_name);
        assert_eq!(sample["report"]["environment"]["os"], "macos");
        assert_eq!(sample["report"]["environment"]["architecture"], "aarch64");
      }
    }
  }
  assert_eq!(raw_sample_count, 182);
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

fn string_set(values: &[&str]) -> BTreeSet<String> {
  values.iter().map(|value| (*value).to_owned()).collect()
}
