use super::state_manager::StateManager;
use super::tools::{
  ListDependencyDocsRequest, ReadDependencyDefinitionDocRequest, ReadDependencyModuleDocRequest,
};
use axum::response::Json as ResponseJson;
use serde::Serialize;
use serde_json::Value;

// Common error response structure
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
  pub error: String,
  pub context: Option<String>,
}

// Response structures
#[derive(Debug, Serialize)]
pub struct DependencyDocsListResponse {
  pub definition_docs: Vec<DefinitionDocInfo>,
  pub module_docs: Vec<String>, // List of relative file paths
}

#[derive(Debug, Serialize)]
pub struct DefinitionDocInfo {
  pub namespace: String,
  pub definition: String,
  pub doc: String,
}

#[derive(Debug, Serialize)]
pub struct DefinitionDocResponse {
  pub dependency_name: String,
  pub namespace: String,
  pub definition: String,
  pub doc: String,
}

#[derive(Debug, Serialize)]
pub struct ModuleDocResponse {
  pub dependency_name: String,
  pub doc_path: String,
  pub content: String,
}

/// List all available documentation from a dependency module
pub fn list_dependency_docs(
  state_manager: &StateManager,
  request: ListDependencyDocsRequest,
) -> ResponseJson<Value> {
  match state_manager.get_dependency_module_with_doc(&request.dependency_name) {
    Ok(module_with_doc) => {
      let mut definition_docs = Vec::new();

      // Collect definition docs from all files
      for (file_name, file_snapshot) in &module_with_doc.snapshot.files {
        // Skip meta files
        if file_name.ends_with(".$meta") {
          continue;
        }

        for (def_name, code_entry) in &file_snapshot.defs {
          if !code_entry.doc.is_empty() {
            definition_docs.push(DefinitionDocInfo {
              namespace: file_name.clone(),
              definition: def_name.clone(),
              doc: code_entry.doc.clone(),
            });
          }
        }
      }

      // Get list of module doc file paths
      let module_docs: Vec<String> = module_with_doc.docs.keys().cloned().collect();

      let response = DependencyDocsListResponse {
        definition_docs,
        module_docs,
      };

      ResponseJson(serde_json::to_value(response).unwrap_or_else(|e| {
        serde_json::json!({
          "error": format!("Failed to serialize response: {}", e)
        })
      }))
    }
    Err(e) => ResponseJson(serde_json::json!({
      "error": format!("Failed to load dependency module '{}': {}", request.dependency_name, e)
    })),
  }
}

/// Read the documentation string of a specific definition from a dependency module
pub fn read_dependency_definition_doc(
  state_manager: &StateManager,
  request: ReadDependencyDefinitionDocRequest,
) -> ResponseJson<Value> {
  match state_manager.get_dependency_module_with_doc(&request.dependency_name) {
    Ok(module_with_doc) => {
      if let Some(file_snapshot) = module_with_doc.snapshot.files.get(&request.namespace) {
        if let Some(code_entry) = file_snapshot.defs.get(&request.definition) {
          let response = DefinitionDocResponse {
            dependency_name: request.dependency_name,
            namespace: request.namespace,
            definition: request.definition,
            doc: code_entry.doc.clone(),
          };

          ResponseJson(serde_json::to_value(response).unwrap_or_else(|e| {
            serde_json::json!({
              "error": format!("Failed to serialize response: {}", e)
            })
          }))
        } else {
          ResponseJson(serde_json::json!({
            "error": format!("Definition '{}' not found in namespace '{}'", request.definition, request.namespace)
          }))
        }
      } else {
        ResponseJson(serde_json::json!({
          "error": format!("Namespace '{}' not found in dependency '{}'", request.namespace, request.dependency_name)
        }))
      }
    }
    Err(e) => ResponseJson(serde_json::json!({
      "error": format!("Failed to load dependency module '{}': {}", request.dependency_name, e)
    })),
  }
}

/// Read a module-level document from a dependency module
pub fn read_dependency_module_doc(
  state_manager: &StateManager,
  request: ReadDependencyModuleDocRequest,
) -> ResponseJson<Value> {
  match state_manager.get_dependency_module_with_doc(&request.dependency_name) {
    Ok(module_with_doc) => {
      if let Some(content) = module_with_doc.docs.get(&request.doc_path) {
        let response = ModuleDocResponse {
          dependency_name: request.dependency_name,
          doc_path: request.doc_path,
          content: content.clone(),
        };

        ResponseJson(serde_json::to_value(response).unwrap_or_else(|e| {
          serde_json::json!({
            "error": format!("Failed to serialize response: {}", e)
          })
        }))
      } else {
        ResponseJson(serde_json::json!({
          "error": format!("Documentation file '{}' not found in dependency '{}'", request.doc_path, request.dependency_name)
        }))
      }
    }
    Err(e) => ResponseJson(serde_json::json!({
      "error": format!("Failed to load dependency module '{}': {}", request.dependency_name, e)
    })),
  }
}
