use super::tools::{CreateModuleRequest, DeleteModuleRequest, GetCurrentModuleRequest, ListModulesRequest, SwitchModuleRequest};
use crate::snapshot::{self, Snapshot};
use axum::response::Json as ResponseJson;
use serde_json::Value;

/// Load current module name
fn load_current_module_name(compact_cirru_path: &str) -> Result<String, String> {
  let content = match std::fs::read_to_string(compact_cirru_path) {
    Ok(c) => c,
    Err(e) => return Err(format!("Failed to read compact.cirru: {e}")),
  };

  let edn_data = match cirru_edn::parse(&content) {
    Ok(d) => d,
    Err(e) => return Err(format!("Failed to parse compact.cirru as EDN: {e}")),
  };

  let snapshot: Snapshot = match snapshot::load_snapshot_data(&edn_data, compact_cirru_path) {
    Ok(s) => s,
    Err(e) => return Err(format!("Failed to load current module name: {e}")),
  };

  Ok(snapshot.package.clone())
}

/// Load snapshot data
fn load_snapshot(app_state: &super::AppState) -> Result<Snapshot, String> {
  // Use the load_snapshot function from namespace_handlers, which contains module loading logic
  super::namespace_handlers::load_snapshot(app_state)
}

pub fn get_current_module(app_state: &super::AppState, _request: GetCurrentModuleRequest) -> ResponseJson<Value> {
  match load_current_module_name(&app_state.compact_cirru_path) {
    Ok(module_name) => ResponseJson(serde_json::json!({
      "module": module_name
    })),
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}

pub fn list_modules(app_state: &super::AppState, _request: ListModulesRequest) -> ResponseJson<Value> {
  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  // Collect all modules: main package + modules in entries
  let mut modules: Vec<serde_json::Value> = vec![serde_json::json!({
    "name": snapshot.package,
    "type": "main_package",
    "init_fn": snapshot.configs.init_fn,
    "reload_fn": snapshot.configs.reload_fn,
    "version": snapshot.configs.version
  })];

  // Add other modules
  for (module_name, config) in &snapshot.entries {
    modules.push(serde_json::json!({
      "name": module_name,
      "type": "module",
      "init_fn": config.init_fn,
      "reload_fn": config.reload_fn,
      "version": config.version
    }));
  }

  ResponseJson(serde_json::json!({
    "modules": modules,
    "total_count": modules.len()
  }))
}

pub fn switch_module(app_state: &super::AppState, request: SwitchModuleRequest) -> ResponseJson<Value> {
  let module_name = request.module;

  // Load snapshot to verify if module exists
  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  // Check if module exists (simplified here to check if it's the current package name)
  if module_name != snapshot.package {
    return ResponseJson(serde_json::json!({
      "error": format!("Module '{}' not found", module_name)
    }));
  }

  // In actual implementation, this should update current module state
  // Currently only verifying module existence
  ResponseJson(serde_json::json!({
    "message": format!("Switched to module: {}", module_name),
    "current_module": module_name
  }))
}

pub fn create_module(app_state: &super::AppState, request: CreateModuleRequest) -> ResponseJson<Value> {
  let module_name = request.name;

  // Validate module name
  if module_name.is_empty() {
    return ResponseJson(serde_json::json!({
      "error": "Module name cannot be empty"
    }));
  }

  // Load current snapshot
  let mut snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  // Check if module already exists
  if snapshot.entries.contains_key(&module_name) {
    return ResponseJson(serde_json::json!({
      "error": format!("Module '{}' already exists", module_name)
    }));
  }

  // Create new module configuration
  let new_module_config = crate::snapshot::SnapshotConfigs {
    init_fn: format!("{module_name}.main/main!"),
    reload_fn: format!("{module_name}.main/reload!"),
    version: "0.0.0".to_string(),
    modules: vec![],
  };

  snapshot.entries.insert(module_name.clone(), new_module_config);

  // In actual implementation, this should save snapshot
  // Currently only simulating creation process
  ResponseJson(serde_json::json!({
    "message": format!("Created module: {}", module_name),
    "module": module_name
  }))
}

pub fn delete_module(app_state: &super::AppState, request: DeleteModuleRequest) -> ResponseJson<Value> {
  let module_name = request.module;

  // Load current snapshot
  let mut snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  // Check if module exists
  if !snapshot.entries.contains_key(&module_name) {
    return ResponseJson(serde_json::json!({
      "error": format!("Module '{}' not found", module_name)
    }));
  }

  // Prevent deletion of main module
  if module_name == snapshot.package {
    return ResponseJson(serde_json::json!({
      "error": "Cannot delete the main package module"
    }));
  }

  // Delete module
  snapshot.entries.remove(&module_name);

  // In actual implementation, this should save snapshot
  // Currently only simulating deletion process
  ResponseJson(serde_json::json!({
    "message": format!("Deleted module: {}", module_name)
  }))
}
