use serde_json::Value;

const PROFILE: &str = include_str!("../benchmarks/calx/20260831-compile-profile-macos-arm64.json");

#[test]
fn compile_profile_preserves_clean_provenance_and_complete_kernel_evidence() {
  let report: Value = serde_json::from_str(PROFILE).expect("parse Calx compile profile");
  assert_eq!(report["schema"], "calcit-calx-compile-profile-suite/1");
  assert_eq!(report["environment"]["gitDirty"], false);
  assert_eq!(report["environment"]["gitCommit"].as_str().map(str::len), Some(40));
  assert_eq!(report["kernels"].as_array().map(Vec::len), Some(5));
  assert!(report["measurement"]["samplySamples"].as_u64().is_some_and(|value| value > 10_000));
  assert!(
    report["measurement"]["samplyAllocationStackSamples"]
      .as_u64()
      .is_some_and(|value| value > 0)
  );

  for kernel in report["kernels"].as_array().expect("kernel profile array") {
    let stages = &kernel["stageTimingPerIterationNs"];
    let construction = stages["programConstruction"].as_u64().expect("program construction duration");
    assert!(construction > stages["eligibility"].as_u64().expect("eligibility duration"));
    assert!(construction > stages["planning"].as_u64().expect("planning duration"));
    assert!(construction > stages["validationLowering"].as_u64().expect("validation duration"));
    assert!(
      kernel["allocationsPerIteration"]["allocationCalls"]
        .as_u64()
        .is_some_and(|value| value > 0)
    );
    assert!(
      kernel["allocationsPerIteration"]["requestedBytes"]
        .as_u64()
        .is_some_and(|value| value > 0)
    );
  }
}

#[test]
fn compile_profile_keeps_raw_artifacts_external_but_content_addressed() {
  let report: Value = serde_json::from_str(PROFILE).expect("parse Calx compile profile");
  let raw = &report["rawProfile"];
  assert_eq!(raw["committed"], false);
  assert!(raw["profilePath"].as_str().is_some_and(|value| value.starts_with("target/")));
  assert!(raw["symbolsPath"].as_str().is_some_and(|value| value.starts_with("target/")));
  assert_eq!(raw["profileSha256"].as_str().map(str::len), Some(64));
  assert_eq!(raw["symbolsSha256"].as_str().map(str::len), Some(64));
  assert_eq!(report["interpretation"]["stackRatiosAreInclusiveAndOverlap"], true);
  assert_eq!(report["interpretation"]["requestedBytesArePeakResidentMemory"], false);
}
