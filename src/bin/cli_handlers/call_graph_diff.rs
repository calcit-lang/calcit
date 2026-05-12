use calcit::cli_args::CallGraphDiffCommand;

pub fn handle_call_graph_diff_command(cmd: &CallGraphDiffCommand, snapshot_file: &str) -> Result<(), String> {
  let result = calcit::call_graph_diff::analyze_call_graph_diff(cmd, snapshot_file)?;
  println!("{}", calcit::call_graph_diff::format_call_graph_diff(&result));
  Ok(())
}
