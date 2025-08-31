use actix_web::{App, HttpResponse, HttpServer, Responder, get, post, web};
use argh::FromArgs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use cirru_parser::Cirru;

use calcit::snapshot::{self, Snapshot};

/// MCP server for Calcit
#[derive(FromArgs)]
struct Args {
  /// path to compact.cirru file
  #[argh(option, short = 'f', default = "String::from(\"compact.cirru\")")]
  file: String,

  /// port to bind the server
  #[argh(option, short = 'p', default = "8080")]
  port: u16,
}

// Global state to hold the compact.cirru file path
struct AppState {
  compact_cirru_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct McpTool {
  name: String,
  description: String,
  parameters: Vec<McpToolParameter>,
}

#[derive(Debug, Serialize, Deserialize)]
struct McpToolParameter {
  name: String,
  #[serde(rename = "type")]
  parameter_type: String,
  description: String,
  optional: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct McpRequest {
  tool_name: String,
  parameters: HashMap<String, serde_json::Value>,
}

#[get("/mcp/discover")]
async fn discover() -> impl Responder {
  let tools = vec![
    // 读取操作
    McpTool {
      name: "list_definitions".to_string(),
      description: "List all definitions in a namespace".to_string(),
      parameters: vec![
        McpToolParameter {
          name: "namespace".to_string(),
          parameter_type: "string".to_string(),
          description: "The namespace to list definitions from".to_string(),
          optional: false,
        },
        McpToolParameter {
          name: "include_docs".to_string(),
          parameter_type: "boolean".to_string(),
          description: "Whether to include documentation for each definition".to_string(),
          optional: true,
        },
      ],
    },
    McpTool {
      name: "read_namespace".to_string(),
      description: "Read the namespace definition and import rules".to_string(),
      parameters: vec![McpToolParameter {
        name: "namespace".to_string(),
        parameter_type: "string".to_string(),
        description: "The namespace to read".to_string(),
        optional: false,
      }],
    },
    McpTool {
      name: "read_definition".to_string(),
      description: "Read a specific definition with its documentation and code".to_string(),
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
          parameter_type: "string".to_string(),
          description: "The code for the definition".to_string(),
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
          parameter_type: "string".to_string(),
          description: "New code for the definition (optional)".to_string(),
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
  ];
  HttpResponse::Ok().json(tools)
}

#[post("/mcp/execute")]
async fn execute(data: web::Data<AppState>, req: web::Json<McpRequest>) -> impl Responder {
  match req.tool_name.as_str() {
    // 读取操作
    "list_definitions" => list_definitions(&data, req.into_inner()),
    "read_namespace" => read_namespace(&data, req.into_inner()),
    "read_definition" => read_definition(&data, req.into_inner()),
    // Namespace级别操作
    "add_namespace" => add_namespace(&data, req.into_inner()),
    "delete_namespace" => delete_namespace(&data, req.into_inner()),
    "update_namespace_imports" => update_namespace_imports(&data, req.into_inner()),
    // Definition级别操作
    "add_definition" => add_definition(&data, req.into_inner()),
    "delete_definition" => delete_definition(&data, req.into_inner()),
    "update_definition" => update_definition(&data, req.into_inner()),
    _ => HttpResponse::BadRequest().json("Unknown tool"),
  }
}

// 辅助函数：加载snapshot
fn load_snapshot(app_state: &AppState) -> Result<Snapshot, HttpResponse> {
  let compact_cirru_path = &app_state.compact_cirru_path;
  let content = match std::fs::read_to_string(compact_cirru_path) {
    Ok(c) => c,
    Err(e) => return Err(HttpResponse::InternalServerError().body(format!("Failed to read compact.cirru: {e}"))),
  };

  let edn_data = match cirru_edn::parse(&content) {
    Ok(d) => d,
    Err(e) => return Err(HttpResponse::InternalServerError().body(format!("Failed to parse compact.cirru as EDN: {e}"))),
  };

  let snapshot: Snapshot = match snapshot::load_snapshot_data(&edn_data, compact_cirru_path) {
    Ok(s) => s,
    Err(e) => return Err(HttpResponse::InternalServerError().body(format!("Failed to load snapshot: {e}"))),
  };

  Ok(snapshot)
}

// 辅助函数：保存snapshot
fn save_snapshot(app_state: &AppState, snapshot: &Snapshot) -> Result<(), HttpResponse> {
  let compact_cirru_path = &app_state.compact_cirru_path;
  
  // 构建Edn结构
  let mut edn_map = cirru_edn::EdnMapView::default();
  edn_map.insert_key("package", cirru_edn::Edn::Str(snapshot.package.as_str().into()));
  
  // 构建configs
  let mut configs_map = cirru_edn::EdnMapView::default();
  configs_map.insert_key("init-fn", cirru_edn::Edn::Str(snapshot.configs.init_fn.as_str().into()));
  configs_map.insert_key("reload-fn", cirru_edn::Edn::Str(snapshot.configs.reload_fn.as_str().into()));
  configs_map.insert_key("version", cirru_edn::Edn::Str(snapshot.configs.version.as_str().into()));
  configs_map.insert_key("modules", cirru_edn::Edn::from(snapshot.configs.modules.iter().map(|s| cirru_edn::Edn::Str(s.as_str().into())).collect::<Vec<_>>()));
  edn_map.insert_key("configs", configs_map.into());
  
  // 构建entries
  let mut entries_map = cirru_edn::EdnMapView::default();
  for (k, v) in &snapshot.entries {
    let mut entry_map = cirru_edn::EdnMapView::default();
    entry_map.insert_key("init-fn", cirru_edn::Edn::Str(v.init_fn.as_str().into()));
    entry_map.insert_key("reload-fn", cirru_edn::Edn::Str(v.reload_fn.as_str().into()));
    entry_map.insert_key("version", cirru_edn::Edn::Str(v.version.as_str().into()));
    entry_map.insert_key("modules", cirru_edn::Edn::from(v.modules.iter().map(|s| cirru_edn::Edn::Str(s.as_str().into())).collect::<Vec<_>>()));
    entries_map.insert_key(k.as_str(), entry_map.into());
  }
  edn_map.insert_key("entries", entries_map.into());
  
  // 构建files
  let mut files_map = cirru_edn::EdnMapView::default();
  for (k, v) in &snapshot.files {
    files_map.insert_key(k.as_str(), cirru_edn::Edn::from(v));
  }
  edn_map.insert_key("files", files_map.into());
  
  let edn_data = cirru_edn::Edn::from(edn_map);
  
  // 将Edn格式化为Cirru字符串
  let content = match cirru_edn::format(&edn_data, false) {
    Ok(c) => c,
    Err(e) => return Err(HttpResponse::InternalServerError().body(format!("Failed to format snapshot as Cirru: {e}"))),
  };
  
  // 写入文件
  match std::fs::write(compact_cirru_path, content) {
    Ok(_) => Ok(()),
    Err(e) => Err(HttpResponse::InternalServerError().body(format!("Failed to write compact.cirru: {e}"))),
  }
}

// 读取操作函数
fn list_definitions(app_state: &AppState, req: McpRequest) -> HttpResponse {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("namespace parameter is missing or not a string"),
  };

  let include_docs = req.parameters.get("include_docs")
    .and_then(|v| v.as_bool())
    .unwrap_or(false);

  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => return e,
  };

  match snapshot.files.get(&namespace) {
    Some(file_data) => {
      let mut definitions = Vec::new();
      for (def_name, def_entry) in &file_data.defs {
        if include_docs {
          definitions.push(serde_json::json!({
            "name": def_name,
            "doc": def_entry.doc
          }));
        } else {
          definitions.push(serde_json::json!({
            "name": def_name
          }));
        }
      }
      HttpResponse::Ok().json(definitions)
    },
    None => HttpResponse::NotFound().body(format!("Namespace '{namespace}' not found")),
  }
}

fn read_namespace(app_state: &AppState, req: McpRequest) -> HttpResponse {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("namespace parameter is missing or not a string"),
  };

  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => return e,
  };

  match snapshot.files.get(&namespace) {
    Some(file_data) => {
      HttpResponse::Ok().json(serde_json::json!({
        "namespace": namespace,
        "ns_definition": file_data.ns,
      }))
    },
    None => HttpResponse::NotFound().body(format!("Namespace '{namespace}' not found")),
  }
}

fn read_definition(app_state: &AppState, req: McpRequest) -> HttpResponse {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("namespace parameter is missing or not a string"),
  };

  let definition = match req.parameters.get("definition") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("definition parameter is missing or not a string"),
  };

  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => return e,
  };

  match snapshot.files.get(&namespace) {
    Some(file_data) => {
      match file_data.defs.get(&definition) {
        Some(def_entry) => {
          HttpResponse::Ok().json(serde_json::json!({
            "namespace": namespace,
            "definition": definition,
            "doc": def_entry.doc,
            "code": def_entry.code
          }))
        },
        None => HttpResponse::NotFound().body(format!("Definition '{definition}' not found in namespace '{namespace}'"))
      }
    },
    None => HttpResponse::NotFound().body(format!("Namespace '{namespace}' not found")),
  }
}

// Namespace级别操作函数
fn add_namespace(app_state: &AppState, req: McpRequest) -> HttpResponse {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("namespace parameter is missing or not a string"),
  };

  let imports = req.parameters.get("imports")
    .and_then(|v| v.as_str())
    .unwrap_or("");

  // 加载当前snapshot
  let mut snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => return e,
  };

  // 检查namespace是否已存在
  if snapshot.files.contains_key(&namespace) {
    return HttpResponse::Conflict().body(format!("Namespace '{namespace}' already exists"));
  }

  // 创建新的namespace
  use calcit::snapshot::{FileInSnapShot, CodeEntry};
  
  // 创建namespace的ns定义
  let ns_code = if imports.is_empty() {
    Cirru::List(vec![
      Cirru::leaf("ns"),
      Cirru::leaf(namespace.clone()),
    ])
  } else {
    // 解析imports字符串并创建imports列表
    let import_list: Vec<Cirru> = imports.split_whitespace()
      .map(|s| Cirru::leaf(s))
      .collect();
    
    Cirru::List(vec![
      Cirru::leaf("ns"),
      Cirru::leaf(namespace.clone()),
      Cirru::List(vec![
        Cirru::leaf(":require"),
        Cirru::List(import_list),
      ]),
    ])
  };
  
  let ns_entry = CodeEntry::from_code(ns_code);
  let file_entry = FileInSnapShot {
    ns: ns_entry,
    defs: std::collections::HashMap::new(),
  };
  
  // 添加到snapshot
  snapshot.files.insert(namespace.clone(), file_entry);
  
  // 保存snapshot
  match save_snapshot(app_state, &snapshot) {
    Ok(_) => HttpResponse::Ok().json(serde_json::json!({
      "message": format!("Namespace '{namespace}' added successfully"),
      "namespace": namespace
    })),
    Err(e) => e,
  }
}

fn delete_namespace(app_state: &AppState, req: McpRequest) -> HttpResponse {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("namespace parameter is missing or not a string"),
  };

  // 加载当前快照
  let mut snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => return e,
  };

  // 检查namespace是否存在
  if !snapshot.files.contains_key(&namespace) {
    return HttpResponse::NotFound().json(serde_json::json!({
      "error": format!("Namespace '{namespace}' not found")
    }));
  }

  // 删除namespace
  snapshot.files.remove(&namespace);

  // 保存更新后的快照
  match save_snapshot(app_state, &snapshot) {
     Ok(_) => HttpResponse::Ok().json(serde_json::json!({
       "message": format!("Namespace '{namespace}' deleted successfully"),
       "namespace": namespace
     })),
     Err(e) => e,
   }
}

fn update_namespace_imports(app_state: &AppState, req: McpRequest) -> HttpResponse {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("namespace parameter is missing or not a string"),
  };

  let imports = match req.parameters.get("imports") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("imports parameter is missing or not a string"),
  };

  // 加载当前快照
  let mut snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => return e,
  };

  // 检查namespace是否存在
  if !snapshot.files.contains_key(&namespace) {
    return HttpResponse::NotFound().json(serde_json::json!({
      "error": format!("Namespace '{namespace}' not found")
    }));
  }

  // 获取现有的namespace文件
  let file_in_snapshot = snapshot.files.get_mut(&namespace).unwrap();
  
  // 查找ns定义并更新imports
  if let Some(ns_entry) = file_in_snapshot.defs.get_mut("ns") {
    // 创建新的ns定义，包含更新的imports
    let ns_code = if imports.is_empty() {
      Cirru::List(vec![
        Cirru::leaf("ns"),
        Cirru::leaf(namespace.clone()),
      ])
    } else {
      // 解析imports字符串并创建imports列表
      let import_list: Vec<Cirru> = imports.split_whitespace()
        .map(|s| Cirru::leaf(s))
        .collect();
      
      Cirru::List(vec![
        Cirru::leaf("ns"),
        Cirru::leaf(namespace.clone()),
        Cirru::List(vec![
          Cirru::leaf(":require"),
          Cirru::List(import_list),
        ]),
      ])
    };
    
    // 更新ns定义的代码
    ns_entry.code = ns_code;
  } else {
    return HttpResponse::BadRequest().json(serde_json::json!({
      "error": format!("Namespace '{namespace}' does not have a valid ns definition")
    }));
  }

  // 保存更新后的快照
  match save_snapshot(app_state, &snapshot) {
    Ok(_) => HttpResponse::Ok().json(serde_json::json!({
      "message": format!("Namespace '{namespace}' imports updated successfully"),
      "namespace": namespace,
      "imports": imports
    })),
    Err(e) => e,
  }
}

// Definition级别操作函数
fn add_definition(app_state: &AppState, req: McpRequest) -> HttpResponse {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("namespace parameter is missing or not a string"),
  };

  let definition = match req.parameters.get("definition") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("definition parameter is missing or not a string"),
  };

  let code = match req.parameters.get("code") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("code parameter is missing or not a string"),
  };

  let doc = req.parameters.get("doc")
    .and_then(|v| v.as_str())
    .unwrap_or("");

  // 加载当前快照
  let mut snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => return e,
  };

  // 检查namespace是否存在
  if !snapshot.files.contains_key(&namespace) {
    return HttpResponse::NotFound().json(serde_json::json!({
      "error": format!("Namespace '{namespace}' not found")
    }));
  }

  // 获取namespace文件
  let file_in_snapshot = snapshot.files.get_mut(&namespace).unwrap();
  
  // 检查definition是否已存在
  if file_in_snapshot.defs.contains_key(&definition) {
    return HttpResponse::Conflict().json(serde_json::json!({
      "error": format!("Definition '{definition}' already exists in namespace '{namespace}'")
    }));
  }

  // 解析代码字符串为Cirru结构
  let parsed_code = match cirru_parser::parse(&code) {
    Ok(cirru_list) => {
      if cirru_list.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
          "error": "Code cannot be empty"
        }));
      }
      cirru_list[0].clone()
    },
    Err(e) => {
      return HttpResponse::BadRequest().json(serde_json::json!({
        "error": format!("Failed to parse code: {}", e)
      }));
    }
  };

  // 创建新的CodeEntry
  use calcit::snapshot::CodeEntry;
  let new_entry = CodeEntry {
    code: parsed_code,
    doc: doc.to_string(),
  };

  // 添加definition到namespace
  file_in_snapshot.defs.insert(definition.clone(), new_entry);

  // 保存更新后的快照
  match save_snapshot(app_state, &snapshot) {
    Ok(_) => HttpResponse::Ok().json(serde_json::json!({
      "message": format!("Definition '{definition}' added successfully to namespace '{namespace}'"),
      "namespace": namespace,
      "definition": definition
    })),
    Err(e) => e,
  }
}

fn delete_definition(app_state: &AppState, req: McpRequest) -> HttpResponse {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("namespace parameter is missing or not a string"),
  };

  let definition = match req.parameters.get("definition") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("definition parameter is missing or not a string"),
  };

  // 加载当前快照
  let mut snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => return e,
  };

  // 检查namespace是否存在
  if !snapshot.files.contains_key(&namespace) {
    return HttpResponse::NotFound().json(serde_json::json!({
      "error": format!("Namespace '{namespace}' not found")
    }));
  }

  // 获取namespace文件
  let file_in_snapshot = snapshot.files.get_mut(&namespace).unwrap();
  
  // 检查definition是否存在
  if !file_in_snapshot.defs.contains_key(&definition) {
    return HttpResponse::NotFound().json(serde_json::json!({
      "error": format!("Definition '{definition}' not found in namespace '{namespace}'")
    }));
  }

  // 删除definition
  file_in_snapshot.defs.remove(&definition);

  // 保存更新后的快照
  match save_snapshot(app_state, &snapshot) {
    Ok(_) => HttpResponse::Ok().json(serde_json::json!({
      "message": format!("Definition '{definition}' deleted successfully from namespace '{namespace}'"),
      "namespace": namespace,
      "definition": definition
    })),
    Err(e) => e,
  }
}

fn update_definition(app_state: &AppState, req: McpRequest) -> HttpResponse {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("namespace parameter is missing or not a string"),
  };

  let definition = match req.parameters.get("definition") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("definition parameter is missing or not a string"),
  };

  let code = req.parameters.get("code")
    .and_then(|v| v.as_str());

  let doc = req.parameters.get("doc")
    .and_then(|v| v.as_str());

  if code.is_none() && doc.is_none() {
    return HttpResponse::BadRequest().body("Either code or doc parameter must be provided");
  }

  // 加载当前快照
  let mut snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => return e,
  };

  // 检查namespace是否存在
  if !snapshot.files.contains_key(&namespace) {
    return HttpResponse::NotFound().json(serde_json::json!({
      "error": format!("Namespace '{namespace}' not found")
    }));
  }

  // 获取namespace文件
  let file_in_snapshot = snapshot.files.get_mut(&namespace).unwrap();
  
  // 检查definition是否存在
  if !file_in_snapshot.defs.contains_key(&definition) {
    return HttpResponse::NotFound().json(serde_json::json!({
      "error": format!("Definition '{definition}' not found in namespace '{namespace}'")
    }));
  }

  // 获取现有的definition
  let def_entry = file_in_snapshot.defs.get_mut(&definition).unwrap();
  
  // 更新代码（如果提供）
  if let Some(new_code) = code {
    let parsed_code = match cirru_parser::parse(new_code) {
      Ok(cirru_list) => {
        if cirru_list.is_empty() {
          return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Code cannot be empty"
          }));
        }
        cirru_list[0].clone()
      },
      Err(e) => {
        return HttpResponse::BadRequest().json(serde_json::json!({
          "error": format!("Failed to parse code: {}", e)
        }));
      }
    };
    def_entry.code = parsed_code;
  }
  
  // 更新文档（如果提供）
  if let Some(new_doc) = doc {
    def_entry.doc = new_doc.to_string();
  }

  // 保存更新后的快照
  match save_snapshot(app_state, &snapshot) {
    Ok(_) => {
      let mut message = format!("Definition '{definition}' in namespace '{namespace}' updated successfully");
      if code.is_some() {
        message.push_str(" (code updated)");
      }
      if doc.is_some() {
        message.push_str(" (doc updated)");
      }
      HttpResponse::Ok().json(serde_json::json!({
        "message": message,
        "namespace": namespace,
        "definition": definition
      }))
    },
    Err(e) => e,
  }
}


#[actix_web::main]
async fn main() -> std::io::Result<()> {
  let args: Args = argh::from_env();

  let app_state = web::Data::new(AppState {
    compact_cirru_path: args.file,
  });

  println!(
    "Starting MCP server on port {} with file: {}",
    args.port, app_state.compact_cirru_path
  );

  HttpServer::new(move || App::new().app_data(app_state.clone()).service(discover).service(execute))
    .bind(("127.0.0.1", args.port))?
    .run()
    .await
}
