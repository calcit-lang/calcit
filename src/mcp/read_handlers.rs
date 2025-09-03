use super::cirru_utils::cirru_to_json;
use super::tools::{GetPackageNameRequest, ListDefinitionsRequest, ReadDefinitionRequest, ReadNamespaceRequest};
use crate::snapshot::Snapshot;
use axum::response::Json as ResponseJson;
use serde_json::Value;

/// Load snapshot data
fn load_snapshot(app_state: &super::AppState) -> Result<Snapshot, String> {
  // Use the load_snapshot function from namespace_handlers, which contains module loading logic
  super::namespace_handlers::load_snapshot(app_state)
}

pub fn list_definitions(app_state: &super::AppState, request: ListDefinitionsRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;

  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  // Check if namespace exists
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

// list_namespaces function moved to namespace_handlers.rs to avoid duplication

pub fn get_package_name(app_state: &super::AppState, _request: GetPackageNameRequest) -> ResponseJson<Value> {
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

pub fn read_namespace(app_state: &super::AppState, request: ReadNamespaceRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;

  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  // Check if namespace exists
  let file_data = match snapshot.files.get(&namespace) {
    Some(data) => data,
    None => {
      return ResponseJson(serde_json::json!({
        "error": format!("Namespace '{namespace}' not found")
      }));
    }
  };

  // Convert namespace data to JSON, only return definition names, documentation and first 40 characters of code
  let mut definitions = serde_json::Map::new();
  for (def_name, code_entry) in &file_data.defs {
    // Convert code to string and truncate to first 40 characters
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

pub fn read_definition(app_state: &super::AppState, request: ReadDefinitionRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;
  let definition = request.definition;

  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  // Check if namespace exists
  let file_data = match snapshot.files.get(&namespace) {
    Some(data) => data,
    None => {
      return ResponseJson(serde_json::json!({
        "error": format!("Namespace '{namespace}' not found")
      }));
    }
  };

  // Check if definition exists
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
