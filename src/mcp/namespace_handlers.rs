use super::cirru_utils::{json_to_cirru, validate_cirru_structure};
use super::tools::{AddNamespaceRequest, DeleteNamespaceRequest, ListNamespacesRequest, UpdateNamespaceImportsRequest};
use crate::snapshot::{self, CodeEntry, FileInSnapShot, Snapshot};
use axum::response::Json as ResponseJson;
use serde_json::Value;
use std::collections::HashMap;

/// Load snapshot data, including main file and all module files
pub fn load_snapshot(app_state: &super::AppState) -> Result<Snapshot, String> {
  use std::path::Path;

  let content = match std::fs::read_to_string(&app_state.compact_cirru_path) {
    Ok(c) => c,
    Err(e) => return Err(format!("Failed to read compact.cirru: {e}")),
  };

  let edn_data = match cirru_edn::parse(&content) {
    Ok(d) => d,
    Err(e) => return Err(format!("Failed to parse compact.cirru as EDN: {e}")),
  };

  let mut main_snapshot = match snapshot::load_snapshot_data(&edn_data, &app_state.compact_cirru_path) {
    Ok(snapshot) => snapshot,
    Err(e) => return Err(format!("Failed to load snapshot: {e}")),
  };

  // Load all module files and merge namespaces
  let base_dir = Path::new(&app_state.compact_cirru_path).parent().unwrap_or(Path::new("."));
  let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
  let module_folder = Path::new(&home_dir).join(".config/calcit/modules"); // Use standard calcit module directory

  println!(
    "Loading modules from main snapshot, found {} modules",
    main_snapshot.configs.modules.len()
  );

  for module_path in &main_snapshot.configs.modules {
    println!("Attempting to load module: {module_path}");
    match crate::load_module(module_path, base_dir, &module_folder) {
      Ok(module_snapshot) => {
        println!(
          "Successfully loaded module {}, found {} namespaces",
          module_path,
          module_snapshot.files.len()
        );
        // Merge module files into main snapshot
        for (namespace, file_entry) in module_snapshot.files {
          println!("Adding namespace: {namespace}");
          main_snapshot.files.insert(namespace, file_entry);
        }
        // Merge module entries
        for (entry_name, entry_config) in module_snapshot.entries {
          main_snapshot.entries.insert(entry_name, entry_config);
        }
      }
      Err(e) => {
        println!("Warning: Failed to load module {module_path}: {e}");
        // Continue loading other modules, don't stop due to one module failure
      }
    }
  }

  println!("Final snapshot has {} namespaces", main_snapshot.files.len());

  Ok(main_snapshot)
}

/// Save snapshot data
// save_snapshot function moved to cirru_utils::save_snapshot_to_file to avoid duplication
fn save_snapshot(app_state: &super::AppState, snapshot: &Snapshot) -> Result<(), ResponseJson<Value>> {
  super::cirru_utils::save_snapshot_to_file(&app_state.compact_cirru_path, snapshot).map_err(|e| {
    ResponseJson(serde_json::json!({
      "error": e
    }))
  })
}

pub fn add_namespace(app_state: &super::AppState, request: AddNamespaceRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;

  let mut snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  // Check if namespace already exists
  if snapshot.files.contains_key(&namespace) {
    return ResponseJson(serde_json::json!({
      "error": format!("Namespace '{namespace}' already exists")
    }));
  }

  // Create new namespace file
  let new_file = FileInSnapShot {
    ns: CodeEntry::from_code(cirru_parser::Cirru::from(vec!["ns", &namespace])),
    defs: HashMap::new(),
  };

  snapshot.files.insert(namespace.clone(), new_file);

  // Save snapshot
  if let Err(e) = save_snapshot(app_state, &snapshot) {
    return e;
  }

  ResponseJson(serde_json::json!({
    "message": format!("Namespace '{namespace}' created successfully")
  }))
}

pub fn delete_namespace(app_state: &super::AppState, request: DeleteNamespaceRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;

  let mut snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  // Check if namespace exists
  if !snapshot.files.contains_key(&namespace) {
    return ResponseJson(serde_json::json!({
      "error": format!("Namespace '{namespace}' not found")
    }));
  }

  // Delete namespace
  snapshot.files.remove(&namespace);

  // Save snapshot
  if let Err(e) = save_snapshot(app_state, &snapshot) {
    return e;
  }

  ResponseJson(serde_json::json!({
    "message": format!("Namespace '{namespace}' deleted successfully")
  }))
}

pub fn list_namespaces(app_state: &super::AppState, _request: ListNamespacesRequest) -> ResponseJson<Value> {
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

pub fn update_namespace_imports(app_state: &super::AppState, request: UpdateNamespaceImportsRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;
  let ns_definition = request.imports;

  // Validate if ns_definition conforms to Cirru structure
  if let Err(e) = validate_cirru_structure(&ns_definition) {
    return ResponseJson(serde_json::json!({
      "error": format!("Invalid ns_definition structure: {e}")
    }));
  }

  let mut snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  // Check if namespace exists
  let file_data = match snapshot.files.get_mut(&namespace) {
    Some(data) => data,
    None => {
      return ResponseJson(serde_json::json!({
        "error": format!("Namespace '{namespace}' not found")
      }));
    }
  };

  // Convert JSON to Cirru
  let ns_cirru = match json_to_cirru(&ns_definition) {
    Ok(c) => c,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Failed to convert ns_definition to Cirru: {e}")
      }));
    }
  };

  // Update namespace definition
  file_data.ns = CodeEntry::from_code(ns_cirru);

  // Save snapshot
  if let Err(e) = save_snapshot(app_state, &snapshot) {
    return e;
  }

  ResponseJson(serde_json::json!({
    "message": format!("Namespace '{namespace}' imports updated successfully")
  }))
}
