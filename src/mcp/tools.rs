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
      description: "List all function and macro definitions in a Calcit namespace. Calcit organizes code in namespaces, where each namespace contains definitions (functions, macros, variables). This tool helps explore the structure of Calcit code by showing what's available in a specific namespace.\n\nExample: {\"namespace\": \"app.main\"}",
      schema_generator: || serde_json::to_value(schema_for!(ListDefinitionsRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "list_namespaces",
      description: "List all namespaces in the Calcit project. Calcit projects are organized into namespaces (similar to modules in other languages). Each namespace typically represents a logical grouping of related functions and can import from other namespaces.\n\nExample: {}",
      schema_generator: || serde_json::to_value(schema_for!(ListNamespacesRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "get_package_name",
      description: "Get the package name of the current Calcit project. Calcit projects have a package name that identifies them, useful for understanding the project structure and dependencies.\n\nExample: {}",
      schema_generator: || serde_json::to_value(schema_for!(GetPackageNameRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "read_namespace",
      description: "Read detailed information about a Calcit namespace, including its import rules and metadata. Calcit namespaces can import functions from other namespaces using import rules, and this tool shows the complete namespace configuration.\n\nExample: {\"namespace\": \"app.main\"}",
      schema_generator: || serde_json::to_value(schema_for!(ReadNamespaceRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "read_definition",
      description: "Read the source code and documentation of a specific function or macro definition in Calcit. Calcit uses Cirru syntax (a Lisp-like notation with parentheses) and this tool returns the actual code structure.\n\nExample: {\"namespace\": \"app.main\", \"definition\": \"main!\"}",
      schema_generator: || serde_json::to_value(schema_for!(ReadDefinitionRequest)).unwrap(),
    },
    // Namespace Management Operations
    McpToolWithSchema {
      name: "add_namespace",
      description: "Create a new namespace in the Calcit project. Namespaces in Calcit are like modules that group related functions together. Each namespace can have its own import rules to access functions from other namespaces.\n\nExample: {\"namespace\": \"app.new-module\"}",
      schema_generator: || serde_json::to_value(schema_for!(AddNamespaceRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "delete_namespace",
      description: "Remove a namespace from the Calcit project. This will delete all functions and macros defined in that namespace. Use with caution as this operation cannot be undone.\n\nExample: {\"namespace\": \"app.unused-module\"}",
      schema_generator: || serde_json::to_value(schema_for!(DeleteNamespaceRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "update_namespace_imports",
      description: "Modify the import rules of a Calcit namespace. Import rules determine which functions from other namespaces are available in the current namespace. Calcit uses a flexible import system similar to Clojure.\n\nExample: {\"namespace\": \"app.main\", \"imports\": [[\"app.lib\", \":refer\", [\"add\", \"minus\"]], [\"app.config\", \":as\", \"config\"]]}",
      schema_generator: || serde_json::to_value(schema_for!(UpdateNamespaceImportsRequest)).unwrap(),
    },
    // Function/Macro Definition Operations
    McpToolWithSchema {
      name: "add_definition",
      description: "Create a new function or macro definition in a Calcit namespace. Calcit functions are defined using Cirru syntax (Lisp-like with parentheses, but stripped outermost pair of parentheses). The code parameter should be a nested array representing the syntax tree structure.\n\nExample: {\"namespace\": \"app.main\", \"definition\": \"square\", \"code\": [\"fn\", [\"x\"], [\"*\", \"x\", \"x\"]]}",
      schema_generator: || serde_json::to_value(schema_for!(AddDefinitionRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "delete_definition",
      description: "Remove a function or macro definition from a Calcit namespace. This permanently deletes the definition and cannot be undone. Make sure no other code depends on this definition.\n\nExample: {\"namespace\": \"app.main\", \"definition\": \"unused-function\"}",
      schema_generator: || serde_json::to_value(schema_for!(DeleteDefinitionRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "overwrite_definition",
      description: "Completely overwrite an existing function or macro definition in Calcit. This replaces the entire definition with new code and documentation. The code parameter should be a nested array representing the syntax tree structure, not a flattened list of strings. Example: [\"fn\", [\"x\", \"y\"], [\"+\", \"x\", \"y\"]] for a function that adds two numbers. ⚠️ RECOMMENDATION: Avoid using this tool for most cases. Instead, use 'read_definition_at' first to understand the current structure, then use 'update_definition_at' for precise modifications.\n\nExample: {\"namespace\": \"app.main\", \"definition\": \"square\", \"code\": [\"fn\", [\"x\"], [\"*\", \"x\", \"x\"]]}",
      schema_generator: || serde_json::to_value(schema_for!(OverwriteDefinitionRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "update_definition_at",
      description: "Update a specific part of a function or macro definition using coordinate-based targeting with various operation modes. Cirru code is a tree structure that can be navigated using coordinate arrays (Vec<Int>). This tool allows precise updates to specific nodes in the code tree with validation. 💡 BEST PRACTICE: Always use 'read_definition_at' multiple times first to explore and understand the code structure, then generate correct 'match' and 'coord' parameters for safe updates. Empty coord [] operates on the root node.\n\nExample: {\"namespace\": \"app.main\", \"definition\": \"square\", \"coord\": [2, 1], \"new_value\": \"+\", \"mode\": \"replace\", \"match\": null, \"value_type\": \"leaf\"}",
      schema_generator: || serde_json::to_value(schema_for!(UpdateDefinitionAtRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "read_definition_at",
      description: "Read a specific part of a function or macro definition in Calcit. This allows for examining a particular location in the code tree without retrieving the entire definition.\n\nExample: {\"namespace\": \"app.main\", \"definition\": \"square\", \"coord\": [2, 1]}",
      schema_generator: || serde_json::to_value(schema_for!(ReadDefinitionAtRequest)).unwrap(),
    },
    // Module Management
    McpToolWithSchema {
      name: "list_modules",
      description: "List all modules in the Calcit project. Calcit projects can have multiple modules, each representing a separate compilation unit.\n\nExample: {}",
      schema_generator: || serde_json::to_value(schema_for!(ListModulesRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "get_current_module",
      description: "Get the currently active module in the Calcit project. This shows which module is being edited or compiled.\n\nExample: {}",
      schema_generator: || serde_json::to_value(schema_for!(GetCurrentModuleRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "switch_module",
      description: "Switch to a different module in the Calcit project. This changes the active module for editing and compilation.\n\nExample: {\"module\": \"app\"}",
      schema_generator: || serde_json::to_value(schema_for!(SwitchModuleRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "create_module",
      description: "Create a new module in the Calcit project. This adds a new compilation unit to the project.\n\nExample: {\"name\": \"new-module\"}",
      schema_generator: || serde_json::to_value(schema_for!(CreateModuleRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "delete_module",
      description: "Delete a module from the Calcit project. This removes a compilation unit from the project. Use with caution as this operation cannot be undone.\n\nExample: {\"module\": \"unused-module\"}",
      schema_generator: || serde_json::to_value(schema_for!(DeleteModuleRequest)).unwrap(),
    },
    // Cirru Syntax Tools
    McpToolWithSchema {
      name: "parse_cirru_to_json",
      description: "Parse Cirru syntax to JSON. Cirru is the syntax notation used by Calcit, and this tool converts it to a JSON structure for easier processing.\n\nExample: {\"cirru_code\": \"fn (x) (* x x)\"}",
      schema_generator: || serde_json::to_value(schema_for!(ParseCirruToJsonRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "format_json_to_cirru",
      description: "Format JSON data to Cirru syntax. This is the reverse of parse_cirru_to_json and converts a JSON structure to Cirru notation.\n\nExample: {\"json_data\": [\"fn\", [\"x\"], [\"*\", \"x\", \"x\"]]}",
      schema_generator: || serde_json::to_value(schema_for!(FormatJsonToCirruRequest)).unwrap(),
    },
    // Library and Utility Tools
    McpToolWithSchema {
      name: "fetch_calcit_libraries",
      description: "Fetch available Calcit libraries from the official registry at https://libs.calcit-lang.org/base.cirru. This helps discover official and community libraries that can be used in Calcit projects.\n\nExample: {}",
      schema_generator: || serde_json::to_value(schema_for!(FetchCalcitLibrariesRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "parse_cirru_edn_to_json",
      description: "Parse Cirru EDN format to simplified JSON. Cirru EDN is the data format used by Calcit for configuration and data storage. This tool converts it to standard JSON for easier processing.\n\nExample: {\"cirru_edn\": \"{}\"}",
      schema_generator: || serde_json::to_value(schema_for!(ParseCirruEdnToJsonRequest)).unwrap(),
    },
    // Documentation Tools
    McpToolWithSchema {
      name: "query_api_docs",
      description: "Query the API documentation for Calcit. This provides information about built-in functions, macros, and libraries in the Calcit language.\n\nExample: {\"query\": \"map\"}",
      schema_generator: || serde_json::to_value(schema_for!(QueryApiDocsRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "query_guidebook",
      description: "Query the guidebook for Calcit. The guidebook contains tutorials, examples, and best practices for using the Calcit language.\n\nExample: {\"query\": \"getting started\"}",
      schema_generator: || serde_json::to_value(schema_for!(QueryGuidebookRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "list_api_docs",
      description: "List all available API documentation topics for Calcit. This shows what documentation is available for reference.\n\nExample: {}",
      schema_generator: || serde_json::to_value(schema_for!(ListApiDocsRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "list_guidebook_docs",
      description: "List all available guidebook topics for Calcit. This shows what tutorials and examples are available for learning.\n\nExample: {}",
      schema_generator: || serde_json::to_value(schema_for!(ListGuidebookDocsRequest)).unwrap(),
    },
    // Configuration Management
    McpToolWithSchema {
      name: "read_configs",
      description: "Read the configuration settings for the Calcit project. This includes settings for initialization, reloading, and versioning.\n\nExample: {}",
      schema_generator: || serde_json::to_value(schema_for!(ReadConfigsRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "update_configs",
      description: "Update the configuration settings for the Calcit project. This allows changing settings for initialization, reloading, and versioning.\n\nExample: {\"init_fn\": \"app.main/main!\", \"reload_fn\": \"app.main/reload!\", \"version\": \"0.1.0\"}",
      schema_generator: || serde_json::to_value(schema_for!(UpdateConfigsRequest)).unwrap(),
    },
    // Dependency Documentation Tools
    McpToolWithSchema {
      name: "list_dependency_docs",
      description: "List documentation for dependencies in a Calcit module. This shows what documentation is available for the libraries used by the project.\n\nExample: {\"module_namespace\": \"app.main\"}",
      schema_generator: || serde_json::to_value(schema_for!(ListDependencyDocsRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "read_dependency_definition_doc",
      description: "Read documentation for a specific definition in a dependency. This provides detailed information about a function or macro from a library used by the project.\n\nExample: {\"dependency_name\": \"core\", \"namespace\": \"core.list\", \"definition\": \"map\"}",
      schema_generator: || serde_json::to_value(schema_for!(ReadDependencyDefinitionDocRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "read_dependency_module_doc",
      description: "Read documentation for a specific module in a dependency. This provides information about a module from a library used by the project.\n\nExample: {\"module_namespace\": \"core.list\", \"doc_path\": \"overview\"}",
      schema_generator: || serde_json::to_value(schema_for!(ReadDependencyModuleDocRequest)).unwrap(),
    },
  ]
}

// Request structures with JsonSchema derive for automatic schema generation
/// # List Definitions in Namespace
/// Retrieves all available function and variable definitions in the specified namespace.
///
/// Example: `{"namespace": "app.core"}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListDefinitionsRequest {
  /// # Namespace Path
  /// The full path of the namespace to query.
  ///
  /// Example: "app.core" or "lib.util"
  pub namespace: String,
}

/// # Get Package Name
/// Retrieves the name of the current project package.
/// No parameters required.
///
/// Example: `{}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetPackageNameRequest {
  // No parameters needed
}

/// # Read Namespace Information
/// Retrieves detailed information about the specified namespace, including import declarations and definition list.
///
/// Example: `{"namespace": "app.core"}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ReadNamespaceRequest {
  /// # Namespace Path
  /// The full path of the namespace to read.
  ///
  /// Example: "app.core" or "lib.util"
  pub namespace: String,
}

/// # Read Definition Content
/// Retrieves the complete content of a specific definition in the specified namespace.
///
/// Example: `{"namespace": "app.core", "definition": "add-numbers"}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ReadDefinitionRequest {
  /// # Namespace Path
  /// The full path of the namespace containing the definition.
  ///
  /// Example: "app.core" or "lib.util"
  pub namespace: String,
  /// # Definition Name
  /// The name of the function or variable to read.
  ///
  /// Example: "add-numbers" or "config-data"
  pub definition: String,
}

/// # Add New Namespace
/// Creates a new namespace for organizing related function and variable definitions.
/// Returns an error if the namespace already exists.
///
/// Example: `{"namespace": "app.new-module"}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AddNamespaceRequest {
  /// # Namespace Path
  /// The full path of the new namespace to create.
  ///
  /// Example: "app.core" or "lib.util"
  pub namespace: String,
}

/// # Delete Namespace
/// Removes the specified namespace and all its definitions.
/// This operation cannot be undone, use with caution.
///
/// Example: `{"namespace": "app.unused-module"}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeleteNamespaceRequest {
  /// # Namespace Path
  /// The full path of the namespace to delete.
  ///
  /// Example: "app.unused-module"
  pub namespace: String,
}

/// # List All Namespaces
/// Retrieves a list of all available namespaces in the current project.
/// No parameters required.
///
/// Example: `{}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListNamespacesRequest {
  // No parameters needed
}

/// # Update Namespace Imports
/// Updates the list of import declarations for the specified namespace.
/// This will replace all existing import declarations.
///
/// Example: `{"namespace": "app.core", "imports": [{"ns": "app.lib", "alias": "lib"}, {"ns": "app.util"}]}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateNamespaceImportsRequest {
  /// # Namespace Path
  /// The full path of the namespace to update imports for.
  ///
  /// Example: "app.core"
  pub namespace: String,
  /// # Import List
  /// The new list of import declarations, where each element is an import declaration object.
  ///
  /// Example: [{"ns": "app.lib", "alias": "lib"}, {"ns": "app.util"}]
  pub imports: Vec<serde_json::Value>,
}

/// # Add New Definition
/// Creates a new function or variable definition in the specified namespace.
/// Returns an error if the definition already exists.
///
/// Example: `{"namespace": "app.core", "definition": "add-numbers", "code": ["fn", ["a", "b"], ["+", "a", "b"]]}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AddDefinitionRequest {
  /// # Namespace Path
  /// The full path of the namespace where the definition will be added.
  ///
  /// Example: "app.core"
  pub namespace: String,
  /// # Definition Name
  /// The name of the new function or variable.
  ///
  /// Example: "add-numbers" or "config-data"
  pub definition: String,
  /// # Code Content
  /// The code tree for the definition, represented as a nested array.
  ///
  /// Example for a function: ["fn", ["a", "b"], ["+", "a", "b"]]
  /// Example for a variable: ["def", "x", "100"]
  pub code: serde_json::Value,
}

/// # Delete Definition
/// Removes a function or variable definition from the specified namespace.
/// This operation cannot be undone, use with caution.
///
/// Example: `{"namespace": "app.core", "definition": "unused-function"}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeleteDefinitionRequest {
  /// # Namespace Path
  /// The full path of the namespace containing the definition to delete.
  ///
  /// Example: "app.core"
  pub namespace: String,
  /// # Definition Name
  /// The name of the function or variable to delete.
  ///
  /// Example: "unused-function"
  pub definition: String,
}

/// # Completely Overwrite Definition Content
/// Replaces the entire content of an existing definition with a new code tree.
/// Note: This operation replaces the entire definition, use with caution.
/// It is recommended to first use read_definition to view the existing structure before making precise modifications.
///
/// Example: `{"namespace": "app.core", "definition": "add-numbers", "code": ["fn", ["x", "y"], ["+", "x", "y"]]}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct OverwriteDefinitionRequest {
  /// # Namespace Path
  /// The full path of the namespace containing the definition to overwrite.
  ///
  /// Example: "app.core"
  pub namespace: String,
  /// # Definition Name
  /// The name of the function or variable to overwrite.
  ///
  /// Example: "add-numbers"
  pub definition: String,
  /// # New Code
  /// The complete new code tree, represented as a nested array.
  ///
  /// Example: ["fn", ["x", "y"], ["+", "x", "y"]]
  pub code: serde_json::Value,
}

// Request structures for definition operations
/// # Update Definition at Specific Position
/// Precisely locates and updates a specific node in the Cirru code tree using a coordinate system.
/// Coordinates are an array representing the path indices from the root node to the target node.
/// For example: `[0, 1]` refers to the second child of the first child of the root node.
/// An empty coordinate `[]` refers to the root node itself.
///
/// Example: `{"namespace": "app.core", "definition": "add-numbers", "coord": [2, 1], "new_value": "*", "mode": "replace", "match": "+", "value_type": "leaf"}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateDefinitionAtRequest {
  /// # Namespace Path
  /// The full path of the namespace containing the definition to update.
  ///
  /// Example: "app.core"
  pub namespace: String,
  /// # Definition Name
  /// The name of the function or variable to update.
  ///
  /// Example: "add-numbers"
  pub definition: String,
  /// # Coordinate Position
  /// An array of integers representing the position of the node to update in the code tree.
  ///
  /// Example: [2, 1] refers to the second element of the third expression
  pub coord: serde_json::Value,
  /// # New Value
  /// The new content to replace with, can be a string or a nested array.
  ///
  /// Example: "*" or ["*", "a", "b"]
  pub new_value: serde_json::Value,
  /// # Update Mode
  /// Specifies how to apply the update, possible values: "replace", "before", "after".
  ///
  /// Example: "replace"
  pub mode: String,
  /// # Match Content
  /// Used to verify that the content at the current position matches expectations, increasing update safety.
  ///
  /// Example: "+" or ["a", "b"]
  #[serde(rename = "match")]
  pub match_content: serde_json::Value,
  /// # Value Type
  /// Specifies the type of the new value, "leaf" for string values, "list" for array values.
  ///
  /// Example: "leaf" or "list"
  pub value_type: String,
}

/// # Read Content at Specific Position in Definition
/// Precisely locates and reads a specific node in the Cirru code tree using a coordinate system.
/// Commonly used to check existing structures before updates, or to explore the code tree structure.
///
/// Example: `{"namespace": "app.core", "definition": "add-numbers", "coord": [2, 1]}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ReadDefinitionAtRequest {
  /// # Namespace Path
  /// The full path of the namespace containing the definition to read.
  ///
  /// Example: "app.core"
  pub namespace: String,
  /// # Definition Name
  /// The name of the function or variable to read.
  ///
  /// Example: "add-numbers"
  pub definition: String,
  /// # Coordinate Position
  /// An array of integers representing the position of the node to read in the code tree.
  /// For example: [0, 1] refers to the second element of the first expression.
  /// An empty coordinate [] means reading the entire definition.
  ///
  /// Example: [2, 1] or []
  pub coord: serde_json::Value,
}

/// # Switch Active Module
/// Changes the currently active module in the editor.
///
/// Example: `{"module": "app"}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SwitchModuleRequest {
  /// # Module Name
  /// The name of the module to switch to.
  ///
  /// Example: "app" or "lib"
  pub module: String,
}

/// # Create New Module
/// Creates a new module in the project.
///
/// Example: `{"name": "new-module"}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreateModuleRequest {
  /// # Module Name
  /// The name for the new module to create.
  ///
  /// Example: "new-module"
  pub name: String,
}

/// # Delete Module
/// Removes an existing module from the project.
/// This operation cannot be undone, use with caution.
///
/// Example: `{"module": "unused-module"}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeleteModuleRequest {
  /// # Module Name
  /// The name of the module to delete.
  ///
  /// Example: "unused-module"
  pub module: String,
}

/// # Parse Cirru Code to JSON
/// Converts Cirru syntax code into a JSON representation.
///
/// Example: `{"cirru_code": "(def a 1)"}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ParseCirruToJsonRequest {
  /// # Cirru Code
  /// The Cirru syntax code to be parsed into JSON.
  ///
  /// Example: "(def a 1)" or "(fn [x y] (+ x y))"
  pub cirru_code: String,
}

/// # Format JSON to Cirru Code
/// Converts a JSON representation back into Cirru syntax code.
///
/// Example: `{"json_data": ["def", "a", "1"]}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct FormatJsonToCirruRequest {
  /// # JSON Data
  /// The JSON representation to be formatted as Cirru code.
  ///
  /// Example: ["def", "a", "1"] or ["fn", ["x", "y"], ["+", "x", "y"]]
  pub json_data: serde_json::Value,
}

/// # Query API Documentation
/// Searches for API documentation based on the specified query type and value.
///
/// Example: `{"query_type": "function", "query_value": "map"}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct QueryApiDocsRequest {
  /// # Query Type
  /// The type of API documentation to search for.
  ///
  /// Example: "function", "macro", or "namespace"
  pub query_type: String,
  /// # Query Value
  /// The specific value to search for within the specified type.
  ///
  /// Example: "map" or "filter"
  pub query_value: Option<String>,
}

/// # Query Guidebook Documentation
/// Searches for guidebook documentation based on the specified query type and value.
///
/// Example: `{"query_type": "tutorial", "query_value": "getting-started"}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct QueryGuidebookRequest {
  /// # Query Type
  /// The type of guidebook documentation to search for.
  ///
  /// Example: "tutorial", "guide", or "example"
  pub query_type: String,
  /// # Query Value
  /// The specific value to search for within the specified type.
  ///
  /// Example: "getting-started" or "advanced-features"
  pub query_value: String,
}

/// # Read Project Configurations
/// Retrieves the current project configuration settings.
/// No parameters required.
///
/// Example: `{}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ReadConfigsRequest {
  // No parameters needed
}

/// # Update Project Configurations
/// Updates the project configuration settings with new values.
///
/// Example: `{"init_fn": "app.main/main!", "reload_fn": "app.main/reload!", "version": "0.7.3"}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateConfigsRequest {
  /// # Init Function
  /// The function to call when initializing the application.
  ///
  /// Example: "app.main/main!"
  pub init_fn: Option<String>,
  /// # Reload Function
  /// The function to call when reloading the application.
  ///
  /// Example: "app.main/reload!"
  pub reload_fn: Option<String>,
  /// # Version
  /// The version of the project.
  ///
  /// Example: "0.7.3"
  pub version: Option<String>,
}

/// # List Dependency Documentation
/// Retrieves documentation for dependencies in the specified module namespace.
///
/// Example: `{"module_namespace": "app.lib"}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListDependencyDocsRequest {
  /// # Module Namespace
  /// The namespace of the module to list dependency documentation for.
  ///
  /// Example: "app.lib"
  pub module_namespace: String,
}

/// # Read Dependency Definition Documentation
/// Retrieves documentation for a specific definition in a dependency.
///
/// Example: `{"dependency_name": "calcit.std", "namespace": "std.string", "definition": "trim"}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ReadDependencyDefinitionDocRequest {
  /// # Dependency Name
  /// The name of the dependency.
  ///
  /// Example: "calcit.std"
  pub dependency_name: String,
  /// # Namespace
  /// The namespace containing the definition.
  ///
  /// Example: "std.string"
  pub namespace: String,
  /// # Definition
  /// The name of the definition to read documentation for.
  ///
  /// Example: "trim"
  pub definition: String,
}

/// # Fetch Calcit Libraries
/// Retrieves a list of available Calcit libraries.
/// No parameters required.
///
/// Example: `{}`
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FetchCalcitLibrariesRequest {
  // No parameters needed
}

/// # Parse Cirru EDN to JSON
/// Converts Cirru EDN syntax into a JSON representation.
///
/// Example: `{"cirru_edn": "[] 1 2 3"}`
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ParseCirruEdnToJsonRequest {
  /// # Cirru EDN
  /// The Cirru EDN syntax to be parsed into JSON.
  ///
  /// Example: "[] 1 2 3" or "{} (:a 1) (:b 2)"
  pub cirru_edn: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadDependencyModuleDocRequest {
  pub module_namespace: String,
  pub doc_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListApiDocsRequest {
  // No parameters needed
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListGuidebookDocsRequest {
  // No parameters needed
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetCurrentModuleRequest {
  // No parameters needed
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListModulesRequest {
  // No parameters needed
}

pub fn get_standard_mcp_tools() -> Vec<Tool> {
  get_mcp_tools_with_schema().iter().map(|tool| tool.to_standard_tool()).collect()
}
