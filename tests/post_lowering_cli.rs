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
