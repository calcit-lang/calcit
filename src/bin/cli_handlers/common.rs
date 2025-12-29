//! Common utilities shared between CLI handlers

use cirru_parser::Cirru;
use std::sync::Arc;

// Error message constants
pub const ERR_MULTIPLE_INPUT_SOURCES: &str =
  "Multiple input sources provided. Use only one of: --stdin/-s, --file/-f, --code/-e, or --json/-j.";

pub const ERR_CONFLICTING_INPUT_FLAGS: &str = "Conflicting input flags: use only one of --json-leaf or --json-input.";

pub const ERR_CONFLICTING_JSON_CIRRU: &str = "Conflicting input flags: use only one of --json-input or Cirru input.";

pub const ERR_CODE_INPUT_REQUIRED: &str = "Code input required: use --file, --code, --json, or --stdin";

pub const ERR_JSON_OBJECTS_NOT_SUPPORTED: &str = "JSON objects not supported, use arrays";

/// Convert JSON Value to Cirru syntax tree
pub fn json_value_to_cirru(json: &serde_json::Value) -> Result<Cirru, String> {
  match json {
    serde_json::Value::String(s) => Ok(Cirru::Leaf(Arc::from(s.as_str()))),
    serde_json::Value::Number(n) => Ok(Cirru::Leaf(Arc::from(n.to_string()))),
    serde_json::Value::Bool(b) => Ok(Cirru::Leaf(Arc::from(b.to_string()))),
    serde_json::Value::Null => Ok(Cirru::Leaf(Arc::from("null"))),
    serde_json::Value::Array(arr) => {
      let items: Result<Vec<Cirru>, String> = arr.iter().map(json_value_to_cirru).collect();
      Ok(Cirru::List(items?))
    }
    serde_json::Value::Object(_) => Err(ERR_JSON_OBJECTS_NOT_SUPPORTED.to_string()),
  }
}

/// Parse path string like "2,1,0" to Vec<usize>
pub fn parse_path(path_str: &str) -> Result<Vec<usize>, String> {
  if path_str.is_empty() {
    return Ok(vec![]);
  }

  path_str
    .split(',')
    .map(|s| s.trim().parse::<usize>().map_err(|e| format!("Invalid path index '{s}': {e}")))
    .collect()
}

/// Validate input source conflicts
pub fn validate_input_sources(sources: &[bool]) -> Result<(), String> {
  let count = sources.iter().filter(|&&x| x).count();
  if count > 1 {
    return Err(ERR_MULTIPLE_INPUT_SOURCES.to_string());
  }
  Ok(())
}

/// Validate input flag conflicts
pub fn validate_input_flags(json_leaf: bool, json_input: bool, cirru: bool) -> Result<(), String> {
  // In tree handlers, `cirru` is always false (one-liner only). In other handlers, it may be used.
  if json_leaf && (json_input || cirru) {
    return Err(ERR_CONFLICTING_INPUT_FLAGS.to_string());
  }
  if json_input && cirru {
    return Err(ERR_CONFLICTING_JSON_CIRRU.to_string());
  }
  Ok(())
}
