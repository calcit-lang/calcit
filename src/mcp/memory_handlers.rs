use crate::mcp::error_handling::{create_tool_execution_error, create_tool_success};
use crate::mcp::tools::{
  FeedbackToCalcitMcpServerRequest, ListCalcitWorkMemoryRequest, ReadCalcitWorkMemoryRequest, WriteCalcitWorkMemoryRequest,
};
use axum::response::Json as ResponseJson;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryEntry {
  preview: String,
  detail: String,
}

const MEMORY_FILE_PATH: &str = "~/.config/calcit/mcp-memory.json";
const FEEDBACK_DIR_PATH: &str = "~/.config/calcit/mcp-feedbacks/";

/// Expand tilde in path to home directory
fn expand_path(path: &str) -> String {
  if path.starts_with("~/") {
    if let Some(home) = dirs::home_dir() {
      return path.replacen("~", &home.to_string_lossy(), 1);
    }
  }
  path.to_string()
}

/// Ensure directory exists, create if not
fn ensure_dir_exists(dir_path: &str) -> Result<(), String> {
  let expanded_path = expand_path(dir_path);
  let path = Path::new(&expanded_path);
  if !path.exists() {
    fs::create_dir_all(path).map_err(|e| format!("Failed to create directory {expanded_path}: {e}"))?
  }
  Ok(())
}

/// Load memory from JSON file
fn load_memory() -> Result<HashMap<String, MemoryEntry>, String> {
  let expanded_path = expand_path(MEMORY_FILE_PATH);
  let path = Path::new(&expanded_path);

  if !path.exists() {
    // Create parent directory if it doesn't exist
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent).map_err(|e| format!("Failed to create config directory: {e}"))?;
    }
    // Create empty memory file
    let empty_memory = HashMap::new();
    save_memory(&empty_memory)?;
    return Ok(empty_memory);
  }

  let content = fs::read_to_string(&expanded_path).map_err(|e| format!("Failed to read memory file: {e}"))?;

  let memory: HashMap<String, MemoryEntry> = serde_json::from_str(&content).map_err(|e| format!("Failed to parse memory file: {e}"))?;

  Ok(memory)
}

/// Save memory to JSON file
fn save_memory(memory: &HashMap<String, MemoryEntry>) -> Result<(), String> {
  let expanded_path = expand_path(MEMORY_FILE_PATH);
  let content = serde_json::to_string_pretty(memory).map_err(|e| format!("Failed to serialize memory: {e}"))?;

  fs::write(&expanded_path, content).map_err(|e| format!("Failed to write memory file: {e}"))?;

  Ok(())
}

/// List all work memory entries
pub fn list_calcit_work_memory(_request: ListCalcitWorkMemoryRequest) -> ResponseJson<Value> {
  match load_memory() {
    Ok(memory) => {
      let entries: Vec<Value> = memory
        .iter()
        .map(|(key, entry)| {
          json!({
            "key": key,
            "preview": entry.preview,
            "detail_length": entry.detail.len()
          })
        })
        .collect();

      ResponseJson(create_tool_success(
        None,
        json!({
          "entries": entries,
          "total_count": memory.len()
        }),
      ))
    }
    Err(error) => ResponseJson(create_tool_execution_error(None, format!("Failed to list memory: {error}"))),
  }
}

/// Read work memory entry by key or search by keywords
pub fn read_calcit_work_memory(request: ReadCalcitWorkMemoryRequest) -> ResponseJson<Value> {
  match load_memory() {
    Ok(memory) => {
      if let Some(key) = request.key {
        // Read specific key
        if let Some(entry) = memory.get(&key) {
          ResponseJson(create_tool_success(
            None,
            json!({
              "key": key,
              "preview": entry.preview,
              "detail": entry.detail
            }),
          ))
        } else {
          ResponseJson(create_tool_execution_error(None, format!("Memory entry '{key}' not found")))
        }
      } else if let Some(keywords) = request.keywords {
        // Search by keywords
        let keywords_lower = keywords.to_lowercase();
        let matches: Vec<Value> = memory
          .iter()
          .filter(|(key, entry)| {
            key.to_lowercase().contains(&keywords_lower)
              || entry.preview.to_lowercase().contains(&keywords_lower)
              || entry.detail.to_lowercase().contains(&keywords_lower)
          })
          .map(|(key, entry)| {
            json!({
              "key": key,
              "preview": entry.preview,
              "detail": entry.detail
            })
          })
          .collect();

        ResponseJson(create_tool_success(
          None,
          json!({
            "matches": matches,
            "search_keywords": keywords,
            "match_count": matches.len()
          }),
        ))
      } else {
        ResponseJson(create_tool_execution_error(
          None,
          "Either 'key' or 'keywords' parameter is required".to_string(),
        ))
      }
    }
    Err(error) => ResponseJson(create_tool_execution_error(None, format!("Failed to read memory: {error}"))),
  }
}

/// Write or update a work memory entry
pub fn write_calcit_work_memory(request: WriteCalcitWorkMemoryRequest) -> ResponseJson<Value> {
  match load_memory() {
    Ok(mut memory) => {
      let was_existing = memory.contains_key(&request.key);

      // Create preview (first 100 characters or less)
      let preview = if request.content.chars().count() > 100 {
        let truncated: String = request.content.chars().take(97).collect();
        format!("{truncated}...")
      } else {
        request.content.clone()
      };

      let entry = MemoryEntry {
        preview,
        detail: request.content.clone(),
      };

      memory.insert(request.key.clone(), entry);

      match save_memory(&memory) {
        Ok(()) => {
          let action = if was_existing { "updated" } else { "created" };
          ResponseJson(create_tool_success(
            None,
            json!({
              "key": request.key,
              "action": action,
              "content_length": request.content.len()
            }),
          ))
        }
        Err(error) => ResponseJson(create_tool_execution_error(None, format!("Failed to save memory: {error}"))),
      }
    }
    Err(error) => ResponseJson(create_tool_execution_error(None, format!("Failed to load memory: {error}"))),
  }
}

/// Provide feedback about MCP server usage
pub fn feedback_to_calcit_mcp_server(request: FeedbackToCalcitMcpServerRequest) -> ResponseJson<Value> {
  let expanded_dir = expand_path(FEEDBACK_DIR_PATH);

  // Ensure feedback directory exists
  if let Err(error) = ensure_dir_exists(&expanded_dir) {
    return ResponseJson(create_tool_execution_error(
      None,
      format!("Failed to create feedback directory: {error}"),
    ));
  }

  // Generate timestamp-based filename
  let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
  let filename = format!("{timestamp}_feedback.md");
  let file_path = Path::new(&expanded_dir).join(&filename);

  // Create feedback content
  let feedback_content = format!(
    "# MCP Server Feedback - {}

## Timestamp
{}

## Feedback Content
{}

## Context
- Generated by: feedback_to_calcit_mcp_server tool
- Purpose: Improve MCP server functionality and reduce future issues
",
    timestamp,
    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
    request.feedback
  );

  // Write feedback file
  match fs::write(&file_path, feedback_content) {
    Ok(()) => ResponseJson(create_tool_success(
      None,
      json!({
        "feedback_file": file_path.to_string_lossy(),
        "timestamp": timestamp,
        "content_length": request.feedback.len()
      }),
    )),
    Err(error) => ResponseJson(create_tool_execution_error(None, format!("Failed to write feedback file: {error}"))),
  }
}
