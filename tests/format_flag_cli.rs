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
fn graph_commands_keep_legacy_spellings_and_reject_unknown_values() {
  for base in [
    &["analyze", "call-graph"][..],
    &["analyze", "call-graph", "--counts"][..],
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
    // Since 0.31.0 an unknown value is an error instead of a fallback to the human report.
    let mut args = base.to_vec();
    args.extend(["--format", "yaml"]);
    let output = run(&args);
    assert!(!output.status.success(), "{args:?} should be rejected");
    assert!(stdout(&output).is_empty(), "{args:?} should not report on stdout");
    assert!(
      stderr(&output).contains("Unsupported") && stderr(&output).contains("'yaml'"),
      "{args:?}: {}",
      stderr(&output)
    );
    assert!(
      stderr(&output).contains("human (or text, tree), edn, or json"),
      "{args:?}: {}",
      stderr(&output)
    );
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
fn removed_json_switch_names_format_json_and_ffi_export_keeps_it_deprecated() {
  for (args, subject) in [
    (&["query", "schema", MAIN_DEF, "--json"][..], "`calcit query schema --json`"),
    (&["query", "def", MAIN_DEF, "--json"][..], "`calcit query def --json`"),
    (
      &["tree", "show", MAIN_DEF, "--path", "0", "--json"][..],
      "`calcit tree show --json`",
    ),
    (
      &["query", "schema", MAIN_DEF, "--json", "--format", "edn"][..],
      "`calcit query schema --json`",
    ),
    (
      &["query", "def", MAIN_DEF, "--json", "--format", "edn"][..],
      "`calcit query def --json`",
    ),
  ] {
    let output = run(args);
    assert!(!output.status.success(), "{args:?} should be rejected");
    assert!(stdout(&output).is_empty(), "{args:?} should not report on stdout");
    assert!(
      stderr(&output).contains(&format!("{subject} was removed in 0.31.0; use `--format json`")),
      "{args:?}: {}",
      stderr(&output)
    );
  }

  let def = run(&["query", "def", MAIN_DEF, "--format", "json"]);
  let value = assert_json_document(&def, "query def --format json");
  assert_eq!(value["command"], "query.def");

  // `ffi export --json` stays deprecated: the calcit.std FFI inventory workflow still uses it.
  let ffi = run(&["ffi", "export", "--json"]);
  assert_json_document(&ffi, "ffi export --json");
  assert!(stderr(&ffi).contains("[Deprecated] `calcit ffi export --json`"), "{}", stderr(&ffi));
  for args in [
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
  let mark = run_in(Some(&dir), &snapshot, &["cursor", "marks", "set", "first"]);
  assert!(mark.status.success(), "{}", stderr(&mark));

  for view in [&["show"][..], &["marks"][..], &["marks", "list"][..]] {
    let label = view.join(" ");
    let edn = run_in(Some(&dir), &snapshot, &[&["cursor"][..], view, &["--format", "edn"]].concat());
    assert_edn_document(&edn, &format!("cursor {label} --format edn"));
    let json = run_in(Some(&dir), &snapshot, &[&["cursor"][..], view, &["--format", "json"]].concat());
    assert_json_document(&json, &format!("cursor {label} --format json"));
  }
  let anchor = run_in(Some(&dir), &snapshot, &["cursor", "anchor"]);
  assert!(anchor.status.success(), "{}", stderr(&anchor));
  let next = run_in(Some(&dir), &snapshot, &["cursor", "move", "next"]);
  assert!(next.status.success(), "{}", stderr(&next));
  let copy = run_in(Some(&dir), &snapshot, &["cursor", "clipboard", "copy"]);
  assert!(copy.status.success(), "{}", stderr(&copy));
  for view in [&["anchor", "show"][..], &["clipboard"][..], &["clipboard", "show"][..]] {
    let label = view.join(" ");
    let edn = run_in(Some(&dir), &snapshot, &[&["cursor"][..], view, &["--format", "edn"]].concat());
    assert_edn_document(&edn, &format!("cursor {label} --format edn"));
    let json = run_in(Some(&dir), &snapshot, &[&["cursor"][..], view, &["--format", "json"]].concat());
    assert_json_document(&json, &format!("cursor {label} --format json"));
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

/// This is a read-only CLI serialization/enumeration contract, not a language semantic test.
#[test]
fn query_defs_enumerates_names_filters_and_signatures_without_snapshot_parsing() {
  let directory = tempfile::tempdir().expect("isolated query project");
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy(FIXTURE, &snapshot).unwrap();
  let call = |args: &[&str]| run_in(Some(directory.path()), &snapshot, args);
  let operations = serde_json::json!([
    ["edit", "add-ns", "test-effects-graph.enumeration"],
    ["edit", "add-ns", "test-effects-graph.empty"],
    [
      "edit",
      "def",
      "test-effects-graph.enumeration/*cache",
      "--input-format",
      "cirru",
      "--code",
      "quote $ defref *cache ({} (:secret 99))"
    ],
    [
      "edit",
      "def",
      "test-effects-graph.enumeration/has-value?",
      "--input-format",
      "cirru",
      "--code",
      "quote $ defn has-value? (x) $ some? x"
    ],
    [
      "edit",
      "def",
      "test-effects-graph.enumeration/λ-计数!",
      "--input-format",
      "cirru",
      "--code",
      "quote $ defn λ-计数! (x) $ + x 1"
    ],
    [
      "edit",
      "schema",
      "test-effects-graph.enumeration/λ-计数!",
      "--code",
      "quote $ :: 'Fn $ {} (:args $ [] 'Number) (:return 'Number)",
      "--input-format",
      "cirru"
    ],
    ["edit", "tags", "test-effects-graph.enumeration/λ-计数!", "macro,unit"],
    ["edit", "tags", "test-effects-graph.enumeration/has-value?", "unit"],
    [
      "edit",
      "add-test",
      "test-effects-graph.enumeration/λ-计数!",
      "original-test",
      "--input-format",
      "cirru",
      "--code",
      "quote $ assert= 2 $ λ-计数! 1"
    ],
    [
      "edit",
      "add-example",
      "test-effects-graph.enumeration/λ-计数!",
      "--input-format",
      "cirru",
      "--code",
      "quote $ λ-计数! 1"
    ]
  ])
  .to_string();
  let before = fs::read(&snapshot).unwrap();
  let preview = assert_json_document(
    &call(&["edit", "transaction", "--code", &operations, "--dry-run", "--format", "json"]),
    "preview enumeration fixture",
  );
  assert_eq!(fs::read(&snapshot).unwrap(), before);
  assert_json_document(
    &call(&[
      "edit",
      "transaction",
      "--code",
      &operations,
      "--expect-revision",
      preview["scoped_revision"].as_str().unwrap(),
      "--format",
      "json",
    ]),
    "create enumeration fixture",
  );
  let before_queries = fs::read(&snapshot).unwrap();
  let list = assert_json_document(
    &call(&["query", "defs", "test-effects-graph.enumeration", "--format", "json"]),
    "defs json",
  );
  assert_eq!(list["schema_version"], 1);
  assert_eq!(list["command"], "query.defs");
  assert!(list["revision"].as_str().unwrap().starts_with("md5:"));
  assert_eq!(list["diagnostics"], serde_json::json!([]));
  assert_eq!(list["next"], serde_json::json!([]));
  assert_eq!(
    list["data"],
    serde_json::json!({
      "namespace": "test-effects-graph.enumeration", "count": 3, "definition_count": 3,
      "filters": {"tag": null, "signatures": false},
      "definitions": [{"name": "*cache"}, {"name": "has-value?"}, {"name": "λ-计数!"}]
    })
  );
  let ns = assert_json_document(
    &call(&["query", "ns", "test-effects-graph.enumeration", "--format", "json"]),
    "namespace count",
  );
  assert_eq!(list["data"]["definition_count"], ns["data"]["definition_count"]);
  assert_eq!(
    stdout(&call(&["query", "defs", "test-effects-graph.enumeration"])),
    "Definitions: 3\n  *cache\n  has-value? [:unit]\n  λ-计数! [:macro,:unit] [schema]\n"
  );
  for definition in list["data"]["definitions"].as_array().unwrap() {
    let target = format!("test-effects-graph.enumeration/{}", definition["name"].as_str().unwrap());
    let value = assert_json_document(&call(&["query", "def", &target, "--raw", "--format", "json"]), "enumerated def");
    if target.ends_with("λ-计数!") {
      assert_eq!(value["data"]["tests"][0]["name"], "original-test");
      assert_eq!(value["data"]["examples"].as_array().unwrap().len(), 1);
    }
  }
  let signatures = assert_json_document(
    &call(&[
      "query",
      "defs",
      "test-effects-graph.enumeration",
      "--signatures",
      "--format",
      "json",
    ]),
    "defs signatures",
  );
  assert_eq!(signatures["data"]["definitions"][0]["signature"], "(untyped) defref *cache");
  assert_eq!(signatures["data"]["definitions"][1]["signature"], "(untyped) defn has-value? $ x");
  assert!(
    signatures["data"]["definitions"][2]["signature"]
      .as_str()
      .unwrap()
      .contains("'Number")
  );
  let human = stdout(&call(&["query", "defs", "test-effects-graph.enumeration", "--signatures"]));
  for definition in signatures["data"]["definitions"].as_array().unwrap() {
    assert!(human.contains(definition["signature"].as_str().unwrap()));
  }
  let filtered = assert_json_document(
    &call(&[
      "query",
      "defs",
      "test-effects-graph.enumeration",
      "--tag",
      "macro",
      "--format",
      "json",
    ]),
    "filtered defs",
  );
  assert_eq!(filtered["data"]["count"], 1);
  assert_eq!(filtered["data"]["definition_count"], 3);
  assert_eq!(filtered["data"]["definitions"], serde_json::json!([{ "name": "λ-计数!" }]));
  let colon = assert_json_document(
    &call(&[
      "query",
      "defs",
      "test-effects-graph.enumeration",
      "--tag",
      ":macro",
      "--format",
      "json",
    ]),
    "colon tag",
  );
  assert_eq!(filtered, colon, "equivalent tag spellings share the result revision");
  assert_ne!(filtered["revision"], list["revision"]);
  assert_ne!(signatures["revision"], list["revision"]);
  let edn_output = call(&[
    "query",
    "defs",
    "test-effects-graph.enumeration",
    "--tag",
    ":macro",
    "--format",
    "edn",
  ]);
  assert_edn_document(&edn_output, "defs edn");
  let edn = cirru_edn::parse(&stdout(&edn_output)).unwrap();
  let expected = cirru_edn::parse(&format!(
    "{{}} (:schema-version 1) (:command |query.defs) (:revision |{}) (:diagnostics $ []) (:next $ [])\n  :data $ {{}} (:namespace |test-effects-graph.enumeration) (:count 1) (:definition-count 3)\n    :filters $ {{}} (:tag |macro) (:signatures false)\n    :definitions $ [] $ {{}} (:name |λ-计数!)",
    filtered["revision"].as_str().unwrap()
  )).unwrap();
  assert_eq!(edn, expected, "EDN and JSON envelopes carry equivalent data");
  for args in [
    vec!["query", "defs", "test-effects-graph.empty", "--format", "json"],
    vec![
      "query",
      "defs",
      "test-effects-graph.enumeration",
      "--tag",
      "absent",
      "--format",
      "json",
    ],
  ] {
    let empty = assert_json_document(&call(&args), "empty enumeration");
    assert_eq!(empty["data"]["count"], 0);
    assert_eq!(empty["data"]["definitions"], serde_json::json!([]));
    assert_eq!(
      empty["data"]["definition_count"],
      if args[2] == "test-effects-graph.empty" { 0 } else { 3 }
    );
  }
  assert_eq!(
    fs::read(&snapshot).unwrap(),
    before_queries,
    "all enumeration/follow-up queries are read-only"
  );
}

#[test]
fn query_defs_errors_keep_structured_stdout_and_reject_unknown_formats() {
  for format in ["edn", "json"] {
    for (namespace, tag, expected_code) in [
      ("missing.namespace", "unit", "E_QUERY_TARGET_NOT_FOUND"),
      ("test-effects-graph.main", ":", "E_QUERY_FAILED"),
    ] {
      let output = run(&["query", "defs", namespace, "--tag", tag, "--format", format]);
      assert!(!output.status.success());
      if format == "edn" {
        assert_edn_stdout(&output, "defs error");
        assert!(stdout(&output).contains(expected_code));
      } else {
        let value = assert_json_stdout(&output, "defs error");
        assert_eq!(value["command"], "query.defs");
        assert!(value["data"].is_null());
        assert!(value["revision"].is_null());
        assert_eq!(value["diagnostics"][0]["code"], expected_code);
        assert_eq!(value["diagnostics"][0]["severity"], "error");
        assert_eq!(value["diagnostics"][0]["target"], namespace);
      }
    }
  }
  let output = run_in(
    None,
    Path::new("does-not-exist.cirru"),
    &["query", "defs", "app.main", "--format", "yaml"],
  );
  assert!(!output.status.success());
  assert!(output.stdout.is_empty());
  assert!(stderr(&output).contains("Unsupported query output format 'yaml'"));
  assert!(!stderr(&output).contains("does not exist"), "reject format before Snapshot I/O");
}

/// Git/source-metadata evidence and transport are CLI contracts, not language semantics.
#[test]
fn program_diff_formats_preserve_value_schema_evidence_and_widening_gate() {
  use cirru_edn::Edn;

  let directory = tempfile::tempdir().expect("isolated program-diff project");
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy(FIXTURE, &snapshot).unwrap();
  let call = |args: &[&str]| run_in(Some(directory.path()), &snapshot, args);
  let mutate = |operations: serde_json::Value| {
    let transaction = operations.to_string();
    let preview = assert_json_document(
      &call(&["edit", "transaction", "--code", &transaction, "--dry-run", "--format", "json"]),
      "preview metadata transaction",
    );
    assert_json_document(
      &call(&[
        "edit",
        "transaction",
        "--code",
        &transaction,
        "--expect-revision",
        preview["scoped_revision"].as_str().unwrap(),
        "--format",
        "json",
      ]),
      "apply metadata transaction",
    );
  };
  let git = |args: &[&str]| {
    let output = Command::new("git")
      .current_dir(directory.path())
      .args([
        "-c",
        "user.name=Calcit Fixture",
        "-c",
        "user.email=calcit-fixture@example.invalid",
        "-c",
        "commit.gpgsign=false",
      ])
      .arg("-c")
      .arg(format!("core.hooksPath={}", directory.path().join("no-hooks").display()))
      .args(args)
      .output()
      .expect("fixture git should run");
    assert!(output.status.success(), "git {args:?}: {}", stderr(&output));
  };
  let target = "test-effects-graph.program-diff/Probe";
  let amount = "test-effects-graph.program-diff/amount";
  assert_json_document(&call(&["query", "config", "--format", "json"]), "fixture config");
  mutate(serde_json::json!([
    ["edit", "add-ns", "test-effects-graph.program-diff"],
    [
      "edit",
      "def",
      target,
      "--input-format",
      "cirru",
      "--code",
      "quote $ defstruct Probe (:value 'Number)"
    ],
    ["edit", "schema", target, "--input-format", "cirru", "--code", "quote 'Struct"],
    ["edit", "def", amount, "--input-format", "cirru", "--code", "quote $ def amount 1"],
    ["edit", "schema", amount, "--input-format", "cirru", "--code", "quote 'Number"]
  ]));
  git(&["init", "--quiet"]);
  git(&["add", "calcit.cirru"]);
  git(&["commit", "--quiet", "-m", "Original schema"]);
  mutate(serde_json::json!([[
    "edit",
    "schema",
    target,
    "--input-format",
    "cirru",
    "--code",
    "quote 'StructDef"
  ]]));
  let before_reports = fs::read(&snapshot).unwrap();
  let readback = assert_json_document(&call(&["query", "def", target, "--format", "json"]), "schema readback");
  assert_eq!(readback["data"]["schema"], "'StructDef");
  let json = assert_json_document(
    &call(&["analyze", "program-diff", "HEAD", "--format", "json", "--fail-on-widening"]),
    "incomparable kinds require review but are not widening",
  );
  let evidence = &json["data"]["evidence"];
  assert_eq!(evidence["classification"], "semantic-change");
  assert_eq!(evidence["semantic_review_required"], true);
  assert_eq!(evidence["categories"], serde_json::json!(["schema-signature"]));
  assert_eq!(evidence["schema_widening"], serde_json::json!([]));
  assert_eq!(evidence["changes"].as_array().unwrap().len(), 1);
  let change = &evidence["changes"][0];
  assert_eq!(change["definition"], target);
  assert_eq!(change["schema_direction"], "changed");
  assert_eq!(change["path"], "files/test-effects-graph.program-diff/defs/Probe/schema");

  let edn = call(&["analyze", "program-diff", "HEAD", "--format", "edn"]);
  assert_edn_document(&edn, "program-diff EDN");
  let root = cirru_edn::parse(&stdout(&edn)).unwrap().view_map().unwrap();
  let data = root.tag_get("data").unwrap().view_map().unwrap();
  let native_evidence = data.tag_get("evidence").unwrap().view_map().unwrap();
  assert_eq!(native_evidence.tag_get("semantic-review-required"), Some(&Edn::Bool(true)));
  for (edn_key, json_key) in [
    ("classification", "classification"),
    ("before-revision", "before_revision"),
    ("after-revision", "after_revision"),
  ] {
    assert_eq!(
      native_evidence.tag_get(edn_key),
      Some(&Edn::str(evidence[json_key].as_str().unwrap()))
    );
  }
  let changes = native_evidence.tag_get("changes").unwrap().view_list().unwrap();
  assert_eq!(changes.len(), 1);
  let native_change = changes.get(0).unwrap().view_map().unwrap();
  for key in ["category", "definition", "path"] {
    assert_eq!(native_change.tag_get(key), Some(&Edn::str(change[key].as_str().unwrap())));
  }
  assert_eq!(native_change.tag_get("schema-direction"), Some(&Edn::str("changed")));
  let human = call(&["analyze", "program-diff", "HEAD"]);
  assert!(human.status.success(), "{}", stderr(&human));
  for text in [
    "# Program Diff",
    "classification: semantic-change",
    "semantic review required: true",
    "schema-signature",
    "StructDef",
  ] {
    assert!(stdout(&human).contains(text), "missing {text}: {}", stdout(&human));
  }
  assert_eq!(fs::read(&snapshot).unwrap(), before_reports);
  git(&["add", "calcit.cirru"]);
  git(&["commit", "--quiet", "-m", "Correct prototype schema"]);
  let historical = assert_json_document(
    &call(&["analyze", "program-diff", "HEAD", "--base", "HEAD^", "--format", "json"]),
    "two-ref program-diff",
  );
  assert_eq!(historical["data"]["evidence"], *evidence);
  assert_eq!(fs::read(&snapshot).unwrap(), before_reports);

  mutate(serde_json::json!([[
    "edit",
    "schema",
    amount,
    "--input-format",
    "cirru",
    "--code",
    "quote 'Dynamic"
  ]]));
  let before_gate = fs::read(&snapshot).unwrap();
  let widened = call(&["analyze", "program-diff", "HEAD", "--format", "json", "--fail-on-widening"]);
  assert!(!widened.status.success(), "actual widening must still fail");
  assert!(stderr(&widened).contains("Schema widening"));
  let widened = assert_json_stdout(&widened, "widening report");
  assert_eq!(widened["data"]["evidence"]["schema_widening"], serde_json::json!([amount]));
  assert_eq!(
    widened["data"]["evidence"]["changes"][0]["widened_slots"],
    serde_json::json!(["schema"])
  );
  assert_eq!(fs::read(&snapshot).unwrap(), before_gate);
}
