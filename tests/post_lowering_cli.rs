//! Valid programs for the constructs behind past lowering regressions (#1553),
//! checked with the post-lowering validator switched on. The validator itself
//! is exercised on hand-built broken trees in
//! `src/runner/preprocess/post_lowering.rs`.
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
    let path = std::env::temp_dir().join(format!("calcit-post-lowering-cli-{}-{nonce}-{counter}", std::process::id()));
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
    .env("CALCIT_LINT_CORE", "1")
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

fn define(snapshot: &Path, name: &str, code: &str, schema: &str, overwrite: bool) {
  let target = format!("app.main/{name}");
  let mut args = vec!["edit", "def", target.as_str(), "--code", code];
  if overwrite {
    args.push("--overwrite");
  }
  assert_success(&run_calcit(snapshot, &args), "edit definition");
  assert_success(
    &run_calcit(snapshot, &["edit", "schema", target.as_str(), "--code", schema]),
    "edit schema",
  );
}

fn guarded_transaction(snapshot: &Path, operations: serde_json::Value) {
  let code = operations.to_string();
  let preview = run_calcit(snapshot, &["edit", "transaction", "--code", &code, "--dry-run", "--format", "json"]);
  assert_success(&preview, "preview CLI regression setup");
  let report: serde_json::Value = serde_json::from_slice(&preview.stdout).expect("transaction preview should be one JSON document");
  let revision = report["scoped_revision"].as_str().expect("preview should guard its scope");
  assert_success(
    &run_calcit(
      snapshot,
      &[
        "edit",
        "transaction",
        "--code",
        &code,
        "--expect-revision",
        revision,
        "--format",
        "json",
      ],
    ),
    "apply guarded CLI regression setup",
  );
}

#[test]
fn selected_entry_queries_and_host_method_results_keep_independent_evidence() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/add.cirru", &snapshot).expect("minimal source should copy");
  guarded_transaction(
    &snapshot,
    serde_json::json!([
      [
        "edit",
        "def",
        "app.main/NodeHost",
        "--input-format",
        "cirru",
        "--code",
        "quote $ deftrait NodeHost\n  .text $ :: 'Fn $ {} (:args $ [] 'NodeHost) (:return 'String)\n  .number $ :: 'Fn $ {} (:args $ [] 'NodeHost 'Number) (:return 'Number)\n  .open $ :: 'Fn $ {} (:args $ [] 'NodeHost) (:return 'Dynamic)"
      ],
      [
        "edit",
        "ffi",
        "app.main/NodeHost",
        "--code",
        "{} (:backend :js) (:kind :external-object) (:target :node)"
      ],
      [
        "edit",
        "def",
        "app.main/read-text",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn read-text (host) (host .text)"
      ],
      [
        "edit",
        "schema",
        "app.main/read-text",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'app.main/NodeHost) (:return 'String) (:features $ #{} :js-ffi)"
      ],
      [
        "edit",
        "def",
        "app.main/read-text-prefix",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn read-text-prefix (host) (.text host)"
      ],
      [
        "edit",
        "schema",
        "app.main/read-text-prefix",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'app.main/NodeHost) (:return 'String) (:features $ #{} :js-ffi)"
      ],
      [
        "edit",
        "def",
        "app.main/main!",
        "--overwrite",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn main! () &unit"
      ],
      [
        "edit",
        "schema",
        "app.main/main!",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)"
      ],
      ["config", "set", "reload-fn", "app.main/main!"],
      ["config", "set", "mode", "js"],
      ["config", "set", "target", "browser"],
      ["config", "add-entry", "server", "--from", "default", "--target", "node"]
    ]),
  );
  let before = fs::read(&snapshot).expect("read source before queries");
  let browser = run_calcit(
    &snapshot,
    &["query", "type-at", "app.main/read-text", "--path", "3", "--format", "json"],
  );
  assert_success(&browser, "default target query emits its diagnostic envelope");
  assert!(String::from_utf8_lossy(&browser.stdout).contains("E_JS_FFI_TARGET_MISMATCH"));
  for definition in ["read-text", "read-text-prefix"] {
    let target = format!("app.main/{definition}");
    for format in ["json", "edn"] {
      let queried = run_calcit(
        &snapshot,
        &["--entry", "server", "query", "type-at", &target, "--path", "3", "--format", format],
      );
      assert_success(&queried, "selected Node target query");
      if format == "json" {
        let report: serde_json::Value = serde_json::from_slice(&queried.stdout).expect("one JSON query document");
        assert_eq!(report["data"]["inferred_type"], "'String");
        assert_eq!(report["diagnostics"], serde_json::json!([]));
      } else {
        let text = String::from_utf8_lossy(&queried.stdout);
        cirru_edn::parse(&text).expect("one EDN query document");
        assert!(text.contains(":inferred-type |'String"), "{text}");
      }
    }
    let proven = run_calcit(
      &snapshot,
      &[
        "--entry",
        "server",
        "fix",
        "--rule",
        "concrete-return-proof-v1",
        "--ns",
        "app.main",
        "--def",
        definition,
        "--format",
        "json",
      ],
    );
    assert_success(&proven, "host method supplies independent return evidence");
    let report: serde_json::Value = serde_json::from_slice(&proven.stdout).expect("one proof document");
    assert_eq!(report["diagnostics"], serde_json::json!([]));
    assert_eq!(report["data"]["suggestions"], serde_json::json!([]));
  }
  for command in ["type", "context"] {
    let selected = run_calcit(
      &snapshot,
      &["--entry", "server", "query", command, "app.main/read-text", "--format", "json"],
    );
    assert_success(&selected, "semantic queries inherit the selected entry");
    serde_json::from_slice::<serde_json::Value>(&selected.stdout).expect("one semantic query document");
    assert!(!String::from_utf8_lossy(&selected.stdout).contains("E_JS_FFI_TARGET_MISMATCH"));
    let missing = run_calcit(
      &snapshot,
      &["--entry", "missing", "query", command, "app.main/read-text", "--format", "json"],
    );
    assert!(!missing.status.success(), "invalid entries must not silently select default");
    serde_json::from_slice::<serde_json::Value>(&missing.stdout).expect("one structured failure document");
  }
  assert_eq!(
    fs::read(&snapshot).unwrap(),
    before,
    "queries and proof previews must not write source"
  );

  for (name, code, schema) in [
    (
      "open-result",
      "quote $ defn open-result (host) (host .open)",
      "quote $ :: 'Fn $ {} (:args $ [] 'app.main/NodeHost) (:return 'String) (:features $ #{} :js-ffi)",
    ),
    (
      "wrong-input",
      "quote $ defn wrong-input (host) (host .number |bad)",
      "quote $ :: 'Fn $ {} (:args $ [] 'app.main/NodeHost) (:return 'Number) (:features $ #{} :js-ffi)",
    ),
    (
      "wrong-arity",
      "quote $ defn wrong-arity (host) (host .text 1)",
      "quote $ :: 'Fn $ {} (:args $ [] 'app.main/NodeHost) (:return 'String) (:features $ #{} :js-ffi)",
    ),
    (
      "open-receiver",
      "quote $ defn open-receiver (host) (host .text)",
      "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return 'String) (:features $ #{} :js-ffi)",
    ),
    (
      "missing-feature",
      "quote $ defn missing-feature (host) (host .text)",
      "quote $ :: 'Fn $ {} (:args $ [] 'app.main/NodeHost) (:return 'String)",
    ),
    (
      "rigid-caller",
      "quote $ defn rigid-caller (items) (items .append 1)",
      "quote $ :: 'Fn $ {} (:args $ [] (:: 'List 'T)) (:return $ :: 'List 'T) (:generics $ [] 'T)",
    ),
  ] {
    define(&snapshot, name, code, schema, false);
    let unchanged = fs::read(&snapshot).unwrap();
    let rejected = run_calcit(
      &snapshot,
      &[
        "--entry",
        "server",
        "fix",
        "--rule",
        "concrete-return-proof-v1",
        "--ns",
        "app.main",
        "--def",
        name,
        "--format",
        "json",
      ],
    );
    assert!(!rejected.status.success(), "{name} must not gain false method return evidence");
    assert_eq!(fs::read(&snapshot).unwrap(), unchanged, "{name}: no mutation");
  }
}

#[test]
fn semantic_query_entry_selects_modules_slots_policy_and_search_override() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/js-ffi-consumer.cirru", &snapshot).expect("consumer should copy");
  let module = directory.0.join("js-ffi-module");
  fs::create_dir_all(module.join("js-ffi-assets")).unwrap();
  fs::copy("calcit/js-ffi-module/calcit.cirru", module.join("calcit.cirru")).unwrap();
  for file in ["add-two.js", "count.js", "base-name.js", "counter-host.js"] {
    fs::copy(
      Path::new("calcit/js-ffi-module/js-ffi-assets").join(file),
      module.join("js-ffi-assets").join(file),
    )
    .unwrap();
  }
  guarded_transaction(
    &snapshot,
    serde_json::json!([
      ["config", "set", "reload-fn", "test-nil.main/main!"],
      ["config", "add-entry", "isolated", "--from", "default", "--target", "browser"],
      ["config", "rm-module", "--entry", "isolated", "./js-ffi-module/"],
      [
        "edit",
        "def",
        "test-nil.main/NumberBox",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defstruct NumberBox (:value 'Number)"
      ],
      [
        "edit",
        "def",
        "test-nil.main/TextBox",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defstruct TextBox (:value 'String)"
      ],
      [
        "edit",
        "def",
        "test-nil.main/query-slot",
        "--input-format",
        "cirru",
        "--code",
        "quote $ deftype-slot :entry-value"
      ],
      [
        "edit",
        "def",
        "test-nil.main/slot-value",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn slot-value (value) , value"
      ],
      [
        "edit",
        "schema",
        "test-nil.main/slot-value",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] '*entry-value) (:return 'test-nil.main/NumberBox)"
      ],
      ["config", "set-type-slot", ":entry-value", "test-nil.main/NumberBox"],
      [
        "config",
        "set-type-slot",
        "--entry",
        "isolated",
        ":entry-value",
        "test-nil.main/TextBox"
      ],
      [
        "edit",
        "def",
        "test-nil.main/raw-host",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn raw-host () js/globalThis"
      ],
      [
        "edit",
        "schema",
        "test-nil.main/raw-host",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return $ :: 'JsNullish 'JsObject)"
      ],
      ["config", "set", "feature-policy.js-ffi", "allow"],
      ["config", "set", "--entry", "isolated", "feature-policy.js-ffi", "error"]
    ]),
  );
  let before = fs::read(&snapshot).unwrap();
  // The same symbolic slot must satisfy NumberBox only under the default entry.
  for entry in ["default", "isolated"] {
    let result = run_calcit(
      &snapshot,
      &[
        "--entry",
        entry,
        "query",
        "type-at",
        "test-nil.main/slot-value",
        "--path",
        "3",
        "--format",
        "json",
      ],
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_success(&result, "selected slot query emits one diagnostic envelope");
    if entry == "default" {
      assert_eq!(report["diagnostics"], serde_json::json!([]));
    } else {
      assert!(
        report["diagnostics"]
          .as_array()
          .is_some_and(|items| items.iter().any(|item| item["code"] == "W_FN_RETURN_TYPE_MISMATCH")),
        "the other slot binding must not satisfy NumberBox: {report}"
      );
    }
    let policy = run_calcit(
      &snapshot,
      &[
        "--entry",
        entry,
        "query",
        "type-at",
        "test-nil.main/raw-host",
        "--path",
        "3",
        "--format",
        "json",
      ],
    );
    assert_success(&policy, "policy query has one diagnostic envelope");
    let policy: serde_json::Value = serde_json::from_slice(&policy.stdout).unwrap();
    if entry == "default" {
      assert!(
        policy["diagnostics"]
          .as_array()
          .is_some_and(|items| items.iter().all(|item| item["code"] == "W_TYPE_AT_UNRESOLVED")),
        "{policy}"
      );
    } else {
      assert!(
        policy["diagnostics"]
          .as_array()
          .is_some_and(|items| items.iter().any(|item| item["code"] == "E_JS_FFI_FEATURE_REQUIRED")),
        "{policy}"
      );
    }
  }
  for command in ["type", "context"] {
    let available = run_calcit(&snapshot, &["query", command, "app.main/counter-add", "--format", "json"]);
    assert_success(&available, "default entry loads its dependency");
    let absent = run_calcit(
      &snapshot,
      &["--entry", "isolated", "query", command, "app.main/counter-add", "--format", "json"],
    );
    assert!(!absent.status.success(), "isolated entry must not inherit default modules");
    let report: serde_json::Value = serde_json::from_slice(&absent.stdout).unwrap();
    assert_eq!(report["diagnostics"][0]["code"], "E_QUERY_TARGET_NOT_FOUND");
  }
  for (top, local, expected) in [
    ("default", None, 1),
    ("isolated", None, 0),
    ("isolated", Some("default"), 1),
    ("default", Some("isolated"), 0),
  ] {
    let mut args = vec![
      "--entry",
      top,
      "query",
      "search",
      "counter-add",
      "--source",
      "deps",
      "--exact",
      "--format",
      "json",
    ];
    if let Some(local) = local {
      args.extend(["--entry", local]);
    }
    let found = run_calcit(&snapshot, &args);
    assert_success(&found, "search inherits top-level entry unless locally overridden");
    let report: serde_json::Value = serde_json::from_slice(&found.stdout).unwrap();
    assert_eq!(
      report["data"]["summary"]["matches"].as_u64().is_some_and(|count| count > 0),
      expected > 0,
      "{report}"
    );
  }
  assert_eq!(fs::read(&snapshot).unwrap(), before, "entry-scoped queries do not write source");
}

#[test]
fn past_lowering_regressions_pass_post_lowering_validation() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  fs::copy("calcit/add.cirru", &snapshot).expect("minimal snapshot fixture should copy");
  assert_success(&run_calcit(&snapshot, &["config", "set", "target", "native"]), "set native target");

  // Nominal declarations carry no function schema.
  for (name, code) in [
    ("Node", "quote $ defstruct Node (:name 'String)"),
    ("Component", "quote $ defstruct Component (:tree (:: 'Option 'Node))"),
  ] {
    let target = format!("app.main/{name}");
    assert_success(
      &run_calcit(&snapshot, &["edit", "def", target.as_str(), "--code", code]),
      "edit struct",
    );
  }

  // #1737: a payload-free `Option :none` bound to a local still proves a concrete field.
  define(
    &snapshot,
    "build-component",
    "quote $ defn build-component () $ let ((absent-tree $ Option :none)) (Component :tree absent-tree)",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'app.main/Component)",
    false,
  );
  // #1378: typed `get` lowering keeps the `Option<Node>` payload.
  define(
    &snapshot,
    "find-node",
    "quote $ defn find-node (nodes) $ get nodes :a",
    "quote $ :: 'Fn $ {} (:args $ [] (:: 'Map 'Tag 'app.main/Node)) (:return $ :: 'Option 'app.main/Node)",
    false,
  );
  // #1494: an inlined builtin method keeps its argument checks and `Map<Tag, Number>` result.
  define(
    &snapshot,
    "add-key",
    "quote $ defn add-key (m) $ m .assoc :b 2",
    "quote $ :: 'Fn $ {} (:args $ [] (:: 'Map 'Tag 'Number)) (:return $ :: 'Map 'Tag 'Number)",
    false,
  );
  // #2068: open descendants do not erase a proven root Map or its key type.
  for (name, code, schema) in [
    (
      "write-open-map",
      "quote $ defn write-open-map (db) $ assoc-in db ([] :repo :current) |main",
      "quote $ :: 'Fn $ {} (:args $ [] (:: 'Map 'Tag 'Dynamic)) (:return $ :: 'Map 'Tag 'Dynamic)",
    ),
    (
      "write-generic-key",
      "quote $ defn write-generic-key (db key) $ assoc-in db ([] key :current) |main",
      "quote $ :: 'Fn $ {} (:args $ [] (:: 'Map 'K 'Dynamic) 'K) (:return $ :: 'Map 'K 'Dynamic) (:generics $ [] 'K)",
    ),
    (
      "write-nested-prefix",
      "quote $ defn write-nested-prefix (db) $ assoc-in db ([] :repo |current :value) |main",
      "quote $ :: 'Fn $ {} (:args $ [] (:: 'Map 'Tag (:: 'Map 'String 'Dynamic))) (:return $ :: 'Map 'Tag (:: 'Map 'String 'Dynamic))",
    ),
    (
      "update-open-map",
      "quote $ defn update-open-map (db) $ update-in db ([] :current) $ fn (value) (hint-fn $ {} (:args $ [] (:: 'Option 'Dynamic)) (:return 'String)) |main",
      "quote $ :: 'Fn $ {} (:args $ [] (:: 'Map 'Tag 'Dynamic)) (:return $ :: 'Map 'Tag 'Dynamic)",
    ),
  ] {
    define(&snapshot, name, code, schema, false);
  }
  // #1428: `recur` follows the lexical function contract.
  define(
    &snapshot,
    "typed-recur",
    "quote $ defn typed-recur (label index) $ if (>= index 2) index $ recur label (inc index)",
    "quote $ :: 'Fn $ {} (:args $ [] 'String 'Number) (:return 'Number)",
    false,
  );
  define(
    &snapshot,
    "main!",
    "quote $ defn main! () $ typed-recur |point 0",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)",
    true,
  );
  define(
    &snapshot,
    "reload!",
    "quote $ defn reload! () &unit",
    "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)",
    false,
  );

  assert_success(&run_calcit(&snapshot, &["--check-only"]), "entry check with validator");
  assert_success(
    &run_calcit(&snapshot, &["--check-only", "--ns", "app.main"]),
    "every definition passes the validator",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &["fix", "--rule", "concrete-return-proof-v1", "--ns", "app.main", "--format", "json"],
    ),
    "root and generic Map results have independent return evidence",
  );
}

#[test]
fn open_path_write_proof_does_not_invent_receiver_shapes() {
  let directory = TestDirectory::create();
  for (label, body, arguments, returned) in [
    (
      "empty",
      "assoc-in db ([]) |changed",
      "(:: 'Map 'Tag 'Dynamic) (:: 'List 'Tag) 'Tag",
      "(:: 'Map 'Tag 'Dynamic)",
    ),
    (
      "unknown",
      "assoc-in db path |changed",
      "(:: 'Map 'Tag 'Dynamic) (:: 'List 'Tag) 'Tag",
      "(:: 'Map 'Tag 'Dynamic)",
    ),
    (
      "wrong-key",
      "assoc-in db ([] |wrong) |changed",
      "(:: 'Map 'Tag 'Dynamic) (:: 'List 'Tag) 'Tag",
      "(:: 'Map 'Tag 'Dynamic)",
    ),
    (
      "wrong-leaf",
      "assoc-in db ([] :count) |changed",
      "(:: 'Map 'Tag 'Number) (:: 'List 'Tag) 'Tag",
      "(:: 'Map 'Tag 'Number)",
    ),
    (
      "scalar-hop",
      "assoc-in db ([] :count :value) 1",
      "(:: 'Map 'Tag 'Number) (:: 'List 'Tag) 'Tag",
      "(:: 'Map 'Tag 'Number)",
    ),
    (
      "open-root",
      "assoc-in db ([] :count) 1",
      "'Dynamic (:: 'List 'Tag) 'Tag",
      "(:: 'Map 'Tag 'Dynamic)",
    ),
    (
      "unknown-key",
      "assoc-in db ([] key) 1",
      "(:: 'Map 'Tag 'Dynamic) (:: 'List 'Tag) 'Dynamic",
      "(:: 'Map 'Tag 'Dynamic)",
    ),
    (
      "known-prefix-family",
      "assoc-in db ([] :repo) |changed",
      "(:: 'Map 'Tag (:: 'Map 'String 'Dynamic)) (:: 'List 'Tag) 'Tag",
      "(:: 'Map 'Tag (:: 'Map 'String 'Dynamic))",
    ),
    (
      "empty-update",
      "update-in db ([]) $ fn (value) (hint-fn $ {} (:args $ [] (:: 'Option 'Dynamic)) (:return 'String)) |changed",
      "(:: 'Map 'Tag 'Dynamic) (:: 'List 'Tag) 'Tag",
      "(:: 'Map 'Tag 'Dynamic)",
    ),
  ] {
    let snapshot = directory.snapshot();
    fs::copy("calcit/add.cirru", &snapshot).expect("minimal snapshot fixture should copy");
    define(
      &snapshot,
      "rejected",
      &format!("quote $ defn rejected (db path key) ({body})"),
      &format!("quote $ :: 'Fn $ {{}} (:args $ [] {arguments}) (:return {returned})"),
      false,
    );
    let before = fs::read(&snapshot).expect("read source before proof");
    let result = run_calcit(
      &snapshot,
      &[
        "fix",
        "--rule",
        "concrete-return-proof-v1",
        "--ns",
        "app.main",
        "--def",
        "rejected",
        "--format",
        "json",
      ],
    );
    assert!(!result.status.success(), "{label} must not acquire a concrete Map result");
    let output = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    if label == "empty-update" {
      assert!(
        stderr.contains("W_FN_RETURN_TYPE_MISMATCH") && stderr.contains("body returns `:string`"),
        "{label}: {output}\n{stderr}"
      );
    } else {
      assert!(output.contains("E_FN_RETURN_UNPROVEN"), "{label}: {output}\n{stderr}");
    }
    assert_eq!(
      fs::read(&snapshot).expect("read source after proof"),
      before,
      "{label} must not mutate source"
    );
  }
}
