use super::jsonrpc::Tool;
use schemars::r#gen::SchemaGenerator as SchemarsGenerator;
use schemars::{
  JsonSchema,
  schema::{InstanceType, Schema, SchemaObject},
  schema_for,
};
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
      name: "list_namespace_definitions",
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
      description: "Create a new function or macro definition in a Calcit namespace. Calcit functions are defined using Cirru syntax (Lisp-like with parentheses, but stripped outermost pair of parentheses).\n\n🚨 PARAMETER FORMAT REQUIREMENTS:\n• The 'code' parameter MUST be a native JSON array, NOT a string\n• Do NOT wrap the array in quotes\n• Do NOT escape quotes inside the array\n\n✅ CORRECT FORMAT:\n{\"code\": [\"fn\", [\"x\"], [\"*\", \"x\", \"x\"]]}\n\n❌ WRONG FORMATS:\n{\"code\": \"[fn [x] [* x x]]\"} ← STRING (WRONG)\n{\"code\": \"[\\\"fn\\\", [\\\"x\\\"], [\\\"*\\\", \\\"x\\\", \\\"x\\\"]]\"} ← ESCAPED STRING (WRONG)\n\nExample: {\"namespace\": \"app.main\", \"definition\": \"square\", \"code\": [\"fn\", [\"x\"], [\"*\", \"x\", \"x\"]]}",
      schema_generator: || serde_json::to_value(schema_for!(AddDefinitionRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "delete_definition",
      description: "Remove a function or macro definition from a Calcit namespace. This permanently deletes the definition and cannot be undone. Make sure no other code depends on this definition.\n\nExample: {\"namespace\": \"app.main\", \"definition\": \"unused-function\"}",
      schema_generator: || serde_json::to_value(schema_for!(DeleteDefinitionRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "overwrite_definition",
      description: "Completely overwrite an existing function or macro definition in Calcit. This replaces the entire definition with new code and documentation.\n\n🚨 PARAMETER FORMAT REQUIREMENTS:\n• The 'code' parameter MUST be a native JSON array, NOT a string\n• Do NOT wrap the array in quotes\n• Do NOT escape quotes inside the array\n\n✅ CORRECT FORMAT:\n{\"code\": [\"defcomp\", \"my-comp\", [], [\"div\", {}, \"Hello\"]]}\n\n❌ WRONG FORMATS:\n{\"code\": \"[defcomp my-comp [] [div {} Hello]]\"} ← STRING (WRONG)\n{\"code\": \"[\\\"defcomp\\\", \\\"my-comp\\\"]\"} ← ESCAPED STRING (WRONG)\n\n⚠️ RECOMMENDATION: Avoid using this tool for most cases. Instead, use 'read_definition_at' first to understand the current structure, then use 'update_definition_at' for precise modifications.\n\nExample: {\"namespace\": \"app.main\", \"definition\": \"square\", \"code\": [\"fn\", [\"x\"], [\"*\", \"x\", \"x\"]]}",
      schema_generator: || serde_json::to_value(schema_for!(OverwriteDefinitionRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "update_definition_at",
      description: "Update a specific part of a function or macro definition using coordinate-based targeting with various operation modes. Cirru code is a tree structure that can be navigated using coordinate arrays (Vec<Int>). This tool allows precise updates to specific nodes in the code tree with validation.\n\n🚨 PARAMETER FORMAT REQUIREMENTS:\n• The 'coord' parameter MUST be a native JSON array of integers, NOT a string\n• Do NOT wrap the array in quotes\n\n✅ CORRECT FORMAT:\n{\"coord\": [2, 1]} or {\"coord\": []}\n\n❌ WRONG FORMATS:\n{\"coord\": \"[2, 1]\"} ← STRING (WRONG)\n{\"coord\": \"[]\"}← STRING (WRONG)\n\n💡 BEST PRACTICE: Always use 'read_definition_at' multiple times first to explore and understand the code structure, then generate correct 'match' and 'coord' parameters for safe updates. Empty coord [] operates on the root node.\n\nExample: {\"namespace\": \"app.main\", \"definition\": \"square\", \"coord\": [2, 1], \"new_value\": \"+\", \"mode\": \"replace\", \"match\": null, \"value_type\": \"leaf\"}",
      schema_generator: || serde_json::to_value(schema_for!(UpdateDefinitionAtRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "update_definition_at_with_leaf",
      description: "Update a specific part of a function or macro definition with a leaf value (string). This is a simplified version of update_definition_at specifically for replacing leaf nodes, eliminating the need for value_type parameter.\n\n🚨 PARAMETER FORMAT REQUIREMENTS:\n• The 'coord' parameter MUST be a native JSON array of integers, NOT a string\n• The 'new_value' parameter MUST be a string (leaf value)\n• Do NOT wrap the coord array in quotes\n\n✅ CORRECT FORMAT:\n{\"coord\": [2, 1]} or {\"coord\": []}\n\n❌ WRONG FORMATS:\n{\"coord\": \"[2, 1]\"} ← STRING (WRONG)\n{\"coord\": \"[]\"}← STRING (WRONG)\n\n💡 BEST PRACTICE: Use this tool when you need to replace a leaf node (symbol, string, number) with another leaf value. For complex expressions, use the general 'update_definition_at' tool.\n\nExample: {\"namespace\": \"app.main\", \"definition\": \"square\", \"coord\": [2, 1], \"new_value\": \"+\", \"mode\": \"replace\", \"match\": \"*\"}",
      schema_generator: || serde_json::to_value(schema_for!(UpdateDefinitionAtWithLeafRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "read_definition_at",
      description: "Read a specific part of a function or macro definition in Calcit. This allows for examining a particular location in the code tree without retrieving the entire definition.\n\n🚨 PARAMETER FORMAT REQUIREMENTS:\n• The 'coord' parameter MUST be a native JSON array of integers, NOT a string\n• Do NOT wrap the array in quotes\n\n✅ CORRECT FORMAT:\n{\"coord\": [2, 1]} or {\"coord\": []}\n\n❌ WRONG FORMATS:\n{\"coord\": \"[2, 1]\"} ← STRING (WRONG)\n{\"coord\": \"[]\"} ← STRING (WRONG)\n\nExample: {\"namespace\": \"app.main\", \"definition\": \"square\", \"coord\": [2, 1]}",
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
      name: "create_config_entry",
      description: "Create a new module in the Calcit project. This adds a new compilation unit to the project.\n\nExample: {\"name\": \"new-module\"}",
      schema_generator: || serde_json::to_value(schema_for!(CreateModuleRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "delete_config_entry",
      description: "Delete a module from the Calcit project. This removes a compilation unit from the project. Use with caution as this operation cannot be undone.\n\nExample: {\"module\": \"unused-module\"}",
      schema_generator: || serde_json::to_value(schema_for!(DeleteModuleRequest)).unwrap(),
    },
    // Cirru Syntax Tools
    McpToolWithSchema {
      name: "calcit_parse_cirru_to_json",
      description: "Parse Cirru syntax to JSON. Cirru is the syntax notation used by Calcit, and this tool converts it to a JSON structure for easier processing.\n\nExample: {\"cirru_code\": \"fn (x) (* x x)\"}",
      schema_generator: || serde_json::to_value(schema_for!(ParseCirruToJsonRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "calcit_format_json_to_cirru",
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
      name: "query_calcit_apis",
      description: "Query the API documentation for Calcit. This provides information about built-in functions, macros, and libraries in the Calcit language.\n\nExample: {\"query\": \"map\"}",
      schema_generator: || serde_json::to_value(schema_for!(QueryCalcitApisRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "query_calcit_reference",
      description: "Query the reference documentation for Calcit. The reference contains tutorials, examples, and best practices for using the Calcit language.\n\nExample: {\"query\": \"getting started\"}",
      schema_generator: || serde_json::to_value(schema_for!(QueryCalcitReferenceRequest)).unwrap(),
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
    // Calcit Runner Management Tools
    McpToolWithSchema {
      name: "start_calcit_runner",
      description: "Start a Calcit runner in background mode using `cr <filepath>` command(or `cr js <filepath>` for compiling to js). This launches the Calcit interpreter in service mode, collecting logs in a queue for later retrieval. Returns startup success/failure status.\n\nExample: {\"filename\": \"main.cirru\"}",
      schema_generator: || serde_json::to_value(schema_for!(StartCalcitRunnerRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "grab_calcit_runner_logs",
      description: "Grab logs from the running Calcit runner and clear the internal log queue. This retrieves accumulated logs and service status information, then empties the queue for fresh log collection.\n\nExample: {}",
      schema_generator: || serde_json::to_value(schema_for!(GrabCalcitRunnerLogsRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "stop_calcit_runner",
      description: "Stop the running Calcit runner and retrieve all remaining logs. This terminates the background service and returns any remaining log content from the queue.\n\nExample: {}",
      schema_generator: || serde_json::to_value(schema_for!(StopCalcitRunnerRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "generate_calcit_incremental",
      description: "Generate incremental file (.compact-inc.cirru) by comparing current source file with the .calcit-runner.cirru copy created when starting the runner. This creates a diff file that can be used by the Calcit runner to apply incremental updates. After generating the incremental file, check the runner logs to verify if the updates were applied successfully.\n\nExample: {\"source_file\": \"compact.cirru\"}",
      schema_generator: || serde_json::to_value(schema_for!(GenerateCalcitIncrementalRequest)).unwrap(),
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
  /// Example: "app.core" or "lib.util", should start with package root namespace
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
  #[schemars(with = "Vec<serde_json::Value>")]
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
  /// 🚨 CRITICAL FORMAT REQUIREMENTS:
  /// • MUST be a native JSON array, NOT a string
  /// • Do NOT wrap the array in quotes
  /// • Do NOT escape quotes inside the array
  ///
  /// ✅ CORRECT FORMATS:
  /// ["fn", ["a", "b"], ["+", "a", "b"]]
  /// ["def", "x", "100"]
  ///
  /// ❌ WRONG FORMATS:
  /// "[fn [a b] [+ a b]]" ← STRING (WRONG)
  /// "[\"fn\", [\"a\", \"b\"]]" ← ESCAPED STRING (WRONG)
  ///
  /// Example for a function: ["fn", ["a", "b"], ["+", "a", "b"]]
  /// Example for a variable: ["def", "x", "100"]
  #[schemars(with = "Vec<serde_json::Value>")]
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
/// It is recommended to first use `read_definition_at`` to view the existing structure before making precise modifications.
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
  /// 🚨 CRITICAL FORMAT REQUIREMENTS:
  /// • MUST be a native JSON array, NOT a string
  /// • Do NOT wrap the array in quotes
  /// • Do NOT escape quotes inside the array
  ///
  /// ✅ CORRECT FORMATS:
  /// ["defcomp", "my-comp", [], ["div", {}, "Hello"]]
  /// ["fn", ["x", "y"], ["+", "x", "y"]]
  ///
  /// ❌ WRONG FORMATS:
  /// "[defcomp my-comp [] [div {} Hello]]" ← STRING (WRONG)
  /// "[\"defcomp\", \"my-comp\"]" ← ESCAPED STRING (WRONG)
  ///
  /// Example: ["fn", ["x", "y"], ["+", "x", "y"]]
  #[schemars(with = "Vec<serde_json::Value>")]
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
  /// 🚨 CRITICAL FORMAT REQUIREMENTS:
  /// • MUST be a native JSON array of integers, NOT a string
  /// • Do NOT wrap the array in quotes
  ///
  /// ✅ CORRECT FORMATS:
  /// [2, 1] or []
  ///
  /// ❌ WRONG FORMATS:
  /// "[2, 1]" ← STRING (WRONG)
  /// "[]" ← STRING (WRONG)
  ///
  /// Example: [2, 1] refers to the second element of the third expression
  #[schemars(with = "Vec<i32>")]
  pub coord: serde_json::Value,
  /// # New Value
  /// The new content to replace with in Cirru format. Must be provided as a JSON array.
  ///
  /// Examples:
  /// - Simple leaf: ["+"] (single element array)
  /// - List: ["*", "a", "b"] (array)
  /// - Complex: ["fn", ["x"], ["*", "x", "x"]] (nested array)
  ///
  /// IMPORTANT: Always provide as an array, even for single values.
  #[schemars(with = "Vec<serde_json::Value>")]
  pub new_value: serde_json::Value,
  /// # Update Mode
  /// Specifies how to apply the update, possible values: "replace", "before", "after".
  ///
  /// Example: "replace"
  pub mode: String,
  /// # Match Content
  /// Used to verify that the content at the current position matches expectations, increasing update safety.
  /// Provide the expected content in Cirru format:
  /// - For leaf values: use a string like "+"
  /// - For list values: use an array like ["a", "b"]
  ///
  /// Examples:
  /// - Leaf: "+" (string)
  /// - List: ["a", "b"] (array)
  /// - Complex: ["fn", ["x"], ["*", "x", "x"]] (nested array)
  #[serde(rename = "match")]
  #[schemars(schema_with = "match_content_schema")]
  pub match_content: serde_json::Value,
}

/// # Update Definition at Specific Position with Leaf Value
/// A simplified version of update_definition_at specifically for updating leaf nodes (strings, symbols, numbers).
/// This eliminates the need for the value_type parameter since it's always "leaf".
/// Coordinates are an array representing the path indices from the root node to the target node.
/// For example: `[0, 1]` refers to the second child of the first child of the root node.
/// An empty coordinate `[]` refers to the root node itself.
///
/// Example: `{"namespace": "app.core", "definition": "add-numbers", "coord": [2, 1], "new_value": "*", "mode": "replace", "match": "+"}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateDefinitionAtWithLeafRequest {
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
  /// 🚨 CRITICAL FORMAT REQUIREMENTS:
  /// • MUST be a native JSON array of integers, NOT a string
  /// • Do NOT wrap the array in quotes
  ///
  /// ✅ CORRECT FORMATS:
  /// [2, 1] or []
  ///
  /// ❌ WRONG FORMATS:
  /// "[2, 1]" ← STRING (WRONG)
  /// "[]" ← STRING (WRONG)
  ///
  /// Example: [2, 1] refers to the second element of the third expression
  #[schemars(with = "Vec<i32>")]
  pub coord: serde_json::Value,
  /// # New Leaf Value
  /// The new leaf value to replace with. This must be a string representing a symbol, string literal, or number.
  ///
  /// Examples: "+", "my-variable", "42", "hello-world"
  pub new_value: String,
  /// # Update Mode
  /// Specifies how to apply the update, possible values: "replace", "before", "after", "delete".
  ///
  /// Example: "replace"
  pub mode: String,
  /// # Match Content
  /// Used to verify that the content at the current position matches expectations, increasing update safety.
  /// Provide the expected leaf value as a string.
  ///
  /// Examples: "+", "old-variable", "123"
  #[serde(rename = "match")]
  pub match_content: Option<String>,
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
  /// 🚨 CRITICAL FORMAT REQUIREMENTS:
  /// • MUST be a native JSON array of integers, NOT a string
  /// • Do NOT wrap the array in quotes
  ///
  /// ✅ CORRECT FORMATS:
  /// [2, 1] or []
  ///
  /// ❌ WRONG FORMATS:
  /// "[2, 1]" ← STRING (WRONG)
  /// "[]" ← STRING (WRONG)
  ///
  /// Example: [2, 1] or []
  #[schemars(with = "Vec<i32>")]
  pub coord: serde_json::Value,
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
  #[schemars(with = "Vec<serde_json::Value>")]
  pub json_data: serde_json::Value,
}

/// # Query API Documentation
/// Searches for API documentation based on the specified query type and value.
///
/// Example: `{"query_type": "function", "query_value": "map"}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct QueryCalcitApisRequest {
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
pub struct QueryCalcitReferenceRequest {
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

// Calcit Runner Management Tools

// CalcitRunnerMode validation function
pub fn validate_calcit_runner_mode(mode: &str) -> Result<(), String> {
  match mode.trim().to_lowercase().as_str() {
    "" | "run" | "js" => Ok(()),
    _ => Err(format!("Invalid mode '{mode}'. Only 'run', 'js', or empty string are allowed.")),
  }
}

/// Start a Calcit runner in background mode using `cr <filename>` command
/// Example: `{"filename": "compact.cirru"}` or `{"filename": "compact.cirru", "mode": "js"}`
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StartCalcitRunnerRequest {
  /// # Filename
  /// The Calcit file to run with `cr` command.
  ///
  /// Example: "main.cirru" or "test.cirru"
  pub filename: String,
  /// # Mode
  /// The mode to run the Calcit runner in. Defaults to "run" if not specified.
  /// Only accepts "run", "js", or empty string (which defaults to "run").
  ///
  /// Example: "run" or "js"
  #[serde(default)]
  pub mode: String,
}

/// Grab logs from the running Calcit runner and clear the internal log queue
/// Example: `{}`
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GrabCalcitRunnerLogsRequest {
  // No parameters needed
}

/// Stop the running Calcit runner and retrieve all remaining logs
/// Example: `{}`
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StopCalcitRunnerRequest {
  // No parameters needed
}

/// # Generate Calcit Incremental File
/// Generate incremental file (.compact-inc.cirru) by comparing current compact.cirru with the temporary copy.
/// This creates a diff file that can be used by the Calcit runner to apply incremental updates.
/// After generating the incremental file, check the runner logs to verify if the updates were applied successfully.
///
/// Example: `{"source_file": "compact.cirru"}`
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GenerateCalcitIncrementalRequest {
  /// Source file path (optional, defaults to "compact.cirru")
  pub source_file: Option<String>,
}

pub fn get_standard_mcp_tools() -> Vec<Tool> {
  get_mcp_tools_with_schema().iter().map(|tool| tool.to_standard_tool()).collect()
}

fn match_content_schema(_generator: &mut SchemarsGenerator) -> Schema {
  Schema::Object(SchemaObject {
    metadata: Some(Box::new(schemars::schema::Metadata {
      title: Some("Match Content".to_string()),
      description: Some("Used to verify that the content at the current position matches expectations, increasing update safety.\nProvide the expected content in Cirru format:\n- For leaf values: use a string like \"+\"\n- For list values: use an array like [\"a\", \"b\"]\n\nExamples:\n- Leaf: \"+\" (string)\n- List: [\"a\", \"b\"] (array)\n- Complex: [\"fn\", [\"x\"], [\"*\", \"x\", \"x\"]] (nested array)".to_string()),
      ..Default::default()
    })),
    subschemas: Some(Box::new(schemars::schema::SubschemaValidation {
      one_of: Some(vec![
        Schema::Object(SchemaObject {
          instance_type: Some(schemars::schema::SingleOrVec::Single(Box::new(InstanceType::String))),
          ..Default::default()
        }),
        Schema::Object(SchemaObject {
          instance_type: Some(schemars::schema::SingleOrVec::Single(Box::new(InstanceType::Array))),
          array: Some(Box::new(schemars::schema::ArrayValidation {
            items: Some(schemars::schema::SingleOrVec::Single(Box::new(Schema::Bool(true)))),
            ..Default::default()
          })),
          ..Default::default()
        })
      ]),
      ..Default::default()
    })),
    ..Default::default()
  })
}
