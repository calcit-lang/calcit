use super::tools::{
  DeleteDefinitionRequest, OperateDefinitionAtRequest, ReadDefinitionAtRequest, ReadDefinitionDocRequest, UpdateDefinitionDocRequest,
};
use super::validation::{validate_definition_name, validate_namespace_name};
use crate::mcp::definition_update::{UpdateMode, operate_definition_at_coord};
use crate::mcp::definition_utils::{navigate_to_coord, parse_coord_from_json};
use crate::mcp::tools::OperateDefinitionAtWithLeafRequest;
use crate::snapshot::CodeEntry;
use axum::response::Json as ResponseJson;
use cirru_parser::Cirru;
use serde_json::Value;

/// Internal function to handle both add and overwrite operations
pub fn upsert_definition(
  app_state: &super::AppState,
  namespace: String,
  definition: String,
  syntax_tree: serde_json::Value,
  doc: String,
  replacing: bool,
) -> ResponseJson<Value> {
  // Validate namespace name
  if let Err(validation_error) = validate_namespace_name(&namespace) {
    return ResponseJson(serde_json::json!({
      "error": validation_error
    }));
  }

  // Validate definition name
  if let Err(validation_error) = validate_definition_name(&definition) {
    return ResponseJson(serde_json::json!({
      "error": validation_error
    }));
  }

  let code_cirru = match &syntax_tree {
    serde_json::Value::String(_) => {
      return ResponseJson(serde_json::json!({
        "error": "String format is not supported. Please use nested array format to represent the syntax tree. Example: [\"fn\", [\"x\"], [\"*\", \"x\", \"x\"]]"
      }));
    }
    code_json => {
      // Check if it's a stringified array (common mistake)
      if let serde_json::Value::Array(arr) = code_json {
        if let Some(serde_json::Value::String(first)) = arr.first() {
          if first.starts_with('[') {
            return ResponseJson(serde_json::json!({
              "error": "Detected stringified array format. Please use actual nested arrays, not strings. Example: [\"fn\", [\"x\"], [\"*\", \"x\", \"x\"]] instead of \"[fn [x] [* x x]]\""
            }));
          }
        }
      }

      // Handle array format code
      match super::cirru_utils::json_to_cirru(code_json) {
        Ok(cirru) => cirru,
        Err(e) => {
          return ResponseJson(serde_json::json!({
            "error": format!("Failed to convert code from JSON: {e}")
          }));
        }
      }
    }
  };

  let result = app_state.state_manager.update_current_module(|snapshot| {
    // Check if namespace exists
    let file_data = match snapshot.files.get_mut(&namespace) {
      Some(data) => data,
      None => {
        let available_namespaces: Vec<String> = snapshot.files.keys().cloned().collect();
        return Err(format!(
          "Namespace '{namespace}' not found.\n\nAvailable namespaces: {}\n\nSuggested fixes:\n• Check the namespace name for typos\n• Create the namespace first using 'add_namespace' tool\n• Use one of the existing namespaces listed above",
          if available_namespaces.is_empty() {
            "(none - create a namespace first)".to_string()
          } else {
            available_namespaces.join(", ")
          }
        ));
      }
    };

    let definition_exists = file_data.defs.contains_key(&definition);

    // Check existence based on operation type
    if replacing {
      // For overwrite, definition must exist
      if !definition_exists {
        let existing_definitions: Vec<String> = file_data.defs.keys().cloned().collect();
        return Err(format!(
          "Definition '{definition}' not found in namespace '{namespace}'.\n\nExisting definitions in this namespace: {}\n\nSuggested fixes:\n• Check the definition name for typos\n• Use 'upsert_definition' tool with replacing=false to create a new definition\n• Use one of the existing definitions listed above\n• For incremental updates, consider using 'operate_definition_at' tool",
          if existing_definitions.is_empty() {
            "(none - add a definition first)".to_string()
          } else {
            existing_definitions.join(", ")
          }
        ));
      }
    } else {
      // For add, definition must not exist
      if definition_exists {
        let existing_definitions: Vec<String> = file_data.defs.keys().cloned().collect();
        return Err(format!(
          "Definition '{definition}' already exists in namespace '{namespace}'.\n\nExisting definitions in this namespace: {}\n\nSuggested fixes:\n• Use a different definition name\n• Use 'upsert_definition' tool with replacing=true to replace the existing definition\n• For incremental updates, consider using 'operate_definition_at' tool",
          existing_definitions.join(", ")
        ));
      }
    }

    // Add or update definition
    let code_entry = CodeEntry { doc, examples: vec![], code: code_cirru };
    file_data.defs.insert(definition.clone(), code_entry);
    Ok(())
  });

  match result {
    Ok(()) => {
      let action = if replacing { "updated" } else { "added" };
      let preposition = if replacing { "in" } else { "to" };
      ResponseJson(serde_json::json!({
        "message": format!("Definition '{definition}' {action} {preposition} namespace '{namespace}' successfully")
      }))
    }
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}

pub fn delete_definition(app_state: &super::AppState, request: DeleteDefinitionRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;
  let definition = request.definition;

  // Validate namespace name
  if let Err(validation_error) = validate_namespace_name(&namespace) {
    return ResponseJson(serde_json::json!({
      "error": validation_error
    }));
  }

  // Validate definition name
  if let Err(validation_error) = validate_definition_name(&definition) {
    return ResponseJson(serde_json::json!({
      "error": validation_error
    }));
  }

  let result = app_state.state_manager.update_current_module(|snapshot| {
    // Check if namespace exists
    let file_data = match snapshot.files.get_mut(&namespace) {
      Some(data) => data,
      None => {
        let available_namespaces: Vec<String> = snapshot.files.keys().cloned().collect();
        return Err(format!(
          "Namespace '{namespace}' not found.\n\nAvailable namespaces: {}\n\nSuggested fixes:\n• Check the namespace name for typos\n• Use one of the existing namespaces listed above",
          if available_namespaces.is_empty() {
            "(none)".to_string()
          } else {
            available_namespaces.join(", ")
          }
        ));
      }
    };

    // Check if definition exists
    if !file_data.defs.contains_key(&definition) {
      let existing_definitions: Vec<String> = file_data.defs.keys().cloned().collect();
      return Err(format!(
        "Definition '{definition}' not found in namespace '{namespace}'.\n\nExisting definitions in this namespace: {}\n\nSuggested fixes:\n• Check the definition name for typos\n• Use one of the existing definitions listed above\n• Use 'list_namespace_definitions' tool to see all available definitions",
        if existing_definitions.is_empty() {
          "(none)".to_string()
        } else {
          existing_definitions.join(", ")
        }
      ));
    }

    // Delete definition
    file_data.defs.remove(&definition);
    Ok(())
  });

  match result {
    Ok(()) => ResponseJson(serde_json::json!({
      "message": format!("Definition '{definition}' deleted from namespace '{namespace}' successfully")
    })),
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}

pub fn update_definition_doc(app_state: &super::AppState, request: UpdateDefinitionDocRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;
  let definition = request.definition;
  let doc = request.doc;

  // Validate namespace name
  if let Err(validation_error) = validate_namespace_name(&namespace) {
    return ResponseJson(serde_json::json!({
      "error": validation_error
    }));
  }

  // Validate definition name
  if let Err(validation_error) = validate_definition_name(&definition) {
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

    // Check if definition exists
    let code_entry = match file_data.defs.get_mut(&definition) {
      Some(entry) => entry,
      None => {
        let existing_definitions: Vec<String> = file_data.defs.keys().cloned().collect();
        return Err(format!(
          "Definition '{definition}' not found in namespace '{namespace}'.\n\nExisting definitions in this namespace: {}\n\nSuggested fixes:\n• Check the definition name for typos\n• Use one of the existing definitions listed above\n• Use 'list_namespace_definitions' tool to see all available definitions",
          if existing_definitions.is_empty() {
            "(none - add a definition first)".to_string()
          } else {
            existing_definitions.join(", ")
          }
        ));
      }
    };

    // Update the documentation
    code_entry.doc = doc.clone();
    Ok(())
  });

  match result {
    Ok(()) => ResponseJson(serde_json::json!({
      "message": format!("Documentation for definition '{definition}' in namespace '{namespace}' updated successfully")
    })),
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}

pub fn operate_definition_at(app_state: &super::AppState, request: OperateDefinitionAtRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;
  let definition = request.definition;

  // Validate namespace name
  if let Err(validation_error) = validate_namespace_name(&namespace) {
    return ResponseJson(serde_json::json!({
      "error": validation_error
    }));
  }

  // Validate definition name
  if let Err(validation_error) = validate_definition_name(&definition) {
    return ResponseJson(serde_json::json!({
      "error": validation_error
    }));
  }

  let coord: Vec<usize> = match parse_coord_from_json(&request.coord) {
    Ok(coord_vec) => coord_vec,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Invalid coord parameter: {}\n\nCoord format requirements:\n• Must be a JSON array of non-negative integers\n• Index starts from 0 (zero-based indexing)\n• Example: [0] for first element, [1, 2] for third element of second element\n• Use empty array [] for root level\n\nSuggested fixes:\n• Check that all values are non-negative integers\n• Ensure proper JSON array format: [0, 1, 2]\n• Use 'read_definition_at' tool to explore the structure first", e)
      }));
    }
  };

  // Parse update mode
  let mode = match request.operation.parse::<UpdateMode>() {
    Ok(mode) => mode,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Invalid mode parameter: {}.\n\nValid modes:\n• 'replace' - Replace the element at coord\n• 'after' - Insert new element after the coord position\n• 'before' - Insert new element before the coord position\n• 'delete' - Remove the element at coord (no new_value needed)\n• 'prepend' - Add new element at the beginning of list at coord\n• 'append' - Add new element at the end of list at coord\n\nSuggested fixes:\n• Use one of the exact mode names listed above\n• Check for typos in the mode parameter", e)
      }));
    }
  };

  // Parse new value (required for all modes except delete)
  let new_value_cirru = match &request.new_value {
    serde_json::Value::Array(_) => {
      // Handle JSON array format new_value
      match super::cirru_utils::json_to_cirru(&request.new_value) {
        Ok(cirru) => Some(cirru),
        Err(e) => {
          return ResponseJson(serde_json::json!({
            "error": format!("Failed to convert new_value from JSON: {}\n\nNew value format requirements:\n• Must be a valid JSON array representing Cirru syntax\n• Use nested arrays for complex expressions\n\nValid examples:\n• Simple value: [\"my-value\"]\n• Number: [\"42\"]\n• Function call: [\"fn-name\", \"arg1\", \"arg2\"]\n• Nested expression: [\"fn\", [\"x\"], [\"*\", \"x\", \"x\"]]\n\nSuggested fixes:\n• Check JSON array syntax\n• Ensure proper nesting for complex expressions\n• Use strings for all atomic values", e)
          }));
        }
      }
    }
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "new_value must be a JSON array representing Cirru syntax.\n\nFormat requirements:\n• Must be a JSON array, not a string or other type\n• Use nested arrays for complex expressions\n\nValid examples:\n• Simple value: [\"my-value\"]\n• Number: [\"42\"]\n• Function call: [\"fn-name\", \"arg1\", \"arg2\"]\n• Nested expression: [\"fn\", [\"x\"], [\"*\", \"x\", \"x\"]]\n\nSuggested fixes:\n• Convert string values to JSON arrays: \"value\" → [\"value\"]\n• Use proper JSON array syntax with square brackets"
      }));
    }
  };

  // For delete mode, new_value should be None
  let new_value_cirru = if matches!(mode, UpdateMode::Delete) {
    None
  } else {
    new_value_cirru
  };

  // Parse match content
  let match_content: Option<Cirru> = match super::cirru_utils::json_to_cirru(&request.shallow_check) {
    Ok(cirru) => Some(cirru),
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Failed to convert shallow_check from JSON: {}\n\nShallow check format requirements:\n• Must be a valid JSON array representing the expected Cirru syntax at the coord\n• Used for verification before making changes\n• Only needs to match the beginning part of the content for verification\n\nValid examples:\n• Simple value: [\"current-value\"]\n• Function call: [\"current-fn\", \"arg1\", \"arg2\"]\n• Partial match: [\"fn\", \"...\"] (beginning part with \"...\" indicating more content)\n\nSuggested fixes:\n• Use 'read_definition_at' tool to see current content at coord\n• Ensure shallow_check matches the beginning of the current structure\n• Check JSON array syntax and nesting", e)
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

    // Check if definition exists
    let code_entry = match file_data.defs.get_mut(&definition) {
      Some(entry) => entry,
      None => {
        return Err(format!("Definition '{definition}' not found in namespace '{namespace}'"));
      }
    };

    // Clone the code for the new update logic
    let mut code = code_entry.code.clone();

    // Perform the update using the new logic
    if let Err(e) = operate_definition_at_coord(&mut code, &coord, new_value_cirru.as_ref(), mode, match_content.as_ref()) {
      return Err(format!("Failed to update: {e}"));
    }

    // Update the code entry with the modified code
    code_entry.code = code;
    Ok(())
  });

  match result {
    Ok(()) => ResponseJson(serde_json::json!({
      "message": format!("Definition '{}' updated at coordinate {:?} in namespace '{}' successfully", definition, coord, namespace),
      "tips": {
        "next_steps": [
          format!("Use 'read_definition_at' with namespace='{}', definition='{}', coord=[] to verify the updated definition", namespace, definition),
          "Use 'generate_calcit_incremental' to apply changes to a running Calcit process",
          "Use 'grab_calcit_runner_logs' to check if any errors occurred after incremental update"
        ],
        "check_error": "If the runner reports errors, use 'read_calcit_error_file' to see detailed stack traces"
      }
    })),
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}

/// Read documentation for a specific definition in any namespace (current project or dependency)
pub fn read_definition_doc(app_state: &super::AppState, request: ReadDefinitionDocRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;
  let definition = request.definition;

  // Validate namespace name
  if let Err(validation_error) = validate_namespace_name(&namespace) {
    return ResponseJson(serde_json::json!({
      "error": validation_error
    }));
  }

  // Validate definition name
  if let Err(validation_error) = validate_definition_name(&definition) {
    return ResponseJson(serde_json::json!({
      "error": validation_error
    }));
  }

  // First try to find in current module
  let current_module_result = app_state.state_manager.with_current_module(|snapshot| {
    if let Some(file) = snapshot.files.get(&namespace) {
      if let Some(def_entry) = file.defs.get(&definition) {
        return Some((def_entry.doc.clone(), def_entry.examples.clone()));
      }
    }
    None
  });

  // Get available definitions in current module for context
  let (current_namespace_exists, current_available_definitions) = app_state
    .state_manager
    .with_current_module(|snapshot| {
      if let Some(file) = snapshot.files.get(&namespace) {
        let available_definitions: Vec<String> = file.defs.keys().cloned().collect();
        (true, available_definitions) // namespace exists
      } else {
        (false, Vec::new()) // namespace doesn't exist
      }
    })
    .unwrap_or((false, Vec::new()));

  match current_module_result {
    Ok(Some((doc, examples))) => {
      return ResponseJson(serde_json::json!({
        "documentation": doc,
        "examples": examples,
        "source": "current_project",
        "namespace": namespace,
        "definition": definition
      }));
    }
    Ok(None) => {
      // Not found in current module, check if namespace exists for better error context
      if current_namespace_exists {
        // Namespace exists but definition not found - we'll try dependencies but prepare context
      } else {
        // Namespace doesn't exist in current project - we'll try dependencies but prepare context
      }
    }
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Failed to read current module: {}", e),
        "context": {
          "namespace": namespace,
          "definition": definition,
          "source": "current_project",
          "issue": "module_read_failure",
          "suggestions": [
            "Check if the project is properly loaded",
            "Verify the project structure is valid",
            "Try reloading the project",
            "Check for file system permissions issues"
          ]
        }
      }));
    }
  }

  // Try to find in dependency modules
  match app_state.state_manager.get_dependency_module(&namespace) {
    Ok(dep_snapshot) => {
      if let Some(file) = dep_snapshot.files.get(&namespace) {
        if let Some(def_entry) = file.defs.get(&definition) {
          ResponseJson(serde_json::json!({
            "documentation": def_entry.doc,
            "examples": def_entry.examples,
            "source": "dependency",
            "namespace": namespace,
            "definition": definition
          }))
        } else {
          // Get available definitions in the dependency namespace for context
          let available_definitions: Vec<String> = file.defs.keys().cloned().collect();
          ResponseJson(serde_json::json!({
            "error": format!("Definition '{}' not found in namespace '{}' (dependency module)", definition, namespace),
            "context": {
              "namespace": namespace,
              "definition": definition,
              "source": "dependency",
              "available_definitions": available_definitions,
              "suggestions": [
                "Check the definition name for typos",
                "Use 'list_namespace_definitions' tool to see all available definitions",
                "Verify the definition exists in the dependency module",
                format!("Available definitions in '{}': {}", namespace,
                  if available_definitions.is_empty() { "(none)".to_string() }
                  else { available_definitions.join(", ") })
              ]
            }
          }))
        }
      } else {
        ResponseJson(serde_json::json!({
          "error": format!("Namespace '{}' not found in dependency modules", namespace),
          "context": {
            "namespace": namespace,
            "definition": definition,
            "source": "dependency",
            "issue": "namespace_not_found_in_dependencies",
            "suggestions": [
              "Check if the namespace name is correct",
              "Verify the dependency is properly loaded",
              "Use 'list_dependency_namespaces' tool to see available dependency namespaces",
              "Check if this namespace exists in the current project instead"
            ]
          }
        }))
      }
    }
    Err(dep_error) => {
      // Namespace not found in dependencies either, provide comprehensive context
      let mut error_context = serde_json::json!({
        "namespace": namespace,
        "definition": definition,
        "issue": "namespace_not_found_anywhere",
        "dependency_error": format!("{}", dep_error),
        "current_project": {
          "namespace_exists": current_namespace_exists,
          "available_definitions": current_available_definitions
        }
      });

      let mut suggestions = vec![
        "Verify the namespace name is spelled correctly".to_string(),
        "Check if the namespace exists using 'list_namespaces' tool".to_string(),
        "Use 'list_dependency_namespaces' tool to see available dependency namespaces".to_string(),
        "Ensure all required dependencies are loaded".to_string(),
      ];

      if current_namespace_exists {
        suggestions.insert(
          0,
          format!(
            "Definition '{}' exists in current project namespace '{}' but not found. Available definitions: {}",
            definition,
            namespace,
            if current_available_definitions.is_empty() {
              "(none)".to_string()
            } else {
              current_available_definitions.join(", ")
            }
          ),
        );
        suggestions.push("Check if the definition name has typos".to_string());
      } else {
        suggestions.push("Consider if this might be a new namespace that needs to be created".to_string());
      }

      error_context["suggestions"] = serde_json::json!(suggestions);

      ResponseJson(serde_json::json!({
        "error": format!("Namespace '{}' not found in current project or dependencies", namespace),
        "context": error_context
      }))
    }
  }
}

pub fn read_definition_at(app_state: &super::AppState, request: ReadDefinitionAtRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;
  let definition = request.definition;

  // Validate namespace name
  if let Err(validation_error) = validate_namespace_name(&namespace) {
    return ResponseJson(serde_json::json!({
      "error": validation_error
    }));
  }

  // Validate definition name
  if let Err(validation_error) = validate_definition_name(&definition) {
    return ResponseJson(serde_json::json!({
      "error": validation_error
    }));
  }

  let coord: Vec<usize> = match parse_coord_from_json(&request.coord) {
    Ok(coord_vec) => coord_vec,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Invalid coord parameter: {}\n\nCoord format requirements:\n• Must be a JSON array of non-negative integers\n• Index starts from 0 (zero-based indexing)\n• Example: [0] for first element, [1, 2] for third element of second element\n• Use empty array [] for root level\n\nSuggested fixes:\n• Check that all values are non-negative integers\n• Ensure proper JSON array format: [0, 1, 2]\n• Start with [] to read the entire definition", e)
      }));
    }
  };

  // First try to find in current module
  let current_module_result = app_state.state_manager.with_current_module(|snapshot| {
    if let Some(file_data) = snapshot.files.get(&namespace) {
      if let Some(code_entry) = file_data.defs.get(&definition) {
        return Some((code_entry.clone(), "current_project".to_string()));
      } else {
        // Namespace exists but definition not found - collect available definitions for context
        let available_definitions: Vec<String> = file_data.defs.keys().cloned().collect();

        let mut error_context = serde_json::json!({
          "namespace": namespace,
          "definition": definition,
          "source": "current_project",
          "issue": "definition_not_found_in_namespace",
          "available_definitions": available_definitions
        });

        let mut suggestions = vec![
          "Check if the definition name is spelled correctly".to_string(),
          "Use 'list_definitions' tool to see all available definitions in this namespace".to_string(),
        ];

        if !available_definitions.is_empty() {
          suggestions.insert(
            0,
            format!("Available definitions in '{}': {}", namespace, available_definitions.join(", ")),
          );
          suggestions.push("Look for similar definition names in the namespace".to_string());
        } else {
          suggestions.push("This namespace exists but has no definitions yet".to_string());
          suggestions.push("Consider adding the definition using 'add_definition' tool".to_string());
        }

        error_context["suggestions"] = serde_json::json!(suggestions);

        // Return early with error context
        return None; // Will be handled below
      }
    }
    None
  });

  // Check if we found it in current module
  if let Ok(Some((entry, src))) = current_module_result {
    return ResponseJson(serde_json::json!({
      "definition": entry,
      "source": src
    }));
  }

  // Check if namespace exists in current module but definition doesn't
  let namespace_exists_in_current = app_state
    .state_manager
    .with_current_module(|snapshot| snapshot.files.contains_key(&namespace))
    .unwrap_or(false);

  if namespace_exists_in_current {
    // We already handled this case above, but need to return the error here
    let available_definitions = app_state
      .state_manager
      .with_current_module(|snapshot| {
        snapshot
          .files
          .get(&namespace)
          .map(|file_data| file_data.defs.keys().cloned().collect::<Vec<String>>())
          .unwrap_or_default()
      })
      .unwrap_or_default();

    let mut error_context = serde_json::json!({
      "namespace": namespace,
      "definition": definition,
      "source": "current_project",
      "issue": "definition_not_found_in_namespace",
      "available_definitions": available_definitions
    });

    let mut suggestions = vec![
      "Check if the definition name is spelled correctly".to_string(),
      "Use 'list_definitions' tool to see all available definitions in this namespace".to_string(),
    ];

    if !available_definitions.is_empty() {
      suggestions.insert(
        0,
        format!("Available definitions in '{}': {}", namespace, available_definitions.join(", ")),
      );
      suggestions.push("Look for similar definition names in the namespace".to_string());
    } else {
      suggestions.push("This namespace exists but has no definitions yet".to_string());
      suggestions.push("Consider adding the definition using 'add_definition' tool".to_string());
    }

    error_context["suggestions"] = serde_json::json!(suggestions);

    return ResponseJson(serde_json::json!({
      "error": format!("Definition '{}' not found in namespace '{}'", definition, namespace),
      "context": error_context
    }));
  }

  // Not found in current module, try dependencies
  let (code_entry, source) = match app_state.state_manager.get_dependency_module(&namespace) {
    Ok(dep_snapshot) => {
      if let Some(file_data) = dep_snapshot.files.get(&namespace) {
        if let Some(code_entry) = file_data.defs.get(&definition) {
          (code_entry.clone(), "dependency".to_string())
        } else {
          // Get available definitions in dependency namespace for context
          let available_definitions: Vec<String> = file_data.defs.keys().cloned().collect();

          let mut error_context = serde_json::json!({
            "namespace": namespace,
            "definition": definition,
            "source": "dependency",
            "issue": "definition_not_found_in_dependency_namespace",
            "available_definitions": available_definitions
          });

          let mut suggestions = vec![
            "Check if the definition name is spelled correctly".to_string(),
            "Use 'list_definitions' tool to see all available definitions in this namespace".to_string(),
          ];

          if !available_definitions.is_empty() {
            suggestions.insert(
              0,
              format!(
                "Available definitions in dependency '{}': {}",
                namespace,
                available_definitions.join(", ")
              ),
            );
            suggestions.push("Look for similar definition names in the dependency namespace".to_string());
          } else {
            suggestions.push("This dependency namespace exists but has no definitions".to_string());
          }

          error_context["suggestions"] = serde_json::json!(suggestions);

          return ResponseJson(serde_json::json!({
            "error": format!("Definition '{}' not found in namespace '{}' (dependency)", definition, namespace),
            "context": error_context
          }));
        }
      } else {
        // Get available dependency namespaces for context
        let available_dep_namespaces: Vec<String> = dep_snapshot.files.keys().cloned().collect();

        let mut error_context = serde_json::json!({
          "namespace": namespace,
          "definition": definition,
          "source": "dependency",
          "issue": "namespace_not_found_in_dependencies",
          "available_dependency_namespaces": available_dep_namespaces
        });

        let mut suggestions = vec![
          "Check if the namespace name is spelled correctly".to_string(),
          "Use 'list_dependency_namespaces' tool to see all available dependency namespaces".to_string(),
          "Ensure all required dependencies are loaded".to_string(),
        ];

        if !available_dep_namespaces.is_empty() {
          suggestions.insert(
            0,
            format!("Available dependency namespaces: {}", available_dep_namespaces.join(", ")),
          );
          suggestions.push("Look for similar namespace names in the available dependencies".to_string());
        }

        error_context["suggestions"] = serde_json::json!(suggestions);

        return ResponseJson(serde_json::json!({
          "error": format!("Namespace '{}' not found in dependency modules", namespace),
          "context": error_context
        }));
      }
    }
    Err(_) => {
      // Get available namespaces from current module for error message
      let available_namespaces = app_state
        .state_manager
        .with_current_module(|snapshot| snapshot.files.keys().cloned().collect::<Vec<String>>())
        .unwrap_or_else(|_| vec![]);

      let mut error_context = serde_json::json!({
        "namespace": namespace,
        "definition": definition,
        "source": "current_project",
        "issue": "namespace_not_found_anywhere",
        "available_namespaces": available_namespaces
      });

      let mut suggestions = vec![
        "Check the namespace name for typos".to_string(),
        "Use 'list_namespaces' tool to see all available namespaces".to_string(),
      ];

      if !available_namespaces.is_empty() {
        suggestions.insert(0, format!("Available namespaces: {}", available_namespaces.join(", ")));
        suggestions.push("Use one of the existing namespaces listed above".to_string());
      } else {
        suggestions.push("Consider creating the namespace first using 'add_namespace' tool".to_string());
      }

      error_context["suggestions"] = serde_json::json!(suggestions);

      return ResponseJson(serde_json::json!({
        "error": format!("Namespace '{namespace}' not found"),
        "context": error_context
      }));
    }
  };

  // Navigate to the target coordinate
  let target = match navigate_to_coord(&code_entry.code, &coord) {
    Ok(t) => t,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Failed to navigate to coordinate {coord:?}: {e}\n\nNavigation troubleshooting:\n• Coordinate {coord:?} may be out of bounds\n• Use empty array [] to read the entire definition\n• Use shorter coordinates to navigate step by step\n• Check if the target is a list (only lists can have child elements)\n\nSuggested fixes:\n• Start with [] to see the root structure\n• Use coordinates like [0], [1], [2] for top-level elements\n• For nested access, build coordinates incrementally: [0] → [0, 1] → [0, 1, 2]")
      }));
    }
  };

  let expr_length = match &target {
    Cirru::Leaf(_) => None,
    Cirru::List(xs) => Some(xs.len()),
  };

  ResponseJson(serde_json::json!({
    "value": target,
    "expr_length": expr_length,
    "source": source,
    "message": format!("Definition '{}' read at coordinate {:?} in namespace '{}' successfully (from {})", definition, coord, namespace, source),
    "tips": {
      "next_steps": if source == "current_project" {
        vec![
          format!("Use 'operate_definition_at' with namespace='{}', definition='{}', coord={:?} to modify this location", namespace, definition, coord),
          "Use 'operate_definition_at_with_leaf' for simpler leaf value replacements".to_string(),
          "Navigate deeper by appending indices to the coord array (e.g., [2, 1] becomes [2, 1, 0])".to_string()
        ]
      } else {
        vec![
          "This definition is from a dependency and is read-only".to_string(),
          "Use 'read_definition_doc' to see documentation".to_string()
        ]
      },
      "coord_hint": "Each integer in coord navigates one level deeper into the expression tree (0-indexed)"
    }
  }))
}

pub fn operate_definition_at_with_leaf(
  app_state: &super::AppState,
  request: OperateDefinitionAtWithLeafRequest,
) -> ResponseJson<Value> {
  let namespace = request.namespace;
  let definition = request.definition;

  // Validate namespace name
  if let Err(validation_error) = validate_namespace_name(&namespace) {
    return ResponseJson(serde_json::json!({
      "error": validation_error
    }));
  }

  // Validate definition name
  if let Err(validation_error) = validate_definition_name(&definition) {
    return ResponseJson(serde_json::json!({
      "error": validation_error
    }));
  }

  let coord: Vec<usize> = match parse_coord_from_json(&request.coord) {
    Ok(coord_vec) => coord_vec,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Invalid coord parameter: {}", e)
      }));
    }
  };

  // Parse update mode
  let mode = match request.operation.parse::<UpdateMode>() {
    Ok(mode) => mode,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Invalid mode parameter: {}. Valid modes are: replace, after, before, delete, prepend, append", e)
      }));
    }
  };

  // Create new value as Cirru leaf
  let new_value_cirru = Cirru::Leaf(request.new_value.into());

  // Parse match content if provided
  let match_content_cirru = request.shallow_check.map(|s| Cirru::Leaf(s.into()));

  let result = app_state.state_manager.update_current_module(|snapshot| {
    // Check if namespace exists
    let file_data = match snapshot.files.get_mut(&namespace) {
      Some(data) => data,
      None => {
        return Err(format!("Namespace '{namespace}' not found"));
      }
    };

    // Check if definition exists
    let code_entry = match file_data.defs.get_mut(&definition) {
      Some(entry) => entry,
      None => {
        return Err(format!("Definition '{definition}' not found in namespace '{namespace}'"));
      }
    };

    // Clone the code for the new update logic
    let mut code = code_entry.code.clone();

    // Perform the update using the new logic
    if let Err(e) = operate_definition_at_coord(&mut code, &coord, Some(&new_value_cirru), mode, match_content_cirru.as_ref()) {
      return Err(format!("Failed to update: {e}"));
    }

    // Update the code entry with the modified code
    code_entry.code = code;
    Ok(())
  });

  match result {
    Ok(()) => ResponseJson(serde_json::json!({
      "message": format!("Definition '{}' updated at coordinate {:?} in namespace '{}' successfully", definition, coord, namespace),
      "tips": {
        "next_steps": [
          format!("Use 'read_definition_at' with namespace='{}', definition='{}', coord=[] to verify the updated definition", namespace, definition),
          "Use 'generate_calcit_incremental' to apply changes to a running Calcit process",
          "Use 'grab_calcit_runner_logs' to check if any errors occurred after incremental update"
        ],
        "check_error": "If the runner reports errors, use 'read_calcit_error_file' to see detailed stack traces"
      }
    })),
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}
