use actix_web::HttpResponse;
use crate::snapshot::{self, Snapshot};
use super::tools::McpRequest;
use super::cirru_utils::cirru_to_json;

/// 加载快照数据
fn load_snapshot(app_state: &super::AppState) -> Result<Snapshot, HttpResponse> {
  let compact_cirru_path = &app_state.compact_cirru_path;
  let content = match std::fs::read_to_string(compact_cirru_path) {
    Ok(c) => c,
    Err(e) => return Err(HttpResponse::InternalServerError().body(format!("Failed to read compact.cirru: {e}"))),
  };

  let edn_data = match cirru_edn::parse(&content) {
    Ok(d) => d,
    Err(e) => return Err(HttpResponse::InternalServerError().body(format!("Failed to parse compact.cirru as EDN: {e}"))),
  };

  let snapshot: Snapshot = match snapshot::load_snapshot_data(&edn_data, compact_cirru_path) {
    Ok(s) => s,
    Err(e) => return Err(HttpResponse::InternalServerError().body(format!("Failed to load snapshot: {e}"))),
  };

  Ok(snapshot)
}

/// 列出指定命名空间中的所有定义
pub fn list_definitions(app_state: &super::AppState, req: McpRequest) -> HttpResponse {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("namespace parameter is missing or not a string"),
  };

  let include_docs = req.parameters.get("include_docs").and_then(|v| v.as_bool()).unwrap_or(false);

  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => return e,
  };

  match snapshot.files.get(&namespace) {
    Some(file_data) => {
      let mut definitions = Vec::new();
      for (def_name, def_entry) in &file_data.defs {
        if include_docs {
          definitions.push(serde_json::json!({
            "name": def_name,
            "doc": def_entry.doc
          }));
        } else {
          definitions.push(serde_json::json!({
            "name": def_name
          }));
        }
      }
      HttpResponse::Ok().json(definitions)
    }
    None => HttpResponse::NotFound().body(format!("Namespace '{namespace}' not found")),
  }
}

/// 读取指定命名空间的信息
pub fn read_namespace(app_state: &super::AppState, req: McpRequest) -> HttpResponse {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("namespace parameter is missing or not a string"),
  };

  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => return e,
  };

  match snapshot.files.get(&namespace) {
    Some(file_data) => HttpResponse::Ok().json(serde_json::json!({
      "namespace": namespace,
      "ns_definition": file_data.ns,
    })),
    None => HttpResponse::NotFound().body(format!("Namespace '{namespace}' not found")),
  }
}

/// 读取指定定义的详细信息
pub fn read_definition(app_state: &super::AppState, req: McpRequest) -> HttpResponse {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("namespace parameter is missing or not a string"),
  };

  let definition = match req.parameters.get("definition") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("definition parameter is missing or not a string"),
  };

  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => return e,
  };

  match snapshot.files.get(&namespace) {
    Some(file_data) => match file_data.defs.get(&definition) {
      Some(def_entry) => {
        let code_json = cirru_to_json(&def_entry.code);
        HttpResponse::Ok().json(serde_json::json!({
          "namespace": namespace,
          "definition": definition,
          "doc": def_entry.doc,
          "code": code_json
        }))
      },
      None => HttpResponse::NotFound().body(format!("Definition '{definition}' not found in namespace '{namespace}'")),
    },
    None => HttpResponse::NotFound().body(format!("Namespace '{namespace}' not found")),
  }
}