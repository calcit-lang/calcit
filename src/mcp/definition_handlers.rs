use super::tools::{
  AddDefinitionRequest, DeleteDefinitionRequest, OverwriteDefinitionRequest, ReadDefinitionAtRequest, UpdateDefinitionAtRequest,
};
use crate::mcp::definition_update::{UpdateMode, update_definition_at_coord};
use crate::mcp::definition_utils::{navigate_to_coord, parse_coord_from_json};
use crate::snapshot::CodeEntry;
use axum::response::Json as ResponseJson;
use cirru_parser::Cirru;
use serde_json::Value;

/// Save snapshot data
/// save_snapshot function moved to cirru_utils::save_snapshot_to_file to avoid duplication
pub fn add_definition(app_state: &super::AppState, request: AddDefinitionRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;
  let definition = request.definition;

  let code_cirru = match &request.code {
    serde_json::Value::String(s) => {
      // Handle string format code (backward compatibility)
      match cirru_parser::parse(s) {
        Ok(parsed) => {
          if parsed.is_empty() {
            return ResponseJson(serde_json::json!({
              "error": "Code cannot be empty"
            }));
          }
          parsed[0].clone()
        }
        Err(e) => {
          return ResponseJson(serde_json::json!({
            "error": format!("Failed to parse code: {e}")
          }));
        }
      }
    }
    code_json => {
      // Handle array format code (new format)
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

  let doc = "".to_string(); // AddDefinitionRequest doesn't include doc field

  let result = app_state.state_manager.update_current_module(|snapshot| {
    // Check if namespace exists
    let file_data = match snapshot.files.get_mut(&namespace) {
      Some(data) => data,
      None => {
        return Err(format!("Namespace '{namespace}' not found"));
      }
    };

    // Check if definition already exists
    if file_data.defs.contains_key(&definition) {
      return Err(format!("Definition '{definition}' already exists in namespace '{namespace}'"));
    }

    // Add new definition
    let code_entry = CodeEntry { doc, code: code_cirru };
    file_data.defs.insert(definition.clone(), code_entry);
    Ok(())
  });

  match result {
    Ok(()) => {}
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  }

  ResponseJson(serde_json::json!({
    "message": format!("Definition '{definition}' added to namespace '{namespace}' successfully")
  }))
}

pub fn delete_definition(app_state: &super::AppState, request: DeleteDefinitionRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;
  let definition = request.definition;

  let result = app_state.state_manager.update_current_module(|snapshot| {
    // Check if namespace exists
    let file_data = match snapshot.files.get_mut(&namespace) {
      Some(data) => data,
      None => {
        return Err(format!("Namespace '{namespace}' not found"));
      }
    };

    // Check if definition exists
    if !file_data.defs.contains_key(&definition) {
      return Err(format!("Definition '{definition}' not found in namespace '{namespace}'"));
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

pub fn overwrite_definition(app_state: &super::AppState, request: OverwriteDefinitionRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;
  let definition = request.definition;

  let code_cirru = match &request.code {
    serde_json::Value::String(s) => {
      // Handle string format code (backward compatibility)
      match cirru_parser::parse(s) {
        Ok(parsed) => {
          if parsed.is_empty() {
            return ResponseJson(serde_json::json!({
              "error": "Code cannot be empty"
            }));
          }
          parsed[0].clone()
        }
        Err(e) => {
          return ResponseJson(serde_json::json!({
            "error": format!("Failed to parse code: {e}")
          }));
        }
      }
    }
    code_json => {
      // Handle array format code (new format)
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

  let doc = "".to_string(); // OverwriteDefinitionRequest doesn't include doc field

  let result = app_state.state_manager.update_current_module(|snapshot| {
    // Check if namespace exists
    let file_data = match snapshot.files.get_mut(&namespace) {
      Some(data) => data,
      None => {
        return Err(format!("Namespace '{namespace}' not found"));
      }
    };

    // Check if definition exists
    if !file_data.defs.contains_key(&definition) {
      return Err(format!("Definition '{definition}' not found in namespace '{namespace}'"));
    }

    // Update definition
    let code_entry = CodeEntry { doc, code: code_cirru };
    file_data.defs.insert(definition.clone(), code_entry);
    Ok(())
  });

  match result {
    Ok(()) => ResponseJson(serde_json::json!({
      "message": format!("Definition '{definition}' updated in namespace '{namespace}' successfully")
    })),
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}

pub fn update_definition_at(app_state: &super::AppState, request: UpdateDefinitionAtRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;
  let definition = request.definition;

  let coord: Vec<usize> = match parse_coord_from_json(&request.coord) {
    Ok(coord_vec) => coord_vec,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Invalid coord parameter: {}", e)
      }));
    }
  };

  // Parse update mode (default to replace for backward compatibility)
  let mode = match request.mode.as_deref().unwrap_or("replace").parse::<UpdateMode>() {
    Ok(mode) => mode,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Invalid mode parameter: {}. Valid modes are: replace, after, before, delete, prepend, append", e)
      }));
    }
  };

  // Parse new value (optional for delete mode)
  let new_value_cirru = match &request.new_value {
    Some(serde_json::Value::String(s)) => {
      // Parse new value as Cirru
      match cirru_parser::parse(s) {
        Ok(parsed) => {
          if parsed.is_empty() {
            Some(Cirru::Leaf(s.clone().into()))
          } else {
            Some(parsed[0].clone())
          }
        }
        Err(_) => {
          // If parsing fails, treat as string literal
          Some(Cirru::Leaf(s.clone().into()))
        }
      }
    }
    Some(code_json) => {
      // Handle JSON format new_value
      match super::cirru_utils::json_to_cirru(code_json) {
        Ok(cirru) => Some(cirru),
        Err(e) => {
          return ResponseJson(serde_json::json!({
            "error": format!("Failed to convert new_value from JSON: {}", e)
          }));
        }
      }
    }
    None => {
      // No new value provided - only valid for delete mode
      if matches!(mode, UpdateMode::Delete) {
        None
      } else {
        return ResponseJson(serde_json::json!({
          "error": "new_value parameter is required for all modes except 'delete'"
        }));
      }
    }
  };

  // Parse match content (optional validation)
  let match_content: Option<Cirru> = match &request.match_content {
    Some(match_json) => match super::cirru_utils::json_to_cirru(match_json) {
      Ok(cirru) => Some(cirru),
      Err(e) => {
        return ResponseJson(serde_json::json!({
          "error": format!("Failed to convert match content from JSON: {}", e)
        }));
      }
    },
    None => None,
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
    if let Err(e) = update_definition_at_coord(&mut code, &coord, new_value_cirru.as_ref(), mode, match_content.as_ref()) {
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

  let coord: Vec<usize> = match parse_coord_from_json(&request.coord) {
    Ok(coord_vec) => coord_vec,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Invalid coord parameter: {}", e)
      }));
    }
  };

  let result = app_state.state_manager.with_current_module(|snapshot| {
    // Check if namespace exists
    let file_data = match snapshot.files.get(&namespace) {
      Some(data) => data,
      None => {
        return Err(format!("Namespace '{namespace}' not found"));
      }
    };

    // Check if definition exists
    let code_entry = match file_data.defs.get(&definition) {
      Some(entry) => entry,
      None => {
        return Err(format!("Definition '{definition}' not found in namespace '{namespace}'"));
      }
    };

    // Navigate to the target coordinate
    let target = match navigate_to_coord(&code_entry.code, &coord) {
      Ok(t) => t,
      Err(e) => {
        return Err(format!("Failed to navigate to coordinate {coord:?}: {e}"));
      }
    };

    Ok(super::cirru_utils::cirru_to_json(target))
  });

  match result {
    Ok(value) => ResponseJson(serde_json::json!({
      "namespace": namespace,
      "definition": definition,
      "coord": coord,
      "value": value
    })),
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}
