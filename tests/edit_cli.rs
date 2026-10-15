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
fn concurrent_snapshot_writes_keep_successful_edits_or_report_conflicts() {
  // This is a process/filesystem invariant, not a Calcit language semantic test.
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/test-wasm.cirru", &snapshot).expect("copy isolated concurrency fixture");
  let targets = ["test-wasm.main/main!", "test-wasm.helper/add-and-double"];
  let mut successful_writes = 0;
  for round in 0..20 {
    let start = std::sync::Barrier::new(2);
    let results = std::thread::scope(|scope| {
      let handles: Vec<_> = targets
        .iter()
        .enumerate()
        .map(|(writer, target)| {
          let snapshot = &snapshot;
          let start = &start;
          scope.spawn(move || {
            let doc = format!("writer-{writer}-round-{round}");
            start.wait();
            let output = run_calcit(snapshot, &["edit", "doc", target, &doc]);
            (*target, doc, output)
          })
        })
        .collect();
      handles
        .into_iter()
        .map(|handle| handle.join().expect("writer thread should finish"))
        .collect::<Vec<_>>()
    });
    let content = fs::read_to_string(&snapshot).expect("snapshot remains readable after concurrent writes");
    let data = cirru_edn::parse(&content).expect("snapshot remains valid Cirru EDN");
    let parsed = calcit::snapshot::load_snapshot_data(&data, snapshot.to_str().unwrap()).expect("snapshot remains loadable");
    for (target, expected_doc, output) in results {
      if output.status.success() {
        successful_writes += 1;
        let (ns, name) = target.split_once('/').unwrap();
        assert_eq!(
          parsed.files[ns].defs[name].doc, expected_doc,
          "round {round}: successful edit of {target} must not be silently overwritten"
        );
      } else {
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(
          diagnostic.contains("Snapshot revision mismatch") || diagnostic.contains("writer lock"),
          "failed writer must report a recoverable conflict, not an unrelated failure: {diagnostic}"
        );
      }
    }
  }
  assert!(successful_writes > 0, "write protection must permit progress");
}

#[test]
fn snapshot_mutation_entrypoints_respect_a_live_old_writer_lock() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/test-wasm.cirru", &snapshot).unwrap();
  let original = fs::read(&snapshot).unwrap();
  let guard = calcit::util::atomic_write::SnapshotWriteGuard::acquire(&snapshot).unwrap();
  // Age must not override ownership. Only this isolated fixture is modified.
  fs::write(directory.0.join(".calcit/calcit.cirru.lock"), "pid=123 acquired-at=1\n").unwrap();
  let commands: Vec<Vec<&str>> = vec![
    vec!["edit", "doc", "test-wasm.main/main!", "blocked"],
    vec!["edit", "format"],
    vec!["edit", "schema", "test-wasm.main/main!", "--clear"],
    vec![
      "edit",
      "transaction",
      "--code",
      "[[\"edit\",\"doc\",\"test-wasm.main/main!\",\"blocked\"]]",
    ],
    vec!["config", "set", "init-fn", "test-wasm.main/main!"],
    vec!["config", "set", "package", "test-wasm"],
    vec![
      "tree",
      "replace",
      "test-wasm.main/main!",
      "--path",
      "3",
      "--input-format",
      "cirru",
      "--code",
      "quote 42",
    ],
    vec!["cursor", "cut"],
    vec!["fix", "--apply", "--rule", "unnecessary-do-v1"],
  ];
  std::thread::scope(|scope| {
    let handles: Vec<_> = commands
      .iter()
      .map(|args| {
        let snapshot = &snapshot;
        scope.spawn(move || (args, run_calcit(snapshot, args)))
      })
      .collect();
    for handle in handles {
      let (args, output) = handle.join().unwrap();
      assert!(!output.status.success(), "active writer should block {args:?}");
      assert!(
        String::from_utf8_lossy(&output.stderr).contains("writer lock"),
        "{args:?}: {:?}",
        output
      );
    }
  });
  assert_eq!(fs::read(&snapshot).unwrap(), original);
  drop(guard);
  assert_success(
    &run_calcit(&snapshot, &["edit", "doc", "test-wasm.main/main!", "after release"]),
    "write after release",
  );
}

#[test]
fn interrupted_cli_writer_releases_ownership_and_reports_recovery() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/test-wasm.cirru", &snapshot).unwrap();
  let original = fs::read(&snapshot).unwrap();
  let mut child = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .arg(&snapshot)
    .args(["edit", "def", "test-wasm.main/main!", "--overwrite", "--input-format", "cirru"])
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .spawn()
    .unwrap();
  let lock = directory.0.join(".calcit/calcit.cirru.lock");
  let started = std::time::Instant::now();
  while !fs::read_to_string(&lock).is_ok_and(|content| content.contains(&format!("pid={}", child.id()))) {
    if started.elapsed() > std::time::Duration::from_secs(5) {
      let _ = child.kill();
      let _ = child.wait();
      panic!("child did not acquire the writer lock");
    }
    std::thread::sleep(std::time::Duration::from_millis(10));
  }
  child.kill().unwrap();
  child.wait().unwrap();
  assert_eq!(fs::read(&snapshot).unwrap(), original);
  let retry = run_calcit(&snapshot, &["edit", "doc", "test-wasm.main/main!", "recovered"]);
  assert_success(&retry, "write after interrupted process");
  assert!(String::from_utf8_lossy(&retry.stderr).contains("Recovered writer lock"));
  assert_eq!(query_definition(&snapshot, "test-wasm.main/main!")["data"]["doc"], "recovered");
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

#[test]
fn edit_def_creates_exact_names_beside_shell_sensitive_variants() {
  let directory = TestDirectory::create();
  let snapshot = prepare_minimal_snapshot(&directory);
  for (name, suffix, overwrite) in [("example", "!", false), ("ready", "?", false), ("convert", "->", true)] {
    let variant = format!("app.main/{name}{suffix}");
    let variant_code = format!("quote $ defn {name}{suffix} () 1");
    assert_success(
      &run_calcit(
        &snapshot,
        &["edit", "def", &variant, "--input-format", "cirru", "--code", &variant_code],
      ),
      "create shell-sensitive variant",
    );
    let variant_before = query_definition(&snapshot, &variant)["data"].clone();
    let target = format!("app.main/{name}");
    let alias_code = format!("quote $ def {name} {variant}");
    let mut args = vec!["edit", "def", &target, "--input-format", "cirru", "--code", &alias_code];
    if overwrite {
      args.push("--overwrite");
    }
    assert_success(&run_calcit(&snapshot, &args), "create exact intentional name");
    let alias = query_definition(&snapshot, &target)["data"].clone();
    assert_ne!(alias["code"], variant_before["code"]);
    assert_eq!(query_definition(&snapshot, &variant)["data"], variant_before);

    let before_refusal = fs::read(&snapshot).unwrap();
    let replacement = format!("quote $ defn {name} () 2");
    let refused = run_calcit(
      &snapshot,
      &["edit", "def", &target, "--input-format", "cirru", "--code", &replacement],
    );
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("already exists"));
    assert_eq!(fs::read(&snapshot).unwrap(), before_refusal);
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &target,
          "--overwrite",
          "--input-format",
          "cirru",
          "--code",
          &replacement,
        ],
      ),
      "overwrite only the exact name",
    );
    assert_eq!(query_definition(&snapshot, &variant)["data"], variant_before);
    assert_ne!(query_definition(&snapshot, &target)["data"]["code"], alias["code"]);
  }
}

#[test]
fn intentional_creation_keeps_missing_lookup_hints_and_write_guards() {
  let directory = TestDirectory::create();
  let snapshot = prepare_minimal_snapshot(&directory);
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "app.main/example!",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn example! () 1",
      ],
    ),
    "create effect-style name",
  );
  let original = fs::read(&snapshot).unwrap();
  let query = run_calcit(&snapshot, &["query", "def", "app.main/example", "--format", "json"]);
  assert_success(&query, "read-only lookup keeps its correction");
  let queried: serde_json::Value = serde_json::from_slice(&query.stdout).expect("single JSON document");
  assert_eq!(queried["data"], query_definition(&snapshot, "app.main/example!")["data"]);
  assert!(String::from_utf8_lossy(&query.stderr).contains("Auto-correcting"));
  assert_eq!(fs::read(&snapshot).unwrap(), original);

  let update = run_calcit(&snapshot, &["edit", "doc", "app.main/example", "|must not change example!"]);
  assert!(!update.status.success());
  assert!(String::from_utf8_lossy(&update.stderr).contains("likely intended definition"));
  assert_eq!(fs::read(&snapshot).unwrap(), original);
  let mismatch = run_calcit(
    &snapshot,
    &[
      "edit",
      "def",
      "app.main/example",
      "--overwrite",
      "--input-format",
      "cirru",
      "--code",
      "quote $ defn example! () 2",
    ],
  );
  assert!(!mismatch.status.success());
  assert!(String::from_utf8_lossy(&mismatch.stderr).contains("Definition name mismatch"));
  assert_eq!(fs::read(&snapshot).unwrap(), original);
}

#[test]
fn guarded_transaction_renames_then_recreates_the_old_name() {
  let directory = TestDirectory::create();
  let snapshot = prepare_minimal_snapshot(&directory);
  assert_success(
    &run_calcit(&snapshot, &["config", "set", "target", "native"]),
    "set migration target",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "app.main/example",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn example () 1",
      ],
    ),
    "create original definition",
  );
  let schema = "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)";
  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "schema", "app.main/example", "--input-format", "cirru", "--code", schema],
    ),
    "declare original contract",
  );
  let original_schema = query_definition(&snapshot, "app.main/example")["data"]["schema"].clone();
  let operations = serde_json::json!([
    ["edit", "rename", "app.main/example", "example!"],
    [
      "edit",
      "def",
      "app.main/example",
      "--input-format",
      "cirru",
      "--code",
      "quote $ def example app.main/example!"
    ],
    ["edit", "schema", "app.main/example", "--input-format", "cirru", "--code", schema]
  ]);
  let encoded = operations.to_string();
  let original = fs::read(&snapshot).unwrap();
  let revision = transaction_scope(&snapshot, &encoded);
  assert_eq!(fs::read(&snapshot).unwrap(), original);

  let mut invalid = operations.clone();
  invalid.as_array_mut().unwrap().push(serde_json::json!([
    "edit",
    "def",
    "app.main/broken",
    "--input-format",
    "cirru",
    "--code",
    "quote $ defn mismatched () 2"
  ]));
  let failed = run_calcit(&snapshot, &["edit", "transaction", "--code", &invalid.to_string()]);
  assert!(!failed.status.success());
  assert_eq!(fs::read(&snapshot).unwrap(), original);

  assert_success(
    &run_calcit(&snapshot, &["edit", "doc", "app.main/example", "|concurrent change"]),
    "change the guarded source",
  );
  let concurrent = fs::read(&snapshot).unwrap();
  let stale = run_calcit(
    &snapshot,
    &["edit", "transaction", "--code", &encoded, "--expect-revision", &revision],
  );
  assert!(!stale.status.success());
  assert_eq!(fs::read(&snapshot).unwrap(), concurrent);
  let refreshed = transaction_scope(&snapshot, &encoded);
  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "transaction", "--code", &encoded, "--expect-revision", &refreshed],
    ),
    "apply intentional rename and alias creation",
  );
  let renamed = query_definition(&snapshot, "app.main/example!")["data"].clone();
  let alias = query_definition(&snapshot, "app.main/example")["data"].clone();
  assert_eq!(renamed["schema"], original_schema);
  assert_eq!(alias["schema"], original_schema);
  assert_ne!(renamed["code"], alias["code"]);
  assert!(renamed["doc"].to_string().contains("concurrent change"));
  assert_success(
    &run_calcit(&snapshot, &["--check-only", "--ns", "app.main"]),
    "strict check migrated namespace",
  );
}

#[test]
fn edit_def_rejects_invalid_named_shapes_without_writing() {
  let directory = TestDirectory::create();
  let snapshot = prepare_minimal_snapshot(&directory);
  let original = fs::read(&snapshot).expect("snapshot before invalid writes");
  for code in [
    "quote 42",
    "quote $ hello world",
    "quote $ defn bar (x) x",
    "quote $ ### app.main/foo",
    "quote $ fn foo 1",
    "quote $ fn ()",
    "quote $ deftype-slot other",
    "quote $ deftype-slot :slot :extra",
  ] {
    let output = run_calcit(
      &snapshot,
      &["edit", "def", "app.main/foo", "--input-format", "cirru", "--code", code],
    );
    assert!(!output.status.success(), "invalid definition must fail: {code}");
    assert_eq!(fs::read(&snapshot).unwrap(), original, "invalid definition must not write: {code}");
    if code.contains("defn bar") {
      let error = String::from_utf8_lossy(&output.stderr);
      assert!(
        error.contains("foo") && error.contains("bar"),
        "name mismatch must show both names: {error}"
      );
    }
  }
  let operations = serde_json::json!([
    [
      "edit",
      "def",
      "app.main/valid",
      "--input-format",
      "cirru",
      "--code",
      "quote $ defn valid () 1"
    ],
    [
      "edit",
      "def",
      "app.main/foo",
      "--input-format",
      "cirru",
      "--code",
      "quote $ defn bar () 2"
    ]
  ]);
  let output = run_calcit(&snapshot, &["edit", "transaction", "--code", &operations.to_string()]);
  assert!(!output.status.success(), "invalid definition must abort the transaction");
  assert_eq!(fs::read(&snapshot).unwrap(), original, "failed transaction must not write");
  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "def", "app.main/*slot", "--code", "quote $ deftype-slot :slot"],
    ),
    "type-slot declarations use slot names, not definition keys",
  );
}

#[test]
fn edit_def_preserves_unchanged_historical_names_but_rejects_changed_mismatches() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/test-doc-smoke.cirru", &snapshot).unwrap();
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "test-doc-smoke.main/DocEnum0",
        "--overwrite",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defenum DocEnum (:ok 'String)",
      ],
    ),
    "unchanged nominal type must round-trip without renaming",
  );
  let original = fs::read(&snapshot).unwrap();
  for flags in [vec![], vec!["--allow-unknown-head"]] {
    let mut args = vec![
      "edit",
      "def",
      "test-doc-smoke.main/DocEnum0",
      "--overwrite",
      "--input-format",
      "cirru",
      "--code",
      "quote $ defenum DocEnum (:ok 'Number)",
    ];
    args.extend(flags);
    let output = run_calcit(&snapshot, &args);
    assert!(!output.status.success(), "changed mismatch must be rejected");
    assert_eq!(fs::read(&snapshot).unwrap(), original, "changed mismatch must not write");
  }
}

#[test]
fn edit_def_round_trips_repository_program_definitions() {
  fn syntax_json(node: &cirru_parser::Cirru) -> serde_json::Value {
    match node {
      cirru_parser::Cirru::Leaf(leaf) => serde_json::Value::String(leaf.to_string()),
      cirru_parser::Cirru::List(items) => serde_json::Value::Array(items.iter().map(syntax_json).collect()),
    }
  }
  let mut programs = fs::read_dir("calcit")
    .unwrap()
    .map(|entry| entry.unwrap().path())
    .filter(|path| path.extension().is_some_and(|extension| extension == "cirru"))
    .collect::<Vec<_>>();
  programs.sort();
  // Allow the same filesystem-boundary regression to replay a real consumer
  // snapshot locally without checking ecosystem source into this repository.
  if let Some(consumer) = std::env::var_os("CALCIT_EDIT_ROUNDTRIP_SNAPSHOT") {
    programs.push(PathBuf::from(consumer));
  }
  let mut definitions = 0;
  let mut anonymous_functions = 0;
  for program in programs {
    let directory = TestDirectory::create();
    let snapshot = directory.snapshot();
    fs::copy(&program, &snapshot).unwrap();
    let source = fs::read_to_string(&snapshot).unwrap();
    let data = cirru_edn::parse(&source).unwrap();
    let original = calcit::snapshot::load_snapshot_data(&data, snapshot.to_str().unwrap()).unwrap();
    for (namespace, file) in &original.files {
      // Path metadata is synthesized by the loader, not persisted program code.
      if namespace == &format!("{}.$meta", original.package) {
        continue;
      }
      for (name, entry) in &file.defs {
        let target = format!("{namespace}/{name}");
        let code = syntax_json(&entry.code).to_string();
        assert_success(
          &run_calcit(
            &snapshot,
            &["edit", "def", &target, "--overwrite", "--input-format", "json-ast", "--code", &code],
          ),
          &format!("round-trip {} {target}", program.display()),
        );
        definitions += 1;
        if matches!(&entry.code, cirru_parser::Cirru::List(items) if items.first().is_some_and(|head| head.eq_leaf("fn"))) {
          anonymous_functions += 1;
        }
      }
    }
    let rewritten = cirru_edn::parse(&fs::read_to_string(&snapshot).unwrap()).unwrap();
    let loaded = calcit::snapshot::load_snapshot_data(&rewritten, snapshot.to_str().unwrap()).unwrap();
    for (namespace, file) in &original.files {
      for (name, entry) in &file.defs {
        assert_eq!(
          calcit::snapshot::definition_revision(entry).unwrap(),
          calcit::snapshot::definition_revision(&loaded.files[namespace].defs[name]).unwrap(),
          "round-trip must retain all definition metadata: {} {namespace}/{name}",
          program.display()
        );
      }
    }
  }
  assert!(definitions > 0, "repository corpus must not be empty");
  assert!(anonymous_functions > 0, "anonymous function compatibility must be exercised");
}

#[test]
fn edit_def_accepts_source_macros_and_bounds_the_unknown_head_override() {
  let directory = TestDirectory::create();
  let snapshot = prepare_minimal_snapshot(&directory);
  assert_success(
    &run_calcit(&snapshot, &["edit", "add-ns", "app.macros"]),
    "create local macro namespace",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "app.macros/defview",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defmacro defview (name) $ quasiquote $ defn (~ name) () 1",
      ],
    ),
    "declare source macro",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "add-import", "app.main", "--code", "quote $ app.macros :as macros"],
    ),
    "import local macro namespace",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "def", "app.main/view", "--code", "quote $ macros/defview view"],
    ),
    "accept qualified source macro",
  );
  let modules = directory.0.join(".calcit/modules/ui");
  fs::create_dir_all(&modules).unwrap();
  let module = modules.join("calcit.cirru");
  fs::copy("calcit/test-wasm.cirru", &module).unwrap();
  assert_success(&run_calcit(&module, &["edit", "add-ns", "test-wasm.ui"]), "create module namespace");
  assert_success(
    &run_calcit(
      &module,
      &[
        "edit",
        "def",
        "test-wasm.ui/defwidget",
        "--code",
        "quote $ defmacro defwidget (name) $ quasiquote $ defn (~ name) () 1",
      ],
    ),
    "declare dependency macro",
  );
  assert_success(&run_calcit(&snapshot, &["config", "add-module", "ui/"]), "configure module");
  assert_success(
    &run_calcit(&snapshot, &["config", "add-module", "missing/"]),
    "configure unavailable module for lookup diagnostics",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-import",
        "app.main",
        "--code",
        "quote $ test-wasm.ui :refer $ defwidget",
      ],
    ),
    "import dependency macro",
  );
  assert_success(
    &run_calcit(&snapshot, &["edit", "def", "app.main/widget", "--code", "quote $ defwidget widget"]),
    "accept source-declared dependency macro",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "add-import", "app.main", "--code", "quote $ unavailable.ui :as broken"],
    ),
    "import unavailable dependency namespace",
  );
  let original = fs::read(&snapshot).unwrap();
  let broken = run_calcit(
    &snapshot,
    &[
      "edit",
      "def",
      "app.main/missing-widget",
      "--code",
      "quote $ broken/defwidget missing-widget",
    ],
  );
  assert!(!broken.status.success(), "unavailable source must not validate a macro");
  let error = String::from_utf8_lossy(&broken.stderr);
  assert!(
    error.contains("missing/") && error.contains("load"),
    "module failure must remain visible: {error}"
  );
  assert!(
    !error.contains("Use --allow-unknown-head"),
    "dependency failures must not recommend bypassing validation: {error}"
  );
  assert_eq!(fs::read(&snapshot).unwrap(), original, "failed macro resolution must not write");
  let unknown = run_calcit(&snapshot, &["edit", "def", "app.main/custom", "--code", "quote $ special custom 1"]);
  assert!(!unknown.status.success());
  let error = String::from_utf8_lossy(&unknown.stderr);
  assert!(
    error.contains("Unrecognized definition head") && error.contains("--allow-unknown-head"),
    "{error}"
  );
  assert_eq!(fs::read(&snapshot).unwrap(), original);
  for code in ["quote 42", "quote $ special other 1"] {
    let output = run_calcit(
      &snapshot,
      &["edit", "def", "app.main/custom", "--allow-unknown-head", "--code", code],
    );
    assert!(!output.status.success(), "override must retain shape/name checks");
    assert_eq!(fs::read(&snapshot).unwrap(), original);
  }
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "app.main/custom",
        "--allow-unknown-head",
        "--code",
        "quote $ special custom 1",
      ],
    ),
    "explicitly allow an intentional custom head",
  );
}

#[test]
fn import_edits_recognize_legacy_list_prefixed_rules() {
  let directory = TestDirectory::create();
  let snapshot = prepare_minimal_snapshot(&directory);
  let imports = "quote $ [] ([] calcit.core :refer $ [] inc) ([] app.other :as other) (app.keep :as keep)";
  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "imports", "app.main", "--input-format", "cirru", "--code", imports],
    ),
    "install mixed legacy and modern imports",
  );
  let read_ns = || {
    let source = fs::read_to_string(&snapshot).unwrap();
    let data = cirru_edn::parse(&source).unwrap();
    let parsed = calcit::snapshot::load_snapshot_data(&data, snapshot.to_str().unwrap()).unwrap();
    parsed.files["app.main"].ns.code.clone()
  };
  let original = read_ns();
  assert_success(
    &run_calcit(&snapshot, &["edit", "rm-import", "app.main", "calcit.core"]),
    "remove legacy refer import",
  );
  let mut expected = original.clone();
  let cirru_parser::Cirru::List(ns) = &mut expected else {
    panic!("namespace expression")
  };
  let cirru_parser::Cirru::List(require) = &mut ns[2] else {
    panic!("require expression")
  };
  require.remove(1);
  assert_eq!(read_ns(), expected, "unrelated rules must retain their exact AST");

  let before_missing = fs::read(&snapshot).unwrap();
  let missing = run_calcit(&snapshot, &["edit", "rm-import", "app.main", "app.missing"]);
  assert!(!missing.status.success());
  assert_eq!(fs::read(&snapshot).unwrap(), before_missing, "missing imports must not write");

  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-import",
        "app.main",
        "--overwrite",
        "--input-format",
        "cirru",
        "--code",
        "quote $ app.other :as renamed",
      ],
    ),
    "overwrite an existing legacy alias rather than append a duplicate",
  );
  let rendered = read_ns().to_string();
  assert_eq!(rendered.matches("app.other").count(), 1);
  assert!(rendered.contains("renamed"));
  assert_success(
    &run_calcit(&snapshot, &["edit", "rm-import", "app.main", "app.keep"]),
    "remove modern alias import",
  );
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
        "--verbose",
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

  // Without --verbose the decoded-input echo stays out of the default output.
  let quiet = run_calcit(
    &snapshot,
    &[
      "edit",
      "add-example",
      "app.main/main!",
      "--input-format",
      "cirru",
      "--code",
      "quote $ inc 2",
    ],
  );
  assert_success(&quiet, "add example without verbose output");
  let stdout = String::from_utf8_lossy(&quiet.stdout);
  assert!(!stdout.contains("Decoded syntax input"), "stdout:\n{stdout}");

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
      "--verbose",
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
fn unloadable_macro_schema_edits_are_rejected_without_changing_the_snapshot() {
  let directory = TestDirectory::create();
  let snapshot = prepare_minimal_snapshot(&directory);
  create_macro(&snapshot, "echo-source", "quote $ defmacro echo-source (value) , value");
  let original = fs::read(&snapshot).expect("snapshot should read");

  let invalid_expansion = "quote $ :: 'Macro $ {} (:required $ [] 'Syntax) (:capabilities $ #{}) (:expansion 'Syntax)";
  let rejected = [
    vec![
      "edit",
      "schema",
      "app.main/echo-source",
      "--input-format",
      "cirru",
      "--code",
      invalid_expansion,
    ],
    vec![
      "edit",
      "schema",
      "app.main/echo-source",
      "--input-format",
      "cirru",
      "--code",
      "quote $ :: 'Fn $ {} (:args $ [] 'Syntax) (:return 'Dynamic)",
    ],
    vec!["edit", "schema", "app.main/echo-source", "--clear"],
  ];
  for args in &rejected {
    let output = run_calcit(&snapshot, args);
    assert!(!output.status.success(), "unloadable schema edit must fail: {args:?}");
    assert!(
      String::from_utf8_lossy(&output.stderr).contains("Schema validation failed"),
      "stderr:\n{}",
      String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
      fs::read(&snapshot).expect("snapshot should read"),
      original,
      "failed edit changed the Snapshot: {args:?}"
    );
  }

  let transaction_code = format!(
    r#"[["edit","schema","app.main/echo-source","--input-format","cirru","--code",{}]]"#,
    serde_json::to_string(invalid_expansion).expect("code should encode")
  );
  let transaction = run_calcit(&snapshot, &["edit", "transaction", "--code", &transaction_code]);
  assert!(!transaction.status.success(), "transaction with an unloadable schema must fail");
  assert_eq!(
    fs::read(&snapshot).expect("snapshot should read"),
    original,
    "failed transaction changed the Snapshot"
  );

  query_definition(&snapshot, "app.main/echo-source");
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "app.main/echo-source",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Macro $ {} (:required $ [] 'Syntax) (:capabilities $ #{}) (:expansion $ :: 'Expr 'Dynamic)",
      ],
    ),
    "valid macro schema edit",
  );
  query_definition(&snapshot, "app.main/echo-source");
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

/// Diagnostic serialization is a CLI/artifact boundary, not a language semantic test.
#[test]
fn diagnostic_ir_preserves_loaded_external_trait_declarations() {
  use cirru_edn::Edn;

  fn member(value: &Edn, key: Edn) -> &Edn {
    let Edn::Map(map) = value else { panic!("expected a map: {value}") };
    map.get(&key).unwrap_or_else(|| panic!("missing {key}: {value}"))
  }

  fn find_trait<'a>(value: &'a Edn, name: &str) -> Option<&'a Edn> {
    match value {
      Edn::Map(map)
        if map.get(&Edn::tag("kind")) == Some(&Edn::tag("trait")) && map.get(&Edn::tag("name")) == Some(&Edn::str(name)) =>
      {
        Some(value)
      }
      Edn::Map(map) => map.0.iter().find_map(|(_, item)| find_trait(item, name)),
      Edn::List(items) => items.iter().find_map(|item| find_trait(item, name)),
      _ => None,
    }
  }

  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/add.cirru", &snapshot).expect("minimal source should copy");
  assert_success(
    &run_calcit(&snapshot, &["config", "show", "--format", "json"]),
    "read fixture config",
  );
  query_definition(&snapshot, "app.main/main!");
  let operations = serde_json::json!([
    ["edit", "add-ns", "app.host"],
    ["edit", "def", "app.host/LabelHost", "--input-format", "cirru", "--code",
      "quote $ defexternal LabelHost (:target :node) (:label 'String)"],
    ["edit", "def", "app.host/WidgetHost", "--input-format", "cirru", "--code",
      "quote $ deftrait WidgetHost\n  requires LabelHost\n  :id 'String\n  .refresh! $ :: 'Fn $ {} (:args $ [] 'WidgetHost) (:return 'Unit)"],
    ["edit", "schema", "app.host/WidgetHost", "--input-format", "cirru", "--code", "quote $ :: 'Trait"],
    ["edit", "ffi", "app.host/WidgetHost", "--code",
      "{} (:backend :js) (:kind :external-object) (:target :node) (:names $ {} (:id |nodeId) (:refresh! |refresh)) (:writable $ #{} :id)"],
    ["edit", "def", "app.main/identity-host", "--input-format", "cirru", "--code",
      "quote $ defn identity-host (host) (, host)"],
    ["edit", "schema", "app.main/identity-host", "--input-format", "cirru", "--code",
      "quote $ :: 'Fn $ {} (:args $ [] 'app.host/WidgetHost) (:return 'app.host/WidgetHost)"],
    ["edit", "def", "app.host/cast-host", "--input-format", "cirru", "--code",
      "quote $ defn cast-host (raw) (js-cast raw WidgetHost)"],
    ["edit", "schema", "app.host/cast-host", "--input-format", "cirru", "--code",
      "quote $ :: 'Fn $ {} (:args $ [] 'JsObject) (:return 'app.host/WidgetHost) (:features $ #{} :js-ffi)"],
    ["edit", "def", "app.main/main!", "--overwrite", "--input-format", "cirru", "--code",
      "quote $ defn main! () (assert= identity-host identity-host) (assert= app.host/cast-host app.host/cast-host) &unit"],
    ["edit", "schema", "app.main/main!", "--input-format", "cirru", "--code",
      "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)"],
    ["edit", "def", "app.main/reload!", "--input-format", "cirru", "--code", "quote $ defn reload! () &unit"],
    ["edit", "schema", "app.main/reload!", "--input-format", "cirru", "--code",
      "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)"]
  ]).to_string();
  let revision = transaction_scope(&snapshot, &operations);
  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "transaction", "--code", &operations, "--expect-revision", &revision],
    ),
    "create guarded external trait fixture",
  );
  assert_success(&run_calcit(&snapshot, &["--check-only"]), "strict external trait preprocessing");
  assert_success(&run_calcit(&snapshot, &[]), "ordinary native entry");
  let emit_path = directory.0.join("artifacts");
  let emit = emit_path.to_str().expect("UTF-8 artifact path");
  assert_success(&run_calcit(&snapshot, &["--emit-path", emit, "js"]), "ordinary JS generation");
  assert_success(&run_calcit(&snapshot, &["--emit-path", emit, "ir"]), "diagnostic IR generation");

  let ir = cirru_edn::parse(&fs::read_to_string(emit_path.join("program-ir.cirru")).expect("IR artifact"))
    .expect("IR should remain valid Cirru EDN");
  let files = member(&ir, Edn::tag("files"));
  let defs = member(member(files, Edn::str("app.host")), Edn::tag("defs"));
  let host = find_trait(member(defs, Edn::str("cast-host")), "WidgetHost").expect("resolved trait in cast IR");
  assert_eq!(member(host, Edn::tag("kind")), &Edn::tag("trait"));
  assert_eq!(member(host, Edn::tag("name")), &Edn::str("WidgetHost"));
  assert_eq!(member(host, Edn::tag("definition-ref")), &Edn::str("app.host/WidgetHost"));
  let Edn::List(requires) = member(host, Edn::tag("requires")) else {
    panic!("trait requirements should be a list")
  };
  assert_eq!(requires.len(), 1);
  let ffi = member(host, Edn::tag("ffi"));
  assert_eq!(member(ffi, Edn::tag("kind")), &Edn::tag("external-object"));
  assert_eq!(member(ffi, Edn::tag("target")), &Edn::tag("node"));
  let names = member(ffi, Edn::tag("names"));
  assert_eq!(member(names, Edn::tag("id")), &Edn::str("nodeId"));
  assert_eq!(member(names, Edn::tag("refresh!")), &Edn::str("refresh"));
  assert_eq!(
    member(ffi, Edn::tag("writable")),
    &Edn::Set(cirru_edn::EdnSetView([Edn::tag("id")].into()))
  );
  let Edn::List(members) = member(host, Edn::tag("members")) else {
    panic!("trait members should be a list")
  };
  assert_eq!(members.len(), 2);
  for (item, name, kind, expected_type) in [
    (members.get(0).unwrap(), "id", "field", Edn::sym("String")),
    (
      members.get(1).unwrap(),
      "refresh!",
      "method",
      Edn::enum_value(
        "Fn",
        vec![Edn::map_from_iter([
          (Edn::tag("args"), Edn::from(vec![Edn::sym("WidgetHost")])),
          (Edn::tag("return"), Edn::sym("Unit")),
        ])],
      ),
    ),
  ] {
    assert_eq!(member(item, Edn::tag("name")), &Edn::str(name));
    assert_eq!(member(item, Edn::tag("kind")), &Edn::tag(kind));
    assert_eq!(member(item, Edn::tag("type")), &expected_type);
    assert_eq!(member(item, Edn::tag("default")), &Edn::Nil);
  }
  let parent = requires.get(0).unwrap();
  assert_eq!(member(parent, Edn::tag("name")), &Edn::str("LabelHost"));
  assert_eq!(member(parent, Edn::tag("definition-ref")), &Edn::str("app.host/LabelHost"));
  let Edn::List(parent_members) = member(parent, Edn::tag("members")) else {
    panic!("parent members should be a list")
  };
  assert_eq!(parent_members.len(), 1);
  assert_eq!(member(parent_members.get(0).unwrap(), Edn::tag("type")), &Edn::sym("String"));
}

/// Default output of common agent commands stays compact; details need explicit flags.
#[test]
fn agent_commands_print_compact_default_output() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/test.cirru", &snapshot).unwrap();

  // A one-leaf replacement reports operation, location and inline before/after only.
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "app.main/compact-sample",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn compact-sample () (println |old-text)",
      ],
    ),
    "create compact sample",
  );
  let replaced = run_calcit(
    &snapshot,
    &[
      "tree",
      "search-replace",
      "app.main/compact-sample",
      "--pattern",
      "|old-text",
      "--input-format",
      "cirru",
      "--code",
      "quote |new-text",
    ],
  );
  assert_success(&replaced, "search-replace");
  let stdout = String::from_utf8_lossy(&replaced.stdout);
  assert!(stdout.contains("# Tree mutation `search-replace` at `@3.1`"), "stdout:\n{stdout}");
  assert!(
    stdout.contains("- before: `|old-text`") && stdout.contains("- after: `|new-text`"),
    "stdout:\n{stdout}"
  );
  assert!(!stdout.contains("Decoded syntax input"), "stdout:\n{stdout}");

  // Docs are stored verbatim, so a Cirru string prefix earns a warning.
  let doc = run_calcit(&snapshot, &["edit", "doc", "app.main/compact-sample", "|Prints text"]);
  assert_success(&doc, "edit doc");
  assert!(String::from_utf8_lossy(&doc.stderr).contains("stored verbatim"));
  let plain_doc = run_calcit(&snapshot, &["edit", "doc", "app.main/compact-sample", "Prints text"]);
  assert!(!String::from_utf8_lossy(&plain_doc.stderr).contains("stored verbatim"));

  // Short definitions are shown whole even above the node trigger.
  let short = run_calcit(&snapshot, &["query", "def", "app.main/main!"]);
  assert_success(&short, "query short definition");
  let stdout = String::from_utf8_lossy(&short.stdout);
  assert!(stdout.contains("## Cirru\n") && !stdout.contains("Chunked"), "stdout:\n{stdout}");
  let forced = run_calcit(
    &snapshot,
    &[
      "query",
      "def",
      "app.main/main!",
      "--chunk-trigger-bytes",
      "0",
      "--chunk-trigger-nodes",
      "10",
      "--chunk-target-nodes",
      "8",
      "--chunk-max-nodes",
      "12",
    ],
  );
  assert!(String::from_utf8_lossy(&forced.stdout).contains("## Chunked Cirru"));

  // Context leaves out core syntax/macros and generic Fn methods unless asked.
  let context = |extra: &[&str]| -> serde_json::Value {
    let mut args = vec![
      "query",
      "context",
      "app.main/test-fn",
      "--format",
      "json",
      "--dependency-limit",
      "200",
    ];
    args.extend_from_slice(extra);
    let output = run_calcit(&snapshot, &args);
    assert_success(&output, "query context");
    serde_json::from_slice(&output.stdout).unwrap()
  };
  let ids = |value: &serde_json::Value, key: &str, field: &str| -> Vec<String> {
    value["data"][key]["items"]
      .as_array()
      .map(|items| {
        items
          .iter()
          .map(|item| item[field].as_str().unwrap_or_default().to_owned())
          .collect()
      })
      .unwrap_or_default()
  };
  let compact = context(&[]);
  let full = context(&["--include-core"]);
  for core in ["calcit.core/defn", "calcit.core/let", "calcit.core/fn"] {
    let in_full = ids(&full, "dependencies", "id").iter().any(|id| id == core);
    if in_full {
      assert!(
        !ids(&compact, "dependencies", "id").iter().any(|id| id == core),
        "{core}: {compact}"
      );
    }
  }
  assert!(
    ids(&full, "dependencies", "id").len() > ids(&compact, "dependencies", "id").len(),
    "{full}"
  );
  assert!(ids(&full, "static_methods", "name").iter().any(|name| name == ".apply"), "{full}");
  assert!(
    !ids(&compact, "static_methods", "name").iter().any(|name| name == ".apply"),
    "{compact}"
  );
}

/// `query def --format cirru` output is accepted by `edit def --overwrite` and leaves the Snapshot unchanged.
#[test]
fn query_def_cirru_view_writes_back_byte_identically() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/test.cirru", &snapshot).unwrap();
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "app.main/view-sample",
        "--input-format",
        "json-ast",
        "--code",
        r#"["defn","view-sample",["x"],[";","keep","this","note"],["println","|a b $ c","|",["str","|(x)","|"]],["let",[["y",["[]","1","2"]]],[",","y"]]]"#,
      ],
    ),
    "create definition with comments and tricky leaves",
  );
  let original = fs::read(&snapshot).unwrap();
  for target in ["app.main/view-sample", "app.main/main!", "app.main/test-fn"] {
    let view = run_calcit(&snapshot, &["query", "def", target, "--format", "cirru"]);
    assert_success(&view, &format!("cirru view of {target}"));
    let text = String::from_utf8(view.stdout).unwrap();
    assert!(text.starts_with("quote $ "), "{target} view must be a quoted definition:\n{text}");
    let file = directory.0.join("view.cirru");
    fs::write(&file, &text).unwrap();
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          target,
          "--overwrite",
          "--input-format",
          "cirru",
          "--file",
          file.to_str().unwrap(),
        ],
      ),
      &format!("write back {target}"),
    );
    assert_eq!(fs::read(&snapshot).unwrap(), original, "writing back {target} changed the Snapshot");
  }
  let missing = run_calcit(&snapshot, &["query", "def", "app.main/no-such-definition", "--format", "cirru"]);
  assert!(!missing.status.success());
  assert!(missing.stdout.is_empty(), "failed view must not print partial source");
}

fn query_ns_view(snapshot: &Path, namespace: &str) -> String {
  let output = run_calcit(snapshot, &["query", "ns", namespace, "--format", "cirru"]);
  assert_success(&output, &format!("FileEntry data of namespace {namespace}"));
  String::from_utf8(output.stdout).unwrap()
}

fn edit_ns_json(snapshot: &Path, namespace: &str, view: &Path, extra: &[&str]) -> Output {
  let mut args = vec!["edit", "ns", namespace, "--file", view.to_str().unwrap(), "--format", "json"];
  args.extend_from_slice(extra);
  run_calcit(snapshot, &args)
}

fn definition_statuses(report: &serde_json::Value) -> std::collections::BTreeMap<String, String> {
  report["definitions"]
    .as_array()
    .unwrap()
    .iter()
    .map(|item| {
      (
        item["name"].as_str().unwrap().to_owned(),
        item["status"].as_str().unwrap().to_owned(),
      )
    })
    .collect()
}

fn definition_fields(report: &serde_json::Value, name: &str) -> serde_json::Value {
  report["definitions"]
    .as_array()
    .unwrap()
    .iter()
    .find(|item| item["name"] == name)
    .unwrap_or_else(|| panic!("no report entry for {name}: {report}"))["fields"]
    .clone()
}

fn replace_once(text: &str, from: &str, to: &str) -> String {
  assert_eq!(text.matches(from).count(), 1, "expected exactly one `{from}` in:\n{text}");
  text.replacen(from, to, 1)
}

/// Drop one `'name $ %{} 'CodeEntry ...` entry from the `:defs` map of a FileEntry view.
fn remove_definition_entry(view: &str, name: &str) -> String {
  let lines = view.lines().collect::<Vec<_>>();
  let start = lines
    .iter()
    .position(|line| line.starts_with(&format!("    '{name} $ ")))
    .unwrap_or_else(|| panic!("no entry for {name}"));
  let end = lines[start + 1..]
    .iter()
    .position(|line| !line.starts_with("     "))
    .map(|offset| start + 1 + offset)
    .unwrap_or(lines.len());
  let mut kept = lines[..start].to_vec();
  kept.extend_from_slice(&lines[end..]);
  format!("{}\n", kept.join("\n"))
}

/// `query ns --format cirru` prints the stored FileEntry data; writing it back leaves each Snapshot byte-identical.
#[test]
fn query_ns_file_entry_data_writes_back_byte_identically() {
  for (source, namespace) in [
    ("calcit/test.cirru", "app.main"),
    ("calcit/test-def-meta.cirru", "test-def-meta.main"),
    ("calcit/test-types.cirru", "test-types.main"),
    ("calcit/test-struct.cirru", "test-struct.main"),
    // Its stored `ns` form names `test-algebra`; unchanged data is accepted.
    ("calcit/test-algebra.cirru", "test-algebra.main"),
    ("src/cirru/calcit-core.cirru", "calcit.core"),
  ] {
    let directory = TestDirectory::create();
    let snapshot = directory.snapshot();
    fs::copy(source, &snapshot).unwrap();
    let original = fs::read(&snapshot).unwrap();
    let text = query_ns_view(&snapshot, namespace);
    assert!(
      text.starts_with("%{} 'FileEntry\n"),
      "{namespace} data must be a FileEntry:\n{text}"
    );
    let view = directory.0.join("view.cirru");
    fs::write(&view, &text).unwrap();
    let output = edit_ns_json(&snapshot, namespace, &view, &[]);
    assert_success(&output, &format!("write back {namespace}"));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["changed"], false, "{namespace}: {report}");
    assert!(
      definition_statuses(&report).values().all(|status| status == "unchanged"),
      "{namespace}: {report}"
    );
    assert_eq!(
      fs::read(&snapshot).unwrap(),
      original,
      "writing back {namespace} changed the Snapshot"
    );
  }

  // For a canonically formatted Snapshot the data is exactly the namespace's slice of the file.
  let file = fs::read_to_string("calcit/test-def-meta.cirru").unwrap();
  let text = query_ns_view(Path::new("calcit/test-def-meta.cirru"), "test-def-meta.main");
  let indented = text.lines().map(|line| format!("    {line}\n")).collect::<String>();
  assert!(file.contains(&indented), "data is not a slice of the Snapshot:\n{text}");
}

#[test]
fn edit_ns_writes_only_changed_definitions() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/test-def-meta.cirru", &snapshot).unwrap();
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        "test-def-meta.main/reload!",
        "smoke",
        "--input-format",
        "cirru",
        "--code",
        "quote $ assert= 1 1",
      ],
    ),
    "attach a definition test",
  );
  let before = fs::read(&snapshot).unwrap();
  let namespace = "test-def-meta.main";
  let text = query_ns_view(&snapshot, namespace);
  assert!(text.contains("assert= 1 1"), "tests are part of the data:\n{text}");

  // One changed definition: the result equals `edit def --overwrite` of just that definition.
  let edited = replace_once(
    &text,
    ":code $ quote $ defn reload! () (:: 'Unit)",
    ":code $ quote $ defn reload! () (println |reloaded) (:: 'Unit)",
  );
  let view = directory.0.join("view.cirru");
  fs::write(&view, &edited).unwrap();
  let preview = edit_ns_json(&snapshot, namespace, &view, &["--dry-run"]);
  assert_success(&preview, "preview one changed definition");
  let report: serde_json::Value = serde_json::from_slice(&preview.stdout).unwrap();
  assert_eq!(report["changed"], true);
  let statuses = definition_statuses(&report);
  assert_eq!(statuses["reload!"], "changed");
  assert_eq!(definition_fields(&report, "reload!"), serde_json::json!(["code"]));
  assert_eq!(statuses.values().filter(|status| *status != "unchanged").count(), 1, "{report}");
  assert_eq!(fs::read(&snapshot).unwrap(), before, "dry-run must not write");

  let reference = directory.0.join("reference.cirru");
  fs::copy(&snapshot, &reference).unwrap();
  assert_success(
    &run_calcit(
      &reference,
      &[
        "edit",
        "def",
        "test-def-meta.main/reload!",
        "--overwrite",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn reload! () (println |reloaded) (:: 'Unit)",
      ],
    ),
    "reference single-definition edit",
  );
  let scoped = report["scoped_revision"].as_str().unwrap().to_owned();
  assert!(scoped.starts_with("scope:def:test-def-meta.main/reload!@"), "{scoped}");
  assert_success(
    &edit_ns_json(&snapshot, namespace, &view, &["--expect-revision", &scoped]),
    "apply one changed definition",
  );
  assert_eq!(fs::read(&snapshot).unwrap(), fs::read(&reference).unwrap());
  let reload = query_definition(&snapshot, "test-def-meta.main/reload!");
  assert_eq!(reload["data"]["tests"].as_array().map(Vec::len), Some(1), "{reload}");

  // Every field in the data is authoritative: a doc edit and a new definition.
  let text = query_ns_view(&snapshot, namespace);
  let edited = replace_once(
    &text,
    "(:doc \"|Sample definition for def metadata lookup tests\")",
    "(:doc \"|Updated through namespace data\")",
  );
  let edited = replace_once(
    &edited,
    "  :defs $ {}\n",
    "  :defs $ {}\n    'added-fn $ %{} 'CodeEntry (:doc \"|Added through namespace data\")\n      :code $ quote $ defn added-fn (x) x\n      :examples $ []\n",
  );
  fs::write(&view, &edited).unwrap();
  let output = edit_ns_json(&snapshot, namespace, &view, &[]);
  assert_success(&output, "apply doc change and new definition");
  let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
  let statuses = definition_statuses(&report);
  assert_eq!(statuses["MetaSample"], "changed");
  assert_eq!(definition_fields(&report, "MetaSample"), serde_json::json!(["doc"]));
  assert_eq!(statuses["added-fn"], "added");
  assert_eq!(statuses["main!"], "unchanged");
  let sample = query_definition(&snapshot, "test-def-meta.main/MetaSample");
  assert_eq!(sample["data"]["doc"], "Updated through namespace data");
  let added = query_definition(&snapshot, "test-def-meta.main/added-fn");
  assert_eq!(added["data"]["doc"], "Added through namespace data");
}

#[test]
fn edit_ns_rejects_removal_without_flag_stale_revisions_and_malformed_data() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/test-def-meta.cirru", &snapshot).unwrap();
  let namespace = "test-def-meta.main";
  let before = fs::read(&snapshot).unwrap();
  let text = query_ns_view(&snapshot, namespace);
  let view = directory.0.join("view.cirru");

  let without = remove_definition_entry(&text, "test-missing-doc");
  fs::write(&view, &without).unwrap();
  for extra in [&[][..], &["--dry-run"][..]] {
    let rejected = edit_ns_json(&snapshot, namespace, &view, extra);
    assert!(!rejected.status.success(), "removal without --allow-remove must fail");
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("--allow-remove"));
    assert_eq!(fs::read(&snapshot).unwrap(), before);
  }

  fs::write(&view, &text).unwrap();
  let stale = edit_ns_json(
    &snapshot,
    namespace,
    &view,
    &["--expect-revision", "md5:00000000000000000000000000000000"],
  );
  assert!(!stale.status.success(), "unchanged data still checks the revision");
  let changed = replace_once(
    &text,
    ":code $ quote $ defn reload! () (:: 'Unit)",
    ":code $ quote $ defn reload! () (println |stale) (:: 'Unit)",
  );
  fs::write(&view, &changed).unwrap();
  let stale = edit_ns_json(
    &snapshot,
    namespace,
    &view,
    &["--expect-revision", "md5:00000000000000000000000000000000"],
  );
  assert!(!stale.status.success(), "a stale revision must reject the write");
  assert!(String::from_utf8_lossy(&stale.stderr).contains("revision mismatch"));
  assert_eq!(fs::read(&snapshot).unwrap(), before);

  // Malformed data is rejected before anything is written.
  for (label, data, expected) in [
    ("unparsable EDN", "%{} 'FileEntry (:defs".to_owned(), "Failed to parse"),
    (
      "missing :code",
      replace_once(&changed, ":code $ quote $ defn reload! () (println |stale) (:: 'Unit)\n", ""),
      "reload!",
    ),
    (
      "definition shape",
      replace_once(&changed, "defn reload! () (println |stale)", "defn other-name () (println |stale)"),
      "name mismatch",
    ),
    (
      "renamed ns form",
      replace_once(&changed, "ns test-def-meta.main", "ns test-def-meta.other"),
      "Namespace name mismatch",
    ),
    // The loader ignores unknown fields and collapses duplicate keys; edit ns must not lose data that way.
    (
      "misspelled :tests",
      replace_once(
        &text,
        ":code $ quote $ defn reload! () (:: 'Unit)\n",
        ":code $ quote $ defn reload! () (:: 'Unit)\n      :test $ []\n",
      ),
      "Unknown key `:test` in definition 'test-def-meta.main/reload!'",
    ),
    (
      "misspelled :doc",
      replace_once(&text, "(:doc \"|Reload handler\")", "(:dco \"|Reload handler\")"),
      "Unknown key `:dco` in definition 'test-def-meta.main/reload!'",
    ),
    (
      "duplicate CodeEntry key",
      replace_once(
        &text,
        ":code $ quote $ defn reload! () (:: 'Unit)\n",
        ":code $ quote $ defn reload! () (:: 'Unit)\n      :doc |again\n",
      ),
      "Duplicate key `:doc` in definition 'test-def-meta.main/reload!'",
    ),
    (
      "duplicate definition",
      replace_once(
        &text,
        "  :defs $ {}\n",
        "  :defs $ {}\n    'reload! $ %{} 'CodeEntry (:doc |)\n      :code $ quote $ defn reload! () nil\n",
      ),
      "Definition 'test-def-meta.main/reload!' appears more than once",
    ),
    (
      "unknown FileEntry key",
      replace_once(&text, "  :defs $ {}\n", "  :def $ {}\n"),
      "Unknown key `:def` in the FileEntry",
    ),
  ] {
    fs::write(&view, &data).unwrap();
    let rejected = edit_ns_json(&snapshot, namespace, &view, &[]);
    assert!(!rejected.status.success(), "{label} must be rejected");
    let stderr = String::from_utf8_lossy(&rejected.stderr);
    assert!(stderr.contains(expected), "{label}: expected `{expected}` in\n{stderr}");
    assert_eq!(fs::read(&snapshot).unwrap(), before, "{label} wrote to the Snapshot");
  }

  fs::write(&view, &without).unwrap();
  let output = edit_ns_json(&snapshot, namespace, &view, &["--allow-remove"]);
  assert_success(&output, "remove with --allow-remove");
  let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
  assert_eq!(definition_statuses(&report)["test-missing-doc"], "removed");
  let removed = run_calcit(
    &snapshot,
    &["query", "def", "test-def-meta.main/test-missing-doc", "--format", "cirru"],
  );
  assert!(!removed.status.success());
}

/// A macro added in the same data is a valid definition head for the other definitions in it.
#[test]
fn edit_ns_accepts_definition_heads_from_macros_in_the_same_data() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/test-def-meta.cirru", &snapshot).unwrap();
  let namespace = "test-def-meta.things";
  assert_success(&run_calcit(&snapshot, &["edit", "add-ns", namespace]), "create empty namespace");
  let entries = "    'defthing $ %{} 'CodeEntry (:doc |)\n      :code $ quote $ defmacro defthing (name value) (quasiquote (def ~name ~value))\n      :examples $ []\n      :schema $ :: 'Macro $ {} (:capabilities $ #{}) (:expansion $ :: 'Expr 'Dynamic) (:required $ [] 'Syntax 'Syntax)\n    'my-thing $ %{} 'CodeEntry (:doc |)\n      :code $ quote $ defthing my-thing 1\n      :examples $ []\n";
  let data = |extra: &str| {
    format!(
      "%{{}} 'FileEntry\n  :defs $ {{}}\n{entries}{extra}  :ns $ %{{}} 'NsEntry (:doc |)\n    :code $ quote $ ns test-def-meta.things\n"
    )
  };
  let view = directory.0.join("view.cirru");
  fs::write(&view, data("")).unwrap();
  let output = edit_ns_json(&snapshot, namespace, &view, &[]);
  assert_success(&output, "write data whose definition uses a macro from the same data");
  let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
  let statuses = definition_statuses(&report);
  assert_eq!(statuses["defthing"], "added");
  assert_eq!(statuses["my-thing"], "added");

  // The same-data lookup does not hide a name mismatch.
  fs::write(
    &view,
    data("    'other-thing $ %{} 'CodeEntry (:doc |)\n      :code $ quote $ defthing wrong-name 2\n      :examples $ []\n"),
  )
  .unwrap();
  let mismatch = edit_ns_json(&snapshot, namespace, &view, &[]);
  assert!(!mismatch.status.success());
  assert!(String::from_utf8_lossy(&mismatch.stderr).contains("name mismatch"));
}

#[test]
fn config_add_entry_creates_complete_named_entries_through_guarded_transactions() {
  let directory = TestDirectory::create();
  let snapshot = prepare_minimal_snapshot(&directory);
  let original = fs::read(&snapshot).expect("snapshot should read");

  // Invalid creations fail before writing.
  for args in [
    vec!["config", "add-entry", "default", "--from", "default"],
    vec!["config", "add-entry", "demo", "--mode", "js"],
    vec!["config", "add-entry", "demo", "--from", "missing"],
    vec![
      "config",
      "add-entry",
      "demo",
      "--mode",
      "js",
      "--init-fn",
      "app.main/missing",
      "--reload-fn",
      "app.main/reload!",
    ],
    vec![
      "config",
      "add-entry",
      "demo",
      "--mode",
      "lua",
      "--init-fn",
      "app.main/main!",
      "--reload-fn",
      "app.main/reload!",
    ],
  ] {
    let output = run_calcit(&snapshot, &args);
    assert!(!output.status.success(), "invalid add-entry must fail: {args:?}");
    assert_eq!(
      fs::read(&snapshot).expect("snapshot should read"),
      original,
      "failed add-entry wrote: {args:?}"
    );
  }

  let operations = r#"[["config","add-entry","demo","--mode","js","--target","browser","--init-fn","app.main/main!","--reload-fn","app.main/reload!","--description","Demo page"],["config","add-module","--entry","demo","calcit-test/"],["config","set","--entry","demo","feature-policy.js-ffi","warn"]]"#;
  let dry_run = run_calcit(
    &snapshot,
    &["edit", "transaction", "--code", operations, "--dry-run", "--format", "json"],
  );
  assert_success(&dry_run, "dry-run entry creation");
  let report: serde_json::Value = serde_json::from_slice(&dry_run.stdout).expect("transaction JSON");
  assert_eq!(fs::read(&snapshot).expect("snapshot should read"), original, "dry run wrote");
  let revision = report["original_revision"].as_str().expect("revision").to_owned();

  let stale = run_calcit(
    &snapshot,
    &["edit", "transaction", "--code", operations, "--expect-revision", "md5:stale"],
  );
  assert!(!stale.status.success(), "stale revision must be rejected");
  assert_eq!(
    fs::read(&snapshot).expect("snapshot should read"),
    original,
    "stale transaction wrote"
  );

  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "transaction", "--code", operations, "--expect-revision", &revision],
    ),
    "apply entry creation",
  );
  let show = run_calcit(&snapshot, &["config", "show", "--entry", "demo", "--format", "json"]);
  assert_success(&show, "show new entry");
  let shown: serde_json::Value = serde_json::from_slice(&show.stdout).expect("config JSON");
  let entry = &shown["data"]["entries"][0];
  assert_eq!(entry["name"], "demo");
  assert_eq!(entry["mode"], "js");
  assert_eq!(entry["target"], "browser");
  assert_eq!(entry["init_fn"], "app.main/main!");
  assert_eq!(entry["reload_fn"], "app.main/reload!");
  assert_eq!(entry["description"], "Demo page");
  assert_eq!(entry["modules"], serde_json::json!(["calcit-test/"]));
  assert_eq!(entry["feature_policy"]["js-ffi"], "warn");

  assert_success(
    &run_calcit(
      &snapshot,
      &["config", "add-entry", "demo-copy", "--from", "demo", "--mode", "native"],
    ),
    "clone entry",
  );
  let copy = run_calcit(&snapshot, &["config", "show", "--entry", "demo-copy", "--format", "json"]);
  assert_success(&copy, "show cloned entry");
  let copied: serde_json::Value = serde_json::from_slice(&copy.stdout).expect("config JSON");
  assert_eq!(copied["data"]["entries"][0]["mode"], "native");
  assert_eq!(copied["data"]["entries"][0]["modules"], serde_json::json!(["calcit-test/"]));
  assert_success(
    &run_calcit(
      &snapshot,
      &["config", "add-entry", "plain", "--from", "default", "--description", "Plain copy"],
    ),
    "clone default entry",
  );
  assert_success(
    &run_calcit(&snapshot, &["--entry", "plain", "--check-only"]),
    "cloned entry runs strict checks",
  );
}

#[test]
fn config_package_repairs_local_ownership_through_a_guarded_metadata_transaction() {
  // This exercises the CLI/filesystem boundary, not language semantics.
  let directory = TestDirectory::create();
  let snapshot = prepare_minimal_snapshot(&directory);
  let definition = query_definition(&snapshot, "app.main/main!");
  assert_success(
    &run_calcit(&snapshot, &["config", "set", "package", "legacy"]),
    "legacy package fixture",
  );
  let original = fs::read(&snapshot).unwrap();

  let blocked = run_calcit(&snapshot, &["edit", "doc", "app.main/main!", "local"]);
  assert!(!blocked.status.success());
  assert!(String::from_utf8_lossy(&blocked.stderr).contains("only namespaces under package 'legacy'"));
  for args in [
    vec!["config", "set", "package", ""],
    vec!["config", "set", "package", "app..main"],
    vec!["config", "set", "package", "app/main"],
    vec!["config", "set", "package", "app main"],
    vec!["config", "set", "--entry", "default", "package", "app"],
  ] {
    assert!(!run_calcit(&snapshot, &args).status.success(), "invalid package: {args:?}");
    assert_eq!(fs::read(&snapshot).unwrap(), original);
  }

  let operations = r#"[["config","set","package","app"]]"#;
  let dry_run = run_calcit(
    &snapshot,
    &["edit", "transaction", "--code", operations, "--dry-run", "--format", "json"],
  );
  assert_success(&dry_run, "preview package repair");
  assert_eq!(fs::read(&snapshot).unwrap(), original);
  let report: serde_json::Value = serde_json::from_slice(&dry_run.stdout).unwrap();
  let revision = report["scoped_revision"].as_str().unwrap();
  assert!(revision.starts_with("scope:project@md5:"), "{revision}");
  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "transaction", "--code", operations, "--expect-revision", revision],
    ),
    "commit package repair",
  );
  let repaired = query_definition(&snapshot, "app.main/main!");
  assert_eq!(repaired["data"], definition["data"], "metadata repair must not rewrite definitions");
  assert_success(
    &run_calcit(&snapshot, &["edit", "doc", "app.main/main!", "local"]),
    "edit recovered local source",
  );
  let dependency = run_calcit(&snapshot, &["edit", "doc", "calcit.core/identity", "forbidden"]);
  assert!(!dependency.status.success(), "dependency ownership must remain protected");
  assert!(String::from_utf8_lossy(&dependency.stderr).contains("only namespaces under package 'app'"));
  let stale = run_calcit(
    &snapshot,
    &["edit", "transaction", "--code", operations, "--expect-revision", revision],
  );
  assert!(!stale.status.success(), "old project revision must be rejected");

  let before_rollback = fs::read(&snapshot).unwrap();
  let rollback = run_calcit(
    &snapshot,
    &[
      "edit",
      "transaction",
      "--code",
      r#"[["config","set","package","other"],["config","set","package","bad/name"]]"#,
    ],
  );
  assert!(!rollback.status.success());
  assert_eq!(
    fs::read(&snapshot).unwrap(),
    before_rollback,
    "failed transaction must not commit package changes"
  );
}

#[test]
fn package_metadata_preserves_legacy_file_forms_directly_and_in_transactions() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  let source = r#"#! /usr/bin/env calcit
{} (:package |legacy)
  :entries $ {}
    :default $ {} (:mode :native) (:init-fn |app.main/main!) (:reload-fn |app.main/main!)
      :modules $ []
  :files $ {}
    |app.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry
          :code $ quote $ defn main! () nil
          :schema $ :: :fn $ {} (:args $ []) (:return :any) (:features $ #{})
      :ns $ %{} :CodeEntry (:code $ quote $ ns app.main)
"#;
  let parse = |content: &str| cirru_edn::parse(content.strip_prefix("#! /usr/bin/env calcit\n").unwrap()).unwrap();
  let original = parse(source);
  for args in [
    vec!["config", "set", "package", "app"],
    vec!["edit", "transaction", "--code", r#"[["config","set","package","app"]]"#],
  ] {
    fs::write(&snapshot, source).unwrap();
    assert_success(&run_calcit(&snapshot, &args), "preserve legacy package fixture");
    let output = fs::read_to_string(&snapshot).unwrap();
    let cirru_edn::Edn::Map(after) = parse(&output) else {
      panic!("root map")
    };
    let cirru_edn::Edn::Map(before) = &original else {
      panic!("root map")
    };
    assert_eq!(after.get_or_nil("files"), before.get_or_nil("files"));
    assert_eq!(after.get_or_nil("package"), cirru_edn::Edn::Str("app".into()));
  }
}

/// Legacy parsing and failure-without-write are CLI host boundaries, not language semantics.
#[test]
fn format_drops_reviewed_fields_across_entries_without_writing_on_failure() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  let source = r#"{} (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/main!) (:modules $ [])
  :entries $ {}
    |server.prod $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/main!) ('port 6001) (|:port 6002)
    |worker $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/main!) (:storage-key |worker.cirru)
  :files $ {}
"#;
  fs::write(&snapshot, source).unwrap();
  let rejected = run_calcit(&snapshot, &["edit", "format"]);
  assert!(!rejected.status.success());
  let error = String::from_utf8_lossy(&rejected.stderr);
  for diagnostic in [
    "entries.server.prod: unknown field `:port`",
    "entries.worker: unknown field `:storage-key`",
  ] {
    assert!(error.contains(diagnostic), "{error}");
  }
  assert!(
    error.contains(
      "`calcit <snapshot> edit format --drop-legacy-entry-field server.prod.port --drop-legacy-entry-field worker.storage-key`"
    ),
    "{error}"
  );
  assert_eq!(fs::read_to_string(&snapshot).unwrap(), source);
  for (flags, reason) in [
    (vec!["server.prod.port"], "entries.worker: unknown field `:storage-key`"),
    (vec!["server.prod.port", "worker.missing"], "entry `worker` has no `:missing` field"),
    (vec!["server.prod.:mode"], "is a current entry field"),
    (vec![".port"], "neither part may be empty"),
    (vec!["server.prod."], "neither part may be empty"),
  ] {
    let mut args = vec!["edit", "format"];
    for flag in flags {
      args.extend(["--drop-legacy-entry-field", flag]);
    }
    let rejected = run_calcit(&snapshot, &args);
    assert!(!rejected.status.success(), "{args:?}");
    let diagnostic = String::from_utf8_lossy(&rejected.stderr);
    assert!(diagnostic.contains(reason), "{args:?}: expected {reason}; got {diagnostic}");
    assert_eq!(
      fs::read_to_string(&snapshot).unwrap(),
      source,
      "{args:?}: preserve every original byte"
    );
  }
  let formatted = run_calcit(
    &snapshot,
    &[
      "edit",
      "format",
      "--drop-legacy-entry-field",
      "server.prod.port",
      "--drop-legacy-entry-field",
      "worker.storage-key",
    ],
  );
  assert_success(&formatted, "format every reviewed field");
  let report = String::from_utf8_lossy(&formatted.stdout);
  let mut reported = report
    .lines()
    .filter_map(|line| line.split_once("Dropped legacy entry field ").map(|(_, field_value)| field_value))
    .collect::<Vec<_>>();
  reported.sort();
  assert_eq!(
    reported,
    vec![
      "`server.prod.port`: do 6001",
      "`server.prod.port`: do 6002",
      "`worker.storage-key`: do |worker.cirru",
    ],
    "every original value must remain associated with its field"
  );
  let parsed = cirru_edn::parse(&fs::read_to_string(&snapshot).unwrap()).unwrap();
  let loaded = calcit::snapshot::load_snapshot_data(&parsed, snapshot.to_str().unwrap()).unwrap();
  for entry in ["default", "server.prod", "worker"] {
    assert_eq!(loaded.entries[entry].init_fn, "app.main/main!");
  }
  let before = fs::read(&snapshot).unwrap();
  assert_success(&run_calcit(&snapshot, &["edit", "format"]), "idempotent canonical format");
  assert_eq!(fs::read(&snapshot).unwrap(), before);
}

/// Source repair and atomic persistence are CLI host boundaries, not language tests.
#[test]
fn legacy_macro_schema_repair_keeps_namespace_and_runtime_guards() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  let source = r#"{} (:package |legacy)
  :entries $ {}
    :default $ {} (:mode :native) (:init-fn |app.main/main!) (:reload-fn |app.main/main!)
      :modules $ []
  :files $ {}
    |app.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry
          :code $ quote $ defn main! () (forty-two)
          :schema $ :: :fn $ {} (:args $ []) (:return :number)
        |forty-two $ %{} :CodeEntry
          :code $ quote $ defmacro forty-two () (quote 42)
          :schema $ :: :fn $ {} (:args $ []) (:return :any)
      :ns $ %{} :CodeEntry (:code $ quote $ ns app.main)
"#;
  fs::write(&snapshot, source).unwrap();
  let contract = "quote $ :: 'Macro $ {} (:required $ []) (:expansion $ :: 'Expr 'Number) (:capabilities $ #{})";
  let denied = run_calcit(&snapshot, &["edit", "schema", "app.main/forty-two", "--code", contract]);
  assert!(!denied.status.success());
  assert!(String::from_utf8_lossy(&denied.stderr).contains("only namespaces under package"));
  assert_eq!(fs::read_to_string(&snapshot).unwrap(), source);
  assert!(!run_calcit(&snapshot, &["--check-only"]).status.success());

  let operations = serde_json::to_string(&vec![
    vec!["config", "set", "package", "app"],
    vec!["edit", "schema", "app.main/forty-two", "--code", contract],
  ])
  .unwrap();
  let scope = transaction_scope(&snapshot, &operations);
  assert_eq!(fs::read_to_string(&snapshot).unwrap(), source, "preview must not write");
  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "transaction", "--expect-revision", &scope, "--code", &operations],
    ),
    "guarded macro/package repair",
  );
  let after = fs::read_to_string(&snapshot).unwrap();
  let before_edn = cirru_edn::parse(source).unwrap();
  let after_edn = cirru_edn::parse(&after).unwrap();
  let original = calcit::snapshot::load_snapshot_data_for_metadata(&before_edn, snapshot.to_str().unwrap(), None).unwrap();
  let repaired = calcit::snapshot::load_snapshot_data(&after_edn, snapshot.to_str().unwrap()).unwrap();
  assert_eq!(original.files["app.main"].defs["main!"], repaired.files["app.main"].defs["main!"]);
  assert_eq!(
    original.files["app.main"].defs["forty-two"].code,
    repaired.files["app.main"].defs["forty-two"].code
  );
  assert_success(&run_calcit(&snapshot, &["--check-only"]), "repaired macro runtime gate");

  let invalid = run_calcit(&snapshot, &["edit", "schema", "app.main/forty-two", "--clear"]);
  assert!(!invalid.status.success(), "explicit macro edits must not remove its contract");
  assert_eq!(fs::read_to_string(&snapshot).unwrap(), after);
  fs::write(&snapshot, source).unwrap();
  let incomplete = run_calcit(
    &snapshot,
    &["edit", "transaction", "--code", r#"[["config","set","package","app"]]"#],
  );
  assert!(!incomplete.status.success(), "a transaction must finish with valid macro contracts");
  assert_eq!(fs::read_to_string(&snapshot).unwrap(), source);
}

fn transaction_scope(snapshot: &Path, operations: &str) -> String {
  let dry_run = run_calcit(
    snapshot,
    &["edit", "transaction", "--dry-run", "--format", "json", "--code", operations],
  );
  assert_success(&dry_run, "transaction dry-run");
  let report: serde_json::Value = serde_json::from_slice(&dry_run.stdout).expect("transaction JSON");
  report["scoped_revision"].as_str().expect("scoped revision").to_owned()
}

/// Transactions on different definitions commit in either order with scoped revisions.
#[test]
fn scoped_revisions_let_transactions_on_different_definitions_commit_independently() {
  let directory = TestDirectory::create();
  let snapshot = prepare_minimal_snapshot(&directory);
  let edit = |name: &str, value: &str| {
    format!(r#"[["edit","def","app.main/{name}","--overwrite","--input-format","cirru","--code","quote $ defn {name} () {value}"]]"#)
  };
  for name in ["first", "second"] {
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &format!("app.main/{name}"),
          "--input-format",
          "cirru",
          "--code",
          &format!("quote $ defn {name} () 0"),
        ],
      ),
      "create definition",
    );
  }
  let first = transaction_scope(&snapshot, &edit("first", "1"));
  let second = transaction_scope(&snapshot, &edit("second", "2"));
  assert_eq!(first.matches('@').count(), 1, "scope covers only the touched definition: {first}");
  assert!(first.starts_with("scope:def:app.main/first@md5:"), "{first}");

  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "transaction", "--expect-revision", &second, "--code", &edit("second", "2")],
    ),
    "commit second",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "transaction", "--expect-revision", &first, "--code", &edit("first", "1")],
    ),
    "commit first after an unrelated commit",
  );

  // The same definition changed since the preview: the later commit gets a conflict naming it.
  let stale = run_calcit(
    &snapshot,
    &["edit", "transaction", "--expect-revision", &first, "--code", &edit("first", "3")],
  );
  assert!(!stale.status.success());
  let stderr = String::from_utf8_lossy(&stale.stderr);
  assert!(stderr.contains("Definition conflict: def:app.main/first"), "{stderr}");

  // A scope cannot be widened by different operations.
  let widened = run_calcit(
    &snapshot,
    &[
      "edit",
      "transaction",
      "--expect-revision",
      &transaction_scope(&snapshot, &edit("first", "4")),
      "--code",
      &edit("second", "4"),
    ],
  );
  assert!(!widened.status.success());
  assert!(String::from_utf8_lossy(&widened.stderr).contains("def:app.main/second"));

  // Whole-Snapshot revisions keep working.
  let dry_run = run_calcit(
    &snapshot,
    &[
      "edit",
      "transaction",
      "--dry-run",
      "--format",
      "json",
      "--code",
      &edit("first", "5"),
    ],
  );
  let report: serde_json::Value = serde_json::from_slice(&dry_run.stdout).unwrap();
  let revision = report["original_revision"].as_str().unwrap();
  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "transaction", "--expect-revision", revision, "--code", &edit("first", "5")],
    ),
    "full revision",
  );
}

fn git(directory: &Path, args: &[&str]) -> Output {
  Command::new("git")
    .arg("-C")
    .arg(directory)
    .args(["-c", "user.name=calcit", "-c", "user.email=calcit@example.com"])
    .args(args)
    .output()
    .expect("git should run")
}

/// The Git merge driver merges per definition and reports same-definition conflicts.
#[test]
fn merge_driver_merges_definitions_and_reports_same_definition_conflicts() {
  let directory = TestDirectory::create();
  let snapshot = prepare_minimal_snapshot(&directory);
  assert_success(&run_calcit(&snapshot, &["edit", "format"]), "canonical base");
  let canonical = fs::read(&snapshot).unwrap();

  // Merging identical versions keeps the canonical bytes.
  let copy = directory.0.join("copy.cirru");
  fs::copy(&snapshot, &copy).unwrap();
  let copy_arg = copy.to_str().unwrap();
  assert_success(
    &run_calcit(&snapshot, &["edit", "merge", "--base", copy_arg, "--theirs", copy_arg]),
    "no-op merge",
  );
  assert_eq!(fs::read(&snapshot).unwrap(), canonical, "no-op merge must keep the Snapshot bytes");

  let root = &directory.0;
  assert!(git(root, &["init", "-q", "-b", "main"]).status.success());
  fs::write(root.join(".gitattributes"), "calcit.cirru merge=calcit\n").unwrap();
  let driver = format!(
    "{} --tips-level none %A edit merge --base %O --theirs %B",
    env!("CARGO_BIN_EXE_calcit")
  );
  assert!(git(root, &["config", "merge.calcit.driver", &driver]).status.success());
  assert!(git(root, &["add", "calcit.cirru", ".gitattributes"]).status.success());
  assert!(git(root, &["commit", "-qm", "base"]).status.success());

  let add = |name: &str, value: &str| {
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &format!("app.main/{name}"),
          "--overwrite",
          "--input-format",
          "cirru",
          "--code",
          &format!("quote $ defn {name} () {value}"),
        ],
      ),
      "edit definition",
    );
  };
  // Both branches add a definition at the same text position, which plain Git reports as a conflict.
  assert!(git(root, &["switch", "-qc", "other"]).status.success());
  add("added-b", "2");
  assert!(git(root, &["commit", "-qam", "other"]).status.success());
  assert!(git(root, &["switch", "-q", "main"]).status.success());
  add("added-a", "1");
  assert!(git(root, &["commit", "-qam", "mine"]).status.success());
  let merged = git(root, &["merge", "--no-edit", "other"]);
  assert!(merged.status.success(), "{}", String::from_utf8_lossy(&merged.stderr));
  for name in ["added-a", "added-b"] {
    query_definition(&snapshot, &format!("app.main/{name}"));
  }
  assert_success(&run_calcit(&snapshot, &["edit", "format"]), "merged result is canonical");

  // Both branches change the same definition: Git stops with a definition-level conflict.
  assert!(git(root, &["switch", "-qc", "conflict"]).status.success());
  add("added-a", "30");
  assert!(git(root, &["commit", "-qam", "theirs"]).status.success());
  assert!(git(root, &["switch", "-q", "main"]).status.success());
  add("added-a", "10");
  assert!(git(root, &["commit", "-qam", "ours"]).status.success());
  let conflict = git(root, &["merge", "--no-edit", "conflict"]);
  assert!(!conflict.status.success(), "same-definition edits must conflict");
  let report = format!(
    "{}{}",
    String::from_utf8_lossy(&conflict.stdout),
    String::from_utf8_lossy(&conflict.stderr)
  );
  assert!(report.contains("def:app.main/added-a"), "{report}");
  let code = query_definition(&snapshot, "app.main/added-a")["data"]["code"].clone();
  assert_eq!(code[3], "10", "the conflicted file keeps our version");
}

/// A namespace deletion conflicts with surviving definitions, but an unchanged namespace can be removed.
#[test]
fn merge_driver_reports_namespace_deletion_conflicts() {
  for change in ["added", "changed", "unchanged"] {
    let directory = TestDirectory::create();
    let snapshot = prepare_minimal_snapshot(&directory);
    assert_success(&run_calcit(&snapshot, &["edit", "add-ns", "app.extra"]), "add namespace");
    let define = |target: &str, code: &str| {
      assert_success(
        &run_calcit(
          &snapshot,
          &["edit", "def", target, "--overwrite", "--input-format", "cirru", "--code", code],
        ),
        "edit definition",
      );
    };
    define("app.extra/original", "quote $ defn original () 1");
    let root = &directory.0;
    assert!(git(root, &["init", "-q", "-b", "main"]).status.success());
    fs::write(root.join(".gitattributes"), "calcit.cirru merge=calcit\n").unwrap();
    let driver = format!(
      "{} --tips-level none %A edit merge --base %O --theirs %B",
      env!("CARGO_BIN_EXE_calcit")
    );
    assert!(git(root, &["config", "merge.calcit.driver", &driver]).status.success());
    assert!(git(root, &["add", "calcit.cirru", ".gitattributes"]).status.success());
    assert!(git(root, &["commit", "-qm", "base"]).status.success());
    assert!(git(root, &["switch", "-qc", "delete-namespace"]).status.success());
    assert_success(&run_calcit(&snapshot, &["edit", "rm-ns", "app.extra"]), "delete namespace");
    assert!(git(root, &["commit", "-qam", "remove namespace"]).status.success());
    assert!(git(root, &["switch", "-q", "main"]).status.success());
    // Make both branches diverge even when this namespace is unchanged.
    define("app.main/independent", "quote $ defn independent () 3");
    match change {
      "added" => define("app.extra/added", "quote $ defn added () 2"),
      "changed" => define("app.extra/original", "quote $ defn original () 2"),
      _ => {}
    }
    assert!(git(root, &["commit", "-qam", "ours"]).status.success());
    let merged = git(root, &["merge", "--no-edit", "delete-namespace"]);
    let report = format!(
      "{}{}",
      String::from_utf8_lossy(&merged.stdout),
      String::from_utf8_lossy(&merged.stderr)
    );
    query_definition(&snapshot, "app.main/independent");
    if change == "unchanged" {
      assert!(merged.status.success(), "unchanged namespace must be removable: {report}");
      assert!(!run_calcit(&snapshot, &["query", "ns", "app.extra"]).status.success());
    } else {
      assert!(
        !merged.status.success(),
        "namespace deletion must conflict with {change} definitions: {report}"
      );
      assert!(report.contains("ns:app.extra"), "namespace conflict must be reported: {report}");
      let target = if change == "added" {
        "app.extra/added"
      } else {
        "app.extra/original"
      };
      assert_eq!(query_definition(&snapshot, target)["data"]["code"][3], "2");
      if change == "changed" {
        assert!(
          report.contains("def:app.extra/original"),
          "definition conflict must remain visible: {report}"
        );
      }
      let unmerged = git(root, &["diff", "--name-only", "--diff-filter=U"]);
      assert_eq!(String::from_utf8_lossy(&unmerged.stdout).trim(), "calcit.cirru");
    }
  }
}
