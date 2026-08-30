use calcit::cli_args::{FfiCommand, FfiSubcommand};
use calcit::ffi_interface_ir::{FFI_INTERFACE_IR_SCHEMA_ID, export_snapshot, format_human_report};

use super::common::package_version_for_snapshot;
use super::query::load_main_snapshot;

pub fn handle_ffi_command(command: &FfiCommand, input_path: &str) -> Result<(), String> {
  match &command.subcommand {
    FfiSubcommand::Export(options) => {
      let mut snapshot = load_main_snapshot(input_path)?;
      if let Some(version) = package_version_for_snapshot(input_path)? {
        snapshot.version = version;
      }
      let report = export_snapshot(&snapshot, options.ns.as_deref())?;
      if options.json {
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
        println!(
          "{}",
          serde_json::to_string_pretty(&envelope).map_err(|error| format!("Failed to encode FFI Interface IR JSON: {error}"))?
        );
      } else {
        print!("{}", format_human_report(&report));
      }
      Ok(())
    }
  }
}
