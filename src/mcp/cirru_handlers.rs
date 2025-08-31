use super::cirru_utils::cirru_to_json;
use super::tools::McpRequest;
use axum::response::Json as ResponseJson;
use serde_json::Value;

/// 将 Cirru 代码解析为 JSON 结构
pub fn parse_to_json(_app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let cirru_code = match req.parameters.get("cirru_code") {
    Some(code) => code.as_str().unwrap_or(""),
    None => {
      return ResponseJson(serde_json::json!({
        "error": "cirru_code parameter is missing"
      }));
    }
  };

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

/// 将 JSON 结构格式化为 Cirru 代码
pub fn format_from_json(_app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let json_data = match req.parameters.get("json_data") {
    Some(data) => data,
    None => {
      return ResponseJson(serde_json::json!({
        "error": "json_data parameter is missing"
      }));
    }
  };

  // 将 JSON 转换为 Cirru 结构
  let cirru_data = match super::cirru_utils::json_to_cirru(json_data) {
    Ok(cirru) => cirru,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Failed to convert JSON to Cirru: {e}")
      }));
    }
  };

  // 格式化为 Cirru 字符串
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
