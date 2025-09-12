use super::tools::{
  AddDefinitionRequest, DeleteDefinitionRequest, OverwriteDefinitionRequest, ReadDefinitionAtRequest, UpdateDefinitionAtRequest, UpdateDefinitionAtWithLeafRequest,
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
  // Note: This function returns ResponseJson<Value> for compatibility with existing handlers
  // The actual MCP error handling with isError flag is handled at the protocol level
  let namespace = request.namespace;
  let definition = request.definition;

  let code_cirru = match &request.code {
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

  // Parse update mode
  let mode = match request.mode.parse::<UpdateMode>() {
    Ok(mode) => mode,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Invalid mode parameter: {}. Valid modes are: replace, after, before, delete, prepend, append", e)
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
            "error": format!("Failed to convert new_value from JSON: {}", e)
          }));
        }
      }
    }
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "new_value must be a JSON array. Examples: [\"my-value\"] for single values, [\"fn\", [\"x\"], [\"*\", \"x\", \"x\"]] for complex expressions"
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
  let match_content: Option<Cirru> = match super::cirru_utils::json_to_cirru(&request.match_content) {
    Ok(cirru) => Some(cirru),
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Failed to convert match content from JSON: {}", e)
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

    let expr_length = match target {
      Cirru::Leaf(_) => None,
      Cirru::List(xs) => Some(xs.len()),
    };

    Ok((target.clone(), expr_length))
  });

  match result {
    Ok(Ok((value, expr_length))) => ResponseJson(serde_json::json!({
      "namespace": namespace,
      "definition": definition,
      "coord": coord,
      "value": value,
      "expr_length": expr_length
    })),
    Ok(Err(e)) => ResponseJson(serde_json::json!({
      "error": e
    })),
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}

pub fn update_definition_at_with_leaf(app_state: &super::AppState, request: UpdateDefinitionAtWithLeafRequest) -> ResponseJson<Value> {
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

  // Parse update mode
  let mode = match request.mode.parse::<UpdateMode>() {
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
  let match_content_cirru = request.match_content.map(|s| Cirru::Leaf(s.into()));

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
    if let Err(e) = update_definition_at_coord(&mut code, &coord, Some(&new_value_cirru), mode, match_content_cirru.as_ref()) {
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
