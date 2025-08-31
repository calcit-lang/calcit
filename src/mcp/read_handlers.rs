use super::cirru_utils::cirru_to_json;
use super::tools::McpRequest;
use crate::snapshot::{self, Snapshot};
use axum::response::Json as ResponseJson;
use serde_json::Value;

/// 加载快照数据
fn load_snapshot(app_state: &super::AppState) -> Result<Snapshot, String> {
  // 使用 namespace_handlers 中的 load_snapshot 函数，它包含模块加载逻辑
  super::namespace_handlers::load_snapshot(app_state)
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

pub fn get_package_name(app_state: &super::AppState, _req: McpRequest) -> ResponseJson<Value> {
  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  ResponseJson(serde_json::json!({
    "package_name": snapshot.package
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

  // 转换命名空间数据为 JSON，只返回定义名称、文档和代码前40个字符
  let mut definitions = serde_json::Map::new();
  for (def_name, code_entry) in &file_data.defs {
    // 将代码转换为字符串并截取前40个字符
    let code_json = cirru_to_json(&code_entry.code);
    let code_str = code_json.to_string();
    let code_preview = if code_str.len() > 40 {
      format!("{}...", &code_str[..40])
    } else {
      code_str
    };
    
    definitions.insert(
      def_name.clone(),
      serde_json::json!({
        "doc": code_entry.doc,
        "code_preview": code_preview
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