use super::jsonrpc::Tool;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
  pub parameters: HashMap<String, serde_json::Value>,
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
    
    // 读取操作 - Reading Operations
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
    // Namespace级别操作 - Namespace Management Operations
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
    // Definition级别操作 - Function/Macro Definition Operations
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
      name: "update_definition".to_string(),
      description: "Modify an existing function or macro definition in Calcit. You can update the code implementation, documentation, or both. The code must be provided in Cirru syntax format.".to_string(),
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
    // 模块管理操作 - Module Management Operations
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
    // Cirru 转换工具 - Cirru Syntax Conversion Tools
    McpTool {
      name: "parse_to_json".to_string(),
      description: "Parse Cirru syntax string into JSON structure. Cirru is the syntax format used by Calcit - it's like Lisp but uses indentation and spaces instead of parentheses. This tool converts human-readable Cirru code into a structured format that can be programmatically manipulated.".to_string(),
      parameters: vec![McpToolParameter {
        name: "cirru_code".to_string(),
        parameter_type: "string".to_string(),
        description: "Cirru syntax code as a string. Example: 'fn (x) (+ x 1)' which represents a function that adds 1 to its argument.".to_string(),
        optional: false,
      }],
    },
    McpTool {
      name: "format_from_json".to_string(),
      description: "Convert JSON structure back to readable Cirru syntax string. This is the reverse of parse_to_json - it takes structured data and formats it as human-readable Cirru code that can be saved to files or displayed to users.".to_string(),
      parameters: vec![McpToolParameter {
        name: "json_data".to_string(),
        parameter_type: "array".to_string(),
        description: "JSON structure representing Cirru code as nested arrays. Example: ['fn', ['x'], ['+', 'x', '1']] which will be formatted as readable Cirru syntax.".to_string(),
        optional: false,
      }],
    },
    
    // Calcit 文档和教程工具 - Calcit Documentation and Tutorial Tools
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
  ]
}

/// Get tools in standard MCP format
pub fn get_standard_mcp_tools() -> Vec<Tool> {
  get_mcp_tools().iter().map(mcp_tool_to_standard).collect()
}
