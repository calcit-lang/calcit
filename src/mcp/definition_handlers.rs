use super::tools::McpRequest;
use crate::mcp::definition_update::{UpdateMode, update_definition_at_coord};
use crate::mcp::definition_utils::{navigate_to_coord, parse_coord_from_json};
use crate::snapshot::{CodeEntry, Snapshot};
use axum::response::Json as ResponseJson;
use cirru_parser::Cirru;
use serde_json::Value;

/// Save snapshot data
// save_snapshot function moved to cirru_utils::save_snapshot_to_file to avoid duplication
fn save_snapshot(app_state: &super::AppState, snapshot: &Snapshot) -> Result<(), ResponseJson<Value>> {
  super::cirru_utils::save_snapshot_to_file(&app_state.compact_cirru_path, snapshot).map_err(|e| {
    ResponseJson(serde_json::json!({
      "error": e
    }))
  })
}

pub fn add_definition(app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "namespace parameter is missing or not a string"
      }));
    }
  };

  let definition = match req.parameters.get("definition") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "definition parameter is missing or not a string"
      }));
    }
  };

  let code_cirru = match req.parameters.get("code") {
    Some(serde_json::Value::String(s)) => {
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
    Some(code_json) => {
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
    None => {
      return ResponseJson(serde_json::json!({
        "error": "code parameter is missing"
      }));
    }
  };

  let doc = req.parameters.get("doc").and_then(|v| v.as_str()).unwrap_or("").to_string();

  let mut snapshot = match super::namespace_handlers::load_snapshot(app_state) {
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

  // Check if definition already exists
  if file_data.defs.contains_key(&definition) {
    return ResponseJson(serde_json::json!({
      "error": format!("Definition '{definition}' already exists in namespace '{namespace}'")
    }));
  }

  // code_cirru has been processed above

  // Add new definition
  let code_entry = CodeEntry { doc, code: code_cirru };
  file_data.defs.insert(definition.clone(), code_entry);

  // Save snapshot
  if let Err(e) = save_snapshot(app_state, &snapshot) {
    return e;
  }

  ResponseJson(serde_json::json!({
    "message": format!("Definition '{definition}' added to namespace '{namespace}' successfully")
  }))
}

pub fn delete_definition(app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "namespace parameter is missing or not a string"
      }));
    }
  };

  let definition = match req.parameters.get("definition") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "definition parameter is missing or not a string"
      }));
    }
  };

  let mut snapshot = match super::namespace_handlers::load_snapshot(app_state) {
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

  // Check if definition exists
  if !file_data.defs.contains_key(&definition) {
    return ResponseJson(serde_json::json!({
      "error": format!("Definition '{definition}' not found in namespace '{namespace}'")
    }));
  }

  // Delete definition
  file_data.defs.remove(&definition);

  // Save snapshot
  if let Err(e) = save_snapshot(app_state, &snapshot) {
    return e;
  }

  ResponseJson(serde_json::json!({
    "message": format!("Definition '{definition}' deleted from namespace '{namespace}' successfully")
  }))
}

pub fn overwrite_definition(app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "namespace parameter is missing or not a string"
      }));
    }
  };

  let definition = match req.parameters.get("definition") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "definition parameter is missing or not a string"
      }));
    }
  };

  let code_cirru = match req.parameters.get("code") {
    Some(serde_json::Value::String(s)) => {
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
    Some(code_json) => {
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
    None => {
      return ResponseJson(serde_json::json!({
        "error": "code parameter is missing"
      }));
    }
  };

  let doc = req.parameters.get("doc").and_then(|v| v.as_str()).unwrap_or("").to_string();

  let mut snapshot = match super::namespace_handlers::load_snapshot(app_state) {
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

  // Check if definition exists
  if !file_data.defs.contains_key(&definition) {
    return ResponseJson(serde_json::json!({
      "error": format!("Definition '{definition}' not found in namespace '{namespace}'")
    }));
  }

  // code_cirru has been processed above

  // Update definition
  let code_entry = CodeEntry { doc, code: code_cirru };
  file_data.defs.insert(definition.clone(), code_entry);

  // Save snapshot
  if let Err(e) = save_snapshot(app_state, &snapshot) {
    return e;
  }

  ResponseJson(serde_json::json!({
    "message": format!("Definition '{definition}' updated in namespace '{namespace}' successfully")
  }))
}

pub fn update_definition_at(app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "namespace parameter is missing or not a string"
      }));
    }
  };

  let definition = match req.parameters.get("definition") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "definition parameter is missing or not a string"
      }));
    }
  };

  let coord: Vec<usize> = match req.parameters.get("coord") {
    Some(coord_value) => match parse_coord_from_json(coord_value) {
      Ok(coord_vec) => coord_vec,
      Err(e) => {
        return ResponseJson(serde_json::json!({
          "error": format!("Invalid coord parameter: {}", e)
        }));
      }
    },
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "coord parameter is missing"
      }));
    }
  };

  // Parse update mode (default to replace for backward compatibility)
  let mode = match req.parameters.get("mode") {
    Some(serde_json::Value::String(mode_str)) => match mode_str.parse::<UpdateMode>() {
      Ok(mode) => mode,
      Err(e) => {
        return ResponseJson(serde_json::json!({
          "error": e
        }));
      }
    },
    _ => UpdateMode::Replace,
  };

  // Parse new value (optional for delete mode)
  let new_value_cirru = match req.parameters.get("new_value") {
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
    Some(new_value_json) => {
      // Handle JSON format new_value
      match super::cirru_utils::json_to_cirru(new_value_json) {
        Ok(cirru) => Some(cirru),
        Err(e) => {
          return ResponseJson(serde_json::json!({
            "error": format!("Failed to convert new_value from JSON: {}", e)
          }));
        }
      }
    }
    None => {
      // Only delete mode doesn't require new_value
      match mode {
        UpdateMode::Delete => None,
        _ => {
          return ResponseJson(serde_json::json!({
            "error": "new_value parameter is required for this mode"
          }));
        }
      }
    }
  };

  // Parse match content (optional validation)
  let match_content = match req.parameters.get("match") {
    Some(match_value) => match super::cirru_utils::json_to_cirru(match_value) {
      Ok(cirru) => Some(cirru),
      Err(e) => {
        return ResponseJson(serde_json::json!({
          "error": format!("Failed to convert match from JSON: {}", e)
        }));
      }
    },
    None => None,
  };

  let mut snapshot = match super::namespace_handlers::load_snapshot(app_state) {
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
        "error": format!("Namespace '{}' not found", namespace)
      }));
    }
  };

  // Check if definition exists
  let code_entry = match file_data.defs.get_mut(&definition) {
    Some(entry) => entry,
    None => {
      return ResponseJson(serde_json::json!({
        "error": format!("Definition '{}' not found in namespace '{}'", definition, namespace)
      }));
    }
  };

  // Clone the code for the new update logic
  let mut code = code_entry.code.clone();

  // Perform the update using the new logic
  if let Err(e) = update_definition_at_coord(&mut code, &coord, new_value_cirru.as_ref(), mode, match_content.as_ref()) {
    return ResponseJson(serde_json::json!({
      "error": format!("Failed to update: {}", e)
    }));
  }

  // Update the code entry with the modified code
  code_entry.code = code;

  // Save snapshot
  if let Err(e) = save_snapshot(app_state, &snapshot) {
    return e;
  }

  ResponseJson(serde_json::json!({
    "message": format!("Definition '{}' updated at coordinate {:?} in namespace '{}' successfully", definition, coord, namespace)
  }))
}

pub fn read_definition_at(app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "namespace parameter is missing or not a string"
      }));
    }
  };

  let definition = match req.parameters.get("definition") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "definition parameter is missing or not a string"
      }));
    }
  };

  let coord = match req.parameters.get("coord") {
    Some(serde_json::Value::Array(arr)) => {
      let mut coord_vec = Vec::new();
      for item in arr {
        match item.as_u64() {
          Some(i) => coord_vec.push(i as usize),
          None => {
            return ResponseJson(serde_json::json!({
              "error": "coord array must contain only integers"
            }));
          }
        }
      }
      coord_vec
    }
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "coord parameter is missing or not an array"
      }));
    }
  };

  let snapshot = match super::namespace_handlers::load_snapshot(app_state) {
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
        "error": format!("Namespace '{}' not found", namespace)
      }));
    }
  };

  // Check if definition exists
  let code_entry = match file_data.defs.get(&definition) {
    Some(entry) => entry,
    None => {
      return ResponseJson(serde_json::json!({
        "error": format!("Definition '{}' not found in namespace '{}'", definition, namespace)
      }));
    }
  };

  // Navigate to the target coordinate
  let target = match navigate_to_coord(&code_entry.code, &coord) {
    Ok(t) => t,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Failed to navigate to coordinate {:?}: {}", coord, e)
      }));
    }
  };

  ResponseJson(serde_json::json!({
    "namespace": namespace,
    "definition": definition,
    "coord": coord,
    "value": super::cirru_utils::cirru_to_json(target)
  }))
}
