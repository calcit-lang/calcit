use super::cirru_utils::json_to_cirru;
use super::tools::{
  AddNamespaceRequest, DeleteNamespaceRequest, ListNamespacesRequest, UpdateNamespaceDocRequest, UpdateNamespaceImportsRequest,
};
use super::validation::validate_namespace_name;
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

  // Validate namespace name
  if let Err(validation_error) = validate_namespace_name(&namespace) {
    return ResponseJson(serde_json::json!({
      "error": validation_error
    }));
  }

  let result = app_state.state_manager.update_current_module(|snapshot| {
    // Check if namespace already exists
    if snapshot.files.contains_key(&namespace) {
      let existing_namespaces: Vec<String> = snapshot.files.keys().cloned().collect();
      return Err(format!(
        "Namespace '{namespace}' already exists.\n\nExisting namespaces: {}\n\nSuggested fixes:\n• Use a different namespace name\n• Use 'delete_namespace' tool to remove the existing namespace first\n• Use 'update_namespace_imports' tool to modify the existing namespace",
        existing_namespaces.join(", ")
      ));
    }

    // Check if namespace starts with current package name followed by a dot
    let package_prefix = format!("{}.", snapshot.package);
    if !namespace.starts_with(&package_prefix) {
      let existing_namespaces: Vec<String> = snapshot.files.keys().cloned().collect();
      let valid_examples: Vec<String> = existing_namespaces.iter()
        .filter(|ns| ns.starts_with(&package_prefix))
        .take(3)
        .cloned()
        .collect();

      return Err(format!(
        "Namespace '{namespace}' must start with current package name '{}' followed by a dot.\n\nCurrent package: {}\nRequired prefix: {}\n\nValid namespace examples: {}\n\nSuggested fixes:\n• Use format: {}.your-namespace-name\n• Check existing namespaces for naming patterns",
        snapshot.package,
        snapshot.package,
        package_prefix,
        if valid_examples.is_empty() {
          format!("{}.example", snapshot.package)
        } else {
          valid_examples.join(", ")
        },
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

pub fn update_namespace_doc(app_state: &super::AppState, request: UpdateNamespaceDocRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;
  let doc = request.doc;

  // Validate namespace name
  if let Err(validation_error) = validate_namespace_name(&namespace) {
    return ResponseJson(serde_json::json!({
      "error": validation_error
    }));
  }

  // Check if this is a dependency namespace (not in current root namespace/package)
  let result = app_state.state_manager.update_current_module(|snapshot| {
    // Check if namespace exists in current project
    let file_data = match snapshot.files.get_mut(&namespace) {
      Some(data) => data,
      None => {
        // Check if it might be a dependency namespace
        let available_namespaces: Vec<String> = snapshot.files.keys().cloned().collect();
        let error_msg = if namespace.contains('.') && !available_namespaces.iter().any(|ns| ns.starts_with(&namespace.split('.').next().unwrap_or("").to_string())) {
          format!(
            "Namespace '{namespace}' appears to be from a dependency module and cannot be modified.\n\nThis tool only works for namespaces in the current root namespace/package.\n\nAvailable namespaces in current project: {}\n\nSuggested fixes:\n• Use a namespace from the current project\n• Dependencies are read-only and cannot be modified",
            if available_namespaces.is_empty() {
              "(none - create a namespace first)".to_string()
            } else {
              available_namespaces.join(", ")
            }
          )
        } else {
          format!(
            "Namespace '{namespace}' not found in current project.\n\nAvailable namespaces: {}\n\nSuggested fixes:\n• Check the namespace name for typos\n• Create the namespace first using 'add_namespace' tool\n• Use one of the existing namespaces listed above",
            if available_namespaces.is_empty() {
              "(none - create a namespace first)".to_string()
            } else {
              available_namespaces.join(", ")
            }
          )
        };
        return Err(error_msg);
      }
    };

    // Update the namespace documentation
    file_data.ns.doc = doc.clone();
    Ok(())
  });

  match result {
    Ok(()) => ResponseJson(serde_json::json!({
      "message": format!("Documentation for namespace '{namespace}' updated successfully")
    })),
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}

pub fn delete_namespace(app_state: &super::AppState, request: DeleteNamespaceRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;

  // Validate namespace name
  if let Err(validation_error) = validate_namespace_name(&namespace) {
    return ResponseJson(serde_json::json!({
      "error": validation_error
    }));
  }

  let result = app_state.state_manager.update_current_module(|snapshot| {
    // Check if namespace exists
    if !snapshot.files.contains_key(&namespace) {
      let existing_namespaces: Vec<String> = snapshot.files.keys().cloned().collect();
      return Err(format!(
        "Namespace '{namespace}' not found.\n\nExisting namespaces: {}\n\nSuggested fixes:\n• Check the namespace name for typos\n• Use 'list_namespaces' tool to see all available namespaces\n• Use one of the existing namespaces listed above",
        if existing_namespaces.is_empty() {
          "(none)".to_string()
        } else {
          existing_namespaces.join(", ")
        }
      ));
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

pub fn list_namespaces(app_state: &super::AppState, request: ListNamespacesRequest) -> ResponseJson<Value> {
  match app_state.state_manager.with_current_module(|snapshot| {
    let mut namespaces: Vec<String> = snapshot.files.keys().cloned().collect();

    // If include_dependency_namespaces is true, add dependency namespaces
    if request.include_dependency_namespaces {
      // Get dependency namespaces from state manager
      if let Ok(dep_namespaces) = app_state.state_manager.get_dependency_namespaces() {
        for namespace in dep_namespaces {
          if !namespaces.contains(&namespace) {
            namespaces.push(namespace);
          }
        }
      }
    }

    namespaces.sort();
    namespaces
  }) {
    Ok(namespaces) => ResponseJson(serde_json::json!({
      "namespaces": namespaces,
      "tips": {
        "next_steps": [
          "Use 'list_namespace_definitions' with namespace='<name>' to see all definitions in a namespace",
          "Use 'read_namespace' with namespace='<name>' to see namespace details including imports",
          "Use 'add_namespace' to create a new namespace"
        ],
        "info": "Namespaces are the primary organizational units in Calcit, similar to modules in other languages"
      }
    })),
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}

pub fn update_namespace_imports(app_state: &super::AppState, request: UpdateNamespaceImportsRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;
  let imports_array = request.imports;

  // Validate namespace name
  if let Err(validation_error) = validate_namespace_name(&namespace) {
    return ResponseJson(serde_json::json!({
      "error": validation_error
    }));
  }

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
