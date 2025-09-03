use super::cirru_utils::cirru_to_json;
use super::tools::{FormatJsonToCirruRequest, ParseCirruToJsonRequest};
use axum::response::Json as ResponseJson;
use serde_json::Value;

/// Parse Cirru code to JSON structure
pub fn parse_cirru_to_json(_app_state: &super::AppState, request: ParseCirruToJsonRequest) -> ResponseJson<Value> {
  let cirru_code = &request.cirru_code;

  match cirru_parser::parse(cirru_code) {
    Ok(cirru_data) => {
      let json_data: Vec<serde_json::Value> = cirru_data.iter().map(cirru_to_json).collect();
      ResponseJson(serde_json::json!({
        "result": json_data
      }))
    }
    Err(e) => ResponseJson(serde_json::json!({
      "error": format!("Failed to parse Cirru code: {e}")
    })),
  }
}

/// Format JSON structure to Cirru code
pub fn format_json_to_cirru(_app_state: &super::AppState, request: FormatJsonToCirruRequest) -> ResponseJson<Value> {
  let json_data = &request.json_data;

  // Convert JSON to Cirru structure
  let cirru_data = match super::cirru_utils::json_to_cirru(json_data) {
    Ok(cirru) => cirru,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Failed to convert JSON to Cirru: {e}")
      }));
    }
  };

  // Format to Cirru string
  let cirru_code = match cirru_parser::format(&[cirru_data], cirru_parser::CirruWriterOptions { use_inline: true }) {
    Ok(formatted) => formatted,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Failed to format Cirru: {e}")
      }));
    }
  };

  ResponseJson(serde_json::json!({
    "result": cirru_code
  }))
}
