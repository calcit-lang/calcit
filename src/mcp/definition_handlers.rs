use super::tools::{
  DeleteDefinitionRequest, OperateDefinitionAtRequest, ReadDefinitionAtRequest, UpdateDefinitionDocRequest, UpsertDefinitionRequest,
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
fn upsert_definition(
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
    let code_entry = CodeEntry { doc, code: code_cirru };
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
    Err(e) => {
      ResponseJson(serde_json::json!({
        "error": e
      }))
    }
  }
}

/// Save snapshot data
/// save_snapshot function moved to cirru_utils::save_snapshot_to_file to avoid duplication

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
      "message": format!("Definition '{}' updated at coordinate {:?} in namespace '{}' successfully", definition, coord, namespace)
    })),
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
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

  let result = app_state.state_manager.with_current_module(|snapshot| {
    // Check if namespace exists
    let file_data = match snapshot.files.get(&namespace) {
      Some(data) => data,
      None => {
        let available_namespaces: Vec<String> = snapshot.files.keys().cloned().collect();
        return Err(format!(
          "Namespace '{namespace}' not found.\n\nAvailable namespaces: {}\n\nSuggested fixes:\n• Check the namespace name for typos\n• Use 'list_namespaces' tool to see all available namespaces\n• Use one of the existing namespaces listed above",
          if available_namespaces.is_empty() {
            "(none)".to_string()
          } else {
            available_namespaces.join(", ")
          }
        ));
      }
    };

    // Check if definition exists
    let code_entry = match file_data.defs.get(&definition) {
      Some(entry) => entry,
      None => {
        let existing_definitions: Vec<String> = file_data.defs.keys().cloned().collect();
        return Err(format!(
          "Definition '{definition}' not found in namespace '{namespace}'.\n\nExisting definitions in this namespace: {}\n\nSuggested fixes:\n• Check the definition name for typos\n• Use 'list_namespace_definitions' tool to see all available definitions\n• Use one of the existing definitions listed above",
          if existing_definitions.is_empty() {
            "(none)".to_string()
          } else {
            existing_definitions.join(", ")
          }
        ));
      }
    };

    // Navigate to the target coordinate
    let target = match navigate_to_coord(&code_entry.code, &coord) {
      Ok(t) => t,
      Err(e) => {
        return Err(format!("Failed to navigate to coordinate {coord:?}: {e}\n\nNavigation troubleshooting:\n• Coordinate {coord:?} may be out of bounds\n• Use empty array [] to read the entire definition\n• Use shorter coordinates to navigate step by step\n• Check if the target is a list (only lists can have child elements)\n\nSuggested fixes:\n• Start with [] to see the root structure\n• Use coordinates like [0], [1], [2] for top-level elements\n• For nested access, build coordinates incrementally: [0] → [0, 1] → [0, 1, 2]"));
      }
    };

    let expr_length = match target {
      Cirru::Leaf(_) => None,
      Cirru::List(xs) => Some(xs.len()),
    };

    Ok((target.clone(), expr_length))
  });

  match result {
    Ok(Ok((value, expr_length))) => ResponseJson(serde_json::json!({
      "value": value,
      "expr_length": expr_length,
      "message": format!("Definition '{}' read at coordinate {:?} in namespace '{}' successfully", definition, coord, namespace)
    })),
    Ok(Err(e)) => ResponseJson(serde_json::json!({
      "error": e
    })),
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}

pub fn upsert_definition_public(app_state: &super::AppState, request: UpsertDefinitionRequest) -> ResponseJson<Value> {
  upsert_definition(
    app_state,
    request.namespace,
    request.definition,
    request.syntax_tree,
    request.doc,
    request.replacing,
  )
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
      "message": format!("Definition '{}' updated at coordinate {:?} in namespace '{}' successfully", definition, coord, namespace)
    })),
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}
