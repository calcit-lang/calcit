//! Cirru subcommand handlers
//!
//! Handles: cr cirru parse, format, parse-edn, show-guide

use calcit::cli_args::{CirruCommand, CirruSubcommand};

use super::cirru_validator;

pub fn handle_cirru_command(cmd: &CirruCommand) -> Result<(), String> {
  match &cmd.subcommand {
    CirruSubcommand::Parse(opts) => handle_parse(&opts.code, opts.expr_one_liner, opts.validate),
    CirruSubcommand::Format(opts) => handle_format(&opts.json),
    CirruSubcommand::ParseEdn(opts) => handle_parse_edn(&opts.edn),
    CirruSubcommand::ShowGuide(_) => handle_show_guide(),
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

fn handle_parse(code: &str, expr_one_liner: bool, validate: bool) -> Result<(), String> {
  if expr_one_liner {
    let trimmed = code.trim();
    if trimmed.is_empty() {
      return Err("Input is empty. Provide Cirru code to parse or omit --cirru.".to_string());
    }
    if code.contains('\t') {
      return Err(
        "Input contains tab characters. Cirru requires spaces for indentation.\n\
         Please replace tabs with 2 spaces."
          .to_string(),
      );
    }

    let cirru_expr =
      cirru_parser::parse_expr_one_liner(code).map_err(|e| format!("Failed to parse Cirru one-liner expression: {e}"))?;

    // Validate basic Cirru syntax if requested
    if validate {
      cirru_validator::validate_cirru_syntax(&cirru_expr)?;
    }

    let json_result = cirru_to_json(&cirru_expr);
    let json_str = serde_json::to_string_pretty(&json_result).map_err(|e| format!("Failed to serialize JSON: {e}"))?;
    println!("{json_str}");
    return Ok(());
  }

  // Check if input looks like JSON (but allow Cirru's [] list syntax)
  let trimmed = code.trim_start();
  if let Some(after_bracket) = trimmed.strip_prefix('[') {
    // Cirru [] syntax: "[] 1 2 3" or "[]" - bracket followed by ] or space+non-quote
    let is_cirru_list =
      after_bracket.starts_with(']') || (after_bracket.starts_with(' ') && !after_bracket.trim_start().starts_with('"'));

    if !is_cirru_list {
      return Err(
        "Input appears to be JSON format (starts with '[\"'), not Cirru code.\n\
         This tool is for parsing Cirru syntax only.\n\
         Note: Cirru's [] list syntax (e.g. '[] 1 2 3') is supported."
          .to_string(),
      );
    }
  }

  let cirru_data = cirru_parser::parse(code).map_err(|e| format!("Failed to parse Cirru code: {e}"))?;

  // Validate basic Cirru syntax if requested
  if validate {
    for node in &cirru_data {
      cirru_validator::validate_cirru_syntax(node)?;
    }
  }

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

fn handle_show_guide() -> Result<(), String> {
  let home_dir = std::env::var("HOME").map_err(|_| "Failed to get HOME directory".to_string())?;
  let guide_path = format!("{home_dir}/.config/calcit/guidebook-repo/docs/cirru-syntax.md");

  match std::fs::read_to_string(&guide_path) {
    Ok(content) => {
      println!("{content}");
      Ok(())
    }
    Err(_) => Err(format!(
      "Cirru syntax guide not found at: {guide_path}\n\
       Please ensure the guidebook repository is cloned to ~/.config/calcit/guidebook-repo/"
    )),
  }
}
