use super::AppState;
use super::tools::McpRequest;
use axum::response::Json as ResponseJson;
use serde_json::{Value, json};
use std::fs;

/// Read current project configuration
pub fn read_configs(app_state: &AppState, _req: McpRequest) -> ResponseJson<Value> {
  println!("[CONFIG] Reading project configuration from: {}", app_state.compact_cirru_path);

  // Read and parse the compact.cirru file
  match fs::read_to_string(&app_state.compact_cirru_path) {
    Ok(content) => match cirru_edn::parse(&content) {
      Ok(data) => match crate::snapshot::load_snapshot_data(&data, &app_state.compact_cirru_path) {
        Ok(snapshot) => {
          let configs = json!({
            "init_fn": snapshot.configs.init_fn,
            "reload_fn": snapshot.configs.reload_fn,
            "version": snapshot.configs.version,
            "modules": snapshot.configs.modules,
            "package": snapshot.package
          });

          println!("[CONFIG] Successfully read configuration: {configs:?}");
          ResponseJson(json!({
            "success": true,
            "data": configs
          }))
        }
        Err(e) => {
          println!("[CONFIG ERROR] Failed to load snapshot: {e}");
          ResponseJson(json!({
            "success": false,
            "error": format!("Failed to load snapshot: {}", e)
          }))
        }
      },
      Err(e) => {
        println!("[CONFIG ERROR] Failed to parse EDN: {e:?}");
        ResponseJson(json!({
          "success": false,
          "error": format!("Failed to parse EDN: {:?}", e)
        }))
      }
    },
    Err(e) => {
      println!("[CONFIG ERROR] Failed to read file: {e}");
      ResponseJson(json!({
        "success": false,
        "error": format!("Failed to read file: {}", e)
      }))
    }
  }
}

/// Update multiple configuration fields at once
pub fn update_configs(app_state: &AppState, req: McpRequest) -> ResponseJson<Value> {
  println!("[CONFIG] Updating multiple configuration fields");

  let mut updates = Vec::new();

  // Check which fields to update
  if let Some(Value::String(init_fn)) = req.parameters.get("init_fn") {
    updates.push(format!("init_fn: {init_fn}"));
  }
  if let Some(Value::String(reload_fn)) = req.parameters.get("reload_fn") {
    updates.push(format!("reload_fn: {reload_fn}"));
  }
  if let Some(Value::String(version)) = req.parameters.get("version") {
    updates.push(format!("version: {version}"));
  }

  if updates.is_empty() {
    return ResponseJson(json!({
      "success": false,
      "error": "No valid configuration fields provided for update"
    }));
  }

  match update_config_field(app_state, |configs| {
    // Update init_fn if provided
    if let Some(Value::String(init_fn)) = req.parameters.get("init_fn") {
      configs.init_fn = init_fn.clone();
    }

    // Update reload_fn if provided
    if let Some(Value::String(reload_fn)) = req.parameters.get("reload_fn") {
      configs.reload_fn = reload_fn.clone();
    }

    // Update version if provided
    if let Some(Value::String(version)) = req.parameters.get("version") {
      configs.version = version.clone();
    }
  }) {
    Ok(_) => {
      println!("[CONFIG] Successfully updated configuration fields");
      ResponseJson(json!({
        "success": true,
        "message": format!("Updated: {}", updates.join(", "))
      }))
    }
    Err(e) => {
      println!("[CONFIG ERROR] Failed to update configuration: {e}");
      ResponseJson(json!({
        "success": false,
        "error": e
      }))
    }
  }
}

/// Helper function to update configuration fields
fn update_config_field<F>(app_state: &AppState, update_fn: F) -> Result<(), String>
where
  F: FnOnce(&mut crate::snapshot::SnapshotConfigs),
{
  // Read current file
  let content = fs::read_to_string(&app_state.compact_cirru_path).map_err(|e| format!("Failed to read file: {e}"))?;

  // Parse EDN
  let data = cirru_edn::parse(&content).map_err(|e| format!("Failed to parse EDN: {e:?}"))?;

  // Load snapshot
  let mut snapshot =
    crate::snapshot::load_snapshot_data(&data, &app_state.compact_cirru_path).map_err(|e| format!("Failed to load snapshot: {e}"))?;

  // Apply the update
  update_fn(&mut snapshot.configs);

  // Save snapshot using existing save logic
  save_snapshot(app_state, &snapshot).map_err(|e| format!("Failed to save snapshot: {e:?}"))?;

  Ok(())
}

/// Save snapshot data (reused from other handlers)
fn save_snapshot(app_state: &AppState, snapshot: &crate::snapshot::Snapshot) -> Result<(), String> {
  let compact_cirru_path = &app_state.compact_cirru_path;

  // Build root level Edn mapping
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

  // Format Edn as Cirru string
  let content = cirru_edn::format(&edn_data, false).map_err(|e| format!("Failed to format snapshot as Cirru: {e}"))?;

  // Write to file
  std::fs::write(compact_cirru_path, content).map_err(|e| format!("Failed to write compact.cirru: {e}"))?;

  Ok(())
}
