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
    // 读取操作
    McpTool {
      name: "list_definitions".to_string(),
      description: "List all definitions in a namespace".to_string(),
      parameters: vec![McpToolParameter {
        name: "namespace".to_string(),
        parameter_type: "string".to_string(),
        description: "The namespace to list definitions from".to_string(),
        optional: false,
      }],
    },
    McpTool {
      name: "list_namespaces".to_string(),
      description: "List all namespaces in the project".to_string(),
      parameters: vec![],
    },
    McpTool {
      name: "read_namespace".to_string(),
      description: "Read namespace information including imports".to_string(),
      parameters: vec![McpToolParameter {
        name: "namespace".to_string(),
        parameter_type: "string".to_string(),
        description: "The namespace to read".to_string(),
        optional: false,
      }],
    },
    McpTool {
      name: "read_definition".to_string(),
      description: "Read a specific definition from a namespace".to_string(),
      parameters: vec![
        McpToolParameter {
          name: "namespace".to_string(),
          parameter_type: "string".to_string(),
          description: "The namespace containing the definition".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "definition".to_string(),
          parameter_type: "string".to_string(),
          description: "The name of the definition to read".to_string(),
          optional: false,
        },
      ],
    },
    // Namespace级别操作
    McpTool {
      name: "add_namespace".to_string(),
      description: "Add a new namespace to the project".to_string(),
      parameters: vec![
        McpToolParameter {
          name: "namespace".to_string(),
          parameter_type: "string".to_string(),
          description: "The name of the new namespace".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "imports".to_string(),
          parameter_type: "string".to_string(),
          description: "Import rules for the namespace (optional)".to_string(),
          optional: true,
        },
      ],
    },
    McpTool {
      name: "delete_namespace".to_string(),
      description: "Delete a namespace from the project".to_string(),
      parameters: vec![McpToolParameter {
        name: "namespace".to_string(),
        parameter_type: "string".to_string(),
        description: "The namespace to delete".to_string(),
        optional: false,
      }],
    },
    McpTool {
      name: "update_namespace_imports".to_string(),
      description: "Update the import rules of a namespace".to_string(),
      parameters: vec![
        McpToolParameter {
          name: "namespace".to_string(),
          parameter_type: "string".to_string(),
          description: "The namespace to update".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "imports".to_string(),
          parameter_type: "string".to_string(),
          description: "New import rules".to_string(),
          optional: false,
        },
      ],
    },
    // Definition级别操作
    McpTool {
      name: "add_definition".to_string(),
      description: "Add a new definition to a namespace".to_string(),
      parameters: vec![
        McpToolParameter {
          name: "namespace".to_string(),
          parameter_type: "string".to_string(),
          description: "The namespace to add the definition to".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "definition".to_string(),
          parameter_type: "string".to_string(),
          description: "The name of the new definition".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "code".to_string(),
          parameter_type: "array".to_string(),
          description: "The code for the definition as Cirru recursive structure (array of strings/arrays)".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "doc".to_string(),
          parameter_type: "string".to_string(),
          description: "Documentation for the definition (optional)".to_string(),
          optional: true,
        },
      ],
    },
    McpTool {
      name: "delete_definition".to_string(),
      description: "Delete a definition from a namespace".to_string(),
      parameters: vec![
        McpToolParameter {
          name: "namespace".to_string(),
          parameter_type: "string".to_string(),
          description: "The namespace containing the definition".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "definition".to_string(),
          parameter_type: "string".to_string(),
          description: "The name of the definition to delete".to_string(),
          optional: false,
        },
      ],
    },
    McpTool {
      name: "update_definition".to_string(),
      description: "Update a definition's code or documentation".to_string(),
      parameters: vec![
        McpToolParameter {
          name: "namespace".to_string(),
          parameter_type: "string".to_string(),
          description: "The namespace containing the definition".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "definition".to_string(),
          parameter_type: "string".to_string(),
          description: "The name of the definition to update".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "code".to_string(),
          parameter_type: "array".to_string(),
          description: "New code for the definition as Cirru recursive structure (optional)".to_string(),
          optional: true,
        },
        McpToolParameter {
          name: "doc".to_string(),
          parameter_type: "string".to_string(),
          description: "New documentation for the definition (optional)".to_string(),
          optional: true,
        },
      ],
    },
    // 模块管理操作
    McpTool {
      name: "list_modules".to_string(),
      description: "List all available modules including current and dependencies".to_string(),
      parameters: vec![],
    },
    McpTool {
      name: "read_module".to_string(),
      description: "Read module information including package name and available namespaces".to_string(),
      parameters: vec![McpToolParameter {
        name: "module_path".to_string(),
        parameter_type: "string".to_string(),
        description: "The module path to read (empty for current module)".to_string(),
        optional: true,
      }],
    },
    McpTool {
      name: "add_module_dependency".to_string(),
      description: "Add a module dependency to the current project".to_string(),
      parameters: vec![McpToolParameter {
        name: "module_path".to_string(),
        parameter_type: "string".to_string(),
        description: "The path to the module to add as dependency".to_string(),
        optional: false,
      }],
    },
    McpTool {
      name: "remove_module_dependency".to_string(),
      description: "Remove a module dependency from the current project".to_string(),
      parameters: vec![McpToolParameter {
        name: "module_path".to_string(),
        parameter_type: "string".to_string(),
        description: "The path to the module to remove from dependencies".to_string(),
        optional: false,
      }],
    },
    McpTool {
      name: "clear_module_cache".to_string(),
      description: "Clear the module cache to force reload of all dependencies".to_string(),
      parameters: vec![],
    },
    // Cirru 转换工具
    McpTool {
      name: "parse_to_json".to_string(),
      description: "Parse Cirru syntax string to JSON recursive structure".to_string(),
      parameters: vec![McpToolParameter {
        name: "cirru_code".to_string(),
        parameter_type: "string".to_string(),
        description: "Cirru syntax code string to parse".to_string(),
        optional: false,
      }],
    },
    McpTool {
      name: "format_from_json".to_string(),
      description: "Format JSON recursive structure to Cirru syntax string".to_string(),
      parameters: vec![McpToolParameter {
        name: "json_structure".to_string(),
        parameter_type: "array".to_string(),
        description: "JSON recursive structure (array of strings/arrays) to format".to_string(),
        optional: false,
      }],
    },
  ]
}

/// Get tools in standard MCP format
pub fn get_standard_mcp_tools() -> Vec<Tool> {
  get_mcp_tools().iter().map(mcp_tool_to_standard).collect()
}
