use calcit::cli_args::{FfiCommand, FfiSubcommand};
use calcit::ffi_interface_ir::{
  COMPONENT_INTERFACE_IR_SCHEMA_ID, FFI_INTERFACE_IR_SCHEMA_ID, export_component_snapshot, export_snapshot,
  format_component_human_report, format_human_report,
};

use super::common::package_version_for_snapshot;
use super::query::load_main_snapshot;
use super::structured_output::{StructuredOutputFormat, format_json_value_as_edn};

fn selected_format(boundary: &str, format: Option<&str>, legacy_json: bool) -> Result<StructuredOutputFormat, String> {
  match format {
    Some(format) => StructuredOutputFormat::parse(format, "ffi export"),
    None if legacy_json => Ok(StructuredOutputFormat::Json),
    None if boundary == "component" => Ok(StructuredOutputFormat::Edn),
    None => Ok(StructuredOutputFormat::Human),
  }
}

fn print_structured(value: &serde_json::Value, format: StructuredOutputFormat) -> Result<(), String> {
  match format {
    StructuredOutputFormat::Json => println!(
      "{}",
      serde_json::to_string_pretty(value).map_err(|error| format!("Failed to encode FFI Interface IR JSON: {error}"))?
    ),
    StructuredOutputFormat::Edn => println!("{}", format_json_value_as_edn(value)?),
    StructuredOutputFormat::Human => unreachable!("human FFI reports are rendered separately"),
  }
  Ok(())
}

pub fn handle_ffi_command(command: &FfiCommand, input_path: &str) -> Result<(), String> {
  match &command.subcommand {
    FfiSubcommand::Export(options) => {
      let mut snapshot = load_main_snapshot(input_path)?;
      if let Some(version) = package_version_for_snapshot(input_path)? {
        snapshot.version = version;
      }
      let output_format = selected_format(&options.boundary, options.format.as_deref(), options.json)?;
      match options.boundary.as_str() {
        "native" => {
          let report = export_snapshot(&snapshot, options.ns.as_deref())?;
          if output_format == StructuredOutputFormat::Human {
            print!("{}", format_human_report(&report));
          } else {
            let envelope = serde_json::json!({
              "schema_version": 1,
              "interface_schema": FFI_INTERFACE_IR_SCHEMA_ID,
              "command": "ffi.export",
              "revision": report.revision,
              "data": {
                "filters": {
                  "namespace": options.ns,
                  "include_dependencies": false,
                },
                "interface": report.interface,
                "summary": report.summary,
              },
              "diagnostics": report.diagnostics,
            });
            print_structured(&envelope, output_format)?;
          }
        }
        "component" => {
          let report = export_component_snapshot(&snapshot, options.ns.as_deref())?;
          if output_format == StructuredOutputFormat::Human {
            print!("{}", format_component_human_report(&report));
          } else {
            let envelope = serde_json::json!({
              "schema_version": 1,
              "interface_schema": COMPONENT_INTERFACE_IR_SCHEMA_ID,
              "command": "ffi.export",
              "revision": report.revision,
              "data": {
                "filters": {
                  "boundary": "component",
                  "namespace": options.ns,
                  "include_dependencies": false,
                },
                "interface": report.interface,
                "summary": report.summary,
              },
              "diagnostics": report.diagnostics,
            });
            print_structured(&envelope, output_format)?;
          }
        }
        other => return Err(format!("Unsupported ffi export boundary '{other}'. Expected native or component.")),
      }
      Ok(())
    }
  }
}
