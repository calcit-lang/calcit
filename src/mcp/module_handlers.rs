use actix_web::HttpResponse;
use crate::snapshot::{self, Snapshot};
use super::tools::McpRequest;

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

  Ok(snapshot.package)
}

/// 加载依赖模块
fn load_dependency_module(module_path: &str, base_dir: &std::path::Path) -> Result<Snapshot, String> {
  let full_path = if module_path.starts_with('/') {
    std::path::PathBuf::from(module_path)
  } else {
    base_dir.join(module_path)
  };

  let compact_file = full_path.join("compact.cirru");
  
  if !compact_file.exists() {
    return Err(format!("Module compact.cirru not found at: {}", compact_file.display()));
  }

  let content = match std::fs::read_to_string(&compact_file) {
    Ok(c) => c,
    Err(e) => return Err(format!("Failed to read compact.cirru: {e}")),
  };

  let edn_data = match cirru_edn::parse(&content) {
    Ok(d) => d,
    Err(e) => return Err(format!("Failed to parse compact.cirru as EDN: {e}")),
  };

  let snapshot: Snapshot = match snapshot::load_snapshot_data(&edn_data, compact_file.to_str().unwrap()) {
    Ok(s) => s,
    Err(e) => return Err(format!("Failed to load module {module_path}: {e}")),
  };

  Ok(snapshot)
}

/// 获取依赖模块（带缓存）
fn get_dependency_module(app_state: &super::AppState, module_path: &str) -> Result<Snapshot, String> {
  // 检查缓存
  {
    let cache = app_state.module_cache.read().unwrap();
    if let Some(cached_snapshot) = cache.get(module_path) {
      return Ok(cached_snapshot.clone());
    }
  }

  // 获取基础目录
  let base_dir = std::path::Path::new(&app_state.compact_cirru_path)
    .parent()
    .unwrap_or_else(|| std::path::Path::new("."));

  // 加载模块
  let snapshot = load_dependency_module(module_path, base_dir)?;

  // 缓存结果
  {
    let mut cache = app_state.module_cache.write().unwrap();
    cache.insert(module_path.to_string(), snapshot.clone());
  }

  Ok(snapshot)
}

/// 列出所有可用的模块
pub fn list_modules(app_state: &super::AppState, _req: McpRequest) -> HttpResponse {
  let current_module = match load_current_module_name(&app_state.compact_cirru_path) {
    Ok(name) => name,
    Err(e) => return HttpResponse::InternalServerError().body(format!("Failed to get current module: {e}")),
  };

  // 获取缓存中的模块列表
  let cached_modules: Vec<String> = {
    let cache = app_state.module_cache.read().unwrap();
    cache.keys().cloned().collect()
  };

  let mut modules = vec![serde_json::json!({
    "name": current_module,
    "type": "current",
    "cached": false
  })];

  for module_name in cached_modules {
    modules.push(serde_json::json!({
      "name": module_name,
      "type": "dependency",
      "cached": true
    }));
  }

  HttpResponse::Ok().json(serde_json::json!({
    "modules": modules
  }))
}

/// 读取指定模块的信息
pub fn read_module(app_state: &super::AppState, req: McpRequest) -> HttpResponse {
  let module_name = match req.parameters.get("module") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("module parameter is missing or not a string"),
  };

  // 如果是当前模块
  if module_name == app_state.current_module_name {
    let content = match std::fs::read_to_string(&app_state.compact_cirru_path) {
      Ok(c) => c,
      Err(e) => return HttpResponse::InternalServerError().body(format!("Failed to read compact.cirru: {e}")),
    };

    let edn_data = match cirru_edn::parse(&content) {
      Ok(d) => d,
      Err(e) => return HttpResponse::InternalServerError().body(format!("Failed to parse compact.cirru as EDN: {e}")),
    };

    match snapshot::load_snapshot_data(&edn_data, &app_state.compact_cirru_path) {
      Ok(snapshot) => {
        let namespaces: Vec<String> = snapshot.files.keys().cloned().collect();
        return HttpResponse::Ok().json(serde_json::json!({
          "module": module_name,
          "type": "current",
          "namespaces": namespaces
        }));
      }
      Err(e) => return HttpResponse::InternalServerError().body(format!("Failed to load current module: {e}")),
    }
  }

  // 尝试加载依赖模块
  match get_dependency_module(app_state, &module_name) {
    Ok(snapshot) => {
      let namespaces: Vec<String> = snapshot.files.keys().cloned().collect();
      HttpResponse::Ok().json(serde_json::json!({
        "module": module_name,
        "type": "dependency",
        "namespaces": namespaces
      }))
    }
    Err(e) => HttpResponse::NotFound().body(format!("Module '{module_name}' not found: {e}")),
  }
}

/// 添加模块依赖
pub fn add_module_dependency(app_state: &super::AppState, req: McpRequest) -> HttpResponse {
  let module_path = match req.parameters.get("module_path") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("module_path parameter is missing or not a string"),
  };

  match get_dependency_module(app_state, &module_path) {
    Ok(_) => HttpResponse::Ok().json(serde_json::json!({
      "message": format!("Module dependency '{module_path}' added and cached successfully")
    })),
    Err(e) => HttpResponse::BadRequest().body(format!("Failed to add module dependency: {e}")),
  }
}

/// 移除模块依赖
pub fn remove_module_dependency(app_state: &super::AppState, req: McpRequest) -> HttpResponse {
  let module_path = match req.parameters.get("module_path") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("module_path parameter is missing or not a string"),
  };

  {
    let mut cache = app_state.module_cache.write().unwrap();
    if cache.remove(&module_path).is_some() {
      HttpResponse::Ok().json(serde_json::json!({
        "message": format!("Module dependency '{module_path}' removed from cache successfully")
      }))
    } else {
      HttpResponse::NotFound().body(format!("Module dependency '{module_path}' not found in cache"))
    }
  }
}

/// 清空模块缓存
pub fn clear_module_cache(app_state: &super::AppState, _req: McpRequest) -> HttpResponse {
  {
    let mut cache = app_state.module_cache.write().unwrap();
    let count = cache.len();
    cache.clear();
    HttpResponse::Ok().json(serde_json::json!({
      "message": format!("Module cache cleared successfully. Removed {count} cached modules.")
    }))
  }
}