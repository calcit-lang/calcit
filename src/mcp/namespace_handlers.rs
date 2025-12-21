use super::tools::ListNamespacesRequest;
use crate::snapshot::Snapshot;
use axum::response::Json as ResponseJson;
use serde_json::Value;

/// Load snapshot data, including main file and all module files
/// This function is kept for backward compatibility, but new code should use state_manager
pub fn load_snapshot(app_state: &super::AppState) -> Result<Snapshot, String> {
  app_state.state_manager.with_current_module(|snapshot| snapshot.clone())
}

// NOTE: Namespace editing functions moved to CLI: `cr edit add-ns`, `cr edit delete-ns`, `cr edit update-imports`, `cr edit update-ns-doc`

pub fn list_namespaces(app_state: &super::AppState, request: ListNamespacesRequest) -> ResponseJson<Value> {
  match app_state.state_manager.with_current_module(|snapshot| {
    let mut namespaces: Vec<String> = snapshot.files.keys().cloned().collect();

    // If include_dependency_namespaces is true, add dependency namespaces
    if request.include_dependency_namespaces {
      // Get dependency namespaces from state manager
      if let Ok(dep_namespaces) = app_state.state_manager.get_dependency_namespaces() {
        for namespace in dep_namespaces {
          if !namespaces.contains(&namespace) {
            namespaces.push(namespace);
          }
        }
      }
    }

    namespaces.sort();
    namespaces
  }) {
    Ok(namespaces) => ResponseJson(serde_json::json!({
      "namespaces": namespaces,
      "tips": {
        "next_steps": [
          "Use 'list_namespace_definitions' with namespace='<name>' to see all definitions in a namespace",
          "Use 'read_namespace' with namespace='<name>' to see namespace details including imports",
          "Use 'cr edit add-ns' CLI command to create a new namespace"
        ],
        "info": "Namespaces are the primary organizational units in Calcit, similar to modules in other languages"
      }
    })),
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}
