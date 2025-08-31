use super::tools::McpRequest;
use axum::response::Json as ResponseJson;
use cirru_edn::Edn;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

///// 全局缓存
static API_DOCS_CACHE: OnceLock<Arc<Mutex<HashMap<String, ApiDoc>>>> = OnceLock::new();
static GUIDEBOOK_CACHE: OnceLock<Arc<Mutex<HashMap<String, GuideDoc>>>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiDoc {
  name: String,
  desc: String,
  tags: HashSet<String>,
  snippets: Vec<Edn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuideDoc {
  filename: String,
  path: String,
  content: String,
}

/// 加载 API 文档数据
pub fn load_api_docs() -> Result<HashMap<String, ApiDoc>, String> {
  let home_dir = std::env::var("HOME").map_err(|_| "无法获取 HOME 环境变量")?;
  let docs_dir = Path::new(&home_dir).join(".config/calcit/apis-repo/docs");

  if !docs_dir.exists() {
    return Err(format!(
      "API documentation directory not found: {docs_dir:?}\n\nTo set up API documentation, please run:\n1. Clone calcit-lang/apis-repo repository to ~/.config/calcit/apis-repo\n2. Or run: git clone https://github.com/calcit-lang/apis-repo.git ~/.config/calcit/apis-repo\n\nThis will provide official Calcit language API documentation data."
    ));
  }

  let mut api_docs = HashMap::new();

  for entry in fs::read_dir(&docs_dir).map_err(|e| format!("读取目录失败: {e}"))? {
    let entry = entry.map_err(|e| format!("读取目录项失败: {e}"))?;
    let path = entry.path();

    if path.extension().and_then(|s| s.to_str()) == Some("cirru") {
      let content = fs::read_to_string(&path).map_err(|e| format!("Failed to read file {path:?}: {e}"))?;
      let edn_data = cirru_edn::parse(&content).map_err(|e| format!("Failed to parse Cirru EDN {path:?}: {e}"))?;

      // First try to parse as Vec, then parse each element as ApiDoc
      match cirru_edn::from_edn::<Vec<Edn>>(edn_data) {
        Ok(vec_data) => {
          for item in vec_data {
            match cirru_edn::from_edn::<ApiDoc>(item.to_owned()) {
              Ok(doc) => {
                api_docs.insert(doc.name.clone(), doc);
              }
              Err(e) => {
                eprintln!(
                  "Warning: Failed to parse API doc item in {path:?}: {e}, item: {}",
                  cirru_edn::format(&item, true).unwrap()
                );
              }
            }
          }
        }
        Err(_) => {
          // Fallback: try to parse directly as ApiDoc
          match cirru_edn::from_edn::<ApiDoc>(
            cirru_edn::parse(&content).map_err(|e| format!("Failed to parse Cirru EDN {path:?}: {e}"))?,
          ) {
            Ok(doc) => {
              api_docs.insert(doc.name.clone(), doc);
            }
            Err(e) => {
              eprintln!("Warning: Skipping unparseable API documentation file {path:?}: {e}");
            }
          }
        }
      }
    }
  }

  Ok(api_docs)
}

/// 加载指南文档数据
pub fn load_guidebook_docs() -> Result<HashMap<String, GuideDoc>, String> {
  let home_dir = std::env::var("HOME").map_err(|_| "无法获取 HOME 环境变量")?;
  let docs_dir = Path::new(&home_dir).join(".config/calcit/guidebook-repo/docs");

  if !docs_dir.exists() {
    return Err(format!(
      "Guidebook documentation directory not found: {docs_dir:?}\n\nTo set up guidebook documentation, please run:\n1. Clone calcit-lang/guidebook-repo repository to ~/.config/calcit/guidebook-repo\n2. Or run: git clone https://github.com/calcit-lang/guidebook-repo.git ~/.config/calcit/guidebook-repo\n\nThis will provide official Calcit language tutorial and guide documentation."
    ));
  }

  let mut guide_docs = HashMap::new();

  fn visit_dir(dir: &Path, base_dir: &Path, docs: &mut HashMap<String, GuideDoc>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("读取目录失败: {e}"))? {
      let entry = entry.map_err(|e| format!("读取目录项失败: {e}"))?;
      let path = entry.path();

      if path.is_dir() {
        visit_dir(&path, base_dir, docs)?;
      } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
        let content = fs::read_to_string(&path).map_err(|e| format!("Failed to read file {path:?}: {e}"))?;

        let relative_path = path.strip_prefix(base_dir).map_err(|_| "Unable to get relative path")?;

        let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();

        docs.insert(
          filename.clone(),
          GuideDoc {
            filename: filename.clone(),
            path: relative_path.to_string_lossy().to_string(),
            content,
          },
        );
      }
    }
    Ok(())
  }

  visit_dir(&docs_dir, &docs_dir, &mut guide_docs)?;
  Ok(guide_docs)
}

/// Ensure data is loaded
/// Generic cache initialization function
fn ensure_cache_loaded<T>(
  cache: &OnceLock<Arc<Mutex<HashMap<String, T>>>>,
  loader: fn() -> Result<HashMap<String, T>, String>,
) -> Result<(), String> {
  let cache_ref = cache.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));
  let mut cache_guard = cache_ref.lock().unwrap();
  if cache_guard.is_empty() {
    *cache_guard = loader()?;
  }
  Ok(())
}

fn ensure_data_loaded() -> Result<(), String> {
  // Initialize API documentation cache
  ensure_cache_loaded(&API_DOCS_CACHE, load_api_docs)?;

  // Initialize guidebook documentation cache
  ensure_cache_loaded(&GUIDEBOOK_CACHE, load_guidebook_docs)?;

  Ok(())
}

/// Handle API documentation queries
pub fn handle_query_api_docs(_app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  // Ensure data is loaded and get cache
  let api_cache = match ensure_data_loaded() {
    Ok(_) => API_DOCS_CACHE.get().unwrap(),
    Err(e) => {
      return ResponseJson(serde_json::json!({
          "error": format!("Failed to load data: {}", e)
      }));
    }
  };
  let cache = api_cache.lock().unwrap();
  let api_docs = &*cache;

  let query_type = req.parameters.get("query_type").and_then(|v| v.as_str()).unwrap_or("all");

  let results: Vec<ApiDoc> = match query_type {
    "all" => api_docs.values().cloned().collect(),
    "tag" => {
      let tag = req.parameters.get("query_value").and_then(|v| v.as_str()).unwrap_or("");
      if tag.is_empty() {
        return ResponseJson(serde_json::json!({
          "error": "query_value parameter is required for tag queries"
        }));
      }
      api_docs
        .values()
        .filter(|doc| doc.tags.iter().any(|t| t.contains(tag)))
        .cloned()
        .collect()
    }
    "keyword" => {
      let keyword = req.parameters.get("query_value").and_then(|v| v.as_str()).unwrap_or("");
      if keyword.is_empty() {
        return ResponseJson(serde_json::json!({
          "error": "query_value parameter is required for keyword queries"
        }));
      }
      api_docs
        .values()
        .filter(|doc| doc.name.contains(keyword) || doc.desc.contains(keyword) || doc.tags.iter().any(|t| t.contains(keyword)))
        .cloned()
        .collect()
    }
    _ => {
      return ResponseJson(serde_json::json!({
        "error": format!("Invalid query_type '{}'. Valid types are: 'all', 'tag', 'keyword'", query_type)
      }));
    }
  };

  let response_data: Vec<Value> = results
    .iter()
    .map(|doc| {
      serde_json::json!({
          "name": doc.name,
          "desc": doc.desc,
          "tags": doc.tags,
          "snippets": doc.snippets
      })
    })
    .collect();

  ResponseJson(serde_json::json!({
      "results": response_data,
      "count": response_data.len()
  }))
}

/// Handle list_api_docs tool request
pub fn handle_list_api_docs(_app_state: &super::AppState, _req: McpRequest) -> ResponseJson<Value> {
  if let Err(e) = ensure_data_loaded() {
    return ResponseJson(serde_json::json!({
      "error": e
    }));
  }

  let api_docs_cache = API_DOCS_CACHE.get().unwrap();
  let api_docs = api_docs_cache.lock().unwrap();

  let response_data: Vec<String> = api_docs.keys().cloned().collect();

  let mut sorted_data = response_data;
  sorted_data.sort();

  ResponseJson(serde_json::json!({
    "results": sorted_data,
    "count": sorted_data.len(),
    "message": "List of all available API documentation"
  }))
}

/// Handle list_guidebook_docs tool request
pub fn handle_list_guidebook_docs(_app_state: &super::AppState, _req: McpRequest) -> ResponseJson<Value> {
  if let Err(e) = ensure_data_loaded() {
    return ResponseJson(serde_json::json!({
      "error": e
    }));
  }

  let guidebook_cache = GUIDEBOOK_CACHE.get().unwrap();
  let guide_docs = guidebook_cache.lock().unwrap();

  let response_data: Vec<Value> = guide_docs
    .values()
    .map(|doc| {
      let content_preview = if doc.content.len() > 200 {
        format!("{}...", doc.content.get(..200).unwrap_or(&doc.content))
      } else {
        doc.content.clone()
      };

      serde_json::json!({
        "filename": doc.filename,
        "path": doc.path,
        "content_preview": content_preview,
        "content_length": doc.content.len()
      })
    })
    .collect();

  ResponseJson(serde_json::json!({
    "results": response_data,
    "count": response_data.len(),
    "message": "List of all available guidebook documentation"
  }))
}

/// Handle guidebook documentation queries
pub fn handle_query_guidebook(_app_state: &super::AppState, req: McpRequest) -> ResponseJson<Value> {
  // Ensure data is loaded and get cache
  let guide_cache = match ensure_data_loaded() {
    Ok(_) => GUIDEBOOK_CACHE.get().unwrap(),
    Err(e) => {
      return ResponseJson(serde_json::json!({
          "error": format!("Failed to load data: {}", e)
      }));
    }
  };
  let cache = guide_cache.lock().unwrap();
  let guide_docs = &*cache;

  let query_type = req.parameters.get("query_type").and_then(|v| v.as_str()).unwrap_or("all");

  let results: Vec<GuideDoc> = match query_type {
    "all" => guide_docs.values().cloned().collect(),
    "filename" => {
      let filename = req.parameters.get("query_value").and_then(|v| v.as_str()).unwrap_or("");
      if filename.is_empty() {
        return ResponseJson(serde_json::json!({
          "error": "query_value parameter is required for filename queries"
        }));
      }
      guide_docs
        .values()
        .filter(|doc| doc.filename.contains(filename) || doc.path.contains(filename))
        .cloned()
        .collect()
    }
    "keyword" => {
      let keyword = req.parameters.get("query_value").and_then(|v| v.as_str()).unwrap_or("");
      if keyword.is_empty() {
        return ResponseJson(serde_json::json!({
          "error": "query_value parameter is required for keyword queries"
        }));
      }
      guide_docs
        .values()
        .filter(|doc| doc.filename.contains(keyword) || doc.path.contains(keyword) || doc.content.contains(keyword))
        .cloned()
        .collect()
    }
    _ => {
      return ResponseJson(serde_json::json!({
        "error": format!("Invalid query_type '{}'. Valid types are: 'all', 'filename', 'keyword'", query_type)
      }));
    }
  };

  let response_data: Vec<Value> = results
    .iter()
    .map(|doc| {
      serde_json::json!({
          "filename": doc.filename,
          "path": doc.path,
          "content": doc.content
      })
    })
    .collect();

  ResponseJson(serde_json::json!({
      "results": response_data,
      "count": response_data.len()
  }))
}
