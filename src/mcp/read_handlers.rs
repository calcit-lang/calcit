use super::cirru_utils::cirru_to_json;
use super::tools::McpRequest;
use crate::snapshot::{self, Snapshot};
use axum::response::Json as ResponseJson;
use serde_json::Value;

/// 加载快照数据
fn load_snapshot(app_state: &super::AppState) -> Result<Snapshot, String> {
  let compact_cirru_path = &app_state.compact_cirru_path;
  let content = match std::fs::read_to_string(compact_cirru_path) {
    Ok(c) => c,
    Err(e) => {
      eprintln!("Failed to read compact.cirru: {e}");
      return Err(format!("Failed to read file: {e}"));
    }
  };

  let edn_data = match cirru_edn::parse(&content) {
    Ok(d) => d,
    Err(e) => {
      eprintln!("Failed to parse compact.cirru as EDN: {e}");
      return Err(format!("Failed to parse EDN: {e}"));
    }
  };

  match snapshot::load_snapshot_data(&edn_data, compact_cirru_path) {
    Ok(snapshot) => Ok(snapshot),
    Err(e) => {
      eprintln!("Failed to load snapshot: {e}");
      Err(format!("Failed to load snapshot: {e}"))
    }
  }
}

pub fn list_definitions(app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "namespace parameter is missing or not a string"
      }));
    }
  };

  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  // 检查命名空间是否存在
  let file_data = match snapshot.files.get(&namespace) {
    Some(data) => data,
    None => {
      return ResponseJson(serde_json::json!({
        "error": format!("Namespace '{namespace}' not found")
      }));
    }
  };

  let definitions: Vec<String> = file_data.defs.keys().cloned().collect();

  ResponseJson(serde_json::json!({
    "namespace": namespace,
    "definitions": definitions
  }))
}

pub fn list_namespaces(app_state: &super::AppState, _req: McpRequest) -> ResponseJson<Value> {
  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  let namespaces: Vec<String> = snapshot.files.keys().cloned().collect();

  ResponseJson(serde_json::json!({
    "namespaces": namespaces
  }))
}

pub fn read_namespace(app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "namespace parameter is missing or not a string"
      }));
    }
  };

  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  // 检查命名空间是否存在
  let file_data = match snapshot.files.get(&namespace) {
    Some(data) => data,
    None => {
      return ResponseJson(serde_json::json!({
        "error": format!("Namespace '{namespace}' not found")
      }));
    }
  };

  // 转换命名空间数据为 JSON
  let mut definitions = serde_json::Map::new();
  for (def_name, code_entry) in &file_data.defs {
    definitions.insert(
      def_name.clone(),
      serde_json::json!({
        "doc": code_entry.doc,
        "code": cirru_to_json(&code_entry.code)
      }),
    );
  }

  ResponseJson(serde_json::json!({
    "namespace": namespace,
    "definitions": definitions,
    "ns": file_data.ns
  }))
}

pub fn read_definition(app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "namespace parameter is missing or not a string"
      }));
    }
  };

  let definition = match req.parameters.get("definition") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "definition parameter is missing or not a string"
      }));
    }
  };

  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  // 检查命名空间是否存在
  let file_data = match snapshot.files.get(&namespace) {
    Some(data) => data,
    None => {
      return ResponseJson(serde_json::json!({
        "error": format!("Namespace '{namespace}' not found")
      }));
    }
  };

  // 检查定义是否存在
  let code_entry = match file_data.defs.get(&definition) {
    Some(entry) => entry,
    None => {
      return ResponseJson(serde_json::json!({
        "error": format!("Definition '{definition}' not found in namespace '{namespace}'")
      }));
    }
  };

  ResponseJson(serde_json::json!({
    "namespace": namespace,
    "definition": definition,
    "doc": code_entry.doc,
    "code": cirru_to_json(&code_entry.code)
  }))
}
