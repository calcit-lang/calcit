use std::fs;
use std::path::Path;
use std::process::{Command, Output};

/// The default entry loads no modules; the `page` entry also loads
/// `query-entry-page-only-module.cirru`, which defines `page-icons.core`.
const FIXTURE: &str = "tests/fixtures/query-entry-page-only.cirru";
const PAGE_ONLY_TARGET: &str = "page-icons.core/comp-icon";

fn run_query(entry: Option<&str>, args: &[&str]) -> Output {
  let mut command = Command::new(env!("CARGO_BIN_EXE_calcit"));
  command.env("NO_COLOR", "1").arg("--tips-level").arg("none");
  if let Some(entry) = entry {
    command.arg("--entry").arg(entry);
  }
  command
    .arg(FIXTURE)
    .arg("query")
    .args(args)
    .output()
    .expect("calcit command should run")
}

fn stdout(output: &Output) -> String {
  String::from_utf8(output.stdout.clone()).expect("stdout should be UTF-8")
}

fn assert_success(output: &Output) -> String {
  assert!(
    output.status.success(),
    "command failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  stdout(output)
}

fn parse_single_json(output: &Output) -> serde_json::Value {
  serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
    panic!(
      "stdout should be one JSON document: {error}\nstdout:\n{}",
      String::from_utf8_lossy(&output.stdout)
    )
  })
}

/// #2223: `--entry page query def/schema` must read dependencies that only the page entry loads.
#[test]
fn selected_entry_resolves_page_only_dependency_for_def_and_schema() {
  let schema = assert_success(&run_query(Some("page"), &["schema", PAGE_ONLY_TARGET]));
  assert!(schema.contains(":: 'Fn"), "schema should be shown:\n{schema}");

  let def = assert_success(&run_query(Some("page"), &["def", PAGE_ONLY_TARGET, "--format", "cirru"]));
  assert!(def.contains("defn comp-icon"), "definition should be shown:\n{def}");

  for args in [
    ["peek", PAGE_ONLY_TARGET].as_slice(),
    ["examples", PAGE_ONLY_TARGET].as_slice(),
    ["defs", "page-icons.core"].as_slice(),
  ] {
    assert_success(&run_query(Some("page"), args));
  }
}

#[test]
fn selected_entry_keeps_structured_output_a_single_document() {
  for format in ["json", "edn"] {
    let output = run_query(Some("page"), &["schema", PAGE_ONLY_TARGET, "--format", format]);
    let out = assert_success(&output);
    if format == "json" {
      let value = parse_single_json(&output);
      assert_eq!(value["command"], "query.schema");
      assert_eq!(value["data"]["id"], PAGE_ONLY_TARGET);
      assert_eq!(value["data"]["source"], "dependency");
    } else {
      cirru_edn::parse(&out).unwrap_or_else(|error| panic!("stdout should be one EDN document: {error}\n{out}"));
      assert!(out.contains(PAGE_ONLY_TARGET), "EDN should name the target:\n{out}");
    }

    let defs = run_query(Some("page"), &["defs", "page-icons.core", "--format", format]);
    let text = assert_success(&defs);
    if format == "json" {
      let value = parse_single_json(&defs);
      assert_eq!(value["command"], "query.defs");
      assert_eq!(
        value["data"]["definitions"],
        serde_json::json!([{ "name": "comp-icon" }, { "name": "main!" }])
      );
      assert_eq!(value["data"]["count"], 2);
    } else {
      cirru_edn::parse(&text).expect("dependency defs should be one EDN document");
      assert!(text.contains("comp-icon"));
    }
  }
}

/// Without `--entry` the default entry is used, which does not load the page-only module.
#[test]
fn default_entry_does_not_see_page_only_dependency() {
  let output = run_query(None, &["schema", PAGE_ONLY_TARGET]);
  assert!(
    !output.status.success(),
    "default entry must not resolve page-only items:\n{}",
    stdout(&output)
  );
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(stderr.contains("page-icons.core"), "error should name the namespace:\n{stderr}");

  let explicit_default = run_query(Some("default"), &["def", PAGE_ONLY_TARGET]);
  assert!(
    !explicit_default.status.success(),
    "explicit default entry must not resolve page-only items"
  );

  assert_success(&run_query(None, &["def", "app.main/main!"]));
}

#[test]
fn unknown_entry_is_rejected_even_for_project_definitions() {
  let output = run_query(Some("missing"), &["def", "app.main/main!"]);
  assert!(!output.status.success(), "unknown entry must fail:\n{}", stdout(&output));
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(
    stderr.contains("Unknown entry `missing`") && stderr.contains("default, page"),
    "error should name the entry and the available ones:\n{stderr}"
  );

  let structured = run_query(Some("missing"), &["def", "app.main/main!", "--format", "json"]);
  assert!(!structured.status.success());
  let value = parse_single_json(&structured);
  assert_eq!(value["diagnostics"][0]["severity"], "error");
  assert!(
    value["diagnostics"][0]["message"]
      .as_str()
      .is_some_and(|message| message.contains("Unknown entry `missing`")),
    "structured failure should carry the entry error: {value}"
  );

  let defs = run_query(Some("missing"), &["defs", "app.main", "--format", "json"]);
  assert!(!defs.status.success());
  assert_eq!(parse_single_json(&defs)["command"], "query.defs");
}

fn run_isolated(directory: &Path, args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .current_dir(directory)
    .args(["--tips-level", "none"])
    .args(args)
    .output()
    .expect("isolated calcit command should run")
}

fn configure_isolated(directory: &Path, snapshot: &str, operations: &str) {
  let preview = run_isolated(
    directory,
    &[
      snapshot,
      "edit",
      "transaction",
      "--code",
      operations,
      "--dry-run",
      "--format",
      "json",
    ],
  );
  assert_success(&preview);
  let preview = parse_single_json(&preview);
  let revision = preview["scoped_revision"]
    .as_str()
    .expect("preview should provide a scoped revision");
  assert_success(&run_isolated(
    directory,
    &[
      snapshot,
      "edit",
      "transaction",
      "--code",
      operations,
      "--expect-revision",
      revision,
      "--format",
      "json",
    ],
  ));
}

/// #2406: module loading is a host/filesystem contract; observable assertions
/// remain attached to the generated Calcit definition, not duplicated in Rust.
#[test]
fn library_default_graph_is_independent_of_the_demo_entry() {
  let directory = tempfile::tempdir().expect("temporary consumer should create");
  let root = directory.path();
  let snapshot = root.join("library.cirru");
  fs::copy(FIXTURE, &snapshot).expect("copy opaque library fixture");
  let operations = serde_json::json!([
    ["config", "set", "target", "native"],
    ["config", "set", "target", "native", "--entry", "page"],
    ["config", "set", "init-fn", "app.demo/main!", "--entry", "page"],
    ["config", "set", "reload-fn", "app.demo/main!", "--entry", "page"],
    ["edit", "add-ns", "app.core"],
    [
      "edit",
      "def",
      "app.core/increment",
      "--input-format",
      "cirru",
      "--code",
      "quote $ defn increment (n) (&+ n 1)"
    ],
    [
      "edit",
      "schema",
      "app.core/increment",
      "--input-format",
      "cirru",
      "--code",
      "quote $ Fn Number (-> Number)"
    ],
    [
      "edit",
      "add-test",
      "app.core/increment",
      "increments-number",
      "--input-format",
      "cirru",
      "--code",
      "quote $ assert= 42 (increment 41)"
    ],
    ["edit", "add-ns", "app.demo"],
    [
      "edit",
      "add-import",
      "app.demo",
      "--input-format",
      "cirru",
      "--code",
      "quote $ page-icons.core :refer $ comp-icon"
    ],
    [
      "edit",
      "def",
      "app.demo/main!",
      "--input-format",
      "cirru",
      "--code",
      "quote $ defn main! () (assert= |icon:demo (comp-icon |demo)) &unit"
    ],
    [
      "edit",
      "schema",
      "app.demo/main!",
      "--input-format",
      "cirru",
      "--code",
      "quote $ Fn (-> Unit)"
    ]
  ])
  .to_string();
  configure_isolated(root, "library.cirru", &operations);

  assert_success(&run_isolated(
    root,
    &["eval", "--dep", "./library.cirru", "assert= 42 (app.core/increment 41)"],
  ));
  assert_success(&run_isolated(
    root,
    &["library.cirru", "test", "app.core/increment", "--require-match"],
  ));
  assert_success(&run_isolated(root, &["library.cirru", "--check-only", "--ns", "app.core"]));
  let wrong_type = run_isolated(root, &["eval", "--dep", "./library.cirru", "app.core/increment |wrong"]);
  assert!(
    !wrong_type.status.success(),
    "isolating the library must retain its Number input contract"
  );
  let diagnostic = format!("{}{}", stdout(&wrong_type), String::from_utf8_lossy(&wrong_type.stderr));
  assert!(
    diagnostic.to_lowercase().contains("number"),
    "failure must retain the Number contract: {diagnostic}"
  );

  let replay = Command::new("node")
    .current_dir(env!("CARGO_MANIFEST_DIR"))
    .env("CALCIT_BIN", env!("CARGO_BIN_EXE_calcit"))
    .args([
      "scripts/run-core-tests.mjs",
      "--snapshot",
      snapshot.to_str().expect("fixture path should be UTF-8"),
      "--target",
      "app.core/increment",
      "--backend",
      "native,js",
    ])
    .output()
    .expect("shared native/JS replay should run");
  let replay = assert_success(&replay);
  assert!(
    replay.contains("native: ran 1 tests") && replay.contains("js: ran 1 tests"),
    "{replay}"
  );

  // Unlike eval, this root program actually selects its named entry. The
  // dependency's equally named entry still must not load its missing module.
  fs::copy("tests/fixtures/query-entry-page-only-module.cirru", root.join("consumer.cirru")).expect("copy independent consumer");
  let operations = serde_json::json!([
    [
      "edit",
      "def",
      "page-icons.core/main!",
      "--overwrite",
      "--input-format",
      "cirru",
      "--code",
      "quote $ defn main! () (assert= 42 (app.core/increment 41)) &unit"
    ],
    ["config", "add-entry", "page", "--from", "default", "--target", "native"],
    ["config", "add-module", "./library.cirru", "--entry", "page"]
  ])
  .to_string();
  configure_isolated(root, "consumer.cirru", &operations);
  assert_success(&run_isolated(root, &["--entry", "page", "consumer.cirru", "--check-only"]));
  assert_success(&run_isolated(root, &["--entry", "page", "consumer.cirru"]));
  let default_consumer = run_isolated(root, &["consumer.cirru", "--check-only"]);
  assert!(
    !default_consumer.status.success(),
    "the consumer default must not silently inherit the page module graph"
  );
  assert!(String::from_utf8_lossy(&default_consumer.stderr).contains("app.core"));
  let missing_demo = run_isolated(root, &["--entry", "page", "library.cirru", "--check-only"]);
  assert!(
    !missing_demo.status.success(),
    "an explicitly selected demo must not ignore its missing module"
  );
  assert!(String::from_utf8_lossy(&missing_demo.stderr).contains("query-entry-page-only-module.cirru"));
  let incomplete_all = run_isolated(root, &["library.cirru", "--check-only", "--all-defs"]);
  assert!(
    !incomplete_all.status.success(),
    "library-only checks must not masquerade as whole-project validation"
  );
  let diagnostic = format!("{}{}", stdout(&incomplete_all), String::from_utf8_lossy(&incomplete_all.stderr));
  assert!(
    diagnostic.contains("page-icons.core"),
    "whole-project check must expose the demo dependency: {diagnostic}"
  );

  fs::copy(
    "tests/fixtures/query-entry-page-only-module.cirru",
    root.join("query-entry-page-only-module.cirru"),
  )
  .expect("install the actual demo dependency");
  assert_success(&run_isolated(
    root,
    &["--entry", "page", "library.cirru", "--check-only", "--all-defs"],
  ));
  assert_success(&run_isolated(
    root,
    &[
      "--entry",
      "page",
      "library.cirru",
      "fix",
      "--workflow",
      "strict",
      "--verify",
      "--format",
      "json",
    ],
  ));
  assert_success(&run_isolated(root, &["--entry", "page", "library.cirru"]));
}
