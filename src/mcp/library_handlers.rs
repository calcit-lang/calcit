//! Handlers for library-related operations in Calcit MCP

use super::tools::{FetchCalcitLibrariesRequest, ParseCirruEdnToJsonRequest};
use axum::response::Json as ResponseJson;
use reqwest::blocking::Client;
use serde_json::{Value, json};

/// Fetches the list of available Calcit libraries from the official registry
pub fn handle_fetch_calcit_libraries(
  _app_state: &super::AppState,
  _request: FetchCalcitLibrariesRequest,
) -> ResponseJson<serde_json::Value> {
  // 创建HTTP客户端
  let client = Client::new();

  // 从官方仓库获取库列表
  let url = "https://libs.calcit-lang.org/base.cirru";

  match client.get(url).send() {
    Ok(response) => {
      if response.status().is_success() {
        // 获取响应内容
        match response.text() {
          Ok(text) => {
            // 解析Cirru EDN格式
            match cirru_edn::parse(&text) {
              Ok(edn) => {
                // 使用serde_json将Edn转换为JSON
                match serde_json::to_value(edn) {
                  Ok(json_value) => ResponseJson(json_value),
                  Err(err) => ResponseJson(json!({
                    "error": format!("Failed to convert EDN to JSON: {}", err)
                  })),
                }
              }
              Err(err) => ResponseJson(json!({
                "error": format!("Failed to parse Cirru EDN: {}", err)
              })),
            }
          }
          Err(err) => ResponseJson(json!({
            "error": format!("Failed to read response text: {}", err)
          })),
        }
      } else {
        ResponseJson(json!({
          "error": format!("Failed to fetch libraries: HTTP status {}", response.status())
        }))
      }
    }
    Err(err) => ResponseJson(json!({
      "error": format!("Failed to connect to library registry: {}", err)
    })),
  }
}

/// Parses Cirru EDN format to simplified JSON
pub fn handle_parse_cirru_edn_to_json(_app_state: &super::AppState, request: ParseCirruEdnToJsonRequest) -> ResponseJson<Value> {
  // Parse the Cirru EDN string
  match cirru_edn::parse(&request.cirru_edn) {
    Ok(edn) => {
      // 直接使用serde_json将Edn转换为JSON
      match serde_json::to_value(&edn) {
        Ok(json_value) => ResponseJson(json!({
          "result": json_value,
          "message": "Successfully parsed Cirru EDN to JSON"
        })),
        Err(err) => ResponseJson(json!({
          "error": format!("Failed to convert EDN to JSON: {}", err)
        })),
      }
    }
    Err(err) => ResponseJson(json!({
      "error": format!("Failed to parse Cirru EDN: {}", err)
    })),
  }
}
