use super::tools::{
  ListDependencyDocsRequest, ListModuleDocsRequest, ReadDependencyDefinitionDocRequest, ReadDependencyModuleDocRequest,
};
use crate::snapshot::{DocumentEntry, Snapshot};
use axum::response::Json as ResponseJson;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

// Common error response structure
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
  pub error: String,
  pub context: Option<String>,
}

// parse_request function removed as it's no longer needed with typed requests

// Helper function to create success response
fn success_response<T: Serialize>(data: T) -> ResponseJson<Value> {
  ResponseJson(serde_json::to_value(data).unwrap_or_else(|_| {
    serde_json::json!({
        "error": "Failed to serialize response"
    })
  }))
}

// Helper function to create error response
fn error_response(error: ErrorResponse) -> ResponseJson<Value> {
  ResponseJson(serde_json::to_value(error).unwrap_or_else(|_| {
    serde_json::json!({
        "error": "Internal serialization error"
    })
  }))
}

// ===== DEFINITION DOCUMENT HANDLERS =====

#[derive(Debug, Deserialize)]
pub struct ReadDefinitionDocRequest {
  pub namespace: String,
  pub symbol: String,
}

#[derive(Debug, Serialize)]
pub struct DefinitionDocResponse {
  pub namespace: String,
  pub symbol: String,
  pub doc: String,
}

/// Read definition documentation for a specific symbol
pub fn read_definition_doc(app_state: &super::AppState, request: ReadDefinitionDocRequest) -> ResponseJson<Value> {
  let result = app_state.state_manager.with_current_module(|snapshot| {
    // Find the definition in the snapshot
    if let Some(file_snapshot) = snapshot.files.get(&request.namespace) {
      if let Some(definition) = file_snapshot.defs.get(&request.symbol) {
        let response = DefinitionDocResponse {
          namespace: request.namespace,
          symbol: request.symbol,
          doc: definition.doc.clone(),
        };
        return success_response(response);
      }
    }

    error_response(ErrorResponse {
      error: format!("Definition {}/{} not found", request.namespace, request.symbol),
      context: Some("Check if the namespace and symbol exist in the current snapshot".to_string()),
    })
  });

  match result {
    Ok(response) => response,
    Err(e) => error_response(ErrorResponse { error: e, context: None }),
  }
}

#[derive(Debug, Deserialize)]
pub struct UpdateDefinitionDocRequest {
  pub namespace: String,
  pub symbol: String,
  pub doc: String,
}

#[derive(Debug, Serialize)]
pub struct UpdateDefinitionDocResponse {
  pub namespace: String,
  pub symbol: String,
  pub doc: String,
  pub status: String,
}

/// Update definition documentation for a specific symbol
pub fn update_definition_doc(app_state: &super::AppState, request: UpdateDefinitionDocRequest) -> ResponseJson<Value> {
  let result = app_state.state_manager.update_current_module(|snapshot| {
    // Update the definition documentation
    if let Some(file_snapshot) = snapshot.files.get_mut(&request.namespace) {
      if let Some(definition) = file_snapshot.defs.get_mut(&request.symbol) {
        definition.doc = request.doc.clone();
        return Ok(());
      }
    }

    Err(format!("Definition {}/{} not found", request.namespace, request.symbol))
  });

  match result {
    Ok(()) => {
      let response = UpdateDefinitionDocResponse {
        namespace: request.namespace,
        symbol: request.symbol,
        doc: request.doc,
        status: "updated".to_string(),
      };
      success_response(response)
    }
    Err(e) => error_response(ErrorResponse {
      error: e,
      context: Some("Ensure the namespace and symbol exist before attempting to update documentation".to_string()),
    }),
  }
}

// ===== MODULE DOCUMENT HANDLERS =====

// ===== DEPENDENCY DOCUMENT HANDLERS =====

#[derive(Debug, Serialize)]
pub struct DependencyDocsListResponse {
  pub dependencies: Vec<String>,
}

/// List all dependency modules
pub fn list_dependency_docs(app_state: &super::AppState, _request: ListDependencyDocsRequest) -> ResponseJson<Value> {
  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return error_response(ErrorResponse { error: e, context: None });
    }
  };

  // Get all dependency modules from configs
  let dependencies: Vec<String> = snapshot.configs.modules.clone();

  let response = DependencyDocsListResponse { dependencies };
  success_response(response)
}

#[derive(Debug, Serialize)]
pub struct DependencyDefinitionDocResponse {
  pub dependency: String,
  pub namespace: String,
  pub symbol: String,
  pub doc: String,
  pub note: String,
}

/// Read definition documentation from a dependency
pub fn read_dependency_definition_doc(
  _app_state: &super::AppState,
  request: ReadDependencyDefinitionDocRequest,
) -> ResponseJson<Value> {
  let response = DependencyDefinitionDocResponse {
    dependency: request.dependency_name,
    namespace: request.namespace,
    symbol: request.definition,
    doc: "Dependency documentation access not yet implemented".to_string(),
    note: "This is a read-only operation for external dependencies".to_string(),
  };

  success_response(response)
}

#[derive(Debug, Serialize)]
pub struct DependencyModuleDocResponse {
  pub dependency: String,
  pub title: String,
  pub content: String,
  pub note: String,
}

/// Read module documentation from a dependency
pub fn read_dependency_module_doc(_app_state: &super::AppState, request: ReadDependencyModuleDocRequest) -> ResponseJson<Value> {
  let response = DependencyModuleDocResponse {
    dependency: request.dependency_name,
    title: request.title,
    content: "Dependency module documentation access not yet implemented".to_string(),
    note: "This is a read-only operation for external dependencies".to_string(),
  };

  success_response(response)
}

/// Load snapshot data
fn load_snapshot(app_state: &super::AppState) -> Result<Snapshot, String> {
  super::namespace_handlers::load_snapshot(app_state)
}

/// Save snapshot data
fn save_snapshot(app_state: &super::AppState, snapshot: &Snapshot) -> Result<(), ResponseJson<Value>> {
  super::cirru_utils::save_snapshot_to_file(&app_state.compact_cirru_path, snapshot).map_err(|e| {
    ResponseJson(serde_json::json!({
      "error": e
    }))
  })
}

#[derive(Debug, Serialize)]
pub struct ModuleDocsListResponse {
  pub documents: Vec<String>,
}

/// List all module document titles in the project
pub fn list_module_docs(app_state: &super::AppState, _request: ListModuleDocsRequest) -> ResponseJson<Value> {
  let result = app_state.state_manager.with_current_module(|snapshot| {
    let documents: Vec<String> = match &snapshot.docs {
      Some(docs) => docs.keys().cloned().collect(),
      None => vec![],
    };

    let response = ModuleDocsListResponse { documents };
    success_response(response)
  });

  match result {
    Ok(response) => response,
    Err(e) => error_response(ErrorResponse { error: e, context: None }),
  }
}

#[derive(Debug, Deserialize)]
pub struct ReadModuleDocRequest {
  pub title: String,
}

#[derive(Debug, Serialize)]
pub struct ModuleDocResponse {
  pub title: String,
  pub content: String,
}

/// Read module documentation by title
pub fn read_module_doc(app_state: &super::AppState, request: ReadModuleDocRequest) -> ResponseJson<Value> {
  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return error_response(ErrorResponse { error: e, context: None });
    }
  };

  match &snapshot.docs {
    Some(docs) => match docs.get(&request.title) {
      Some(doc_entry) => {
        let response = ModuleDocResponse {
          title: doc_entry.title.clone(),
          content: doc_entry.content.clone(),
        };
        success_response(response)
      }
      None => error_response(ErrorResponse {
        error: format!("Document '{}' not found", request.title),
        context: Some("Check if the document title exists in the current snapshot".to_string()),
      }),
    },
    None => error_response(ErrorResponse {
      error: format!("Document '{}' not found", request.title),
      context: Some("No documents found in the current snapshot".to_string()),
    }),
  }
}

#[derive(Debug, Deserialize)]
pub struct UpdateModuleDocRequest {
  pub title: String,
  pub content: String,
}

#[derive(Debug, Serialize)]
pub struct UpdateModuleDocResponse {
  pub title: String,
  pub content: String,
  pub status: String,
}

/// Create or update a document
pub fn update_module_doc(app_state: &super::AppState, request: UpdateModuleDocRequest) -> ResponseJson<Value> {
  let mut snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return error_response(ErrorResponse { error: e, context: None });
    }
  };

  // Initialize docs if it doesn't exist
  if snapshot.docs.is_none() {
    snapshot.docs = Some(HashMap::new());
  }

  let docs = snapshot.docs.as_mut().unwrap();
  let doc_entry = DocumentEntry {
    title: request.title.clone(),
    content: request.content.clone(),
  };

  docs.insert(request.title.clone(), doc_entry);

  // Save snapshot
  if let Err(e) = save_snapshot(app_state, &snapshot) {
    return e;
  }

  let response = UpdateModuleDocResponse {
    title: request.title,
    content: request.content,
    status: "updated".to_string(),
  };
  success_response(response)
}

#[derive(Debug, Deserialize)]
pub struct RenameModuleDocRequest {
  pub old_title: String,
  pub new_title: String,
}

#[derive(Debug, Serialize)]
pub struct RenameModuleDocResponse {
  pub old_title: String,
  pub new_title: String,
  pub status: String,
}

/// Rename a module document
pub fn rename_module_doc(app_state: &super::AppState, request: RenameModuleDocRequest) -> ResponseJson<Value> {
  let mut snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return error_response(ErrorResponse { error: e, context: None });
    }
  };

  match &mut snapshot.docs {
    Some(docs) => {
      // Check if old document exists
      if !docs.contains_key(&request.old_title) {
        return error_response(ErrorResponse {
          error: format!("Document '{}' not found", request.old_title),
          context: Some("Check if the document title exists before attempting to rename".to_string()),
        });
      }

      // Check if new title already exists
      if docs.contains_key(&request.new_title) {
        return error_response(ErrorResponse {
          error: format!("Document '{}' already exists", request.new_title),
          context: Some("Choose a different title that doesn't already exist".to_string()),
        });
      }

      // Move the document
      if let Some(mut doc_entry) = docs.remove(&request.old_title) {
        doc_entry.title = request.new_title.clone();
        docs.insert(request.new_title.clone(), doc_entry);
      }
    }
    None => {
      return error_response(ErrorResponse {
        error: format!("Document '{}' not found", request.old_title),
        context: Some("No documents found in the current snapshot".to_string()),
      });
    }
  }

  // Save snapshot
  if let Err(e) = save_snapshot(app_state, &snapshot) {
    return e;
  }

  let response = RenameModuleDocResponse {
    old_title: request.old_title,
    new_title: request.new_title,
    status: "renamed".to_string(),
  };
  success_response(response)
}

#[derive(Debug, Deserialize)]
pub struct DeleteModuleDocRequest {
  pub title: String,
}

#[derive(Debug, Serialize)]
pub struct DeleteModuleDocResponse {
  pub title: String,
  pub status: String,
}

/// Delete a module document
pub fn delete_module_doc(app_state: &super::AppState, request: DeleteModuleDocRequest) -> ResponseJson<Value> {
  let mut snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return error_response(ErrorResponse { error: e, context: None });
    }
  };

  match &mut snapshot.docs {
    Some(docs) => {
      if docs.remove(&request.title).is_none() {
        return error_response(ErrorResponse {
          error: format!("Document '{}' not found", request.title),
          context: Some("Check if the document title exists before attempting to delete".to_string()),
        });
      }
    }
    None => {
      return error_response(ErrorResponse {
        error: format!("Document '{}' not found", request.title),
        context: Some("No documents found in the current snapshot".to_string()),
      });
    }
  }

  // Save snapshot
  if let Err(e) = save_snapshot(app_state, &snapshot) {
    return e;
  }

  let response = DeleteModuleDocResponse {
    title: request.title,
    status: "deleted".to_string(),
  };
  success_response(response)
}
