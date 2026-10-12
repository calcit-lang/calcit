//! Unified `--format human|edn|json` surface (#1566 phase A): every leaf with structured
//! output accepts the same values, legacy spellings keep working, and the deprecated
//! `--json` switch warns on stderr.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/calcit/test-effects-graph.cirru");
const MAIN_DEF: &str = "test-effects-graph.main/main!";

fn run_in(dir: Option<&Path>, snapshot: &Path, args: &[&str]) -> Output {
  let mut command = Command::new(env!("CARGO_BIN_EXE_calcit"));
  command
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .arg(snapshot)
    .args(args);
  if let Some(dir) = dir {
    command.current_dir(dir);
  }
  command.output().expect("calcit command should run")
}

fn run(args: &[&str]) -> Output {
  run_in(None, Path::new(FIXTURE), args)
}

fn stdout(output: &Output) -> String {
  String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
  String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Structured stdout must be exactly one document: it starts with the document and parses completely.
fn assert_edn_document(output: &Output, label: &str) {
  assert!(output.status.success(), "{label} failed: {}", stderr(output));
  assert_edn_stdout(output, label);
}

fn assert_edn_stdout(output: &Output, label: &str) {
  let text = stdout(output);
  let trimmed = text.trim_start();
  assert!(trimmed.starts_with("{}"), "{label} stdout should start with an EDN map:\n{text}");
  cirru_edn::parse(trimmed).unwrap_or_else(|error| panic!("{label} should print one valid Cirru EDN document: {error}\n{text}"));
}

fn assert_json_document(output: &Output, label: &str) -> serde_json::Value {
  assert!(output.status.success(), "{label} failed: {}", stderr(output));
  assert_json_stdout(output, label)
}

fn assert_json_stdout(output: &Output, label: &str) -> serde_json::Value {
  let text = stdout(output);
  let trimmed = text.trim_start();
  assert!(
    trimmed.starts_with('{') || trimmed.starts_with('['),
    "{label} stdout should start with JSON:\n{text}"
  );
  serde_json::from_str(trimmed).unwrap_or_else(|error| panic!("{label} should print one valid JSON document: {error}\n{text}"))
}

#[test]
fn graph_commands_accept_unified_format_values() {
  for base in [
    &["analyze", "call-graph"][..],
    &["analyze", "call-graph", "--counts"][..],
    &["analyze", "count-calls"][..],
    &["analyze", "effects-graph"][..],
  ] {
    let with_format = |format: &'static str| {
      let mut args = base.to_vec();
      args.extend(["--format", format]);
      args
    };
    let edn_args = with_format("edn");
    let edn = run(&edn_args);
    assert_edn_document(&edn, &edn_args.join(" "));
    let json_args = with_format("json");
    let json = run(&json_args);
    assert_json_document(&json, &json_args.join(" "));
    // Status lines ("Analyzing ...", "Counting ...") belong on stderr in structured modes.
    for output in [&edn, &json] {
      assert!(
        !stdout(output).contains("Analyzing") && !stdout(output).contains("Counting"),
        "{base:?}"
      );
      assert!(
        stderr(output).contains("Analyzing") || stderr(output).contains("Counting"),
        "{base:?}: {}",
        stderr(output)
      );
    }
    let human_args = with_format("human");
    let human = run(&human_args);
    assert!(human.status.success(), "{human_args:?}: {}", stderr(&human));
    assert!(
      !stdout(&human).trim_start().starts_with('{'),
      "{human_args:?} should print the human report"
    );
  }
}

#[test]
fn graph_commands_keep_legacy_spellings_and_warn_on_unknown_values() {
  for base in [
    &["analyze", "call-graph"][..],
    &["analyze", "count-calls"][..],
    &["analyze", "effects-graph"][..],
  ] {
    for legacy in ["text", "tree"] {
      let mut args = base.to_vec();
      args.extend(["--format", legacy]);
      let output = run(&args);
      assert!(output.status.success(), "{args:?}: {}", stderr(&output));
      assert!(!stdout(&output).trim_start().starts_with('{'), "{args:?}");
      assert!(!stderr(&output).contains("[Warn]"), "{args:?}: {}", stderr(&output));
    }
    // Patch-safe: unknown values still print the human report, with a stderr warning.
    let mut args = base.to_vec();
    args.extend(["--format", "yaml"]);
    let output = run(&args);
    assert!(output.status.success(), "{args:?}: {}", stderr(&output));
    assert!(!stdout(&output).trim_start().starts_with('{'), "{args:?}");
    assert!(stderr(&output).contains("[Warn] Unsupported"), "{args:?}: {}", stderr(&output));
    assert!(stderr(&output).contains("next non-patch release"), "{args:?}: {}", stderr(&output));
  }
  let tree = run(&["analyze", "effects-graph", "--format", "tree"]);
  assert!(stdout(&tree).contains("Effects Graph"));
}

#[test]
fn call_graph_diff_still_accepts_only_the_human_format() {
  let rejected = run(&["analyze", "call-graph", "--diff", "HEAD", "--format", "json"]);
  assert!(!rejected.status.success());
  assert!(stderr(&rejected).contains("--format"), "{}", stderr(&rejected));
}

#[test]
fn test_command_accepts_edn() {
  let output = run(&["test", "--list", "--format", "edn"]);
  assert_edn_document(&output, "test --format edn");
  assert!(stdout(&output).contains(":command |test"));
}

#[test]
fn query_ns_schema_and_tree_show_accept_unified_format_values() {
  assert_edn_document(&run(&["query", "ns", "--format", "edn"]), "query ns --format edn");
  let ns_json = assert_json_document(
    &run(&["query", "ns", "test-effects-graph.main", "--format", "json"]),
    "query ns <ns> --format json",
  );
  assert_eq!(ns_json["data"]["namespace"], "test-effects-graph.main");
  let cirru = run(&["query", "ns", "test-effects-graph.main", "--format", "cirru"]);
  assert!(cirru.status.success(), "{}", stderr(&cirru));

  assert_edn_document(&run(&["query", "schema", MAIN_DEF, "--format", "edn"]), "query schema --format edn");
  let schema = assert_json_document(
    &run(&["query", "schema", MAIN_DEF, "--format", "json"]),
    "query schema --format json",
  );
  assert_eq!(schema["command"], "query.schema");

  assert_edn_document(
    &run(&["tree", "show", MAIN_DEF, "--path", "0", "--format", "edn"]),
    "tree show --format edn",
  );
  let tree = assert_json_document(
    &run(&["tree", "show", MAIN_DEF, "--path", "0", "--format", "json"]),
    "tree show --format json",
  );
  assert_eq!(tree["command"], "tree.show");
  assert_eq!(tree["tree"], "defn");
}

#[test]
fn legacy_json_switch_keeps_working_and_warns_on_stderr() {
  let schema = run(&["query", "schema", MAIN_DEF, "--json"]);
  let value = assert_json_document(&schema, "query schema --json");
  assert_eq!(value["command"], "query.schema");
  assert!(
    stderr(&schema).contains("[Deprecated] `calcit query schema --json`"),
    "{}",
    stderr(&schema)
  );

  let def = run(&["query", "def", MAIN_DEF, "--json"]);
  assert!(def.status.success(), "{}", stderr(&def));
  assert!(stdout(&def).contains("## JSON"));
  assert!(stderr(&def).contains("[Deprecated] `calcit query def --json`"), "{}", stderr(&def));

  let show = run(&["tree", "show", MAIN_DEF, "--path", "0", "--json"]);
  assert!(show.status.success(), "{}", stderr(&show));
  assert!(
    stderr(&show).contains("[Deprecated] `calcit tree show --json`"),
    "{}",
    stderr(&show)
  );

  let conflict = run(&["query", "schema", MAIN_DEF, "--json", "--format", "edn"]);
  assert!(!conflict.status.success());
  assert!(stderr(&conflict).contains("--json"), "{}", stderr(&conflict));

  let ffi = run(&["ffi", "export", "--json"]);
  assert_json_document(&ffi, "ffi export --json");
  assert!(stderr(&ffi).contains("[Deprecated] `calcit ffi export --json`"), "{}", stderr(&ffi));

  for args in [
    &["query", "def", MAIN_DEF, "--json", "--format", "edn"][..],
    &["tree", "show", MAIN_DEF, "--path", "0", "--json", "--format", "edn"][..],
    &["ffi", "export", "--json", "--format", "edn"][..],
    &["ffi", "export", "--json", "--format", "human"][..],
  ] {
    let output = run(args);
    assert!(!output.status.success(), "{args:?} should reject the contradictory combination");
    assert!(stderr(&output).contains("--json"), "{args:?}: {}", stderr(&output));
  }

  let quiet = run(&["query", "schema", MAIN_DEF, "--format", "json"]);
  assert!(!stderr(&quiet).contains("[Deprecated]"), "{}", stderr(&quiet));
}

#[test]
fn root_format_flag_points_subcommands_to_the_trailing_position() {
  let rejected = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args(["--format", "edn", FIXTURE, "query", "ns"])
    .output()
    .expect("calcit command should run");
  assert!(!rejected.status.success());
  assert!(
    stderr(&rejected).contains("put `--format` after the subcommand"),
    "{}",
    stderr(&rejected)
  );
}

fn temp_workspace(name: &str) -> PathBuf {
  let dir = std::env::temp_dir().join(format!("calcit-format-flag-{name}-{}", std::process::id()));
  let _ = fs::remove_dir_all(&dir);
  fs::create_dir_all(&dir).expect("temp workspace");
  fs::copy(FIXTURE, dir.join("app.cirru")).expect("copy fixture");
  dir
}

#[test]
fn cursor_views_accept_edn() {
  let dir = temp_workspace("cursor");
  let snapshot = dir.join("app.cirru");
  let set = run_in(Some(&dir), &snapshot, &["cursor", "set", MAIN_DEF, "--path", "0"]);
  assert!(set.status.success(), "{}", stderr(&set));
  let mark = run_in(Some(&dir), &snapshot, &["cursor", "mark", "first"]);
  assert!(mark.status.success(), "{}", stderr(&mark));

  for view in ["show", "marks"] {
    let edn = run_in(Some(&dir), &snapshot, &["cursor", view, "--format", "edn"]);
    assert_edn_document(&edn, &format!("cursor {view} --format edn"));
    let json = run_in(Some(&dir), &snapshot, &["cursor", view, "--format", "json"]);
    assert_json_document(&json, &format!("cursor {view} --format json"));
  }
  let anchor = run_in(Some(&dir), &snapshot, &["cursor", "anchor"]);
  assert!(anchor.status.success(), "{}", stderr(&anchor));
  let next = run_in(Some(&dir), &snapshot, &["cursor", "next"]);
  assert!(next.status.success(), "{}", stderr(&next));
  let copy = run_in(Some(&dir), &snapshot, &["cursor", "copy"]);
  assert!(copy.status.success(), "{}", stderr(&copy));
  for view in ["region", "clipboard"] {
    let edn = run_in(Some(&dir), &snapshot, &["cursor", view, "--format", "edn"]);
    assert_edn_document(&edn, &format!("cursor {view} --format edn"));
    let json = run_in(Some(&dir), &snapshot, &["cursor", view, "--format", "json"]);
    assert_json_document(&json, &format!("cursor {view} --format json"));
  }
  let rejected = run_in(Some(&dir), &snapshot, &["cursor", "show", "--format", "yaml"]);
  assert!(!rejected.status.success());
  assert!(stderr(&rejected).contains("Expected human, edn, or json"), "{}", stderr(&rejected));
  let _ = fs::remove_dir_all(&dir);
}

#[test]
fn quality_accepts_edn_and_ns_rejects_unknown_values() {
  // The fixture has quality regressions, so the gate exits non-zero; stdout is still one document.
  assert_edn_stdout(&run(&["analyze", "quality", "--format", "edn"]), "analyze quality --format edn");
  assert_json_stdout(&run(&["analyze", "quality", "--format", "json"]), "analyze quality --format json");

  let rejected = run(&["query", "ns", "--format", "yaml"]);
  assert!(!rejected.status.success());
  assert!(
    stderr(&rejected).contains("Expected human, edn, json, or cirru"),
    "{}",
    stderr(&rejected)
  );
}
