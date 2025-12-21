use super::tools::{GetPackageNameRequest, ListDefinitionsRequest, ReadNamespaceRequest};
use super::validation::validate_namespace_name;
use axum::response::Json as ResponseJson;
use serde_json::Value;

pub fn list_namespace_definitions(app_state: &super::AppState, request: ListDefinitionsRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;

  // Validate namespace name
  if let Err(validation_error) = validate_namespace_name(&namespace) {
    return ResponseJson(serde_json::json!({
      "error": validation_error
    }));
  }

  // First try to find in current module
  let current_module_result = app_state.state_manager.with_current_module(|snapshot| {
    if let Some(file_data) = snapshot.files.get(&namespace) {
      let definitions: Vec<String> = file_data.defs.keys().cloned().collect();
      Some(ResponseJson(serde_json::json!({
        "namespace": namespace,
        "definitions": definitions,
        "source": "current_project",
        "tips": {
          "next_steps": [
            format!("Use 'read_definition_at' with namespace='{}' and definition='<name>' and coord=[] to read a specific definition's full content", namespace),
            format!("Use 'upsert_definition' with namespace='{}' to create a new definition", namespace),
            format!("Use 'read_namespace' with namespace='{}' to see import rules and namespace metadata", namespace)
          ],
          "edit_hint": "For modifying existing definitions, first use 'read_definition_at' to understand the structure, then use 'operate_definition_at' for precise edits"
        }
      })))
    } else {
      None
    }
  });

  match current_module_result {
    Ok(Some(response)) => {
      return response;
    }
    Ok(None) => {
      // Not found in current module, try dependencies
    }
    Err(_) => {
      // If current module fails to load, try dependencies directly
    }
  }

  // Try to find in dependency modules
  match app_state.state_manager.find_namespace_in_dependencies(&namespace) {
    Ok(Some((package_name, file_data))) => {
      let definitions: Vec<String> = file_data.defs.keys().cloned().collect();
      ResponseJson(serde_json::json!({
        "namespace": namespace,
        "definitions": definitions,
        "source": "dependency",
        "package": package_name,
        "tips": {
          "next_steps": [
            format!("Use 'read_definition_doc' with namespace='{}' and definition='<name>' to read documentation for a definition", namespace),
            "Dependency definitions are read-only and cannot be modified"
          ],
          "info": "This namespace is from a dependency package"
        }
      }))
    }
    Ok(None) => {
      // Namespace not found anywhere, collect available namespaces for error message
      let mut available_namespaces = Vec::new();
      let mut available_dep_namespaces = Vec::new();

      // Try to get current module namespaces (if available)
      if let Ok(current_namespaces) = app_state
        .state_manager
        .with_current_module(|snapshot| snapshot.files.keys().cloned().collect::<Vec<String>>())
      {
        available_namespaces = current_namespaces;
      }

      // Get dependency namespaces
      if let Ok(dep_namespaces) = app_state.state_manager.get_all_dependency_namespaces() {
        available_dep_namespaces = dep_namespaces.into_iter().map(|(ns, _pkg)| ns).collect();
      }

      ResponseJson(serde_json::json!({
        "error": format!("Namespace '{}' not found", namespace),
        "context": {
          "namespace": namespace,
          "available_namespaces": available_namespaces,
          "available_dependency_namespaces": available_dep_namespaces,
          "suggestions": [
            "Check the namespace name for typos",
            "Use 'list_namespaces' tool to see all available namespaces",
            "Use 'list_dependency_namespaces' tool to see available dependency namespaces"
          ]
        }
      }))
    }
    Err(e) => ResponseJson(serde_json::json!({
      "error": format!("Failed to search dependencies: {}", e)
    })),
  }
}

// list_namespaces function moved to namespace_handlers.rs to avoid duplication

pub fn get_package_name(app_state: &super::AppState, _request: GetPackageNameRequest) -> ResponseJson<Value> {
  let result = app_state.state_manager.with_current_module(|snapshot| {
    ResponseJson(serde_json::json!({
      "package_name": snapshot.package
    }))
  });

  match result {
    Ok(response) => response,
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}

pub fn read_namespace(app_state: &super::AppState, request: ReadNamespaceRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;

  // Validate namespace name
  if let Err(validation_error) = validate_namespace_name(&namespace) {
    return ResponseJson(serde_json::json!({
      "error": validation_error
    }));
  }

  let result = app_state.state_manager.with_current_module(|snapshot| {
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
      let code_str = cirru_parser::format(&[code_entry.code.clone()], true.into()).unwrap_or("(failed to convert code)".to_string());
      let code_preview = if code_str.len() > 40 {
        format!("{}...(too long)", &code_str[..40])
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
      "doc": file_data.ns.doc
    }))
  });

  match result {
    Ok(response) => response,
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}
