use super::tools::{ReadDefinitionAtRequest, ReadDefinitionDocRequest};
use super::validation::{validate_definition_name, validate_namespace_name};
use crate::mcp::definition_utils::{navigate_to_coord, parse_coord_from_json};
use axum::response::Json as ResponseJson;
use cirru_parser::Cirru;
use serde_json::Value;

// NOTE: Definition editing functions moved to CLI:
// - `upsert_definition` → `cr edit upsert-def`
// - `delete_definition` → `cr edit delete-def`
// - `operate_definition_at` → `cr edit operate-at`
// - `operate_definition_at_with_leaf` → `cr edit operate-at`
// - `update_definition_doc` → `cr edit update-def-doc`

/// Read documentation for a specific definition in any namespace (current project or dependency)
pub fn read_definition_doc(app_state: &super::AppState, request: ReadDefinitionDocRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;
  let definition = request.definition;

  // Validate namespace name
  if let Err(validation_error) = validate_namespace_name(&namespace) {
    return ResponseJson(serde_json::json!({
      "error": validation_error
    }));
  }

  // Validate definition name
  if let Err(validation_error) = validate_definition_name(&definition) {
    return ResponseJson(serde_json::json!({
      "error": validation_error
    }));
  }

  // First try to find in current module
  let current_module_result = app_state.state_manager.with_current_module(|snapshot| {
    if let Some(file) = snapshot.files.get(&namespace) {
      if let Some(def_entry) = file.defs.get(&definition) {
        return Some((def_entry.doc.clone(), def_entry.examples.clone()));
      }
    }
    None
  });

  // Get available definitions in current module for context
  let (current_namespace_exists, current_available_definitions) = app_state
    .state_manager
    .with_current_module(|snapshot| {
      if let Some(file) = snapshot.files.get(&namespace) {
        let available_definitions: Vec<String> = file.defs.keys().cloned().collect();
        (true, available_definitions) // namespace exists
      } else {
        (false, Vec::new()) // namespace doesn't exist
      }
    })
    .unwrap_or((false, Vec::new()));

  match current_module_result {
    Ok(Some((doc, examples))) => {
      return ResponseJson(serde_json::json!({
        "documentation": doc,
        "examples": examples,
        "source": "current_project",
        "namespace": namespace,
        "definition": definition
      }));
    }
    Ok(None) => {
      // Not found in current module, check if namespace exists for better error context
      if current_namespace_exists {
        // Namespace exists but definition not found - we'll try dependencies but prepare context
      } else {
        // Namespace doesn't exist in current project - we'll try dependencies but prepare context
      }
    }
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Failed to read current module: {}", e),
        "context": {
          "namespace": namespace,
          "definition": definition,
          "source": "current_project",
          "issue": "module_read_failure",
          "suggestions": [
            "Check if the project is properly loaded",
            "Verify the project structure is valid",
            "Try reloading the project",
            "Check for file system permissions issues"
          ]
        }
      }));
    }
  }

  // Try to find in dependency modules
  match app_state.state_manager.get_dependency_module(&namespace) {
    Ok(dep_snapshot) => {
      if let Some(file) = dep_snapshot.files.get(&namespace) {
        if let Some(def_entry) = file.defs.get(&definition) {
          ResponseJson(serde_json::json!({
            "documentation": def_entry.doc,
            "examples": def_entry.examples,
            "source": "dependency",
            "namespace": namespace,
            "definition": definition
          }))
        } else {
          // Get available definitions in the dependency namespace for context
          let available_definitions: Vec<String> = file.defs.keys().cloned().collect();
          ResponseJson(serde_json::json!({
            "error": format!("Definition '{}' not found in namespace '{}' (dependency module)", definition, namespace),
            "context": {
              "namespace": namespace,
              "definition": definition,
              "source": "dependency",
              "available_definitions": available_definitions,
              "suggestions": [
                "Check the definition name for typos",
                "Use 'list_namespace_definitions' tool to see all available definitions",
                "Verify the definition exists in the dependency module",
                format!("Available definitions in '{}': {}", namespace,
                  if available_definitions.is_empty() { "(none)".to_string() }
                  else { available_definitions.join(", ") })
              ]
            }
          }))
        }
      } else {
        ResponseJson(serde_json::json!({
          "error": format!("Namespace '{}' not found in dependency modules", namespace),
          "context": {
            "namespace": namespace,
            "definition": definition,
            "source": "dependency",
            "issue": "namespace_not_found_in_dependencies",
            "suggestions": [
              "Check if the namespace name is correct",
              "Verify the dependency is properly loaded",
              "Use 'list_dependency_namespaces' tool to see available dependency namespaces",
              "Check if this namespace exists in the current project instead"
            ]
          }
        }))
      }
    }
    Err(dep_error) => {
      // Namespace not found in dependencies either, provide comprehensive context
      let mut error_context = serde_json::json!({
        "namespace": namespace,
        "definition": definition,
        "issue": "namespace_not_found_anywhere",
        "dependency_error": format!("{}", dep_error),
        "current_project": {
          "namespace_exists": current_namespace_exists,
          "available_definitions": current_available_definitions
        }
      });

      let mut suggestions = vec![
        "Verify the namespace name is spelled correctly".to_string(),
        "Check if the namespace exists using 'list_namespaces' tool".to_string(),
        "Use 'list_dependency_namespaces' tool to see available dependency namespaces".to_string(),
        "Ensure all required dependencies are loaded".to_string(),
      ];

      if current_namespace_exists {
        suggestions.insert(
          0,
          format!(
            "Definition '{}' exists in current project namespace '{}' but not found. Available definitions: {}",
            definition,
            namespace,
            if current_available_definitions.is_empty() {
              "(none)".to_string()
            } else {
              current_available_definitions.join(", ")
            }
          ),
        );
        suggestions.push("Check if the definition name has typos".to_string());
      } else {
        suggestions.push("Consider if this might be a new namespace that needs to be created".to_string());
      }

      error_context["suggestions"] = serde_json::json!(suggestions);

      ResponseJson(serde_json::json!({
        "error": format!("Namespace '{}' not found in current project or dependencies", namespace),
        "context": error_context
      }))
    }
  }
}

pub fn read_definition_at(app_state: &super::AppState, request: ReadDefinitionAtRequest) -> ResponseJson<Value> {
  let namespace = request.namespace;
  let definition = request.definition;

  // Validate namespace name
  if let Err(validation_error) = validate_namespace_name(&namespace) {
    return ResponseJson(serde_json::json!({
      "error": validation_error
    }));
  }

  // Validate definition name
  if let Err(validation_error) = validate_definition_name(&definition) {
    return ResponseJson(serde_json::json!({
      "error": validation_error
    }));
  }

  let coord: Vec<usize> = match parse_coord_from_json(&request.coord) {
    Ok(coord_vec) => coord_vec,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Invalid coord parameter: {}\n\nCoord format requirements:\n• Must be a JSON array of non-negative integers\n• Index starts from 0 (zero-based indexing)\n• Example: [0] for first element, [1, 2] for third element of second element\n• Use empty array [] for root level\n\nSuggested fixes:\n• Check that all values are non-negative integers\n• Ensure proper JSON array format: [0, 1, 2]\n• Start with [] to read the entire definition", e)
      }));
    }
  };

  // First try to find in current module
  let current_module_result = app_state.state_manager.with_current_module(|snapshot| {
    if let Some(file_data) = snapshot.files.get(&namespace) {
      if let Some(code_entry) = file_data.defs.get(&definition) {
        return Some((code_entry.clone(), "current_project".to_string()));
      } else {
        // Namespace exists but definition not found - collect available definitions for context
        let _available_definitions: Vec<String> = file_data.defs.keys().cloned().collect();
        // Return early with error context
        return None; // Will be handled below
      }
    }
    None
  });

  // Check if we found it in current module
  if let Ok(Some((entry, src))) = current_module_result {
    // Navigate to the target coordinate
    let target = match navigate_to_coord(&entry.code, &coord) {
      Ok(t) => t,
      Err(e) => {
        return ResponseJson(serde_json::json!({
          "error": format!("Failed to navigate to coordinate {coord:?}: {e}\n\nNavigation troubleshooting:\n• Coordinate {coord:?} may be out of bounds\n• Use empty array [] to read the entire definition\n• Use shorter coordinates to navigate step by step\n• Check if the target is a list (only lists can have child elements)\n\nSuggested fixes:\n• Start with [] to see the root structure\n• Use coordinates like [0], [1], [2] for top-level elements\n• For nested access, build coordinates incrementally: [0] → [0, 1] → [0, 1, 2]")
        }));
      }
    };

    let expr_length = match &target {
      Cirru::Leaf(_) => None,
      Cirru::List(xs) => Some(xs.len()),
    };

    return ResponseJson(serde_json::json!({
      "value": target,
      "expr_length": expr_length,
      "source": src,
      "message": format!("Definition '{}' read at coordinate {:?} in namespace '{}' successfully (from {})", definition, coord, namespace, src),
      "tips": {
        "next_steps": [
          format!("Use 'cr edit operate-at' CLI to modify this location"),
          "Navigate deeper by appending indices to the coord array (e.g., [2, 1] becomes [2, 1, 0])".to_string()
        ],
        "coord_hint": "Each integer in coord navigates one level deeper into the expression tree (0-indexed)"
      }
    }));
  }

  // Check if namespace exists in current module but definition doesn't
  let namespace_exists_in_current = app_state
    .state_manager
    .with_current_module(|snapshot| snapshot.files.contains_key(&namespace))
    .unwrap_or(false);

  if namespace_exists_in_current {
    // We already handled this case above, but need to return the error here
    let available_definitions = app_state
      .state_manager
      .with_current_module(|snapshot| {
        snapshot
          .files
          .get(&namespace)
          .map(|file_data| file_data.defs.keys().cloned().collect::<Vec<String>>())
          .unwrap_or_default()
      })
      .unwrap_or_default();

    let mut error_context = serde_json::json!({
      "namespace": namespace,
      "definition": definition,
      "source": "current_project",
      "issue": "definition_not_found_in_namespace",
      "available_definitions": available_definitions
    });

    let mut suggestions = vec![
      "Check if the definition name is spelled correctly".to_string(),
      "Use 'list_definitions' tool to see all available definitions in this namespace".to_string(),
    ];

    if !available_definitions.is_empty() {
      suggestions.insert(
        0,
        format!("Available definitions in '{}': {}", namespace, available_definitions.join(", ")),
      );
      suggestions.push("Look for similar definition names in the namespace".to_string());
    } else {
      suggestions.push("This namespace exists but has no definitions yet".to_string());
      suggestions.push("Consider adding the definition using 'cr edit upsert-def' CLI".to_string());
    }

    error_context["suggestions"] = serde_json::json!(suggestions);

    return ResponseJson(serde_json::json!({
      "error": format!("Definition '{}' not found in namespace '{}'", definition, namespace),
      "context": error_context
    }));
  }

  // Not found in current module, try dependencies
  let (code_entry, source) = match app_state.state_manager.get_dependency_module(&namespace) {
    Ok(dep_snapshot) => {
      if let Some(file_data) = dep_snapshot.files.get(&namespace) {
        if let Some(code_entry) = file_data.defs.get(&definition) {
          (code_entry.clone(), "dependency".to_string())
        } else {
          // Get available definitions in dependency namespace for context
          let available_definitions: Vec<String> = file_data.defs.keys().cloned().collect();

          let mut error_context = serde_json::json!({
            "namespace": namespace,
            "definition": definition,
            "source": "dependency",
            "issue": "definition_not_found_in_dependency_namespace",
            "available_definitions": available_definitions
          });

          let mut suggestions = vec![
            "Check if the definition name is spelled correctly".to_string(),
            "Use 'list_definitions' tool to see all available definitions in this namespace".to_string(),
          ];

          if !available_definitions.is_empty() {
            suggestions.insert(
              0,
              format!(
                "Available definitions in dependency '{}': {}",
                namespace,
                available_definitions.join(", ")
              ),
            );
            suggestions.push("Look for similar definition names in the dependency namespace".to_string());
          } else {
            suggestions.push("This dependency namespace exists but has no definitions".to_string());
          }

          error_context["suggestions"] = serde_json::json!(suggestions);

          return ResponseJson(serde_json::json!({
            "error": format!("Definition '{}' not found in namespace '{}' (dependency)", definition, namespace),
            "context": error_context
          }));
        }
      } else {
        // Get available dependency namespaces for context
        let available_dep_namespaces: Vec<String> = dep_snapshot.files.keys().cloned().collect();

        let mut error_context = serde_json::json!({
          "namespace": namespace,
          "definition": definition,
          "source": "dependency",
          "issue": "namespace_not_found_in_dependencies",
          "available_dependency_namespaces": available_dep_namespaces
        });

        let mut suggestions = vec![
          "Check if the namespace name is spelled correctly".to_string(),
          "Use 'list_dependency_namespaces' tool to see all available dependency namespaces".to_string(),
          "Ensure all required dependencies are loaded".to_string(),
        ];

        if !available_dep_namespaces.is_empty() {
          suggestions.insert(
            0,
            format!("Available dependency namespaces: {}", available_dep_namespaces.join(", ")),
          );
          suggestions.push("Look for similar namespace names in the available dependencies".to_string());
        }

        error_context["suggestions"] = serde_json::json!(suggestions);

        return ResponseJson(serde_json::json!({
          "error": format!("Namespace '{}' not found in dependency modules", namespace),
          "context": error_context
        }));
      }
    }
    Err(_) => {
      // Get available namespaces from current module for error message
      let available_namespaces = app_state
        .state_manager
        .with_current_module(|snapshot| snapshot.files.keys().cloned().collect::<Vec<String>>())
        .unwrap_or_else(|_| vec![]);

      let mut error_context = serde_json::json!({
        "namespace": namespace,
        "definition": definition,
        "source": "current_project",
        "issue": "namespace_not_found_anywhere",
        "available_namespaces": available_namespaces
      });

      let mut suggestions = vec![
        "Check the namespace name for typos".to_string(),
        "Use 'list_namespaces' tool to see all available namespaces".to_string(),
      ];

      if !available_namespaces.is_empty() {
        suggestions.insert(0, format!("Available namespaces: {}", available_namespaces.join(", ")));
        suggestions.push("Use one of the existing namespaces listed above".to_string());
      } else {
        suggestions.push("Consider creating the namespace first using 'cr edit add-ns' CLI".to_string());
      }

      error_context["suggestions"] = serde_json::json!(suggestions);

      return ResponseJson(serde_json::json!({
        "error": format!("Namespace '{namespace}' not found"),
        "context": error_context
      }));
    }
  };

  // Navigate to the target coordinate
  let target = match navigate_to_coord(&code_entry.code, &coord) {
    Ok(t) => t,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Failed to navigate to coordinate {coord:?}: {e}\n\nNavigation troubleshooting:\n• Coordinate {coord:?} may be out of bounds\n• Use empty array [] to read the entire definition\n• Use shorter coordinates to navigate step by step\n• Check if the target is a list (only lists can have child elements)\n\nSuggested fixes:\n• Start with [] to see the root structure\n• Use coordinates like [0], [1], [2] for top-level elements\n• For nested access, build coordinates incrementally: [0] → [0, 1] → [0, 1, 2]")
      }));
    }
  };

  let expr_length = match &target {
    Cirru::Leaf(_) => None,
    Cirru::List(xs) => Some(xs.len()),
  };

  ResponseJson(serde_json::json!({
    "value": target,
    "expr_length": expr_length,
    "source": source,
    "message": format!("Definition '{}' read at coordinate {:?} in namespace '{}' successfully (from {})", definition, coord, namespace, source),
    "tips": {
      "next_steps": if source == "current_project" {
        vec![
          "Use 'cr edit operate-at' CLI to modify this location".to_string(),
          "Navigate deeper by appending indices to the coord array (e.g., [2, 1] becomes [2, 1, 0])".to_string()
        ]
      } else {
        vec![
          "This definition is from a dependency and is read-only".to_string(),
          "Use 'read_definition_doc' to see documentation".to_string()
        ]
      },
      "coord_hint": "Each integer in coord navigates one level deeper into the expression tree (0-indexed)"
    }
  }))
}
