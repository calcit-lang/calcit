use calcit::cli_args::ProgramDiffCommand;

use super::structured_output::{StructuredOutputFormat, format_json_value_as_edn};

pub fn handle_program_diff_command(cmd: &ProgramDiffCommand, snapshot_file: &str) -> Result<(), String> {
  let format = StructuredOutputFormat::parse(&cmd.format, "analyze program-diff")?;
  if let Some(def) = &cmd.def {
    if cmd.fail_on_widening {
      return Err("`--fail-on-widening` gates a whole Snapshot; omit `--def` so every definition's schema is compared.".to_owned());
    }
    if format != StructuredOutputFormat::Human {
      return Err(
        "Structured program-diff evidence currently requires a whole Snapshot; omit `--def` or use `--format human`.".to_owned(),
      );
    }
    let result = calcit::def_diff::analyze_def_diff(def, &cmd.git_ref, cmd.base.as_deref(), snapshot_file)?;
    println!("{}", calcit::def_diff::format_def_diff(&result));
  } else {
    let result = calcit::program_diff::analyze_program_diff(&cmd.git_ref, cmd.base.as_deref(), snapshot_file)?;
    let widening = result.evidence.schema_widening.clone();
    match format {
      StructuredOutputFormat::Human => println!("{}", calcit::program_diff::format_program_diff(&result)),
      StructuredOutputFormat::Edn => {
        let value = serde_json::json!({
          "schema_version": 1,
          "kind": "program-diff",
          "data": {
            "git_ref": result.git_ref,
            "file_path": result.file_path,
            "evidence": result.evidence,
          },
        });
        println!("{}", format_json_value_as_edn(&value)?);
      }
      StructuredOutputFormat::Json => {
        let value = serde_json::json!({
          "schema_version": 1,
          "kind": "program-diff",
          "data": {
            "git_ref": result.git_ref,
            "file_path": result.file_path,
            "evidence": result.evidence,
          },
        });
        println!(
          "{}",
          serde_json::to_string_pretty(&value).map_err(|error| format!("Failed to encode program-diff JSON: {error}"))?
        );
      }
    }
    if cmd.fail_on_widening && !widening.is_empty() {
      return Err(format!(
        "Schema widening in {}; review each widened slot instead of relaxing the contract.",
        widening.join(", ")
      ));
    }
  }
  Ok(())
}
