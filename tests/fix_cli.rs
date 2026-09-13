use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEST_DIRECTORY_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
  fn create() -> Self {
    let nonce = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("test clock should be valid")
      .as_nanos();
    let counter = TEST_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("calcit-fix-cli-{}-{nonce}-{counter}", std::process::id()));
    fs::create_dir(&path).expect("temporary project should create");
    Self(path)
  }

  fn path(&self) -> &Path {
    &self.0
  }
}

impl Drop for TestDirectory {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

fn run_fix(snapshot: &Path, args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .arg(snapshot)
    .arg("fix")
    .args(args)
    .output()
    .expect("fix command should run")
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

fn parse_stdout(output: &Output) -> serde_json::Value {
  serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
    panic!(
      "fix stdout should contain one JSON value: {error}\nstdout:\n{}\nstderr:\n{}",
      String::from_utf8_lossy(&output.stdout),
      String::from_utf8_lossy(&output.stderr)
    )
  })
}

#[test]
fn fix_preview_apply_and_repeat_are_revision_safe() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let original = fs::read(&snapshot).expect("fixture should read");

  let human = run_fix(&snapshot, &["--ns", "fix-command.main", "--def", "fixable"]);
  assert!(human.status.success(), "stderr:\n{}", String::from_utf8_lossy(&human.stderr));
  let human_stdout = String::from_utf8_lossy(&human.stdout);
  assert!(human_stdout.starts_with("# Compiler-guided source fixes\n"));
  assert!(human_stdout.contains("## Suggestion 1\n"));
  assert!(human_stdout.contains("### Before\n\n- node kind: `leaf`\n\n```cirru\n"));
  assert!(human_stdout.contains("### After\n\n- node kind: `leaf`\n\n```cirru\n"));
  assert_eq!(fs::read(&snapshot).expect("human preview should not write"), original);

  let edn = run_fix(&snapshot, &["--ns", "fix-command.main", "--def", "fixable", "--format", "edn"]);
  assert!(edn.status.success(), "stderr:\n{}", String::from_utf8_lossy(&edn.stderr));
  let edn_value = cirru_edn::parse(&String::from_utf8_lossy(&edn.stdout)).expect("fix EDN should parse");
  let cirru_edn::Edn::Map(edn_map) = edn_value else {
    panic!("fix EDN should be a map");
  };
  assert_eq!(edn_map.get(&cirru_edn::Edn::tag("command")), Some(&cirru_edn::Edn::str("fix")));
  assert_eq!(fs::read(&snapshot).expect("EDN preview should not write"), original);

  let preview = run_fix(&snapshot, &["--ns", "fix-command.main", "--def", "fixable", "--format", "json"]);
  assert!(preview.status.success(), "stderr:\n{}", String::from_utf8_lossy(&preview.stderr));
  let preview_json = parse_stdout(&preview);
  assert_eq!(preview_json["command"], "fix");
  assert_eq!(preview_json["data"]["mode"], "preview");
  assert_eq!(preview_json["data"]["changed"], true);
  assert_eq!(preview_json["data"]["suggestions"][0]["rule_id"], "removed-data-api-v1");
  assert_eq!(
    preview_json["data"]["suggestions"][0]["source_file"],
    snapshot.to_string_lossy().as_ref()
  );
  assert_eq!(preview_json["data"]["suggestions"][0]["replacement"]["value"], "enum-definition");
  assert_eq!(preview_json["data"]["validation"]["status"], "passed");
  assert_eq!(preview_json["data"]["validation"]["staged_scope_preprocess"], true);
  assert_eq!(preview_json["data"]["validation"]["checked_operations"], 1);
  assert_eq!(fs::read(&snapshot).expect("preview fixture should read"), original);

  let revision = preview_json["revision"].as_str().expect("preview revision should be a string");
  let no_vcs = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "fixable",
      "--apply",
      "--expect-revision",
      revision,
      "--format",
      "json",
    ],
  );
  assert!(!no_vcs.status.success());
  assert!(String::from_utf8_lossy(&no_vcs.stderr).contains("--allow-no-vcs"));
  assert_eq!(fs::read(&snapshot).expect("no-vcs fixture should read"), original);

  let stale = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "fixable",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      "md5:stale",
      "--format",
      "json",
    ],
  );
  assert!(!stale.status.success());
  assert_eq!(fs::read(&snapshot).expect("stale fixture should read"), original);

  let applied = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "fixable",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      revision,
      "--format",
      "json",
    ],
  );
  assert!(applied.status.success(), "stderr:\n{}", String::from_utf8_lossy(&applied.stderr));
  let applied_json = parse_stdout(&applied);
  assert_eq!(applied_json["data"]["mode"], "apply");
  assert_eq!(applied_json["data"]["changed"], true);
  let updated = fs::read_to_string(&snapshot).expect("updated fixture should read");
  assert!(updated.contains("enum-definition value"));
  assert!(!updated.contains("defn fixable (value) (tuple-enum value)"));
  assert!(updated.contains("defn shadowed (tuple-enum value) (tuple-enum value)"));
  let calcit_test = run_calcit(&snapshot, &["test", "fix-command.main/fixable", "--require-match"]);
  assert!(
    calcit_test.status.success(),
    "Calcit definition test failed:\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&calcit_test.stdout),
    String::from_utf8_lossy(&calcit_test.stderr)
  );

  let repeated = run_fix(&snapshot, &["--ns", "fix-command.main", "--def", "fixable", "--format", "json"]);
  assert!(repeated.status.success(), "stderr:\n{}", String::from_utf8_lossy(&repeated.stderr));
  let repeated_json = parse_stdout(&repeated);
  assert_eq!(repeated_json["data"]["changed"], false);
  assert_eq!(repeated_json["data"]["validation"]["status"], "not-needed");
  assert_eq!(repeated_json["data"]["validation"]["staged_scope_preprocess"], false);
  assert_eq!(repeated_json["data"]["validation"]["checked_operations"], 0);
  assert_eq!(repeated_json["data"]["suggestions"].as_array().map(Vec::len), Some(0));
}

#[test]
fn ambiguous_removed_predicate_is_reported_without_a_replacement() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  let preview = run_fix(&snapshot, &["--ns", "fix-command.main", "--def", "ambiguous", "--format", "json"]);
  assert!(preview.status.success(), "stderr:\n{}", String::from_utf8_lossy(&preview.stderr));
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["changed"], false);
  assert_eq!(report["data"]["suggestions"][0]["applicability"], "requires-review");
  assert!(report["data"]["suggestions"][0]["replacement"].is_null());
}

#[test]
fn local_binding_with_a_removed_core_name_is_not_rewritten() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  let preview = run_fix(&snapshot, &["--ns", "fix-command.main", "--def", "shadowed", "--format", "json"]);
  assert!(preview.status.success(), "stderr:\n{}", String::from_utf8_lossy(&preview.stderr));
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["changed"], false);
  assert_eq!(report["data"]["suggestions"].as_array().map(Vec::len), Some(0));
}

#[test]
fn staged_fix_rejects_a_new_compiler_warning_without_writing() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let incompatible_schema = run_calcit(
    &snapshot,
    &[
      "edit",
      "schema",
      "fix-command.main/fixable",
      "--code",
      "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return $ :: 'Option 'Tag)",
    ],
  );
  assert!(
    incompatible_schema.status.success(),
    "schema setup failed: {}",
    String::from_utf8_lossy(&incompatible_schema.stderr)
  );
  let before = fs::read(&snapshot).expect("fixture should read before rejected preview");

  let preview = run_fix(&snapshot, &["--ns", "fix-command.main", "--def", "fixable", "--format", "json"]);
  assert!(!preview.status.success());
  assert!(String::from_utf8_lossy(&preview.stderr).contains("W_FN_RETURN_TYPE_MISMATCH"));
  assert_eq!(fs::read(&snapshot).expect("rejected fixture should read"), before);
}

#[test]
fn retired_surface_rules_point_to_the_published_migration_bridge() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  for rule in ["tag-match-to-match-v1", "required-struct-field-v1"] {
    let output = run_fix(&snapshot, &["--rule", rule, "--format", "json"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Calcit 0.14.15"), "stderr: {stderr}");
    assert!(stderr.contains("before upgrading"), "stderr: {stderr}");
  }
}

#[test]
fn surface_latest_preset_migrates_named_enum_and_struct_constructors() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  let enum_preview = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "legacy-enum",
      "--preset",
      "surface-latest-v1",
      "--format",
      "json",
    ],
  );
  assert!(
    enum_preview.status.success(),
    "stderr:\n{}",
    String::from_utf8_lossy(&enum_preview.stderr)
  );
  let report = parse_stdout(&enum_preview);
  assert_eq!(report["data"]["filters"]["preset_id"], "surface-latest-v1");
  assert_eq!(report["data"]["filters"]["expanded_rule_ids"].as_array().map(Vec::len), Some(4));
  assert_eq!(report["data"]["filters"]["expanded_rules"].as_array().map(Vec::len), Some(4));
  assert!(
    report["data"]["filters"]["expanded_rules"]
      .as_array()
      .expect("expanded rule metadata should be an array")
      .iter()
      .all(|rule| rule["lifecycle"] == "current-semantics" && rule["source_version_required"] == false)
  );
  assert_eq!(
    report["data"]["filters"]["expanded_rules"][0]["evidence_source"],
    "current-diagnostic"
  );
  assert_eq!(report["data"]["suggestions"][0]["rule_id"], "named-enum-constructor-v1");

  let enum_applied = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "legacy-enum",
      "--preset",
      "surface-latest-v1",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert!(
    enum_applied.status.success(),
    "stderr:\n{}",
    String::from_utf8_lossy(&enum_applied.stderr)
  );

  let struct_preview = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "legacy-struct",
      "--preset",
      "surface-latest-v1",
      "--format",
      "json",
    ],
  );
  assert!(
    struct_preview.status.success(),
    "stderr:\n{}",
    String::from_utf8_lossy(&struct_preview.stderr)
  );
  assert_eq!(
    parse_stdout(&struct_preview)["data"]["suggestions"][0]["rule_id"],
    "named-struct-constructor-v1"
  );
  let struct_applied = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "legacy-struct",
      "--preset",
      "surface-latest-v1",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert!(
    struct_applied.status.success(),
    "stderr:\n{}",
    String::from_utf8_lossy(&struct_applied.stderr)
  );

  let nested_applied = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "legacy-nested",
      "--preset",
      "surface-latest-v1",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert!(
    nested_applied.status.success(),
    "stderr:\n{}",
    String::from_utf8_lossy(&nested_applied.stderr)
  );
  for definition in ["legacy-enum-overlap", "legacy-struct-overlap"] {
    let applied = run_fix(
      &snapshot,
      &[
        "--ns",
        "fix-command.main",
        "--def",
        definition,
        "--preset",
        "surface-latest-v1",
        "--apply",
        "--allow-no-vcs",
        "--format",
        "json",
      ],
    );
    assert!(
      applied.status.success(),
      "overlap migration failed for {definition}:\n{}",
      String::from_utf8_lossy(&applied.stderr)
    );
  }
  let updated = fs::read_to_string(&snapshot).expect("updated fixture should read");
  assert!(updated.contains("defn legacy-enum () (FixPersonChoice :none)"));
  assert!(updated.contains("defn legacy-struct () (FixPerson :name |Ada :age 1)"));
  assert!(updated.contains("FixPersonChoice :person $ FixPerson :name |Ada :age 1"));

  for definition in [
    "legacy-enum",
    "legacy-struct",
    "legacy-nested",
    "legacy-enum-overlap",
    "legacy-struct-overlap",
  ] {
    let target = format!("fix-command.main/{definition}");
    let calcit_test = run_calcit(&snapshot, &["test", target.as_str(), "--require-match"]);
    assert!(
      calcit_test.status.success(),
      "Calcit definition test failed for {definition}:\nstdout:\n{}\nstderr:\n{}",
      String::from_utf8_lossy(&calcit_test.stdout),
      String::from_utf8_lossy(&calcit_test.stderr)
    );
  }

  for definition in [
    "legacy-enum",
    "legacy-struct",
    "legacy-nested",
    "legacy-enum-overlap",
    "legacy-struct-overlap",
  ] {
    let repeated = run_fix(
      &snapshot,
      &[
        "--ns",
        "fix-command.main",
        "--def",
        definition,
        "--preset",
        "surface-latest-v1",
        "--format",
        "json",
      ],
    );
    assert!(repeated.status.success(), "stderr:\n{}", String::from_utf8_lossy(&repeated.stderr));
    assert_eq!(parse_stdout(&repeated)["data"]["changed"], false);
  }
}

#[test]
fn surface_latest_v2_unwraps_single_expression_do_without_changing_v1() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  let v1_preview = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "single-do-positions",
      "--preset",
      "surface-latest-v1",
      "--format",
      "json",
    ],
  );
  assert!(
    v1_preview.status.success(),
    "stderr:\n{}",
    String::from_utf8_lossy(&v1_preview.stderr)
  );
  let v1_report = parse_stdout(&v1_preview);
  assert_eq!(v1_report["data"]["changed"], false);
  assert_eq!(
    v1_report["data"]["filters"]["expanded_rule_ids"],
    serde_json::json!([
      "removed-data-api-v1",
      "named-enum-constructor-v1",
      "named-struct-constructor-v1",
      "redundant-do-v1"
    ])
  );

  let preview = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "single-do-positions",
      "--preset",
      "surface-latest-v2",
      "--format",
      "json",
    ],
  );
  assert!(preview.status.success(), "stderr:\n{}", String::from_utf8_lossy(&preview.stderr));
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["filters"]["preset_id"], "surface-latest-v2");
  assert_eq!(
    report["data"]["filters"]["expanded_rule_ids"],
    serde_json::json!([
      "removed-data-api-v1",
      "named-enum-constructor-v1",
      "named-struct-constructor-v1",
      "redundant-do-v1",
      "single-expression-do-v1"
    ])
  );
  assert_eq!(report["data"]["suggestions"].as_array().map(Vec::len), Some(4));
  assert!(
    report["data"]["suggestions"]
      .as_array()
      .expect("suggestions should be an array")
      .iter()
      .all(|suggestion| suggestion["rule_id"] == "single-expression-do-v1")
  );

  let applied = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "single-do-positions",
      "--preset",
      "surface-latest-v2",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert!(applied.status.success(), "stderr:\n{}", String::from_utf8_lossy(&applied.stderr));
  let updated = fs::read_to_string(&snapshot).expect("updated fixture should read");
  assert!(updated.contains("'single-do-positions"));
  assert!(updated.contains("chosen value"));
  assert!(updated.contains("if true (+ chosen 1) 0"));

  let calcit_test = run_calcit(&snapshot, &["test", "fix-command.main/single-do-positions", "--require-match"]);
  assert!(
    calcit_test.status.success(),
    "Calcit definition test failed:\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&calcit_test.stdout),
    String::from_utf8_lossy(&calcit_test.stderr)
  );

  let repeated = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "single-do-positions",
      "--preset",
      "surface-latest-v2",
      "--format",
      "json",
    ],
  );
  assert!(repeated.status.success(), "stderr:\n{}", String::from_utf8_lossy(&repeated.stderr));
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"].as_array().map(Vec::len), Some(0));

  let overlap = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "legacy-enum-overlap",
      "--preset",
      "surface-latest-v2",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert!(overlap.status.success(), "stderr:\n{}", String::from_utf8_lossy(&overlap.stderr));
  let overlap_test = run_calcit(&snapshot, &["test", "fix-command.main/legacy-enum-overlap", "--require-match"]);
  assert!(
    overlap_test.status.success(),
    "Calcit overlap test failed:\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&overlap_test.stdout),
    String::from_utf8_lossy(&overlap_test.stderr)
  );

  for definition in ["quoted-single-do", "single-do-macro"] {
    let preview = run_fix(
      &snapshot,
      &[
        "--ns",
        "fix-command.main",
        "--def",
        definition,
        "--preset",
        "surface-latest-v2",
        "--format",
        "json",
      ],
    );
    assert!(preview.status.success(), "stderr:\n{}", String::from_utf8_lossy(&preview.stderr));
    assert_eq!(parse_stdout(&preview)["data"]["suggestions"].as_array().map(Vec::len), Some(0));
    let target = format!("fix-command.main/{definition}");
    let calcit_test = run_calcit(&snapshot, &["test", target.as_str(), "--require-match"]);
    assert!(
      calcit_test.status.success(),
      "Calcit definition test failed for {definition}:\nstdout:\n{}\nstderr:\n{}",
      String::from_utf8_lossy(&calcit_test.stdout),
      String::from_utf8_lossy(&calcit_test.stderr)
    );
  }
}

#[test]
fn preset_and_rule_selection_conflict() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let output = run_fix(
    &snapshot,
    &["--preset", "surface-latest-v1", "--rule", "redundant-do-v1", "--format", "json"],
  );
  assert!(!output.status.success());
  assert!(String::from_utf8_lossy(&output.stderr).contains("conflicts with `--preset`"));
}

#[test]
fn named_constructor_preset_preserves_quoted_data() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let preview = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "quoted-legacy-constructor",
      "--preset",
      "surface-latest-v1",
      "--format",
      "json",
    ],
  );
  assert!(preview.status.success(), "stderr:\n{}", String::from_utf8_lossy(&preview.stderr));
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["changed"], false);
  assert_eq!(report["data"]["suggestions"].as_array().map(Vec::len), Some(0));

  let target = "fix-command.main/quoted-legacy-constructor";
  let calcit_test = run_calcit(&snapshot, &["test", target, "--require-match"]);
  assert!(
    calcit_test.status.success(),
    "Calcit definition test failed:\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&calcit_test.stdout),
    String::from_utf8_lossy(&calcit_test.stderr)
  );
}
