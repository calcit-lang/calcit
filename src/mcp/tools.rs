use super::jsonrpc::Tool;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct McpTool {
  pub name: String,
  pub description: String,
  pub parameters: Vec<McpToolParameter>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpToolParameter {
  pub name: String,
  #[serde(rename = "type")]
  pub parameter_type: String,
  pub description: String,
  pub optional: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpRequest {
  pub tool_name: String,
  pub parameters: serde_json::Value,
}

/// Convert legacy McpTool to standard MCP Tool format
fn mcp_tool_to_standard(mcp_tool: &McpTool) -> Tool {
  let mut properties = serde_json::Map::new();
  let mut required = Vec::new();

  for param in &mcp_tool.parameters {
    let mut param_schema = serde_json::Map::new();
    param_schema.insert("type".to_string(), serde_json::Value::String(param.parameter_type.clone()));
    param_schema.insert("description".to_string(), serde_json::Value::String(param.description.clone()));

    properties.insert(param.name.clone(), serde_json::Value::Object(param_schema));

    if !param.optional {
      required.push(serde_json::Value::String(param.name.clone()));
    }
  }

  let mut input_schema = serde_json::Map::new();
  input_schema.insert("type".to_string(), serde_json::Value::String("object".to_string()));
  input_schema.insert("properties".to_string(), serde_json::Value::Object(properties));

  if !required.is_empty() {
    input_schema.insert("required".to_string(), serde_json::Value::Array(required));
  }

  Tool {
    name: mcp_tool.name.clone(),
    description: mcp_tool.description.clone(),
    input_schema: serde_json::Value::Object(input_schema),
  }
}

pub fn get_mcp_tools() -> Vec<McpTool> {
  vec![
    // Calcit Language Tools - Calcit is a functional programming language with Lisp-like syntax using Cirru notation
    // These tools help interact with Calcit projects, which organize code in namespaces containing function/macro definitions

    // Reading Operations
    McpTool {
      name: "list_definitions".to_string(),
      description: "List all function and macro definitions in a Calcit namespace. Calcit organizes code in namespaces, where each namespace contains definitions (functions, macros, variables). This tool helps explore the structure of Calcit code by showing what's available in a specific namespace.".to_string(),
      parameters: vec![McpToolParameter {
        name: "namespace".to_string(),
        parameter_type: "string".to_string(),
        description: "The namespace to list definitions from (e.g., 'app.main', 'utils.math')".to_string(),
        optional: false,
      }],
    },
    McpTool {
      name: "list_namespaces".to_string(),
      description: "List all namespaces in the Calcit project. Calcit projects are organized into namespaces (similar to modules in other languages). Each namespace typically represents a logical grouping of related functions and can import from other namespaces.".to_string(),
      parameters: vec![],
    },
    McpTool {
      name: "get_package_name".to_string(),
      description: "Get the package name of the current Calcit project. Calcit projects have a package name that identifies them, useful for understanding the project structure and dependencies.".to_string(),
      parameters: vec![],
    },
    McpTool {
      name: "read_namespace".to_string(),
      description: "Read detailed information about a Calcit namespace, including its import rules and metadata. Calcit namespaces can import functions from other namespaces using import rules, and this tool shows the complete namespace configuration.".to_string(),
      parameters: vec![McpToolParameter {
        name: "namespace".to_string(),
        parameter_type: "string".to_string(),
        description: "The namespace to read (e.g., 'app.main', 'utils.math')".to_string(),
        optional: false,
      }],
    },
    McpTool {
      name: "read_definition".to_string(),
      description: "Read the source code and documentation of a specific function or macro definition in Calcit. Calcit uses Cirru syntax (a Lisp-like notation with parentheses) and this tool returns the actual code structure.".to_string(),
      parameters: vec![
        McpToolParameter {
          name: "namespace".to_string(),
          parameter_type: "string".to_string(),
          description: "The namespace containing the definition (e.g., 'app.main')".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "definition".to_string(),
          parameter_type: "string".to_string(),
          description: "The name of the function or macro to read (e.g., 'fibonacci', 'main')".to_string(),
          optional: false,
        },
      ],
    },
    // Namespace Management Operations
    McpTool {
      name: "add_namespace".to_string(),
      description: "Create a new namespace in the Calcit project. Namespaces in Calcit are like modules that group related functions together. Each namespace can have its own import rules to access functions from other namespaces.".to_string(),
      parameters: vec![
        McpToolParameter {
          name: "namespace".to_string(),
          parameter_type: "string".to_string(),
          description: "The name of the new namespace (e.g., 'app.utils', 'math.geometry')".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "imports".to_string(),
          parameter_type: "string".to_string(),
          description: "Import rules for the namespace in Cirru format (optional). Defines which functions to import from other namespaces.".to_string(),
          optional: true,
        },
      ],
    },
    McpTool {
      name: "delete_namespace".to_string(),
      description: "Remove a namespace from the Calcit project. This will delete all functions and macros defined in that namespace. Use with caution as this operation cannot be undone.".to_string(),
      parameters: vec![McpToolParameter {
        name: "namespace".to_string(),
        parameter_type: "string".to_string(),
        description: "The namespace to delete (e.g., 'app.old-module')".to_string(),
        optional: false,
      }],
    },
    McpTool {
      name: "update_namespace_imports".to_string(),
      description: "Modify the import rules of a Calcit namespace. Import rules determine which functions from other namespaces are available in the current namespace. Calcit uses a flexible import system similar to Clojure.".to_string(),
      parameters: vec![
        McpToolParameter {
          name: "namespace".to_string(),
          parameter_type: "string".to_string(),
          description: "The namespace to update (e.g., 'app.main')".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "imports".to_string(),
          parameter_type: "string".to_string(),
          description: "New import rules in Cirru format (e.g., rules to import specific functions or entire namespaces)".to_string(),
          optional: false,
        },
      ],
    },
    // Function/Macro Definition Operations
    McpTool {
      name: "add_definition".to_string(),
      description: "Create a new function or macro definition in a Calcit namespace. Calcit functions are defined using Cirru syntax (Lisp-like with parentheses). Functions can be pure functions, macros for code generation, or variables holding values.".to_string(),
      parameters: vec![
        McpToolParameter {
          name: "namespace".to_string(),
          parameter_type: "string".to_string(),
          description: "The namespace to add the definition to (e.g., 'app.main', 'utils.math')".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "definition".to_string(),
          parameter_type: "string".to_string(),
          description: "The name of the new function or macro (e.g., 'fibonacci', 'my-macro', 'config-value')".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "code".to_string(),
          parameter_type: "array".to_string(),
          description: "The function body in Cirru format as nested arrays. Example: ['fn', ['x'], ['+', 'x', '1']] for a function that adds 1 to its argument.".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "doc".to_string(),
          parameter_type: "string".to_string(),
          description: "Documentation string describing what the function does (optional but recommended)".to_string(),
          optional: true,
        },
      ],
    },
    McpTool {
      name: "delete_definition".to_string(),
      description: "Remove a function or macro definition from a Calcit namespace. This permanently deletes the definition and cannot be undone. Make sure no other code depends on this definition.".to_string(),
      parameters: vec![
        McpToolParameter {
          name: "namespace".to_string(),
          parameter_type: "string".to_string(),
          description: "The namespace containing the definition (e.g., 'app.main')".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "definition".to_string(),
          parameter_type: "string".to_string(),
          description: "The name of the function or macro to delete (e.g., 'old-function')".to_string(),
          optional: false,
        },
      ],
    },
    McpTool {
      name: "overwrite_definition".to_string(),
      description: "Completely overwrite an existing function or macro definition in Calcit. This replaces the entire definition with new code and documentation. The code must be provided in Cirru syntax format. ⚠️ RECOMMENDATION: Avoid using this tool for most cases. Instead, use 'read_definition_at' to explore the code structure first, then use 'update_definition_at' for precise, localized updates.".to_string(),
      parameters: vec![
        McpToolParameter {
          name: "namespace".to_string(),
          parameter_type: "string".to_string(),
          description: "The namespace containing the definition (e.g., 'app.main')".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "definition".to_string(),
          parameter_type: "string".to_string(),
          description: "The name of the function or macro to update (e.g., 'fibonacci')".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "code".to_string(),
          parameter_type: "array".to_string(),
          description: "New function body in Cirru format as nested arrays (optional). Leave empty to only update documentation.".to_string(),
          optional: true,
        },
        McpToolParameter {
          name: "doc".to_string(),
          parameter_type: "string".to_string(),
          description: "New documentation string (optional). Leave empty to only update code.".to_string(),
          optional: true,
        },
      ],
    },
    McpTool {
      name: "update_definition_at".to_string(),
      description: "Update a specific part of a function or macro definition using coordinate-based targeting with various operation modes. Cirru code is a tree structure that can be navigated using coordinate arrays (Vec<Int>). This tool allows precise updates to specific nodes in the code tree with validation. 💡 BEST PRACTICE: Always use 'read_definition_at' multiple times first to explore and understand the code structure, then generate correct 'match' and 'coord' parameters for safe updates. Empty coord [] operates on the root node.".to_string(),
      parameters: vec![
        McpToolParameter {
          name: "namespace".to_string(),
          parameter_type: "string".to_string(),
          description: "The namespace containing the definition (e.g., 'app.main')".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "definition".to_string(),
          parameter_type: "string".to_string(),
          description: "The name of the function or macro to update (e.g., 'fibonacci')".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "coord".to_string(),
          parameter_type: "array".to_string(),
          description: "Coordinate array (Vec<Int>) specifying the exact location in the code tree to update (e.g., [0, 1, 2] to target the 3rd element of the 2nd element of the 1st element)".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "mode".to_string(),
          parameter_type: "string".to_string(),
          description: "Operation mode: 'replace' (default), 'after', 'before', 'delete', 'prepend', 'append'. Replace updates the target, after/before insert adjacent to target, delete removes target, prepend/append modify target if it's a list.".to_string(),
          optional: true,
        },
        McpToolParameter {
          name: "new_value".to_string(),
          parameter_type: "string".to_string(),
          description: "The new value to set at the specified coordinate. Can be a string literal or Cirru code. Not required for 'delete' mode.".to_string(),
          optional: true,
        },
        McpToolParameter {
          name: "match_content".to_string(),
          parameter_type: "string".to_string(),
          description: "Optional validation: string to verify exact match, or array like ['fn', '...'] to verify list starts with 'fn'. If validation fails, returns detailed error with current content.".to_string(),
          optional: true,
        },
      ],
    },
    McpTool {
      name: "read_definition_at".to_string(),
      description: "Read a specific part of a function or macro definition using coordinate-based targeting. This allows precise querying of specific nodes in the Cirru code tree structure.".to_string(),
      parameters: vec![
        McpToolParameter {
          name: "namespace".to_string(),
          parameter_type: "string".to_string(),
          description: "The namespace containing the definition (e.g., 'app.main')".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "definition".to_string(),
          parameter_type: "string".to_string(),
          description: "The name of the function or macro to read from (e.g., 'fibonacci')".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "coord".to_string(),
          parameter_type: "array".to_string(),
          description: "Coordinate array (Vec<Int>) specifying the exact location in the code tree to read (e.g., [0, 1, 2] to target the 3rd element of the 2nd element of the 1st element)".to_string(),
          optional: false,
        },
      ],
    },
    // Module Management Operations
    McpTool {
      name: "list_modules".to_string(),
      description: "List all available Calcit modules including the current project and its dependencies. Calcit projects can depend on other Calcit modules, similar to how Node.js projects depend on npm packages. This shows the module ecosystem available to the current project.".to_string(),
      parameters: vec![],
    },
    McpTool {
      name: "read_module".to_string(),
      description: "Read detailed information about a Calcit module, including its package name and available namespaces. Modules in Calcit are self-contained units that can be shared and reused across projects.".to_string(),
      parameters: vec![McpToolParameter {
        name: "module_path".to_string(),
        parameter_type: "string".to_string(),
        description: "The file system path to the module to read (leave empty to read the current module)".to_string(),
        optional: true,
      }],
    },
    McpTool {
      name: "add_module_dependency".to_string(),
      description: "Add a dependency on another Calcit module to the current project. This allows the current project to import and use functions from the dependency module. Similar to adding a library dependency in other languages.".to_string(),
      parameters: vec![McpToolParameter {
        name: "module_path".to_string(),
        parameter_type: "string".to_string(),
        description: "The file system path to the Calcit module to add as a dependency".to_string(),
        optional: false,
      }],
    },
    McpTool {
      name: "remove_module_dependency".to_string(),
      description: "Remove a module dependency from the current Calcit project. This will prevent the project from accessing functions from that module. Make sure no code in the project depends on the module before removing it.".to_string(),
      parameters: vec![McpToolParameter {
        name: "module_path".to_string(),
        parameter_type: "string".to_string(),
        description: "The file system path to the module to remove from dependencies".to_string(),
        optional: false,
      }],
    },
    McpTool {
      name: "clear_module_cache".to_string(),
      description: "Clear the Calcit module cache to force reload of all dependencies. This is useful when dependency modules have been updated and you want to ensure the latest versions are loaded.".to_string(),
      parameters: vec![],
    },
    // Cirru Syntax Conversion Tools
    McpTool {
      name: "calcit_parse_cirru_to_json".to_string(),
      description: "Parse Cirru syntax string into JSON structure. Cirru is the syntax format used by Calcit - it's like Lisp but uses indentation and spaces instead of parentheses. This tool converts human-readable Cirru code into a structured format that can be programmatically manipulated.".to_string(),
      parameters: vec![McpToolParameter {
        name: "cirru_code".to_string(),
        parameter_type: "string".to_string(),
        description: "Cirru syntax code as a string. Example: 'fn (x) (+ x 1)' which represents a function that adds 1 to its argument.".to_string(),
        optional: false,
      }],
    },
    McpTool {
      name: "calcit_format_json_to_cirru".to_string(),
      description: "Convert JSON structure back to readable Cirru syntax string. This is the reverse of cirru_to_json - it takes structured data and formats it as human-readable Cirru code that can be saved to files or displayed to users.".to_string(),
      parameters: vec![McpToolParameter {
        name: "json_data".to_string(),
        parameter_type: "array".to_string(),
        description: "JSON structure representing Cirru code as nested arrays. Example: ['fn', ['x'], ['+', 'x', '1']] which will be formatted as readable Cirru syntax.".to_string(),
        optional: false,
      }],
    },

    // Calcit Documentation and Tutorial Tools
    McpTool {
      name: "query_api_docs".to_string(),
      description: "Query Calcit API documentation from the official API repository. This tool searches through Calcit's built-in functions and macros, allowing you to find APIs by tag (like :syntax, :macro, :function) or by keyword search. The API data includes function names, descriptions, usage examples, and code snippets to help with Calcit development.".to_string(),
      parameters: vec![
        McpToolParameter {
          name: "query_type".to_string(),
          parameter_type: "string".to_string(),
          description: "Type of query: 'tag' to search by tag (e.g., 'syntax', 'macro', 'function'), 'keyword' to search by text in name/description, or 'all' to list all APIs".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "query_value".to_string(),
          parameter_type: "string".to_string(),
          description: "The search value: tag name (for tag queries), keyword text (for keyword queries), or empty string (for 'all' queries)".to_string(),
          optional: true,
        },
      ],
    },
    McpTool {
      name: "query_guidebook".to_string(),
      description: "Query Calcit guidebook and tutorial documentation. This tool searches through Calcit's official guidebook which contains tutorials, language guides, and learning materials. You can search by filename/path or by keyword to find relevant documentation sections about Calcit language features, syntax, and best practices.".to_string(),
      parameters: vec![
        McpToolParameter {
          name: "query_type".to_string(),
          parameter_type: "string".to_string(),
          description: "Type of query: 'filename' to search by file path/name, 'keyword' to search by text content, or 'all' to list all available documents".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "query_value".to_string(),
          parameter_type: "string".to_string(),
          description: "The search value: filename/path (for filename queries), keyword text (for keyword queries), or empty string (for 'all' queries)".to_string(),
          optional: true,
        },
      ],
    },
    McpTool {
      name: "list_api_docs".to_string(),
      description: "List all available API documentation files and their basic information. This tool provides an overview of all Calcit API documentation files in the repository, showing file names, tags, and brief descriptions to help users navigate and discover available APIs before querying specific content.".to_string(),
      parameters: vec![],
    },
    McpTool {
      name: "list_guidebook_docs".to_string(),
      description: "List all available guidebook and tutorial documentation files. This tool provides an overview of all Calcit guidebook files in the repository, showing file names, paths, and brief descriptions to help users navigate and discover available tutorials and guides before querying specific content.".to_string(),
      parameters: vec![],
    },

    // Configuration Management Tools
    McpTool {
      name: "read_configs".to_string(),
      description: "Read the current project configuration settings. Calcit projects have configuration settings that control initialization functions, reload functions, and version information. This tool returns the complete configuration structure including init-fn, reload-fn, and version.".to_string(),
      parameters: vec![],
    },
    McpTool {
      name: "update_configs".to_string(),
      description: "Update multiple configuration settings at once. This tool allows updating any combination of init-fn, reload-fn, and version in a single operation. Only provide the fields you want to update - omitted fields will remain unchanged.".to_string(),
      parameters: vec![
        McpToolParameter {
          name: "init_fn".to_string(),
          parameter_type: "string".to_string(),
          description: "New initialization function in 'namespace/function-name' format (optional)".to_string(),
          optional: true,
        },
        McpToolParameter {
          name: "reload_fn".to_string(),
          parameter_type: "string".to_string(),
          description: "New reload function in 'namespace/function-name' format (optional)".to_string(),
          optional: true,
        },
        McpToolParameter {
          name: "version".to_string(),
          parameter_type: "string".to_string(),
          description: "New version string (optional)".to_string(),
          optional: true,
        },
      ],
    },

    // Dependency Documentation Tools (read-only access to dependency modules)
    McpTool {
      name: "list_dependency_docs".to_string(),
      description: "List all available documentation from dependency modules. This provides read-only access to documentation from external modules that this project depends on, including both definition-level docs and module-level docs.".to_string(),
      parameters: vec![
        McpToolParameter {
          name: "dependency_name".to_string(),
          parameter_type: "string".to_string(),
          description: "The name of the dependency module to query".to_string(),
          optional: false,
        },
      ],
    },
    McpTool {
      name: "read_dependency_definition_doc".to_string(),
      description: "Read the documentation string of a specific definition from a dependency module. This provides read-only access to definition-level documentation from external modules.".to_string(),
      parameters: vec![
        McpToolParameter {
          name: "dependency_name".to_string(),
          parameter_type: "string".to_string(),
          description: "The name of the dependency module".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "namespace".to_string(),
          parameter_type: "string".to_string(),
          description: "The namespace within the dependency".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "definition".to_string(),
          parameter_type: "string".to_string(),
          description: "The name of the definition".to_string(),
          optional: false,
        },
      ],
    },
    McpTool {
      name: "read_dependency_module_doc".to_string(),
      description: "Read a module-level document from a dependency module. This provides read-only access to complex documentation from external modules.".to_string(),
      parameters: vec![
        McpToolParameter {
          name: "dependency_name".to_string(),
          parameter_type: "string".to_string(),
          description: "The name of the dependency module".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "doc_path".to_string(),
          parameter_type: "string".to_string(),
          description: "The relative path of the documentation file to read".to_string(),
          optional: false,
        },
      ],
    },
  ]
}

// Request structs for read_handlers
#[derive(Debug, Serialize, Deserialize)]
pub struct ListDefinitionsRequest {
  pub namespace: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetPackageNameRequest {
  // No parameters needed
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReadNamespaceRequest {
  pub namespace: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReadDefinitionRequest {
  pub namespace: String,
  pub definition: String,
}

// Request structs for namespace_handlers
#[derive(Debug, Serialize, Deserialize)]
pub struct AddNamespaceRequest {
  pub namespace: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteNamespaceRequest {
  pub namespace: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListNamespacesRequest {
  // No parameters needed
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateNamespaceImportsRequest {
  pub namespace: String,
  pub imports: serde_json::Value,
}

// Request structs for definition_handlers
#[derive(Debug, Serialize, Deserialize)]
pub struct AddDefinitionRequest {
  pub namespace: String,
  pub definition: String,
  pub code: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteDefinitionRequest {
  pub namespace: String,
  pub definition: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OverwriteDefinitionRequest {
  pub namespace: String,
  pub definition: String,
  pub code: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateDefinitionAtRequest {
  pub namespace: String,
  pub definition: String,
  pub coord: serde_json::Value,
  pub new_value: Option<serde_json::Value>,
  pub mode: Option<String>,
  #[serde(rename = "match")]
  pub match_content: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ReadDefinitionAtRequest {
  pub namespace: String,
  pub definition: String,
  pub coord: serde_json::Value,
}

// Module management request structs
#[derive(Debug, Deserialize)]
pub struct SwitchModuleRequest {
  pub module: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateModuleRequest {
  pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct DeleteModuleRequest {
  pub module: String,
}

#[derive(Debug, Deserialize)]
pub struct ParseCirruToJsonRequest {
  pub cirru_code: String,
}

#[derive(Debug, Deserialize)]
pub struct FormatJsonToCirruRequest {
  pub json_data: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct QueryApiDocsRequest {
  pub query_type: String,
  pub query_value: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct QueryGuidebookRequest {
  pub query_type: String,
  pub query_value: String,
}

// Config handlers request structs
#[derive(Debug, Deserialize)]
pub struct ReadConfigsRequest {
  // No parameters needed for reading configs
}

#[derive(Debug, Deserialize)]
pub struct UpdateConfigsRequest {
  pub init_fn: Option<String>,
  pub reload_fn: Option<String>,
  pub version: Option<String>,
}

// Dependency document handlers request structs
#[derive(Debug, Deserialize)]
pub struct ListDependencyDocsRequest {
  pub dependency_name: String,
}

#[derive(Debug, Deserialize)]
pub struct ReadDependencyDefinitionDocRequest {
  pub dependency_name: String,
  pub namespace: String,
  pub definition: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadDependencyModuleDocRequest {
  pub dependency_name: String,
  pub doc_path: String,
}

// Docs handlers request structs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListApiDocsRequest {
  // No parameters needed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListGuidebookDocsRequest {
  // No parameters needed
}

// Module handlers request structs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCurrentModuleRequest {
  // No parameters needed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListModulesRequest {
  // No parameters needed
}

/// Get tools in standard MCP format
pub fn get_standard_mcp_tools() -> Vec<Tool> {
  get_mcp_tools().iter().map(mcp_tool_to_standard).collect()
}
