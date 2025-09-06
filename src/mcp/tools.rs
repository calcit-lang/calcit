use super::jsonrpc::Tool;
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct McpRequest {
  pub tool_name: String,
  pub parameters: serde_json::Value,
}

// Tool definition with integrated schema generation
pub struct McpToolWithSchema {
  pub name: &'static str,
  pub description: &'static str,
  pub schema_generator: fn() -> serde_json::Value,
}

impl McpToolWithSchema {
  pub fn to_standard_tool(&self) -> Tool {
    Tool {
      name: self.name.to_string(),
      description: self.description.to_string(),
      input_schema: (self.schema_generator)(),
    }
  }
}

pub fn get_mcp_tools_with_schema() -> Vec<McpToolWithSchema> {
  vec![
    // Calcit Language Tools - Calcit is a functional programming language with Lisp-like syntax using Cirru notation
    // These tools help interact with Calcit projects, which organize code in namespaces containing function/macro definitions

    // Reading Operations
    McpToolWithSchema {
      name: "list_definitions",
      description: "List all function and macro definitions in a Calcit namespace. Calcit organizes code in namespaces, where each namespace contains definitions (functions, macros, variables). This tool helps explore the structure of Calcit code by showing what's available in a specific namespace.",
      schema_generator: || serde_json::to_value(schema_for!(ListDefinitionsRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "list_namespaces",
      description: "List all namespaces in the Calcit project. Calcit projects are organized into namespaces (similar to modules in other languages). Each namespace typically represents a logical grouping of related functions and can import from other namespaces.",
      schema_generator: || serde_json::to_value(schema_for!(ListNamespacesRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "get_package_name",
      description: "Get the package name of the current Calcit project. Calcit projects have a package name that identifies them, useful for understanding the project structure and dependencies.",
      schema_generator: || serde_json::to_value(schema_for!(GetPackageNameRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "read_namespace",
      description: "Read detailed information about a Calcit namespace, including its import rules and metadata. Calcit namespaces can import functions from other namespaces using import rules, and this tool shows the complete namespace configuration.",
      schema_generator: || serde_json::to_value(schema_for!(ReadNamespaceRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "read_definition",
      description: "Read the source code and documentation of a specific function or macro definition in Calcit. Calcit uses Cirru syntax (a Lisp-like notation with parentheses) and this tool returns the actual code structure.",
      schema_generator: || serde_json::to_value(schema_for!(ReadDefinitionRequest)).unwrap(),
    },
    // Namespace Management Operations
    McpToolWithSchema {
      name: "add_namespace",
      description: "Create a new namespace in the Calcit project. Namespaces in Calcit are like modules that group related functions together. Each namespace can have its own import rules to access functions from other namespaces.",
      schema_generator: || serde_json::to_value(schema_for!(AddNamespaceRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "delete_namespace",
      description: "Remove a namespace from the Calcit project. This will delete all functions and macros defined in that namespace. Use with caution as this operation cannot be undone.",
      schema_generator: || serde_json::to_value(schema_for!(DeleteNamespaceRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "update_namespace_imports",
      description: "Modify the import rules of a Calcit namespace. Import rules determine which functions from other namespaces are available in the current namespace. Calcit uses a flexible import system similar to Clojure.",
      schema_generator: || serde_json::to_value(schema_for!(UpdateNamespaceImportsRequest)).unwrap(),
    },
    // Function/Macro Definition Operations
    McpToolWithSchema {
      name: "add_definition",
      description: "Create a new function or macro definition in a Calcit namespace. Calcit functions are defined using Cirru syntax (Lisp-like with parentheses, but stripped outermost pair of parentheses). The code parameter should be a nested array representing the syntax tree structure.",
      schema_generator: || serde_json::to_value(schema_for!(AddDefinitionRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "delete_definition",
      description: "Remove a function or macro definition from a Calcit namespace. This permanently deletes the definition and cannot be undone. Make sure no other code depends on this definition.",
      schema_generator: || serde_json::to_value(schema_for!(DeleteDefinitionRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "overwrite_definition",
      description: "Completely overwrite an existing function or macro definition in Calcit. This replaces the entire definition with new code and documentation. The code parameter should be a nested array representing the syntax tree structure, not a flattened list of strings.",
      schema_generator: || serde_json::to_value(schema_for!(OverwriteDefinitionRequest)).unwrap(),
    },
  ]
}

// Request structures with JsonSchema derive for automatic schema generation
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListDefinitionsRequest {
  pub namespace: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetPackageNameRequest {
  // No parameters needed
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ReadNamespaceRequest {
  pub namespace: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ReadDefinitionRequest {
  pub namespace: String,
  pub definition: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AddNamespaceRequest {
  pub namespace: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeleteNamespaceRequest {
  pub namespace: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListNamespacesRequest {
  // No parameters needed
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateNamespaceImportsRequest {
  pub namespace: String,
  pub imports: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AddDefinitionRequest {
  pub namespace: String,
  pub definition: String,
  pub code: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeleteDefinitionRequest {
  pub namespace: String,
  pub definition: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct OverwriteDefinitionRequest {
  pub namespace: String,
  pub definition: String,
  pub code: serde_json::Value,
}

// Additional request structures (without JsonSchema for now as they're not in the main tool list)
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateDefinitionAtRequest {
  pub namespace: String,
  pub definition: String,
  pub coord: serde_json::Value,
  pub new_value: serde_json::Value,
  pub mode: String,
  #[serde(rename = "match")]
  pub match_content: serde_json::Value,
  pub value_type: String, // "leaf" for string values, "list" for array values
}

#[derive(Debug, Deserialize)]
pub struct ReadDefinitionAtRequest {
  pub namespace: String,
  pub definition: String,
  pub coord: serde_json::Value,
}

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

#[derive(Debug, Deserialize)]
pub struct ReadConfigsRequest {
  // No parameters needed
}

#[derive(Debug, Deserialize)]
pub struct UpdateConfigsRequest {
  pub init_fn: Option<String>,
  pub reload_fn: Option<String>,
  pub version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListDependencyDocsRequest {
  pub module_namespace: String,
}

#[derive(Debug, Deserialize)]
pub struct ReadDependencyDefinitionDocRequest {
  pub dependency_name: String,
  pub namespace: String,
  pub definition: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadDependencyModuleDocRequest {
  pub module_namespace: String,
  pub doc_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListApiDocsRequest {
  // No parameters needed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListGuidebookDocsRequest {
  // No parameters needed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCurrentModuleRequest {
  // No parameters needed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListModulesRequest {
  // No parameters needed
}

pub fn get_standard_mcp_tools() -> Vec<Tool> {
  get_mcp_tools_with_schema()
    .iter()
    .map(|tool| tool.to_standard_tool())
    .collect()
}
