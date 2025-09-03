use super::tools::McpRequest;
use crate::snapshot::{DocumentEntry, Snapshot};
use axum::response::Json as ResponseJson;
use serde_json::Value;
use std::collections::HashMap;

/// Read definition documentation for a specific symbol
pub fn read_definition_doc(app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "namespace parameter is missing or not a string"
      }));
    }
  };

  let symbol = match req.parameters.get("symbol") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "symbol parameter is missing or not a string"
      }));
    }
  };

  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  // Find the definition in the snapshot
  if let Some(file_snapshot) = snapshot.files.get(&namespace) {
    if let Some(definition) = file_snapshot.defs.get(&symbol) {
      return ResponseJson(serde_json::json!({
        "namespace": namespace,
        "symbol": symbol,
        "doc": definition.doc
      }));
    }
  }

  ResponseJson(serde_json::json!({
    "error": format!("Definition {}/{} not found", namespace, symbol)
  }))
}

/// Update definition documentation for a specific symbol
pub fn update_definition_doc(app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "namespace parameter is missing or not a string"
      }));
    }
  };

  let symbol = match req.parameters.get("symbol") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "symbol parameter is missing or not a string"
      }));
    }
  };

  let doc = match req.parameters.get("doc") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "doc parameter is missing or not a string"
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

  // Update the definition documentation
  if let Some(file_snapshot) = snapshot.files.get_mut(&namespace) {
    if let Some(definition) = file_snapshot.defs.get_mut(&symbol) {
      definition.doc = doc.clone();

      // Save the updated snapshot
      if let Err(e) = save_snapshot(app_state, &snapshot) {
        return e;
      }

      return ResponseJson(serde_json::json!({
        "namespace": namespace,
        "symbol": symbol,
        "doc": doc,
        "status": "updated"
      }));
    }
  }

  ResponseJson(serde_json::json!({
    "error": format!("Definition {}/{} not found", namespace, symbol)
  }))
}

/// List all dependency modules
pub fn list_dependency_docs(app_state: &super::AppState, _req: McpRequest) -> ResponseJson<Value> {
  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  // Get all dependency modules from configs
  let dependencies: Vec<String> = snapshot.configs.modules.clone();

  ResponseJson(serde_json::json!({
    "dependencies": dependencies
  }))
}

/// Read definition documentation from a dependency module
pub fn read_dependency_definition_doc(_app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let dependency = match req.parameters.get("dependency") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "dependency parameter is missing or not a string"
      }));
    }
  };

  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "namespace parameter is missing or not a string"
      }));
    }
  };

  let symbol = match req.parameters.get("symbol") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "symbol parameter is missing or not a string"
      }));
    }
  };

  ResponseJson(serde_json::json!({
    "dependency": dependency,
    "namespace": namespace,
    "symbol": symbol,
    "doc": "Dependency documentation access not yet implemented",
    "note": "This is a read-only operation for external dependencies"
  }))
}

/// Read module documentation from a dependency
pub fn read_dependency_module_doc(_app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let dependency = match req.parameters.get("dependency") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "dependency parameter is missing or not a string"
      }));
    }
  };

  let title = match req.parameters.get("title") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "title parameter is missing or not a string"
      }));
    }
  };

  ResponseJson(serde_json::json!({
    "dependency": dependency,
    "title": title,
    "content": "Dependency module documentation access not yet implemented",
    "note": "This is a read-only operation for external dependencies"
  }))
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

/// List all module document titles in the project
pub fn list_module_docs(app_state: &super::AppState, _req: McpRequest) -> ResponseJson<Value> {
  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  let documents: Vec<String> = match &snapshot.docs {
    Some(docs) => docs.keys().cloned().collect(),
    None => vec![],
  };

  ResponseJson(serde_json::json!({
    "documents": documents
  }))
}

/// Read a specific module document by title
pub fn read_module_doc(app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let title = match req.parameters.get("title") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "title parameter is missing or not a string"
      }));
    }
  };

  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  match &snapshot.docs {
    Some(docs) => match docs.get(&title) {
      Some(doc_entry) => ResponseJson(serde_json::json!({
        "title": doc_entry.title,
        "content": doc_entry.content
      })),
      None => ResponseJson(serde_json::json!({
        "error": format!("Document '{}' not found", title)
      })),
    },
    None => ResponseJson(serde_json::json!({
      "error": format!("Document '{}' not found", title)
    })),
  }
}

/// Create or update a document
pub fn update_module_doc(app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let title = match req.parameters.get("title") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "title parameter is missing or not a string"
      }));
    }
  };

  let content = match req.parameters.get("content") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "content parameter is missing or not a string"
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

  // Initialize docs if it doesn't exist
  if snapshot.docs.is_none() {
    snapshot.docs = Some(HashMap::new());
  }

  let docs = snapshot.docs.as_mut().unwrap();
  let doc_entry = DocumentEntry {
    title: title.clone(),
    content,
  };

  let is_new = !docs.contains_key(&title);
  docs.insert(title.clone(), doc_entry);

  // Save snapshot
  if let Err(e) = save_snapshot(app_state, &snapshot) {
    return e;
  }

  let action = if is_new { "created" } else { "updated" };
  ResponseJson(serde_json::json!({
    "message": format!("Document '{}' {} successfully", title, action)
  }))
}

/// Rename a document
pub fn rename_module_doc(app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let old_title = match req.parameters.get("old_title") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "old_title parameter is missing or not a string"
      }));
    }
  };

  let new_title = match req.parameters.get("new_title") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "new_title parameter is missing or not a string"
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

  match &mut snapshot.docs {
    Some(docs) => {
      // Check if old document exists
      if !docs.contains_key(&old_title) {
        return ResponseJson(serde_json::json!({
          "error": format!("Document '{}' not found", old_title)
        }));
      }

      // Check if new title already exists
      if docs.contains_key(&new_title) {
        return ResponseJson(serde_json::json!({
          "error": format!("Document '{}' already exists", new_title)
        }));
      }

      // Move the document
      if let Some(mut doc_entry) = docs.remove(&old_title) {
        doc_entry.title = new_title.clone();
        docs.insert(new_title.clone(), doc_entry);
      }
    }
    None => {
      return ResponseJson(serde_json::json!({
        "error": format!("Document '{}' not found", old_title)
      }));
    }
  }

  // Save snapshot
  if let Err(e) = save_snapshot(app_state, &snapshot) {
    return e;
  }

  ResponseJson(serde_json::json!({
    "message": format!("Document renamed from '{}' to '{}' successfully", old_title, new_title)
  }))
}

/// Delete a document
pub fn delete_module_doc(app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let title = match req.parameters.get("title") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "title parameter is missing or not a string"
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

  match &mut snapshot.docs {
    Some(docs) => {
      if docs.remove(&title).is_none() {
        return ResponseJson(serde_json::json!({
          "error": format!("Document '{}' not found", title)
        }));
      }
    }
    None => {
      return ResponseJson(serde_json::json!({
        "error": format!("Document '{}' not found", title)
      }));
    }
  }

  // Save snapshot
  if let Err(e) = save_snapshot(app_state, &snapshot) {
    return e;
  }

  ResponseJson(serde_json::json!({
    "message": format!("Document '{}' deleted successfully", title)
  }))
}
