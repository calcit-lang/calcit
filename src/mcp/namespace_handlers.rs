use actix_web::HttpResponse;
use std::collections::HashMap;
use crate::snapshot::{self, Snapshot, CodeEntry, FileInSnapShot};
use super::tools::McpRequest;
use super::cirru_utils::{validate_cirru_structure, json_to_cirru};

/// 加载快照数据
fn load_snapshot(app_state: &super::AppState) -> Result<Snapshot, HttpResponse> {
  let content = match std::fs::read_to_string(&app_state.compact_cirru_path) {
    Ok(c) => c,
    Err(e) => return Err(HttpResponse::InternalServerError().body(format!("Failed to read compact.cirru: {e}"))),
  };

  let edn_data = match cirru_edn::parse(&content) {
    Ok(d) => d,
    Err(e) => return Err(HttpResponse::InternalServerError().body(format!("Failed to parse compact.cirru as EDN: {e}"))),
  };

  let snapshot: Snapshot = match snapshot::load_snapshot_data(&edn_data, &app_state.compact_cirru_path) {
    Ok(s) => s,
    Err(e) => return Err(HttpResponse::InternalServerError().body(format!("Failed to load snapshot: {e}"))),
  };

  Ok(snapshot)
}

/// 保存快照数据
fn save_snapshot(app_state: &super::AppState, snapshot: &Snapshot) -> Result<(), HttpResponse> {
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
    Err(e) => return Err(HttpResponse::InternalServerError().body(format!("Failed to format snapshot as Cirru: {e}"))),
  };

  // 写入文件
  match std::fs::write(compact_cirru_path, content) {
    Ok(_) => Ok(()),
    Err(e) => Err(HttpResponse::InternalServerError().body(format!("Failed to write compact.cirru: {e}"))),
  }
}

/// 添加新的命名空间
pub fn add_namespace(app_state: &super::AppState, req: McpRequest) -> HttpResponse {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("namespace parameter is missing or not a string"),
  };

  let ns_definition = req.parameters.get("ns_definition").cloned().unwrap_or(serde_json::json!(["ns", namespace]));

  // 验证 ns_definition 是否符合 Cirru 结构
  if let Err(e) = validate_cirru_structure(&ns_definition) {
    return HttpResponse::BadRequest().body(format!("Invalid ns_definition structure: {e}"));
  }

  let mut snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => return e,
  };

  // 检查命名空间是否已存在
  if snapshot.files.contains_key(&namespace) {
    return HttpResponse::Conflict().body(format!("Namespace '{namespace}' already exists"));
  }

  // 将 JSON 转换为 Cirru
  let ns_cirru = match json_to_cirru(&ns_definition) {
    Ok(c) => c,
    Err(e) => return HttpResponse::BadRequest().body(format!("Failed to convert ns_definition to Cirru: {e}")),
  };

  // 创建新的文件快照
  let file_snapshot = FileInSnapShot {
    ns: CodeEntry::from_code(ns_cirru),
    defs: HashMap::new(),
  };

  snapshot.files.insert(namespace.clone(), file_snapshot);

  // 保存快照
  if let Err(e) = save_snapshot(app_state, &snapshot) {
    return e;
  }

  HttpResponse::Ok().json(serde_json::json!({
    "message": format!("Namespace '{namespace}' created successfully")
  }))
}

/// 删除命名空间
pub fn delete_namespace(app_state: &super::AppState, req: McpRequest) -> HttpResponse {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("namespace parameter is missing or not a string"),
  };

  let mut snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => return e,
  };

  // 检查命名空间是否存在
  if !snapshot.files.contains_key(&namespace) {
    return HttpResponse::NotFound().body(format!("Namespace '{namespace}' not found"));
  }

  // 删除命名空间
  snapshot.files.remove(&namespace);

  // 保存快照
  if let Err(e) = save_snapshot(app_state, &snapshot) {
    return e;
  }

  HttpResponse::Ok().json(serde_json::json!({
    "message": format!("Namespace '{namespace}' deleted successfully")
  }))
}

/// 列出所有命名空间
pub fn list_namespaces(app_state: &super::AppState, _req: McpRequest) -> HttpResponse {
  let snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => return e,
  };

  let namespaces: Vec<String> = snapshot.files.keys().cloned().collect();

  HttpResponse::Ok().json(serde_json::json!({
    "namespaces": namespaces
  }))
}

/// 更新命名空间的导入声明
pub fn update_namespace_imports(app_state: &super::AppState, req: McpRequest) -> HttpResponse {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("namespace parameter is missing or not a string"),
  };

  let ns_definition = match req.parameters.get("ns_definition") {
    Some(def) => def.clone(),
    None => return HttpResponse::BadRequest().body("ns_definition parameter is missing"),
  };

  // 验证 ns_definition 是否符合 Cirru 结构
  if let Err(e) = validate_cirru_structure(&ns_definition) {
    return HttpResponse::BadRequest().body(format!("Invalid ns_definition structure: {e}"));
  }

  let mut snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => return e,
  };

  // 检查命名空间是否存在
  let file_data = match snapshot.files.get_mut(&namespace) {
    Some(data) => data,
    None => return HttpResponse::NotFound().body(format!("Namespace '{namespace}' not found")),
  };

  // 将 JSON 转换为 Cirru
  let ns_cirru = match json_to_cirru(&ns_definition) {
    Ok(c) => c,
    Err(e) => return HttpResponse::BadRequest().body(format!("Failed to convert ns_definition to Cirru: {e}")),
  };

  // 更新命名空间定义
  file_data.ns = CodeEntry::from_code(ns_cirru);

  // 保存快照
  if let Err(e) = save_snapshot(app_state, &snapshot) {
    return e;
  }

  HttpResponse::Ok().json(serde_json::json!({
    "message": format!("Namespace '{namespace}' imports updated successfully")
  }))
}