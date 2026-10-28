//! `query type-at` on source symbols that the compiler resolves to another
//! definition (#2329). Compiled imports carry no source coordinates, so the
//! query recovers the reference from the compiler's own resolution trace.
use std::process::{Command, Output};

/// `app.main` imports `app.ui` both by alias and by `:refer`. `defcomp` wraps
/// its body in `let`/`do` like Respo's macro, and `app.ui/noisy` prints when
/// it is evaluated.
const FIXTURE: &str = "tests/fixtures/type-at-definition-reference.cirru";

fn type_at(target: &str, path: &str) -> (Output, serde_json::Value) {
  let output = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .arg(FIXTURE)
    .args(["query", "type-at", target, "--path", path, "--format", "json"])
    .output()
    .expect("calcit command should run");
  assert!(
    output.status.success(),
    "type-at {target} {path} failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  let report = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
    panic!(
      "stdout should be one JSON document: {error}\nstdout:\n{}",
      String::from_utf8_lossy(&output.stdout)
    )
  });
  (output, report)
}

fn diagnostic_codes(report: &serde_json::Value) -> Vec<String> {
  report["diagnostics"]
    .as_array()
    .expect("diagnostics array")
    .iter()
    .map(|diagnostic| diagnostic["code"].as_str().expect("diagnostic code").to_owned())
    .collect()
}

fn has_next(report: &serde_json::Value, command: &str) -> bool {
  report["next"]
    .as_array()
    .expect("next array")
    .iter()
    .any(|item| item.as_str().is_some_and(|text| text.ends_with(command)))
}

#[test]
fn imported_constants_report_their_declared_contract_inside_and_outside_macros() {
  for (target, path, expression, reference, macro_expanded) in [
    ("app.main/comp-styled", "@3.1", "ui/global", "app.ui/global", true),
    ("app.main/comp-styled", "@3.2", "column", "app.ui/column", true),
    ("app.main/plain-styled", "@3.1", "ui/global", "app.ui/global", false),
    ("app.main/plain-styled", "@3.2", "column", "app.ui/column", false),
  ] {
    let (_, report) = type_at(target, path);
    let context = format!("{target} {path}");
    assert_eq!(report["data"]["expression"], expression, "{context}");
    assert_eq!(report["data"]["inferred_type"], "Map Tag Number", "{context}");
    assert_eq!(report["data"]["confidence"], "exact", "{context}");
    assert_eq!(diagnostic_codes(&report), Vec::<String>::new(), "{context}");
    let evidence = &report["data"]["evidence"][0];
    assert_eq!(evidence["kind"], "definition-schema", "{context}");
    let detail = evidence["detail"].as_str().expect("evidence detail");
    assert!(
      detail.contains(&format!("declared contract of `{reference}`")),
      "{context}: {detail}"
    );
    assert!(detail.contains("not a call-site instantiation proof"), "{context}: {detail}");
    assert_eq!(detail.contains("app.main/defcomp"), macro_expanded, "{context}: {detail}");
    assert!(
      has_next(&report, &format!("query def '{reference}'")),
      "{context}: {}",
      report["next"]
    );
  }
}

#[test]
fn definition_contract_lookup_does_not_evaluate_the_definition() {
  let (output, report) = type_at("app.main/comp-styled", "@3.3");
  assert_eq!(report["data"]["inferred_type"], "Map Tag Number");
  let printed = format!(
    "{}{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  assert!(!printed.contains("SIDE EFFECT FROM DEFINITION"), "{printed}");
}

#[test]
fn lexical_bindings_shadow_same_named_imports() {
  // `column` is a parameter and `global` a `&let` binding, both inside `defcomp`.
  for (path, expected) in [("@3.2.1", "Dynamic"), ("@3.2.2", "Number")] {
    let (_, report) = type_at("app.main/shadowed", path);
    assert_eq!(report["data"]["inferred_type"], expected, "{path}");
    let detail = report["data"]["evidence"][0]["detail"].as_str().expect("evidence detail");
    assert!(!detail.contains("declared contract"), "{path}: {detail}");
    assert!(!has_next(&report, "query def 'app.ui/column'") && !has_next(&report, "query def 'app.ui/global'"));
  }
}

#[test]
fn open_and_quoted_references_stay_unproven() {
  // A Dynamic declaration is reported as declared, without inventing members.
  let (_, opaque) = type_at("app.main/opaque-styled", "@3.1.1");
  assert_eq!(opaque["data"]["inferred_type"], "Dynamic");
  assert_eq!(opaque["data"]["confidence"], "unknown");
  assert!(has_next(&opaque, "query def 'app.ui/opaque'"));

  // Quoted source is data, so the compiler resolves no reference there.
  let (_, quoted) = type_at("app.main/quoted", "@3.1");
  assert_eq!(quoted["data"]["inferred_type"], serde_json::Value::Null);
  assert_eq!(diagnostic_codes(&quoted), vec!["W_TYPE_AT_UNRESOLVED".to_owned()]);
  assert!(!has_next(&quoted, "query def 'app.ui/global'"));
}
