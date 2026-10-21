use std::process::{Command, Output};

const FIXTURE: &str = "tests/fixtures/query-usages-alias.cirru";

fn run_query_usages(target: &str) -> String {
  let output = run_query(&["usages", target]);
  String::from_utf8(output.stdout).expect("stdout should be UTF-8")
}

fn run_query(args: &[&str]) -> Output {
  let output: Output = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .arg(FIXTURE)
    .arg("query")
    .args(args)
    .output()
    .expect("calcit command should run");
  assert!(
    output.status.success(),
    "command failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  output
}

fn usages_count_line(out: &str) -> &str {
  out.lines().find(|line| line.starts_with("Usages:")).unwrap_or_default()
}

/// #2236: callers that import the target namespace with `:as` must be reported,
/// alongside `:refer`, fully qualified and same-namespace references.
#[test]
fn usages_resolve_namespace_alias_refer_and_qualified_calls() {
  let out = run_query_usages("app.schema/decode-story");

  assert_eq!(usages_count_line(&out), "Usages: 6", "unexpected usages report:\n{out}");
  for expected in [
    "app.alias-caller/reading-list [code]",
    "app.alias-caller/reading-list [tests]",
    "app.refer-caller/refer-call [code]",
    "app.refer-caller/qualified-call [code]",
    "app.schema/message-input [code]",
    "app.main/main! [code]",
  ] {
    assert!(out.contains(expected), "missing `{expected}` in:\n{out}");
  }
  assert!(out.contains("schema/decode-story"), "alias call context should be shown:\n{out}");
  // Only `:as` is imported there, so the bare parameter name is not a reference.
  assert!(!out.contains("unrelated-bare"), "bare name without :refer must not match:\n{out}");
}

/// Schema leaves are quoted symbols; alias, refer and qualified spellings must all match.
#[test]
fn usages_resolve_quoted_schema_references() {
  let out = run_query_usages("app.schema/Story");

  assert_eq!(usages_count_line(&out), "Usages: 3", "unexpected usages report:\n{out}");
  for expected in [
    "app.alias-caller/alias-story [schema]",
    "app.refer-caller/refer-story [schema]",
    "app.refer-caller/qualified-story [schema]",
  ] {
    assert!(out.contains(expected), "missing `{expected}` in:\n{out}");
  }
}

#[test]
fn zero_usages_warns_that_the_query_is_not_a_proof() {
  let out = run_query_usages("app.alias-caller/unrelated-bare");

  assert_eq!(usages_count_line(&out), "Usages: 0", "unexpected usages report:\n{out}");
  assert!(
    out.contains("Source-reference candidates: unexpanded macros") && out.contains("zero usages is not proof of non-use"),
    "zero result should state its limits:\n{out}"
  );
}

#[test]
fn usages_exclude_local_bindings_and_reads_without_losing_global_initializers() {
  let out = run_query_usages("app.shadow/shadow-sentinel");
  assert_eq!(usages_count_line(&out), "Usages: 5", "{out}");
  assert!(
    out.contains("Source-reference candidates"),
    "positive results should state their limits:\n{out}"
  );
  for expected in [
    "global-read",
    "initializer-read",
    "local-head-read",
    "qualified-read",
    "restored-read",
  ] {
    assert!(out.contains(&format!("app.shadow/{expected} [code]")), "{out}");
  }
  for local in [
    "parameter-shadow",
    "let-shadow",
    "sequential-shadow",
    "alias-let-shadow",
    "loop-shadow",
    "quoted-data",
  ] {
    assert!(!out.contains(&format!("app.shadow/{local} [code]")), "{out}");
  }
  assert!(!out.contains("[tests]"), "the attached let test only reads its own binding:\n{out}");
  assert!(
    out.contains("at [@3.1.0.1]"),
    "initializer use should retain its exact source position:\n{out}"
  );
  assert!(!out.contains("[@3.1.0.0]"), "the binding name is not a reference:\n{out}");
}

#[test]
fn usages_keep_implicit_core_fallback_but_not_a_same_named_parameter() {
  let out = run_query_usages("calcit.core/identity");
  assert_eq!(usages_count_line(&out), "Usages: 2", "{out}");
  assert!(out.contains("app.core-fallback/implicit [code]"), "{out}");
  assert!(out.contains("app.core-fallback/qualified [code]"), "{out}");
  assert!(!out.contains("app.core-fallback/shadowed [code]"), "{out}");
}

#[test]
fn usages_keep_core_binding_operators_but_not_a_local_callable_named_let() {
  let out = run_query_usages("calcit.core/let");
  for expected in ["let-shadow", "sequential-shadow", "alias-let-shadow", "initializer-read"] {
    assert!(out.contains(&format!("app.shadow/{expected} [code]")), "{out}");
  }
  assert!(!out.contains("app.shadow/local-head-read [code]"), "{out}");
  assert!(out.contains("app.shadow/global-read [tests]"), "{out}");

  let out = run_query_usages("calcit.core/quote");
  assert!(out.contains("app.shadow/quoted-data [code]"), "{out}");
  let out = run_query_usages("calcit.core/defn");
  assert!(out.contains("app.shadow/global-read [code]"), "{out}");
}

#[test]
fn context_uses_the_same_lexical_reference_collection() {
  let output = run_query(&["context", "app.shadow/shadow-sentinel", "--format", "json"]);
  let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("context stdout should be one JSON document");
  assert_eq!(value["data"]["usages"]["total"], 5, "{value}");
  for usage in value["data"]["usages"]["items"].as_array().unwrap() {
    let id = usage["id"].as_str().unwrap();
    assert!(
      [
        "global-read",
        "initializer-read",
        "local-head-read",
        "qualified-read",
        "restored-read"
      ]
      .iter()
      .any(|name| id == format!("app.shadow/{name}")),
      "unexpected context reference: {usage}"
    );
    assert_eq!(usage["area"], "code");
  }
}
