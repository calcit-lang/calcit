use super::AppState;
use super::tools::UpdateConfigsRequest;
use axum::response::Json as ResponseJson;
use serde_json::{Value, json};

// NOTE: read_configs function has been moved to CLI: `cr query configs`

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
  app_state.state_manager.update_current_module(|snapshot| {
    // Apply the update
    update_fn(&mut snapshot.configs);
    Ok(())
  })
}

// save_snapshot function moved to cirru_utils::save_snapshot_to_file to avoid duplication
