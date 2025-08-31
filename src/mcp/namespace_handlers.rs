use super::cirru_utils::{json_to_cirru, validate_cirru_structure};
use super::tools::McpRequest;
use crate::snapshot::{self, CodeEntry, FileInSnapShot, Snapshot};
use axum::response::Json as ResponseJson;
use serde_json::Value;
use std::collections::HashMap;

/// 加载快照数据，包括主文件和所有模块文件
pub fn load_snapshot(app_state: &super::AppState) -> Result<Snapshot, String> {
  use std::path::Path;
  
  let content = match std::fs::read_to_string(&app_state.compact_cirru_path) {
    Ok(c) => c,
    Err(e) => return Err(format!("Failed to read compact.cirru: {e}")),
  };

  let edn_data = match cirru_edn::parse(&content) {
    Ok(d) => d,
    Err(e) => return Err(format!("Failed to parse compact.cirru as EDN: {e}")),
  };

  let mut main_snapshot = match snapshot::load_snapshot_data(&edn_data, &app_state.compact_cirru_path) {
    Ok(snapshot) => snapshot,
    Err(e) => return Err(format!("Failed to load snapshot: {e}")),
  };

  // 加载所有模块文件并合并命名空间
  let base_dir = Path::new(&app_state.compact_cirru_path).parent().unwrap_or(Path::new("."));
  let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
  let module_folder = Path::new(&home_dir).join(".config/calcit/modules"); // 使用标准的 calcit 模块目录
  
  println!("Loading modules from main snapshot, found {} modules", main_snapshot.configs.modules.len());
  
  for module_path in &main_snapshot.configs.modules {
    println!("Attempting to load module: {}", module_path);
    match crate::load_module(module_path, base_dir, &module_folder) {
      Ok(module_snapshot) => {
        println!("Successfully loaded module {}, found {} namespaces", module_path, module_snapshot.files.len());
        // 合并模块的文件到主快照中
        for (namespace, file_entry) in module_snapshot.files {
          println!("Adding namespace: {}", namespace);
          main_snapshot.files.insert(namespace, file_entry);
        }
        // 合并模块的 entries
        for (entry_name, entry_config) in module_snapshot.entries {
          main_snapshot.entries.insert(entry_name, entry_config);
        }
      },
      Err(e) => {
        println!("Warning: Failed to load module {}: {}", module_path, e);
        // 继续加载其他模块，不因为一个模块失败而停止
      }
    }
  }
  
  println!("Final snapshot has {} namespaces", main_snapshot.files.len());

  Ok(main_snapshot)
}

/// 保存快照数据
fn save_snapshot(app_state: &super::AppState, snapshot: &Snapshot) -> Result<(), ResponseJson<Value>> {
  let compact_cirru_path = &app_state.compact_cirru_path;

  // 构建根级别的 Edn 映射
  let mut edn_map = cirru_edn::EdnMapView::default();

  // 构建package
  edn_map.insert_key("package", cirru_edn::Edn::Str(snapshot.package.as_str().into()));

  // 构建configs
  let mut configs_map = cirru_edn::EdnMapView::default();
  configs_map.insert_key("init-fn", cirru_edn::Edn::Str(snapshot.configs.init_fn.as_str().into()));
  configs_map.insert_key("reload-fn", cirru_edn::Edn::Str(snapshot.configs.reload_fn.as_str().into()));
  configs_map.insert_key("version", cirru_edn::Edn::Str(snapshot.configs.version.as_str().into()));
  configs_map.insert_key(
    "modules",
    cirru_edn::Edn::from(
      snapshot
        .configs
        .modules
        .iter()
        .map(|s| cirru_edn::Edn::Str(s.as_str().into()))
        .collect::<Vec<_>>(),
    ),
  );
  edn_map.insert_key("configs", configs_map.into());

  // 构建entries
  let mut entries_map = cirru_edn::EdnMapView::default();
  for (k, v) in &snapshot.entries {
    let mut entry_map = cirru_edn::EdnMapView::default();
    entry_map.insert_key("init-fn", cirru_edn::Edn::Str(v.init_fn.as_str().into()));
    entry_map.insert_key("reload-fn", cirru_edn::Edn::Str(v.reload_fn.as_str().into()));
    entry_map.insert_key("version", cirru_edn::Edn::Str(v.version.as_str().into()));
    entry_map.insert_key(
      "modules",
      cirru_edn::Edn::from(v.modules.iter().map(|s| cirru_edn::Edn::Str(s.as_str().into())).collect::<Vec<_>>()),
    );
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
    Err(e) => {
      return Err(ResponseJson(serde_json::json!({
        "error": format!("Failed to format snapshot as Cirru: {e}")
      })));
    }
  };

  // 写入文件
  match std::fs::write(compact_cirru_path, content) {
    Ok(_) => Ok(()),
    Err(e) => Err(ResponseJson(serde_json::json!({
      "error": format!("Failed to write compact.cirru: {e}")
    }))),
  }
}

pub fn add_namespace(app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "namespace parameter is missing or not a string"
      }));
    }
  };

  let mut snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  // 检查命名空间是否已存在
  if snapshot.files.contains_key(&namespace) {
    return ResponseJson(serde_json::json!({
      "error": format!("Namespace '{namespace}' already exists")
    }));
  }

  // 创建新的命名空间文件
  let new_file = FileInSnapShot {
    ns: CodeEntry::from_code(cirru_parser::Cirru::from(vec!["ns", &namespace])),
    defs: HashMap::new(),
  };

  snapshot.files.insert(namespace.clone(), new_file);

  // 保存快照
  if let Err(e) = save_snapshot(app_state, &snapshot) {
    return e;
  }

  ResponseJson(serde_json::json!({
    "message": format!("Namespace '{namespace}' created successfully")
  }))
}

pub fn delete_namespace(app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "namespace parameter is missing or not a string"
      }));
    }
  };

  let mut snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  // 检查命名空间是否存在
  if !snapshot.files.contains_key(&namespace) {
    return ResponseJson(serde_json::json!({
      "error": format!("Namespace '{namespace}' not found")
    }));
  }

  // 删除命名空间
  snapshot.files.remove(&namespace);

  // 保存快照
  if let Err(e) = save_snapshot(app_state, &snapshot) {
    return e;
  }

  ResponseJson(serde_json::json!({
    "message": format!("Namespace '{namespace}' deleted successfully")
  }))
}

pub fn list_namespaces(app_state: &super::AppState, _req: McpRequest) -> ResponseJson<Value> {
  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  let namespaces: Vec<String> = snapshot.files.keys().cloned().collect();

  ResponseJson(serde_json::json!({
    "namespaces": namespaces
  }))
}

pub fn update_namespace_imports(app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "namespace parameter is missing or not a string"
      }));
    }
  };

  let ns_definition = match req.parameters.get("ns_definition") {
    Some(def) => def.clone(),
    None => {
      return ResponseJson(serde_json::json!({
        "error": "ns_definition parameter is missing"
      }));
    }
  };

  // 验证 ns_definition 是否符合 Cirru 结构
  if let Err(e) = validate_cirru_structure(&ns_definition) {
    return ResponseJson(serde_json::json!({
      "error": format!("Invalid ns_definition structure: {e}")
    }));
  }

  let mut snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  // 检查命名空间是否存在
  let file_data = match snapshot.files.get_mut(&namespace) {
    Some(data) => data,
    None => {
      return ResponseJson(serde_json::json!({
        "error": format!("Namespace '{namespace}' not found")
      }));
    }
  };

  // 将 JSON 转换为 Cirru
  let ns_cirru = match json_to_cirru(&ns_definition) {
    Ok(c) => c,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": format!("Failed to convert ns_definition to Cirru: {e}")
      }));
    }
  };

  // 更新命名空间定义
  file_data.ns = CodeEntry::from_code(ns_cirru);

  // 保存快照
  if let Err(e) = save_snapshot(app_state, &snapshot) {
    return e;
  }

  ResponseJson(serde_json::json!({
    "message": format!("Namespace '{namespace}' imports updated successfully")
  }))
}
