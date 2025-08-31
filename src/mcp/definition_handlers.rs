use super::tools::McpRequest;
use crate::snapshot::{self, CodeEntry, Snapshot};
use axum::response::Json as ResponseJson;
use serde_json::Value;

/// Load snapshot data
fn load_snapshot(app_state: &super::AppState) -> Result<Snapshot, String> {
  let content = match std::fs::read_to_string(&app_state.compact_cirru_path) {
    Ok(c) => c,
    Err(e) => return Err(format!("Failed to read compact.cirru: {e}")),
  };

  let edn_data = match cirru_edn::parse(&content) {
    Ok(d) => d,
    Err(e) => return Err(format!("Failed to parse compact.cirru as EDN: {e}")),
  };

  match snapshot::load_snapshot_data(&edn_data, &app_state.compact_cirru_path) {
    Ok(snapshot) => Ok(snapshot),
    Err(e) => Err(format!("Failed to load snapshot: {e}")),
  }
}

/// Save snapshot data
fn save_snapshot(app_state: &super::AppState, snapshot: &Snapshot) -> Result<(), ResponseJson<Value>> {
  let compact_cirru_path = &app_state.compact_cirru_path;

  // Build root-level Edn mapping
  let mut edn_map = cirru_edn::EdnMapView::default();

  // Build package
  edn_map.insert_key("package", cirru_edn::Edn::Str(snapshot.package.as_str().into()));

  // Build configs
  let mut configs_map = cirru_edn::EdnMapView::default();
  configs_map.insert_key("init-fn", cirru_edn::Edn::Str(snapshot.configs.init_fn.as_str().into()));
  configs_map.insert_key("reload-fn", cirru_edn::Edn::Str(snapshot.configs.reload_fn.as_str().into()));
  configs_map.insert_key("version", cirru_edn::Edn::Str(snapshot.configs.version.as_str().into()));
  configs_map.insert_key(
    "modules",
    cirru_edn::Edn::from(
      snapshot
        .configs
        .modules
        .iter()
        .map(|s| cirru_edn::Edn::Str(s.as_str().into()))
        .collect::<Vec<_>>(),
    ),
  );
  edn_map.insert_key("configs", configs_map.into());

  // Build entries
  let mut entries_map = cirru_edn::EdnMapView::default();
  for (k, v) in &snapshot.entries {
    let mut entry_map = cirru_edn::EdnMapView::default();
    entry_map.insert_key("init-fn", cirru_edn::Edn::Str(v.init_fn.as_str().into()));
    entry_map.insert_key("reload-fn", cirru_edn::Edn::Str(v.reload_fn.as_str().into()));
    entry_map.insert_key("version", cirru_edn::Edn::Str(v.version.as_str().into()));
    entry_map.insert_key(
      "modules",
      cirru_edn::Edn::from(v.modules.iter().map(|s| cirru_edn::Edn::Str(s.as_str().into())).collect::<Vec<_>>()),
    );
    entries_map.insert_key(k.as_str(), entry_map.into());
  }
  edn_map.insert_key("entries", entries_map.into());

  // Build files
  let mut files_map = cirru_edn::EdnMapView::default();
  for (k, v) in &snapshot.files {
    files_map.insert_key(k.as_str(), cirru_edn::Edn::from(v));
  }
  edn_map.insert_key("files", files_map.into());

  let edn_data = cirru_edn::Edn::from(edn_map);

  // Format Edn to Cirru string
  let content = match cirru_edn::format(&edn_data, false) {
    Ok(c) => c,
    Err(e) => {
      return Err(ResponseJson(serde_json::json!({
        "error": format!("Failed to format snapshot as Cirru: {e}")
      })));
    }
  };

  // Write to file
  match std::fs::write(compact_cirru_path, content) {
    Ok(_) => Ok(()),
    Err(e) => Err(ResponseJson(serde_json::json!({
      "error": format!("Failed to write compact.cirru: {e}")
    }))),
  }
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

pub fn update_definition(app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
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
