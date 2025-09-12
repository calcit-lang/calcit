use super::tools::{GetPackageNameRequest, ListDefinitionsRequest, ReadNamespaceRequest};
use axum::response::Json as ResponseJson;
use cirru_parser::CirruWriterOptions;
use serde_json::Value;

pub fn list_namespace_definitions(app_state: &super::AppState, request: ListDefinitionsRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;

  let result = app_state.state_manager.with_current_module(|snapshot| {
    // Check if namespace exists
    let file_data = match snapshot.files.get(&namespace) {
      Some(data) => data,
      None => {
        return ResponseJson(serde_json::json!({
          "error": format!("Namespace '{namespace}' not found")
        }));
      }
    };

    let definitions: Vec<String> = file_data.defs.keys().cloned().collect();

    ResponseJson(serde_json::json!({
      "namespace": namespace,
      "definitions": definitions
    }))
  });

  match result {
    Ok(response) => response,
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}

// list_namespaces function moved to namespace_handlers.rs to avoid duplication

pub fn get_package_name(app_state: &super::AppState, _request: GetPackageNameRequest) -> ResponseJson<Value> {
  let result = app_state.state_manager.with_current_module(|snapshot| {
    ResponseJson(serde_json::json!({
      "package_name": snapshot.package
    }))
  });

  match result {
    Ok(response) => response,
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}

pub fn read_namespace(app_state: &super::AppState, request: ReadNamespaceRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;

  let result = app_state.state_manager.with_current_module(|snapshot| {
    // Check if namespace exists
    let file_data = match snapshot.files.get(&namespace) {
      Some(data) => data,
      None => {
        return ResponseJson(serde_json::json!({
          "error": format!("Namespace '{namespace}' not found")
        }));
      }
    };

    // Convert namespace data to JSON, only return definition names, documentation and first 40 characters of code
    let mut definitions = serde_json::Map::new();
    for (def_name, code_entry) in &file_data.defs {
      // Convert code to string and truncate to first 40 characters
      let code_str = cirru_parser::format(&[code_entry.code.clone()], CirruWriterOptions { use_inline: true })
        .unwrap_or("(failed to convert code)".to_string());
      let code_preview = if code_str.len() > 40 {
        format!("{}...(too long)", &code_str[..40])
      } else {
        code_str
      };

      definitions.insert(
        def_name.clone(),
        serde_json::json!({
          "doc": code_entry.doc,
          "code_preview": code_preview
        }),
      );
    }

    ResponseJson(serde_json::json!({
      "namespace": namespace,
      "definitions": definitions,
      "ns": file_data.ns
    }))
  });

  match result {
    Ok(response) => response,
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}
