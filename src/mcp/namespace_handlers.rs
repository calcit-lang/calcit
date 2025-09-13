use super::cirru_utils::json_to_cirru;
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

    // Check if namespace starts with current package name followed by a dot
    let package_prefix = format!("{}.", snapshot.package);
    if !namespace.starts_with(&package_prefix) {
      return Err(format!(
        "Namespace '{namespace}' must start with current package name '{}' followed by a dot",
        snapshot.package
      ));
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
  let imports_array = request.imports;

  // Validate that all elements in the imports array are arrays
  for (i, import_item) in imports_array.iter().enumerate() {
    let import_array = match import_item.as_array() {
      Some(arr) => arr,
      None => {
        return ResponseJson(serde_json::json!({
          "error": format!("Import item at index {i} is not an array")
        }));
      }
    };

    // Validate that the array has at least 2 elements
    if import_array.len() < 2 {
      return ResponseJson(serde_json::json!({
        "error": format!("Import item at index {i} must have at least 2 elements")
      }));
    }

    // Validate that the second element is one of :refer, :as, or :default
    let second_element = match import_array[1].as_str() {
      Some(s) => s,
      None => {
        return ResponseJson(serde_json::json!({
          "error": format!("Second element of import item at index {i} must be a string")
        }));
      }
    };

    match second_element {
      ":refer" => {
        // For :refer, the third element must be an array
        if import_array.len() < 3 {
          return ResponseJson(serde_json::json!({
            "error": format!("Import item at index {i} with :refer must have a third element")
          }));
        }
        if !import_array[2].is_array() {
          return ResponseJson(serde_json::json!({
            "error": format!("Third element of import item at index {i} with :refer must be an array")
          }));
        }
      }
      ":as" | ":default" => {
        // For :as and :default, the third element must be a string (symbol)
        if import_array.len() < 3 {
          return ResponseJson(serde_json::json!({
            "error": format!("Import item at index {i} with {second_element} must have a third element")
          }));
        }
        if !import_array[2].is_string() {
          return ResponseJson(serde_json::json!({
            "error": format!("Third element of import item at index {i} with {second_element} must be a string (symbol)")
          }));
        }
      }
      _ => {
        return ResponseJson(serde_json::json!({
          "error": format!("Second element of import item at index {i} must be one of :refer, :as, or :default, got: {second_element}")
        }));
      }
    }
  }

  // Convert imports JSON array to Cirru structures
  let mut imports_cirru = Vec::new();
  for import_json in imports_array {
    match json_to_cirru(&import_json) {
      Ok(cirru) => imports_cirru.push(cirru),
      Err(e) => {
        return ResponseJson(serde_json::json!({
          "error": format!("Failed to convert import to Cirru: {e}")
        }));
      }
    }
  }

  // Create the imports section as Cirru: [:require, ...imports]
  let mut require_section = vec![cirru_parser::Cirru::leaf(":require")];
  require_section.extend(imports_cirru);
  let imports_cirru_list = cirru_parser::Cirru::List(require_section);

  let result = app_state.state_manager.update_current_module(|snapshot| {
    // Check if namespace exists
    let file_data = match snapshot.files.get_mut(&namespace) {
      Some(data) => data,
      None => {
        return Err(format!("Namespace '{namespace}' not found"));
      }
    };

    // Get the current namespace definition as Cirru
    let current_ns_cirru = &file_data.ns.code;

    // Ensure the current namespace definition is a list with at least 2 elements
    let (ns_keyword, ns_name) = match current_ns_cirru {
      cirru_parser::Cirru::List(list) if list.len() >= 2 => (&list[0], &list[1]),
      _ => {
        return Err(format!("Invalid namespace definition structure for '{namespace}'"));
      }
    };

    // Create the new complete namespace definition: ["ns", "namespace_name", [":require", ...imports]]
    let complete_ns_cirru = cirru_parser::Cirru::List(vec![
      ns_keyword.clone(), // "ns"
      ns_name.clone(),    // namespace name
      imports_cirru_list, // [":require", ...imports]
    ]);

    // Update namespace definition
    file_data.ns = CodeEntry::from_code(complete_ns_cirru);
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
