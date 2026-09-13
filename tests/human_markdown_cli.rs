use std::process::{Command, Output};

fn run_calcit(args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .arg("calcit/test.cirru")
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
fn code_bearing_tree_diagnostic_stays_on_stderr_with_a_fence() {
  let output = run_calcit(&["tree", "show", "app.main/main!", "--path", "999"]);
  assert!(!output.status.success());
  assert!(output.stdout.is_empty());
  let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
  assert!(stderr.contains("# Error: invalid path\n"));
  assert!(stderr.contains("## Node at longest valid path\n\n```cirru\n"));
}
