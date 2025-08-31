use actix_web::HttpResponse;
use calcit::snapshot::{self, Snapshot, CodeEntry};
use crate::mcp::tools::McpRequest;
use crate::mcp::cirru_utils::{validate_cirru_structure, json_to_cirru};

/// 加载快照数据
fn load_snapshot(app_state: &crate::AppState) -> Result<Snapshot, HttpResponse> {
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
fn save_snapshot(app_state: &crate::AppState, snapshot: &Snapshot) -> Result<(), HttpResponse> {
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

/// 添加新的定义
pub fn add_definition(app_state: &crate::AppState, req: McpRequest) -> HttpResponse {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("namespace parameter is missing or not a string"),
  };

  let definition = match req.parameters.get("definition") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("definition parameter is missing or not a string"),
  };

  let code = match req.parameters.get("code") {
    Some(c) => c.clone(),
    None => return HttpResponse::BadRequest().body("code parameter is missing"),
  };

  let doc = req.parameters.get("doc")
    .and_then(|v| v.as_str())
    .unwrap_or("")
    .to_string();

  // 验证 code 是否符合 Cirru 结构
  if let Err(e) = validate_cirru_structure(&code) {
    return HttpResponse::BadRequest().body(format!("Invalid code structure: {e}"));
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

  // 检查定义是否已存在
  if file_data.defs.contains_key(&definition) {
    return HttpResponse::Conflict().body(format!("Definition '{definition}' already exists in namespace '{namespace}'"));
  }

  // 将 JSON 转换为 Cirru
  let code_cirru = match json_to_cirru(&code) {
    Ok(c) => c,
    Err(e) => return HttpResponse::BadRequest().body(format!("Failed to convert code to Cirru: {e}")),
  };

  // 创建新的定义条目
  let new_entry = CodeEntry {
    code: code_cirru,
    doc,
  };

  file_data.defs.insert(definition.clone(), new_entry);

  // 保存快照
  if let Err(e) = save_snapshot(app_state, &snapshot) {
    return e;
  }

  HttpResponse::Ok().json(serde_json::json!({
    "message": format!("Definition '{definition}' added to namespace '{namespace}' successfully")
  }))
}

/// 删除定义
pub fn delete_definition(app_state: &crate::AppState, req: McpRequest) -> HttpResponse {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("namespace parameter is missing or not a string"),
  };

  let definition = match req.parameters.get("definition") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("definition parameter is missing or not a string"),
  };

  let mut snapshot = match load_snapshot(app_state) {
    Ok(s) => s,
    Err(e) => return e,
  };

  // 检查命名空间是否存在
  let file_data = match snapshot.files.get_mut(&namespace) {
    Some(data) => data,
    None => return HttpResponse::NotFound().body(format!("Namespace '{namespace}' not found")),
  };

  // 检查定义是否存在
  if !file_data.defs.contains_key(&definition) {
    return HttpResponse::NotFound().body(format!("Definition '{definition}' not found in namespace '{namespace}'"));
  }

  // 删除定义
  file_data.defs.remove(&definition);

  // 保存快照
  if let Err(e) = save_snapshot(app_state, &snapshot) {
    return e;
  }

  HttpResponse::Ok().json(serde_json::json!({
    "message": format!("Definition '{definition}' deleted from namespace '{namespace}' successfully")
  }))
}

/// 更新定义
pub fn update_definition(app_state: &crate::AppState, req: McpRequest) -> HttpResponse {
  let namespace = match req.parameters.get("namespace") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("namespace parameter is missing or not a string"),
  };

  let definition = match req.parameters.get("definition") {
    Some(serde_json::Value::String(s)) => s.clone(),
    _ => return HttpResponse::BadRequest().body("definition parameter is missing or not a string"),
  };

  let code = req.parameters.get("code");
  let doc = req.parameters.get("doc").and_then(|v| v.as_str());

  // 至少需要提供 code 或 doc 中的一个
  if code.is_none() && doc.is_none() {
    return HttpResponse::BadRequest().body("At least one of 'code' or 'doc' parameters must be provided");
  }

  // 如果提供了 code，验证其结构
  if let Some(c) = code {
    if let Err(e) = validate_cirru_structure(c) {
      return HttpResponse::BadRequest().body(format!("Invalid code structure: {e}"));
    }
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

  // 检查定义是否存在
  let def_entry = match file_data.defs.get_mut(&definition) {
    Some(entry) => entry,
    None => return HttpResponse::NotFound().body(format!("Definition '{definition}' not found in namespace '{namespace}'"))
  };

  // 更新 code（如果提供）
  if let Some(c) = code {
    let code_cirru = match json_to_cirru(c) {
      Ok(cirru) => cirru,
      Err(e) => return HttpResponse::BadRequest().body(format!("Failed to convert code to Cirru: {e}")),
    };
    def_entry.code = code_cirru;
  }

  // 更新 doc（如果提供）
  if let Some(d) = doc {
    def_entry.doc = d.to_string();
  }

  // 保存快照
  if let Err(e) = save_snapshot(app_state, &snapshot) {
    return e;
  }

  HttpResponse::Ok().json(serde_json::json!({
    "message": format!("Definition '{definition}' in namespace '{namespace}' updated successfully")
  }))
}