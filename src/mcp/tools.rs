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
    // NOTE: Namespace editing tools moved to CLI: `cr edit add-ns`, `cr edit delete-ns`, `cr edit update-imports`, `cr edit update-ns-doc`
    // ═══════════════════════════════════════════════════════════════════════════════
    // 🔷 PRIMARY TOOLS - Definition Reading Operations
    // ═══════════════════════════════════════════════════════════════════════════════
    // NOTE: Definition editing tools moved to CLI: `cr edit upsert-def`, `cr edit delete-def`, `cr edit operate-at`, `cr edit update-def-doc`
    McpToolWithSchema {
      name: "read_definition_at",
      description: "[PRIMARY] Read a specific part of a function or macro definition in Calcit. This allows for examining a particular location in the code tree without retrieving the entire definition.\n\n🚨 PARAMETER FORMAT REQUIREMENTS:\n• The 'coord' parameter MUST be a native JSON array of integers, NOT a string\n• Do NOT wrap the array in quotes\n• Index starts from 0 (zero-based indexing)\n\n✅ CORRECT FORMAT:\n{\"coord\": [2, 1]} or {\"coord\": []}\n\n❌ WRONG FORMATS:\n{\"coord\": \"[2, 1]\"} ← STRING (WRONG)\n{\"coord\": \"[]\"}← STRING (WRONG)\n\nExample: {\"namespace\": \"app.main\", \"definition\": \"square\", \"coord\": [2, 1]}",
      schema_generator: || serde_json::to_value(schema_for!(ReadDefinitionAtRequest)).unwrap(),
    },
    // NOTE: Module editing tools moved to CLI: `cr edit add-module`, `cr edit delete-module`
    // ═══════════════════════════════════════════════════════════════════════════════
    // 🔶 SECONDARY TOOLS - Module Management (read-only)
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
    // NOTE: Configuration editing tools moved to CLI: `cr edit set-config`
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

// NOTE: AddNamespaceRequest, DeleteNamespaceRequest moved to CLI: `cr edit add-ns`, `cr edit delete-ns`

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

// NOTE: UpdateNamespaceImportsRequest, UpsertDefinitionRequest, DeleteDefinitionRequest,
// OperateDefinitionAtRequest, OperateDefinitionAtWithLeafRequest moved to CLI:
// - `cr edit update-imports`
// - `cr edit upsert-def`
// - `cr edit delete-def`
// - `cr edit operate-at`

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

// NOTE: CreateModuleRequest, DeleteModuleRequest moved to CLI:
// - `cr edit add-module`
// - `cr edit delete-module`

// NOTE: ParseCirruToJsonRequest, FormatJsonToCirruRequest, QueryCalcitApisRequest,
// QueryCalcitReferenceRequest, ReadConfigsRequest moved to CLI commands.
// Use `cr cirru parse`, `cr cirru format`, `cr docs api`, `cr docs ref`, `cr query configs` instead.

// NOTE: UpdateConfigsRequest moved to CLI: `cr edit set-config`

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

// NOTE: UpdateDefinitionDocRequest, UpdateNamespaceDocRequest moved to CLI:
// - `cr edit update-def-doc`
// - `cr edit update-ns-doc`

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
