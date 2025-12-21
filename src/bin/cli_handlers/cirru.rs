//! Cirru subcommand handlers
//!
//! Handles: cr cirru parse, format, parse-edn

use calcit::cli_args::{CirruCommand, CirruSubcommand};

pub fn handle_cirru_command(cmd: &CirruCommand) -> Result<(), String> {
  match &cmd.subcommand {
    CirruSubcommand::Parse(opts) => handle_parse(&opts.code),
    CirruSubcommand::Format(opts) => handle_format(&opts.json),
    CirruSubcommand::ParseEdn(opts) => handle_parse_edn(&opts.edn),
  }
}

fn cirru_to_json(cirru: &cirru_parser::Cirru) -> serde_json::Value {
  match cirru {
    cirru_parser::Cirru::Leaf(s) => serde_json::Value::String(s.to_string()),
    cirru_parser::Cirru::List(items) => serde_json::Value::Array(items.iter().map(cirru_to_json).collect()),
  }
}

fn json_to_cirru(json: &serde_json::Value) -> Result<cirru_parser::Cirru, String> {
  match json {
    serde_json::Value::String(s) => Ok(cirru_parser::Cirru::Leaf(std::sync::Arc::from(s.as_str()))),
    serde_json::Value::Array(arr) => {
      let items: Result<Vec<cirru_parser::Cirru>, String> = arr.iter().map(json_to_cirru).collect();
      Ok(cirru_parser::Cirru::List(items?))
    }
    serde_json::Value::Number(n) => Ok(cirru_parser::Cirru::Leaf(std::sync::Arc::from(n.to_string()))),
    serde_json::Value::Bool(b) => Ok(cirru_parser::Cirru::Leaf(std::sync::Arc::from(b.to_string()))),
    serde_json::Value::Null => Ok(cirru_parser::Cirru::Leaf(std::sync::Arc::from("nil"))),
    serde_json::Value::Object(_) => Err("JSON objects cannot be converted to Cirru".to_string()),
  }
}

fn handle_parse(code: &str) -> Result<(), String> {
  // Check if input looks like JSON
  let trimmed = code.trim_start();
  if trimmed.starts_with('[') {
    return Err("Input appears to be JSON format, not Cirru code. This tool is for parsing Cirru syntax only.".to_string());
  }

  let cirru_data = cirru_parser::parse(code).map_err(|e| format!("Failed to parse Cirru code: {e}"))?;

  let json_result = if cirru_data.len() == 1 {
    cirru_to_json(&cirru_data[0])
  } else {
    serde_json::Value::Array(cirru_data.iter().map(cirru_to_json).collect())
  };

  let json_str = serde_json::to_string_pretty(&json_result).map_err(|e| format!("Failed to serialize JSON: {e}"))?;

  println!("{json_str}");

  Ok(())
}

fn handle_format(json_str: &str) -> Result<(), String> {
  let json_data: serde_json::Value = serde_json::from_str(json_str).map_err(|e| format!("Failed to parse JSON: {e}"))?;

  let cirru_data = json_to_cirru(&json_data)?;

  let cirru_code = cirru_parser::format(&[cirru_data], true.into()).map_err(|e| format!("Failed to format Cirru: {e}"))?;

  println!("{cirru_code}");

  Ok(())
}

fn handle_parse_edn(edn_str: &str) -> Result<(), String> {
  let edn = cirru_edn::parse(edn_str).map_err(|e| format!("Failed to parse Cirru EDN: {e}"))?;

  let json_value = serde_json::to_value(&edn).map_err(|e| format!("Failed to convert EDN to JSON: {e}"))?;

  let json_str = serde_json::to_string_pretty(&json_value).map_err(|e| format!("Failed to serialize JSON: {e}"))?;

  println!("{json_str}");

  Ok(())
}
