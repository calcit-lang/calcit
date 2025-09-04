use super::cirru_utils::{json_to_cirru, validate_cirru_structure};
use super::tools::{AddNamespaceRequest, DeleteNamespaceRequest, ListNamespacesRequest, UpdateNamespaceImportsRequest};
use crate::snapshot::{CodeEntry, FileInSnapShot, Snapshot};
use axum::response::Json as ResponseJson;
use serde_json::Value;
use std::collections::HashMap;

/// Load snapshot data, including main file and all module files
/// This function is kept for backward compatibility, but new code should use state_manager
pub fn load_snapshot(app_state: &super::AppState) -> Result<Snapshot, String> {
  app_state.state_manager.with_current_module(|snapshot| snapshot.clone())
}

/// Save snapshot data
/// save_snapshot function moved to cirru_utils::save_snapshot_to_file to avoid duplication
pub fn add_namespace(app_state: &super::AppState, request: AddNamespaceRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;

  let result = app_state.state_manager.update_current_module(|snapshot| {
    // Check if namespace already exists
    if snapshot.files.contains_key(&namespace) {
      return Err(format!("Namespace '{namespace}' already exists"));
    }

    // Create new namespace file
    let new_file = FileInSnapShot {
      ns: CodeEntry::from_code(cirru_parser::Cirru::from(vec!["ns", &namespace])),
      defs: HashMap::new(),
    };

    snapshot.files.insert(namespace.clone(), new_file);
    Ok(())
  });

  match result {
    Ok(()) => ResponseJson(serde_json::json!({
      "message": format!("Namespace '{namespace}' created successfully")
    })),
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}

pub fn delete_namespace(app_state: &super::AppState, request: DeleteNamespaceRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;

  let result = app_state.state_manager.update_current_module(|snapshot| {
    // Check if namespace exists
    if !snapshot.files.contains_key(&namespace) {
      return Err(format!("Namespace '{namespace}' not found"));
    }

    // Delete namespace
    snapshot.files.remove(&namespace);
    Ok(())
  });

  match result {
    Ok(()) => ResponseJson(serde_json::json!({
      "message": format!("Namespace '{namespace}' deleted successfully")
    })),
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}

pub fn list_namespaces(app_state: &super::AppState, _request: ListNamespacesRequest) -> ResponseJson<Value> {
  match app_state.state_manager.with_current_module(|snapshot| {
    let namespaces: Vec<String> = snapshot.files.keys().cloned().collect();
    namespaces
  }) {
    Ok(namespaces) => ResponseJson(serde_json::json!({
      "namespaces": namespaces
    })),
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}

pub fn update_namespace_imports(app_state: &super::AppState, request: UpdateNamespaceImportsRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;
  let ns_definition = request.imports;

  // Validate if ns_definition conforms to Cirru structure
  if let Err(e) = validate_cirru_structure(&ns_definition) {
    return ResponseJson(serde_json::json!({
      "error": format!("Invalid ns_definition structure: {e}")
    }));
  }

  // Convert JSON to Cirru
  let ns_cirru = match json_to_cirru(&ns_definition) {
    Ok(c) => c,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Failed to convert ns_definition to Cirru: {e}")
      }));
    }
  };

  let result = app_state.state_manager.update_current_module(|snapshot| {
    // Check if namespace exists
    let file_data = match snapshot.files.get_mut(&namespace) {
      Some(data) => data,
      None => {
        return Err(format!("Namespace '{namespace}' not found"));
      }
    };

    // Update namespace definition
    file_data.ns = CodeEntry::from_code(ns_cirru);
    Ok(())
  });

  match result {
    Ok(()) => ResponseJson(serde_json::json!({
      "message": format!("Namespace '{namespace}' imports updated successfully")
    })),
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}
