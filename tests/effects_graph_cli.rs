use std::process::Command;

use serde_json::Value;

/// Replays the `calcit/test-effects-graph.cirru` fixture through `analyze effects-graph`.
#[test]
fn effects_graph_reports_console_state_and_io_effects() {
  let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/calcit/test-effects-graph.cirru");
  let output = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args(["--tips-level", "none", fixture, "analyze", "effects-graph", "--format", "json"])
    .output()
    .expect("calcit command should run");
  assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

  let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
  let json_start = stdout.find('{').expect("stdout should contain a JSON document");
  let graph: Value = serde_json::from_str(&stdout[json_start..]).expect("graph should be valid JSON");

  assert_eq!(graph["entry"], "test-effects-graph.main/main!");
  assert_eq!(graph["tree"]["effects"][0]["kind"], "console");
  let children = graph["tree"]["children"].as_array().expect("entry should have children");
  let child = |def: &str| {
    children
      .iter()
      .find(|c| c["def"] == def)
      .unwrap_or_else(|| panic!("missing child {def}"))
  };
  assert_eq!(child("state-helper")["state"][0]["kind"], "atom-def");
  assert_eq!(child("io-helper")["effects"][0]["kind"], "io/read");
  assert_eq!(graph["stats"]["reachable_count"], 3);
  assert_eq!(graph["stats"]["state_items"], 3);
}
