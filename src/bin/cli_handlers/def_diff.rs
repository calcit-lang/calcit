use calcit::cli_args::DefDiffCommand;

pub fn handle_def_diff_command(cmd: &DefDiffCommand, snapshot_file: &str) -> Result<(), String> {
  let result = calcit::def_diff::analyze_def_diff(&cmd.target, &cmd.git_ref, snapshot_file)?;
  println!("{}", calcit::def_diff::format_def_diff(&result));
  Ok(())
}
