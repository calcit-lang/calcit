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
    let path = std::env::temp_dir().join(format!("calcit-mutation-markdown-{}-{nonce}-{counter}", std::process::id()));
    fs::create_dir(&path).expect("temporary project should create");
    Self(path)
  }

  fn snapshot(&self) -> PathBuf {
    let snapshot = self.0.join("calcit.cirru");
    fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
    snapshot
  }
}

impl Drop for TestDirectory {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

fn run_calcit(args: &[&str]) -> Output {
  run_snapshot(Path::new("calcit/test.cirru"), args)
}

fn run_snapshot(snapshot: &Path, args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .arg(snapshot)
    .args(args)
    .output()
    .expect("calcit command should run")
}

fn stdout(output: &Output) -> String {
  assert!(
    output.status.success(),
    "command failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8(output.stdout.clone()).expect("stdout should be UTF-8")
}

#[test]
fn code_bearing_read_commands_use_markdown_fences() {
  let definition = stdout(&run_calcit(&["query", "def", "app.main/main!"]));
  assert!(definition.starts_with("# Definition `app.main/main!`\n"));
  assert!(definition.contains("## Schema\n\n```cirru\n"));
  assert!(definition.contains("## Cirru\n\n```cirru\n"));
  assert!(!definition.contains("Command:"), "command echo must remain on stderr");

  let context = stdout(&run_calcit(&["query", "context", "app.main/main!", "--budget", "1200"]));
  assert!(context.starts_with("# Definition context `app.main/main!`\n"));
  assert!(context.contains("## Code preview\n"));
  assert!(context.contains("```cirru\n"));

  let tree = stdout(&run_calcit(&["tree", "show", "app.main/main!", "--depth", "2"]));
  assert!(tree.starts_with("# Tree node `app.main/main!`\n"));
  assert!(tree.contains("## Cirru preview\n\n```cirru\n"));

  let examples = stdout(&run_calcit(&["query", "examples", "calcit.core/let"]));
  assert!(examples.starts_with("# Examples for `calcit.core/let`\n"));
  assert!(examples.contains("```cirru\n"));
  assert!(examples.contains("### JSON AST\n\n```json\n"));

  let search = stdout(&run_calcit(&[
    "query",
    "search",
    "defn",
    "--filter",
    "app.main/main!",
    "--format",
    "human",
  ]));
  assert!(search.starts_with("# Search results\n"));
  assert!(search.contains("### Match #0\n"));
  assert!(search.contains("```cirru\n"));
}

#[test]
fn json_query_contract_remains_one_clean_value() {
  let output = run_calcit(&["query", "def", "app.main/main!", "--format", "json"]);
  let value: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("JSON stdout should stay parseable");
  assert_eq!(value["command"], "query.def");
}

#[test]
fn trait_leaf_example_context_is_one_cirru_edn_envelope() {
  let output = run_snapshot(
    Path::new("calcit/test-traits.cirru"),
    &["query", "context", "test-traits.external/CounterOps", "--format", "edn"],
  );
  let text = stdout(&output);
  let parsed = cirru_edn::parse(&text).expect("context stdout should contain one Cirru EDN envelope");
  let cirru_edn::Edn::Map(root) = parsed else {
    panic!("context envelope should be a map");
  };
  assert_eq!(
    root.get(&cirru_edn::Edn::tag("command")),
    Some(&cirru_edn::Edn::str("query.context"))
  );
  assert!(
    text.contains("(:cirru |CounterOps)"),
    "trait leaf example should be rendered: {text}"
  );
}

#[test]
fn code_bearing_tree_diagnostic_stays_on_stderr_with_a_fence() {
  let output = run_calcit(&["tree", "show", "app.main/main!", "--path", "999"]);
  assert!(!output.status.success());
  assert!(output.stdout.is_empty());
  let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
  assert!(stderr.contains("# Error: invalid path\n"));
  assert!(stderr.contains("## Node at longest valid path\n\n```cirru\n"));
}

#[test]
fn mutation_and_cursor_apply_share_markdown_source_boundaries() {
  let directory = TestDirectory::create();
  let snapshot = directory.snapshot();
  let mutation = run_snapshot(
    &snapshot,
    &[
      "tree",
      "replace",
      "fix-command.main/fixable",
      "--path",
      "3",
      "--input-format",
      "cirru",
      "--code",
      "quote $ enum-definition value",
    ],
  );
  let mutation_stdout = stdout(&mutation);
  assert!(mutation_stdout.contains("# Tree mutation\n"), "stdout:\n{mutation_stdout}");
  assert!(mutation_stdout.contains("## Before\n\n- node kind: `expression`\n\n```cirru\n"));
  assert!(mutation_stdout.contains("## After\n\n- node kind: `expression`\n\n```cirru\n"));

  let guard = run_snapshot(
    &snapshot,
    &[
      "tree",
      "replace",
      "fix-command.main/fixable",
      "--path",
      "3",
      "--expect",
      "quote $ tuple-enum wrong",
      "--code",
      "quote $ tuple-enum value",
    ],
  );
  assert!(!guard.status.success());
  let guard_stderr = String::from_utf8(guard.stderr).expect("guard stderr should be UTF-8");
  assert!(guard_stderr.contains("# Error: node guard failed\n"));
  assert!(guard_stderr.contains("## Expected\n\n- node kind: `expression`\n\n```cirru\n"));
  assert!(guard_stderr.contains("## Actual\n\n- node kind: `expression`\n\n```cirru\n"));

  let cursor_directory = TestDirectory::create();
  let cursor_snapshot = cursor_directory.snapshot();
  let set = run_snapshot(&cursor_snapshot, &["cursor", "set", "fix-command.main/fixable", "--path", "3"]);
  assert!(set.status.success(), "stderr:\n{}", String::from_utf8_lossy(&set.stderr));
  let applied = run_snapshot(
    &cursor_snapshot,
    &[
      "cursor",
      "apply",
      "replace",
      "--input-format",
      "cirru",
      "--code",
      "quote $ enum-definition value",
    ],
  );
  let applied_stdout = stdout(&applied);
  assert!(applied_stdout.contains("# Tree mutation\n"));
  assert!(applied_stdout.contains("## Before\n\n- node kind: `expression`\n\n```cirru\n"));
  assert!(applied_stdout.contains("## After\n\n- node kind: `expression`\n\n```cirru\n"));
}
