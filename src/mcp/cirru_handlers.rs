use actix_web::HttpResponse;
use super::tools::McpRequest;
use super::cirru_utils::{validate_cirru_structure, cirru_to_json};

/// 将 Cirru 代码解析为 JSON 结构
pub fn parse_to_json(_app_state: &super::AppState, req: McpRequest) -> HttpResponse {
  let cirru_code = match req.parameters.get("cirru_code") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("cirru_code parameter is missing or not a string"),
  };

  match cirru_parser::parse(&cirru_code) {
    Ok(cirru_data) => {
      let json_data: Vec<serde_json::Value> = cirru_data.iter().map(cirru_to_json).collect();
      HttpResponse::Ok().json(serde_json::json!({
        "result": json_data
      }))
    }
    Err(e) => HttpResponse::BadRequest().body(format!("Failed to parse Cirru code: {e}")),
  }
}

/// 将 JSON 结构格式化为 Cirru 字符串
pub fn format_from_json(_app_state: &super::AppState, req: McpRequest) -> HttpResponse {
  let json_structure = match req.parameters.get("json_structure") {
    Some(structure) => structure.clone(),
    None => return HttpResponse::BadRequest().body("json_structure parameter is missing"),
  };

  // 验证 JSON 结构是否符合 Cirru 格式
  if let Err(e) = validate_cirru_structure(&json_structure) {
    return HttpResponse::BadRequest().body(format!("Invalid JSON structure for Cirru: {e}"));
  }

  // 将 JSON 转换为 Cirru
  let cirru_data = match crate::mcp::cirru_utils::json_to_cirru(&json_structure) {
    Ok(c) => c,
    Err(e) => return HttpResponse::BadRequest().body(format!("Failed to convert JSON to Cirru: {e}")),
  };

  // 格式化为字符串
  let cirru_string = cirru_parser::format(&[cirru_data], cirru_parser::CirruWriterOptions { use_inline: true });

  HttpResponse::Ok().json(serde_json::json!({
    "result": cirru_string
  }))
}