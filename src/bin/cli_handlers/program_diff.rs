use calcit::cli_args::ProgramDiffCommand;

pub fn handle_program_diff_command(cmd: &ProgramDiffCommand, snapshot_file: &str) -> Result<(), String> {
  let result = calcit::program_diff::analyze_program_diff(&cmd.git_ref, snapshot_file)?;
  println!("{}", calcit::program_diff::format_program_diff(&result));
  Ok(())
}
