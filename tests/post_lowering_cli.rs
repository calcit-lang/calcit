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
}
