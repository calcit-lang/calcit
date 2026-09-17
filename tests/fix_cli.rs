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

fn run_fix_with_entry(snapshot: &Path, entry: &str, args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .arg("--entry")
    .arg(entry)
    .arg(snapshot)
    .arg("fix")
    .args(args)
    .output()
    .expect("entry-scoped fix command should run")
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

fn run_calcit_with_emit_path(snapshot: &Path, emit_path: &Path, args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .arg("--emit-path")
    .arg(emit_path)
    .arg(snapshot)
    .args(args)
    .output()
    .expect("calcit command should run with an isolated emit path")
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

fn assert_success(output: &Output, context: &str) {
  assert!(
    output.status.success(),
    "{context} failed:\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
}

#[test]
fn strict_workflow_composes_a_resumable_project_manifest() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  let preview = run_fix(&snapshot, &["--workflow", "strict", "--format", "json"]);
  assert_success(&preview, "strict workflow plan");
  let report = parse_stdout(&preview);
  let workflow = &report["data"]["workflow"];
  assert_eq!(workflow["workflow"], "strict-v1");
  assert_eq!(workflow["mode"], "preview");
  assert_eq!(workflow["status"], "planned");
  assert_eq!(workflow["safe_fixes"]["preset"], "surface-latest-v2");
  assert!(workflow["safe_fixes"]["suggestions"].as_u64().is_some_and(|count| count > 0));
  assert_eq!(workflow["entries"][0]["name"], "default");
  assert_eq!(workflow["verification"]["commands"][0][0], "calcit");
  assert_eq!(workflow["verification"]["external_commands"], serde_json::json!([]));
  assert!(
    workflow["review_required"]["type_findings"]
      .as_array()
      .expect("review-required type findings should be an array")
      .iter()
      .all(|finding| matches!(
        finding["intent"].as_str(),
        Some("unresolved" | "declared-optional" | "explicit-unsafe")
      ))
  );
  assert!(
    workflow["retained_type_boundaries"]
      .as_array()
      .expect("retained type boundaries should be an array")
      .iter()
      .all(|finding| matches!(
        finding["intent"].as_str(),
        Some("intentional-js-ffi" | "intentional-type-slot-dynamic")
      ))
  );
  assert_eq!(workflow["resume"]["revision"], report["revision"]);
  assert_eq!(workflow["resume"]["apply_command"][3], "--workflow");
  assert_eq!(workflow["resume"]["apply_command"][4], "strict");

  let conflict = run_fix(
    &snapshot,
    &["--workflow", "strict", "--preset", "surface-latest-v2", "--format", "json"],
  );
  assert!(!conflict.status.success());
  assert!(String::from_utf8_lossy(&conflict.stderr).contains("project-scoped"));

  let unbound_apply = run_fix(
    &snapshot,
    &["--workflow", "strict", "--apply", "--allow-no-vcs", "--format", "json"],
  );
  assert!(!unbound_apply.status.success());
  assert!(String::from_utf8_lossy(&unbound_apply.stderr).contains("requires `--expect-revision`"));
}

#[test]
fn strict_workflow_applies_safe_fixes_and_verifies_the_result() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  let preview = run_fix(&snapshot, &["--workflow", "strict", "--format", "json"]);
  assert_success(&preview, "strict workflow plan");
  let preview_report = parse_stdout(&preview);
  let revision = preview_report["revision"].as_str().expect("workflow revision should be text");

  let applied = run_fix(
    &snapshot,
    &[
      "--workflow",
      "strict",
      "--apply",
      "--expect-revision",
      revision,
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "strict workflow apply");
  let applied_report = parse_stdout(&applied);
  assert_eq!(applied_report["data"]["workflow"]["status"], "applied");
  assert_eq!(applied_report["data"]["workflow"]["safe_fixes"]["status"], "applied");

  let verification = run_fix(&snapshot, &["--workflow", "strict", "--verify", "--format", "json"]);
  assert_success(&verification, "strict workflow verification");
  let verification_report = parse_stdout(&verification);
  assert_eq!(verification_report["data"]["workflow"]["status"], "passed");
  assert_eq!(verification_report["data"]["workflow"]["safe_fixes"]["status"], "clear");
  assert!(
    verification_report["data"]["workflow"]["verification"]["results"]
      .as_array()
      .is_some_and(|results| !results.is_empty() && results.iter().all(|result| result["status"] == "passed"))
  );
}

#[test]
fn staged_fix_validation_preserves_the_selected_browser_entry() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  let fixture = fs::read_to_string("tests/fixtures/fix-command.cirru").expect("fixture should read");
  let fixture = fixture.replace(":entries $ {} $ :default\n    {}", ":entries $ {}\n    :default $ {}");
  let fixture = fixture.replace(
    "      :type-slots $ {}\n  :files $ {} $ 'fix-command.main",
    "      :type-slots $ {}\n    :browser $ {} (:description |Browser) (:init-fn 'fix-command.main/main!) (:mode :js) (:reload-fn 'fix-command.main/reload!) (:target :browser)\n      :feature-policy $ {} (:js-ffi :error)\n      :modules $ []\n      :type-slots $ {}\n  :files $ {} $ 'fix-command.main",
  );
  let fixture = fixture.replace(
    "        'main! $ %{} 'CodeEntry",
    "        'BrowserElement $ %{} 'CodeEntry (:doc |)\n          :code $ quote $ deftrait BrowserElement\n            .focus! $ :: 'Fn $ {} (:args $ [] 'fix-command.main/BrowserElement) (:return 'Unit)\n          :examples $ []\n          :ffi $ {} (:backend :js) (:kind :external-object) (:target :browser)\n            :names $ {} (:focus! |focus)\n          :schema $ :: 'Trait\n        'browser-focus! $ %{} 'CodeEntry (:doc |)\n          :code $ quote $ defn browser-focus! (element)\n            do (element .focus!) &unit\n          :examples $ []\n          :ffi $ {} (:backend :js) (:target :browser)\n          :schema $ :: 'Fn $ {} (:return 'Unit)\n            :args $ [] 'fix-command.main/BrowserElement\n            :features $ #{} :js-ffi\n        'main! $ %{} 'CodeEntry",
  );
  fs::write(&snapshot, fixture).expect("entry fixture should write");

  let preview = run_fix_with_entry(
    &snapshot,
    "browser",
    &[
      "--preset",
      "surface-latest-v2",
      "--ns",
      "fix-command.main",
      "--def",
      "browser-focus!",
      "--format",
      "json",
    ],
  );

  assert_success(&preview, "browser-entry fix preview");
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["changed"], true);
  assert_eq!(report["data"]["validation"]["status"], "passed");
  assert_eq!(report["data"]["suggestions"][0]["rule_id"], "redundant-do-v1");
}

#[test]
fn semantic_rename_updates_only_compiler_resolved_usages_atomically() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/rename-old",
        "--code",
        "quote $ defn rename-old (x) + x 1",
      ],
      "create rename source",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/rename-old",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Number) (:return 'Number)",
      ],
      "type rename source",
    ),
    (vec!["edit", "add-ns", "fix-command.other"], "create consumer namespace"),
    (
      vec![
        "edit",
        "imports",
        "fix-command.other",
        "--code",
        "quote $ [] (fix-command.main :refer $ [] rename-old) (fix-command.main :as m)",
      ],
      "create consumer imports",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.other/caller",
        "--code",
        "quote $ defn caller () [] (rename-old 1) (m/rename-old 2)",
      ],
      "create consumer",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.other/caller",
        "--code",
        "quote $ :: 'Fn $ {} (:return $ :: 'List 'Number)",
      ],
      "type consumer",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/rename-shadow",
        "--code",
        "quote $ defn rename-shadow (rename-old) , rename-old",
      ],
      "create shadowed local",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/rename-shadow",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return 'Dynamic)",
      ],
      "type shadowed local",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }

  let preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "rename-definition-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "rename-old",
      "--to",
      "rename-new",
      "--format",
      "json",
    ],
  );
  assert_success(&preview, "semantic rename preview");
  let preview_report = parse_stdout(&preview);
  assert_eq!(preview_report["data"]["changed"], true);
  assert_eq!(preview_report["data"]["validation"]["checked_operations"], 4);
  assert_eq!(preview_report["data"]["suggestions"].as_array().map(Vec::len), Some(4));

  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "rename-definition-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "rename-old",
      "--to",
      "rename-new",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "semantic rename apply");
  let updated = fs::read_to_string(&snapshot).expect("updated snapshot should read");
  assert!(updated.contains("'rename-new $ %{} 'CodeEntry"));
  assert!(updated.contains("defn rename-new (x) + x 1"));
  assert!(updated.contains("fix-command.main/rename-new 1"));
  assert!(updated.contains("m/rename-new 2"));
  assert!(updated.contains("defn rename-shadow (rename-old) rename-old"));
  assert!(!updated.contains("fix-command.main :refer"));
  assert_success(&run_calcit(&snapshot, &["--check-only"]), "strict check after semantic rename");

  let repeated = run_fix(
    &snapshot,
    &[
      "--rule",
      "rename-definition-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "rename-old",
      "--to",
      "rename-new",
      "--format",
      "json",
    ],
  );
  assert_success(&repeated, "semantic rename repeat");
  assert_eq!(parse_stdout(&repeated)["data"]["changed"], false);
}

#[test]
fn semantic_rename_updates_attached_tests_and_examples_atomically() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/rename-old",
        "--code",
        "quote $ defn rename-old (x) + x 1",
      ],
      "create rename source",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/rename-old",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Number) (:return 'Number)",
      ],
      "type rename source",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/caller",
        "--code",
        "quote $ defn caller () rename-old 1",
      ],
      "create caller",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/caller",
        "--code",
        "quote $ :: 'Fn $ {} (:return 'Number)",
      ],
      "type caller",
    ),
    (
      vec![
        "edit",
        "add-test",
        "fix-command.main/caller",
        "calls-old",
        "--tags",
        "fast,semantic",
        "--code",
        "quote $ = (rename-old 1) 2",
      ],
      "attach target-using test",
    ),
    (
      vec![
        "edit",
        "add-example",
        "fix-command.main/caller",
        "--code",
        "quote $ = (rename-old 2) 3",
      ],
      "attach target-using example",
    ),
    (
      vec![
        "edit",
        "add-test",
        "fix-command.main/caller",
        "shadows-old",
        "--code",
        "quote $ let\n    rename-old 1\n  = rename-old 1",
      ],
      "attach locally shadowed test",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }
  let preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "rename-definition-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "rename-old",
      "--to",
      "rename-new",
      "--format",
      "json",
    ],
  );
  assert_success(&preview, "attached-source semantic rename preview");
  let preview_report = parse_stdout(&preview);
  assert_eq!(preview_report["data"]["validation"]["checked_operations"], 4);
  let paths = preview_report["data"]["suggestions"]
    .as_array()
    .expect("suggestions should be an array")
    .iter()
    .filter_map(|suggestion| suggestion["path"].as_str())
    .collect::<Vec<_>>();
  assert!(paths.contains(&"tests.calls-old"), "paths: {paths:?}");
  assert!(paths.contains(&"examples"), "paths: {paths:?}");

  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "rename-definition-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "rename-old",
      "--to",
      "rename-new",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "attached-source semantic rename apply");
  let updated = fs::read_to_string(&snapshot).expect("updated snapshot should read");
  assert!(updated.contains("fix-command.main/rename-new 1"), "snapshot:\n{updated}");
  assert!(updated.contains("fix-command.main/rename-new 2"), "snapshot:\n{updated}");
  assert!(updated.contains("= rename-old 1"), "snapshot:\n{updated}");
  assert!(updated.contains(":tags $ #{} :fast :semantic"), "snapshot:\n{updated}");
  assert_success(
    &run_calcit(&snapshot, &["test", "fix-command.main/caller", "--require-match"]),
    "renamed attached test",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &["analyze", "check-examples", "--ns", "fix-command.main", "--def", "caller"],
    ),
    "renamed attached example",
  );
}

#[test]
fn semantic_rename_rejects_quoted_attached_source_without_partial_writes() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/rename-old",
        "--code",
        "quote $ defn rename-old (x) + x 1",
      ],
      "create quoted-boundary rename source",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/rename-old",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Number) (:return 'Number)",
      ],
      "type quoted-boundary rename source",
    ),
    (
      vec![
        "edit",
        "add-test",
        "fix-command.main/rename-old",
        "quoted-old-name",
        "--code",
        "quote $ quote rename-old",
      ],
      "attach quoted target name",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }
  let before = fs::read(&snapshot).expect("snapshot should read before rejected rename");
  let rejected = run_fix(
    &snapshot,
    &[
      "--rule",
      "rename-definition-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "rename-old",
      "--to",
      "rename-new",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert!(!rejected.status.success());
  let stderr = String::from_utf8_lossy(&rejected.stderr);
  assert!(stderr.contains("quoted-old-name"), "stderr: {stderr}");
  assert!(stderr.contains("quoted source contains"), "stderr: {stderr}");
  assert_eq!(fs::read(&snapshot).expect("rejected snapshot should read"), before);
}

#[test]
fn semantic_rename_updates_schema_type_refs_atomically() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/RenameType",
        "--code",
        "quote $ defstruct RenameType",
      ],
      "create rename type",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/typed-value",
        "--code",
        "quote $ defn typed-value (value) , 1",
      ],
      "create typed value",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/typed-value",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'RenameType) (:return 'Number)",
      ],
      "reference rename type from schema",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }

  let preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "rename-definition-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "RenameType",
      "--to",
      "RenamedType",
      "--format",
      "json",
    ],
  );
  assert_success(&preview, "schema semantic rename preview");
  let preview_report = parse_stdout(&preview);
  assert_eq!(preview_report["data"]["validation"]["checked_operations"], 2);
  let schema_suggestion = preview_report["data"]["suggestions"]
    .as_array()
    .and_then(|suggestions| suggestions.iter().find(|suggestion| suggestion["path"] == "schema"))
    .expect("schema rewrite should be included");
  assert_eq!(schema_suggestion["origin_chain"][0]["kind"], "resolved-schema-type");
  assert_eq!(schema_suggestion["origin_chain"][0]["occurrences"], 1);

  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "rename-definition-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "RenameType",
      "--to",
      "RenamedType",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "schema semantic rename apply");
  let updated = fs::read_to_string(&snapshot).expect("updated snapshot should read");
  assert!(updated.contains("defstruct RenamedType"), "snapshot:\n{updated}");
  assert!(updated.contains("fix-command.main/RenamedType"), "snapshot:\n{updated}");
  assert!(!updated.contains("'RenameType"), "snapshot:\n{updated}");
  assert_success(&run_calcit(&snapshot, &["--check-only"]), "strict check after schema rename");
}

#[test]
fn semantic_rename_updates_schema_trait_bounds_atomically() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/RenameTrait",
        "--code",
        "quote $ deftrait RenameTrait (.show :fn)",
      ],
      "create rename trait",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/trait-bound-value",
        "--code",
        "quote $ defn trait-bound-value (value) value",
      ],
      "create trait-bound value",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/trait-bound-value",
        "--code",
        "quote $ :: 'Fn $ {} (:generics $ [] 'T) (:args $ [] 'T) (:where $ {} ('T 'RenameTrait)) (:return 'T)",
      ],
      "reference rename trait from schema bound",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }

  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "rename-definition-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "RenameTrait",
      "--to",
      "RenamedTrait",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "trait-bound semantic rename apply");
  let applied_report = parse_stdout(&applied);
  assert_eq!(applied_report["data"]["validation"]["checked_operations"], 2);
  let updated = fs::read_to_string(&snapshot).expect("updated snapshot should read");
  assert!(updated.contains("deftrait RenamedTrait"), "snapshot:\n{updated}");
  assert!(updated.contains("'T 'RenamedTrait"), "snapshot:\n{updated}");
  assert!(!updated.contains("'T 'RenameTrait"), "snapshot:\n{updated}");
  assert_success(&run_calcit(&snapshot, &["--check-only"]), "strict check after trait-bound rename");
}

#[test]
fn value_to_zero_arg_fn_updates_resolved_reads_atomically() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/deferred-number",
        "--code",
        "quote $ def deferred-number 41",
      ],
      "create value definition",
    ),
    (
      vec!["edit", "schema", "fix-command.main/deferred-number", "--code", "quote 'Number"],
      "type value definition",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/main!",
        "--code",
        "quote $ defn main! () deferred-number",
        "--overwrite",
      ],
      "make the native and JavaScript entry read the value",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/main!",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)",
      ],
      "type the value-reading entry",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/read-deferred-number",
        "--code",
        "quote $ defn read-deferred-number () + deferred-number 1",
      ],
      "create value reader",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/read-deferred-number",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)",
      ],
      "type value reader",
    ),
    (
      vec![
        "edit",
        "add-test",
        "fix-command.main/read-deferred-number",
        "reads-converted-value",
        "--code",
        "quote $ = deferred-number 41",
        "--tags",
        "semantic,fast",
      ],
      "attach direct value-read test",
    ),
    (
      vec![
        "edit",
        "examples",
        "fix-command.main/read-deferred-number",
        "--code",
        "quote $ + deferred-number 2",
      ],
      "attach direct value-read example",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }

  let preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "value-to-zero-arg-fn-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "deferred-number",
      "--format",
      "json",
    ],
  );
  assert_success(&preview, "value-to-function preview");
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["changed"], true);
  assert!(
    report["data"]["suggestions"]
      .as_array()
      .is_some_and(|items| items.iter().any(|item| item["origin_chain"][0]["kind"] == "resolved-value-read"))
  );

  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "value-to-zero-arg-fn-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "deferred-number",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "value-to-function apply");
  let updated = fs::read_to_string(&snapshot).expect("updated snapshot should read");
  assert!(updated.contains("defn deferred-number () 41"), "snapshot:\n{updated}");
  assert!(updated.contains("+ (deferred-number) 1"), "snapshot:\n{updated}");
  assert!(updated.contains(":tags $ #{} :fast :semantic"), "snapshot:\n{updated}");
  assert_success(
    &run_calcit(&snapshot, &["test", "fix-command.main/read-deferred-number", "--require-match"]),
    "converted attached test",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "analyze",
        "check-examples",
        "--ns",
        "fix-command.main",
        "--def",
        "read-deferred-number",
      ],
    ),
    "converted attached example",
  );
  assert_success(&run_calcit(&snapshot, &[]), "converted native entry");
  let js_output = directory.path().join("js-out");
  assert_success(
    &run_calcit_with_emit_path(&snapshot, &js_output, &["js"]),
    "converted JavaScript entry codegen",
  );
  assert!(js_output.join("fix-command.main.mjs").is_file());

  let repeated = run_fix(
    &snapshot,
    &[
      "--rule",
      "value-to-zero-arg-fn-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "deferred-number",
      "--format",
      "json",
    ],
  );
  assert_success(&repeated, "idempotent value-to-function preview");
  assert_eq!(parse_stdout(&repeated)["data"]["changed"], false);
}

#[test]
fn value_to_zero_arg_fn_rejects_quoted_references_without_writing() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/deferred-data",
        "--code",
        "quote $ def deferred-data 1",
      ],
      "create quoted-boundary value",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/quoted-deferred-data",
        "--code",
        "quote $ def quoted-deferred-data $ quote deferred-data",
      ],
      "create quoted target name",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }
  let before = fs::read(&snapshot).expect("snapshot should read before rejected refactor");
  let rejected = run_fix(
    &snapshot,
    &[
      "--rule",
      "value-to-zero-arg-fn-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "deferred-data",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert!(!rejected.status.success());
  assert!(String::from_utf8_lossy(&rejected.stderr).contains("quoted source contains"));
  assert_eq!(fs::read(&snapshot).expect("rejected snapshot should read"), before);
}

#[test]
fn value_to_zero_arg_fn_rejects_direct_macro_references_without_writing() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/deferred-syntax-value",
        "--code",
        "quote $ def deferred-syntax-value 1",
      ],
      "create macro-referenced value",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/read-deferred-syntax",
        "--code",
        "quote $ defmacro read-deferred-syntax () deferred-syntax-value",
      ],
      "create direct macro reference",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }
  let before = fs::read(&snapshot).expect("snapshot should read before rejected refactor");
  let rejected = run_fix(
    &snapshot,
    &[
      "--rule",
      "value-to-zero-arg-fn-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "deferred-syntax-value",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert!(!rejected.status.success());
  assert!(String::from_utf8_lossy(&rejected.stderr).contains("macro source may produce"));
  assert_eq!(fs::read(&snapshot).expect("rejected snapshot should read"), before);
}

#[test]
fn value_to_zero_arg_fn_wraps_reads_used_as_callees() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/stored-adder",
        "--code",
        "quote $ def stored-adder $ fn (x) + x 1",
      ],
      "create function-valued definition",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/stored-adder",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Number) (:return 'Number)",
      ],
      "type function-valued definition",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/call-stored-adder",
        "--code",
        "quote $ defn call-stored-adder () stored-adder 1",
      ],
      "create stored function caller",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/call-stored-adder",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)",
      ],
      "type stored function caller",
    ),
    (
      vec![
        "edit",
        "add-test",
        "fix-command.main/call-stored-adder",
        "calls-wrapped-callee",
        "--code",
        "quote $ = (call-stored-adder) 2",
      ],
      "attach caller behavior test",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }

  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "value-to-zero-arg-fn-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "stored-adder",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "callee-position value-to-function apply");
  let updated = fs::read_to_string(&snapshot).expect("updated snapshot should read");
  assert!(updated.contains("defn stored-adder ()"), "snapshot:\n{updated}");
  assert!(updated.contains("(stored-adder) 1"), "snapshot:\n{updated}");
  assert_success(
    &run_calcit(&snapshot, &["test", "fix-command.main/call-stored-adder", "--require-match"]),
    "wrapped callee behavior",
  );
}

#[test]
fn schema_synthesis_applies_exact_compiler_evidence_and_is_target_stable() {
  let directory = TestDirectory::create();
  let native_snapshot = directory.path().join("native.cirru");
  let js_snapshot = directory.path().join("js.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &native_snapshot).expect("fixture should copy");
  let native_source = fs::read_to_string(&native_snapshot).expect("native fixture should read");
  fs::write(&js_snapshot, native_source.replace("(:mode :native)", "(:mode :js)")).expect("JS fixture should write");

  for snapshot in [&native_snapshot, &js_snapshot] {
    assert_success(
      &run_calcit(
        snapshot,
        &[
          "edit",
          "def",
          "fix-command.main/inferred-number",
          "--code",
          "quote $ defn inferred-number () + 1 2",
        ],
      ),
      "create untyped zero-argument function",
    );
    assert_success(
      &run_calcit(
        snapshot,
        &[
          "edit",
          "def",
          "fix-command.main/inferred-add-one",
          "--code",
          "quote $ defn inferred-add-one (x) + x 1",
        ],
      ),
      "create untyped one-argument function",
    );
    assert_success(
      &run_calcit(
        snapshot,
        &[
          "edit",
          "def",
          "fix-command.main/use-inferred-add-one",
          "--code",
          "quote $ defn use-inferred-add-one () (inferred-add-one 2)",
        ],
      ),
      "create typed callsite",
    );
    assert_success(
      &run_calcit(
        snapshot,
        &[
          "edit",
          "schema",
          "fix-command.main/use-inferred-add-one",
          "--code",
          "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)",
        ],
      ),
      "type callsite owner",
    );
  }

  let args = [
    "--rule",
    "synthesize-schema-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "inferred-number",
    "--format",
    "json",
  ];
  let native_preview = run_fix(&native_snapshot, &args);
  let js_preview = run_fix(&js_snapshot, &args);
  assert_success(&native_preview, "native schema preview");
  assert_success(&js_preview, "JS schema preview");
  let native_report = parse_stdout(&native_preview);
  let js_report = parse_stdout(&js_preview);
  assert_eq!(
    native_report["data"]["suggestions"][0]["replacement"],
    js_report["data"]["suggestions"][0]["replacement"]
  );
  assert_eq!(native_report["data"]["suggestions"][0]["applicability"], "machine-applicable");

  let argument_args = [
    "--rule",
    "synthesize-schema-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "inferred-add-one",
    "--format",
    "json",
  ];
  let native_argument_preview = run_fix(&native_snapshot, &argument_args);
  let js_argument_preview = run_fix(&js_snapshot, &argument_args);
  assert_success(&native_argument_preview, "native argument schema preview");
  assert_success(&js_argument_preview, "JS argument schema preview");
  let native_argument_report = parse_stdout(&native_argument_preview);
  let js_argument_report = parse_stdout(&js_argument_preview);
  assert_eq!(
    native_argument_report["data"]["suggestions"][0]["replacement"],
    js_argument_report["data"]["suggestions"][0]["replacement"]
  );
  assert!(
    native_argument_report["data"]["suggestions"][0]["origin_chain"]
      .as_array()
      .expect("origin chain should be an array")
      .iter()
      .any(|evidence| evidence["kind"] == "resolved-callsite-arguments" && evidence["slot"] == "schema.args.0")
  );
  assert_eq!(
    native_argument_report["data"]["suggestions"][0]["replacement"]["value"],
    serde_json::json!(["::", "'Fn", ["{}", [":return", "'Number"], [":args", ["[]", "'Number"]]]])
  );

  let applied = run_fix(
    &native_snapshot,
    &[
      "--rule",
      "synthesize-schema-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "inferred-number",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "apply inferred schema");
  let argument_applied = run_fix(
    &native_snapshot,
    &[
      "--rule",
      "synthesize-schema-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "inferred-add-one",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert_success(&argument_applied, "apply callsite-backed schema");
  let updated = fs::read_to_string(&native_snapshot).expect("updated snapshot should read");
  assert!(updated.contains("(:return 'Number)"));
  assert!(updated.contains("defn inferred-add-one (x) + x 1"));
  assert_success(
    &run_calcit(&native_snapshot, &["--check-only"]),
    "strict check after schema synthesis",
  );

  let repeated = run_fix(&native_snapshot, &args);
  assert_success(&repeated, "repeat schema synthesis");
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"].as_array().map(Vec::len), Some(0));
}

#[test]
fn schema_evidence_reuses_schema_synthesis_and_reports_structural_candidates() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/evidence-number",
        "--code",
        "quote $ defn evidence-number () + 1 2",
      ],
      "create exact evidence target",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/evidence-number",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Dynamic)",
      ],
      "structure exact evidence schema",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/evidence-add-one",
        "--code",
        "quote $ defn evidence-add-one (x) + x 1",
      ],
      "create usage-derived evidence target",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/evidence-add-one",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return 'Dynamic)",
      ],
      "structure usage-derived evidence schema",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/use-evidence-add-one",
        "--code",
        "quote $ defn use-evidence-add-one () (evidence-add-one 2)",
      ],
      "create schema evidence callsite",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/use-evidence-add-one",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)",
      ],
      "type schema evidence callsite",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/conflicting-evidence",
        "--code",
        "quote $ defn conflicting-evidence (x) 1",
      ],
      "create conflicting evidence target",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/conflicting-evidence",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return 'Dynamic)",
      ],
      "structure conflicting evidence schema",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/use-conflicting-number",
        "--code",
        "quote $ defn use-conflicting-number () (conflicting-evidence 1)",
      ],
      "create numeric conflicting callsite",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/use-conflicting-number",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)",
      ],
      "type numeric conflicting callsite",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/use-conflicting-string",
        "--code",
        "quote $ defn use-conflicting-string () (conflicting-evidence |x)",
      ],
      "create string conflicting callsite",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/use-conflicting-string",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)",
      ],
      "type string conflicting callsite",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/person-a",
        "--code",
        "quote $ defn person-a () ({} (:name |Ada) (:age 1))",
      ],
      "create first repeated map shape",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/person-a",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return $ :: 'Map 'Tag 'Dynamic)",
      ],
      "type first repeated map shape",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/person-b",
        "--code",
        "quote $ defn person-b () ({} (:name |Bob) (:age 2))",
      ],
      "create second repeated map shape",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/person-b",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return $ :: 'Map 'Tag 'Dynamic)",
      ],
      "type second repeated map shape",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/summarize-evidence",
        "--code",
        "quote $ defn summarize-evidence (x)\n  match x\n    (:ok value) value\n    (:err message) message",
      ],
      "create dispatch evidence",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/summarize-evidence",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return 'Dynamic)",
      ],
      "structure dispatch evidence schema",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }

  let output = run_calcit(&snapshot, &["analyze", "weak-types", "--schema-evidence", "--format", "json"]);
  assert_success(&output, "schema evidence report");
  let report = parse_stdout(&output);
  assert_eq!(report["schema_version"], 8);
  assert_eq!(report["data"]["filters"]["schema_evidence"], true);
  let schemas = report["data"]["evidence"]["schema_candidates"]
    .as_array()
    .expect("schema candidates should be an array");
  assert!(
    schemas
      .iter()
      .any(|candidate| { candidate["definition"] == "fix-command.main/evidence-number" && candidate["confidence"] == "exact" })
  );
  assert!(schemas.iter().any(|candidate| {
    candidate["definition"] == "fix-command.main/conflicting-evidence"
      && candidate["confidence"] == "conflict"
      && candidate["evidence"]
        .as_array()
        .is_some_and(|evidence| evidence.iter().any(|item| item["kind"] == "conflicting-callsite-arguments"))
  }));
  assert!(schemas.iter().any(|candidate| {
    candidate["definition"] == "fix-command.main/evidence-add-one"
      && candidate["confidence"] == "usage-derived"
      && candidate["affected_usages"].as_array().is_some_and(|usages| {
        usages
          .iter()
          .any(|usage| usage.as_str().is_some_and(|path| path.contains("use-evidence-add-one")))
      })
  }));
  assert!(
    report["data"]["evidence"]["map_shapes"].as_array().is_some_and(|candidates| {
      candidates.iter().any(|candidate| {
        candidate["fields"]
          .as_array()
          .is_some_and(|fields| fields.iter().any(|field| field["name"] == ":name"))
          && candidate["evidence_paths"].as_array().is_some_and(|paths| paths.len() >= 2)
      })
    }),
    "map shape evidence: {}",
    report["data"]["evidence"]["map_shapes"]
  );
  assert!(
    report["data"]["evidence"]["dispatch"].as_array().is_some_and(|candidates| {
      candidates
        .iter()
        .any(|candidate| candidate["variants"] == serde_json::json!([":err", ":ok"]))
    }),
    "dispatch evidence: {}",
    report["data"]["evidence"]["dispatch"]
  );

  let summary = run_calcit(
    &snapshot,
    &["analyze", "weak-types", "--schema-evidence", "--summary-only", "--format", "json"],
  );
  assert_success(&summary, "schema evidence summary");
  let summary = parse_stdout(&summary);
  assert!(
    summary["data"]["summary"]["schema_candidates"]
      .as_u64()
      .is_some_and(|count| count > 0)
  );
  assert_eq!(summary["data"]["evidence"]["schema_candidates"], serde_json::json!([]));
  assert_eq!(summary["data"]["evidence"]["map_shapes"], serde_json::json!([]));
  assert_eq!(summary["data"]["evidence"]["dispatch"], serde_json::json!([]));

  let edn = run_calcit(
    &snapshot,
    &["analyze", "weak-types", "--schema-evidence", "--summary-only", "--format", "edn"],
  );
  assert_success(&edn, "schema evidence Cirru EDN summary");
  assert!(matches!(
    cirru_edn::parse(String::from_utf8_lossy(&edn.stdout).as_ref()),
    Ok(cirru_edn::Edn::Map(_))
  ));

  let workflow = run_fix(&snapshot, &["--workflow", "strict", "--format", "json"]);
  assert_success(&workflow, "strict workflow schema evidence");
  let workflow = parse_stdout(&workflow);
  assert!(
    workflow["data"]["workflow"]["review_required"]["schema_candidates"]
      .as_array()
      .is_some_and(|candidates| !candidates.is_empty())
  );
  assert!(
    workflow["data"]["workflow"]["review_required"]["structural_candidates"]["map_shapes"]
      .as_array()
      .is_some_and(|candidates| !candidates.is_empty())
  );
}

#[test]
fn schema_synthesis_does_not_treat_sample_namespaces_as_argument_proof() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/sample-observed",
        "--code",
        "quote $ defn sample-observed (x) 1",
      ],
      "create schema target",
    ),
    (vec!["edit", "add-ns", "fix-command.test"], "create test namespace"),
    (
      vec![
        "edit",
        "def",
        "fix-command.test/use-sample-observed",
        "--code",
        "quote $ defn use-sample-observed () (fix-command.main/sample-observed 2)",
      ],
      "create test-only callsite",
    ),
    (vec!["edit", "add-ns", "fix-command.examples"], "create example namespace"),
    (
      vec![
        "edit",
        "def",
        "fix-command.examples/show-sample-observed",
        "--code",
        "quote $ defn show-sample-observed () (fix-command.main/sample-observed 3)",
      ],
      "create example-only callsite",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }

  let preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "synthesize-schema-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "sample-observed",
      "--format",
      "json",
    ],
  );
  assert_success(&preview, "sample-only schema preview");
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["suggestions"][0]["applicability"], "needs-review");
  assert_eq!(
    report["data"]["suggestions"][0]["origin_chain"][0]["unresolved_slots"],
    serde_json::json!(["schema.args.0"])
  );
  assert!(
    report["data"]["suggestions"][0]["origin_chain"]
      .as_array()
      .expect("origin chain should be an array")
      .iter()
      .all(|evidence| evidence["kind"] != "resolved-callsite-arguments")
  );
}

#[test]
fn schema_synthesis_does_not_invent_a_wasm_import_return_type() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/imported-value",
        "--code",
        "quote $ defwasm-import imported-value (x) |host |read-value",
      ],
    ),
    "create untyped WASM import",
  );
  let preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "synthesize-schema-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "imported-value",
      "--format",
      "json",
    ],
  );
  assert!(!preview.status.success());
  assert!(
    String::from_utf8_lossy(&preview.stderr).contains("could not recover static implementation evidence"),
    "stderr:\n{}",
    String::from_utf8_lossy(&preview.stderr)
  );
}

#[test]
fn schema_synthesis_preserves_precise_ref_shape_and_partial_holes() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  let atom_preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "synthesize-schema-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "*fix-person-calls",
      "--format",
      "json",
    ],
  );
  assert_success(&atom_preview, "atom schema preview");
  let atom_report = parse_stdout(&atom_preview);
  assert_eq!(
    atom_report["data"]["suggestions"][0]["replacement"]["value"],
    serde_json::json!(["::", "'Ref", "'Number"])
  );

  let before = fs::read(&snapshot).expect("fixture bytes should read");
  let partial = run_fix(
    &snapshot,
    &[
      "--rule",
      "synthesize-schema-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "option-struct-field",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert_success(&partial, "partial schema preview");
  let partial_report = parse_stdout(&partial);
  assert_eq!(partial_report["data"]["changed"], false);
  assert_eq!(partial_report["data"]["suggestions"][0]["applicability"], "needs-review");
  assert_eq!(
    partial_report["data"]["suggestions"][0]["origin_chain"][0]["unresolved_slots"],
    serde_json::json!(["schema.return.type-args.0"])
  );
  assert_eq!(fs::read(&snapshot).expect("fixture bytes should reread"), before);

  for (target, code) in [
    ("fix-command.main/inferred-identity", "quote $ defn inferred-identity (x) 1"),
    (
      "fix-command.main/use-identity-number",
      "quote $ defn use-identity-number () (inferred-identity 1)",
    ),
    (
      "fix-command.main/use-identity-string",
      "quote $ defn use-identity-string () (inferred-identity |x)",
    ),
  ] {
    assert_success(
      &run_calcit(&snapshot, &["edit", "def", target, "--code", code]),
      "create conflicting callsite fixture",
    );
  }
  let conflicting = run_fix(
    &snapshot,
    &[
      "--rule",
      "synthesize-schema-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "inferred-identity",
      "--format",
      "json",
    ],
  );
  assert_success(&conflicting, "conflicting callsite schema preview");
  let conflicting_report = parse_stdout(&conflicting);
  assert_eq!(conflicting_report["data"]["changed"], false);
  assert_eq!(conflicting_report["data"]["suggestions"][0]["applicability"], "needs-review");
  assert_eq!(
    conflicting_report["data"]["suggestions"][0]["origin_chain"][0]["unresolved_slots"],
    serde_json::json!(["schema.args.0"])
  );

  for (target, code) in [
    ("fix-command.main/macro-observed", "quote $ defn macro-observed (x) 1"),
    (
      "fix-command.main/use-macro-observed",
      "quote $ defn use-macro-observed () (macro-observed 2)",
    ),
    (
      "fix-command.main/expand-macro-observed",
      "quote $ defmacro expand-macro-observed () macro-observed 3",
    ),
  ] {
    assert_success(
      &run_calcit(&snapshot, &["edit", "def", target, "--code", code]),
      "create macro-boundary schema fixture",
    );
  }
  let macro_boundary = run_fix(
    &snapshot,
    &[
      "--rule",
      "synthesize-schema-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "macro-observed",
      "--format",
      "json",
    ],
  );
  assert_success(&macro_boundary, "macro-boundary schema preview");
  let macro_report = parse_stdout(&macro_boundary);
  assert_eq!(macro_report["data"]["suggestions"][0]["applicability"], "needs-review");
  assert_eq!(
    macro_report["data"]["suggestions"][0]["origin_chain"][0]["unresolved_slots"],
    serde_json::json!(["schema.args.0"])
  );

  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/attached-observed",
        "--code",
        "quote $ defn attached-observed (x) 1",
      ],
      "create attached-source schema target",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/use-attached-observed",
        "--code",
        "quote $ defn use-attached-observed () (attached-observed 2)",
      ],
      "create ordinary attached-source callsite",
    ),
    (
      vec![
        "edit",
        "add-test",
        "fix-command.main/use-attached-observed",
        "rejects-string",
        "--code",
        "quote $ = (attached-observed |x) 1",
      ],
      "create conflicting attached test",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }
  let attached_conflict = run_fix(
    &snapshot,
    &[
      "--rule",
      "synthesize-schema-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "attached-observed",
      "--format",
      "json",
    ],
  );
  assert!(!attached_conflict.status.success());
  assert!(String::from_utf8_lossy(&attached_conflict.stderr).contains("test `rejects-string`"));
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
