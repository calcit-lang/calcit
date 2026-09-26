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
    let path = std::env::temp_dir().join(format!("calcit-edit-cli-{}-{nonce}-{counter}", std::process::id()));
    fs::create_dir(&path).expect("temporary project should create");
    Self(path)
  }

  fn snapshot(&self) -> PathBuf {
    self.0.join("calcit.cirru")
  }
}

impl Drop for TestDirectory {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
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

fn assert_success(output: &Output, context: &str) {
  assert!(
    output.status.success(),
    "{context} failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
}

fn query_definition(snapshot: &Path, target: &str) -> serde_json::Value {
  let output = run_calcit(snapshot, &["query", "def", target, "--format", "json"]);
  assert_success(&output, "query def");
  serde_json::from_slice(&output.stdout).expect("query def stdout should contain one JSON value")
}

#[test]
fn schema_clear_and_structural_edits_preserve_missing_intent() {
  let directory = TestDirectory::create();
  let snapshot = prepare_minimal_snapshot(&directory);
  let target = "app.main/helper";
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn helper (x) x",
      ],
    ),
    "create unannotated helper",
  );
  let read_entry = || {
    let source = fs::read_to_string(&snapshot).unwrap();
    let data = cirru_edn::parse(&source).unwrap();
    let parsed = calcit::snapshot::load_snapshot_data(&data, snapshot.to_str().unwrap()).unwrap();
    parsed.files["app.main"].defs["helper"].clone()
  };
  let missing = read_entry();
  assert!(calcit::snapshot::schema_annotation_is_missing(&missing.schema));

  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "schema", target, "--input-format", "cirru", "--code", "quote $ :: 'Dynamic"],
    ),
    "explicit Dynamic remains a declaration",
  );
  assert_success(&run_calcit(&snapshot, &["edit", "format"]), "format explicit schema");
  let explicit = read_entry();
  assert!(!calcit::snapshot::schema_annotation_is_missing(&explicit.schema));
  assert_ne!(
    calcit::snapshot::definition_revision(&missing).unwrap(),
    calcit::snapshot::definition_revision(&explicit).unwrap()
  );

  assert_success(&run_calcit(&snapshot, &["edit", "schema", target, "--clear"]), "remove schema");
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "app.main/unrelated",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn unrelated () 2",
      ],
    ),
    "edit unrelated definition",
  );
  assert_success(&run_calcit(&snapshot, &["edit", "format"]), "format missing schema");
  let cleared = read_entry();
  assert!(calcit::snapshot::schema_annotation_is_missing(&cleared.schema));
  assert_eq!(
    calcit::snapshot::definition_revision(&missing).unwrap(),
    calcit::snapshot::definition_revision(&cleared).unwrap()
  );
  let failure = run_calcit(&snapshot, &["--init-fn", target, "--check-only"]);
  assert!(!failure.status.success(), "clearing must not bypass strict validation");
  assert!(String::from_utf8_lossy(&failure.stderr).contains("has no declared function schema"));
}

#[test]
fn schema_feature_edit_preserves_contract_and_works_in_guarded_transaction() {
  let directory = TestDirectory::create();
  let snapshot = prepare_minimal_snapshot(&directory);
  let target = "app.main/main!";
  let before = query_definition(&snapshot, target);
  let original = fs::read(&snapshot).expect("snapshot should read");
  let operation = "[] $ [] |edit |schema |app.main/main! |--add-feature |js-ffi";

  let stale = run_calcit(
    &snapshot,
    &[
      "edit",
      "transaction",
      "--code",
      operation,
      "--expect-revision",
      "md5:stale",
      "--format",
      "json",
    ],
  );
  assert!(!stale.status.success());
  assert!(String::from_utf8_lossy(&stale.stderr).contains("Snapshot revision mismatch"));
  assert_eq!(fs::read(&snapshot).expect("snapshot should remain"), original);

  let preview = run_calcit(
    &snapshot,
    &["edit", "transaction", "--code", operation, "--dry-run", "--format", "json"],
  );
  assert_success(&preview, "preview schema feature change");
  let preview_value: serde_json::Value = serde_json::from_slice(&preview.stdout).expect("preview JSON should parse");
  assert_eq!(preview_value["changed"], true);
  assert_eq!(fs::read(&snapshot).expect("preview must not write"), original);

  let original_revision = preview_value["original_revision"].as_str().expect("transaction revision");
  let apply = run_calcit(
    &snapshot,
    &[
      "edit",
      "transaction",
      "--code",
      operation,
      "--expect-revision",
      original_revision,
      "--format",
      "json",
    ],
  );
  assert_success(&apply, "apply schema feature change");
  let after = query_definition(&snapshot, target);
  assert_eq!(after["data"]["code"], before["data"]["code"]);
  assert_eq!(after["data"]["doc"], before["data"]["doc"]);
  assert_eq!(after["data"]["ffi"], before["data"]["ffi"]);
  let schema = after["data"]["schema"].to_string();
  assert!(schema.contains(":return") && schema.contains("'Number"), "schema: {schema}");
  assert!(schema.contains(":features"), "schema: {schema}");
  assert!(schema.contains(":js-ffi"), "schema: {schema}");

  let first_apply = fs::read(&snapshot).expect("updated snapshot should read");
  let repeat = run_calcit(&snapshot, &["edit", "schema", target, "--add-feature", "js-ffi"]);
  assert_success(&repeat, "repeat feature add should be idempotent");
  assert_eq!(fs::read(&snapshot).expect("idempotent snapshot should read"), first_apply);

  let unsupported = run_calcit(&snapshot, &["edit", "schema", target, "--add-feature", "unknown"]);
  assert!(!unsupported.status.success());
  assert!(String::from_utf8_lossy(&unsupported.stderr).contains("Unsupported schema feature"));
  assert_eq!(fs::read(&snapshot).expect("rejected snapshot should remain"), first_apply);
}

#[test]
fn ffi_metadata_edit_commits_with_schema_and_rolls_back_on_failure() {
  let directory = TestDirectory::create();
  let snapshot = prepare_minimal_snapshot(&directory);
  let target = "app.main/main!";
  let original = fs::read(&snapshot).expect("snapshot should read");
  let operations = serde_json::json!([
    ["edit", "schema", target, "--add-feature", "js-ffi"],
    ["edit", "ffi", target, "--code", "{} (:backend :js) (:target :browser)"]
  ])
  .to_string();

  let preview = run_calcit(
    &snapshot,
    &["edit", "transaction", "--code", &operations, "--dry-run", "--format", "json"],
  );
  assert_success(&preview, "preview schema and FFI metadata together");
  let preview_value: serde_json::Value = serde_json::from_slice(&preview.stdout).expect("preview JSON should parse");
  assert_eq!(preview_value["changed"], true);
  assert_eq!(fs::read(&snapshot).expect("preview should not write"), original);

  let stale = run_calcit(
    &snapshot,
    &["edit", "transaction", "--code", &operations, "--expect-revision", "md5:stale"],
  );
  assert!(!stale.status.success());
  assert!(String::from_utf8_lossy(&stale.stderr).contains("Snapshot revision mismatch"));
  assert_eq!(fs::read(&snapshot).expect("stale transaction should not write"), original);

  let rollback_operations = serde_json::json!([
    ["edit", "ffi", target, "--code", "{} (:backend :js)"],
    ["edit", "schema", target, "--code", "not-a-schema"]
  ])
  .to_string();
  let rollback = run_calcit(&snapshot, &["edit", "transaction", "--code", &rollback_operations]);
  assert!(!rollback.status.success());
  assert!(String::from_utf8_lossy(&rollback.stderr).contains("Transaction operation 2 failed"));
  assert_eq!(fs::read(&snapshot).expect("failed transaction should not write"), original);

  let invalid_metadata = serde_json::json!([["edit", "ffi", target, "--code", "[]"]]).to_string();
  let invalid = run_calcit(&snapshot, &["edit", "transaction", "--code", &invalid_metadata]);
  assert!(!invalid.status.success());
  assert!(String::from_utf8_lossy(&invalid.stderr).contains("FFI metadata must be an EDN map"));
  assert_eq!(fs::read(&snapshot).expect("invalid FFI should not write"), original);

  let revision = preview_value["original_revision"].as_str().expect("preview revision");
  let apply = run_calcit(
    &snapshot,
    &["edit", "transaction", "--code", &operations, "--expect-revision", revision],
  );
  assert_success(&apply, "apply schema and FFI metadata together");
  let after = query_definition(&snapshot, target);
  assert!(after["data"]["schema"].to_string().contains(":js-ffi"));
  assert!(after["data"]["ffi"].to_string().contains(":backend"));
}

fn prepare_minimal_snapshot(directory: &TestDirectory) -> PathBuf {
  let snapshot = directory.snapshot();
  fs::copy("calcit/add.cirru", &snapshot).expect("minimal snapshot fixture should copy");
  let schema = run_calcit(
    &snapshot,
    &[
      "edit",
      "schema",
      "app.main/main!",
      "--code",
      "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)",
    ],
  );
  assert_success(&schema, "make minimal entry schema strict");
  let reload = run_calcit(
    &snapshot,
    &["edit", "def", "app.main/reload!", "--code", "quote $ defn reload! () nil"],
  );
  assert_success(&reload, "create minimal reload definition");
  let reload_schema = run_calcit(
    &snapshot,
    &[
      "edit",
      "schema",
      "app.main/reload!",
      "--code",
      "quote $ :: 'Fn $ {} (:args $ []) (:return 'Nil)",
    ],
  );
  assert_success(&reload_schema, "make minimal reload schema strict");
  snapshot
}

fn create_macro(snapshot: &Path, name: &str, code: &str) {
  let target = format!("app.main/{name}");
  let output = run_calcit(snapshot, &["edit", "def", &target, "--code", code]);
  assert_success(&output, "edit defmacro");
}

#[test]
fn explicit_syntax_input_formats_preserve_ambiguous_node_shapes() {
  let directory = TestDirectory::create();
  let snapshot = prepare_minimal_snapshot(&directory);

  for (input_format, code, expected_kind) in [
    ("json-ast", r#""[]""#, "leaf"),
    ("json-ast", "[]", "empty-list"),
    ("json-ast", r#"["inc","1"]"#, "expression"),
    ("cirru", "quote $ []", "expression"),
  ] {
    let output = run_calcit(
      &snapshot,
      &[
        "edit",
        "add-example",
        "app.main/main!",
        "--input-format",
        input_format,
        "--code",
        code,
      ],
    );
    assert_success(&output, "add explicitly decoded example");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&format!("- input format: `{input_format}`")), "stdout:\n{stdout}");
    assert!(stdout.contains(&format!("- node kind: `{expected_kind}`")), "stdout:\n{stdout}");
    assert!(stdout.contains("## Canonical Cirru syntax\n"), "stdout:\n{stdout}");
    assert!(stdout.contains("```cirru\n"), "stdout:\n{stdout}");
    if input_format == "json-ast" {
      assert!(stdout.contains("## Canonical JSON AST\n"), "stdout:\n{stdout}");
      assert!(stdout.contains("```json\n"), "stdout:\n{stdout}");
    } else {
      assert!(!stdout.contains("## Canonical JSON AST\n"), "stdout:\n{stdout}");
    }
  }

  let definition = query_definition(&snapshot, "app.main/main!");
  assert_eq!(definition["data"]["examples"], serde_json::json!(["[]", [], ["inc", "1"], ["[]"]]));

  let invalid = run_calcit(
    &snapshot,
    &[
      "edit",
      "add-example",
      "app.main/main!",
      "--input-format",
      "json-ast",
      "--code",
      r#"{"node":[]}"#,
    ],
  );
  assert!(!invalid.status.success(), "object-shaped JSON AST must fail");
  let stderr = String::from_utf8_lossy(&invalid.stderr);
  assert!(stderr.contains("format `json-ast`"), "stderr:\n{stderr}");
  assert!(stderr.contains("expected node kind `string leaf or array`"), "stderr:\n{stderr}");
  assert!(stderr.contains("received node kind `object`"), "stderr:\n{stderr}");
}

#[test]
fn bulk_imports_use_explicit_syntax_transport_and_keep_legacy_auto_compatibility() {
  let directory = TestDirectory::create();
  let snapshot = prepare_minimal_snapshot(&directory);

  let explicit = run_calcit(
    &snapshot,
    &[
      "edit",
      "imports",
      "app.main",
      "--input-format",
      "json-ast",
      "--code",
      r#"["[]",["calcit.core",":refer",["inc"]]]"#,
    ],
  );
  assert_success(&explicit, "explicit JSON AST imports");
  let stdout = String::from_utf8_lossy(&explicit.stdout);
  assert!(stdout.contains("- input format: `json-ast`"), "stdout:\n{stdout}");
  assert!(stdout.contains("Updated imports for namespace 'app.main'"), "stdout:\n{stdout}");

  let legacy = run_calcit(
    &snapshot,
    &["edit", "imports", "app.main", "--code", r#"[["calcit.core",":refer",["dec"]]]"#],
  );
  assert_success(&legacy, "legacy auto imports");

  let invalid_explicit = run_calcit(
    &snapshot,
    &[
      "edit",
      "imports",
      "app.main",
      "--input-format",
      "json-ast",
      "--code",
      r#"[["calcit.core",":refer",["inc"]]]"#,
    ],
  );
  assert!(!invalid_explicit.status.success(), "legacy shape should fail in explicit mode");
  let stderr = String::from_utf8_lossy(&invalid_explicit.stderr);
  assert!(
    stderr.contains("expected an imports vector node headed by `[]`"),
    "stderr:\n{stderr}"
  );
}

#[test]
fn structural_code_and_ffi_metadata_report_distinct_expected_inputs() {
  let directory = TestDirectory::create();
  let snapshot = prepare_minimal_snapshot(&directory);

  let ffi_with_code = run_calcit(&snapshot, &["edit", "ffi", "app.main/main!", "--code", "quote $ []"]);
  assert!(
    !ffi_with_code.status.success(),
    "quoted code should not be accepted as FFI metadata"
  );
  let ffi_error = String::from_utf8_lossy(&ffi_with_code.stderr);
  assert!(ffi_error.contains("FFI metadata must be an EDN map"), "stderr:\n{ffi_error}");

  let syntax_with_metadata = run_calcit(
    &snapshot,
    &[
      "edit",
      "add-example",
      "app.main/main!",
      "--input-format",
      "cirru",
      "--code",
      "{} (:backend :js)",
    ],
  );
  assert!(
    !syntax_with_metadata.status.success(),
    "FFI metadata should not be accepted as syntax"
  );
  let syntax_error = String::from_utf8_lossy(&syntax_with_metadata.stderr);
  assert!(
    syntax_error.contains("expected node kind `quoted syntax`"),
    "stderr:\n{syntax_error}"
  );
}

#[test]
fn edit_defmacro_keeps_required_optional_and_rest_macros_loadable() {
  let directory = TestDirectory::create();
  let snapshot = prepare_minimal_snapshot(&directory);

  create_macro(&snapshot, "required-id", "quote $ defmacro required-id (value) value");
  create_macro(&snapshot, "optional-id", "quote $ defmacro optional-id (value ? ignored) value");
  create_macro(&snapshot, "rest-id", "quote $ defmacro rest-id (value & ignored) value");

  for (name, expected_schema) in [
    (
      "required-id",
      serde_json::json!([
        "::",
        "'Macro",
        [
          "{}",
          [":capabilities", ["#{}"]],
          [":expansion", ["::", "'Expr", "'Dynamic"]],
          [":required", ["[]", "'Syntax"]]
        ]
      ]),
    ),
    (
      "optional-id",
      serde_json::json!([
        "::",
        "'Macro",
        [
          "{}",
          [":capabilities", ["#{}"]],
          [":expansion", ["::", "'Expr", "'Dynamic"]],
          [":optional", ["[]", "'Syntax"]],
          [":required", ["[]", "'Syntax"]]
        ]
      ]),
    ),
    (
      "rest-id",
      serde_json::json!([
        "::",
        "'Macro",
        [
          "{}",
          [":rest", "'Syntax"],
          [":capabilities", ["#{}"]],
          [":expansion", ["::", "'Expr", "'Dynamic"]],
          [":required", ["[]", "'Syntax"]]
        ]
      ]),
    ),
  ] {
    let definition = query_definition(&snapshot, &format!("app.main/{name}"));
    assert_eq!(definition["data"]["schema"], expected_schema, "schema for {name}");
  }

  let metadata = run_calcit(&snapshot, &["edit", "doc", "app.main/required-id", "|Created through edit def"]);
  assert_success(&metadata, "edit macro metadata");
  let schema_edit = run_calcit(
    &snapshot,
    &[
      "edit",
      "schema",
      "app.main/required-id",
      "--code",
      "quote $ :: 'Macro $ {} (:required $ [] 'Syntax) (:expansion $ :: 'Expr 'Dynamic) (:capabilities $ #{})",
    ],
  );
  assert_success(&schema_edit, "edit macro schema");
  assert_success(&run_calcit(&snapshot, &["--check-only"]), "strict check after macro edits");
}

#[test]
fn definition_attached_calcit_tests_cover_created_macro_expansion() {
  let snapshot = Path::new("calcit/test-edit-defmacro.cirru");
  let strict = run_calcit(snapshot, &["test", "app.main/main!", "--tag", "macro-edit", "--require-match"]);
  assert_success(&strict, "definition-attached strict macro tests");
  let optional_compat = run_calcit(
    snapshot,
    &[
      "--compat-types",
      "test",
      "app.main/main!",
      "--tag",
      "macro-edit-compat",
      "--require-match",
    ],
  );
  assert_success(&optional_compat, "definition-attached optional compatibility test");
}

#[test]
fn edit_transaction_and_format_share_the_canonical_macro_schema() {
  let directory = TestDirectory::create();
  let snapshot = prepare_minimal_snapshot(&directory);

  create_macro(
    &snapshot,
    "direct-shape",
    "quote $ defmacro direct-shape (required ? optional & rest) required",
  );
  let preview_edn = run_calcit(
    &snapshot,
    &[
      "edit",
      "transaction",
      "--code",
      r#"[["edit","def","app.main/preview-only","--code","quote $ defn preview-only () 1"]]"#,
      "--dry-run",
      "--format",
      "edn",
    ],
  );
  assert_success(&preview_edn, "Cirru EDN transaction preview");
  let preview_value = cirru_edn::parse(&String::from_utf8_lossy(&preview_edn.stdout)).expect("transaction EDN should parse");
  let cirru_edn::Edn::Map(preview_map) = preview_value else {
    panic!("transaction EDN should be a map");
  };
  assert_eq!(preview_map.get(&cirru_edn::Edn::tag("changed")), Some(&cirru_edn::Edn::Bool(true)));

  let preview_human = run_calcit(
    &snapshot,
    &[
      "edit",
      "transaction",
      "--code",
      r#"[["edit","def","app.main/preview-only","--code","quote $ defn preview-only () 1"]]"#,
      "--dry-run",
    ],
  );
  assert_success(&preview_human, "human transaction preview");
  let preview_stdout = String::from_utf8_lossy(&preview_human.stdout);
  assert!(preview_stdout.starts_with("# Edit transaction\n"), "stdout:\n{preview_stdout}");
  assert!(preview_stdout.contains("## Operation 1\n\n```text\n"), "stdout:\n{preview_stdout}");
  let transaction = run_calcit(
    &snapshot,
    &[
      "edit",
      "transaction",
      "--code",
      r#"[["edit","def","app.main/transaction-shape","--code","quote $ defmacro transaction-shape (required ? optional & rest) required"]]"#,
      "--format",
      "json",
    ],
  );
  assert_success(&transaction, "transactional edit defmacro");

  let direct_schema = query_definition(&snapshot, "app.main/direct-shape")["data"]["schema"].clone();
  let transaction_schema = query_definition(&snapshot, "app.main/transaction-shape")["data"]["schema"].clone();
  assert_eq!(transaction_schema, direct_schema);

  assert_success(&run_calcit(&snapshot, &["edit", "format"]), "edit format");
  assert_eq!(
    query_definition(&snapshot, "app.main/direct-shape")["data"]["schema"],
    direct_schema
  );
  assert_eq!(
    query_definition(&snapshot, "app.main/transaction-shape")["data"]["schema"],
    direct_schema
  );
  assert_success(&run_calcit(&snapshot, &["--check-only"]), "strict check after format");
}

#[test]
fn overwrite_converts_non_macro_schema_but_preserves_a_strict_macro_contract() {
  let directory = TestDirectory::create();
  let snapshot = prepare_minimal_snapshot(&directory);

  let ordinary = run_calcit(
    &snapshot,
    &[
      "edit",
      "def",
      "app.main/convert-me",
      "--code",
      "quote $ defn convert-me (value) value",
    ],
  );
  assert_success(&ordinary, "create ordinary definition");
  let ordinary_schema = run_calcit(
    &snapshot,
    &[
      "edit",
      "schema",
      "app.main/convert-me",
      "--code",
      "quote $ :: 'Fn $ {} (:args $ [] 'Number) (:return 'Number)",
    ],
  );
  assert_success(&ordinary_schema, "set ordinary definition schema");

  let convert = run_calcit(
    &snapshot,
    &[
      "edit",
      "def",
      "app.main/convert-me",
      "--overwrite",
      "--code",
      "quote $ defmacro convert-me (value) value",
    ],
  );
  assert_success(&convert, "convert ordinary definition to macro");
  let generated = query_definition(&snapshot, "app.main/convert-me")["data"]["schema"].clone();
  assert!(generated.to_string().contains("'Macro"));

  let refined = run_calcit(
    &snapshot,
    &[
      "edit",
      "schema",
      "app.main/convert-me",
      "--code",
      "quote $ :: 'Macro $ {} (:required $ [] 'Syntax) (:expansion $ :: 'Expr 'Number) (:capabilities $ #{})",
    ],
  );
  assert_success(&refined, "refine macro schema");
  let refined_schema = query_definition(&snapshot, "app.main/convert-me")["data"]["schema"].clone();

  let overwrite = run_calcit(
    &snapshot,
    &[
      "edit",
      "def",
      "app.main/convert-me",
      "--overwrite",
      "--code",
      "quote $ defmacro convert-me (next-value) next-value",
    ],
  );
  assert_success(&overwrite, "overwrite macro implementation");
  assert_eq!(query_definition(&snapshot, "app.main/convert-me")["data"]["schema"], refined_schema);
  assert_success(&run_calcit(&snapshot, &["--check-only"]), "strict check after macro overwrite");
}

#[test]
fn malformed_defmacro_is_rejected_without_changing_the_snapshot() {
  let directory = TestDirectory::create();
  let snapshot = prepare_minimal_snapshot(&directory);
  let original = fs::read(&snapshot).expect("minimal snapshot should read");

  for code in [
    "quote $ defmacro broken not-a-list not-a-list",
    "quote $ defmacro broken (value & rest extra) value",
    "quote $ defmacro broken (value & ? optional) value",
    "quote $ defmacro broken (value & rest ? optional) value",
  ] {
    let output = run_calcit(&snapshot, &["edit", "def", "app.main/broken", "--code", code]);
    assert!(!output.status.success(), "malformed macro must fail: {code}");
    assert!(
      String::from_utf8_lossy(&output.stderr).contains("cannot derive a strict `defmacro` schema"),
      "stderr:\n{}",
      String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
      fs::read(&snapshot).expect("snapshot should remain readable"),
      original,
      "failed edit changed the Snapshot: {code}"
    );
  }
  assert_success(&run_calcit(&snapshot, &["--check-only"]), "strict check after rejected edit");
}

#[test]
fn overwriting_with_defexternal_drops_retained_ffi_metadata() {
  let directory = TestDirectory::create();
  let snapshot = prepare_minimal_snapshot(&directory);

  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "def", "app.main/Host", "--code", "quote $ deftrait Host (:value 'String)"],
    ),
    "create external trait",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "ffi",
        "app.main/Host",
        "--code",
        "{} (:backend :js) (:kind :external-object) (:target :node) (:names $ {} (:value |nodeValue))",
      ],
    ),
    "set explicit ffi metadata",
  );

  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "app.main/Host",
        "--overwrite",
        "--code",
        "quote $ defexternal Host (:target :browser) (:value 'String)",
      ],
    ),
    "overwrite with defexternal shorthand",
  );

  // Writing `defexternal` alongside retained `:ffi` would fail the next load.
  assert_success(&run_calcit(&snapshot, &["--check-only"]), "reload after shorthand overwrite");

  let report = query_definition(&snapshot, "app.main/Host");
  assert_eq!(report["data"]["schema"], "'Trait");
  assert_eq!(report["data"]["ffi"][":target"]["__edn_tag"], "browser");
  // The overwrite replaced the previous metadata, so the old `:names` override is gone.
  assert!(report["data"]["ffi"][":names"].is_null());
}
