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
    // ═══════════════════════════════════════════════════════════════════════════════
    // 🔷 PRIMARY TOOLS - Core operations for daily Calcit development
    // ═══════════════════════════════════════════════════════════════════════════════
    // Calcit Language Tools - Calcit is a functional programming language with Lisp-like syntax using Cirru notation
    // These tools help interact with Calcit projects, which organize code in namespaces containing function/macro definitions

    // Reading Operations (Primary)
    McpToolWithSchema {
      name: "list_namespace_definitions",
      description: "[PRIMARY] List all function and macro definitions in a Calcit namespace. Calcit organizes code in namespaces, where each namespace contains definitions (functions, macros, variables). This tool helps explore the structure of Calcit code by showing what's available in a specific namespace.\n\nExample: {\"namespace\": \"app.main\"}",
      schema_generator: || serde_json::to_value(schema_for!(ListDefinitionsRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "list_namespaces",
      description: "[PRIMARY] List all namespaces in the Calcit project. Calcit projects are organized into namespaces (similar to modules in other languages). Each namespace typically represents a logical grouping of related functions and can import from other namespaces. Optionally include dependency namespaces from external packages.\n\nExample: {\"include_dependency_namespaces\": false}",
      schema_generator: || serde_json::to_value(schema_for!(ListNamespacesRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "get_package_name",
      description: "[SECONDARY] Get the package name of the current Calcit project. Calcit projects have a package name that identifies them, useful for understanding the project structure and dependencies.\n\nExample: {}",
      schema_generator: || serde_json::to_value(schema_for!(GetPackageNameRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "read_namespace",
      description: "[PRIMARY] Read detailed information about a Calcit namespace, including its import rules and metadata. Calcit namespaces can import functions from other namespaces using import rules, and this tool shows the complete namespace configuration.\n\nExample: {\"namespace\": \"app.main\"}",
      schema_generator: || serde_json::to_value(schema_for!(ReadNamespaceRequest)).unwrap(),
    },
    // ═══════════════════════════════════════════════════════════════════════════════
    // 🔶 SECONDARY TOOLS - Namespace Management Operations (less frequently used)
    // ═══════════════════════════════════════════════════════════════════════════════
    McpToolWithSchema {
      name: "add_namespace",
      description: "[SECONDARY] Create a new namespace in the Calcit project. Namespaces in Calcit are like modules that group related functions together. Each namespace can have its own import rules to access functions from other namespaces.\n\nExample: {\"namespace\": \"app.new-module\"}",
      schema_generator: || serde_json::to_value(schema_for!(AddNamespaceRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "delete_namespace",
      description: "[SECONDARY] Remove a namespace from the Calcit project. This will delete all functions and macros defined in that namespace. Use with caution as this operation cannot be undone.\n\nExample: {\"namespace\": \"app.unused-module\"}",
      schema_generator: || serde_json::to_value(schema_for!(DeleteNamespaceRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "update_namespace_imports",
      description: "[SECONDARY] Modify the import rules of a Calcit namespace. Import rules determine which functions from other namespaces are available in the current namespace. Calcit uses a flexible import system similar to Clojure.\n\nExample: {\"namespace\": \"app.main\", \"imports\": [[\"app.lib\", \":refer\", [\"add\", \"minus\"]], [\"app.config\", \":as\", \"config\"]]}",
      schema_generator: || serde_json::to_value(schema_for!(UpdateNamespaceImportsRequest)).unwrap(),
    },
    // ═══════════════════════════════════════════════════════════════════════════════
    // 🔷 PRIMARY TOOLS - Function/Macro Definition Operations
    // ═══════════════════════════════════════════════════════════════════════════════
    McpToolWithSchema {
      name: "upsert_definition",
      description: "[PRIMARY] Create a new function or macro definition in a Calcit namespace, or completely overwrite an existing one. This unified tool combines the functionality of add_definition and overwrite_definition. Calcit functions are defined using Cirru syntax (Lisp-like with parentheses, but stripped outermost pair of parentheses).\n\n🚨 PARAMETER FORMAT REQUIREMENTS:\n• The 'syntax_tree' parameter MUST be a native JSON array, NOT a string\n• Do NOT wrap the array in quotes\n• Do NOT escape quotes inside the array\n\n✅ CORRECT FORMAT:\n{\"syntax_tree\": [\"fn\", [\"x\"], [\"*\", \"x\", \"x\"]]}\n\n❌ WRONG FORMATS:\n{\"syntax_tree\": \"[fn [x] [* x x]]\"}← STRING (WRONG)\n{\"syntax_tree\": \"[\\\"fn\\\", [\\\"x\\\"], [\\\"*\\\", \\\"x\\\", \\\"x\\\"]]\"}← ESCAPED STRING (WRONG)\n\n💡 BEHAVIOR:\n• When replacing=false: Creates a new definition (fails if definition already exists)\n• When replacing=true: Overwrites existing definition (fails if definition doesn't exist)\n\n⚠️ RECOMMENDATION: For existing definitions, consider using 'read_definition_at' first to understand the current structure, then use 'operate_definition_at' for precise modifications instead of complete replacement.\n\nExample: {\"namespace\": \"app.main\", \"definition\": \"square\", \"replacing\": false, \"syntax_tree\": [\"fn\", [\"x\"], [\"*\", \"x\", \"x\"]]}",
      schema_generator: || serde_json::to_value(schema_for!(UpsertDefinitionRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "delete_definition",
      description: "[SECONDARY] Remove a function or macro definition from a Calcit namespace. This permanently deletes the definition and cannot be undone. Make sure no other code depends on this definition.\n\nExample: {\"namespace\": \"app.main\", \"definition\": \"unused-function\"}",
      schema_generator: || serde_json::to_value(schema_for!(DeleteDefinitionRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "update_definition_doc",
      description: "[SECONDARY] Update the documentation for a specific definition in the current root namespace/package. This tool only works for definitions in the current project, not for dependencies.\n\nExample: {\"namespace\": \"app.core\", \"definition\": \"add-numbers\", \"doc\": \"Adds two numbers together\"}",
      schema_generator: || serde_json::to_value(schema_for!(UpdateDefinitionDocRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "update_namespace_doc",
      description: "[SECONDARY] Update the documentation for a specific namespace in the current root namespace/package. This tool only works for namespaces in the current project, not for dependencies.\n\nExample: {\"namespace\": \"app.core\", \"doc\": \"Core utilities for the application\"}",
      schema_generator: || serde_json::to_value(schema_for!(UpdateNamespaceDocRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "operate_definition_at",
      description: "[PRIMARY] Update a specific part of a function or macro definition using coordinate-based targeting with various operation modes. Cirru code is a tree structure that can be navigated using coordinate arrays (Vec<Int>). This tool allows precise updates to specific nodes in the code tree with validation.\n\n🚨 PARAMETER FORMAT REQUIREMENTS:\n• The 'coord' parameter MUST be a native JSON array of integers, NOT a string\n• Do NOT wrap the array in quotes\n• Index starts from 0 (zero-based indexing)\n\n✅ CORRECT FORMAT:\n{\"coord\": [2, 1]} or {\"coord\": []}\n\n❌ WRONG FORMATS:\n{\"coord\": \"[2, 1]\"} ← STRING (WRONG)\n{\"coord\": \"[]\"}← STRING (WRONG)\n\n💡 BEST PRACTICE: Always use 'read_definition_at' multiple times first to explore and understand the code structure, then generate correct 'shallow_check' and 'coord' parameters for safe updates. Empty coord [] operates on the root node.\n\nExample: {\"namespace\": \"app.main\", \"definition\": \"square\", \"coord\": [2, 1], \"new_value\": \"+\", \"operation\": \"replace\", \"shallow_check\": null, \"value_type\": \"leaf\"}",
      schema_generator: || serde_json::to_value(schema_for!(OperateDefinitionAtRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "operate_definition_at_with_leaf",
      description: "[PRIMARY] Update a specific part of a function or macro definition with a leaf value (string). This is a simplified version of operate_definition_at specifically for replacing leaf nodes, eliminating the need for value_type parameter.\n\n🚨 PARAMETER FORMAT REQUIREMENTS:\n• The 'coord' parameter MUST be a native JSON array of integers, NOT a string\n• The 'new_value' parameter MUST be a string (leaf value)\n• Do NOT wrap the coord array in quotes\n• Index starts from 0 (zero-based indexing)\n\n✅ CORRECT FORMAT:\n`{\"coord\": [2, 1]}` or `{\"coord\": []}`\n\n❌ WRONG FORMATS:\n{\"coord\": \"[2, 1]\"} ← STRING (WRONG)\n{\"coord\": \"[]\"}← STRING (WRONG)\n\n💡 BEST PRACTICE: Use this tool when you need to replace a leaf node (symbol, string, number) with another leaf value. For complex expressions, use the general 'operate_definition_at' tool.\n\nExample: {\"namespace\": \"app.main\", \"definition\": \"square\", \"coord\": [2, 1], \"new_value\": \"+\", \"operation\": \"replace\", \"match\": \"*\"}",
      schema_generator: || serde_json::to_value(schema_for!(OperateDefinitionAtWithLeafRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "read_definition_at",
      description: "[PRIMARY] Read a specific part of a function or macro definition in Calcit. This allows for examining a particular location in the code tree without retrieving the entire definition.\n\n🚨 PARAMETER FORMAT REQUIREMENTS:\n• The 'coord' parameter MUST be a native JSON array of integers, NOT a string\n• Do NOT wrap the array in quotes\n• Index starts from 0 (zero-based indexing)\n\n✅ CORRECT FORMAT:\n{\"coord\": [2, 1]} or {\"coord\": []}\n\n❌ WRONG FORMATS:\n{\"coord\": \"[2, 1]\"} ← STRING (WRONG)\n{\"coord\": \"[]\"}← STRING (WRONG)\n\nExample: {\"namespace\": \"app.main\", \"definition\": \"square\", \"coord\": [2, 1]}",
      schema_generator: || serde_json::to_value(schema_for!(ReadDefinitionAtRequest)).unwrap(),
    },
    // ═══════════════════════════════════════════════════════════════════════════════
    // 🔶 SECONDARY TOOLS - Module Management (advanced, less frequently used)
    // ═══════════════════════════════════════════════════════════════════════════════
    McpToolWithSchema {
      name: "list_modules",
      description: "[SECONDARY] List all modules in the Calcit project. Calcit projects can have multiple modules, each representing a separate compilation unit.\n\nExample: {}",
      schema_generator: || serde_json::to_value(schema_for!(ListModulesRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "get_current_module",
      description: "[SECONDARY] Get the currently active module in the Calcit project. This shows which module is being edited or compiled.\n\nExample: {}",
      schema_generator: || serde_json::to_value(schema_for!(GetCurrentModuleRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "create_config_entry",
      description: "[SECONDARY] Create a new module in the Calcit project. This adds a new compilation unit to the project.\n\nExample: {\"name\": \"new-module\"}",
      schema_generator: || serde_json::to_value(schema_for!(CreateModuleRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "delete_config_entry",
      description: "[SECONDARY] Delete a module from the Calcit project. This removes a compilation unit from the project. Use with caution as this operation cannot be undone.\n\nExample: {\"module\": \"unused-module\"}",
      schema_generator: || serde_json::to_value(schema_for!(DeleteModuleRequest)).unwrap(),
    },
    // ═══════════════════════════════════════════════════════════════════════════════
    // 🔶 SECONDARY TOOLS - Configuration Management
    // ═══════════════════════════════════════════════════════════════════════════════
    // NOTE: read_configs moved to CLI: `cr query configs`
    McpToolWithSchema {
      name: "update_configs",
      description: "[SECONDARY] Update the configuration settings for the Calcit project. This allows changing settings for initialization, reloading, and versioning.\n\nExample: {\"init_fn\": \"app.main/main!\", \"reload_fn\": \"app.main/reload!\", \"version\": \"0.1.0\"}",
      schema_generator: || serde_json::to_value(schema_for!(UpdateConfigsRequest)).unwrap(),
    },
    // ═══════════════════════════════════════════════════════════════════════════════
    // 🔷 PRIMARY TOOLS - Calcit Runner Management (essential for running/testing code)
    // ═══════════════════════════════════════════════════════════════════════════════
    McpToolWithSchema {
      name: "start_calcit_runner",
      description: "[PRIMARY] Start a Calcit runner in background mode using `cr <filepath>` command(or `cr <filepath> js` for compiling to js). This launches the Calcit interpreter in service mode, collecting logs in a queue for later retrieval. Returns startup success/failure status.\n\nExample: {\"filename\": \"main.cirru\"}",
      schema_generator: || serde_json::to_value(schema_for!(StartCalcitRunnerRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "grab_calcit_runner_logs",
      description: "[PRIMARY] Grab logs from the running Calcit runner and clear the internal log queue. This retrieves accumulated logs and service status information, then empties the queue for fresh log collection.\n\nExample: {}",
      schema_generator: || serde_json::to_value(schema_for!(GrabCalcitRunnerLogsRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "stop_calcit_runner",
      description: "[PRIMARY] Stop the running Calcit runner and retrieve all remaining logs. This terminates the background service and returns any remaining log content from the queue.\n\nExample: {}",
      schema_generator: || serde_json::to_value(schema_for!(StopCalcitRunnerRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "generate_calcit_incremental",
      description: "[PRIMARY] Generate incremental file (.compact-inc.cirru) by comparing current source file with the .calcit-runner.cirru copy created when starting the runner. This creates a diff file that can be used by the Calcit runner to apply incremental updates. After generating the incremental file, check the runner logs to verify if the updates were applied successfully.\n\nExample: {\"source_file\": \"compact.cirru\"}",
      schema_generator: || serde_json::to_value(schema_for!(GenerateCalcitIncrementalRequest)).unwrap(),
    },
    // NOTE: read_calcit_error_file moved to CLI: `cr query error`
    // ═══════════════════════════════════════════════════════════════════════════════
    // 🔶 SECONDARY TOOLS - Dependency Documentation Tools
    // ═══════════════════════════════════════════════════════════════════════════════
    McpToolWithSchema {
      name: "list_dependency_docs",
      description: "[SECONDARY] List documentation for dependencies in a Calcit module. This shows what documentation is available for the libraries used by the project.\n\nExample: {\"module_namespace\": \"app.main\"}",
      schema_generator: || serde_json::to_value(schema_for!(ListDependencyDocsRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "read_dependency_definition_doc",
      description: "[SECONDARY] Read documentation for a specific definition in a dependency. This provides detailed information about a function or macro from a library used by the project.\n\nExample: {\"dependency_name\": \"core\", \"namespace\": \"core.list\", \"definition\": \"map\"}",
      schema_generator: || serde_json::to_value(schema_for!(ReadDependencyDefinitionDocRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "read_dependency_module_doc",
      description: "[SECONDARY] Read documentation for a specific module in a dependency. This provides information about a module from a library used by the project.\n\nExample: {\"module_namespace\": \"core.list\", \"doc_path\": \"overview\"}",
      schema_generator: || serde_json::to_value(schema_for!(ReadDependencyModuleDocRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "read_definition_doc",
      description: "[SECONDARY] Read documentation for a specific definition (function/macro) in any namespace. This tool works for both current project definitions and dependency definitions. It automatically detects whether the namespace belongs to the current project or a dependency module.\n\nExample: {\"namespace\": \"app.core\", \"definition\": \"add-numbers\"}",
      schema_generator: || serde_json::to_value(schema_for!(ReadDefinitionDocRequest)).unwrap(),
    },
    // ═══════════════════════════════════════════════════════════════════════════════
    // 🔶 SECONDARY TOOLS - Memory Management Tools (optional, for learning)
    // ═══════════════════════════════════════════════════════════════════════════════
    McpToolWithSchema {
      name: "list_calcit_work_memory",
      description: "[SECONDARY] List all work memory entries with their keys and brief descriptions. This shows all the accumulated knowledge and tips stored by the model during Calcit development work.\n\nExample: {}",
      schema_generator: || serde_json::to_value(schema_for!(ListCalcitWorkMemoryRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "read_calcit_work_memory",
      description: "[SECONDARY] Read work memory entry by key or search by keywords. This allows retrieving specific knowledge or searching through accumulated tips and solutions.\n\nExample: {\"key\": \"error-handling-tips\"} or {\"keywords\": \"syntax error\"}",
      schema_generator: || serde_json::to_value(schema_for!(ReadCalcitWorkMemoryRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "write_calcit_work_memory",
      description: "[SECONDARY] Write or update a work memory entry. This allows storing new knowledge, tips, or solutions learned through trial and error to reduce future mistakes.\n\nExample: {\"key\": \"error-handling-tips\", \"content\": \"When encountering syntax errors, check parentheses balance first\"}",
      schema_generator: || serde_json::to_value(schema_for!(WriteCalcitWorkMemoryRequest)).unwrap(),
    },
    McpToolWithSchema {
      name: "feedback_to_calcit_mcp_server",
      description: "[SECONDARY] Provide feedback about MCP server usage and improvement suggestions. This creates a timestamped feedback file to help improve the server's functionality and reduce future issues.\n\nExample: {\"feedback\": \"The error messages could be more specific about which parenthesis is unmatched\"}",
      schema_generator: || serde_json::to_value(schema_for!(FeedbackToCalcitMcpServerRequest)).unwrap(),
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
/// Example: `{"include_dependency_namespaces": true}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListNamespacesRequest {
  /// Whether to include dependency namespaces in the result
  #[serde(default)]
  pub include_dependency_namespaces: bool,
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

/// # Upsert Definition (Add or Update)
/// Creates a new definition or updates an existing one in the specified namespace.
/// The `replacing` parameter controls whether to allow overwriting existing definitions.
///
/// 💡 **Recommendation for Updates**: For incremental modifications to existing definitions,
/// consider using `operate_definition_at` tool instead, which allows precise updates to specific
/// parts of the syntax tree without replacing the entire definition.
///
/// Example: `{"namespace": "app.core", "definition": "add-numbers", "syntax_tree": ["fn", ["a", "b"], ["+", "a", "b"]], "replacing": false}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpsertDefinitionRequest {
  /// # Namespace Path
  /// The full path of the namespace where the definition will be added or updated.
  /// This should be a valid namespace identifier following Calcit naming conventions.
  /// Namespace names typically use dot notation for hierarchical organization.
  ///
  /// Examples:
  /// - "app.core" - Main application logic
  /// - "app.util" - Utility functions
  /// - "lib.math" - Mathematical operations library
  pub namespace: String,

  /// # Definition Name
  /// The name of the function or variable to be created or updated.
  /// Must be a valid Calcit identifier.
  /// Function names typically use kebab-case convention.
  ///
  /// Examples:
  /// - "add-numbers" - A function that adds numbers
  /// - "config-data" - A configuration variable
  /// - "user-profile" - A data structure or function
  pub definition: String,

  /// # Allow Replacing Existing Definition
  /// Controls whether to allow overwriting an existing definition.
  /// - `false`: Only create new definitions (fails if definition already exists)
  /// - `true`: Allow overwriting existing definitions
  ///
  /// Examples:
  /// - `false` - Safe mode, prevents accidental overwrites
  /// - `true` - Update mode, allows replacing existing definitions
  pub replacing: bool,

  /// # Documentation String
  /// Optional documentation string for the definition.
  /// This will be stored as metadata and can be used for generating documentation.
  ///
  /// Examples:
  /// - "Adds two numbers together"
  /// - "Configuration data for the application"
  /// - "" (empty string for no documentation)
  #[serde(default)]
  pub doc: String,

  /// # Syntax Tree
  /// The complete syntax tree for the definition, represented as nested JSON arrays.
  /// This is the core structure that defines the behavior of your function or variable.
  ///
  /// 🚨 CRITICAL FORMAT REQUIREMENTS:
  /// • MUST be a native JSON array, NOT a string
  /// • Do NOT wrap the array in quotes
  /// • Do NOT escape quotes inside the array
  /// • Each element can be a string (symbol/literal) or another array (sub-expression)
  ///
  /// ✅ CORRECT FORMATS:
  /// Function definition: ["fn", ["a", "b"], ["+", "a", "b"]]
  /// Variable definition: ["def", "x", "100"]
  /// Component definition: ["defcomp", "button", ["props"], ["div", {}, "Click me"]]
  ///
  /// ❌ WRONG FORMATS:
  /// "[fn [a b] [+ a b]]" ← STRING (WRONG)
  /// "[\"fn\", [\"a\", \"b\"]]" ← ESCAPED STRING (WRONG)
  ///
  /// Structure explanation:
  /// - First element: definition type ("fn", "def", "defcomp", etc.)
  /// - Second element: name or parameters
  /// - Remaining elements: body/implementation
  ///
  /// Examples:
  /// - Function: ["fn", ["x", "y"], ["*", "x", "y"]]
  /// - Variable: ["def", "pi", "3.14159"]
  /// - Macro: ["defmacro", "when", ["condition", "&", "body"], ["if", "condition", ["do", "&", "body"]]]
  #[schemars(with = "Vec<serde_json::Value>")]
  pub syntax_tree: serde_json::Value,
}

// AddDefinitionRequest removed - use UpsertDefinitionRequest instead

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

// OverwriteDefinitionRequest removed - use UpsertDefinitionRequest instead

// Request structures for definition operations
/// # Update Definition at Specific Position
/// Precisely locates and updates a specific node in the Cirru code tree using a coordinate system.
/// Coordinates are an array representing the path indices from the root node to the target node.
/// For example: `[0, 1]` refers to the second child of the first child of the root node.
/// An empty coordinate `[]` refers to the root node itself.
/// `Delete` operation is also supported in this request.
///
/// Example: `{"namespace": "app.core", "definition": "add-numbers", "coord": [2, 1], "new_value": "*", "mode": "replace", "shallow_check": "+", "value_type": "leaf"}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct OperateDefinitionAtRequest {
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
  /// # Update Operation
  /// Specifies how to apply the update, possible values: "replace", "before", "after", "delete", "prepend", "append".
  ///
  /// Example: "replace"
  pub operation: String,
  /// # Shallow Check Content
  /// Used to verify that the content at the current position matches expectations, increasing update safety.
  /// You only need to provide the beginning part of the content for verification, not the complete structure.
  /// Provide the expected content in Cirru format:
  /// - For leaf values: use a string like "+"
  /// - For list values: use an array like ["a", "b"] or partial like ["fn", "..."]
  ///
  /// Examples:
  /// - Leaf: "+" (string)
  /// - List: ["a", "b"] (array)
  /// - Partial: ["fn", "..."] (beginning part with "..." indicating more content)
  #[serde(rename = "shallow_check")]
  #[schemars(schema_with = "shallow_check_schema")]
  pub shallow_check: serde_json::Value,
}

/// # Update Definition at Specific Position with Leaf Value
/// A simplified version of operate_definition_at specifically for updating leaf nodes (strings, symbols, numbers).
/// This eliminates the need for the value_type parameter since it's always "leaf".
/// Coordinates are an array representing the path indices from the root node to the target node.
/// For example: `[0, 1]` refers to the second child of the first child of the root node.
/// An empty coordinate `[]` refers to the root node itself.
///
/// Example: `{"namespace": "app.core", "definition": "add-numbers", "coord": [2, 1], "new_value": "*", "mode": "replace", "shallow_check": "+"}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct OperateDefinitionAtWithLeafRequest {
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
  /// # Update Operation
  /// Specifies how to apply the update, possible values: "replace", "before", "after", "delete".
  ///
  /// Example: "replace"
  pub operation: String,
  /// # Shallow Check Content
  /// Used to verify that the content at the current position matches expectations, increasing update safety.
  /// Provide the expected leaf value as a string. You only need to provide the beginning part for verification.
  ///
  /// Examples: "+", "old-variable", "123"
  #[serde(rename = "shallow_check")]
  pub shallow_check: Option<String>,
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

// NOTE: ParseCirruToJsonRequest, FormatJsonToCirruRequest, QueryCalcitApisRequest,
// QueryCalcitReferenceRequest, ReadConfigsRequest moved to CLI commands.
// Use `cr cirru parse`, `cr cirru format`, `cr docs api`, `cr docs ref`, `cr query configs` instead.

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

// NOTE: FetchCalcitLibrariesRequest, ParseCirruEdnToJsonRequest moved to CLI.
// Use `cr libs` and `cr cirru parse-edn` instead.

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadDependencyModuleDocRequest {
  pub module_namespace: String,
  pub doc_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadDefinitionDocRequest {
  /// The namespace containing the definition
  pub namespace: String,
  /// The name of the definition (function/macro) to read documentation for
  pub definition: String,
}

// NOTE: ListApiDocsRequest, ListGuidebookDocsRequest moved to CLI.
// Use `cr docs list-api` and `cr docs list-guide` instead.

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetCurrentModuleRequest {
  // No parameters needed
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListModulesRequest {
  // No parameters needed
}

/// # Update Definition Documentation
/// Updates the documentation for a specific definition in the current root namespace/package.
/// This tool only works for definitions in the current project, not for dependencies.
///
/// Example: `{"namespace": "app.core", "definition": "add-numbers", "doc": "Adds two numbers together"}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateDefinitionDocRequest {
  /// # Namespace Path
  /// The full path of the namespace containing the definition to update documentation for.
  /// Must be within the current root namespace/package.
  ///
  /// Example: "app.core"
  pub namespace: String,
  /// # Definition Name
  /// The name of the function or variable to update documentation for.
  ///
  /// Example: "add-numbers"
  pub definition: String,
  /// # Documentation
  /// The new documentation content for the definition.
  ///
  /// Example: "Adds two numbers together"
  pub doc: String,
}

/// # Update Namespace Documentation
/// Updates the documentation for a specific namespace in the current root namespace/package.
/// This tool only works for namespaces in the current project, not for dependencies.
///
/// Example: `{"namespace": "app.core", "doc": "Core utilities for the application"}`
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateNamespaceDocRequest {
  /// # Namespace Path
  /// The full path of the namespace to update documentation for.
  /// Must be within the current root namespace/package.
  ///
  /// Example: "app.core"
  pub namespace: String,
  /// # Documentation
  /// The new documentation content for the namespace.
  ///
  /// Example: "Core utilities for the application"
  pub doc: String,
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
  /// # Operation
  /// The operation to run the Calcit runner in. Defaults to "run" if not specified.
  /// Only accepts "run", "js", or empty string (which defaults to "run").
  ///
  /// Example: "run" or "js"
  #[serde(default)]
  pub operation: String,
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

// NOTE: ReadCalcitErrorFileRequest moved to CLI: `cr query error`

// Memory Management Tools

/// List all work memory entries with their keys and brief descriptions
/// Example: `{}`
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListCalcitWorkMemoryRequest {
  // No parameters needed
}

/// Read work memory entry by key or search by keywords
/// Example: `{"key": "error-handling-tips"}` or `{"keywords": "syntax error"}`
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadCalcitWorkMemoryRequest {
  /// # Memory Key
  /// The specific key to read from memory
  ///
  /// Example: "error-handling-tips"
  pub key: Option<String>,
  /// # Search Keywords
  /// Keywords to search for in memory entries
  ///
  /// Example: "syntax error"
  pub keywords: Option<String>,
}

/// Write or update a work memory entry
/// Example: `{"key": "error-handling-tips", "content": "When encountering syntax errors, check parentheses balance first"}`
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WriteCalcitWorkMemoryRequest {
  /// # Memory Key
  /// A short, meaningful identifier for this memory entry
  ///
  /// Example: "error-handling-tips"
  pub key: String,
  /// # Memory Content
  /// The content to store in this memory entry
  ///
  /// Example: "When encountering syntax errors, check parentheses balance first"
  pub content: String,
}

/// Provide feedback about MCP server usage and improvement suggestions
/// Example: `{"feedback": "The error messages could be more specific about which parenthesis is unmatched"}`
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FeedbackToCalcitMcpServerRequest {
  /// # Feedback Content
  /// Detailed feedback about issues encountered and suggestions for improvement
  ///
  /// Example: "The error messages could be more specific about which parenthesis is unmatched"
  pub feedback: String,
}

pub fn get_standard_mcp_tools() -> Vec<Tool> {
  get_mcp_tools_with_schema().iter().map(|tool| tool.to_standard_tool()).collect()
}

fn shallow_check_schema(_generator: &mut SchemarsGenerator) -> Schema {
  Schema::Object(SchemaObject {
    metadata: Some(Box::new(schemars::schema::Metadata {
        title: Some("Shallow Check Content".to_string()),
        description: Some("Used to verify that the content at the current position matches expectations, increasing update safety.\nYou only need to provide the beginning part of the content for verification, not the complete structure.\nProvide the expected content in Cirru format:\n- For leaf values: use a string like \"+\"\n- For list values: use an array like [\"a\", \"b\"] or partial like [\"fn\", \"...\"]\n\nExamples:\n- Leaf: \"+\" (string)\n- List: [\"a\", \"b\"] (array)\n- Partial: [\"fn\", \"...\"] (beginning part with \"...\" indicating more content)".to_string()),
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
