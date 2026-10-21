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
}
