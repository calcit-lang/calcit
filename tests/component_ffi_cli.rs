use std::path::Path;
use std::process::{Command, Output};

use cirru_edn::Edn;

fn run_calcit(args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .arg(Path::new("calcit/test-wasm.cirru"))
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

fn json_as_edn(value: &serde_json::Value) -> Edn {
  match value {
    serde_json::Value::Null => Edn::Nil,
    serde_json::Value::Bool(value) => Edn::Bool(*value),
    serde_json::Value::Number(value) => Edn::Number(value.as_f64().expect("contract numbers should fit f64")),
    serde_json::Value::String(value) => Edn::str(value.as_str()),
    serde_json::Value::Array(values) => Edn::List(cirru_edn::EdnListView(values.iter().map(json_as_edn).collect())),
    serde_json::Value::Object(values) => Edn::map_from_iter(
      values
        .iter()
        .map(|(key, value)| (Edn::tag(key.replace('_', "-")), json_as_edn(value))),
    ),
  }
}

#[test]
fn component_contract_defaults_to_edn_and_matches_explicit_json() {
  let edn_output = run_calcit(&["ffi", "export", "--boundary", "component"]);
  let edn = cirru_edn::parse(&stdout(&edn_output)).expect("default component stdout should be one Cirru EDN value");
  let json_output = run_calcit(&["ffi", "export", "--boundary", "component", "--format", "json"]);
  let json: serde_json::Value =
    serde_json::from_str(&stdout(&json_output)).expect("explicit component JSON stdout should be one JSON value");

  assert_eq!(edn, json_as_edn(&json));
  assert!(
    json["interface_schema"]
      .as_str()
      .is_some_and(|schema| schema.ends_with("component-interface-ir-v2.schema.json"))
  );
  assert_eq!(json["data"]["filters"]["boundary"], "component");
  assert_eq!(json["data"]["interface"]["version"], 2);
  assert_eq!(json["data"]["summary"]["unsupported"], 0);
  assert_eq!(json["data"]["interface"]["definitions"][0]["direction"], "import");
}

#[test]
fn native_json_keeps_the_v3_envelope_without_a_boundary_field() {
  let output = run_calcit(&["ffi", "export", "--json", "--ns", "test-wasm.main"]);
  let json: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("native JSON stdout should stay parseable");

  assert!(
    json["interface_schema"]
      .as_str()
      .is_some_and(|schema| schema.ends_with("ffi-interface-ir-v3.schema.json"))
  );
  assert_eq!(json["data"]["interface"]["version"], 3);
  assert!(json["data"]["filters"].get("boundary").is_none());
}
