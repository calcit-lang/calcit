use super::tools::{GetCurrentModuleRequest, ListModulesRequest};
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
  let result = app_state.state_manager.with_current_module(|snapshot| {
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

    serde_json::json!({
      "modules": modules,
      "total_count": modules.len()
    })
  });

  match result {
    Ok(response) => ResponseJson(response),
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}

// NOTE: Module editing functions moved to CLI: `cr edit add-module`, `cr edit delete-module`
