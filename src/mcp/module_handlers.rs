use super::tools::McpRequest;
use crate::snapshot::{self, Snapshot};
use axum::response::Json as ResponseJson;
use serde_json::Value;

/// 加载当前模块名称
fn load_current_module_name(compact_cirru_path: &str) -> Result<String, String> {
  let content = match std::fs::read_to_string(compact_cirru_path) {
    Ok(c) => c,
    Err(e) => return Err(format!("Failed to read compact.cirru: {e}")),
  };

  let edn_data = match cirru_edn::parse(&content) {
    Ok(d) => d,
    Err(e) => return Err(format!("Failed to parse compact.cirru as EDN: {e}")),
  };

  let snapshot: Snapshot = match snapshot::load_snapshot_data(&edn_data, compact_cirru_path) {
    Ok(s) => s,
    Err(e) => return Err(format!("Failed to load current module name: {e}")),
  };

  Ok(snapshot.package.clone())
}

/// 加载快照数据
fn load_snapshot(app_state: &super::AppState) -> Result<Snapshot, String> {
  let compact_cirru_path = &app_state.compact_cirru_path;
  let content = match std::fs::read_to_string(compact_cirru_path) {
    Ok(c) => c,
    Err(e) => return Err(format!("Failed to read compact.cirru: {e}")),
  };

  let edn_data = match cirru_edn::parse(&content) {
    Ok(d) => d,
    Err(e) => return Err(format!("Failed to parse compact.cirru as EDN: {e}")),
  };

  match snapshot::load_snapshot_data(&edn_data, compact_cirru_path) {
    Ok(snapshot) => Ok(snapshot),
    Err(e) => Err(format!("Failed to load snapshot: {e}")),
  }
}

pub fn get_current_module(app_state: &super::AppState, _req: McpRequest) -> ResponseJson<Value> {
  match load_current_module_name(&app_state.compact_cirru_path) {
    Ok(module_name) => ResponseJson(serde_json::json!({
      "module": module_name
    })),
    Err(e) => ResponseJson(serde_json::json!({
      "error": e
    })),
  }
}

pub fn list_modules(app_state: &super::AppState, _req: McpRequest) -> ResponseJson<Value> {
  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  // 收集所有模块：主包 + entries 中的模块
  let mut modules: Vec<serde_json::Value> = vec![serde_json::json!({
    "name": snapshot.package,
    "type": "main_package",
    "init_fn": snapshot.configs.init_fn,
    "reload_fn": snapshot.configs.reload_fn,
    "version": snapshot.configs.version
  })];

  // 添加其他模块
  for (module_name, config) in &snapshot.entries {
    modules.push(serde_json::json!({
      "name": module_name,
      "type": "module",
      "init_fn": config.init_fn,
      "reload_fn": config.reload_fn,
      "version": config.version
    }));
  }

  ResponseJson(serde_json::json!({
    "modules": modules,
    "total_count": modules.len()
  }))
}

pub fn switch_module(app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let module_name = match req.parameters.get("module") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "module parameter is missing or not a string"
      }));
    }
  };

  // 加载快照以验证模块是否存在
  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  // 检查模块是否存在（这里简化为检查是否为当前包名）
  if module_name != snapshot.package {
    return ResponseJson(serde_json::json!({
      "error": format!("Module '{}' not found", module_name)
    }));
  }

  // 在实际实现中，这里应该更新当前模块状态
  // 目前只是验证模块存在性
  ResponseJson(serde_json::json!({
    "message": format!("Switched to module: {}", module_name),
    "current_module": module_name
  }))
}

pub fn create_module(app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let module_name = match req.parameters.get("name") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "name parameter is missing or not a string"
      }));
    }
  };

  // 验证模块名称
  if module_name.is_empty() {
    return ResponseJson(serde_json::json!({
      "error": "Module name cannot be empty"
    }));
  }

  // 加载当前快照
  let mut snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  // 检查模块是否已存在
  if snapshot.entries.contains_key(&module_name) {
    return ResponseJson(serde_json::json!({
      "error": format!("Module '{}' already exists", module_name)
    }));
  }

  // 创建新模块配置
  let new_module_config = crate::snapshot::SnapshotConfigs {
    init_fn: format!("{module_name}.main/main!"),
    reload_fn: format!("{module_name}.main/reload!"),
    version: "0.0.0".to_string(),
    modules: vec![],
  };

  snapshot.entries.insert(module_name.clone(), new_module_config);

  // 在实际实现中，这里应该保存快照
  // 目前只是模拟创建过程
  ResponseJson(serde_json::json!({
    "message": format!("Created module: {}", module_name),
    "module": module_name
  }))
}

pub fn delete_module(app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  let module_name = match req.parameters.get("module") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => {
      return ResponseJson(serde_json::json!({
        "error": "module parameter is missing or not a string"
      }));
    }
  };

  // 加载当前快照
  let mut snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => {
      return ResponseJson(serde_json::json!({
        "error": e
      }));
    }
  };

  // 检查模块是否存在
  if !snapshot.entries.contains_key(&module_name) {
    return ResponseJson(serde_json::json!({
      "error": format!("Module '{}' not found", module_name)
    }));
  }

  // 防止删除主模块
  if module_name == snapshot.package {
    return ResponseJson(serde_json::json!({
      "error": "Cannot delete the main package module"
    }));
  }

  // 删除模块
  snapshot.entries.remove(&module_name);

  // 在实际实现中，这里应该保存快照
  // 目前只是模拟删除过程
  ResponseJson(serde_json::json!({
    "message": format!("Deleted module: {}", module_name)
  }))
}
