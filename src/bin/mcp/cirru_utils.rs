use cirru_parser::Cirru;
use serde_json::Value as JsonValue;

/// 验证 JSON 值是否符合 Cirru 递归结构
pub fn validate_cirru_structure(value: &JsonValue) -> Result<(), String> {
  match value {
    JsonValue::String(_) => Ok(()),
    JsonValue::Array(arr) => {
      for item in arr {
        validate_cirru_structure(item)?;
      }
      Ok(())
    }
    _ => Err("Cirru structure must be strings or arrays only".to_string()),
  }
}

/// 将 JSON 值转换为 Cirru 结构
pub fn json_to_cirru(value: &JsonValue) -> Result<Cirru, String> {
  match value {
    JsonValue::String(s) => Ok(Cirru::Leaf(s.as_str().into())),
    JsonValue::Array(arr) => {
      let mut cirru_list = Vec::new();
      for item in arr {
        cirru_list.push(json_to_cirru(item)?);
      }
      Ok(Cirru::List(cirru_list))
    }
    _ => Err("Invalid JSON structure for Cirru conversion".to_string()),
  }
}

/// 将 Cirru 结构转换为 JSON 值
pub fn cirru_to_json(cirru: &Cirru) -> JsonValue {
  match cirru {
    Cirru::Leaf(s) => JsonValue::String(s.to_string()),
    Cirru::List(list) => {
      let json_list: Vec<JsonValue> = list.iter().map(cirru_to_json).collect();
      JsonValue::Array(json_list)
    }
  }
}