use super::AppState;
use super::tools::{ReadConfigsRequest, UpdateConfigsRequest};
use axum::response::Json as ResponseJson;
use serde_json::{Value, json};
use std::fs;

/// Read current project configuration
pub fn read_configs(app_state: &AppState, _request: ReadConfigsRequest) -> ResponseJson<Value> {
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
pub fn update_configs(app_state: &AppState, request: UpdateConfigsRequest) -> ResponseJson<Value> {
  println!("[CONFIG] Updating multiple configuration fields");

  let mut updates = Vec::new();

  // Check which fields to update
  if let Some(ref init_fn) = request.init_fn {
    updates.push(format!("init_fn: {init_fn}"));
  }
  if let Some(ref reload_fn) = request.reload_fn {
    updates.push(format!("reload_fn: {reload_fn}"));
  }
  if let Some(ref version) = request.version {
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
    if let Some(ref init_fn) = request.init_fn {
      configs.init_fn = init_fn.clone();
    }

    // Update reload_fn if provided
    if let Some(ref reload_fn) = request.reload_fn {
      configs.reload_fn = reload_fn.clone();
    }

    // Update version if provided
    if let Some(ref version) = request.version {
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
  super::cirru_utils::save_snapshot_to_file(&app_state.compact_cirru_path, &snapshot)?;

  Ok(())
}

// save_snapshot function moved to cirru_utils::save_snapshot_to_file to avoid duplication
