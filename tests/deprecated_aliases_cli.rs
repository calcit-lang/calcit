//! Deprecated CLI aliases (#1566) keep their stdout contract for one non-patch
//! release, print a single migration hint on stderr, and match the replacement form.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const HINT_PREFIX: &str = "[Deprecated] `calcit ";

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

/// Timing fields are the only nondeterministic part of these reports.
fn stable_stdout(output: &Output) -> String {
  String::from_utf8_lossy(&output.stdout)
    .lines()
    .map(|line| {
      if line.starts_with("- result:") {
        line.split(" (").next().unwrap_or(line).to_owned()
      } else if line.contains("duration_ms") || line.contains("duration-ms") {
        line.split("duration").next().unwrap_or(line).to_owned()
      } else {
        line.to_owned()
      }
    })
    .collect::<Vec<_>>()
    .join("\n")
}

fn hint_lines(output: &Output) -> Vec<String> {
  String::from_utf8_lossy(&output.stderr)
    .lines()
    .filter(|line| line.starts_with(HINT_PREFIX))
    .map(str::to_owned)
    .collect()
}

fn assert_alias_matches(snapshot: &Path, old: &[&str], new: &[&str], replacement: &str) {
  let old_output = run_calcit(snapshot, old);
  let new_output = run_calcit(snapshot, new);
  assert_eq!(
    old_output.status.code(),
    new_output.status.code(),
    "{old:?} and {new:?} should exit alike\nold stderr:\n{}\nnew stderr:\n{}",
    String::from_utf8_lossy(&old_output.stderr),
    String::from_utf8_lossy(&new_output.stderr)
  );
  assert!(!old_output.stdout.is_empty(), "{old:?} should report on stdout");
  assert_eq!(stable_stdout(&old_output), stable_stdout(&new_output), "{old:?} vs {new:?}");
  let hints = hint_lines(&old_output);
  assert_eq!(hints.len(), 1, "{old:?} should print exactly one hint: {hints:?}");
  assert!(
    hints[0].contains(replacement),
    "{old:?} hint should name `{replacement}`: {}",
    hints[0]
  );
  assert!(hint_lines(&new_output).is_empty(), "{new:?} must not print a deprecation hint");
  assert!(
    !String::from_utf8_lossy(&old_output.stdout).contains("[Deprecated]"),
    "{old:?} must keep the hint off stdout"
  );
}

#[test]
fn type_debt_aliases_match_weak_types_views() {
  let snapshot = Path::new("calcit/test.cirru");
  assert_alias_matches(
    snapshot,
    &["analyze", "check-types", "--summary-only"],
    &["analyze", "weak-types", "--only", "coverage", "--summary-only"],
    "weak-types --only coverage",
  );
  assert_alias_matches(
    snapshot,
    &["analyze", "check-types", "--only", "none", "--ns", "app.main", "--format", "json"],
    &[
      "analyze",
      "weak-types",
      "--only",
      "coverage",
      "--coverage-level",
      "none",
      "--ns",
      "app.main",
      "--format",
      "json",
    ],
    "weak-types --only coverage",
  );
  assert_alias_matches(
    snapshot,
    &["analyze", "deprecated", "--format", "json"],
    &["analyze", "weak-types", "--only", "deprecated-call", "--format", "json"],
    "weak-types --only deprecated-call",
  );
  assert_alias_matches(
    Path::new("calcit/test-wasm.cirru"),
    &["analyze", "dynamic-methods", "--format", "json"],
    &["analyze", "weak-types", "--only", "dynamic-method", "--format", "json"],
    "weak-types --only dynamic-method",
  );
}

/// Converts a Cirru EDN envelope to JSON with the key spelling of the JSON envelope (`-` as `_`).
fn edn_to_json(value: &cirru_edn::Edn) -> serde_json::Value {
  use cirru_edn::Edn;
  match value {
    Edn::Nil => serde_json::Value::Null,
    Edn::Bool(value) => serde_json::Value::Bool(*value),
    Edn::Number(value) => serde_json::json!(*value),
    Edn::Str(value) => serde_json::Value::String(value.to_string()),
    Edn::Tag(value) => serde_json::Value::String(value.ref_str().to_owned()),
    Edn::List(values) => serde_json::Value::Array(values.0.iter().map(edn_to_json).collect()),
    Edn::Map(fields) => serde_json::Value::Object(
      fields
        .0
        .iter()
        .map(|(key, value)| {
          let Edn::Tag(key) = key else {
            panic!("EDN envelope keys should be tags: {key:?}");
          };
          (key.ref_str().replace('-', "_"), edn_to_json(value))
        })
        .collect(),
    ),
    other => panic!("unexpected EDN value in an analysis envelope: {other:?}"),
  }
}

/// Numbers become f64 (EDN has one number type) and timing fields are dropped.
fn normalize_report(value: &serde_json::Value) -> serde_json::Value {
  match value {
    serde_json::Value::Object(fields) => serde_json::Value::Object(
      fields
        .iter()
        .filter(|(key, _)| key.as_str() != "duration_ms")
        .map(|(key, value)| (key.clone(), normalize_report(value)))
        .collect(),
    ),
    serde_json::Value::Array(values) => serde_json::Value::Array(values.iter().map(normalize_report).collect()),
    serde_json::Value::Number(number) => serde_json::json!(number.as_f64().expect("finite number")),
    other => other.clone(),
  }
}

#[test]
fn weak_types_views_offer_edn_equivalent_to_json() {
  for (snapshot, view) in [
    ("calcit/test.cirru", "coverage"),
    ("calcit/test.cirru", "deprecated-call"),
    ("calcit/test.cirru", "dynamic-method"),
    ("calcit/test-wasm.cirru", "dynamic-method"),
    ("tests/fixtures/option-nil-roundtrip.cirru", "option-nil-roundtrip"),
  ] {
    let args = |format| vec!["analyze", "weak-types", "--only", view, "--format", format];
    let json = run_calcit(Path::new(snapshot), &args("json"));
    let edn = run_calcit(Path::new(snapshot), &args("edn"));
    assert!(edn.status.success(), "{view}: {}", String::from_utf8_lossy(&edn.stderr));
    // Each stdout must be exactly one document; program output during preprocessing stays off stdout.
    let json: serde_json::Value = serde_json::from_slice(&json.stdout).expect("stdout should be exactly one JSON document");
    let edn = cirru_edn::parse(&String::from_utf8_lossy(&edn.stdout)).expect("stdout should be exactly one Cirru EDN document");
    assert_eq!(
      normalize_report(&edn_to_json(&edn)),
      normalize_report(&json),
      "{snapshot} --only {view}: EDN and JSON envelopes differ"
    );
  }
}

#[test]
fn weak_types_views_reject_options_they_cannot_honor() {
  let snapshot = Path::new("calcit/test.cirru");
  for args in [
    &["analyze", "weak-types", "--only", "coverage,code-nil"][..],
    &["analyze", "weak-types", "--only", "dynamic-method", "--ns", "app.main"][..],
    &["analyze", "weak-types", "--coverage-level", "none"][..],
    &["analyze", "weak-types", "--only", "option-nil-roundtrip,code-nil"][..],
    &["analyze", "weak-types", "--only", "option-nil-roundtrip", "--incremental"][..],
    &["analyze", "weak-types", "--only", "option-nil-roundtrip", "--intent", "unresolved"][..],
    &[
      "analyze",
      "weak-types",
      "--only",
      "option-nil-roundtrip",
      "--coverage-level",
      "full",
    ][..],
  ] {
    let output = run_calcit(snapshot, args);
    assert!(!output.status.success(), "{args:?} should be rejected");
    assert!(output.stdout.is_empty(), "{args:?} should not report on stdout");
  }
}

#[test]
fn option_nil_view_reports_source_types_and_consumers_without_writes_or_new_diagnostics() {
  // Rust owns the CLI envelope and read-only boundary. The fixture's attached
  // Calcit tests own None/Some-nil/false/zero/empty/Unicode value semantics.
  let snapshot = Path::new("tests/fixtures/option-nil-roundtrip.cirru");
  let before = fs::read(snapshot).unwrap();
  let strict_args = ["--check-only", "--all-defs", "--format", "json"];
  let strict_before = run_calcit(snapshot, &strict_args);
  assert!(strict_before.status.success(), "{}", String::from_utf8_lossy(&strict_before.stderr));
  let view = ["analyze", "weak-types", "--only", "option-nil-roundtrip"];
  let report = run_calcit(snapshot, &[&view[..], &["--format", "json"]].concat());
  assert!(report.status.success(), "{}", String::from_utf8_lossy(&report.stderr));
  let report: serde_json::Value = serde_json::from_slice(&report.stdout).expect("one JSON document");
  assert_eq!(report["diagnostics"], serde_json::json!([]));
  assert_eq!(report["data"]["summary"], serde_json::json!({"findings":9,"unknown":0}));
  let findings = report["data"]["findings"].as_array().unwrap();
  let at = |id: &str| {
    findings
      .iter()
      .find(|finding| finding["definition"] == id)
      .expect("source location")
  };
  assert_eq!(at("app.roundtrip/read-field")["path"], "code@3.1");
  assert_eq!(at("app.roundtrip/read-field")["input"]["path"], "code@3.1.1");
  assert_eq!(
    at("app.roundtrip/read-field")["consumer"],
    serde_json::json!({"path":"code@3","head":"either","child_index":1})
  );
  assert_eq!(at("app.roundtrip/threaded-value")["input"]["path"], "code@3.0");
  for finding in findings {
    assert_eq!(finding["input"]["inferred_type"], ":: 'calcit.core/Option 'Dynamic");
    assert_eq!(finding["input"]["confidence"], "partial");
    let query = run_calcit(
      snapshot,
      &[
        "query",
        "type-at",
        finding["definition"].as_str().unwrap(),
        "--path",
        finding["input"]["path"].as_str().unwrap(),
        "--format",
        "json",
      ],
    );
    assert!(query.status.success(), "{}", String::from_utf8_lossy(&query.stderr));
    let query: serde_json::Value = serde_json::from_slice(&query.stdout).unwrap();
    assert_eq!(finding["input"]["inferred_type"], query["data"]["inferred_type"]);
    assert_eq!(finding["input"]["confidence"], query["data"]["confidence"]);
  }
  assert!(!findings.iter().any(|finding| matches!(
    finding["definition"].as_str(),
    Some("app.roundtrip/quoted-value" | "app.roundtrip/other-match")
  )));
  let scoped = run_calcit(snapshot, &[&view[..], &["--ns", "app.roundtrip", "--format", "json"]].concat());
  let scoped: serde_json::Value = serde_json::from_slice(&scoped.stdout).unwrap();
  assert_eq!(scoped["data"]["summary"]["findings"], 8);
  let summary = run_calcit(
    snapshot,
    &[&view[..], &["--ns-prefix", "app.round", "--summary-only", "--format", "json"]].concat(),
  );
  let summary: serde_json::Value = serde_json::from_slice(&summary.stdout).unwrap();
  assert_eq!(summary["data"]["summary"], scoped["data"]["summary"]);
  assert_eq!(summary["data"]["findings"], serde_json::json!([]));
  let human = run_calcit(snapshot, &view);
  assert!(human.status.success());
  let human = String::from_utf8(human.stdout).unwrap();
  assert!(human.starts_with("# Option → nil\n\n"));
  assert!(human.contains("```cirru\n") && human.contains("Direct consumer: either @ code@3"));
  let strict_after = run_calcit(snapshot, &strict_args);
  assert_eq!(strict_before.status.code(), strict_after.status.code());
  let strict_before: serde_json::Value = serde_json::from_slice(&strict_before.stdout).unwrap();
  let strict_after: serde_json::Value = serde_json::from_slice(&strict_after.stdout).unwrap();
  assert_eq!(normalize_report(&strict_before), normalize_report(&strict_after));
  assert_eq!(
    fs::read(snapshot).unwrap(),
    before,
    "read-only view must preserve every source byte"
  );
}

#[test]
fn option_nil_view_keeps_unavailable_evidence_unknown_and_never_runs_the_entry() {
  let directory = tempfile::tempdir().unwrap();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/option-nil-roundtrip.cirru", &snapshot).unwrap();
  let operations = serde_json::json!([
    [
      "edit",
      "def",
      "app.main/main!",
      "--overwrite",
      "--input-format",
      "cirru",
      "--code",
      "quote $ defn main! () $ raise |entry-must-not-run"
    ],
    [
      "edit",
      "def",
      "app.roundtrip/unwrap-value",
      "--overwrite",
      "--input-format",
      "cirru",
      "--code",
      "quote $ defn unwrap-value (value) $ .unwrap-or (unknown-expression value) nil"
    ],
    [
      "edit",
      "def",
      "app.roundtrip/loud-option",
      "--input-format",
      "cirru",
      "--code",
      "quote $ defmacro loud-option (value)\n  println |macro-output-sentinel\n  quasiquote $ Option :some ~value"
    ],
    [
      "edit",
      "schema",
      "app.roundtrip/loud-option",
      "--input-format",
      "cirru",
      "--code",
      "quote $ :: 'Macro $ {} (:required $ [] 'Syntax) (:expansion $ :: 'Expr $ :: 'Option 'Dynamic) (:capabilities $ #{} :log)"
    ],
    [
      "edit",
      "def",
      "app.roundtrip/loud-value",
      "--input-format",
      "cirru",
      "--code",
      "quote $ defn loud-value (value) $ .unwrap-or (loud-option value) nil"
    ],
    [
      "edit",
      "schema",
      "app.roundtrip/loud-value",
      "--input-format",
      "cirru",
      "--code",
      "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return 'Dynamic)"
    ]
  ])
  .to_string();
  let preview = run_calcit(
    &snapshot,
    &["edit", "transaction", "--code", &operations, "--dry-run", "--format", "json"],
  );
  assert!(preview.status.success(), "{}", String::from_utf8_lossy(&preview.stderr));
  let preview: serde_json::Value = serde_json::from_slice(&preview.stdout).unwrap();
  let applied = run_calcit(
    &snapshot,
    &[
      "edit",
      "transaction",
      "--code",
      &operations,
      "--expect-revision",
      preview["scoped_revision"].as_str().unwrap(),
      "--format",
      "json",
    ],
  );
  assert!(applied.status.success(), "{}", String::from_utf8_lossy(&applied.stderr));
  let before = fs::read(&snapshot).unwrap();
  let output = run_calcit(
    &snapshot,
    &[
      "analyze",
      "weak-types",
      "--only",
      "option-nil-roundtrip",
      "--ns",
      "app.roundtrip",
      "--format",
      "json",
    ],
  );
  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
  assert!(
    String::from_utf8_lossy(&output.stderr).contains("macro-output-sentinel"),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  let finding = report["data"]["findings"]
    .as_array()
    .unwrap()
    .iter()
    .find(|finding| finding["definition"] == "app.roundtrip/unwrap-value")
    .unwrap();
  assert_eq!(finding["input"]["inferred_type"], serde_json::Value::Null);
  assert_eq!(finding["input"]["confidence"], "unknown");
  assert_eq!(report["diagnostics"], serde_json::json!([]));
  assert_eq!(fs::read(&snapshot).unwrap(), before);
}

#[test]
fn check_public_alias_matches_check_only_namespace_scope() {
  let snapshot = Path::new("calcit/test-wasi-command.cirru");
  assert_alias_matches(
    snapshot,
    &["analyze", "check-public", "--ns", "calcit.test", "--deps", "--summary-only"],
    &["--check-only", "--ns", "calcit.test", "--deps", "--summary-only"],
    "--check-only --ns",
  );
  assert_alias_matches(
    snapshot,
    &["analyze", "check-public", "--ns", "calcit.test", "--deps", "--format", "json"],
    &["--check-only", "--ns", "calcit.test", "--deps", "--format", "json"],
    "--check-only --ns",
  );
  // A namespace that is not loaded fails the same way in both forms.
  assert_alias_matches(
    snapshot,
    &["analyze", "check-public", "--ns", "app.missing", "--format", "json"],
    &["--check-only", "--ns", "app.missing", "--format", "json"],
    "--check-only --ns",
  );

  let json = run_calcit(snapshot, &["--check-only", "--ns", "calcit.test", "--deps", "--format", "json"]);
  let edn = run_calcit(snapshot, &["--check-only", "--ns", "calcit.test", "--deps", "--format", "edn"]);
  assert!(edn.status.success(), "{}", String::from_utf8_lossy(&edn.stderr));
  let json: serde_json::Value = serde_json::from_slice(&json.stdout).expect("one JSON document");
  let edn = cirru_edn::parse(&String::from_utf8_lossy(&edn.stdout)).expect("one Cirru EDN document");
  let cirru_edn::Edn::Map(root) = edn else {
    panic!("EDN envelope should be a map");
  };
  assert_eq!(
    root.get(&cirru_edn::Edn::tag("command")),
    Some(&cirru_edn::Edn::str(json["command"].as_str().expect("command name")))
  );
  let Some(cirru_edn::Edn::Map(data)) = root.get(&cirru_edn::Edn::tag("data")) else {
    panic!("EDN data should be a map");
  };
  let Some(cirru_edn::Edn::List(ids)) = data.get(&cirru_edn::Edn::tag("checked-definition-ids")) else {
    panic!("EDN should list checked definition ids");
  };
  assert_eq!(ids.len(), json["data"]["checked_definition_ids"].as_array().expect("ids").len());

  for args in [
    &["--ns", "calcit.test"][..],
    &["--check-only", "--deps"][..],
    &["--check-only", "--ns", "calcit.test", "--keep-going"][..],
  ] {
    let output = run_calcit(snapshot, args);
    assert!(!output.status.success(), "{args:?} should be rejected");
  }
}

#[test]
fn quality_and_query_aliases_keep_output_and_hint() {
  let snapshot = Path::new("calcit/test.cirru");
  let quality = run_calcit(snapshot, &["analyze", "quality", "--format", "json"]);
  let hints = hint_lines(&quality);
  assert_eq!(hints.len(), 1, "{hints:?}");
  assert!(hints[0].contains("--check-only") && hints[0].contains("weak-types"), "{}", hints[0]);
  serde_json::from_slice::<serde_json::Value>(&quality.stdout).expect("quality keeps one JSON document on stdout");

  let pkg = run_calcit(snapshot, &["query", "pkg"]);
  assert!(pkg.status.success());
  assert_eq!(String::from_utf8_lossy(&pkg.stdout), "app\n");
  assert!(hint_lines(&pkg)[0].contains("`calcit query config`"));
  let config = run_calcit(snapshot, &["query", "config"]);
  assert!(String::from_utf8_lossy(&config.stdout).contains("Package: `app`"));
  assert!(hint_lines(&config).is_empty());

  let modules = run_calcit(snapshot, &["query", "modules"]);
  assert!(modules.status.success());
  assert!(hint_lines(&modules)[0].contains("`calcit config modules`"));
  let config_modules = run_calcit(snapshot, &["config", "modules"]);
  assert!(hint_lines(&config_modules).is_empty());
  let module_lines = |output: &Output| {
    String::from_utf8_lossy(&output.stdout)
      .lines()
      .filter(|line| line.starts_with("  "))
      .map(str::to_owned)
      .collect::<Vec<_>>()
  };
  assert_eq!(module_lines(&modules), module_lines(&config_modules));
}

struct TempProject(PathBuf);

impl TempProject {
  fn create(name: &str) -> Self {
    let path = std::env::temp_dir().join(format!("calcit-deprecated-aliases-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("temporary project should create");
    let project = Self(path);
    fs::copy("calcit/add.cirru", project.snapshot()).expect("copy minimal snapshot");
    let output = run_calcit(
      &project.snapshot(),
      &["edit", "def", "app.main/items", "--code", "quote $ def items $ [] 1 2 3 4"],
    );
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    project
  }

  fn snapshot(&self) -> PathBuf {
    self.0.join("calcit.cirru")
  }
}

impl Drop for TempProject {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

#[test]
fn batch_delete_alias_matches_edit_transaction() {
  let legacy = TempProject::create("batch");
  let output = run_calcit(
    &legacy.snapshot(),
    &["tree", "batch-delete", "app.main/items", "--paths", "@2.1", "--paths", "@2.3"],
  );
  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  let hints = hint_lines(&output);
  assert_eq!(hints.len(), 1, "{hints:?}");
  assert!(hints[0].contains("edit transaction"), "{}", hints[0]);

  let transaction = TempProject::create("transaction");
  let output = run_calcit(
    &transaction.snapshot(),
    &[
      "edit",
      "transaction",
      "--code",
      r#"[["tree","delete","app.main/items","--path","@2.3"],["tree","delete","app.main/items","--path","@2.1"]]"#,
    ],
  );
  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  assert!(hint_lines(&output).is_empty());

  let read = |project: &TempProject| fs::read_to_string(project.snapshot()).expect("snapshot should read");
  assert_eq!(read(&legacy), read(&transaction));
  assert!(read(&legacy).contains("[] 2 4"), "{}", read(&legacy));
}

#[test]
fn batch_delete_transaction_operation_prints_the_hint() {
  let project = TempProject::create("transaction-batch");
  let output = run_calcit(
    &project.snapshot(),
    &[
      "edit",
      "transaction",
      "--code",
      r#"[["tree","batch-delete","app.main/items","--paths","@2.1","--paths","@2.3"]]"#,
    ],
  );
  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  let stderr = String::from_utf8_lossy(&output.stderr);
  let hints = stderr
    .lines()
    .filter(|line| line.starts_with("[Deprecated] transaction operation 1 uses `tree batch-delete`"))
    .collect::<Vec<_>>();
  assert_eq!(hints.len(), 1, "{stderr}");
  assert!(hints[0].contains("tree delete"), "{}", hints[0]);
  assert!(!String::from_utf8_lossy(&output.stdout).contains("[Deprecated]"));
  let content = fs::read_to_string(project.snapshot()).expect("snapshot should read");
  assert!(content.contains("[] 2 4"), "{content}");
}

#[test]
fn call_graph_aliases_match_call_graph_options() {
  let snapshot = Path::new("calcit/test.cirru");
  assert_alias_matches(
    snapshot,
    &["analyze", "count-calls"],
    &["analyze", "call-graph", "--counts"],
    "call-graph --counts",
  );
  assert_alias_matches(
    snapshot,
    &["analyze", "count-calls", "--sort", "name", "--ns-prefix", "app.", "--include-core"],
    &[
      "analyze",
      "call-graph",
      "--counts",
      "--sort",
      "name",
      "--ns-prefix",
      "app.",
      "--include-core",
    ],
    "call-graph --counts",
  );
  assert_alias_matches(
    snapshot,
    &["analyze", "count-calls", "--format", "json"],
    &["analyze", "call-graph", "--counts", "--format", "json"],
    "call-graph --counts",
  );
  assert_alias_matches(
    snapshot,
    &["analyze", "call-graph-diff", "HEAD", "--ns-prefix", "app.", "--max-depth", "2"],
    &["analyze", "call-graph", "--diff", "HEAD", "--ns-prefix", "app.", "--max-depth", "2"],
    "call-graph --diff",
  );

  for args in [
    &["analyze", "call-graph", "--counts", "--diff", "HEAD"][..],
    &["analyze", "call-graph", "--sort", "name"][..],
    &["analyze", "call-graph", "--diff", "HEAD", "--format", "json"][..],
  ] {
    let output = run_calcit(snapshot, args);
    assert!(!output.status.success(), "{args:?} should be rejected");
    assert!(output.stdout.is_empty(), "{args:?} should not report on stdout");
  }
}

#[test]
fn js_escape_aliases_keep_output_and_name_no_replacement() {
  let snapshot = Path::new("calcit/test.cirru");
  for (args, expected) in [
    (&["analyze", "js-escape", "a?"][..], "a_$q_"),
    (&["analyze", "js-unescape", "a_$q_"][..], "a?"),
  ] {
    let output = run_calcit(snapshot, args);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    let hints = hint_lines(&output);
    assert_eq!(hints.len(), 1, "{hints:?}");
    assert!(hints[0].contains("has no replacement"), "{}", hints[0]);
  }
}

/// A HOME whose `.config/calcit/docs` points at this repository's docs, so the guidebook commands can run.
struct GuidebookHome(PathBuf);

impl GuidebookHome {
  fn create(name: &str) -> Self {
    let home = std::env::temp_dir().join(format!("calcit-deprecated-aliases-{}-home-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&home);
    fs::create_dir_all(home.join(".config/calcit")).expect("temporary HOME should create");
    let docs = fs::canonicalize("docs").expect("repository docs directory");
    std::os::unix::fs::symlink(docs, home.join(".config/calcit/docs")).expect("link guidebook docs");
    Self(home)
  }

  fn run(&self, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_calcit"))
      .env("NO_COLOR", "1")
      .env("HOME", &self.0)
      .arg("--tips-level")
      .arg("none")
      .arg("calcit/test.cirru")
      .args(args)
      .output()
      .expect("calcit command should run")
  }
}

impl Drop for GuidebookHome {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

#[test]
fn guidebook_aliases_match_docs_read() {
  let home = GuidebookHome::create("aliases");
  for (old, new) in [
    (
      &["docs", "read-lines", "cirru-syntax.md", "--start", "4", "--lines", "6"][..],
      &["docs", "read", "cirru-syntax.md", "--start", "4", "--lines", "6"][..],
    ),
    (
      &["docs", "read-lines", "cirru-syntax.md"][..],
      &["docs", "read", "cirru-syntax.md", "--lines", "80"][..],
    ),
    (
      &["docs", "read-lines", "no-such-doc.md", "--lines", "3"][..],
      &["docs", "read", "no-such-doc.md", "--lines", "3"][..],
    ),
  ] {
    let old_output = home.run(old);
    let new_output = home.run(new);
    assert_eq!(old_output.status.code(), new_output.status.code(), "{old:?} vs {new:?}");
    assert_eq!(old_output.stdout, new_output.stdout, "{old:?} vs {new:?}");
    let hints = hint_lines(&old_output);
    assert_eq!(hints.len(), 1, "{hints:?}");
    assert!(hints[0].contains("docs read <file> --start"), "{}", hints[0]);
    assert!(hint_lines(&new_output).is_empty());
  }
  let ranged = home.run(&["docs", "read", "cirru-syntax.md", "--start", "4", "--lines", "6"]);
  assert!(String::from_utf8_lossy(&ranged.stdout).contains("Lines 5-10 of"));
  let rejected = home.run(&["docs", "read", "cirru-syntax.md", "Mistakes", "--lines", "6"]);
  assert!(!rejected.status.success() && rejected.stdout.is_empty());

  // `cirru show-guide` keeps printing the raw guide; `docs read` renders the same file.
  let guide = fs::read_to_string("docs/cirru-syntax.md").expect("guide source");
  let show_guide = home.run(&["cirru", "show-guide"]);
  assert!(show_guide.status.success(), "{}", String::from_utf8_lossy(&show_guide.stderr));
  assert_eq!(String::from_utf8_lossy(&show_guide.stdout), format!("{guide}\n"));
  let hints = hint_lines(&show_guide);
  assert_eq!(hints.len(), 1, "{hints:?}");
  assert!(hints[0].contains("`calcit docs read cirru-syntax.md`"), "{}", hints[0]);
  let read = home.run(&["docs", "read", "cirru-syntax.md"]);
  assert!(read.status.success(), "{}", String::from_utf8_lossy(&read.stderr));
  let read_stdout = String::from_utf8_lossy(&read.stdout);
  let heading = guide
    .lines()
    .find(|line| line.starts_with("## "))
    .expect("guide should have a section heading");
  assert!(read_stdout.contains(heading.trim_start_matches("## ")), "{read_stdout}");
}

fn edit_ok(project: &TempProject, args: &[&str]) -> Output {
  let output = run_calcit(&project.snapshot(), args);
  assert!(output.status.success(), "{args:?}: {}", String::from_utf8_lossy(&output.stderr));
  output
}

fn project_with_examples(name: &str) -> TempProject {
  let project = TempProject::create(name);
  edit_ok(
    &project,
    &[
      "edit",
      "examples",
      "app.main/items",
      "--code",
      "quote $ count items\nquote $ first items\nquote $ last items",
    ],
  );
  project
}

#[test]
fn rm_example_alias_matches_edit_examples() {
  let read = |project: &TempProject| fs::read_to_string(project.snapshot()).expect("snapshot should read");

  let legacy = project_with_examples("rm-example");
  let output = edit_ok(&legacy, &["edit", "rm-example", "app.main/items", "1"]);
  let hints = hint_lines(&output);
  assert_eq!(hints.len(), 1, "{hints:?}");
  assert!(hints[0].contains("`calcit edit examples <ns/def>`"), "{}", hints[0]);

  let replacement = project_with_examples("examples");
  let output = edit_ok(
    &replacement,
    &[
      "edit",
      "examples",
      "app.main/items",
      "--code",
      "quote $ count items\nquote $ last items",
    ],
  );
  assert!(hint_lines(&output).is_empty());
  assert_eq!(read(&legacy), read(&replacement));
  assert!(!read(&legacy).contains("first items"), "{}", read(&legacy));

  // The staged form prints its own hint and still removes the example.
  let staged = project_with_examples("rm-example-transaction");
  let output = edit_ok(
    &staged,
    &["edit", "transaction", "--code", r#"[["edit","rm-example","app.main/items","1"]]"#],
  );
  let stderr = String::from_utf8_lossy(&output.stderr);
  let hints = stderr
    .lines()
    .filter(|line| line.starts_with("[Deprecated] transaction operation 1 uses `edit rm-example`"))
    .collect::<Vec<_>>();
  assert_eq!(hints.len(), 1, "{stderr}");
  assert_eq!(read(&staged), read(&replacement));
}

#[test]
fn ns_doc_alias_matches_edit_ns() {
  let read = |project: &TempProject| fs::read_to_string(project.snapshot()).expect("snapshot should read");

  let legacy = TempProject::create("ns-doc");
  let output = edit_ok(&legacy, &["edit", "ns-doc", "app.main", "Demo namespace."]);
  let hints = hint_lines(&output);
  assert_eq!(hints.len(), 1, "{hints:?}");
  assert!(hints[0].contains("`calcit edit ns <ns>`"), "{}", hints[0]);

  let replacement = TempProject::create("edit-ns");
  let view = edit_ok(&replacement, &["query", "ns", "app.main", "--format", "cirru"]);
  let view = String::from_utf8_lossy(&view.stdout).into_owned();
  assert!(view.contains("'NsEntry (:doc |)"), "{view}");
  let changed = view.replace("'NsEntry (:doc |)", "'NsEntry (:doc \"|Demo namespace.\")");
  let output = edit_ok(&replacement, &["edit", "ns", "app.main", "--code", &changed]);
  assert!(hint_lines(&output).is_empty());
  assert_eq!(read(&legacy), read(&replacement));
  assert!(read(&legacy).contains("Demo namespace."), "{}", read(&legacy));
}

#[test]
fn call_graph_json_stdout_is_one_document_and_honors_sort() {
  let snapshot = Path::new("calcit/test.cirru");
  for args in [
    &["analyze", "call-graph", "--format", "json"][..],
    &["analyze", "call-graph", "--counts", "--format", "json"][..],
    &["analyze", "count-calls", "--format", "json"][..],
  ] {
    let output = run_calcit(snapshot, args);
    assert!(output.status.success(), "{args:?}: {}", String::from_utf8_lossy(&output.stderr));
    serde_json::from_slice::<serde_json::Value>(&output.stdout).expect("stdout should be exactly one JSON document");
    assert!(
      String::from_utf8_lossy(&output.stderr).contains(" from: app.main/main!"),
      "{args:?} should keep the status line on stderr"
    );
  }

  let names = |args: &[&str]| {
    let output = run_calcit(snapshot, args);
    assert!(output.status.success(), "{args:?}: {}", String::from_utf8_lossy(&output.stderr));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("one JSON document");
    report["counts"]
      .as_array()
      .expect("counts array")
      .iter()
      .map(|entry| {
        (
          entry["count"].as_u64().expect("count"),
          entry["fqn"].as_str().expect("fqn").to_owned(),
        )
      })
      .collect::<Vec<_>>()
  };
  let by_name = names(&["analyze", "call-graph", "--counts", "--sort", "name", "--format", "json"]);
  assert!(by_name.windows(2).all(|pair| pair[0].1 <= pair[1].1), "{by_name:?}");
  let by_count = names(&["analyze", "call-graph", "--counts", "--format", "json"]);
  assert!(
    by_count
      .windows(2)
      .all(|pair| pair[0].0 > pair[1].0 || (pair[0].0 == pair[1].0 && pair[0].1 <= pair[1].1)),
    "{by_count:?}"
  );
  assert_eq!(by_count.len(), by_name.len());

  // An unknown sort key fails the same way in both forms, before any report.
  for args in [
    &["analyze", "call-graph", "--counts", "--sort", "size"][..],
    &["analyze", "count-calls", "--sort", "size"][..],
    &["analyze", "count-calls", "--sort", "size", "--format", "json"][..],
  ] {
    let output = run_calcit(snapshot, args);
    assert_eq!(output.status.code(), Some(1), "{args:?} should be rejected");
    assert!(output.stdout.is_empty(), "{args:?} should not report on stdout");
    assert!(
      String::from_utf8_lossy(&output.stderr).contains("expected `count` or `name`"),
      "{}",
      String::from_utf8_lossy(&output.stderr)
    );
  }
}

#[test]
fn docs_read_line_range_past_the_end_is_an_error() {
  let home = GuidebookHome::create("past-end");
  for args in [
    &["docs", "read", "cirru-syntax.md", "--start", "99999"][..],
    &["docs", "read-lines", "cirru-syntax.md", "--start", "99999"][..],
  ] {
    let output = home.run(args);
    assert_eq!(
      output.status.code(),
      Some(1),
      "{args:?}: {}",
      String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty(), "{args:?} should not report on stdout");
    assert!(
      String::from_utf8_lossy(&output.stderr).contains("past the end"),
      "{}",
      String::from_utf8_lossy(&output.stderr)
    );
  }
  // A range that runs past the end is clipped rather than rejected.
  let clipped = home.run(&["docs", "read", "cirru-syntax.md", "--start", "1", "--lines", "99999"]);
  assert!(clipped.status.success(), "{}", String::from_utf8_lossy(&clipped.stderr));
  assert!(String::from_utf8_lossy(&clipped.stdout).contains("End of document."));
}

#[test]
fn tree_insert_and_swap_match_the_legacy_leaves() {
  let read = |project: &TempProject| fs::read_to_string(project.snapshot()).expect("snapshot should read");
  let cases: [(&str, &[&str], &[&str], &str); 6] = [
    (
      "before",
      &["tree", "insert-before", "app.main/items", "--path", "@2.2", "--code", "quote 9"],
      &[
        "tree",
        "insert",
        "app.main/items",
        "--at",
        "before",
        "--path",
        "@2.2",
        "--code",
        "quote 9",
      ],
      "tree insert --at before",
    ),
    (
      "after",
      &["tree", "insert-after", "app.main/items", "--path", "@2.2", "--code", "quote 9"],
      &[
        "tree",
        "insert",
        "app.main/items",
        "--at",
        "after",
        "--path",
        "@2.2",
        "--code",
        "quote 9",
      ],
      "tree insert --at after",
    ),
    (
      "prepend",
      &["tree", "insert-child", "app.main/items", "--path", "@2", "--code", "quote 9"],
      &[
        "tree",
        "insert",
        "app.main/items",
        "--at",
        "prepend-child",
        "--path",
        "@2",
        "--code",
        "quote 9",
      ],
      "tree insert --at prepend-child",
    ),
    (
      "append",
      &["tree", "append-child", "app.main/items", "--path", "@2", "--code", "quote 9"],
      &[
        "tree",
        "insert",
        "app.main/items",
        "--at",
        "append-child",
        "--path",
        "@2",
        "--code",
        "quote 9",
      ],
      "tree insert --at append-child",
    ),
    (
      "next",
      &["tree", "swap-next", "app.main/items", "--path", "@2.1"],
      &["tree", "swap", "app.main/items", "--with", "next", "--path", "@2.1"],
      "tree swap --with next",
    ),
    (
      "prev",
      &["tree", "swap-prev", "app.main/items", "--path", "@2.2"],
      &["tree", "swap", "app.main/items", "--with", "prev", "--path", "@2.2"],
      "tree swap --with prev",
    ),
  ];
  for (name, old, new, replacement) in cases {
    let legacy = TempProject::create(&format!("legacy-{name}"));
    let merged = TempProject::create(&format!("merged-{name}"));
    let before = read(&legacy);
    let old_output = edit_ok(&legacy, old);
    let new_output = edit_ok(&merged, new);
    let hints = hint_lines(&old_output);
    assert_eq!(hints.len(), 1, "{old:?}: {hints:?}");
    assert!(hints[0].contains(replacement), "{}", hints[0]);
    assert!(hint_lines(&new_output).is_empty(), "{new:?} must not print a hint");
    assert!(
      !String::from_utf8_lossy(&old_output.stdout).contains("[Deprecated]"),
      "{old:?} must keep the hint off stdout"
    );
    assert_ne!(read(&legacy), before, "{old:?} should change the snapshot");
    assert_eq!(read(&legacy), read(&merged), "{old:?} vs {new:?}");
  }
}

#[test]
fn tree_insert_and_swap_reject_unknown_positions_without_writing() {
  let project = TempProject::create("bad-position");
  let before = fs::read_to_string(project.snapshot()).expect("snapshot should read");
  for args in [
    &[
      "tree",
      "insert",
      "app.main/items",
      "--at",
      "around",
      "--path",
      "@2.1",
      "--code",
      "quote 9",
    ][..],
    &["tree", "swap", "app.main/items", "--with", "up", "--path", "@2.1"][..],
  ] {
    let output = run_calcit(&project.snapshot(), args);
    assert!(!output.status.success(), "{args:?} should fail");
    assert!(String::from_utf8_lossy(&output.stderr).contains("Unsupported"), "{args:?}");
  }
  assert_eq!(before, fs::read_to_string(project.snapshot()).expect("snapshot should read"));
}

#[test]
fn tree_insert_and_swap_run_inside_transactions_and_legacy_ops_hint() {
  let project = TempProject::create("transaction-merged");
  let output = edit_ok(
    &project,
    &[
      "edit",
      "transaction",
      "--code",
      r#"[["tree","insert","app.main/items","--at","after","--path","@2.4","--code","quote 5"],["tree","swap","app.main/items","--with","prev","--path","@2.5"]]"#,
    ],
  );
  assert!(hint_lines(&output).is_empty());
  assert!(!String::from_utf8_lossy(&output.stderr).contains("[Deprecated]"));
  let content = fs::read_to_string(project.snapshot()).expect("snapshot should read");
  assert!(content.contains("[] 1 2 3 5 4"), "{content}");

  let legacy = TempProject::create("transaction-legacy");
  let output = edit_ok(
    &legacy,
    &[
      "edit",
      "transaction",
      "--code",
      r#"[["tree","insert-after","app.main/items","--path","@2.4","--code","quote 5"]]"#,
    ],
  );
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(
    stderr.contains("[Deprecated] transaction operation 1 uses `tree insert-after`") && stderr.contains("tree insert --at after"),
    "{stderr}"
  );
}

#[test]
fn host_procs_is_deprecated_without_replacement() {
  let output = run_calcit(Path::new("calcit/test.cirru"), &["query", "host-procs"]);
  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  let hints = hint_lines(&output);
  assert_eq!(hints.len(), 1, "{hints:?}");
  assert!(hints[0].contains("has no replacement"), "{}", hints[0]);
  assert!(!output.stdout.is_empty() && !String::from_utf8_lossy(&output.stdout).contains("[Deprecated]"));
}

fn snapshot_text(project: &TempProject) -> String {
  fs::read_to_string(project.snapshot()).expect("snapshot should read")
}

#[test]
fn tree_insert_and_swap_guard_expectations_and_root_paths() {
  let project = TempProject::create("guards");
  let before = snapshot_text(&project);
  for args in [
    &[
      "tree",
      "insert",
      "app.main/items",
      "--at",
      "after",
      "--path",
      "@2.1",
      "--expect",
      "quote 7",
      "--code",
      "quote 9",
    ][..],
    &[
      "tree",
      "insert-after",
      "app.main/items",
      "--path",
      "@2.1",
      "--expect",
      "quote 7",
      "--code",
      "quote 9",
    ][..],
  ] {
    let output = run_calcit(&project.snapshot(), args);
    assert!(!output.status.success(), "{args:?} should be rejected");
    assert!(String::from_utf8_lossy(&output.stderr).contains("guard failed"), "{args:?}");
    assert_eq!(before, snapshot_text(&project), "{args:?} must not write");
  }
  // The matching guard lets the same insert through.
  edit_ok(
    &project,
    &[
      "tree",
      "insert",
      "app.main/items",
      "--at",
      "after",
      "--path",
      "@2.1",
      "--expect",
      "quote 1",
      "--code",
      "quote 9",
    ],
  );
  assert!(snapshot_text(&project).contains("[] 1 9 2 3 4"));

  let project = TempProject::create("root");
  let before = snapshot_text(&project);
  for (args, command) in [
    (
      &[
        "tree",
        "insert",
        "app.main/items",
        "--at",
        "before",
        "--path",
        "",
        "--code",
        "quote 9",
      ][..],
      "tree insert --at before",
    ),
    (
      &[
        "tree",
        "insert",
        "app.main/items",
        "--at",
        "append-child",
        "--path",
        "",
        "--code",
        "quote 9",
      ][..],
      "tree insert --at append-child",
    ),
    (
      &["tree", "swap", "app.main/items", "--with", "next", "--path", ""][..],
      "tree swap --with next",
    ),
  ] {
    let output = run_calcit(&project.snapshot(), args);
    assert!(!output.status.success(), "{args:?} should fail at the root");
    let stderr = String::from_utf8_lossy(&output.stderr);
    for legacy in ["insert-before", "insert-after", "insert-child", "swap-next", "swap-prev"] {
      assert!(!stderr.contains(legacy), "{args:?} leaks legacy name `{legacy}`: {stderr}");
    }
    assert!(stderr.contains(command) || stderr.contains("path"), "{args:?}: {stderr}");
    assert_eq!(before, snapshot_text(&project), "{args:?} must not write");
  }
}

#[test]
fn tree_insert_reads_code_from_file() {
  let project = TempProject::create("file-input");
  let snippet = project.0.join("snippet.cirru");
  fs::write(&snippet, "quote 9\n").expect("snippet should write");
  edit_ok(
    &project,
    &[
      "tree",
      "insert",
      "app.main/items",
      "--at",
      "after",
      "--path",
      "@2.1",
      "--input-format",
      "cirru",
      "--file",
      snippet.to_str().expect("utf8 path"),
    ],
  );
  assert!(snapshot_text(&project).contains("[] 1 9 2 3 4"), "{}", snapshot_text(&project));
}

#[test]
fn tree_insert_and_swap_resolve_cursor_and_maintain_it() {
  let project = TempProject::create("cursor");
  edit_ok(&project, &["cursor", "set", "app.main/items", "--path", "@2.3"]);
  let cursor_path = |project: &TempProject| {
    let output = edit_ok(project, &["cursor", "show", "--format", "json"]);
    String::from_utf8_lossy(&output.stdout).to_string()
  };
  assert!(cursor_path(&project).contains("@2.3"), "{}", cursor_path(&project));

  // Inserting before the selected node shifts the saved coordinate.
  edit_ok(
    &project,
    &[
      "tree",
      "insert",
      "app.main/items",
      "--at",
      "before",
      "--path",
      "@2.1",
      "--code",
      "quote 8",
    ],
  );
  assert!(cursor_path(&project).contains("@2.4"), "{}", cursor_path(&project));

  // Swaps follow the selected subtree.
  edit_ok(&project, &["tree", "swap", "app.main/items", "--with", "next", "--path", "@2.4"]);
  assert!(cursor_path(&project).contains("@2.5"), "{}", cursor_path(&project));

  // `@cursor` works as both target and path for the merged commands.
  edit_ok(
    &project,
    &[
      "tree", "insert", "@cursor", "--at", "after", "--path", "@cursor", "--code", "quote 7",
    ],
  );
  assert!(snapshot_text(&project).contains("[] 8 1 2 4 3 7"), "{}", snapshot_text(&project));
  edit_ok(&project, &["tree", "swap", "@cursor", "--with", "prev", "--path", "@cursor"]);
  assert!(snapshot_text(&project).contains("[] 8 1 2 3 4 7"), "{}", snapshot_text(&project));
}

/// One cursor step: the legacy spelling, the merged spelling, and the replacement the hint names.
type CursorStep<'a> = (&'a [&'a str], &'a [&'a str], &'a str);

fn cursor_project(name: &str) -> TempProject {
  let project = TempProject::create(name);
  edit_ok(
    &project,
    &["edit", "def", "app.main/nested", "--code", "quote $ def nested $ [] 1 ([] 2 3) 4"],
  );
  project
}

fn cursor_json(project: &TempProject) -> String {
  // The state file path names the temporary project, which differs between the two replays.
  String::from_utf8_lossy(&edit_ok(project, &["cursor", "show", "--format", "json"]).stdout)
    .replace(&project.0.display().to_string(), "<project>")
}

/// Replays the legacy and merged spellings on two fresh projects and requires equal snapshots
/// and cursor state after every step, one hint per legacy step, and none for the merged one.
fn assert_cursor_steps_match(name: &str, setup: &[&str], steps: &[CursorStep]) {
  let legacy = cursor_project(&format!("{name}-legacy"));
  let merged = cursor_project(&format!("{name}-merged"));
  edit_ok(&legacy, setup);
  edit_ok(&merged, setup);
  for (old, new, replacement) in steps {
    let old_output = edit_ok(&legacy, old);
    let new_output = edit_ok(&merged, new);
    let hints = hint_lines(&old_output);
    if replacement.is_empty() {
      assert!(hints.is_empty(), "{old:?} is not deprecated: {hints:?}");
    } else {
      assert_eq!(hints.len(), 1, "{old:?} should print exactly one hint: {hints:?}");
      assert!(
        hints[0].contains(replacement),
        "{old:?} hint should name `{replacement}`: {}",
        hints[0]
      );
    }
    assert!(hint_lines(&new_output).is_empty(), "{new:?} must not print a hint");
    assert!(
      !String::from_utf8_lossy(&old_output.stdout).contains("[Deprecated]"),
      "{old:?} must keep the hint off stdout"
    );
    assert_eq!(snapshot_text(&legacy), snapshot_text(&merged), "{old:?} vs {new:?}");
    assert_eq!(cursor_json(&legacy), cursor_json(&merged), "{old:?} vs {new:?}");
  }
}

#[test]
fn cursor_move_matches_the_legacy_navigation_leaves() {
  assert_cursor_steps_match(
    "move",
    &["cursor", "set", "app.main/items", "--path", "@2"],
    &[
      (&["cursor", "child", "1"], &["cursor", "move", "child", "1"], "cursor move child"),
      (
        &["cursor", "next", "--count", "2"],
        &["cursor", "move", "next", "--count", "2"],
        "cursor move next",
      ),
      (&["cursor", "prev"], &["cursor", "move", "prev"], "cursor move prev"),
      (&["cursor", "parent"], &["cursor", "move", "parent"], "cursor move parent"),
      (
        &["cursor", "child", "--last"],
        &["cursor", "move", "child", "--last"],
        "cursor move child",
      ),
      (&["cursor", "parent"], &["cursor", "move", "parent"], "cursor move parent"),
      (&["cursor", "child"], &["cursor", "move", "child"], "cursor move child"),
      (
        &["cursor", "forward", "--count", "2"],
        &["cursor", "move", "forward", "--count", "2"],
        "cursor move forward",
      ),
      (&["cursor", "backward"], &["cursor", "move", "backward"], "cursor move backward"),
      (&["cursor", "back"], &["cursor", "move", "back"], "cursor move back"),
    ],
  );
}

#[test]
fn cursor_anchor_matches_clear_anchor_and_region() {
  assert_cursor_steps_match(
    "anchor",
    &["cursor", "set", "app.main/items", "--path", "@2.1"],
    &[
      (&["cursor", "anchor"], &["cursor", "anchor", "set"], ""),
      (
        &["cursor", "next", "--count", "2"],
        &["cursor", "move", "next", "--count", "2"],
        "cursor move next",
      ),
      (&["cursor", "region"], &["cursor", "anchor", "show"], "cursor anchor show"),
      (
        &["cursor", "region", "--format", "json"],
        &["cursor", "anchor", "show", "--format", "json"],
        "cursor anchor show",
      ),
      (&["cursor", "clear-anchor"], &["cursor", "anchor", "clear"], "cursor anchor clear"),
    ],
  );
}

#[test]
fn cursor_marks_match_mark_goto_and_rm_mark() {
  assert_cursor_steps_match(
    "marks",
    &["cursor", "set", "app.main/items", "--path", "@2.1"],
    &[
      (
        &["cursor", "mark", "first"],
        &["cursor", "marks", "set", "first"],
        "cursor marks set",
      ),
      (
        &["cursor", "next", "--count", "2"],
        &["cursor", "move", "next", "--count", "2"],
        "cursor move next",
      ),
      (
        &["cursor", "mark", "later"],
        &["cursor", "marks", "set", "later"],
        "cursor marks set",
      ),
      (&["cursor", "goto", "first"], &["cursor", "marks", "go", "first"], "cursor marks go"),
      (&["cursor", "marks"], &["cursor", "marks", "list"], ""),
      (
        &["cursor", "rm-mark", "later"],
        &["cursor", "marks", "drop", "later"],
        "cursor marks drop",
      ),
    ],
  );
}

#[test]
fn cursor_clipboard_matches_copy_cut_paste_and_clear() {
  assert_cursor_steps_match(
    "clipboard",
    &["cursor", "set", "app.main/items", "--path", "@2.1"],
    &[
      (&["cursor", "copy"], &["cursor", "clipboard", "copy"], "cursor clipboard copy"),
      (
        &["cursor", "next", "--count", "2"],
        &["cursor", "move", "next", "--count", "2"],
        "cursor move next",
      ),
      (&["cursor", "paste"], &["cursor", "clipboard", "paste"], "cursor clipboard paste"),
      (
        &["cursor", "paste", "--at", "before"],
        &["cursor", "clipboard", "paste", "--at", "before"],
        "cursor clipboard paste",
      ),
      (&["cursor", "cut"], &["cursor", "clipboard", "cut"], "cursor clipboard cut"),
      (&["cursor", "clipboard"], &["cursor", "clipboard", "show"], ""),
      (
        &["cursor", "clear-clipboard"],
        &["cursor", "clipboard", "clear"],
        "cursor clipboard clear",
      ),
    ],
  );
}

#[test]
fn cursor_apply_matches_slurp_barf_and_duplicate_leaves() {
  assert_cursor_steps_match(
    "apply",
    &["cursor", "set", "app.main/nested", "--path", "@2.2"],
    &[
      (
        &["cursor", "slurp-next"],
        &["cursor", "apply", "slurp-next"],
        "cursor apply slurp-next",
      ),
      (
        &["cursor", "barf-last"],
        &["cursor", "apply", "barf-last"],
        "cursor apply barf-last",
      ),
      (
        &["cursor", "barf-first"],
        &["cursor", "apply", "barf-first"],
        "cursor apply barf-first",
      ),
      (
        &["cursor", "slurp-prev"],
        &["cursor", "apply", "slurp-prev"],
        "cursor apply slurp-prev",
      ),
      (
        &["cursor", "duplicate"],
        &["cursor", "apply", "duplicate"],
        "cursor apply duplicate",
      ),
      (
        &["cursor", "duplicate", "--at", "before"],
        &["cursor", "apply", "duplicate", "--at", "before"],
        "cursor apply duplicate",
      ),
      (&["cursor", "apply", "delete"], &["cursor", "apply", "delete"], ""),
    ],
  );
}

#[test]
fn cursor_merged_forms_reject_invalid_values_without_writing() {
  let project = cursor_project("cursor-invalid");
  edit_ok(&project, &["cursor", "set", "app.main/nested", "--path", "@2.2"]);
  edit_ok(&project, &["cursor", "copy"]);
  let snapshot_before = snapshot_text(&project);
  let cursor_before = cursor_json(&project);
  let cases: [(&[&str], &str); 24] = [
    (&["cursor", "move", "sideways"], "Unsupported `cursor move sideways`"),
    (&["cursor", "move", "parent", "--count", "2"], "does not accept --count"),
    (&["cursor", "move", "next", "--last"], "does not accept an index"),
    (&["cursor", "move", "next", "3"], "does not accept an index"),
    (&["cursor", "anchor", "drop"], "Unsupported `cursor anchor drop`"),
    (&["cursor", "anchor", "clear", "--format", "json"], "do not accept --format"),
    (&["cursor", "marks", "teleport"], "Unsupported `cursor marks teleport`"),
    (&["cursor", "marks", "set"], "requires a mark name"),
    (&["cursor", "marks", "list", "extra"], "does not accept a mark name"),
    (&["cursor", "clipboard", "yank"], "Unsupported `cursor clipboard yank`"),
    (&["cursor", "clipboard", "copy", "--at", "before"], "does not accept --at"),
    (&["cursor", "clipboard", "paste", "--at", "around"], "around"),
    (
      &["cursor", "apply", "duplicate", "--at", "around"],
      "Unsupported duplicate position",
    ),
    (
      &["cursor", "apply", "slurp-next", "--code", "quote 1"],
      "does not accept --file, --code",
    ),
    (&["cursor", "apply", "barf-last", "--at", "before"], "does not accept --at"),
    (&["cursor", "apply", "delete", "--at", "before"], "does not accept --at"),
    (&["cursor", "apply", "tidy"], "Unsupported cursor operation"),
    (&["cursor", "marks", "set", "a", "b"], "at most one mark name"),
    (&["cursor", "marks", "set", "a", "b", "c"], "at most one mark name"),
    (&["cursor", "marks", "drop"], "requires a mark name"),
    (&["cursor", "move", "prev", "2"], "does not accept an index"),
    (&["cursor", "marks", "set", "a", "--format", "json"], "does not accept --format"),
    (&["cursor", "marks", "go", "a", "--format", "json"], "does not accept --format"),
    (&["cursor", "clipboard", "paste", "--format", "json"], "does not accept --format"),
  ];
  for (args, expected) in cases {
    let output = run_calcit(&project.snapshot(), args);
    assert!(!output.status.success(), "{args:?} should fail");
    assert!(
      String::from_utf8_lossy(&output.stderr).contains(expected),
      "{args:?}: {}",
      String::from_utf8_lossy(&output.stderr)
    );
    assert!(hint_lines(&output).is_empty(), "{args:?} must not print a deprecation hint");
    assert_eq!(snapshot_text(&project), snapshot_before, "{args:?} must not write the snapshot");
    assert_eq!(cursor_json(&project), cursor_before, "{args:?} must not move the cursor");
  }
}

#[test]
fn cursor_clipboard_actions_reject_format_but_bare_forms_keep_it() {
  let project = cursor_project("cursor-format");
  edit_ok(&project, &["cursor", "set", "app.main/items", "--path", "@2.1"]);
  for action in ["copy", "cut", "clear"] {
    let output = run_calcit(&project.snapshot(), &["cursor", "clipboard", action, "--format", "edn"]);
    assert!(!output.status.success(), "clipboard {action} --format should fail");
    assert!(String::from_utf8_lossy(&output.stderr).contains("does not accept --format"));
  }
  edit_ok(&project, &["cursor", "clipboard", "copy"]);
  edit_ok(&project, &["cursor", "marks", "set", "here"]);
  for args in [
    &["cursor", "marks", "--format", "json"][..],
    &["cursor", "marks", "list", "--format", "json"][..],
    &["cursor", "clipboard", "--format", "json"][..],
    &["cursor", "clipboard", "show", "--format", "json"][..],
  ] {
    let output = edit_ok(&project, args);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
      stdout.trim_start().starts_with('{'),
      "{args:?} should print one JSON document: {stdout}"
    );
    assert!(hint_lines(&output).is_empty(), "{args:?} must not print a hint");
  }
}

#[test]
fn cursor_after_focus_works_with_the_merged_forms() {
  let project = cursor_project("cursor-focus");
  edit_ok(&project, &["cursor", "set", "app.main/nested", "--path", "@2.2"]);
  for args in [
    &["cursor", "move", "next"][..],
    &["cursor", "marks", "set", "m"][..],
    &["cursor", "move", "prev"][..],
    &["cursor", "marks", "go", "m"][..],
    &["cursor", "apply", "duplicate"][..],
    &["cursor", "clipboard", "copy"][..],
    &["cursor", "clipboard", "paste", "--at", "before"][..],
  ] {
    let output = Command::new(env!("CARGO_BIN_EXE_calcit"))
      .env("NO_COLOR", "1")
      .args(["--tips-level", "none", "--cursor-after", "focus"])
      .arg(project.snapshot())
      .args(args)
      .output()
      .expect("calcit command should run");
    assert!(output.status.success(), "{args:?}: {}", String::from_utf8_lossy(&output.stderr));
    assert!(hint_lines(&output).is_empty(), "{args:?} must not print a hint");
  }
}
