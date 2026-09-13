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
fn tag_match_fix_uses_resolved_source_origin_and_preserves_semantics() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let original = fs::read(&snapshot).expect("fixture should read");

  let before = run_calcit(
    &snapshot,
    &["test", "fix-command.main/tag-match-case", "--summary-only", "--require-match"],
  );
  assert!(before.status.success(), "stderr:\n{}", String::from_utf8_lossy(&before.stderr));
  let quoted_before = run_calcit(
    &snapshot,
    &["test", "fix-command.main/tag-match-quoted", "--summary-only", "--require-match"],
  );
  assert!(
    quoted_before.status.success(),
    "stderr:\n{}",
    String::from_utf8_lossy(&quoted_before.stderr)
  );
  let shadow_tests = run_calcit(&snapshot, &["query", "tests", "fix-command.main/tag-match-shadowed"]);
  assert!(
    shadow_tests.status.success(),
    "stderr:\n{}",
    String::from_utf8_lossy(&shadow_tests.stderr)
  );
  assert!(String::from_utf8_lossy(&shadow_tests.stdout).contains("keeps-local-shadow"));
  let preview = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "tag-match-case",
      "--rule",
      "tag-match-to-match-v1",
      "--format",
      "json",
    ],
  );
  assert!(preview.status.success(), "stderr:\n{}", String::from_utf8_lossy(&preview.stderr));
  let preview_json = parse_stdout(&preview);
  let suggestion = &preview_json["data"]["suggestions"][0];
  assert_eq!(suggestion["rule_id"], "tag-match-to-match-v1");
  assert_eq!(suggestion["diagnostic_code"], "W_DEPRECATED_API");
  assert_eq!(suggestion["origin_chain"][0]["target"], "calcit.core/tag-match");
  assert_eq!(suggestion["original"]["value"], "tag-match");
  assert_eq!(suggestion["replacement"]["value"], "match");
  assert!(
    suggestion["message"]
      .as_str()
      .is_some_and(|message| message.contains("Calcit tests"))
  );
  assert_eq!(preview_json["data"]["validation"]["checked_operations"], 1);
  assert_eq!(fs::read(&snapshot).expect("preview fixture should read"), original);

  for definition in ["tag-match-shadowed", "tag-match-quoted"] {
    let negative = run_fix(
      &snapshot,
      &[
        "--ns",
        "fix-command.main",
        "--def",
        definition,
        "--rule",
        "tag-match-to-match-v1",
        "--format",
        "json",
      ],
    );
    assert!(
      negative.status.success(),
      "{definition} stderr:\n{}",
      String::from_utf8_lossy(&negative.stderr)
    );
    let report = parse_stdout(&negative);
    assert_eq!(report["data"]["changed"], false, "definition: {definition}");
    assert_eq!(report["data"]["suggestions"].as_array().map(Vec::len), Some(0));
  }

  let revision = preview_json["revision"].as_str().expect("preview revision should be a string");
  let applied = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "tag-match-case",
      "--rule",
      "tag-match-to-match-v1",
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
  assert_eq!(applied_json["data"]["changed"], true);
  let updated = fs::read_to_string(&snapshot).expect("updated fixture should read");
  assert!(updated.contains("match value"));
  assert!(updated.contains("defn tag-match-shadowed (tag-match value) (tag-match value)"));
  assert!(updated.contains("quote $ tag-match (%some 1)"));

  let after = run_calcit(
    &snapshot,
    &["test", "fix-command.main/tag-match-case", "--summary-only", "--require-match"],
  );
  assert!(after.status.success(), "stderr:\n{}", String::from_utf8_lossy(&after.stderr));
  let quoted_after = run_calcit(
    &snapshot,
    &["test", "fix-command.main/tag-match-quoted", "--summary-only", "--require-match"],
  );
  assert!(
    quoted_after.status.success(),
    "stderr:\n{}",
    String::from_utf8_lossy(&quoted_after.stderr)
  );
  let repeated = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "tag-match-case",
      "--rule",
      "tag-match-to-match-v1",
      "--format",
      "json",
    ],
  );
  assert!(repeated.status.success(), "stderr:\n{}", String::from_utf8_lossy(&repeated.stderr));
  let repeated_json = parse_stdout(&repeated);
  assert_eq!(repeated_json["data"]["changed"], false);
  assert_eq!(repeated_json["data"]["suggestions"].as_array().map(Vec::len), Some(0));
}

#[test]
fn default_fix_applies_leaf_replacements_before_structural_splices() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  let preview = run_fix(
    &snapshot,
    &["--ns", "fix-command.main", "--def", "tag-match-case", "--format", "json"],
  );
  assert!(preview.status.success(), "stderr:\n{}", String::from_utf8_lossy(&preview.stderr));
  let preview_json = parse_stdout(&preview);
  let rule_ids = preview_json["data"]["suggestions"]
    .as_array()
    .expect("suggestions should be an array")
    .iter()
    .map(|suggestion| suggestion["rule_id"].as_str().expect("rule id should be a string"))
    .collect::<Vec<_>>();
  assert_eq!(rule_ids, vec!["tag-match-to-match-v1", "redundant-do-v1"]);
  assert_eq!(preview_json["data"]["validation"]["checked_operations"], 3);

  let revision = preview_json["revision"].as_str().expect("preview revision should be a string");
  let applied = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "tag-match-case",
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
  assert_eq!(applied_json["data"]["changed"], true);
  let updated = fs::read_to_string(&snapshot).expect("updated fixture should read");
  assert!(updated.contains("match value"));
  assert!(!updated.contains("do $ match value"));

  let after = run_calcit(
    &snapshot,
    &["test", "fix-command.main/tag-match-case", "--summary-only", "--require-match"],
  );
  assert!(after.status.success(), "stderr:\n{}", String::from_utf8_lossy(&after.stderr));
  let repeated = run_fix(
    &snapshot,
    &["--ns", "fix-command.main", "--def", "tag-match-case", "--format", "json"],
  );
  assert!(repeated.status.success(), "stderr:\n{}", String::from_utf8_lossy(&repeated.stderr));
  let repeated_json = parse_stdout(&repeated);
  assert_eq!(repeated_json["data"]["changed"], false);
  assert_eq!(repeated_json["data"]["suggestions"].as_array().map(Vec::len), Some(0));
}
